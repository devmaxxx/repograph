# The cost-family probe

What every reader and writer costs, measured the same way every time. This directory is the
2026-09-07 kit brought into the tree so the bars it sets can be reviewed in a diff; the pinned
fixture, the one writable worktree and the legacy graph stay under `~/bench/`.

Two directories, both locked: `~/bench/beauty-crm-502e8a6d`, the pinned fixture, read-only for ever
— `bench` and `dump` read its store as it stands, nothing walks it, nothing writes it; and
`~/bench/beauty-crm-test`, the one writable worktree, where every writer and every reader row runs.

- `quiet.sh` — the precondition every row is read under: ≥ 85% idle, 1-minute load < 3.0, AC power,
  no `cargo`/`rustc`/other `repograph` at all, and no other `node` at or above 5.0% CPU. The two
  rules differ because the two kinds of noise do. This project's own tools are bursty, so presence
  is the whole rule for them: a `cargo` at 0% is between two crates and a `repograph` at 0% is
  about to embed. A `node` at 0% is a sibling harness or an MCP server asleep, and a process
  consuming no CPU moves no wall clock — counting those refused this machine on 66 of them, which
  measured the session and not the noise. The one exemption is this script's own ancestor chain
  (`$$` upwards through `PPID`), at any CPU: the harness that launched the probe is unavoidable and
  the `load1` clause refuses it on its own when it is busy. The printed line names what each rule
  counted — `busy=0 (cargo/rustc/repograph: 0, node ≥5.0%: 0)` — and each refusal lists the
  `pid pcpu comm` rows behind its count. What says whether the machine spoiled a row is not this
  clause but G23's control, the same binary through the suite twice; the precondition is here to
  catch the obvious cases cheaply. Sourcing the script defines `ancestor_pids`, `busy_lines` and
  `busy_rows` and runs nothing, which is how `test_quiet.py` reads the walk and both rules on a
  machine that is not quiet. `readers.sh` and `embed.sh`, the two that time anything, run it first
  and refuse; `reset.sh` and `arms.sh` measure no wall clock and do not.
- `reset.sh` — the writable worktree put back to the fixture's state: tree reverted and cleaned,
  store copied in, `repograph.toml` written, stamps settled by one `--no-dense update`. Refuses any
  directory that is not the locked worktree at `502e8a6d`. Before every arm, reader suites included.
- `measure.sh NAME LOG -- cmd…` — one command: wall, user, sys, max RSS, sampled peak CPU and threads.
- `readers.sh BIN WORKTREE LOG [N]` — the ten reader rows, N runs each, every row `--stale`, medians
  in `LOG/medians.txt`. In the writable worktree straight after `reset.sh`; the suite writes nothing,
  and the store's checksums before and after are how that is shown.
- `embed.sh NAME BIN WORKTREE LOG` — a whole-store embed with stamped stderr, for the cadence. It
  removes the vectors first, so it refuses the pinned fixture by path.
- `arms.sh BIN REPO LOG TAG` — the four bench arms, recorded into `bench/history/runs.jsonl`; `REPO`
  is the fixture for a baseline read.
- `judge.py` — medians, the twice-through-one-binary control, a candidate against a reference, the
  cadence reading. `graphdiff.py` — two `graph.json` files, same visible graph or not.

The bars: a reader row is judged as a median of n ≥ 5, within 10% wall and 5% max RSS of its
reference, and only once `judge.py control` has shown the same binary twice inside those bars on
this machine (`docs/bench/2026-09-10-next-version-levers-results.md` §1 is the first such control).
That `n` is read and not just printed: `control` and `compare` refuse a row built from fewer runs
than the floor, which is five unless a last argument (`judge.py compare REF NEW 3`) says otherwise.
A whole-store embed is judged as a median of three against a control of three, and §9 reads its
three clauses off the summary lines by hand.

**A missed floor is a reading; a broken store is not.** `judge.py medians` refuses a whole suite
when any row exited non-zero, because a command that failed still gets a full `time` report and its
row would enter the median as an honestly-measured fast run. One non-zero exit is not that: a
`bench` row that answered every case and then missed a floor spent its wall clock reading, and its
wall, max RSS and peak CPU are readings of that reader — while a `bench` row that found no store
never did the work at all. `bench` exits 1 for both (`src/main.rs`, `anyhow::bail!("bench floors not
met")`, the same status an empty graph or a missing case file bails with), so the wording on stderr
is what separates them: `measure.sh` greps the row's own transcript and writes `floors_missed=1`
beside `rc=`, and `judge.py` believes that field only on a row that runs `bench` — `bench`,
`bench-dense`, `bench-nodense`. Every other non-zero exit is still refused, and the refusal now
quotes the tail of what the row said, so a reader is told what failed and not only that something
did. The field rides through `medians.txt` on the rows that carry it, and `control` and `compare`
print it in the verdict cell and say it in a line under the table. `arms.sh` has ruled the same way
on the same event since `deceb5c` — an arm that fails a floor is still a recorded arm — and the two
now agree. If `bench`'s wording ever changes, `bench/probe/test_measure.py` goes red, and until the
grep follows the rename a missed floor refuses the suite rather than passing unnoticed.

**The peak CPU column, and what it cannot say.** `judge.py compare` judges three columns — wall
against 10%, max RSS against 5%, peak CPU against 10% — because §9's gate asks for the run's peak
CPU unmoved and a clause no code reads is green whatever the run did. The column is only as good
as the sampler under it: `measure.sh` samples with `top -l 2 -s 1`, so a command that finishes
inside about two seconds is never sampled and reports `peak_cpu = 0`. That is every reader row,
and it is not the whole-store embed of §9 (≈ 1,930 s, 293% recorded), which is the row the column
was added for. A row whose *reference* read 0 prints `n/a` and is left unjudged on that column
rather than passing on a zero that means "never sampled". The one row the column judges never
carries that zero into the table at all: `embed.sh` counts the samples its peak was taken over,
puts the count on the row as `samples=` beside `measure.sh`'s, and refuses to print a row where the
sampler read nothing or read a peak of zero — refusing when no PID was ever found guards one door
of two, and the other is every `top` failing or the process ending between two checks. Two verdicts
keep the columns their own committed clauses name, and adding this one widened neither: `judge.py control` judges wall and max
RSS only, which is §1's Gate as written, and a reader row read under §1's bars is judged by the
wall and RSS columns — its peak CPU is printed beside them and is not part of that clause. macOS only:
`/usr/bin/time -l`, `top -l`, `pmset` and `caffeinate` are Darwin's, and CI runs none of the shell
here — `python3 -m unittest discover -s bench/probe` is what a change to this directory is gated on,
and its `reset.sh` and `quiet.sh` tests skip themselves off POSIX, the latter's one whole-script
case that needs a real harness also where there is no `node` to be one.
