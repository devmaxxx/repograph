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

**Reading.**

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

**Reading.**

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

**Reading.**

## 10 · What did not close
