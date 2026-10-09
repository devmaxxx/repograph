# ADR-004 · haiku is the default rerank model

**Status:** Accepted, 2026-10-09. Supersedes the rerank-model half of ADR-001's fourth amendment.

## Context

ADR-001 made sonnet the reranker's model because haiku, given the same prompt, read 11/14
paraphrase cases against sonnet's 14/14. In that prompt each candidate was shown as its title and
the first 120 characters of its body.

That snippet hid evidence. NFR-STAFF-04 says «ведомость мастера не видна» about 170 characters
in, behind a sentence about attribution, and both models passed it over. Since 12d5c8e the snippet
is the sentence of the body that shares the most stemmed terms with the question. A body that
matches nowhere still shows its start. At the same length the prompt grew 1.8% (median 53,263 →
54,219 bytes).

Read 2026-10-09 on a copy of the fixture re-embedded under embeddinggemma-300m, without enrich,
rerank depth 200, one run per cell (`bench/history/runs.jsonl`, tag `copy-gemma:*`):

| | 82 cases | dev 60 | wall, 82 / dev |
| --- | --- | --- | --- |
| sonnet, first 120 characters | 40 · 30 · 12 | 55/60 | 316 s / 283 s |
| haiku, first 120 characters | **39** · 30 · 12 | 54/60 | 215 s / 214 s |
| sonnet, matched sentence | 40 · 30 · 12 | 55/60 | 233 s / 169 s |
| haiku, matched sentence | 40 · 30 · 12 | 55/60 | 215 s / 221 s |

Every miss left on the dev suite is outside the 200-deep pool under either model, so no choice of
model can reach it (issue #164).

## Decision

**`RERANK_MODEL` is `haiku`.** Shown the matching sentence, it reads what sonnet reads on both
suites, and it is the cheaper model per token.

## Consequences

- `--rerank` costs less per question: the prompt is the same, and haiku's tokens are cheaper.
  Speed is not established. Each pair of runs went side by side through the same `claude -p`, and the
  wall times point both ways — haiku faster on three of the four columns, slower on the last.
- One run per cell. A model is only as good as the next run, and haiku's keyword case at fused
  rank 2 was a coin flip for both models before the snippet change. A project that sees haiku drop
  a case sets `rerank_model = "sonnet"` in `repograph.toml`, or `REPOGRAPH_RERANK_MODEL=sonnet` for
  one run.
- `--rerank` stays measured, not floored, as before.
