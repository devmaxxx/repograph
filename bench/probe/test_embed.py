import os
import subprocess
import tempfile
import time
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
        return sourced(f"summary_row control-1 1930.05 5600.11 120.00 2.15 {rc} '{self.f}'")

    def test_the_row_carries_the_count_of_samples_its_peak_came_from(self):
        r = self.row(SAMPLES)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(r.stdout.strip(),
                         "control-1  wall=1930.05s user=5600.11s sys=120.00s maxrss=2.15GB peak_cpu=121.4% samples=3 rc=0")

    def test_the_row_reads_back_through_judge_the_way_a_reader_row_does(self):
        m = judge.medians([self.row(SAMPLES).stdout])
        self.assertEqual(m["control"], {"wall": 1930.05, "maxrss": 2.15, "peak_cpu": 121.4,
                                        "avg_cpu": 2.96, "n": 1})

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


@unittest.skipUnless(os.name == "posix", "bash, pgrep and POSIX signals: the macOS kit's own platform")
class SamplerRefusal(unittest.TestCase):
    """What the refusal does to the run it refuses. The tree here is two sleeps rather than an
    embed — the assertion is that everything under the wrapper is gone and that the refusal did
    not sit and wait for it, which is what the sleeps can say and a real embed could not say in
    under half an hour."""

    def alive(self, pid):
        return subprocess.run(["kill", "-0", str(pid)], capture_output=True).returncode == 0

    def test_the_wrapper_and_everything_under_it_are_killed_not_waited_out(self):
        wrapper = subprocess.Popen(["bash", "-c", "sleep 60 & sleep 60"])
        self.addCleanup(wrapper.wait)
        self.addCleanup(wrapper.kill)
        for _ in range(50):
            under = subprocess.run(["pgrep", "-P", str(wrapper.pid)], capture_output=True, text=True).stdout.split()
            if len(under) == 2:
                break
            time.sleep(0.1)
        self.assertEqual(len(under), 2, "the tree to kill never came up")
        started = time.monotonic()
        r = sourced(f"kill_tree {wrapper.pid}")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertLess(time.monotonic() - started, 30.0)
        wrapper.wait(timeout=10)
        self.assertEqual([pid for pid in under if self.alive(pid)], [])

    def test_the_refusal_says_the_store_it_left_behind_is_half_written(self):
        # 4 and not 3: 3 is the binary's verdict status everything else in this kit reads as a
        # reading, and an operator — or a harness — reading this script's 3 the same way would take
        # a half-written store for an answered question.
        r = sourced("refuse_partial_store 'no repograph to sample' /tmp/beauty-crm-test")
        self.assertEqual(r.returncode, 4)
        self.assertIn("no repograph to sample", r.stderr)
        self.assertIn("/tmp/beauty-crm-test/.repograph/", r.stderr)
        self.assertIn("partial", r.stderr)
        self.assertIn("reset.sh", r.stderr)


if __name__ == "__main__":
    unittest.main()
