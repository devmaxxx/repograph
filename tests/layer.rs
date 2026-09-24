//! The binary over repositories the language layer reads nothing new in: the store it writes is
//! the store 0.5.3 wrote, and a glob that reaches past the grammars says so.

mod common;

use std::path::Path;

fn write(repo: &Path, rel: &str, text: &str) {
    let p = repo.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

#[test]
fn a_globbed_file_no_grammar_reads_is_named_once_on_stderr() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    write(repo, "repograph.toml", "code_globs = [\"**/*.ts\", \"**/*.cs\"]\n");
    write(repo, "web/a.ts", "export function boot() {}\n");
    write(repo, "svc/Program.cs", "public class Program {}\n");
    write(repo, "svc/Order.cs", "public class Order {}\n");
    let out = common::run(repo, &["build"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");
    assert_eq!(err.matches("code: no grammar reads 2 .cs — indexed as files only").count(), 1, "{err}");
}

/// A family's readings take its language in before the defaults do, and write no `repograph.toml`
/// into the tree they measure: the environment names the globs over the repository's own file.
#[test]
fn the_environment_names_the_globs_for_one_run_over_the_repository_s_own() {
    // `common::run` strips the variable, so these builds set their environment themselves.
    let build = |globs: Option<&str>| -> Vec<String> {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        write(repo, "repograph.toml", "code_globs = [\"**/*.ts\"]\n");
        write(repo, "web/a.ts", "export function boot() {}\n");
        write(repo, "mobile/Pet.kt", "class Pet\n");
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_repograph"));
        cmd.env_remove("REPOGRAPH_BENCH_REPO").env_remove("REPOGRAPH_EMBED_MODEL").env_remove("REPOGRAPH_CODE_GLOBS");
        if let Some(g) = globs {
            cmd.env("REPOGRAPH_CODE_GLOBS", g);
        }
        let out = cmd.arg("--no-dense").arg("--repo").arg(repo).arg("build").output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let graph: serde_json::Value = serde_json::from_slice(&std::fs::read(repo.join(".repograph").join("graph.json")).unwrap()).unwrap();
        graph["nodes"].as_object().unwrap().keys().cloned().collect()
    };
    let own = build(None);
    let widened = build(Some("**/*.ts **/*.kt"));
    assert!(own.iter().any(|i| i == "file:web/a.ts") && !own.iter().any(|i| i == "file:mobile/Pet.kt"), "{own:?}");
    assert!(widened.iter().any(|i| i == "file:mobile/Pet.kt"), "{widened:?}");
}

#[test]
fn a_store_that_holds_no_name_indexed_file_carries_no_header_file() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    write(repo, "repograph.toml", "code_globs = [\"**/*.ts\", \"**/*.cs\"]\n");
    write(repo, "web/a.ts", "export function boot() {}\n");
    write(repo, "svc/Program.cs", "public class Program {}\n");
    assert!(common::run(repo, &["build"]).status.success());
    write(repo, "web/a.ts", "export function boot() { return 1; }\n");
    std::fs::remove_file(repo.join("svc/Program.cs")).unwrap();
    assert!(common::run(repo, &["update"]).status.success());
    assert!(!repo.join(".repograph").join("headers.json").exists());
}
