# The three-graph comparison — result, 2026-09-03

One corpus, one commit, the same 102 questions to three tools. The protocol is
[`three-graphs.md`](three-graphs.md); the raw rows are
[`../../bench/results/2026-09-03-beauty-crm.json`](../../bench/results/2026-09-03-beauty-crm.json);
this file is what the rows mean and what was decided because of them.

| | |
|---|---|
| corpus | `beauty-crm` at `7733bd53`, clean tree |
| run at | 2026-09-03 19:43 |
| cases | 82 retrieval (40 keyword · 30 paraphrase · 12 code), 20 blast (10 impact · 8 trace · 2 changes) |
| repograph | 0.4.0, `enrich` on opus — 8 306 nodes / 32 590 edges |
| graphify | rebuilt the same day, `update --force` in a worktree — 28 868 nodes / 54 769 edges, 0 tokens off the semantic cache |
| gitnexus | analyzed the same day at `d04b19a4` — 25 745 nodes / 42 654 edges |

## Retrieval — 82 questions

Strict means the expected id appears in the answer. Soft means a file that spells
that id appears. The gap between the two columns is the portrait of a tool.

| | keyword | paraphrase | code | **strict** | soft | median ms | median chars |
|---|---|---|---|---|---|---|---|
| **repograph** | **40/40** | **16/30** | **12/12** | **68/82** | 80/82 | **385** | **606** |
| graphify | 23/40 | 1/30 | 9/12 | 33/82 | 47/82 | 1 548 | 6 812 |
| gitnexus | 0/40 | 0/30 | 12/12 | 12/82 | 60/82 | 1 438 | 6 246 |

Three different shapes, and each one is a statement about what the tool models.

**gitnexus scores 0 strict on every prose question and 60 soft.** It is never in the
wrong file and it never names the id. It indexes code; a requirement id is a token
in a Markdown line, and nothing in its model treats that token as an entity. Its
12/12 on code symbols is the same fact from the other side: symbols are what it has.

**graphify is the only tool that loses on both criteria** — 33 strict, 47 soft. The
LLM-built concept graph answers with a cluster, and the cluster is frequently the
neighbouring subject. Its 1/30 on paraphrases is the number that ended its use here:
a concept graph was supposed to be exactly the thing that survives a question sharing
no word with its answer, and it does not.

**repograph's own gap is 68 strict against 80 soft** — twelve cases where it opened
the right file and named a neighbouring requirement. Those twelve are the subject of
[`next-version-gaps.md`](next-version-gaps.md); they are a retrieval problem, not an
extraction one.

Answer size is the other axis and it is not close: 606 median characters against
6 812 and 6 246. A tool that is read by an agent pays for every character of context
it spends, ten times per session.

## Blast radius — 20 cases

### impact, 10 targets by fan-in

Recall is the share of files that reference the symbol which the answer names.

| | mean recall | files found / wanted | median ms |
|---|---|---|---|
| **repograph** | **0.949** | **110 / 114** | **41** |
| gitnexus | 0.650 | 70 / 114 | 948 |
| graphify | 0.600 | 45 / 114 | 427 |

repograph's four missing files are three targets: `TenantContextInterceptor` 3/4,
`OutboxPublisher` 6/7, `AuthService` 9/10. Every narrow target is 1.0.

graphify's 0.600 is measured over six targets, not ten: `affected` answered
`No unique node match` for `DatabaseService`, `TenantContextInterceptor`,
`ActorResolver` and `OutboxPublisher` — an ambiguous-name refusal, scored as a miss
because a blast radius nobody can obtain is a blast radius of nothing.

### trace, 8 pairs — six real chains, two with no path

| | hit | real chains | correct refusals | median ms |
|---|---|---|---|---|
| **repograph** | **8/8** | **6/6** | **2/2** | **42** |
| graphify | 5/8 | 3/6 | 2/2 | 1 604 |
| gitnexus | 2/8 | 0/6 | 2/2 | 924 |

gitnexus answered `"no_path"` to all six chains that exist. Its two points come from
the two pairs where no path was the right answer, which a tool that always says
`no_path` collects for free. This is the single row that ended its use here: blast
radius and call traces were the one thing it was still kept for.

### changes, 2 diffs

| | symbols found / wanted | files found / wanted | median ms |
|---|---|---|---|
| **repograph** | **32 / 43** | **13 / 20** | **204** |
| gitnexus | 1 / 43 | 1 / 20 | 1 270 |
| graphify | — (no equivalent command) | — | — |

Split by case, repograph is 5/5 symbols on the small diff (`cbc931ba~1`, 2 files) and
27/38 on the large one (`0f27d2d1`, 18 files). The eleven it misses and the seven
files behind them are the language boundary, not a ranking failure — see the gaps
document.

gitnexus's 1/43 is partly its CLI: `detect-changes` prints fifteen symbols then
`... and N more`. The MCP payload was read by hand for base `cbc931ba~1` and carries
`staffMember` but still not `AvailabilityAnswer`, so the cap is not the whole of it.

## Cost to build, and what that buys

| | build | tokens to build |
|---|---|---|
| **repograph** | ~1 s lexical + ~100 s for 7 398 vectors | **0** |
| graphify | minutes, off a warm semantic cache | 17 516 682 input tokens over 16 historical runs |
| gitnexus | 236 s, ~598 MB on disk | 0 (local embeddings) |

repograph's optional stages are separate and opt-in: `enrich` is ≈$2.50 once on
haiku, `ask --rerank` is ≈19k input tokens per question on sonnet. Neither is on any
floor, and the build costs above are the zero-token path's; the retrieval numbers in
this document are not, since this store had had an `enrich` pass (see the caveats).

## What was decided

1. **repograph is the only graph this project uses.** It wins strict retrieval
   68/82 against 33 and 12, impact 0.949 against 0.650 and 0.600, trace 8/8 against
   5/8 and 2/8, and answers in a tenth of the characters.
2. **graphify's edges are not worth importing.** Measured separately:
   `repograph import-legacy graphify-out/graph.json` adds 8 577 concept nodes and
   23 117 edges in 0.2 s and moves the shipped bench from 40/15/12 to 38/16/12 — two
   keyword hits traded for one paraphrase, below the binary's own floors. Rolled back.
3. **Both predecessors stay on disk and unused** in `beauty-crm`: `graphify-out/`
   tracked, `.gitnexus/` ignored, no command from either run against that tree. The
   record is worth keeping; running the tools is not.

## Caveats that travel with these numbers

- One run per row, no repeats. For a fraction out of 30 the Wilson interval is wider
  than the gap between adjacent rows; only the large gaps above are safe to read.
- Strict is a substring test. An id ranked fifth of five counts as a hit.
- Latency is whole-process wall clock, so every tool pays its own start-up. gitnexus
  has an `eval-server` that skips it and was not used, because the other two have no
  equivalent.
- repograph's 16/30 here is above the README's shipped floor of 14/30 because this
  run used an `enrich` pass on opus. That 14/30 is the floor for a store carrying
  generated questions — a store without them is graded on its own lower numbers —
  and this table is what one particular store measured.
- The case set lives in this repository and was written against `beauty-crm`. The
  keyword and code halves are fair to all three; the paraphrases are what make the
  set hard, and they are hard in Russian, which is a property of this corpus.
