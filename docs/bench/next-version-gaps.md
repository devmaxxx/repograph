# What the 2026-09-03 numbers say is weak

Every item below is a gap the [three-graph run](2026-09-03-three-graphs-results.md)
measured on repograph 0.4.0, not a wish. Each carries the number that exposed it, the
diagnostic that would confirm the cause, the lever, and the gate that would tell us
it worked. They are ordered by how much of a real answer is missing, not by effort.

Two of them are the language work already scheduled for 0.6.0; what this file adds is
that the bench says they are not a nice-to-have.

Each heading now carries a status line from the 2026-09-04 run
([`2026-09-04-repograph-0.5.0-results.md`](2026-09-04-repograph-0.5.0-results.md)). Three
closed, one was measured and rejected, and two are closed on one half and deferred on the
other — a gap is not closed because a task ran against it.

G7 is the exception to both paragraphs above: it was raised after the 2026-09-04 run rather than
by it, by re-measuring the enriched store once `bench` began grading stores by state, so it
carries a raised line where the others carry a status one.

---

## G1 · A third of a large blast radius is invisible — `changes` 27/38 symbols

**Status (2026-09-04):** the file half is **closed by Task 2, against a narrow
denominator** — `changes` names 498/498 files over 8 diffs, and the case that read 11/18
files reads 25/25, because a hunk in a file the graph never indexed is now printed as that
file rather than dropped. The denominator is `code_files`, the files the truth harness
attributed a declaration to, not the files the diff touched: on `bc9db289~1` 119 files carry
hunks and 18 are counted, the excluded 101 including fifteen TypeScript files and nine of
sixteen Kotlin ones — the very files this gap is about. 498/498 is therefore evidence the
tool stopped dropping the files it is asked about, not a measurement that it names every
changed file.

The symbol half is **baseline written; the extractor is 0.6.0's** — 57 of the 61 symbols
still unnamed are Kotlin declarations, and the remaining 4 are one truth-side
mis-attribution, not a repograph miss. Numbers and the bucketing:
[`2026-09-04-repograph-0.5.0-results.md`](2026-09-04-repograph-0.5.0-results.md). The
gate's "≥34/38 symbols on the 18-file case" cannot be checked as written: that case's
want-total grew with the corpus, so the run reads 48/54 at a different HEAD.

**Measured.** `changes` scored 32/43 symbols and 13/20 files overall, and the split is
the whole story: **5/5 symbols on a 2-file diff, 27/38 on an 18-file one.** Seven of
the twenty files were never named at all.

**Cause, and it is not ranking.** `code_globs = ["**/*.ts", "**/*.tsx"]`
(`src/config.rs:31`). `changes::touched` intersects hunk line ranges with `Symbol`
nodes; a file with no symbols falls back to its file node, and a file with no node at
all is invisible. `beauty-crm`'s large diffs carry `.kt`, `.sql`, `.json`, `.yaml`
and `.gradle.kts`, and its 18-file case is exactly that mix. A tool whose answer to
"what does this change touch" silently omits a third of the change is worse than one
that says it does not know.

**Diagnostic before the lever.** Re-run the `0f27d2d1` case with the per-file list
printed, and bucket the eleven missing symbols by extension. If they are all
non-TypeScript, this is G1 and nothing else; if some are `.ts`, a second cause is
hiding behind the first and it belongs in G3.

**Lever.** The 0.6.0 language work — Kotlin, C#, Python — plus a decision this run
makes urgent: **a file the extractor cannot parse still deserves a file node**, so
`touched` reports it rather than dropping it. That is cheap, it is language-independent,
and it converts a silent omission into an honest "this file changed, I cannot tell you
which symbol".

**Gate.** The 18-file case reaches ≥34/38 symbols and 18/20 files with the unparsed
files reported as files. Add a third `changes` case whose diff is deliberately
polyglot, so the suite keeps measuring this after the fix.

---

## G2 · Fourteen paraphrases, twelve of them a neighbour away — strict 16/30, soft 28/30

**Status (2026-09-04):** **measured and rejected, not closed** — Task 6 ran the local
cross-encoder this gap named as the one unmeasured lever.
[ADR-001 Amendment 4](../adr/ADR-001-paraphrase-recall-was-a-prediction.md) is the
authority: `--rerank-local` at depth 200 reads paraphrase 17/30 against a control of
15/30, but keyword 39/40 and 17.9 s a question against a bar of one second. Two of the
five conditions fail, so no floor moved and the flag ships opt-in. The gap stays open;
what closes is only the question of whether that lever was worth trying.

**Measured.** Strict 16/30, soft 28/30. Twelve of the fourteen misses opened the right
file and named a neighbouring requirement; only two (`FR-AI-102`
«сколько времени держим записи кто решает», `FR-CAL-101`
«как отдаём освободившееся окно тому, кто ждёт») failed to surface the file at all.

**What is already exhausted.** [ADR-001](../adr/ADR-001-paraphrase-recall-was-a-prediction.md)
and its three amendments measured and rejected: more seeds (one hit per 1.5× tokens),
a wider one-hop cap (one hit, p90 218→510), `MultilingualE5Base` (6/14, a *different*
six), 512-token passages (5/14 at 2× embedding time), label-only second vectors
(6/14), `BGEM3` (5/14 at 1 454 s), quantized paraphrase MiniLM (2/14, keyword down to
21/24), doc2query rows pooled with passages (4/14), and query rewriting (paraphrase
6/14, keyword down to 20/24). None is adopted. Six of the eight misses on the old set
were shared by every E5 variant, which points at the model's distance between question
and target, not at the index.

**What works and what it costs.** `ask --rerank` with candidate snippets, a pool of
200 and no pinned seeds reads 14/14 paraphrase, 24/24 keyword, 3/3 code on three
consecutive sonnet runs — at ≈19k input tokens and ~4.3 s per question. It is opt-in
and deliberately not on a floor.

**The one lever never measured.** A local cross-encoder — `bge-reranker-v2-m3`, a
2.3 GB download — is named in ADR-001 as unmeasured. It is the only candidate that
could give reranker-shaped gains at zero API cost and without a per-question latency
in seconds. **Measure it before designing anything else for paraphrases.** If it lands
between 10/30 and 14/30 at sub-second latency, the paraphrase floors move for the
first time since 0.4.0; if it does not, the honest answer is that this corpus's
paraphrases need a model and the floors stay where the measurement put them.

**The two soft misses are a different bug.** They are coverage, not ranking: the
target file never entered the pool. Diagnose them with
`repograph dump --queries` and read the four retriever lists 300 deep. If the target
is absent from all four, no reranker will ever help, and the question is what the
extractor did with that requirement.

**Gate.** The 400 held-out question set with a paired exact McNemar test against the
shipped rule — the existing bar — before the 82 real cases are consulted as the smoke
test they are. Do not move a floor on 30 noisy cases alone: Wilson 95% on 16/30 is
0.35–0.69.

---

## G3 · Three hub targets short — `impact` 110/114

**Status (2026-09-04):** **closed by Tasks 3 and 4** — 123/123 files over 16 targets,
mean recall 1.0, every target at 1.0. The gate asked for 114/114 or a named exclusion;
what the diagnostic found is that the answer is both, in different files. Three of the
114 were a symbol name inside a string literal — never a reference, so Task 3 removed
them from the truth rather than excluding them in prose, which takes the original ten
targets to 111/111. The fourth was real: `AuthService`'s tenth file imported it through
a barrel, and Task 4 made a re-exporting barrel an importer.

**Measured.** Mean recall 0.949. Every narrow target is 1.0; the misses are all in the
tiers an agent trusts most — `TenantContextInterceptor` 3/4, `OutboxPublisher` 6/7,
`AuthService` 9/10.

**Candidate cause, already documented.** The README names four constructs that produce
no `Calls` edge: a chained expression, a destructured method, a callback parameter, a
global. Four missing files out of 114 is small enough that a per-case read will name
the construct exactly rather than leaving it to a guess.

**Diagnostic.** For each of the three targets, diff the truth file list against the
answer and open the missing file. One of three verdicts: a construct on the
documented list (then the lever is that construct's extractor), a barrel path
`impact` should have resolved (then it is a bug), or a reference in a string/template
(then it is out of scope and the caveat should say so by name).

**Why it is above its size.** `impact` is the command whose answer gets acted on
destructively — someone deletes or renames on the strength of it. The README already
says to confirm a "nothing uses this" with `rg -l`; a measured 0.949 is the number
that decides whether that sentence is a caveat or a defect.

**Gate.** 114/114 with the identified construct handled, or a named, measured
exclusion in the README's blast-radius caveats. No silent 0.949.

---

## G4 · The blast suite has 20 cases and no confidence story

**Status (2026-09-04):** **closed by Task 5** — `blast.jsonl` is 32 cases: `impact` 10
→ 16 (six narrow targets, the tier a mean over hubs hides), `changes` 2 → 8 diffs
spanning small/large and mono/polyglot, `trace` unchanged at 8. The decision rule this
gap asked for is written down in
[`three-graphs.md`](three-graphs.md#when-a-blast-delta-counts), per suite, with a noise
column — stated before the changes above were measured against it.

**Measured.** One run per row, no repeats, on 10 + 8 + 2 cases. Retrieval has a real
methodology behind it — 400 held-out questions, recall@5 with a ±5-point interval, a
paired McNemar test. Blast has nothing equivalent, and `changes` rests on **two**
cases, one of which carries most of the signal.

**Why it matters now.** G1 and G3 both propose changes that will be judged on this
suite. A suite that cannot distinguish a real gain from noise will approve whatever is
tried first.

**Lever.** Grow `blast.jsonl` where it is thinnest: `changes` from 2 to at least 8
diffs spanning small/large and mono/polyglot, and `impact` with more narrow targets,
which are the tier a mean over ten hubs hides. Then state a per-suite decision rule the
way `bench` states its floors, so "did this help" has an answer before the change is
written.

**Gate.** A documented rule in `three-graphs.md` for when a blast delta counts, and a
`changes` set large enough that one diff cannot carry the verdict.

---

## G5 · Strict is a substring, so ranking is unmeasured

**Status (2026-09-04):** **closed by Task 1** — every retrieval row carries `rank`, the
summary carries `mrr`, and this run measures **0.635** overall: keyword 0.816, code
1.000, paraphrase **0.247** with 2 of 30 at rank 1. One clause of the gate is unmet and
unmeetable: the 2026-09-03 rows cannot be rescored, because the harness stored counts
and never the answer text, so the field starts here with no prior value. The paraphrase
row also answers the question this gap was raised to serve — reordering cannot produce
the 14 ids that are absent, and finding them would not by itself put them first.

**Measured.** The protocol says it outright: an id ranked fifth of five counts the
same as first. So 68/82 strict says nothing about whether the right answer is at the
top of the answer or at the bottom of it.

**Why it matters.** It is the difference between "the reranker is necessary" and "the
answer only needs reordering" — G2's whole question. It also affects an agent
directly: an agent reading an answer acts on the first plausible id.

**Lever.** Record the rank of the expected id per row, and report MRR beside strict.
The rows already exist; this is a scoring change in `report.py`, not new cases.

**Gate.** Rank recorded on every retrieval row of the next result file, MRR in the
summary, and the 2026-09-03 rows rescored so the two runs are comparable.

---

## G6 · One corpus, one language, one id census

**Status (2026-09-04):** **baseline written; the extractor is 0.6.0's, and so are two
thirds of this gate.** `bench/corpora/beauty-crm-mobile/blast.jsonl` holds eight Kotlin
`impact` targets and
[`../../bench/results/2026-09-04-beauty-crm-mobile.json`](../../bench/results/2026-09-04-beauty-crm-mobile.json)
records what they read today: recall 0.0, 0/20 files, every case wanting at least one
file. Zero is the intended number — there is no Kotlin extractor yet — and writing it
down before the work is the whole point. The gate is **not** met: it asks for one
`impact`, one `trace` **and** one `changes` case per corpus, and only the `impact` third
was built; the retrieval half was not attempted either, because it needs that corpus's
own `id_families` census. Second corpus, second language, still one id census.

**Measured, structurally.** All 102 cases are `beauty-crm`. All 30 paraphrases are
Russian. `repograph.toml` says of its own `id_families` list: "This is beauty-crm's
census, not a generic default; a different repository should list its own families."
Nothing in the suite measures the tool on a repository it was not tuned against.

**Why it matters for the next version.** 0.6.0 targets C#, Kotlin and Python for
`bonliva-crm-nx`, `beauty-crm/mobile` and `bonliva-erp`. Shipping language support
with no cases from those corpora repeats exactly the mistake ADR-001 exists to record:
a number that was a prediction travelling as though it had been measured.

**Lever.** A second case file per new corpus — smaller is fine, 20–30 cases — built
the same way: expectations recomputed from the repository at run time, so the set goes
stale only when a name disappears. The retrieval half will need each corpus's own id
families; the blast half needs nothing but symbols and should port directly.

**Gate.** No language ships in 0.6.0 without at least one `impact`, one `trace` and
one `changes` case in its own corpus, and a result file beside this one.

---

## G7 · Enrichment costs two exact keyword seeds — `--no-dense` keyword 37/40 against a raw 39/40

**Raised (2026-09-04):** not by the three-graph run. It surfaced once `bench` began grading a
store on the configuration it actually is
([ADR-001 Amendment 5](../adr/ADR-001-paraphrase-recall-was-a-prediction.md)): the enriched
lexical-only arm had been measured against the enriched floors all along, and reading it honestly
made it red.

**Measured.** On `beauty-crm` with its generated questions in the store,
`repograph bench --no-dense` reads
`keyword 37/40  paraphrase 15/30  code 12/12  p90 220 tok  enriched=true (1996/1996 nodes)` and
exits 1. The three misses are `FR-WH-53` «отчёты склада», `FR-PH-43`
«критерий готовности рыночному запуску» and `W-206` «каскадного сдвига дня нет» — exact-id
keyword cases, the kind lexical retrieval is supposed to be unbeatable at. The same corpus with
no questions in the store reads **39/40** in that arm, missing only `FR-PH-43`, so enrichment
costs the other two; both states read 40/40 with embeddings. Paraphrase runs the other way on the
same arm, 7/30 raw against 15/30 enriched, so this is a trade the fusion makes unasked, not a
straight loss.

**Candidate cause, named and not confirmed.** `src/query.rs:98-101`. On the plain, non-rerank
path the generated-questions BM25 list is pushed into `lists` *before* the passage BM25 list, and
`fuse::interleave` takes the leading list's head first, so a questions row of some other node
outranks the exact passage row it should sit behind. With embeddings the dense passage list is
pushed first and leads instead — which is exactly the arm that holds 40/40. Two exact seeds lost
at a depth of five is the shape that order predicts.

**Diagnostic before the lever.** `repograph dump --queries` on the three questions, reading the
four retriever lists 300 deep with their scores. It confirms the cause only if the target sits in
the passage BM25 list at a rank the interleave would have taken had that list led, and the rows
that displaced it are other nodes' questions. If the target is missing from the passage list as
well, this is not fusion order and it belongs with the extractor or the id census.

**Why the floor is not the lever.** The enriched keyword floor stays at 40. Lowering it to 37
would bless a regression enrichment itself causes, which is the inversion of what Amendment 5
says. The arm stays red until this is fixed.

**Lever, deliberately not chosen yet.** At least four are available — lead with the passage list,
weight the interleave instead of alternating, drop the questions list from the plain path, or
confine it to the rerank pool — and each pays differently on the paraphrase split the same
fusion buys. It needs measuring across all four arms (dense/no-dense × raw/enriched), the way G2
and G4 insist: a change judged on the lexical-only enriched arm alone could trade eight
paraphrase hits for three keyword ones without anyone noticing.

**Gate.** Enriched `--no-dense` back to keyword 40/40 with paraphrase no lower than its floor of
11/30, and the other three arms unmoved at their measured numbers — on the 400 held-out question
set with a paired exact McNemar test, before the 82 real cases are consulted as the smoke test
they are. `repograph bench --no-dense` exits 0 on the reference store.

---

## Suggested order

| | gap | why here |
|---|---|---|
| 1 | **G7** the fusion order on the plain path | the only floor a shipped arm is currently failing; `bench --no-dense` exits 1 until it is settled |
| 2 | **G5** rank + MRR | a scoring change over rows that already exist, and G2 cannot be argued without it |
| 3 | **G1** unparsed files get a file node | the largest missing share of a real answer, and the fix is language-independent |
| 4 | **G3** the three impact diagnostics | three files to read; it either finds a bug or writes an honest caveat |
| 5 | **G4** grow the blast set | must land before G1's and G6's changes are judged on it |
| 6 | **G2** measure `bge-reranker-v2-m3` | the one unmeasured retrieval lever; everything cheaper is already rejected |
| 7 | **G6** a case file per new corpus | ships with the 0.6.0 languages, not after them |

## What is explicitly not on this list

- **Importing graphify's edges.** Measured: 8 577 concept nodes and 23 117 edges move
  the shipped bench from 40/15/12 to 38/16/12. Rolled back, and ADR-001's reasoning
  says why more concept nodes do not help a question that shares no stem with its
  target.
- **More seeds, wider hops, a different E5 size.** All measured in ADR-001 and its
  amendments; each buys about one hit and pays in the token budget the bench exists to
  protect.
- **Query rewriting by a model.** Measured at paraphrase 6/14 with keyword falling to
  20/24, and rejected.
