"""Blast cases for C# and Razor on a private corpus, chosen by rule so that no case is picked by hand.

Writes `blast.jsonl` and `truth.json` under `--out`, a private directory, and prints counts only: the
corpus is not this repository's to quote, so neither the rule's output nor its log may name anything
in it.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import truth as T  # noqa: E402

ROOTS = ("apps", "packages", "libs-dotnet")
DOTNET = (".cs", ".razor", ".cshtml")
# Spec §5's tiers, by the files that reference a declaration.
QUOTA = {"narrow": 3, "wide": 3, "hub": 2}
RAZOR_IMPACT = 3
TRACE = {".cs": 3, ".razor": 2}
# A generated client is one package among many; more of its types than this would make it the reading.
CLIENT = "packages/crm-api-client-dotnet/"
CLIENT_MAX = 2
# The fewest files of each extension a `changes` case's diff must carry.
CHANGES = {".cs": 8, ".razor": 3}
NAMESPACE = re.compile(r"^[ \t]*@?(?:global[ \t]+)?(?:namespace|using)[ \t]+(?:static[ \t]+)?(?:\w+[ \t]*=[ \t]*)?([\w.]+)", re.M)


def order(name: str) -> str:
    """A shuffle nobody chose: the same corpus gives the same cases on every machine."""
    return hashlib.sha1(name.encode()).hexdigest()


def tier(refs: int) -> str | None:
    # One referencing file makes a case that a single miss zeroes; spec §5 wants recall over several.
    if refs >= 20:
        return "hub"
    if refs >= 6:
        return "wide"
    if refs >= 2:
        return "narrow"
    return None


def git(repo: Path, *args: str) -> str:
    return subprocess.run(["git", *args], cwd=repo, capture_output=True, text=True, check=True).stdout


def tracked(repo: Path, suffixes: tuple[str, ...]) -> list[str]:
    # A pathspec's `*` crosses `/`, so `apps/*.cs` is every C# file under `apps`.
    return sorted(git(repo, "ls-files", "--", *[f"{r}/*{s}" for r in ROOTS for s in suffixes]).split())


def read(repo: Path, rel: str) -> str:
    return (repo / rel).read_text(encoding="utf8", errors="replace")


@dataclass
class Facts:
    types: dict[str, set[str]] = field(default_factory=lambda: defaultdict(set))
    members: set[str] = field(default_factory=set)
    segments: set[str] = field(default_factory=set)
    components: dict[str, list[str]] = field(default_factory=lambda: defaultdict(list))

    def owners(self, name: str) -> set[str]:
        """The files declaring `name`, a component and its code-behind counted as one."""
        files = set(self.types.get(name, ())) | set(self.components.get(name, ()))
        return {f for f in files if not (f.endswith(".razor.cs") and f[: -len(".cs")] in files)}


def facts(repo: Path, files: list[str]) -> Facts:
    f = Facts()
    for rel in files:
        src = read(repo, rel)
        if rel.endswith(".razor") and not rel.endswith("_Imports.razor"):
            f.components[Path(rel).name[: -len(".razor")]].append(rel)
        lines, found = T.declarations(rel, src)
        for line, name in found:
            m = T.CS_TYPE.match(lines[line - 1]) or T.CS_DELEGATE.match(lines[line - 1])
            if m and m.group("name") == name:
                f.types[name].add(rel)
            else:
                f.members.add(name)
        for m in NAMESPACE.finditer(T.blanked_source(rel, src) if rel.endswith(".cs") else src):
            f.segments.update(m.group(1).split("."))
    return f


def clear(f: Facts, name: str) -> bool:
    """A name whose every spelling is the declaration: one declaring file, never a member, never a
    namespace segment. Anything else makes the word-based truth count files that name something else."""
    return len(f.owners(name)) == 1 and name not in f.members and name not in f.segments


def dotnet_refs(repo: Path, name: str, decl: str) -> list[str]:
    """The .NET files naming `name` besides its declaration. L10 forbids an edge from another
    language, so a TypeScript file spelling the same name is no reference; each case carries `exts`
    so the truth counts exactly these files too."""
    return [r for r in T.code_files_naming(repo, name) if r != decl and r.endswith(DOTNET)]


def csharp_impact(repo: Path, f: Facts, quota: dict[str, int] = QUOTA) -> list[dict]:
    cases: list[dict] = []
    filled = dict.fromkeys(quota, 0)
    client = 0
    for name in sorted((n for n in f.types if clear(f, n) and n not in f.components), key=order):
        if all(filled[k] >= quota[k] for k in quota):
            break
        (decl,) = f.owners(name)
        t = tier(len(dotnet_refs(repo, name, decl)))
        if t is None or filled[t] >= quota[t]:
            continue
        if decl.startswith(CLIENT):
            if client >= CLIENT_MAX:
                continue
            client += 1
        filled[t] += 1
        cases.append({"kind": "impact", "target": name, "file": decl, "tier": t, "lang": ".cs", "exts": list(DOTNET)})
    return cases


def razor_impact(repo: Path, f: Facts, razor: list[str], count: int = RAZOR_IMPACT) -> list[dict]:
    cases: list[dict] = []
    for name in sorted((n for n in f.components if clear(f, n)), key=order):
        if len(cases) >= count:
            break
        (decl,) = f.owners(name)
        tag = re.compile(rf"<{re.escape(name)}[\s/>]")
        # Rendered by tag in at least two other components: the reference Razor adds to C#'s.
        if sum(1 for r in razor if r != decl and tag.search(read(repo, r))) < 2:
            continue
        t = tier(len(dotnet_refs(repo, name, decl)))
        if t is not None:
            cases.append({"kind": "impact", "target": name, "file": decl, "tier": t, "lang": ".razor", "exts": list(DOTNET)})
    return cases


def traces(graph: dict, f: Facts, quota: dict[str, int] = TRACE) -> list[dict]:
    """Paths of two to four calls through injected members, one per starting class, between names
    that each have one declaring file."""
    edges = graph["edges"]
    cases: list[dict] = []
    filled = dict.fromkeys(quota, 0)
    for src in sorted(edges, key=order):
        owners = f.owners(src)
        if len(owners) != 1:
            continue
        ext = ".razor" if next(iter(owners)).endswith(".razor") else ".cs"
        if filled[ext] >= quota[ext]:
            continue
        dist, queue = {src: 0}, [src]
        while queue:
            cur = queue.pop(0)
            for edge in edges.get(cur, ()):
                owner = edge.split(".")[0]
                if owner not in dist:
                    dist[owner] = dist[cur] + 1
                    queue.append(owner)
        for dst in sorted((d for d, n in dist.items() if 2 <= n <= 4), key=order):
            path = T.shortest_path(graph, src, dst)
            if not path:
                continue
            via = [edge.split(".")[0] for edge in path[1:-1]]
            if all(len(f.owners(n)) == 1 for n in [dst, *via]):
                cases.append({"kind": "trace", "from": src, "to": dst, "expect": "path", "via": via, "lang": ext})
                filled[ext] += 1
                break
        if all(filled[k] >= quota[k] for k in quota):
            break
    return cases


def changes_base(repo: Path, suffix: str, fewest: int, limit: int = 1000) -> tuple[str, int] | None:
    """The smallest N whose `HEAD~N` diff carries at least `fewest` files of `suffix`: the resolved
    base, and that count. First parents only, so N counts the history the branch saw."""
    for n in range(1, limit + 1):
        probe = subprocess.run(["git", "rev-parse", "--verify", "-q", f"HEAD~{n}"], cwd=repo, capture_output=True, text=True)
        if probe.returncode:
            return None
        base = probe.stdout.strip()
        count = sum(1 for name in git(repo, "diff", "--name-only", base, "HEAD").split() if name.endswith(suffix))
        if count >= fewest:
            return base, count
    return None


def select(repo: Path) -> tuple[list[dict], dict]:
    code = tracked(repo, DOTNET)
    f = facts(repo, code)
    graph = T.di_call_graph(repo, [r for r in ROOTS if (repo / r).is_dir()])
    razor = [r for r in code if r.endswith(".razor")]
    cases = csharp_impact(repo, f) + razor_impact(repo, f, razor) + traces(graph, f)
    counts: dict[str, int] = defaultdict(int)
    for c in cases:
        counts[f"{c['kind']} {c['lang']}" + (f" {c['tier']}" if c["kind"] == "impact" else "")] += 1
    for suffix, fewest in CHANGES.items():
        found = changes_base(repo, suffix, fewest)
        if found:
            base, count = found
            cases.append({"kind": "changes", "base": base, "lang": suffix, "note": f"smallest N whose diff carries at least {fewest} {suffix} files"})
            counts[f"changes {suffix} files"] = count
    return cases, dict(sorted(counts.items()))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", required=True)
    ap.add_argument("--out", required=True, help="a private directory; never under this repository")
    args = ap.parse_args()
    repo, out = Path(args.repo).resolve(), Path(args.out).resolve()
    if Path(__file__).resolve().parents[2] in out.parents:
        sys.exit("select_dotnet: --out is inside this repository, and the cases name private code")
    cases, counts = select(repo)
    truth = T.build(repo, [], cases)
    empty = [c["kind"] for c in cases if (
        (c["kind"] == "impact" and not truth["impact"][c["target"]]["refs"])
        or (c["kind"] == "trace" and truth["trace"][f"{c['from']}->{c['to']}"] is None)
        or (c["kind"] == "changes" and not truth["changes"][c["base"]]["symbols"])
    )]
    out.mkdir(parents=True, exist_ok=True)
    (out / "blast.jsonl").write_text("".join(json.dumps(c, ensure_ascii=False) + "\n" for c in cases), encoding="utf8")
    (out / "truth.json").write_text(json.dumps(truth, ensure_ascii=False, indent=1), encoding="utf8")
    print(json.dumps({**counts, "di_edges": truth["di_edges"], "empty_truth": len(empty)}))
    if empty:
        sys.exit(1)


if __name__ == "__main__":
    main()
