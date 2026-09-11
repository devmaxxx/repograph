#!/usr/bin/env python3
"""The arithmetic behind the cost-family bars, in one reviewable place.

A single reading of a reader row cannot be judged: on 2026-09-07 the same command through the
same binary read 0.61 s and 0.75 s, and max RSS bounced between 1.36 and 1.56 GB on both sides of
a control. So a row is a median of n runs, and a bar is usable only once the same binary run
through the suite twice reads inside it. This file is what says so, and the tests beside it are
what a reader checks instead of the shell that produced a table.

`floors_missed` — the one non-zero exit this file reads as a reading. A `bench` row that misses a
floor did all of its work and answered: its wall clock, max RSS and peak CPU are readings of that
reader, and only the verdict on the answers failed. A row that never found a store did not do the
work at all and its clock measured a failing setup, which is no reading of anything. `bench` exits
2 for the first and 1 for the second, so the status is what separates them and the sentence on
stderr is only a message to a person: `measure.sh` writes `floors_missed=1` on a row that exited 2,
and this file believes that field only where the status is still 2 on the line and the row is one
that runs `bench` — a name is a weaker claim than a status, so both have to agree. The name says
the fact and not the consequence, because the consequence differs by reader: to `medians` it is "judge this row", to a person reading the
table it is "this reader's `bench` verdict failed", and a field called `ok` or `refused` would have
had to pick one. It is a flag and not a count: `bench` prints one verdict, never a tally. Absent on
every clean row, so its absence admits nothing — an instrument too old to write it is refused.
`bench/probe/arms.sh` has ruled the same way since `deceb5c` on the same event, for the same reason
in the other direction: an arm that fails a floor is still a recorded arm. The two now agree that a
missed floor is a reading and a broken store is not, and disagree about nothing.

`avg_cpu` — cores busy over the whole run, `(user + sys) / wall` off the fields `measure.sh` has
always written. It is the column every reader row can be judged on: `peak_cpu` is sampled by `top`
once a second, so a command that finishes inside about two seconds reports 0 and stays unjudged,
while the average is derived from the same `time` report the wall clock comes from and exists for
every row that ran at all. A summary too old to carry `user` and `sys` is refused rather than
having the column quietly dropped from it. A row whose wall clock rounded to zero reads 0 and, like
an unsampled `peak_cpu`, is compared against nothing rather than against a number that means
"never measured".
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
# The rows that run `repograph bench` — `bench-dense`, `bench-nodense` in `readers.sh`, `bench` on
# its own elsewhere. No other command can miss a floor, so no other row's `floors_missed` is
# believed however its stderr happened to read.
BENCH_ROW = re.compile(r"^bench(-|$)")
# `/usr/bin/time -l`'s report shares a row's `.time` file with the measured command's stderr and is
# written last, so the file's tail is the report and not what the command said. Every report line
# begins with its own number — `0.61 real …`, `1550000000  maximum resident set size` — which is
# what tells the two apart in a file that has no other structure.
REPORT_LINE = re.compile(r"^\s*\d+(\.\d+)?\s")


def median(xs):
    """The upper middle on an even count, the way `bench`'s p90 and `median` are taken."""
    s = sorted(xs)
    return s[len(s) // 2]


def fields(text):
    return {k: float(v) for k, v in FIELD.findall(text)}


def avg_cpu(wall, user, sys_):
    """Cores busy over the whole run. Zero where the wall clock rounded to zero — nothing was
    measured, and `compare` judges that row against nothing rather than against a zero."""
    return round((user + sys_) / wall, 2) if wall else 0.0


def row_metrics(wall, maxrss, peak_cpu, avg, n, floors_missed=0):
    """One row's readings. `floors_missed` is on the row only where it happened, so every clean row
    reads — and prints, and compares — exactly as it did before the field existed."""
    m = {"wall": wall, "maxrss": maxrss, "peak_cpu": peak_cpu, "avg_cpu": avg, "n": n}
    if floors_missed:
        m["floors_missed"] = 1
    return m


def stderr_tail(logdir, run, keep=2, width=160):
    """The last thing the measured command wrote, off the row's own transcript beside the summary.

    A refusal that says a row failed and not what failed sends its reader looking through a log
    directory for the file this function already knows the name of. `time`'s report is dropped by
    `REPORT_LINE`, so what is quoted is the command's own words — and a line of the command's that
    happens to begin with a number goes with the report, which costs a line of the quote and never
    puts one of `time`'s numbers in the refusal as something the command said.
    """
    if logdir is None:
        return ""
    try:
        text = Path(logdir, f"{run}.time").read_text()
    except OSError:
        return ""
    said = [l.strip() for l in text.splitlines() if l.strip() and not REPORT_LINE.match(l)]
    return "; it said: " + " / ".join(l[:width] for l in said[-keep:]) if said else ""


def medians(lines, logdir=None):
    runs = {}
    for line in lines:
        m = RUN.match(line.strip())
        if not m:
            continue
        f = fields(m.group(3))
        # `measure.sh` writes `wall=s` / `maxrss=GB` when its `time` transcript held no `real`
        # line — the command died before it ran. Naming the row beats a bare KeyError, and beats
        # dropping it: a row silently missing from one side is what `control` exists to catch.
        missing = [k for k in ("wall", "maxrss", "peak_cpu", "user", "sys") if k not in f]
        if missing:
            raise SystemExit(f"{m.group(0)!r}: no {', '.join(missing)} — the run did not complete")
        # `rc` is measure.sh's exit status for the measured command. A failed command still gets a
        # full `time` report, so its row looks like a fast one; it is refused rather than averaged.
        # The exception is the floor verdict the docstring above states, which is a reading — and it
        # takes the status 2, the field, and a row that runs `bench` to claim it.
        floors = (f.get("rc", 0.0) == 2.0 and f.get("floors_missed", 0.0) != 0.0
                  and BENCH_ROW.match(m.group(1)) is not None)
        if f.get("rc", 0.0) != 0.0 and not floors:
            raise SystemExit(f"{m.group(1)}: a run exited {int(f['rc'])} — that row measured a failure, "
                             f"not a reader{stderr_tail(logdir, f'{m.group(1)}-{m.group(2)}')}")
        runs.setdefault(m.group(1), []).append({**f, "floors_missed": 1.0 if floors else 0.0,
                                                "avg_cpu": avg_cpu(f["wall"], f["user"], f["sys"])})
    out = {}
    for name, rs in runs.items():
        out[name] = row_metrics(
            median(r["wall"] for r in rs),
            median(r["maxrss"] for r in rs),
            median(r["peak_cpu"] for r in rs),
            median(r["avg_cpu"] for r in rs),
            len(rs),
            # Any run of the row: a median taken over runs one of which missed a floor is still a
            # reading, and a reader told about it for one run in five knows what they are reading.
            floors_missed=any(r["floors_missed"] for r in rs),
        )
    return out


# `row wall=0.61 maxrss=1.55 peak_cpu=120.0 avg_cpu=0.98 n=5`: what `medians` prints, and what `control` and
# `compare` read back. A run line carries `-<i>` after the row and a `user=` field; a medians
# line carries neither, so the two shapes cannot be confused for each other. `floors_missed=1`
# rides on the end of the rows that carry it and nowhere else, which is what makes the round trip
# through a `medians.txt` lossless without changing the line every other row prints.
MEDIAN_LINE = re.compile(r"^(\S+)\s+(wall=[0-9.]+ maxrss=[0-9.]+ peak_cpu=[0-9.]+ avg_cpu=[0-9.]+ n=\d+(?: floors_missed=1)?)$")


def read_lines(path):
    """A file's lines, or the path in a refusal. A mistyped path is the same mistake `main`'s usage
    lines exist for — and `print_table` already names it as one of the ways a comparison arrives
    with no rows — so it reads as a refusal naming the file, not as a traceback out of `pathlib`.
    """
    try:
        return Path(path).read_text().splitlines()
    except OSError as e:
        raise SystemExit(f"{path}: cannot be read ({e.strerror})")


def read_medians(path):
    lines = read_lines(path)
    out = {}
    for line in lines:
        m = MEDIAN_LINE.match(line.strip())
        if m:
            f = fields(m.group(2))
            out[m.group(1)] = row_metrics(f["wall"], f["maxrss"], f["peak_cpu"], f["avg_cpu"], int(f["n"]),
                                          floors_missed=f.get("floors_missed", 0.0))
    # A summary file's rows have their transcripts beside them, and a refusal that can quote one
    # says more than the row's name — which is the whole of what a reader has to go on.
    return out or medians(lines, Path(path).parent)


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

    Average CPU is the column those rows do have: derived, not sampled, so a reader that finishes
    in half a second still says how many cores it kept busy. It is judged against the same bar, and
    a reference of 0 — a wall clock that rounded away — is unjudged for the same reason.
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
        a = round((new[row]["avg_cpu"] - ref[row]["avg_cpu"]) / ref[row]["avg_cpu"], 4) if ref[row]["avg_cpu"] else None
        ok = (abs(w) <= wall_bar and abs(r) <= rss_bar and (c is None or abs(c) <= cpu_bar)
              and (a is None or abs(a) <= cpu_bar))
        out.append((row, w, r, c, a, ok))
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


def floors_rows(*sides):
    """The rows either side of a comparison read off a `bench` that missed a floor."""
    return {name for side in sides for name, m in side.items() if m.get("floors_missed")}


def floors_note(names):
    return ("floors_missed: " + ", ".join(sorted(names)) + " — a `bench` row that answered every "
            "case and then missed a floor. The wall clock, max RSS and peak CPU are readings of "
            "that reader; the floor verdict is `bench`'s own and is not what this table judges.")


def print_table(rows, head, floors=()):
    # `all([])` is True, so without this a pair of files that parsed to no rows at all — a
    # mistyped path, a suite that died before its first row — prints an empty table and exits 0,
    # which reads exactly like a bar that was cleared.
    if not rows:
        raise SystemExit("no rows to judge — the medians files parsed to nothing")
    # `control` hands over two delta columns and `compare` four, so the width comes from the
    # heads: the two clauses those verdicts answer to were committed before any run and neither
    # gains or loses a column because the other one did. The floor fact rides in the verdict cell
    # for that reason — it is a word about the row's reading, not a fourth thing measured, and a
    # column of its own would have widened a table two committed clauses read.
    print("| row | " + " | ".join(head) + " | |")
    print("|" + "---|" * (len(head) + 2))
    for row in rows:
        name, deltas, ok = row[0], row[1:-1], row[-1]
        cells = " | ".join("n/a" if d is None else f"{d:+.1%}" for d in deltas)
        verdict = ("ok" if ok else "OUTSIDE") + (", floors_missed" if name in floors else "")
        print(f"| {name} | {cells} | {verdict} |")
    said = [row[0] for row in rows if row[0] in floors]
    if said:
        # Blank line first: these tables are pasted into the results document, and a paragraph
        # crowding the last row is a line that renders inside the table it is about.
        print()
        print(floors_note(said))
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
        # `isdecimal`, not `isdigit`: the latter is true of `²` and `int` still raises on it, which
        # is the traceback this clause replaces, arriving through the clause itself.
        min_n = MIN_N
        if len(argv) == 5:
            if not argv[4].isdecimal() or int(argv[4]) < 1:
                raise SystemExit(f"usage: judge.py {cmd} A B [MIN_N] — MIN_N is a count of runs a row, not {argv[4]!r}")
            min_n = int(argv[4])
        a, b = read_medians(argv[2]), read_medians(argv[3])
        floors = floors_rows(a, b)
        if cmd == "control":
            return 0 if print_table(control(a, b, min_n=min_n), ("wall spread", "RSS spread"), floors) else 1
        return 0 if print_table(compare(a, b, min_n=min_n), ("wall Δ", "RSS Δ", "peak CPU Δ", "avg CPU Δ"), floors) else 1
    if cmd == "medians":
        rows = read_medians(argv[2])
        # Printing nothing and exiting 0 reads like a suite with no regressions rather than like a
        # file nothing in it parsed — the same trap `print_table` guards against.
        if not rows:
            raise SystemExit(f"{argv[2]}: no run or medians line parsed — nothing to take a median of")
        for name, m in rows.items():
            floors = " floors_missed=1" if m.get("floors_missed") else ""
            print(f"{name} wall={m['wall']} maxrss={m['maxrss']} peak_cpu={m['peak_cpu']} "
                  f"avg_cpu={m['avg_cpu']} n={m['n']}{floors}")
        # `readers.sh` writes this file and copies it out of the run, so the sentence belongs beside
        # the rows and not only in the table a later `control` prints from them.
        said = floors_rows(rows)
        if said:
            print(floors_note(said))
        return 0
    if cmd == "cadence":
        c = cadence(read_lines(argv[2]))
        ok, why = cadence_ok(c)
        print(f"first={c['first']} max={c['max']} median={c['median']} ratio={c['ratio']:.3f} n={len(c['intervals'])}")
        print(f"intervals={c['intervals']}")
        print(("ok: " if ok else "OUTSIDE: ") + why)
        return 0 if ok else 1
    raise SystemExit(f"unknown command {cmd}")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
