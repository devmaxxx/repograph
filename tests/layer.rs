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
