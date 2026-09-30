"""Blast cases for a corpus whose code may not be named in this repository, chosen by a rule.

The rule is what is committed; the cases it picks are written wherever `--out` points, which for
an employer's corpus is outside this repository. Nothing printed here names a file, a symbol or
a commit — counts only — so a transcript of a run can be pasted anywhere.
"""

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import truth as T  # noqa: E402

WIDE = 5


def visible(repo: Path, ext: str) -> list[str]:
    # `truth.rg` walks dotted paths, as `build` does; the resolver's walk does not, so an import
    # inside a dotted directory binds nothing. Only undotted files make cases.
    files = T.rg(repo, ["--files", "-g", f"*{ext}"])
    return sorted(f for f in files if not any(p.startswith(".") for p in f.split("/")))


def dotted_naming(repo: Path, name: str, decl: str) -> bool:
    # A name a dotted file spells would be scored against refs no edge can reach: the resolver
    # binds no import inside a dotted directory.
    spelled = T.rg(repo, ["--hidden", "-g", "!.git/**", "-l", "--word-regexp", "--fixed-strings", name])
    return any(f != decl and any(p.startswith(".") for p in f.split("/")) for f in spelled)


def imports_module(src: str, module: str) -> bool:
    stem = re.escape(module)
    return bool(re.search(rf"^[ \t]*(?:from[ \t]+\.*(?:[\w.]*\.)?{stem}[ \t]+import\b|import[ \t]+(?:[\w.]*\.)?{stem}\b)", src, re.M))


def top_level(repo: Path, files: list[str]) -> dict[str, list[str]]:
    declared: dict[str, list[str]] = {}
    for rel in files:
        lines, found = T.declarations(rel, (repo / rel).read_text(encoding="utf8", errors="replace"))
        for line, name in found:
            if not lines[line - 1].startswith((" ", "\t")):
                declared.setdefault(name, []).append(rel)
    return declared


def impact_cases(repo: Path, ext: str, files: list[str]) -> list[dict]:
    declared = top_level(repo, files)
    candidates = []
    for name, where in declared.items():
        if len(where) != 1 or name.startswith(("_", "test")):
            continue
        decl = where[0]
        refs = [f for f in T.code_files_naming(repo, name) if f != decl]
        if not refs or any(not f.endswith(ext) or f not in files for f in refs) or dotted_naming(repo, name, decl):
            continue
        module = Path(decl).stem
        if not all(imports_module((repo / f).read_text(encoding="utf8", errors="replace"), module) for f in refs):
            continue
        candidates.append((len(refs), name, decl))
    candidates.sort(key=lambda c: (-c[0], c[1]))
    wide = [c for c in candidates if c[0] >= WIDE][:1]
    narrow = [c for c in candidates if c[0] < WIDE][:2]
    return [{"kind": "impact", "target": n, "id": f"sym:{d}::{n}", "file": d, "tier": "wide" if k >= WIDE else "narrow", "exts": [ext]} for k, n, d in wide + narrow]


def trace_case(graph: dict, src: str, dst: str, expect: str) -> dict:
    ids = {name: f"sym:{graph['declared'][name]}::{name}" for name in (src, dst)}
    return {"kind": "trace", "from": src, "to": dst, "from_id": ids[src], "to_id": ids[dst], "expect": expect, "via": []}


def trace_cases(repo: Path, files: list[str]) -> tuple[list[dict], list[str]]:
    graph = T.di_call_graph(repo, files)
    # `di_call_graph` keeps one file per name, so a name two files declare would get the other
    # file's id: a wrong case is worse than none.
    unique = {name for name, where in top_level(repo, files).items() if len(where) == 1}
    for src in sorted(graph["edges"]):
        if src not in unique:
            continue
        for dst in sorted({e.split(".")[0] for e in graph["edges"][src]} - {src}):
            if dst not in graph["declared"] or dst not in unique or T.shortest_path(graph, src, dst) is None:
                continue
            cases = [trace_case(graph, src, dst, "path")]
            if T.shortest_path(graph, dst, src) is None:
                cases.append(trace_case(graph, dst, src, "none"))
            return cases, []
    edges = sum(len(v) for v in graph["edges"].values())
    return [], [f"trace: no class calls another through a typed field in the walk-visible files (DI edges {edges})"]


def changes_cases(repo: Path, ext: str, files: list[str]) -> tuple[list[dict], list[str]]:
    # With no file to name, `git log --` would read the whole history.
    if not files:
        return [], [f"changes: no walk-visible {ext} file"]
    log = subprocess.run(["git", "log", "--format=%h %p", "--", *files], cwd=repo, capture_output=True, text=True).stdout
    for line in log.splitlines():
        commit, *parents = line.split()
        if not parents:
            continue
        got = T.changed_symbols(repo, f"{commit}~1")
        if any(rel.endswith(ext) for rel in got["symbols"]):
            return [{"kind": "changes", "base": f"{commit}~1"}], []
    return [], [f"changes: no commit touching a walk-visible {ext} file credits a symbol"]


def select(repo: Path, ext: str) -> tuple[list[dict], list[str]]:
    files = visible(repo, ext)
    impact = impact_cases(repo, ext, files)
    waivers = [] if impact else [f"impact: no {ext} name is referenced only by files that import its module"]
    trace, trace_waivers = trace_cases(repo, files)
    changes, changes_waivers = changes_cases(repo, ext, files)
    return impact + trace + changes, waivers + trace_waivers + changes_waivers


def main(argv: list[str] | None = None) -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--repo", required=True)
    ap.add_argument("--ext", required=True, help="the extension the cases are for, with its dot")
    ap.add_argument("--out", required=True, help="the case file to write")
    ap.add_argument("--waivers", required=True, help="the file each waiver's reason is written to")
    args = ap.parse_args(argv)
    cases, waivers = select(Path(args.repo), args.ext)
    out, waiver_file = Path(args.out).expanduser(), Path(args.waivers).expanduser()
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text("".join(json.dumps(c, ensure_ascii=False) + "\n" for c in cases), encoding="utf8")
    waiver_file.parent.mkdir(parents=True, exist_ok=True)
    waiver_file.write_text("".join(f"{args.ext} {w}\n" for w in waivers), encoding="utf8")
    impact = [c for c in cases if c["kind"] == "impact"]
    wide = sum(1 for c in impact if c["tier"] == "wide")
    print(
        f"impact {len(impact)} (wide {wide}, narrow {len(impact) - wide}) · "
        f"trace {sum(1 for c in cases if c['kind'] == 'trace')} · "
        f"changes {sum(1 for c in cases if c['kind'] == 'changes')} · waivers {len(waivers)}"
    )


if __name__ == "__main__":
    main(sys.argv[1:])
