"""Ask repograph, graphify and gitnexus the same questions and score the answers.

Two suites run against one corpus at one commit:

  retrieval  bench/cases.jsonl — a question, one expected id or symbol name
  blast      bench/blast.jsonl — impact, trace and changes, the call-graph work

Scoring never asks a tool to agree with another tool. It asks whether the answer
contains what `truth.py` read out of the repository.
"""

from __future__ import annotations

import argparse
import json
import re
import statistics
import subprocess
import time
from pathlib import Path

import truth as T

NO_PATH = re.compile(r"no call path|No directed path|\"status\":\s*\"no_path\"|no path", re.I)

# What competes with an answer for the reader's eye: another id of the same shape, or for a
# file case another path. `BE-M17`, `FR-AI-138`, `INV-16`, `N-137` all match the first;
# `docs/prd/x.md` and `apps/api/src/y.ts` the second.
ID_TOKEN = re.compile(r"\b[A-Z]{1,5}(?:-[A-Z]{1,6})?-[A-Z]?\d{1,4}\b")
PATH_TOKEN = re.compile(r"[\w./-]+/[\w.-]+\.(?:tsx?|kt|md|json|ya?ml|sql)\b")


def rank_of(answer: str, want: str) -> int | None:
    """1 + the distinct competing answers a reader passes before `want`.

    Strict is a substring test and says nothing about where in the answer the id sits — an id
    fifth of five counted the same as first on 2026-09-03. This counts the other ids (for a
    file case, the other paths) that appear before the first occurrence of `want`: first reads
    1, buried behind four neighbours reads 5. Tool-agnostic on purpose: it reads the text every
    tool prints, not a structure only one of them has.
    """
    at = answer.find(want)
    if at < 0:
        return None
    pattern = PATH_TOKEN if "/" in want else ID_TOKEN
    seen: list[str] = []
    for m in pattern.finditer(answer[:at]):
        token = m.group(0)
        if token != want and token not in seen:
            seen.append(token)
    return len(seen) + 1


class Tool:
    name = ""

    def __init__(self, repo: Path, opts: argparse.Namespace):
        self.repo = repo
        self.opts = opts

    def run(self, argv: list[str]) -> tuple[str, float]:
        started = time.perf_counter()
        proc = subprocess.run(argv, cwd=self.repo, capture_output=True, text=True, timeout=self.opts.timeout)
        ms = (time.perf_counter() - started) * 1000
        out = proc.stdout + proc.stderr
        # graphify reads the graph of a worktree, so its paths carry that directory.
        for prefix in self.opts.strip_prefix:
            out = out.replace(prefix.rstrip("/") + "/", "")
        return out, ms

    def ask(self, q: str): raise NotImplementedError
    def impact(self, target: str): return None
    def trace(self, a: str, b: str): return None
    def changes(self, base: str): return None


class Repograph(Tool):
    name = "repograph"

    def bin(self) -> list[str]:
        return [self.opts.repograph, "--repo", str(self.repo)]

    def ask(self, q): return self.run(self.bin() + ["ask", q])
    def impact(self, target): return self.run(self.bin() + ["impact", target, "--depth", "3"])
    def trace(self, a, b): return self.run(self.bin() + ["trace", a, b])
    def changes(self, base): return self.run(self.bin() + ["changes", "--base", base])


class Graphify(Tool):
    name = "graphify"

    def g(self) -> list[str]:
        return ["--graph", self.opts.graphify_graph]

    def ask(self, q): return self.run(["graphify", "query", q, *self.g()])
    def impact(self, target): return self.run(["graphify", "affected", target, "--depth", "3", *self.g()])
    def trace(self, a, b): return self.run(["graphify", "path", a, b, *self.g()])
    # No git entry point: graphify has no command that reads a diff.
    def changes(self, base): return None


class Gitnexus(Tool):
    name = "gitnexus"

    def r(self) -> list[str]:
        return ["-r", self.opts.gitnexus_repo]

    def ask(self, q): return self.run(["gitnexus", "query", q, *self.r()])
    def impact(self, target):
        return self.run(["gitnexus", "impact", target, "-d", "upstream", "--depth", "3", *self.r()])
    def trace(self, a, b): return self.run(["gitnexus", "trace", a, b, *self.r()])
    def changes(self, base):
        return self.run(["gitnexus", "detect-changes", "-s", "compare", "-b", base, "-l", "500", *self.r()])


TOOLS = {"repograph": Repograph, "graphify": Graphify, "gitnexus": Gitnexus}


def named(answer: str, paths: list[str]) -> list[str]:
    return [p for p in paths if p in answer]


def score_retrieval(tool: Tool, cases: list[dict], truth: dict) -> list[dict]:
    rows = []
    for case in cases:
        answer, ms = tool.ask(case["q"])
        want = case["expect"]
        files = truth["retrieval"].get(want, [])
        rows.append({
            "suite": "retrieval", "kind": case["kind"], "q": case["q"], "expect": want,
            "strict": want in answer, "soft": bool(named(answer, files)),
            "rank": rank_of(answer, want),
            "ms": round(ms), "chars": len(answer.strip()),
        })
    return rows


def score_blast(tool: Tool, cases: list[dict], truth: dict) -> list[dict]:
    rows = []
    for case in cases:
        kind = case["kind"]
        if kind == "impact":
            got = tool.impact(case["target"])
            if got is None:
                continue
            answer, ms = got
            want = truth["impact"][case["target"]]["refs"]
            found = named(answer, want)
            rows.append({
                "suite": "blast", "kind": "impact", "target": case["target"], "tier": case["tier"],
                "want_files": len(want), "found_files": len(found),
                "recall": round(len(found) / len(want), 3) if want else None,
                "ms": round(ms), "chars": len(answer.strip()),
            })
        elif kind == "trace":
            got = tool.trace(case["from"], case["to"])
            if got is None:
                continue
            answer, ms = got
            says_none = bool(NO_PATH.search(answer))
            if case["expect"] == "none":
                hit = says_none
            else:
                hit = not says_none and all(v in answer for v in case["via"]) and case["to"] in answer
            rows.append({
                "suite": "blast", "kind": "trace", "from": case["from"], "to": case["to"],
                "expect": case["expect"], "hit": hit, "says_none": says_none,
                "ms": round(ms), "chars": len(answer.strip()),
            })
        elif kind == "changes":
            got = tool.changes(case["base"])
            if got is None:
                continue
            answer, ms = got
            want = truth["changes"][case["base"]]
            symbols = sorted({s for names in want["symbols"].values() for s in names})
            files = want["code_files"]
            rows.append({
                "suite": "blast", "kind": "changes", "base": case["base"],
                "want_files": len(files), "found_files": len(named(answer, files)),
                "want_symbols": len(symbols), "found_symbols": sum(1 for s in symbols if s in answer),
                "ms": round(ms), "chars": len(answer.strip()),
            })
    return rows


def summarise(rows: list[dict]) -> dict:
    ret = [r for r in rows if r["suite"] == "retrieval"]
    out: dict = {}
    if ret:
        by_kind: dict[str, dict] = {}
        for r in ret:
            slot = by_kind.setdefault(r["kind"], {"n": 0, "strict": 0, "soft": 0})
            slot["n"] += 1
            slot["strict"] += r["strict"]
            slot["soft"] += r["soft"]
        ranks = [r.get("rank") for r in ret]
        out["retrieval"] = {
            "by_kind": by_kind,
            "strict": sum(r["strict"] for r in ret), "soft": sum(r["soft"] for r in ret), "n": len(ret),
            "mrr": round(statistics.mean(1 / r if r else 0.0 for r in ranks), 3),
            "ms_median": round(statistics.median(r["ms"] for r in ret)),
            "chars_median": round(statistics.median(r["chars"] for r in ret)),
        }
    imp = [r for r in rows if r.get("kind") == "impact"]
    if imp:
        out["impact"] = {
            "n": len(imp),
            "recall_mean": round(statistics.mean(r["recall"] for r in imp if r["recall"] is not None), 3),
            "files_found": sum(r["found_files"] for r in imp), "files_want": sum(r["want_files"] for r in imp),
            "ms_median": round(statistics.median(r["ms"] for r in imp)),
            "chars_median": round(statistics.median(r["chars"] for r in imp)),
        }
    tr = [r for r in rows if r.get("kind") == "trace"]
    if tr:
        out["trace"] = {
            "n": len(tr), "hit": sum(r["hit"] for r in tr),
            "path_hit": sum(r["hit"] for r in tr if r["expect"] == "path"),
            "path_n": sum(1 for r in tr if r["expect"] == "path"),
            "none_hit": sum(r["hit"] for r in tr if r["expect"] == "none"),
            "none_n": sum(1 for r in tr if r["expect"] == "none"),
            "ms_median": round(statistics.median(r["ms"] for r in tr)),
        }
    ch = [r for r in rows if r.get("kind") == "changes"]
    if ch:
        out["changes"] = {
            "n": len(ch),
            "symbols_found": sum(r["found_symbols"] for r in ch), "symbols_want": sum(r["want_symbols"] for r in ch),
            "files_found": sum(r["found_files"] for r in ch), "files_want": sum(r["want_files"] for r in ch),
            "ms_median": round(statistics.median(r["ms"] for r in ch)),
            "chars_median": round(statistics.median(r["chars"] for r in ch)),
        }
    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", required=True, help="corpus root")
    ap.add_argument("--cases", default="", help="retrieval cases; default bench/cases.jsonl")
    ap.add_argument("--blast", default="", help="blast cases; default bench/blast.jsonl")
    ap.add_argument("--tools", default="repograph,graphify,gitnexus")
    ap.add_argument("--suites", default="retrieval,blast")
    ap.add_argument("--repograph", default="repograph", help="repograph binary")
    ap.add_argument("--graphify-graph", default="graphify-out/graph.json")
    ap.add_argument("--gitnexus-repo", default="")
    ap.add_argument("--strip-prefix", action="append", default=[])
    ap.add_argument("--timeout", type=int, default=180)
    ap.add_argument("--truth", default="", help="reuse a truth file instead of rebuilding it")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    repo = Path(args.repo).resolve()
    bench = Path(__file__).resolve().parent.parent
    cases_path = Path(args.cases).resolve() if args.cases else bench / "cases.jsonl"
    blast_path = Path(args.blast).resolve() if args.blast else bench / "blast.jsonl"
    cases = T.read_jsonl(cases_path)
    blast = T.read_jsonl(blast_path)
    args.gitnexus_repo = args.gitnexus_repo or repo.name

    if args.truth and Path(args.truth).exists():
        truth = json.loads(Path(args.truth).read_text(encoding="utf8"))
    else:
        truth = T.build(repo, cases, blast)
        if args.truth:
            Path(args.truth).write_text(json.dumps(truth, ensure_ascii=False, indent=1), encoding="utf8")

    head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=repo,
                          capture_output=True, text=True).stdout.strip()
    report = {
        "corpus": str(repo), "commit": head, "when": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "cases": {"retrieval": len(cases), "blast": len(blast)},
        "case_files": {"retrieval": str(cases_path), "blast": str(blast_path)}, "tools": {},
    }
    suites = args.suites.split(",")
    for name in args.tools.split(","):
        tool = TOOLS[name](repo, args)
        rows: list[dict] = []
        if "retrieval" in suites:
            rows += score_retrieval(tool, cases, truth)
        if "blast" in suites:
            rows += score_blast(tool, cases=blast, truth=truth)
        report["tools"][name] = {"rows": rows, "summary": summarise(rows)}
        print(f"{name}: {json.dumps(summarise(rows), ensure_ascii=False)}", flush=True)

    Path(args.out).write_text(json.dumps(report, ensure_ascii=False, indent=1), encoding="utf8")
    print(f"written: {args.out}")


if __name__ == "__main__":
    main()
