import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

import judge

HERE = Path(__file__).resolve().parent
MEASURE = HERE / "measure.sh"
MAIN_RS = HERE.parents[1] / "src" / "main.rs"
# The one wording that separates a floor verdict from a broken store, quoted here so the case that
# pins it and the case that uses it read the same string.
FLOOR_BAIL = "bench floors not met"


def time_l_reads():
    """`/usr/bin/time -l` is Darwin's; GNU time takes `-l` for something else and exits non-zero."""
    try:
        return subprocess.run(["/usr/bin/time", "-l", "true"], capture_output=True).returncode == 0
    except OSError:
        return False


@unittest.skipUnless(os.name == "posix" and time_l_reads(), "`/usr/bin/time -l`: the macOS kit's own platform")
class FloorRow(unittest.TestCase):
    """Which non-zero exit `measure.sh` records as a reading, and which it leaves as a failure.

    `repograph bench` exits 1 both when a floor is missed and when the store it was pointed at is
    not there, so the row is built over a stub that says one thing or the other on stderr and exits
    1 either way — which is the whole shape of the ambiguity, without a store or a binary under it.
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
        p = self.row("bench-dense-1", f"Error: {FLOOR_BAIL}", 1)
        self.assertEqual(p.returncode, 1, p.stderr)
        line = self.summary()
        self.assertIn("rc=1", line)
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

    def test_a_row_that_did_its_work_says_nothing_about_floors(self):
        p = self.row("bench-dense-1", "", 0)
        self.assertEqual(p.returncode, 0, p.stderr)
        self.assertIn("rc=0", self.summary())
        self.assertNotIn("floors_missed", self.summary())

    def test_the_wording_the_field_is_read_off_is_still_the_wording_bench_bails_with(self):
        # The grep in `measure.sh` is the only thing separating the two readings, so a rename in
        # `src/main.rs` would turn every missed floor into a refused suite — silently, since the
        # exit code does not change. Red here is the notice; the grep follows the rename.
        self.assertIn(FLOOR_BAIL, MAIN_RS.read_text())


if __name__ == "__main__":
    unittest.main()
