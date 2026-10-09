# How it works

It is built around one claim: **the hard part is not building a better graph, it is building a better
door onto it.** A repository's requirement ids already form a dense, human-authored graph. What was
missing was a way in that finds the right entry point from a question phrased in ordinary words.

Measured against `graphify`, the LLM-extracted graph it replaces, on the 38 questions of the day:
0/14 paraphrase and 11/24 keyword at 1,027–1,555 tokens an answer and 14.6 M tokens to build,
against 7/14, 24/24, 197 median tokens and zero. The table and its caveats are in
[the measurements](history.md#the-head-to-head-against-the-graph-it-replaces); the recorded set
has since grown to the 82 cases [Bench](benchmarks.md#bench) floors.

Exact id or symbol first, then BM25 over passages, then a
dense list, interleaved rank by rank; the surviving seeds expand one hop over the id graph, and the
answer is rendered as `ID  path:line  headline`. The node kinds, the edge kinds, what the
TypeScript and markdown extractors take, and what `impact`, `trace` and `changes` can and cannot
prove are in [the graph model](graph-model.md).

Two things worth knowing before you trust an answer: the graph records only what a file proves, so
a call through a chained expression or a callback has no edge and a "nothing uses this" wants an
`rg -l` beside it; and `impact`'s risk label is four fixed thresholds printed with the counts they
came from, so it can be argued with.

## Design

The full design note and implementation plan are in
[`docs/superpowers/specs/2026-09-01-repograph-design.md`](superpowers/specs/2026-09-01-repograph-design.md)
and [`docs/superpowers/plans/2026-09-01-repograph.md`](superpowers/plans/2026-09-01-repograph.md).
The plan's deviations table lists four simplifications against the spec, none of which change the
node/edge model or the answer shape; one of them moved a measured number, and
[ADR-001](adr/ADR-001-paraphrase-recall-was-a-prediction.md) says which.
