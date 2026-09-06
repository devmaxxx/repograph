# The coverage admission — the rule, written before the shipped form was run

**Status:** pre-registered 2026-09-06. Nothing in this document was measured against a binary that
carries the coverage admission; every number quoted below is the *baseline* the shipped ratio reads
today, copied from a named file, and every number this rule will be judged on is still unmeasured
when it is written.

**Honesty clause, first, because it is the weakest joint in this document.** L1 of the 0.5.0 gap
plan derived three scale-free admission forms and scored all three offline. Their numbers were
known when this rule was written. So this rule is a *decision made with prior evidence in hand*,
not the blind prediction L1's own rule was, and it is weaker evidence for that reason. What keeps
it honest is that it is written down, in full, before the shipped form is run even once, and that
the runs which judge it are runs of the binary rather than replays of a dump. A reader who thinks
the bar below was set where it is because the offline replay already cleared it is reading the
situation correctly, and should weigh the result accordingly.

## What changes

The questions list joins the fusion on a statistic of its own list rather than on a ratio between
two lists. Today `query::ask` admits the questions list when its best BM25 score is at least
`QUESTIONS_GATE = 0.85` of the passage list's best — a comparison between two indices whose scores
are not in the same unit, which is gap **G12**, and a constant fitted to one store, which is gap
**G8**. The coverage form keeps the comparison between the two lists and changes what is compared.
Each list first answers a question about itself: how much of this query did my best document
actually cover, `best / attainable`, where `attainable(query)` is the sum of idf over the unique
query terms present in that index — BM25 at tf = 1 and mean document length. That statistic is
dimensionless, so the two lists' values are in the same unit whatever their raw scores are worth,
and the admission is their ratio against `c`. The questions list is seated when

    (best_q / attainable_q) / (best_p / attainable_p)  >=  c

with the ends spelled out as `bench/admission.py` spells them: an empty questions list or one whose
best is not positive is never seated, and a passage list whose own statistic is zero is never a bar.
This is what closes G12 — the two numbers compared are now coverages rather than raw BM25 scores
from two indices that were never in the same unit — and what makes the constant scale-free, which
is G8.

**The arithmetic is part of the specification.** The constant was derived by the replay, which
computes both statistics and their ratio in double precision over `f32` inputs. The binary must do
the same: widen the scores and the attainable sums to `f64` for the gate. Computing the gate in
`f32` would be a different function at the boundary, and clause 5 below would be judging the binary
against a replay of something else.

Scope is stage A only: the questions list. Stage B, a seat for the code list under the same form,
is a separate lever with a rule of its own and is not run here.

## The constant

`c = 0.761`, the crossover of the held-out set's 400 questions in the `--no-dense` arm, derived
before either suite was opened (`/Users/max/bench/gaps-2026-09-05/admission-verdict.txt`). It is
not re-derived, not rounded and not tuned. If the rule below fails, the form fails; the constant
does not move to save it.

## The baseline this is judged against

The shipped ratio, measured on the pinned fixture `beauty-crm-502e8a6d` at the 0.5.0 head, from
`/Users/max/bench/gaps-2026-09-05/base-bench-*.txt` and the held-out replays in
`admission-verdict.txt`:

| arm | recorded (82) | developer (60) | held-out (400) |
|---|---|---|---|
| dense, enriched | keyword 40/40, paraphrase 15/30, code 12/12, p90 220 | 36 | 103 |
| lexical, enriched | keyword 39/40, paraphrase 14/30, code 12/12, p90 215 | 37 | 109 |

## The rule

The coverage admission ships if, and only if, **all five clauses hold on the binary that carries
it**. Any one failing ends the lever, the ratio stays, and the failure is recorded as such.

1. **No floor breaks.** Every floor `bench` grades against holds in all four fixture arms, and
   p90 stays at or under 230 tokens. The floors are the table in `src/bench.rs`; this clause reads
   whatever that table says at the time of the run, not a number copied here.
2. **No arm loses ground on the recorded suite.** In each of the two enriched arms, keyword,
   paraphrase and code are each at least the baseline count in the table above. Higher is allowed
   and is not itself a pass.
3. **No arm loses ground on the developer suite.** Dense total at least 36, lexical at least 37,
   and `where` no lower than 0/9 in either arm.
4. **No significant held-out loss.** Paired exact McNemar against the recorded baseline, both arms,
   on the same 400-question set built at seed 20260905. A loss is disqualifying at p < 0.05; a
   gain need not be significant, and a wash passes. The comparison is run per arm and both must
   hold.
5. **The replay still agrees with the binary.** `bench/admission.py` under `--form coverage` and
   this constant reproduces the binary's answers on all three case sets in both arms — recorded 82,
   developer 60, held-out 400 — so the offline tool stays a description of the shipped code rather
   than of a code path that no longer exists.

## What this rule deliberately does not require

**Byte identity.** Rule 1 of the perf work required four dumps and four bench lines to be
unchanged, because those changes were meant to move nothing. This change is meant to move answers:
that is its entire purpose. Requiring byte identity here would be requiring the lever to do
nothing. The dumps and bench lines are therefore re-recorded as the new baseline after the fact,
and clause 5 is what stands in for the identity check — the replay and the binary must still be
the same function.

**Exact reproduction of the four arms.** This is the clause L1 wrote, the clause all three forms
failed, and the reason the coverage form was not shipped in 0.5.0 despite scoring better than the
gate that ships. It was the right clause for a change whose stated purpose was to re-express an
existing decision in a better unit without altering it. It is the wrong clause for a change made
because the re-expression is *better*, which is what the evidence then showed. Clauses 2 and 3
replace it with the requirement that nothing gets worse, which is what "exactly" was standing in
for.

## What is recorded either way

The four fixture arms, both developer arms, both held-out comparisons with their McNemar p-values,
the replay agreement counts, and the G13 `where` table regenerated under the shipped form. If the
lever fails a clause, the same list is recorded, the ratio stays, and G8 and G12 stay open with one
more form named as measured and rejected.
