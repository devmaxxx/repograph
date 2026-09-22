# The embedder, read on nine models

The store's dense index is written by one sentence-embedding model, named by `embed_model` and
defaulting to `intfloat/multilingual-e5-small`. This document is what nine candidates for that seat
read on the bench suite, and what changing the seat costs in time, memory and disk.

One run per model, sequentially, on an idle machine. Corpus: the pinned fixture beauty-crm at
`502e8a6d` — Russian requirement documents and a TypeScript monorepo — enriched, 1,996 enriched
nodes, 33,525 embedded rows. Suite: the 82 cases, 40 keyword, 30 paraphrase, 12 code, against the
enriched store with no rerank. Machine: Apple M3 Pro, 36 GB, macOS 26.6.2; fp32 ONNX on CPU
throughout, no Metal and no quantisation. Each arm re-embedded the whole store under the candidate,
ran the suite, then took seven cold `ask --stale --no-serve` on one Russian question. Raw output per
arm in `~/bench/embedders-2026-09-21/out/<label>.{txt,embed.txt,ask.txt,size.txt}`.

## The table

| hub id | params | dim | keyword | paraphrase | flips vs control | code | p90 tok | embed s | peak RSS GB | cold ask s | vectors MB | cache on disk | licence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `intfloat/multilingual-e5-small` (control) | 118M | 384 | 40/40 | 15/30 | — | 12/12 | 221 | 144 | 1.8 | 0.36 | 51 | 578M | MIT |
| `onnx-community/embeddinggemma-300m-ONNX` | 308M | 768 | 40/40 | **21/30** | +7 −1, p 0.07 | 12/12 | 221 | 733 | **1.3** | 0.64 | 103 | 1.2G | Gemma |
| `Snowflake/snowflake-arctic-embed-l-v2.0` | 568M | 1024 | 40/40 | **21/30** | +6 −0, p 0.03 | 12/12 | 224 | 1944 | 2.0 | 0.84 | 137 | 2.1G | Apache-2.0 |
| `BAAI/bge-m3` | 568M | 1024 | 40/40 | **21/30** | +7 −1, p 0.07 | 12/12 | 217 | 2059 | 2.0 | 0.91 | 137 | 2.1G | MIT |
| `onnx-community/Qwen3-Embedding-0.6B-ONNX` | 596M | 1024 | 40/40 | **21/30** | +6 −0, p 0.03 | 12/12 | 228 | 4153 | 3.1 | 1.66 | 137 | 2.2G | Apache-2.0 |
| `intfloat/multilingual-e5-base` | 278M | 768 | 40/40 | 17/30 | +4 −2, p 0.69 | 12/12 | 221 | 608 | 2.6 | 0.69 | 103 | 1.0G | MIT |
| `Teradata/granite-embedding-278m-multilingual` | 278M | 768 | 40/40 | 16/30 | +2 −1, p 1.00 | 12/12 | 216 | 548 | 2.9 | 0.73 | 103 | 1.0G | Apache-2.0 |
| `Teradata/granite-embedding-107m-multilingual` | 107M | 384 | 39/40 | 14/30 | +2 −3, p 1.00 | 12/12 | 227 | 97 | 1.7 | 0.39 | 51 | 424M | Apache-2.0 |
| `Snowflake/snowflake-arctic-embed-m-v2.0` | 305M | 768 | 40/40 | 14/30 | +1 −2, p 1.00 | 12/12 | 224 | 663 | 3.4 | 0.71 | 103 | 1.2G | Apache-2.0 |

*flips* are the paraphrase cases this model hits that the control misses and the ones it misses that
the control hits, with the exact two-sided McNemar p on that pair. *embed s* is the wall clock of one
whole `embed` over the 33,525 rows, model open included, at `REPOGRAPH_RESOURCES=full`. *peak RSS*
is `maximum resident set size` from `/usr/bin/time -l` on that embed, in decimal GB. *cold ask* is
the median of seven. *cache on disk* is `du -sh` of the model's directory under
`~/.cache/repograph/fastembed` after the run.

## What the readings say

**Four models tie at 21/30 against the control's 15/30.** embeddinggemma-300m, arctic-embed-l-v2.0,
bge-m3 and Qwen3-Embedding-0.6B all land on 21, and nothing in this document separates them on
recall. Three of the nine land below the control. e5-base's +4 −2 and granite-278m's +2 −1 are the
shape of noise, not of an upgrade.

**Paraphrase is the only column the embedder moves.** Keyword is 40/40 and code 12/12 for every
model but granite-107m, which drops one keyword case (`FR-WH-53`). Those two arms are saturated on
this corpus: keyword questions are answered by the lexical half of the index and code questions by
the graph, and a better embedder has nothing left to win there. p90 tokens moves 216–228 across the
whole field, which is the context budget doing its job rather than a difference between models.

**embeddinggemma-300m is the recommendation.** It reads the same 21/30 as the two 568M models at
733 s of embed against their 1944 s and 2059 s, and it has the lowest peak RSS of anything measured
here — 1.3 GB, under the 1.8 GB of the 384-dimension control it would replace. Its index is 103 MB
of vectors rather than the 137 MB the 1024-dimension models write, and its cold ask is 0.64 s
against 0.84 s and 0.91 s.

**Its licence is the one reason to refuse it.** embeddinggemma is published under the Gemma Terms of
Use, which is not an OSI-approved licence: it carries use restrictions and passes them on to
anything redistributing the weights. That is a decision for whoever ships, not a benchmark result,
and it is why the default is not being changed to it.

**The OSI alternative at the same recall is bge-m3**, MIT, 21/30, at 2059 s — 2.8× embeddinggemma's
embed and 14× the control's — with 137 MB of vectors and 0.91 s cold ask. A project that cannot take
the Gemma terms gives up the cost, not the recall.

**Qwen3-Embedding-0.6B buys nothing over embeddinggemma here.** Same 21/30, at 5.7× its embed
(4153 s against 733 s) and 2.6× its cold ask (1.66 s against 0.64 s), with the largest peak RSS of
the field. Its last-token pooling and instruction prefix work; they do not pay.

**Of the 30 paraphrase cases, 12 are hit by all nine models and 5 by none** (`FR-PAY-49`,
`FR-MKT-35`, `FR-RPT-13`, `FR-CAL-101`, `FR-MIG-17`). So the embedder decides about 13 cases, and
the whole 15 → 21 spread lives inside them. The five nobody finds are the more useful number: they
are where the next improvement is, and no model in this field is it.

## What this does not measure

1. **One run per model.** With 30 paraphrase cases and a single run, anything within about four hits
   of the control is noise. The four-way tie at 21/30 is not a ranking, and the order the rows appear
   in is the order of embed cost, not of quality. The exact McNemar p is printed because it is the
   right test for a paired flip count, not because 0.03 settles anything at n = 1.
2. **The only reproducibility check is the control.** e5-small was embedded twice from scratch and
   read 15/30 and p90 221 both times. Nothing else was repeated, so the run-to-run spread of the
   other eight is unknown and assumed to be the control's.
3. **The Qwen3 embed did not run as one process.** It was killed part-way at 23,488 of 33,525 rows,
   apparently under memory pressure from the rest of the machine, and resumed from its checkpoint.
   Its 4153 s is the sum of the two processes (2558 s + 1595 s) and its 3.1 GB is the higher of their
   two peaks. Its suite and ask rows were taken on the finished index and are ordinary readings.
4. **The suite ran off a copy of the cases.** Since 0.5.2 the built-in `bench` cannot run on the
   pinned fixture at all: the `staleClaims` case at `bench/cases.jsonl:82` expects
   `packages/task-sync/src/cli.ts`, which does not exist at `502e8a6d`, and the anchor check refuses
   the whole suite rather than that one case. Every arm here ran with `--cases` against a copy
   holding the previous anchor, `tools/tasks/src/cli.ts`; the control on that copy reproduced the
   historical figures exactly (40/40, 15/30, 12/12, p90 221). The broken anchor is tracked
   separately and was not touched for this run.
5. **One corpus, one language pair, one shape of question.** Russian prose over a TypeScript
   monorepo. A model that handles this pair well is not thereby better on an English-only or
   code-heavy repository, and multilingual capacity is most of what separates these candidates.
6. **No rerank and no hybrid weighting was re-tuned.** Every arm ran the same retrieval settings as
   the control, so these are readings of the embedder in the seat the current pipeline gives it.
7. **fp32 on CPU.** No quantised export was measured, and a quantised embeddinggemma or bge-m3 would
   change both the cost column and, unmeasured, the recall one.

## Changing the seat

`repograph model` prints what wrote the store, what is configured, and the table above in short
form; `repograph model <hub id>` opens the model first, writes `embed_model` into `repograph.toml`
only once it opens, and re-embeds. Changing the model drops every row the previous one wrote — a
change of dimension rewrites the index whole rather than extending it — so the switch costs the
embed column above, once.

## The widened paraphrase arm

The readings above were taken on 30 paraphrase cases, and four models tied on them at 21/30. That
tie was the arm's, not the models': the best stack this project has — an embedder plus a sonnet
rerank — already read 29 of those 30, so one hit was the whole remaining headroom.

So the arm was widened to 60. The 30 new cases were authored against the same pinned fixture and
admitted only by a mechanical gate: at most 0.34 of a question's content stems may also occur in
the target node's label and body, the anchor must resolve, and no second node may answer it. The
threshold is stricter than 6 of the 30 cases already recorded, so the new half is the harder half
by construction. Six cells ran on the widened suite, each re-embedded under its own model, rerank
on sonnet at depth 200:

| model | paraphrase, no rerank | paraphrase, + rerank |
| --- | --- | --- |
| `intfloat/multilingual-e5-small` | 24/60 | 48/60 |
| `onnx-community/embeddinggemma-300m-ONNX` | 35/60 | 50/60 |
| `Snowflake/snowflake-arctic-embed-l-v2.0` | **39/60** | **52/60** |

Two things read off that table. The embedders separate — 24, 35, 39 — where the 30-case arm called
three of them a tie, and `snowflake-arctic-embed-l-v2.0` is the widest of the three. And the rerank
compresses the spread back to 48, 50, 52: it recovers most of what a weaker embedder loses, which
is why an arm of 30 could not tell the two levers apart. Keyword falls 40/40 to 39/40 in every
rerank cell, the same case each time, and three paraphrase cases are missed by all six.

This is what moved the recommendation from `embeddinggemma-300m` to `snowflake-arctic-embed-l-v2.0`
— on recall, and with the Gemma terms no longer standing in the way of it. The cost columns above
are unchanged by any of this: arctic is still 2.7× embeddinggemma's embed and writes 137 MB of
vectors against its 103 MB, and the default is still the default.

The same caveats apply, and one more. One run per cell, so four hits are still noise; the widened
suite is not the graded one — `RECORDED_SHAPE` in `src/bench.rs` still names 40/30/12, so a
112-case file is measured and not graded — and the cases live in the bench kit rather than in
`bench/cases.jsonl`. The six cells are recorded in `bench/history/runs.jsonl` as `paraphrase60:*`.

## What the catalogue offers

`repograph model` lists two of the nine: the default and `snowflake-arctic-embed-l-v2.0`. The other
seven stay in this document, which is the record of what was measured rather than a menu. Removing
a model from the catalogue does not remove support for it: `embed_model` and `REPOGRAPH_EMBED_MODEL`
still take any hub id, and the per-model profiles — pooling, prefixes, decoder inputs — are all
still in `src/index/embed.rs`.
