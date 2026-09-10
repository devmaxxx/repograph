import os
import shutil
import subprocess
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
QUIET = HERE / "quiet.sh"


def sourced(snippet, stdin=""):
    """quiet.sh's functions without its body: sourcing the script defines and runs nothing."""
    return subprocess.run(["bash", "-c", f"source '{QUIET}'; {snippet}"], input=stdin,
                          capture_output=True, text=True, check=True).stdout


@unittest.skipUnless(os.name == "posix", "bash, ps and pgrep: the macOS kit's own platform")
class Ancestors(unittest.TestCase):
    """One half of the clause is real here and one half is simulated: the PIDs are real — this
    test process really is an ancestor of the shell doing the filtering, and the `sleep` really is
    not — while the name `node` on a fixture line stands in for what `pgrep` matched, so the
    filter's decision is read without the machine's own dozens of node processes in the way.
    `RealNode` below simulates neither half."""

    def test_the_walk_reaches_the_process_that_launched_the_shell(self):
        self.assertIn(str(os.getpid()), sourced("ancestor_pids").split())

    def test_a_node_in_the_ancestor_chain_does_not_refuse(self):
        self.assertEqual(sourced("busy_lines", stdin=f"{os.getpid()} node\n"), "")

    def test_a_node_outside_the_ancestor_chain_does(self):
        other = subprocess.Popen(["sleep", "30"])
        self.addCleanup(other.wait)
        self.addCleanup(other.terminate)
        kept = sourced("busy_lines", stdin=f"{other.pid} node\n").strip()
        self.assertEqual(kept, f"{other.pid} node")

    def test_one_ancestor_does_not_exempt_the_rest_of_the_list(self):
        other = subprocess.Popen(["sleep", "30"])
        self.addCleanup(other.wait)
        self.addCleanup(other.terminate)
        lines = sourced("busy_lines", stdin=f"{os.getpid()} node\n{other.pid} node\n").splitlines()
        self.assertEqual(lines, [f"{other.pid} node"])


@unittest.skipUnless(os.name == "posix", "bash, ps and pgrep: the macOS kit's own platform")
@unittest.skipIf(shutil.which("node") is None, "no node on this machine to be the noise or the harness")
class RealNode(unittest.TestCase):
    """The whole script, with a real `node` process on each side of the rule. The verdict is not
    asserted — a machine that is loud for other reasons still fails the other three clauses — only
    which PIDs the busy clause named, which is what the correction is about."""

    def test_a_sibling_node_is_named_by_the_busy_clause(self):
        noise = subprocess.Popen(["node", "-e", "setTimeout(() => {}, 30000)"])
        self.addCleanup(noise.wait)
        self.addCleanup(noise.terminate)
        out = subprocess.run(["bash", str(QUIET)], capture_output=True, text=True).stdout
        self.assertIn("cargo/rustc/node/repograph processes running", out)
        self.assertIn(f"{noise.pid} node", out.splitlines())

    def test_the_node_that_launched_the_script_is_not(self):
        launcher = ("console.log('NODEPID ' + process.pid);"
                    "try { require('child_process').execFileSync('bash', [process.argv[1]], {stdio: 'inherit'}); }"
                    "catch (e) { /* a machine that is not quiet exits 1 */ }")
        out = subprocess.run(["node", "-e", launcher, str(QUIET)], capture_output=True, text=True).stdout
        lines = out.splitlines()
        pid = lines[0].split()[1]
        self.assertTrue(any(line.startswith("quiet: ") for line in lines), out)
        self.assertNotIn(f"{pid} node", lines)


if __name__ == "__main__":
    unittest.main()
