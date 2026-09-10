import contextlib
import io
import unittest

import judge

SUMMARY = """\
ask-fused-1  wall=0.61s user=0.40s sys=0.20s maxrss=1.55GB peak_cpu=120% peak_threads=8 samples=1
ask-fused-2  wall=0.75s user=0.41s sys=0.21s maxrss=1.56GB peak_cpu=118% peak_threads=8 samples=1
ask-fused-3  wall=0.60s user=0.40s sys=0.20s maxrss=1.36GB peak_cpu=121% peak_threads=8 samples=1
impact-1  wall=0.04s user=0.02s sys=0.01s maxrss=0.05GB peak_cpu=0% peak_threads=1 samples=0
impact-2  wall=0.05s user=0.02s sys=0.01s maxrss=0.05GB peak_cpu=0% peak_threads=1 samples=0
impact-3  wall=0.04s user=0.02s sys=0.01s maxrss=0.05GB peak_cpu=0% peak_threads=1 samples=0
"""

STAMPED = """\
100.00 start
100.40 dense: model open in 0.4s
100.90 dense: 128/33525 rows, 256.0 rows/s, ~2 min left
103.90 dense: 512/33525 rows, 170.7 rows/s, ~3 min left
109.90 dense: 1536/33525 rows, 170.7 rows/s, ~3 min left
118.20 dense: 33525/33525 rows, 180.0 rows/s, ~0 min left
118.30 dense: embedded 33525 rows in 18.3s
118.31        18.31 real        60.00 user         2.00 sys
118.31           1740000000  maximum resident set size
"""


class Medians(unittest.TestCase):
    def test_runs_of_one_row_are_grouped_and_the_median_taken(self):
        m = judge.medians(SUMMARY.splitlines())
        self.assertEqual(m["ask-fused"], {"wall": 0.61, "maxrss": 1.55, "peak_cpu": 120.0, "n": 3})
        self.assertEqual(m["impact"]["wall"], 0.04)

    def test_an_even_count_takes_the_upper_middle_like_bench_does(self):
        self.assertEqual(judge.median([1.0, 2.0, 3.0, 4.0]), 3.0)

    def test_a_medians_file_reads_back_as_written(self):
        import os
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "m.txt")
            with open(p, "w") as f:
                f.write("ask-fused wall=0.61 maxrss=1.55 peak_cpu=120.0 n=5\n")
            self.assertEqual(judge.read_medians(p)["ask-fused"], {"wall": 0.61, "maxrss": 1.55, "peak_cpu": 120.0, "n": 5})

    def test_a_summary_file_of_runs_reads_as_medians_too(self):
        import os
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "s.txt")
            with open(p, "w") as f:
                f.write(SUMMARY)
            self.assertEqual(judge.read_medians(p)["impact"]["n"], 3)


class RefusedRuns(unittest.TestCase):
    """What `medians` must not average. Both shapes come off `measure.sh` looking like readings."""

    def test_a_row_whose_command_exited_non_zero_is_refused_not_averaged(self):
        line = "ask-fused-1  wall=0.02s user=0.01s sys=0.00s maxrss=0.01GB peak_cpu=0% peak_threads=1 samples=0 rc=2"
        with self.assertRaises(SystemExit) as e:
            judge.medians([line])
        self.assertIn("exited 2", str(e.exception))

    def test_a_row_with_no_wall_at_all_names_the_field(self):
        line = "ask-fused-1  wall=s user=s sys=s maxrss=GB peak_cpu=0% peak_threads=1 samples=0 rc=0"
        with self.assertRaises(SystemExit) as e:
            judge.medians([line])
        self.assertIn("wall", str(e.exception))
        self.assertIn("maxrss", str(e.exception))

    def test_a_clean_row_still_reads_with_rc_on_the_line(self):
        line = "impact-1  wall=0.04s user=0.02s sys=0.01s maxrss=0.05GB peak_cpu=0% peak_threads=1 samples=0 rc=0"
        self.assertEqual(judge.medians([line])["impact"]["n"], 1)

    def test_a_table_of_no_rows_is_refused_because_all_of_nothing_is_true(self):
        with self.assertRaises(SystemExit):
            judge.print_table([], ("wall", "RSS"))


class Control(unittest.TestCase):
    def test_the_same_binary_twice_inside_the_bars_passes(self):
        a = {"ask-fused": {"wall": 0.60, "maxrss": 1.50, "peak_cpu": 120.0, "n": 5}}
        b = {"ask-fused": {"wall": 0.63, "maxrss": 1.55, "peak_cpu": 121.0, "n": 5}}
        rows = judge.control(a, b)
        self.assertEqual(rows, [("ask-fused", 0.05, 0.0333, True)])

    def test_a_row_outside_either_bar_fails_and_names_which(self):
        a = {"dump10": {"wall": 0.70, "maxrss": 1.36, "peak_cpu": 100.0, "n": 5}}
        b = {"dump10": {"wall": 0.90, "maxrss": 1.36, "peak_cpu": 100.0, "n": 5}}
        (row, wall, rss, ok), = judge.control(a, b)
        self.assertFalse(ok)
        self.assertAlmostEqual(wall, 0.2857, places=4)
        self.assertEqual(rss, 0.0)

    def test_a_row_missing_from_one_side_is_reported_not_skipped(self):
        with self.assertRaises(SystemExit):
            judge.control({"a": {"wall": 1, "maxrss": 1, "peak_cpu": 1, "n": 5}}, {})

    def test_a_row_only_the_second_run_has_is_reported_too(self):
        with self.assertRaises(SystemExit):
            judge.control({}, {"a": {"wall": 1, "maxrss": 1, "peak_cpu": 1, "n": 5}})

    def test_a_candidate_row_the_reference_never_had_is_reported(self):
        with self.assertRaises(SystemExit):
            judge.compare({}, {"a": {"wall": 1, "maxrss": 1, "peak_cpu": 1, "n": 5}})


class ComparePeakCpu(unittest.TestCase):
    """The third judged column. G19's control triple — 1,930 s, 293%, 2.15 GB — is the shape the
    column exists for; the reader rows are the shape it cannot speak about."""

    def row(self, wall, rss, cpu):
        return {"wall": wall, "maxrss": rss, "peak_cpu": cpu, "n": 5}

    def test_a_candidate_inside_the_cpu_bar_passes(self):
        ref = {"embed": self.row(1930.0, 2.15, 293.0)}
        new = {"embed": self.row(1930.0, 2.15, 315.0)}
        (_, _, _, cpu, ok), = judge.compare(ref, new)
        self.assertAlmostEqual(cpu, 0.0751, places=4)
        self.assertTrue(ok)

    def test_a_candidate_outside_the_cpu_bar_fails_on_that_column_alone(self):
        ref = {"embed": self.row(1930.0, 2.15, 293.0)}
        new = {"embed": self.row(2026.5, 2.15, 360.0)}
        (_, wall, rss, cpu, ok), = judge.compare(ref, new)
        self.assertEqual((wall, rss), (0.05, 0.0))
        self.assertAlmostEqual(cpu, 0.2287, places=4)
        self.assertFalse(ok)

    def test_a_row_the_sampler_never_caught_is_unjudged_rather_than_green(self):
        ref = {"impact": self.row(0.04, 0.05, 0.0)}
        (_, _, _, cpu, ok), = judge.compare(ref, {"impact": self.row(0.04, 0.05, 0.0)})
        self.assertIsNone(cpu)
        self.assertTrue(ok)
        # A reference of 0 is no reading, not a reading of zero, so a candidate that did get
        # sampled has nothing to be within 10% of either — the wall column is what moved.
        (_, _, _, cpu, ok), = judge.compare(ref, {"impact": self.row(0.04, 0.05, 130.0)})
        self.assertIsNone(cpu)
        self.assertTrue(ok)

    def test_the_table_prints_three_delta_columns_and_names_the_unjudged_one(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            judge.print_table(judge.compare({"impact": self.row(0.04, 0.05, 0.0)},
                                            {"impact": self.row(0.04, 0.05, 0.0)}),
                              ("wall Δ", "RSS Δ", "peak CPU Δ"))
        lines = out.getvalue().splitlines()
        self.assertEqual(lines[0], "| row | wall Δ | RSS Δ | peak CPU Δ | |")
        self.assertEqual(lines[1], "|---|---|---|---|---|")
        self.assertEqual(lines[2], "| impact | +0.0% | +0.0% | n/a | ok |")

    def test_the_control_table_keeps_the_two_columns_its_clause_names(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            judge.print_table(judge.control({"impact": self.row(0.04, 0.05, 0.0)},
                                            {"impact": self.row(0.04, 0.05, 0.0)}),
                              ("wall spread", "RSS spread"))
        self.assertEqual(out.getvalue().splitlines()[0], "| row | wall spread | RSS spread | |")


class RunCount(unittest.TestCase):
    """The `n` every row has always carried, now read. §1 judges its control at five runs a row;
    §9's embed is three, and a suite with its own shape says the number instead of inheriting it."""

    def rows(self, n):
        return {"ask-fused": {"wall": 0.60, "maxrss": 1.50, "peak_cpu": 120.0, "n": n}}

    def test_a_control_over_one_run_a_row_is_refused_not_cleared(self):
        with self.assertRaises(SystemExit) as e:
            judge.control(self.rows(1), self.rows(1))
        self.assertIn("n=1", str(e.exception))

    def test_a_candidate_under_the_floor_is_refused_too(self):
        with self.assertRaises(SystemExit):
            judge.compare(self.rows(5), self.rows(3))

    def test_a_suite_of_three_runs_is_judged_when_the_floor_is_said_out_loud(self):
        (_, wall, _, _, ok), = judge.compare(self.rows(3), self.rows(3), min_n=3)
        self.assertEqual((wall, ok), (0.0, True))

    def test_the_two_file_subcommands_print_usage_instead_of_an_index_error(self):
        for argv in (["judge.py", "control", "a.txt"], ["judge.py", "compare", "a.txt"]):
            with self.assertRaises(SystemExit) as e:
                judge.main(argv)
            self.assertIn("usage", str(e.exception))

    def test_a_floor_that_is_not_a_number_prints_usage_instead_of_a_traceback(self):
        with self.assertRaises(SystemExit) as e:
            judge.main(["judge.py", "compare", "a.txt", "b.txt", "three"])
        self.assertIn("usage", str(e.exception))

    def test_a_floor_of_zero_is_refused_because_it_admits_the_reading_the_floor_exists_to_stop(self):
        for floor in ("0", "-3"):
            with self.assertRaises(SystemExit) as e:
                judge.main(["judge.py", "control", "a.txt", "b.txt", floor])
            self.assertIn("usage", str(e.exception))


class Cadence(unittest.TestCase):
    def test_intervals_are_read_from_the_stamps_not_from_rows_per_second(self):
        c = judge.cadence(STAMPED.splitlines())
        self.assertEqual(c["first"], 0.9)
        self.assertEqual(c["intervals"], [0.9, 3.0, 6.0, 8.3])
        self.assertEqual(c["max"], 8.3)
        self.assertEqual(c["median"], 6.0)
        self.assertAlmostEqual(c["ratio"], 8.3 / 6.0, places=3)

    def test_the_first_interval_counts_from_process_start_not_from_model_open(self):
        lines = ["50.00 start", "80.00 dense: model open in 30.0s", "85.00 dense: 128/9 rows, 1.0 rows/s, ~1 min left"]
        self.assertEqual(judge.cadence(lines)["first"], 35.0)

    def test_a_transcript_with_no_progress_line_is_refused(self):
        with self.assertRaises(SystemExit):
            judge.cadence(["1.00 start", "2.00 dense: embedded 0 rows in 1.0s"])


class Verdict(unittest.TestCase):
    def test_cadence_verdict_needs_every_interval_under_the_bar_and_a_flat_tail(self):
        ok, why = judge.cadence_ok({"first": 0.9, "intervals": [0.9, 3.0, 6.0, 8.3], "max": 8.3, "median": 6.0, "ratio": 8.3 / 6.0}, bar=60.0, tail=1.3)
        self.assertFalse(ok)
        self.assertIn("1.383", why)
        ok, _ = judge.cadence_ok({"first": 0.9, "intervals": [5.0, 6.0], "max": 6.0, "median": 6.0, "ratio": 1.0}, bar=60.0, tail=1.3)
        self.assertTrue(ok)


if __name__ == "__main__":
    unittest.main()
