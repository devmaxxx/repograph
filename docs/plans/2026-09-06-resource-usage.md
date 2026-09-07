# Resource usage of every command — measurements, causes, and the plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:executing-plans (or superpowers:subagent-driven-development) to implement this task-by-task. Steps use checkbox (`- [ ]`) syntax. Every number in this file came from a run named in §1; a number you write comes from a run you name.

**Goal:** a `build`, `update`, `embed`, `enrich` or `watch` that re-embeds a store no longer takes every performance core for three quarters of an hour with nothing on the terminal, and its peak memory is bounded by a documented budget rather than by the batch the longest passages happen to fall in. Every other command is measured and left alone, with the numbers that say why.

**Report being answered:** "when we build/rebuild graph it uses 90% resources of system and fan out of memory", then "investigate all commands which can use a lot of resources".

**Architecture:** repograph is a Rust CLI (`src/`) over a `.repograph/` store. The writers (`build`, `update`, `enrich`, `embed`, `watch`) walk the tree, extract, save the graph, then `embed_all` → `DenseIndex::sync` → `Embedder::embed` over every row whose hash is new. The embedder is an ONNX Runtime session (`ort` 2.0.0-rc.13, pyke's static binaries, no OpenMP) opened with no thread configuration; its intra-op pool is sized by ORT to the performance-core count and spins between operators. Texts are batched 64 at a time by byte length and padded to the batch's longest member (`length_batches`, `BATCH = 64`, `MAX_TOKENS = 256`). The readers (`ask`, `serve`, `bench`, `dump`) open the same session for one query at a time; `--rerank-local` opens a second, 2.1 GB session beside it.

**Tech Stack:** Rust 2021 pinned to 1.98.0 (`rust-toolchain.toml`), `cargo test --release`, `cargo clippy --release --all-targets -- -D warnings`; bash 3.2; `/usr/bin/time -l` and `top -l 2 -pid` for the measurements; no new dependencies (`rayon` is already a direct dependency).

## Global constraints

- Work in this worktree: `W=/Users/max/Documents/projects/repograph/.claude/worktrees/graph-build-resource-usage-b554bc`, branch `claude/graph-build-resource-usage-b554bc`. Never `cd` into the main checkout; use `cargo … --manifest-path "$W/Cargo.toml"` and absolute paths. `B="$W/target/release/repograph"`, rebuilt before any measurement.
- The fixture `F=/Users/max/bench/beauty-crm-502e8a6d` is read-only: `ask --stale`, `bench`, `dump` only. Every writer runs on a copy. A copy with no `repograph.toml` re-embeds its rows whole under the default (large) model on the first writer — that is the reproduction, and also the trap: pin `embed_model = "intfloat/multilingual-e5-small"` in a copy's `repograph.toml` whenever the large model is not the thing being measured.
- Scratch for this work: `S=/private/tmp/claude-502/-Users-max-Documents-projects-repograph--claude-worktrees-graph-build-resource-usage-b554bc/d854dd4b-1b8c-403b-a458-5463278607fa/scratchpad` holds the plan phase's copies and logs (`$S/log/summary.txt`, one `.time`/`.samples`/`.out` per run, `$S/measure.sh`, the harness under `$S/harness`). A reboot removes it; the build phase copies what it reuses to `$G=/Users/max/bench/resources-2026-09-07` first (Task 0).
- Comments say why, never what; no ticket ids in code; tool directives stay. Test names are `snake_case` sentences in the style of the file they join. Commit subjects are Conventional (`perf(embed): …`, `feat(config): …`, `docs: …`); no AI trailers; a heredoc and a `git commit` go in two shell commands.
- No model tokens are spent: `enrich` is measured with a stub command only.
- Measurements export `REPOGRAPH_NO_SERVE=1` and `REPOGRAPH_EMBED_MODEL`; `cargo test` must run in a shell where neither is set (§3), and a writer on a copy must have the model it is meant to measure pinned by file, not by an inherited variable.

---

## 1. Measurements

Machine: Apple Silicon, 12 logical cores = 6 performance + 6 efficiency (`hw.perflevel0.physicalcpu` = 6), 36 GB. Corpus: the fixture copy — 908 files, 8,316 nodes, 32,601 edges, 7,408 passages (median 103 chars, p90 1,004, max 30,951), 26,117 generated questions; 33,533 dense rows in the pristine index, 33,525 of them live (8 holes), so a full re-embed writes 33,525. `wall`/`user` from `/usr/bin/time -l`, `max RSS` its "maximum resident set size", `CPU%` the peak of one-second `top` samples (100% = one core), `threads` the process's thread count at that peak. Commands under a second have no samples; their `user/wall` ratio says how parallel they were. Files: `$S/log/<name>.{time,samples,out}`.

### 1.1 The writers

| name | command | wall | user | max RSS | CPU% peak (avg) | threads |
|---|---|---|---|---|---|---|
| build-nodense | `build --no-dense` (wipe, re-extract 908 files, save 12 MB JSON + 10 MB mirror) | 1.91 s | 0.93 s | 0.07 GB | single-threaded | — |
| update-nodense | `update --no-dense`, nothing changed | 0.07 s | 0.04 s | 0.07 GB | — | — |
| build-small-vectors-kept | `build` with `REPOGRAPH_EMBED_MODEL=…-e5-small` on the small-model store: every hash matches, 0 rows embedded | 1.63 s | 1.50 s | 1.57 GB | — | — |
| embed-A-small-full | `embed`, small model, store copied without `vectors.*`: all 33,533 rows | 214.1 s | 1,051 s | 1.63 GB | 399% (≈330%) | 18, 6 running |
| embed-B-large-320longest | `embed`, large model, a store cut to the 320 longest passages (all ≥ 256 tokens): five forwards of 64×256 | 110.8 s | 627 s | **2.70 GB** | 402% (377%) | 18, 6 running |
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
| ask-large-stale | `ask --stale` on the e5-large store (`~/bench/gaps-2026-09-05/e5large`) | 1.18 s | 1.73 s | 1.74 GB | — | — |
| ask-large-stale-rerank-local | the same with `--rerank-local` | 42.1 s | 179 s | **3.38 GB** | 326% | 23 |
| serve | resident, small store, after two fused asks through the socket; idle | — | — | 1.39 GB (85 MB before the first fused ask) | 0% idle | 19 |
| watch | `--every 2`, small model pinned; quiet poll / after one doc edit (1 row embedded) | — | — | 0.04 GB / 1.37 GB | 1.8% | 18 |
| bench-nodense | `bench --no-dense` (82 cases) | 0.17 s | 0.43 s | 0.20 GB | — | — |
| bench-dense | `bench` (small) | 1.53 s | 2.34 s | 1.54 GB | — | — |
| bench-large | `bench` on the e5-large store (82 queries through the large model; `40/40 22/30 12/12 p90 224 gated=true`) | 4.81 s | 12.3 s | 1.74 GB | 216% | 18 |
| dump10 | `dump --queries 10 lines --depth 300` (small, dense) | 0.68 s | 1.07 s | 1.36 GB | — | — |
| dump10-nodense | the same `--no-dense` | 0.12 s | 0.36 s | 0.20 GB | — | — |
| impact / trace | `impact cn --depth 3` (81 callers) / `trace main cn --depth 6` | 0.04 s | 0.02 s | 0.05 GB | — | — |
| changes | `changes --depth 2` over a one-hunk diff (walk + refresh + git) | 0.14 s | 0.06 s | 0.07 GB | — | — |
| import-legacy | the 41 MB graphify `graph.json` | 0.13 s | 0.10 s | 0.18 GB | — | — |
| verify / explain | | 0.03 s | 0.02 s | 0.04 GB | — | — |

`serve` could not be measured on the scratch copy: its path exceeds `SUN_LEN` (104 bytes on macOS) and `bind` fails with `path must be shorter than SUN_LEN` — a side finding (§6), measured on the fixture instead. Two readers of the fixture wrote nothing but `manifest.json`'s stamps (every `ask` on it does the same since its restore truncated mtimes to whole seconds; hashes unchanged, verified against the morning's copy).

### 1.3 What the numbers say — the causes

**The CPU.** During any run that embeds, the process holds 18 threads: the main thread, five ORT intra-op workers, and rayon's twelve, which `tokenizers::encode_batch` spawns through `into_maybe_par_iter` (gated by `TOKENIZERS_PARALLELISM`). Six threads run at once and the process sits at 370–440% for the whole run (A: 121 samples averaging 330%; D: 1,514 one-second samples averaging 372%, peak 444%, six threads running in 1,073 of them), never higher: ORT sized its pool to the six performance cores, and the workers are genuinely busy — the harness's H8 (§1.4) shows spin-waiting between operators is not where the time goes, since user time barely moves when spinning is turned off. Nothing else in the binary is close: `build --no-dense` — walk, blake3, tree-sitter over 908 files, the graph save — is 1.9 s of one core, `Lexical::build`'s `par_iter` runs for tens of milliseconds, `store.save` writes 22 MB once. On a machine without efficiency cores ORT's default is every physical core; here it is every performance core, which is what "90% of the system" looks like on the CPU-history graph, with the fans following. The cause is one line: `Session::builder()` in `src/index/embed.rs:128` (and `src/index/cross.rs:77`) sets no thread count and lets ORT choose.

**The memory.** The rows themselves are not it: `todo_texts` for 33,533 rows is ≈ 5 MB, the returned `vecs` plus the copy `sync` appends to `self.vectors` are 2 × 137 MB for 1024-d rows. The peak is the ORT session: 2.24 GB of fp32 weights (`model.onnx_data`, memory-mapped, so a one-query `ask` touches only the embedding rows its tokens need — 1.74 GB max RSS — while a full embed touches most of the 1 GB vocabulary table) plus the arena the runtime grows for the largest input shape. At 64 × 256 tokens that shape's attention scores alone are 64·16·256·256·4 B = 268 MB per layer, the FFN intermediate another 268 MB, and the arena keeps whatever it grew to: **2.70 GB** for five such forwards on the large model (B), and 2.18 GB on the small model whose weights are 0.45 GB (C) — the arena, not the weights, sets the peak, and it overshoots when it starts cold on big shapes (A, which reached the same 64 × 256 tail after thousands of small batches, stayed at 1.63 GB). The full large run measured **2.96 GB**, and its trajectory is the arena's: 1.63 GB one second in (weights mapped, session open), 1.81 GB at 14 s, 2.15 GB at 9 min, 2.64 GB at 14 min as the length-sorted batches crossed into longer shapes, 2.96 GB by the tail — each step a new largest shape, none of it ever released. On this 36 GB machine none of that is an out-of-memory; on a 16 GB laptop with an IDE and a browser open, 3–3.5 GB of resident anonymous memory that stays for 45 minutes is the swap-and-kill the report describes, and `ask --rerank-local` reaches the same 3.1–3.4 GB in one query. The batching decides the shape: `length_batches` (`embed.rs:220`) counts texts, not tokens, so the longest sixty-four passages of the corpus always form one 16,384-token forward whatever the machine has.

**What is not a cause.** Walk, parse, lexical, store, `impact`, `trace`, `changes`, `import-legacy`, `verify`, `explain`: all under 0.2 s and 0.2 GB. `enrich`'s cost is its eight `claude` children, already bounded by `--parallel`. `serve`/`watch` hold the model by design (1.4 GB small, ≈1.8 GB large) and are idle otherwise.

**The immediate workaround, no code:** `embed_model = "intfloat/multilingual-e5-small"` in `repograph.toml` (214 s and 1.63 GB for the whole store instead of 43 minutes and 2.96 GB), or `--no-dense` on the writer to skip the embedder altogether.

### 1.4 The levers, measured before they are designed

`$S/harness` is a 120-line Cargo project pinning the same `ort`/`tokenizers` versions and sharing `$W/target` (so the ORT binaries are reused), with repograph's tokenize → forward → pool loop and four switches as arguments: intra-op threads, a texts-per-forward cap, a padded-token budget per forward, memory-pattern and spin-wait on/off. Run over the same 320 longest passages as B, large model, `/usr/bin/time -l` and the sampler around it (`$S/log/H*.{time,samples}`):

| run | threads | cap × budget | mem pattern | spin | wall | user | max RSS | CPU% peak (avg) | threads |
|---|---|---|---|---|---|---|---|---|---|
| H0 (control: today's shape) | ORT default (6) | 64 × none → 5 forwards of 64×256 | on | on | 106.9 s | 613 s | 3.63 GB | 426% (414%) | 18 |
| H1 | 4 | 64 × none | on | on | 148.7 s (+39%) | 588 s | 3.62 GB | 294% | 16 |
| H2 | 3 | 64 × none | on | on | 205.3 s (+92%) | 573 s | 3.62 GB | 224% | 15 |
| H3 | 2 | 64 × none | on | on | 292.0 s (+173%) | 559 s | 3.62 GB | 152% | 14 |
| H4 | default | 64 × 8,192 → 32×256 | on | on | 103.9 s (−3%) | 592 s | 2.65 GB | 435% | 18 |
| H5 | default | 64 × 4,096 → 16×256 | on | on | 103.9 s (−3%) | 583 s | 2.17 GB | 431% (414%) | 18 |
| H6 | default | 64 × 2,048 → 40 forwards of 8×256 | on | on | 103.8 s (−3%) | 574 s | **1.93 GB** | 429% (409%) | 18 |
| H7 | default | 64 × none | **off** | on | 119.2 s (+11%) | 639 s | 2.72 GB | 429% | 18 |
| H8 | default | 64 × none | on | **off** | 129.7 s (+21%) | 650 s | 3.62 GB | 432% (364%) | 18 |
| H9 (the shipped combination) | 4 | 64 × 2,048 | on | on | 134.1 s (+25%) | 531 s | **1.93 GB** | **293%** | 16 |
| H9-full: the same over all 33,525 rows (`texts-all.json`, the store's passages and questions) | 4 | 64 × 2,048 | on | on | **1,821 s (30 min; D: 2,582)** | **7,214 s (D: 13,342)** | **2.11 GB (D: 2.96)** | **294% (D: 444%)** | 16 (D: 18) |

Reading it: (1) **the token budget is free** — 8 × 256 forwards run the whole set in the same 104 s as 64 × 256 while max RSS falls from 3.63 GB to 1.93 GB, within 0.2 GB of the weights the run touches; the pool is saturated by a 2,048 × 1,024 GEMM already, and every larger batch only buys arena. (2) **The thread cap is a straight trade** — user time is flat (613 → 559 s) so nothing is wasted in spin, and wall grows as the cap shrinks: 4 threads is −31% peak CPU for +39% wall. (3) Memory-pattern off and spin-control off each cost wall (+11%, +21%) for less than the budget buys: rejected. (4) The harness reads 0.9 GB above repograph's own B on the identical shapes (3.63 vs 2.70 GB; it tokenises all 320 texts in one parallel pass before the forwards, repograph one batch at a time), so its rows compare against each other, not against §1.1 — the after-fix number is re-measured in repograph (§4). (5) **The full store under the shipped combination is faster than today on four threads, not slower**: 30 minutes against 43, with 46% less CPU time, 0.85 GB less memory and a third less peak CPU. The wall win is padding: today's batches are sorted by byte length, which interleaves Cyrillic and Latin texts of very different token counts and pads sixty-four of them to the longest; sorting by tokens and closing on a budget (776 forwards, most of them 64 × ~20-token questions and 8 × 256 passages) means the attention that is quadratic in the padded length is spent on real tokens. So the thread cap's +39% (reading 2) is paid out of a −46%, and the rebuild still ends sooner.

---

## 2. Design

### 2.1 Candidates, accepted and rejected

| lever | verdict | reason |
|---|---|---|
| Cap ORT intra-op threads (`SessionBuilder::with_intra_threads`) with a config key and an env override | **accept** | the whole CPU story is the pool's size; the cap is one builder call, and both sessions (embedder, reranker) go through it. ORT's build here is static and OpenMP-free (no `omp_*`/`GOMP_*` symbols in the binary), so the call takes effect |
| `with_inter_threads` / `with_parallel_execution(true)` | **reject** | inter-op threads only exist in parallel execution mode, which is off (`ORT_SEQUENTIAL`) and costs memory ("at the cost of higher memory usage" — ort's own doc); a BERT graph has no parallel branches worth it |
| `session.intra_op.allow_spinning = 0` | **reject** | H8: +21% wall and +6% user for −12% average CPU; the workers are busy, not waiting, and the cap is the cheaper way to give cores back |
| Token-budgeted batches (close a batch on count × longest ≤ budget) instead of 64 texts | **accept** | bounds the arena's largest shape on every machine and corpus at no throughput cost (H4–H6: same wall, −1.7 GB); the sort by length already exists, so it is one pure function replacing another. Budget 2,048 from §1.4 |
| Shrink `BATCH` alone | **reject** | a count bounds nothing: 16 ten-token labels and 16 truncated passages are 160 and 4,096 tokens |
| `with_memory_pattern(false)` | **reject** | H7: −0.9 GB but +11% wall; the budget takes −1.7 GB for nothing, so this would only be paying twice |
| Arena extend strategy / `OrtArenaCfg` | **reject** | ort rc.13 exposes it only for the CANN and MIGraphX providers; the CPU allocator's arena is not reachable from the crate without raw `ort_sys` calls |
| Make rayon's global pool honour the same cap (`ThreadPoolBuilder::num_threads(n).build_global()` once in `main`) | **accept, cheap** | the twelve tokenizer threads are the other pool the binary owns; one number bounding every pool is what a `threads` key promises. Idle rayon threads cost nothing measurable, so this is for the promise, not for a number |
| Stream `sync`: embed in chunks, checkpoint after each, print progress | **accept as its own task** | not a memory lever (2 × 137 MB) but the durability one: today a 45-minute run killed at minute 40 — the report's "gets killed" — restarts from zero, and prints nothing while it runs. Chunking makes both a checkpoint and a progress line fall out |
| Bound `enrich --parallel` | **reject** | already bounded by the flag; the cost is in the eight `claude` processes, not in this binary, and lowering the default doubles a token-bound wall time for nothing this report names |
| Walk / parse / lexical / store changes | **reject** | 1.9 s and 70 MB for the whole tree; nothing to win |
| `--rerank-local`'s 44 s and 3.4 GB | **out of scope, noted** | the reranker session takes the same thread cap; its `BATCH = 16` × ~80 tokens is small, the weights are the cost. A candidate depth below 200 is a retrieval question, not a resource one |

### 2.2 Defaults and overrides

- `threads`: an integer in `repograph.toml` **and** in the machine file (it describes the machine, like `enrich_model`); `REPOGRAPH_THREADS` outranks both for one run; absent or `0` means the default. Default: **`max(1, available_parallelism() / 3)`** — 4 on this 12-logical-core machine, two of the six performance cores left for the person (H1: 294% peak instead of 426%, +39% wall on its own — and with the token budget beside it the 43-minute rebuild becomes 30, §1.4 reading 5); a third rather than a half because ORT already ignores efficiency cores on Apple Silicon, so "half the logical cores" would be exactly today's pool and change nothing. The cap is a trade with no free side (§1.4 reading 2), so it is one word to undo: `threads = 6` in `~/.config/repograph/config.toml` restores ORT's own pick, `REPOGRAPH_THREADS=6` does it for one run, and a CI box that wants every core says so once. `1` is the floor.
- Token budget per forward: a constant, `TOKEN_BUDGET = 2048` in `embed.rs`, beside `BATCH = 64` (kept as the count cap: sixty-four twenty-token questions are 1,280 tokens and hit the count first). Not configurable: H4–H6 show it costs nothing to bound, and a user who wants a faster run has `threads`.
- Chunk size for the streaming sync: `SYNC_CHUNK = 1024` rows (a checkpoint about every minute on the large model here — H9-full ran at 18.4 rows/s; 33 checkpoints for the whole store, each an append of ≤ 4 MB plus a 3 MB `vectors.json` rewrite).

### 2.3 Edits, file by file

**`src/config.rs`**
- `Config`: add `pub threads: usize` with a doc comment on why it is a machine setting and what `0` means. `Default` sets `0`.
- `Machine`: add `threads: Option<usize>`.
- `Config::load`: layer `threads` the way `reranker_dir` is layered (project key wins when named, else machine, else default) — the existing `layer` closure is `String`-typed; add a second closure or make it generic over `T: Clone`. Then `REPOGRAPH_THREADS`: parsed as `usize`; empty is unset; anything else that does not parse is an error naming the variable (the file's own rule: a setting silently ignored is worse than a refusal).
- Update the doc comment on `Machine` and the README's "may set only …" sentence (§2.3 README).

**`src/index/embed.rs`**
- `pub fn threads(configured: usize) -> usize` + `fn threads_from(configured: usize, cores: usize) -> usize`: the effective count — `configured` if > 0, else the default rule over `cores = std::thread::available_parallelism()`; never 0. Pure, tested.
- `pub(crate) fn session_builder(threads: usize) -> Result<SessionBuilder>`: `Session::builder()` + `Level1` + `with_intra_threads(threads)`, nothing else (§1.4 rejected spin control and memory-pattern). `load_session(model, threads)` uses it; `CrossEncoder::open` uses it.
- `Embedder::open(model: &str, threads: usize)`.
- Replace `length_batches(texts, batch)` with `fn token_batches(lens: &[usize], max_batch: usize, budget: usize) -> Vec<Vec<usize>>`: stable sort by token length, a batch closes when it holds `max_batch` texts or when `(len + 1) × longest` would exceed `budget`; a text longer than the budget is a batch of one; `budget == 0` disables the budget (the old behaviour, kept so the rule is one function).
- `embed()`: a length pass first — `encode_batch` in chunks of 1,024 texts, length = the attention mask's sum (the tokenizer pads, the mask says what is real) — then `token_batches(&lens, BATCH, TOKEN_BUDGET)`, then the forwards as today. Update the doc comment (the "batched by length" paragraph) to say what bounds the shape and why.
- `const TOKEN_BUDGET: usize = 2048;` with the comment carrying B's 2.70 GB at 64 × 256 and the harness's flat wall down to 8 × 256.

**`src/index/cross.rs`**
- `CrossEncoder::open(dir: &Path, threads: usize)` builds its session through `embed::session_builder(threads)`; remove the duplicated builder lines.

**`src/ask.rs`**
- `embedder_or_notice(no_dense, model, threads)`, `open_embedder(no_dense, model, threads)`, `warm_model(dense, whole, model, threads)`; every call passes `crate::index::embed::threads(cfg.threads)` (the `Context` holds `cfg`; `main`'s callers hold one). `CrossEncoder::open(&dir, threads)`.
- Tests: `the_model_is_not_warmed_for_an_exact_answer_or_a_lexical_arm` gains the argument; no behaviour change.

**`src/bench.rs`, `src/dump.rs`**: pass `embed::threads(cfg.threads)` to `Embedder::open` and `CrossEncoder::open`; call `crate::cap_pools(threads)` (below) after their own `Config::load`, since neither goes through `main`'s config.

**`src/main.rs`**
- `fn cap_pools(threads: usize)`: `rayon::ThreadPoolBuilder::new().num_threads(threads).build_global()`, its `Err` (already initialised) ignored on purpose with a comment saying why (a test binary, or `bench` after `main`, may have built the pool). Called once per arm right after `load_cfg()` for `Build|Update|Enrich|Embed|Ask|Serve|Watch`; `bench::run` and `dump::run` call it themselves. `explain`/`verify` load no config and touch no pool.
- `embed_all(repo, no_dense, cfg)` takes the config (it needs `embed_model` and `threads`), opens the embedder with the cap, and drives the streaming sync (below) with a progress line on stderr: `dense: 4096/33533 rows, 12.3 rows/s, ~40 min left` after each chunk, and the final `dense: embedded N rows in Ts` as today. `run_watch` passes the cap too.

**`src/index/dense.rs`**
- `pub fn sync(…)` keeps its signature and calls `sync_chunked(graph, questions, embed, usize::MAX, &mut |_, _| Ok(()))`.
- `pub fn sync_chunked(&mut self, graph, questions, embed: &mut dyn FnMut(&[String]) -> Result<Vec<Vec<f32>>>, chunk: usize, after_chunk: &mut dyn FnMut(&mut DenseIndex, Progress) -> Result<()>) -> Result<usize>` where `Progress { done: usize, total: usize }`. The todo lists are built as today; the model-width check happens on the first chunk's vectors; rows are appended chunk by chunk; `after_chunk` runs after each append with the index in a loadable state (old rows still alive, new rows appended — the next `sync` retires the old ones by hash, so a checkpoint an interrupted run leaves behind is consistent and costs at most one duplicate row per edited node until then). Holes, `reindex` and compaction happen once at the end, as today.
- `embed_all`'s `after_chunk` is `|idx, p| { idx.save(store)?; eprintln!(progress line); Ok(()) }`. `save` already appends only the rows past `persisted`, so a checkpoint is the chunk's bytes plus the metadata rewrite.
- Doc comment on `sync_chunked`: why a checkpoint is consistent mid-sync, and why compaction waits.

**`README.md`**: the settings table gains `threads` (`the run` row: `REPOGRAPH_THREADS`; the machine file "may set only …" list gains `threads`); a short "Resources" paragraph under the Performance section with the before/after numbers from §4 and the `embed_model = small` / `--no-dense` workarounds; the `watch`/`serve` "1.3 GB" mentions updated to the measured 1.4 GB (small) / 1.8 GB (large).

**`docs/bench/2026-09-07-resource-usage-results.md`** (new): §1's tables, the harness table, and the after-fix re-measurement (§4), in the style of `docs/bench/2026-09-06-perf-results.md`.

Nothing changes in the store format, the CLI flags, or the socket protocol. `repograph.toml` files without `threads` load as before (serde default); a file naming `threads` is refused by an older binary (`deny_unknown_fields`), exactly as `embed_model` was when it arrived.

---

## 3. Tests

All inline `#[cfg(test)]` modules, no model download, run by `cargo test --release --manifest-path "$W/Cargo.toml"` (today: 445 unit tests pass, 2 `#[ignore]`d ones need models, and `tests/serve.rs` adds 12 that spawn the binary — measured green on this worktree at `868f4c1`, `$S/log/cargo-test-serve-clean.txt`). Run it with no `REPOGRAPH_*` variable in the environment: `REPOGRAPH_NO_SERVE=1` left over from a measurement makes every client `ask` skip the socket and fails nine serve tests with "serve never answered over the socket". Names are the assertion.

**`src/index/embed.rs`** (module `tests`)
- `token_batches_close_on_the_padded_token_budget_and_cover_every_text_once` — lens `[10, 300, 20, 256, 5]`, cap 64, budget 512: batches `[[4,0,2],[3],[1]]` (4 × 256 = 1,024 and 2 × 300 = 600 both exceed the budget; 3 × 20 = 60 does not), every index exactly once.
- `token_batches_keep_a_text_longer_than_the_budget_as_a_batch_of_one` — a 1,000-token text under budget 512 is its own batch and the run does not stall.
- `token_batches_never_exceed_the_count_cap_however_small_the_texts` — 200 one-token texts, cap 64, budget 100,000: four batches of ≤ 64.
- `token_batches_with_no_budget_are_the_old_count_batches` — budget 0 reproduces today's `[[1,2],[4,0],[3]]` shape from the existing `length_batches` test, so the old tests move to the new name unchanged.
- `token_batches_keep_original_order_among_equal_lengths` and `…_with_cap_larger_than_the_collection_yield_one_sorted_batch` — the two existing tie/one-batch tests renamed.
- `the_thread_cap_is_the_configured_count_or_a_third_of_the_cores_and_never_zero` — `threads_from(0, 12)` = 4, `threads_from(0, 8)` = 2, `threads_from(0, 2)` = 1, `threads_from(0, 0)` = 1, `threads_from(3, 12)` = 3, `threads_from(9, 2)` = 9 (a configured count is never clipped: the person said so).
- `pool_…` and `normalise_…` tests untouched.

**`src/config.rs`** (module `tests`, inside the `ENV` lock helper `with_machine`)
- `threads_defaults_to_zero_and_reads_from_the_project_file` — absent → 0; `threads = 3` → 3.
- `the_machine_file_may_set_threads_and_the_project_still_wins` — machine `threads = 2`, project `threads = 5` → 5; project silent → 2.
- `the_environment_beats_the_project_for_threads` — `REPOGRAPH_THREADS=4` over `threads = 2` → 4; empty env → 2.
- `a_threads_value_that_is_not_a_number_is_an_error_naming_the_file` — `threads = "many"` in the project file errors with `repograph.toml` in the message; `REPOGRAPH_THREADS=lots` errors naming `REPOGRAPH_THREADS`.
- Existing `the_machine_file_may_not_set_a_corpus_key` stays green (unchanged rule).

**`src/index/dense.rs`** (module `tests`, using the existing `fake` embedder and `wide()` graph)
- `sync_chunked_appends_every_row_and_reports_progress_after_each_chunk` — ten rows, chunk 4: `after_chunk` sees `(4,10)`, `(8,10)`, `(10,10)`; the index equals `synced(&wide(0))` in ids, hashes and vectors.
- `a_checkpoint_saved_mid_sync_loads_and_the_next_sync_finishes_the_rest` — `after_chunk` saves to a temp store and then errors after the first chunk (an interrupted run); `DenseIndex::load` returns four rows; a fresh `sync` on that store embeds exactly the remaining six and the final search equals the uninterrupted one.
- `a_chunked_sync_over_an_edited_store_leaves_the_same_holes_as_a_plain_one` — `wide(0)` then `wide(1)` with chunk 3: `free == [0]`, `live == 1..11`, same as `an_append_leaves_the_surviving_rows_at_their_offsets`.
- `sync_is_sync_chunked_with_one_chunk` — same result and `after_chunk` called exactly once with `(n, n)`.

**`src/index/cross.rs`**: none (the builder is shared; nothing pure changed).

**`src/main.rs`**: `cap_pools_tolerates_a_pool_already_built` — calling it twice does not panic (rayon's global pool can only be built once per process and the test binary shares one).

**Integration (`tests/serve.rs`)**: nothing; the socket protocol did not move.

---

## 4. Verification

Run from `$W` after every task, in this order; the numbers that count as fixed are next to each.

```bash
W=/Users/max/Documents/projects/repograph/.claude/worktrees/graph-build-resource-usage-b554bc; B="$W/target/release/repograph"
cargo build --release --manifest-path "$W/Cargo.toml" 2>&1 | tail -1
cargo test --release --manifest-path "$W/Cargo.toml" 2>&1 | grep -E "^test result|FAILED|panicked"
cargo clippy --release --all-targets --manifest-path "$W/Cargo.toml" -- -D warnings 2>&1 | tail -3
```
Expected: every `test result: ok`, the counts 445 (unit) and 12 (serve) plus the new ones, 2 ignored, clippy silent.

**The rebuild (D, after):** the same store copy, the same sampler.
```bash
G=/Users/max/bench/resources-2026-09-07; S=…scratchpad (or $G/scratch after Task 0)
rm -rf "$G/store-D2" && mkdir -p "$G/store-D2" && cp -a "$G/store-pristine" "$G/store-D2/.repograph" && rm -f "$G/store-D2/.repograph/vectors.*"
"$S/measure.sh" embed-D2-large-full-after "$G/log" -- "$B" --repo "$G/store-D2" embed
```
Fixed when, against `embed-D-large-full` (2,582 s, 2.96 GB, 444%/372% CPU, 18 threads): max RSS ≤ **2.3 GB** (H9-full read 2.11 in the harness, which runs above repograph); CPU% peak ≤ **310%** and average ≤ 300% (four threads: H9-full 294% peak, 289% average) and the thread count ≤ **9** (main + three ORT workers + ORT's own + a rayon pool of four); wall ≤ **2,000 s** (H9-full: 1,821 s, against 2,582 today — the fix must not be slower, and this predicts it is faster); `stderr` shows a progress line at least every 60 s; and `kill -INT` at minute 5 followed by a second `embed` finishes with `dense: embedded <fewer than 33525> rows` — the checkpoint held. Then the overrides, on the 320-row copy (`$G/store-B`, vectors removed before each run): `REPOGRAPH_THREADS=2 "$B" --repo "$G/store-B" embed` samples at ≤ 2 running threads and ≤ 160% CPU with top's thread column ≤ 6 (main + one ORT worker + a rayon pool capped at two, plus ORT's housekeeping thread) — today's 18; `threads = 2` in that copy's `repograph.toml` reads the same; `threads = 6` reads today's 18 threads and ≈ 400%. Record each in `$G/log/t1-threads-<n>.samples`.

**The readers, unchanged or better:** re-run `$S/quick.sh`'s ask/bench/dump lines on `$G/fixture`; every max RSS within 5% of §1.2 (a `--rerank-local` ask may drop with the thread cap, never rise), every wall within 10%.

**The bench floors — must not move.** The floors are constants in `src/bench.rs::FLOORS`; this work does not touch them. The measured lines must still pass:
```bash
cd "$W" && ARMS="dense lexical" NOTE="resource-usage after" bash bench/history/run-repograph.sh
```
Expected: exit 0 and the two summary lines identical to the last `bench:dense+enriched` / `bench:lexical+enriched` rows for commit `502e8a6d` in `bench/history/runs.jsonl` (`python3 bench/history/track.py report` prints the reading). The fixture's rows are the small model's and are not re-embedded, so the dense line cannot move through this change; it is run because `Lexical::build` now runs under a capped rayon pool.

**The re-embedded store, floors held:** the token-budgeted batches change padding, so a re-embedded store's vectors differ at float precision. Prove it does not cost a case:
```bash
rm -rf "$G/e5large2" && mkdir -p "$G/e5large2" && cp -a /Users/max/bench/gaps-2026-09-05/e5large/.repograph "$G/e5large2/.repograph" && cp /Users/max/bench/gaps-2026-09-05/e5large/repograph.toml "$G/e5large2/"
rm -f "$G/e5large2/.repograph/vectors.*"; "$B" --repo "$G/e5large2" embed 2>&1 | tail -1
"$B" --repo "$G/e5large2" bench 2>&1 | tail -1
```
Expected: `keyword 40/40  paraphrase ≥22/30  code 12/12  p90 ≤230 … model=large gated=true`, exit 0 (the large floors, README "Bench"). Record the line in the results doc beside the pre-change `t5-large-enriched-1.txt` reading (`40/40 22/30 12/12`).

---

## 5. Comment hygiene

Every comment added by this plan explains a why the code cannot say: why the pool is capped at all (the measurement), why a token budget and not a count, why a checkpoint mid-sync is a consistent store, why `build_global`'s error is ignored, why `threads` is a machine key when `embed_model` is not. No comment restates a name, a constant or a signature; no ticket ids; the existing `#[allow(clippy::…)]` directives stay. Test names carry the assertion, not the mechanism.

---

## 6. Side findings, not in this plan

- `serve` cannot bind when `<repo>/.repograph/serve.sock` exceeds 104 bytes (`SUN_LEN`); the error names the cause. A repo deep under `/private/tmp/…` cannot use `serve` at all. Worth its own issue: bind a shorter socket in `$TMPDIR` keyed by a hash of the canonical path, or say so in the README.
- A `serve` killed with SIGTERM leaves `serve.sock` behind (`Unlink` is a `Drop`, and there is no signal handler); the next `serve` removes a dead one before binding and a client's connect just fails, so it is harmless, but `--idle` is the only clean exit.
- The fixture's `manifest.json` stamps were whole-second mtimes after its restore; the first reader rewrote them to the nanosecond ones. Content unchanged.

---

## Tasks

### Task 0: Scratch, copies, baselines carried out of the session directory

**Files:** none in the repo.

- [ ] **Step 1**: `G=/Users/max/bench/resources-2026-09-07; mkdir -p "$G/log"`; copy from `$S`: `repograph-store-pristine` → `$G/store-pristine`, `fixture` → `$G/fixture` (the git-initialised copy: `changes` needs a base commit), `harness` → `$G/harness`, `measure.sh`, `quick.sh` (edit its `S=` line to `$G`), `log/` → `$G/log-before`. `ls -la "$G"` into `$G/task0.txt`.
- [ ] **Step 2**: confirm the before numbers are on disk: `grep -c . "$G/log-before/summary.txt"` ≥ 30 and `grep embed-D-large-full "$G/log-before/summary.txt"` prints the §1.1 line.

### Task 1: `threads` — the key, the cap, both sessions, both pools

**Files:** `src/config.rs`, `src/index/embed.rs`, `src/index/cross.rs`, `src/ask.rs`, `src/bench.rs`, `src/dump.rs`, `src/main.rs`; README settings table.

- [ ] **Step 1 — failing tests first**: add the `config.rs` and `embed.rs` tests of §3 (`threads_from`, the four config tests). `cargo test --release … threads` fails to compile.
- [ ] **Step 2**: `Config.threads`, `Machine.threads`, the layering and `REPOGRAPH_THREADS` in `Config::load` (§2.3). Tests green.
- [ ] **Step 3**: `embed::threads`/`threads_from`, `session_builder(threads)`, `Embedder::open(model, threads)`, `CrossEncoder::open(dir, threads)`; thread the argument through `ask.rs`, `bench.rs`, `dump.rs`, `main.rs`. `cap_pools` in `main.rs` with its test.
- [ ] **Step 4**: `cargo test --release`, `cargo clippy … -D warnings`, then the override check from §4 (`REPOGRAPH_THREADS=2` → thread count) on `$G/store-B`-style copy (320 rows, `$S/store-B` copied to `$G`), recorded in `$G/log/t1-*.txt`.
- [ ] **Step 5**: commit `feat(config): threads caps the model sessions and the tokenizer pool` with the measured before/after of the 320-row run in the body.

### Task 2: token-budgeted batches

**Files:** `src/index/embed.rs`.

- [ ] **Step 1 — failing tests**: the four `token_batches_*` tests of §3 (renaming the three `length_batches_*` ones). Fails to compile.
- [ ] **Step 2**: `token_batches`, the length pass in `embed()`, `TOKEN_BUDGET = 2048`, the doc comment. Tests green, clippy silent.
- [ ] **Step 3**: the 320-row run again (`$G/log/t2-embed-320.txt`): max RSS ≤ 2.0 GB against B's 2.70 (the harness went 3.63 → 1.93 on the same shapes; repograph starts 0.9 GB lower), wall within 10% of B's 111 s × 1.39 (the thread cap from Task 1) ≈ 155 s.
- [ ] **Step 4**: commit `perf(embed): batches close on a token budget, so the largest shape is bounded` with B/H9/t2 numbers in the body.

### Task 3: the streaming sync — checkpoints and a progress line

**Files:** `src/index/dense.rs`, `src/main.rs`.

- [ ] **Step 1 — failing tests**: the four `dense.rs` tests of §3.
- [ ] **Step 2**: `sync_chunked`, `Progress`, `sync` as its one-chunk case; `embed_all(repo, no_dense, cfg)` drives it with the save + progress closure; `run_watch` unchanged (small deltas use `sync`).
- [ ] **Step 3**: tests green, clippy silent. Interrupt test by hand on the 320-row copy: `kill -INT` after the first progress line, run again, `dense: embedded <320` rows. Record in `$G/log/t3-interrupt.txt`.
- [ ] **Step 4**: commit `feat(embed): a long embed saves after every chunk and says where it is`.

### Task 4: the full re-measurement and the docs

**Files:** `docs/bench/2026-09-07-resource-usage-results.md` (new), `README.md`.

- [ ] **Step 1**: §4's `embed-D2-large-full-after` run (≈ 1,821 s expected, ≤ 2,000 s required), the readers' re-run, the bench floors run, the `e5large2` re-embed and bench. Every output under `$G/log`.
- [ ] **Step 2**: write the results doc: §1's tables verbatim, the harness table, the after table, verdicts. README edits of §2.3.
- [ ] **Step 3**: commit `docs: resource usage measured before and after, and how to bound it`.
- [ ] **Step 4**: open the PR to `devmaxxx/repograph`, base `main`, body = the results doc's summary and the §4 numbers; do not merge.
