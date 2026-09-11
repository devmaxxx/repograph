//! Two exit codes where there was one. A command that was asked a question and answered it —
//! `trace` finding no call path, `bench` measuring every case and missing a floor — exits 3, and
//! 1 is left to mean the command could not answer at all: no such symbol, no store to read. The
//! status is the interface; the wording on stderr is a message to a person and no script reads it,
//! so what is pinned here is the status and the shape of the answer beside it.
//!
//! 3 and not 2 because 2 is spoken by two layers that never reached the command: `clap` writes it
//! for a usage error, and the npm launcher writes it when no platform binary is installed. The
//! usage case is pinned below beside the verdicts, since a status a harness reads as "answered"
//! has to be one no other layer can produce.

mod common;

use common::run;
use std::path::{Path, PathBuf};
use std::process::Output;

fn code(out: &Output) -> Option<i32> { out.status.code() }

fn err(out: &Output) -> String { String::from_utf8_lossy(&out.stderr).into_owned() }

fn out(out: &Output) -> String { String::from_utf8_lossy(&out.stdout).into_owned() }

/// The path an `anyhow` error takes: `main` prints "Error: …" and exits 1. A verdict is not that,
/// so the assertion on a verdict's stderr is that it is *not* this — which stays true however the
/// sentence itself is reworded. Any line of it, not the first: a command that logged a line before
/// it failed would have hidden the prefix behind the log and read as a verdict.
fn is_anyhow_error(text: &str) -> bool { text.lines().any(|l| l.starts_with("Error:")) }

/// Two top-level functions, one calling the other: a call path exists one way round and not the
/// other, which is the pair `trace` needs to answer and to fail to answer. A method reaching
/// another through `this.ledger` would not give the graph a `Calls` edge at all — the field's type
/// is not resolved — so a fixture built that way would be pinning the extractor, not the statuses.
const CODE: &str = "export function refund(id: string) {\n  return write(id);\n}\n\n\
    export function write(id: string) {\n  return id;\n}\n";

/// `docs/req.md` is not decoration: `bench`'s cases anchor every expectation to this path, so the
/// document and the case file below stand or fall together.
const DOC: &str = "# Требования\n\n**FR-PAY-22 · MUST · Отмена визита**\n\nОтмена возможна за сутки.\n";

/// The case file every `bench` row here is measured over: 40 keyword, 30 paraphrase and 12 code
/// cases, none of which any answer can reach. Two couplings ride on that. The counts are
/// `RECORDED_SHAPE` in `src/bench.rs` — a file of any other shape is measured and reported with
/// `gated=false`, and this suite's verdict would go with it — and every case anchors
/// `docs/req.md`, which is `built()`'s document above; `bench` refuses a case whose anchor is no
/// node and no declared file, so renaming that document breaks the suite here and not in `bench`.
const FLOORS_MISSED: (usize, usize, usize) = (40, 30, 12);

fn cases() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/floors-missed.jsonl")
}

fn built() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/req.md"), DOC).unwrap();
    std::fs::write(dir.path().join("billing.ts"), CODE).unwrap();
    let built = run(dir.path(), &["build"]);
    assert!(built.status.success(), "the fixture repository built: {}", err(&built));
    dir
}

#[test]
fn a_trace_with_no_path_is_a_verdict_and_an_unknown_symbol_is_a_failure() {
    let dir = built();

    let found = run(dir.path(), &["trace", "refund", "write"]);
    assert_eq!(code(&found), Some(0), "{}", err(&found));

    let none = run(dir.path(), &["trace", "write", "refund"]);
    assert_eq!(code(&none), Some(3), "a question that was answered: {}", err(&none));
    assert!(!is_anyhow_error(&err(&none)), "a verdict, not a failure: {}", err(&none));
    // The verdict is spoken as well as exited: a silent 3 leaves a person at a prompt with no
    // answer. The words are a message no script reads, so what is pinned is that there are some.
    assert!(!err(&none).trim().is_empty(), "the verdict says something: {:?}", err(&none));

    // Either end of the pair: the symbol that is not there is the one thing the command could not
    // resolve, and which side it stands on does not change what it could not do.
    let unknown = run(dir.path(), &["trace", "nosuchsymbol", "write"]);
    assert_eq!(code(&unknown), Some(1), "nothing was traced: {}", err(&unknown));
    let unknown_to = run(dir.path(), &["trace", "refund", "nosuchsymbol"]);
    assert_eq!(code(&unknown_to), Some(1), "nothing was traced: {}", err(&unknown_to));
}

#[test]
fn a_bench_that_missed_its_floors_is_a_verdict_and_an_empty_store_is_a_failure() {
    let dir = built();
    let cases = cases();
    let lines = std::fs::read_to_string(&cases).unwrap().lines().filter(|l| !l.trim().is_empty()).count();
    let (keyword, paraphrase, code_cases) = FLOORS_MISSED;
    assert_eq!(lines, keyword + paraphrase + code_cases,
               "the fixture must hold RECORDED_SHAPE's counts (src/bench.rs), or the run is ungated");
    let cases = cases.to_str().unwrap();

    let missed = run(dir.path(), &["bench", "--cases", cases]);
    assert_eq!(code(&missed), Some(3), "every case ran and the floors were not met: {}", err(&missed));
    assert!(!is_anyhow_error(&err(&missed)), "a verdict, not a failure: {}", err(&missed));
    let said = out(&missed);
    assert!(said.contains("gated=true"), "the suite was graded: {said}");
    // Every case ran and every one of them missed: a suite that stopped early would report fewer
    // than 40 keyword cases and still exit 3, which is the reading "answered" must not cover.
    assert!(said.contains(&format!("keyword 0/{keyword}")), "every case ran and none was hit: {said}");

    // Repeated, the verdict is still a verdict: each run has to meet the floors, so two runs that
    // both missed answer with the same status as one.
    let twice = run(dir.path(), &["bench", "--cases", cases, "--repeat", "2"]);
    assert_eq!(code(&twice), Some(3), "both runs answered and both missed: {}", err(&twice));
    // And both of them ran: a `--repeat` that measured once and exited 3 on the first miss would
    // pin the same status off half the work.
    assert!(out(&twice).contains("median of 2"), "two runs were taken and summarised: {}", out(&twice));

    let unbuilt = tempfile::tempdir().unwrap();
    let empty = run(unbuilt.path(), &["bench", "--cases", cases]);
    assert_eq!(code(&empty), Some(1), "nothing was measured: {}", err(&empty));
    assert!(err(&empty).contains("graph is empty"), "{}", err(&empty));
}

/// A case file of another shape is measured and reported, and its exit code claims nothing: the
/// floors were recorded over `RECORDED_SHAPE` and grading five cases against them would be a
/// verdict on a suite nobody measured. So the run that misses every case still exits 0, and says
/// `gated=false` for the reader who has to know why.
#[test]
fn a_case_file_of_another_shape_is_reported_and_not_graded() {
    let dir = built();
    let five: String = std::fs::read_to_string(cases()).unwrap().lines().take(5)
        .map(|l| format!("{l}\n")).collect();
    let path = dir.path().join("five.jsonl");
    std::fs::write(&path, five).unwrap();

    let ungated = run(dir.path(), &["bench", "--cases", path.to_str().unwrap()]);
    assert_eq!(code(&ungated), Some(0), "measured, not graded: {}", err(&ungated));
    assert!(out(&ungated).contains("gated=false"), "and it says so: {}", out(&ungated));
    // Reported means measured: the five cases ran and every one of them missed, which is what an
    // ungated 0 has to be distinguishable from a suite that never got as far as running.
    assert!(out(&ungated).contains("keyword 0/5"), "all five were measured: {}", out(&ungated));
}

/// The status a verdict may not take. `clap` answers a mistyped flag with 2 before the command
/// runs at all, so a harness reading 2 as "the suite answered and missed a floor" would read a
/// typo as a reading. Nothing was measured here, which is what the empty stdout says: no store is
/// built for this case because the command never reaches one.
#[test]
fn a_usage_error_exits_two_and_is_therefore_not_a_verdict() {
    let dir = tempfile::tempdir().unwrap();
    let usage = run(dir.path(), &["bench", "--nosuchflag"]);
    assert_eq!(code(&usage), Some(2), "clap's own status: {}", err(&usage));
    assert_eq!(out(&usage), "", "nothing was measured: {}", out(&usage));
}

/// Cargo runs this file's tests on one process, so a test that writes the environment writes it
/// for every other one. The idiom is `src/config.rs`'s — a lock taken before the mutation, and a
/// Safety note saying why the mutation is sound — so the crate has one of them and not two.
static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// What a shell that has been running the bench kit exports, and what a test may not inherit from
/// it. `REPOGRAPH_BENCH_REPO` sends `bench` to another repository: the case below would read an
/// empty directory, fail on the missing graph and exit 1 — a verdict test failing as though the
/// verdict had gone, for a reason nothing in the run names.
#[test]
fn the_bench_kits_variables_do_not_reach_the_child() {
    let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let dir = built();
    let elsewhere = tempfile::tempdir().unwrap();
    // Safety: the lock above makes this the only thread touching the environment, and no reader
    // races it — every spawn in this crate goes through `common::run`, which removes both
    // variables from the child whatever the parent holds.
    unsafe {
        std::env::set_var("REPOGRAPH_BENCH_REPO", elsewhere.path());
        std::env::set_var("REPOGRAPH_EMBED_MODEL", "intfloat/multilingual-e5-large");
    }
    let missed = run(dir.path(), &["bench", "--cases", cases().to_str().unwrap()]);
    unsafe {
        std::env::remove_var("REPOGRAPH_BENCH_REPO");
        std::env::remove_var("REPOGRAPH_EMBED_MODEL");
    }
    assert_eq!(code(&missed), Some(3), "the child read the repository it was given: {}", err(&missed));
}
