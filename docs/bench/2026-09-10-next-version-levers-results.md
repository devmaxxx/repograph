# Next-version levers — what the branch measured

Every gate in this document was written and committed before the run it judges (Task 0 of
[the plan](../superpowers/plans/2026-09-10-next-version-levers.md)); readings are pasted under
their gate and no clause is edited after a number is read. A clause a reading fails is recorded
as failed. `main` is `d688e56`; the branch binary in each section is named by its commit.

Machine: Mac15,7, 12 cores, 36 GB, macOS 26.6.2. Directories: the pinned fixture `~/bench/beauty-crm-502e8a6d`
(locked, read-only — the four arms of every baseline read are taken there, and nothing is ever
written there) and the one writable worktree `~/bench/beauty-crm-test` (locked — every writer runs
there, and every reader row is read there straight after `bench/probe/reset.sh` has copied the
fixture's store in). What each arm wrote, copied out before the next arm ran:
`~/bench/levers-2026-09-10/log/<task>/<arm>/`. Transcripts: `~/bench/levers-2026-09-10/log/`.

## 1 · G23 — the reader bars, read through their own control

**Gate.** `bench/probe/quiet.sh` passes (idle ≥ 85%, 1-minute load < 3.0, AC power, no `cargo`,
`rustc`, `node` or another `repograph` running); `bench/probe/readers.sh` at n = 5 per row, twice
through the same binary (`repograph-main`), in `~/bench/beauty-crm-test` straight after one
`bench/probe/reset.sh`, every row `--stale`, with `shasum` over the store's files identical before
the first suite and after the second (nothing wrote under them); for every row
|median wall A − median wall B| ≤ 10% of A and |median max RSS A − B| ≤ 5% of A. A row outside
at n = 5 is re-run once at n = 9; a row still outside is recorded as *not judgeable at this shape
on this machine* and stays in the table with that word — it is not widened and it is not dropped.
The passing rows' medians become the reference every later reader reading on this branch is
judged against, under the same 10% / 5%.

**Gate, the missed-floor case.** A `bench` row whose run missed a floor is read as a timing like
every other row: the run answered every case, and its median wall and max RSS are judged under the
same 10% / 5%. The floor verdict is `bench`'s own, is recorded beside the reading as
`floors_missed`, and is not what this clause judges. A row that exited non-zero for any other
reason voids the suite, which is re-run.

Added on 2026-09-10, before §1 held any reading: it says how a case the committed clauses did not
name is treated, and changes nothing about what they judge.

**The index under the rows.** G23's lever asked for a pinned index — the reader rows taken against
a copy no writer in the session touches. On this branch that index is the locked fixture's store,
byte for byte: `reset.sh` copies it from `~/bench/beauty-crm-502e8a6d` into the one writable
worktree moments before the suite, every row reads the store as it stands (`--stale` on every
command that could walk; `bench` and `dump` never walk), and the checksums say nothing moved. The
cause this replaces is the fixture's own index growing 33,526 → 33,554 rows mid-suite on
2026-09-07 because `watch` and `update` ran against it. The fixture is never written now — it is
locked, and the rule allows it readers only — and no row of the suite can write anywhere.

**Correction, 2026-09-10 — the script, before any number.** The Gate above states the precondition
as no `cargo`, `rustc`, `node` or another `repograph` running. `bench/probe/quiet.sh` as first
committed (`a53e50c`) greps `cargo|rustc|repograph` and leaves `node` out deliberately, its comment
reasoning that the agent harness which launched it is itself a `node` process. The gate is right
and the script was wrong: a `node` burning a core is exactly the pollution this row excludes, and a
harness that exempts its own noise measures its own convenience. So the script was fixed and the
clause was left exactly as committed — `quiet.sh` now counts `node` like the others and exempts
only the PIDs of its own ancestor chain (`$$` upwards through `PPID`), which the `load1` clause
already refuses on its own when that harness is busy; a sibling `node`, another session's harness
or a dev server all still refuse. `bench/probe/test_quiet.py` holds the two cases that matter — a
`node` in the ancestor chain does not refuse, one outside it does — and `bench/probe/README.md`,
which had documented the exemption as intended behaviour, is corrected. This correction was taken
before the reading it governs: §1's **Reading.** is still empty and this row has never been
measured. That is the honest half of the distinction this branch keeps — a clause or a harness
corrected *after* its number is read is the thing the branch exists to prevent, and neither this
section nor §9 has one of those.

**Correction, 2026-09-10 — the third revision of the busy clause, and the last.** Recorded in full
because three revisions of one precondition is the shape of a moving gate, even where no number has
moved behind it. Stage one (`a53e50c`): `pgrep -l 'cargo|rustc|repograph'`, `node` deliberately out
of the pattern because the harness that launched the script is itself a `node` process. Stage two
(`8b642d8`): `node` counted like the other three, on presence, with only this script's own ancestor
chain exempt. Stage three (this commit): `cargo`, `rustc` and any other `repograph` still refuse on
presence at any CPU — they are this project's own work and they are bursty, and one at 0% now is
compiling or embedding a second later — while any other `node` refuses only at or above 5.0% CPU,
and both the printed line and the refusal rows now name which rule counted what. No reading has
ever been taken under any of the three: §1's **Reading.** was empty at stage one, empty at stage
two, and is empty now, and no number anywhere in this document was let through by any version of
this clause. Presence-counting was not merely inconvenient, it was the wrong measurement: the count
was named `busy_processes` and it counted processes, reading 66 on this machine while
`ps -Ao pid,pcpu,comm | awk '$3 ~ /node/ && $2+0 >= 5.0'` returned zero rows — all 66 were other
sessions and their MCP servers, asleep. A process consuming no CPU does not move a wall clock, so
refusing on it excludes nothing from the reading and refuses every working machine; that is a
process census standing where a quiet check should be, unsatisfiable for a reason that has nothing
to do with the row. The Gate above is left exactly as committed, so its letter — no `node` running
— now reads stricter than the script enforces; the divergence is recorded here rather than repaired
by editing a committed clause. The idle, `load1` and AC clauses are untouched and remain the
load-bearing three: ≥ 85% idle, 1-minute load < 3.0, AC power, the numbers as first written. This
clause is now frozen. A further change to it once §1's **Reading.** holds a number would be exactly
the failure this branch was built to prevent, and would have to be recorded as that and not as a
fourth correction.

**Correction 4, 2026-09-11 — the suite itself, and again before any reading.** Two changes to what
§1 measures, both forced by the first two attempts at this control, and both recorded here because
§1's **Reading.** was empty when they were made and is empty as this paragraph is written.

*The precondition waits out its own setup.* Attempt one refused: `quiet.sh` passed at 85.90% idle,
`reset.sh` then copied 78 MB of store and walked the tree, and `readers.sh` re-read idle at 83.76%
one second later and refused. Every arm on this branch is reset-then-measure, so the kit would have
refused every one of them for noise it made itself. `quiet.sh --wait <seconds>` now re-reads until
the machine is quiet and reports how long it settled for; `readers.sh` and `embed.sh` take a 600 s
budget. A machine that is genuinely busy still refuses when the budget runs out. No clause of the
Gate moved: the numbers a row is judged quiet against are the same three, read at a moment the
kit's own rsync no longer spoils.

*The `trace` row was timing an error.* It asked for a call path from `main` to `cn`. There is none
within six hops, and `trace` exits **1** when it determines that — one of its two documented
answers — so the row measured a **0.02 s early exit**, and `judge.py` refused every suite it was in
for having measured a failure. The 2026-09-07 kit published that 0.02 s as the `trace` reader bar,
which is to say the bar this branch was sent to check was itself measuring an error path. The row
now traces a five-hop chain that finds something: `applyAppointmentCreate` → `evaluateAvailability`
→ `loadAvailabilitySnapshot` → `toWallClock` → `faceToWallClock` → `pad`. **The `trace` row's
reference median is therefore not comparable to 2026-09-07's**, and §1's reference table says so
where it prints that row; every other row is unchanged. The general defect is recorded as G41, whose
audit says only `bench` and `trace` do this — `impact`, `changes` and `ask` all exit 0 on a
legitimate nothing.

*And a clause the Gate never had.* Attempt two was killed at minute eight, mid-suite, because the
machine ran out of memory: 12 GB held by one container helper, 5 GB by an editor, ~8 GB across other
sessions' `node` processes, against `ask --rerank-local`'s measured 3.03 GB. `quiet.sh` now reads
reclaimable memory (free + inactive + speculative) against a 6 GB bar — twice the heaviest row — and
names the four processes holding the most when it refuses. This is an **addition**, not a change: no
committed clause said anything about memory, a suite that dies mid-read is not a reading, and the
kit should refuse before spending eight minutes rather than after.

**Reading.**

## 2 · G15 — the anchor line in the history

**Rule.** Every before/after on this branch carries the `anchors` line and is recorded through
`track.py`, which reads it. The four arms on the pinned fixture `~/bench/beauty-crm-502e8a6d`
reproduce the 2026-09-09 baseline exactly — counts, p90, anchors — after every task that touches a
read path.

**Reading.**

## 3 · G35 — what `derive` costs on an update that changes a file

**Gate.** Median of five `families derived` steps under `REPOGRAPH_TIMING=1` on a one-file
`--no-dense update` in `~/bench/beauty-crm-test` after `reset.sh`, recorded beside `tree walked`
from the same runs. The ledger expects it under the walk's own number; it is recorded whichever
side it lands.

**Reading.**

## 4 · G39 — extraction by one grammar, the family set a view

**Gate.** In `~/bench/beauty-crm-test`, each build from an empty store after `reset.sh`, each
build's `graph.json` copied to `log/g39/<main|branch>/` before the next build runs: (a) `graph.json`
written by `repograph-main build` and by the branch's `build` are the same visible graph —
`bench/probe/graphdiff.py` over the two copies reports `nodes: same` and `edges: same`; (b) the
branch's `graph.json` is at most 110% of `main`'s in bytes, and the number of edges held aside
(`pending`) is recorded; (c) an `update` after adding `docs/oq.md` defining `OQ-1` prints
`changed 1 removed 0`, prints `families: +OQ`, re-reads no other file, and its `--no-dense` wall is
under 0.5 s as the median of five; (d) `--no-dense build` wall on the branch is within 10% of
`main`'s, medians of three; (e) every reader row of §1 through the branch binary — read in the same
directory as §1's reference after a reset and one warm `--no-dense ask` that writes the branch's own
mirror (the fixture's is `RGM1`, the branch's `RGM2`, and a mirror can only be written where writing
is allowed) — is within §1's bars of the reference. Read path: the four arms on the pinned fixture
read the baseline table of §2 exactly. Failing (a), (b) or (e) stops the branch at this task.

**Reading.**

## 5 · G40 — a corpus that defines no ids

**Gate.** A build over documents with no definition line yields file nodes only, an empty family
line, its `ISO-8601` mention held aside and counted by `verify`, and `ask ISO-8601` answering from
retrieval rather than an exact seed. The never-matching alternation is gone from `src/ids.rs`.

**Reading (2026-09-11).** A tests-only gate, so this is a local run of the binary this branch
builds over one document — `docs/notes.md`, holding `даты по ISO-8601, см. RFC-7231 и -M01` and no
definition line — and the same four assertions ride in
`tests/families.rs::a_corpus_that_defines_no_ids_reads_as_one`.

```
$ repograph --no-dense build
families: (none) · milestones: (none)          # stderr
changed 1 removed 0 nodes 1 edges 0

$ repograph --no-dense verify
nodes: 1  {"File": 1}
edges: 0  {}
dangling edges: 0
held aside: 2 edges to ids in 2 prefixes no line defines  ISO RFC

$ repograph --no-dense families
families                              nodes  defs  defined
  (none)
milestones                            nodes  defs  defined
  (none)
mention-only prefixes               written   ids  files  e.g.
  ISO                                     1     1      1  docs/notes.md:3  даты по ISO-8601, см. RFC-7231 и -M01
  RFC                                     1     1      1  docs/notes.md:3  даты по ISO-8601, см. RFC-7231 и -M01

$ repograph --no-dense ask ISO-8601
                                               # nothing: no node, so no exact seat and no passage
```

File nodes only, no edge a reader follows, both id-shaped mentions held aside and counted, and the
bare `-M01` yielding no third prefix. The last clause of the gate — the never-matching alternation
gone from `src/ids.rs` — was already true when this branch opened: #27 removed the constructor with
it, so the `Option<IdMatcher>` the ledger asked for has nothing to wrap and the state is the
graph's. **Clause by clause: all four green, the fifth green before the branch.**

## 6 · G34 — one owner, the property test, the corpus

**Gate.** The property test is green on macOS, Linux and Windows CI; a build on the branch after
this task, in `~/bench/beauty-crm-test` from an empty store, has the same node and edge counts as
the build Task 6 copied to `log/g39/branch/graph.json`, and every node label that differs is a
heading that lost a CommonMark closing `#` sequence, each listed. `families=` is
gone from the bench summary line and `track.py` still reads every transcript in
`bench/history/runs.jsonl`'s history.

**Reading.**

## 7 · G38 — the report's own edge

**Gate.** `repograph families` in `~/bench/beauty-crm-test` as Task 8's build left it puts `OQ`
first in the mention-only section and every family with exactly one definition carries
`defined once`; `bench/compare` still reads the JSON.

**Reading (2026-09-11), on the second and third clauses only.** No fixture bench was run on this
branch, so `~/bench/beauty-crm-test` was never built and the corpus clause — `OQ` first in the
mention-only section — **is unread**. What is read is the sort that would put it there, on a
two-document repository built by this branch's binary:

```
families                              nodes  defs  defined
  OD                                      1     1  docs/b.md:1  defined once
  REQ                                     2     2  docs/a.md:1
milestones                            nodes  defs  defined
  (none)
mention-only prefixes               written   ids  files  e.g.
  OQ                                      3     3      1  docs/a.md:3  тело, см. OQ-1, OQ-2, OQ-3
  ISO                                     2     1      1  docs/a.md:7  тело, даты по ISO-8601 и ещё раз ISO-8601
```

`OD`, defined by one line, sorts above `REQ` and carries the mark; `OQ`, written under three ids,
sorts above `ISO`, written twice under one — which is the corpus clause's shape at the scale a test
can hold. `src/families.rs`'s two inline cases pin both sorts (`a_family_defined_once_sorts_first_
and_is_marked`, `mention_only_prefixes_sort_by_distinct_ids_then_by_mentions`), and
`python3 -m unittest discover -s bench/compare` is OK — `bench/compare/run.py:54` reads `family` and
nothing else, so `definitions` and `ids` are additive. **Clause by clause: the corpus clause unread,
the `defined once` clause green, the JSON clause green.**

## 8 · G32 — the fixture copy, rebuilt and enriched

**Gate.** *Before* — the four arms on the pinned fixture `~/bench/beauty-crm-502e8a6d` read the
baseline table of §2 exactly; *rebuilt* — in `~/bench/beauty-crm-test` after `reset.sh`, `build`
prints the family line and ``N requirement-like nodes have no questions — run `repograph enrich` ``
with N = eligible − covered from the bench line, and both recorded arms print `enriched=false`;
*after* — one `enrich` (≈ $0.20 of haiku) brings coverage to ≥ 99%, both recorded arms print
`enriched=true gated=true` and exit 0, the developer arms are recorded, and the enriched store is
copied to `log/g32/after/store/` before anything else runs in the directory. Coverage still under
99% after a second `enrich` is a failed gate, recorded with the count the model declined.

**Reading.**

## 9 · G19 — a chunk budgeted in tokens

**Gate.** Control: `repograph-main embed` in `~/bench/beauty-crm-test` after `reset.sh`, the
vectors removed before each run by `bench/probe/embed.sh`, three runs under it and `caffeinate -di`,
wall / user / max RSS / peak CPU and the stamped cadence. Candidate: the branch binary, three runs
in the same directory after a second reset and one warm `--no-dense ask` for the branch's own
mirror, vectors removed before each.
Pass when, on the candidate's median run, every interval is under 60 s including the first (from
process start to the first `dense: N/M rows` line), the longest interval is at most 1.3 × the
median interval, the median wall lies inside the control's [min, max] or within 10% of the
control's median (whichever is wider), median max RSS within 5% and median peak CPU within 10% of
the control's. Small model only; the large-model cadence is the 2.1 GB download this branch does
not take. A candidate that misses a clause is recorded, and the bar is not moved.

**Correction, 2026-09-10 — the code, before any number.** The Gate above judges the candidate's
median peak CPU within 10% of the control's. `bench/probe/judge.py` as first committed (`a53e50c`)
defines `CPU_BAR = 0.10` and never reads it: `compare` judged wall and max RSS only, so this
gate's peak-CPU clause was green by construction whatever an embed did — and the recorded triple
for this row is 1,930 s wall, 293% peak CPU, 2.15 GB, a number a bar that is never read cannot
protect. `compare` now judges peak CPU as a third column, printed and verdicted like the other
two, against the `CPU_BAR` that was already committed. No clause moved and no other verdict
changed: `judge.py control` still judges the two columns §1's Gate names, and a reader row read
under §1's bars is judged on its wall and RSS columns with its peak CPU printed beside them.
This correction, like §1's, was taken before the reading it governs — no embed has been run on
this branch and this section's **Reading.** is empty. A clause corrected after its number is read
is the other half of that distinction and is what this branch exists to prevent; neither section
has one.

**Note beside the gate — what the CPU column can and cannot say.** `bench/probe/measure.sh` samples
with `top -l 2 -s 1`, so any command that finishes in under about two seconds is never sampled and
reports `peak_cpu = 0`. That makes the column meaningless for the reader rows of §1 — every one of
them is under a second — and meaningful for the whole-store embed this gate judges, which runs
about 1,930 s and is sampled throughout. A row whose *reference* read 0 prints `n/a` and stays
unjudged on that column rather than passing on a zero that means "never sampled". This is a note on
what the reading can carry, not a change to the gate: the numbers above are as committed.

**Correction, 2026-09-11 — the CPU column, before any number.** The note above says the peak-CPU
column is meaningless for every reader row and meaningful for this row alone. That is still true of
the *sampled* column, and it is why `judge.py compare` now prints a fourth: `avg_cpu`,
`(user + sys) / wall`, derived per run from the `time` report `measure.sh` has always written and
medianed like the rest. It exists for a row that lasted 40 ms, so the reader rows of §1 have a CPU
number a bar could be set on — printed, and judged by nobody until one is. `/usr/bin/time` reports
to 10 ms, so a 0.04 s row moves a fifth to a third on one tick, and the spread a bar would have to
come from is what `judge.py control` now prints beside the two spreads it judges. The sampled peak
is unchanged and is what this section's clause reads, against the `CPU_BAR` it has always named. No
committed clause is edited: §9's peak-CPU clause is the same sentence judged on the same column, and
§1's Gate names wall and max RSS only, which `judge.py control` still judges alone. A summary too
old to carry `user` and `sys` is read without the column rather than refused — the pair is what the
average is derived from and nothing else reads it — and `bench/probe/embed.sh` now writes `sys=`
beside its `user=` so the one row §9 judges carries the column too. Like §1's corrections and §9's first, this was taken before the reading it governs: no
embed has been run on this branch, §1's and §9's **Reading.** are both empty as this paragraph is
written, and no bar has been set on the new column — that is G23's reading, and G23 is open.

**Reading.**

## 10 · What did not close

Written 2026-09-11, at the end of the branch that closed G38, G40, G41 and G42. Everything below is
a gate this document names and no run answered, or a ledger row this branch never opened. None of
it is recorded as a refusal: an unread gate is unread.

- **§1, G23 — the control has never run.** Four corrections to the precondition and the suite, all
  before any reading, and still no number: this machine has refused `quiet.sh` every time it was
  asked. Everything downstream of §1 inherits that — the reference medians every later reader
  reading would be judged against do not exist, so §4(e), §9's whole comparison and any bar on the
  new `avg_cpu` column have nothing to be judged against.
- **§2, G15 — the baseline was not re-read.** No task on this branch touched a read path, so the
  four arms on the pinned fixture were not run and the anchor line was not recorded through
  `track.py`. Nothing to compare, and nothing claimed.
- **§3, G35 — moot, not measured.** No writer derives a family set since #27, so there is no
  `derive` stage to put on the `REPOGRAPH_TIMING` line and the gate's median of five has no subject.
  Recorded in the ledger as moot.
- **§6 / §4, G39 — Task 6 was not run.** The design landed in #27; the price the gap calls the whole
  question — the store's growth under a generic grammar, the one-file `update`'s wall, the
  `graphdiff` against `main`'s build — needs the corpus copy and a build, and neither happened here.
- **§6, G34 — the corpus half.** One grammar with one owner is in code and the property test is in
  the suite; what is unread is the corpus build behind the `families=` clause and the CI run on
  three platforms that the clause names.
- **§8, G32 — the rebuild and its ≈ $0.20.** The fixture copy was never rebuilt, never enriched,
  and the before/after arms were never read. It stays the first thing anyone hits after a rebuild.
- **§9, G19 — the token budget and the large-model cadence.** No embed was run on this branch, so
  the cadence clauses are unread; the large model's cadence is the 2.1 GB download the branch
  already said it would not take.
- **Rows this branch never opened:** G13 (the code seat, priced and unwritten), G14 (the register
  prompt, retired as framed), G17 (the constant's derivation set), G22 (mapped weights under
  pressure), G25 (the fp32 floor), G28 (the reranked p90 on the ceiling), G37 (no non-Claude
  number, and the stacked levers). Each is left exactly as the ledger has it.
