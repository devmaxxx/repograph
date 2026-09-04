"""The scorer, on answers copied from real tool output."""

import collections
import unittest
from pathlib import Path

import truth as T
from run import rank_of, summarise

BENCH = Path(__file__).resolve().parent.parent

# Each line carries one id but a different number of paths, so counting ids and counting
# paths before "FR-B-2" disagree (1 vs. 3) — a `rank_of` that ignored `"/" in want` and
# always counted paths would pass every other fixture here yet fail this one.
MIXED_COUNTS = "FR-A-1  docs/aaa.md docs/bbb.md docs/ccc.md\nFR-B-2  docs/ddd.md\n"

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
        self.assertEqual(rank_of("FR-A-1 x\nFR-A-1 y\nFR-B-2\n", "FR-B-2"), 2)

    def test_an_id_want_counts_ids_even_when_the_answer_has_more_paths(self):
        self.assertEqual(rank_of(MIXED_COUNTS, "FR-B-2"), 2)


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
        self.assertEqual(collections.Counter(r["tier"] for r in rows), {"hub": 3, "wide": 2, "narrow": 3})


if __name__ == "__main__":
    unittest.main()
