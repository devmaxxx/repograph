# A rebuild measured against the person at the keyboard

The request was one sentence: reduce resource usage further, as much as possible, so that the
person does not notice when repograph is working. "Does not notice" is not a number, so the first
job was to make it one — and then to find the lever that moves it, which turned out to be a single
system call and not any of the six things that looked more promising.

Machine: `Mac15,7`, macOS 26.6.2, arm64, 12 logical cores = 6 performance + 6 efficiency
(`hw.perflevel0.physicalcpu` = 6, `hw.perflevel1.physicalcpu` = 6), 36 GB. Corpus: the pinned
`beauty-crm` fixture at `502e8a6d` — 908 files, 8,316 nodes, 32,601 edges, 33,525 live dense rows.
Two stores are used throughout: **store-B**, the 320 longest passages (all ≥ 256 tokens, no
`repograph.toml`, so the default e5-large model), and the whole store under the same model.

Every row names a transcript under `$U = ~/bench/resources-2026-09-07/unnoticeable`: `log-plan/`
holds the rows measured while the plan was being written, `log/` the rows measured from the built
binary, and `log/summary.txt` carries the line each table row was read from. The probes, their
sources and the runner scripts are in `$U/probe/` and `$U/*.sh`.

**A note on the machine, because it is a person's own laptop and not a bench rig.** Every probe
row names the idle baseline it is read against. The morning's baseline was 2,583–2,585 ms for P6
and 269–291 µs for the wake p99 (`$U/log-plan/R0{a,b}.probe6`), and the R1–R6 and V1–V3 rows were
all taken against it. From about 12:05 the machine picked up work that is not this measurement's —
Spotlight reindexing everything this session had written, then a large clang build, load average
peaking near 200 with 0% idle — and the idle probe alone read 2,610 to 8,366 ms while that lasted.
Rows taken after that point carry their own idle control (`R0c`) and say so. A number that could
not be taken on a quiet machine is reported as what it is rather than as what it would have been.

**Before:** the shipped state at `786b994` — `threads` = cores / 3 (4 here), token-budgeted
batches, checkpoints every 1,024 rows. A full re-embed of 33,525 rows read 1,930 s wall, 7,641 s
user, 2.15 GB max RSS, 293% peak CPU, every thread on a performance core
(`docs/bench/2026-09-07-resource-usage-results.md`). That document bounded what the rebuild costs.
This one asks a different question: what it costs *the person sitting there while it runs*.

## 1. What "unnoticeable" means here, and how it is measured

A person at the keyboard notices four things: their own work slowing down, the pointer and the
typing stuttering, the fan, and the machine starting to swap. Each is measured directly, with the
rebuild running beside it, rather than through the process's own CPU% — a process at 260% of a core
on the efficiency cluster is invisible, and one at 100% of a performance core is not.

### 1.1 The probes

`$U/probe/probe.c` (C, `clang -O2`), started 20 s into every run so the session is open and the
forwards are under way, on the same machine as the rebuild:

- **P6** — six threads, the performance-core count, at default QoS, each running 1.5 × 10⁹
  iterations of a dependent xorshift-and-fma chain; eight rounds; the median round's wall. This is
  the person's compile, or their browser laying out a page.
- **P1** — the same with one thread, four rounds: the single-threaded app.
- **W** — one thread at `QOS_CLASS_USER_INTERACTIVE` that `nanosleep`s 1 ms in a loop for the whole
  probe and records how late it woke; p50, p99 and max in µs. This is an app's main thread waiting
  on the next input event or frame.

Where the rebuild's CPU time actually ran, read from the kernel rather than inferred:

- **Band** — `proc_pid_rusage(RUSAGE_INFO_V4)`'s `ri_cpu_time_qos_*` counters, polled by
  `$U/probe/rusage.c` until the process exits: the seconds the scheduler charged to each QoS band.
- **PRI** — `ps -M -p <pid>`, the Mach priority of every thread: 31 is the default band, 4 the
  background one.
- **Clock** — `cycles elapsed / (user + sys)` from `/usr/bin/time -l`: this machine's performance
  cores sustain ~3.5–4.0 GHz, its efficiency cores ~1.7–2.7 GHz.

Memory is `/usr/bin/time -l`'s `maximum resident set size` and `peak memory footprint` — the
anonymous, compressible part, which is what memory pressure and jetsam act on. Disk is
`ri_diskio_byteswritten` from the same rusage sample.

**What could not be measured:** `powermetrics` (CPU power, per-cluster residency, fan) needs root,
and `sudo -n true` answers that a password is required; `ri_billed_energy` reads 0 on this machine.
The fan's proxy is therefore the band and the clock: work confined to the efficiency cluster at
1.70 GHz is the low-power configuration Apple's own scheduler puts background work in, and it is
what the fan curve does not follow. That is a proxy and it is named as one.

### 1.2 The bars

Idle baselines (`$U/log-plan/R0{a,b}.probe6`, `R0.probe1`, the probe alone with nothing else
running):

| probe | idle |
|---|---|
| P6 median | 2,585 ms and 2,583 ms |
| P1 median | 2,493 ms |
| W | p50 257–259 µs, p99 269–291 µs, max 5–14 ms (the machine's own jitter) |

A rebuild is **unnoticeable** when, with it running:

1. **P6 within 5% of idle** (≤ 2,714 ms) and **P1 within 5%** (≤ 2,618 ms).
2. **W p99 ≤ 1 ms**: a frame is 8.3–16.7 ms, so a wake that lands within a millisecond is never a
   dropped one. The p50 is not a bar — it sits at ~505–515 µs under any load at all, against 258
   idle, which is the kernel coalescing a 1 ms timer on a busy machine.
3. **100% of the process's CPU time in the background band**, every thread at PRI 4, average clock
   ≤ 2.0 GHz.
4. **Memory:** max RSS ≤ 2.0 GB, peak footprint ≤ 1.8 GB, swapouts unchanged.
5. **Disk:** ≤ 300 MB written for a whole-store rebuild.
6. **Wall** is reported beside every row and **is not a bar**: the person is not waiting for it.

## 2. The levers, measured before they were designed

All on store-B through `$U/run2.sh` / `$U/run3.sh`: `/usr/bin/time -l`, a one-second `top` sampler,
the rusage poller, `ps -M` at t = 20 s, then P6 and P1 inside the run. `wall` therefore includes
~35 s of the probes competing for the machine; the clean shipped wall for this store is 133.3 s.

### 2.1 Mechanism first: what a QoS call actually reaches (`$U/probe/qos.c`)

| call | what a thread created afterwards reads | reversible |
|---|---|---|
| `pthread_set_qos_class_self_np(QOS_CLASS_BACKGROUND)` on the main thread | `qos=default` — **not inherited**; ORT's workers are created inside `commit_from_file`, rayon's inside `build_global`, and both would stay in the default band | per thread |
| `setpriority(PRIO_DARWIN_PROCESS, 0, PRIO_DARWIN_BG)` | `getpriority` reads 1 in every thread; `ps -M` shows PRI 4 on all eight | yes, with `prio = 0` |
| `taskpolicy -c background` (a QoS clamp) | main reads `qos=background`, children `default`, the clamp holds them all | set through `posix_spawnattr` only — no call a running process can make on itself |

So the per-thread route would need `SessionBuilder::with_thread_manager` **plus** a rayon
`spawn_handler` to reach the band that one process-wide call reaches. `taskpolicy -b` *is*
`setpriority(PRIO_DARWIN_PROCESS, 0, PRIO_DARWIN_BG)` (`man taskpolicy`), so every `-b` row below
measured the code's effect before the code was written.

### 2.2 Scheduling levers

| run | wrapper | threads | wall | user / sys | max RSS / footprint | CPU peak (avg) | clock | band (PRI) | P6 | P1 | W p99 | verdict |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| R1-shipped-t4-fg | none | 4 | 152.3 s | 571.9 / 4.5 s | 1.83 / 1.63 GB | 262% ¹ | 3.49 GHz | default 100% (31) | **2,991 ms (+15.7%)** | 2,585 (+3.7%) | **2,469 µs** | the before: fails bars 1–3 |
| R2-darwinbg-t4 | `taskpolicy -b` | 4 | 626.3 s (4.1×) | 2,255 / 42.8 s | 1.82 / 1.64 GB | 280% (259%) | **1.70 GHz** | **background 100%** (4) | **2,630 ms (+1.7%)** | 2,481 (−0.5%) | **603 µs** | **passes 1–4** |
| R3-clampbg-t4 | `taskpolicy -c background` | 4 | 624.6 s | 2,254 / 41.5 s | 1.82 / 1.64 GB | 280% (259%) | 1.70 GHz | background 100% (4) | 2,631 (+1.8%) | 2,489 (−0.2%) | 569 µs | = R2 in every column: the clamp and the task policy land in the same band |
| R4-clamputil-t4 | `taskpolicy -c utility` | 4 | 158.9 s (1.0×) | 600.5 / 5.8 s | 1.83 / 1.63 GB | 277% (264%) | **3.38 GHz** | utility 100% (20) | **2,767 (+7.0%)** | 2,526 (+1.3%) | 849 µs | the compromise: no wall cost, the pointer fixed, the compile still 7% slower — **fails bar 1** |
| R5-darwinbg-t6 | `taskpolicy -b`, 6 threads | 6 | 594.1 s (−5% on R2) | 2,602 / 66.5 s (+15%) | 1.80 / 1.64 GB | 378% (313%) | 1.50 GHz | background 100% (4) | 2,678 (+3.6%) | 2,507 (+0.6%) | 731 µs | six efficiency cores share one clock: a third more CPU-seconds buys 5% of wall and costs the probe two points — **four stays** |
| R6-fg-t2 | 2 threads | 2 | 288.1 s (1.9×) | 555.7 / 4.0 s | 1.77 / 1.64 GB | 150% (138%) | 3.52 GHz | default 100% (31) | **2,820 (+9.1%)** | 2,529 (+1.4%) | 812 µs | half the threads on the performance cores: twice the wall and the compile still 9% slower — **fails bar 1**; the count is the wrong lever |

¹ R1's sampler followed the wrapper's pid; its 262% is read from `ps -M` at t = 20 s, its band and
clock from a rusage poller started late on the right pid and from `/usr/bin/time -l`'s cycles.

Reading R1 against R2: the same 8.43 × 10¹² instructions retired in both; on the efficiency cluster
they took 3.92 × 10¹² cycles instead of 2.01 × 10¹² (half the IPC) at 1.70 GHz instead of 3.49
(half the clock), which is the 4×. The `sys` time growing from 4.5 s to 42.8 s is the throttled
band's scheduling overhead, and it is charged to the background band too.

### 2.3 Memory and pacing levers — the harness

`$U/harness` (repograph's tokenize → forward → pool loop with the levers as arguments), same 320
texts, four threads, foreground:

| run | switch | wall | user | max RSS | footprint | verdict |
|---|---|---|---|---|---|---|
| H16 control (today's build, every switch at its default) | budget 2,048 | 145.5 s | 556.9 s | 1.86 GB | 1.74 GB | the memory control; max RSS varies 1.82–1.93 GB run to run at this shape, so only the footprint column separates the levers |
| H10 | arena shrinkage on every run | 151.0 s (+13%) | 561.7 s | 1.82 GB | 1.74 GB | **reject**: the arena is given back after each forward and grown again before the next, and the arena is not what fills the footprint |
| H11 | memory pattern off | 147.2 s (+10%) | 560.9 s | 1.82 GB | 1.63 GB | **reject**: −0.11 GB for +10% wall |
| H12 | budget 1,024 | 141.3 s (+5%) | 541.9 s | 1.78 GB | 1.62 GB | reject with H13 |
| H13 | budget 512 | 132.5 s (−1%) | 515.1 s | 1.74 GB | 1.55 GB | **reject**: −0.19 GB on 320 long rows, where the budget only turns 8 × 256 forwards into 2 × 256; on the short rows that are nine tenths of a store it would run 1,300 forwards where the shipped budget runs 776, unmeasured |
| H14 | `session.disable_prepacking` | 150.2 s (+12%) | 584.9 s | 1.92 GB | **0.61 GB** | **accept for the writers** — see §4.2 |
| H15 | duty cycle 50% | 288.1 s (2.15×) | 556.7 s | 1.93 GB | 1.74 GB | **reject** as a default: CPU average 240% → 120% for twice the wall and the same CPU-seconds, a linear trade with nothing free in it. Under the band the pointer and the compile are already at idle, so pacing could only lower the efficiency cluster's load, which nobody at the keyboard shares |

H14's 1.13 GB of anonymous memory was MLAS's packed copies of the 24 layers' GEMM weights
(12.6 M parameters × 4 B × 24 ≈ 1.2 GB), made at session open. Without them the GEMMs read the
memory-mapped `model.onnx_data` pages, which are clean, reclaimable and shared between processes.

### 2.4 Disk

R2 wrote 1.3 MB for 320 rows: one checkpoint. `PRIO_DARWIN_BG` throttles the process's disk I/O as
well (`setpriority(2)`: "disk IO is throttled"), so the rows go out in the background I/O tier
without a second call. Fewer or larger checkpoints: rejected — a minute's loss on interrupt was the
previous plan's trade and nothing here is noticeable.

### 2.5 Quantized weights — named, not measured

The hub lists `onnx/model_qint8_avx512_vnni.onnx` for both models (562 MB for the large, 118 MB for
the small); there is no `model_quantized.onnx` and no fp16. It is int8 quantized for AVX-512 VNNI;
on arm64 its GEMMs would run through MLAS's NEON kernels at an unmeasured speed, its vectors differ
from the fp32 ones, and `written_by` records only the hub id, so a reader with the fp32 weights
would search int8 rows at the same width and never know. Measuring it means a 562 MB download and a
full re-embed plus `bench` for the floors. **Deferred, not proposed:** the download is a decision
for the person, and the scheduling lever already leaves memory the one bar this work does not move.

## 3. What shipped

| lever | verdict |
|---|---|
| `setpriority(PRIO_DARWIN_PROCESS, 0, PRIO_DARWIN_BG)` in the writers | **shipped** — every bar 1–4 met from a single call; reaches ORT's and rayon's threads because it is a task policy, not a thread one |
| `priority` in `repograph.toml`, the machine file and `REPOGRAPH_PRIORITY` | **shipped** — `"background"` (default) or `"normal"` |
| `session.disable_prepacking` for the writers | **shipped** — footprint 1.63 → 0.50 GB for 1.006× the wall (§4.2) |
| `watch` drops the model between refreshes | **shipped** — idle footprint 932 → 130 MB (§4.3) |
| per-thread `QOS_CLASS_BACKGROUND` via `with_thread_manager` + rayon `spawn_handler` | rejected: not inherited (§2.1) — two thread factories to reach what one call reaches |
| `taskpolicy -c background` clamp | rejected: no self-set API, and R3 reads the same as R2 anyway |
| the utility band | rejected: R4, the work stays on the performance cores and the compile still loses 7% |
| `threads = 6` under the band | rejected: R5, −5% wall for +15% CPU-seconds and two points of P6 |
| a lower default `threads` without the band | rejected: R6, two threads are 1.9× the wall and still +9% on the compile |
| pacing / duty cycle | rejected: H15 |
| arena shrinkage, memory pattern off, budget 1,024 / 512 | rejected: H10–H13 |
| int8 weights | deferred (§2.5) |
| fewer checkpoints | rejected (§2.4) |
| readers (`ask`, `serve`, `bench`, `dump`) | stay foreground and stay packed — they answer a person |

## 4. After

### 4.1 The band, through the code

The same store-B run as R2, through the built binary with no wrapper: V1 takes the band from the
code's own default, V2 opts out with `REPOGRAPH_PRIORITY=normal`.

| run | wall | user / sys | max RSS / footprint | clock | band | PRI | P6 | P1 | W p99 | disk written |
|---|---|---|---|---|---|---|---|---|---|---|
| R1 shipped, before | 152.3 s | 571.9 / 4.5 s | 1.83 / 1.63 GB | 3.49 GHz | default 100% | 31 | 2,991 ms (+15.7%) | 2,585 (+3.7%) | 2,469 µs | — |
| **V1 built, default** | 658.0 s | 2,264.7 / 50.5 s | **1.82 / 1.63 GB** | **1.71 GHz** | **background 2,315 s of 2,315** | **4 on all 8** | **2,657 ms (+2.8%)** | **2,419 (−3.0%)** | **564 µs** | 1.3 MB |
| **V2 built, `REPOGRAPH_PRIORITY=normal`** | 148.0 s | 567.1 / 3.1 s | 1.83 / 1.63 GB | 3.49 GHz | legacy 570 s | 31 on all 8 | 2,974 ms (+15.0%) | 2,568 (+3.0%) | 2,897 µs | 1.3 MB |
| the bars | not a bar | — | ≤ 2.0 / ≤ 1.8 GB | ≤ 2.0 GHz | 100% background | 4 | ≤ 2,714 ms | ≤ 2,618 ms | ≤ 1,000 µs | ≤ 300 MB |

**Every bar is met.** V1 lands 57 ms under the P6 bar rather than R2's 84 — the difference between
2,630 and 2,657 is within the probe's own spread — and its P1 comes in *faster than idle*, which is
what a probe that has the performance cores to itself looks like. V2 reads the band, the Mach
priority and the clock of the shipped state, so the opt-out is real and not a spelling. The same
two readings come from `priority = "normal"` in `repograph.toml` and from `priority = "normal"` in
a machine file with the project file silent, and a project file saying `background` still beats a
machine file saying `normal` — six arms, each read as the Mach priority of every thread 25 s in
(`$U/log/t1-layers.txt`):

```
built-in default (nothing set)                 4 threads at PRI 46   4 threads at PRI 4
REPOGRAPH_PRIORITY=normal                      8 threads at PRI 31
priority = normal in repograph.toml            8 threads at PRI 31
priority = normal in the machine file          8 threads at PRI 31
project background beats machine normal        8 threads at PRI 4
back to the default                            4 threads at PRI 46   4 threads at PRI 4
```

The 46s are not a third band: a thread blocked faulting the mapped weights in carries macOS's
I/O-wait boost for as long as it is blocked, and on a machine with memory to spare (V1 and V3, when
it had some) all sixteen readings are 4.

**The vectors are unchanged.** `cmp` against a copy of what the `786b994` binary wrote for the same
store: exit 0 for V1 and exit 0 for V2. The band moves no arithmetic, so no bench floor can move
through it.

### 4.2 The weights, read from the mapped file

V3 is V1 again with the writers' sessions built `session.disable_prepacking`:

| run | wall | max RSS | **peak footprint** | clock | P6 | P1 | W p99 |
|---|---|---|---|---|---|---|---|
| V1, packed | 658.0 s | 1.82 GB | 1.63 GB | 1.71 GHz | 2,657 ms | 2,419 ms | 564 µs |
| **V3, mapped** | 662.2 s (**1.006×**) | 1.82 GB | **0.50 GB** | 1.80 GHz | 2,637 ms | 2,487 ms | 562 µs |
| the bars | ≤ 1.25 × V1 | within 0.1 GB of V1 | ≤ 0.8 GB | — | ≤ 2,714 | ≤ 2,618 | ≤ 1,000 |

Two thirds of the anonymous memory goes and the wall does not move. **The wall is the surprise.**
The harness read +12% for this switch on the performance cores (H14); on the efficiency cluster,
where a writer now runs, the packed layout buys back nothing measurable — 4.2 s on 658, which is
inside the run-to-run spread. Max RSS is unchanged because the same pages are still touched; what
changed is that they are now the mapped file's, clean and reclaimable, rather than the process's
own dirty copies.

**The vectors are *not* bit-identical, which the plan expected them to be.** The packed and
unpacked MLAS kernels sum in a different order and float addition is not associative. Measured
rather than waved at:

| | store-B, 320 rows × 1024 d | whole store, 33,526 rows × 384 d |
|---|---|---|
| floats differing | 317,834 of 327,680 (97.0%) | 11,763,708 of 12,873,984 (91.4%) |
| largest component difference | 4.69 × 10⁻⁷ (on components up to 0.25) | 1.49 × 10⁻⁷ |
| smallest row-to-row cosine | 0.99999895 | 0.99999904 |

Small is not a floor, so it was measured as one. A whole 33,526-row store was embedded twice
through the same binary pair — once packed, once mapped — and benched both ways
(`$U/log/t2-floors-packed.txt`, `t2-floors-mapped.txt`):

```
packed  keyword 40/40  paraphrase 15/30  code 12/12  p90 221 tok  dense=true
mapped  keyword 40/40  paraphrase 15/30  code 12/12  p90 221 tok  dense=true
packed  keyword 39/40  paraphrase 15/30  code 12/12  p90 215 tok  dense=false
mapped  keyword 39/40  paraphrase 15/30  code 12/12  p90 215 tok  dense=false
```

Case for case and token for token, with 11.7 M of 12.9 M floats different between the two indexes.

Writers only. A reader opens a session, runs a handful of GEMMs for one query and leaves, so it
would pay the unpacked layout on every one of them; `ask`, `bench`, `dump`, `serve` and the
cross-encoder keep the packed weights.

**And mapped is not free on a machine that is already short of memory** — §5.5 measures the other
side of this trade, which V3 could not see because V3 had headroom.

### 4.3 `watch` holds nothing between refreshes

`run_watch` kept its embedder and its dense index in two `Option`s for the life of the process once
the first change had arrived. They are now locals of the refresh arm: opened, synced, saved,
dropped. Measured on the fixture with the small model pinned, `--every 5`, one edit, then sampled
every 10 s for two minutes — three arms of the same script, differing only by the binary
(`$U/log/t3-watch.txt`, `t3-steady-*.txt`):

| | resident (`ps -o rss=`) t=0 / 10 s / 120 s | **physical footprint** t=0 / 10 s / 120 s |
|---|---|---|
| packed weights, model held (before) | 0.05 / 0.29 / 0.18 GB | 39.2 M / 1.5 G / **931.8 M** |
| mapped weights, model held | 0.05 / 1.55 / 0.03 GB | 39.2 M / 1.5 G / **932.0 M** |
| **mapped weights, model dropped (now)** | 0.05 / 0.44 / 0.03 GB | 39.3 M / **129.5 M** / **129.5 M** |

Mapping the weights moves a small-model watcher by 0.2 MB; dropping the model is the whole **7.2×**.
The refresh itself still peaks at 1.5 GB in every arm, because the session has to exist while the
row is embedded; what changed is whether it is still there a minute later, and a watcher spends
almost all its life a minute later.

Three readings that need saying rather than rounding off:

- **`ps -o rss=` disagrees with the footprint in both directions here.** The held session's
  anonymous pages are compressed within two minutes, so its resident size falls to 0.18 GB while
  931.8 MB is still charged to it. The dropped session's file-backed weight pages stay resident for
  about 100 s after it is gone, so a reading taken 8 s after the refresh says 0.44 GB where the
  footprint already says 129.5 MB. The earlier documents' `watch` rows are resident sizes; this
  table is why the footprint column was added.
- **Under refreshes 10 s apart, five in a row**, the footprint sits at 1.5–1.7 GB instead:
  `MALLOC_LARGE (empty) 1.2G resident 1.1G` in `vmmap`'s region table — memory the process has
  freed and the system allocator has not returned (`$U/log/t3-deep-after.txt`). A watcher polling
  every 30 s on human edits does not reach that cadence. No allocator call was added to force the
  return: that is a design change with its own measurement to earn.
- A refresh whose model cannot be opened now says so again on the next refresh instead of once, and
  a watcher started before the model was downloaded picks it up when it appears.

### 4.4 The whole store — attempted twice, not obtained

This is the one row of §5 this session could not take, and the reason is worth more than the row
would have been.

The plan's own full-store run (`taskpolicy -b`, four threads, the `786b994` binary) was started
before this session and stopped at **7,168 of 33,525 rows after 34.7 minutes**, its rate still
climbing — 2.2, 2.6, 3.0, 3.2, 3.3, 3.4, 3.5 rows/s across its first seven checkpoints against the
shipped foreground run's 10.0 on the same first chunk (`$U/log/FULL-plan-partial.time`). It was
stopped deliberately: a run of the *shipped binary*, with the mapped weights in it, is the row this
document wants, and repeating it later cost the same wall clock as letting that one finish. Its
in-run probes did land, at t = 60–100 s, and they are the only whole-store probe readings there
are: every thread at PRI 4, **P6 2,714 ms (+5.0%) — on the bar, not under it**, P1 2,511 (+0.7%),
W p50 504–509 µs and p99 512–626 µs. The three points between R2's +1.7% and that +5.0% are the
difference between 320 long rows and a whole store's mix: the tokenizer's length pass and the many
short forwards on rayon's four threads are more memory traffic beside the probe than eight
256-token forwards are.

The shipped binary's run was then started twice and starved both times. On the second attempt it
managed **80.9 seconds of CPU in 15.5 minutes of wall** and never reached its first checkpoint. That
is not a defect and `sample` says so: the process is inside ONNX Runtime's thread pool doing
forwards, not blocked on a lock or on I/O. By then the machine had picked up an Android emulator
(5.5 GB), Android Studio (4.0 GB), two JVMs, `lldb-rpc-server` (4.2 GB), Xcode and Chrome, with
64 MB of free pages and a load average that had peaked near 200. **A background-band rebuild on a
machine like that runs when the machine is free, and that is the whole of what this work bought.**
The trade is stated in bar 6 and it is real in both directions: the person did not wait for the
rebuild, and the rebuild waited for the person.

**And it is not the whole store that is the problem — it is the band on a busy machine.** The same
store-B that V1 and V3 each finished in 11 minutes was started again in the band under the
afternoon's load and stopped after 8.1 minutes with **81.4 seconds of CPU**, an average of 17% of one
core against V1's 231% (`$U/log/Q1-partial.txt`). The load average was 5.6 and the machine had
twelve cores, so it was not saturated in any sense a person would recognise — but the background
band on Apple Silicon *is* the efficiency cluster (§2.2, measured), and the efficiency cluster is
also where macOS puts its own background work, an emulator's idle threads and every utility-class
daemon on the machine. **The band's throughput depends on how busy the six efficiency cores are,
not on how busy the machine looks**, and that inference — measured band, measured 17%, inferred
contention — is the honest shape of it, because `powermetrics` would have shown the per-cluster
residency directly and it needs root.

A third run was left going, detached, when this document was written. Its line lands in
`$U/log/summary.txt` as `FULL-built-default` and its transcripts beside it; the two numbers it
carries that store-B cannot are the whole-store wall and **bar 5**, the bytes a whole rebuild
writes. Bar 5 can also be read off the file sizes without it: a checkpoint appends its chunk to
`vectors.f32` (1,024 rows × 1024 d × 4 B = 4 MB) and rewrites `vectors.json` whole (3.0 MB at full
size, less earlier), 32 of them, plus the final `vectors.f32` of 137 MB already counted in the
appends and one 22 MB graph save — **≈ 180 MB, and under the 300 MB bar** — which is consistent
with the 1.3 MB that V1's single checkpoint of 320 rows actually wrote.

### 4.5 The floors did not move

`ARMS="dense lexical" bench/history/run-repograph.sh` against the pinned fixture, built at
`766ae26`, exit 0, both arms green and gated, both lines identical to the `502e8a6d` rows already
in `bench/history/runs.jsonl`:

```
keyword 40/40  paraphrase 15/30  code 12/12  p90 221 tok  dense=true   enriched=true  model=small  gated=true
keyword 39/40  paraphrase 15/30  code 12/12  p90 215 tok  dense=false  enriched=true  model=small  gated=true
```

The fixture's rows are the small model's and were not re-embedded, so this run is proof about the
walk, `Lexical::build` and the readers rather than about the vectors; §4.2's packed/mapped A/B is
the proof about the vectors, and it re-embedded a whole store to get it.

### 4.6 The readers, unchanged

`$U/quick.sh` re-run end to end (`$U/log/readers-after.txt`), then a controlled A/B, because the
suite's own numbers had drifted against
`docs/bench/2026-09-07-resource-usage-results.md` §4.4 by more than §5's ±10% wall / ±5% RSS bars —
`ask-fused` read 0.58 s against 0.35, `bench-dense` 1.65 s against 1.47, `dump10` 0.90 s against
0.70.

**The drift is not the change, and the control says so.** The same three commands run alternately
through the pre-change binary and the shipped one, twice each, on the same fixture
(`$U/log/readers-control.txt`):

| | before-binary | shipped binary |
|---|---|---|
| ask-fused | 0.75 s / 1.56 GB, 0.61 s / 1.55 GB | 0.65 s / 1.54 GB, 0.60 s / 1.36 GB |
| ask-stale | 0.52 s / 1.56 GB, 0.66 s / 1.36 GB | 0.62 s / 1.55 GB, 0.60 s / 1.37 GB |
| dump10 | 1.35 s / 1.55 GB, 1.22 s / 1.54 GB | 1.37 s / 1.55 GB, 1.17 s / 1.55 GB |
| bench-dense | 1.67 s / 1.54 GB, 1.67 s / 1.55 GB | 1.90 s / 1.54 GB, 1.73 s / 1.55 GB |

Max RSS bounces between 1.36 and 1.56 GB run to run **on both binaries**, which is a wider spread
than the 5% bar the plan set: the bar is tighter than the measurement's own repeatability at this
shape, and the honest statement is the A/B and not the pass. Two causes for the wall drift, neither
of them code: the fixture's index grew from 33,526 to 33,554 rows over this session's `watch` and
`update` measurements (28 of them now free), and the suite's first command pays for whatever
re-extraction the previous command left behind — which is also why `changes` reads 0.16 s here
against 1.11 s in the earlier suite run, the same artefact with the sign reversed.

`ps -M` on a fused `ask` reads PRI 31: readers are never lowered.

## 5. Different machines

Everything above was measured on one machine. The single call this work leans on means something
different at each of the other points in the space the binary ships into, so each row below is
labelled **measured here** or **reasoned from the API contract**, and nothing reasoned is written
as if it had been run.

### 5.1 What the call buys, platform by platform

| platform | what `priority = "background"` does | what it buys | how this is known |
|---|---|---|---|
| **macOS, Apple Silicon** | `setpriority(PRIO_DARWIN_PROCESS, 0, PRIO_DARWIN_BG)` | Mach PRI 4 on every thread, 100% of the process's CPU time in the background band — and on Apple Silicon that band is the efficiency cluster: 1.71 GHz against 3.49, the person's compile back from +15.7% to +2.8%, wake p99 from 2.5 ms to 0.56 ms, disk I/O throttled by the same call. Wall 4.3× | **measured here** — R1 against V1 |
| **macOS, Intel** | the same call, the same task policy | `setpriority(2)` promises three things and no more: "the scheduling priority is set to the lowest value, disk IO is throttled (with behavior similar to using `setiopolicy_np(3)` to set a throttleable policy), and network IO is throttled for any sockets opened after going into background state". None of that names a core type, because on Intel there is only one. Bars 1, 2 and 5 hold by preemption — anything the person runs at default priority takes the core the moment it is runnable — and **bar 3 does not**: with the machine otherwise idle the GEMMs run on the same cores at the same turbo clock, so the fan curve is a foreground run's fan curve and the wall is near the foreground wall rather than four times it | **reasoned from `setpriority(2)` and `getiopolicy_np(3)`**; no Intel Mac in this session and no Intel number is claimed |
| **Linux** | `setpriority(PRIO_PROCESS, 0, 19)`, then `sched_setscheduler(0, SCHED_IDLE, {0})`, then `ioprio_set(IOPRIO_WHO_PROCESS, 0, IOPRIO_CLASS_IDLE << 13)` | SCHED_IDLE runs only when nothing in SCHED_OTHER is runnable, which is bars 1 and 2 by the same preemption argument as Intel; the third call is bar 5, because a nice value does not reach the block layer under `none` or `mq-deadline` and only shades a weight under BFQ, where macOS throttles the disk inside its one call. No cluster claim is made: a big.LITTLE arm64 Linux box has an EAS scheduler that may or may not place an idle-class task on a little core | **reasoned from the syscall contracts**, with the bindings checked against `libc-0.2.189`'s source and the branch compiled for the target (§5.2); no Linux box in this session |
| **Windows, and every other target** | nothing, plus one line on stderr: `priority: background is not implemented on this platform, so this run keeps normal priority` | a Windows user is told the setting had no effect instead of believing it did. Windows has `SetPriorityClass(PROCESS_MODE_BACKGROUND_BEGIN)`, the near-exact analogue — lowest scheduling priority plus throttled I/O — and it is **named, not implemented**: there is no Windows dependency, no Windows CI and no Windows measurement here, and a lever nobody can check is worse than a warning that says so | the branch is compiled for `x86_64-pc-windows-msvc` (§5.2); the rest is a no-op |

Two consequences worth writing down rather than discovering:

- **The same reading, a different meaning.** `ri_cpu_time_qos_background` would read 100% on an
  Intel Mac too — it is the same band. What differs is the hardware under it. §4.1's V1 check
  proves the call reached the kernel on any Mac; it proves the *cluster* only on this one.
- **Ordering is a Linux requirement, not a style.** On macOS `PRIO_DARWIN_PROCESS` is a task
  policy: threads created before and after it all read PRI 4 (§2.1). On Linux the nice value and
  the scheduling policy are per-thread and inherited at `clone`, and the I/O context is copied
  there too. So `priority::apply` runs before `cap_pools` and before any session is committed, or
  Linux would get a polite main thread and eleven rude workers.

### 5.2 The other platforms, compiled rather than assumed

A single-platform session can prove exactly one thing about the other platforms: that the code
compiles for them and names no constant they do not have. It was proved.

`cargo check --target x86_64-unknown-linux-gnu` on the whole crate fails in `openssl-sys`, which
needs a Linux OpenSSL toolchain and has nothing to do with this code, so the check was run on a
scratch crate holding `src/priority.rs`, `libc`, `anyhow` and `serde` and nothing else — the branch
code is what is being compiled, not `ort`. It passes for `x86_64-unknown-linux-gnu`,
`x86_64-pc-windows-msvc` and `aarch64-apple-darwin`. That the Linux arm is genuinely compiled and
not skipped was checked by breaking it on purpose: a bad constant inside the `#[cfg(target_os =
"linux")]` block fails the Linux check with `cannot find value NOT_A_CONST` and passes the macOS
one untouched.

Read out of `libc-0.2.189`'s source in the same pass:

- `sched_setscheduler(pid_t, c_int, *const sched_param)` — present for `linux-gnu`, `linux-musl`
  and `android`.
- `SCHED_IDLE` — **absent** for `linux-gnu` and `linux-musl`; it exists only under
  `linux_l4re_shared.rs`, `android/mod.rs` and `emscripten/mod.rs`. The module defines it itself as
  `5`, which is architecture-independent in `include/uapi/linux/sched.h`.
- `sched_param` is `{ sched_priority: c_int }` on gnu and shaped differently on musl, so the
  parameter is built by `std::mem::zeroed()` rather than by naming the field: SCHED_IDLE requires a
  priority of 0 on every libc, and a zeroed struct is that on all of them.
- `SYS_ioprio_set` — present for **every** Linux architecture with a syscall table: every file
  under `linux/gnu`, `linux/musl` and `linux/uclibc` that defines `SYS_read` also defines it, 33
  files with none missing. Its type is `c_long` everywhere except musl/hexagon, where it is
  `c_int`, so the call casts. There is no `ioprio_set` wrapper in `libc`, hence `libc::syscall`.
- `IOPRIO_WHO_PROCESS` = 1, `IOPRIO_CLASS_IDLE` = 3, `IOPRIO_CLASS_SHIFT` = 13 are not in `libc` at
  all and are local constants with their kernel header named beside them.

Every one of the calls is best-effort: a failure is one line on stderr and the rebuild continues.
A hardened container whose seccomp profile filters `ioprio_set` still rebuilds its graph, only
louder. The one thing a rebuild must never do is refuse to run because it could not be made polite.

**One-way, on purpose, and on Linux by necessity.** An unprivileged process may lower its nice
value and may enter SCHED_IDLE; raising the nice value back, or leaving SCHED_IDLE for SCHED_OTHER,
needs `CAP_SYS_NICE` (`RLIMIT_NICE` can permit part of it and is 0 on most distributions). So on
Linux `priority = "normal"` is a decision taken before the run and never during it, and `apply`
never tries to restore. macOS could restore and does not, so that the two platforms have one
contract.

### 5.3 Core counts: 2 cores and 24

`threads` keeps its rule — `available_parallelism() / 3`, floor 1 — and under the band it is asked
to decide less than it used to:

| logical cores | default `threads` | the machine that is |
|---|---|---|
| 2 | 1 | a small VM, a CI container with `--cpus=2` |
| 4 | 1 | an entry laptop, a 4-core devcontainer |
| 8 | 2 | a quad-core with SMT |
| 12 | 4 | **this machine** (6 P + 6 E) |
| 16 | 5 | an M-series Max |
| 24 | 8 | a workstation or a build server |

**Small machines: the floor of 1 stays.** Bar 1 is what the person feels, and one throttled thread
is the least any configuration can take; a floor of 2 on a 2-core box would put half the machine in
the band, and R5 already read that extra threads on a shared clock cost the probe more than they
buy in wall. What one thread costs is wall, and it was measured rather than projected —
**R7-built-t1**, the shipped binary with `REPOGRAPH_THREADS=1` and the band from its own default,
on the same store-B as R2.

**It was attempted and it could not be finished, for the same reason §4.4's run could not.** R7 ran
66 minutes and accumulated 24.5 minutes of CPU before it was stopped: with one thread in the
background band on a machine running someone else's build, the thread gets what is left, and what
was left fell to 17% of a core. The number that machine noise cannot eat is the CPU time, and the
run that gives it is the same work with one thread in the **foreground**, where it competes fairly:

| run | threads | band | wall | user + sys | max RSS / footprint | clock |
|---|---|---|---|---|---|---|
| R1 | 4 | foreground | 152.3 s | 576.4 s | 1.83 / 1.63 GB | 3.49 GHz |
| R6 | 2 | foreground | 288.1 s | 559.7 s | 1.77 / 1.64 GB | 3.52 GHz |
| **R8-fg-t1** | **1** | **foreground** | **606.1 s** | **575.6 s** | **1.70 / 0.47 GB** | 3.56 GHz |
| V1 | 4 | background | 658.0 s | 2,315.2 s | 1.82 / 1.63 GB | 1.71 GHz |

One thread foreground is 606 s, which is 3.98 × the four-thread 152 s: on the performance cores the
count trades linearly and nothing is lost to it — the CPU-seconds are flat at 560–576 s across one,
two and four threads. The band's own cost is a measured multiplier on the same work: V1's 2,315
CPU-seconds against R1's 576 is **4.02×**, all of it the halved clock and the halved IPC of the
efficiency cluster. So **one thread in the background band on store-B is ≈ 2,300 s, about 38
minutes**, and R7's 1,471 CPU-seconds before it was stopped are consistent with that. This number
is a product of two measurements rather than one, and it is labelled as such.

What that says for a 2- or 4-core box: the rebuild takes roughly four times its foreground wall and
its foreground wall is roughly four times this machine's, so an hour where this machine takes ten
minutes — and bar 6 is why that is the default anyway. The smaller machine gets one thing back that
this one does not: **one thread's arena is smaller**, so R8's peak footprint is 0.47 GB against
V3's 0.50 and its max RSS 1.70 GB against 1.82.

**Big machines: `cores / 3` stays, and the thing to change on a build server is `priority`.**
R5's six threads were the *whole efficiency cluster sharing one clock*, so its diminishing return
is a property of this machine's asymmetry rather than of thread counts in general: a 24-core server
has 24 equal cores and the trade there is nearer linear. But a build server has nobody at the
keyboard, so what it wants is `priority = "normal"` — and then `threads` as high as it likes — in
`~/.config/repograph/config.toml`. One line, and the machine file is exactly the layer for it,
because both keys describe the machine and not the corpus. **A second, band-specific `threads` rule
is rejected**: once the band answers bar 1 the count only trades wall against CPU-seconds, and a
number with nothing behind it is worse than the one rule already there.

### 5.4 Containers and cgroup limits

`threads` is `available_parallelism() / 3`, and on Linux `available_parallelism` is documented to
honour both the process's CPU affinity mask and the cgroup v1/v2 CPU quota. So inside a
devcontainer started with `--cpus=2` it reads 2 and the default is one thread — not a third of the
*host's* cores, which is the case where "all the cores" is never the machine's cores and the one
most likely to be met in practice. **Read from the standard library's documented contract for
`available_parallelism`; not measured here — this session has no Linux container.** What the call
cannot see, and what the README does not promise it does: a `cpu.weight`/`cpu.shares` share (a
weight, not a count), and a CPU set narrowed after the process started. On macOS it reads
`hw.logicalcpu` — 12 here, which is why the default is 4, two thirds of the efficiency cluster.

The band composes with the cgroup rather than replacing it: SCHED_IDLE inside a container is idle
*within that container's share*, so a CI box that already bounds repograph with `--cpus` gets both
bounds and neither is wasted.

### 5.5 Memory-poor machines: the model, not a flag

The floor is honest and it is not configurable. A full embed under the default model touches
1.63 GB of fp32 weights, and §4.2 changes *what kind* of memory those are — anonymous packed copies
become the mapped file's own clean, reclaimable, shareable pages, footprint 1.63 → 0.50 GB —
without changing that max RSS stays at 1.82 GB, because the same pages are still touched. On an
8 GB machine that is a quarter of the machine for the length of the rebuild.

**Mapping the weights cuts both ways here, and this is the measurement V3 could not make.** Clean,
reclaimable pages are pages the system is free to reclaim, and a machine with nothing spare reclaims
them and makes the run read them again. Both binaries were run alternately on store-B, in the
foreground so the scheduler is not part of the answer, while the machine held an Android emulator,
Android Studio, two JVMs, Xcode and Chrome, with 64–115 MB of free pages
(`$U/log/summary.txt`, the `P1`/`P2` rows):

| | wall | max RSS | peak footprint | disk **read** |
|---|---|---|---|---|
| packed, round 1 | 164.5 s | 1.57 GB | 1.64 GB | 1,572.8 MB |
| mapped, round 1 | 206.5 s (+26%) | 1.52 GB | **0.48 GB** | 1,896.4 MB |
| packed, round 2 | 157.9 s | 1.73 GB | 1.64 GB | 381.9 MB |
| mapped, round 2 | 167.7 s (+6%) | 1.73 GB | **0.50 GB** | 1,577.9 MB |

The footprint column is constant and it is the win: 1.14 GB less of the memory that decides which
process the system compresses, swaps or kills. The wall column is the price, and on a machine with
headroom it is 0.6% (V3) while on this one it is 6–26%. The disk column says why: mapped reads
1.6–1.9 GB on every run, which is the weight file coming back from disk after the system took the
pages, where packed reads it once and keeps its own copy. A single-threaded run under the same
pressure read 7.5 GB (`R8-fg-t1`), the same effect with a longer run to evict across.

That is the honest shape of the trade on a memory-poor machine: **less likely to be the process the
system kills, more likely to wait on the disk it kept nothing in.** It is still the right default
for a writer, because a rebuild is not waiting on anything and a jetsam kill loses the run — but a
person on 8 GB should take the model below, not the mapping, as their lever.

The answer there is a smaller model, and it is a line in `repograph.toml` rather than a flag:

```toml
embed_model = "intfloat/multilingual-e5-small"
```

0.45 GB of weights, 214 s for the whole store against 1,930 s, and it is the model the bench floors
were set with. **There is no `--low-memory` flag and this work does not add one**: such a flag would
have to choose between a slower rebuild and a *different index*, and which vectors are on disk is a
property of the corpus, not of the machine — which is exactly why `embed_model` is refused from the
machine file.

### 5.6 Where the model is unavailable or slow

Nothing to do, and one thing not to break. `--no-dense` opens no model at all — walk, tree-sitter,
BM25 and the store save are 1.9 s and 70 MB for the whole tree — and a first run on a machine with
no network fails the hub fetch and continues lexical-only. `priority::apply` runs before either, so
a `--no-dense` writer in the background band is a 1.9 s walk in the background band and nothing
else. The floors run keeps the lexical arm (§4.5) as the proof that it did not regress.

## 6. Side findings, not fixed here

- The `peak memory footprint` of a **reader** is not measured anywhere in this document or the last
  one. `ask --rerank-local` reads 3.1 GB of max RSS, and how much of that is anonymous decides
  whether it is a problem on a 16 GB laptop. Cheap to measure, and it belongs to a readers plan.
- `Weights::Mapped` opens a session 0.4 s faster than `Weights::Packed` (H14's `session open`
  column: 0.27 s against 0.75 s), which is a reader-side win of the same size as the whole of a
  fused `ask`. It was not taken here because the reader then pays the unpacked layout on every GEMM
  of its query, and nobody has measured which is larger for a one-query process.
- The allocator holds up to 1.1 GB of freed large blocks after a burst of `watch` refreshes
  (§4.3). `malloc_zone_pressure_relief` would return them; whether that is worth a macOS-only call
  in the refresh loop is a question with a measurement attached, and the measurement was not taken.
- `Weights::Mapped` under memory pressure re-reads the weight file (§5.5). If that ever matters
  enough to fix, the shape of the fix is `madvise(MADV_WILLNEED)` on the mapping, or choosing the
  packed layout when `memory_pressure` reports the machine is already short — both of which are
  policy in the writer rather than a builder entry, and neither is measured here.
- The **fan** is still a proxy, not a reading. `powermetrics` needs root and this session had no
  password; the band and the clock stand in for it. Someone with `sudo` could close that in ten
  minutes and it is the only bar in §1.2 that is argued rather than measured.
