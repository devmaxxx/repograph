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

G9, G10 and G11 were raised on 2026-09-05 by the first run of the developer-questions suite and
carry a **Verdict** line from the six levers measured against them the same day
([results](2026-09-05-dev-cases-results.md)); G12, G13 and G14 were raised by those measurements.

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
a barrel, and Task 4 made a re-exporting barrel an importer. The 0.949 this gap was raised
on is in [the three-graph results](2026-09-03-three-graphs-results.md); what closed it is in
[the 0.5.0 gap-closing results](2026-09-04-repograph-0.5.0-results.md).

---

## G4 · The blast suite has 20 cases and no confidence story

**Status (2026-09-04):** **closed by Task 5** — `blast.jsonl` is 32 cases: `impact` 10
→ 16 (six narrow targets, the tier a mean over hubs hides), `changes` 2 → 8 diffs
spanning small/large and mono/polyglot, `trace` unchanged at 8. The decision rule this
gap asked for is written down in
[`three-graphs.md`](three-graphs.md#when-a-blast-delta-counts), per suite, with a noise
column — stated before the changes above were measured against it. Numbers:
[the 0.5.0 gap-closing results](2026-09-04-repograph-0.5.0-results.md).

---

## G5 · Strict is a substring, so ranking is unmeasured

**Status (2026-09-04):** **closed by Task 1** — every retrieval row carries `rank`, the
summary carries `mrr`, and this run measures **0.635** overall: keyword 0.816, code
1.000, paraphrase **0.247** with 2 of 30 at rank 1. One clause of the gate is unmet and
unmeetable: the 2026-09-03 rows cannot be rescored, because the harness stored counts
and never the answer text, so the field starts here with no prior value. The paraphrase
row also answers the question this gap was raised to serve — reordering cannot produce
the 14 ids that are absent, and finding them would not by itself put them first. Numbers:
[the 0.5.0 gap-closing results](2026-09-04-repograph-0.5.0-results.md).

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

## G7 · Enrichment cost two exact keyword seeds — closed 2026-09-05

**Raised (2026-09-04):** not by the three-graph run. It surfaced once `bench` began grading a
store on the configuration it actually is
([ADR-001 Amendment 5](../adr/ADR-001-paraphrase-recall-was-a-prediction.md)): the enriched
lexical-only arm read `keyword 37/40` against the raw store's `39/40` on the same corpus, missing
`FR-WH-53` and `W-206` that the raw store answered, and `repograph bench --no-dense` exited 1.

**Closed by** the per-question gate on the generated-questions list described in
[ADR-001 Amendment 6](../adr/ADR-001-paraphrase-recall-was-a-prediction.md): the list joins the
plain-path fusion only when its best BM25 score is at least 0.85 of the passage list's. The
enriched lexical-only arm reads `keyword 39/40  paraphrase 14/30  code 12/12  p90 215` — parity
with the raw store, both runs identical case by case — the arm with embeddings is unchanged at
40/40 and 15/30 with p90 down from 226 to 220, the raw arms are untouched by construction, and 400
held-out questions moved by 5 gained and 7 lost, exact McNemar p = 0.77. The floor in that arm moved
from 40 to 39, which is what both stores measure at this commit; it did not move while the arm read
37, because 37 was a cost enrichment imposed.

**What the measurement corrected.** The cause first named here — the questions list pushed ahead of
the passage list — was wrong: leading with the passage list moves the three misses from fused rank
6, 10 and 42 to 6, 9 and 41. The cost was the list's *share* of five seeds in a round-robin, on
questions it held no answer to (ten of the forty keyword cases, against zero for the passage list).
Thinning that share for every question was the first lever tried and was rejected by this gap's own
gate — lexical held-out recall@5 0.283 → 0.255, 13 lost and 2 gained, p = 0.007 — because on a
paraphrase the questions list is the retriever doing the work. And the held-out set had to be
rebuilt before any of that could be read: written as kind `paraphrase` instead of `synthetic`,
`dump` had not held anything out, and the set read 0.955. `bench/heldout.py` now builds it with the
right kind and a recorded seed, and `dump`'s recorded answer comes from the held-out indices too.

**What stays open.** `FR-PH-43` — «критерий готовности рыночному запуску» — sits at passage rank
23 and no lexical path reaches it in either store; only the dense list does. That is a lexical
retrieval limit, not an enrichment cost, and it is why both lexical-only floors are 39 rather than
40. And the gate's window is narrow, and narrower than first published. The closing note said
held-out tolerates any threshold up to 0.90; the review measured 0.87 and 0.90 reddening the arm
with embeddings — paraphrase 13/30 under its floor of 14 — while held-out never binds at any value
from 0.80 to 0.95 (p = 0.34 to 0.79). The window on the 82 is (0.802, 0.866]: below it `FR-WH-53`
(ratio 0.801) loses its seat, at 0.87 a paraphrase (ratio 0.867) loses its list. The centre, 0.83,
reads the same on the 82 and loses three more lexical held-out questions than 0.85 (109 → 106,
none gained), so 0.85 stays, with 0.006 of room above. Why the ratio is a constant of this store
and what would carry across stores is gap G8. Any move of it is judged on the held-out set first —
the order of operations in `bench/heldout.py`'s header, which now covers both arms.

---

## G8 · The questions gate is a constant of one store

**Raised (2026-09-05)** by the review of G7's fix. The gate compares the best BM25 score of the
generated-questions list with the best of the passage list, and the two are only half
comparable — why is measured in
[ADR-001, Amendment 7](../adr/ADR-001-paraphrase-recall-was-a-prediction.md): each index
normalises length and weights terms against its own population, so the ratio moves with
enrichment coverage and with questions per node, and 0.85 is where *this* store's two populations
part — window (0.802, 0.866] on the 82, 0.006 of room above. A store with short bodies, full
enrichment or five questions per node lands somewhere else, and nothing in the code would say so.

**Gate.** A scale-free form — each list's best against its own *k*-th score, or a z-score within
its own list — replaces the constant only if it reproduces the four arms exactly on this store
(`40/40 15/30 12/12`, `39/40 14/30 12/12`, raw `40/40 9/30`, `39/40 7/30`, p90 ≤ 230), is not
significantly worse on the held-out set in either arm, and reads at least as well on the 60
`dev-cases`. The order of operations is `bench/heldout.py`'s header. Not tried.

**Status (2026-09-05, 0.5.0): measured, not closed.** All three scale-free forms the design note
pre-registered were derived and scored offline over the fixture's dumps, in order, and all three
failed the first clause — the exact reproduction of the four arms. `coverage` at c = 0.761 reads
`40/40 15/30 12/12` with embeddings and `39/40 **15**/30 12/12` without, `peak` at c = 0.894 reads
`40/40 14/30 12/12` and `38/40 14/30 12/12`, `z` at c = 0.548 reads `40/40 15/30 12/12` and
`37/40 15/30 12/12`. The gate stays at 0.85 and `src/query.rs` is untouched, so there is no ADR-001
amendment. Coverage fails only by being *better* — one paraphrase more in the lexical arm, both
developer arms up, held-out up in both arms and significant in neither — and a plan that
pre-registers "every floor held, developer suite not down, held-out not worse" can ship it on the
recorded evidence without re-measuring. What that evidence still lacks is the fourth clause: the
second population (the fixture's questions cut to five per node) was never built, so nothing yet
shows the constant is a property of the query rather than of this store. Every number is in
[the 0.5.0 gap results](2026-09-05-0.5.0-gaps-results.md), L1.

---

## G9 · Code is unreachable from prose — `where` 0/9

**Raised (2026-09-05)** by the first run of the developer-questions suite
([results](2026-09-05-dev-cases-results.md)). Nine questions describe a behaviour and ask which
file to edit; none reaches its file in either arm, and 12 of the 13 file anchors are absent from
every lexical list 300 deep. The store said nothing about what code does: a symbol's text was its
name and signature line, a file node had no text and was not indexed, and `enrich` asked about
documents only. The `cross` kind reads the same hole from the requirement's side — 12/15 reach the
requirement, 11 of 15 file anchors are absent.

**Levers, measured in that document.** A1: doc comments and file heads become node bodies,
files with a head are indexed, identifiers are indexed by their words — no tokens. A2: `enrich
--code` generates questions about code through a prompt of its own — tokens, opt-in, and the
`enriched` grading of documents unchanged. Each ships only on the rule written there: every
recorded floor held in all four arms, held-out not significantly worse in either, dev-suite hits
up in both arms with `where` above zero.

**Verdict (2026-09-05):** **partly closed, and not in the plain `ask`.** Four levers were measured
against that rule and all four were rejected. A1: the bodies in the passage index moved the BM25
statistics and cost paraphrase, 15/30 → 13/30. A2: the code questions inside the documents'
questions index lifted that index's average length, every questions score rose by about a quarter
(`N-071` 9.01 → 11.57), and the gate admitted the questions list over the passage holding the
answer — keyword 39/40 → 38/40 without embeddings, `where` 1/9. A3: the same questions as a list of
their own, admitted last on the 0.85 gate, cost `FR-WH-53` and `FR-CRM-11` and lost six and seven
held-out questions with none gained. A4: that list capped at one seat holds every floor and reads
`where` 2/9, and still loses six held-out questions in each arm with none gained, p = 0.031. What
ships is the machinery — doc comments and file heads extracted for the prompt and for display,
`enrich --code` writing questions about 3,463 of 3,475 code nodes, and those questions in an index
of their own that reaches the `ask --rerank` pool and never the plain fusion. `where` therefore
stays 0/9 in the shipped configuration, and what would move it without taking a document seat is an
admission rule that is not a ratio of two indices' raw scores — G12, with G13's table as what it is
judged on.

## G10 · Five seeds over three lists — the right answers that were ranked and not seated

**Raised (2026-09-05)** by the same run. 31 missed anchors sit at rank ≤ 20 of some retriever
list and are not among the five seeds, most of them `multi` siblings (`FR-STAFF-20/21/22`,
`FR-PAY-22/25/26/27`, `FR-CAL-110/111/112`): a round-robin over three lists gives each list at
most two seeds, so a list's third right answer never surfaces. Seven of the recorded suite's
fifteen paraphrase misses have the same shape (rank 3–9 of their best list). More seeds was
measured and rejected in ADR-001 (~1.5× tokens a step for about one hit); a fourth list (dense
questions) was simulated over the dumps and rejected (paraphrase unchanged at every gate,
keyword −1, held-out 102 → 92–96). What remains untried is seating by *agreement between
lists* rather than by turn — an id two lists rank in their top ten ahead of an id one list ranks
second — which ADR-001's first amendment rejected as RRF for a different reason (it buried a
dense rank-2 target) and which a sibling-aware expansion could serve instead: when a seed's
document neighbours are themselves ranked, name them on the expanded line. Gate: `multi` and
`rule` up on the dev suite, the recorded suite unchanged, held-out not worse.

**Verdict (2026-09-05):** **confirmed as the binding constraint, and priced.** A4 is the
measurement this gap was missing: a fourth list, admitted only when it clears the gate and capped
at a single seat, reads `where` 0/9 → 2/9 on the developer suite and loses six of 400 held-out
questions in each arm with none gained (p = 0.031); uncapped (A3) it loses six and seven and takes
`FR-WH-53` and `FR-CRM-11` off the recorded suite as well. Five seeds are the budget the recorded
floors were set on, and any list that earns its turn displaces a document seed. Closed as measured,
not as fixed: seating by agreement between lists, the one shape still untried, is what this gap
asks for.

## G11 · The embedder, for Russian paraphrase

**Raised (2026-09-05)**, sharpened from ADR-001's first amendment. Eight of the recorded
suite's fifteen paraphrase misses sit at rank 31+ in every list while the store's own generated
questions for the target are near-synonyms of the query — «Где лежит короткий код на
устройстве?» against «как хранится код доступа сотрудника» at dense-questions rank 209. That is
`multilingual-e5-small`'s distance between two Russian paraphrases, and no fusion rule repairs it.
The first amendment measured `e5-base` and `bge-m3` on fourteen cases without generated questions
and found no gain; neither was measured with the question rows in the index, which is where a
stronger model would show. Lever: `REPOGRAPH_EMBED_MODEL` re-embeds a store copy under another
hub model; `multilingual-e5-large` is the one measured in the results document. Gate: the same
three-way rule, dense arms only, plus query latency under a second.

**Verdict (2026-09-05):** **closed for the dense arm, as an option.**
`intfloat/multilingual-e5-large` on a copy of the fixture store reads paraphrase **22/30** against
the small model's 15/30 — seven of the fifteen misses — with keyword 40/40, code 12/12, p90 224,
the developer suite 37/60 and held-out 103 → 119 of 400 (+19 −3, p = 0.0009): the whole three-way
rule, in the arm it applies to, and query latency still under a second. It ships as a property of
the store — `embed_model` in `repograph.toml`, recorded in `vectors.json`, opened by every reader —
and the default moved to it at `35357c1`; `UNNAMED_MODEL` keeps a store with no recorded model
reading as the small one, so an older store never silently reinterprets under the new default. The
cost of that default is measured in [Embeddings](../../README.md#embeddings). Nothing here reaches
the `--no-dense` arm, which has no dense list to improve.

**Update (2026-09-06, 0.5.0):** `bench` has floors of its own for the default model since 0.5.0 —
40/22 enriched and 40/17 raw ([the 0.5.0 gap results](2026-09-05-0.5.0-gaps-results.md)).

## G12 · The gate compares raw BM25 scores across two indices

**Raised (2026-09-05)** by A2. The questions gate — and the code gate A3 gave the code list — divides
one index's best score by another index's best score. Each index normalises length against its own
mean and weights terms by its own vocabulary, so a population change in one of them moves the ratio
while nothing about the query, the answer or the fusion changes. A2 is the proof: 3,463 code nodes
gaining questions turned one-token documents in the questions index into question-length ones, every
questions score rose by about a quarter (`N-071` 9.01 → 11.57) while the passage scores stayed,
`FR-WH-53`'s ratio moved 0.80 → 1.03, and the questions list was admitted over the passage that held
the answer — keyword 39/40 → 38/40 in the `--no-dense` arm, on a change that added no document to
that arm's passage index. G8 says the constant's *value* is a property of one store; this says the
comparison itself is between two units.

**Lever.** A normalised admission: each list's best as a fraction of what the query could attain in
that index, or each list's best against its own *k*-th score, so the constant is a property of the
query rather than of the store's populations. That is the same shape G8 asks for, and closing it
would close G8 with it.

**Gate.** The four arms reproduced exactly on this store (`40/40 15/30 12/12`, `39/40 14/30 12/12`,
raw `40/40 9/30`, `39/40 7/30`, p90 ≤ 230), held-out not significantly worse in either arm, and the
developer suite not down — then a second store, enriched to a different depth, reading the same
admissions the ratio would have given it. Unmeasured.

**Status (2026-09-05, 0.5.0): measured, not closed.** Three normalised admissions were built and
scored — a list's best over what the query could attain in that index (`coverage`), over its own
fifth score (`peak`), and as a z-score within its own list (`z`) — each with its constant derived
from the 400 held-out questions in the `--no-dense` arm before either suite was opened, and each
failing the exact-reproduction clause. The replay that produced them is not throwaway: `bench/admission.py`
replays the plain fusion offline over `dump` records and reproduces the shipped binary case for case
(82/82, 60/60, 400/400 in both arms) under the ratio, and `dump` now carries an `attainable_*` column
per list, so the next attempt costs a replay rather than a build. The comparison is still between two
units in the shipped binary. See [the 0.5.0 gap results](2026-09-05-0.5.0-gaps-results.md), L1.

## G13 · Ten `where` anchors sit at code-list rank 1–9 and six are below the gate

**Raised (2026-09-05)** by A3's dumps (`dump --queries`, `--no-dense` arm, 300 deep). The ratio is
the code list's best BM25 score over the passage list's best for the same query; A3 admits the code
list at 0.85, so a file anchor at code-list rank 1 with a ratio of 0.45 is never seated.

| kind | file anchor | code-list rank | ratio | best code | best passage |
|---|---|---|---|---|---|
| cross | apps/api/src/modules/availability/booking-create.service.ts | 1 | 0.86 | 30.06 | 34.86 |
| where | packages/domain/src/schedule/subjectAvailability.ts | 1 | 1.44 | 25.59 | 17.75 |
| where | packages/domain/src/money/index.ts | 1 | 0.64 | 23.61 | 36.92 |
| where | packages/db/src/schema/salon/catalog.ts | 1 | 0.99 | 28.28 | 28.69 |
| where | apps/api/src/shared/audit/pii-read.ts | 1 | 0.45 | 21.21 | 46.79 |
| cross | packages/ports/src/llm.ts | 1 | 0.65 | 24.30 | 37.38 |
| cross | packages/db/src/auditChain.ts | 1 | 0.56 | 19.53 | 34.80 |
| where | packages/contracts/src/money.ts | 2 | 0.64 | 23.61 | 36.92 |
| cross | packages/ports/src/fiscal.ts | 3 | 0.60 | 22.86 | 38.04 |
| where | apps/api/src/shared/audit/contact-access.repository.ts | 3 | 0.45 | 21.21 | 46.79 |
| where | apps/api/src/modules/identity/identity.repository.ts | 5 | 0.84 | 22.26 | 26.58 |
| where | apps/api/src/shared/db/database.service.ts | 6 | 0.90 | 22.69 | 25.28 |
| cross | packages/domain/src/availability/segments.ts | 8 | 0.61 | 17.80 | 29.09 |
| cross | apps/api/src/modules/sync/change-log-compaction.service.ts | 8 | 0.51 | 17.99 | 35.56 |
| where | apps/api/src/shared/context/tenant-context.interceptor.ts | 8 | 0.90 | 22.69 | 25.28 |
| where | apps/api/src/modules/identity/auth.service.ts | 9 | 0.84 | 22.26 | 26.58 |
| cross | apps/api/src/modules/identity/totp.ts | 15 | 0.69 | 22.60 | 32.90 |
| cross | packages/domain/src/availability/core.ts | 17 | 0.74 | 26.68 | 36.05 |
| cross | packages/contracts/src/generated/permissions.ts | 19 | 0.42 | 17.22 | 40.71 |

Ten `where` file anchors sit at code-list rank 1–9: four clear the gate (1.44, 0.99, 0.90, 0.90) and
six sit below it at 0.45–0.84. The retrieval is not the problem — the code questions put those files
at the top of their own list — the admission is. A lower gate for the code list is not the lever: it
is a second constant chosen by looking at the developer suite, which is how a suite stops measuring
anything. The honest form is G12's, and this table is what it would be judged on.

**Gate.** `where` above 0/9 in both arms with every recorded floor held and held-out not
significantly worse in either — the same three-way rule A2 to A4 were read against, which each of
them failed. Not tried in this shape.

**Status (2026-09-05, 0.5.0): regenerated, still open.** Stage B of the 0.5.0 admission work — the
code list admitted to the plain fusion under a scale-free form at one seat — needs a shipped form
and its constant, and no form shipped, so it was not run. `where` stays 0/9 and A4's price for one
seat (six held-out questions lost in each arm, none gained, p = 0.031) is still the number to beat.
The table is not the one above with columns added: it is a fresh 27-row table, code-list rank
1–286, from a new dump against a different store (`$G/bc-a1`, this plan's own copy, not the store
the table above was read against), with a column per candidate form so the next attempt reads what
each admission would have done to these anchors: six of its 27 rows clear the shipped ratio's 0.85,
14 clear `coverage`'s 0.761, 14 clear `peak`'s 0.894 and 26 clear `z`'s 0.548. It is in
[the 0.5.0 gap results](2026-09-05-0.5.0-gaps-results.md), L1.

## G14 · The enrichment prompt does not know the document's kind

**Raised (2026-09-05)** by D1, the strongest paraphrase lever this campaign measured and one it did
not ship. The documents' questions regenerated by sonnet instead of haiku — 1,996 nodes, 27,394
questions at a median of 13 a node, 1,283 s, ≈ $13.62 estimated from characters against ≈ $2.50 on
haiku — read paraphrase 17/30 and 16/30 against 15/30 and 14/30 and held-out 103 → 132 (+46 −17,
p = 0.0003) and 109 → 128 (+45 −26, p = 0.032), both arms significantly better. It fails the third
rule on one kind: the developer suite's `rule` questions go 5/9 → 2/9 in both arms — `ADR-005`,
`ADR-031`, `INV-07` and `INV-10` lost, `ADR-003` gained — which takes the `--no-dense` arm 37 → 36
and the whole lever out of the default.

**Candidate cause, and the diagnostic is cheap.** `enrich` asks the same
twelve-questions-plus-synonyms prompt of a requirement, an ADR and an invariant alike; the four
answers lost are two ADRs and two invariants, and the one gained is an ADR. Read the sonnet and
haiku questions for `ADR-005`, `ADR-031`, `INV-07` and `INV-10` side by side before designing
anything: either the sonnet lines drift towards "where do I find this" where the case asks "is this
allowed", or the cause is elsewhere and a per-kind prompt is the wrong lever.

**Lever.** A prompt per node kind: for ADR and INV documents, questions of the "is this allowed /
what forbids it" shape rather than "where do I find this". Measured under the same three-way rule,
on the same generator, so the prompt and the model are not changed in one step. A larger model is
not the next step — opus only if a kind-aware sonnet still falls short.

**Gate.** `rule` back to 5/9 or better in both arms with the paraphrase and held-out gains kept, and
the recorded floors held. Until then `ENRICH_COMMAND` stays haiku; `enrich_command` is per-store
configuration, so a store that wants sonnet's paraphrase recall today can have it in one line of
`repograph.toml`, at roughly five times the token cost.

**Status (2026-09-05, 0.5.0): the stated cause is measured and does not hold; the gap stays open.**
The free diagnostic this gap asked for was run and stopped the lever before any prompt was written
or any token spent. Haiku's and sonnet's questions for `ADR-005`, `ADR-031`, `INV-07` and `INV-10`
were read side by side and each line counted rule-shaped or behaviour-shaped: sonnet carries fewer
rule-shaped questions on 1 of the 4 lost nodes, against a pre-registered bar of 3 of 4. Because a
hand count was deciding a lever, a second reader classified the same two files without seeing the
first count or the plan and read 2 of 4. The two disagree on absolute counts and agree where it
matters — `ADR-031` moves as G14 predicted, `ADR-005` and `INV-07` move the *wrong* way, sonnet
asking *more* permission-shaped questions there. So the prompt's shape is not the difference the
questions show, and the next attempt should read the retrieval side of those four nodes before
writing a prompt. `ENRICH_COMMAND` stays haiku for the reason it always did, which is the developer
suite and not this diagnostic. Both counts, question by question, are in
[the 0.5.0 gap results](2026-09-05-0.5.0-gaps-results.md), L4.

---

## Suggested order

| | gap | why here |
|---|---|---|
| 1 | ~~**G7** the questions list's share of the seeds~~ | closed 2026-09-05 — gated on the ratio of the two lists' best scores, held-out p = 0.77, lexical-only arm at raw parity; the window (0.802, 0.866] is recorded above, corrected from the 0.85–0.90 first published |
| 2 | **G8** the questions gate is a constant of one store | measured, not closed (2026-09-05, 0.5.0) — three scale-free forms derived and scored, all three fail the exact-reproduction clause; `coverage` at c = 0.761 fails only by gaining a paraphrase, and a rule written as "every floor held" rather than "reproduced exactly" can ship it on the recorded numbers |
| 3 | **G12** the gate compares raw BM25 scores across two indices | measured, not closed (2026-09-05, 0.5.0) — `coverage`, `peak` and `z` each built, each with its constant derived from the held-out set before either suite was opened, each failing clause (i); the comparison is still between two units, and the replay that scored them (`bench/admission.py`, plus `dump`'s `attainable_*` columns) makes the next attempt a replay rather than a build |
| 4 | **G14** the prompt does not know the document's kind | measured, not closed (2026-09-05, 0.5.0) — the free diagnostic reads 1 of 4 lost nodes, and 2 of 4 on an independent second reading, against a bar of 3 of 4; two of the four move the wrong way, so the prompt's shape is not the difference the questions show. No tokens spent and no prompt written; the retrieval side of those four nodes is what a next attempt should read |
| 5 | **G13** ten `where` anchors ranked and gated out | regenerated, still open (2026-09-05, 0.5.0) — stage B needs a shipped admission and none shipped, so `where` stays 0/9 and A4's six held-out questions per arm are still the price to beat; the table now carries a column per candidate form |
| — | ~~the lexical arm's 49 ms~~ | closed 2026-09-05 (0.5.0) — the perf results left this number here and nowhere else. The BM25 indexes are built once by a resident context and kept: socket lexical 55.0 → 6.8 ms, median of 33 against a base spread of 0.9 ms, the design note's 30 ms target met; Rule 1 sixteen byte-identical verdicts and Rule 2 142/142 in four pairings |
| — | ~~**G9** code is unreachable from prose~~ | measured 2026-09-05 — A1 to A4 each rejected by the rule; the code questions ship into an index of their own for the `ask --rerank` pool, `where` stays 0/9 in the plain fusion, and G12 is what would move it |
| — | ~~**G10** five seeds over three lists~~ | measured 2026-09-05 — A4 prices one seat for a fourth list at 6 held-out questions in each arm, none gained, p = 0.031; closed as measured, not as fixed |
| — | ~~**G11** the embedder for Russian paraphrase~~ | shipped 2026-09-05 as a store option — e5-large reads paraphrase 22/30 and held-out 103 → 119, at 0.8 s an `ask` and a 2.1 GB download; the default followed at `35357c1`, and a store with no recorded model still reads as the small one |
| — | ~~**G5** rank + MRR~~ | closed by Task 1 (2026-09-04) — every retrieval row carries `rank`, the summary carries `mrr`, and the run reads 0.635; the 2026-09-03 rows cannot be rescored |
| 6 | **G1** unparsed files get a file node | the file half is closed by Task 2 (2026-09-04) against a `code_files` denominator; the symbol half is 57 Kotlin declarations and waits on 0.6.0's extractor |
| — | ~~**G3** the three impact diagnostics~~ | closed by Tasks 3 and 4 (2026-09-04) — 123/123 files over 16 targets, mean recall 1.0 |
| — | ~~**G4** grow the blast set~~ | closed by Task 5 (2026-09-04) — `blast.jsonl` is 32 cases and the per-suite decision rule is written down before the changes judged on it |
| 7 | **G2** fourteen paraphrases a neighbour away | the lever this row named is measured and rejected (2026-09-04): `--rerank-local` reads 17/30 against a control of 15/30, at 17.9 s a question; the gap stays open with nothing cheaper left to try |
| 8 | **G6** a case file per new corpus | the `impact` third is a written baseline (2026-09-04); `trace` and `changes` ship with the 0.6.0 languages, not after them |

## What is explicitly not on this list

- **Importing graphify's edges.** Measured: 8 577 concept nodes and 23 117 edges move
  the shipped bench from 40/15/12 to 38/16/12. Rolled back, and ADR-001's reasoning
  says why more concept nodes do not help a question that shares no stem with its
  target.
- **More seeds and wider hops.** Both measured in ADR-001 and its amendments; each buys about one
  hit and pays in the token budget the bench exists to protect. **A different E5 size** was on this
  list for the same reason and is no longer: `e5-base` and `bge-m3` were measured without generated
  questions in the index, and re-measuring the size with them — G11, 2026-09-05 — read paraphrase
  22/30 against 15/30 and held-out 103 → 119. It pays in latency, memory and a 2.1 GB download
  rather than in tokens, and it is the default since `35357c1`, with `embed_model` the way back.
- **Query rewriting by a model.** Measured at paraphrase 6/14 with keyword falling to
  20/24, and rejected.
