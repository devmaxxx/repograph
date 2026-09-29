"""The GraphQL blast cases on a private corpus, chosen at run time by the rules written here.

The corpus is an employer's, so the cases it yields never enter this repository: this script writes them to
a private path and prints counts. What is reviewed is the rule, pinned by `test_select_graphql_cases.py`.
"""

import argparse
import json
import re
import subprocess
from collections import Counter, defaultdict
from pathlib import Path

import truth

EXTS = (".gql", ".graphql")
OPERATIONS = ("query/", "mutation/", "subscription/")
MOST_FILES = 40


def git(repo: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), "-c", "core.quotepath=false", *args],
                          capture_output=True, text=True, check=True).stdout


def documents(repo: Path) -> dict[str, str]:
    """Every tracked GraphQL document, blanked as the truth reads it."""
    rels = [r for r in git(repo, "ls-files", *(f"*{e}" for e in EXTS)).split("\n") if r]
    # `ls-files` still lists a file deleted from the working tree.
    return {rel: truth.blank_graphql((repo / rel).read_text(encoding="utf8", errors="replace")) for rel in rels if (repo / rel).is_file()}


def label(name: str) -> str:
    """What the store labels a declaration: the name after its namespace, `Type.field` for a field."""
    return name.rsplit("/", 1)[-1]


def homes(declared: dict[str, set[str]]) -> dict[str, set[str]]:
    """Each declared name to the files declaring it."""
    where: dict[str, set[str]] = defaultdict(set)
    for rel, names in declared.items():
        for n in names:
            where[n].add(rel)
    return where


def impact_case(docs: dict[str, str], declared: dict[str, set[str]], spreads: dict[str, list[tuple[str, str]]]) -> dict | None:
    labels = Counter(label(n).lower() for names in declared.values() for n in names)
    where = homes(declared)
    best = None
    for name, rels in sorted(where.items()):
        if not name.startswith("fragment/") or len(rels) != 1 or labels[label(name).lower()] != 1:
            continue
        (home,) = rels
        word = re.compile(rf"\b{re.escape(label(name))}\b")
        spelling = {rel for rel, text in docs.items() if rel != home and word.search(text)}
        spreading = {rel for rel, edges in spreads.items() if rel != home and any(target == name for _, target in edges)}
        # A document that spells the name without spreading it from a named definition is a file the truth would
        # want and no answer could name.
        if len(spreading) >= 3 and spelling == spreading and (best is None or len(spreading) > best[0]):
            best = (len(spreading), name, home)
    if best is None:
        return None
    return {"kind": "impact", "target": label(best[1]), "file": best[2], "tier": "wide", "exts": list(EXTS)}


def trace_cases(declared: dict[str, set[str]], spreads: dict[str, list[tuple[str, str]]]) -> list[dict]:
    where = homes(declared)
    calls: dict[str, set[str]] = defaultdict(set)
    for edges in spreads.values():
        for owner, target in edges:
            calls[owner].add(target)
    once = lambda n: len(where[n]) == 1  # noqa: E731 — a name declared twice makes `trace <id>` pick one of two.
    for op in sorted(n for n in where if n.startswith(OPERATIONS) and once(n)):
        for middle in sorted(calls[op]):
            if not once(middle) or where[middle] == where[op]:
                continue
            for leaf in sorted(calls[middle]):
                if leaf == middle or leaf in calls[op] or not once(leaf):
                    continue
                # Through exactly one of the operation's spreads, or the answer's path could go through another.
                if sum(1 for m in calls[op] if leaf in calls[m]) != 1:
                    continue
                return [
                    {"kind": "trace", "from": op, "to": leaf, "expect": "path", "via": [middle]},
                    {"kind": "trace", "from": leaf, "to": op, "expect": "none", "via": []},
                ]
    return []


def changes_case(repo: Path) -> dict | None:
    for sha in git(repo, "log", "--format=%H", "--", *(f"*{e}" for e in EXTS)).split():
        try:
            window = [f for f in git(repo, "diff", "--name-only", f"{sha}~1", "HEAD").split("\n") if f]
        except subprocess.CalledProcessError:
            return None
        if len(window) > MOST_FILES:
            return None
        if sum(1 for f in window if f.endswith(EXTS)) >= 3:
            return {"kind": "changes", "base": f"{sha}~1", "note": "the newest window holding three or more GraphQL documents and at most 40 files"}
    return None


def select(repo: Path) -> list[dict]:
    docs = documents(repo)
    declared = {rel: {n for _, n in truth.graphql_declarations(text)} for rel, text in docs.items()}
    spreads = {rel: truth.graphql_spread_calls(text) for rel, text in docs.items()}
    cases = []
    impact = impact_case(docs, declared, spreads)
    if impact:
        cases.append(impact)
    cases.extend(trace_cases(declared, spreads))
    changes = changes_case(repo)
    if changes:
        cases.append(changes)
    return cases


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", required=True)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    chosen = select(Path(args.repo).resolve())
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text("".join(json.dumps(c) + "\n" for c in chosen), encoding="utf8")
    kinds = Counter(c["kind"] for c in chosen)
    print(f"cases: impact {kinds['impact']} trace {kinds['trace']} changes {kinds['changes']}")
