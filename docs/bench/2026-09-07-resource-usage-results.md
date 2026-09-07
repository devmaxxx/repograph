# What every command costs, and how a rebuild was bounded

The report was two sentences: "when we build/rebuild graph it uses 90% resources of system and fan
out of memory", then "investigate all commands which can use a lot of resources". This file is
every command measured, which two of them were the cost, what the cause turned out to be, and what
the fix reads afterwards.

Machine: Apple Silicon, 12 logical cores = 6 performance + 6 efficiency (`hw.perflevel0.physicalcpu`
= 6), 36 GB. Corpus: a copy of the pinned `beauty-crm` fixture at `502e8a6d` — 908 files, 8,316
nodes, 32,601 edges, 7,408 passages (median 103 chars, p90 1,004, max 30,951), 26,117 generated
questions; 33,533 dense rows in the pristine index, 33,525 of them live, so a full re-embed writes
33,525.

`wall`, `user` and `max RSS` are `/usr/bin/time -l`'s; `CPU%` is the peak of one-second `top`
samples (100% = one core) with the average in brackets, and `threads` is the process's thread count
at that peak, with the running count after it where `top` reported one. Every row names a
transcript under `$G = /Users/max/bench/resources-2026-09-07`: the before rows in `$G/log-before`,
the after rows in `$G/log`, one `.time`, `.samples` and `.out` per run, and a `summary.txt`
carrying the line each table row was read from. Commands under a second have no samples; their
`user/wall` ratio is what says how parallel they were.

## 1. Before

### 1.1 The writers

| name | command | wall | user | max RSS | CPU% peak (avg) | threads |
|---|---|---|---|---|---|---|
| build-nodense | `build --no-dense` (wipe, re-extract 908 files, save 12 MB JSON + 10 MB mirror) | 1.91 s | 0.93 s | 0.07 GB | single-threaded | — |
| update-nodense | `update --no-dense`, nothing changed | 0.07 s | 0.04 s | 0.07 GB | — | — |
| build-small-vectors-kept | `build` with `REPOGRAPH_EMBED_MODEL=…-e5-small` on the small-model store: every hash matches, 0 rows embedded | 1.63 s | 1.50 s | 1.57 GB | — | — |
| embed-A-small-full | `embed`, small model, store copied without `vectors.*`: all 33,533 rows | 214.1 s | 1,051 s | 1.63 GB | 399% (≈330%) | 18, 6 running |
| embed-B-large-320longest | `embed`, large model, a store cut to the 320 longest passages (all ≥ 256 tokens): five forwards of 64×256 | 110.8 s | 627 s | **2.70 GB** | 402% (377%) | 18 |
| embed-C-small-320longest | the same 320 through the small model | 10.7 s | 55.9 s | 2.18 GB | 377% (309%) | 18 |
| embed-D-large-full | `embed`, **default (large) model, all 33,533 rows** — what `build`, `update`, `enrich`, `embed` and `watch` all do on a store whose rows are the small model's, or on any store after `embed_model` changed | **2,582.5 s (43 min)** | 13,342 s (5.2 cores for 43 min) | **2.96 GB** | 444% (372%) | 18, 6 running |
| enrich | not run: `--parallel` (default 8) headless `claude -p` processes cost tokens. repograph's own share is the graph and the questions, ≈ 0.1 GB; the eight children are the cost and are already bounded by the flag | — | — | — | — | — |

### 1.2 The readers

| name | command | wall | user | max RSS | CPU% peak | threads |
|---|---|---|---|---|---|---|
| ask-fused | `ask` (small store, fused) | 0.41 s | 0.79 s | 1.55 GB | — | — |
| ask-nodense | `ask --no-dense` | 0.09 s | 0.35 s | 0.10 GB | — | — |
| ask-stale | `ask --stale` (small) | 0.31 s | 0.72 s | 1.43 GB | — | — |
| ask-exact-id | `ask FR-PAY-22` (exact id: no model, no vectors) | 0.05 s | 0.02 s | 0.05 GB | — | — |
| ask-rerank-local | `ask --rerank-local` (small store; 200 candidates through the 2.1 GB reranker) | **44.2 s** | 158 s | **3.13 GB** | 392% | 23 |
| ask-large-stale | `ask --stale` on the e5-large store | 1.18 s | 1.73 s | 1.74 GB | — | — |
| ask-large-stale-rerank-local | the same with `--rerank-local` | 42.1 s | 179 s | **3.38 GB** | 326% | 23 |
| serve | resident, small store, after two fused asks through the socket; idle | — | — | 1.39 GB (85 MB before the first fused ask) | 0% idle | 19 |
| watch | `--every 2`, small model pinned; quiet poll / after one doc edit (1 row embedded) | — | — | 0.04 GB / 1.37 GB | 1.8% | 18 |
| bench-nodense | `bench --no-dense` (82 cases) | 0.17 s | 0.43 s | 0.20 GB | — | — |
| bench-dense | `bench` (small) | 1.53 s | 2.34 s | 1.54 GB | — | — |
| bench-large | `bench` on the e5-large store (82 queries through the large model) | 4.81 s | 12.3 s | 1.74 GB | 216% | 18 |
| dump10 | `dump --queries 10 lines --depth 300` (small, dense) | 0.68 s | 1.07 s | 1.36 GB | — | — |
| dump10-nodense | the same `--no-dense` | 0.12 s | 0.36 s | 0.20 GB | — | — |
| impact / trace | `impact cn --depth 3` (81 callers) / `trace main cn --depth 6` | 0.04 s | 0.02 s | 0.05 GB | — | — |
| changes | `changes --depth 2` over a one-hunk diff (walk + refresh + git) | 0.14 s | 0.06 s | 0.07 GB | — | — |
| import-legacy | the 41 MB graphify `graph.json` | 0.13 s | 0.10 s | 0.18 GB | — | — |
| verify / explain | | 0.03 s | 0.02 s | 0.04 GB | — | — |

Everything but the two embedding rows and `--rerank-local` is under a second and under 0.2 GB, or
is a resident process holding the model on purpose. Walk, tree-sitter, BM25, the store: 1.9 s and
70 MB for the whole tree. There was nothing to win there and nothing was changed there.

### 1.3 The causes

**The CPU.** During any run that embeds, the process held 18 threads: the main thread, five ORT
intra-op workers, and rayon's twelve, which `tokenizers::encode_batch` spawns through
`into_maybe_par_iter`. Six ran at once and the process sat at 370–440% for the whole run (D: 1,514
one-second samples averaging 372%, peak 444%, six threads running in 1,073 of them), never higher:
ORT had sized its pool to the six performance cores. On a machine with no efficiency cores that
default is every physical core, which is what "90% of the system" looks like on the CPU-history
graph. The cause was one line — `Session::builder()` in `src/index/embed.rs` and again in
`src/index/cross.rs`, committed with no thread count.

**The memory.** The rows are not it: 33,533 texts are ≈ 5 MB and their 1024-d vectors 2 × 137 MB.
The peak is the session — 2.24 GB of memory-mapped fp32 weights, so a one-query `ask` touches only
the embedding rows its tokens need (1.74 GB) while a full embed walks most of the 1 GB vocabulary —
plus the arena the runtime grows for the largest input shape it has seen. At 64 × 256 tokens the
attention scores alone are 64·16·256·256·4 B = 268 MB per layer and the FFN intermediate another
268 MB, and the arena keeps whatever it grew to: 2.70 GB for five such forwards (B) against 2.18 GB
on a model whose weights are 0.45 GB (C) — the arena, not the weights, sets the peak. D's
trajectory is the arena's: 1.63 GB one second in, 1.81 GB at 14 s, 2.15 GB at 9 min, 2.64 GB at
14 min as the length-sorted batches crossed into longer shapes, 2.96 GB by the tail. On 36 GB none
of that is an out-of-memory; on a 16 GB laptop with an IDE and a browser open it is the swap the
report describes. The batching decided the shape: batches were sixty-four *texts* sorted by *byte*
length, so the longest sixty-four passages of the corpus always formed one 16,384-token forward
whatever the machine had.

**The immediate workaround, then and now, with no code at all:**
`embed_model = "intfloat/multilingual-e5-small"` in `repograph.toml` (214 s and 1.63 GB for the
whole store instead of 43 minutes and 2.96 GB), or `--no-dense` on the writer.

## 2. The levers, measured before they were designed

A 120-line harness (`$G/harness`) pinning the same `ort` and `tokenizers` versions, with
repograph's tokenize → forward → pool loop and four switches as arguments, over the same 320
longest passages as B, large model:

| run | threads | cap × budget | mem pattern | spin | wall | user | max RSS | CPU% peak (avg) | threads |
|---|---|---|---|---|---|---|---|---|---|
| H0 (control: the shape as shipped) | ORT default (6) | 64 × none → 5 forwards of 64×256 | on | on | 106.9 s | 613 s | 3.63 GB | 426% (414%) | 18 |
| H1 | 4 | 64 × none | on | on | 148.7 s (+39%) | 588 s | 3.62 GB | 294% | 16 |
| H2 | 3 | 64 × none | on | on | 205.3 s (+92%) | 573 s | 3.62 GB | 224% | 15 |
| H3 | 2 | 64 × none | on | on | 292.0 s (+173%) | 559 s | 3.62 GB | 152% | 14 |
| H4 | default | 64 × 8,192 → 32×256 | on | on | 103.9 s (−3%) | 592 s | 2.65 GB | 435% | 18 |
| H5 | default | 64 × 4,096 → 16×256 | on | on | 103.9 s (−3%) | 583 s | 2.17 GB | 431% (414%) | 18 |
| H6 | default | 64 × 2,048 → 40 forwards of 8×256 | on | on | 103.8 s (−3%) | 574 s | **1.93 GB** | 429% (409%) | 18 |
| H7 | default | 64 × none | **off** | on | 119.2 s (+11%) | 639 s | 2.72 GB | 429% | 18 |
| H8 | default | 64 × none | on | **off** | 129.7 s (+21%) | 650 s | 3.62 GB | 432% (364%) | 18 |
| H9 (the combination that shipped) | 4 | 64 × 2,048 | on | on | 134.1 s (+25%) | 531 s | **1.93 GB** | **293%** | 16 |
| H9-full: the same over all 33,525 rows | 4 | 64 × 2,048 | on | on | **1,821 s** (D: 2,582) | **7,214 s** (D: 13,342) | **2.11 GB** (D: 2.96) | **294%** (D: 444%) | 16 (D: 18) |

Reading it:

1. **The token budget is free.** 8 × 256 forwards ran the whole set in the same 104 s as 64 × 256
   while max RSS fell from 3.63 GB to 1.93 GB, within 0.2 GB of the weights the run touches. The
   pool is saturated by a 2,048 × 1,024 GEMM already; every larger batch buys arena and nothing
   else.
2. **The thread cap is a straight trade.** User time is flat (613 → 559 s), so nothing is lost to
   spin: wall grows as the cap shrinks, and four threads is −31% peak CPU for +39% wall.
3. **Memory-pattern off and spin-control off** each cost wall (+11%, +21%) for less than the budget
   gives: both rejected.
4. The harness reads ~0.9 GB above repograph on identical shapes (3.63 vs 2.70 GB; it tokenises all
   320 texts in one parallel pass before the forwards, repograph one batch at a time), so its rows
   compare against each other and never against §1. The after numbers are re-measured in repograph.
5. **The full store under the combination is faster than before, not slower**: 30 minutes against
   43, with 46% less CPU time, 0.85 GB less memory and a third less peak CPU. The wall win is
   padding — sorting by tokens and closing on a budget spends the quadratic attention on real
   tokens rather than on padding — so the thread cap's +39% is paid out of a −46%.

## 3. What shipped

| lever | verdict |
|---|---|
| `threads` config key capping `with_intra_threads` on both sessions | **shipped** — `repograph.toml`, the machine file, `REPOGRAPH_THREADS`; `0` = a third of the logical cores |
| rayon's global pool capped to the same number | **shipped** — one number bounds every pool the binary owns |
| token-budgeted batches (`TOKEN_BUDGET` = 2,048 padded tokens, `BATCH` = 64 kept) | **shipped** |
| `DenseIndex::sync_chunked` — a checkpoint and a progress line every 1,024 rows | **shipped** |
| `with_inter_threads` / parallel execution | rejected: inter-op threads exist only in parallel execution mode, which is off, and cost memory for a graph with no parallel branches |
| `allow_spinning = 0` | rejected: H8, +21% wall for −12% average CPU |
| shrinking `BATCH` alone | rejected: a count bounds nothing (16 ten-token labels and 16 truncated passages are 160 and 4,096 tokens) |
| `with_memory_pattern(false)` | rejected: H7, −0.9 GB for +11% wall, where the budget takes −1.7 GB for nothing |
| arena extend strategy | rejected: `ort` rc.13 exposes `OrtArenaCfg` only for the CANN and MIGraphX providers |
| bounding `enrich --parallel` | rejected: already bounded by the flag, and the cost is in the eight `claude` children |
| walk / parse / lexical / store | rejected: 1.9 s and 70 MB for the whole tree |
| `--rerank-local`'s 44 s and 3.4 GB | out of scope; the reranker's session takes the same cap, and a candidate depth below 200 is a retrieval question |

Nothing in the store format, the CLI flags or the socket protocol moved. A `repograph.toml` without
`threads` loads exactly as before.

## 4. After

### 4.1 The rebuild

The same store copy, the same sampler, the same command — `embed` over a pristine copy with
`vectors.*` removed, default (large) model, 33,525 rows.

| run | wall | user | max RSS | CPU% peak (avg) | threads/running |
|---|---|---|---|---|---|
| `embed-D-large-full`, before | 2,582.5 s (43 min) | 13,342 s | 2.96 GB | 444% (372%) | 18 / 6 |
| `embed-D2-large-full-after` | **1,930.2 s (32 min)** | **7,641 s** | **2.15 GB** | **293.3% (288.9%)** | **8 / 4** |
| the target | ≤ 2,000 s | — | ≤ 2.3 GB | ≤ 310% (≤ 300%) | ≤ 9 |

Every fixed number is met: 25% off the wall, 43% off the CPU time, 27% off the peak memory, a
third off the peak CPU, and ten threads fewer. The two cores the cap hands back are handed back for
the whole 32 minutes, and the run still ends eleven minutes sooner than the one that took them —
the padding the token budget stops paying for is worth more than the cores it gave up, exactly as
H9-full predicted (1,821 s there, 1,930 s here, on a harness that runs a little leaner than the
binary).

The one bar not met is the progress cadence. Thirty-three lines are printed over the run where
there were none; the first lands at **102.4 s** and the thirty-two after it are **42.7 s to 96.7 s
apart, mean 57.0**, against a target of one at least every 60 s. Neither extreme is a fault in the
chunking: the first chunk carries the run's fixed startup — the 2.24 GB of weights are paged in
from the mmap as the first forwards touch them, which is also why chunks two and three are still
decaying (60.1 s, 46.4 s) — and the last chunks are slow because a chunk is a count of rows, and
the rows the graph's iteration order puts last are the long passages: 96.7 s for the 32,768th
thousand. Bounding a chunk by tokens rather than by rows would even it out, and it is a design
change with its own measurement to earn, so `SYNC_CHUNK` stays the 1,024 the plan set.

### 4.2 The overrides

The 320 longest passages, large model, vectors removed before each run
(`$G/log/t1-summary.txt`, `$G/log/summary.txt`):

| run | wall | user | max RSS | CPU% peak (avg) | threads/running |
|---|---|---|---|---|---|
| `embed-B-large-320longest`, before | 110.8 s | 627.0 s | 2.70 GB | 401.6% (377%) | 18 / 6 |
| `REPOGRAPH_THREADS=2` | 268.8 s | 536.1 s | 2.69 GB | 148.1% (145.7%) | 4 / 2 |
| `threads = 2` in `repograph.toml` | 269.2 s | 537.0 s | 2.69 GB | 148.5% (145.6%) | 4 / 2 |
| `threads = 6` in `repograph.toml` | 103.4 s | 597.9 s | 2.70 GB | 434.3% (411.4%) | 12 / 6 |
| `t2-embed-320`: the defaults, cap and budget together | 133.3 s | 525.1 s | **1.82 GB** | 292.6% (288.4%) | 8 / 4 |

The environment and the project file read the same to the second. `threads = 6` is the one word
back to the pool ORT would have chosen itself — and it is *not* the 18 threads of the before row,
because rayon's twelve are capped by the same number: 12 threads, six running, at the same 400-odd
per cent. The last row is what a run with no configuration at all does: a third of the peak CPU and
a third of the memory of the before row, for 20% more wall on a set of texts that is all 256-token
passages — the worst case for the budget, since there is no padding to win back.

### 4.3 The checkpoint

Killed mid-embed and started again, the whole store under the small model
(`$G/log/t3-interrupt.txt`). The 320-row copy is a single chunk and can never show a checkpoint, so
the whole store is what this is run on; `kill -TERM` rather than the plan's `kill -INT`, because a
background child of a non-interactive shell inherits `SIG_IGN` for SIGINT and to a process with no
handler for either the two are the same event.

```
dense: 1024/33525 rows, 121.2 rows/s, ~4 min left      first run, 23 chunks in when it was killed
dense: 23552/33525 rows, 233.4 rows/s, ~1 min left
                                                       23,552 rows on disk afterwards
dense: 1024/9973 rows, 222.8 rows/s, ~1 min left       second run: 9,973 rows, the exact remainder
dense: embedded 9973 rows in 57.4s
```

The resumed store holds 33,525 rows, no holes, the model recorded, and answers
`штраф за отмену записи` with `FR-CAL-95`, `FR-TOOL-35`, `N-025`, `FR-PAY-26`, `FR-DM-48`.

And the same on the default model, killed at minute five (`$G/log/t4-interrupt.txt`):

```
dense: 1024/33525 rows, 10.1 rows/s, ~54 min left      four checkpoints in five minutes
dense: 4096/33525 rows, 16.1 rows/s, ~31 min left
                                                       4,096 rows on disk, the model recorded
dense: 1024/29429 rows, 19.7 rows/s, ~24 min left      the second run has 29,429 left, not 33,525
```

The second run's *total* is the claim: 29,429 = 33,525 − 4,096, so the five minutes were kept. It
was not run to the end here — the small-model transcript above already carries an end-to-end
resume, and twenty-five more minutes of the large model would only restate it.

### 4.4 The readers, unchanged

Every reader re-run on the same fixture copy (`$G/quick.sh`, transcripts in `$G/log`), against
its §1.2 row:

| run | wall before → after | max RSS before → after | peak CPU before → after |
|---|---|---|---|
| ask-fused | 0.41 → 0.35 s | 1.55 → 1.36 GB | — |
| ask-nodense | 0.09 → 0.12 s | 0.10 → 0.10 GB | — |
| ask-stale | 0.31 → 0.28 s | 1.43 → 1.36 GB | — |
| ask-exact-id | 0.05 → 0.04 s | 0.05 → 0.05 GB | — |
| ask-rerank-local | **44.2 → 31.9 s** | 3.13 → 3.09 GB | **391.5% → 292.2%** |
| impact / trace | 0.04 → 0.04 s | 0.05 → 0.05 GB | — |
| changes | 0.14 → 0.07 s | 0.07 → 0.07 GB | — |
| bench-nodense | 0.17 → 0.19 s | 0.20 → 0.20 GB | — |
| bench-dense | 1.53 → 1.47 s | 1.54 → 1.42 GB | — |
| dump10 | 0.68 → 0.70 s | 1.36 → 1.36 GB | — |
| import-legacy | 0.13 → 0.14 s | 0.18 → 0.15 GB | — |
| serve, idle after two fused asks | — | 1.39 → 1.38 GB | — |
| watch, quiet / after one edit | — | 0.04 / 1.37 → 0.05 / 1.37 GB | — |

Nothing costs more. `--rerank-local` is the one that moved: **28% faster on a third less CPU**,
because four threads through a 16 × 80-token batch contend for less than six do. The one-shot
commands that read as slower — `ask --no-dense` at +0.03 s, `bench --no-dense` at +0.02 s — are
sub-tenth-of-a-second commands whose jitter is larger than the difference.

Three readings in the suite run needed a second look and none was this change. `changes` read
1.11 s in the sequence and 0.07 s on its own three times over (`$G/log/changes-retry-*.time`): in
the suite it was the run that found the store stale and paid the re-extract and the 22 MB save.
`serve` read 1.59 GB and `watch` 1.55 GB there, both about 0.2 GB above §1.2 — and both come back
at 1.38 GB and 1.37 GB when the store is settled and each does the same work its before row did
(`$G/log/serve2.*`, `$G/log/watch2.*`). What raised them was a resident process refreshing and
re-embedding mid-measurement, which the before run happened not to do; the one-shot rows above are
`/usr/bin/time -l`'s and carry no such ordering.

### 4.5 The floors

**The bench floors did not move.** `ARMS="dense lexical" bench/history/run-repograph.sh` against
the pinned fixture, exit 0, both arms green and gated, both lines identical to the `502e8a6d` rows
already in `bench/history/runs.jsonl`:

```
keyword 40/40  paraphrase 15/30  code 12/12  p90 221 tok  dense=true   enriched=true  model=small  gated=true
keyword 39/40  paraphrase 15/30  code 12/12  p90 215 tok  dense=false  enriched=true  model=small  gated=true
```

The fixture's rows are the small model's and were not re-embedded, so the dense line could not have
moved through the batching; it is run because `Lexical::build` now runs under a capped rayon pool.

**And the re-embedded rows hold their floors too.** The token budget changes the padding, so a
store re-embedded under it has vectors that differ at float precision. The e5-large store was
copied, its `vectors.*` removed, embedded again (33,525 rows in 1,918.1 s) and benched
(`$G/log/t4-e5large2.txt`), against the pre-change reading in
`~/bench/gaps-2026-09-05/t5-large-enriched-1.txt`:

```
before  keyword 40/40  paraphrase 22/30  code 12/12  p90 224 tok  model=large  gated=true
after   keyword 40/40  paraphrase 22/30  code 12/12  p90 224 tok  model=large  gated=true
```

Case for case and token for token, exit 0. The padding was never part of the answer.

## 5. Side findings, not fixed here

- `serve` cannot bind when `<repo>/.repograph/serve.sock` exceeds `SUN_LEN` (104 bytes on macOS);
  the error names the cause. A repo deep under `/private/tmp/…` cannot use `serve` at all. Worth
  its own issue: bind a shorter socket in `$TMPDIR` keyed by a hash of the canonical path, or say
  so in the README.
- A `serve` killed with SIGTERM leaves `serve.sock` behind (`Unlink` is a `Drop`, and there is no
  signal handler). The next `serve` removes a dead one before binding and a client's connect just
  fails, so it is harmless, but `--idle` is the only clean exit.
- The fixture's `manifest.json` stamps were whole-second mtimes after its restore; the first reader
  rewrote them to nanosecond ones. Content unchanged.
