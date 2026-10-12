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

## Step 2 · Bicep

Old: `bin/repograph-step1c`, the step-1 binary rebuilt on the clean commit `00022dd` (below). New:
`bin/repograph-step2`. Bicep's old reader is deleted, so the old graph cannot come from
`REPOGRAPH_LEGACY`.

- **Step 1 re-recorded.** The step-1 fixture row was taken from an uncommitted tree on `c7e8b68`
  (`tool_dirty: true`). It was re-run on `00022dd`, which is step 1 plus its two deferred review
  fixes, from a clean tree. The result is keyword 39/40, paraphrase 7/30, code 12/12, p90 238 tok,
  green, and every case reads as before. The transcript is `log/bench-step1c.txt`, the new row
  supersedes the dirty one, and the old row stays. On that binary the kit corpus graph equals
  `new-1` byte for byte, and the fixture graph equals `fixture-base`.
- **Regressions:** R1–R9 = 0 from `langdiff.py --lang bicep` on three inputs: the kit corpus
  (`new-1c` vs `new-2`), the scratchpad corpus (`store-base` vs `store-step2`) and the fixture
  (`fixture-base` vs the port's store).
- **Accepted:** none. On all three, `graph.json` and `headers.json` are byte-identical to the old
  store, so every other language is untouched too.
- **Bounded kinds:** none changed. Bicep reads 1,379 symbols, 1,379 Declares, 66 Imports and 4,344
  References on the corpus, each identical, so no hand read is needed.
- **§3.3 decision for References:** the default resolution, measured with the same query, read
  4,344 of 4,344 with 0 added (P = R = 1). It fails five Bicep cases, so References is owned by
  `exceptions/symbolic_names.rs`. The reading is in `log/langdiff-step2-default.txt`.
- **Blast:** skipped under §5.5, because the graph is identical.
- **Bench:** not run. The fixture holds no Bicep file (§5.4), and its graph is byte-identical.
- **Gate:** `cargo test --all-targets` shows 0 failed, with no assertion edited, and
  `cargo clippy --all-targets -- -D warnings` is clean. `cargo tree --duplicates --depth 0` lists
  no tree-sitter crate, and `src/code/bicep/` is gone.

## Step 3 · HCL and Terraform

Old: `bin/repograph-step2`. New: `bin/repograph-step3`, built on the clean commit `23748b5`. HCL's
old reader is deleted, so the old graph cannot come from `REPOGRAPH_LEGACY`.

- **Step-2 review fixes** landed first (`e9caf84`). On both corpora, `graph.json` and
  `headers.json` are byte-identical to `new-2` and `store-step2`.
- **Regressions:** R1–R9 = 0 from `langdiff.py --lang hcl,tf` on three inputs: the kit corpus
  (`new-2` vs `new-3`), the scratchpad corpus (`store-base` vs `store-step3`) and the fixture
  (`fixture-base` vs the port's store in `~/bench/beauty-crm-test`). The readings are in
  `log/langdiff-step3-kit.txt` and `log/langdiff-step3-scratch.txt`.
- **Accepted:** none. On all three inputs, `graph.json` and `headers.json` are byte-identical to
  the old store, so every other language is untouched too.
- **The Terraform split** moves nothing in the graph. No node or edge carries a `Lang`, and ids,
  family and headers are unchanged.
- **Bounded kinds:** none changed. On the corpus, Terraform reads 6 `.tf` files, 62 symbols, 62
  Declares and 66 References, with 0 Imports, each identical; the corpus has no `.hcl`. On the
  fixture's `.tf` files it reads 34 symbols, 37 References and 12 cited ids, also identical. No
  hand read is needed.
- **§3.3 decision for References:**
  - Terraform: the default resolution, measured with the same query, reads 0 of 66 (R = 0), so
    References is owned by `exceptions/traversal_addresses.rs`. The reading is in
    `log/langdiff-step3-default.txt`.
  - HCL: the corpus is thin (0 edges), so the cases decide, and the bake rule is
    `exceptions/target_or_group.rs`.
- **Adversarial file:** 47 nodes and 79 edges, built for this step. Old and new differ by one edge.
  A list inside a function call (`inherits = concat(["base"], …)`) is not read; the old reader
  took the first list anywhere in the value.
- **Blast:** `beauty-crm-hcl` was skipped under §5.5, because the graph is identical.
- **Bench:** run on `23748b5` from a clean tree after `reset.sh`. The result is keyword 39/40,
  paraphrase 7/30, code 12/12, p90 238 tok, green, with zero case flips against
  `log/bench-step1c.txt`. The transcript is `log/bench-step3.txt`, and the row is appended to
  `bench/history/runs.jsonl`.
- **Gate:** `cargo test --all-targets` shows 0 failed, and `cargo clippy --all-targets -- -D
  warnings` is clean. One assertion was edited, as the split forces: the grammar case now expects
  `main.tf` to be `Lang::Terraform`. `cargo tree --duplicates --depth 0` lists no tree-sitter
  crate, and `src/code/hcl/` is gone.

## Step 4 · Swift

Old: `bin/repograph-step3`. New: `bin/repograph-step4`. Swift's old reader is deleted, so the old
graph cannot come from `REPOGRAPH_LEGACY`.

- **Step-3 review fixes** landed first (`2d3be08`). On both corpora and the fixture, `graph.json` and
  `headers.json` are byte-identical to `new-3`, `store-step3` and `fixture-base`
  (`bin/repograph-step4a`).
- **Regressions:** R1–R9 = 0 from `langdiff.py --lang swift` on two inputs: the kit corpus (`new-3`
  vs `new-4`) and the scratchpad corpus (`store-base` vs `store-step4`). The readings are in
  `log/langdiff-step4-kit.txt` and `log/langdiff-step4-scratch.txt`. The fixture holds no `.swift`.
  Its store, rebuilt by the port in `~/bench/beauty-crm-test` after `reset.sh`, is byte-identical
  to `fixture-base`.
- **Accepted:** none. On both corpora and the fixture, `graph.json` and `headers.json` are
  byte-identical to the old store, so every other language is untouched, including by the two
  engine changes (one definition per name, a repeated id's own Declares).
- **Bounded kinds:** none changed. The corpus has 4 `.swift` files, with 11 symbols, 11 Declares and
  0 Extends, each identical. No hand read is needed.
- **§3.3 decision for Extends:** the corpus is thin (0 edges), so the cases decide. Under the default,
  an extension's clause starts at the file, which owns the site, and not at the extended type, so
  `an_inheritance_clause_is_an_edge_only_to_a_name_the_repository_declares` and the nested-extension
  case fail. Extends is therefore owned by `exceptions/inheritance_clauses.rs`, which keeps the old
  rule.
- **Adversarial repository:** 3 files, 61 symbols and 81 edges, kept in `adversarial-step4/`. Old and
  new differ by 5 nodes and 5 Declares, from two constructs (`log/langdiff-step4-adversarial.txt`):
  - `extension [Int]` names its member `[Int].arr`, where the old reader wrote `.arr`.
  - The members of a type nested inside four others are not declared, because a query cannot
    recurse.

  All 18 of its Extends match, as do every span and every body.
- **Blast:** `pawfice` was skipped under §5.5, because the graph is identical.
- **Bench:** not scheduled at this step. The fixture holds no Swift, and its graph is byte-identical.
- **Gate:** `cargo test --all-targets` shows 1,378 passed, 0 failed and 8 ignored, with no assertion
  edited. `cargo clippy --all-targets -- -D warnings` is clean. `cargo tree --duplicates --depth 0`
  lists no tree-sitter crate, and `src/code/swift/` is gone.

## Exception sweep · 2026-10-12

Old: today's stores (`new-4`, `store-step4` and `fixture-base`). New: `bin/repograph-sweep`, built
on the clean commit `b2521b5`. Every exception was measured under §3.3's rule of 2026-10-12, and the
table, the accepted diff and the rewritten assertions are in the plan's §3.5.

- **Deleted:**
  - `symbolic_names`;
  - `traversal_addresses`;
  - `target_or_group`;
  - `inheritance_clauses`, `attribute_lines` and `type_paths`;
  - `module_reach`'s two import hooks.
- **Kept:**
  - `computed_paths`, without which Shell loses all 15 Imports, 26 of 192 Calls and all 11
    References;
  - `module_reach::reach`, without which Bicep loses 2,046 of 4,344 References.
- **Regressions:** `langdiff.py --lang sh,bash,bicep,hcl,tf,swift` on three inputs:
  - kit corpus: R3 = 1 (Swift), and every other class 0;
  - scratchpad corpus: R3 = 1 (Swift), and every other class 0;
  - fixture: 0.

  The readings are in `log/langdiff-sweep-{kit,scratch,fixture}.txt`.
- **Accepted:** the one R3 diff. AppDelegate's signature line is now its declaration line. Every edge
  and `headers.json` are identical on both corpora, and the fixture graph is byte-identical to
  `fixture-base`. The kit store is kept as `new-sweep`.
- **Bench** (`--no-dense`, e5-small): keyword 39/40, paraphrase 7/30, code 12/12, p90 238 tok, green.
  There are zero flips: the transcript `log/bench-sweep.txt` is identical line for line to
  `log/bench-step3.txt`. The row is appended to `bench/history/runs.jsonl`.
- **Gate:**
  - `cargo test --all-targets`: 1,381 passed, 0 failed, 8 ignored;
  - `cargo clippy --all-targets -- -D warnings`: clean;
  - `cargo tree --duplicates --depth 0`: no tree-sitter crate.

## Step 6 · SQL

Old: `bin/repograph-5`, with its stores (`new-5`, the scratchpad's step-5 store and `fixture-base`).
New: `bin/repograph-step6`, built on `f7358e1`. `bin/repograph-step6a`, built on the clean commit
`8c2e454` before the compile warm-up, ran the bench. Both write byte-identical graphs on both
corpora. SQL's old reader is deleted, so the old graph cannot come from `REPOGRAPH_LEGACY`.

- **Regressions:** from `langdiff.py --lang sql` on three inputs:
  - kit corpus (`new-5` → `new-6`): R1–R9 = 0;
  - scratchpad corpus (vs `store-base`): 0 for SQL, plus the Swift R3 the exception sweep accepted;
  - fixture: R1 = 3, R4 = 3 and R7 = 1.

  The readings are in `log/langdiff-step6-{kit,scratch,fixture}.txt`.
- **Accepted:** the fixture's seven are one construct. `CREATE FOREIGN TABLE` is now declared, with
  its two columns, in `packages/db/test/fixtures/d-foreign-table-grant.sql`, and `headers.json`
  gains the table's name. Every node and edge of any other language is identical on all three
  inputs.
- **Bounded kinds:** all are unchanged.
  - Kit corpus: Calls 145/145 and References 388/388, so P = R = 1.0.
  - Fixture: Calls 79/79 and References 828/828.
- **Exceptions:**
  - `identifier_case`: without it, R1 = R4 = 3,396 on the kit corpus and 3,100 on the fixture.
  - `attachments`: without it, References R is 0.55 on the kit corpus and 0.43 on the fixture.
- **Bench** (`--no-dense`, e5-small): keyword 39/40, paraphrase 7/30, code 12/12, p90 238 tok,
  green.
  - There are zero flips: the transcript `log/bench-step6.txt` is identical line for line to
    `log/bench-sweep.txt`.
  - The row is appended to `bench/history/runs.jsonl` at `8c2e454`.
- **Build time:** median of five, interleaved, on the corpus:
  - step 0: 1.211 s;
  - step 5: 1.391 s;
  - step 6: 1.436 s, which is 1.19×.

  Before the warm-up, step 6 read 1.62 s. Query compile on the Postgres grammar takes 43 ms for
  `decl` and 73 ms for `refs`.
- **Gate:**
  - `cargo test --all-targets`: 1,387 passed, 0 failed, 8 ignored;
  - `cargo clippy --all-targets -- -D warnings`: clean;
  - `cargo tree --duplicates --depth 0`: no tree-sitter crate;
  - `src/code/sql/` is gone.

## Step 7 · Dart

New: `bin/repograph-step7a`, built on `47e6c4f` (the `within` key alone), and `bin/repograph-step7`,
built on the Dart port. Dart's old reader is deleted. The step-6 binary `bin/repograph-step6b-head`
(`fd3de01`) is the old graph.

- **Regressions:** from `langdiff.py --lang dart` and `--lang sql` on three inputs. All three
  graphs are byte-identical to step 6's, so R1–R9 = 0 for both languages:
  - kit corpus (`new-6` → `new-7`);
  - scratchpad corpus (vs the step-6 store);
  - fixture, which holds no `.dart` file.

  The readings are in `log/langdiff-step7-{kit,scratch,fixture}.txt`. `step7a` was byte-identical
  on all three as well.
- **Bounded kinds:** on the kit corpus, Dart Calls 76/76, so P = R = 1.0. Imports are 18/18, with
  their used-name contexts. Declares are 239 (165 `export`). ReExports and Extends are 0 before and
  after.
- **Exception:** `library_scope`. Without its `resolve` hook, the kit corpus keeps Calls 6 of 76
  (R 0.08) and Imports 0 of 18.
- **Outside the inputs:** a probe repository diffed against step 6 shows two divergences:
  - a supertype from another file lost its Extends edge (−3) until `library_scope` resolved supertypes too;
  - a call in a later initializer of a multi-name declaration is owned by the first name (3 moved).
- **Bench:** not run. The fixture has no Dart, so no floor can move.
- **Build time:** median of five, interleaved, on the corpus:
  - step 0: 1.225 s;
  - step 7: 1.452 s, which is 1.19× (1.199× the recorded 1.211 s).

  Reading the top-level names through `Resolver::facts` cost +24 ms here and was replaced; a
  200-file Dart copy extracts in 483 ms, against the old reader's 487 ms.
- **Gate:**
  - `cargo test --all-targets`: 1,390 passed, 0 failed, 8 ignored;
  - `cargo clippy --all-targets -- -D warnings`: clean;
  - `cargo tree --duplicates --depth 0`: no tree-sitter crate;
  - `src/code/dart/` is gone.
