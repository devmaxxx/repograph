"""The private Vue corpus's case selection on synthetic repositories: what it picks and what it waives."""

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import select_vue_cases as S

# An empty hooks path and no signing: the developer's global git config must not fail the fixture.
GIT = ["git", "-c", "core.hooksPath=", "-c", "commit.gpgsign=false", "-c", "user.email=select@test", "-c", "user.name=select"]


def repository(root: Path, commits: list[dict[str, str]]) -> list[str]:
    """A git repository at `root` with one commit per dict; the commit hashes, oldest first."""
    subprocess.run([*GIT, "init", "-q", "-b", "main", str(root)], check=True, capture_output=True)
    shas = []
    for i, files in enumerate(commits):
        for rel, body in files.items():
            (root / rel).parent.mkdir(parents=True, exist_ok=True)
            (root / rel).write_text(body, encoding="utf8")
        subprocess.run([*GIT, "add", "-A"], cwd=root, check=True, capture_output=True)
        subprocess.run([*GIT, "commit", "-q", "-m", f"c{i}"], cwd=root, check=True, capture_output=True)
        shas.append(subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, text=True).stdout.strip())
    return shas


SHOP = {
    "src/components/Cart.vue": "<template>\n  <ul class=\"cart\"></ul>\n</template>\n",
    "src/components/Checkout.vue": (
        "<template>\n  <Cart />\n</template>\n\n<script lang=\"ts\">\n"
        "import Cart from './Cart.vue';\nimport { Pricing } from '../pricing';\n\n"
        "export default class Checkout {\n  private pricing!: Pricing;\n  parts = [Cart];\n"
        "  pay(): number { return this.pricing.total(); }\n}\n</script>\n"
    ),
    "src/pricing.ts": "export class Pricing {\n  total(): number { return 1; }\n}\n",
    "src/routes.ts": "import Checkout from '@/components/Checkout.vue';\n\nexport const routes = [{ component: Checkout }];\n",
}
EDIT = {"src/components/Cart.vue": "<template>\n  <ol class=\"cart\"></ol>\n</template>\n"}


class Selection(unittest.TestCase):
    def test_a_corpus_with_every_kind_gets_its_cases_and_no_waiver(self):
        with tempfile.TemporaryDirectory() as d:
            repo, out = Path(d) / "corpus", Path(d) / "out"
            repo.mkdir()
            shas = repository(repo, [SHOP, EDIT])
            got = S.select(repo, out)
            cases = [json.loads(line) for line in (out / "blast.jsonl").read_text().splitlines()]
            self.assertEqual(cases[0], {"kind": "impact", "target": "Cart", "file": "src/components/Cart.vue", "tier": "narrow"})
            self.assertEqual(cases[1], {"kind": "impact", "target": "Checkout", "file": "src/components/Checkout.vue", "tier": "narrow"})
            self.assertEqual(cases[2], {"kind": "trace", "from": "Checkout", "to": "Pricing", "expect": "path", "via": []})
            self.assertEqual(cases[3]["base"], f"{shas[1][:12]}~1")
            self.assertEqual(got["kept"], {"impact": 2, "trace": 1, "changes": 1})
            self.assertEqual((got["vue"], got["scripts"], got["ts"]), (2, 1, 2))
            self.assertEqual((out / "waivers.txt").read_text(), "")
            self.assertEqual((out / "pin.txt").read_text().strip(), shas[1])
            self.assertEqual((out / "names.txt").read_text(), "Cart\nCheckout\nPricing\n")
            truth = json.loads((out / "truth.json").read_text())
            # A template-only edit names the component: the whole-file span, read from the truth side.
            self.assertEqual(truth["changes"][cases[3]["base"]]["symbols"], {"src/components/Cart.vue": ["Cart"]})

    def test_a_kind_with_no_candidate_is_waived_with_its_reason_class(self):
        with tempfile.TemporaryDirectory() as d:
            repo, out = Path(d) / "corpus", Path(d) / "out"
            repo.mkdir()
            repository(repo, [{"src/Lone.vue": "<template>\n  <p>alone</p>\n</template>\n"}])
            got = S.select(repo, out)
            self.assertEqual((out / "blast.jsonl").read_text(), "")
            self.assertEqual((out / "waivers.txt").read_text(),
                             "impact\tno-component-named-outside-its-file\n"
                             "trace\tno-injected-call-path-through-a-component\n"
                             "changes\tno-window-with-a-component-change\n")
            self.assertEqual(got["kept"], {"impact": 0, "trace": 0, "changes": 0})

    def test_the_printed_summary_carries_counts_and_no_name(self):
        with tempfile.TemporaryDirectory() as d:
            repo, out = Path(d) / "corpus", Path(d) / "out"
            repo.mkdir()
            repository(repo, [SHOP, EDIT])
            script = Path(__file__).resolve().parent / "select_vue_cases.py"
            printed = subprocess.run([sys.executable, str(script), "--corpus", str(repo), "--out", str(out)],
                                     capture_output=True, text=True, check=True).stdout
            for name in ("Cart", "Checkout", "Pricing", "routes", "src"):
                self.assertNotIn(name, printed)
            self.assertEqual(json.loads(printed)["kept"], {"impact": 2, "trace": 1, "changes": 1})
