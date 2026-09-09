# A rebuild nobody at the keyboard notices — the definition, the measurements, the plan

> **Superseded 2026-09-09.** The band this plan designed and shipped was removed; `libc`,
> `src/priority.rs`, the `priority` key and `REPOGRAPH_PRIORITY` are gone with it, and so is
> `threads`, replaced by `resources`. The measurement method here still stands and is reused by
> [the removal plan](2026-09-09-normal-band-only.md).

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:executing-plans (or superpowers:subagent-driven-development) to implement this task-by-task. Steps use checkbox (`- [ ]`) syntax. Every number in this file came from a run named beside it; a number you write comes from a run you name.

**Goal:** a `build`, `update`, `enrich`, `embed` or `watch` that re-embeds a store is something the person using the machine cannot feel — not in their own compile, not in the pointer, not in the fan, not in swap. Wall time is reported and no longer a bar: the person is not waiting for it.

**Request being answered (translated):** "reduce resource usage further, as much as possible, so that the user does not notice when repograph is working."

**Before:** the shipped state at `786b994` — `threads` = cores / 3 (4 here), token-budgeted batches, checkpoints every 1,024 rows. A full e5-large re-embed of 33,525 rows: 1,930 s wall, 7,641 s user, 2.15 GB max RSS, 293% CPU peak on 8 threads (4 running), every one of them on a performance core (`docs/bench/2026-09-07-resource-usage-results.md`).

**Architecture:** unchanged from the previous plan (`docs/plans/2026-09-06-resource-usage.md`): the writers reach `embed_all` → `DenseIndex::sync_chunked` → `Embedder::embed`; the embedder is an `ort` 2.0.0-rc.13 session whose intra-op pool is capped by `session_builder(threads)`; `tokenizers` fans batches over rayon's global pool, capped by `cap_pools`. The readers (`ask`, `serve`, `bench`, `dump`) open the same session for one query at a time and answer a person.

**Tech stack:** Rust 1.98 (`rust-toolchain.toml`), `cargo build --release`, `cargo test --release` with no `REPOGRAPH_*` variable set, `cargo clippy --release --all-targets -- -D warnings`. One new direct dependency, `libc = "=0.2.189"` — already in `Cargo.lock` as a transitive one, pinned like every other line.

## Global constraints

- Worktree `W=/Users/max/Documents/projects/repograph/.claude/worktrees/graph-build-resource-usage-b554bc`, branch `claude/graph-build-resource-usage-b554bc`; never `cd` into the main checkout. `B="$W/target/release/repograph"`, rebuilt before any measurement.
- Bench dir `G=/Users/max/bench/resources-2026-09-07`: `store-B` (320 longest rows, no `repograph.toml`, so the default large model), `store-pristine` (the full store), `fixture` (read-only for writers), `measure.sh`, `harness/`, `log-before/`, `log/`. Scratch for this plan phase: `S=/private/tmp/claude-502/-Users-max-Documents-projects-repograph--claude-worktrees-graph-build-resource-usage-b554bc/d854dd4b-1b8c-403b-a458-5463278607fa/scratchpad` — `S/probe/{probe,qos,rusage}.c` and their binaries, `S/run2.sh`, `S/chain{1,2}.sh`, `S/log/<run>.{time,samples,probe6,probe1,psM,rusage,out}` and `S/log/summary.txt`. A reboot removes `S`; Task 0 copies it to `$G/unnoticeable/` first.
- The pinned fixture `/Users/max/bench/beauty-crm-502e8a6d` is read-only: `ask --stale`, `bench`, `dump` only. Writers run on copies; a copy pins `embed_model = "intfloat/multilingual-e5-small"` unless the large model is what is measured.
- Measurements export `REPOGRAPH_NO_SERVE=1`; `cargo test` runs in a shell where no `REPOGRAPH_*` is set.
- No model tokens are spent. No file is downloaded: the int8 model on the hub (§2.5) is named, not fetched.
- Comments say why, never what; no ticket ids; tool directives stay. Test names are `snake_case` sentences. Commit subjects are Conventional; no AI trailers; a heredoc and a `git commit` go in two shell commands.

---

## 1. What "unnoticeable" means here, and how it is measured

A person at the keyboard notices four things: their own work slowing down, the pointer and the typing stuttering, the fan, and the machine starting to swap. Each of those is measured directly, with the rebuild running beside it, rather than through the process's own CPU% — a process at 260% of a core on the efficiency cluster is invisible, and one at 100% of a performance core is not.

### 1.1 The probes

`S/probe/probe.c` (C, `clang -O2`), started 20 s into every run so the session is open and the forwards are under way, on the same machine as the rebuild:

- **P6** — six threads, the performance-core count, at default QoS, each running 1.5 × 10⁹ iterations of a dependent xorshift-and-fma chain; eight rounds; the median round's wall. This is the person's compile, or their browser laying out a page: work that wants every performance core for a couple of seconds.
- **P1** — the same with one thread, four rounds: the single-threaded app.
- **W** — one thread at `QOS_CLASS_USER_INTERACTIVE` that `nanosleep`s 1 ms in a loop for the whole probe and records how late it woke; p50, p99 and max in µs. This is an app's main thread waiting on the next input event or frame.

Where the rebuild's CPU time actually ran, read from the kernel rather than inferred:

- **Band** — `proc_pid_rusage(RUSAGE_INFO_V4)`'s `ri_cpu_time_qos_*` counters, polled by `S/probe/rusage.c` until the process exits: the seconds the scheduler charged to each QoS band. On Apple Silicon the background band is confined to the efficiency cluster.
- **PRI** — `ps -M -p <pid>`, the Mach priority of every thread: 31 is the default band, 4 the background/throttled one.
- **Clock** — `cycles elapsed / (user + sys)` from `/usr/bin/time -l`: the M3 Pro's performance cores sustain ~3.5–4.0 GHz, the efficiency cores ~1.7–2.7 GHz.

Memory is `/usr/bin/time -l`'s `maximum resident set size` and `peak memory footprint` (the anonymous, compressible part — what memory pressure counts), and `vm_stat`'s swapouts before and after. Disk is `ri_diskio_byteswritten` / `ri_logical_writes` from the same rusage sample.

**What could not be measured:** `powermetrics` (CPU power, per-cluster residency, fan) needs root, and `sudo -n true` answers "a password is required"; `ri_billed_energy` reads 0 on this machine. The fan's proxy is therefore the band and the clock: work confined to the efficiency cluster at 1.7 GHz is the low-power configuration Apple's own scheduler puts background work in, and it is what the fan curve does not follow.

### 1.2 The bars

Idle baselines, `S/log/R0{a,b}.probe6`, `S/log/R0.probe1` (the probe alone, nothing else running):

| probe | idle |
|---|---|
| P6 median | 2,585 ms and 2,583 ms |
| P1 median | 2,493 ms |
| W | p50 257–259 µs, p99 269–291 µs, max 5–14 ms (the machine's own jitter) |

A rebuild is **unnoticeable** when, with it running:

1. **P6 within 5% of idle** (≤ 2,714 ms) and **P1 within 5%** (≤ 2,618 ms): the person's own compile runs at its own speed.
2. **W p99 ≤ 1 ms**: a frame is 8.3–16.7 ms, so a wake that lands within a millisecond is never a dropped one; the shipped state's 2.5 ms p99 is what "the pointer hitches" looks like in numbers. The p50 is not a bar: it sits at ~505–515 µs under any load at all (R2, R3 and R4 alike, against 258 idle) — the kernel coalescing a 1 ms timer on a busy machine, a constant quarter-millisecond no one can feel — where the p99 is the tail that shows.
3. **100% of the process's CPU time in the background band**, every thread at PRI 4, average clock ≤ 2.0 GHz: nothing on a performance core, which is the fan bar by proxy (§1.1).
4. **Memory:** max RSS ≤ 2.0 GB, peak footprint ≤ 1.8 GB, swapouts unchanged. Honest floor: a full embed touches 1.63 GB of the fp32 weights (R1's `disk read 1627 MB` is exactly the pages of `model.onnx_data` it faulted in), so nothing under 1 GB is reachable without other weights (§2.5).
5. **Disk:** ≤ 300 MB written for a whole-store rebuild and no fsync more often than the checkpoint (once a minute).
6. **Wall** is reported beside every row and is not a bar.

Bars 1–3 are the request; 4–5 are what the previous plan already bounded, kept so nothing regresses.

---

## 2. Measurements

All on `$G/store-B` (320 longest passages, default large model, `vectors.*` removed before each run) through `S/run2.sh NAME -- <wrapper>`: `/usr/bin/time -l`, a one-second `top` sampler, the rusage poller, `ps -M` at t = 20 s, then P6 and P1 inside the run. `wall` therefore includes ~35 s of the probes competing for the machine; the clean shipped wall for this store is 133.3 s (`$G/log/t2-embed-320.time`). Each row names its log under `S/log/<name>.*`; the summary line is in `S/log/summary.txt`.

### 2.1 Mechanism first: what a QoS call reaches (`S/probe/qos.c`)

| call | what a thread created afterwards reads | reversible |
|---|---|---|
| `pthread_set_qos_class_self_np(QOS_CLASS_BACKGROUND)` on the main thread | `qos=default` — **not inherited**; ORT's workers are created inside `commit_from_file`, rayon's inside `build_global`, and both would stay in the default band | per thread |
| `setpriority(PRIO_DARWIN_PROCESS, 0, PRIO_DARWIN_BG)` | `getpriority` reads 1 in every thread, before and after; `ps -M` shows PRI 4 on all eight (`S/log/R2-darwinbg-t4.psM`) | yes: `setpriority(PRIO_DARWIN_PROCESS, 0, 0)` reads 0 again |
| `taskpolicy -c background` (a QoS clamp) | main reads `qos=background`, children `default`, the clamp holds them all | set through `posix_spawnattr` only — no call a running process can make on itself |

So the per-thread route needs `SessionBuilder::with_thread_manager` (ort rc.13 has it: `environment::ThreadManager`) plus a rayon `spawn_handler`, to reach the same band the one process-wide call reaches. `taskpolicy -b` *is* `setpriority(PRIO_DARWIN_PROCESS, 0, PRIO_DARWIN_BG)` (`man taskpolicy`), so every `-b` row below is the code's own effect measured before it was written.

### 2.2 Scheduling levers (lever 1 and lever 2)

| run | wrapper | threads | wall | user / sys | max RSS / footprint | CPU peak (avg) | clock | band (PRI) | P6 | P1 | W p50 / p99 | verdict |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| R1-shipped-t4-fg | none | 4 | 152.3 s | 571.9 / 4.5 s | 1.83 / 1.63 GB | 262% ¹ | 3.49 GHz | default 100% (31) | **2,991 ms (+15.7%)** | 2,585 (+3.7%) | 261 / **2,469 µs** | the before: fails bars 1–3 |
| R2-darwinbg-t4 | `taskpolicy -b` | 4 | 626.3 s (4.1×) | 2,255 / 42.8 s | 1.82 / 1.64 GB | 280% (259%) | **1.70 GHz** | **background 100%** (4) | **2,630 ms (+1.7%)** | 2,481 (−0.5%) | 504 / **603 µs** | **passes 1–4**; wall ×4.1 |
| R3-clampbg-t4 | `taskpolicy -c background` | 4 | 624.6 s (4.1×) | 2,254 / 41.5 s | 1.82 / 1.64 GB | 280% (259%) | 1.70 GHz | background 100% (4) | 2,631 (+1.8%) | 2,489 (−0.2%) | 514 / 569 µs | = R2 in every column: the clamp and the task policy land in the same band |
| R4-clamputil-t4 | `taskpolicy -c utility` | 4 | 158.9 s (1.0×) | 600.5 / 5.8 s | 1.83 / 1.63 GB | 277% (264%) | **3.38 GHz** | utility 100% (20) | **2,767 (+7.0%)** | 2,526 (+1.3%) | 509 / 849 µs | the compromise: performance cores at a lower priority, no wall cost, the pointer fixed (bar 2), the compile still 7% slower — **fails bar 1** |
| R5-darwinbg-t6 | `taskpolicy -b`, `REPOGRAPH_THREADS=6` | 6 | 594.1 s (−5% on R2) | 2,602 / 66.5 s (+15%) | 1.80 / 1.64 GB | 378% (313%) | 1.50 GHz | background 100% (4) | 2,678 (+3.6%) | 2,507 (+0.6%) | 511 / 731 µs | six efficiency cores share one clock: all of them busy runs each slower, so a third more CPU-seconds buys 5% of wall and costs the probe two points — **four stays** |
| R6-fg-t2 | `REPOGRAPH_THREADS=2` | 2 | 288.1 s (1.9×) | 555.7 / 4.0 s | 1.77 / 1.64 GB | 150% (138%) | 3.52 GHz | default 100% (31) | **2,820 (+9.1%)** | 2,529 (+1.4%) | 259 / 812 µs | half the threads on the performance cores: twice the wall and the compile still 9% slower — **fails bar 1**; the count is the wrong lever |

¹ R1's sampler followed the wrong pid (the `time` wrapper; fixed in `run2.sh`); its 262% is read from `ps -M` at t = 20 s (`S/log/R1-shipped-t4-fg.psM2`: three workers at 61–64% and the main thread at 70%), its band and clock from a rusage poller started late on the right pid (`S/log/R1-shipped-t4-fg.rusage2`, `legacy 575 s`) and from `/usr/bin/time -l`'s cycles.

Reading R1 against R2: the same 8.43 × 10¹² instructions retired in both (`.time`); on the efficiency cluster they took 3.92 × 10¹² cycles instead of 2.01 × 10¹² (half the IPC) at 1.70 GHz instead of 3.49 (half the clock), which is the 4× — and the person's compile got its 16% back, the pointer its 2 ms. The `sys` time growing from 4.5 s to 42.8 s is the throttled band's scheduling overhead; it is charged to the background band too.

### 2.3 Memory and pacing levers (lever 2's pacing, lever 3) — the harness

`$G/harness` (repograph's tokenize → forward → pool loop with the levers as arguments, built with `CARGO_TARGET_DIR=$W/target`), extended in this phase with three switches: `SHRINK` (`memory.enable_memory_arena_shrinkage = cpu:0` on every `run_with_options`), `DUTY` (sleep `(1/d − 1) ×` the forward's own time after each forward) and `PREPACK` (`session.disable_prepacking`). Same 320 texts, four threads, foreground, against the previous plan's H9 (134.1 s, 1.93 GB):

| run | switch | forwards | wall | user | max RSS | footprint | session open | verdict |
|---|---|---|---|---|---|---|---|---|
| H9 (control, previous plan's build; `$G/log-before/H9-t4-64-b2048.time`) | budget 2,048 | 40 | 134.1 s | 530.7 s | 1.93 GB | 1.74 GB | — | the wall control for H10–H15, which ran on an idle machine |
| H16-t4-b2048-control (today's build, every switch at its default; ran beside the full run of §2.6, which sat on the efficiency cluster) | budget 2,048 | 40 | 145.5 s | 556.9 s | 1.86 GB | 1.74 GB | 0.75 s | the memory control: max RSS varies 1.82–1.93 GB run to run at this shape, so H10's 1.82 GB is noise and only the footprint column below separates the levers |
| H10-t4-b2048-shrink | arena shrinkage on every run | 40 | 151.0 s (+13%) | 561.7 s | 1.82 GB | 1.74 GB | 2.33 s (cold cache) | **reject**: the arena is given back after each forward and grown again before the next; the footprint does not move because the arena is not what fills it |
| H11-t4-b2048-nomempat | memory pattern off | 40 | 147.2 s (+10%) | 560.9 s | 1.82 GB | 1.63 GB | 0.73 s | **reject**: −0.11 GB for +10% wall, the previous plan's H7 again |
| H12-t4-b1024 | budget 1,024 | 80 | 141.3 s (+5%) | 541.9 s | 1.78 GB | 1.62 GB | 0.76 s | reject with H13 |
| H13-t4-b512 | budget 512 | 160 | 132.5 s (−1%) | 515.1 s | 1.74 GB | 1.55 GB | 0.67 s | **reject**: −0.19 GB on these 320 long rows, where the budget only turns 8 × 256 forwards into 2 × 256; on the short rows that are nine tenths of a store it would close the sixty-four twenty-token questions at twenty-five and run 1,300 forwards where H9-full ran 776, unmeasured; bar 4 is already met (R2) |
| H14-t4-b2048-noprepack | `session.disable_prepacking` | 40 | 150.2 s (+12%) | 584.9 s | 1.92 GB | **0.61 GB** | **0.27 s** | **accept for the writers** (§3.3): the anonymous 1.13 GB was MLAS's packed copies of the 24 layers' GEMM weights (12.6 M parameters × 4 B × 24 = 1.2 GB), made at session open; without them the GEMMs read the memory-mapped `model.onnx_data` pages, which are clean, reclaimable and shared between processes. Max RSS is unchanged because the same pages are touched; what memory pressure counts is a third of what it was |
| H15-t4-b2048-duty0.5 | duty cycle 50% (sleep a forward's length after each) | 40 | 288.1 s (2.15×) | 556.7 s | 1.93 GB | 1.74 GB | 0.68 s | **reject** as a default: CPU average 240% → 120% for twice the wall and the same CPU-seconds — a linear trade with nothing free in it. Under the band the pointer and the compile are already at idle (R2), so pacing could only lower the efficiency cluster's load, which nobody at the keyboard shares. Not shipped, not configured; the band is the lever |

`footprint` is `/usr/bin/time -l`'s `peak memory footprint`: anonymous plus compressed, the number `memory_pressure` and jetsam act on; file-backed pages are outside it. The shipped repograph reads 1.63 GB footprint on this store (`$G/log/t2-embed-320.time`) and 1.84 GB on the full one (`$G/log/embed-D2-large-full-after.time`).

### 2.4 Disk (lever 4)

R2 wrote 1.3 MB (`ri_diskio_byteswritten`, `S/log/R2-darwinbg-t4.rusage`) for 320 rows: one checkpoint. A whole-store rebuild is 33 checkpoints of a ≤ 4 MB fsync'd append plus a 3 MB `vectors.json` rename (`DenseIndex::save`, `Store::append_after`) and one 22 MB graph save: ≈ 250 MB over the run, under 0.2 MB/s. The full run in §2.6 reads the real number; `PRIO_DARWIN_BG` throttles the process's disk I/O as well (`setpriority(2)`: "I/O throttling"), so the rows are written in the background I/O tier without a second call. **Fewer or larger checkpoints: rejected** — a minute's loss on interrupt was the previous plan's trade and nothing here is noticeable.

### 2.5 Weights (lever 3's quantized model) — named, not measured

The hub lists `onnx/model_qint8_avx512_vnni.onnx` for both models (562 MB for the large, 118 MB for the small, `curl https://huggingface.co/api/models/intfloat/multilingual-e5-large/tree/main/onnx`); there is no `model_quantized.onnx` or fp16. It is int8 weights quantized for AVX-512 VNNI; on arm64 the int8 GEMMs would run through MLAS's NEON kernels at an unmeasured speed, its vectors differ from the fp32 ones, and `written_by` records only the hub id, so a store's rows would need the file name recorded too or a reader with the fp32 weights would search int8 rows at the same width and never know. Measuring it means a 562 MB download and a full re-embed plus `bench` for the floors (`paraphrase ≥ 22/30` on the large model). **Deferred, not proposed:** the download is a decision for the person, the floors are an afternoon, and the scheduling lever already makes memory the one bar this plan leaves where the previous one put it.

### 2.6 The full store under the shipped combination

Started in this phase and left running, detached, because it outlives the session: `S/chain3.sh` — `$G/store-D3` is `store-pristine` without `vectors.*` (33,525 rows), `taskpolicy -b`, four threads (the rule in the script chose four from R5: it wanted six only for ≥ 15% of wall at P6 ≤ 2,714 ms, and R5 gave 5%), the probes at t = 60 s, `vm_stat` swapouts before and after. Its numbers land in `S/log/FULL-darwinbg-t4.{time,rusage,probe6,probe1,psM,samples}` and its summary line in `S/log/summary.txt`; Task 0 carries them to `$G/unnoticeable/log-plan/`, and Task 4 reads them into the results doc.

**Projection**, from the ratios R2 / R1 on the same shapes, applied to the shipped full run (`$G/log/embed-D2-large-full-after.time`: 1,930 s, 7,641 s user, 2.15 GB, 1.84 GB footprint):

| | shipped (measured) | projected under the band |
|---|---|---|
| wall | 1,930 s (32 min) | ≈ 7,900–8,700 s (2.2–2.4 h) — ×4.1 from R2/R1, ×4.5 from the run's own first checkpoint (below) |
| user + sys | 7,659 s | ≈ 31,000–34,000 s on the efficiency cluster |
| max RSS / footprint | 2.15 / 1.84 GB | the same: the band moves no memory |
| disk written | not counted then | ≈ 33 × 7 MB + 22 MB ≈ 250 MB (§2.4) |
| P6 / P1 / W p99 beside it | +16% / +4% / 2.5 ms (R1) | +2% / 0% / 0.6 ms (R2) |

**The in-run probe landed before this file was written** (`S/log/FULL-darwinbg-t4.{probe6,probe1,psM}`, t = 60–100 s, the first chunks of a full store): every thread at PRI 4; **P6 2,714 ms (+5.0%) — on the bar, not under it**; P1 2,511 ms (+0.7%); W p50 504–509 µs, p99 512–626 µs. The 3 points between R2's +1.7% and this +5.0% are the difference between 320 long rows and a whole store's mix: the tokenizer's length pass and the many short forwards on rayon's four threads are more memory traffic beside the probe than eight 256-token forwards are. It passes bars 2–3 outright and bar 1 at its edge; the results doc reads the full run's line as it is, and if the mapped weights (Task 2) move it — less anonymous memory is less to keep in cache beside a compile — that is a measured reading, not a promise. The one number the projection cannot give — whether four threads on the efficiency cluster hold the short rows at the same 4.1× as the long — is what the run's wall is for. Its first checkpoint landed too: `dense: 1024/33525 rows, 2.2 rows/s` against the shipped run's `10.0 rows/s` on the same first chunk (`$G/log/embed-D2-large-full-after.time`) — ×4.5, read while the probes, the H16 harness control and a `cargo test` build were all on the performance cores beside it. The shipped run climbed from 10.0 to a 17.4 rows/s average as the short rows arrived; if this one climbs the same way the whole store is ≈ 8,700 s (2.4 h), and the run's own straight-line "~244 min left" is the bound it prints if it does not.

---

## 3. Design

Two calls and one deletion. The writers put the whole process in the background band before any pool is built (one `setpriority`), open their model without packed weight copies (one builder entry), and `watch` stops holding a model between refreshes. Everything else measured above is rejected with its number, and the readers are not touched.

### 3.1 Candidates, accepted and rejected

| lever | verdict | reason |
|---|---|---|
| `setpriority(PRIO_DARWIN_PROCESS, 0, PRIO_DARWIN_BG)` in the writers | **accept** | R2: every bar 1–4 met from a single call; reaches ORT's and rayon's threads because it is a task policy, not a thread one (§2.1); `taskpolicy -b` measured it before it was written |
| per-thread `QOS_CLASS_BACKGROUND` via `with_thread_manager` + rayon `spawn_handler` | reject | not inherited (§2.1): two thread factories to reach what one call reaches |
| `taskpolicy -c background` clamp | reject | no self-set API; R3 reads the same as R2 in every column anyway |
| the utility band (`-c utility`, or `QOS_CLASS_UTILITY` per thread) | reject | R4: wall unchanged and the pointer fixed, but the work stays on the performance cores at a lower priority and the person's compile still loses 7%; "as much as possible" is the background band, and `REPOGRAPH_PRIORITY=normal` is the whole of the other direction |
| `threads = 6` (every efficiency core) as the default under the band | reject | R5: −5% wall for +15% CPU-seconds, P6 +3.6% against +1.7%; the six cores share a clock. `threads` keeps its rule and its overrides |
| a lower default `threads` without the band (2, 1) | reject | R6: two performance-core threads are 1.9× the wall and still +9% on the compile; the person feels priority, not count. One thread would be ~4× the wall (H3 of the previous plan: +173% at two) for the same band |
| pacing / duty cycle between forwards | reject | H15: 2.15× the wall for half the CPU average and the same CPU-seconds; under the band there is no one left to pace for |
| arena shrinkage per run, memory pattern off, budget 1,024 / 512 | reject | H10–H13 (§2.3): 0.1–0.2 GB for 5–13% wall, or an unmeasured cost on the short rows; bar 4 is met without them |
| `session.disable_prepacking` for the writers | **accept** | H14: the anonymous footprint 1.74 → 0.61 GB for +12% wall; the weights the GEMMs read are then the memory-mapped file's own pages, clean and reclaimable. Writers only: a reader pays the packing once for a query that runs a few GEMMs, and it opens 0.4 s faster without it, which is a reader measurement for another plan |
| int8 weights | deferred (§2.5) | |
| fewer checkpoints | reject (§2.4) | |
| readers (`ask`, `serve`, `bench`, `dump`) | stay foreground | they answer a person; `serve`'s catch-up sync runs on the answer path (`ask.rs` `resync`, set by `adopt`) and embeds only the rows that moved |
| `watch` | background for its whole life (Task 1), and drops the model between refreshes (Task 3) | it answers nobody; measured 1.37 GB resident after one edit, held until exit |

### 3.2 Defaults and overrides

- `priority`: `"background"` (default) or `"normal"`, in `repograph.toml` **and** the machine file (a machine-shaped setting, like `threads`); `REPOGRAPH_PRIORITY` outranks both for one run. Applies to `build`, `update`, `enrich`, `embed`, `watch`. Readers ignore it.
- `threads`: unchanged — cores / 3 (4 here), `REPOGRAPH_THREADS` and the two files as before. Under the band the count no longer decides what the person feels (R2 and R5 both pass bars 1–3); it decides the wall, and R5 says the fifth and sixth efficiency cores are not worth their CPU-seconds.

### 3.3 Edits, file by file

**`Cargo.toml`**: `libc = "=0.2.189"` under `[dependencies]`.

**`src/priority.rs`** (new):
- `#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)] #[serde(rename_all = "lowercase")] pub enum Priority { #[default] Background, Normal }`.
- `pub fn from_env(configured: Priority, var: Option<&str>) -> Result<Priority>`: empty or unset keeps `configured`; `background` / `normal` (case-insensitive) parse; anything else is an error naming `REPOGRAPH_PRIORITY`.
- `pub fn apply(p: Priority)`: `Normal` does nothing. `Background`: macOS `libc::setpriority(libc::PRIO_DARWIN_PROCESS, 0, libc::PRIO_DARWIN_BG)`; Linux `libc::setpriority(libc::PRIO_PROCESS, 0, 19)`, then `libc::sched_setscheduler(0, SCHED_IDLE, &zeroed sched_param)`, then `libc::syscall(libc::SYS_ioprio_set, IOPRIO_WHO_PROCESS, 0, IOPRIO_CLASS_IDLE << IOPRIO_CLASS_SHIFT)` for the disk side macOS gets free from the same one call; on every other target a single line on stderr naming the setting, so a Windows user reads that it had no effect rather than believing it did. The constants `libc` does not carry for these targets (`SCHED_IDLE`, the three `IOPRIO_*`) are local, with their kernel header named — §3.5.2 lists which and why. A failing call is a warning on stderr, never an error: the rebuild still runs, only louder. One-way on purpose — every caller either exits after its embed or (`watch`) never answers a person — and Linux cannot raise a nice value back without `CAP_SYS_NICE`, so a guard would promise what one platform cannot keep.
- Doc comment carries R1 → R2 (P6 +15.7% → +1.7%, W p99 2.5 ms → 0.6 ms, wall ×4.1) and why the call is process-wide (§2.1).

**`src/config.rs`**: `pub priority: Priority` on `Config` (serde default), `priority: Option<Priority>` on `Machine`, `layer(&named, "priority", machine.priority, &mut cfg.priority)` beside `threads`, then `cfg.priority = priority::from_env(cfg.priority, std::env::var("REPOGRAPH_PRIORITY").ok().as_deref())?`. The doc comment on `Machine` gains the key.

**`src/main.rs`**: `priority::apply(cfg.priority)` in the `Build | Update`, `Enrich`, `Embed` and `Watch` arms, right after `load_cfg()` and **before** `cap_pools` — on Linux nice and SCHED_IDLE are per-thread and inherited at creation, so the rayon pool has to be built after the call; on macOS the order is free. `enrich`'s `claude` children inherit the band too (their network traffic takes the background class); `REPOGRAPH_PRIORITY=normal` is the word back. One comment at the first call site says why the readers have none.

**`src/index/embed.rs`** (Task 2): `session_builder(threads: usize, weights: Weights)` where `pub enum Weights { Packed, Mapped }` lives beside it — `Mapped` adds `.with_prepacking(false)`. `Embedder::open(model, threads, weights)`; `load_session(model, threads, weights)`. The doc comment on `Weights` carries H14 (1.74 → 0.61 GB anonymous, +12% wall) and why a reader keeps `Packed`. `CrossEncoder::open` passes `Weights::Packed` (a reranker answers a person).

**`src/ask.rs`** (Task 2): `embedder_or_notice`, `open_embedder`, `warm_model` take `weights: Weights` and pass it on; the reader call sites in `ask.rs`, `bench.rs`, `dump.rs` and `serve` pass `Packed`; `embed_all` and `run_watch` in `main.rs` pass `Mapped`. The test `the_model_is_not_warmed_for_an_exact_answer_or_a_lexical_arm` gains the argument; no behaviour change.

**`README.md`**: the settings table gains `priority`; the layer row gains `REPOGRAPH_PRIORITY`; the machine-file "may set only" list gains `priority`; the Resources paragraph gains a "what the person sees" table with the R1/R2/full numbers and the sentence that wall time is now the price.

Nothing changes in the store format, the CLI flags or the socket protocol. A `repograph.toml` or machine file without `priority` loads as before (serde default); a file naming it is refused by an older binary (`deny_unknown_fields`), exactly as `threads` was when it arrived. Platforms are §3.5: macOS on Apple Silicon is the measured one, the Intel Mac and Linux rows are reasoned from the API contracts and labelled as such, and Windows and the rest compile to a warning rather than to a silent no-op.

### 3.4 `watch` holds nothing between refreshes

`run_watch` (`src/main.rs`) keeps the embedder and the dense index in two `Option`s for the life of the process once the first change has arrived: 1.37 GB resident on the small model after one edit, about 1.8 GB on the default, held through every quiet poll (`$G/log/watch-after.samples`, README "Resources"). The model opens in ~0.5 s and a poll is 30 s apart, so keeping it warm buys a watcher nothing a person can see, while the held memory is the one thing about `watch` a person on a 16 GB laptop can. The change: the embedder and the index become locals of the `Polled::Refreshed` arm — opened, synced, saved, dropped. The `Option<Option<Embedder>>` and `Option<DenseIndex>` go; `ask::open_embedder` is called per refresh, and the "the model costs ~220 ms and 1.3 GB to open, so it waits for the first change" comment is rewritten to say why it is now dropped after each one. No new function; a small deletion.

### 3.5 Different machines

Everything in §2 was measured on one machine: `Mac15,7`, macOS 26.6.2, arm64, 6 performance + 6 efficiency cores (`hw.perflevel0.physicalcpu` = 6, `hw.perflevel1.physicalcpu` = 6), 36 GB. That is one point in a space the binary ships into, and the single call this plan accepts means something different at each of the other points. Every row below is labelled **measured here** or **reasoned from the API contract**; nothing reasoned is written as if it had been run.

#### 3.5.1 What the call buys, platform by platform

| platform | what `Priority::Background` does | what it buys | how this is known |
|---|---|---|---|
| **macOS, Apple Silicon** | `setpriority(PRIO_DARWIN_PROCESS, 0, PRIO_DARWIN_BG)` | Mach PRI 4 on every thread, 100% of the process's CPU time in the background QoS band, and on Apple Silicon that band is the efficiency cluster: 1.70 GHz against 3.49, the person's compile back from +15.7% to +1.7%, wake p99 from 2.5 ms to 0.6 ms, disk I/O throttled by the same call. Wall ×4.1 | **measured here** — R1 vs R2, §2.2 |
| **macOS, Intel** | the same call, the same task policy | `setpriority(2)` promises exactly three things and no more: "the scheduling priority is set to the lowest value, disk IO is throttled (with behavior similar to using `setiopolicy_np(3)` to set a throttleable policy), and network IO is throttled for any sockets opened after going into background state". None of that names a core type, because on Intel there is only one. So bars 1, 2 and 5 hold by preemption — anything the person runs at default priority takes the core the moment it is runnable — and **bar 3 does not**: with the machine otherwise idle the GEMMs run on the same cores at the same turbo clock, so the fan curve is a foreground run's fan curve and the wall is near the foreground wall rather than 4.1× it | **reasoned from `setpriority(2)` and `getiopolicy_np(3)`**; this session has no Intel Mac and no Intel number is claimed |
| **Linux** | `setpriority(PRIO_PROCESS, 0, 19)`, then `sched_setscheduler(0, SCHED_IDLE, {0})`, then `ioprio_set(IOPRIO_WHO_PROCESS, 0, IOPRIO_CLASS_IDLE << 13)` | SCHED_IDLE runs only when nothing in SCHED_OTHER is runnable, which is bars 1 and 2 by the same preemption argument as Intel; the third call is bar 5, because Linux's nice value does not reach the block layer under `none`/`mq-deadline` and only shades a weight under BFQ, where macOS throttles the disk as part of the one call. No cluster claim: a big.LITTLE arm64 Linux box has an EAS scheduler that may or may not place an idle-class task on a little core, and this plan does not say which | **reasoned from the syscall contracts**, with the libc bindings verified against `libc-0.2.189`'s source in this phase (below); no Linux box in this session |
| **Windows, and every other target** | nothing, plus one line on stderr | a Windows user is told the setting had no effect instead of believing it did. Windows has `SetPriorityClass(PROCESS_MODE_BACKGROUND_BEGIN)`, the near-exact analogue — lowest scheduling priority plus throttled I/O — and it is **named, not implemented**: repograph has no Windows dependency, no Windows CI and no Windows measurement, and a lever nobody here can check is worse than a warning that says so | a no-op is a no-op |

Two consequences worth writing down rather than discovering:

- **The same reading, a different meaning.** `ri_cpu_time_qos_background` reads 100% on an Intel Mac too — the band is the same band. What differs is the hardware under it. §5's V1 check therefore proves the call reached the kernel on any Mac, and proves the *cluster* only on this one.
- **Ordering is a Linux requirement, not a style.** On macOS `PRIO_DARWIN_PROCESS` is a task policy: threads created before and after it all read PRI 4 (§2.1). On Linux both the nice value and the scheduling policy are per-thread and are inherited at `clone`; the I/O context is copied there too. So `priority::apply` must run before `cap_pools` and before the ORT session is committed, or Linux gets a polite main thread and eleven rude workers. §3.3's call sites already sit there; this is why.

#### 3.5.2 The Linux bindings, checked rather than assumed

Read out of `~/.cargo/registry/src/index.crates.io-*/libc-0.2.189/src` in this phase:

- `sched_setscheduler(pid_t, c_int, *const sched_param)` — present for `linux-gnu`, `linux-musl` and `android`.
- `SCHED_IDLE` — **absent** for `linux-gnu` and `linux-musl` (it exists only under `linux_l4re_shared.rs`, `android/mod.rs` and `emscripten/mod.rs`). The module therefore defines it itself as `5`, which is architecture-independent in `include/uapi/linux/sched.h`.
- `sched_param` is `{ sched_priority: c_int }` on gnu and something else on musl, so the parameter is built by `std::mem::zeroed()` rather than by naming the field: SCHED_IDLE requires `sched_priority == 0` on every libc, and a zeroed struct is that on all of them.
- `SYS_ioprio_set` — present for **every** Linux architecture that has a syscall table: checked by listing every file under `linux/gnu`, `linux/musl` and `linux/uclibc` that defines `SYS_read` and confirming each of them also defines `SYS_ioprio_set` (33 files, none missing). Its type is `c_long` everywhere except musl/hexagon, where it is `c_int`, so the call casts. There is no `ioprio_set` wrapper in `libc`, hence `libc::syscall`.
- `IOPRIO_WHO_PROCESS` = 1, `IOPRIO_CLASS_IDLE` = 3, `IOPRIO_CLASS_SHIFT` = 13 are not in `libc` at all and are local constants with the kernel header named in the comment.

Every one of the three Linux calls is best-effort: a failure is one line on stderr and the rebuild continues. A hardened container whose seccomp profile filters `ioprio_set` still rebuilds its graph, only louder. The one thing a rebuild must never do is refuse to run because it could not be made polite.

**One-way, on purpose, and on Linux by necessity.** An unprivileged process may lower its nice value and may enter SCHED_IDLE; raising the nice value back, or leaving SCHED_IDLE for SCHED_OTHER, needs `CAP_SYS_NICE` (`RLIMIT_NICE` can permit part of it and is 0 on most distributions). So on Linux `priority = "normal"` is a decision taken before the run and never during it, and `apply` never tries to restore. macOS could restore and does not, so that the two platforms have one contract.

#### 3.5.3 Core counts: 2 cores and 24

`threads` keeps its rule — `available_parallelism() / 3`, floor 1 (`embed::threads_from`) — and under the band the rule is asked to do less than it was:

| logical cores | default `threads` | the machine that is |
|---|---|---|
| 2 | 1 | a small VM, a CI container with `--cpus=2` |
| 4 | 1 | an entry laptop, a 4-core devcontainer |
| 8 | 2 | a quad-core with SMT |
| 12 | 4 | **this machine** (6 P + 6 E) |
| 16 | 5 | an M-series Max |
| 24 | 8 | a workstation or a build server |

**Small machines: the floor of 1 stays, and one thread in the background band is the right default.** Bar 1 is what the person feels, and one throttled thread is the least any configuration can take; a floor of 2 on a 2-core box would put half the machine in the band, and R5 already read that extra threads on a shared clock cost the probe more than they buy in wall (+3.6% P6 against +1.7%, for 5% of the wall). What one thread costs is wall, and this plan measures it rather than projecting: **R7-darwinbg-t1**, `taskpolicy -b` with `REPOGRAPH_THREADS=1` on the same 320-row store as R2, run in §5 and reported beside R2's 626 s and R5's 594 s. Its number is the answer to "is a 4-core laptop still usable" — and the honest framing is that the person on a 4-core box is not waiting for the rebuild either (bar 6), so a long wall is the price the band charges everywhere, larger there.

**Big machines: `cores / 3` stays, and the thing to change on a build server is `priority`, not `threads`.** R5's six threads were the *whole efficiency cluster sharing one clock*, so its diminishing return is a property of this machine's asymmetry, not of thread counts in general: a 24-core server has 24 equal cores and the trade there is nearer linear. But a build server has nobody at the keyboard, so the setting it wants is `priority = "normal"` (and then `threads` as high as it likes) in `~/.config/repograph/config.toml` — one line, and the machine file is exactly the layer for it, because both keys describe the machine and not the corpus. **A second, band-specific `threads` rule is rejected**: once the band answers bar 1, the count only trades wall against CPU-seconds, and a number with nothing behind it is worse than the one rule already there.

#### 3.5.4 Containers and cgroup limits

`threads` is `available_parallelism() / 3`, and on Linux `available_parallelism` is documented to honour both the process's CPU affinity mask and the cgroup v1/v2 CPU quota. So inside a devcontainer started with `--cpus=2` it reads 2 and the default is one thread — not a third of the *host's* cores, which is the case where "all the cores" is never the machine's cores and the one most likely to be met in practice. **Label: read from the standard library's documented contract for `available_parallelism`; not measured here — this session has no Linux container.** What the call cannot see, and what the README should not promise it does: a `cpu.weight`/`cpu.shares` share (a weight, not a count), and a CPU-set narrowed after the process started. On macOS it reads `hw.logicalcpu` (12 here), which is why the default on this machine is 4 — two thirds of the efficiency cluster.

The band composes with the cgroup rather than replacing it: SCHED_IDLE inside a container is idle *within that container's share*, so a CI box that already bounds repograph with `--cpus` gets both bounds and neither is wasted.

#### 3.5.5 Memory-poor machines: the model, not a flag

The floor is honest and it is not configurable. A full embed under the default model touches 1.63 GB of fp32 weights (R1's `disk read 1627 MB` is exactly the pages of `model.onnx_data` it faulted in), and Task 2 changes *what kind* of memory those are — anonymous packed copies become the mapped file's own clean, reclaimable, shareable pages, footprint 1.74 → 0.61 GB — without changing that max RSS stays around 1.8 GB, because the same pages are still touched. On an 8 GB machine that is a quarter of the machine for the length of the rebuild.

The answer there is a smaller model, and it is a line in `repograph.toml` rather than a flag:

```toml
embed_model = "intfloat/multilingual-e5-small"
```

0.45 GB of weights, 214 s for the whole store against 1,930 s, and it is the model the bench floors were set with. **There is no `--low-memory` flag and this plan does not add one**: such a flag would have to choose between a slower rebuild and a *different index*, and which vectors are on disk is a property of the corpus, not of the machine — which is exactly why `embed_model` is refused from the machine file (§3.3, `config.rs`'s `Machine`). The README's Resources section says this in one sentence (Task 4).

#### 3.5.6 Where the model is unavailable or slow

Nothing to do, and one thing not to break. `--no-dense` opens no model at all — walk, tree-sitter, BM25 and the store save are 1.9 s and 70 MB for the whole tree — and a first run on a machine with no network fails the hub fetch and continues lexical-only through `embedder_or_notice`. `priority::apply` runs before either, so a `--no-dense` writer in the background band is a 1.9 s walk in the background band and nothing else; Task 1 adds no call on that path and Task 2's `Weights` is only reached once a session is being built. §5 keeps `--no-dense` in the floors run (`ARMS="dense lexical"`) so the lexical arm is proof it did not regress.


---

## 4. Tests

All inline `#[cfg(test)]`, no model download, run by `cargo test --release` with no `REPOGRAPH_*` set. The pure parts — the priority's resolution and layering, the platform fallback — are unit-tested; the kernel's side of the call and the memory shape are checked by hand (§5), because no test can assert where the scheduler ran a thread without becoming a measurement.

**`src/priority.rs`** (module `tests`, no model, no threads):
- `the_run_variable_outranks_the_file_and_an_empty_one_keeps_it` — `from_env(Background, None) == Background`; `from_env(Background, Some("normal")) == Normal`; `from_env(Normal, Some("")) == Normal`; `from_env(Normal, Some(" BACKGROUND ")) == Background`.
- `a_priority_the_run_variable_misspells_is_an_error_naming_it` — `from_env(Background, Some("fast"))` errs with `REPOGRAPH_PRIORITY` in the message.
- `applying_normal_touches_nothing` — `getpriority(PRIO_DARWIN_PROCESS, 0)` before and after `apply(Normal)` are equal (macOS only; a no-op assertion elsewhere).
- `lowering_the_process_is_visible_to_the_kernel_and_undone_after_the_test` (`#[cfg(target_os = "macos")]`) — `apply(Background)` makes `getpriority(PRIO_DARWIN_PROCESS, 0)` read 1; the test restores 0 with `setpriority(PRIO_DARWIN_PROCESS, 0, 0)` in a guard so the rest of the test binary does not run in the background band. The one test that touches the process; it is fast and it is the only automated proof the call reaches the kernel.

**`src/config.rs`** (inside `with_machine`, which also clears `REPOGRAPH_PRIORITY`):
- `priority_defaults_to_background_and_reads_from_the_project_file`.
- `the_machine_file_may_set_priority_and_the_project_still_wins`.
- `the_environment_beats_the_project_for_priority`.
- `a_priority_that_is_not_a_word_it_knows_is_an_error_naming_the_file` — `priority = "fast"` errs naming `repograph.toml`.

**`src/index/embed.rs`, `src/ask.rs`** (Task 2): nothing new — `Weights` is one builder entry with no pure part; `the_model_is_not_warmed_for_an_exact_answer_or_a_lexical_arm` gains the argument and asserts what it asserted.

**`src/main.rs`** (Task 3): nothing new — `run_watch` is a loop over a socket-less poll; its `Watcher` tests cover `poll`, and the drop is a scope's end.

**By hand only** (the kernel's side of the call and the memory's shape): thread PRIs (`ps -M`), the QoS bands (`S/probe/rusage`), the probes, the footprint, `watch`'s resident size — §5's commands, recorded under `$G/unnoticeable/log`.

---

## 5. Verification

Run from `$W` after every task, in this order.

```bash
W=/Users/max/Documents/projects/repograph/.claude/worktrees/graph-build-resource-usage-b554bc; B="$W/target/release/repograph"
G=/Users/max/bench/resources-2026-09-07; U="$G/unnoticeable"; L="$U/log"
cargo build --release --manifest-path "$W/Cargo.toml" 2>&1 | tail -1
cargo test --release --manifest-path "$W/Cargo.toml" 2>&1 | grep -E "^test result|FAILED|panicked"
cargo clippy --release --all-targets --manifest-path "$W/Cargo.toml" -- -D warnings 2>&1 | tail -3
```
Expected: every `test result: ok`; the unit count is the shipped **458** (measured at `786b994` in this phase with no `REPOGRAPH_*` set, `S/log/cargo-test-786b994.txt`: `458 passed; 0 failed; 2 ignored`, then `12 passed` for `tests/serve.rs`) plus §4's eight = 466; 2 ignored; 12 in `tests/serve.rs`; clippy silent.

**The band, without `taskpolicy`** — the same 320-row run as R2 through the built binary and no wrapper:
```bash
STORE="$G/store-B" "$U/run2.sh" V1-built-default          # priority from the default
STORE="$G/store-B" "$U/run2.sh" V2-built-normal -- env REPOGRAPH_PRIORITY=normal
```
V1 passes when its line in `$L/summary.txt` reads like R2: `qos: … background <all of it> …` with every other band `0.0`, every thread in `V1-built-default.psM` at PRI 4, `ghz` ≤ 2.0, `probe6_median` ≤ 2,714 ms, `probe1_median` ≤ 2,618 ms, `wake_p99` ≤ 1,000 µs, `maxrss` ≤ 2.0 GB. V2 passes when it reads like R1: band `default`/`legacy`, PRI 31, `ghz` ≥ 3.0 — the opt-out is real. Then `priority = "normal"` in `$G/store-B/repograph.toml` (removed afterwards) reads as V2, and `priority = "normal"` in a temporary `REPOGRAPH_CONFIG` file with the project file silent reads as V2 too.

**The weights, mapped (Task 2)** — V1 again after Task 2 lands, as `V3-built-mapped`: pass when `footprint` (the `peak memory footprint` line of `$L/V3-built-mapped.time`) ≤ 0.8 GB against V1's ≈ 1.64 GB, `maxrss` within 0.1 GB of V1, and `wall` ≤ 1.25 × V1's — H14 read +12% on the performance cores and the efficiency cores have not been asked; more than a quarter and the writers keep `Packed` (Task 2, Step 2).

**The vectors are bit-identical** — nothing in this plan changes an arithmetic: `cmp "$G/store-B/.repograph/vectors.f32" "$L/V0-vectors.f32"` where `V0-vectors.f32` was copied from a run of the `786b994` binary on the same store, exit 0 after Task 1 and again after Task 2 (packing is a layout, not a rounding). That is the proof the bench floors cannot move through either.

**The other machines** (§3.5) — what can be checked here, checked here; nothing else claimed:
```bash
rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-msvc
cargo check --target x86_64-unknown-linux-gnu   # the Linux branch: nice, SCHED_IDLE, ioprio_set
cargo check --target x86_64-pc-windows-msvc     # the warn-once branch
STORE="$G/store-B" REPOGRAPH_THREADS=1 "$U/run2.sh" R7-darwinbg-t1 -- taskpolicy -b
```
The two `cargo check`s are the whole of what a single-platform session can prove about the other platforms: that the code compiles for them and names no constant they do not have. If either target's dependency graph will not check on this machine, the fallback is a scratch crate holding `src/priority.rs` and `libc` alone, checked for the same target — the branch code is what is being compiled, not `ort`. R7 is §3.5.3's small-machine number: one thread under the band on the 320-row store, read beside R2's 626 s (four threads) and R5's 594 s (six). No bar is attached to it — bar 6 says the wall is not one — and it is reported so the rule for a 2- or 4-core box is a number rather than an assumption.

**The full store** — §2.6's run, repeated with the built binary only if §2.6 was read from `taskpolicy` rather than from the code (it was); otherwise the §2.6 line stands as the after number.

**The floors** — run because `Lexical::build` and the walk now run in the background band on the writers, and nothing else:
```bash
cd "$W" && ARMS="dense lexical" NOTE="unnoticeable after" bash bench/history/run-repograph.sh
```
Expected: exit 0 and both lines identical to the `502e8a6d` rows in `bench/history/runs.jsonl` (`keyword 40/40 paraphrase 15/30 code 12/12 p90 221 … dense=true`, `39/40 15/30 12/12 p90 215 … dense=false`).

**`watch`** (Task 3), by hand on `$G/fixture` with the small model pinned, the way `$G/quick.sh` measures it: `ps -o rss= -p <pid>` after the quiet poll, after one edit's refresh, and after the *next* quiet poll. Pass: the third reading is within 0.1 GB of the first (today 1.37 GB, expected ≈ 0.05 GB), and `watch.out` still shows `refresh: … 1 vectors`.

**The readers, unchanged** — `$G/quick.sh`'s ask/bench/dump lines on `$G/fixture`: every wall within 10% and every max RSS within 5% of the after rows in `docs/bench/2026-09-07-resource-usage-results.md` §4.4; `ask-fused.psM`-style `ps -M` on a fused `ask` reads PRI 31 (readers never lowered).

---

## 6. Comment hygiene

Every comment this plan adds says a why the code cannot: why the call is process-wide and not per-thread (§2.1), why it is one-way (§3.3), why the readers have none, why `watch` drops the model it used to keep, why the rayon pool is built after the call. No comment restates a name, a constant or a signature; no ticket ids; the existing `#[allow(clippy::…)]` directives stay.

---

## Tasks

### Task 0: carry the plan phase's scratch out of the session directory

**Files:** none in the repo.

- [x] **Step 1**: `U=/Users/max/bench/resources-2026-09-07/unnoticeable; mkdir -p "$U/log"`; copy `S/probe/{probe,qos,rusage}.c` and their binaries to `$U/probe/`, `S/run2.sh`, `S/chain{1,2,3}.sh`, `S/driver.sh` to `$U/`, `S/log/` to `$U/log-plan/`; edit `run2.sh`'s `S=` line to `$U`. `ls -laR "$U" > "$U/task0.txt"`.
- [x] **Step 2**: confirm the before numbers are on disk: `grep -E "^R[0-9]|^H1[0-5]|^FULL" "$U/log-plan/summary.txt"` prints §2's rows; the idle probe files `R0{a,b}.probe6`, `R0.probe1` are there.

### Task 1: `priority` — the writers in the background band

**Files:** `Cargo.toml`, `src/priority.rs` (new), `src/config.rs`, `src/main.rs`, `README.md`.

- [x] **Step 1 — failing tests first**: the `priority.rs` and `config.rs` tests of §4. `cargo test --release … priority` fails to compile.
- [x] **Step 2**: `libc` in `Cargo.toml`; `priority::{Priority, from_env, apply}` with the platform branches (§3.3); `Config.priority`, `Machine.priority`, the layering and `REPOGRAPH_PRIORITY`; `mod priority;` and the four call sites in `main.rs` before `cap_pools`. Tests green, clippy silent.
- [x] **Step 3**: §5's V1 and V2 runs, the two project/machine-file `normal` reads, the two cross-target `cargo check`s (§3.5), R7, and the `cmp` of the vectors, recorded under `$L`.
- [x] **Step 4**: README edits of §3.3.
- [x] **Step 5**: commit `feat(config): priority puts the writers in the background band` with R1 → R2 → V1 in the body.

### Task 2: the writers read the weights from the mapped file

**Files:** `src/index/embed.rs`, `src/index/cross.rs`, `src/ask.rs`, `src/bench.rs`, `src/dump.rs`, `src/serve.rs`, `src/main.rs`.

- [x] **Step 1**: `Weights`, the `session_builder` / `Embedder::open` / `load_session` signatures, the three `ask.rs` helpers, every call site (§3.3). Tests green (the one renamed signature in `ask.rs`'s test), clippy silent.
- [x] **Step 2**: §5's V3 run: footprint and wall against V1, the `cmp` of the vectors (packing changes no arithmetic: bit-identical, exit 0). If V3's wall is more than 1.25 × V1's on the efficiency cores, `Mapped` stays in the code and the writers keep `Packed` — the commit then says so and carries both numbers.
- [x] **Step 3**: commit `perf(embed): a writer reads its weights from the mapped file instead of packed copies`.

### Task 3: `watch` opens the model for a refresh and drops it after

**Files:** `src/main.rs`.

- [x] **Step 1**: the `run_watch` edit of §3.4; tests green (none touch it — `Watcher` tests exercise `poll`, not the embed), clippy silent.
- [x] **Step 2**: §5's `watch` hand check, three `rss` readings in `$L/t3-watch.txt`.
- [x] **Step 3**: commit `perf(watch): the model is opened for a refresh and dropped after it`.

### Task 4: the results and the docs

**Files:** `docs/bench/2026-09-07-unnoticeable-results.md` (new), `README.md`.

- [x] **Step 1**: §5's floors run and the readers' re-run, outputs under `$L`; the full-store line (§2.6, or its repeat).
- [x] **Step 2**: the results doc in the style of `docs/bench/2026-09-07-resource-usage-results.md`: §1's definition and bars, §2's tables verbatim with their verdicts, the after rows (V1, V2, V3, R7, the full store, `watch`), a **different machines** section carrying §3.5's table with every row labelled measured-here or reasoned-from-the-contract, and what was deferred (§2.5) and why. The README's Resources section gains the same hardware paragraph in three sentences: the band on Apple Silicon, what it does and does not buy elsewhere, and `embed_model = small` as the answer on an 8 GB machine.
- [x] **Step 3**: commit `docs: a rebuild measured against the person at the keyboard`.
- [ ] **Step 4**: open the PR to `devmaxxx/repograph`, base `main`, body = the results doc's summary; do not merge. *(not done: the person opens their own pull requests.)*
