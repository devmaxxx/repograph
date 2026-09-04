import json
import tempfile
import unittest
from pathlib import Path

import track

TRANSCRIPT = """\
keyword    FR-AI-138    HIT   143 tok  расход виден салону
keyword    FR-WH-53     miss  210 tok  склад списание
paraphrase FR-PH-43     miss  198 tok  как клиент платит телефоном
code       apps/api/src/main.ts HIT   88 tok  где точка входа
paraphrase W-206        HIT   512 tok  предупреждение о переносе

keyword 37/40  paraphrase 15/30  code 12/12  p90 220 tok  dense=true  enriched=true (1996/1996 nodes)
"""

PASSES = """\
pub fn passes(s: &Summary, dense: bool, enriched: bool) -> bool {
    // a comment mentioning (true, true) => (99, 99) inside prose
    let (keyword, paraphrase) = match (enriched, dense) {
        // and one inside the block: (true, true) => (98, 98) was the old pair
        (true, true) => (40, 14),
        (true, false) => (40, 11),
        (false, true) => (40, 9),
        (false, false) => (39, 7),
    };
    s.keyword.0 >= keyword && s.paraphrase.0 >= paraphrase && s.code.0 >= 12 && s.p90_tokens <= 230
}
"""


class ParseBench(unittest.TestCase):
    def test_reads_every_case_and_the_summary(self):
        p = track.parse_bench(TRANSCRIPT)
        self.assertEqual(p["metrics"]["keyword"], [37, 40])
        self.assertEqual(p["metrics"]["paraphrase"], [15, 30])
        self.assertEqual(p["metrics"]["p90_tokens"], 220)
        self.assertTrue(p["dense"])
        self.assertTrue(p["enriched"])
        self.assertEqual(p["coverage"], [1996, 1996])
        self.assertEqual(p["cases"]["keyword/FR-AI-138"], 1.0)
        self.assertEqual(p["cases"]["keyword/FR-WH-53"], 0.0)
        self.assertEqual(p["cases"]["code/apps/api/src/main.ts"], 1.0)
        self.assertEqual(p["tokens"]["paraphrase/W-206"], 512)

    def test_a_run_that_never_reached_the_summary_is_refused(self):
        # A crashed or interrupted run must not enter the history as a row of zeroes.
        with self.assertRaises(SystemExit):
            track.parse_bench("keyword FR-AI-138    HIT   143 tok  расход\n")

    def test_rerank_arms_are_named_apart(self):
        line = ("\nkeyword 40/40  paraphrase 18/30  code 12/12  p90 228 tok  dense=true  "
                "enriched=true (1996/1996 nodes) rerank_local=true depth=200\n")
        p = track.parse_bench(TRANSCRIPT.rsplit("\n\n", 1)[0] + line)
        self.assertEqual(p["rerank"], "local")
        self.assertEqual(p["depth"], 200)
        self.assertEqual(track.arm_name(p), "bench:dense+enriched+rerank-local-200")

    def test_arm_names_separate_the_four_grading_arms(self):
        base = dict(rerank=None, depth=None)
        names = {track.arm_name(dict(dense=d, enriched=e, **base))
                 for d in (True, False) for e in (True, False)}
        self.assertEqual(len(names), 4)


class Floors(unittest.TestCase):
    def test_reads_the_four_arms_from_the_rust_source(self):
        with tempfile.NamedTemporaryFile("w", suffix=".rs", delete=False) as f:
            f.write(PASSES)
            path = Path(f.name)
        f = track.floors(path)
        self.assertEqual(f[(True, True)], {"keyword": 40, "paraphrase": 14, "code": 12, "p90_tokens": 230})
        self.assertEqual(f[(False, False)], {"keyword": 39, "paraphrase": 7, "code": 12, "p90_tokens": 230})

    def test_a_changed_shape_fails_loudly(self):
        # Silently falling back to remembered floors would make every headroom figure wrong
        # for as long as nobody noticed.
        with tempfile.NamedTemporaryFile("w", suffix=".rs", delete=False) as f:
            f.write("pub fn passes() -> bool { true }\n")
            path = Path(f.name)
        with self.assertRaises(SystemExit):
            track.floors(path)

    def test_the_live_source_still_parses(self):
        f = track.floors()
        self.assertEqual(set(f), {(True, True), (True, False), (False, True), (False, False)})
        for arm in f.values():
            self.assertGreater(arm["keyword"], 0)
            self.assertGreater(arm["p90_tokens"], 0)


class Scored(unittest.TestCase):
    def test_each_suite_is_read_in_its_own_shape(self):
        self.assertEqual(track.scored({"kind": "keyword", "expect": "FR-1", "strict": True}),
                         ("keyword/FR-1", 1.0))
        self.assertEqual(track.scored({"kind": "impact", "target": "Db", "recall": 0.857}),
                         ("impact/Db", 0.857))
        self.assertEqual(track.scored({"kind": "trace", "from": "A", "to": "B", "hit": False}),
                         ("trace/A->B", 0.0))
        self.assertEqual(track.scored(
            {"kind": "changes", "base": "bc9db289~1", "found_symbols": 27, "want_symbols": 38}),
            ("changes/bc9db289~1", 0.7105))

    def test_an_unknown_kind_raises_rather_than_scoring_zero(self):
        # A silent zero would enter the history as a regression that never happened.
        with self.assertRaises(SystemExit):
            track.scored({"kind": "routes", "expect": "x"})

    def test_a_changes_case_wanting_nothing_is_not_a_division_by_zero(self):
        self.assertEqual(track.scored(
            {"kind": "changes", "base": "b", "found_symbols": 0, "want_symbols": 0})[1], 1.0)


class Headroom(unittest.TestCase):
    def test_negative_under_the_bar_positive_over_it(self):
        m = {"keyword": [37, 40], "paraphrase": [15, 30], "code": [12, 12], "p90_tokens": 220}
        r = track.headroom(m, {"keyword": 40, "paraphrase": 14, "code": 12, "p90_tokens": 230})
        self.assertEqual(r["keyword"], -3)
        self.assertEqual(r["paraphrase"], 1)
        self.assertEqual(r["code"], 0)
        self.assertEqual(r["p90_tokens"], 10)


def run(cases):
    return {"cases": cases, "when": "t", "source": "bench", "metrics": {}}


class Analysis(unittest.TestCase):
    def test_delta_separates_what_was_fixed_from_what_broke(self):
        fixed, broke = track.delta(run({"a": 0.0, "b": 1.0, "c": 1.0}),
                                   run({"a": 1.0, "b": 0.0, "c": 1.0}))
        self.assertEqual(fixed, ["a"])
        self.assertEqual(broke, ["b"])

    def test_a_case_absent_from_the_earlier_run_is_not_a_regression(self):
        # The suite grows. A case with no predecessor has not moved.
        fixed, broke = track.delta(run({"a": 1.0}), run({"a": 1.0, "new": 0.0}))
        self.assertEqual((fixed, broke), ([], []))

    def test_partial_recall_moves_are_reported_with_both_values(self):
        fixed, broke = track.delta(run({"i": 0.6}), run({"i": 0.9}))
        self.assertEqual(fixed, ["i 0.6->0.9"])
        self.assertEqual(broke, [])

    def test_chronic_counts_misses_across_the_window(self):
        w = [run({"a": 0.0, "b": 1.0}), run({"a": 0.0, "b": 0.0}), run({"a": 0.0, "b": 1.0})]
        self.assertEqual(track.weak(w, 0.66), ["a (3/3, mean 0.00)"])

    def test_a_case_seen_once_is_not_yet_chronic(self):
        # One appearance cannot show a pattern; calling it chronic invents a trend.
        w = [run({"a": 0.0, "b": 1.0}), run({"a": 0.0}), run({"a": 0.0, "c": 0.0})]
        self.assertEqual(track.weak(w, 0.66), ["a (3/3, mean 0.00)"])

    def test_a_high_recall_chronic_reads_apart_from_a_zero(self):
        w = [run({"i": 0.98}), run({"i": 0.98})]
        self.assertEqual(track.weak(w, 0.66), ["i (2/2, mean 0.98)"])

    def test_flaky_needs_more_than_one_flip(self):
        # One flip is a fix or a regression and is reported as such; two is instability.
        once = [run({"a": 0.0}), run({"a": 0.0}), run({"a": 1.0})]
        twice = [run({"a": 0.0}), run({"a": 1.0}), run({"a": 0.0})]
        self.assertEqual(track.flaky(once), [])
        self.assertEqual(track.flaky(twice), ["a (2 flips)"])

    def test_a_moved_corpus_is_not_a_before_and_after(self):
        # The mistake this history exists to stop making: a `changes` case is a diff against
        # the corpus HEAD, so a corpus that moved changed the question as well as the answer.
        a = {"corpus_commit": "7733bd53", "cases": {"x": 1.0}}
        b = {"corpus_commit": "502e8a6d", "cases": {"x": 0.0}}
        self.assertIn("the corpus moved", track.incomparable(a, b))

    def test_a_grown_case_set_is_not_a_before_and_after(self):
        a = {"corpus_commit": "c", "cases": {"x": 1.0}}
        b = {"corpus_commit": "c", "cases": {"x": 1.0, "y": 0.0}}
        self.assertIn("the case set changed", track.incomparable(a, b))

    def test_moved_floors_break_comparability(self):
        a = {"corpus_commit": "c", "cases": {}, "floors": {"keyword": 40}}
        b = {"corpus_commit": "c", "cases": {}, "floors": {"keyword": 39}}
        self.assertIn("the floors moved", track.incomparable(a, b))

    def test_two_runs_of_one_setup_are_comparable(self):
        a = {"corpus_commit": "c", "cases": {"x": 1.0}, "floors": {"keyword": 40}}
        b = {"corpus_commit": "c", "cases": {"x": 0.0}, "floors": {"keyword": 40}}
        self.assertIsNone(track.incomparable(a, b))

    def test_metric_moves_only_report_what_changed(self):
        prev = {"metrics": {"keyword": [37, 40], "p90_tokens": 220}}
        now = {"metrics": {"keyword": [40, 40], "p90_tokens": 220}}
        self.assertEqual(track.metric_moves(prev, now), [("keyword", "37/40", "40/40")])


if __name__ == "__main__":
    unittest.main()
