# Where a fused `ask` spends its time — P1, P2 and P3 measured

Three performance levers landed on `feat/perf` between 2026-09-05 and 2026-09-06: stop asking the
model hub for a file the small model never had (P1), open the model while the lexical work runs
(P2), and answer from a resident process over a Unix socket (P3). None of them may change an
answer, and none of them did. This file is what they cost and what they bought.

Every number below comes from a named transcript under `/Users/max/bench/perf-2026-09-06` (`$S`
throughout), or — for the five rows this plan did not measure itself — from the design note
`docs/superpowers/specs/2026-09-05-perf-and-tokens-design.md`, which is on the `feat/weak-spots`
branch and not this one. Those are attributed as the note's and read back out of the note's own
transcripts, which survive and are copied under `$S/perf5-spec-*.txt`. The fixture is the pinned
`beauty-crm` worktree at `502e8a6d` (908 files, 8.3k nodes, enriched); `serve` is measured on a
copy of it with its source tree, never on the fixture, because `serve` refreshes and writes.

The fixture's store records no embedding model, so it resolves through `UNNAMED_MODEL` and answers
as a small-model store. Every timing here is that store's. `main` has since made
`intfloat/multilingual-e5-large` the default, which does not move these numbers — the store names
its own model — but it does divide the levers: P1 removes a 404 for a weights file the small model
has never had, and the large model's weights genuinely do live beside its graph, so that lookup was
always a cache hit for it. P1's saving is the small model's. P2, P3a and `serve` are
model-independent.

## The table is one sitting, and that is the point

The levers were implemented and measured over five separate sittings, and this machine drifts
by more than the effects under measurement. The *same* binary read 369.8 ms in one sitting and
346.1 ms an hour later (Task 3), and one sitting ran under an unrelated job that Task 4's prose
puts at ~590 % CPU — a figure read off `top` at the time, with no transcript behind it. Assembling
the stored per-task medians into a before/after table would publish that drift as if it were the
levers' work.

So the four historical binaries were rebuilt and measured **in one sitting, interleaved** — base,
P1, P2, head, base, P1, … — seven rounds, both arms, so the drift is spread across all four arms
rather than landing on one.

| arm | commit | what it is | binary `sha1` |
| --- | --- | --- | --- |
| base | `336066e` | the branch before P1; behaviourally the pre-plan `ask` | `7c0179d9106dddcbb46e5b2904bbb008af9e963b` |
| P1 | `6ba8d0b` | no hub round trip in `fetch` | `dcc8721ba2f807691cb76c2dee8dd6cf73f199a2` |
| P2 | `1148642` | the model opened beside the lexical work | `5c7c629bb0ba18646da18007fa15efcee920f9c2` |
| head | `4ec3062` | P3a and P3b, answering in-process, no server | `b2a1f14f79d94380725f72bc565bb31d9f4c2916` |

Each was built from `git archive <commit>` into a scratch directory and a shared target dir
(`$S/perf5-build.sh`, log `$S/perf5-build.log`, shas `$S/perf5-binaries.txt`); the branch's own
worktree and HEAD were never moved. The shas are in the table because the first build round
produced four identical ones: `git archive` stamps the *commit's* mtimes, which are older than the
artifacts already in a shared target dir, so cargo called every build fresh and copied the first
commit's binary out under all four names. The script now `touch`es the extracted tree, comparing
the shas is what caught it, and [`runbook.md`](runbook.md) carries the trap.

Those binaries are not byte-identical to the ones the tasks committed, because the source path is
compiled in, so the head build was checked against the committed one on five ask shapes, two
streams each: **10 of 10 identical**, 797 / 311 / 408 / 1525 / 5966 bytes — the same sizes every
round of Task 4 reports (`$S/perf5-headcheck.txt`). The committed build reproduced its own
`sha1 d85d38a618f3f69446c4815222d01c202b57e4dc` from Task 4.

**Method.** `REPOGRAPH_TIMING=1 <binary> [--no-dense] ask --stale 'штраф за отмену записи'` in the
fixture, one warm-up round discarded, then seven measured rounds; medians of seven.
Script `$S/perf5-stages.py`, raw per-run stage lines `$S/perf5-stages-raw.txt`, table
`$S/perf5-stages-table.txt`. A second sitting two minutes later, `$S/perf5-stages-raw-2.txt` and
`$S/perf5-stages-table-2.txt`, repeats the whole thing as a check.

**Machine.** Load average 8.4 → 7.0 (one-minute) across the pinned sitting and 4.2 → 3.5 across
the check, written round by round into `$S/perf5-stages-table.txt` and
`$S/perf5-stages-table-2.txt`. That is the whole of the recorded evidence about the machine, and it
carries the argument on its own — what makes the table safe is the interleaving, not the quiet. Two
things were seen in `top` at the time and captured in no transcript, so read them as observations
rather than as measurements: FortiClient's `epctrl` held about one core throughout, and nothing of
the size Task 4 blames for its own timing pair was running.

## The stage table

Medians of seven, one interleaved sitting, `$S/perf5-stages-table.txt`. Totals are wall time from
the process's own start; the bracketed figure is that stage's own step.

### Dense arm

| stage | base `336066e` | P1 `6ba8d0b` | P2 `1148642` | head `4ec3062` |
| --- | ---: | ---: | ---: | ---: |
| graph loaded | 24.8 (+24.8) | 24.3 (+24.3) | 24.4 (+24.4) | 24.3 (+24.3) |
| ids ready | 25.9 (+1.1) | 25.4 (+1.1) | 25.5 (+1.1) | 25.4 (+1.1) |
| questions ready | 48.3 (+22.4) | 48.0 (+22.5) | 47.9 (+22.4) | 48.2 (+22.8) |
| vectors loaded | 63.8 (+15.5) | 63.5 (+15.5) | 63.7 (+15.7) | 63.5 (+15.3) |
| model opened | 475.2 (+410.5) | 277.4 (+213.3) | 283.9 (+220.7) | 283.7 (+220.4) |
| query embedded and searched | 487.7 (+12.5) | 290.4 (+13.0) | 296.3 (+12.5) | 296.2 (+12.6) |
| answered | 537.6 (+49.9) | 339.3 (+48.8) | 297.2 (+0.9) | 297.0 (+0.8) |
| **printed** | **537.6** | **339.3** | **297.2** | **297.0** |
| min / max of seven | 509.3 / 567.4 | 335.4 / 346.1 | 294.6 / 304.6 | 294.1 / 307.2 |
| spread | 58.1 | 10.7 | 10.0 | 13.1 |

Cumulatively **537.6 → 297.0 ms, −240.6 ms, 1.81×**: P1 −198.3, P2 −42.1, P3a −0.2.

### Lexical arm (`--no-dense`)

| stage | base | P1 | P2 | head |
| --- | ---: | ---: | ---: | ---: |
| graph loaded | 26.2 (+26.2) | 25.7 (+25.7) | 24.4 (+24.4) | 24.4 (+24.4) |
| ids ready | 27.3 (+1.1) | 26.8 (+1.1) | 25.5 (+1.1) | 25.4 (+1.1) |
| questions ready | 49.9 (+22.7) | 49.6 (+22.7) | 48.0 (+22.4) | 48.4 (+23.0) |
| answered | 101.4 (+51.0) | 100.3 (+50.9) | 100.7 (+52.8) | 101.8 (+53.6) |
| **printed** | **101.4** | **100.4** | **100.7** | **101.8** |
| min / max of seven | 99.6 / 102.8 | 100.0 / 103.3 | 98.3 / 111.3 | 99.2 / 104.7 |
| spread | 3.2 | 3.3 | 13.0 | 5.5 |

The lexical arm is flat, which is the correct result: no lever changes what a `--no-dense`
question measurably costs — P2 removed a dead `exact_seeds` scan from that path, worth 0.1–0.4 ms,
and nothing else on it moved. The 1.4 ms between the highest and the lowest of the four medians is
smaller than any one binary's own spread across its seven runs (3.2–13.0 ms).

### The check sitting

Two minutes later, same method (`$S/perf5-stages-table-2.txt`): dense **534.9 / 329.4 / 301.5 /
294.2**, lexical **100.1 / 99.9 / 98.4 / 97.1**. Every step reproduces — `model opened` 407.3 →
206.9 with P1, the `answered` step 48.8 → 0.9 with P2 — and the four-arm shape is the same. The
two sittings differ by at most 10 ms on any arm, against a P1 effect of 198 ms and a P2 effect of
42 ms.

## What each lever actually did

### P1 — `6ba8d0b`, `src/index/embed.rs`

`fetch` asked `hf-hub` for `onnx/model.onnx_data` on every fused `ask`. The small model has no such
file, nothing is cached under that name, and the client therefore went to the network to be told
404. The lookup now happens only when the cached `model.onnx` is a graph-only stub under 64 MB.

The `model opened` step falls **410.5 → 213.3 ms, −197.2 ms**, and the whole dense ask falls
198.3 ms with it: the round trip was pure latency in front of the answer. The design note's own
breakdown of the open, measured three times (`$S/perf5-spec-open-parts.txt`), named the right
cause and the right order of magnitude and overshot the size by 21–30 %: the absent file cost
238.2 / 255.5 / 242.6 ms of a 299.2 / 236.5 / 253.4 ms `fetch`, against the 197.2 ms actually
recovered, while the four files that *are* cached resolved in 0.07–0.65 ms.

It is also what makes the dense arm work offline: an unreachable hub used to cost the connect
failure or its timeout on every question.

Note the spread column. Base's dense ask varies by 58.1 ms across seven runs while P1's varies by
10.7 — the network round trip was the largest source of run-to-run noise as well as the largest
single cost.

### P2 — `1148642`, `src/main.rs` and `src/query.rs`: the interesting finding

**As the plan specified it, P2's cost equalled its gain.** The design note's premise was that the
model open would overlap "ids, questions, three BM25 builds and the fusion", 55–70 ms of work a
fused answer does anyway. Two of those were already finished before the thread could start: it is
spawned after the vectors load, because the vectors are what name the model, and `ids ready`
(25 ms) and `questions ready` (48 ms) both precede that. The BM25 builds, which are the expensive
part, did not overlap at all — `query::ask` called the dense retriever **first** and only then
built the lexical lists, so when the join began to wait, no index had started building. All that
was left beside the open was the closure setup and the second `exact_seeds` scan inside `ask`,
which Task 2 §5 bounds at roughly 3.7 ms and measures at 0.5 ms on a one-word question.

That scan is also the whole of what the lever cost. Spawning the thread means deciding first
whether the question is a whole id — an id is answered without the model at all — so the `Cmd::Ask`
arm gained a pre-spawn `query::exact_seeds(&graph, &ids, &words)`, the *same* pure scan over the
same three arguments that `query::ask` then runs again a moment later. As specified, the lever paid
one `exact_seeds` to overlap one `exact_seeds`. The first measurement put the whole thing at
single-digit milliseconds against a 44.9 ms spread between two five-run sets of *identical* code:
not distinguishable from noise (Task 2, §5).

It pays because `query::ask` was then reordered to build the lexical lists **before** it calls the
dense retriever, keeping every list's position in the fusion identical. That is a change to
`src/query.rs`, which the task's own file list did not include, and it is what turns the lever
from zero into 42 ms.

The mechanism is visible in the stage table rather than only in the total: the **`answered` step
falls from 48.8 ms to 0.9 ms** while `model opened` grows by 7.4 ms. The BM25 builds did not get
faster; they moved inside the open's wait. The dense ask is 42.1 ms shorter and the lexical arm,
which builds the same indexes with nothing to overlap them with, is unchanged at ~100 ms — which
is exactly the shape an overlap should have.

The same commit removed a dead `exact_seeds` scan the lexical arm was paying for and could not
use. It measured 0.1 ms on a one-word question and 0.4 ms on a seventeen-word one
(`$S/perf2fix-lexical-paired-raw.txt`, `$S/perf2fix-lexical-multiword-raw.txt`) — the work was
provably dead and is provably gone, and its cost was below what this machine can resolve.

### P3a — `b23806c`, `src/ask.rs`

A pure refactor: the `Cmd::Ask` arm became `ask::Context::open` plus `Context::answer`, so the
graph, the ids, the questions, the vectors, the model and the cross-encoder can be opened once and
answered from many times. It buys nothing on its own and is not supposed to: **−0.2 ms** in the
pinned sitting, −7.3 ms in the check, both inside the spread. It is what makes P3b possible.

Its own evidence is byte-level rather than statistical, because `dump` and `bench` never execute
the `Cmd::Ask` arm: nine ask shapes × two streams, before and after, **18 of 18 identical** —
including the shape that refreshes and prints notices (`$S/perf4fix3-askout.txt`).

### P3b — `83aa8e1` … `4ec3062`, `src/serve.rs`: the resident process

`serve` is `watch`'s poll loop plus a Unix socket at `.repograph/serve.sock`, holding one
`ask::Context`. `ask` uses it without being told to and answers in its own process whenever it
cannot — no socket, a socket nobody listens on, another binary's version, a timeout, `--no-serve`,
`REPOGRAPH_NO_SERVE`. The client never deletes the socket file: a refused connect is also what a
live server with a full listen backlog gives.

## `serve` against in-process

Head binary, on the copy, one sitting, socket and in-process runs interleaved. Wall time by
`time.perf_counter` around the whole process, `ask --stale`, `--no-serve` for the in-process arm;
n = 11 each. The server was started from the same binary and not one request was sent before its
own log showed the bind. Script `$S/perf5-socket.py`, transcript `$S/perf5-socket-table.txt`,
server log `$S/perf5-serve.log`.

| | resident | one process | difference | min–max of the eleven |
| --- | ---: | ---: | ---: | --- |
| fused question, dense | **66.5 ms** | **326.2 ms** | −259.7 ms, 4.9× | socket 65.6–75.3, in-process 321.2–356.9 |
| lexical | **54.0 ms** | **106.2 ms** | −52.2 ms, 2.0× | socket 52.7–59.3, in-process 104.6–112.5 |

All 23 socket runs — the eleven per arm and the first answer — were answered by the resident
process; the harness aborts if one falls back, and every answer was the same size as its
in-process twin (601 B dense, 624 B lexical).

**The first answer costs 259.7 ms**, against 66.5 ms for the ones after it. That difference is the
model opening in the server: `Context::open` reads the graph, the questions and the vectors, but
the model is opened by the first answer that actually needs it, and then kept. A `--no-dense`
server has no such first answer to pay for.

**What is left in a resident answer.** The process floor — starting `repograph` and parsing its
arguments — is 4.8 ms (median of six `repograph --version`). The socket round trip is
sub-millisecond. Everything else is the answer itself.

The spec's targets were dense under 100 ms and lexical under 30 ms after the first question.
**Dense passes; lexical does not** — see below. Task 4 measured the same pair in its own sitting
and reached the same verdict: 71.6 / 385.2 ms dense and 55.4 / 82.2 ms lexical
(`$S/perf4-timings.txt`).

Its in-process figures are not this sitting's, and they do not even differ in one direction: dense
answered more slowly there (385.2 against 326.2 here) and lexical faster (82.2 against 106.2), and
Task 4's own fix round measured that same lexical arm at 124.2 and 125.1 ms across the
`83aa8e1`/`e17eb21` pair — two binaries that do differ, in `serve`, `ask`, `main` and the dense
index, and the difference reaches the lexical answering path, where `Cmd::Ask` now calls
`load_cfg` above the socket attempt rather than below it, but nothing on it that changes what it
costs: the hoist adds one TOML parse on the socket path and moves one on the other, and the
`ask.rs` edits beside it are gated on the dense callback and on a resident refresh. That round
read its own numbers the same way — "The hoist of load_cfg and the stale manifest stamp cost
nothing measurable" (`$S/perf4fix-timings.txt`). Three sittings have now read 82.2, 106.2 and
124.2/125.1 ms for a one-process lexical ask, on a path whose measurable cost **no lever in this
plan changes** — a spread of 43 ms, wider than P2's entire effect on the dense arm. (The one lever
that reached that path at all is P2's removal of a dead `exact_seeds` scan, worth 0.1–0.4 ms,
below what this machine can resolve.) That is this document's opening thesis turning up inside its
own numbers. It is why Task 4's pair is cited for its verdict rather than for its absolute
figures, and why only the interleaved table above is read as a before and after.

### The lexical arm misses the 30 ms target, and the cause is one thing

**54.0 ms through the socket against a 30 ms goal.** Not the socket: the process floor is 4.8 ms
and the round trip is sub-millisecond, so 49 ms of the 54 is spent answering.

Almost all of that 49 ms is the per-question BM25 build inside `query::ask`, which builds the
lexical index and the question index from the graph on every question. The evidence bounds the
build rather than isolating it: the lexical arm's `answered` step in the stage table above is
**53.6 ms**, and that step is the whole of `query::ask` — those builds plus their scoring, the
fusion and the `exact_seeds` scan. Task 3 read the same step at 55–62 ms in its own sitting and
Task 4 cites that figure as the build's cost, but it is the same bound under another name rather
than a second measurement. So read 49 ms as the size of the prize, not as a figure for the builds.
What is not in doubt is which component holds it: the BM25 indexes are the one expensive thing a
resident `Context` does *not* cache. The graph, the questions, the vectors, the model and the
cross-encoder are all held across answers; the indexes are rebuilt for each one.

Caching them is a change to `query::ask`, not to `serve`. It is a lever of its own, it would need
its own pre-registered rule and its own evidence, and this plan's scope ends at P3, so nothing here
implements it. What this document does is record the number to beat: **49 ms**.

> **Answered (2026-09-06, 0.5.0).** That lever was built as L2 of the 0.5.0 gap plan, with the
> pre-registered rule this paragraph asks for. The indexes are now built by the first answer and
> held by `ask::Context`, and a socket lexical answer reads **6.8 ms** against this document's
> 55.0, median of 33 against a base spread of 0.9. Every present-tense sentence below about
> `query::ask` rebuilding per question describes the binary as it stood on this document's date,
> not the released one. See [the 0.5.0 gap results](2026-09-05-0.5.0-gaps-results.md).

## Rule 1 and Rule 2 — what they prove, and what they do not

Both rules were written before any number was measured, and neither was relaxed.

### Rule 1 — a performance change ships only if it changes no answer

Four `dump` files (recorded and developer suites × dense and lexical arms, against the enriched
fixture store) must be byte-identical to the Task 0 baselines, and `bench` must print the same four
lines. Verified after **every** commit that touched shipped code — P1, P2 and its fix round, P3a,
P3b and its three fix rounds — eight comparisons each time, all identical every time. (P1's own fix
round added a test and nothing else, and was not re-measured.)

| dump | `sha1`, unchanged from Task 0 through `4ec3062` |
| --- | --- |
| recorded suite, dense | `7803801b41af9890b5237ff1daa2b15593ef366e` |
| recorded suite, lexical | `e6449c22526b8acce8df825ee2e986c14b4c7d97` |
| developer suite, dense | `7cd5ba127dedc20302ce31d261d51d9724cfa96a` |
| developer suite, lexical | `b7388618d7fd934d3ab9ec1409cc458065bcc17a` |

`$S/perf0-shasums.txt` holds the baselines and `$S/perf4fix3-shasums.txt` the head's; both were
re-read while writing this file and both still hash to those four values. The four `bench` lines
(`$S/perf0-bench-{rec,dev}-{dense,lexical}.txt`) are unchanged too:

```
keyword 40/40  paraphrase 15/30  code 12/12  p90 220 tok  dense=true  enriched=true (1996/1996 nodes)  suite=built-in gated=true
keyword 39/40  paraphrase 14/30  code 12/12  p90 215 tok  dense=false  enriched=true (1996/1996 nodes)  suite=built-in gated=true
long 10/15  cross 12/15  multi 9/12  where 0/9  rule 5/9  p90 240 tok  dense=true  enriched=true (1996/1996 nodes)  suite=dev-cases gated=false
long 11/15  cross 11/15  multi 10/12  where 0/9  rule 5/9  p90 242 tok  dense=false  enriched=true (1996/1996 nodes)  suite=dev-cases gated=false
```

**What Rule 1 does not prove.** `dump` and `bench` build their own dense closure and call
`query::ask` directly — neither of them ever executes the `Cmd::Ask` arm. So the four dumps say
nothing about the code P3a rewrote wholesale and P3b put a client in front of. That is why the
ask-path comparison exists beside it: nine shapes (fused paraphrase, exact id, a seventeen-word
question, no-lexical-match in both arms, `--json`, `--bodies`, `--rerank-local`, and a non-`--stale`
refresh on the copy that prints notices), stdout and stderr compared separately against a binary
built before the change:

```
ask path base b23806c vs head: identical 18 differing 0 (of 18)
```

`$S/perf4fix3-askout.txt`, with nothing listening on either repo and `REPOGRAPH_NO_SERVE` unset, so
it is the real no-server path rather than the switched-off one. The same 18 came back identical in
each of Task 4's three fix rounds.

### Rule 2 — `serve` is byte-identical to in-process

Every one of the 142 bench cases (82 recorded + 60 developer) asked twice in both arms — through
the socket and with `--no-serve` — 284 comparisons, on the copy, never on the fixture:

```
binary /Users/max/Documents/projects/repograph/.worktrees/perf/target/release/repograph
sha1   d85d38a618f3f69446c4815222d01c202b57e4dc
bound  pid 91840: serve: .../bc-perf/.repograph/serve.sock every 30s, batch 1, idle 1800s
identical 284 differing 0
answered by the resident process 284, fell back 0
```

`$S/perf4fix3-rule2.txt`, harness `$S/perf4-rule2.py`, server log `$S/perf4fix3-serve-rule2.log`
(one line, the bind — so no poll moved the store under the comparison).

The second line carries as much weight as the first. A client that fell back would produce
*identical* output while proving nothing, so every socket run's stderr is checked for
`serve: answered by the resident process`. For the same reason the harness starts the server
itself from the binary under test, records that binary's `sha1`, and refuses to send a request
before the bind line appears in the server's own log: a `--stale` question against a quiescent
store is exactly the shape a server from an older build answers identically.

**What Rule 2 does not prove.** Its 284 questions are `--stale` against a quiet tree and produce no
notices at all, so the notice path is covered separately — a non-stale ask over a disturbed copy,
stdout identical and stderr identical **apart from the client's own
`serve: answered by the resident process`**, which only the socket run can carry: both runs print
`refresh: 1 changed, 0 removed` and `refresh: 1 vectors embedded`, and `cmp` on the client's stderr
with that one line removed is identical. Plus two forced failures, a model that cannot open and a
`--rerank` command that does not exist, each producing the same line in-process and through the
socket (`$S/perf4-notices-{inproc,socket}.{out,err}`, `$S/perf4-notice-{dense,rerank}-*`). And it
says nothing about a store that moved under a running server — see the next section.

## What `serve` does not do

Four limits, all measured or read out of the code during Task 4, all in the README.

- **A foreign `enrich` or `embed` is invisible to a running server.** Both of the server's reload
  paths — the background poll and the pre-answer reload on the `--stale` path — are gated on
  `manifest.json`. `enrich` writes `questions.json` (and `vectors.*` when it embeds) and `embed`
  writes `vectors.*`; neither moves the manifest. Run either against a repo with a `serve` on it
  and the resident answers keep the questions and the vectors they started with. Stop the server,
  or ask with `--no-serve`.
- **The configuration is read once, at start-up.** `serve` stamps `repograph.toml` before it opens
  anything and compares it at every poll; when it moves, the server prints one line and exits, so
  the next `ask` answers in its own process under the new file. Up to one poll interval (30 s by
  default) can pass between the edit and the exit.
- **One request is answered at a time.** A second client waits rather than being turned away: its
  connect succeeds into the kernel backlog and it is answered in turn — 2, 8 and 32 simultaneous
  clients were all answered by the resident process, with identical bytes, at roughly one answer
  each of queueing (`$S/perf4-concurrency.txt`). It falls back only if that wait passes the 30 s
  read timeout.
- **Beyond the listen backlog, clients fall back.** 200 simultaneous clients: 135 answered by the
  server, 65 in their own processes, all 200 producing the same 624 bytes, and the server still
  listening afterwards (`$S/perf4fix-burst-post.txt`). `ECONNREFUSED` from a full backlog is
  indistinguishable from a dead socket, which is why the client no longer unlinks anything.

## What was measured and not shipped

Six things were considered and none shipped. The first two, and the stage figure behind the third,
are the design note's measurements rather than this plan's; the note's transcripts survive at the
session scratchpad path it names and are copied to `$S/perf5-spec-*.txt`, with their provenance in
`$S/perf5-spec-transcripts.txt`. Every such figure below was read back out of those files rather
than off the note's tables, and each agreed.

- **A different model file — int8.** The int8 build opens in 166.7–188.0 ms against fp32's
  134.1–137.4 ms at the shipped optimisation level — *slower*, not faster — and it changes the
  vectors: cosine 0.9963–0.9976 against fp32 across four queries. It buys 1.8 ms per query (3.4 ms
  against 5.2). A change that moves every vector would have to clear the three-way rule for a gain
  of under 2 ms per question, so it was not planned. (`$S/perf5-spec-open-variants.txt`, which
  carries the timings and the cosines. The file it names, `model_qint8_avx512_vnni.onnx` at 113 MB,
  comes from the note's own table and not from that transcript.)
- **A different model file — external data.** fp32 with the weights in `model.onnx_data` beside the
  graph opens in 90.0–256.0 ms against 134.1–137.4, identical vectors (cosine 1.0000 ×4). The
  overlap with the shipped variant is the whole of the measurement: the file format is not the
  lever. Reading the file into memory first and committing it (44.3–58.3 + 101.2–110.2 ms) and the
  other optimisation levels (Disable 95.8–204.5, Level3 109.5–114.3) say the same.
  (`$S/perf5-spec-open-variants.txt`.) The note's own summary of that table adds "the session is a
  quarter of the open", and that clause does not travel: the note flags in the same breath that its
  session opened faster here than in the parts run — 135 against 200 ms — and that only the ratios
  were meant to be read. A quarter was in any case the proportion *before* P1; after it, the
  concurrently opened tokenizer and session are essentially the whole of the 220 ms open.
- **Persisting the BM25 indexes.** The three builds live inside the `answered` stage, which the
  design note measured at 50.5 ms dense and 53.1 ms lexical
  (`$S/perf5-spec-stages-{dense,lexical}.txt`) and this sitting measures at 53.6 ms in the lexical
  arm. The design note's non-goal list rejects persisting them, along with two other ideas, on the
  ground that each is "inside the noise or made moot by P3". That is the note's reasoning, and
  **its premise is false**: P3 does not keep the BM25 indexes resident. An `ask::Context` holds the
  graph, the questions, the vectors, the model and the cross-encoder across answers, and
  `query::ask` rebuilds the lexical index and the question index from the graph on every question,
  resident process or not. The conclusion still stands for a fused ask, where P2 hides the build
  inside the model open and the socket answer is 66.5 ms. It does not stand for the lexical arm,
  where that rebuild *is* the missed target — almost all of 49 of its 54 ms, on evidence that
  bounds the build rather than isolating it, as the lexical section above says. Caching the indexes
  is a change to `query::ask` rather than to `serve`: a lever in its own right, needing a
  pre-registered rule and evidence of its own, and outside this plan, whose scope ends at P3. The
  number to beat is 49 ms.
- **`mmap`-ing the vectors.** The note's non-goal list names this beside persisting the BM25
  indexes, and this plan measured neither a variant nor a prototype of it. It did not have to: the
  step `mmap` would attack is `vectors loaded`, which the table above puts at **+15.5 ms** in every
  dense arm (15.3–15.7 across the four binaries, 14.9–15.2 in the check sitting) — under 3 % of the
  base ask and about 5 % of the head's. A resident `Context` reads the vectors once and holds them,
  so a socket answer pays none of it at all. A lever whose ceiling is 15 ms in a fresh process and
  nothing in a resident one does not earn a three-way rule.
- **`enrich` wall time.** Not planned: `enrich` is already parallel over batches behind
  `--parallel`. This is the design note's prose, with no transcript behind it here or in its own
  scratchpad.
- **`embed` throughput.** ~103 s once per corpus for the small model's 33,525 rows, from the
  README's Embeddings section — a one-off cost per store, not a per-question one. Also the note's
  prose rather than a transcript.

For the record on the levers that did land: before P1, `/usr/bin/time` on the fixture read 0.72,
0.75 and 0.77 s for a fused ask and 0.08 s twice for a lexical one
(`$S/perf5-spec-ask-wall.txt`), and the stage table of the day read 582.5 ms dense / 75.5 ms
lexical (`$S/perf5-spec-stages-dense.txt`, `-lexical.txt`). Embedding the query itself costs
4.3–5.0 ms (`$S/perf5-spec-open-runs.txt`), which is most of the `query embedded and searched`
step's 12.5 ms.

Those two readings are also the plainest statement of why the table above had to be taken in one
sitting. The pre-plan `ask` has now been measured three times on this fixture, on code whose ask
path never changed — `336066e` touches only `dump` and `bench` — and it read **582.5 ms** for the
design note, **609.5 ms** for Task 0's medians of five (`$S/perf0-medians.txt`) and **537.6 ms**
here. Seventy-two milliseconds of that range is the machine. P2's whole effect is 42.

## Provenance

| what | file under `$S` |
| --- | --- |
| the four binaries, their commits and shas | `perf5-binaries.txt`, `perf5-build.sh`, `perf5-build.log` |
| the interleaved sitting: script, raw runs, table | `perf5-stages.py`, `perf5-stages-raw.txt`, `perf5-stages-table.txt` |
| the check sitting | `perf5-stages-raw-2.txt`, `perf5-stages-table-2.txt` |
| the archive-built head against the committed binary | `perf5-headcheck.sh`, `perf5-headcheck.txt` |
| socket against in-process, and the server's log | `perf5-socket.py`, `perf5-socket-table.txt`, `perf5-serve.log` |
| Rule 1: baselines and head, dumps and bench lines | `perf0-shasums.txt`, `perf4fix3-shasums.txt`, `perf0-bench-*.txt` |
| Rule 2: 284 comparisons | `perf4fix3-rule2.txt`, `perf4-rule2.py`, `perf4fix3-serve-rule2.log` |
| the ask path with no server | `perf4fix3-askout.txt` |
| Task 4's own socket pair, both of its rounds | `perf4-timings.txt`, `perf4fix-timings.txt` |
| notices across the socket | `perf4-notices-*`, `perf4-notice-*` |
| concurrency and the 200-client burst | `perf4-concurrency.txt`, `perf4fix-burst-post.txt` |
| the design note's own transcripts, recovered | `perf5-spec-*.txt`, `perf5-spec-transcripts.txt` |
| the suite at `4ec3062` | `perf5-testcount.txt` — 426 passed, 0 failed, 2 ignored |

[`runbook.md`](runbook.md) has the recipe for taking these timings again, and the three traps this
run learned the hard way.
