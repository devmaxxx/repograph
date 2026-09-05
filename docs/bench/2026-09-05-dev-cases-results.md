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

**C1 — a larger embedder.** `multilingual-e5-large` over the same rows on a copy of the store, dense
arms only, same rule.

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
0/9. The prose about code reaches retrieval through A2's questions instead of through the
passages.

### A2 — two defects found before a number was read

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
but joins both questions lists once it carries questions; a store without code questions is the
old index byte for byte (`c11dcbe`).
