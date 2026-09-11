import os
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

import judge

HERE = Path(__file__).resolve().parent
MEASURE = HERE / "measure.sh"


def time_l_reads():
    """`/usr/bin/time -l` is Darwin's; GNU time takes `-l` for something else and exits non-zero."""
    try:
        return subprocess.run(["/usr/bin/time", "-l", "true"], capture_output=True).returncode == 0
    except OSError:
        return False


class WhichRowsHaveFloors(unittest.TestCase):
    """The instrument and the judge name the same rows, in two copies of one expression.

    `measure.sh` decides which rows may carry `floors_missed` and `judge.py` decides which rows may
    be believed when they do; the two disagreeing is a row written by one and refused by the other,
    on a suite nobody reads until it has been run. Read off the shell rather than restated here, so
    the assertion cannot pass by agreeing with a third copy.
    """

    def test_the_shell_and_the_judge_match_the_same_rows(self):
        literal = re.search(r"^BENCH_ROW='(.*)'$", MEASURE.read_text(), re.M)
        self.assertIsNotNone(literal, "measure.sh no longer assigns BENCH_ROW as a quoted literal")
        self.assertEqual(literal.group(1), judge.BENCH_ROW.pattern)


@unittest.skipUnless(os.name == "posix" and time_l_reads(), "`/usr/bin/time -l`: the macOS kit's own platform")
class FloorRow(unittest.TestCase):
    """Which non-zero exit `measure.sh` records as a reading, and which it leaves as a failure.

    `repograph` exits 3 for a question it answered — a suite that ran every case and missed a
    floor, a `trace` that found no path — and 1 when it could not answer at all. The row is built
    over a stub that exits with either status, which is the whole of what the instrument reads,
    without a store or a binary under it. The status is the interface and the wording is a message
    no script reads: a row that says the old words and exits 1 is refused, so a binary older than
    this release cannot buy the field with its wording.

    `floors_missed` is scoped to the rows that can miss a floor. A `trace` row's verdict is a
    reading of that reader too, but it has no floors to miss, and a field claiming it had would be
    a fact about the row that is not true.
    """

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.log = self.tmp / "log"
        self.stub = self.tmp / "stub.sh"
        self.stub.write_text("#!/bin/bash\nprintf '%s\\n' \"$1\" >&2\nexit \"$2\"\n")
        self.stub.chmod(0o755)

    def row(self, name, said, rc):
        return subprocess.run(["bash", str(MEASURE), name, str(self.log), "--", str(self.stub), said, str(rc)],
                              capture_output=True, text=True)

    def summary(self):
        return (self.log / "summary.txt").read_text().strip()

    def test_a_bench_row_that_missed_a_floor_carries_the_field_beside_its_status(self):
        p = self.row("bench-dense-1", "bench floors not met", 3)
        self.assertEqual(p.returncode, 3, p.stderr)
        line = self.summary()
        self.assertIn("rc=3", line)
        self.assertIn("floors_missed=1", line)
        self.assertIn("the timing stands", p.stderr)
        # And the row reads through `judge` as what it is: a reading of that reader.
        self.assertEqual(judge.medians([line])["bench-dense"]["floors_missed"], 1)

    def test_a_bench_row_that_broke_on_the_store_carries_no_field_and_is_refused(self):
        p = self.row("bench-dense-1", "Error: graph is empty at /nope — run build first", 1)
        self.assertEqual(p.returncode, 1, p.stderr)
        line = self.summary()
        self.assertIn("rc=1", line)
        self.assertNotIn("floors_missed", line)
        self.assertIn("measured a failure", p.stderr)
        with self.assertRaises(SystemExit):
            judge.medians([line])

    def test_a_verdict_on_a_row_with_no_floors_is_a_reading_and_claims_no_field(self):
        # `trace` answers "no path within the depth" with the same status. The row spent its wall
        # clock traversing and is a reading; `floors_missed` on it would say it missed a floor it
        # never had, and `judge.py` refuses a non-bench row that claims the field.
        p = self.row("trace-1", "no call path from main to cn within 6 hops", 3)
        self.assertEqual(p.returncode, 3, p.stderr)
        line = self.summary()
        self.assertIn("rc=3", line)
        self.assertNotIn("floors_missed", line)
        self.assertIn("answered with a verdict", p.stderr)
        self.assertEqual(judge.medians([line])["trace"]["n"], 1)
        self.assertNotIn("floors_missed", str(judge.medians([line])["trace"]))

    def test_a_usage_error_is_a_failure_and_not_a_verdict(self):
        # `clap` exits 2 before the command runs — a mistyped flag, not an answer — which is why
        # the verdict is 3. A row that measured a binary refusing its own argv measured nothing.
        p = self.row("bench-dense-1", "error: unexpected argument '--nosuchflag' found", 2)
        self.assertEqual(p.returncode, 2, p.stderr)
        self.assertNotIn("floors_missed", self.summary())
        self.assertIn("measured a failure", p.stderr)
        with self.assertRaises(SystemExit):
            judge.medians([self.summary()])

    def test_a_row_that_did_its_work_says_nothing_about_floors(self):
        p = self.row("bench-dense-1", "", 0)
        self.assertEqual(p.returncode, 0, p.stderr)
        self.assertIn("rc=0", self.summary())
        self.assertNotIn("floors_missed", self.summary())

    def test_a_binary_too_old_to_exit_three_cannot_buy_the_field_with_its_wording(self):
        # The release before this one exited 1 with these words for a missed floor. Read as a
        # reading, its row would enter a median beside rows a newer binary produced; refused, the
        # suite says which binary it was pointed at.
        p = self.row("bench-dense-1", "Error: bench floors not met", 1)
        self.assertEqual(p.returncode, 1, p.stderr)
        self.assertNotIn("floors_missed", self.summary())
        self.assertIn("measured a failure", p.stderr)
        with self.assertRaises(SystemExit):
            judge.medians([self.summary()])


if __name__ == "__main__":
    unittest.main()
