"""The GraphQL case selection rule, on a synthetic repository: the private corpus's cases are never committed, so
the rule is what gets reviewed, and this pins what it picks and what it refuses."""

import subprocess
import tempfile
import unittest
from pathlib import Path

import select_graphql_cases as S

GIT = ["git", "-c", "core.hooksPath=", "-c", "commit.gpgsign=false", "-c", "user.email=select@test", "-c", "user.name=select"]

FILES = {
    "schema.graphql": "type Shelf { id: ID }\ntype Book { title: String }\ntype Tag { name: String }\n",
    "web/list.gql": "query ListShelves { shelves { ...ShelfCard ...Tag } }\n",
    "web/card.gql": "fragment ShelfCard on Shelf { id books { ...BookLine } }\n",
    "web/line.gql": "fragment BookLine on Book { title }\n",
    "web/one.gql": "query One { book { ...BookLine ...Tag } }\n",
    "web/two.gql": "mutation Two { add { ...BookLine ...Tag } }\n",
    "web/three.gql": "subscription Three { added { ...BookLine ...Tag } }\n",
    "web/five.gql": "query Five { tags { ...Tag } }\n",
    "web/tag.gql": "fragment Tag on Tag { name }\n",
}


def git(repo: Path, *args: str) -> str:
    return subprocess.run([*GIT, "-C", str(repo), *args], check=True, capture_output=True, text=True).stdout


class SelectGraphQlCases(unittest.TestCase):
    def test_the_rule_picks_an_unambiguous_fragment_a_two_hop_chain_and_the_newest_small_window(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            git(repo, "init", "-q", "-b", "main")
            for rel, text in FILES.items():
                (repo / rel).parent.mkdir(parents=True, exist_ok=True)
                (repo / rel).write_text(text, encoding="utf8")
            git(repo, "add", "-A")
            git(repo, "commit", "-q", "-m", "documents")
            for rel in ("web/list.gql", "web/one.gql", "web/two.gql"):
                (repo / rel).write_text(FILES[rel] + "# edited\n", encoding="utf8")
            git(repo, "commit", "-q", "-am", "edit three documents")
            head = git(repo, "rev-parse", "HEAD").strip()

            cases = S.select(repo)

        self.assertEqual(cases, [
            # `Tag` is spread by more documents, but a type shares its name, so `impact Tag` could mean either.
            {"kind": "impact", "target": "BookLine", "file": "web/line.gql", "tier": "wide", "exts": [".gql", ".graphql"]},
            {"kind": "trace", "from": "query/ListShelves", "to": "fragment/BookLine", "expect": "path", "via": ["fragment/ShelfCard"]},
            {"kind": "trace", "from": "fragment/BookLine", "to": "query/ListShelves", "expect": "none", "via": []},
            {"kind": "changes", "base": f"{head}~1", "note": "the newest window holding three or more GraphQL documents and at most 40 files"},
        ])

    def test_a_fragment_spelled_by_a_document_that_does_not_spread_it_is_refused(self):
        docs = {"a.gql": "fragment F on T { x }\n", "b.gql": "query B { ...F }\n", "c.gql": "query C { ...F }\n",
                "d.gql": "query D { ...F }\n", "e.gql": "query E { F }\n"}
        blanked = {rel: S.truth.blank_graphql(text) for rel, text in docs.items()}
        declared = {rel: {n for _, n in S.truth.graphql_declarations(t)} for rel, t in blanked.items()}
        spreads = {rel: S.truth.graphql_spread_calls(t) for rel, t in blanked.items()}
        self.assertIsNone(S.impact_case(blanked, declared, spreads))


if __name__ == "__main__":
    unittest.main()
