# Issue #77: an asker's-voice pass added to enrich — the rule, written before any reading

**Date:** 2026-09-23. **Branch:** `feat/enrich-asker-pass`, from `main` at `28d55ba` (0.5.3).
**Kit:** `/Users/max/bench/issue77-2026-09-23/` (the #77 diagnosis) and its `asker-pass/`
subdirectory (everything this attempt writes).

This file is committed before the pass exists and before any store is read with it. The
numbers are appended below the rule afterwards; the rule above them is not edited.

## What is being tried

G14's second reading (`2026-09-06-g14-second-diagnostic.md`) found the paraphrase loss is
register: a node's questions written in the document's own vocabulary stop matching a question
asked the way a person talks. The union of an asker's-voice set and a document's-voice set read
paraphrase 20/30 on the fixture against 15 and 17 for either alone. PR #14 then *split* the fixed
12-question budget between the two voices and fell to 13/30: it took vocabulary away to make room.
Issue #77 is the same loss on beauty-crm HEAD `a3bf96ff`: 12/30 on the store HEAD had, 11/30
after a full English-and-Russian re-enrich, against the pin floor of 14.

The lever, chosen by Max: **add, never reallocate.** The existing 12-question prompt stays byte
for byte what it was. A second prompt asks for about six more questions per entry in the asker's
colloquial voice — first person allowed, everyday words, the way someone who never read the
documentation asks — in the same `language_rule` languages. They are stored beside the node's
existing questions, read back after them, and staled by a hash of their own, so a store whose
questions are current pays for this pass alone.

## The arms

| arm | checkout | store | asker pass | languages the pass writes in |
|---|---|---|---|---|
| **P0** | pin `502e8a6d` | `store-before` — the fixture's own store | none | — |
| **P1** | pin `502e8a6d` | `store-before` | added | Russian, English (detected: the pin has no `repograph.toml`) |
| **H0** | HEAD `a3bf96ff` | `store-head-copy` | none | — |
| **H1** | HEAD `a3bf96ff` | `store-head-copy` | added | English, Russian (HEAD's `repograph.toml`) |

`store-head-copy` is the store HEAD had before #77's re-enrich — questions from 2026-09-12, in
each entry's own language, the 12/30 reading. The re-enriched English-and-Russian store (11/30)
was not kept, so it is not an arm. Paying for another roll of it would buy a third sample of the
generation variance #77 already measured, not a reading of this pass.

Every arm is read with `bench` twice, dense and `--no-dense`, by the same binary built from this
branch. P0 and P1 are also read on the developer suite (`--cases bench/dev-cases.jsonl`), both
arms, because `rule` is the kind G14 showed moves with register. P0 and H0 are re-read with that
binary rather than quoted, so every comparison below is one binary against itself.

**The pass runs alone.** Both stores were written before a language list was pinned, so under
the lists their checkouts resolve today (above) every one of their existing entries is stale, and
a plain `enrich` would re-roll the whole main set along with adding the new one. A re-roll is the
lottery #77 measured; mixed into this reading it would make the pass's effect unreadable. So the
measured runs use `enrich_command` set, through `REPOGRAPH_CONFIG`, to a wrapper that answers the
main prompt with nothing — those batches fail, and their entries keep the questions they have —
and sends the asker's prompt to the default generator unchanged: the same `claude -p --model
haiku` line with the same flags, except `--output-format json`, which the wrapper unpacks to the
reply text and a cost line. The main-set failures are expected and reported, not counted against
the run.

## The rule

The pass ships only if **every** clause holds. A clause's reference is the reading named in it,
never a number remembered from another day.

1. **#77 is closed on HEAD, dense.** H1 dense: paraphrase ≥ 14/30, keyword 40/40, code 12/12,
   p90 ≤ 230.
2. **HEAD, lexical, loses nothing.** H1 lexical: paraphrase ≥ H0 lexical, keyword ≥ 39/40, code
   12/12, p90 ≤ 230.
3. **The fixture loses nothing, dense.** P1 dense: paraphrase ≥ P0 dense (15/30 on the record),
   keyword 40/40, code 12/12, p90 ≤ 230.
4. **The fixture loses nothing, lexical.** P1 lexical: paraphrase ≥ P0 lexical, keyword ≥ 39/40,
   code 12/12, p90 ≤ 230.
5. **Register did not cost `rule`.** On the developer suite, in each arm: P1 `rule` ≥ P0 `rule`,
   and P1's total of cases answered ≥ P0's.

Not in the rule, and why:

- **Held-out.** G14's list asks for it judged on a set built from a generator not in the store.
  No such set exists; the one that does was built from the fixture's own questions and favours
  any store that holds them, which both P0 and P1 do.
- **Rerank arms.** They cost model tokens per case and are not what #77 reported.
- **Cost.** Reported beside the verdict, never a clause: whether a pass that works is worth its
  price is Max's decision, not this measurement's.

If a clause fails, the pass is refused: the code is reverted, and this file keeps the numbers,
the refusal, and what they teach.

## What is recorded

Every `enrich`, `embed` and `bench` transcript, the wrapper, its cost log and the binary go to
`asker-pass/` in the kit. Each bench run is appended to `bench/history/runs.jsonl` through
`track.py record` with a `--tag` naming its arm, so no store copy's rows pool with the fixture's
routine runs. Per-case flips are listed against P0, H0 and, for HEAD, the 11/30 roll in the
#77 kit's `bench-after.txt`.

## Results

**2026-09-23.** Every arm was read by one binary, `repograph-2f051a8`, built from this branch after
it was rebased onto `main` at `01ffbf8`. The pass is `71f8353`; `2f051a8` makes the lexical index
read a store whose only questions are the asker's. Before any paid run, P0 and H0 were read by the
first build, `6e125f9`, and the re-reads matched them case for case. The runs are in `runs.jsonl`
under the tags `asker77-P0`, `asker77-P1`, `asker77-H0` and `asker77-H1`.

### Readings

| arm | reading | keyword | paraphrase | code | p90 | graded |
|---|---|---|---|---|---|---|
| P0 | dense | 40/40 | 15/30 | 12/12 | 221 | enriched (1996/1996) |
| P1 | dense | 40/40 | **20/30** | 12/12 | 223 | enriched (1996/1996) |
| P0 | lexical | 39/40 | 15/30 | 12/12 | 215 | enriched |
| P1 | lexical | **38/40** | 19/30 | 12/12 | 213 | enriched; `bench` exits red on the keyword floor |
| H0 | dense | 40/40 | 12/30 | 12/12 | 231 | not enriched (2147/2154, other languages) |
| H1 | dense | 40/40 | **11/30** | 12/12 | **234** | enriched (2154/2154) |
| H0 | lexical | 39/40 | 12/30 | 12/12 | 221 | not enriched |
| H1 | lexical | 40/40 | **11/30** | 12/12 | 223 | enriched |

On the developer suite:

| arm | reading | long | cross | multi | where | rule | total | p90 |
|---|---|---|---|---|---|---|---|---|
| P0 | dense | 10/15 | 12/15 | 10/12 | 0/9 | 5/9 | 37/60 | 244 |
| P1 | dense | 11/15 | 11/15 | 10/12 | 0/9 | 6/9 | 38/60 | 252 |
| P0 | lexical | 11/15 | 12/15 | 9/12 | 0/9 | 5/9 | 37/60 | 239 |
| P1 | lexical | 12/15 | 12/15 | 12/12 | 0/9 | 7/9 | 43/60 | 235 |

H1 grades as enriched only because of how the run was made, not because its own questions changed.
`enrich` writes the language list into the store at the end of a run even when every main batch
failed, which this run's wrapper made them do. After that, `bench` no longer sees the store as
"written for other languages". The own set of each store is identical, entry for entry, to the
store it started from.

### The verdict: refused

| clause | reading | holds |
|---|---|---|
| 1. #77 closed on HEAD, dense | H1 dense paraphrase 11 < 14; p90 234 > 230 | **no** |
| 2. HEAD lexical loses nothing | H1 lexical paraphrase 11 < H0's 12 | **no** |
| 3. Fixture dense loses nothing | P1 dense 40/20/12, p90 223 | yes |
| 4. Fixture lexical loses nothing | P1 lexical keyword 38 < 39 | **no** |
| 5. Register did not cost `rule` | dense rule 6 ≥ 5, total 38 ≥ 37; lexical rule 7 ≥ 5, total 43 ≥ 37 | yes |

Three clauses fail, so the pass is refused. `c1d98fb` reverts `71f8353` and `2f051a8` on this
branch. This file and the `runs.jsonl` rows stay.

### Per-case flips

On the pin, P0 → P1:

- **Dense**, won 5, lost 0: paraphrase FR-VIS-01, FR-AI-21, FR-STAFF-45, FR-CRM-11 and N-109.
- **Lexical**, won 4, lost 1:
  - won paraphrase FR-PAY-03, FR-AI-21, FR-STAFF-45 and N-109;
  - **lost keyword FR-WH-53** («отчёты склада»).
- **Developer suite, dense**, won 3, lost 2:
  - won long FR-OPS-05, multi FR-OPS-60+61 and rule ADR-003;
  - lost cross FR-CAL-50 + `core.ts` and multi FR-SHELL-70+71+FR-APP-42.
- **Developer suite, lexical**, won 6, lost 0: multi FR-CAL-110..112, rule ADR-031, multi
  FR-MKT-21..24, long FR-OPS-05, multi FR-OPS-60+61, and rule ADR-003.

On HEAD:

- **H0 → H1 dense**, won 1, lost 2: won FR-STAFF-45; lost FR-WEB-34 and FR-DM-66.
- **H0 → H1 lexical**, won 2, lost 2:
  - won keyword FR-PH-43 and paraphrase FR-STAFF-45;
  - lost FR-WEB-34 and FR-DM-66.
- **The 11/30 re-enrich roll → H1 dense**, won 1, lost 1: won FR-STAFF-45; lost FR-TOOL-22.

**Why FR-WH-53 fell.** In P0 lexical it ranked fourth for the two-word query «отчёты склада». In P1,
W-175 («[Отчёты] Пятьдесят два отчёта…») moved to first and FR-DM-56 entered the top five, which
pushed FR-WH-53 out. The asker's set more than doubles the question text the lexical index
reads: on the pin, 91% more questions and 122% more characters. That is enough to reorder a
two-word keyword query that was already near the edge. The case was marginal before the pass. It
is still lost.

**Why FR-WEB-34 fell on HEAD.** FR-WEB-34's asker set came back in English and *Polish*. HEAD's
list is English and Russian. It is one of 19 nodes on HEAD (48 of 25,753 questions) where haiku
wrote Polish instead. Meanwhile FR-DM-50's Russian gift-card questions («Можно ли передать
оставшуюся сумму на сертификате кому-то другому?») took its place in the top five. FR-DM-66 went
the same way: a neighbour with close questions (N-147, on a calendar falling out of sync) moved
ahead of it.

### What HEAD's gap is not

The cases the pin hits and HEAD misses stay missed in every HEAD reading, with the asker's set or
without it. ADR-004, ADR-005 and FR-SVC-39 are hit in all four pin readings and missed in all four
HEAD readings. Their documents are byte-identical between `502e8a6d` and `a3bf96ff`. Each node has
Russian asker's-voice questions on HEAD, and on the probe (`ask --stale --no-serve`, on a scratch
copy of the H1 store) none of the three reaches the top five.

The register the pass adds works on the pin: dense 15 → 20. It does not reach these cases. What
separates HEAD from the pin is either the roll of the own set (the 2026-09-12 questions HEAD
carries, not the fixture's) or HEAD's 158 extra nodes competing for the same seats.
This run does not say which.

### Cost and time

| arm | calls | output tokens | cost | enrich | embed |
|---|---|---|---|---|---|
| P1 | 337 (333 + 4 retries) | 1.10 M | $12.69 | 1,618 s | 23,853 rows in 68 s |
| H1 | 374 (359 + 15 retries) | 1.20 M | $13.71 | 1,992 s | 25,752 rows in 74 s |
| **total** | | | **$26.40** | | |

The estimate was ≈ $32. Also spent: the two probe calls ($0.06), and before them the accidental
run on a scratch copy (about $1–1.8). That run is why `main` now refuses a project-level
`enrich_command` (#79).

- **P1:** every node got the asker's set. Its four short batches all recovered on the retry.
- **H1:** 2,148 of 2,154 nodes got the set. The six-node batch starting at W-106 answered for
  nobody twice.
- **Main batches:** they failed by design in both arms (333 and 359). No entry of either own set
  changed.

**Transport noise.** claude.ai connectors reach `claude -p` even with `--setting-sources ""`, and
haiku sometimes tries to write its answer into a Claude Docs document. It then replies "I don't
have permission to create a Claude Doc" and follows with the questions in a format the parser only
partly reads.

Short asker batches were 4 of 333 on the pin and 15 of 359 on HEAD, most of them this failure. The
main prompt goes through the same command, so it is exposed in the same way. It is not a finding
about the pass, but any paid enrich on this machine carries it while the connectors are attached.

### What this teaches

1. **The asker's voice is real signal, and not #77's cure.** On the fixture it lifts paraphrase by
   5 dense and 4 lexical, and the developer suite's `rule` by 1 and 2. That is G14's union reading
   (20/30) reproduced by the additive pass. On HEAD it moves nothing: 12 → 11, with one win and
   two losses. HEAD's paraphrase loss is not a missing register.
2. **Added text is not free for keyword queries.** Doubling the question text was enough to
   reorder one marginal two-word query and break the lexical keyword floor. If the asker set comes
   back, it needs a seat of its own, for example a separate list or a lower weight. It should not
   be appended to the questions every keyword query reads.
3. **The paid stores are kept, so the next retrieval-side idea reads for free.** `store-P1` and
   `store-H1` in the kit hold both sets. A separate asker's list, a weight, or a gate re-derived
   for the union can all be read against them without another enrich. Each needs its own rule,
   written before it is read.
4. **HEAD's gap needs its own diagnosis.** Take the three cases above. Give HEAD the fixture's own
   set for those nodes, or read HEAD with its 158 extra nodes left out. Either one separates
   "roll" from "competition" at no model cost.

### The kit

`/Users/max/bench/issue77-2026-09-23/asker-pass/` holds:

- **Binaries:** `repograph-6e125f9` and `repograph-2f051a8`.
- **Transport:** `wrapper.sh` and `machine.toml`.
- **Scripts:**
  - `bench-arm.sh <arm> [dev]`;
  - `run-arm.sh <read0> <read1> [dev]`, which reads the baseline, enriches, copies the store and
    reads the arm;
  - `flips.py`.
- **Logs:**
  - cost logs `costs-{probe,P1,H1}.jsonl`;
  - enrich transcripts `enrich-{P1,H1}.*`;
  - kept replies `raw-{P1,H1}/`.
- **Stores:** `store-P1/` and `store-H1/`.
- **Bench transcripts:** `bench-*.txt` and `dev-*.txt`, with the first reads by `6e125f9` in
  `read-6e125f9/`.

The test worktree was restored to `502e8a6d` with `store-before`: `git status` is clean, and
`diff -rq` against `store-before` shows no difference.
