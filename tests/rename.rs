//! A repository built before the rename keeps its store under `.repograph/`. Upgrading the binary
//! must not rebuild it, so a reader answers from the old directory where it stands and the first
//! write moves it.

mod common;

use common::run;

const DOC: &str = "# Требования\n\n**FR-PAY-22 · MUST · Отмена визита**\n\nОтмена возможна за сутки.\n";

fn repo() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("docs")).unwrap();
    std::fs::write(d.path().join("docs/req.md"), DOC).unwrap();
    d
}

fn ok(out: &std::process::Output) -> String {
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn a_store_built_before_the_rename_answers_in_place_and_moves_on_the_next_write() {
    let d = repo();
    ok(&run(d.path(), &["build"]));
    std::fs::rename(d.path().join(".zerograph"), d.path().join(".repograph")).unwrap();

    let answer = ok(&run(d.path(), &["explain", "FR-PAY-22", "--stale"]));
    assert!(answer.contains("Отмена визита"), "{answer}");
    assert!(!d.path().join(".zerograph").exists(), "a read moved the store");

    let out = ok(&run(d.path(), &["update"]));
    assert!(out.starts_with("changed 0 removed 0 "), "the move re-read something: {out}");
    assert!(!d.path().join(".repograph").exists());
    assert!(d.path().join(".zerograph/graph.json").exists());
    let answer = ok(&run(d.path(), &["explain", "FR-PAY-22", "--stale"]));
    assert!(answer.contains("Отмена визита"), "{answer}");
}
