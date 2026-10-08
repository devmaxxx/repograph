# A low-confidence line on `ask` — the rule, written before it is measured

**Status:** rule only. Results are appended below once measured; nothing above this line changes
after the first number is read.

## Why

An agent reading `ask` cannot tell a miss from a hit: five seeds come back either way, and a
plausible neighbour of the answer is what a model then reasons from. A line saying the retrievers
did not find the question in the store tells it to verify, to say "not found", or to spend
`ask --rerank` on that one question — instead of spending it on every question or on none.

## The signals

All from the plain fused path; an answer whose exact seeds cover the whole question is confident by
construction and never flagged. Per question, from `repograph dump`'s record:

- **coverage** `cov = max(best_p / att_p, best_q / att_q)` — the BM25 coverage the admission
  already computes, over the passage list and, where enriched, the questions list.
- **dense** `cos` — the top cosine in the dense passage list.
- **agreement** `agree` — the dense list's first passage is in the top five of either BM25 list, or
  either BM25 list's first is in the dense top five.

Candidate rules: `cov < T`; `cos < D`; `!agree`; `cov < T && !agree`; `cos < D && !agree`.

## Choosing and judging

- **Thresholds** `T` and `D` are chosen on the 60 dev cases (`bench/dev-cases.jsonl`): the value
  that flags the most misses while flagging at most 15% of hits.
- **Judged** on the 82 recorded cases, never tuned on them, on two stores: the fixture's
  (e5-small) and the same corpus under embeddinggemma-300m.
- **Ships** if one rule, on both stores, flags **at least half the misses** and **at most 20% of
  the hits**. The best such rule by misses flagged wins; ties go to the simpler rule.
- **Does not ship** otherwise: then only the skill's rule (cite `file:line` from the answer or say
  "not found") ships, and no line is printed.
- Reported either way: each rule's flagged misses and hits on both suites and both stores, and what
  `--rerank` on the flagged recorded cases would cost against reranking every case.

A hit is `bench`'s: an anchor id among seeds or the expanded line, or an anchor file among the
seeds. The offline count is checked against `bench`'s totals for the same store before any rule is
read.

## Results, 2026-10-08

Read with `repograph dump` over both suites on each store, and replayed offline. The offline hit
count matched `bench` on both stores before any rule was read: 40/14/12 and dev 34/60 under
e5-small, 40/20/12 and dev 36/60 under embeddinggemma-300m.

Recorded suite, with each threshold chosen on the dev suite:

| rule | e5-small misses | e5-small hits | gemma misses | gemma hits |
| --- | --- | --- | --- | --- |
| `cov < T` | 0/16 | 0/66 | 0/10 | 0/72 |
| `cos < D` | 12/16 | 25/66 | 5/10 | 15/72 |
| `!agree` | no setting under 15% of dev hits | | no setting | |
| `cov < T && !agree` | 0/16 | 0/66 | 0/10 | 0/72 |
| `cos < D && !agree` | **9/16** | **9/66** | 4/10 | 11/72 |

**Not shipped.** `cos < D && !agree` passes on the small model and misses the bar on gemma by one
case, 4 of 10 misses where 5 were required. No rule passes on both stores, and gemma is the default,
so no line is printed. The skill's rule ships alone: cite a `path:line` an answer printed, or say
the repository does not show it.

BM25 coverage separates nothing on the recorded suite: every recorded question, hit or miss, covers
more of its query than the dev-chosen threshold. The dense cosine and the agreement between
retrievers are where any signal is, and ten gemma misses are too few to calibrate one on.
