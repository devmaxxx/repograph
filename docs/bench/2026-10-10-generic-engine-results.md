# Generic code engine, step by step

The record §5.6 of `docs/superpowers/plans/2026-10-10-generic-code-engine.md` asks for: one section
per step, written before the step merges and never edited afterwards.

Kit: `~/bench/engine-2026-10-10`. It holds `corpus` and `store-base`, `bin/` with one release
binary per step, `fixture-base` (the fixture store the step-0 binary wrote), and the transcripts
under `log/`. The fixture is the pinned `~/bench/beauty-crm-502e8a6d`. Every store it reads is
written in `~/bench/beauty-crm-test` after `bench/probe/reset.sh`.

## Step 0 · Engine and front

The base readings are recorded in the plan's "Step 0 as built" note. The fixture base was taken
at the start of step 1 with the step-0 binary, `bin/repograph-step0`:

- bench (`--no-dense`, `intfloat/multilingual-e5-small`): keyword 39/40, paraphrase 7/30,
  code 12/12, p90 238 tok, green. The transcript is `log/bench-step0.txt`, and the row is in
  `bench/history/runs.jsonl`.
- `fixture-base`: 1,409 files read, 12,999 nodes, 41,113 edges.

## Step 1 · Shell

Old: `bin/repograph-step0`. New: `bin/repograph-step1`. Shell's old reader is deleted, so the old
graph cannot come from `REPOGRAPH_LEGACY`.

- **Regressions:** R1–R9 = 0 on the kit corpus (`old-1` vs `new-1`), on the scratchpad corpus
  (`store-base` vs `store-step1`), and on the fixture (`fixture-base` vs the port's store). The
  readings come from `langdiff.py --lang sh,bash`.
- **Accepted:** none. On all three, `graph.json` and `headers.json` are byte-identical to the old
  store, so every other language is untouched too.
- **Bounded kinds:** none changed. Calls (192 on the corpus) and References (11) are the same
  edges, so no P/R or hand read is needed.
- **Blast:** skipped under §5.5, because the graph is identical.
- **Bench** (`--no-dense`, e5-small): keyword 39/40, paraphrase 7/30, code 12/12, p90 238 tok,
  green, zero flips. The transcript is `log/bench-step1.txt`, identical case by case to step 0's.
- **Build time:** the corpus build takes 1.22 s on step 1 against 1.26 s on step 0 (median of
  three). The deepest file, `beauty-crm/apps/booking/src/account/visits.tsx` (bracket depth 17),
  takes 0.01 s on both.
- **Gate:** `cargo test --all-targets` shows 0 failed, `cargo clippy --all-targets -- -D warnings`
  is clean, `cargo tree --duplicates` lists no tree-sitter crate, and `src/code/shell/` is gone.
