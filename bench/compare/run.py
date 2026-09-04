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
import sys
import time
import tomllib
from pathlib import Path

import truth as T

NO_PATH = re.compile(r"no call path|No directed path|\"status\":\s*\"no_path\"|no path", re.I)

REPO_ROOT = Path(__file__).resolve().parent.parent.parent


def load_id_families(repograph_toml: Path = REPO_ROOT / "repograph.toml") -> list[str]:
    """The corpus's own closed list of requirement-id families, e.g. `FR-AI`, `INV`, `N`.

    A pattern built from anything looser than this list also matches `UTF-8`, `SHA-256`,
    `RFC-7807` — tokens that look like an id but never compete with one for a reader's eye.
    Failing loudly here beats falling back to that looser pattern, which was the defect.
    """
    if not repograph_toml.exists():
        raise RuntimeError(f"cannot build the id pattern: {repograph_toml} does not exist")
    with repograph_toml.open("rb") as f:
        config = tomllib.load(f)
    families = config.get("id_families")
    if not families:
        raise RuntimeError(f"cannot build the id pattern: no `id_families` key in {repograph_toml}")
    return families


def _id_token_pattern(families: list[str]) -> re.Pattern[str]:
    # Longest-first: matched literally, so `NFR` would otherwise shadow `NFR-PH` at the same
    # position (alternation takes the first branch that can match, not the longest overall).
    alts = sorted((re.escape(f) for f in families), key=len, reverse=True)
    return re.compile(rf"\b(?:{'|'.join(alts)})-[A-Z]?\d{{1,4}}\b")


# What competes with an answer for the reader's eye: another id of a real family, or for a
# file case another path. `FR-AI-138`, `INV-16`, `N-137` all match the first;
# `docs/prd/x.md` and `apps/api/src/y.ts` the second.
ID_TOKEN = _id_token_pattern(load_id_families())
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
        # Calls a tool refused to answer at all — distinct from a call it answered badly.
        # Only Gitnexus increments this; it stays 0, and thus invisible in the report, for
        # a tool whose failures don't need this filter.
        self.failed_calls = 0

    def _exec(self, argv: list[str]) -> tuple[subprocess.CompletedProcess, float]:
        started = time.perf_counter()
        proc = subprocess.run(argv, cwd=self.repo, capture_output=True, text=True, timeout=self.opts.timeout)
        ms = (time.perf_counter() - started) * 1000
        return proc, ms

    def _clean(self, out: str) -> str:
        # graphify reads the graph of a worktree, so its paths carry that directory.
        for prefix in self.opts.strip_prefix:
            out = out.replace(prefix.rstrip("/") + "/", "")
        return out

    def run(self, argv: list[str]) -> tuple[str, float]:
        proc, ms = self._exec(argv)
        return self._clean(proc.stdout + proc.stderr), ms

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


def gitnexus_looks_like_a_result(command: str, stdout: str) -> bool:
    """Whether `stdout` positively looks like a `command` result — a whitelist, not a blacklist.

    A blacklist of known error shapes fails dangerous: any failure shape it doesn't recognise
    — a renamed field, a different status, a crash that still exits 0 — falls through as an
    answer, and an echoed query field can satisfy the scorer's substring checks exactly as it
    did before (bench/results/2026-09-04-three-graphs.json's discarded predecessor run: 2
    false `trace` hits). So instead this only recognises gitnexus 1.6.9's own success shapes,
    read out of `dist/mcp/local/local-backend.js` and confirmed against a live index:
    `query` always returns a `processes` list; `trace` returns `status: "ok"` (path found,
    including from==to) or `"no_path"` — never `"not_found"`, `"ambiguous"` or `"error"`,
    which are all failures; `impact` carries `impactedCount` and omits `error` (every failure
    branch sets `error`, including the `impactedCount: 0` ones); `detect-changes` isn't JSON
    at all, just one of two plain-text openers. Anything else scores as no answer.
    """
    if command == "query":
        try:
            payload = json.loads(stdout)
        except ValueError:
            return False
        return isinstance(payload, dict) and isinstance(payload.get("processes"), list)
    if command == "trace":
        try:
            payload = json.loads(stdout)
        except ValueError:
            return False
        return isinstance(payload, dict) and payload.get("status") in ("ok", "no_path")
    if command == "impact":
        try:
            payload = json.loads(stdout)
        except ValueError:
            return False
        return isinstance(payload, dict) and "impactedCount" in payload and "error" not in payload
    if command in ("detect-changes", "detect_changes"):
        return stdout.startswith("Changes:") or stdout.startswith("No changes detected.")
    return False


def gitnexus_failure_reason(argv: list[str], proc: subprocess.CompletedProcess) -> str | None:
    """None for a genuine result; otherwise why it isn't one, for the stderr warning."""
    if proc.returncode != 0:
        return f"exit {proc.returncode}"
    command = argv[1] if len(argv) > 1 else ""
    if not gitnexus_looks_like_a_result(command, proc.stdout):
        return f"`{command}` output matched none of its known result shapes"
    return None


class Gitnexus(Tool):
    name = "gitnexus"

    def r(self) -> list[str]:
        return ["-r", self.opts.gitnexus_repo]

    def run(self, argv: list[str]) -> tuple[str, float]:
        proc, ms = self._exec(argv)
        reason = gitnexus_failure_reason(argv, proc)
        if reason:
            self.failed_calls += 1
            print(f"gitnexus: not counted as an answer, {reason}: {' '.join(argv)}", file=sys.stderr)
            return "", ms
        return self._clean(proc.stdout + proc.stderr), ms

    def ask(self, q): return self.run(["gitnexus", "query", q, *self.r()])
    def impact(self, target):
        return self.run(["gitnexus", "impact", target, "-d", "upstream", "--depth", "3", *self.r()])
    def trace(self, a, b): return self.run(["gitnexus", "trace", a, b, *self.r()])
    def changes(self, base):
        return self.run(["gitnexus", "detect-changes", "-s", "compare", "-b", base, "-l", "500", *self.r()])


TOOLS = {"repograph": Repograph, "graphify": Graphify, "gitnexus": Gitnexus}


def named(answer: str, paths: list[str]) -> list[str]:
    return [p for p in paths if p in answer]


def spells(answer: str, symbol: str) -> bool:
    """Whether `answer` names `symbol` as a whole word rather than as a run of characters.

    The changes truth reads class members, and a member name is often three or four letters:
    `has`, `key`, `now`, `code`, `next`, `parse`, `flag`, `lock`. A bare `in` test credits a
    tool for those letters landing anywhere in its output — `has` inside `hash`, `key` inside
    `keys` — which inflates the numerator exactly where the denominator grew, and the two
    errors do not cancel. `$` counts as part of a name because JavaScript allows it in one.
    """
    return re.search(rf"(?<![\w$]){re.escape(symbol)}(?![\w$])", answer) is not None


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
                "want_symbols": len(symbols), "found_symbols": sum(1 for s in symbols if spells(answer, s)),
                "ms": round(ms), "chars": len(answer.strip()),
            })
    return rows


def mrr(ranks: list[int | None]) -> float:
    """Mean of 1/rank, an absent id scored 0."""
    return round(statistics.mean(1 / r if r else 0.0 for r in ranks), 3)


def summarise(rows: list[dict]) -> dict:
    ret = [r for r in rows if r["suite"] == "retrieval"]
    out: dict = {}
    if ret:
        # `rank` present and null is a question the tool missed; `rank` absent is a result file
        # written before the field existed. Scoring the second as the first would report a
        # rescored old run as a measured zero, so it is an error rather than an absence.
        stale = [r for r in ret if "rank" not in r]
        if stale:
            raise KeyError(
                f"{len(stale)} retrieval rows carry no `rank`: this result predates the field "
                "(pre-2026-09-04) and cannot be scored for MRR"
            )
        by_kind: dict[str, dict] = {}
        for r in ret:
            slot = by_kind.setdefault(r["kind"], {"n": 0, "strict": 0, "soft": 0, "rank1": 0, "ranks": []})
            slot["n"] += 1
            slot["strict"] += r["strict"]
            slot["soft"] += r["soft"]
            slot["rank1"] += r["rank"] == 1
            slot["ranks"].append(r["rank"])
        for slot in by_kind.values():
            slot["mrr"] = mrr(slot.pop("ranks"))
        out["retrieval"] = {
            "by_kind": by_kind,
            "strict": sum(r["strict"] for r in ret), "soft": sum(r["soft"] for r in ret), "n": len(ret),
            "mrr": mrr([r["rank"] for r in ret]),
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
    # gitnexus registers a repo by a name it derives itself, not by directory basename — a
    # worktree (or any corpus dir renamed from the clone gitnexus indexed) then matches nothing.
    # The absolute path is what it actually resolves by, disambiguating even two worktrees
    # registered under the same name (verified against a live index: 2026-09-04).
    args.gitnexus_repo = args.gitnexus_repo or str(repo)

    if args.truth and Path(args.truth).exists():
        truth = json.loads(Path(args.truth).read_text(encoding="utf8"))
    else:
        truth = T.build(repo, cases, blast)
        if args.truth:
            Path(args.truth).write_text(json.dumps(truth, ensure_ascii=False, indent=1), encoding="utf8")

    head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=repo,
                          capture_output=True, text=True).stdout.strip()
    suites = args.suites.split(",")
    # Only the suites that ran. A blast-only run that still printed the retrieval case count
    # claimed 82 scored questions it never asked.
    loaded = {"retrieval": (cases, cases_path), "blast": (blast, blast_path)}
    report = {
        "corpus": str(repo), "commit": head, "when": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "suites": suites,
        "cases": {s: len(loaded[s][0]) for s in suites if s in loaded},
        "case_files": {s: str(loaded[s][1]) for s in suites if s in loaded}, "tools": {},
    }
    for name in args.tools.split(","):
        tool = TOOLS[name](repo, args)
        rows: list[dict] = []
        if "retrieval" in suites:
            rows += score_retrieval(tool, cases, truth)
        if "blast" in suites:
            rows += score_blast(tool, cases=blast, truth=truth)
        summary = summarise(rows)
        # A tool that failed outright scored 0 for a different reason than one that answered
        # badly — collapsing the two into the same 0 flatters nothing else in the comparison
        # so consistently as it flatters whichever tool failed most.
        report["tools"][name] = {"rows": rows, "summary": summary, "failed_calls": tool.failed_calls}
        suffix = f" (failed_calls={tool.failed_calls})" if tool.failed_calls else ""
        print(f"{name}: {json.dumps(summary, ensure_ascii=False)}{suffix}", flush=True)

    Path(args.out).write_text(json.dumps(report, ensure_ascii=False, indent=1), encoding="utf8")
    print(f"written: {args.out}")


if __name__ == "__main__":
    main()
