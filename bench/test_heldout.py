#!/usr/bin/env python3
"""`heldout.py`'s pieces that a hand can check: which entries are code, and the paired test."""
import unittest

import heldout as h


class Kinds(unittest.TestCase):
    def test_code_entries_are_the_graph_s_symbol_and_file_ids(self):
        self.assertTrue(h.is_code("sym:apps/a.ts::revoke") and h.is_code("file:apps/a.ts"))
        self.assertFalse(h.is_code("FR-PAY-22") or h.is_code("entity:Money") or h.is_code("ADR-005"))


class McNemar(unittest.TestCase):
    def test_a_wash_is_p_one_and_six_lost_to_none_gained_is_the_price_a4_paid(self):
        before = {k: True for k in "abcdef"}
        self.assertEqual(h.mcnemar(before, dict(before)), (0, 0, 1.0))
        lost, gained, p = h.mcnemar(before, {k: False for k in before})
        self.assertEqual((lost, gained), (6, 0))
        self.assertAlmostEqual(p, 0.03125)


if __name__ == "__main__":
    unittest.main()
