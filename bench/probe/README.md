# The cost-family probe

What every reader and writer costs, measured the same way every time. This directory is the
2026-09-07 kit brought into the tree so the bars it sets can be reviewed in a diff; the pinned
fixture, the one writable worktree and the legacy graph stay under `~/bench/`.

Two directories, both locked: `~/bench/beauty-crm-502e8a6d`, the pinned fixture, read-only for ever
— `bench` and `dump` read its store as it stands, nothing walks it, nothing writes it; and
`~/bench/beauty-crm-test`, the one writable worktree, where every writer and every reader row runs.

- `quiet.sh` — the precondition every row is read under: ≥ 85% idle, 1-minute load < 3.0, AC power,
  no `cargo`/`rustc`/`node`/other `repograph` running. Every other script runs it first and refuses.
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
A whole-store embed is judged as a median of three against a control of three. macOS only:
`/usr/bin/time -l`, `top -l`, `pmset` and `caffeinate` are Darwin's, and CI runs none of the shell
here — `python3 -m unittest discover -s bench/probe` is what a change to this directory is gated on,
and its `reset.sh` tests skip themselves off POSIX.
