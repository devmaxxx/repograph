//! The binary over a two-crate Rust workspace: a crate named across the workspace, a module
//! tree, an `impl` of another crate's type, and a trait held in a struct field — built, asked
//! `impact`, `trace` and `changes`, and updated after a body-only edit.

mod common;

use std::path::Path;
use std::process::Command;

// Rust is not in the default globs, so the repository names it.
const TREE: &[(&str, &str)] = &[
    ("repograph.toml", "code_globs = [\"**/*.rs\"]\n"),
    (".gitignore", ".repograph/\ntarget/\n"),
    ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
    ("crates/shop-core/Cargo.toml", "[package]\nname = \"shop-core\"\nversion = \"0.1.0\"\n"),
    ("crates/shop-core/src/lib.rs", "pub mod exec;\npub mod store;\n"),
    ("crates/shop-core/src/exec.rs", "pub trait Exec {\n    fn run(&self, cmd: &str);\n}\n"),
    ("crates/shop-core/src/store.rs", "pub struct Store;\n\nimpl Store {\n    pub fn open() -> Store {\n        Store\n    }\n}\n"),
    ("crates/shopd/Cargo.toml", "[package]\nname = \"shopd\"\nversion = \"0.1.0\"\n"),
    ("crates/shopd/src/main.rs", "mod handler;\nmod ops;\n\nuse shop_core::store::Store;\n\nfn main() {\n    let _ = Store::open();\n    ops::sync();\n}\n"),
    ("crates/shopd/src/ops.rs", "use shop_core::store::Store;\n\nimpl Store {\n    pub fn sync_all() {}\n}\n\npub fn sync() {\n    Store::sync_all();\n}\n"),
    (
        "crates/shopd/src/handler.rs",
        "use shop_core::exec::Exec;\n\npub struct Handler<E: Exec> {\n    exec: E,\n}\n\nimpl<E: Exec> Handler<E> {\n    pub fn handle(&self) {\n        self.exec.run(\"ls\");\n    }\n}\n",
    ),
];

fn repograph(repo: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let out = common::run(repo, args);
    (out.status.code(), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn a_rust_workspace_answers_impact_trace_and_changes_and_survives_an_update() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    for (rel, text) in TREE {
        let p = repo.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    git(repo, &["init", "-q"]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "init"]);

    let (code, out, err) = repograph(repo, &["build"]);
    assert_eq!(code, Some(0), "{out}{err}");

    // `Store` is declared in one crate; its callers reach it through a `use` of the crate's name,
    // and through an `impl Store` in the other crate that only a `References` edge ties back.
    let (code, out, err) = repograph(repo, &["impact", "sym:crates/shop-core/src/store.rs::Store"]);
    assert_eq!(code, Some(0), "{out}{err}");
    for want in ["crates/shopd/src/main.rs", "crates/shopd/src/ops.rs", "sym:crates/shopd/src/ops.rs::Store.sync_all"] {
        assert!(out.contains(want), "impact Store names {want}: {out}");
    }

    // The call resolves to the member this file's own `impl Store` writes, so the store id names
    // `ops.rs`, not the crate that declares `Store`.
    let (code, out, err) = repograph(repo, &["impact", "sym:crates/shopd/src/ops.rs::Store.sync_all"]);
    assert_eq!(code, Some(0), "{out}{err}");
    assert!(out.contains("sym:crates/shopd/src/ops.rs::sync"), "sync reaches its own file's Store.sync_all: {out}");

    // A trait held in a field typed by the struct's parameter is reached through the bound.
    let (code, out, err) = repograph(repo, &["trace", "Handler.handle", "Exec"]);
    assert_eq!(code, Some(0), "{out}{err}");
    assert!(out.contains("Handler.handle") && out.contains("Exec.run"), "{out}");
    let (code, out, _) = repograph(repo, &["trace", "Exec", "Handler"]);
    assert_eq!(code, Some(3), "no path is a verdict: {out}");

    // A body-only edit, applied to the store by `update` before any reader can: `changes`, like
    // the other readers, refreshes the store itself, which would leave `update` nothing to report.
    let ops = repo.join("crates/shopd/src/ops.rs");
    let edited = std::fs::read_to_string(&ops).unwrap().replace("    Store::sync_all();\n", "    Store::sync_all();\n    Store::sync_all();\n");
    std::fs::write(&ops, edited).unwrap();

    // One file re-read, and the foreign impl's tie to `Store` is written again with it.
    let (code, out, err) = repograph(repo, &["update"]);
    assert_eq!(code, Some(0), "{out}{err}");
    assert!(out.starts_with("changed 1 removed 0"), "{out}");
    let (_, out, _) = repograph(repo, &["impact", "sym:crates/shop-core/src/store.rs::Store"]);
    assert!(out.contains("crates/shopd/src/ops.rs"), "the update kept the impl's reference: {out}");

    // `changes` names the Rust symbol the hunk is in.
    let (code, out, err) = repograph(repo, &["changes", "--base", "HEAD"]);
    assert_eq!(code, Some(0), "{out}{err}");
    assert!(out.contains("crates/shopd/src/ops.rs") && out.contains("sym:crates/shopd/src/ops.rs::sync"), "{out}");
}
