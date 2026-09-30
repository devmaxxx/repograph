//! The binary over a directory of Python scripts that import each other as siblings, the way a
//! script's `sys.path[0]` lets them: built, asked `impact`, `trace` and `changes`, and updated.

mod common;

use common::git;
use std::path::Path;

// Python is not in the default globs, so the repository names it.
const TREE: &[(&str, &str)] = &[
    ("repograph.toml", "code_globs = [\"**/*.py\"]\n"),
    (".gitignore", ".repograph/\n__pycache__/\n"),
    (
        "tools/compare/truth.py",
        "\"\"\"Truth for the comparison.\"\"\"\n\n\ndef read_jsonl(path):\n    return [path]\n\n\nclass Graph:\n    def shortest(self, a, b):\n        return [a, b]\n",
    ),
    (
        "tools/compare/run.py",
        "import truth as T\nfrom truth import Graph\n\n\nclass Runner:\n    def __init__(self, graph: Graph):\n        self.graph = graph\n\n    def go(self):\n        rows = T.read_jsonl(\"cases.jsonl\")\n        return self.graph.shortest(rows, rows)\n\n\nif __name__ == \"__main__\":\n    Runner(Graph()).go()\n",
    ),
    (
        "tools/compare/test_run.py",
        "import unittest\n\nfrom run import Runner\n\n\nclass RunnerTest(unittest.TestCase):\n    def test_go(self):\n        Runner(None).go()\n",
    ),
];

fn repograph(repo: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let out = common::run(repo, args);
    (out.status.code(), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

#[test]
fn sibling_scripts_answer_impact_trace_and_changes_and_survive_an_update() {
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

    // A sibling import by bare name, through a module alias, reaches the function's importer.
    let read = "sym:tools/compare/truth.py::read_jsonl";
    let (code, out, err) = repograph(repo, &["impact", read]);
    assert_eq!(code, Some(0), "{out}{err}");
    assert!(out.contains("sym:tools/compare/run.py::Runner.go"), "impact read_jsonl names its caller: {out}");

    // A test file importing its script by bare name depends on the class it constructs.
    let (code, out, err) = repograph(repo, &["impact", "sym:tools/compare/run.py::Runner"]);
    assert_eq!(code, Some(0), "{out}{err}");
    assert!(out.contains("tools/compare/test_run.py"), "{out}");

    // `self.graph` is typed by the annotated `__init__` parameter it is assigned from.
    let (code, out, err) = repograph(repo, &["trace", "sym:tools/compare/run.py::Runner.go", "sym:tools/compare/truth.py::Graph.shortest"]);
    assert_eq!(code, Some(0), "{out}{err}");
    assert!(out.contains("Runner.go") && out.contains("Graph.shortest"), "{out}");
    let (code, out, _) = repograph(repo, &["trace", "sym:tools/compare/truth.py::Graph.shortest", "sym:tools/compare/run.py::Runner.go"]);
    assert_eq!(code, Some(3), "no path is a verdict: {out}");

    // A body-only edit, applied by `update` before any reader can: `changes`, like the other
    // readers, refreshes the store itself, which would leave `update` nothing to report.
    let truth = repo.join("tools/compare/truth.py");
    let edited = std::fs::read_to_string(&truth).unwrap().replace("    return [path]\n", "    rows = [path]\n    return rows\n");
    std::fs::write(&truth, edited).unwrap();

    let (code, out, err) = repograph(repo, &["update"]);
    assert_eq!(code, Some(0), "{out}{err}");
    assert!(out.starts_with("changed 1 removed 0"), "{out}");
    let (_, out, _) = repograph(repo, &["impact", read]);
    assert!(out.contains("tools/compare/run.py"), "the update kept the importer: {out}");

    let (code, out, err) = repograph(repo, &["changes", "--base", "HEAD"]);
    assert_eq!(code, Some(0), "{out}{err}");
    assert!(out.contains("tools/compare/truth.py") && out.contains(read), "{out}");
}
