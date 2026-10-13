import io
import json
import os
import tempfile
import unittest
from contextlib import redirect_stdout

import langdiff


def node(nid, file, body="", line=1, community=None):
    return {"id": nid, "kind": "Symbol", "label": nid, "body": body, "file": file, "line": line, "end": line, "files": [file], "community": community}


def edge(source, target, kind="Calls", file="a.sh"):
    return {"source": source, "target": target, "kind": kind, "context": "", "file": file}


BASE_NODES = [
    node("file:a.sh", "a.sh"),
    node("sym:a.sh::f", "a.sh", "# Says hi.\nf() {"),
    node("sym:a.sh::g", "a.sh", "g() {"),
    node("file:b.py", "b.py"),
    node("sym:b.py::h", "b.py", "def h():"),
]
BASE_EDGES = [
    edge("file:a.sh", "sym:a.sh::f", "Declares"),
    edge("file:a.sh", "sym:a.sh::g", "Declares"),
    edge("sym:a.sh::f", "sym:a.sh::g"),
    edge("file:b.py", "sym:b.py::h", "Declares", "b.py"),
]


class Diff(unittest.TestCase):
    def store(self, root, name, nodes, edges, pending=0, headers=b"{}"):
        d = os.path.join(root, name)
        os.makedirs(d)
        g = {"nodes": {n["id"]: n for n in nodes}, "edges": edges, "pending": [edge("x", "ADR-1", "References")] * pending}
        with open(os.path.join(d, "graph.json"), "w") as f:
            json.dump(g, f)
        with open(os.path.join(d, "headers.json"), "wb") as f:
            f.write(headers)
        return d

    def run_diff(self, nodes, edges, langs={"sh"}, **kw):
        with tempfile.TemporaryDirectory() as root:
            old = self.store(root, "old", BASE_NODES, BASE_EDGES)
            new = self.store(root, "new", nodes, edges, **kw)
            return langdiff.diff(old, new, langs)

    def test_identical_stores_have_no_regression(self):
        r = self.run_diff(BASE_NODES, BASE_EDGES)
        self.assertEqual(r["regressions"], 0)

    def test_a_moved_call_in_the_ported_language_is_bounded_not_a_regression(self):
        edges = BASE_EDGES[:2] + [edge("sym:a.sh::g", "sym:a.sh::f")] + BASE_EDGES[3:]
        r = self.run_diff(BASE_NODES, edges)
        self.assertEqual(r["regressions"], 0)
        self.assertEqual(len(r["added"][("sh", "Calls")]), 1)
        self.assertEqual(len(r["missed"][("sh", "Calls")]), 1)

    def test_a_lost_declaration_is_r4_and_a_lost_node_r1(self):
        nodes = [n for n in BASE_NODES if n["id"] != "sym:a.sh::g"]
        edges = [e for e in BASE_EDGES if e["target"] != "sym:a.sh::g"]
        r = self.run_diff(nodes, edges)
        self.assertEqual(r["counts"]["sh"], {"R1": 1, "R4": 1})

    def test_an_edge_kind_in_neither_list_is_r4_not_silently_dropped(self):
        edges = BASE_EDGES + [edge("sym:a.sh::f", "sym:a.sh::g", "Legacy")]
        r = self.run_diff(BASE_NODES, edges)
        self.assertEqual(r["counts"]["sh"], {"R4": 1})

    def test_a_signature_change_is_r3_and_a_doc_change_is_accepted(self):
        nodes = [dict(n) for n in BASE_NODES]
        nodes[1]["body"] = "# Says hello.\nf() {"
        nodes[2]["body"] = "g () {"
        r = self.run_diff(nodes, BASE_EDGES)
        self.assertEqual(r["counts"]["sh"], {"R3": 1})
        self.assertEqual(r["accepted"], {"sh": 1})

    def test_any_change_in_another_language_is_r6(self):
        nodes = [dict(n) for n in BASE_NODES]
        nodes[4]["line"] = 2
        nodes[4]["body"] = "# doc\ndef h():"
        edges = BASE_EDGES + [edge("sym:b.py::h", "sym:b.py::h", "Calls", "b.py")]
        r = self.run_diff(nodes, edges)
        self.assertEqual(r["counts"]["py"], {"R6": 3})

    def test_a_dangling_edge_is_r5(self):
        edges = BASE_EDGES[:2] + [edge("sym:a.sh::f", "sym:a.sh::gone")] + BASE_EDGES[3:]
        r = self.run_diff(BASE_NODES, edges)
        self.assertEqual(r["counts"]["sh"], {"R5": 1})

    def test_headers_pending_and_community_are_store_wide(self):
        nodes = [dict(n) for n in BASE_NODES]
        nodes[0]["community"] = 3
        r = self.run_diff(nodes, BASE_EDGES, pending=1, headers=b"{\"a\":1}")
        self.assertEqual(r["store"], {"R7": 1, "R8": 1, "R9": 1})

    def test_the_exit_code_reads_the_regressions_and_the_sample_is_seeded(self):
        with tempfile.TemporaryDirectory() as root:
            old = self.store(root, "old", BASE_NODES, BASE_EDGES)
            same = self.store(root, "same", BASE_NODES, BASE_EDGES)
            moved = self.store(root, "moved", BASE_NODES, BASE_EDGES[:2] + [edge("sym:a.sh::g", "sym:a.sh::f")] + BASE_EDGES[3:])
            lost = self.store(root, "lost", BASE_NODES[:2] + BASE_NODES[3:], BASE_EDGES[:1] + BASE_EDGES[3:])
            out = io.StringIO()
            with redirect_stdout(out):
                self.assertEqual(langdiff.main(["langdiff.py", "--lang", "sh", old, same]), 0)
                self.assertEqual(langdiff.main(["langdiff.py", "--lang", "sh", "--sample", "1", "--seed", "4", old, moved]), 0)
                self.assertEqual(langdiff.main(["langdiff.py", "--lang", "sh", old, lost]), 1)
            self.assertIn("sh Calls: +1", out.getvalue())


if __name__ == "__main__":
    unittest.main()
