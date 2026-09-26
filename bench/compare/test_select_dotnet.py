"""The selection rule over a throwaway .NET tree: which names become cases, and which never may."""

import subprocess
import tempfile
import unittest
from pathlib import Path

import select_dotnet as S
import truth as T

GIT = ["git", "-c", "core.hooksPath=", "-c", "commit.gpgsign=false", "-c", "user.email=select@test", "-c", "user.name=select"]

TREE = {
    "libs-dotnet/Shop/Payments/Ledger.cs": "namespace Shop.Payments;\npublic class Ledger\n{\n    private Receipt Last;\n    public void Post() { }\n}\n",
    "libs-dotnet/Shop/Payments/CardGateway.cs": "namespace Shop.Payments;\npublic class CardGateway\n{\n    private readonly Ledger _ledger;\n    public void Charge() { _ledger.Post(); }\n}\n",
    "libs-dotnet/Shop/Payments/Receipt.cs": "namespace Shop.Payments;\npublic class Receipt { }\n",
    "libs-dotnet/Shop/Orders/OrderService.cs": "namespace Shop.Orders;\nusing Shop.Payments;\npublic class OrderService(CardGateway gateway)\n{\n    public void Place() => gateway.Charge();\n}\n",
    "libs-dotnet/Shop/Orders/Status.cs": "namespace Shop.Orders;\npublic enum Status { Open }\n",
    "libs-dotnet/Shop/Legacy/Orders.cs": "namespace Shop.Legacy;\npublic class Orders { }\n",
    "libs-dotnet/Shop/Legacy/CartA.cs": "namespace Shop.Legacy;\npublic partial class Cart { Orders First; }\n",
    "libs-dotnet/Shop/Legacy/CartB.cs": "namespace Shop.Legacy;\npublic partial class Cart { Orders Second; }\n",
    "libs-dotnet/Shop/Reports/Tracked.cs": "namespace Shop.Reports;\nclass Tracked\n{\n    Ledger Source { get; }\n    Receipt Last;\n    public Status Status { get; }\n}\n",
    "apps/web-ts/receipt.ts": "export type Receipt = { id: string };\n",
    "apps/web/Pages/Badge.razor": "<span>badge</span>\n",
    "apps/web/Pages/Lonely.razor": "<span>alone</span>\n",
    "apps/web/Pages/A.razor": "<Badge />\n<Lonely />\n",
    "apps/web/Pages/B.razor": "<div><Badge></Badge></div>\n",
    "apps/web/Pages/Checkout.razor": "@inject CardGateway Gateway\n<h3>Checkout</h3>\n@code {\n    void Pay() { Gateway.Charge(); }\n}\n",
}


class Selection(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.repo = Path(cls.tmp.name)
        subprocess.run([*GIT, "init", "-q", "-b", "main", str(cls.repo)], check=True, capture_output=True)
        cls.commit(TREE, "base")
        cls.commit({"libs-dotnet/Shop/Payments/Ledger.cs": TREE["libs-dotnet/Shop/Payments/Ledger.cs"] + "// one\n",
                    "libs-dotnet/Shop/Reports/Tracked.cs": TREE["libs-dotnet/Shop/Reports/Tracked.cs"] + "// one\n"}, "two C# files")
        cls.commit({"apps/web/Pages/Badge.razor": "<span>badge!</span>\n",
                    "libs-dotnet/Shop/Payments/CardGateway.cs": TREE["libs-dotnet/Shop/Payments/CardGateway.cs"] + "// two\n"}, "a component and a C# file")
        cls.facts = S.facts(cls.repo, S.tracked(cls.repo, S.DOTNET))

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    @classmethod
    def commit(cls, files, message):
        for rel, body in files.items():
            (cls.repo / rel).parent.mkdir(parents=True, exist_ok=True)
            (cls.repo / rel).write_text(body, encoding="utf8")
        subprocess.run([*GIT, "add", "-A"], cwd=cls.repo, check=True, capture_output=True)
        subprocess.run([*GIT, "commit", "-q", "-m", message], cwd=cls.repo, check=True, capture_output=True)

    def rev(self, spec):
        return subprocess.run(["git", "rev-parse", spec], cwd=self.repo, capture_output=True, text=True, check=True).stdout.strip()

    def test_tiers_follow_the_spec_and_one_reference_is_no_case(self):
        self.assertEqual([S.tier(n) for n in (1, 2, 5, 6, 19, 20)], [None, "narrow", "narrow", "wide", "wide", "hub"])

    def test_csharp_impact_takes_only_names_the_truth_can_count(self):
        # Status is also a member, Orders is a namespace segment, Cart is declared twice, and
        # OrderService is referenced by no file. Receipt is spelled by TypeScript too: it is kept, and
        # its case's `exts` leaves that file out of the truth.
        cases = S.csharp_impact(self.repo, self.facts, {"narrow": 9, "wide": 9, "hub": 9})
        self.assertCountEqual(cases, [
            {"kind": "impact", "target": "Ledger", "file": "libs-dotnet/Shop/Payments/Ledger.cs", "tier": "narrow", "lang": ".cs", "exts": list(S.DOTNET)},
            {"kind": "impact", "target": "CardGateway", "file": "libs-dotnet/Shop/Payments/CardGateway.cs", "tier": "narrow", "lang": ".cs", "exts": list(S.DOTNET)},
            {"kind": "impact", "target": "Receipt", "file": "libs-dotnet/Shop/Payments/Receipt.cs", "tier": "narrow", "lang": ".cs", "exts": list(S.DOTNET)},
        ])

    def test_razor_impact_takes_components_rendered_by_two_others(self):
        razor = [r for r in S.tracked(self.repo, S.DOTNET) if r.endswith(".razor")]
        self.assertEqual(S.razor_impact(self.repo, self.facts, razor, 9), [
            {"kind": "impact", "target": "Badge", "file": "apps/web/Pages/Badge.razor", "tier": "narrow", "lang": ".razor", "exts": list(S.DOTNET)},
        ])

    def test_a_trace_is_two_to_four_calls_through_injected_members(self):
        graph = T.di_call_graph(self.repo, ["apps", "libs-dotnet"])
        self.assertCountEqual(S.traces(graph, self.facts, {".cs": 9, ".razor": 9}), [
            {"kind": "trace", "from": "OrderService", "to": "Ledger", "expect": "path", "via": ["CardGateway"], "lang": ".cs"},
            {"kind": "trace", "from": "Checkout", "to": "Ledger", "expect": "path", "via": ["CardGateway"], "lang": ".razor"},
        ])

    def test_a_changes_base_is_the_smallest_n_that_carries_enough_files(self):
        self.assertEqual(S.changes_base(self.repo, ".cs", 2), (self.rev("HEAD~2"), 3))
        self.assertEqual(S.changes_base(self.repo, ".razor", 1), (self.rev("HEAD~1"), 1))
        self.assertIsNone(S.changes_base(self.repo, ".razor", 5))


class DuplicateComponent(unittest.TestCase):
    def test_a_component_name_two_files_declare_is_no_case(self):
        # The truth counts files spelling the word, so it cannot tell which of two same-named
        # components a tag renders; one declaring file is the rule for components as for types.
        tree = {
            "apps/admin/Shared/Badge.razor": "<span>admin</span>\n",
            "apps/web/Shared/Badge.razor": "<span>web</span>\n",
            "apps/web/Pages/A.razor": "<Badge />\n",
            "apps/web/Pages/B.razor": "<Badge></Badge>\n",
            "apps/web/Pages/C.razor": "<Badge />\n",
        }
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            subprocess.run([*GIT, "init", "-q", "-b", "main", str(repo)], check=True, capture_output=True)
            for rel, body in tree.items():
                (repo / rel).parent.mkdir(parents=True, exist_ok=True)
                (repo / rel).write_text(body, encoding="utf8")
            subprocess.run([*GIT, "add", "-A"], cwd=repo, check=True, capture_output=True)
            subprocess.run([*GIT, "commit", "-q", "-m", "base"], cwd=repo, check=True, capture_output=True)
            files = S.tracked(repo, S.DOTNET)
            razor = [r for r in files if r.endswith(".razor")]
            self.assertEqual(S.razor_impact(repo, S.facts(repo, files), razor, 9), [])


if __name__ == "__main__":
    unittest.main()
