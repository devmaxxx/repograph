import os
import re
import shutil
import subprocess
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
QUIET = HERE / "quiet.sh"

HEAD = re.compile(r"^quiet: idle=\S+ load1=\S+ ac=\d+ avail=\S+GB swap=\S+GB busy=(\d+) "
                  r"\(cargo/rustc/repograph: (\d+), node ≥5\.0%: (\d+)\)$")


def sourced(snippet, stdin=""):
    """quiet.sh's functions without its body: sourcing the script defines and runs nothing."""
    return subprocess.run(["bash", "-c", f"source '{QUIET}'; {snippet}"], input=stdin,
                          capture_output=True, text=True, check=True).stdout


def decided(rows, kind):
    """The pipeline `main` runs, minus the `ps` that feeds it: ancestors dropped, then one rule."""
    return sourced(f"busy_lines | busy_rows {kind}", stdin=rows).strip()


@unittest.skipUnless(os.name == "posix", "bash, ps and awk: the macOS kit's own platform")
class Rules(unittest.TestCase):
    """Half of each case is real and half is simulated: the PIDs are real — this test process
    really is an ancestor of the shell doing the filtering, and the `sleep` really is not — while
    the CPU percentage and the executable path on a row stand in for what `ps` printed, so both
    rules are read without waiting for a machine that happens to be loud in the right way.
    `WholeScript` below simulates neither half and asserts only what it can."""

    def setUp(self):
        other = subprocess.Popen(["sleep", "30"])
        self.addCleanup(other.wait)
        self.addCleanup(other.terminate)
        self.other = other.pid

    def test_an_idle_node_outside_the_chain_does_not_refuse(self):
        self.assertEqual(decided(f"{self.other} 0.0 /usr/local/bin/node\n", "node"), "")

    def test_a_node_burning_a_core_outside_the_chain_refuses(self):
        row = f"{self.other} 12.0 /usr/local/bin/node"
        self.assertEqual(decided(row + "\n", "node"), row)

    def test_a_node_in_the_ancestor_chain_does_not_refuse_at_any_cpu(self):
        self.assertEqual(decided(f"{os.getpid()} 99.9 /usr/local/bin/node\n", "node"), "")

    def test_a_cargo_at_zero_percent_refuses(self):
        row = f"{self.other} 0.0 /Users/max/.cargo/bin/cargo"
        self.assertEqual(decided(row + "\n", "project"), row)

    def test_a_repograph_at_zero_percent_refuses_under_any_name_it_is_built_as(self):
        row = f"{self.other} 0.0 /Users/max/bench/bin/repograph-main"
        self.assertEqual(decided(row + "\n", "project"), row)

    def test_a_path_with_a_space_in_it_is_still_read_as_its_own_name(self):
        row = f"{self.other} 12.0 /Applications/Some App/Contents/MacOS/node"
        self.assertEqual(decided(row + "\n", "node"), row)

    def test_the_two_rules_do_not_borrow_each_others_rows(self):
        rows = f"{self.other} 0.0 /usr/local/bin/node\n{self.other} 0.0 /usr/bin/rustc\n"
        self.assertEqual(decided(rows, "node"), "")
        self.assertEqual(decided(rows, "project"), f"{self.other} 0.0 /usr/bin/rustc")

    def test_one_ancestor_does_not_exempt_the_rest_of_the_list(self):
        rows = f"{os.getpid()} 40.0 /usr/local/bin/node\n{self.other} 40.0 /usr/local/bin/node\n"
        self.assertEqual(decided(rows, "node"), f"{self.other} 40.0 /usr/local/bin/node")

    def test_the_walk_reaches_the_process_that_launched_the_shell(self):
        self.assertIn(str(os.getpid()), sourced("ancestor_pids").split())


# The four readings `main` takes off the machine — `vm_stat`, `sysctl vm.swapusage`, `pmset -g
# batt` and `top -l 1` — are Darwin's, and the kit measures on Darwin. The classes that source a
# function and feed it rows need none of them and run wherever bash does; the one that runs the
# script whole needs all four, so on another platform it is skipped rather than red.
@unittest.skipUnless(sys.platform == "darwin", "vm_stat, sysctl vm.swapusage, pmset and `top -l`")
class WholeScript(unittest.TestCase):
    """The script over this machine's own process table. The verdict is not asserted — a machine
    that is loud for other reasons still fails the other three clauses — only that the line says
    which rule counted what, and that no row the node clause named is below the bar."""

    def test_the_line_names_what_each_rule_counted(self):
        out = subprocess.run(["bash", str(QUIET)], capture_output=True, text=True).stdout
        head = HEAD.match(out.splitlines()[0])
        self.assertIsNotNone(head, out)
        busy, project, node = (int(g) for g in head.groups())
        self.assertEqual(busy, project + node)

    def test_no_row_below_the_bar_is_named_by_the_node_clause(self):
        out = subprocess.run(["bash", str(QUIET)], capture_output=True, text=True).stdout
        named = out.split("node processes at 5.0% CPU or above\n")
        for line in (named[1].splitlines() if len(named) > 1 else []):
            pid, pcpu, comm = line.split(maxsplit=2)
            self.assertGreaterEqual(float(pcpu), 5.0, line)
            self.assertIn("node", comm)

    @unittest.skipIf(shutil.which("node") is None, "no node on this machine to be the harness")
    def test_the_node_that_launched_the_script_is_not_counted(self):
        launcher = ("console.log('NODEPID ' + process.pid);"
                    "try { require('child_process').execFileSync('bash', [process.argv[1]], {stdio: 'inherit'}); }"
                    "catch (e) { /* a machine that is not quiet exits 1 */ }")
        out = subprocess.run(["node", "-e", launcher, str(QUIET)], capture_output=True, text=True).stdout
        lines = out.splitlines()
        pid = lines[0].split()[1]
        self.assertTrue(any(HEAD.match(line) for line in lines), out)
        self.assertEqual([line for line in lines if line.split()[:1] == [pid]], [])


if __name__ == "__main__":
    unittest.main()


@unittest.skipUnless(os.name == "posix", "bash: the macOS kit's own platform")
class Wait(unittest.TestCase):
    """`--wait` exists because the kit's own reset is what makes the machine loud: `main` is
    stubbed here so the loop's decision is read without waiting on a real machine to settle."""

    def loop(self, script, budget):
        return subprocess.run(["bash", "-c", f"source '{QUIET}'; {script}; wait_quiet {budget}"],
                              capture_output=True, text=True)

    def test_a_machine_that_settles_is_waited_out_and_the_settling_is_reported(self):
        # `main` fails once and then succeeds, which is the reset-then-quiet shape.
        r = self.loop("n=0; main() { n=$((n+1)); [ $n -gt 1 ]; }; step=0", 60)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("quiet: reached after", r.stdout)

    def test_a_machine_that_stays_busy_still_refuses_when_the_budget_runs_out(self):
        r = self.loop("main() { return 1; }", 0)
        self.assertEqual(r.returncode, 1)
        self.assertIn("still not quiet after 0s — refusing", r.stdout)

    def test_a_machine_already_quiet_says_nothing_about_settling(self):
        r = self.loop("main() { return 0; }", 600)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertNotIn("reached after", r.stdout)

    def test_an_unknown_argument_is_the_usage_line_not_a_reading(self):
        r = subprocess.run(["bash", str(QUIET), "--forever"], capture_output=True, text=True)
        self.assertEqual(r.returncode, 2)
        self.assertIn("usage: quiet.sh", r.stderr)
