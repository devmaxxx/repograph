# The coverage admission, measured on the binary that carries it

**Date:** 2026-09-06. **Branch:** `feat/coverage-admission`, from `main` at `4ce704f` (0.5.0).
**Rule:** [`docs/superpowers/specs/2026-09-06-coverage-admission-design.md`](../superpowers/specs/2026-09-06-coverage-admission-design.md),
committed at `f1d7d4b` before a line of the form was written.
**Store:** the pinned fixture `beauty-crm-502e8a6d` — 908 files, 8.3k nodes, 1,996 enriched
documents, small-model rows. Read only: `bench` and `dump`, never a writer.
**Measurement files:** `/Users/max/bench/coverage-2026-09-06/`, baselines
`/Users/max/bench/gaps-2026-09-05/`.

## What shipped

The generated-questions list joins the plain fusion when its coverage, over the passage list's
coverage, reaches `c = 0.761`. A list's coverage is `best / attainable`: what its best document
scored, over the idf the query could have collected in that index at all. Both are BM25 numbers
from one index, so the fraction is dimensionless and the two lists arrive in one unit — which is
what the shipped ratio could not do, because it compared raw bests from two indices that
normalise length against their own means and weight terms by their own vocabularies.

`c` is the crossover of the 400 held-out questions in the `--no-dense` arm, derived on
2026-09-05 before either suite was opened (`$G/admission-verdict.txt`). It was not re-derived for
this run, not rounded and not tuned.

## The five clauses

Each was written before the form ran once. Each is answered from a file named beside it.

### 1. No floor breaks

| arm | keyword | paraphrase | code | p90 | gated |
|---|---|---|---|---|---|
| dense, enriched | 40/40 | 15/30 | 12/12 | 221 | true |
| lexical, enriched | 39/40 | 15/30 | 12/12 | 215 | true |

Both recorded arms ran `gated=true` and the process exited zero, which is `bench` saying every
floor it grades held. p90 221 and 215, both inside the 230 the clause allows.
(`$O/bench-rec-dense.txt`, `$O/bench-rec-lexical.txt`.) **Pass.**

### 2. Neither arm loses ground on the recorded suite

| arm | ratio (baseline) | coverage | movement |
|---|---|---|---|
| dense | 40/40 · 15/30 · 12/12 · p90 220 | 40/40 · 15/30 · 12/12 · p90 221 | none; p90 +1 token |
| lexical | 39/40 · 14/30 · 12/12 · p90 215 | 39/40 · **15/30** · 12/12 · p90 215 | one paraphrase gained |

Baselines from `$G/base-bench-rec-{dense,lexical}.txt`. **Pass**, and the one paraphrase the
lexical arm gains is the whole reason this rule exists: it is what the 0.5.0 clause, requiring
the four arms to reproduce *exactly*, refused the form for.

### 3. Neither arm loses ground on the developer suite

| arm | long | cross | multi | where | rule | total | baseline total |
|---|---|---|---|---|---|---|---|
| dense | 10/15 | 13/15 | 10/12 | 0/9 | 5/9 | **38** | 36 |
| lexical | 11/15 | 13/15 | 9/12 | 0/9 | 5/9 | **38** | 37 |

`$O/bench-dev-{dense,lexical}.txt` against `$G/base-bench-dev-{dense,lexical}.txt`. Both totals
rise and `where` is unmoved at 0/9, which the clause required. **Pass.**

The per-kind movement is worth stating even though the clause reads totals: the dense arm gains
one `cross` and one `multi`; the lexical arm gains two `cross` and **loses one `multi`**, 10/12 to
9/12. A rule written on totals passes that; a reader deciding what to do next should know a case
moved the other way.

### 4. No significant held-out loss

| arm | before | after | changed | exact McNemar |
|---|---|---|---|---|
| dense | 103/400 | 104/400 | 3 gained, 2 lost | p = 1.0000 |
| lexical | 109/400 | 112/400 | 5 gained, 2 lost | p = 0.4531 |

`$O/heldout-{dense,lexical}.txt`, paired against the ratio's dumps in `$G/base-ho-*.json` over the
same 400 questions built at seed 20260905. Both arms gain, neither significantly, and neither
loses. **Pass.** Run before the two suites were opened, which is the order `bench/heldout.py`'s
header makes non-optional.

### 5. The replay still agrees with the binary

```
rec-dense.json: 82/82 replicated      dev-dense.json: 60/60 replicated      ho-dense.json: 400/400 replicated
rec-lexical.json: 82/82 replicated    dev-lexical.json: 60/60 replicated    ho-lexical.json: 400/400 replicated
```

`bench/admission.py check --form coverage --c 0.761` over dumps from this binary
(`$O/replay-check.txt`). Under the shipped rule that check proved the replay was `query::ask`;
here, on dumps from the binary carrying the new rule, it proves the binary is the replay.
**Pass.**

## The offline replay predicted this exactly

The 2026-09-05 replay scored `coverage` at this constant over the ratio's dumps and read: dense
`40/40 15/30 12/12`, lexical `39/40 15/30 12/12`, developer 38 and 38, held-out 103 → 104 and
109 → 112. The binary reads the same four arms, the same two totals and the same two held-out
counts, gained and lost case for case. The replay was a faithful model of a binary that did not
exist when it ran.

## What the form actually discriminates on, and the asymmetry in it

Two unit tests on the two-node test graph had to change, and why they changed is a property of
the form rather than a detail of the tests. On that graph the questions list covers 0.696 of any
query and the passages 0.858, whatever the raw scores are — for «штраф считается», which the
ratio refused at 0.41, and for «штраф отмену», which it admitted at 1.62, alike. The ratio's
discrimination there was magnitude; coverage removes magnitude on purpose, and what is left on a
graph that small is a constant.

So the form's discrimination lives entirely in `attainable`, and `attainable` counts only the
query terms **present in that index**. A term the questions index never saw leaves its
denominator rather than lowering its score, which lifts its coverage. The form therefore rewards
an index for the narrowness of its vocabulary as well as for the quality of its match, and on the
test graph that is exactly what seats the list the ratio refused.

Whether that costs anything is what the fixture answers, and the answer is that it does not: one
paraphrase and three held-out questions gained in the lexical arm, one held-out question in the
dense arm, nothing lost that the clauses measure. But it is a real property of the shipped form,
it is not what the form was advertised to do, and the next attempt on this gate should read it
before assuming `attainable` is neutral.

## G13, regenerated

`$O/g13.md`, 27 anchors on the code-questions store `$G/bc-a1` — 13 `where` and 14 `cross` — each
row carrying the code list's rank for that anchor and its statistic under all four candidate
forms.

The table says something the gap's own framing did not. Ten of the thirteen `where` anchors sit
in the code list's **top ten**, four of them at rank 1; the other three sit deep, at 28, 38 and
286. The code list is not failing to find these files, ten times in thirteen. It never gets a seat on the plain path to say so, and
that is why `where` reads 0/9 in both arms whatever the admission does. Changing the form the
questions list is admitted under was never going to move it, which is why stage B — one seat for
the code list, under this same form — is the only lever left on G13.

Stage B was not run. It is a separate change needing a rule of its own, and the price is already
known: A4 measured one seat for a fourth list at six held-out questions lost in each arm, none
gained, p = 0.031.

## What this closes and what it does not

**G12 closes.** The admission no longer compares raw BM25 scores across two indices. It compares
two dimensionless coverages, and the binary and the replay agree on every one of 1,084 queries.

**G8 closes as stated, with a caveat named above.** The constant is scale-free in the sense the
gap asked for — it does not move with the two indices' mean lengths or vocabulary sizes. It is
not free of the store in every sense: `attainable`'s denominator depends on which query terms each
index happens to hold, so a store whose questions use a much narrower vocabulary than its passages
will admit its questions list more readily. The gap's own test — a store with short bodies, full
enrichment or five questions per node lands somewhere else and nothing in the code would say so —
is answered for scale and not for vocabulary.

**G13 stays open, and the table names its cause more precisely than the gap did.** The code
list ranks ten of the thirteen `where` anchors in its top ten; the admission was never what
kept them out of the answer, a seat was.

The ratio's history is kept: `QUESTIONS_GATE` is now 0.761 and a coverage constant, the window
(0.802, 0.866] belonged to the number it replaced, and `REPOGRAPH_QUESTIONS_GATE` still overrides
it for a measurement, with `0` still the ungated fusion ADR-001 Amendment 6 read against.

## The weakest joint, restated

The rule above was written knowing what the 2026-09-05 replay had already scored. It is a
decision taken with prior evidence, not the blind prediction the 0.5.0 clause was, and it is
weaker evidence for that reason. What keeps it honest: the rule was committed before the form was
implemented, the clauses were not touched once numbers existed, the measurements are of the
binary rather than of a replay, and the held-out set was read before either suite. A reader who
discounts this result relative to a properly blind one is reading it correctly.
