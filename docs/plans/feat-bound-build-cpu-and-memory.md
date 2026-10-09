# feat/bound-build-cpu-and-memory

## Ticket
- **#162: perf: keep graph build and update at about 30% of CPU and memory** (GitHub issue · open ·
  milestone backlog)
  - Problem: a whole-store `build` or `update` takes enough of the machine that the person beside
    it feels it. `balanced` caps the embed at a third of the logical cores (4 on 12), and a
    `model` switch on the bench store was measured at 382%, 32% of the machine.
  - Acceptance: under `balanced`, on the fixture, peak CPU ≤ 30% of the logical cores in every
    phase (≤ 360% on 12); peak RSS measured per phase, recorded, and bounded by something that
    does not grow with the corpus or explained where it does; `low` and `full` keep their
    ordering; the wall-time cost recorded in `docs/configuration.md#resources`.

## What the measurement has to answer first
Per phase of `build` (empty store, every row embedded) and `update` (about a thousand rows owed)
on `~/bench/beauty-crm-test`, at each level: wall, peak CPU over a 1 s window, mean CPU, peak RSS,
threads. Phases are cut by `REPOGRAPH_TIMING` lines: walk, parse (extract), graph (settle), save,
model open, embed. The BM25 build is not in a writer; it runs in the first reader, and is measured
on `--no-dense ask`.

Threads a level does not count, to look for: ORT's inter-op pool (only built in parallel execution
mode, which this binary never asks for), ORT's intra-op pool (the caller plus `n - 1` workers),
the tokenizer's parallelism (it fans out over rayon's global pool, already capped by `cap_pools`),
any `std::thread::spawn` on the writer path (the tokenizer load beside the session load), and the
`ignore` walker (single-threaded `build()`, not `build_parallel()`).

## Files to change
- `src/index/embed.rs` — `threads_from`: `balanced` becomes 30% of the logical cores, rounded
  down, one thread as the floor; its doc comment carries the measurement; unit tests state the
  rule as a property over core counts (never above 30% of the machine, never above `full`, never
  below `low`) beside the 12-core worked values.
- `src/ask.rs`, `src/main.rs` — `REPOGRAPH_TIMING` marks a writer's phases (walked, extracted,
  settled, saved), which is how the measurement cuts one run into phases.
- `docs/configuration.md` — the `resources` row and the Resources section: the new rule, the
  table re-measured on today's fixture and default model, per phase, with the date and the
  wall-time cost.
- `docs/history.md` — the paragraphs that state "a third" as the current rule.
- `docs/bench/2026-10-09-bound-build-results.md` (new) — the transcripts' numbers, the method,
  the threads found, the memory accounting.

## Steps
1. Phase marks under `REPOGRAPH_TIMING`; a "before" binary built from them.
2. Before matrix: `update` and `build` at low / balanced / full, `--no-dense ask` for BM25.
3. Experiments on the `update` scenario to choose the bound (see Decisions).
4. The change in `threads_from`, its tests, an "after" binary.
5. After matrix at the same three levels.
6. Docs: configuration, history, the results file.
7. `cargo test`, `cargo clippy`, `/code-review high --fix`, re-test.
8. `bench/history/run-repograph.sh` to show retrieval did not move; commit, push, PR.

## Tests
- `src/index/embed.rs` — `balanced` is the largest whole thread count within 30% of the logical
  cores on every machine from 1 to 256 cores, one thread where none fits; the ordering
  `low ≤ balanced ≤ full` holds from 0 to 256 and is strict from 7 up; the 12-core values.
- Existing `cap_pools` and `resources_from_env` tests unchanged.
- Perf itself is measured, not unit-tested: the results file is the evidence.

## Decisions
Taken on the `update` scenario (1,011 rows owed, `balanced`, one run each, load 6–7 with two
cores of foreign work — FortiClient `epctrl` and Spotlight — throughout):

| shape | embed wall | peak CPU | mean CPU | peak RSS |
| --- | --- | --- | --- | --- |
| 4 threads, spin on (before) | 106.3 s | 394% | 387% | 1.21 GB |
| 4 threads, spin off | 106.0 s | 394% | 388% | 1.23 GB |
| 3 threads, spin on | 135.6 s | 306% | 300% | 1.17 GB |
| 3 threads, spin off | 136.7 s | 299% | 296% | 1.20 GB |

- **No thread runs that `resources` does not count.** Eight threads at four: the caller and three
  ORT intra-op workers, and rayon's four, which tokenize while the caller waits on them. No
  inter-op pool (sequential execution), a single-threaded walker and parse. The issue's 382% is
  four busy threads, not hidden ones: `n` threads read as `n` cores.
- **Spin-wait is not the lever.** Off changes nothing at either count; the workers compute for
  the whole forward. It stays on, as 2026-09-07 left it.
- **`balanced` becomes `cores * 3 / 10`, floor one.** Twelve cores → 3 threads → ≤ 306%. The
  ~30% wall is the price, and there is no shape that meets the bar for less: the bar is a
  count of busy cores. `low` (a sixth) and `full` (a half) are unchanged, so the ordering is
  `2 < 3 < 6` here and `low ≤ balanced ≤ full` everywhere.
- **Memory is not bounded by a change.** Peak RSS is the model (weights memory-mapped, plus an
  arena bounded by the token budget) plus the graph and the rows owed, which are the corpus
  itself. Recorded per phase and explained, not capped: 30% of this machine is 10.8 GB.
