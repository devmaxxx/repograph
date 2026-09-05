#!/usr/bin/env python3
"""The replay's pieces against the Rust they mirror: `fuse::interleave`, the admission forms,
and the one-neighbour expansion — small enough to check by hand."""
import math
import unittest

import admission as a


def scored(*pairs):
    return [[i, float(s)] for i, s in pairs]


class Interleave(unittest.TestCase):
    def test_every_list_places_its_first_hit_before_any_second_hit(self):
        self.assertEqual([i for i, _ in a.interleave([["x", "y"], ["z", "y"]])], ["x", "z", "y"])

    def test_an_id_keeps_its_first_position_and_score(self):
        out = a.interleave([["a", "b"], ["b", "a"]])
        self.assertEqual(out, [("a", 1.0), ("b", 1.0)])

    def test_empty_lists_leave_no_gaps(self):
        self.assertEqual([i for i, _ in a.interleave([["a"], [], ["b", "c", "d"]])], ["a", "b", "c", "d"])


class Forms(unittest.TestCase):
    def test_a_list_with_nothing_to_say_is_never_seated(self):
        for form in a.FORMS:
            self.assertFalse(a.admitted(form, 0.0, [], scored(("p", 2.0)), 1.0, 1.0), form)
            self.assertFalse(a.admitted(form, 0.0, scored(("q", 0.0)), scored(("p", 2.0)), 1.0, 1.0), form)

    def test_a_list_alone_in_having_something_to_say_is_seated_under_every_form(self):
        for form in a.FORMS:
            self.assertTrue(a.admitted(form, 5.0, scored(("q", 0.3)), [], 1.0, 0.0), form)

    def test_the_ratio_form_is_the_shipped_gate_in_f32(self):
        # 0.85 * 2.64 = 2.244; 2.14 clears 1.32 * 0.85 and 1.07 does not — the numbers the
        # query.rs tests were written on.
        self.assertTrue(a.admitted("ratio", 0.85, scored(("q", 2.14)), scored(("p", 1.32)), 1.0, 1.0))
        self.assertFalse(a.admitted("ratio", 0.85, scored(("q", 1.07)), scored(("p", 2.64)), 1.0, 1.0))

    def test_coverage_is_the_best_over_the_attainable(self):
        self.assertAlmostEqual(a.stat("coverage", scored(("q", 1.5)), 3.0), 0.5)
        self.assertEqual(a.stat("coverage", scored(("q", 1.5)), 0.0), 0.0)

    def test_peak_is_the_best_over_the_fifth_and_a_short_list_is_its_own_peak(self):
        five = scored(("a", 4.0), ("b", 3.0), ("c", 2.0), ("d", 1.5), ("e", 1.0), ("f", 0.5))
        self.assertAlmostEqual(a.stat("peak", five, 1.0), 4.0)
        self.assertEqual(a.stat("peak", five[:3], 1.0), math.inf)

    def test_z_is_the_best_as_a_z_score_over_the_list_as_the_plain_path_sees_it(self):
        lst = scored(("a", 3.0), ("b", 1.0), ("c", 1.0), ("d", 1.0))
        # mean 1.5, population sd sqrt(0.75)
        self.assertAlmostEqual(a.stat("z", lst, 1.0), 1.5 / math.sqrt(0.75))
        self.assertEqual(a.stat("z", lst[:1], 1.0), math.inf)

    def test_only_the_first_twenty_of_a_deeper_list_count(self):
        deep = scored(*[(f"d{i}", 30.0 - i) for i in range(300)])
        self.assertAlmostEqual(a.stat("peak", deep, 1.0), 30.0 / 26.0)


class Expansion(unittest.TestCase):
    def test_the_expanded_line_is_the_ranked_neighbour_not_the_top_seed_s(self):
        rec = {"exact": {"ids": [], "whole_question": False}}
        lists = [["s1", "s2", "n2"]]
        nb = {"s1": ["n1"], "s2": ["n2"]}
        seeds, expanded = a.answer(rec, lists, nb)
        self.assertEqual(seeds, ["s1", "s2", "n2"])
        # n2 is a seed already; n1 is unranked and the only candidate left.
        self.assertEqual(expanded, ["n1"])

    def test_an_exact_answer_to_the_whole_question_fuses_nothing(self):
        rec = {"exact": {"ids": ["FR-1"], "whole_question": True}}
        seeds, expanded = a.answer(rec, [["x", "y"]], {"FR-1": ["file:a.md", "N-2"]})
        self.assertEqual((seeds, expanded), (["FR-1"], ["N-2"]))


if __name__ == "__main__":
    unittest.main()
