# 0001 · Paraphrase recall was a prediction, not a measurement

**Status:** Accepted, 2026-09-01

## Context

The design note ([`docs/superpowers/specs/2026-09-01-repograph-design.md`](../superpowers/specs/2026-09-01-repograph-design.md))
proposed exact-id → BM25 → local embeddings, fused by reciprocal rank, then a one-hop expansion over
the hand-written id graph. Its own alternatives table labels the fused approach's paraphrase score
`≥12/14` as a **target** — the word the table itself uses next to every other row's measured number.
That target then travelled unchanged into the implementation plan and its phase-4 gate
(`docs/superpowers/plans/2026-09-01-repograph.md`, Phase 4: "paraphrase ≥12/14 on `bench/cases.jsonl`")
as though it had been measured rather than guessed.

It hadn't been. Every paraphrase number this project measured, in the order it measured them:

| approach                                                    | paraphrase |
| ------------------------------------------------------------ | ------------ |
| graphify (LLM graph, BFS depth 2)                             | 0/14         |
| BM25 over requirement bodies, Russian stemming (prototyped)   | 4/14         |
| BM25 seed + one-hop over the hand-written id graph (prototyped) | 7/14       |
| **repograph, shipped: exact → BM25 → dense, RRF-fused, one hop** | **5/14** at `--seeds 5` |

`--seeds` trades cost for a little more recall — 4/14 at `--seeds 3`, 5/14 at `--seeds 5`, 6/14 at
`--seeds 8` — about one extra hit per doubling, at roughly 1.5× the tokens. None of those points reach
the ≥12/14 target.

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
and `MultilingualE5Small` places five of the eight misses outside its own top-50 of 6,691 candidates —
the embedding model itself does not consider them close. The named next lever, if paraphrase recall
needs to move further, is a larger embedding model (`MultilingualE5Base`), not more seeds and not a
wider expansion cap.

## Consequences

- `bench`'s floors, and the README's Bench section, state 5/14 (dense) and 2/14 (`--no-dense`) as
  what was measured. `repograph` still beats graphify — the only other measured system — on every
  axis: paraphrase, keyword, tokens per answer, and tokens to build the graph.
- The implementation plan's deviations table (`docs/superpowers/plans/2026-09-01-repograph.md`)
  claims "None of these changes the accuracy gates" for its four spec deviations. That claim is false
  for one of the four: reproducing the BM25-over-bodies prototype's configuration on the shipped
  hand-rolled index scores 2/14, where `tantivy` recorded 4/14 on the same prototype. The claim has
  been struck from that table and replaced with a citation to this ADR.
- If paraphrase recall needs to improve further, the next experiment is `MultilingualE5Base`, not
  another seeds/hops tuning pass — both of those levers are now measured as exhausted.
