"""Cases for a private JVM corpus, chosen by rule at run time and verified by the truth reader.

The corpus is employer code, and nothing it names may enter this repository (spec §5). So its cases
are not written here: this script reads the corpus, picks candidates by the rules below, verifies
each through `truth.py`, and writes what survives to a directory outside the repository, with a
waiver line for every kind no candidate survived. It prints counts only, so its transcript can be
pasted into the results document.

usage: select_jvm_cases.py --corpus <clone> --out ~/bench/corpora-private/<name>
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import truth as T  # noqa: E402
from select_dotnet import private  # noqa: E402

JVM = (".java", ".kt")
IMPACT_MAX = 8
TRACE_MAX = 2
# The reason classes are what the results document publishes; they say why, never what.
WAIVERS = {
    "impact": "no-java-type-named-only-by-jvm-files",
    "trace": "no-injected-call-path-through-java",
    "changes": "no-window-with-a-java-declaration",
}


def git(repo: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, check=True).stdout


def read(repo: Path, rel: str) -> str:
    return (repo / rel).read_text(encoding="utf8", errors="replace")


def is_type(line: str, name: str) -> bool:
    return bool(re.search(rf"\b(?:class|interface|enum|record|object|@interface)\s+`?{re.escape(name)}`?(?!\w)", line))


def top_level_types(rel: str, src: str) -> list[str]:
    """Types the truth reads at brace depth zero: what the JVM index holds and `impact` is asked about."""
    lines, found = T.declarations(rel, src)
    depth, depths = 0, []
    for line in lines:
        depths.append(depth)
        depth += line.count("{") - line.count("}")
    return [name for line, name in found if depths[line - 1] == 0 and is_type(lines[line - 1], name)]


def store_id(rel: str, name: str) -> str:
    """The id repograph's store gives a top-level JVM type. `select_dotnet.store_id` spells the same
    form but finds the declaration with C#'s patterns, which miss `final class`, `data class` and a
    Java annotation on the declaring line; every JVM case here names a top-level type, whose local is
    its bare name."""
    return f"sym:{rel}::{name}"


def impact_cases(repo: Path, files: list[str]) -> tuple[list[dict], int]:
    """A Java type named outside its file by JVM files only: a name a TypeScript file also spells is a
    reference the truth would score and L10 forbids repograph to write."""
    candidates = []
    for rel in (f for f in files if f.endswith(".java")):
        for name in top_level_types(rel, read(repo, rel)):
            refs = [f for f in T.code_files_naming(repo, name) if f != rel]
            if refs and all(f.endswith(JVM) for f in refs):
                candidates.append((len(refs), rel, name))
    candidates.sort(key=lambda c: (-c[0], c[1], c[2]))
    cases, seen = [], set()
    for count, rel, name in candidates:
        # The truth keys `impact` by name, so a second declaration of one name would overwrite the first.
        if name in seen:
            continue
        seen.add(name)
        cases.append({"kind": "impact", "target": name, "id": store_id(rel, name), "file": rel,
                      "tier": "wide" if count >= 3 else "narrow", "lang": ".java", "exts": list(JVM)})
        if len(cases) == IMPACT_MAX:
            break
    return cases, len(candidates)


def class_span(repo: Path, rel: str, name: str) -> tuple[list[str], list[tuple[int, str]], int, int] | None:
    """The blanked lines and declarations of `rel`, and the first and last line of the type `name` there."""
    lines, found = T.declarations(rel, read(repo, rel))
    for line, declared in found:
        if declared == name and is_type(lines[line - 1], name):
            return lines, found, line, T.declaration_end(lines, line)
    return None


def member_chain(repo: Path, graph: dict, path: list[str]) -> bool:
    """Every hop of `path` is a call the code makes: the start class calls the first hop, and each
    later hop is called from inside the method the hop before it reached. The graph joins a class's
    calls across all its methods, so without this a path can pass through a method that never makes
    the next call."""
    for i in range(1, len(path)):
        target, method = path[i].rsplit(".", 1)
        caller = path[i - 1].split(".")[0]
        rel = graph["declared"].get(caller)
        span = class_span(repo, rel, caller) if rel else None
        if span is None:
            return False
        lines, found, start, end = span
        body = "\n".join(lines[start - 1 : end])
        if i == 1:
            scopes = [body]
        else:
            reached = path[i - 1].rsplit(".", 1)[1]
            scopes = ["\n".join(lines[n - 1 : T.declaration_end(lines, n)]) for n, d in found if d == reached and start < n <= end]
        if not any(reaches(rel, body, scope, target, method) for scope in scopes):
            return False
    return True


def reaches(rel: str, body: str, scope: str, target: str, method: str) -> bool:
    """Whether `scope` calls `method` on one of the class's fields typed `target`; `body` is the
    whole class, where those fields are declared, read by the truth graph's own patterns."""
    reader = T.DI_READERS[Path(rel).suffix]
    typed: dict[str, str] = {}
    for m in reader.field.finditer(body):
        typed.setdefault(m.group(1), m.group(2))
    receivers = {f for f, t in typed.items() if t == target}
    return any(m.group(1) in receivers and m.group(2) == method for m in reader.call.finditer(scope))


def trace_cases(repo: Path, files: list[str], roots: list[str]) -> tuple[list[dict], int, int, int]:
    """A call path through a Java type, longest first: a path wholly between Kotlin classes reads Kotlin.
    Its ends are top-level JVM types declared once, so each has one store id to be asked by; a field
    typed by a library class gives the graph an edge to a type the corpus never declares, and no
    tool can be asked for a path to that."""
    graph = T.di_call_graph(repo, roots)
    declared: dict[str, list[str]] = {}
    for rel in files:
        for name in top_level_types(rel, read(repo, rel)):
            declared.setdefault(name, []).append(rel)
    java_types = {n for n, rels in declared.items() if any(r.endswith(".java") for r in rels)}
    targets = sorted({edge.split(".")[0] for calls in graph["edges"].values() for edge in calls})
    found = []
    for a in sorted(graph["edges"]):
        for b in targets:
            path = T.shortest_path(graph, a, b) if a != b else None
            if not path:
                continue
            owners = [a] + [edge.split(".")[0] for edge in path[1:]]
            if any(o in java_types for o in owners):
                found.append((-len(path), a, b, path))
    found.sort()
    cases, undeclared, unmade = [], 0, 0
    for _, a, b, path in found:
        if len(cases) == TRACE_MAX:
            break
        if len(declared.get(a, ())) != 1 or len(declared.get(b, ())) != 1:
            undeclared += 1
            continue
        if not member_chain(repo, graph, path):
            unmade += 1
            continue
        cases.append({"kind": "trace", "from": a, "to": b,
                      "from_id": store_id(declared[a][0], a), "to_id": store_id(declared[b][0], b),
                      "expect": "path", "via": [edge.split(".")[0] for edge in path[1:-1]],
                      "lang": ".java", "exts": list(JVM)})
    return cases, len(found), undeclared, unmade


def changes_cases(repo: Path) -> tuple[list[dict], int]:
    """The most recent window, from a Java-touching commit's parent to the pin, holding a Java declaration."""
    windows = 0
    for line in (l for l in git(repo, "log", "--format=%H %P", "--", "*.java").split("\n") if l):
        sha, *parents = line.split()
        # A root commit has no parent to diff from.
        if not parents:
            continue
        base = f"{sha[:12]}~1"
        windows += 1
        if any(f.endswith(".java") for f in T.changed_symbols(repo, base)["symbols"]):
            return [{"kind": "changes", "base": base, "note": "selected by rule", "lang": ".java", "exts": list(JVM)}], windows
    return [], windows


def select(repo: Path, out: Path) -> dict:
    files = sorted(f for f in git(repo, "ls-files", "*.java", "*.kt").split("\n") if f)
    roots = sorted({f.split("/", 1)[0] if "/" in f else "." for f in files})
    impact, impact_candidates = impact_cases(repo, files)
    trace, trace_candidates, trace_undeclared, trace_unmade = trace_cases(repo, files, roots)
    changes, windows = changes_cases(repo)
    truth = T.build(repo, [], impact + trace + changes, roots=roots)
    kept = [c for c in impact if truth["impact"][c["target"]]["refs"]]
    kept += [c for c in trace if truth["trace"][f"{c['from']}->{c['to']}"]]
    kept += [c for c in changes if any(f.endswith(".java") for f in truth["changes"][c["base"]]["symbols"])]
    waivers = [f"{kind}\t{reason}" for kind, reason in WAIVERS.items() if not any(c["kind"] == kind for c in kept)]
    out.mkdir(parents=True, exist_ok=True)
    (out / "blast.jsonl").write_text("".join(json.dumps(c, ensure_ascii=False) + "\n" for c in kept), encoding="utf8")
    (out / "truth.json").write_text(json.dumps(truth, ensure_ascii=False, indent=1), encoding="utf8")
    (out / "waivers.txt").write_text("".join(w + "\n" for w in waivers), encoding="utf8")
    (out / "pin.txt").write_text(git(repo, "rev-parse", "HEAD"), encoding="utf8")
    return {
        "java": sum(f.endswith(".java") for f in files),
        "kt": sum(f.endswith(".kt") for f in files),
        "roots": len(roots),
        "impact_candidates": impact_candidates,
        "trace_candidates": trace_candidates,
        "trace_end_not_one_jvm_type": trace_undeclared,
        "trace_no_member_call": trace_unmade,
        "windows": windows,
        "kept": {kind: sum(c["kind"] == kind for c in kept) for kind in WAIVERS},
        "waived": [w.split("\t")[1] for w in waivers],
    }


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", required=True, help="the pinned clone, never a working checkout")
    ap.add_argument("--out", required=True, help="a directory outside this repository")
    args = ap.parse_args()
    out = Path(args.out).expanduser().resolve()
    if not private(out):
        sys.exit("select_jvm_cases: --out is inside this repository, and the cases name private code")
    print(json.dumps(select(Path(args.corpus).resolve(), out), sort_keys=True))
