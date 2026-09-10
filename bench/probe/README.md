# The cost-family probe

What every reader and writer costs, measured the same way every time. This directory is the
2026-09-07 kit brought into the tree so the bars it sets can be reviewed in a diff; the pinned
fixture, the one writable worktree and the legacy graph stay under `~/bench/`.

Two directories, both locked: `~/bench/beauty-crm-502e8a6d`, the pinned fixture, read-only for ever
— `bench` and `dump` read its store as it stands, nothing walks it, nothing writes it; and
`~/bench/beauty-crm-test`, the one writable worktree, where every writer and every reader row runs.

- `quiet.sh` — the precondition every row is read under: ≥ 85% idle, 1-minute load < 3.0, AC power,
  no `cargo`/`rustc`/`node`/other `repograph` running. The one exemption is this script's own
  ancestor chain (`$$` upwards through `PPID`): the harness that launched the probe is unavoidable
  and the `load1` clause refuses it on its own when it is busy, while any other `node` — a sibling
  harness, another session's, a dev server — is a core burnt under the row, so it refuses. Sourcing
  the script defines `ancestor_pids` and `busy_lines` and runs nothing, which is how
  `test_quiet.py` reads the walk and the filter on a machine that is not quiet. `readers.sh` and
  `embed.sh`, the two that time anything, run it first and refuse; `reset.sh` and `arms.sh` measure
  no wall clock and do not.
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

**The peak CPU column, and what it cannot say.** `judge.py compare` judges three columns — wall
against 10%, max RSS against 5%, peak CPU against 10% — because §9's gate asks for the run's peak
CPU unmoved and a clause no code reads is green whatever the run did. The column is only as good
as the sampler under it: `measure.sh` samples with `top -l 2 -s 1`, so a command that finishes
inside about two seconds is never sampled and reports `peak_cpu = 0`. That is every reader row,
and it is not the whole-store embed of §9 (≈ 1,930 s, 293% recorded), which is the row the column
was added for. A row whose *reference* read 0 prints `n/a` and is left unjudged on that column
rather than passing on a zero that means "never sampled". Two verdicts keep the columns their own
committed clauses name, and adding this one widened neither: `judge.py control` judges wall and max
RSS only, which is §1's Gate as written, and a reader row read under §1's bars is judged by the
wall and RSS columns — its peak CPU is printed beside them and is not part of that clause. macOS only:
`/usr/bin/time -l`, `top -l`, `pmset` and `caffeinate` are Darwin's, and CI runs none of the shell
here — `python3 -m unittest discover -s bench/probe` is what a change to this directory is gated on,
and its `reset.sh` and `quiet.sh` tests skip themselves off POSIX, the latter also where there is
no `node` to be either the harness or the noise.
