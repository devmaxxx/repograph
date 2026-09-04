"""The scorer, on answers copied from real tool output."""

import argparse
import collections
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import truth as T
from run import (
    Gitnexus,
    gitnexus_failure_reason,
    gitnexus_looks_like_a_result,
    load_id_families,
    rank_of,
    score_blast,
    spells,
    summarise,
)

BENCH = Path(__file__).resolve().parent.parent

# Each line carries one id but a different number of paths, so counting ids and counting
# paths before "OR-2" disagree (1 vs. 3) — a `rank_of` that ignored `"/" in want` and
# always counted paths would pass every other fixture here yet fail this one. INV and OR are
# real `id_families`, not stand-ins, so this stays valid once ID_TOKEN stops over-matching.
MIXED_COUNTS = "INV-1  docs/aaa.md docs/bbb.md docs/ccc.md\nOR-2  docs/ddd.md\n"

# `repograph ask "расход виден салону"` on beauty-crm at 2483d932, verbatim.
ANSWER = (
    "FR-AI-138  docs/prd-2026-08-16/prd/07-ai-layer.md:1163  Расход виден салону.\n"
    "NFR-PH-7  docs/prd-2026-08-16/prd/13-phasing.md:1470  Стоимость\n"
    "FR-SEC-92  docs/prd-2026-08-16/prd/12-security-prereqs.md:1144  Внутренний экран\n"
    "  FR-WH-30  docs/prd-2026-08-16/prd/08-marketing-inventory.md:981  Норматив  ← N-137\n"
)
# `repograph ask asGrosze`, verbatim.
CODE = (
    "sym:packages/contracts/src/money.ts::asGrosze  packages/contracts/src/money.ts:31  asGrosze\n"
    "sym:packages/domain/src/money/index.ts::asGrosze  packages/domain/src/money/index.ts:115  asGrosze\n"
)


class RankOf(unittest.TestCase):
    def test_first_line_is_rank_one(self):
        self.assertEqual(rank_of(ANSWER, "FR-AI-138"), 1)

    def test_two_ids_before_it_make_rank_three(self):
        self.assertEqual(rank_of(ANSWER, "FR-SEC-92"), 3)

    def test_a_neighbour_line_counts_its_ids_too(self):
        # FR-AI-138, NFR-PH-7, FR-SEC-92, FR-WH-30 come first: the reader passed four.
        self.assertEqual(rank_of(ANSWER, "N-137"), 5)

    def test_absent_is_none(self):
        self.assertIsNone(rank_of(ANSWER, "FR-CAL-101"))

    def test_a_path_case_counts_paths_not_ids(self):
        self.assertEqual(rank_of(CODE, "packages/contracts/src/money.ts"), 1)
        self.assertEqual(rank_of(CODE, "packages/domain/src/money/index.ts"), 2)

    def test_the_same_token_repeated_is_one_competitor(self):
        self.assertEqual(rank_of("INV-1 x\nINV-1 y\nOR-2\n", "OR-2"), 2)

    def test_an_id_want_counts_ids_even_when_the_answer_has_more_paths(self):
        self.assertEqual(rank_of(MIXED_COUNTS, "OR-2"), 2)

    def test_non_family_tokens_do_not_count_as_competing_ids(self):
        # UTF-8, SHA-256, RFC-7807, ISO-8601, AES-256 all matched the old blanket pattern and
        # inflated every rank before the real id — none names an `id_families` entry.
        noise = (
            "Encoded as UTF-8, hashed with SHA-256 per RFC-7807, dated ISO-8601, sealed AES-256.\n"
            "FR-AI-138  docs/prd-2026-08-16/prd/07-ai-layer.md:1163  Расход виден салону.\n"
        )
        self.assertEqual(rank_of(noise, "FR-AI-138"), 1)

    def test_a_real_family_before_the_noise_still_counts(self):
        noise = "INV-16 first.\nEncoded as UTF-8 and SHA-256.\nFR-AI-138 second.\n"
        self.assertEqual(rank_of(noise, "FR-AI-138"), 2)


class LoadIdFamilies(unittest.TestCase):
    def test_reads_the_real_repograph_toml(self):
        families = load_id_families()
        self.assertIn("FR-AI", families)
        self.assertIn("INV", families)

    def test_missing_file_raises_rather_than_falling_back(self):
        with self.assertRaises(RuntimeError):
            load_id_families(Path("/nonexistent/repograph.toml"))

    def test_missing_key_raises_rather_than_falling_back(self):
        with tempfile.TemporaryDirectory() as tmp:
            toml_path = Path(tmp) / "repograph.toml"
            toml_path.write_text('milestone_families = ["BE"]\n', encoding="utf8")
            with self.assertRaises(RuntimeError):
                load_id_families(toml_path)


class Summarise(unittest.TestCase):
    def test_mrr_is_the_mean_of_1_over_rank_with_absences_as_zero(self):
        rows = [
            {"suite": "retrieval", "kind": "keyword", "strict": True, "soft": True,
             "rank": 1, "ms": 10, "chars": 5},
            {"suite": "retrieval", "kind": "keyword", "strict": True, "soft": True,
             "rank": 4, "ms": 10, "chars": 5},
            {"suite": "retrieval", "kind": "keyword", "strict": False, "soft": False,
             "rank": None, "ms": 10, "chars": 5},
        ]
        # 1/1, 1/4, 0 (absent) -> (1 + 0.25 + 0) / 3 = 0.41666... -> round to 3 places: 0.417
        self.assertEqual(summarise(rows)["retrieval"]["mrr"], 0.417)

    def test_by_kind_carries_the_rank_columns_the_write_up_prints(self):
        rows = [
            {"suite": "retrieval", "kind": "keyword", "strict": True, "soft": True,
             "rank": 1, "ms": 10, "chars": 5},
            {"suite": "retrieval", "kind": "paraphrase", "strict": True, "soft": True,
             "rank": 2, "ms": 10, "chars": 5},
            {"suite": "retrieval", "kind": "paraphrase", "strict": False, "soft": False,
             "rank": None, "ms": 10, "chars": 5},
        ]
        by_kind = summarise(rows)["retrieval"]["by_kind"]
        self.assertEqual(by_kind["keyword"], {"n": 1, "strict": 1, "soft": 1, "rank1": 1, "mrr": 1.0})
        # 1/2 and an absence: (0.5 + 0) / 2, and neither of the two is at rank 1.
        self.assertEqual(by_kind["paraphrase"], {"n": 2, "strict": 1, "soft": 1, "rank1": 0, "mrr": 0.25})

    def test_a_row_with_no_rank_key_is_an_error_not_a_miss(self):
        # A pre-2026-09-04 result file rescored: scoring it 0 would publish an old run's
        # missing field as a measured MRR.
        rows = [{"suite": "retrieval", "kind": "keyword", "strict": True, "soft": True,
                 "ms": 10, "chars": 5}]
        with self.assertRaises(KeyError):
            summarise(rows)


class BlastShape(unittest.TestCase):
    def test_blast_has_the_recorded_shape(self):
        rows = T.read_jsonl(BENCH / "blast.jsonl")
        self.assertEqual(collections.Counter(r["kind"] for r in rows), {"impact": 16, "trace": 8, "changes": 8})
        tiers = collections.Counter(r["tier"] for r in rows if r["kind"] == "impact")
        self.assertEqual(tiers, {"hub": 2, "wide": 3, "narrow": 11})
        self.assertEqual(len({r["target"] for r in rows if r["kind"] == "impact"}), 16)

    def test_mobile_baseline_has_eight_kotlin_impact_cases(self):
        rows = T.read_jsonl(BENCH / "corpora" / "beauty-crm-mobile" / "blast.jsonl")
        self.assertEqual(collections.Counter(r["kind"] for r in rows), {"impact": 8})
        self.assertTrue(all(r["file"].endswith(".kt") for r in rows))
        self.assertEqual(collections.Counter(r["tier"] for r in rows), {"wide": 1, "narrow": 7})


def fake_proc(returncode: int, stdout: str) -> subprocess.CompletedProcess:
    return subprocess.CompletedProcess(args=[], returncode=returncode, stdout=stdout, stderr="")


class GitnexusLooksLikeAResult(unittest.TestCase):
    """Real success and failure shapes, read out of gitnexus 1.6.9's own source and a live
    index (`dist/mcp/local/local-backend.js`), plus the one shape that mattered most: a
    failure the whitelist has never seen before.
    """

    def test_query_success_has_a_processes_list(self):
        payload = json.dumps({"processes": [], "process_symbols": [], "definitions": []})
        self.assertTrue(gitnexus_looks_like_a_result("query", payload))

    def test_query_empty_query_error_has_no_processes_key(self):
        payload = json.dumps({"error": "search_query (or legacy query) parameter is required."})
        self.assertFalse(gitnexus_looks_like_a_result("query", payload))

    def test_trace_path_found_is_status_ok(self):
        payload = json.dumps({"status": "ok", "hopCount": 2, "hops": [], "edges": []})
        self.assertTrue(gitnexus_looks_like_a_result("trace", payload))

    def test_trace_no_path_is_also_a_result(self):
        payload = json.dumps({"status": "no_path", "to": {"name": "AvailabilityService"}})
        self.assertTrue(gitnexus_looks_like_a_result("trace", payload))

    def test_trace_symbol_not_found_is_not_a_result(self):
        payload = json.dumps({"status": "not_found", "error": "Source symbol 'X' not found."})
        self.assertFalse(gitnexus_looks_like_a_result("trace", payload))

    def test_trace_ambiguous_is_not_a_result(self):
        payload = json.dumps({"status": "ambiguous", "role": "from", "candidates": []})
        self.assertFalse(gitnexus_looks_like_a_result("trace", payload))

    def test_impact_success_has_impacted_count_and_no_error(self):
        payload = json.dumps({"target": {"name": "X"}, "impactedCount": 3, "risk": "LOW"})
        self.assertTrue(gitnexus_looks_like_a_result("impact", payload))

    def test_impact_target_not_found_carries_impacted_count_zero_but_also_error(self):
        # The one shape that makes "impactedCount present" alone unsafe: gitnexus's own
        # failure branches set `impactedCount: 0` too, so `error`'s absence is load-bearing.
        payload = json.dumps({"error": "Target 'X' not found", "impactedCount": 0, "risk": "UNKNOWN"})
        self.assertFalse(gitnexus_looks_like_a_result("impact", payload))

    def test_detect_changes_success_is_plain_text_not_json(self):
        self.assertTrue(gitnexus_looks_like_a_result("detect-changes", "No changes detected."))
        self.assertTrue(gitnexus_looks_like_a_result(
            "detect-changes", "Changes: 3 files, 12 symbols\n\nChanged symbols:\n  ..."))

    def test_detect_changes_a_git_failure_is_plain_text_too_but_not_a_result(self):
        # Observed live: a bad --base-ref exits 0 and prints this, not JSON, not either
        # recognised opener — exactly the kind of shape a blacklist would have missed.
        text = "fatal: ambiguous argument 'nope'\nError: Git diff failed: Command failed: git diff nope -U0\n"
        self.assertFalse(gitnexus_looks_like_a_result("detect-changes", text))

    def test_unparseable_stdout_is_not_a_result(self):
        self.assertFalse(gitnexus_looks_like_a_result("trace", "not json at all"))

    def test_an_unrecognized_exit_zero_shape_is_not_a_result(self):
        # No `error` key, no recognised `status` — a shape the old blacklist (which only
        # checked for a literal `error` key) would have scored as a real answer, with the
        # echoed `"to"` field then satisfying the trace substring checks exactly as the
        # original defect did.
        payload = json.dumps({"problem": "no such repository", "to": {"name": "AvailabilityService"}})
        self.assertFalse(gitnexus_looks_like_a_result("trace", payload))


class GitnexusFailureReason(unittest.TestCase):
    def test_nonzero_exit_fails_regardless_of_stdout(self):
        argv = ["gitnexus", "trace", "A", "B"]
        reason = gitnexus_failure_reason(argv, fake_proc(1, json.dumps({"status": "ok"})))
        self.assertIsNotNone(reason)

    def test_a_genuine_result_has_no_failure_reason(self):
        argv = ["gitnexus", "impact", "X"]
        payload = json.dumps({"target": {"name": "X"}, "impactedCount": 3, "risk": "LOW"})
        self.assertIsNone(gitnexus_failure_reason(argv, fake_proc(0, payload)))


class GitnexusUnansweredCallsDoNotScoreAsHits(unittest.TestCase):
    """`via == [to]` is the shape that made the original defect possible — the echoed `"to"`
    field alone satisfies both the `via` and `to` substring checks for a `trace` "path" case.
    """

    def _tool(self) -> Gitnexus:
        opts = argparse.Namespace(gitnexus_repo="/tmp/bench-corpus-3g", timeout=180, strip_prefix=[])
        return Gitnexus(Path("/tmp"), opts)

    def _case(self):
        return {"kind": "trace", "from": "AvailabilityController", "to": "AvailabilityService",
                "expect": "path", "via": ["AvailabilityService"]}

    def test_a_known_error_shape_scores_a_miss(self):
        payload = json.dumps({
            "error": "Repository \"/tmp/bench-corpus-3g\" not found. Available: beauty-crm",
            "to": {"name": "AvailabilityService"},
        })
        with mock.patch("run.subprocess.run", return_value=fake_proc(1, payload)):
            rows = score_blast(self._tool(), cases=[self._case()], truth={})
        self.assertEqual(rows[0]["hit"], False)
        self.assertEqual(rows[0]["chars"], 0)

    def test_an_unrecognized_exit_zero_shape_also_scores_a_miss(self):
        # This is the case that reproduces the old defect today: no `error` key, exit 0,
        # still echoes the query. A blacklist keyed on `error` would score this a hit.
        payload = json.dumps({"problem": "no such repository", "to": {"name": "AvailabilityService"}})
        with mock.patch("run.subprocess.run", return_value=fake_proc(0, payload)):
            rows = score_blast(self._tool(), cases=[self._case()], truth={})
        self.assertEqual(rows[0]["hit"], False)
        self.assertEqual(rows[0]["chars"], 0)

    def test_failed_calls_are_counted_on_the_tool(self):
        payload = json.dumps({"problem": "no such repository", "to": {"name": "AvailabilityService"}})
        tool = self._tool()
        with mock.patch("run.subprocess.run", return_value=fake_proc(0, payload)):
            score_blast(tool, cases=[self._case(), self._case()], truth={})
        self.assertEqual(tool.failed_calls, 2)


class SpellsSymbol(unittest.TestCase):
    """A symbol is credited when the answer names it, not when the letters happen to occur."""

    def test_a_short_name_is_not_credited_by_a_longer_word(self):
        self.assertFalse(spells("the hash of the row", "has"))
        self.assertFalse(spells("returns keys and values", "key"))
        self.assertFalse(spells("parseAll(x)", "parse"))

    def test_a_name_is_credited_when_the_answer_spells_it(self):
        self.assertTrue(spells("calls has() on the row", "has"))
        self.assertTrue(spells("changed: `key`, `code`", "key"))
        self.assertTrue(spells("Registry.count", "count"))

    def test_a_backticked_kotlin_test_name_is_matched_whole(self):
        self.assertTrue(spells("fun `a rule holds`()", "a rule holds"))


if __name__ == "__main__":
    unittest.main()
