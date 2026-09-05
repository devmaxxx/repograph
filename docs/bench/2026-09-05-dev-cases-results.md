# The developer-questions suite: first run and what it exposed

Sixty questions written the way a developer asks them while implementing — 18 to 43 words, product
and code vocabulary mixed, Russian and English, several right places per question — run against
the pinned fixture (`beauty-crm` at `502e8a6d`) on 2026-09-05, in both arms, at repograph `7d65aad`.
The recorded 82-case suite measured the same day, on the same binary, reads exactly what it read
before: `keyword 40/40 paraphrase 15/30 code 12/12 p90 220` with embeddings and `39/40 14/30 12/12
p90 215` without.

The file is `bench/dev-cases.jsonl`; `bench/history/README.md` says how it is run and recorded.
Three authors wrote twenty cases each from the corpus's documents and code, by domain, and were
not allowed to run the tool, so nothing in the file is tuned to what already retrieves. Anchors were
validated against the graph before the first question was asked.

## What it measures

A case's `expect` may be a list. `bench` reports how many of a case's anchors the answer reached
over how many it has; one reached is a **hit** — the developer has an entry point — and the share
is what the history keeps per case, so a question answered by one of its three places scores 0.33
there and reads as a miss in the chronic list. The five kinds:

| kind | n | what it asks | anchors |
|---|---|---|---|
| `long` | 15 | a full natural-language question about one requirement, phrased as the developer's own situation | one requirement or NFR |
| `cross` | 15 | a product rule and where it lives in code | a requirement and the file that implements it |
| `multi` | 12 | a feature whose honest answer is several requirements | 2–4 requirements |
| `where` | 9 | which file to edit, described in prose, no symbol names | 1–2 files |
| `rule` | 9 | is this allowed, what forbids it | an INV, ADR or NFR |

The suite has no floors. `bench` measures it, prints `gated=false`, and exits 0 whatever it reads;
the history records its runs under arms of their own (`bench[dev-cases]:dense+enriched`,
`bench[dev-cases]:lexical+enriched`).

## The first run

| arm | long | cross | multi | where | rule | hits | mean share | p90 |
|---|---|---|---|---|---|---|---|---|
| embeddings, enriched | 10/15 | 12/15 | 9/12 | **0/9** | 5/9 | 36/60 | 0.37 | 240 |
| `--no-dense`, enriched | 11/15 | 11/15 | 10/12 | **0/9** | 5/9 | 37/60 | — | 242 |

Both arms read the same holes. The p90 of 240–242 tokens is over the recorded suite's ceiling of
230 — a longer question meets more of the corpus and the answer's one-hop expansion follows —
and is reported, not graded.

## Where the misses sit

Every anchor was located in the four retriever lists 300 deep (`dump`), and each missed anchor
classed by its best rank: **reachable** (≤ 20 in some list — a fusion problem), **deep** (21–300 —
a ranking problem), **absent** (no list within 300 — coverage or vocabulary). Embeddings arm:

| kind | anchors reached | reachable | deep | absent |
|---|---|---|---|---|
| long | 10 | 4 | 1 | 0 |
| cross | 12 | 2 | 5 | 11 |
| multi | 12 | 22 | 5 | 0 |
| where | 0 | 0 | 8 | 5 |
| rule | 5 | 3 | 1 | 0 |

Three weak spots, in order of size.

**A. Code is unreachable from prose.** `where` reads 0/9, and 12 of its 13 file anchors are absent
from every lexical list within 300 (`--no-dense` arm); with embeddings the dense passage list holds
some of them at ranks 22–194, which is the model bridging Russian prose to an English identifier on
its own. The cause is in the store, not the retrievers: a symbol's indexed text was its id, its name
and its signature line; a file node had no text at all and was not indexed; and `enrich` generated
questions for documents only. Nothing in the index said what a piece of code *does*. The `cross`
kind shows the same hole from the other side — 12 of 15 questions reach their requirement, 11 of
their 15 file anchors are absent.

**B. Five seeds over three lists.** 31 missed anchors sit at rank ≤ 20 of some list (BM25 over the
generated questions 13, dense questions 10, BM25 passages 5, dense passages 3) and are not reached,
most of them `multi` siblings — `FR-STAFF-20/21/22`, `FR-PAY-22/25/26/27`, `FR-CAL-110/111/112` —
that a round-robin over three lists cannot seat: each list gets at most two of five seeds, so the
third and fourth right answer of one list never surfaces. The recorded suite's paraphrase misses
show the same shape: seven of fifteen sit at rank 3–9 of their best list. Widening the seed count
was measured and rejected in ADR-001 (each step costs ~1.5× tokens for about one hit); adding the
dense questions list as a fourth list was simulated offline over the dumps and rejected — paraphrase
15/30 at every gate value and position, one keyword lost, held-out 102 → 92–96.

**C. The embedder, for Russian paraphrase.** Eight of the recorded suite's fifteen paraphrase
misses sit at rank 31+ in every list although the store's generated questions for the target are
near-synonyms of the query — «Где лежит короткий код на устройстве?» for «как хранится код доступа
сотрудника» sits at dense-questions rank 209. That is `multilingual-e5-small` placing two Russian
paraphrases far apart, which no fusion rule repairs.

## What was tried against them

Rules were written before each measurement; the numbers are in the sections that follow as they
were read.

**A1 — code carries its prose (no tokens).** A symbol's body becomes its doc comment (contiguous,
capped at 600 characters) followed by its signature; a file's body becomes its head comment (capped
at 1,200); files with a head are indexed like any node; identifiers are indexed whole and by their
camel-case words (`revokeAllSessions` is also `revoke`, `all`, `sessions`). On this corpus that gives
546 files a text and 1,530 symbols a doc comment. Ships only if the recorded suite holds every floor
in all four arms with p90 ≤ 230, held-out is not significantly worse in either arm, and the dev
suite's hits rise in both arms with `where` above zero.

**A2 — questions about code (tokens).** `enrich --code` asks the model about symbols with a doc
comment or a body and files with a head — 3,475 nodes here — through a prompt that forbids repeating
the identifier and asks for four Russian and four English questions per node. Document coverage,
which decides the `enriched` floors, is unchanged; the summary line reports `code_questions=` beside
it. Same rule as A1, read against A1's store.

**A3 and A4 — the same questions, searched somewhere else (no new tokens).** After A2 the code
questions move out of the documents' questions index into an index of their own, joining the plain
fusion as a third list admitted last on the same 0.85 gate (A3), and then capped at a single seat
(A4). Same rule, same store, no second `enrich`.

**C1 — a larger embedder.** `multilingual-e5-large` over the same rows on a copy of the store, dense
arms only, same rule.

**D1 — a stronger generator (tokens).** The documents' questions regenerated by sonnet rather than
haiku on a copy of the store, with nothing in the retrievers changed. Same rule, both arms.

## Results

### A1 — rejected whole, decomposed, reshaped

Whole A1 on a copy of the corpus at `502e8a6d` — bodies indexed, files indexed, identifiers split:

| arm | recorded suite | dev suite | held-out (400) |
|---|---|---|---|
| embeddings, enriched | 40/13/12 — under the paraphrase floor of 14 | 34/60, `where` 1/9, `rule` 5 → 3 | unchanged, p = 1.0 |
| `--no-dense`, enriched | 39/12/12 p90 219 | 34/60, `where` 2/9, `rule` 5 → 3 | 109 → 102 (+1 −8), p = 0.039 |
| raw, either arm | 40/9/12 p90 227 · 39/7/12 p90 227 | — | — |

Rejected by its rule. Per case: the recorded suite loses paraphrase `FR-DM-66` and `FR-SVC-39` in
both arms (`FR-CRM-11` too without embeddings, where `N-109` is gained); the dev suite loses
`rule/ADR-005` and `rule/ADR-031` in both arms — code doc comments that cite the ADRs now outrank
the ADRs — and gains two `where` files (`availability-snapshot.service.ts`; `auth.service.ts` +
`identity.repository.ts` at 0.5). So the bodies do make files reachable, and do take seats from
documents on document questions.

Decomposed before being discarded. **X**, the bodies and the indexed files under the old
tokenizer: embeddings 40/13/12 p90 224, `--no-dense` 39/12/12 p90 227 — the paraphrase cost is the
bodies. **Y**, identifier splitting alone on the fixture: embeddings 40/14/12 p90 221 (one
paraphrase lost, the floor met exactly), `--no-dense` 39/14/12 p90 217; dev 35/60 and 36/60 with
`where` still 0/9 — splitting buys nothing without the bodies and costs a case. Neither passes
alone.

Why the bodies cost paraphrase: over dumps of the A1 store (`code-list-sim.py`), dropping every
code row from every retriever list still leaves the recorded paraphrases at 13/30. The
competition is not for seats. The 4,851 code documents lengthened by their comments moved the
BM25 statistics — average length, idf — that the requirement documents are scored against. A
code list of its own, admitted on a confidence gate like the questions list, seats two `where`
files at γ = 1.0 and nothing else, five at γ = 0.8 for two keyword cases lost.

What shipped instead (`c3aef20`): the bodies stay extracted — doc comment ahead of the
signature, head comments for files — for the enrichment prompt and for display; the passage index
carries a symbol's declaring line only and no file; identifiers stay whole. The copy then reads
exactly the fixture: 40/15/12 p90 220 and 39/14/12 p90 215, dev 36/60 and 37/60 with `where`
0/9. The prose about code reaches retrieval through A2's questions, in the reranked pool, instead
of through the passages — which is A4's ruling below, not A2's own.

### A2 — two defects found before a number was read, then the number

The first `enrich --code` run was stopped after 12 batches: six had come back with every node
"skipped". The captured answers showed the model replying with the entry's label —
`AvailabilityService`, `index.ts` — where the parser wanted the full
`sym:apps/api/src/…::AvailabilityService` id; document ids are short and were always copied.
Entries now carry a `c<n>` key and the parser accepts the key or the id, never the label (an
`index.ts` is ambiguous inside one batch). Verified on three batches: 36 of 36 written, eight
questions per node at the median.

The second: both question indexes dropped File nodes outright — the passage-side ruling had been
applied to both — so the 546 files' questions would never have been searchable; a `--limit 36`
run wrote 36 `file:` entries, first in id order, and embedded 0 rows. A file is a passage nowhere
but carries questions like any other node once it has them; which index searches them is what A2,
A3 and A4 below decide, and a store without code questions is the old index byte for byte
(`c11dcbe`).

The rule is A1's, read against A1's store: every recorded floor held in all four arms with p90 ≤
230, held-out not significantly worse in either arm, dev-suite hits up in both arms with `where`
above zero. Measured on the copy with `code_questions=3463/3475` (run transcripts `a2-enr-*.txt`,
held-out dumps `ho-a2-*.json`, chain log `a2-chain.log`):

| arm | recorded suite | dev suite | held-out (400) |
|---|---|---|---|
| embeddings, enriched | 40/15/12 p90 221 | 40/60, `where` 1/9 | 103 → 102 (+9 −10), p = 1.0 |
| `--no-dense`, enriched | **38**/15/12 p90 220 — under the keyword floor of 39 | 39/60, `where` 1/9 | 109 → 113 (+13 −9), p = 0.52 |

Rejected by rule (i), on one case: the `--no-dense` arm loses the keyword case `FR-WH-53` and gains
the paraphrase `FR-PAY-104` (14/30 → 15/30), while the arm with embeddings moves no recorded case
at all. The dev suite moved in both arms and in the direction the lever was for — 36 → 40 with
embeddings, 37 → 39 without — gaining the same three cases in each: the `where` file
`packages/domain/src/schedule/subjectAvailability.ts`, the `rule` document `ADR-003`, and the
`cross` case `FR-LIFE-05+apps/api/src/shared/db/terms-gate.ts`. `multi` is where the two arms part,
the dense arm gaining `FR-SHELL-70+…` and the lexical arm losing `FR-MKT-21+…`. The floor is what
it cost.

The mechanism is BM25's length normalisation, not the fusion. Until A2 a code node was an id-only
document in the documents' questions index; the 3,463 nodes that gained questions turned those
one-token documents into question-length ones and lifted the index's average length, so every
questions score rose by about a quarter — `N-071` 9.01 → 11.57 — while the passage scores stayed
where they were. The gate that admits the questions list divides one index's best score by the
other's, so `FR-WH-53`'s ratio moved 0.80 → 1.03 and the questions list was admitted over the
passage that held the answer. Nothing about the query, the answer or the fusion changed; a
population change in one index moved the constant. That is gap G12.

### A3 — the code questions as a list of their own

Same rule. The code questions leave the documents' index for one of their own and join the plain
fusion as a third list, admitted last on the same 0.85 gate (`a3-enr-*.txt`, `ho-a3-*.json`):

| arm | recorded suite | dev suite | held-out (400) |
|---|---|---|---|
| embeddings, enriched | 40/15/12 p90 219 | 38/60, `where` 2/9 | 103 → 97 (0 gained, 6 lost), p = 0.031 |
| `--no-dense`, enriched | **38**/13/12 p90 220 — under the keyword floor of 39 | 39/60, `where` 2/9 | 109 → 102 (0 gained, 7 lost), p = 0.016 |

Rejected by rules (i) and (ii). Moving the questions out of the documents' index puts that index's
statistics back and the A2 mechanism with them; what is left is the seat. The `--no-dense` arm
loses the keyword case `FR-WH-53` and the paraphrase `FR-CRM-11` — 13/30 is still above that arm's
paraphrase floor of 11, so the floor that fails is keyword — and the held-out set loses six
questions with embeddings and seven without, gaining none in either arm.

### A4 — one seat

Same rule, with the code list capped at a single seat on the plain path (`7b94f4c`; `a4-enr-*.txt`,
`ho-a4-*.json`):

| arm | recorded suite | dev suite | held-out (400) |
|---|---|---|---|
| embeddings, enriched | 40/15/12 p90 219 — no case moved | 38/60, `where` 2/9 | 103 → 97 (0 gained, 6 lost), p = 0.031 |
| `--no-dense`, enriched | 39/13/12 p90 220 — every floor held | 39/60, `where` 2/9 | 109 → 103 (0 gained, 6 lost), p = 0.031 |

One seat gives the keyword case back — the `--no-dense` arm reads 39/40 again and holds every
floor, with `FR-CRM-11` still lost against the shipped store. Both arms gain the same two `where`
files, `packages/domain/src/schedule/subjectAvailability.ts` and
`packages/db/src/schema/salon/catalog.ts`, and the dense arm drops a row of the `multi` case
`FR-CAL-91+…+FR-CAL-99` (2/4 → 1/4). So A4 passes rules (i) and (iii) and fails rule (ii): six
held-out questions lost in each arm, none gained, p = 0.031.

That is the finding of this campaign, and it is about seats rather than about code. Five seeds are
the budget the recorded floors were set on, and a list that earns its turn displaces a document
seed — two of nine `where` questions bought with six of four hundred document questions, in each
arm. What shipped instead (`32c047b`, `fb369ca`): the code questions live in an index of their own,
the plain `ask` fusion is byte for byte the pre-A2 one, and the code list enters only the reranked
pool, where the candidates are `--depth` deep rather than five seats and what the list is worth to
the reranking model is unmeasured. `where` reads 0/9 in the shipped configuration.

### C1 — the larger embedder

The same rule, dense arms only and its third clause read as the developer suite not being down —
`where` is a code question and the embedder is not a code lever. On a copy of the fixture store,
re-embedded under `intfloat/multilingual-e5-large` (`c1-enr-*.txt`, `ho-c1-dense.json`):

| arm | recorded suite | dev suite | held-out (400) |
|---|---|---|---|
| embeddings, enriched, e5-large | 40/22/12 p90 224 | 37/60, `where` 0/9 | 103 → 119 (+19 −3), p = 0.0009 |

Passes on all three. Paraphrase 15/30 → 22/30 is seven of the fifteen misses closed — weak spot C,
and the only lever here that moved it — with keyword and code unchanged and the dev suite up one
hit. It closes nothing for `--no-dense`, which has no dense list to improve, and it does not touch
`where`.

The cost is in the README's [Embeddings](../../README.md#embeddings) section, measured there and
not re-measured here: an `ask` at 0.8 s against 0.55 s with the model opening in 676 ms against
418, 1.9 GB resident against 1.7, 2,680 s to embed the corpus's 33,525 rows against ~103 s, and a
2.1 GB download. The rows themselves are 1024 floats instead of 384, so the store's `vectors.f32`
is 137.3 MB against the fixture's 51.5 MB (`ls -l`, decimal megabytes).

Shipped as a property of the store (`9f83517`, `a34214f`, `8142ebd`): `embed_model` in
`repograph.toml` names what writers write with, `vectors.json` records it, readers open the model
the store records, `bench` and `dump` refuse a width mismatch and `ask` warns and answers
lexical-only. The default stays `intfloat/multilingual-e5-small` and the floors stay the small
model's, so a store on the larger model is measured against them rather than graded by them.
Flipping the default is a decision, not a measurement.

### D1 — a stronger generator

The documents' questions regenerated by `claude -p --model sonnet` on a copy of the store, nothing
in `src/` changed, the same three-way rule in both arms with its third clause again read as the
developer suite not being down (`d1-enr-*.txt`, `ho-d1-*.json`, `d1-enrich.log`). The pass wrote
1,996 nodes with 0 dropped in 167 batches and 1,283 s — 27,394
questions at a median of 13 a node, against the haiku store's 26,117 at the same median — and
embedded 27,375 rows in 62.4 s. Cost ≈ $13.62, an estimate from the tee'd prompt and answer bytes
÷ 4 at sonnet's list price against ≈ $2.50 on haiku; the true figure is higher, because the text is
mostly Russian and a Cyrillic character costs more than a quarter of a token.

| arm | recorded suite | dev suite | held-out (400) |
|---|---|---|---|
| embeddings, enriched, sonnet | 40/17/12 p90 228 | 37/60, `where` 0/9, `rule` **2/9** | 103 → 132 (+46 −17), p = 0.0003 |
| `--no-dense`, enriched, sonnet | 39/16/12 p90 222 | **36**/60, `where` 0/9, `rule` **2/9** | 109 → 128 (+45 −26), p = 0.032 |

Rules (i) and (ii) hold — every floor, and held-out significantly *better* in both arms, the
largest move any lever in this campaign made. Rule (iii) fails: the dev suite reads 37 → 36 in the
`--no-dense` arm (36 → 37 with embeddings), and the loss is one kind. `rule` goes 5/9 → 2/9 in both
arms — `ADR-005`, `ADR-031`, `INV-07` and `INV-10` lost, `ADR-003` gained — while `long` gains
`FR-STAFF-02` and `FR-OPS-05` and `cross` gains `FR-LIFE-05`. On the recorded suite the dense arm
gains `FR-VIS-01`, `FR-AI-21`, `FR-STAFF-45`, `FR-CRM-11` and `N-109` and loses `FR-SVC-39`,
`FR-DM-66` and `ADR-005`; the lexical arm gains `FR-PH-43` (keyword), `FR-VIS-01`, `FR-PAY-03`,
`FR-AI-21`, `FR-STAFF-45`, `FR-CAL-101` and `N-109` and loses `FR-WH-53` (keyword), `FR-SVC-39`,
`FR-CAL-06`, `FR-DM-66` and `ADR-005`.

Not shipped as the default. The rule was written before the numbers and is not re-read to fit them,
and a lever that trades four is-this-allowed answers for paraphrase is the trade the developer
suite exists to catch; `ENRICH_COMMAND` stays haiku. Nothing about it is out of reach: `enrich_command`
is per-store configuration, so a store can choose sonnet today in one line of `repograph.toml`, at
roughly five times the token cost and with `rule` as the thing to watch. One prompt is written for
every kind of document, which is the gap this opens — G14.

## What ships and what stays open

Shipped from this campaign, in order: `c3aef20` — a symbol carries its doc comment and a file its
head comment, for the enrichment prompt and for display, while the passage index keeps a symbol's
declaring line and no file at all; `c11dcbe` — `enrich --code`, questions about code behind a flag,
with the documents' coverage and floors untouched; `b438249` — `repograph embed`, which re-embeds a
store copy under another model; `32c047b` and `fb369ca` — the code questions in an index of their
own, reaching the reranked pool and never the plain fusion; `9f83517`, `a34214f` and `8142ebd` —
the embedding model as a property of the store. The test suite reads 410 tests at `8142ebd`. No
lever shipped as itself: A1, A2, A3, A4 and D1 were each measured and ruled against, C1 ships as an
option and not as the default, and what stands above is the machinery they left behind.

The three weak spots, as they stand:

**A. Code is unreachable from prose.** Unmoved in what ships: `where` is 0/9 in the plain fusion,
and code is reachable only through `ask --rerank`, whose pool now carries the code questions. Each
lever that lifted `where` — A2 to 1/9, A3 and A4 to 2/9 — paid for it in document answers, by a
floor (A2, A3) or by the held-out set (A3, A4). What could pay for itself is an admission rule that
does not compare raw scores across two indices: G12.

**B. Five seeds over three lists.** Unchanged, and now named as the constraint rather than
suspected: A4 prices one seat for a fourth list at six held-out questions in each arm, none gained,
p = 0.031. G10 is closed as measured, not as fixed.

**C. The embedder, for Russian paraphrase.** Closed for the dense arm, at its cost and as an
option: e5-large reads paraphrase 22/30 against 15/30 and held-out 103 → 119, and ships as a store
property whose default is unchanged. The shipped default still reads paraphrase 15/30 with
embeddings and 14/30 without — exactly the first run. Open for `--no-dense`, which has no dense list.

Three gaps leave this campaign, all in [`next-version-gaps.md`](next-version-gaps.md). **G12**: the
gate divides one index's best BM25 score by another's, so a population change in either index moves
it while nothing about the query changes — A2 is the proof, `FR-WH-53` at 0.80 → 1.03. **G13**: ten
`where` file anchors sit at rank 1–9 of the code list and six of them are below the 0.85 gate, so
those answers are ranked and never seated. **G14**: one enrichment prompt is written for every kind
of document, and D1 prices that — sonnet's questions lift paraphrase and held-out in both arms and
lose four `rule` answers about ADR and INV documents.
