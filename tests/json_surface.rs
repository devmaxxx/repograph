//! Every reader an agent calls answers in JSON. Four commands already did — `ask`, `impact`,
//! `changes`, `families` — and three did not, which were the three a debugging session calls
//! most. An agent that has to parse prose re-parses it on every call and gets it wrong the day a
//! label changes, so the shape is pinned here rather than described in the README.
//!
//! Every object is an object, never a bare array: a field can then be added without breaking a
//! parser that was written against the version before it.

use std::process::Command;

fn repograph(repo: &std::path::Path, args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_repograph"))
        .arg("--no-dense").arg("--repo").arg(repo).args(args).output().unwrap();
    (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned())
}

const DOC: &str = "# Требования\n\n\
    **FR-PAY-22 · MUST · Отмена визита**\n\n\
    Отмена возможна за сутки; см. FR-PAY-26.\n\n\
    **FR-PAY-26 · MUST · Возврат средств**\n\n\
    Возврат в течение трёх дней.\n";

/// Two top-level functions, one calling the other: the shape that gives the graph a `Calls` edge
/// for `trace` to walk. A method reaching another through `this.ledger` does not — the field's
/// type is not resolved — and a test that used one would be testing the extractor, not the
/// JSON surface.
const CODE: &str = "export function refund(id: string) {\n  return write(id);\n}\n\n\
    export function write(id: string) {\n  return id;\n}\n";

fn built() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/req.md"), DOC).unwrap();
    std::fs::write(dir.path().join("billing.ts"), CODE).unwrap();
    let (ok, _) = repograph(dir.path(), &["build"]);
    assert!(ok, "the fixture repository built");
    dir
}

fn parsed(out: &str) -> serde_json::Value {
    serde_json::from_str(out).unwrap_or_else(|e| panic!("not JSON: {e}\n{out}"))
}

#[test]
fn explain_verify_and_trace_answer_in_json_like_the_rest() {
    let dir = built();
    let repo = dir.path();

    let (ok, out) = repograph(repo, &["explain", "FR-PAY-22", "--json"]);
    assert!(ok, "{out}");
    let v = parsed(&out);
    assert!(v.is_object(), "{out}");
    assert_eq!(v["id"], "FR-PAY-22");
    assert_eq!(v["kind"], "Requirement");
    assert_eq!(v["file"], "docs/req.md");
    // The direction is resolved for the caller: an edge's own `source`/`target` would make every
    // reader work out which end it was standing on.
    let edges = v["edges"].as_array().expect("edges is an array");
    assert!(edges.iter().all(|e| e["dir"] == "out" || e["dir"] == "in"), "{out}");
    assert!(edges.iter().any(|e| e["other"] == "FR-PAY-26"), "the cited requirement is a neighbour: {out}");

    let (ok, out) = repograph(repo, &["verify", "--json"]);
    assert!(ok, "{out}");
    let v = parsed(&out);
    assert!(v["nodes"].as_u64().unwrap() > 0, "{out}");
    assert!(v["nodes_by_kind"].is_object() && v["edges_by_kind"].is_object(), "{out}");
    for key in ["undeclared", "gaps", "cite_only"] {
        assert!(v[key].is_array(), "{key} is an array: {out}");
    }

    let (ok, out) = repograph(repo, &["trace", "refund", "write", "--json"]);
    assert!(ok, "{out}");
    let v = parsed(&out);
    assert_eq!(v["depth"], 6);
    let path = v["path"].as_array().expect("a call path was found");
    assert!(path.len() >= 2, "{out}");
    assert!(path.iter().all(|s| s["id"].is_string() && s["at"].is_string()), "{out}");
}

/// No path within the depth is an answer to the question that was asked. The text form treats it
/// as a failed lookup and exits non-zero; the JSON form says `null` and exits 0, because a caller
/// parsing an object should not have to read an exit code to learn what the object already says.
#[test]
fn a_trace_that_finds_nothing_is_a_null_path_and_not_a_failure() {
    let dir = built();
    let (ok, out) = repograph(dir.path(), &["trace", "write", "refund", "--json"]);
    assert!(ok, "the JSON form exits 0: {out}");
    let v = parsed(&out);
    assert!(v["path"].is_null(), "{out}");
    assert_eq!(v["from"], "sym:billing.ts::write");

    let (ok, _) = repograph(dir.path(), &["trace", "write", "refund"]);
    assert!(!ok, "the text form still exits non-zero, which is what a shell script reads");
}

/// A node the store does not have is an error in both forms: an empty object would be a claim
/// that the node exists and has no neighbours.
#[test]
fn explaining_a_node_that_is_not_there_fails_in_both_forms() {
    let dir = built();
    for args in [vec!["explain", "FR-NOPE-1"], vec!["explain", "FR-NOPE-1", "--json"]] {
        let (ok, out) = repograph(dir.path(), &args);
        assert!(!ok, "{args:?} should fail: {out}");
    }
}
