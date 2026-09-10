import json
import os
import tempfile
import unittest

import graphdiff


def graph(nodes, edges, pending=None):
    g = {"nodes": {n: {"id": n, "kind": "Requirement", "label": n, "file": "docs/a.md", "line": 1} for n in nodes},
         "edges": [{"source": s, "target": t, "kind": "References", "context": "body", "file": "docs/a.md"} for s, t in edges]}
    if pending is not None:
        g["pending"] = [{"source": s, "target": t, "kind": "References", "context": "body", "file": "docs/a.md"} for s, t in pending]
    return g


class Diff(unittest.TestCase):
    def write(self, d, name, g):
        p = os.path.join(d, name)
        with open(p, "w") as f:
            json.dump(g, f)
        return p

    def test_the_same_visible_graph_with_edges_held_aside_reads_same(self):
        with tempfile.TemporaryDirectory() as d:
            a = self.write(d, "a.json", graph(["FR-1"], [("FR-1", "FR-2")]))
            b = self.write(d, "b.json", graph(["FR-1"], [("FR-1", "FR-2")], pending=[("FR-1", "ISO-8601")]))
            r = graphdiff.diff(a, b)
            self.assertEqual((r["nodes_same"], r["edges_same"], r["pending"]), (True, True, 1))
            self.assertGreater(r["bytes_ratio"], 1.0)

    def test_a_missing_or_extra_edge_is_named(self):
        with tempfile.TemporaryDirectory() as d:
            a = self.write(d, "a.json", graph(["FR-1"], [("FR-1", "FR-2")]))
            b = self.write(d, "b.json", graph(["FR-1"], [("FR-1", "FR-3")]))
            r = graphdiff.diff(a, b)
            self.assertFalse(r["edges_same"])
            self.assertEqual(r["edges_only_a"], ["FR-1 -> FR-2 [References/body] docs/a.md"])
            self.assertEqual(r["edges_only_b"], ["FR-1 -> FR-3 [References/body] docs/a.md"])

    def test_a_node_whose_label_changed_is_named_with_both_labels(self):
        with tempfile.TemporaryDirectory() as d:
            ga, gb = graph(["FR-1"], []), graph(["FR-1"], [])
            gb["nodes"]["FR-1"]["label"] = "title"
            a, b = self.write(d, "a.json", ga), self.write(d, "b.json", gb)
            r = graphdiff.diff(a, b)
            self.assertFalse(r["nodes_same"])
            self.assertEqual(r["nodes_changed"], [("FR-1", "label", "FR-1", "title")])


if __name__ == "__main__":
    unittest.main()
