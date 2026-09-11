#!/usr/bin/env python3
"""The arithmetic behind the cost-family bars, in one reviewable place.

A single reading of a reader row cannot be judged: on 2026-09-07 the same command through the
same binary read 0.61 s and 0.75 s, and max RSS bounced between 1.36 and 1.56 GB on both sides of
a control. So a row is a median of n runs, and a bar is usable only once the same binary run
through the suite twice reads inside it. This file is what says so, and the tests beside it are
what a reader checks instead of the shell that produced a table.

`floors_missed` — the one non-zero exit this file reads as a reading. A `bench` row that misses a
floor did all of its work and answered: its wall clock, max RSS and both CPU columns are readings
of that reader, and only the verdict on the answers failed. A row that never found a store did not
do the work at all and its clock measured a failing setup, which is no reading of anything.
`repograph`
exits 3 for the first and 1 for the second, so the status is what separates them: the status is
the interface, and the wording is a message no script reads. The status is 3 and not 2 because 2
is written above the command — `clap` for a usage error, the npm launcher for a missing platform
binary — so a 2 is always a row that measured something other than a reader, and is refused
whatever its name.

A verdict is not `bench`'s alone: `trace` answers "no path within the depth" with the same status
after spending its wall clock traversing, so a row of any command that exits 3 is an answered row
and is read — and named: it carries `verdict=1` through `medians.txt` and both tables list it,
because a bar set on such a row is set on an answer and not on the traversal the row was written to
time. The field is narrower than the status. `measure.sh` writes `floors_missed=1` only on a
row that runs `bench`, and this file believes it only there and only beside a 3 — a name is a
weaker claim than a status, so both have to agree — while a `bench` row that exited 3 without the
field is refused, and so is any other row that claims it. The name says
the fact and not the consequence, because the consequence differs by reader: to `medians` it is "judge this row", to a person reading the
table it is "this reader's `bench` verdict failed", and a field called `ok` or `refused` would have
had to pick one. It is a flag and not a count: `bench` prints one verdict, never a tally. Absent on
every clean row, so its absence admits nothing — an instrument too old to write it is refused.
`bench/probe/arms.sh` has ruled the same way since `deceb5c` on the same event, for the same reason
in the other direction: an arm that fails a floor is still a recorded arm. The two now agree that a
missed floor is a reading and a broken store is not, and disagree about nothing.

`avg_cpu` — cores busy over the whole run, `(user + sys) / wall` off the fields `measure.sh` has
always written. It is reported and judged by nobody. `peak_cpu` is sampled by `top` once a second,
so a command that finishes inside about two seconds reports 0 and stays unjudged, and the average
is the column those rows do have: derived from the same `time` report the wall clock comes from,
it exists for every row that ran at all. What it does not yet have is a bar. `/usr/bin/time`
reports user and sys to 10 ms, so on a 0.04 s row one tick of either is a fifth to a third of the
column — a spread that would fail a 10% bar on a machine that did nothing wrong. So `compare`
prints the column and takes no verdict on it, `control` prints its spread beside the two spreads
it judges, and the bar waits on a control that has read what that spread is. A summary too old to
carry `user` and `sys` reads without the column rather than being refused: the pair is what the
average is derived from and nothing else reads them, so a row missing them is a row missing a
column and not a run that did not complete.
"""

import re
import sys
from collections import namedtuple
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
# believed however its stderr happened to read. `measure.sh` scopes the field it writes by the same
# expression, quoted there in these words, so the instrument and the judge name the same rows.
BENCH_ROW = re.compile(r"^bench(-|$)")
# The status a command exits with when it answered the question it was asked and the answer was
# "no". 1 is every failure to answer; 2 belongs to `clap` and to the npm launcher and never
# reaches this file as a reading.
VERDICT_RC = 3.0
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
    """Cores busy over the whole run, or `None` where the wall clock rounded to zero — there is
    nothing to divide by, and nothing was measured to divide.

    A measured zero is a zero and is returned as one: `/usr/bin/time` reports user and sys to
    10 ms, so a 40 ms row that kept a core busy for under one tick reports 0.00 of each, and that
    row was read. The two cases are told apart here rather than downstream, because a reader of
    the column cannot tell an absent reading from a quiet one once they share a value.
    """
    return round((user + sys_) / wall, 2) if wall else None


def row_metrics(wall, maxrss, peak_cpu, avg, n, floors_missed=0, verdict=0):
    """One row's readings. `floors_missed` and `verdict` are on the row only where they happened,
    so every clean row reads — and prints, and compares — exactly as it did before either field
    existed. `avg` is `None` on a row that has no reading of the column — a summary that predates
    it, or a wall clock that rounded away with nothing to divide."""
    m = {"wall": wall, "maxrss": maxrss, "peak_cpu": peak_cpu, "avg_cpu": avg, "n": n}
    if floors_missed:
        m["floors_missed"] = 1
    if verdict:
        m["verdict"] = 1
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


def medians(lines, logdir=None, path=None):
    runs = {}
    for line in lines:
        m = RUN.match(line.strip())
        if not m:
            # A run line whose spelling drifted — an index dropped, a field reworded — reads as
            # prose and is skipped, and the median that follows is taken over fewer runs than the
            # file holds, under an `n` that says otherwise. `read_medians` refuses the same shape
            # on the same check, so a file is judged whole on either path through it.
            refuse_drift(line.strip(), path)
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
        # The exception is the verdict the docstring above states, which is a reading: the status
        # admits the row whatever it ran, and the field rides on it for the rows that have floors —
        # required on those and refused on every other, so neither half stands without the other.
        rc = f.get("rc", 0.0)
        claimed = f.get("floors_missed", 0.0) != 0.0
        is_bench = BENCH_ROW.match(m.group(1)) is not None
        answered = rc == VERDICT_RC and claimed == is_bench
        floors = answered and claimed
        # A row that succeeded and claims a missed floor says two things that cannot both be true —
        # `bench` exits 0 only with its floors met — so neither half is believed, whatever the row
        # is called. Reading it as clean would drop the claim; reading it as a verdict would invent
        # a status the run never had.
        if rc == 0.0 and claimed:
            raise SystemExit(f"{m.group(1)}-{m.group(2)}: a run carries floors_missed=1 beside rc=0 — "
                             "the field contradicts the status, and a row that met its floors exits 0"
                             f"{stderr_tail(logdir, f'{m.group(1)}-{m.group(2)}')}")
        if rc != 0.0 and not answered:
            raise SystemExit(f"{m.group(1)}: a run exited {int(f['rc'])} — that row measured a failure, "
                             f"not a reader{stderr_tail(logdir, f'{m.group(1)}-{m.group(2)}')}")
        # The pair the average is derived from, absent on a summary older than the column. The row
        # keeps every other reading and carries `None` where the average would be.
        avg = avg_cpu(f["wall"], f["user"], f["sys"]) if "user" in f and "sys" in f else None
        runs.setdefault(m.group(1), []).append({**f, "floors_missed": 1.0 if floors else 0.0,
                                                "verdict": 1.0 if answered and not floors else 0.0,
                                                "avg_cpu": avg})
    out = {}
    for name, rs in runs.items():
        # All of the runs or none of them: a median over the subset that carried `user` and `sys`
        # would be a column read on fewer runs than the `n` printed beside it.
        avgs = [r["avg_cpu"] for r in rs if r["avg_cpu"] is not None]
        out[name] = row_metrics(
            median(r["wall"] for r in rs),
            median(r["maxrss"] for r in rs),
            median(r["peak_cpu"] for r in rs),
            median(avgs) if len(avgs) == len(rs) else None,
            len(rs),
            # Any run of the row: a median taken over runs one of which missed a floor is still a
            # reading, and a reader told about it for one run in five knows what they are reading.
            floors_missed=any(r["floors_missed"] for r in rs),
            verdict=any(r["verdict"] for r in rs),
        )
    return out


# `row wall=0.61 maxrss=1.55 peak_cpu=120.0% avg_cpu=98% n=5`: what `medians` prints, and what
# `control` and `compare` read back. A run line carries `-<i>` after the row and a `user=` field; a
# medians line carries neither, so the two shapes cannot be confused for each other.
# `floors_missed=1` and `verdict=1` ride on the end of the rows that carry them and nowhere else,
# which is what makes the round trip through a `medians.txt` lossless without changing the line
# every other row prints.
#
# Both CPU columns are written as a percentage of one core, which is the unit `top` reports the
# peak in; `read_medians` divides the average back into the cores it was derived as. The suffix is
# optional on the peak and the whole `avg_cpu=` field is optional, because the references on disk
# were written before either — a reference that predates the column is read without it rather than
# refused, and a comparison against one prints `n/a` in that column. The average's own suffix is
# not optional: a field that may be written in either of two units is one a reader has to guess
# at, and a guess wrong by a hundredfold is what that column would be read as.
MEDIAN_LINE = re.compile(
    r"^(\S+)\s+(wall=[0-9.]+ maxrss=[0-9.]+ peak_cpu=[0-9.]+%? (?:avg_cpu=[0-9.]+% )?n=\d+"
    r"(?: floors_missed=1)?(?: verdict=1)?)$")
# The average carries its unit or it is not an average: `%` is the one spelling `medians` writes,
# and a bare number is a row whose spelling drifted rather than a second unit to be guessed at.
AVG_CPU = re.compile(r"avg_cpu=([0-9.]+)%")
# Every reading this file parses — a median or a run — carries a `wall=`, and none of the prose
# that travels in the same files does: `readers.sh` heads `medians.txt` with the embedder line,
# `quiet.sh` heads a summary with its own, and `medians` prints the floor and verdict notes under
# the rows. So the field is what separates a sentence from a row whose spelling drifted.
ROW_FIELD = re.compile(r"\bwall=")


def refuse_drift(line, path=None):
    """Refuse a line that carries a reading and reads as neither shape; say nothing about any other.

    The one check both readers share. A drifted line skipped is a row the file holds and the
    judgement does not — and both sides of a control drift together, so the comparison that
    follows is green over a suite quietly short of its rows. The prose these files travel with —
    `readers.sh`'s embedder line, `quiet.sh`'s, the floor and verdict notes — carries no `wall=`
    and passes through.
    """
    if not ROW_FIELD.search(line) or RUN.match(line) or MEDIAN_LINE.match(line):
        return
    where = f"{path}: " if path else ""
    raise SystemExit(f"{where}{line!r} carries a reading and reads as neither a median "
                     "(`row wall=… maxrss=… peak_cpu=…% avg_cpu=…% n=…`) nor a run "
                     "(`row-1  wall=…s user=…s sys=…s maxrss=…GB … rc=0`) — a file judged "
                     "without it would be judged over fewer rows than it holds")


def read_avg_cpu(text):
    """The average CPU off a medians line, in cores, or `None` where the line predates the column.

    `NN%` is percent of one core — the unit `medians` prints, shared with the sampled peak — and
    divides back into cores. It is the only spelling that reads: a bare number would be a second
    unit on one field, a hundredfold out if it were read as the first, and `MEDIAN_LINE` refuses
    the line instead of choosing between them.
    """
    m = AVG_CPU.search(text)
    return float(m.group(1)) / 100 if m else None


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
            out[m.group(1)] = row_metrics(f["wall"], f["maxrss"], f["peak_cpu"],
                                          read_avg_cpu(m.group(2)), int(f["n"]),
                                          floors_missed=f.get("floors_missed", 0.0),
                                          verdict=f.get("verdict", 0.0))
    if out:
        for line in lines:
            refuse_drift(line.strip(), path)
        return out
    # A summary file's rows have their transcripts beside them, and a refusal that can quote one
    # says more than the row's name — which is the whole of what a reader has to go on.
    rows = medians(lines, Path(path).parent, path)
    # And a file that is neither — a medians file one field of which was reworded, a log directory's
    # `summary.txt` truncated before its first row — must not reach a caller as an empty comparison:
    # `print_table` refuses that without naming a file, and `control` finds no row missing from
    # either side because both sides are empty.
    if not rows:
        raise SystemExit(f"{path}: no line of it reads as a median "
                         "(`row wall=… maxrss=… peak_cpu=…% avg_cpu=…% n=…`) or as a run "
                         "(`row-1  wall=…s user=…s sys=…s maxrss=…GB … rc=0`) — nothing to judge")
    return rows


def spread(a, b):
    """The gap between two readings of one thing, as a fraction of the mean of the two.

    Symmetric, because the two sides of a control are the same binary run twice and are
    interchangeable: taken against whichever was named first, an 11% pair read 11% one way and
    9.9% the other, which put the order of the arguments across a 10% bar. Against the mean, a
    zero on one side is a reading of that side — 0 beside a 0.5 is 200% — and not a division by
    zero, and two zeros are no spread at all.
    """
    mean = (a + b) / 2
    return 0.0 if not mean else abs(b - a) / mean


def delta(ref, new, unjudged=0.0):
    """The candidate's move off the reference, signed, as a fraction of the reference.

    `unjudged` is what the column reads where there is no reference to be a fraction of — a peak
    `top` never sampled, a wall clock that rounded away, an average a summary predates. `None`
    there is what `print_table` prints as `n/a` and what `compare` skips its verdict on; the wall
    and RSS columns pass 0.0 instead, because a zero there is a row that measured nothing at all
    and the `n/a` would be read as a column that was merely not sampled.
    """
    if not ref or new is None:
        return unjudged
    return round((new - ref) / ref, 4)


def enough_runs(rows, min_n, side):
    for row, m in rows.items():
        if m["n"] < min_n:
            raise SystemExit(f"{row}: {side} carries n={m['n']}, under the floor of {min_n} — "
                             "a median of that many runs is not a reading these bars can judge")


# What `control` hands `print_table` and what a test unpacks. `avg` is the reported spread and is
# not behind `ok`, which is the whole of what this table's two clauses judge.
Control = namedtuple("Control", "row wall rss avg ok")


def control(a, b, wall_bar=WALL_BAR, rss_bar=RSS_BAR, min_n=MIN_N):
    """The same binary twice: every row's spread against the bars it will later judge with.

    Every column is a spread against the mean of the two readings, so the two sides are as
    interchangeable in the table as they are on the machine: `control A B` and `control B A` are
    one reading of one pair of runs, down to the verdict.

    Wall and max RSS are the two §1's Gate names and the two the verdict is taken on. The average
    CPU spread is reported beside them and decides nothing — it is the reading a bar on that column
    would have to be set from, and this is the run that takes it.
    """
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
        # `None` only where a side has no reading at all — a summary that predates the column, or
        # a wall clock that rounded away with nothing to divide. A measured 0.00 is a reading, and
        # its spread against the other run is what this column exists to report.
        avg, other = a[row]["avg_cpu"], b[row]["avg_cpu"]
        c = round(spread(avg, other), 4) if avg is not None and other is not None else None
        out.append(Control(row, w, r, c, w <= wall_bar and r <= rss_bar))
    return out


# What `compare` hands `print_table` and what a test unpacks: `peak` and `avg` are the two CPU
# columns, and only the first of them is behind `ok`.
Row = namedtuple("Row", "name wall rss peak avg ok")


def compare(ref, new, wall_bar=WALL_BAR, rss_bar=RSS_BAR, cpu_bar=CPU_BAR, min_n=MIN_N):
    """A candidate against the reference, deltas signed so a reader sees which way it moved.

    Peak CPU is judged here and not in `control` because G19's gate asks for it — a clause about
    peak CPU that no code reads is green whatever the run did. It is judgeable only where the
    sampler caught something: `measure.sh` samples with `top -l 2 -s 1`, so a command that
    finishes inside about two seconds reports `peak_cpu = 0`, and a reference of 0 is no reading
    to be within 10% of. Those rows carry `None` and stay unjudged on that column rather than
    passing on a zero that means "never sampled".

    Average CPU is the column those rows do have — derived, not sampled, so a reader that finishes
    in half a second still says how many cores it kept busy — and it is printed and not judged. No
    bar has been set on it: `/usr/bin/time` reports to 10 ms, so one tick on a 0.04 s row is a
    fifth of the column, and a bar would have to be set from a control that has read that spread.
    `control` prints it for exactly that reason.
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
        w = delta(ref[row]["wall"], new[row]["wall"])
        r = delta(ref[row]["maxrss"], new[row]["maxrss"])
        c = delta(ref[row]["peak_cpu"], new[row]["peak_cpu"], None)
        a = delta(ref[row]["avg_cpu"], new[row]["avg_cpu"], None)
        ok = abs(w) <= wall_bar and abs(r) <= rss_bar and (c is None or abs(c) <= cpu_bar)
        out.append(Row(row, w, r, c, a, ok))
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
            "case and then missed a floor. The wall clock, max RSS and both CPU columns are "
            "readings of that reader; the floor verdict is `bench`'s own and is not what this "
            "table judges.")


def verdict_rows(*sides):
    """The rows either side of a comparison that answered with a verdict and had no floors to miss."""
    return {name for side in sides for name, m in side.items() if m.get("verdict")}


def verdict_note(names):
    return (", ".join(sorted(names)) + ": answered with a verdict — a bar set on that row is set "
            "on an answer, not a traversal. `trace` that finds no path says so early and its wall "
            "clock is the search that failed, not the walk the row was written to time.")


def print_table(rows, head, floors=(), verdicts=()):
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
    answered = [row[0] for row in rows if row[0] in verdicts]
    if said or answered:
        # Blank line first: these tables are pasted into the results document, and a paragraph
        # crowding the last row is a line that renders inside the table it is about.
        print()
    if said:
        print(floors_note(said))
    if answered:
        print(verdict_note(answered))
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
        floors, answered = floors_rows(a, b), verdict_rows(a, b)
        if cmd == "control":
            head = ("wall spread", "RSS spread", "avg CPU spread")
            return 0 if print_table(control(a, b, min_n=min_n), head, floors, answered) else 1
        head = ("wall Δ", "RSS Δ", "peak CPU Δ", "avg CPU Δ")
        return 0 if print_table(compare(a, b, min_n=min_n), head, floors, answered) else 1
    if cmd == "medians":
        rows = read_medians(argv[2])
        for name, m in rows.items():
            flags = " floors_missed=1" if m.get("floors_missed") else ""
            flags += " verdict=1" if m.get("verdict") else ""
            # Percent of one core, the unit `top` reports the peak in, so the two CPU columns of a
            # `medians.txt` are read in one unit; the field is dropped where the summary predates it.
            avg = "" if m["avg_cpu"] is None else f" avg_cpu={m['avg_cpu'] * 100:.0f}%"
            print(f"{name} wall={m['wall']} maxrss={m['maxrss']} peak_cpu={m['peak_cpu']}%"
                  f"{avg} n={m['n']}{flags}")
        # `readers.sh` writes this file and copies it out of the run, so the sentences belong beside
        # the rows and not only in the table a later `control` prints from them.
        said, answered = floors_rows(rows), verdict_rows(rows)
        if said:
            print(floors_note(said))
        if answered:
            print(verdict_note(answered))
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
