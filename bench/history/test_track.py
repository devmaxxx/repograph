import argparse
import json
import tempfile
import unittest
from pathlib import Path

import track

TRANSCRIPT = """\
keyword    FR-AI-138    HIT  1/1  143 tok  расход виден салону
keyword    FR-WH-53     miss 0/1  210 tok  склад списание
paraphrase FR-PH-43     miss 0/1  198 tok  как клиент платит телефоном
code       apps/api/src/main.ts HIT  1/1   88 tok  где точка входа
paraphrase W-206        HIT  1/1  512 tok  предупреждение о переносе

keyword 37/40  paraphrase 15/30  code 12/12  p90 220 tok  dense=true  enriched=true (1996/1996 nodes)  suite=built-in gated=true
anchors  keyword 37/40  paraphrase 15/30  code 12/12
"""

# What `bench` printed before the dev suite: no reached/want pair, no suite field.
OLD_TRANSCRIPT = """\
keyword    FR-AI-138    HIT   143 tok  расход виден салону
keyword    FR-WH-53     miss  210 tok  склад списание
code       apps/api/src/main.ts HIT   88 tok  где точка входа

keyword 37/40  paraphrase 15/30  code 12/12  p90 220 tok  dense=true  enriched=true (1996/1996 nodes)
"""

DEV_TRANSCRIPT = """\
long       FR-CAL-96    HIT  1/1  201 tok  Гость набронировал пять окон и не приходит
cross      FR-DM-30+packages/domain/src/availability/segments.ts miss 0/2  190 tok  где считается футпринт
multi      FR-CAL-105+FR-CAL-106+FR-CAL-107 HIT  1/3  240 tok  лист ожидания целиком
where      apps/api/src/modules/staff/staff.controller.ts HIT  1/1  150 tok  куда класть эндпоинт

long 1/1  cross 0/1  multi 1/1  where 1/1  p90 240 tok  dense=true  enriched=true (1996/1996 nodes)  suite=dev-cases gated=false
anchors  long 1/1  cross 0/2  multi 1/3  where 1/1
"""

# `bench --repeat 2`: an anchors line under each run and one more under the median, whose counts
# are on a line no summary regex matches.
REPEAT_TRANSCRIPT = """\
keyword    FR-AI-138    HIT  1/1  143 tok  расход виден салону

keyword 36/40  paraphrase 14/30  code 12/12  p90 221 tok  dense=true  enriched=true (1996/1996 nodes)  suite=built-in gated=true
anchors  keyword 36/40  paraphrase 14/30  code 12/12

keyword    FR-AI-138    HIT  1/1  143 tok  расход виден салону

keyword 37/40  paraphrase 15/30  code 12/12  p90 220 tok  dense=true  enriched=true (1996/1996 nodes)  suite=built-in gated=true
anchors  keyword 37/40  paraphrase 15/30  code 12/12

median of 2  keyword 36/40  paraphrase 14/30  code 12/12  p90 220 tok
anchors  keyword 36/40  paraphrase 14/30  code 12/12
"""

# $G/t5-large-enriched-1.txt, measured before `model=` existed on the summary line.
LARGE_ENRICHED_NO_MODEL = """\
keyword 40/40  paraphrase 22/30  code 12/12  p90 224 tok  dense=true  enriched=true (1996/1996 nodes)  suite=built-in gated=true
"""

DUPLICATE_ANCHOR_TRANSCRIPT = """\
rule       ADR-031      HIT  1/1  180 tok  можно ли создать второй визит
rule       ADR-031      miss 0/1  190 tok  повторный POST плодит бронь

rule 1/2  p90 190 tok  dense=true  enriched=true (1996/1996 nodes)  suite=dev-cases gated=false
"""

PROSE = """\
// Sixteen lines of prose about the floors sit above `passes` in the real file, and they
// discuss the floors in the notation the code uses: s.kind("code").0 >= 11 && s.p90_tokens <= 200
// was the shape before the ceiling was rounded.
"""

FLOORS_TABLE = """\
const FLOORS: [(bool, bool, Floors, usize, usize); 6] = [
    (true, true, Floors::Small, 40, 14),
    (true, false, Floors::Small, 40, 11),
    (false, true, Floors::Small, 40, 9),
    (false, false, Floors::Small, 39, 7),
    (true, true, Floors::Large, 40, 22),
    (false, true, Floors::Large, 40, 17),
];
"""

PASSES = """\
pub fn passes(s: &Summary, dense: bool, enriched: bool, floors: Floors) -> bool {
    // a comment mentioning (true, true, Floors::Small, 99, 99) inside prose
    if dense && floors == Floors::None { return false; }
    let key = if dense { floors } else { Floors::Small };
    let Some(&(_, _, _, keyword, paraphrase)) = FLOORS.iter().find(|r| r.0 == enriched && r.1 == dense && r.2 == key) else { return false };
    s.kind("keyword").0 >= keyword && s.kind("paraphrase").0 >= paraphrase && s.kind("code").0 >= 12 && s.p90_tokens <= 230
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

    def test_the_embedder_line_arms_sh_heads_a_transcript_with_reads_as_nothing(self):
        # `bench/probe/arms.sh` writes the hub id the run's floors were keyed from above the arm's
        # own output, so a recorded transcript says which weights answered it. A line that parsed as
        # a case would put a score under a key no suite has, into a history that is append-only.
        p = track.parse_bench("embedder: REPOGRAPH_EMBED_MODEL=intfloat/multilingual-e5-small\n" + TRANSCRIPT)
        self.assertEqual(p["cases"], track.parse_bench(TRANSCRIPT)["cases"])
        self.assertEqual(track.arm_name(p), track.arm_name(track.parse_bench(TRANSCRIPT)))

    def test_a_transcript_from_before_the_suite_field_still_reads(self):
        p = track.parse_bench(OLD_TRANSCRIPT)
        self.assertEqual(p["cases"]["keyword/FR-WH-53"], 0.0)
        self.assertEqual(p["cases"]["code/apps/api/src/main.ts"], 1.0)
        self.assertEqual(p["suite"], "built-in")
        self.assertTrue(p["gated"], "the recorded shape was always graded")
        self.assertEqual(track.arm_name(p), "bench:dense+enriched")

    def test_a_dev_suite_transcript_reads_its_kinds_shares_and_suite(self):
        p = track.parse_bench(DEV_TRANSCRIPT)
        self.assertEqual(list(p["metrics"]), ["long", "cross", "multi", "where", "p90_tokens"])
        self.assertEqual(p["metrics"]["multi"], [1, 1], "the summary counts entry points")
        self.assertAlmostEqual(p["cases"]["multi/FR-CAL-105+FR-CAL-106+FR-CAL-107"], 0.3333, places=4)
        self.assertEqual(p["cases"]["cross/FR-DM-30+packages/domain/src/availability/segments.ts"], 0.0)
        self.assertEqual(p["suite"], "dev-cases")
        self.assertFalse(p["gated"])
        self.assertEqual(track.arm_name(p), "bench[dev-cases]:dense+enriched")

    def test_two_cases_of_one_kind_expecting_the_same_anchor_keep_their_own_scores(self):
        # The dev suite asks two `rule` questions about ADR-031. Keyed by kind and anchor alone
        # the second overwrites the first, and the row carries one score fewer than the suite
        # has cases -- a case that can never read as weak because it is never recorded.
        p = track.parse_bench(DUPLICATE_ANCHOR_TRANSCRIPT)
        self.assertEqual(p["cases"], {"rule/ADR-031": 1.0, "rule/ADR-031#2": 0.0})
        self.assertEqual(p["tokens"], {"rule/ADR-031": 180, "rule/ADR-031#2": 190})

    def test_the_anchor_line_is_read_per_kind(self):
        p = track.parse_bench(DEV_TRANSCRIPT)
        self.assertEqual(p["anchors"], {"long": [1, 1], "cross": [0, 2], "multi": [1, 3], "where": [1, 1]})

    def test_a_transcript_without_the_anchor_line_records_none(self):
        self.assertIsNone(track.parse_bench(OLD_TRANSCRIPT)["anchors"])

    def test_a_run_whose_own_anchor_line_is_gone_does_not_borrow_the_median_s(self):
        # A stderr write landing on that line under `2>&1` is enough to lose it. Read on to the end
        # of the file, the next `anchors` line is the median's -- 36/40 against counts of 37/40, a
        # pair of numbers from two different runs, in a row nothing afterwards can correct. The
        # row holding no anchors says what happened; the row holding the median's says something
        # that never happened.
        lost = REPEAT_TRANSCRIPT.replace("anchors  keyword 37/40  paraphrase 15/30  code 12/12\n", "")
        p = track.parse_bench(lost)
        self.assertEqual(p["metrics"]["keyword"], [37, 40])
        self.assertIsNone(p["anchors"])

    def test_the_cases_belong_to_the_run_the_row_records(self):
        # Read over the whole file, the second run's copy of a case would land under the `#2`
        # suffix that exists for two questions about one place inside one run: the row would carry
        # two copies of every case, and the next single run of the arm would read `the case set
        # changed` against a history that cannot be corrected afterwards.
        p = track.parse_bench(REPEAT_TRANSCRIPT)
        self.assertEqual(p["cases"], {"keyword/FR-AI-138": 1.0})
        self.assertEqual(p["tokens"], {"keyword/FR-AI-138": 143})

    def test_the_anchors_belong_to_the_summary_the_row_records(self):
        # `--repeat` prints the median's anchors last, and the row records the last run's summary:
        # the last line in the transcript is the wrong line to pair with those counts.
        p = track.parse_bench(REPEAT_TRANSCRIPT)
        self.assertEqual(p["metrics"]["keyword"], [37, 40])
        self.assertEqual(p["anchors"]["keyword"], [37, 40])

    def test_the_row_carries_the_anchors(self):
        table = {(True, True, "small"): {"keyword": 40, "paraphrase": 14, "code": 12, "p90_tokens": 230}}
        row = track.build_row(track.parse_bench(DEV_TRANSCRIPT), "beauty-crm", "502e8a6d", "", "abc", False, table)
        self.assertEqual(row["anchors"]["multi"], [1, 3])

    def test_anchor_moves_are_reported_beside_metric_moves(self):
        prev = {"metrics": {"multi": [9, 12]}, "anchors": {"multi": [16, 39]}}
        latest = {"metrics": {"multi": [9, 12]}, "anchors": {"multi": [13, 39]}}
        self.assertEqual(track.metric_moves(prev, latest), [("anchors/multi", "16/39", "13/39")])

    def test_a_row_without_anchors_moves_nothing(self):
        prev = {"metrics": {"multi": [9, 12]}}
        latest = {"metrics": {"multi": [9, 12]}, "anchors": {"multi": [13, 39]}}
        self.assertEqual(track.metric_moves(prev, latest), [])

    def test_a_row_for_an_ungated_run_carries_no_floors_and_no_verdict(self):
        table = {(True, True, "small"): {"keyword": 40, "paraphrase": 14, "code": 12, "p90_tokens": 230}}
        row = track.build_row(track.parse_bench(DEV_TRANSCRIPT), "beauty-crm", "502e8a6d", "", "abc", False, table)
        self.assertEqual((row["floors"], row["headroom"], row["green"]), (None, None, None))
        self.assertEqual((row["suite"], row["gated"]), ("dev-cases", False))
        self.assertEqual(track.state_of(row), "measured, no floors")
        graded = track.build_row(track.parse_bench(TRANSCRIPT), "beauty-crm", "502e8a6d", "", "abc", False, table)
        self.assertEqual(graded["headroom"]["keyword"], -3)
        self.assertFalse(graded["green"])
        self.assertEqual(track.state_of(graded), "RED")

    def test_a_transcript_without_the_model_field_is_a_small_model_run(self):
        # A transcript from before the model field existed is what every store was, back then.
        p = track.parse_bench(LARGE_ENRICHED_NO_MODEL)
        self.assertEqual(p["model"], "small")
        self.assertEqual(track.arm_name(p), "bench:dense+enriched")

    def test_a_large_model_run_is_graded_on_its_own_floors_and_named_apart(self):
        line = LARGE_ENRICHED_NO_MODEL.replace(" (1996/1996 nodes)", " (1996/1996 nodes) model=large")
        p = track.parse_bench(line)
        self.assertEqual(p["model"], "large")
        self.assertEqual(track.arm_name(p), "bench:dense+enriched+large")
        table = {(True, True, "small"): {"keyword": 40, "paraphrase": 14, "code": 12, "p90_tokens": 230},
                 (True, True, "large"): {"keyword": 40, "paraphrase": 22, "code": 12, "p90_tokens": 230}}
        row = track.build_row(p, "beauty-crm", "502e8a6d", "", "abc", False, table)
        self.assertTrue(row["gated"])
        self.assertTrue(row["green"])
        self.assertEqual(row["floors"]["paraphrase"], 22, "the large floor, not the small model's 14")

    def test_build_row_refuses_to_grade_a_model_the_table_has_no_floors_for(self):
        # This binary always prints gated=false for a model it holds no floors for, so a
        # gated=true transcript naming one is not something this build emits -- only a
        # different one. build_row re-derives gated from the table rather than trusting it.
        unknown_model_line = ("keyword 40/40  paraphrase 15/30  code 12/12  p90 220 tok  dense=true  enriched=true "
                              "(1996/1996 nodes)  suite=built-in gated=true model=BAAI/bge-m3\n")
        p = track.parse_bench(unknown_model_line)
        self.assertEqual(p["model"], "BAAI/bge-m3")
        table = {(True, True, "small"): {"keyword": 40, "paraphrase": 14, "code": 12, "p90_tokens": 230}}
        row = track.build_row(p, "beauty-crm", "502e8a6d", "", "abc", False, table)
        self.assertFalse(row["gated"])
        self.assertEqual((row["floors"], row["headroom"], row["green"]), (None, None, None))
        self.assertEqual(track.state_of(row), "measured, no floors")

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
        base = dict(rerank=None, depth=None, model="small")
        names = {track.arm_name(dict(dense=d, enriched=e, **base))
                 for d in (True, False) for e in (True, False)}
        self.assertEqual(len(names), 4)

    def test_a_tag_names_a_store_copy_s_rows_apart_from_the_fixture_s(self):
        recorded = []
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "bench.txt"
            path.write_text(TRANSCRIPT)
            original, track.append = track.append, recorded.append
            try:
                for tag in ("r1", None):
                    track.cmd_record(argparse.Namespace(
                        transcript=str(path), corpus="beauty-crm", corpus_path=None, note="", tag=tag))
            finally:
                track.append = original
        self.assertEqual([r["arm"] for r in recorded],
                         ["bench:dense+enriched+r1", "bench:dense+enriched"])


class Floors(unittest.TestCase):
    def test_reads_the_six_arms_from_the_rust_source(self):
        with tempfile.NamedTemporaryFile("w", suffix=".rs", delete=False) as f:
            f.write(FLOORS_TABLE + PASSES)
            path = Path(f.name)
        f = track.floors(path)
        self.assertEqual(len(f), 6)
        self.assertEqual(f[(True, True, "small")], {"keyword": 40, "paraphrase": 14, "code": 12, "p90_tokens": 230})
        self.assertEqual(f[(False, False, "small")], {"keyword": 39, "paraphrase": 7, "code": 12, "p90_tokens": 230})
        self.assertEqual(f[(True, True, "large")], {"keyword": 40, "paraphrase": 22, "code": 12, "p90_tokens": 230})

    def test_a_changed_shape_fails_loudly(self):
        # Silently falling back to remembered floors would make every headroom figure wrong
        # for as long as nobody noticed.
        with tempfile.NamedTemporaryFile("w", suffix=".rs", delete=False) as f:
            f.write("pub fn passes() -> bool { true }\n")
            path = Path(f.name)
        with self.assertRaises(SystemExit):
            track.floors(path)

    def test_prose_above_passes_cannot_supply_the_code_floor_or_the_ceiling(self):
        # The one way this parser could be wrong without saying so: reading a sentence about
        # the floors instead of the floors.
        with tempfile.NamedTemporaryFile("w", suffix=".rs", delete=False) as f:
            f.write(PROSE + FLOORS_TABLE + PASSES)
            path = Path(f.name)
        arm = track.floors(path)[(True, True, "small")]
        self.assertEqual(arm["code"], 12)
        self.assertEqual(arm["p90_tokens"], 230)

    def test_a_block_comment_inside_passes_cannot_supply_the_floors(self):
        body = PASSES.replace("    let key = if dense",
                              "    /* s.code.0 >= 11 && s.p90_tokens <= 200 */\n    let key = if dense")
        with tempfile.NamedTemporaryFile("w", suffix=".rs", delete=False) as f:
            f.write(FLOORS_TABLE + body)
            path = Path(f.name)
        arm = track.floors(path)[(True, True, "small")]
        self.assertEqual((arm["code"], arm["p90_tokens"]), (12, 230))

    def test_a_sibling_function_cannot_supply_the_floors(self):
        # Braces are matched rather than trusting a closing one at column zero, so moving
        # `passes` into an impl cannot silently extend its body over the next function.
        inside_impl = ("impl Gate {\n" + "\n".join("    " + l for l in PASSES.splitlines()) +
                       "\n    fn other(s: &Summary) -> bool {\n"
                       "        s.code.0 >= 11 && s.p90_tokens <= 200\n    }\n}\n")
        with tempfile.NamedTemporaryFile("w", suffix=".rs", delete=False) as f:
            f.write(FLOORS_TABLE + inside_impl)
            path = Path(f.name)
        arm = track.floors(path)[(True, True, "small")]
        self.assertEqual((arm["code"], arm["p90_tokens"]), (12, 230))

    def test_the_live_source_still_parses(self):
        f = track.floors()
        self.assertEqual(set(f), {(True, True, "small"), (True, False, "small"),
                                   (False, True, "small"), (False, False, "small"),
                                   (True, True, "large"), (False, True, "large")})
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


class Ordering(unittest.TestCase):
    def test_rows_are_read_newest_last_whatever_order_the_file_holds(self):
        # runs.jsonl is merged with `merge=union`, so file order is not append order after two
        # branches meet, and "the latest run" would otherwise be whichever line landed last.
        import json as _json
        d = Path(tempfile.mkdtemp())/"runs.jsonl"
        rows = [{"when": "2026-09-04T18:00:00+00:00", "arm": "a", "cases": {}},
                {"when": "2026-09-02T10:00:00", "arm": "a", "cases": {}},
                {"when": "2026-09-03T10:00:00+00:00", "arm": "a", "cases": {}}]
        d.write_text("\n".join(_json.dumps(r) for r in rows))
        before, track.RUNS = track.RUNS, d
        try:
            self.assertEqual([r["when"] for r in track.load()],
                             ["2026-09-02T10:00:00", "2026-09-03T10:00:00+00:00",
                              "2026-09-04T18:00:00+00:00"])
        finally:
            track.RUNS = before


class ToolDirty(unittest.TestCase):
    def test_the_run_file_alone_does_not_make_the_tree_dirty(self):
        import subprocess
        with tempfile.TemporaryDirectory() as d:
            repo = Path(d)
            runs = repo / track.RUNS.relative_to(track.REPO)
            runs.parent.mkdir(parents=True)
            runs.write_text("{}\n")
            (repo / "other.txt").write_text("x")
            env = {"GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t",
                   "GIT_COMMITTER_EMAIL": "t@t", "PATH": "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin"}
            for cmd in (["init", "-q"], ["add", "."], ["commit", "-q", "-m", "init"]):
                subprocess.run(["git", "-C", d, *cmd], check=True, env=env, capture_output=True)
            self.assertFalse(track.tool_dirty(repo))
            # Recording the first arm appends to the run file; the second arm is not "dirty".
            with runs.open("a") as f:
                f.write("{}\n")
            self.assertFalse(track.tool_dirty(repo))
            # Anything else being modified is.
            (repo / "other.txt").write_text("y")
            self.assertTrue(track.tool_dirty(repo))


class Headroom(unittest.TestCase):
    def test_negative_under_the_bar_positive_over_it(self):
        m = {"keyword": [37, 40], "paraphrase": [15, 30], "code": [12, 12], "p90_tokens": 220}
        r = track.headroom(m, {"keyword": 40, "paraphrase": 14, "code": 12, "p90_tokens": 230})
        self.assertEqual(r["keyword"], -3)
        self.assertEqual(r["paraphrase"], 1)
        self.assertEqual(r["code"], 0)
        self.assertEqual(r["p90_tokens"], 10)


def run(cases, when="2026-09-04T12:00:00+00:00", commit="502e8a6d"):
    return {"cases": cases, "when": when, "source": "bench", "metrics": {},
            "corpus_commit": commit}


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

    def test_a_single_run_has_no_chronic_cases_at_all(self):
        # A first run has no history to be chronic against, and reporting eighteen "chronic"
        # cases from one run is the noise-versus-standing-hole confusion this is meant to end.
        self.assertEqual(track.weak([run({"a": 0.0, "b": 0.0})], 0.66), [])

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

    def test_a_missing_corpus_commit_is_not_treated_as_a_match(self):
        # Unknown is not equal. Saying nothing would let an unchecked pair pass as checked.
        a = {"corpus_commit": None, "cases": {"x": 1.0}}
        b = {"corpus_commit": "502e8a6d", "cases": {"x": 0.0}}
        self.assertIn("does not record which corpus commit", track.incomparable(a, b))
        self.assertIn("does not record which corpus commit", track.incomparable(b, a))

    def test_the_window_stops_at_the_first_incomparable_run(self):
        # chronic and flaky count across the window, so the window must not cross a setup
        # change -- counting flips over one is the attribution the report just refused.
        old = run({"a": 1.0}, when="2026-09-01T12:00:00+00:00", commit="7733bd53")
        mid = run({"a": 0.0}, when="2026-09-02T12:00:00+00:00")
        new = run({"a": 0.0}, when="2026-09-03T12:00:00+00:00")
        self.assertEqual(track.comparable_window([old, mid, new], 6), [mid, new])

    def test_a_window_size_below_one_still_means_one_run(self):
        # `history[-0:]` is the whole history, which would silently widen the window past
        # what the flag asked for.
        h = [run({"a": 1.0}, when="2026-09-01T12:00:00+00:00"),
             run({"a": 0.0}, when="2026-09-02T12:00:00+00:00")]
        self.assertEqual(track.comparable_window(h, 0), [h[-1]])
        self.assertEqual(track.comparable_window(h, -2), [h[-1]])

    def test_an_unreadable_timestamp_stops_with_a_message(self):
        with self.assertRaises(SystemExit):
            track.when_of({"when": "yesterday"})

    def test_a_window_of_one_when_nothing_before_it_is_comparable(self):
        old = run({"a": 1.0}, when="2026-09-01T12:00:00+00:00", commit="7733bd53")
        new = run({"a": 0.0}, when="2026-09-03T12:00:00+00:00")
        self.assertEqual(track.comparable_window([old, new], 6), [new])

    def test_metric_moves_only_report_what_changed(self):
        prev = {"metrics": {"keyword": [37, 40], "p90_tokens": 220}}
        now = {"metrics": {"keyword": [40, 40], "p90_tokens": 220}}
        self.assertEqual(track.metric_moves(prev, now), [("keyword", "37/40", "40/40")])




class AgentRows(unittest.TestCase):
    """`run.sh`'s summary is the harness's only output; a row that lost the per-task verdicts would
    make an agent configuration unreadable in the same way a bench row without cases is."""

    SUMMARY = {
        "config": "C", "model": "sonnet", "resolved_model": "claude-sonnet-5",
        "tasks": 3, "hits": 2, "tokens": 300000, "cost": 0.42, "asked": 5, "grepped": 1,
        "tokens_per_hit": 150000, "cost_per_hit": 0.21,
        "per_task": {
            "who-calls": {"kind": "who-calls", "hit": True},
            "rename": {"kind": "safe-to-rename", "hit": False},
            "cross": {"kind": "doc-to-code", "hit": True},
        },
    }

    def test_a_summary_round_trips_into_a_row_the_report_can_read(self):
        d = Path(tempfile.mkdtemp())
        (d / "summary.json").write_text(json.dumps(self.SUMMARY))
        before, track.RUNS = track.RUNS, d / "runs.jsonl"
        was_dirty, track.tool_dirty = track.tool_dirty, lambda repo=None: False
        try:
            track.cmd_agent(argparse.Namespace(summary=str(d / "summary.json"), corpus="beauty-crm",
                                               corpus_commit="502e8a6d", note="a test"))
            row = json.loads((d / "runs.jsonl").read_text().strip())
        finally:
            track.RUNS, track.tool_dirty = before, was_dirty
        self.assertEqual(row["arm"], "agent:C+sonnet")
        self.assertEqual(row["suite"], "agent")
        self.assertFalse(row["gated"], "an agent row is measured and never graded")
        self.assertEqual(row["metrics"]["hits"], [2, 3])
        self.assertEqual(row["cases"]["safe-to-rename/rename"], 0.0)
        self.assertEqual(row["cases"]["who-calls/who-calls"], 1.0)
        self.assertEqual(track.state_of(row), "measured, no floors")

    def test_a_repeated_task_is_one_case_read_twice(self):
        """`--repeat 2` exists to expose a flaky task. Keyed `<id>` and `<id>.r2` the history would
        see two cases that each answered once, and the flakiness would be invisible."""
        summary = dict(self.SUMMARY)
        summary["per_task"] = {
            "who-calls": {"kind": "who-calls", "hit": True},
            "who-calls.r2": {"kind": "who-calls", "hit": False},
            "rename": {"kind": "safe-to-rename", "hit": False},
        }
        d = Path(tempfile.mkdtemp())
        (d / "summary.json").write_text(json.dumps(summary))
        before, track.RUNS = track.RUNS, d / "runs.jsonl"
        was_dirty, track.tool_dirty = track.tool_dirty, lambda repo=None: False
        try:
            track.cmd_agent(argparse.Namespace(summary=str(d / "summary.json"), corpus="beauty-crm",
                                               corpus_commit="502e8a6d", note=""))
            row = json.loads((d / "runs.jsonl").read_text().strip())
        finally:
            track.RUNS, track.tool_dirty = before, was_dirty
        self.assertEqual(sorted(row["cases"]), ["safe-to-rename/rename", "who-calls/who-calls"])
        self.assertEqual(row["cases"]["who-calls/who-calls"], 0.5, "one of two attempts answered")

    def test_a_task_missed_in_two_runs_reads_as_chronic(self):
        rows = []
        for when in ("2026-09-09T10:00:00+00:00", "2026-09-09T11:00:00+00:00"):
            rows.append({"when": when, "arm": "agent:C+sonnet", "suite": "agent", "gated": False,
                         "corpus": "beauty-crm", "corpus_commit": "502e8a6d",
                         "tool_commit": "abc1234", "tool_dirty": False,
                         "metrics": {"hits": [2, 3]},
                         "cases": {"who-calls/who-calls": 1.0, "safe-to-rename/rename": 0.0,
                                   "doc-to-code/cross": 1.0}})
        window = track.comparable_window(rows, 2)
        chronic = track.weak(window, 0.66)
        self.assertEqual(len(chronic), 1, chronic)
        self.assertIn("safe-to-rename/rename", chronic[0])


if __name__ == "__main__":
    unittest.main()
