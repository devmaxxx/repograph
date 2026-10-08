# Benchmarks

## Bench

`repograph bench [--cases file]` runs the recorded 82 cases (40 keyword + 30 paraphrase + 12 code)
against a built graph and fails the process if any floor is missed. The recorded `bench/cases.jsonl`
is compiled into the binary, so a release build benches from any directory; `--cases` substitutes
any other file — one of the recorded 40/30/12 shape is graded against the floors below, any other
shape is measured and reported with `gated=false`, the way `bench --cases bench/dev-cases.jsonl` is
used throughout [the runbook](bench/runbook.md).

Twelve of the cases expect a path, and a path is a fact about one checkout, so the recorded suite
names the corpus it was written against: `bench/cases.pin` holds `beauty-crm 502e8a6d`, and a run
prints it above the cases (`suite: built-in bench/cases.jsonl, recorded against beauty-crm
502e8a6d`). The floors below are counts on that tree, and they grade that tree only: a checkout
at any other commit is measured, printed and not graded (`suite: checkout a3bf96ff is not the pin,
so this run is measured and not graded`, `gated=false`, exit 0), because beauty-crm `a3bf96ff`
reads paraphrase 12/30 against the pin's 15 after 203 documents changed, and that is the corpus
growing, not repograph failing. A tree whose commit git cannot read is graded as before. An anchor the graph does not hold stops the
run and the refusal quotes the pin, because editing the case to a newer path is what silently
moves the suite off the tree its floors were counted on; `bench/missing-anchors.py <checkout>`
lists every path anchor a tree is missing, without a build and without CI. The one exception is a
`code` case whose symbol moved: its question is the symbol's name, so when exactly one file defines
that name now, the anchor follows it and the run says so (`suite: "staleClaims" moved:
tools/tasks/src/cli.ts -> packages/task-sync/src/cli.ts`). An id anchor is graded on the node it
names, never on another node declared in the same document: `ADR-024` counts when the ADR's own
node is reached, not when `entity:StaffMember.role_id`, which the ADR defines, is.

The floors are two sets, not one, because [`enrich`](enrich-and-rerank.md) is optional and
paraphrase recall is what it buys. `bench` reads which state the store is in and says so on its
summary line (`dense=true  enriched=true (1996/1996 nodes) model=gemma`): a store carrying questions
on at least 99% of its requirement-like nodes is graded against the enriched floors, anything else
against the raw ones. The bar is a high-water mark rather than every node because equality over
~2,000 nodes is a cliff — one node the model skipped would regrade a paid-for store five paraphrase
points lower, and `bench` would say so through its exit code alone. Questions written for another
language list than the `enrich_languages` the configuration names count for nothing: `enrich` would
rewrite them, and a store whose ADRs carry English questions only is not enriched for a reader who
asks in Russian, so `bench` says so above the cases and grades it `enriched=false`.

The dense floors are keyed by the store's embedder too, since a floor measured on one model says
nothing about another: small-model rows (or rows under no name) are graded against the small model's
numbers, rows under the default model (`embeddinggemma-300m`, printed `model=gemma`) against its own
enriched row, and rows under any other model are measured and never graded. The default model's raw
dense arm (a store with no questions) was never measured, so it has no row and is not graded. The lexical arms
have no embedder in them and keep one set whatever the rows are. The summary line also carries
`code_questions=<covered>/<eligible>` on a store that carries questions about code, which are
searched for the `--rerank` pool rather than in the fusion the floors measure. Beneath it, on a line
of its own, `anchors  <kind> <reached>/<wanted> …` says how much of each answer was reached, not only
whether it was: a case that keeps its verdict and loses two of its three anchors moves that line and
nothing else. `bench --repeat N` runs the suite N times, judges every run on the floors, and prints
a median beneath them.

The exit status says which of three things happened, so a harness never has to read the sentence on
stderr: **0** — the suite answered and every floor was met; **3** — the suite answered and a floor
was missed, which is a verdict on the answers and a reading of the reader; **1** — nothing was
measured, which is an empty graph, a case file that does not parse, a built-in case set of the wrong
shape, or a dense width mismatch. The verdict is **3** and not 2 because 2 is written above the
command and says nothing about a suite: `clap` exits 2 on a usage error, and `npm`'s launcher exits
2 when no platform binary is installed. A `--repeat` run takes the worst of its runs: every run has
to meet the floors, not the median of them.

| | enriched store | store with no questions |
| --- | --- | --- |
| keyword | 40/40 with embeddings, 39/40 with `--no-dense` | 40/40 with embeddings, 39/40 with `--no-dense` |
| paraphrase, gemma rows (the default) | ≥19/30 with embeddings | not graded (never measured) |
| paraphrase, small-model rows | ≥14/30 with embeddings | ≥9/30 with embeddings |
| paraphrase, `--no-dense` (no embedder) | ≥11/30 | ≥7/30 |
| code | 12/12 | 12/12 |
| p90 | ≤250 tokens in every arm | ≤250 tokens in every arm |

The `--no-dense` column applies to every store, since no embedder is in it.

Where each of those floors came from, why keyword is 39 and not 40 in the lexical arms, how the p90
is counted, and the 400-question held-out set a retrieval change has to clear before the 82 cases
are consulted, are in [the measurements](history.md#the-bench-floors-case-by-case).

## Measured

The bench fixture is 908 files and about 8.3k nodes. At 0.6.0, under the default embedder, an
enriched store reads **keyword 40/40, paraphrase 19/30, code 12/12 at 234 p90 tokens** with
embeddings and **39/40, 11/30, 12/12 at 238** with `--no-dense`. The readings below were taken
under the small model that preceded the default. On the recorded 82 cases, both arms run twice
with identical results: **keyword 40/40, paraphrase 15/30, code 12/12 at 220 p90 tokens** with
embeddings, and **39/40, 14/30, 12/12 at 215 p90** with `--no-dense`, both green. Those are the
numbers with `enrich`'s generated questions in the store — the one thing paid for, roughly $2.5 of
haiku, once. The same corpus at zero tokens throughout reads 40/40, 9/30, 12/12 at 221 and 39/40,
7/30, 12/12 at 226; [Bench](benchmarks.md#bench) floors each state on its own numbers.

The graph itself costs nothing to build: 7,525 nodes and 27,412 edges on the corpus of 2026-09-02,
10.2 MB on disk, ~19 ms to load, zero model tokens. The full snapshot, the prior art it replaced and
what that cost are in [the measurements](history.md#the-corpus-as-it-was-measured-and-the-prior-art).
