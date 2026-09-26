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
    "libs-dotnet/Shop/Reports/Kind.cs": "namespace Shop.Reports;\npublic enum Kind\n{\n    [Obsolete] Open,\n    Closed = 2,\n}\n",
    "libs-dotnet/Shop/Reports/Open.cs": "namespace Shop.Reports;\npublic class Open { }\n",
    "libs-dotnet/Shop/Reports/UseA.cs": "namespace Shop.Reports;\nclass UseA { Kind K = Kind.Open; }\n",
    "libs-dotnet/Shop/Reports/UseB.cs": "namespace Shop.Reports;\nclass UseB { Kind K = Kind.Open; }\n",
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
        # OrderService is referenced by no file, and Open is also an enum constant. Receipt is spelled
        # by TypeScript too: it is kept, and its case's `exts` leaves that file out of the truth.
        cases = S.csharp_impact(self.repo, self.facts, {"narrow": 9, "wide": 9, "hub": 9})
        self.assertCountEqual(cases, [
            {"kind": "impact", "target": "Ledger", "file": "libs-dotnet/Shop/Payments/Ledger.cs", "tier": "narrow", "lang": ".cs", "exts": list(S.DOTNET)},
            {"kind": "impact", "target": "CardGateway", "file": "libs-dotnet/Shop/Payments/CardGateway.cs", "tier": "narrow", "lang": ".cs", "exts": list(S.DOTNET)},
            {"kind": "impact", "target": "Receipt", "file": "libs-dotnet/Shop/Payments/Receipt.cs", "tier": "narrow", "lang": ".cs", "exts": list(S.DOTNET)},
            {"kind": "impact", "target": "Kind", "file": "libs-dotnet/Shop/Reports/Kind.cs", "tier": "narrow", "lang": ".cs", "exts": list(S.DOTNET)},
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


def committed(tmp: str, tree: dict[str, str]) -> Path:
    repo = Path(tmp)
    subprocess.run([*GIT, "init", "-q", "-b", "main", str(repo)], check=True, capture_output=True)
    for rel, body in tree.items():
        (repo / rel).parent.mkdir(parents=True, exist_ok=True)
        (repo / rel).write_text(body, encoding="utf8")
    subprocess.run([*GIT, "add", "-A"], cwd=repo, check=True, capture_output=True)
    subprocess.run([*GIT, "commit", "-q", "-m", "base"], cwd=repo, check=True, capture_output=True)
    return repo


def uses(name: str, *files: str) -> dict[str, str]:
    return {rel: f"namespace Use;\nclass {Path(rel).stem} {{ {name} Field; }}\n" for rel in files}


class Rules(unittest.TestCase):
    def selected(self, tree: dict[str, str]) -> tuple[list[dict], list[dict]]:
        with tempfile.TemporaryDirectory() as tmp:
            repo = committed(tmp, tree)
            files = S.tracked(repo, S.DOTNET)
            f = S.facts(repo, files)
            razor = [r for r in files if r.endswith(".razor")]
            return S.csharp_impact(repo, f, {"narrow": 9, "wide": 9, "hub": 9}), S.razor_impact(repo, f, razor, 9)

    def targets(self, cases: list[dict]) -> list[str]:
        return sorted(c["target"] for c in cases)

    def test_a_component_name_two_files_declare_is_no_case(self):
        # The truth counts files spelling the word, so it cannot tell which of two same-named
        # components a tag renders; one declaring file is the rule for components as for types.
        _, razor = self.selected({
            "apps/admin/Shared/Badge.razor": "<span>admin</span>\n",
            "apps/web/Shared/Badge.razor": "<span>web</span>\n",
            "apps/web/Pages/A.razor": "<Badge />\n",
            "apps/web/Pages/B.razor": "<Badge></Badge>\n",
            "apps/web/Pages/C.razor": "<Badge />\n",
        })
        self.assertEqual(razor, [])

    def test_a_declaration_outside_the_roots_still_counts(self):
        # The truth reads the whole tree, so a second `Widget` under `tools` would be counted as a reference.
        cs, _ = self.selected({
            "apps/Web/Widget.cs": "namespace Web;\npublic class Widget { }\n",
            "tools/Gen/Thing.cs": "namespace Gen;\nclass Thing\n{\n    class Widget\n    {\n    }\n}\n",
            **uses("Widget", "apps/Web/UseA.cs", "apps/Web/UseB.cs"),
        })
        self.assertNotIn("Widget", self.targets(cs))

    def test_one_declaration_unless_the_parts_are_partial(self):
        cs, _ = self.selected({
            "apps/Lib/Pair.cs": "namespace Lib;\nclass Left\n{\n    class Item\n    {\n    }\n}\nclass Right\n{\n    class Item\n    {\n    }\n}\n",
            "apps/Lib/Box.cs": "namespace Lib;\nclass Box\n{\n}\nclass Box<T>\n{\n}\n",
            "apps/Lib/Split.cs": "namespace Lib;\npartial class Split\n{\n}\npartial class Split\n{\n}\n",
            **uses("Item", "apps/Use/ItemA.cs", "apps/Use/ItemB.cs"),
            **uses("Box", "apps/Use/BoxA.cs", "apps/Use/BoxB.cs"),
            **uses("Split", "apps/Use/SplitA.cs", "apps/Use/SplitB.cs"),
        })
        self.assertEqual(self.targets(cs), ["Split"])

    def test_a_tag_in_a_comment_or_a_string_renders_nothing(self):
        # B and C still name Badge in code, so its references reach a tier; only the renders fall short.
        _, razor = self.selected({
            "apps/web/Shared/Badge.razor": "<span>badge</span>\n",
            "apps/web/Pages/A.razor": "<Badge />\n",
            "apps/web/Pages/B.razor": "@* <Badge /> *@\n@code {\n    string n = nameof(Badge);\n}\n",
            "apps/web/Pages/C.razor": "@code {\n    string s = \"<Badge />\";\n    string n = nameof(Badge);\n}\n",
        })
        self.assertEqual(razor, [])

    def test_the_generated_client_gives_at_most_two_cases(self):
        tree = {f"{S.CLIENT}Models/{n}.cs": f"namespace Client;\npublic class {n} {{ }}\n" for n in ("Alpha", "Beta", "Gamma")}
        for n in ("Alpha", "Beta", "Gamma"):
            tree.update(uses(n, f"apps/Use/{n}A.cs", f"apps/Use/{n}B.cs"))
        cs, _ = self.selected(tree)
        self.assertEqual(len(cs), S.CLIENT_MAX)
        self.assertTrue(all(c["file"].startswith(S.CLIENT) for c in cs))

    def test_a_component_and_its_code_behind_are_one_declaration(self):
        cs, razor = self.selected({
            "apps/web/Shared/Badge.razor": "<span>badge</span>\n",
            "apps/web/Shared/Badge.razor.cs": "namespace Web.Shared;\npublic partial class Badge\n{\n}\n",
            "apps/web/Pages/A.razor": "<Badge />\n",
            "apps/web/Pages/B.razor": "<div><Badge></Badge></div>\n",
        })
        self.assertEqual(self.targets(cs), [])
        self.assertEqual([(c["target"], c["file"]) for c in razor], [("Badge", "apps/web/Shared/Badge.razor")])

    def test_the_output_may_not_be_this_repository(self):
        root = Path(S.__file__).resolve().parents[2]
        self.assertFalse(S.private(root))
        self.assertFalse(S.private(root / "bench"))
        self.assertTrue(S.private(Path(tempfile.gettempdir()) / "private-cases"))


if __name__ == "__main__":
    unittest.main()
