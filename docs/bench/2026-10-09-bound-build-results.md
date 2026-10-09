# `balanced` under 30% of the machine, per phase

Machine: Apple Silicon, 12 logical cores (6 performance + 6 efficiency), 36 GB, macOS 25.6.
Corpus: `~/bench/beauty-crm-test`, the one writable worktree at corpus commit `502e8a6d` — 1,409
files read, 12,999 nodes, 41,113 edges, 11,590 dense rows at 768-d under the default model,
`onnx-community/embeddinggemma-300m-ONNX`. Tool: `425ac7a` (origin/main) plus this branch's phase
marks for the before rows, this branch for the after rows; `cargo build --release`.
Kit and transcripts: `/Users/max/bench/resources-2026-10-09` — `kit/sample.py` (the sampler),
`kit/analyze.py` (the phase cut), `kit/matrix.sh` and `kit/edit.py` (the scenarios), and one
`.json` plus `.json.log` per run under `log/`.

Method. A sampler (`ps` every 0.25 s) records the process's cumulative CPU time, RSS and thread
count; every line the process prints is time-stamped, and `REPOGRAPH_TIMING=1` makes the writer
print one line per phase (`walked`, `extracted`, `settled`, `saved`), so one run is cut into walk,
parse, graph, save, model open and embed. **Peak** CPU is the busiest one-second window inside a
phase (100% = one core), **mean** is the phase's CPU time over its wall; a phase shorter than a
second reports its mean. `REPOGRAPH_NO_SERVE=1` and the level in `REPOGRAPH_RESOURCES` on every
row. Two scenarios:

- **build** — `.repograph/` removed, so the run walks, parses and settles the whole tree and then
  embeds every one of the 11,590 rows;
- **update** — a pristine copy of the store, then the declaring line of every third requirement
  and task node edited (1,011 nodes in 94 files), so the run re-reads those files and embeds
  1,011 rows; the tree is restored with hooks off afterwards, because the corpus's own
  post-checkout hook runs a repograph of its own over the store.

> **The machine was not quiet.** One-minute load averages ran 6–10 through the session against
> about two cores of foreign work this session could not stop — FortiClient's `epctrl` and
> Spotlight's `spotlightknowledged` at ~100% each. Every row is one run.

---

## 1 · Where `balanced` went past 30%

Before, `update`, per phase:

| phase | `low` peak (mean) | `balanced` peak (mean) | `full` peak (mean) | wall at `balanced` |
| --- | --- | --- | --- | --- |
| walk | —¹ | 101% | 59% | 0.10 s |
| parse | 79% | 126%¹ | —¹ | 0.18 s |
| graph + save | single-threaded, < 0.1 s | — | — | 0.05 s |
| model open | 59% (50%) | 143% | 161% | 0.46 s |
| **embed** | 201% (197%) | **394% (387%)** | 577% (533%) | 106.3 s |

¹ A phase of a fifth of a second spans one or two samples, so it reads nothing (—) or carries the
edge of the next phase; walk, parse, settle and save run on the main thread alone.

Only the embed goes over, and it goes over by being exactly as wide as the level says: **`n`
threads read as `n` cores.** No thread runs that `resources` does not count:

| thread | count at `balanced` before | bounded by |
| --- | --- | --- |
| main (the ORT caller) | 1 | — |
| ORT intra-op workers | 3 (`n - 1`; the caller is the `n`th) | `with_intra_threads(n)` |
| ORT inter-op pool | 0 — built only in parallel execution mode, never asked for | — |
| rayon global pool (`tokenizers::encode_batch`) | 4, busy only between forwards while the caller waits | `cap_pools(n)` |
| tokenizer load, beside the session load | 1, for the model open only | ends before the embed |
| `ignore` walker | 0 — `build()`, not `build_parallel()` | — |

Eight threads with four of them computing at a time; the samples read 8 for the length of every
`balanced` embed. The issue's 382% under `model` is the same four threads.

## 2 · What lowers it, decided by measuring

All four on the `update` scenario at `balanced`, minutes apart:

| shape | embed wall | peak CPU | mean CPU | peak RSS |
| --- | --- | --- | --- | --- |
| 4 threads, spin on (before) | 106.3 s | 394% | 387% | 1.21 GB |
| 4 threads, ORT spin-wait off | 106.0 s | 394% | 388% | 1.23 GB |
| **3 threads, spin on — shipped** | 135.6 s | **306%** | 300% | 1.17 GB |
| 3 threads, spin off | 136.7 s | 299% | 296% | 1.20 GB |

Spin-wait is not where the CPU goes: off, it changes neither the peak nor the wall at either count,
because the workers compute for the whole of every forward. That repeats 2026-09-07's H8 on a
different model, and it stays on. The count is the only lever, and the bar is a count of busy
cores: 30% of twelve is 3.6, so `balanced` is three threads, and the rule that says so on every
machine is `cores * 3 / 10`, one thread as the floor.

## 3 · After

`balanced` = 3 threads; `low` (2) and `full` (6) are unchanged, so their `build` rows were taken
once, with the before binary, which runs the same thread counts; their `update` rows were taken
with both binaries and agree within 2 s.

### 3.1 `build`, every row embedded

| phase | before `balanced` (4) | after `balanced` (3) | `low` (2) | `full` (6) |
| --- | --- | --- | --- | --- |
| walk | 0.14 s · 58% | 0.21 s · single-threaded | 0.17 s · 58% | 0.19 s · single-threaded |
| parse | 1.75 s · 106% (102%) | 1.77 s · 100% (96%) | 1.65 s · 101% (103%) | 1.71 s · 105% (101%) |
| graph + save | 0.05 s | 0.05 s | 0.05 s | 0.05 s |
| model open | 0.48 s · 212% | 0.53 s · 131% | 0.49 s · 144% | 0.47 s · 277% |
| **embed** | 518.6 s · **396% (390%)** | 671.4 s · **301% (296%)** | 967.6 s · 201% (199%) | 400.7 s · 580% (539%) |
| **whole wall** | **521.1 s** | **674.3 s (+29.4%)** | 970.2 s | 403.4 s |
| peak RSS | 1.36 GB | 1.40 GB | 1.33 GB | 1.35 GB |
| threads | 9 (8 in the embed) | 7 (6 in the embed) | 5 (4 in the embed) | 13 (12 in the embed) |

### 3.2 `update`, 1,011 rows owed

| level | threads | wall | embed peak (mean) | other phases' peak | peak RSS |
| --- | --- | --- | --- | --- | --- |
| `low` | 2 | 198.8 s (before 201.0 s) | 201% (198%) | 121% (model open) | 1.20 GB |
| `balanced`, before | 4 | 107.2 s | 394% (387%) | 143% (model open) | 1.21 GB |
| **`balanced`, after** | **3** | **139.2 s (+29.9%)** | **299% (296%)** | 157% (model open) | 1.24 GB |
| `full` | 6 | 83.4 s (before 83.6 s) | 577% (534%) | 232% (model open) | 1.20 GB |

### 3.3 The BM25 build

Not part of a writer: the first reader builds it (`Lexical::build`, rayon over the same capped
global pool), in 0.14 s on this store. Three `--no-dense ask` runs a level, `REPOGRAPH_TIMING`'s stage and
`/usr/bin/time`'s user over real for the whole process:

| level | `lexical built` | whole process user / real |
| --- | --- | --- |
| `low` | 175 ms | 0.31 / 0.22 s |
| `balanced`, before (4) | 128 ms | 0.32 / 0.18 s |
| `balanced`, after (3) | 140 ms | 0.31 / 0.19 s |
| `full` | 107 ms | 0.32 / 0.15 s |

Too short for a one-second window, and bounded by the pool — `n` cores — at every level.

### 3.4 The bars

| bar | measured | |
| --- | --- | --- |
| `balanced` peak CPU ≤ 360% in every phase of `build` | 301% (embed); every other phase ≤ 131% | met |
| `balanced` peak CPU ≤ 360% in every phase of `update` | 299% (embed); every other phase ≤ 157% | met |
| `low` < `balanced` < `full` | build 201% < 301% < 580%, and 970 s > 674 s > 403 s; update 201% < 299% < 577% | met |
| peak RSS recorded per phase, bounded or explained | §4 | explained |
| wall cost recorded | +29.4% `build`, +29.9% `update` at `balanced`; `docs/configuration.md#resources` | recorded |

## 4 · Memory

Peak RSS is 1.20–1.40 GB at every level and in both scenarios, and is three things:

| part | on the fixture | grows with |
| --- | --- | --- |
| the model's weights, memory-mapped (`Weights::Mapped`) | up to 1.23 GB of file pages, clean and reclaimable; 0.67 GB resident late in the `build` | the model, not the corpus |
| ORT's arena | bounded by the token budget (`TOKEN_BUDGET` padded tokens a forward) | nothing |
| the graph, the rows owed and the vectors written | the rest of a 0.34 GB anonymous footprint (`footprint`: 337 MB, peak 365 MB, late in the `build`) | the corpus |

`footprint` is what the system counts under memory pressure, and it is the anonymous part: 337 MB
on a 36 GB machine, under 1%. RSS adds the weights' file pages, which the system may drop and
re-read at will. The part that grows with the corpus is the corpus: the graph a writer must hold
whole to save it, the text of the rows it owes, and the vectors it appends — 11,590 × 768 × 4 bytes
is 36 MB. From 1,011 rows owed to 11,590, peak RSS moved 1.24 → 1.40 GB. No level changes any of
it, because none of it is per thread; the 30% bar is 10.8 GB here, and nothing on this corpus is
within an order of magnitude of it.
