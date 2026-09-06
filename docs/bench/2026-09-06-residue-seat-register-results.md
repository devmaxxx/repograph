# The residue, the seat and the register — three rules, three verdicts

**Branch** `feat/residue-seat-register`, off `origin/main` at `32577e2`.
**Rules committed** at `4977f40`, before any number below existed —
[the design note](../superpowers/specs/2026-09-06-residue-seat-register-design.md), with
[the plan](../superpowers/plans/2026-09-06-residue-seat-register.md) that executed it.
**Fixture** `/Users/max/bench/beauty-crm-502e8a6d`, read-only throughout (`bench`, `dump`,
`ask --stale`; `git status --porcelain` empty before and after).
**Copies** `bc-a1` and `bc-d1` under `/Users/max/bench/gaps-2026-09-05/`, read-only; `bc-raw` and
`bc-r1` under `/Users/max/bench/residue-seat-register-2026-09-06/`, the only stores a writer
touched. **Scratch** `$M = /Users/max/bench/residue-seat-register-2026-09-06`; every number below
names the file it came from.

Three changes were proposed, each with a rule it had to clear. One shipped.

| rule | verdict | the number that decided | file |
|---|---|---|---|
| **R** the coverage denominator charges every query term | **ships** | all seven clauses hold; four recorded arms identical, held-out p = 1.0000 in both arms | `$M/t3-verdict.txt` |
| **S** one seat for the code list on the plain path | **measured, rejected** | clause S2 — recorded lexical paraphrase 15/30 → 14/30 | `$M/t4-verdict.txt` |
| **G** the documents' prompt asks for both registers | **measured, rejected** | clauses G1, G2, G3, G6 — `rule` 5/9 → 2/9 in both arms, paraphrase 15/30 → 13/30, a broken dense floor | `$M/t6-verdict.txt` |

The binary the branch leaves is `768f7b9ba6627734b5843bf518b6bef7debf317b`: rule R's denominator
and nothing else. `src/query.rs`'s code seat was never written; `src/enrich.rs` was written,
measured and reverted.

## The baselines every "before" was read against

Re-taken on the branch point with the shipped binary and required to agree with
[the coverage admission results](2026-09-06-coverage-admission-results.md) before anything else ran
(`$M/task0.txt`). All six dumps `cmp` clean against that campaign's; the eight lines agree.

| arm | recorded (82) | developer (60) | held-out (400, seed 20260905) |
|---|---|---|---|
| dense, enriched | keyword 40/40, paraphrase 15/30, code 12/12, p90 221 | long 10/15, cross 13/15, multi 10/12, where 0/9, rule 5/9 — **38** | 104 |
| lexical, enriched | keyword 39/40, paraphrase 15/30, code 12/12, p90 215 | long 11/15, cross 13/15, multi 9/12, where 0/9, rule 5/9 — **38** | 112 |
| dense, raw | keyword 40/40, paraphrase 9/30, code 12/12, p90 221 | — | — |
| lexical, raw | keyword 39/40, paraphrase 7/30, code 12/12, p90 226 | — | — |

The two raw lines were read on `$M/bc-raw`, a copy of the fixture's store with no `enrich` paid
for, rather than assumed from ADR-001 Amendment 7; they reproduce it exactly.

---

## R — the residue of G8: what `attainable` charges

### The rule, as pre-registered

> The fixed denominator ships if, and only if, **all seven clauses hold on the binary that carries
> it**, under the constant derived above. Any one failing ends the design; the fallback is tried
> once under the same clauses; a second failure leaves the residue as shipped.
>
> 0. **The constant is derived before either suite is opened**, on the new binary's held-out dumps
>    in the `--no-dense` arm, and written to a file before `score` runs.
> 1. **No floor breaks.** Every floor `bench` grades holds in all four fixture arms — the two
>    enriched arms on the fixture, the two raw arms on a raw copy of it — with p90 at or under 230.
> 2. **No arm loses ground on the recorded suite.** Subsumed by clause 6.
> 3. **No arm loses ground on the developer suite.** Dense total at least 38, lexical at least 38,
>    `where` no lower than 0/9 in either arm.
> 4. **No significant held-out loss.** Paired exact McNemar against the shipped binary's dumps,
>    both arms, the same 400 questions; a loss at p < 0.05 is disqualifying, a wash passes.
> 5. **The replay still agrees with the binary.** `bench/admission.py check --form coverage` at the
>    derived constant reproduces the binary on all three case sets in both arms — 82, 60, 400.
> 6. **The four recorded arms do not move.** The three counts of each recorded arm are identical to
>    the baseline table above — `40/40 15/30 12/12`, `39/40 15/30 12/12`, raw `40/40 9/30 12/12`
>    and `39/40 7/30 12/12` — with p90 at or under 230. Per-case movement inside an unchanged count
>    is recorded, not judged.

### What changed

`LexicalIndex::attainable` summed idf over the unique query terms **the index holds**. A term the
index never saw left the denominator instead of lowering the score, so a list's coverage rose with
every query term its vocabulary lacked, and the form rewarded an index for a narrow vocabulary as
much as for a good match. Design A charges every unique query term, one the index never saw at the
idf BM25 gives df = 0, `ln(2n + 2)`. An index over no documents attains `0.0`; the `-0.0` a dump
writes stays the sign of an index that was never built. Design B, the fallback, was never needed.

### The constant

`$M/t2-crossover.txt`, taken on the new binary's held-out dump in the `--no-dense` arm at 15:15:10,
ten seconds before `score` first opened a suite (15:15:20):

```
form=coverage c=0.761  (400 questions with a questions list, 400 distinct finite values)
  above c: 374 questions — questions list holds the answer in top five 23%, passage list 15%
  below c: 26 questions — questions list 23%, passage list 58%
  questions on the side of the list that holds their answer: 69
```

It returned **0.761** — the value the dropping denominator's crossover returned. The literal does
not move, but it is now a constant of the form that ships, derived under it rather than inherited.
It was not compared with the old value and chosen; `$M/t2-constant.txt` was written from the
crossover's own output before `score` ran.

### `search` did not move by a bit

Required before any admission number was read. `$M/lists.py` over the six baseline dumps against
the six new ones: every `exact`, `bm25_passages`, `bm25_questions`, `bm25_code`, `dense_passages`
and `dense_questions` list byte for byte identical, no `attainable_*` field falling
(`$M/t2-lists.txt`):

| suite | passages unchanged | questions unchanged | new/old median | max |
|---|---|---|---|---|
| recorded (82) | 74 | 72 | 1.000 | 3.682 |
| developer (60) | 24 | 22 | 1.085 / 1.094 | 1.457 / 1.475 |
| held-out (400) | 308 | 366 | 1.000 | 2.485 / 2.018 |

The developer suite is where the residue lived: two thirds of its queries carry a term one index
lacks, against a tenth of the recorded suite's.

### The clauses

| clause | verdict | reading | file |
|---|---|---|---|
| R0 | pass | crossover 15:15:10 < score 15:15:20; `C1 = 0.761` written first | `$M/t2-crossover.txt`, `$M/t2-constant.txt` |
| R1 | pass | four recorded arms `gated=true`, exit 0, p90 221 / 215 / 221 / 226 | `$M/t3-bench.txt` |
| R2 | pass | subsumed by R6 | — |
| R3 | pass | dense 38, lexical 38, both at the base's 38; `where` 0/9 in both | `$M/t3-bench-dev-*.txt` |
| R4 | pass | dense 104 → 105, lexical 112 → 113, 2 gained 1 lost each, p = 1.0000 | `$M/t3-heldout.txt` |
| R5 | pass | 82/82, 60/60, 400/400 replicated in both arms, exit 0 | `$M/t3-check.txt` |
| R6 | pass | all four recorded arms' counts identical to the baseline table | `$M/t3-bench.txt` |

The held-out compare was read before any bench line, and it reproduced the offline replay's
prediction question for question (`$M/t2-score.txt` against `$M/t3-heldout.txt`).

### What moved without moving a count

Not one case in either suite in either arm changed its HIT/miss verdict — all four
`$M/t3-case-moves-*.txt` are empty. Seventeen cases changed the **answer they return**
(`$M/t3-case-token-moves-all.txt`), and the only summary figure that moved at all is the developer
lexical p90, 237 → 239 tok:

- recorded, both arms: `FR-PAY-110`, `FR-PAY-28`, `FR-SVC-50`, `FR-PH-43`
- developer: `FR-DM-30+…segments.ts`, `FR-TOOL-35`, `FR-APP-40+FR-APP-47+FR-AI-09`,
  `…tenant-context.interceptor.ts+…database.service.ts`, and in the dense arm also
  `FR-SEC-20+packages/db/src/auditChain.ts`

A corrected denominator that changes which documents are seated without changing whether the
answer is found is exactly the shape clause 6 was written to allow. Nothing here is judged.

### G13 under the new denominator

Regenerated on `bc-a1` and pasted whole in `$M/t3-g13.md` — 27 rows. The code list still ranks ten
of the thirteen `where` anchors in its own top ten, four at rank 1; the `coverage` column is now
the charged one, and the code index's coverages fall against the shipped denominator's table,
because the code index lacks most of a Russian prose query's terms and is now charged for them.
This is evidence for rule S and is scored against nothing.

**R ships.** `attainable` charges every query term; `QUESTIONS_GATE` keeps the literal 0.761 with a
comment that says which derivation is which.

---

## S — one seat for the code list (G13)

### The rule, as pre-registered

> The seat ships if, and only if, **all six clauses hold on the binary that carries it**, on
> `bc-a1` with the seat on against the same binary with the seat off. Any one failing leaves
> `CODE_SEAT` false, the path in the code for the next measurement, and `where` at 0/9 with the
> price recorded.
>
> 0. **The constant is derived before either suite is opened**, from the seat-off mixed held-out
>    dump in the `--no-dense` arm, and written to a file before `score` runs.
> 1. **No floor breaks.** … 2. **No arm loses ground on the recorded suite** on `bc-a1`: keyword,
>    paraphrase and code each at least the seat-off count, both arms. 3. **The developer suite is
>    not down and `where` moves.** … 4. **No significant held-out loss on the document questions.**
>    … This is the clause A4 failed, and the one the seat has to clear. 5. **The replay still
>    agrees with the binary.**

### The held-out sets

`heldout.py build --only` draws by entry kind. On `bc-a1` the documents' set comes back **byte for
byte the fixture's 400 questions** (`cmp` silent) — that store's document questions are the
fixture's, so the same seed draws the same sample — and the code set is 400 of its 3,463 code
entries: 75 file anchors, 325 symbol anchors, 400 distinct nodes. The mixed set the constant is
derived on is the two concatenated, 800 lines, fixed at equal halves in the design note before any
dump was taken.

### The constant

`$M/t4-crossover-code.txt`, on the seat-off mixed dump in the `--no-dense` arm at 15:23:29, ten
seconds before `score` opened a suite:

```
form=coverage c=0.902  (800 questions with a code list, 786 distinct finite values)
  above c: 538 questions — code list holds the answer in top five 40%, passage list 10%
  below c: 262 questions — code list 2%, passage list 25%
  questions on the side of the list that holds their answer: 247
```

The split separates: above the cut the code list holds the answer four times as often as the
passage list, below it the passage list holds it twelve times as often. `c_code = 0.902` is not
`QUESTIONS_GATE`'s 0.761 and was not compared with it.

### The seat-off baseline, and the proof it is the binary

Every one of the nine seat-off dumps replicated — recorded 82, developer 60, document held-out 400,
code held-out 400 per arm and 800 mixed (`$M/t4-off-check.txt`, exit 0). The replay's seat-off
score line reads `keyword 39/40 paraphrase 15/30 code 12/12` in the lexical arm, identical to the
binary's own `bench` line (`$M/t4-score-seatoff.txt` against `$M/t4-off-bench-rec-lexical.txt`), so
the comparison below is like for like and the loss is the seat's, not a replay defect.

### The clauses

| clause | verdict | reading | file |
|---|---|---|---|
| S0 | pass | crossover 15:23:29 < score 15:23:39; `c_code = 0.902` written first | `$M/t4-crossover-code.txt` |
| **S2** | **FAIL** | recorded lexical **paraphrase 15/30 → 14/30**; keyword 39/40 and code 12/12 hold, dense arm holds at 40/40 15/30 12/12 | `$M/t4-score.txt` |
| S3 | pass | developer total 40 and 40 against 38 and 38; **`where` 0/9 → 2/9 in both arms** | `$M/t4-score.txt` |
| S4 | pass | document held-out dense 105 → 100 and lexical 113 → 108, 0 gained 5 lost each, **p = 0.0625** — not significant | `$M/t4-score.txt` |
| S1, S5 | not reached | live clauses; Task 5 does not run on a failed offline verdict | — |

### The one case, by name

`$M/t4-s2-case.txt`. Recorded suite, lexical arm, paraphrase, «что мешает стереть карточку
клиента», anchor `FR-CRM-11`:

```
seat off  ADR-032, FR-AI-33, FR-SEC-07, FR-APP-44, N-129
seat on   ADR-032, FR-AI-33, sym:packages/db/src/seed/golden/clients.ts::GoldenClient, FR-SEC-07, FR-APP-44
```

The code seed takes the third slot and pushes `N-129` off the fifth. The anchor was reached by
expansion from `N-129`, so the seat costs the case through the seed it displaced rather than by
out-ranking the answer. One seat is still one seat.

### The gain, recorded beside the price

The two `where` anchors the seat finds, in both arms — `packages/domain/src/schedule/
subjectAvailability.ts` and `packages/db/src/schema/salon/catalog.ts`, seated at rank 3 in the
lexical arm and rank 4 in the dense — and the code held-out set (`$M/t4-score.txt`):

| arm | code held-out, seat off → on | gained | lost | p |
|---|---|---|---|---|
| dense | 54/400 → 122/400 | 70 | 2 | 0.0000 |
| lexical | 40/400 → 115/400 | 79 | 4 | 0.0000 |

### Beside A4's price

A4, under the raw-best ratio gate, read `where` 0/9 → 2/9 at six held-out document questions lost
in each arm, none gained, **p = 0.031** — a significant loss, disqualifying. This seat, under the
coverage admission at a constant of the code list's own, reads the same `where` 0/9 → 2/9 at five
lost and none gained per arm, **p = 0.0625** — not significant. The coverage form at its own
constant made the document held-out price survivable where the ratio did not. What ends it is a
clause A4 was never read against: one recorded paraphrase case in one arm.

**S is recorded and dropped.** `src/query.rs` is untouched — no `CODE_SEAT`, no `CODE_GATE`, no
override. The tooling that measured it ships (`7cf4d3b`) so the next attempt starts from a derived
constant and a replay that can express the seat, and the cost sitting was not run because the seat
was never built into the binary.

---

## G — the register (G14)

### L4 as framed, retired

The gap's own lever — a prompt per node kind asking for "is this allowed" questions — is retired
on the record. It failed its own diagnostic gate on both readings, 1 and 2 of 4 against a bar of 3,
and [the second diagnostic](2026-09-06-g14-second-diagnostic.md) showed it aimed at *shape* where
the loss is *register*. `src/enrich.rs` was never changed for it and no token was spent on it.

### The rule, as pre-registered

> The register prompt ships if, and only if, **all six clauses hold**, the new store against the
> fixture, both read by the final binary the plan leaves.
>
> 1. **No floor breaks.** … 2. **No arm loses ground on the recorded suite.** … 3. **The developer
> suite is not down and `rule` holds.** Total at least the fixture's in both arms, and `rule` at
> least 5/9 in both arms — the kind at risk, named before the run. 4. **No significant held-out
> loss on the neutral set.** … 5. **The replay agrees with the binary** … 6. **The lever does what
> it is for.** At least one of: paraphrase above the fixture's count in either arm; the neutral
> held-out set significantly better in either arm; `rule` above 5/9 in either arm. A pass on
> clauses 1–5 that moves none of these is recorded as *no effect* and does not ship.

### The store and the generator

`$M/bc-r1`: the fixture's graph and vectors, `questions.json` emptied so every eligible node is
stale, `embed_model` pinned to the small model before any writer ran, `enrich_model = "haiku"` —
what `src/config.rs` ships as the default — named in the copy's own toml so the record says which
generator. No `REPOGRAPH_ENRICH_MODEL` override and no machine config file. The prompt is the only
variable against the fixture, which the same generator enriched under the old prompt.

`enrich`: 1,996 nodes, 167 batches, 0 failed, 1,046 s, then a second pass for the 12 nodes the
first run left — 12 written, 0 still without questions — as the plan provides for. 24,816 questions
at a median of 13 a node against the fixture's 26,117 at 13; **1** question identical to the
fixture's; 32,224 rows on `intfloat/multilingual-e5-small`, dim 384 (`$M/t6-enrich.log`,
`$M/t6-enrich-2.log`, `$M/t6-survey.txt`).

**Cost.** Tee'd bytes 2,647,761 prompt and 2,795,432 output; at bytes ÷ 4 and Haiku 4.5 list rates
that is ≈ **$4.16** — an *estimate*, by the same method D1's ≈ $13.62 used, against the plan's
≈ $2.50 prediction (`$M/t6-bytes.txt`, `$M/t6-cost.txt`). The stronger generator's reading, priced
at ≈ $13.62, was not run: against the fixture it would move two variables.

An earlier attempt spent nothing. The plan's `enrich_command` carried `--no-session-persistence`,
which the installed CLI (2.0.35) does not have; it printed an unknown-option error and exited
without reading stdin, which the pipeline reads as an answer rather than a failure, so the run
reported `0 nodes written … 1996 still without questions` with a 0-byte output file
(`$M/t6-enrich-attempt1.log`, `$M/task6.txt`). The flag was dropped and nothing else changed.

### The set that decides

400 questions built from `bc-d1`'s questions at seed 20260905 — the same 400 anchor nodes as the
fixture's set, since `bc-d1` carries the same 1,996 keys, with texts from a generator in neither
store. The fixture holds 1 of the 400 verbatim and `bc-r1` holds 0 (`$M/t6-neutral-set.txt`). Both
stores are read on it by one binary, paired.

### The binary both stores were read on

Task 5 did not run, so the final retrieval binary is rule R's, `768f7b9…`. Task 6 Step 3 had
rebuilt the binary with the register prompt in it; that the retrieval half is unchanged was proved
rather than assumed (`$M/t6-binary.txt`): the only tracked change since the R commit is
`src/enrich.rs`, and that binary re-takes all four of R's fixture dumps byte for byte. After the
revert the rebuilt binary's sha is `768f7b9…` again, bit for bit.

### The clauses

| clause | verdict | reading | file |
|---|---|---|---|
| **G1** | **FAIL** | `bc-r1` dense breaks the enriched dense paraphrase floor of 14 — `keyword 40/40 paraphrase 13/30 code 12/12 p90 223` → `Error: bench floors not met`, exit 1. Lexical `40/40 13/30 12/12 p90 225` passes; p90 is within 230 in both | `$M/t6-r1-bench-rec-*-full.txt` |
| **G2** | **FAIL** | paraphrase 13/30 in both arms against the fixture's 15/30. keyword rises 39/40 → 40/40 lexical; code holds 12/12 | `$M/t6-*-bench-rec-*.txt` |
| **G3** | **FAIL** | `rule` **2/9 in both arms** against 5/9 — the kind named before the run. Totals 36 dense and 35 lexical against 38 and 38; `multi` rises 10/12 → 11/12 dense | `$M/t6-rule.txt`, `$M/t6-*-bench-dev-*.txt` |
| G4 | pass | neutral set dense 239 → 234 (32 gained, 37 lost, p = 0.6305), lexical 229 → 231 (53 gained, 51 lost, p = 0.9219) — neither significant | `$M/t6-heldout.txt` |
| G5 | pass | 82/82 and 60/60 replicated in both arms on `bc-r1`, exit 0 | `$M/t6-check.txt` |
| **G6** | **FAIL** | paraphrase below the fixture's in both arms; the neutral set not significantly better in either; `rule` 2/9, not above 5/9 | above |

The four `rule` cases lost are `ADR-005`, `INV-10`, `INV-07`, `ADR-031`; `ADR-003` is gained.

Recorded, with its bias stated: on the fixture's own held-out set — built from the fixture's
questions, whose siblings stay in the node when one is held out — dense 105 → 102 (p = 0.7948) and
lexical 113 → 105 (p = 0.3891). That set favours the fixture and did not decide.

### Why, mechanically

The register-share table, the second diagnostic's own metric, recomputed on all three stores
(`$M/t6-register-share.txt`):

| kind | n | fixture | bc-d1 | bc-r1 |
|---|---|---|---|---|
| long | 15 | 0.220 | 0.279 | 0.227 |
| cross | 15 | 0.156 | 0.163 | 0.150 |
| multi | 12 | 0.191 | 0.209 | 0.203 |
| where | 9 | — | — | — |
| rule | 9 | 0.211 | 0.164 | 0.147 |

The fixture and `bc-d1` columns reproduce the second diagnostic's shape — `rule` higher in the
fixture, `long` higher in `bc-d1` — so this is the same metric it named. The `bc-r1` column is the
finding: asking one generator for both registers did not raise the `rule` share toward the
fixture's 0.211; it fell to **0.147**, below both stores. Splitting twelve questions into six and
six did not add the asker's voice to a node — it halved the room the entry's own vocabulary had,
and `rule` and paraphrase both read the cost. The mechanism this lever rests on moved the wrong
way, and the suites agree with it.

**G is recorded and dropped.** `src/enrich.rs` is reverted; `$M/bc-r1` is kept for the next
attempt. The union of two generators' questions, which read paraphrase 20/30 and `rule` 7/9, still
has no rule in front of it and still ships nothing — but it is now the only reading in this line
that moved anything upward, and it is two stores rather than one prompt.

---

## What ships and what stays open

**Ships.** Rule R: `attainable` charges every query term, an absent one at df = 0, at
`QUESTIONS_GATE = 0.761` re-derived under the new form. Every ranked list byte-identical, every
recorded arm's counts identical, both held-out compares p = 1.0000, the replay reproducing the
binary on all 1,084 queries in both arms. With it, the bench tooling that measured the seat.

**Measured and rejected, with the file each rests on.**

| thing | why it did not ship | file |
|---|---|---|
| one seat for the code list, at `c_code = 0.902` | recorded lexical paraphrase 15/30 → 14/30 (S2), through the seed the code seat displaced. Gains recorded: `where` 0/9 → 2/9 both arms, code held-out 54 → 122 and 40 → 115 at p = 0.0000, document held-out p = 0.0625 where A4 read 0.031 | `$M/t4-verdict.txt` |
| the both-registers prompt, on the default generator | `rule` 5/9 → 2/9 both arms, paraphrase 15/30 → 13/30 both arms, the dense paraphrase floor broken, and the register share it rests on falling 0.211 → 0.147 | `$M/t6-verdict.txt` |
| L4 as framed, a prompt per node kind | retired on the record: 1 and 2 of 4 against a bar of 3, and aimed at shape where the loss is register. No tokens | [second diagnostic](2026-09-06-g14-second-diagnostic.md) |
| design B for the residue, the store-vocabulary denominator | not needed — design A passed all seven clauses, so the fallback was never tried | `$M/t2-verdict.txt` |
| the stronger generator's register reading | priced at ≈ $13.62 and not run: two variables against the fixture | this document |

**Still open.** G13 — the code list ranks the `where` anchors and has no seat that survives the
recorded suite; the price is now known under the coverage form at the list's own constant. G14 —
the register is the named cause and the single-generator lever is measured and rejected; what is
left is two stores' questions merged, unruled.

**Tests.** 445 Rust unit tests, 0 failed, 2 ignored; 12 `serve`; clippy clean at
`--release --all-targets -D warnings`. Python: 16 in `bench/test_admission.py`, 2 in the new
`bench/test_heldout.py`, 42 in `bench/history/test_track.py`.

**Token spend.** One point, once: the register enrichment, ≈ $4.16 estimated. Every dump, replay,
bench, held-out set, crossover and embedding in this document is zero tokens.
