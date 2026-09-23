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
