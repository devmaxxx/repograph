# Run history

`runs.jsonl` keeps every recorded benchmark run, one JSON object per line, appended and never
rewritten. `track.py` writes it and reads it back.

A single run answers one question: was the bar cleared. Three questions it cannot answer are
the ones that decide what to work on next.

- **What is chronically weak.** A case that has missed in six consecutive runs is a standing
  hole in retrieval. A case that missed once is noise. Only the history tells them apart.
- **What is fragile.** A case that flips between hit and miss across runs is exposed to
  something the suite does not control. It will eventually flip on the day it matters.
- **What actually moved.** After a change, which cases are newly answered and which are newly
  lost — as opposed to which numbers happen to differ for reasons that have nothing to do with
  the change.

## The routine run

```
bench/history/run-repograph.sh
```

Builds the release binary, runs `bench` in the dense and lexical arms against the pinned
fixture, records both, and prints the report. Roughly two minutes and no model tokens.

The fixture is a corpus checkout at a commit that never moves, so two runs of this script
differ only by repograph's own code. The script refuses to run if the fixture has moved or is
dirty, because a run against a changed corpus produces history rows that cannot be compared to
the rows before them — and the difference would read as a regression in repograph.

`bench/compare/run.py` is the other harness and answers a different question: how the three
tools compare. It needs the graphify and gitnexus indexes and takes far longer. Import its
results here when you run it:

```
bench/history/track.py import bench/results/<file>.json --note "what this run was for"
```

## A second suite

```
CASES=bench/dev-cases.jsonl bench/history/run-repograph.sh
```

`bench/dev-cases.jsonl` holds sixty questions written the way a developer asks them while
implementing — long, mixed product and code vocabulary, and often with several right places:
`expect` may be a list, and a case's score in the history is the share of its anchors the answer
reached, while the summary line counts entry points (at least one anchor). The file has no floors,
so `bench` measures it, prints `gated=false`, and exits 0 whatever it reads; its runs are recorded
under an arm of their own — `bench[dev-cases]:dense+enriched` — and never share a comparability
window with the recorded suite. Kinds are `long`, `cross` (a requirement and the file that
implements it), `multi` (several requirements), `where` (which file to edit) and `rule` (an
invariant, ADR or NFR). Every anchor is checked against the graph before a question is asked, so a
mistyped anchor stops the run instead of reading as a weak spot. Two cases of one kind may point
at the same place — two `rule` questions ask about ADR-031 — and each keeps its own score, the
second under a `#2` suffix. Dev rows recorded before that suffix existed hold 59 keys for 60
cases, so the next dev run reports `NOT COMPARABLE … the case set changed (59 -> 60)` once
against them; that reading is honest, and the runs after it compare normally.

## Reading the report

```
bench/history/track.py report              # every arm
bench/history/track.py report --arm bench:dense+enriched
```

Each arm prints its latest metrics and then, where the history supports it:

- **exposed** — floors the run clears by one point or less. These flip red on noise rather than
  on a regression. The raw floors were set at exactly what they measured, so they start here.
- **improved / REGRESSED** — cases that changed state since the previous run of the same arm.
- **NOT COMPARABLE** — printed instead, when the corpus commit moved, the case set changed size,
  the floors moved, or a run did not record which corpus it ran against. The differences are
  still listed, under words that do not attribute them to the tool. This repository has made
  that attribution error twice and documented both times; the check exists so it cannot be made
  silently again. The same check bounds the window below: chronic and flaky counts stop at the
  first run the latest one cannot be read against, and the report says when it stopped early.
- **chronic** — cases missing in most of the window, worst mean score first. A case needs at
  least two appearances before it can be called chronic. The mean travels with the count
  because a suite scored by recall can sit at 0.98 for six runs, which is a different problem
  from sitting at zero.
- **flaky** — cases that changed state more than once inside the window. One flip is a fix or a
  regression and is reported as one; two is instability.

After the per-arm sections, two cross-arm readings: cases missed by every recorded arm, which
are holes in retrieval rather than any one arm's defect, and cases missed by exactly one arm,
which are that arm's own. Only arms whose latest run sits on the newest corpus commit take
part, and the report names any it left out.

## The floors are not restated here

`headroom` and `green` are computed against the floors read out of `passes` in `src/bench.rs`
at report time. A floor that moved in Rust and not here would make every headroom figure
quietly wrong, so there is no second copy to drift. If `passes` changes shape, `track.py`
stops with an error rather than falling back to remembered numbers.

## The fixture

`~/bench/beauty-crm-502e8a6d` — a detached worktree of the corpus pinned at `502e8a6d`,
carrying the indexes for all three tools so none of them has to be rebuilt. Building it is
described in `docs/bench/runbook.md`.

`FIXTURE=/path` moves where the runner looks; it does not change which commit is acceptable.
The runner refuses anything but `502e8a6d`, because every row already in `runs.jsonl` was
recorded against that corpus and a run against another one is not comparable to any of them.
Changing the pinned commit is a deliberate act: edit the check, and expect the history to
restart from there.
