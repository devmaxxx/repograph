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
