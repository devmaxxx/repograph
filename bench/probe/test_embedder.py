import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

import judge

HERE = Path(__file__).resolve().parent
EMBEDDER = HERE / "embedder.sh"
SMALL = "intfloat/multilingual-e5-small"
LARGE = "intfloat/multilingual-e5-large"

# The four scripts that run a writer or a reader against the writable worktree, each with arguments
# that name nothing real. The declaration is a precondition of the whole kit, so every one of them
# has to refuse over it before it looks at an argument, a directory or a store — which is also what
# makes these cases cheap enough to be unit tests: none of the four gets far enough to need one.
SCRIPTS = {
    "reset.sh": [],
    "readers.sh": ["bin/repograph", "wt", "log", "1"],
    "embed.sh": ["control-1", "bin/repograph", "wt", "log"],
    "arms.sh": ["bin/repograph", "wt", "log", "tag"],
}


def sourced(snippet, model=None):
    """embedder.sh's calls without a script around them: sourcing it defines them and runs nothing."""
    return run(["bash", "-c", f"source '{EMBEDDER}'; {snippet}"], os.getcwd(), model)


def run(argv, cwd, model):
    env = {k: v for k, v in os.environ.items() if k != "REPOGRAPH_EMBED_MODEL"}
    if model is not None:
        env["REPOGRAPH_EMBED_MODEL"] = model
    return subprocess.run(argv, cwd=cwd, capture_output=True, text=True, env=env)


@unittest.skipUnless(os.name == "posix", "bash: the macOS kit's own platform")
class Declaration(unittest.TestCase):
    """Which weights a row was read under, declared in the environment and never guessed."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp()).resolve()
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def script(self, name, model):
        # `reset.sh`'s three directories are named away from this machine's real ones on purpose: a
        # check that ran after them would reset the corpus worktree instead of refusing, and this
        # case would notice by the wording rather than by the damage.
        env = {"FIX": str(self.tmp / "fix"), "WT": str(self.tmp / "wt"), "BIN": str(self.tmp / "bin")}
        argv = ["bash", str(HERE / name), *SCRIPTS[name]]
        e = {**os.environ, **env}
        if model is None:
            e.pop("REPOGRAPH_EMBED_MODEL", None)
        else:
            e["REPOGRAPH_EMBED_MODEL"] = model
        return subprocess.run(argv, cwd=self.tmp, capture_output=True, text=True, env=e)

    def test_the_declared_model_reaches_the_binary_a_script_runs(self):
        # Declared as a plain shell variable and not in the environment, which is what makes the
        # assertion bite: what a row is read under is what the binary's own process sees, and a
        # declaration that stopped at the script would leave the binary on the built-in default.
        r = sourced(f"REPOGRAPH_EMBED_MODEL={SMALL}; embedder_declared && "
                    "bash -c 'echo child sees $REPOGRAPH_EMBED_MODEL'", None)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn(f"child sees {SMALL}", r.stdout)

    def test_an_absent_declaration_refuses_and_says_what_to_export(self):
        r = sourced("embedder_declared", None)
        self.assertEqual(r.returncode, 2, r.stdout)
        self.assertIn("REPOGRAPH_EMBED_MODEL", r.stderr)
        self.assertIn(SMALL, r.stderr)

    def test_another_model_refuses_and_names_both(self):
        r = sourced("embedder_declared", LARGE)
        self.assertEqual(r.returncode, 2, r.stdout)
        self.assertIn(LARGE, r.stderr)
        self.assertIn(SMALL, r.stderr)

    def test_every_script_that_runs_a_reader_or_a_writer_refuses_without_it(self):
        for name in SCRIPTS:
            with self.subTest(script=name):
                r = self.script(name, None)
                self.assertEqual(r.returncode, 2, r.stdout + r.stderr)
                self.assertIn("REPOGRAPH_EMBED_MODEL", r.stderr)

    def test_every_script_refuses_a_declaration_naming_another_model(self):
        for name in SCRIPTS:
            with self.subTest(script=name):
                r = self.script(name, LARGE)
                self.assertEqual(r.returncode, 2, r.stdout + r.stderr)
                self.assertIn(LARGE, r.stderr)

    def test_the_line_a_transcript_carries_is_not_read_back_as_a_row(self):
        # It sits in `summary.txt` beside the rows `judge.py medians` takes its medians over, the
        # way the quiet line does, so a reading says which embedder it was taken under. A line that
        # parsed as a row would enter one of those medians as a run with no numbers on it.
        r = sourced("embedder_line", SMALL)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn(SMALL, r.stdout)
        self.assertEqual(judge.medians([r.stdout]), {})


if __name__ == "__main__":
    unittest.main()
