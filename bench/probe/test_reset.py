import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
RESET = HERE / "reset.sh"
SMALL = "intfloat/multilingual-e5-small"
FIXTURE_GRAPH = '{"nodes": {"FR-1": {}}, "edges": []}'


def git(*args, cwd):
    return subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True, text=True).stdout


@unittest.skipUnless(os.name == "posix", "bash, git worktree and rsync: the macOS kit's own platform")
class Reset(unittest.TestCase):
    """The contract of reset.sh on a throwaway repository: what it refuses, and what a reset
    leaves behind. The settle walk is a stub that prints what `update` prints — the script's
    own job ends where the binary's begins, and the counts it checks are read off that line."""

    def setUp(self):
        # `.resolve()`: the script works in `pwd -P` paths so that a symlinked argument cannot
        # name one directory and mean another, and on macOS the temporary directory is under the
        # `/var` -> `/private/var` link.
        self.tmp = Path(tempfile.mkdtemp()).resolve()
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.repo = self.tmp / "repo"
        self.repo.mkdir()
        git("init", "-q", cwd=self.repo)
        git("config", "user.email", "t@t", cwd=self.repo)
        git("config", "user.name", "t", cwd=self.repo)
        (self.repo / "docs").mkdir()
        (self.repo / "docs/a.md").write_text("# a\n")
        (self.repo / ".gitignore").write_text(".repograph/\n")
        git("add", ".", cwd=self.repo)
        git("commit", "-q", "-m", "one", cwd=self.repo)
        self.sha = git("rev-parse", "HEAD", cwd=self.repo).strip()
        self.wt = self.tmp / "wt"
        git("worktree", "add", "-q", "--detach", str(self.wt), self.sha, cwd=self.repo)
        git("worktree", "lock", str(self.wt), cwd=self.repo)
        self.fix = self.tmp / "fix"
        (self.fix / ".repograph").mkdir(parents=True)
        (self.fix / ".repograph/graph.json").write_text(FIXTURE_GRAPH)
        (self.fix / ".repograph/manifest.json").write_text('{"files": {}}')
        (self.fix / ".repograph/vectors.f32").write_bytes(b"v")
        self.calls = self.tmp / "calls"
        self.bin = self.tmp / "repograph-stub"
        self.stub("changed 0 removed 0 nodes 1 edges 0")

    def stub(self, line):
        self.bin.write_text(f'#!/bin/sh\necho "$@" >> "{self.calls}"\necho "{line}"\n')
        self.bin.chmod(0o755)

    def run_reset(self, model=SMALL, **env):
        # `model` is the only channel for the declaration, and `None` means "not declared" — a
        # second copy of it among the `**env` overrides would be shadowed by this one and a case
        # that meant to run under another model would silently run under the small one.
        e = {**os.environ, "FIX": str(self.fix), "WT": str(self.wt), "PIN": self.sha[:8], "BIN": str(self.bin), **env}
        if model is None:
            e.pop("REPOGRAPH_EMBED_MODEL", None)
        else:
            e["REPOGRAPH_EMBED_MODEL"] = model
        return subprocess.run(["bash", str(RESET)], env=e, capture_output=True, text=True)

    def dirty(self):
        (self.wt / "docs/a.md").write_text("# edited\n")
        (self.wt / "docs/oq.md").write_text("**OQ-1**\n")
        (self.wt / ".repograph").mkdir(exist_ok=True)
        (self.wt / ".repograph/graph.json").write_text("{}")
        (self.wt / ".repograph/left-by-an-arm").write_text("x")
        (self.wt / "repograph.toml").write_text('embed_model = "wrong"\n')

    def test_a_dirty_tree_and_a_foreign_store_are_put_back(self):
        self.dirty()
        r = self.run_reset()
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertEqual((self.wt / "docs/a.md").read_text(), "# a\n")
        self.assertFalse((self.wt / "docs/oq.md").exists())
        self.assertEqual((self.wt / ".repograph/graph.json").read_text(), FIXTURE_GRAPH)
        self.assertEqual((self.wt / ".repograph/vectors.f32").read_bytes(), b"v")
        self.assertFalse((self.wt / ".repograph/left-by-an-arm").exists(), "--delete: what an arm left is gone")
        self.assertEqual(self.calls.read_text(), f"--repo {self.wt} --no-dense update\n")
        self.assertIn("settle  changed 0 removed 0 nodes 1 edges 0", r.stdout)

    def test_the_config_an_earlier_reset_wrote_is_gone_and_the_tree_is_clean(self):
        # The whole of the reason `changes` rows are read here: an untracked `repograph.toml` is a
        # changed path like any other, so a row that touched one file would map two.
        self.dirty()
        r = self.run_reset()
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertFalse((self.wt / "repograph.toml").exists())
        self.assertEqual(git("status", "--porcelain", cwd=self.wt), "")

    def test_a_reset_without_the_declaration_is_refused_and_nothing_is_touched(self):
        self.dirty()
        r = self.run_reset(model=None)
        self.assertEqual(r.returncode, 2, r.stdout + r.stderr)
        self.assertIn("REPOGRAPH_EMBED_MODEL", r.stderr)
        self.assertEqual((self.wt / "docs/a.md").read_text(), "# edited\n")
        self.assertFalse(self.calls.exists())

    def test_a_reset_under_another_model_is_refused_and_nothing_is_touched(self):
        self.dirty()
        r = self.run_reset(model="intfloat/multilingual-e5-large")
        self.assertEqual(r.returncode, 2, r.stdout + r.stderr)
        self.assertIn("intfloat/multilingual-e5-large", r.stderr)
        self.assertEqual((self.wt / "docs/a.md").read_text(), "# edited\n")
        self.assertFalse(self.calls.exists())

    def test_an_unlocked_worktree_is_refused_and_nothing_is_touched(self):
        git("worktree", "unlock", str(self.wt), cwd=self.repo)
        self.dirty()
        r = self.run_reset()
        self.assertEqual(r.returncode, 2, r.stdout + r.stderr)
        self.assertIn("not locked", r.stderr)
        self.assertEqual((self.wt / "docs/a.md").read_text(), "# edited\n")
        self.assertFalse(self.calls.exists())

    def test_a_worktree_at_another_commit_is_refused(self):
        r = self.run_reset(PIN="0000000")
        self.assertEqual(r.returncode, 2, r.stderr)
        self.assertIn("is not at 0000000", r.stderr)

    def test_the_fixture_cannot_be_named_as_the_target(self):
        r = self.run_reset(FIX=str(self.wt))
        self.assertEqual(r.returncode, 2, r.stderr)
        self.assertIn("same directory", r.stderr)

    def test_a_directory_that_is_no_worktree_is_refused(self):
        plain = self.tmp / "plain"
        plain.mkdir()
        r = self.run_reset(WT=str(plain))
        self.assertEqual(r.returncode, 2, r.stderr)
        self.assertIn("not a registered git worktree", r.stderr)

    def test_a_fixture_without_a_store_is_refused(self):
        (self.fix / ".repograph/graph.json").unlink()
        r = self.run_reset()
        self.assertEqual(r.returncode, 2, r.stderr)
        self.assertIn("not a store to restore from", r.stderr)

    def test_a_settle_that_changes_anything_or_miscounts_fails(self):
        self.stub("changed 1 removed 0 nodes 1 edges 0")
        r = self.run_reset()
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("changed something", r.stderr)
        self.stub("changed 0 removed 0 nodes 2 edges 0")
        r = self.run_reset()
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("fixture holds 1 0", r.stderr)


if __name__ == "__main__":
    unittest.main()
