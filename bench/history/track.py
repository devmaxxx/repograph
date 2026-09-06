#!/usr/bin/env python3
"""Append-only history of benchmark runs, and the report that reads it.

A single run tells you whether the bar was cleared. It cannot tell you that a case has been
missing for six runs, that another flips on every second run, or that the arm you just fixed
was already green last week for an unrelated reason. Those three questions -- what is
chronically weak, what is fragile, what actually moved -- need the previous runs kept, so
this keeps them.

Rows are appended to `runs.jsonl`, one per run per arm, and never rewritten: a benchmark
history that can be edited in place is a benchmark history that will be.
"""

import argparse
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
RUNS = HERE / "runs.jsonl"


def repo_root(start=HERE):
    for d in [start, *start.parents]:
        if (d / "Cargo.toml").exists():
            return d
    return start.parent.parent


REPO = repo_root()

# `{:<10} {:<12} {} {reached}/{want} {:>4} tok  {}` from src/bench.rs. Anchors never contain
# spaces, so the question is whatever follows the token count. The `reached/want` pair is
# absent from transcripts older than the dev suite, which expected one place per case.
CASE = re.compile(r"^(\S+)\s+(\S+)\s+(HIT|miss)(?:\s+(\d+)/(\d+))?\s+(\d+) tok  (.*)$")
# One `kind hits/cases` pair per kind the case file named, in the order it named them, then
# the fixed tail. Older transcripts end at `nodes)`; newer ones add `suite=… gated=…`.
SUMMARY = re.compile(
    r"^((?:\S+ \d+/\d+\s+)+)p90 (\d+) tok\s+dense=(true|false)\s+enriched=(true|false) "
    r"\((\d+)/(\d+) nodes\)(.*)$"
)
KIND = re.compile(r"(\S+) (\d+)/(\d+)")
RERANK = re.compile(r"rerank(_local)?=true depth=(\d+)")
SUITE = re.compile(r"suite=(\S+) gated=(true|false)")
MODEL = re.compile(r"model=(\S+)")
GRADED = ("keyword", "paraphrase", "code")

# One row per line of `FLOORS` in src/bench.rs: (enriched, dense, Floors::<Model>, keyword, paraphrase).
ROW = re.compile(r"\((true|false),\s*(true|false),\s*Floors::(Small|Large),\s*(\d+),\s*(\d+)\)")
TAIL = re.compile(r"s\.kind\(\"code\"\)\.0 >= (\d+) && s\.p90_tokens <= (\d+)")


def passes_body(text):
    """The braces-matched body of `passes`, or None.

    Matching braces rather than looking for a closing one at column zero, so that moving the
    function into an `impl` cannot silently extend the body to the end of that block and let
    a sibling function supply the numbers.
    """
    m = re.search(r"\bfn passes\b", text)
    if not m:
        return None
    try:
        start = text.index("{", m.end())
    except ValueError:
        return None
    depth = 0
    for i in range(start, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return text[start + 1:i]
    return None


def floors(source=None):
    """Every (enriched, dense, model) arm's floors plus the code floor and token ceiling,
    read from `FLOORS` and `passes` in src/bench.rs rather than restated here."""
    text = (source or REPO / "src" / "bench.rs").read_text()
    body = passes_body(text)
    body = re.sub(r"//[^\n]*|/\*.*?\*/", "", body, flags=re.S) if body else ""
    tail = TAIL.search(body)
    table = re.search(r"const FLOORS[^=]*=\s*\[(.*?)\];", text, re.S)
    rows = ROW.findall(table.group(1)) if table else []
    if not body or not tail or len(rows) < 6:
        raise SystemExit("cannot read the floors out of src/bench.rs -- `FLOORS` or `passes` changed shape")
    code, p90 = int(tail.group(1)), int(tail.group(2))
    out = {}
    for enriched, dense, model, keyword, paraphrase in rows:
        out[(enriched == "true", dense == "true", model.lower())] = {"keyword": int(keyword), "paraphrase": int(paraphrase), "code": code, "p90_tokens": p90}
    return out


def git(repo, *args):
    try:
        out = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, timeout=30)
        return out.stdout.strip() if out.returncode == 0 else None
    except (OSError, subprocess.SubprocessError):
        return None


def parse_bench(text):
    """A `repograph bench` transcript into per-case scores and the summary metrics.

    A case's score is the share of its anchors the answer reached, so a case that expects three
    places and is pointed at one scores 0.33 -- a partial answer, which the chronic and flaky
    readings treat as a miss. The summary's per-kind counts stay what `bench` printed: cases
    with at least one anchor reached, the developer's entry point.
    """
    cases, tokens = {}, {}
    for line in text.splitlines():
        m = CASE.match(line.rstrip())
        if m:
            kind, expect, verdict, reached, want, tok, _q = m.groups()
            key = f"{kind}/{expect}"
            # Two questions of one kind may be pointed at the same place -- the dev suite asks
            # two `rule` questions about ADR-031. Keyed by kind and anchor alone the later one
            # overwrites the earlier, and the run is recorded holding fewer scores than the
            # suite has cases, so a case that is always missed never reads as chronically weak.
            if key in cases:
                nth = 2
                while f"{key}#{nth}" in cases:
                    nth += 1
                key = f"{key}#{nth}"
            if want:
                cases[key] = round(int(reached) / int(want), 4) if int(want) else 0.0
            else:
                cases[key] = 1.0 if verdict == "HIT" else 0.0
            tokens[key] = int(tok)
    tail = [SUMMARY.match(l.strip()) for l in text.splitlines()]
    tail = [m for m in tail if m]
    if not tail:
        raise SystemExit("no summary line in the transcript -- did the run reach the end?")
    g = tail[-1]
    dense, enriched = g.group(3) == "true", g.group(4) == "true"
    rr = RERANK.search(g.group(7) or "")
    suite = SUITE.search(g.group(7) or "")
    model = MODEL.search(g.group(7) or "")
    metrics = {kind: [int(h), int(n)] for kind, h, n in KIND.findall(g.group(1))}
    metrics["p90_tokens"] = int(g.group(2))
    return {
        "dense": dense,
        "enriched": enriched,
        "coverage": [int(g.group(5)), int(g.group(6))],
        "rerank": ("local" if rr.group(1) else "command") if rr else None,
        "depth": int(rr.group(2)) if rr else None,
        "metrics": metrics,
        # A transcript from before the suite field is the recorded suite, which was the only
        # one `bench` would run, and it was graded whenever it had the three graded kinds.
        "suite": suite.group(1) if suite else "built-in",
        "gated": (suite.group(2) == "true") if suite else all(k in metrics for k in GRADED),
        # A transcript from before the model field is a small-model run, the only kind there was.
        "model": model.group(1) if model else "small",
        "cases": cases,
        "tokens": tokens,
    }


def arm_name(parsed):
    """`bench:dense+enriched` for the recorded suite; another suite names itself in brackets,
    so its runs never share a history -- or a comparability window -- with the recorded one.
    A dense arm under a model other than the small default names itself too (`+large`, or the
    model string for one with no floors), so its history never pools with the small model's."""
    parts = ["dense" if parsed["dense"] else "lexical", "enriched" if parsed["enriched"] else "raw"]
    if parsed["dense"] and parsed["model"] != "small":
        parts.append(parsed["model"])
    if parsed["rerank"]:
        parts.append(f"rerank-{parsed['rerank']}-{parsed['depth']}")
    suite = parsed.get("suite") or "built-in"
    prefix = "bench" if suite == "built-in" else f"bench[{suite}]"
    return prefix + ":" + "+".join(parts)


def headroom(metrics, floor):
    """Distance from each floor. Negative means the arm is under its bar."""
    return {
        "keyword": metrics["keyword"][0] - floor["keyword"],
        "paraphrase": metrics["paraphrase"][0] - floor["paraphrase"],
        "code": metrics["code"][0] - floor["code"],
        "p90_tokens": floor["p90_tokens"] - metrics["p90_tokens"],
    }


def tool_dirty(repo=None):
    """Whether repograph's own tree differs from its commit, the run file itself excepted.

    Recording the first arm of a run appends to `runs.jsonl`, so without the exception the
    second arm of the same run would always read dirty against the same commit and the same
    second -- which is what the two 2026-09-04 enriched rows show.
    """
    return bool(git(repo or REPO, "status", "--porcelain", "--", f":(exclude){RUNS.relative_to(REPO)}"))


def build_row(parsed, corpus, corpus_commit, note, tool_commit, dirty, floor_table=None):
    """One history row. Floors and headroom exist only for a graded run: a suite without floors
    of its own is measured, and a `green` it never earned would read as a claim. The model that
    wrote a dense arm's rows decides which floors it reads; a model not in the table (this
    transcript's `gated` already says so, but the lookup checks it again rather than trust it)
    is measured and never graded."""
    model_key = parsed["model"] if parsed["dense"] else "small"
    found = (floor_table or floors()).get((parsed["enriched"], parsed["dense"], model_key))
    gated = parsed["gated"] and found is not None
    floor = found if gated else None
    room = headroom(parsed["metrics"], floor) if gated else None
    return {
        "when": datetime.now(timezone.utc).replace(microsecond=0).isoformat(),
        "tool": "repograph",
        "source": "bench",
        "arm": arm_name(parsed),
        "suite": parsed["suite"],
        "gated": gated,
        "tool_commit": tool_commit,
        "tool_dirty": dirty,
        "corpus": corpus,
        "corpus_commit": corpus_commit,
        "coverage": parsed["coverage"],
        "metrics": parsed["metrics"],
        "floors": floor,
        "headroom": room,
        "green": all(v >= 0 for v in room.values()) if gated else None,
        "cases": parsed["cases"],
        "worst_tokens": sorted(parsed["tokens"].items(), key=lambda kv: -kv[1])[:3],
        "note": note,
    }


def state_of(row):
    if row.get("gated") is False:
        return "measured, no floors"
    return "green" if row.get("green") else "RED"


def counts_of(metrics):
    return "  ".join(f"{k} {v[0]}/{v[1]}" if isinstance(v, list) else f"{k} {v}" for k, v in metrics.items())


def cmd_record(args):
    text = Path(args.transcript).read_text() if args.transcript != "-" else sys.stdin.read()
    parsed = parse_bench(text)
    row = build_row(
        parsed, args.corpus,
        git(args.corpus_path, "rev-parse", "--short", "HEAD") if args.corpus_path else None,
        args.note, git(REPO, "rev-parse", "--short", "HEAD"), tool_dirty(),
    )
    append(row)
    print(f"{row['arm']}  {state_of(row)}  {counts_of(row['metrics'])}  -> {RUNS.name}")


def scored(row):
    """One comparison-harness row as (case key, score in 0..1).

    The four suites answer in four shapes and each needs its own reading; an unrecognised
    kind raises rather than scoring zero, because a silent zero would enter the history as a
    regression that never happened.
    """
    kind = row["kind"]
    if kind in ("keyword", "paraphrase", "code"):
        return f"{kind}/{row['expect']}", 1.0 if row["strict"] else 0.0
    if kind == "impact":
        return f"impact/{row['target']}", round(float(row["recall"]), 4)
    if kind == "trace":
        return f"trace/{row['from']}->{row['to']}", 1.0 if row["hit"] else 0.0
    if kind == "changes":
        want = row["want_symbols"]
        return f"changes/{row['base']}", round(row["found_symbols"] / want, 4) if want else 1.0
    raise SystemExit(f"unrecognised row kind {kind!r} in the result file")


def cmd_import(args):
    """One row per tool from a `bench/compare/run.py` result file."""
    d = json.loads(Path(args.result).read_text())
    for tool, block in d["tools"].items():
        cases = dict(scored(r) for r in block["rows"])
        append({
            "when": d.get("when") or datetime.now(timezone.utc).replace(microsecond=0).isoformat(),
            "tool": tool,
            "source": "compare",
            "arm": f"compare:{tool}",
            "tool_commit": None,
            "corpus": args.corpus,
            "corpus_commit": d.get("commit"),
            "metrics": {},
            "extra": block["summary"],
            "cases": cases,
            "note": args.note or f"imported from {Path(args.result).name}",
        })
    print(f"imported {len(d['tools'])} tools from {Path(args.result).name} -> {RUNS.name}")


def append(row):
    with RUNS.open("a") as f:
        f.write(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n")


def when_of(row):
    """A row's timestamp as an aware datetime, for ordering.

    Imported comparison rows carry a naive stamp and recorded bench rows an aware one, so a
    string sort would interleave them wrongly and a naive/aware comparison would raise.
    """
    try:
        t = datetime.fromisoformat(row["when"])
    except (KeyError, TypeError, ValueError) as e:
        raise SystemExit(f"{RUNS}: a row has an unreadable `when` ({row.get('when')!r}): {e}")
    return t if t.tzinfo else t.replace(tzinfo=timezone.utc)


def load():
    if not RUNS.exists():
        return []
    rows = [json.loads(l) for l in RUNS.read_text().splitlines() if l.strip()]
    # File order is append order until a union merge interleaves two branches' runs, after
    # which "the latest row" would be whichever branch's line landed last -- and improved and
    # REGRESSED would read backwards. Order by the stamp the row carries instead.
    return sorted(rows, key=when_of)


def by_arm(rows):
    out = {}
    for r in rows:
        out.setdefault(r["arm"], []).append(r)
    return out


def cmd_report(args):
    rows = load()
    if not rows:
        raise SystemExit(f"{RUNS} is empty -- record a run first")
    groups = by_arm(rows)
    arms = [args.arm] if args.arm else sorted(groups)
    for arm in arms:
        history = groups.get(arm)
        if not history:
            raise SystemExit(f"no runs recorded for arm {arm!r} (have: {', '.join(sorted(groups))})")
        latest = history[-1]
        print(f"\n=== {arm} — {len(history)} run(s), latest {latest['when']} "
              f"@ {latest.get('tool_commit') or '?'} on {latest.get('corpus_commit') or '?'} ===")
        if latest.get("metrics"):
            print(f"  {state_of(latest)}: {counts_of(latest['metrics'])}")
        if latest.get("headroom"):
            # Under the bar and close to it are different problems: one is failing now, the
            # other passes today and will fail on noise.
            under = {k: v for k, v in latest["headroom"].items() if v < 0}
            tight = {k: v for k, v in latest["headroom"].items() if 0 <= v <= args.tight}
            if under:
                print("  under its floor: " + ", ".join(f"{k} {v:+d}" for k, v in sorted(under.items())))
            if tight:
                print("  exposed (clears by <=%d, so noise reddens it): " % args.tight +
                      ", ".join(f"{k} {v:+d}" for k, v in sorted(tight.items())))

        if len(history) > 1:
            prev = history[-2]
            why = incomparable(prev, latest)
            fixed, broke = delta(prev, latest)
            if why:
                # Attributing a move to the tool across a moved corpus or a changed case set is
                # the mistake this history exists to stop making, so the words change with it.
                print(f"  NOT COMPARABLE to {prev['when']}: {why}. "
                      f"Differences below belong to the setup, not to the tool.")
                if fixed:
                    print(f"  scores up: " + clip(sorted(fixed), args.top))
                if broke:
                    print(f"  scores down: " + clip(sorted(broke), args.top))
            else:
                if fixed:
                    print(f"  improved since {prev['when']}: " + clip(sorted(fixed), args.top))
                if broke:
                    print(f"  REGRESSED since {prev['when']}: " + clip(sorted(broke), args.top))
                if not fixed and not broke:
                    print(f"  no case changed state since {prev['when']}")
            for k, before, after in metric_moves(prev, latest):
                print(f"  {k}: {before} -> {after}")

        # Counting misses and flips across a moved corpus or a changed case set attributes to
        # the tool exactly what the line above just refused to attribute to it, so the window
        # stops at the first run the latest one cannot be compared to.
        window = comparable_window(history, args.window)
        chronic = weak(window, args.chronic)
        if chronic:
            print(f"  chronic over the last {len(window)} comparable run(s) "
                  f"(missed in >={args.chronic:.0%}), worst first: " + clip(chronic, args.top))
        flappy = flaky(window)
        if flappy:
            print(f"  flaky over the last {len(window)} comparable run(s): " + clip(flappy, args.top))
        if len(window) < min(len(history), args.window):
            print(f"  (window stops at {len(window)} of {len(history)} run(s): the older ones "
                  f"were recorded against a different setup)")

    if not args.arm:
        systemic(groups, args.top)


def clip(items, top):
    """The first `top` entries, saying how many were left out rather than hiding them.

    A tool that fails most of the suite produces a list nobody reads, and an unreadable list
    is the same as no list.
    """
    if top <= 0 or len(items) <= top:
        return ", ".join(items)
    return ", ".join(items[:top]) + f", +{len(items) - top} more"


def comparable_window(history, size):
    """The newest runs that can all be read against the latest one, newest `size` at most."""
    # `history[-0:]` is the whole history and a negative size drops the oldest rows, so a
    # window smaller than one run means one run, not every run.
    size = max(1, size)
    latest = history[-1]
    out = []
    for row in reversed(history[-size:]):
        if row is not latest and incomparable(row, latest):
            break
        out.append(row)
    return list(reversed(out))


def incomparable(prev, latest):
    """Why two runs of one arm cannot be read as a before and after, or None if they can.

    A `changes` case is a diff against the corpus HEAD and a truth denominator is rebuilt from
    the corpus, so a corpus that moved between runs changes the questions and the answers at
    once. A case set that grew or shrank does the same to the totals.
    """
    reasons = []
    a, b = prev.get("corpus_commit"), latest.get("corpus_commit")
    if a is None or b is None:
        # Unknown is not the same as equal: a row with no recorded corpus commit cannot be
        # shown to describe the same corpus, and saying nothing here would let it pass as if
        # it had been checked.
        reasons.append("a run does not record which corpus commit it ran against")
    elif a != b:
        reasons.append(f"the corpus moved {a} -> {b}")
    ca, cb = set(prev.get("cases", {})), set(latest.get("cases", {}))
    if ca != cb:
        reasons.append(f"the case set changed ({len(ca)} -> {len(cb)}, "
                       f"{len(cb - ca)} added, {len(ca - cb)} dropped)")
    fa, fb = prev.get("floors"), latest.get("floors")
    if fa and fb and fa != fb:
        reasons.append("the floors moved")
    return "; ".join(reasons) or None


def delta(prev, latest):
    """Cases whose score rose or fell between two runs of the same arm."""
    fixed, broke = [], []
    for key, now in latest.get("cases", {}).items():
        before = prev.get("cases", {}).get(key)
        if before is None:
            continue
        if now > before:
            fixed.append(f"{key} {before:g}->{now:g}" if now < 1 or before > 0 else key)
        elif now < before:
            broke.append(f"{key} {before:g}->{now:g}" if before < 1 or now > 0 else key)
    return fixed, broke


def metric_moves(prev, latest):
    out = []
    for k, v in latest.get("metrics", {}).items():
        old = prev.get("metrics", {}).get(k)
        if old is None or old == v:
            continue
        fmt = (lambda x: f"{x[0]}/{x[1]}") if isinstance(v, list) else str
        out.append((k, fmt(old), fmt(v)))
    return out


def weak(window, threshold):
    """Cases that miss in most of the window. These are the standing holes, not the noise.

    A case seen once cannot show a pattern, so it needs at least two appearances before it is
    called chronic. The mean score travels with the count: a suite scored by recall can sit at
    0.98 for six runs, which is a different problem from sitting at 0.
    """
    seen, missed, total = {}, {}, {}
    for r in window:
        for key, score in r.get("cases", {}).items():
            seen[key] = seen.get(key, 0) + 1
            missed[key] = missed.get(key, 0) + (score < 1.0)
            total[key] = total.get(key, 0.0) + score
    out = [(k, missed[k], seen[k]) for k in seen
           if seen[k] >= 2 and missed[k] / seen[k] >= threshold]
    return [f"{k} ({m}/{n}, mean {total[k] / n:.2f})"
            for k, m, n in sorted(out, key=lambda t: (total[t[0]] / t[2], t[0]))]


def flaky(window):
    """Cases that change state more than once across the window: fragile, not simply broken."""
    out = []
    for key in {k for r in window for k in r.get("cases", {})}:
        series = [r["cases"][key] for r in window if key in r.get("cases", {})]
        flips = sum(1 for a, b in zip(series, series[1:]) if a != b)
        if flips > 1:
            out.append(f"{key} ({flips} flips)")
    return sorted(out)


def systemic(groups, top):
    """A case every arm misses is a hole in retrieval; one arm alone is that arm's own defect.

    Read suite by suite: the recorded suite and a dev suite ask different questions, and an
    intersection across both would be empty. Only arms whose latest run sits on the newest
    corpus commit take part. Reading one arm's answer at one commit against another arm's
    answer at a different one says nothing about either arm.
    """
    latest = {arm: h[-1] for arm, h in groups.items() if h and h[-1]["source"] == "bench"}
    suites = sorted({r.get("suite") or "built-in" for r in latest.values()})
    for suite in suites:
        rows = {a: r for a, r in latest.items() if (r.get("suite") or "built-in") == suite}
        if len(suites) > 1:
            print(f"\n--- suite {suite} ---")
        systemic_suite(rows, top)


def systemic_suite(latest, top):
    commits = {r.get("corpus_commit") for r in latest.values()}
    if len(commits) > 1:
        # Only rows that recorded a commit can name the newest one. Letting an unrecorded row
        # win would keep the arms that know nothing and drop the arms that do.
        known = [r for r in latest.values() if r.get("corpus_commit")]
        if not known:
            print("\n(no cross-arm reading: no arm recorded which corpus commit it ran against)")
            return
        newest = max(known, key=when_of)["corpus_commit"]
        dropped = sorted(a for a, r in latest.items() if r.get("corpus_commit") != newest)
        latest = {a: r for a, r in latest.items() if r.get("corpus_commit") == newest}
        print(f"\n(cross-arm reading covers {newest} only; "
              f"{', '.join(dropped)} did not record that commit)")
    if len(latest) < 2:
        return
    keys = set.intersection(*(set(r["cases"]) for r in latest.values()))
    everywhere = sorted(k for k in keys if all(r["cases"][k] < 1.0 for r in latest.values()))
    if everywhere:
        print(f"\n=== missed by every recorded arm ({len(latest)}) — a hole in retrieval, "
              f"not an arm's own defect ===\n  " + clip(everywhere, top))
    for arm, r in sorted(latest.items()):
        only = sorted(k for k in keys if r["cases"][k] < 1.0
                      and all(o["cases"][k] == 1.0 for a, o in latest.items() if a != arm))
        if only:
            print(f"\n=== missed only by {arm} — this arm's own defect ===\n  " + clip(only, top))


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)

    r = sub.add_parser("record", help="append a `repograph bench` transcript")
    r.add_argument("transcript", help="file holding the run's stdout, or - for stdin")
    r.add_argument("--corpus", default="beauty-crm")
    r.add_argument("--corpus-path", default=None, help="corpus checkout, to record its commit")
    r.add_argument("--note", default="")
    r.set_defaults(func=cmd_record)

    i = sub.add_parser("import", help="append one row per tool from a compare result JSON")
    i.add_argument("result")
    i.add_argument("--corpus", default="beauty-crm")
    i.add_argument("--note", default="")
    i.set_defaults(func=cmd_import)

    q = sub.add_parser("report", help="what improved, what regressed, what is chronically weak")
    q.add_argument("--arm", default=None)
    q.add_argument("--window", type=int, default=6, help="runs per arm to consider")
    q.add_argument("--chronic", type=float, default=0.66, help="miss rate that counts as chronic")
    q.add_argument("--tight", type=int, default=1, help="headroom at or below this counts as exposed")
    q.add_argument("--top", type=int, default=12, help="entries per list before the rest are counted; 0 for all")
    q.set_defaults(func=cmd_report)

    args = p.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
