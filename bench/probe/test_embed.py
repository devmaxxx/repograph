import os
import subprocess
import tempfile
import unittest
from pathlib import Path

import judge

HERE = Path(__file__).resolve().parent
EMBED = HERE / "embed.sh"

# What `top -l 2 -s 1 -pid P -stats pid,cpu,mem,th | tail -1` leaves in the samples file.
SAMPLES = "1234 118.0 1554M 8/1\n1234 121.4 1602M 9/1\n1234 96.2 1602M 9/1\n"


def sourced(snippet):
    """embed.sh's functions without its body: sourcing the script defines and runs nothing."""
    return subprocess.run(["bash", "-c", f"source '{EMBED}'; {snippet}"], capture_output=True, text=True)


@unittest.skipUnless(os.name == "posix", "bash and awk: the macOS kit's own platform")
class Row(unittest.TestCase):
    """The row §9 judges, read without a 1,930 s embed under it. A real embed cannot be run in a
    unit, but what the script does with the transcript the sampler left — print a row, or refuse
    to — is where the peak-CPU column is either a reading or nothing at all."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(subprocess.run, ["rm", "-rf", str(self.tmp)])
        self.f = self.tmp / "control-1.samples"

    def row(self, samples, rc="0"):
        self.f.write_text(samples)
        return sourced(f"summary_row control-1 1930.05 5600.11 2.15 {rc} '{self.f}'")

    def test_the_row_carries_the_count_of_samples_its_peak_came_from(self):
        r = self.row(SAMPLES)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(r.stdout.strip(),
                         "control-1  wall=1930.05s user=5600.11s maxrss=2.15GB peak_cpu=121.4% samples=3 rc=0")

    def test_the_row_reads_back_through_judge_the_way_a_reader_row_does(self):
        m = judge.medians([self.row(SAMPLES).stdout])
        self.assertEqual(m["control"], {"wall": 1930.05, "maxrss": 2.15, "peak_cpu": 121.4, "n": 1})

    def test_an_embed_the_sampler_never_read_refuses_instead_of_printing_a_row(self):
        r = self.row("")
        self.assertNotEqual(r.returncode, 0)
        self.assertEqual(r.stdout, "")
        self.assertIn("refusing", r.stderr)

    def test_a_peak_of_zero_is_unreachable_on_a_row_that_says_the_embed_succeeded(self):
        r = self.row("1234 0.0 1554M 8/1\n")
        self.assertNotEqual(r.returncode, 0)
        self.assertEqual(r.stdout, "")
        self.assertIn("refusing", r.stderr)

    def test_a_sampled_embed_that_failed_still_carries_its_status_onto_the_row(self):
        r = self.row(SAMPLES, rc="2")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("rc=2", r.stdout)


if __name__ == "__main__":
    unittest.main()
