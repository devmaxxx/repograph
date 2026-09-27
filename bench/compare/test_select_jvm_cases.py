"""The private corpus's case selection on synthetic repositories: what it picks and what it waives."""

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import select_jvm_cases as S

# An empty hooks path and no signing: the developer's global git config must not fail the fixture.
GIT = ["git", "-c", "core.hooksPath=", "-c", "commit.gpgsign=false", "-c", "user.email=select@test", "-c", "user.name=select"]
SCRIPT = Path(__file__).resolve().parent / "select_jvm_cases.py"
JVM = [".java", ".kt"]


def repository(root: Path, commits: list[dict[str, str]]) -> list[str]:
    """A git repository at `root` with one commit per dict; the commit hashes, oldest first."""
    subprocess.run([*GIT, "init", "-q", "-b", "main", str(root)], check=True, capture_output=True)
    shas = []
    for i, files in enumerate(commits):
        for rel, body in files.items():
            (root / rel).parent.mkdir(parents=True, exist_ok=True)
            (root / rel).write_text(body, encoding="utf8")
        subprocess.run([*GIT, "add", "-A"], cwd=root, check=True, capture_output=True)
        subprocess.run([*GIT, "commit", "-q", "-m", f"c{i}"], cwd=root, check=True, capture_output=True)
        shas.append(subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, text=True).stdout.strip())
    return shas


def traces(out: Path) -> list[tuple[str, str]]:
    cases = [json.loads(line) for line in (out / "blast.jsonl").read_text().splitlines()]
    return [(c["from"], c["to"]) for c in cases if c["kind"] == "trace"]


SHOP = {
    "android/shop/Invoice.java": "package shop;\n\npublic class Invoice {\n    public void send() {}\n}\n",
    "android/shop/Checkout.java": "package shop;\n\npublic class Checkout {\n    private final Invoice invoice = null;\n\n    public void pay() {\n        invoice.send();\n    }\n}\n",
    "android/app/Screen.kt": "package app\n\nimport shop.Checkout\n\nclass Screen(private val checkout: Checkout) {\n    fun tap() { checkout.pay() }\n}\n",
    "web/invoice.ts": "export type Invoice = { id: string };\n",
}
EDIT = {"android/shop/Invoice.java": "package shop;\n\npublic class Invoice {\n    public void send() {\n        return;\n    }\n}\n"}


class Selection(unittest.TestCase):
    def test_a_corpus_with_every_kind_gets_its_cases_and_no_waiver(self):
        with tempfile.TemporaryDirectory() as d:
            repo, out = Path(d) / "corpus", Path(d) / "out"
            repo.mkdir()
            shas = repository(repo, [SHOP, EDIT])
            got = S.select(repo, out)
            cases = [json.loads(line) for line in (out / "blast.jsonl").read_text().splitlines()]
            self.assertEqual(cases[0], {"kind": "impact", "target": "Checkout", "id": "sym:android/shop/Checkout.java::Checkout",
                                        "file": "android/shop/Checkout.java", "tier": "narrow", "lang": ".java", "exts": JVM})
            # `Invoice` is named by a TypeScript file too, and L10 forbids the edge that would find it.
            self.assertFalse(any(c.get("target") == "Invoice" for c in cases))
            self.assertEqual(cases[1], {"kind": "trace", "from": "Screen", "to": "Invoice",
                                        "from_id": "sym:android/app/Screen.kt::Screen", "to_id": "sym:android/shop/Invoice.java::Invoice",
                                        "expect": "path", "via": ["Checkout"], "lang": ".java", "exts": JVM})
            self.assertEqual(cases[-1]["base"], f"{shas[1][:12]}~1")
            self.assertEqual(cases[-1]["exts"], JVM)
            self.assertEqual(got["kept"], {"impact": 1, "trace": 2, "changes": 1})
            self.assertEqual((out / "waivers.txt").read_text(), "")
            self.assertEqual((out / "pin.txt").read_text().strip(), shas[1])
            truth = json.loads((out / "truth.json").read_text())
            self.assertEqual(truth["trace"]["Screen->Invoice"], ["Screen", "Checkout.pay", "Invoice.send"])

    def test_a_kind_with_no_candidate_is_waived_with_its_reason_class(self):
        with tempfile.TemporaryDirectory() as d:
            repo, out = Path(d) / "corpus", Path(d) / "out"
            repo.mkdir()
            repository(repo, [{"a/Lone.java": "package a;\n\npublic class Lone {}\n", "a/Other.kt": "package a\n\nclass Other\n"}])
            got = S.select(repo, out)
            self.assertEqual((out / "blast.jsonl").read_text(), "")
            self.assertEqual((out / "waivers.txt").read_text(),
                             "impact\tno-java-type-named-only-by-jvm-files\n"
                             "trace\tno-injected-call-path-through-java\n"
                             "changes\tno-window-with-a-java-declaration\n")
            self.assertEqual(got["waived"], ["no-java-type-named-only-by-jvm-files", "no-injected-call-path-through-java", "no-window-with-a-java-declaration"])

    def test_the_printed_summary_carries_counts_and_no_name(self):
        with tempfile.TemporaryDirectory() as d:
            repo, out = Path(d) / "corpus", Path(d) / "out"
            repo.mkdir()
            repository(repo, [SHOP, EDIT])
            printed = subprocess.run([sys.executable, str(SCRIPT), "--corpus", str(repo), "--out", str(out)],
                                     capture_output=True, text=True, check=True).stdout
            for name in ("Checkout", "Invoice", "Screen", "android", "shop"):
                self.assertNotIn(name, printed)
            self.assertEqual(json.loads(printed)["kept"], {"impact": 1, "trace": 2, "changes": 1})

    def test_an_out_directory_inside_the_repository_is_refused_before_anything_is_written(self):
        root = SCRIPT.parents[2]
        with tempfile.TemporaryDirectory() as d:
            for out in (root, root / "bench" / "private-cases-must-not-exist"):
                run = subprocess.run([sys.executable, str(SCRIPT), "--corpus", d, "--out", str(out)], capture_output=True, text=True)
                self.assertNotEqual(run.returncode, 0)
                self.assertEqual(run.stdout, "")
                self.assertFalse((out / "blast.jsonl").exists())


class TraceEnds(unittest.TestCase):
    def test_a_path_ending_at_a_type_the_corpus_never_declares_is_dropped(self):
        library = {"android/shop/Plugin.java": (
            "package shop;\n\nimport vendor.Bridge;\n\npublic class Plugin {\n    private final Bridge bridge = null;\n\n"
            "    public void load() {\n        bridge.call();\n    }\n}\n")}
        with tempfile.TemporaryDirectory() as d:
            repo, out = Path(d) / "corpus", Path(d) / "out"
            repo.mkdir()
            repository(repo, [library])
            self.assertEqual(S.T.shortest_path(S.T.di_call_graph(repo, ["android"]), "Plugin", "Bridge"), ["Plugin", "Bridge.call"])
            got = S.select(repo, out)
            self.assertEqual(traces(out), [])
            self.assertEqual((got["trace_candidates"], got["trace_end_not_one_jvm_type"]), (1, 1))


class MemberCalls(unittest.TestCase):
    """The injected-call graph joins calls per class, so a path can chain a method into a call that
    only another method of the same class makes. A trace is kept only when every hop is a call the
    method reached actually makes, read on source with its comments and strings blanked."""

    def test_a_path_whose_every_hop_is_a_member_call_is_kept(self):
        with tempfile.TemporaryDirectory() as d:
            repo, out = Path(d) / "corpus", Path(d) / "out"
            repo.mkdir()
            repository(repo, [SHOP])
            S.select(repo, out)
            self.assertIn(("Screen", "Invoice"), traces(out))

    def test_a_path_the_class_graph_joins_but_no_method_chain_makes_is_dropped(self):
        split = {**SHOP, "android/shop/Checkout.java": (
            "package shop;\n\npublic class Checkout {\n    private final Invoice invoice = null;\n\n"
            "    public void pay() {}\n\n    public void refund() {\n        invoice.send();\n    }\n}\n")}
        with tempfile.TemporaryDirectory() as d:
            repo, out = Path(d) / "corpus", Path(d) / "out"
            repo.mkdir()
            repository(repo, [split])
            graph = S.T.di_call_graph(repo, ["android"])
            self.assertEqual(S.T.shortest_path(graph, "Screen", "Invoice"), ["Screen", "Checkout.pay", "Invoice.send"])
            S.select(repo, out)
            self.assertEqual(traces(out), [("Checkout", "Invoice"), ("Screen", "Checkout")])

    def test_a_call_written_only_in_a_comment_is_no_hop(self):
        commented = {**SHOP, "android/shop/Checkout.java": (
            "package shop;\n\npublic class Checkout {\n    private final Invoice invoice = null;\n\n"
            "    public void pay() {\n        // invoice.send();\n    }\n}\n")}
        with tempfile.TemporaryDirectory() as d:
            repo, out = Path(d) / "corpus", Path(d) / "out"
            repo.mkdir()
            repository(repo, [commented])
            self.assertIn("Invoice.send", S.T.di_call_graph(repo, ["android"])["edges"]["Checkout"])
            S.select(repo, out)
            self.assertEqual(traces(out), [("Screen", "Checkout")])
