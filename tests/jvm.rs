//! The binary over a JVM repository: `update` after a nested type appears or disappears writes the
//! graph a fresh `build` of the same tree writes.

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
    let old = tempfile::tempdir().unwrap();
    for (rel, text) in before {
        write(old.path(), rel, text);
    }
    ok(old.path(), &["build"]);
    for (rel, text) in after {
        write(old.path(), rel, text);
    }
    ok(old.path(), &["update"]);
    let fresh = tempfile::tempdir().unwrap();
    for (rel, text) in before.iter().chain(after) {
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
