"""The scorer, on answers copied from real tool output."""

import unittest

from run import rank_of

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


if __name__ == "__main__":
    unittest.main()
