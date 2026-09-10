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
# §1's Gate reads its control at n = 5 a row and `readers.sh` defaults to five, but nothing read
# the `n` this file has always parsed: a comparison built from one run a row cleared the bars in
# silence, which is what the docstring above says the file exists to prevent. A suite with a shape
# of its own — §9's embed is three runs — says the smaller number out loud instead of inheriting it.
MIN_N = 5

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


def enough_runs(rows, min_n, side):
    for row, m in rows.items():
        if m["n"] < min_n:
            raise SystemExit(f"{row}: {side} carries n={m['n']}, under the floor of {min_n} — "
                             "a median of that many runs is not a reading these bars can judge")


def control(a, b, wall_bar=WALL_BAR, rss_bar=RSS_BAR, min_n=MIN_N):
    """The same binary twice: every row's spread against the bars it will later judge with."""
    enough_runs(a, min_n, "the first run")
    enough_runs(b, min_n, "the second run")
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


def compare(ref, new, wall_bar=WALL_BAR, rss_bar=RSS_BAR, cpu_bar=CPU_BAR, min_n=MIN_N):
    """A candidate against the reference, deltas signed so a reader sees which way it moved.

    Peak CPU is judged here and not in `control` because G19's gate asks for it — a clause about
    peak CPU that no code reads is green whatever the run did. It is judgeable only where the
    sampler caught something: `measure.sh` samples with `top -l 2 -s 1`, so a command that
    finishes inside about two seconds reports `peak_cpu = 0`, and a reference of 0 is no reading
    to be within 10% of. Those rows carry `None` and stay unjudged on that column rather than
    passing on a zero that means "never sampled".
    """
    enough_runs(ref, min_n, "the reference")
    enough_runs(new, min_n, "the candidate")
    out = []
    for row in new:
        if row not in ref:
            raise SystemExit(f"{row}: in the candidate and not in the reference")
    for row in ref:
        if row not in new:
            raise SystemExit(f"{row}: in the reference and not in the candidate")
        w = round((new[row]["wall"] - ref[row]["wall"]) / ref[row]["wall"], 4) if ref[row]["wall"] else 0.0
        r = round((new[row]["maxrss"] - ref[row]["maxrss"]) / ref[row]["maxrss"], 4) if ref[row]["maxrss"] else 0.0
        c = round((new[row]["peak_cpu"] - ref[row]["peak_cpu"]) / ref[row]["peak_cpu"], 4) if ref[row]["peak_cpu"] else None
        ok = abs(w) <= wall_bar and abs(r) <= rss_bar and (c is None or abs(c) <= cpu_bar)
        out.append((row, w, r, c, ok))
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
    # `control` hands over two delta columns and `compare` three, so the width comes from the
    # heads: the two clauses those verdicts answer to were committed before any run and neither
    # gains or loses a column because the other one did.
    print("| row | " + " | ".join(head) + " | |")
    print("|" + "---|" * (len(head) + 2))
    for row in rows:
        name, deltas, ok = row[0], row[1:-1], row[-1]
        cells = " | ".join("n/a" if d is None else f"{d:+.1%}" for d in deltas)
        print(f"| {name} | {cells} | {'ok' if ok else 'OUTSIDE'} |")
    return all(row[-1] for row in rows)


def main(argv):
    if len(argv) < 3:
        raise SystemExit("usage: judge.py medians SUMMARY | control A B [MIN_N] | "
                         "compare REF NEW [MIN_N] | cadence STAMPED")
    cmd = argv[1]
    if cmd in ("control", "compare"):
        # Both read `argv[3]`, and the check above let `judge.py control a.txt` reach it: a
        # traceback where the usage line belongs.
        if len(argv) not in (4, 5):
            raise SystemExit(f"usage: judge.py {cmd} A B [MIN_N]")
        # And `int` on a word raises a ValueError traceback in the same place, while a floor of 0
        # or less is no floor at all: it admits the one-run-a-row comparison MIN_N exists to refuse.
        min_n = MIN_N
        if len(argv) == 5:
            if not argv[4].isdigit() or int(argv[4]) < 1:
                raise SystemExit(f"usage: judge.py {cmd} A B [MIN_N] — MIN_N is a count of runs a row, not {argv[4]!r}")
            min_n = int(argv[4])
        a, b = read_medians(argv[2]), read_medians(argv[3])
        if cmd == "control":
            return 0 if print_table(control(a, b, min_n=min_n), ("wall spread", "RSS spread")) else 1
        return 0 if print_table(compare(a, b, min_n=min_n), ("wall Δ", "RSS Δ", "peak CPU Δ")) else 1
    if cmd == "medians":
        rows = read_medians(argv[2])
        # Printing nothing and exiting 0 reads like a suite with no regressions rather than like a
        # file nothing in it parsed — the same trap `print_table` guards against.
        if not rows:
            raise SystemExit(f"{argv[2]}: no run or medians line parsed — nothing to take a median of")
        for row, m in rows.items():
            print(f"{row} wall={m['wall']} maxrss={m['maxrss']} peak_cpu={m['peak_cpu']} n={m['n']}")
        return 0
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
