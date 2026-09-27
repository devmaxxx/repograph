//! The binary over a JVM repository: `update` after a nested type or a member appears, disappears
//! or changes visibility writes the graph a fresh `build` of the same tree writes.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

/// Kotlin and Java are read behind an explicit glob until L7 puts them in the defaults. The globs go
/// in the child's environment so no `repograph.toml` is written.
fn ok(repo: &Path, args: &[&str]) {
    let out = Command::new(env!("CARGO_BIN_EXE_repograph"))
        .env_remove("REPOGRAPH_BENCH_REPO")
        .env_remove("REPOGRAPH_EMBED_MODEL")
        .env("REPOGRAPH_CODE_GLOBS", "**/*.kt **/*.java")
        .arg("--no-dense")
        .arg("--repo")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "{args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
}

fn write(repo: &Path, rel: &str, text: &str) {
    let p = repo.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

/// (source, target, kind, context).
type Edges = BTreeSet<(String, String, String, String)>;

fn edges(repo: &Path) -> Edges {
    let text = std::fs::read_to_string(repo.join(".repograph/graph.json")).unwrap();
    let graph: serde_json::Value = serde_json::from_str(&text).unwrap();
    let field = |e: &serde_json::Value, k: &str| e[k].as_str().unwrap_or_default().to_string();
    graph["edges"].as_array().unwrap().iter().map(|e| (field(e, "source"), field(e, "target"), field(e, "kind"), field(e, "context"))).collect()
}

/// Builds `before`, rewrites it to `after`, runs `update`, and returns that graph's edges with the
/// edges of a fresh build of `after`.
fn updated_and_built(before: &[(&str, &str)], after: &[(&str, &str)]) -> (Edges, Edges) {
    removed_updated_and_built(before, after, &[])
}

/// `updated_and_built`, with the files at `removed` deleted before the update and absent from the
/// fresh build.
fn removed_updated_and_built(before: &[(&str, &str)], after: &[(&str, &str)], removed: &[&str]) -> (Edges, Edges) {
    let old = tempfile::tempdir().unwrap();
    for (rel, text) in before {
        write(old.path(), rel, text);
    }
    ok(old.path(), &["build"]);
    for (rel, text) in after {
        write(old.path(), rel, text);
    }
    for rel in removed {
        std::fs::remove_file(old.path().join(rel)).unwrap();
    }
    ok(old.path(), &["update"]);
    let fresh = tempfile::tempdir().unwrap();
    for (rel, text) in before.iter().chain(after).filter(|(rel, _)| !removed.contains(rel)) {
        write(fresh.path(), rel, text);
    }
    ok(fresh.path(), &["build"]);
    (edges(old.path()), edges(fresh.path()))
}

const USE: (&str, &str) = ("shop/orders/Use.java", "package shop.orders;\n\nimport shop.billing.Invoice.Line;\n\nclass Use extends Line {}\n");
const BARE: (&str, &str) = ("shop/billing/Invoice.java", "package shop.billing;\n\npublic class Invoice {}\n");
const NESTED: (&str, &str) = ("shop/billing/Invoice.java", "package shop.billing;\n\npublic class Invoice {\n    public static class Line {}\n}\n");

#[test]
fn a_nested_type_added_reaches_the_unchanged_file_that_extends_it() {
    let (updated, built) = updated_and_built(&[BARE, USE], &[NESTED]);
    let extends = ("sym:shop/orders/Use.java::Use".to_string(), "sym:shop/billing/Invoice.java::Invoice.Line".to_string(), "Extends".to_string(), String::new());
    assert!(built.contains(&extends), "{built:?}");
    assert_eq!(updated, built);
}

#[test]
fn a_nested_type_removed_leaves_no_edge_to_it_after_update() {
    let (updated, built) = updated_and_built(&[NESTED, USE], &[BARE]);
    assert!(!updated.iter().any(|(_, to, _, _)| to.contains("Invoice.Line")), "{updated:?}");
    assert_eq!(updated, built);
}

#[test]
fn a_kotlin_member_added_reaches_the_file_that_imports_it() {
    let keys = |body: &str| ("shop/Keys.kt", format!("package shop\n\nobject Keys {{\n{body}}}\n"));
    let (rel, before) = keys("");
    let (_, after) = keys("    const val TOKEN = \"t\"\n");
    let use_kt = ("app/Use.kt", "package app\n\nimport shop.Keys.TOKEN\n\nclass Use\n");
    let (updated, built) = updated_and_built(&[(rel, &before), use_kt], &[(rel, &after)]);
    assert!(built.iter().any(|(from, to, kind, _)| from == "file:app/Use.kt" && to == "file:shop/Keys.kt" && kind == "Imports"), "{built:?}");
    assert_eq!(updated, built);
}

#[test]
fn a_java_method_made_public_reaches_the_subclass_in_another_file_that_calls_it() {
    let base = |vis: &str| ("shop/Base.java", format!("package shop;\n\npublic class Base {{\n    {vis} void helper() {{}}\n}}\n"));
    let (rel, before) = base("private");
    let (_, after) = base("public");
    let sub = ("shop/Sub.java", "package shop;\n\nclass Sub extends Base {\n    void go() { helper(); }\n}\n");
    let (updated, built) = updated_and_built(&[(rel, &before), sub], &[(rel, &after)]);
    let call = ("sym:shop/Sub.java::Sub.go".to_string(), "sym:shop/Base.java::Base.helper".to_string(), "Calls".to_string(), String::new());
    assert!(built.contains(&call), "{built:?}");
    assert_eq!(updated, built);
}

#[test]
fn a_java_method_that_comes_to_take_a_call_s_arguments_is_bound_from_the_subclass_in_another_file() {
    let base = |params: &str| ("shop/Base.java", format!("package shop;\n\npublic class Base {{\n    public void helper({params}) {{}}\n}}\n"));
    let (rel, before) = base("int n");
    let (_, after) = base("");
    let sub = ("shop/Sub.java", "package shop;\n\nclass Sub extends Base {\n    void go() { helper(); }\n}\n");
    let (updated, built) = updated_and_built(&[(rel, &before), sub], &[(rel, &after)]);
    let call = ("sym:shop/Sub.java::Sub.go".to_string(), "sym:shop/Base.java::Base.helper".to_string(), "Calls".to_string(), String::new());
    assert!(built.contains(&call), "{built:?}");
    assert_eq!(updated, built);
}

#[test]
fn a_kotlin_member_made_public_reaches_the_subclass_in_another_file_that_calls_it() {
    let base = |vis: &str| ("app/A.kt", format!("package app\n\nopen class A {{\n    {vis} fun helper() {{}}\n}}\n"));
    let (rel, before) = base("private");
    let (_, after) = base("public");
    let sub = ("app/Sub.kt", "package app\n\nclass Sub : A() {\n    fun go() { helper() }\n}\n");
    let call = ("sym:app/Sub.kt::Sub.go".to_string(), "sym:app/A.kt::A.helper".to_string(), "Calls".to_string(), String::new());
    let (updated, built) = updated_and_built(&[(rel, &before), sub], &[(rel, &after)]);
    assert!(built.contains(&call), "{built:?}");
    assert_eq!(updated, built);
    let (updated, built) = updated_and_built(&[(rel, &after), sub], &[(rel, &before)]);
    assert!(!built.contains(&call), "{built:?}");
    assert_eq!(updated, built);
}

#[test]
fn a_kotlin_overload_made_public_beside_a_public_one_reaches_the_subclass_in_another_file() {
    let base = |vis: &str| ("app/A.kt", format!("package app\n\nopen class A {{\n    {vis} fun helper() {{}}\n    fun helper(x: Int) {{}}\n}}\n"));
    let (rel, before) = base("private");
    let (_, after) = base("public");
    let sub = ("app/Sub.kt", "package app\n\nclass Sub : A() {\n    fun go() { helper() }\n}\n");
    let call = ("sym:app/Sub.kt::Sub.go".to_string(), "sym:app/A.kt::A.helper".to_string(), "Calls".to_string(), String::new());
    let (updated, built) = updated_and_built(&[(rel, &before), sub], &[(rel, &after)]);
    assert!(built.contains(&call), "{built:?}");
    assert_eq!(updated, built);
    let (updated, built) = updated_and_built(&[(rel, &after), sub], &[(rel, &before)]);
    assert!(!built.contains(&call), "{built:?}");
    assert_eq!(updated, built);
}

const ANNOTATION: (&str, &str) = ("app/Api.kt", "package app\n\nannotation class Api\n");
const ANNOTATED: (&str, &str) = ("app/Service.kt", "package app\n\n@Api\nclass Service {\n    @Api\n    fun run() {}\n}\n");
const JAVA_ANNOTATED: (&str, &str) = ("app/Job.java", "package app;\n\n@Api\nclass Job {\n    @Api int size;\n}\n");

fn decorated(edges: &Edges) -> Vec<(String, String)> {
    edges.iter().filter(|(_, _, kind, _)| kind == "DecoratedBy").map(|(from, to, _, _)| (from.clone(), to.clone())).collect()
}

#[test]
fn an_annotation_class_added_decorates_the_unchanged_files_that_use_it() {
    let (updated, built) = updated_and_built(&[ANNOTATED, JAVA_ANNOTATED], &[ANNOTATION]);
    assert_eq!(decorated(&built).len(), 4, "{built:?}");
    assert_eq!(updated, built);
}

#[test]
fn an_annotation_class_removed_leaves_no_edge_to_it_after_update() {
    let (updated, built) = removed_updated_and_built(&[ANNOTATION, ANNOTATED, JAVA_ANNOTATED], &[], &[ANNOTATION.0]);
    assert!(decorated(&updated).is_empty(), "{updated:?}");
    assert_eq!(updated, built);
}

#[test]
fn an_annotation_class_renamed_or_moved_is_followed_by_update() {
    let renamed = ("app/Api.kt", "package app\n\nannotation class Endpoint\n");
    let (updated, built) = updated_and_built(&[ANNOTATION, ANNOTATED, JAVA_ANNOTATED], &[renamed]);
    assert!(decorated(&updated).is_empty(), "{updated:?}");
    assert_eq!(updated, built);
    let moved = ("app/meta/Api.kt", "package app\n\nannotation class Api\n");
    let (updated, built) = removed_updated_and_built(&[ANNOTATION, ANNOTATED, JAVA_ANNOTATED], &[moved], &[ANNOTATION.0]);
    assert!(decorated(&updated).iter().all(|(_, to)| to == "sym:app/meta/Api.kt::Api") && decorated(&updated).len() == 4, "{updated:?}");
    assert_eq!(updated, built);
}
