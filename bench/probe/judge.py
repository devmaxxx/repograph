#!/usr/bin/env python3
"""The arithmetic behind the cost-family bars, in one reviewable place.

A single reading of a reader row cannot be judged: on 2026-09-07 the same command through the
same binary read 0.61 s and 0.75 s, and max RSS bounced between 1.36 and 1.56 GB on both sides of
a control. So a row is a median of n runs, and a bar is usable only once the same binary run
through the suite twice reads inside it. This file is what says so, and the tests beside it are
what a reader checks instead of the shell that produced a table.
"""

import re
import sys
from pathlib import Path

WALL_BAR = 0.10
RSS_BAR = 0.05
CPU_BAR = 0.10

# `NAME-i  wall=0.61s user=… maxrss=1.55GB peak_cpu=120% …` from measure.sh; the unit suffixes
# are stripped so the numbers compare, and `NAME-i` is `NAME` with its run index.
RUN = re.compile(r"^(\S+?)-(\d+)\s+(.*)$")
FIELD = re.compile(r"(\w+)=([0-9.]+)")
STAMP = re.compile(r"^(\d+\.\d+)\s+(.*)$")
PROGRESS = re.compile(r"^dense: (\d+)/(\d+) rows")


def median(xs):
    """The upper middle on an even count, the way `bench`'s p90 and `median` are taken."""
    s = sorted(xs)
    return s[len(s) // 2]


def fields(text):
    return {k: float(v) for k, v in FIELD.findall(text)}


def medians(lines):
    runs = {}
    for line in lines:
        m = RUN.match(line.strip())
        if not m:
            continue
        f = fields(m.group(3))
        # `measure.sh` writes `wall=s` / `maxrss=GB` when its `time` transcript held no `real`
        # line — the command died before it ran. Naming the row beats a bare KeyError, and beats
        # dropping it: a row silently missing from one side is what `control` exists to catch.
        missing = [k for k in ("wall", "maxrss", "peak_cpu") if k not in f]
        if missing:
            raise SystemExit(f"{m.group(0)!r}: no {', '.join(missing)} — the run did not complete")
        # `rc` is measure.sh's exit status for the measured command. A failed command still gets a
        # full `time` report, so its row looks like a fast one; it is refused rather than averaged.
        if f.get("rc", 0.0) != 0.0:
            raise SystemExit(f"{m.group(1)}: a run exited {int(f['rc'])} — that row measured a failure, not a reader")
        runs.setdefault(m.group(1), []).append(f)
    out = {}
    for row, rs in runs.items():
        out[row] = {
            "wall": median(r["wall"] for r in rs),
            "maxrss": median(r["maxrss"] for r in rs),
            "peak_cpu": median(r["peak_cpu"] for r in rs),
            "n": len(rs),
        }
    return out


# `row wall=0.61 maxrss=1.55 peak_cpu=120.0 n=5`: what `medians` prints, and what `control` and
# `compare` read back. A run line carries `-<i>` after the row and a `user=` field; a medians
# line carries neither, so the two shapes cannot be confused for each other.
MEDIAN_LINE = re.compile(r"^(\S+)\s+(wall=[0-9.]+ maxrss=[0-9.]+ peak_cpu=[0-9.]+ n=\d+)$")


def read_medians(path):
    lines = Path(path).read_text().splitlines()
    out = {}
    for line in lines:
        m = MEDIAN_LINE.match(line.strip())
        if m:
            f = fields(m.group(2))
            out[m.group(1)] = {"wall": f["wall"], "maxrss": f["maxrss"], "peak_cpu": f["peak_cpu"], "n": int(f["n"])}
    return out or medians(lines)


def spread(a, b):
    return 0.0 if a == 0 else abs(b - a) / a


def control(a, b, wall_bar=WALL_BAR, rss_bar=RSS_BAR):
    """The same binary twice: every row's spread against the bars it will later judge with."""
    out = []
    for row in b:
        if row not in a:
            raise SystemExit(f"{row}: present in one control run and not the other — the suites differ")
    for row in a:
        if row not in b:
            raise SystemExit(f"{row}: present in one control run and not the other — the suites differ")
        w = round(spread(a[row]["wall"], b[row]["wall"]), 4)
        r = round(spread(a[row]["maxrss"], b[row]["maxrss"]), 4)
        out.append((row, w, r, w <= wall_bar and r <= rss_bar))
    return out


def compare(ref, new, wall_bar=WALL_BAR, rss_bar=RSS_BAR):
    """A candidate against the reference, deltas signed so a reader sees which way it moved."""
    out = []
    for row in new:
        if row not in ref:
            raise SystemExit(f"{row}: in the candidate and not in the reference")
    for row in ref:
        if row not in new:
            raise SystemExit(f"{row}: in the reference and not in the candidate")
        w = round((new[row]["wall"] - ref[row]["wall"]) / ref[row]["wall"], 4) if ref[row]["wall"] else 0.0
        r = round((new[row]["maxrss"] - ref[row]["maxrss"]) / ref[row]["maxrss"], 4) if ref[row]["maxrss"] else 0.0
        out.append((row, w, r, abs(w) <= wall_bar and abs(r) <= rss_bar))
    return out


def cadence(lines):
    """Intervals between progress lines, from wall-clock stamps `embed.sh` puts on stderr.

    Stamps and not `rows/s`: a laptop that slept mid-run does not advance the rate, and the run
    that closed G19 on 2026-09-09 had to reconstruct its intervals for exactly that reason.
    """
    start = None
    marks = []
    for line in lines:
        m = STAMP.match(line.rstrip())
        if not m:
            continue
        t, rest = float(m.group(1)), m.group(2)
        if rest == "start":
            start = t
        elif PROGRESS.match(rest):
            marks.append(t)
    if start is None or not marks:
        raise SystemExit("no `start` stamp or no progress line — the run did not embed anything")
    intervals = [round(marks[0] - start, 2)] + [round(b - a, 2) for a, b in zip(marks, marks[1:])]
    med = median(intervals)
    return {"first": intervals[0], "intervals": intervals, "max": max(intervals), "median": med,
            "ratio": max(intervals) / med if med else float("inf")}


def cadence_ok(c, bar=60.0, tail=1.3):
    if c["max"] >= bar:
        return False, f"an interval of {c['max']} s is not under {bar} s"
    if c["ratio"] > tail:
        return False, f"the longest interval is {c['ratio']:.3f} × the median, over {tail}"
    return True, "every interval under the bar and the tail flat"


def print_table(rows, head):
    # `all([])` is True, so without this a pair of files that parsed to no rows at all — a
    # mistyped path, a suite that died before its first row — prints an empty table and exits 0,
    # which reads exactly like a bar that was cleared.
    if not rows:
        raise SystemExit("no rows to judge — the medians files parsed to nothing")
    print(f"| row | {head[0]} | {head[1]} | |")
    print("|---|---|---|---|")
    for row, w, r, ok in rows:
        print(f"| {row} | {w:+.1%} | {r:+.1%} | {'ok' if ok else 'OUTSIDE'} |")
    return all(ok for _, _, _, ok in rows)


def main(argv):
    if len(argv) < 3:
        raise SystemExit("usage: judge.py medians SUMMARY | control A B | compare REF NEW | cadence STAMPED")
    cmd = argv[1]
    if cmd == "medians":
        for row, m in read_medians(argv[2]).items():
            print(f"{row} wall={m['wall']} maxrss={m['maxrss']} peak_cpu={m['peak_cpu']} n={m['n']}")
        return 0
    if cmd == "control":
        return 0 if print_table(control(read_medians(argv[2]), read_medians(argv[3])), ("wall spread", "RSS spread")) else 1
    if cmd == "compare":
        return 0 if print_table(compare(read_medians(argv[2]), read_medians(argv[3])), ("wall Δ", "RSS Δ")) else 1
    if cmd == "cadence":
        c = cadence(Path(argv[2]).read_text().splitlines())
        ok, why = cadence_ok(c)
        print(f"first={c['first']} max={c['max']} median={c['median']} ratio={c['ratio']:.3f} n={len(c['intervals'])}")
        print(f"intervals={c['intervals']}")
        print(("ok: " if ok else "OUTSIDE: ") + why)
        return 0 if ok else 1
    raise SystemExit(f"unknown command {cmd}")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
