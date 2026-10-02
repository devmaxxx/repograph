"""Cases for the private Vue corpus, chosen by rule at run time and verified by the truth reader.

The corpus is employer code, and nothing it names may enter this repository (spec §5). So its cases
are not written here: this script reads the pinned clone, picks candidates by the rules below,
verifies each through `truth.py`, and writes what survives to a directory outside the repository,
with a waiver line for every kind no candidate survived. It prints counts only, so its transcript
can be pasted into the results document.

usage: select_vue_cases.py --corpus <clone> --out ~/bench/corpora-private/customer-portal
"""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

import truth as T

# L10: a `.vue` file is in the TypeScript family, JavaScript included since 0.5.3 (#64), so only these files may hold a reference repograph writes.
FAMILY = (".vue", ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs")
IMPACT_MAX = 8
TRACE_MAX = 2
# The reason classes are what the results document publishes; they say why, never what.
WAIVERS = {
    "impact": "no-component-named-outside-its-file",
    "trace": "no-injected-call-path-through-a-component",
    "changes": "no-window-with-a-component-change",
}


def git(repo: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, check=True).stdout


def read(repo: Path, rel: str) -> str:
    return (repo / rel).read_text(encoding="utf8", errors="replace")


def impact_cases(repo: Path, files: list[str]) -> tuple[list[dict], int]:
    """A component named outside its file by files of its family only, most-named first."""
    stems: dict[str, list[str]] = {}
    for rel in (f for f in files if f.endswith(".vue")):
        stems.setdefault(Path(rel).stem, []).append(rel)
    # The truth keys `impact` by name and `query` resolves a bare name to one id, so a stem two
    # components share, or a TypeScript file also declares, would ask about two symbols at once.
    script_names = {name for rel in files if not rel.endswith(".vue") for _, name in T.declarations(rel, read(repo, rel))[1]}
    candidates = []
    for name, rels in stems.items():
        if len(rels) != 1 or name in script_names:
            continue
        refs = [f for f in T.code_files_naming(repo, name) if f != rels[0]]
        if refs and all(f.endswith(FAMILY) for f in refs):
            candidates.append((len(refs), rels[0], name))
    candidates.sort(key=lambda c: (-c[0], c[1], c[2]))
    cases = [
        {"kind": "impact", "target": name, "file": rel, "tier": "wide" if count >= 3 else "narrow"}
        for count, rel, name in candidates[:IMPACT_MAX]
    ]
    return cases, len(candidates)


def trace_cases(repo: Path, roots: list[str]) -> tuple[list[dict], int]:
    """A call path with a component at its start, its end or a hop, longest first: a path wholly
    between TypeScript classes reads TypeScript, which the TypeScript suites already measure."""
    graph = T.di_call_graph(repo, roots)
    in_components = {name for name, rel in graph["declared"].items() if rel.endswith(".vue")}
    targets = sorted({edge.split(".")[0] for calls in graph["edges"].values() for edge in calls})
    found = []
    for a in sorted(graph["edges"]):
        for b in targets:
            path = T.shortest_path(graph, a, b) if a != b else None
            if not path:
                continue
            owners = [a] + [edge.split(".")[0] for edge in path[1:]]
            if any(o in in_components for o in owners):
                found.append((-len(path), a, b, path))
    found.sort()
    cases = [
        {"kind": "trace", "from": a, "to": b, "expect": "path", "via": [edge.split(".")[0] for edge in path[1:-1]]}
        for _, a, b, path in found[:TRACE_MAX]
    ]
    return cases, len(found)


def changes_cases(repo: Path) -> tuple[list[dict], int]:
    """The most recent window, from a component-touching commit's parent to the pin, whose truth
    still names a component symbol."""
    windows = 0
    for line in (l for l in git(repo, "log", "--format=%H %P", "--", "*.vue").split("\n") if l):
        sha, *parents = line.split()
        # A root commit has no parent to diff from.
        if not parents:
            continue
        base = f"{sha[:12]}~1"
        windows += 1
        if any(f.endswith(".vue") for f in T.changed_symbols(repo, base)["symbols"]):
            return [{"kind": "changes", "base": base, "note": "selected by rule"}], windows
    return [], windows


def private_names(repo: Path, files: list[str], kept: list[dict]) -> list[str]:
    """What the leak check looks for: every component's name, every class a script declares and every
    name a case carries. Members are left out: `data` and `mounted` are words this repository uses."""
    names = {Path(rel).stem for rel in files if rel.endswith(".vue")}
    for rel in (f for f in files if f.endswith(".vue")):
        names.update(m.group(1) for m in T.VUE_CLASS.finditer(read(repo, rel)))
    for case in kept:
        names.update(case[key] for key in ("target", "from", "to") if key in case)
        names.update(case.get("via", []))
    return sorted(names)


def select(repo: Path, out: Path) -> dict:
    files = sorted(f for f in git(repo, "ls-files", *(f"*{ext}" for ext in FAMILY)).split("\n") if f)
    roots = sorted({f.split("/", 1)[0] if "/" in f else "." for f in files})
    impact, impact_candidates = impact_cases(repo, files)
    trace, trace_candidates = trace_cases(repo, roots)
    changes, windows = changes_cases(repo)
    truth = T.build(repo, [], impact + trace + changes, roots=roots)
    kept = [c for c in impact if truth["impact"][c["target"]]["refs"]]
    kept += [c for c in trace if truth["trace"][f"{c['from']}->{c['to']}"]]
    kept += [c for c in changes if any(f.endswith(".vue") for f in truth["changes"][c["base"]]["symbols"])]
    waivers = [f"{kind}\t{reason}" for kind, reason in WAIVERS.items() if not any(c["kind"] == kind for c in kept)]
    out.mkdir(parents=True, exist_ok=True)
    (out / "blast.jsonl").write_text("".join(json.dumps(c, ensure_ascii=False) + "\n" for c in kept), encoding="utf8")
    (out / "truth.json").write_text(json.dumps(truth, ensure_ascii=False, indent=1), encoding="utf8")
    (out / "waivers.txt").write_text("".join(w + "\n" for w in waivers), encoding="utf8")
    (out / "pin.txt").write_text(git(repo, "rev-parse", "HEAD"), encoding="utf8")
    (out / "names.txt").write_text("".join(n + "\n" for n in private_names(repo, files, kept)), encoding="utf8")
    return {
        "vue": sum(f.endswith(".vue") for f in files),
        "scripts": sum(len(T.vue_script_ranges(read(repo, f))) for f in files if f.endswith(".vue")),
        "ts": sum(not f.endswith(".vue") for f in files),
        "roots": len(roots),
        "impact_candidates": impact_candidates,
        "trace_candidates": trace_candidates,
        "windows": windows,
        "kept": {kind: sum(c["kind"] == kind for c in kept) for kind in WAIVERS},
        "waived": [w.split("\t")[1] for w in waivers],
    }


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", required=True, help="the pinned clone, never a working checkout")
    ap.add_argument("--out", required=True, help="a directory outside this repository")
    args = ap.parse_args()
    print(json.dumps(select(Path(args.corpus).resolve(), Path(args.out).expanduser()), sort_keys=True))
