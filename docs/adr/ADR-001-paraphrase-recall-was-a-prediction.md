# ADR-001 · Paraphrase recall was a prediction, not a measurement

**Status:** Accepted, 2026-09-01; amended 2026-09-02 (see the last section)

## Context

The design note ([`docs/superpowers/specs/2026-09-01-repograph-design.md`](../superpowers/specs/2026-09-01-repograph-design.md))
proposed exact-id → BM25 → local embeddings, fused by reciprocal rank, then a one-hop expansion over
the hand-written id graph. Its "Measurements that drive the design" table labels the fused
approach's paraphrase score `target ≥12/14` — the one row whose number is not a measurement. Two
sections later its alternatives table prints the same `≥12/14` bare, and its phase table makes it the
phase-4 gate ("paraphrase ≥12/14 on `bench/cases.jsonl`"). From there it travelled unchanged into
the implementation plan's gates line (`docs/superpowers/plans/2026-09-01-repograph.md`: "paraphrase
≥7/14 with `--no-dense`, ≥12/14 with dense") as though it had been measured rather than guessed.

It hadn't been. Every paraphrase number this project measured, in the order it measured them:

| approach                                                    | paraphrase |
| ------------------------------------------------------------ | ------------ |
| graphify (LLM graph, BFS depth 2)                             | 0/14         |
| BM25 over requirement bodies, Russian stemming (prototyped)   | 4/14         |
| BM25 seed + one-hop over the hand-written id graph (prototyped) | 7/14       |
| **repograph, shipped: exact → BM25 → dense, RRF-fused, one hop** | **5/14** at `--seeds 5` |

`--seeds` trades cost for a little more recall — 4/14 at `--seeds 3`, 5/14 at `--seeds 5`, 6/14 at
`--seeds 8` — one extra hit per step, each step costing roughly 1.5× the tokens (median 131 → 202 →
297). None of those points reach the ≥12/14 target.

## Decision

Set the shipped floors to what was measured, not to what the design note predicted:
`bench`'s paraphrase floor is 5/14 with embeddings and 2/14 with `--no-dense` (`src/bench.rs`,
`passes`). The gap between 5/14 and the ≥12/14 target is recorded here rather than left to be
rediscovered, because two diagnostics were run against the obvious explanations and both came back
negative:

- **Dense retrieval helps, so it is not the cause of the shortfall.** `--no-dense` alone scores 2/14;
  fusing in the embedding retriever raises that to 5/14 on the same questions.
- **Widening the one-hop expansion cap is not the fix.** Raising `MAX_EXPANDED` from 1 to 8 buys at
  most one extra paraphrase hit and takes bench's own p90 from 218 to 510 tokens — a cost `bench`'s
  p90 floor exists to catch, for one hit.
- **The embedding index is not miscomputed.** All 6,691 stored vectors are unit-norm and
  `DenseIndex::search` is true cosine, not an approximation.
- **Restricting the candidate pool to `Requirement` nodes — the "dilution" hypothesis — changes
  nothing.** Top-5 recall is 2/14 either way, because the lexical top-20 for these questions is
  already close to 100% requirements; there is no non-requirement noise to filter out.

The real cause sits in the questions, not the retrievers: three of the fourteen paraphrase targets
share zero stems with their query, and most of the rest share only high-document-frequency filler
words. No BM25 variant ranks a document with no shared terms above one that shares even a rare term,
and `MultilingualE5Small` places five of the missed targets outside its own top-50 of 6,691 candidates —
the embedding model itself does not consider them close. The named next lever, if paraphrase recall
needs to move further, is a larger embedding model (`MultilingualE5Base`), not more seeds and not a
wider expansion cap.

## Consequences

- `bench`'s floors, and the README's Bench section, state 5/14 (dense) and 2/14 (`--no-dense`) as
  what was measured. `repograph` still beats graphify — the incumbent it replaces — on every axis:
  paraphrase, keyword, tokens per answer, and tokens to build the graph.
- The implementation plan's deviations table (`docs/superpowers/plans/2026-09-01-repograph.md`)
  claims "None of these changes the accuracy gates" for its four spec deviations. That claim is false
  for one of the four: reproducing the BM25-over-bodies prototype's configuration on the shipped
  hand-rolled index scores 2/14, where `tantivy` recorded 4/14 on the same prototype. The claim has
  been struck from that table and replaced with a citation to this ADR.
- If paraphrase recall needs to improve further, the next experiment is `MultilingualE5Base`, not
  another seeds/hops tuning pass — both of those levers are measured: each still buys about one hit,
  and each pays for it with the token budget the bench exists to protect.

## Amendment, 2026-09-02

A per-case diagnostic showed five of the six paraphrase hits sitting at dense rank 2 and one
target (`FR-SVC-39`) at dense rank 2 yet absent from the seeds: reciprocal rank fusion ranks ids
that both lists agree on above a single list's second hit, and with 20 candidates per list there
were enough such ids to fill five seeds. Replacing RRF with a dense-first interleave (rank 1 of each
list, then rank 2 of each) measured 6/14 with embeddings, 24/24 keyword, 3/3 code, p90 217 tokens
against 215; `--no-dense` is a single list and is unchanged at 2/14. The dense floor is now 6/14.
Of the eight remaining misses, two targets are outside both lists' top 100 and six sit at dense
rank 32–81, which no fusion rule reaches at five seeds — the gap to ≥12/14 stands and remains a
retrieval question, not a fusion one. Three retrieval levers were then measured under the
interleave, each against the same eight misses: `MultilingualE5Base` 6/14 (a different six —
`FR-AI-102` and `FR-AI-21` in, `FR-TOOL-22` and `FR-SVC-39` out) at p90 225; a 512-token passage
cut 5/14 at 269 s of embedding against 132 s; a label-only second vector per node with max scoring
6/14 at 244 s; `BGEM3` 5/14 at 1,454 s; the quantized paraphrase MiniLM 2/14 with keyword down to
21/24. None is adopted. Six of the eight misses are shared by every E5 variant and the two that
move (`FR-AI-102`, `FR-AI-21`, `FR-TOOL-18` under the larger models) come at the price of others,
which points at the model's distance between these questions and their targets rather than at
anything the index does with the vectors.

## Second amendment, 2026-09-02

With the embedding levers exhausted, the remaining eight misses were measured against model help,
on a copy of the corpus. Generating reader questions per node (doc2query, Haiku headless, ≈$2.5 and
16 minutes once for 1,971 nodes) and embedding them as rows of their own moves nothing at five
seeds — 6/14, the same as without — because the target's questions compete with every other node's
questions in the same space; on a partial run where only the fourteen targets were enriched the
score read 8/14, which was the targets winning an uneven contest, not a gain. Pooling question rows
with passage rows is worse still (4/14). What the questions do is carry targets into a deeper pool:
all six reachable misses sit within the top 100 fused candidates with them, four without.

A model picking five ids from that pool by title (`ask --rerank`, ≈4,800 tokens and 3.5 s per
question, the fused top two pinned) measures 10–11/14 with the questions and 8/14 without, keyword
24/24 either way. Query rewriting by the model was measured at 20/24 keyword and 6/14 paraphrase
and rejected. Both model stages are opt-in and documented in the README; the zero-token floors are
unchanged. A local cross-encoder (`bge-reranker-v2-m3`, a 2.3 GB download) was not measured.

## Amendment 3 — what the model is shown (2026-09-02)

Three of the misses that survived the title-only reranker were not retrieval failures. `FR-VIS-76`'s
paraphrase asked about withdrawing consent where the entry is about who may leave a review
(«отзыв»); «экспорт данных для налоговой отчётности» describes the DAC7 tax-reporting cluster at
least as well as its target `FR-PAY-104`; «перечень незыблемых требований продукта» fits the
individual invariants as well as their registry `FR-VIS-01`. A model shown the pool picked between
the readings at random, 3/5 and 4/5 over five runs of one question. All three cases were rewritten,
still sharing no word with their target lines. The floors did not move by that: the zero-token
path reads 7/14 before and after.

With the exam fixed, the reranker's remaining lever was the prompt. Showing each candidate's title
plus the first 120 characters of its text, deepening the pool to 200 so `FR-TOOL-18` (pool rank 142)
is inside it, and dropping the two pinned seeds — a title is not evidence, so the pins protected a
keyword hit; a snippet is, and the pins were then the retrievers' guess taking two of the model's
five slots — reads 14/14 paraphrase, 24/24 keyword, 3/3 code on three consecutive runs with
sonnet, at ≈19k input tokens and ~4.3 s per question. Haiku with the same prompt reads 11/14 and
opus drops a keyword hit in two runs of two, so the reranker's default model is sonnet while
`enrich` stays on haiku. The zero-token floors are unchanged; `--rerank` stays measured, not floored.

## Amendment 4 — the cross-encoder, measured (2026-09-04)

The one lever the second amendment left unmeasured. `bge-reranker-v2-m3`, exported to ONNX
(`optimum-cli`, 2.27 GB, 1 min 12 s), loaded through the embedder's own `ort` path, scoring
the same 200-deep pool `--rerank` shows the model command. Rule, written before the run: the
paraphrase floors move only on paraphrase ≥ 17/30 with keyword 40/40, code 12/12, p90 ≤ 230 and
a median under one second a question.

| arm | keyword | paraphrase | code | p90 | s / question |
|---|---|---|---|---|---|
| control (this store, dense) | 40/40 | 15/30 | 12/12 | 226 | 0.06 |
| `--rerank-local`, depth 200 | 39/40 | 17/30 | 12/12 | 229 | 17.9 |
| `--rerank-local --depth 100` | 39/40 | 14/30 | 12/12 | 230 | 8.9 |
| `--rerank-local --depth 40` | 39/40 | 15/30 | 12/12 | 237 | 3.6 |

Seconds per question are the run's **mean** wall clock over its 82 cases, on an M-series laptop
with the session opened once for the whole run; the control's 0.06 s is the same arithmetic and
excludes the model open both arms pay. The rule above names a **median**, and no per-question
distribution was captured, so the rule and the number do not name the same statistic. A mean does
not bound a median in general; here the cost of every question is dominated by a fixed 200 forward
passes, so a median under a second behind a mean of 17.9 s would need a skew this arm cannot
produce. The verdict is unaffected, but it rests on that argument rather than on the statistic the
rule was written in.

The depth-200 row is the one the decision rests on, so it was run twice and the second run is
kept, both streams, in
[`bench/results/2026-09-04-beauty-crm-task6-rerank-local-depth200.log`](../../bench/results/2026-09-04-beauty-crm-task6-rerank-local-depth200.log).
**It reproduced case for case** — 39/40, 17/30, 12/12, p90 229, and the same HIT/miss on all 82
questions — and the capture carries stderr, so the record shows that `score` never fell back to
the fused order on any question. Its wall clock read 21.0 s a question rather than 17.9 s, under
other load on the same machine; the table keeps the quiet run's figure and neither reading is
within an order of magnitude of the bar.

**Only that arm's output was kept.** The control, `--depth 100` and `--depth 40` streams were not
captured, so three of the four rows in the table cannot be checked by a reader — and the control
is the row everything else here is measured against. What rests on an uncaptured run: the
control's 15/30 and its 0.06 s, the per-case in/out lists below, the +2-paraphrase-for-−1-keyword
net, and the claim that `NFR-STAFF-04` is the same keyword loss in every arm. What does not: the
depth-200 row itself, which is on disk twice over, and the verdict, which fails on latency
against any control.

It stays opt-in, on two of the five conditions. Latency is the decisive one and it is not close:
17.9 s a question at depth 200 is eighteen times the bar and four times what `--rerank` pays a
remote model. A cross-encoder is one forward pass **per candidate** — 200 pairs of (question,
snippet) through a 568M-parameter XLM-R encoder, against one embedding of the query. Depth is the
dial that was measured, and it trades the paraphrases away: 100 deep costs half the time and
reads 14/30, 40 deep reads 15/30 at a p90 of 237 that is over the token ceiling on its own.
`score` batches in fused order; the length-sorted batching `embed.rs` already uses, which would
stop a batch padding to its longest member, was not tried here. It would not change the verdict —
the gap is eighteen-fold, not marginal — but the claim is that no batching change plausibly
closes it, not that none was available.

Keyword is the second failure and it is the same case in all three arms: `NFR-STAFF-04`
(«ведомость мастера не видна»). The cause is not the reranker's judgement. A candidate is shown
`rerank::text` — the label plus the first **120** characters of the collapsed body — and the
sentence the question quotes verbatim, «Ведомость мастера не видна другим мастерам», begins at
character **139** of that 425-character body. The cross-encoder was ranking a snippet that did
not contain the match, and its five picks are topically coherent; BM25 indexes the whole body and
keeps the case. So **snippet length is a real and unmeasured dial for this arm**, and it is free
here: the 120-character cap is a token-budget constant shaped for `--rerank`, which pays per
character sent to an API. A local model pays nothing for a longer snippet but its own compute. It
was not tried, deliberately: no snippet change moves an eighteen-fold latency gap, so measuring it
would only sharpen the account of an arm already rejected. It is the first thing to measure if
the latency problem is ever solved.

Against the fourteen misses of `docs/bench/2026-09-03-three-graphs-results.md` — the store now
reads 15/30, not that day's 16/30, so the control is the comparison, not the doc — depth 200
gains five. In: `FR-SEC-21`, `FR-AI-102`, `FR-RPT-13`, `FR-WH-18`, `INV-16`.
`FR-AI-102` is the more interesting of them, one of the two G2 named as never surfacing the
right file at all; the reranker finds it. Out, against the committed runs, **four**:
`FR-PAY-03`, `FR-SVC-39`, `ADR-004` and `N-109` — the fused order had all four right and the
cross-encoder scored them below five others. `N-109` is the one only the log settles: it is a
**hit** in every committed run of this store at 16/30 (2026-09-03 and 2026-09-04 alike) and a
miss in the depth-200 capture. So `15 + 5 − 3 = 17` closes only if the uncaptured control missed
`N-109` as well, and nothing on disk shows that it did. `FR-CAL-101`, the other of
G2's two, is missed by the control and by all three reranked arms, which is the coverage finding
G2 predicts and not a reranker failure. `INV-16` is the only case that flips at every depth, so
of the five gains four need the pool 200 deep. Net against the control, the swing at the one depth
that clears the paraphrase bar is +2 paraphrase for −1 keyword, bought at 300× the latency — a
net that is arithmetic on a control row nobody can re-read.

Adoption would in any case have been a separate change, with its own commit, moving the floor in
`bench::passes` and the README's Bench list — this amendment records a number and does not move
anything. The rule is not met, so there is nothing to adopt: `--rerank-local` ships opt-in and
off every floor, as `--rerank` does. What it settles is the ADR's open question — the local
cross-encoder is measured, and it does not buy paraphrase recall at the zero-token, sub-second
budget the floors are written to. Two dials remain untried on this arm and neither is a floor
candidate on its own: snippet length above, and length-sorted batching.

## Amendment 5 — the floors name the store they grade (2026-09-04)

Amendments 2 to 4 call the shipped numbers "the zero-token floors". Building, refreshing and
querying do cost zero tokens, so the phrase was natural, but every floor those amendments left
standing was measured against a store `enrich` had already filled — Amendment 4's control row is
that store — and `enrich` spends money. Built from scratch on the same corpus at the same commit
and never enriched, the store reads `keyword 40/40  paraphrase 9/30  code 12/12  p90 221 tok`
with embeddings and `keyword 39/40  paraphrase 7/30  code 12/12  p90 226 tok` with `--no-dense`,
each arm run twice with identical results. Against a paraphrase floor of 14 and 11 and an exact
keyword floor, `repograph bench` therefore failed on any honestly built index whose owner had not
paid for enrichment, and read as a broken setup rather than as an option not taken.

`bench::passes` now takes the store's state alongside the arm. A store carrying questions on at
least 99% of its requirement-like nodes is graded 14 and 11; one below that mark is graded 9 and
7, with the keyword floor at 39 in the lexical-only arm, and the summary line prints
`enriched=<bool> (<covered>/<eligible> nodes)` so a red run says which bar it was held to. The
enriched floors did not move — they are still what this store measured; what moved is the claim
that they describe a configuration nobody paid for. A part-enriched store is graded raw: the test
is coverage, not passage freshness, so an edited requirement does not reclassify a store that is
otherwise complete while a `--limit` run does not earn the enriched bar. The mark is a high-water
one and not every node because equality over 1 996 nodes is a cliff — one requirement added after
the pass, one node the model skipped past its retry, one entry dropped on load — and a store that
falls off it is regraded five paraphrase points lower, which is a blind spot a gate speaking
through an exit code cannot afford.

**The enriched lexical-only arm is red, and stays red.** Grading the store honestly exposed a
second thing the old grading hid. Run today, the reference store reads
`keyword 40/40  paraphrase 15/30  code 12/12  p90 226 tok  enriched=true (1996/1996 nodes)` with
embeddings, and `keyword 37/40  paraphrase 15/30  code 12/12  p90 220 tok` with `--no-dense`,
where `repograph bench` exits 1 on `FR-WH-53`, `FR-PH-43` and `W-206`. The mechanism is fusion
order, not the floor: on the plain path `query::ask` pushes the generated-questions BM25 list into
`fuse::interleave` before the passage list, so the questions lead the interleave and displace two
exact keyword seeds; the arm with embeddings is unaffected because the dense passage list is
pushed first and leads there. A raw store reads keyword 39/40 in that same arm, already missing
`FR-PH-43`, so enrichment is what costs the other two.

The floor is not lowered to 37. A floor that follows a regression down is not a floor, and the
regression here is caused by the very stage the enriched floors exist to describe — lowering it
would invert this amendment's own thesis. It is recorded as G7 in
[`next-version-gaps.md`](../bench/next-version-gaps.md), to be measured across all four arms
before a lever is chosen, and the arm is red in the meantime.
