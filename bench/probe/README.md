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
- `embedder.sh` — sourced, never run: the one place that names which weights every row here is read
  under, and the refusal when the environment does not say so. `reset.sh`, `readers.sh`, `embed.sh`
  and `arms.sh` each source it and refuse with exit 2 unless `REPOGRAPH_EMBED_MODEL` is exported and
  names `intfloat/multilingual-e5-small`, so every invocation of the kit reads
  `REPOGRAPH_EMBED_MODEL=intfloat/multilingual-e5-small bench/probe/<script> …` (or exports it once
  for the session). Declared in the environment rather than in a `repograph.toml` beside the store
  for two reasons the file could not give: the variable outranks the model a store records, so a
  store restored under a changed built-in default is still read and embedded with the weights the
  reading names — the default was `e5-large` between `35357c1` and 2026-09-07 under ADR-002 — and it
  leaves the corpus worktree with no file of ours in it, where an untracked `repograph.toml` is a
  changed path `changes` reads and every `changes` row would map two paths instead of the one it
  touched. Unset is refused rather than defaulted, so a row can never be read under whatever the
  built-in default happens to be that month, and a value naming another model is refused rather than
  replaced. The declaration travels onto each row's transcript — a line in `summary.txt` beside the
  quiet line for `readers.sh` and `embed.sh`, the head of each arm's own output for `arms.sh` — and
  onto `readers.sh`'s `medians.txt`, which is the file copied out as the reference every later
  reading is judged against, so a reference cannot be compared against a suite read under other
  weights. `judge.py medians` and `read_medians` take only lines shaped like a run and a median, so
  the line travels through both without being read as a row.
- `reset.sh` — the writable worktree put back to the fixture's state: tree reverted and cleaned,
  store copied in, stamps settled by one `--no-dense update`. Refuses any directory that is not the
  locked worktree at `502e8a6d`. Before every arm, reader suites included.
  What it leaves has nothing untracked in it — `git status --porcelain` is empty — which is what the
  `changes` rows need, and it removes a `repograph.toml` an earlier reset of this kit wrote.
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
never did the work at all. **As of 0.6.0 the status says which:** `bench` exits 2 for a suite that
answered every case and missed a floor, and 1 where there was nothing to measure. `measure.sh`
reads that status and writes `floors_missed=1` beside `rc=2`, and `judge.py` believes the field
only where the status is still 2 on the line and the row is one that runs `bench` — `bench`,
`bench-dense`, `bench-nodense`. The sentence on stderr is a message to a person now and nothing
reads it, so it is free to be reworded. **The kit therefore needs a binary that exits 2 for a
verdict:** an older one exits 1 with the old words, and its missed floors are refused as the
failures they are then indistinguishable from. Every other non-zero exit is still refused, and the
refusal quotes the tail of what the row said, so a reader is told what failed and not only that
something did. The field rides through `medians.txt` on the rows that carry it, and `control` and
`compare` print it in the verdict cell and say it in a line under the table. `arms.sh` has ruled
the same way on the same event since `deceb5c` — an arm that fails a floor is still a recorded arm
— and the two now agree.

**Which column a row is judged on.** Every reader row is judged on the derived average, `avg_cpu`;
only the whole-store embed is judged on the sampled peak. The two are different readings of the
same run and neither replaces the other: `avg_cpu` is `(user + sys) / wall` off the `time` report
`measure.sh` already writes, so it exists for a row that lasted 40 ms, while `peak_cpu` is what
`top` caught at its loudest and exists only for a row that lasted long enough to be sampled. A
summary too old to carry `user` and `sys` is refused rather than read without the column.

**The peak CPU column, and what it cannot say.** `judge.py compare` judges four columns — wall
against 10%, max RSS against 5%, peak CPU and average CPU against 10% each — because §9's gate asks for the run's peak
CPU unmoved and a clause no code reads is green whatever the run did. The column is only as good
as the sampler under it: `measure.sh` samples with `top -l 2 -s 1`, so a command that finishes
inside about two seconds is never sampled and reports `peak_cpu = 0`. That is every reader row,
and it is not the whole-store embed of §9 (≈ 1,930 s, 293% recorded), which is the row the column
was added for. A row whose *reference* read 0 prints `n/a` and is left unjudged on that column
rather than passing on a zero that means "never sampled" — which is why the average column was
added: it is the one the reader rows can be judged on. (An average is unjudged on the same terms,
where a wall clock rounded to zero left nothing to divide by.) The one row the column judges never
carries that zero into the table at all: `embed.sh` counts the samples its peak was taken over,
puts the count on the row as `samples=` beside `measure.sh`'s, and refuses to print a row where the
sampler read nothing or read a peak of zero — refusing when no PID was ever found guards one door
of two, and the other is every `top` failing or the process ending between two checks. Two verdicts
keep the columns their own committed clauses name, and adding these did not widen the control:
`judge.py control` judges wall and max RSS only, which is §1's Gate as written, and a reader row read under §1's bars is judged by the
wall and RSS columns — its peak CPU is printed beside them and is not part of that clause. macOS only:
`/usr/bin/time -l`, `top -l`, `pmset` and `caffeinate` are Darwin's, and CI runs none of the shell
here — `python3 -m unittest discover -s bench/probe` is what a change to this directory is gated on,
and its `reset.sh` and `quiet.sh` tests skip themselves off POSIX, the latter's one whole-script
case that needs a real harness also where there is no `node` to be one.
