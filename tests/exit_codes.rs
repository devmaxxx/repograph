//! Two exit codes where there was one. A command that was asked a question and answered it —
//! `trace` finding no call path, `bench` measuring every case and missing a floor — exits 2, and
//! 1 is left to mean the command could not answer at all: no such symbol, no store to read. The
//! wording on stderr stays, as a message to a person; a script reads the status.

use std::path::Path;
use std::process::{Command, Output};

fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_repograph"))
        .arg("--no-dense").arg("--repo").arg(repo).args(args).output().unwrap()
}

fn code(out: &Output) -> Option<i32> { out.status.code() }

fn err(out: &Output) -> String { String::from_utf8_lossy(&out.stderr).into_owned() }

/// Two top-level functions, one calling the other: a call path exists one way round and not the
/// other, which is the pair `trace` needs to answer and to fail to answer.
const CODE: &str = "export function refund(id: string) {\n  return write(id);\n}\n\n\
    export function write(id: string) {\n  return id;\n}\n";

const DOC: &str = "# Требования\n\n**FR-PAY-22 · MUST · Отмена визита**\n\nОтмена возможна за сутки.\n";

fn built() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/req.md"), DOC).unwrap();
    std::fs::write(dir.path().join("billing.ts"), CODE).unwrap();
    assert!(run(dir.path(), &["build"]).status.success(), "the fixture repository built");
    dir
}

#[test]
fn a_trace_with_no_path_is_a_verdict_and_an_unknown_symbol_is_a_failure() {
    let dir = built();

    let found = run(dir.path(), &["trace", "refund", "write"]);
    assert_eq!(code(&found), Some(0), "{}", err(&found));

    let none = run(dir.path(), &["trace", "write", "refund"]);
    assert_eq!(code(&none), Some(2), "a question that was answered: {}", err(&none));
    assert!(err(&none).contains("no call path"), "and the sentence is still printed: {}", err(&none));

    let unknown = run(dir.path(), &["trace", "nosuchsymbol", "write"]);
    assert_eq!(code(&unknown), Some(1), "nothing was traced: {}", err(&unknown));
}

#[test]
fn a_bench_that_missed_its_floors_is_a_verdict_and_an_empty_store_is_a_failure() {
    let dir = built();
    let cases = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/floors-missed.jsonl");
    let cases = cases.to_str().unwrap();

    let missed = run(dir.path(), &["bench", "--cases", cases]);
    assert_eq!(code(&missed), Some(2), "every case ran and the floors were not met: {}", err(&missed));
    assert!(err(&missed).contains("bench floors not met"), "{}", err(&missed));
    let out = String::from_utf8_lossy(&missed.stdout);
    assert!(out.contains("gated=true"), "the suite was graded: {out}");

    let unbuilt = tempfile::tempdir().unwrap();
    let empty = run(unbuilt.path(), &["bench", "--cases", cases]);
    assert_eq!(code(&empty), Some(1), "nothing was measured: {}", err(&empty));
    assert!(err(&empty).contains("graph is empty"), "{}", err(&empty));
}
