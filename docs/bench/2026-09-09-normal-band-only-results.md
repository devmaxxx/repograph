# The default model's numbers, at three levels, with no band

Machine: Apple Silicon, 12 logical cores (6 performance + 6 efficiency), macOS 25.6.
Corpus: the pinned bench fixture `/Users/max/bench/beauty-crm-502e8a6d` at corpus commit
`502e8a6d` — 33,525 live rows at dim 384, embedded with `intfloat/multilingual-e5-small`, which
is the default. Tool: this branch at `99ab6fe`, `cargo build --release`.
Transcripts: `/Users/max/bench/resources-2026-09-09/log`, one `.time`, `.samples` and `.out` per
run, and a `summary.txt` carrying the line every row below was read from.

Method, copied byte for byte from
[the 2026-09-07 kit](2026-09-07-resource-usage-results.md): `/usr/bin/time -l` for wall, user and
max RSS; a one-second `top -l 2 -s 1 -pid … -stats pid,cpu,mem,th` sampler for CPU% (100% = one
core) and thread count, from which `peak` is the largest sample and the parenthesised figure the
mean of all of them. `REPOGRAPH_NO_SERVE=1` on every row but `serve`. Every writer runs on its own
pristine copy of the fixture's `.repograph` with `vectors.*` removed; the pinned fixture is never
rebuilt and was verified clean afterwards.

> **The machine was not quiet, and every row below was taken on it anyway.** One-minute load
> averages ran between 5.6 and 27.5 across the session, against roughly two to four cores of
> foreign work — a `node` process, FortiClient's `epctrl`, Spotlight, WindowServer — that this
> session could not stop and did not own. `uptime` is recorded either side of every long row in
> `summary.txt`. Two bars are missed and the load is the first suspect for both; where a number is
> load-sensitive it is said so at the row. Nothing here was tuned to a bar.

---

## 1 · The three levels, on a whole-store re-embed

`repograph embed` over a store with `vectors.*` removed: all 33,525 rows, the default model, the
level passed only in `REPOGRAPH_RESOURCES` so the three copies' `repograph.toml` files stay
byte-identical.

| level | threads it asks for | wall | user | sys | max RSS | peak CPU (mean) | threads / running | transcript |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `"full"` | 6 (`cores / 2`) | **168.8 s** | 813.8 s | 19.9 s | 1.63 GB | 382.1% (334.8%) | 12 / 6 | `embed-small-full-cap6` |
| `"balanced"` *(the default)* | 4 (`cores / 3`) | **265.7 s** | 831.4 s | 14.6 s | 1.45 GB | 275.4% (218.1%) | 8 / 4 | `embed-small-full-balanced` |
| `"low"` | 2 (`cores / 6`) | **358.7 s** | 664.7 s | 8.5 s | 1.60 GB | 140.4% (127.1%) | 4 / 2 | `embed-small-full-low` |

`balanced` is the median of three runs — 230.7 s, 265.7 s, 292.0 s
(`embed-small-full-balanced-2`, `…-balanced`, `…-balanced-3`) — taken under one-minute loads of
16.1, 20.2 and 13.2. It was re-run because it missed its bar; the median still misses it. The
other two levels are single runs.

Every level was passed through the environment and reached the pools it was meant to reach: the
thread samples read 8/4, 12/6 and 4/2 for the whole length of each run, and the `balanced` row was
taken with no `REPOGRAPH_RESOURCES` set at all, which is what proves the default is the built-in
rule rather than a word anyone typed.

### 1.1 What `full` should mean, decided by measuring both

`full` had two candidate meanings and this is the one open question the retest existed to settle.
Both were measured on their own pristine copy, back to back, minutes apart:

| `full` resolves to | wall | user | max RSS | peak CPU (mean) | threads / running | transcript |
| --- | --- | --- | --- | --- | --- | --- |
| **`(cores / 2).max(1)` = 6 — shipped** | **168.8 s** | 813.8 s | 1.63 GB | 382.1% (334.8%) | **12 / 6** | `embed-small-full-cap6` |
| no cap at all — rejected | 178.1 s | 815.2 s | 1.61 GB | 390.4% (321.2%) | **18 / 6** | `embed-small-full-nocap` |

The capped shape wins by **9.3 s, 5.2% of the wall**, on **813.8 against 815.2 user seconds** — a
0.17% difference, which is to say the two runs did the identical amount of work and the wall
between them is contention and nothing else. Both run six threads; the uncapped one carries twelve
more that are not running, because leaving both pools to size themselves gives ORT the six
performance cores it would have picked anyway *and* rayon all twelve for the tokenizer, and twelve
tokenizer threads queueing for six cores cost more than they add.

This reproduces, independently and on a different shape, what
[2026-09-07 §4.2](2026-09-07-resource-usage-results.md) read on the 320 longest passages under the
large model: `threads = 6` at 103.4 s and 12 / 6 against the uncapped 110.8 s at 18 / 6, a 6.7%
margin in the same direction. Two measurements, two corpora, two models, one answer.

So `full` means *take the machine*, and takes it in the shape that is both faster and cheaper in
threads rather than the shape that is more literally uncapped. Nothing above `full` is reachable
any more — with `threads` removed there is no hand escape hatch — and on this box `full` is the
fastest setting the tool has, which was **not** true while `threads = N` existed.

The rejected reading is not gone from the record: the no-cap row is published above beside the one
that won, and `threads_from`'s doc comment carries the two numbers so the next person to wonder
does not have to re-run them.

### 1.2 The bars, met and missed

| # | bar | measured | |
| --- | --- | --- | --- |
| C1a | `balanced` wall ≤ 214.1 s | **265.7 s** (median of three; range 230.7–292.0) | **missed** |
| C1a | `balanced` max RSS ≤ 1.63 GB | 1.45 GB | met |
| C1a | `balanced` peak CPU ≤ 310%, mean ≤ 300% | 275.4% (218.1%) | met |
| C1a | `balanced` peak threads ≤ 9 | 8 | met |
| C1b | `full` thread shape — 16–20 with 6 running, the sentinel proof | 18 / 6 uncapped; 12 / 6 shipped | met |
| C1b | `full` peak CPU ≥ 380% | 390.4% uncapped, 382.1% shipped | met |
| C1b | `full` wall ≤ 0.90 × `balanced` (≤ 239.1 s) | 178.1 s uncapped, 168.8 s shipped | met |
| C1b | `full` max RSS ≤ 1.80 GB | 1.61 / 1.63 GB | met |
| C1c | `low` peak CPU ≤ 175%, mean ≤ 165% | 140.4% (127.1%) | met |
| C1c | `low` peak CPU ≤ 0.65 × `balanced`'s (≤ 179.0%) | 140.4%, a ratio of 0.51 | met |
| C1c | `low` threads ≤ 5 with 2 running | 4 / 2 | met |
| C1c | `low` max RSS ≤ 1.63 GB | 1.60 GB | met |
| C1c | `low` wall between 1.7× and 2.3× `balanced` (451.6–611.0 s) | **358.7 s, a ratio of 1.35** | **missed, below the band** |

**C1a is missed, and the miss is the finding rather than an accident of it.** The 214.1 s bar was
taken from 2026-09-07 §1.1's `embed-A-small-full` row, which the README has been quoting as the
default model's current cost. That row was measured **uncapped** — 18 threads, 6 running, 399%.
The shipped default has been four threads since the cap landed, and nobody had ever measured the
whole store at four. The plan expected the token budget and the chunked sync that landed since to
more than pay the cap back; on this machine they do not. Like for like, the budget did pay: the
uncapped shape went 214.1 s → 178.1 s, **17% faster than the row the bar was cut from**. It is the
cap, not a regression, that puts the default at 265.7 s, and the README's 214 s was not merely
stale — it understated the default's real cost by about a quarter.

Under load the honest range is what is published: 230.7–292.0 s, median 265.7 s. The bar stands
missed; the number is not moved to meet it.

**C1c's wall is missed below its band for the same reason.** The band was 1.7×–2.3× of
`balanced`, and `low` came in at 1.35×. Both halves of that ratio are measurements, and the
denominator is the one under strain: a four-thread run loses proportionally more to two to four
cores of foreign work than a two-thread run does. Against the `balanced` wall the plan *expected*
(150–185 s), `low`'s 358.7 s is 1.94×–2.39×, straddling the band it was written for. `low`'s own
job — hold the machine down — it does exactly: 140.4% peak against `balanced`'s 275.4%, a ratio of
0.51 where the bar allowed 0.65, and 664.7 user seconds against 831.4, so it is not merely slower,
it is doing less to the machine per second of it.

### 1.3 Progress cadence

`embed`'s 33 progress lines, from the median `balanced` run's `.out`:

```
dense: 1024/33525 rows, 45.5 rows/s, ~12 min left
dense: 2048/33525 rows, 56.8 rows/s, ~9 min left
dense: 3072/33525 rows, 65.6 rows/s, ~8 min left
```

The rate quoted on the first line is a cold average that has not yet shed model open, so the first
estimate is long and every later one shortens. That is [G19](next-version-gaps.md) and is not
re-litigated here; the lines are recorded because they were free.

---

## 2 · The writers that embed nothing

On a copy of the fixture with its vectors present, at `balanced`, in this order — the first finds
every row already embedded, the second wipes the graph and rebuilds it without the dense arm, the
third finds nothing changed.

| run | wall | user | max RSS | bar | |
| --- | --- | --- | --- | --- | --- |
| `build` (embeds nothing) | 1.72 s | 1.46 s | 1.57 GB | ≤ 3.0 s, ≤ 1.63 GB | met |
| `--no-dense build` | 1.04 s | 0.96 s | 0.07 GB | ≤ 3.0 s, ≤ 0.15 GB | met |
| `--no-dense update`, nothing changed | 0.07 s | 0.04 s | 0.07 GB | ≤ 0.5 s, ≤ 0.15 GB | met |

---

## 3 · The readers

At `balanced` only. C5 is a guard against the **removal**, not a hypothesis about the levels: no
reader ever called the deleted `priority::apply`, and a level reaches the same two pools `threads`
reached with no per-command path in it. Bar: within `max(0.05 s, 20%)` of the 2026-09-07 §4.4
after-row and within 0.15 GB of its max RSS; `ask --rerank-local` ≤ 40 s and ≤ 3.20 GB.

| run | 2026-09-07 | 2026-09-09 | RSS then | RSS now | |
| --- | --- | --- | --- | --- | --- |
| `ask` fused | 0.35 s | 0.48 s (median of 3) | 1.36 GB | 1.54 GB | **missed by 0.06 s and 0.03 GB** |
| `--no-dense ask` | 0.12 s | 0.12 s | 0.10 GB | 0.10 GB | met |
| `ask --stale` | 0.28 s | 0.42 s (median of 3) | 1.36 GB | 1.54 GB | **missed by 0.086 s and 0.03 GB** |
| `ask --rerank-local` | 31.9 s | 34.6 s | 3.09 GB | 2.94 GB | met (≤ 40 s, ≤ 3.20 GB) |
| `ask FR-PAY-22` | 0.04 s | 0.04 s | 0.05 GB | 0.05 GB | met |
| `impact cn --depth 3` | 0.04 s | 0.04 s | 0.05 GB | 0.04 GB | met |
| `trace main cn --depth 6` | 0.04 s | 0.04 s | 0.05 GB | 0.04 GB | met |
| `changes --depth 2` | 0.07 s | 0.41 s (median of 3) | 0.07 GB | 0.21 GB | **missed — explained below** |
| `--no-dense bench` | 0.19 s | 0.21 s | 0.20 GB | 0.19 GB | met |
| `bench` | 1.47 s | 1.66 s | 1.42 GB | 1.55 GB | met |
| `dump --depth 300`, 10 queries | 0.70 s | 0.77 s | 1.36 GB | 1.54 GB | **RSS missed by 0.03 GB** |
| `import-legacy`, 41 MB | 0.14 s | 0.10 s | 0.15 GB | 0.12 GB | met |
| `serve`, sampled idle | — | — | 1.38 GB | 1.59 GB | **RSS missed by 0.06 GB** |
| `watch`, after one doc edit | — | — | 1.37 GB | 0.44 GB | met (lower; the sample lands 12 s after the edit and the refresh had finished) |

**`changes` is a corpus difference, not a regression, and it is checkable.** The 2026-09-07 copy of
the fixture is git-clean; the pinned fixture, and therefore every copy taken from it since, carries
four modified files under `graphify-out/` — one of them a 41 MB `graph.json` — plus an untracked
one. `changes` diffs the working tree, so it walks all of that before it reaches the one-hunk edit
the row is about. `git -C … status --porcelain` on both copies is the whole evidence.

**Every dense reader is up by about the same 0.18 GB of max RSS**, and three of them clear the
0.15 GB bar by 0.03–0.06 GB. A single systematic offset across unrelated commands, on a machine
under memory pressure from three to four cores of foreign work, is an environmental reading rather
than something these two commits did: nothing in either touches how a reader opens its model.
The two `ask` walls are up by 0.06–0.09 s on the same machine, medians of three, and are read the
same way — [G23](next-version-gaps.md) says these bars are tighter than the suite's repeatability
and this is what that looks like.

**The unmeasured corner, said out loud.** `ask --rerank-local` under `resources = "full"` opens the
2.1 GB cross-encoder at six threads rather than four, and it is the one reader a level can move.
It is not measured here: the level was designed for the writers, every other reader is under a
second, and a bar nobody ran is worse than a gap somebody named.

---

## 4 · The checkpoint, interrupted and resumed

At `balanced`, on its own copy. `embed` is killed with `SIGTERM` after its second progress line;
the second run must embed exactly the rows the first did not.

```
--- first run, killed after its second progress line ---
dense: 1024/33525 rows, 106.3 rows/s, ~5 min left
dense: 2048/33525 rows, 134.1 rows/s, ~4 min left
--- what the checkpoint left on disk ---
rows 2048 dim 384
--- the second run ---
dense: embedded 31477 rows in 169.5s
--- the finished store's row count ---
rows 33525 dim 384
```

33,525 − 2,048 = **31,477**, which is the second run's total exactly. The finished store answers
`штраф за отмену записи` with `FR-CAL-95`, `FR-TOOL-35`, `N-025`, `FR-PAY-26`, `FR-DM-48`, in that
order — the five ids 2026-09-07 §4.3 recorded. Bar C6 met on both halves.

One footnote on how that answer was taken, because the transcript would otherwise look odd. The
resumed store is a bare `.repograph` with no source tree beside it, and an `ask` against it
refreshes against an empty tree and empties the graph — which is what happened here, to this
session's own probe, after the row counts had already been recorded. The graph was restored from
the pinned fixture (it is the same bytes the copy started from; the vectors are the resumed run's)
and the answer taken in a tree. The row-count half of C6 predates the probe and is untouched by it.

---

## 5 · The floors

```
ARMS="dense lexical" NOTE="levels only" bench/history/run-repograph.sh
```

Exit 0, both rows appended to `bench/history/runs.jsonl` against corpus `502e8a6d`, tool `99ab6fe`:

```
keyword 40/40  paraphrase 15/30  code 12/12  p90 221 tok  dense=true   enriched=true  model=small  gated=true  green=true
keyword 39/40  paraphrase 15/30  code 12/12  p90 215 tok  dense=false  enriched=true  model=small  gated=true  green=true
```

Identical to the `502e8a6d` rows already in the history. This is a guard and not a hypothesis:
removing a scheduling call and renaming a thread count cannot change which nodes a query
retrieves, no ranking, index, gate or store format is touched by either commit, and this run
re-embeds nothing. It is run because a floor that is only asserted is not a floor.

Both rows record `tool_dirty: true`. The tree at floor time held exactly one uncommitted thing —
this plan's own document, untracked until the docs commit — plus `runs.jsonl`, which the script
appends to before it reads the tree's state. No source file was dirty.

---

## 6 · What changed and what did not

**Changed.** The default model's published whole-store cost. The README has carried
**214 s / 1.63 GB** since 2026-09-07; the default's real cost on this machine is **265.7 s /
1.45 GB**, and the 214 s figure was an uncapped row standing in for a four-thread default. The
same shape measured today is 178.1 s, so the work between the two dates got 17% cheaper while the
number being quoted got 24% too optimistic.

**Changed.** How much of the machine a rebuild takes is a word. `full` / `balanced` / `low` are
6 / 4 / 2 threads here and 168.8 / 265.7 / 358.7 seconds, at 382% / 275% / 140% peak CPU. Two keys
were spent to buy it: `priority` and `threads` are both gone, and a file naming either is refused
by name.

**Not changed.** `balanced` is `threads = 0`'s old rule unmoved, and its run sampled 8 threads with
4 running for its whole length with nothing set in the environment — the same shape the shipped
default has had since the cap landed. The token budget, the batch size and the 1,024-row
checkpoint chunk are untouched and no level reads them. The retrieval floors are identical to the
row. Every reader is within noise or explained.

**Not re-measured, deliberately.** The e5-large whole-store row (1,930 s / 2.15 GB, 2026-09-07
§4.1) is 32 minutes for a model that is not the default, and it was taken in the normal band —
which is the only band there is again, so it describes today's behaviour. The one caveat: it
predates the mapped-weights change that shipped in the same PR as the band, priced at +6% to +26%
of wall under memory pressure and free otherwise ([G22](next-version-gaps.md)). If that row is
wanted current it is one detached `embed` against a copy pinned to e5-large, and it blocks nothing.
