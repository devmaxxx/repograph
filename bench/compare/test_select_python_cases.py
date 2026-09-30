"""The run-time case selection, on a synthetic repository shaped like a script directory."""

import io
import subprocess
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

import select_python_cases as S

GIT = ["git", "-c", "core.hooksPath=", "-c", "commit.gpgsign=false", "-c", "user.email=select@test", "-c", "user.name=select"]

BASE = {
    "tools/store.py": "class Store:\n    def open(self):\n        return 1\n\n\ndef connect():\n    return Store()\n",
    "tools/run.py": "import store\nfrom store import Store\n\n\nclass Runner:\n    def __init__(self):\n        self.store = store.Store()\n\n    def go(self):\n        self.store.open()\n        store.connect()\n",
    "tools/other.py": "from store import connect\n\n\ndef again():\n    connect()\n",
    "tools/.cache/hidden.py": "from store import connect\n\n\ndef _secret():\n    connect()\n",
    "notes.md": "connect Store\n",
}


def repository(root: Path) -> None:
    subprocess.run([*GIT, "init", "-q", "-b", "main", str(root)], check=True, capture_output=True)
    for rel, body in BASE.items():
        (root / rel).parent.mkdir(parents=True, exist_ok=True)
        (root / rel).write_text(body, encoding="utf8")
    subprocess.run([*GIT, "add", "-A"], cwd=root, check=True, capture_output=True)
    subprocess.run([*GIT, "commit", "-q", "-m", "base"], cwd=root, check=True, capture_output=True)
    (root / "tools/store.py").write_text(BASE["tools/store.py"].replace("return 1", "return 2"), encoding="utf8")
    subprocess.run([*GIT, "commit", "-qam", "edit"], cwd=root, check=True, capture_output=True)


class Select(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = Path(self.tmp.name)
        repository(self.repo)

    def tearDown(self):
        self.tmp.cleanup()

    def test_impact_takes_names_every_referencing_file_imports(self):
        cases, _ = S.select(self.repo, ".py")
        impact = {c["target"]: c for c in cases if c["kind"] == "impact"}
        self.assertEqual(impact["Store"], {"kind": "impact", "target": "Store", "id": "sym:tools/store.py::Store", "file": "tools/store.py", "tier": "narrow", "exts": [".py"]})
        self.assertNotIn("connect", impact, "a dotted file spells it: the resolver cannot bind an import there")
        self.assertNotIn("Runner", impact, "a name nothing references is no case")
        self.assertNotIn("_secret", impact)

    def test_trace_takes_a_di_path_and_its_reverse_as_none(self):
        cases, waivers = S.select(self.repo, ".py")
        traces = [c for c in cases if c["kind"] == "trace"]
        self.assertEqual(traces, [
            {"kind": "trace", "from": "Runner", "to": "Store", "from_id": "sym:tools/run.py::Runner", "to_id": "sym:tools/store.py::Store", "expect": "path", "via": []},
            {"kind": "trace", "from": "Store", "to": "Runner", "from_id": "sym:tools/store.py::Store", "to_id": "sym:tools/run.py::Runner", "expect": "none", "via": []},
        ])
        self.assertFalse([w for w in waivers if w.startswith("trace")])

    def test_changes_takes_the_newest_commit_with_a_symbol_hunk(self):
        cases, _ = S.select(self.repo, ".py")
        head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=self.repo, capture_output=True, text=True).stdout.strip()
        self.assertEqual([c for c in cases if c["kind"] == "changes"], [{"kind": "changes", "base": f"{head}~1"}])

    def test_a_corpus_with_no_di_edge_records_a_waiver_instead_of_a_case(self):
        (self.repo / "tools/run.py").write_text("import store\n\n\ndef go():\n    store.connect()\n", encoding="utf8")
        cases, waivers = S.select(self.repo, ".py")
        self.assertFalse([c for c in cases if c["kind"] == "trace"])
        self.assertTrue(any(w.startswith("trace:") for w in waivers), waivers)

    def test_the_cli_prints_counts_and_no_name(self):
        out_dir = Path(self.tmp.name) / "private"
        buf = io.StringIO()
        with redirect_stdout(buf):
            S.main(["--repo", str(self.repo), "--ext", ".py", "--out", str(out_dir / "python-blast.jsonl"), "--waivers", str(out_dir / "waivers.txt")])
        printed = buf.getvalue()
        for name in ("tools", "store", "Store", "connect", "Runner"):
            self.assertNotIn(name, printed)
        self.assertRegex(printed, r"impact \d+ \(wide \d+, narrow \d+\) · trace \d+ · changes \d+ · waivers \d+")
        self.assertTrue((out_dir / "python-blast.jsonl").read_text(encoding="utf8").strip())


class Roots(unittest.TestCase):
    def test_trace_reads_the_files_it_is_given(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            repository(repo)
            cases, _ = S.trace_cases(repo, [])
            self.assertEqual(cases, [], "no roots, no graph: the caller must hand the walk-visible files")
            cases, _ = S.trace_cases(repo, S.visible(repo, ".py"))
            self.assertEqual(len(cases), 2)

    def test_a_class_two_files_declare_makes_no_trace_case(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            repository(repo)
            (repo / "tools/twin.py").write_text("class Store:\n    pass\n", encoding="utf8")
            cases, waivers = S.trace_cases(repo, S.visible(repo, ".py"))
            self.assertEqual(cases, [], "one of the two ids would name the wrong file")
            self.assertEqual(len(waivers), 1)


if __name__ == "__main__":
    unittest.main()
