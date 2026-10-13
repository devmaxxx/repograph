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

## Step 8 · Python

New: `bin/repograph-step8a`, built on `8bac67f` (`within` placed by the outermost capture), and
`bin/repograph-step8`, built on the Python port. Python's old reader is deleted. The step-8a
binary still carried the old reader and is the old graph (`sweep/s8-base`); its graphs on the kit,
the scratchpad corpus and the fixture are byte-identical to step 7's.

- **Regressions:** from `langdiff.py --lang py` on three inputs.
  - Kit corpus (`new-7` → `new-8`): R3 = 12, every other class 0. No other language moved.
  - Scratchpad corpus (vs `store-base`): py R3 = 12, plus the accepted Swift R3 = 1 and SQL
    body-above 11.
  - Fixture: graph and headers byte-identical.

  The 12 are docstrings over 600 characters: the old body capped docstring and signature
  together and cut the signature off; the new one keeps it. The readings are in
  `log/langdiff-step8-{kit,scratch,fixture}.txt`.
- **Bounded kinds:** on the kit corpus, P = R = 1.000 for each.
  - Calls: 841 of 841.
  - Imports: 28 of 28, with 15 by name and 13 `*`.
  - Declares: 1,123, of which 647 are `export`.
  - Extends: 5 of 5.
  - DecoratedBy and ReExports: 0 before and after.
- **Exception:** `module_paths`.
  - Without its `resolve` hook, the kit corpus keeps Calls at 445 of 841 (R 0.529) and Imports at
    0 of 28.
  - Without the attribute-type patterns, all four kinds stay at 1.000. They are kept for
    `tests/python.rs`.
  - The readings are in `log/pr-step8-{nohook,notypes}.txt`.
- **Bench:** not run. The fixture's graph and headers are byte-identical to step 7's, so no floor
  can move.
- **Build time:** median of five, interleaved, on the corpus. The readings are in
  `log/buildtime-step8.txt`.
  - step 0: 1.206 s;
  - step 8: 1.412 s, which is 1.17× (1.166× the recorded 1.211 s).
- **Gate:**
  - `cargo test --all-targets`: 1,391 passed, 0 failed, 8 ignored;
  - `cargo clippy --all-targets -- -D warnings`: clean;
  - `cargo tree --duplicates --depth 0`: no tree-sitter crate;
  - `src/code/python/` is gone.

## Step 9 · Rust

Written after step 10, since the re-read below found this section missing. The readings come from
the plan's "Step 9 as built" note and the kit's stores. The engine's additions are `145ab3e` and the
port is `e1a4c68`; their binaries are `bin/repograph-step9e` and `bin/repograph-step9`. Rust's old
reader is deleted.

- **Regressions:** from `langdiff.py --lang rs` on three inputs.
  - Kit corpus (`new-8p` → `new-9`): R3 = 3 and body-above 7, every other class 0. No other
    language moved.
  - Scratchpad corpus (vs `store-base`): rs R3 = 3 and body-above 7, plus the accepted py R3 = 12,
    Swift R3 = 1 and SQL body-above 11.
  - Fixture: no `.rs` file; graph and headers byte-identical to step 8's.

  The 10 are `///` docs: `Stripped` drops a blank `///` line between paragraphs, and where doc and
  signature reach the 600-character cap together, the cut falls a few characters later.
- **Bounded and strict kinds:** on the kit corpus, P = R = 1.000 for each.
  - Calls: 1,068 of 1,068.
  - Imports: 3 of 3. ReExports: 2 of 2.
  - Declares: 1,107, of which 200 are `export`.
- **Exception:** `crate_paths`, with `crate_paths/uses.rs` and `crate_paths/calls.rs`.
  - Without it, the kit corpus keeps Calls at 645 of 1,068 (P 1.000, R 0.604), and no Imports or
    ReExports edge.
  - The plan's note gives that as "Imports and ReExports 0 of 4", where the gate reads Imports 3
    and ReExports 2. Both are right: the five edges join four file pairs. `src/code/dart/mod.rs`
    imports `src/code/dart/declarations.rs` twice, once for `named` and once for `text`, and the
    edge's context keeps them apart. Counted as edges, it is 0 of 5.
- **Bench:** not run. The fixture's graph and headers are byte-identical to step 8's.
- **Build time:** median of five, interleaved, on the corpus (`log/buildtime-step9.txt`).
  - step 0: 1.203 s;
  - step 9: 1.420 s, which is 1.18×. Before the matcher's start depth it read 1.21×.
- **After the re-read:** `135e627` keeps an `impl` for a path off the file's bare type and merges
  every `cfg` twin. The kit graph is unchanged by it: step 10's `--lang kt` reading finds every
  other language byte-identical to `new-9`.

## Re-read after step 9

Plan §6.1 asks for this re-read before step 10 starts (decision 3). Every number comes from one of:

- the plan's "as built" notes and the sections above;
- `bench/history/runs.jsonl`;
- `git diff` and `git log` from the merge-base `7286a56` to `e1a4c68`;
- the logs in `~/bench/engine-2026-10-10/log/`.

Steps 5 and 9 have no section above, so their readings come from the plan's notes.

### Lines per step

The counts come from git, `--no-renames`, with each step's review fixes in its group. The columns
are:

- **old reader:** the language's directory outside its cases;
- **exceptions:** `src/code/exceptions/`, net;
- **engine and spec:** the rest of `src/` outside cases, net. That is the engine, `spec.rs`,
  `queries.rs`, the front and `lang.rs`.

The plan column is §6.1's "Query ~ / Rust − / hook ~" for each step.

| step | plan: query / Rust − / hook | queries | old reader | exceptions | engine and spec |
|---|---|---|---|---|---|
| 0 · engine and front | +1,200 Rust, −200, +250 Python | 0 | +7 (edits) | 0 | +1,383 (+1,719 / −336); +274 Python |
| 1 · Shell | 50 / −250 / 150 | +36 | −365 | +250 | +131 |
| 2 · Bicep | 70 / −250 / — | +137 | −323 | +102 | +52 |
| 3 · HCL, Terraform | 90 / −250 / — | +128 | −356 | +161 | −34 |
| 4 · Swift | 60 / −180 / — | +80 | −196 | +111 | +41 |
| sweep · 2026-10-12 | — | +77 | 0 | −351 | +37 |
| 5 · GraphQL | 60 / −300 / — | +99 | −359 | 0 | +60 |
| 6 · SQL | 130 / −380 / — | +168 | −448 | +37 | +59 |
| 7 · Dart | 100 / −350 / 330 | +82 | −688 | +409 | +54 |
| 8 · Python | 110 / −450 / 450 | +166 | −913 | +305 | +64 |
| 9 · Rust | 150 / −600 / 700 | +162 | −1,297 | +925 | +154 |
| **total, steps 0–9** | **820 / −3,010 / 1,630** (steps 1–9) | **1,135** | **−4,938** | **1,949** | **+2,001** |

- **The total is measured against the merge-base.** The per-step old-reader counts include the lines
  step 0 edited, so the column sums to 4,945 before step 0's +7.
- **Where the plan's notes differ, they count the language alone.**
  - Step 6's queries are 160 there. Git also counts a later fix's 8 lines.
  - Step 7's are 98 Dart lines. Git nets in the 31 SQL lines the `within` key removed.
  - Step 8's are 138. Git adds the review fix's 28 lines.
  - Step 9's old reader is −1,157 there. Git also counts the 140-line test module of `crates.rs`,
    which moved to the cases. The note's net for Rust is −233.
- **Spec rows at HEAD** take 216 of `spec.rs`'s 548 lines: Shell 25, Bicep 17, HCL 16, Terraform
  17, Swift 17, GraphQL 21, SQL 24, Dart 25, Python 26, Rust 28.
- **Kept against removed.** Each step's exception lines over its old-reader lines read: Shell 0.68,
  SQL 0.08, Dart 0.59, Python 0.33 and Rust 0.71. After the sweep, Bicep, HCL, Terraform, Swift and
  GraphQL hold 22 lines between them, all of it `module_reach`.
- **The four predicted hooks against HEAD:**
  - Shell: ~150 predicted, 245 kept;
  - Dart: ~330 predicted, 408 kept;
  - Python: ~450 predicted, 304 kept;
  - Rust: ~700 predicted, 924 kept.
- **Against §6's totals for steps 0–14.** §6 expects about 6,000 Rust lines to leave, about 1,300
  query lines to arrive and about 3,500 hook lines to remain. After step 9:
  - 4,938 old-reader lines are gone;
  - the queries hold 1,135 lines, 87% of the total;
  - the exceptions hold 1,949 lines, 56%.

  Steps 10–13 plan another 590 query lines. They also plan ~1,700 hook lines for C# (~1,100) and
  TypeScript (~600) alone, and give no figure for the JVM hook.

### Exceptions at HEAD

`src/code/exceptions/` holds 10 files and 1,949 lines (`e1a4c68`). In the table, "without it" is the
reading with the exception removed, the engine's default plus the queries, from the as-built notes.
"Kit" is the kit corpus.

| file | lines | rule | languages | without it |
|---|---|---|---|---|
| `mod.rs` | 11 | the module list | — | — |
| `computed_paths.rs` | 245 | a path a script computes from its own location, and a run by path; `collect`, `state`, `import`, `resolve` | Shell | Imports 0 of 15, Calls 166 of 192 (R 0.86), References 0 of 11 (kit) |
| `module_reach.rs` | 22 | a module call reaches every top-level name of its target; `resolve` | Bicep, Terraform | Bicep References 2,298 of 4,344 (R 0.53, kit) |
| `identifier_case.rs` | 15 | an unquoted name folds to lower case, and a quoted one loses its quotes; `fold` | SQL | every SQL id moves: R1 = R4 = 3,396 and R7 = 1 (kit); R1 = R4 = 3,100 (fixture) |
| `attachments.rs` | 20 | a table a file only alters references every other file that declares it; `resolve` | SQL | References R 0.55 (174 of 388 lost, kit); R 0.43 (476 of 828 lost, fixture) |
| `library_scope.rs` | 408 | Dart's library model: parts, `part of`, `show`/`hide`/prefix, export chains, pubspec packages; `collect`, `resolve`, which owns Calls and Extends | Dart | Calls 6 of 76 (R 0.08), Imports 0 of 18 (kit) |
| `module_paths.rs` | 304 | a dotted import placed through package roots, with one `__init__.py` re-export followed; calls, bases and decorators through the bindings; `resolve`, which owns Calls, Extends and DecoratedBy | Python | Calls 445 of 841 (R 0.529), Imports 0 of 28 (kit). Its attribute types move no edge on any input, and stay for `tests/python.rs` |
| `crate_paths.rs`, `crate_paths/uses.rs`, `crate_paths/calls.rs` | 393 + 193 + 338 | the crate module tree, `use` flattening, calls, macros, `impl` ties and attributes; `collect`, `resolve` | Rust | Calls 645 of 1,068 (R 0.604), Imports and ReExports "0 of 4" (kit) |

### Build time against the 1.20× bar

The corpus build is timed as a median of five runs, interleaved with step 0's binary, except where
noted. The bar is 1.20 × 1.211 s = 1.453 s.

| step | step 0, same run | step | same run | against 1.211 s | source |
|---|---|---|---|---|---|
| 0 (base) | — | 1.237 s | 0.92× the parent's 1.349 s | — | plan, step 0 (median of three) |
| 1 | 1.26 s | 1.22 s | 0.97× | — | step 1 above (median of three) |
| 2, 3, 4, sweep | — | — | not taken | — | — |
| 5 | 1.211 s | 1.391 s | 1.15× | 1.15× | step 6 above |
| 6, before the warm-up | 1.27 s | 1.62 s | 1.28× | — | plan, step 6 |
| 6 | 1.211 s | 1.436 s | 1.19× | 1.19× | step 6 above |
| 7a (`within` alone) | 1.204 s | 1.424 s | 1.18× | 1.18× | plan, step 7 |
| 7 | 1.225 s | 1.452 s | 1.19× | 1.199× | step 7 above |
| 8 | 1.206 s | 1.412 s | 1.17× | 1.166× | `log/buildtime-step8.txt` |
| 9, before the start depth | — | — | 1.21× | — | plan, step 9 |
| 9 | 1.203 s | 1.420 s | 1.18× | 1.173× | `log/buildtime-step9.txt` |

- **Step 9 sits 24 ms under the bar in its own run** (1.444 s), and 33 ms under it against 1.211 s.
- **Two ports crossed 1.20×.** SQL crossed it until `queries::warm` compiled queries beside the read.
  Rust crossed it until the matcher's start depth kept rooted queries out of function bodies.
- **Every step from 6 on lands between 1.17× and 1.19×.**

### P and R for every bounded kind so far

The kit and the scratchpad corpus hold the same files, and each reading is the same on both.
"Same graph" means the fixture's graph is byte-identical to the old one. P = R = 1.0 in every row
below. Imports and ReExports are strict (§3.3), not bounded, and are listed for completeness.

| lang | kind | kit corpus | fixture | owner |
|---|---|---|---|---|
| Shell | Calls · References · Imports | 192/192 · 11/11 · 15/15 | same graph | default, plus `computed_paths` |
| Bicep | References · Imports | 4,344/4,344 · 66/66 | none | default, plus `module_reach` |
| Terraform | References · Imports | 66/66 · 0/0 | 37/37 · 0 | default |
| HCL | References | 0/0 | 0/0 | default |
| Swift | Extends | 0/0 | none | default (thin) |
| GraphQL | Calls | 69/69 | same graph | default |
| SQL | Calls · References | 145/145 · 388/388 | 79/79 · 828/828 | default, plus `identifier_case` and `attachments` |
| Dart | Calls · Imports · Extends · ReExports | 76/76 · 18/18 · 0/0 · 0/0 | no `.dart` | `library_scope` |
| Python | Calls · Imports · Extends · DecoratedBy · ReExports | 841/841 · 28/28 · 5/5 · 0/0 · 0/0 | same graph | `module_paths` |
| Rust | Calls · Imports · ReExports | 1,068/1,068 · 3/3 · 2/2 | no `.rs` | `crate_paths` |

**Accepted diffs.** None moves an edge.

| diff | inputs | step | why |
|---|---|---|---|
| Swift R3 = 1 | kit, scratchpad | sweep | AppDelegate's signature is now its declaration line. |
| SQL R1 = 3, R4 = 3, R7 = 1 | fixture | 6 | `CREATE FOREIGN TABLE` is declared with its two columns, and `headers.json` gains its name. |
| SQL body-above 11 and 15 | kit and scratchpad (11), fixture (15) | 6 | `Stripped` drops a doc's blank lines, so the 600-character cap ends later. |
| Python R3 = 12 | kit, scratchpad | 8 | A docstring over 600 characters no longer cuts the signature off. |
| Rust R3 = 3, body-above 7 | kit, scratchpad | 9 | `Stripped` drops a blank `///` line, so the shared cap falls a few characters later. |

The adversarial repositories differ in more places. None is a gate input, and each is listed under
open items below.

### §3.4's predictions against what was built

| lang | predicted | at HEAD | predicted, not needed | needed, not predicted |
|---|---|---|---|---|
| Shell | `import`, `collect`, `widen` | `collect`, `state`, `import`, `resolve` | `widen` (`engine::widen` covers it) | `resolve`, for the runs by path |
| Bicep | `resolve` (reach) | `resolve` (reach) | — | — |
| HCL | `widen`; `resolve` only if `Directory` misses | none | `widen`, `resolve` | — |
| Terraform | as HCL | `resolve` (reach) | `widen` | the reach shared with Bicep |
| Swift | none | none | — | — |
| GraphQL | none | `header`, pointing at `default_header` | — | `header`, which keeps `headers.json` (R7) |
| SQL | `fold` | `fold`, `resolve`, `header` (top only) | — | `resolve` (`attachments`) and `header` |
| Dart | `collect`, `import`, `resolve` owning Calls and Extends | `collect`, `state`, `manifest`, `resolve` owning Calls and Extends | `import`: the exception writes Imports itself | — |
| Python | `collect` (packages, `__all__`), `import`, `resolve` owning Calls | `state`, `manifest`, `resolve` owning Calls, Extends and DecoratedBy | `collect`, `import` | the ownership of Extends and DecoratedBy |
| Rust | `collect`, `import`, `resolve` owning Calls | `collect`, `state`, `manifest`, `resolve`, owning nothing: the queries capture no call site | `import` | — |

- **Predicted but not needed:**
  - `widen` for Shell and Terraform;
  - `import` for Dart, Python and Rust;
  - `collect` for Python;
  - `resolve` for HCL.
- **Needed but not predicted:**
  - Shell's `resolve`;
  - GraphQL's and SQL's `header` pointers;
  - SQL's `attachments`;
  - Terraform's share of the reach;
  - Python's ownership of Extends and DecoratedBy.
- **Built, then deleted.** Steps 2–4 built six exceptions the prediction did not call for: Bicep's
  `symbolic_names`, the Bicep and Terraform import hooks, `traversal_addresses`, `target_or_group`,
  and Swift's three. The sweep measured them against real code and deleted them, taking the folder
  from 624 lines to 273.

### Bench rows

Each run is `--no-dense` with e5-small on the pinned fixture. Every row reads keyword 39/40,
paraphrase 7/30, code 12/12 and p90 238 tok, and is green.

| commit | step | clean | flips against the base, over 82 cases |
|---|---|---|---|
| `c7e8b68` | 0 (base) | yes | — |
| `c7e8b68` | 1, superseded | no (`tool_dirty`) | 0 |
| `00022dd` | 1, re-recorded | yes | 0 |
| `23748b5` | 3 | yes | 0 |
| `b2521b5` | sweep | yes | 0 |
| `8c2e454` | 6 | yes | 0 |

- **The flips are counted from the rows' own `cases`.** The transcripts of steps 1c, 3, sweep and 6
  have one md5. Step 0's transcript differs from step 1c's as a file, while its 82 cases are equal.
- **Steps 2, 4, 5, 7, 8 and 9 ran no bench.** The fixture holds none of their files, or its graph was
  byte-identical.

### Open items carried forward

Review findings left unfixed, and divergences documented but not fixed:

- **SQL:**
  - `CREATE TABLE a.b.c` declares `a`.
  - `ALTER` above the same file's `CREATE` writes `alter` edges to every other creating file.
- **Dart:**
  - A call in a later initializer of a multi-name declaration is owned by the first name.
  - `Libraries::resolve` runs once per call site, uncached.
  - A Dart file is parsed twice.
- **Python:**
  - `__all__` is not read.
  - `global` and `nonlocal` do not unbind.
  - A nested function's locals do not shadow its outer function.
  - A class three deep is not declared.
  - A function-local relative import in `__init__.py` reads as a re-export.
- **Rust:**
  - A `mod` more than three deep declares nothing.
  - A `mod` in a function body declares its items as the file's.
  - A `cfg` twin inside an `impl` or a trait is not merged.
  - An `impl` for a pointer, or a reference to a reference, declares nothing.
  - A `///` block a blank line away is not doc.
  - A `# Panics` heading reads `Panics`.
- **Swift:**
  - A type nested more than four deep declares nothing inside it.
  - `extension [Int]` names `[Int].total`.
  - `extension Box<Int>.Inner: Proto` loses its Extends.
- **GraphQL:** comments above an operation are no longer its doc, and the description starts a
  span. Six smaller shifts are listed in the plan's step-5 note.
- **SQL, documented:**
  - `RENAME TO` drops the old name's index.
  - `SET SCHEMA` declares nothing.
  - A view keeps no `public` twin.
  - A citation inside a column cites from the column.
- **HCL:** a list inside a function call (`concat(["base"], …)`) is not read.
- **Shell:** the single-quoted `source` twin is an ignored `limitation_` case.
- **The record:**
  - Step 5 and step 9 have no section in this document.
  - Step 9's note gives Rust's no-hook Imports and ReExports as "0 of 4", where the gate reads 3
    and 2.
- **GRAMMAR moves at step 14.** Accepted classes read non-zero (decision 1), so the bump is due.

### For Max

- **The 1.20× bar is at risk.** Step 9 reads 1.18×, 24 ms under the bar in its own run, and SQL and
  Rust each crossed it until an engine fix. Kotlin, Java, C#, TypeScript and Vue hold 429 of the
  corpus's 776 code files, against Rust's 38.
- **Exceptions look certain for all four.** Every port that resolves across files kept one: without
  it, Calls R was 0.08–0.60 and Imports 0. Expect them for the JVM pair (one shared exception plus a
  `header` for arity and supertypes), for C# (project state, `global using`, partial types), and
  for TypeScript (tsconfig imports, `deco:` nodes), as §3.4 predicts.
- **The size plan is off, not the approach.** Exceptions hold 1,949 lines against the 1,630 §6.1
  predicted for steps 1–9,
  and Rust kept 925 for 1,297 removed. §6's "~3,500 hooks remain" will be passed. Re-estimate steps
  10–13 from the measured ratios before starting.
- **Nothing in the gates argues for stopping.** No bounded edge moved on any input, and the bench
  has zero flips. Step 10 is the first port since SQL that the fixture sees (126 `.kt` files), so the
  bench runs again there.

## Step 10 · Kotlin

New: `bin/repograph-step10`, built on `f064778`. Kotlin's old reader (`src/code/jvm/kotlin/`) is
deleted. What Kotlin and Java share moved from `src/code/jvm/mod.rs` to
`src/code/exceptions/package_scope.rs`; Java still reads through its own walk until step 11.

- **Regressions:** from `langdiff.py --lang kt` on three inputs. The readings are in
  `log/langdiff-step10-{kit,scratch,fixture}.txt`.
  - Kit corpus (`new-9` → `new-10`): every class 0, and every other language byte-identical.
  - Scratchpad corpus (vs `store-base`, 43 `.kt` files): kt 0. The R6 column reads py 12, rs 10,
    SQL 11 and Swift 1, which are the accepted py R3 = 12, rs R3 = 3 and body-above 7, SQL
    body-above 11 and Swift R3 = 1.
  - Fixture (vs `fixture-base`, 126 `.kt` files): kt 0. The R6 column reads SQL 21 and the store
    R7 = 1, which are step 6's accepted SQL R1 = 3, R4 = 3, body-above 15 and R7 = 1. Against
    `fixture-8p` every class is 0.
- **Bounded and strict kinds:** P = R = 1.000 for each, on both stores.

  | kind | kit | fixture |
  |---|---|---|
  | Declares (`export`) | 461 (371) | 1,777 (1,468) |
  | Imports | 59 | 379 |
  | Calls | 155 | 2,046 |
  | References | 73 | 852 |
  | Extends | 15 | 50 |
  | DecoratedBy, Implements | 0 | 0 |

- **Exception:** `package_scope`, with `package_scope/declared.rs` and
  `package_scope/receivers.rs`. Without its `resolve` hook, measured with the queries plus bare and
  receiver call sites, delegation supertypes and value-argument references under the engine's
  default resolve, declarations stay exact (R1, R2, R3 = 0) and the rest is lost:

  | kind | kit kept (P, R) | fixture kept (P, R) |
  |---|---|---|
  | Imports | 0 of 59 | 0 of 379 |
  | Calls | 18 of 155 (1.000, 0.116) | 471 of 2,046 (0.915, 0.230) |
  | References | 12 of 73 (1.000, 0.164) | 616 of 852 (0.905, 0.723) |
  | Extends | 0 of 15 (—, 0.000) | 4 of 50 (1.000, 0.080) |

  The hook stays. The readings are in `log/pr-step10-nohook-{kit,fixture}.txt`.
- **Outside the inputs:** step 9's binary against step 10's on the Kotlin of `beauty-crm`'s main
  clone and four of its worktrees, each with two or three files that differ from the fixture (630
  files, 10,135 nodes, 29,195 edges): every class 0 (`log/langdiff-step10-outside.txt`). A probe file
  found two divergences, listed in the plan's "Step 10 as built" note.
- **Bench:** on `f064778`, `--no-dense`, `intfloat/multilingual-e5-small`, the store written in
  `~/bench/beauty-crm-test` after `reset.sh`: keyword 39/40, paraphrase 7/30, code 12/12, p90 238
  tok, green. Zero flips against step 0 and step 6, across all 58 cases. The transcript is
  `log/bench-step10.txt`, and the row is in `bench/history/runs.jsonl`.
- **Build time:** median of five, interleaved, on the corpus (`log/buildtime-step10.txt`).
  - step 0: 1.276 s;
  - step 10: 1.509 s, which is 1.18×. A first run on the same code read 1.16×.
- **Gate:**
  - `cargo test --all-targets`: 1,396 passed, 0 failed;
  - `cargo clippy --all-targets -- -D warnings`: clean;
  - `cargo tree --duplicates --depth 0`: no tree-sitter crate;
  - `src/code/jvm/kotlin/` is gone.

## Step 11 · Java

New: `bin/repograph-step11`, built on `bb4aa59`. Java's old reader (`src/code/jvm/java/`) and
`src/code/jvm/` are deleted. Java's lookup lives in `src/code/exceptions/package_scope/` beside
Kotlin's, as `java.rs` and `java/calls.rs`.

- **Regressions:** from `langdiff.py --lang java,kt` on three inputs. The readings are in
  `log/langdiff-step11-{kit,scratch,fixture,fixture-vs-8p}.txt`.
  - Kit corpus (`new-10` → `new-11`): every class 0. `graph.json` and `headers.json` are
    byte-identical.
  - Scratchpad corpus (vs `store-base`): java and kt 0, and the store is byte-identical to step
    10's. The R6 column reads py 12, rs 10, SQL 11 and Swift 1, which are the accepted py R3 = 12,
    rs R3 = 3 and body-above 7, SQL body-above 11 and Swift R3 = 1.
  - Fixture (vs `fixture-base`): java and kt 0. The R6 column reads SQL 21 and the store R7 = 1,
    which are step 6's accepted diffs. Against `fixture-8p` the store is byte-identical.
- **Bounded and strict kinds:** P = R = 1.000 for each on the kit. The fixture holds no Java.

  | kind | kit |
  |---|---|
  | Declares (`export`) | 35 (31) |
  | Imports | 1 |
  | Calls | 14 |
  | References, Extends, DecoratedBy, Implements | 0 |

- **Exception:** `package_scope`, shared with Kotlin. Without the `resolve` hook, 49 of the 75 Java
  and JVM cases fail. The kit is too thin to decide (§3.3), so the cases decide, and the hook stays.
  - Gained by the shared code: `declared::sites`, `declared::facts_of`, and Java's static imports
    in `declared::scope`. `link`, `used` and `decorate` now write through the sink.
  - Lost by the shared code: `body` and `cite`. Kotlin's throwaway `Extraction` copy is gone too.
- **Outside the inputs:** step 10b's binary against step 11's on 949 Java files from Android SDK 35,
  two vendored Ruby gems' Java extensions and a probe file (`log/langdiff-step11-outside.txt`).
  - R1 and R4 are 82. Of these, 74 are members of a type nested four deep, and 8 are members of a
    type nested in an enum's declarations.
  - Calls fall from 26,822 to 26,728, all from or to those nodes, with none added (P 1.000,
    R 0.996).
  - Body-above is 720, all in the doc above the last line. The divergences are listed in the plan's
    "Step 11 as built" note.
- **Bench:** not run. The fixture holds no Java, so the graph the bench reads is byte-identical to
  `fixture-8p`'s.
- **Build time:** interleaved with step 0, on the corpus.
  - Median of five: 1.181× (`log/buildtime-step11-4.txt`). An earlier run at load 11 read 1.211×.
  - Median of nine, three-way: step 0 1.263 s, step 10b 1.502 s (1.189×), step 11 1.495 s (1.184×)
    (`log/buildtime-step11-3.txt`).
- **Gate:**
  - `cargo test --all-targets`: 1,397 passed, 0 failed;
  - `cargo clippy --all-targets -- -D warnings`: clean;
  - `cargo tree --duplicates --depth 0`: no tree-sitter crate;
  - `src/code/jvm/` is gone.

## Step 12 · C#, Razor

New: `bin/repograph-step12c`. `bin/repograph-step12` holds the first, slower query, and the two
write identical stores on the kit, the Razor-globbed kit and the scratchpad corpus. C#'s and Razor's
old readers and `src/code/dotnet/` are deleted.
C#'s lookup lives in `src/code/exceptions/namespace_scope/`, and Razor's components in
`src/code/exceptions/rendered_components/`. Neither is `package_scope`: a C# name resolves through
every namespace around it and a type is the sum of its `partial` parts, where a JVM file names one
package and imports types.

- **Regressions:** from `langdiff.py` on four inputs. The readings are in
  `log/langdiff-12-{kit,razor,scratch}.txt`, `log/langdiff-step12-fixture.txt` and
  `log/langdiff-step12-kit-jvm.txt`.
  - Kit corpus (`old-12` → `new-12`, `--lang cs,csproj`): R3 = 1 and body-above 1, both accepted.
    Every other class is 0, and the 19,757 edges are identical. `old-12` is the old reader under
    `REPOGRAPH_LEGACY=csharp`, identical to `new-11`.
  - Kit corpus with `.razor` and `.cshtml` globbed (step 11 → step 12): razor R3 = 18, accepted. The
    1,242 edges are identical.
  - Scratchpad corpus (vs `store-base`): cs R3 = 1 and body-above 1, the kit's two nodes. The rest
    is the accepted py R3 = 12, rs R3 = 3 and body-above 7, SQL body-above 11 and Swift R3 = 1.
  - Fixture: byte-identical to `fixture-8p`. Against `fixture-base` only step 6's SQL 21 and
    R7 = 1 read.
  - `--lang java,kt` on the kit (`new-11` → `new-12`): 0.
- **Accepted, hand-read:**
  - R3 `TimereportController.CreateTimereport`: the old body was the last line of a multi-line
    attribute, and the new one is the signature line.
  - Body-above `TimereportDayAttachmentPathTests`: a blank line inside the doc is dropped by the
    shared `Stripped` form.
  - Razor R3 = 18: members written on one line with their attribute, such as
    `[Inject] public PayFormPageState PageService { get; set; }`. The old body was empty.
- **Bounded and strict kinds:** P = R = 1.000 for each, as no edge moved.

  | kind | kit (cs) | kit, Razor globbed (razor) |
  |---|---|---|
  | Declares (`export`) | 498 (282) | 380 (255) |
  | Imports | 20 | 1 |
  | Calls | 324 | 19 |
  | References, Extends, DecoratedBy | 0 | 0 |

- **Exceptions:** `namespace_scope` (C#) and `rendered_components` (Razor). Without their `resolve`
  hooks, 37 of the 74 C# cases and 23 of the 60 Razor cases fail. On the kit C# keeps 0 of 324
  Calls and 0 of 20 Imports. Both hooks stay.
- **Outside the inputs:** not measured. No other C# is on this machine outside a Bonliva
  repository, and a copy of its sources was refused by the session's permissions.
- **Bench:** not run. The fixture holds no C# or Razor.
- **Build time:** interleaved with step 0, on the corpus, at load 15–25.
  - Medians of five: 1.175× and 1.226× (`log/buildtime-step12-{8,7}.txt`).
  - Median of nine, three-way: step 0 1.292 s, step 11 1.558 s (1.206×), step 12 1.542 s (1.194×)
    (`log/buildtime-step12-6.txt`).
  - The first query compiled in 138 ms and read 1.239× against step 11's 1.168× in the same run
    (`-4`). Dropping its `#if` contexts and its wildcard parent brought the compile to 37 ms, with
    no change on any input.
- **Gate:**
  - `cargo test --all-targets`: 1,398 passed, 0 failed;
  - `cargo clippy --all-targets -- -D warnings`: clean;
  - `cargo tree --duplicates --depth 0`: no tree-sitter crate;
  - `src/code/dotnet/` is gone.

## Step 13 · TypeScript, TSX, Vue

New: `bin/repograph-step13`, built at `241b838`. The old reader is `bin/repograph-step13d`, and
`src/code/typescript/` is deleted. TypeScript and TSX declare through
`queries/typescript/decl.scm`: 14 patterns, about 52 ms to compile per grammar. Three exceptions keep
what no query says: `import_bindings`, `statement_prose` and `single_file_components`.

- **Regressions:** from `langdiff.py` on three inputs. The readings are in
  `log/langdiff-step13-{kit,scratch,fixture}.txt`.
  - Kit (`new-12c` → `new-13`): tsx R2 = 2. Every other class is 0 in every language.
  - Scratchpad corpus (vs `store-base`): the same tsx R2 = 2, and step 12's accepted diffs.
  - Fixture (old-reader store → `fixture-13`, and the same against `fixture-8p`): ts R2 = 10,
    and References −7 / +7.
  - R7 = R8 = R9 = 0 on all three.
- **Accepted, hand-read:**
  - R2: a decorated class not written `export` starts at its decorator's row, since the grammar
    folds the decorator into the class node (`App.tsx::App` 55 → 54).
  - References ±7: comment cites inside `@Module({…})` above `export class` belong to the class,
    not the file. That is BE-M02, FR-DM-06 and FR-SEC-30 in `app.module.ts`, BE-M02, FR-SEC-30 and
    FR-VIS-47 in `identity.module.ts`, and N-104 in `database.module.ts`.
- **Exceptions without their hook:**

  | hook | kit | fixture |
  |---|---|---|
  | `import_bindings`: Calls P / R | 0.997 / 0.325 | 0.983 / 0.431 |
  | DecoratedBy R | 0 | 0 |
  | Extends P / R | 0.900 / 0.188 | 0 / 0 |
  | Implements R | 0.111 | 0.488 |
  | ReExports R | 0 | 0 |
  | Imports R, ignoring context | 0.997 | 0.999 |
  | `statement_prose`: R3 + body-above | 1,115 | 4,578 |
  | `single_file_components` | no `.vue` declares | no `.vue` files |

  The readings are in `log/pr-step13-nohook-{kit,scratch,fixture}.txt`. Every hook stays.
- **Outside the inputs:** `beauty-crm` main and two worktrees, exported to the scratchpad. The
  reading is in `log/langdiff-step13-outside.txt`.
  - On main: R1 17, R2 31, R4 17 and R5 6, with Calls −20 / +23 and References −48 / +51. The two
    worktrees read the same classes, at R1 22 and 18.
  - R1, R4 and R5 are object literals behind `as unknown as`, which the query does not read. That
    is 17 of 927 literal members (R 0.982), all in test doubles. Reading a nested `as` costs the
    compile 52 → 66 ms per grammar.
  - R2 and the moved References are the two accepted classes above.
  - The rest moves Calls and cites from the class to the method. A member decorator was separated
    from its method by a comment, and the old reader gave it to the class.
- **Bench,** on the fixture, store built by `repograph-step13`:
  - `--no-dense`: `keyword 39/40 paraphrase 7/30 code 12/12 p90 238 tok`. The transcript is
    identical to step 10's.
  - Dense, full build in 2 min 44 s: `keyword 40/40 paraphrase 9/30 code 12/12 p90 237 tok`,
    under e5-small. There are zero flips against the last dense raw row, and §5.4's small-model
    floors hold.
- **Build time:** median of five, interleaved, at load 19–26 (`log/buildtime-step13-final.txt`).
  Step 0 ran 1.265 s, step 12's final binary 1.525 s (1.205×) and step 13 1.432 s (1.132×). The
  double-pass fix (`478282f`) is most of the gap to step 12.
- **Gate:**
  - `cargo test --all-targets`: 1,401 passed, 0 failed;
  - `cargo clippy --all-targets -- -D warnings`: clean;
  - `cargo tree --duplicates --depth 0`: no tree-sitter crate;
  - `src/code/typescript/` is gone.

### Step 13 · review fix `082861f`

New: `bin/repograph-step13b`. A decorated class member reads its line from its first child that is
not a decorator, where it read the first line not opening with `@`.

- **Regressions:** from `langdiff.py --lang ts,tsx,vue`, in `log/langdiff-step13b-{kit,scratch,fixture}.txt`.
  - Kit (`new-13` → `new-13b`): ts R3 = 121, vue R3 = 2. Every other class is 0, and the 19,757
    edges are identical.
  - Scratchpad corpus (vs `store-base`): the same 123 on top of step 13's readings.
  - Fixture (vs `fixture-13`, step 13's store rebuilt for this reading): byte-identical.
- **Accepted, all 123 hand-read:** 121 old lines were a decorator's inner line (`type: Number,`)
  and now read the field (`notDistributed!: number`). Two Vue `@Prop()` fields read empty and now
  read `mode: string` and `show: boolean`.
- **Bench** on `082861f`: `--no-dense` `keyword 39/40 paraphrase 7/30 code 12/12 p90 238 tok`; dense
  `keyword 40/40 paraphrase 9/30 code 12/12 p90 237 tok` (e5-small). Both match step 13 case for
  case: zero flips.

## Step 14 · Remove the old paths

New: `bin/repograph-step14`, built at `0fab918`. The front calls the engine alone, and
`REPOGRAPH_LEGACY` is gone. `walk::GRAMMAR` moves from 5 to 6. Before that, two fixes from step 13's
review: an update drops the read-ahead it did not use (`198dab5`), and a file's decorators are read
in one walk (`f9479b6`, binary `bin/repograph-step14a`).

- **Regressions:** from `langdiff.py` over every language, in `log/langdiff-step14{a,}-{kit,scratch,fixture}.txt`.
  - `step14a` and `step14` against `082861f`'s stores: byte-identical on the kit, the scratchpad
    corpus and the fixture.
  - Against the base stores (`-cumulative.txt`), only the accepted diffs remain. On the scratchpad
    corpus and the kit: swift R3 1, sql body-above 11, py R3 12, rs R3 3 and body-above 7, cs R3 1
    and body-above 1, tsx R2 2, ts R3 121 and vue R3 2. On the fixture: SQL R1 3, R4 3, body-above
    15 and R7 1 (step 6), and ts R2 10 and References ±7 (step 13). R8 and R9 are 0 everywhere.
- **Read-ahead:** the new test fails without the pass and passes with it.
- **Decorators:** corpus build, median of five interleaved (`log/buildtime-step14a.txt`):
  `step13b` 1.390 s wall and 1.852 s CPU, `step14a` 1.374 s and 1.816 s. That is within the noise.
- **Bench** on `0fab918`: `--no-dense` `keyword 39/40 paraphrase 7/30 code 12/12 p90 238 tok`; dense
  `keyword 40/40 paraphrase 9/30 code 12/12 p90 237 tok` (e5-small). Both match `082861f` case for
  case: zero flips. The reset settles with `repograph-step13b`, as a GRAMMAR 6 binary re-reads the
  fixture's store.
- **Build time:** median of five, interleaved with step 0, at load 8.8 (`log/buildtime-step14.txt`).
  Step 0 ran 1.191 s and step 14 1.349 s, which is 1.132×.
- **Gate:**
  - `cargo test --all-targets`: 1,405 passed, 0 failed;
  - `cargo clippy --all-targets -- -D warnings`: clean;
  - `cargo tree --duplicates --depth 0`: no tree-sitter crate.

## Step 15 · Lua

New: `bin/repograph-step15`, built at `c9415d7`. Parent: `bin/repograph-step14b`, built at
`eb316a2`. Lua is read only when a config or `REPOGRAPH_CODE_GLOBS` names `**/*.lua`.

- **Corpus:** `luarocks/luarocks` at `v3.13.0`, `3421bedc2ce2b64e79530bb97497531b014899a8`, 162
  `.lua` files, MIT. It is read as data only: no script of it was run.
- **Parse census:** 162 files, 0 with an ERROR or a MISSING node.
- **Cases:** 8 in `src/code/cases/lua.rs`. They cover every capture role, the three import shapes,
  the roots and `init.lua`, the own-file call, the receiver call, a duplicate member resolving to
  neither file, and the references left unresolved on purpose: `obj:close()` on a value, a function
  two files define, and a parameter named like an import.
- **Hand-read precision** (`--no-dense`, the defaults plus `**/*.lua`, seed 15 unless noted):
  - Lua writes 1,060 nodes (162 files, 898 symbols), Declares 898, Imports 556, Calls 1,283, and no
    References: every Lua citation names an id nothing declares.
  - Symbols: 50 of 50 right in id, line and end.
  - Declares (seed 16): 50 of 50. `os.execute` in `fs/win32.lua` is right: the file assigns it.
  - Imports (seed 17): 50 of 50. Beyond the sample, every one of the 556 edges matches a
    `require` in its file. Of the 558 `require` strings that name a corpus file, the two without an
    edge sit inside shell command strings in `spec/loader_spec.lua` and `spec/unit/loader_spec.lua`.
  - Calls: 50 of 50. A call from a function value is the file's, as in `core/cfg.lua` line 139.
  - Five seeded files: `spec/cmd_spec.lua`, `spec/fixtures/a_rock.lua`,
    `spec/unit/rockspecs_spec.lua` and `spec/which_spec.lua` declare no function, and
    `src/luarocks/pack.lua` declares 5; all five match the hand count. A second draw of files that
    declare something:
    - `legacyexternalcommand.lua` 1/1, `cmd/install.lua` 7/7 and `fs/linux.lua` 2/2 match;
    - `spec/util/quick.lua` has 8 of 14 `function` statements. The other six are inside
      `parse`'s body.
    - `tools/zip.lua` has 20 of 26. The other six are `function zlib_compress()` and two siblings,
      once in each `if` branch, assigning `local`s declared above.
    - `fs/lua.lua` has 62 symbols for 69 statements. Three are second definitions of one id in the
      other branch (`exists`, `file_age`, `download`), three sit two blocks deep (`get_md5`,
      `_unix_umask` and the first `make_temp_dir`), and one is `md5.sumhexa = function` inside a
      branch.
- **Regressions** (`log/langdiff-step15-*.txt`):
  - luarocks, without against with the Lua globs: lua R1 1,060 (every new node), R4 1,454 (its
    Declares and Imports) and Calls +1,283, R5 0, R6 0, R7 0, R9 0. R8 is 1: the pending count goes
    from 2 to 7 with Lua's five citations of undeclared ids.
  - Kit corpus, `step14b` against `step15` with the Lua globs: `graph.json` and `headers.json`
    byte-identical.
  - Scratchpad corpus, `store-14` against `step15` with the Lua globs: byte-identical. `step14b`'s
    store is byte-identical to both.
- **Fixture and bench:** not run. The fixture holds no `.lua` file and Lua is not in its globs, so
  its graph cannot move.
- **Size:** stripped 66,866,256 → 66,982,072 bytes (+115,816); compressed 14,497,719 → 14,539,712
  (+41,993). Both are far under the 8 MB and 1 MB bars.
- **Build time:** median of five, interleaved with step 0, at load 3.3 (`log/buildtime-step15.txt`).
  Step 0 ran 1.242 s and step 15 1.388 s, which is 1.117×.
- **Licence reading:** `tree-sitter-lua 0.5.0 MIT ok`. Its `LICENSE.md` at v0.5.0: "The MIT
  License (MIT) Copyright (c) 2021 Munif Tanjim". `refs.scm` adapts the grammar's `locals.scm` and
  keeps that notice at its top.
- **Edited tests, construction only:** `lang.rs`'s `all_lists_every_language_once` gains Lua's slot,
  and `tests/engine.rs` builds `ImportPaths::Relative` and `Receivers::Hook`. No assertion changed.
- **Platforms:** not verified. The branch was not pushed, so `ci` has not run on it.
- **Gate:**
  - `cargo test --all-targets`: 1,413 passed, 0 failed;
  - `cargo clippy --all-targets -- -D warnings`: clean;
  - `cargo tree --duplicates --depth 0`: no tree-sitter crate.

### Step 15 · review fix `03f3c21`

New: `bin/repograph-step15r`, built at `03f3c21`. Parent: `bin/repograph-step15`. The fix reads a
receiver by what the required file exposes (`@export.root`, or its unowned top-level definitions),
resolves `M.f()` on the file's own table and `self` in a member (`#set! receiver "owner"`), and tells a
`require` alias from a value by node type.

- **Cases:** 11 in `src/code/cases/lua.rs`, 3 new, one per finding. Each fails on `a806536`.
- **luarocks** (`--no-dense`, the defaults plus `**/*.lua`), `step15` against `step15r`:
  - langdiff `--lang lua`: R1–R9 all 0, `lua Calls: +201`.
  - Calls 1,283 → 1,484: 201 added, 0 removed. Every added edge stays inside one file, across 39
    files: 132 through the file's own table (`dir.normalize`, `fetch.fetch_url`) and 69 through
    `self`: 66 in `vendor/argparse.lua`'s classes and 3 in `upload/api.lua`.
  - Declares 898, Imports 556 and References 0 are the same sets as step 15's.
  - No edge was removed: every edge step 15 wrote through an alias names a member of the table its
    target file returns. The false shapes the review found do not occur here; the cases pin them.
  - The finding-3 shape occurs once, `local debmeta = require "debug".getmetatable` in
    `vendor/dkjson.lua`, and `debug` is not a corpus file, so no edge moves.
- **Hand read:**
  - 50 of the 201 added Calls (seed 15): 50 right. Fifteen are `self:m()`, fourteen in
    `argparse.lua` and one in `upload/api.lua`, each to the method its own class declares. Three
    reach `os.remove`, which `spec/util/test_env.lua` reassigns at its top level, as it does
    `os.tmpname`, `lfs.chdir` and `lfs.attributes`, the other such targets of the 201. Two come from the
    file, because the calling function sits two blocks deep and is no symbol (`fs/lua.lua` line 1070,
    `util.lua` line 494), as step 15 already counted for a call from a function value.
  - Removed: none to read.
  - 50 of all 1,484 Calls (seed 15): 50 right.
- **P and R against step 15** (O = step 15's graph):

  | kind | step 15 | review fix |
  |---|---|---|
  | Calls | 1,283, 50/50 | 1,484; P 1.000, R 1.000 (1,283 of 1,283) |
  | Declares | 898, 50/50 | same set; P 1.000, R 1.000 |
  | Imports | 556, 50/50 | same set; P 1.000, R 1.000 |
  | References | 0 | 0 |

- **Byte-identity:**
  - Kit corpus, `step15` against `step15r` with the Lua globs, and `step15r` without them:
    `graph.json` and `headers.json` identical.
  - Scratchpad corpus, `store-14` against `step15r`, with and without the Lua globs: identical. All
    four `graph.json` files hash `566f40d972ba1d21…`.
- **Build time:** median of five, interleaved with step 0, at load 2.96 (`log/buildtime-step15r.txt`).
  Step 0 ran 1.219 s and `step15r` 1.391 s, which is 1.140×. The kit holds no `.lua` file, so this
  is noise around step 15's 1.117×. On luarocks itself, interleaved five times at load 4.0, `step15`
  took 0.333 s and `step15r` 0.342 s.
- **Gate:**
  - `cargo test --all-targets`: 1,416 passed, 0 failed;
  - `cargo clippy --all-targets -- -D warnings`: clean.

## Step 16 · item 0 · Lua leftovers

New: `bin/repograph-step16a`, built from the item 0 tree. Parent: `bin/repograph-step15r`.

- **What changed:** `return setmetatable(M, …)` is an export root; a file-level `return require(…)`
  is a `ReExports` edge whose target's members a receiver reaches; `self` reads its binding
  (`#set! receiver "owner"` on a `@local.definition`), so `local self = …` rebinds it and
  `function M.f()` has none; `Locals::shadows` and `shadows_apart` are one helper.
- **Cases:** 14 in `src/code/cases/lua.rs`, 3 new: a rebound `self` beside an explicit and an
  assigned `self` parameter, `function M.f()` with no `self`, and a closure's own `self`; an alias
  that is also the file's own table; `setmetatable` and `return require` exposing a table's members.
- **luarocks** (`--no-dense`, the defaults plus `**/*.lua`), `step15r` against `step16a`:
  - Calls: the same 1,484 edges. luarocks has no `return setmetatable(`. Its five `local self`
    sit in dotted constructors (`results.new`, `queries.new`, `queries.from_dep_string`,
    `multipart.new_file`, `api.new`), and only `api.new` calls through it: `self:load_config()`
    read as `api.load_config` before and as the local now, and the method is `api.Api.load_config`
    either way, so no edge moves.
  - Imports 556 → 552 and ReExports 0 → 4 (`log/langdiff-step16a-luarocks.txt`, lua R4 8). Read
    by hand, all four are right: `require.lua` line 2, `fetch/hg_ssh.lua` and `fetch/hg_https.lua`
    line 8, `fetch/git_https.lua` line 7, each `return require(…)` at the file's level. Nothing in
    luarocks calls through an alias of those files, so no Calls edge follows them.
  - Declares 898 and References 0 are the same sets.
  - `return { … }`: three files, `tlconfig.lua`, `spec/util/versions.lua` and
    `spec/fixtures/a_rock.lua`, all data. Left unread (plan, step 15 as built).
- **Byte-identity:** kit and scratchpad corpora, `step15r` against `step16a`, with and without the
  Lua globs: `graph.json` and `headers.json` identical, and the scratchpad's matches `store-14`.
- **Gate:** `cargo test --all-targets` 1,419 passed, 0 failed; clippy `-D warnings` clean.

## Step 16 · Go

New: `bin/repograph-step16`, built at the tree committed as `e8510ae`. Parent: `bin/repograph-step16a`,
built at `904039b`. Go is read only when a config or `REPOGRAPH_CODE_GLOBS` names `**/*.go`.

- **Corpus:** `gin-gonic/gin` at `v1.12.0`, `73726dc606796a025971fe451f0aa6f1b9b847f6`, 98 `.go`
  files (40 `_test.go`), one `go.mod` (`module github.com/gin-gonic/gin`), eight package
  directories, MIT. It is read as data only: no script of it was run.
- **Parse census:** 98 files, 0 with an ERROR or a MISSING node.
- **Cases:** 11 in `src/code/cases/go.rs`. They cover every capture role, docs on single and grouped
  specs, `_` and embedded fields, module-path imports with a nested module and the module root,
  the implicit and explicit alias, build variants with the own file first, the receiver call
  across sibling files, generic calls in both parsed shapes, a parameter named like its type,
  `(*T)(nil)`, and the references left unresolved on purpose: a promoted method, a call on a value,
  a rebound receiver, an unexported member through an import, and a local hiding an import.
- **What Go writes** (`--no-dense`, the defaults plus `**/*.go`): 2,119 nodes (98 files, 2,021
  symbols), Declares 2,021 (1,558 exported), Imports 227, Calls 1,347, References 1,482, all
  `[type]`. The pending count goes from 1 to 12 with eleven `UTF-8` citations in strings.
- **Hand read:**
  - Symbols (seed 16): 50 of 50 right in id, line and end.
  - Five seeded files, counted by hand against the source: `codec/json/jsoniter.go` 9/9, `doc.go`
    0/0, `ginS/gins_test.go` 19/19, `internal/fs/fs_test.go` 5/5, `logger.go` 47/47.
  - Declares (seed 16): 50 of 50. A method is exported by its own name, so `queryBinding.Name` is
    exported though its type is not.
  - Imports (seed 17): 50 of 50. Beyond the sample, the 227 edges equal a census of every
    module-path import string expanded to its directory's non-test files: none missing, none extra.
  - Calls (seed 20): 50 of 50. `formBinding.Bind` reaches `binding.go`'s `validate`, and a test
    reaches both build variants of it, `binding.go` and `binding_nomsgpack.go`.
  - References (seed 21): 50 of 50.
- **Census recall.** A new language has no old path, so recall is read from a text census of the
  shapes the rule claims, independent of the grammar, and every miss is hand-read
  (`s16/crecall.py`, `s16/rrecall.py` in the scratchpad):
  - Calls: 1,868 sites (a bare call of a package name, `pkg.F(` through a module import, `recv.m(`
    on a method's receiver), 1,866 with an edge. The two misses are not calls: an interface's method
    declaration, and `c.handlers[c.index](c)`, which calls an element. The four edges no site
    explains are right: three `getTyped[map[string]any](c, key)` the census cannot spell, and the
    conversion `(FOO)(0)`.
  - References: 2,185 sites of a package type's name, 2,075 with an edge. Of the 110 misses, 109
    are not type uses: 90 locals or fields named like a lower-case type (`route`, `header`, `node`),
    14 fields named `Data`, four words inside raw strings, and `(FOO)(0)`, which is a call. One is a
    real miss: `node := &node{}` in `tree_test.go`, whose type reads the new local. The two edges no
    site explains are wrong: `c.handlers[c.index](c)` parses as a conversion to a generic
    qualified type, and the receiver rewrite names `Context.handlers` and `Context.index`.
- **P and R per kind** (§3.3; P from the hand read and the census, R from the census):

  | kind | edges | sample | P | R |
  |---|---|---|---|---|
  | Calls | 1,347 | 50/50 | 1.000 | 1.000 (1,866 of 1,866) |
  | References | 1,482 | 50/50 | 0.9987 (1,480 of 1,482) | 0.9995 (2,075 of 2,076) |
  | Declares | 2,021 | 50/50 | 1.000 | 1.000 (five files) |
  | Imports | 227 | 50/50 | 1.000 | 1.000 (227 of 227) |

  Extends, Implements and DecoratedBy: none, since Go has no such syntax the queries read.
- **What the hand read changed before the commit:** a first build at `OwnFirst`'s predecessor `All`
  wrote 21 References from one build variant into another (`binding_nomsgpack.go` to `binding.go`'s
  `BindingBody`); after the switch, no Calls or References edge leaves a file that declares the same
  name. The census found 16 instantiated calls with two arguments (`getTyped[bool](c, key)`), 31
  `(*T)(nil)` assertions and three signatures with a parameter named like its type unread; each
  is a pattern now, pinned by a case.
- **Regressions** (`log/langdiff-step16-gin.txt`), gin without against with the Go globs: go R1
  2,119 (every new node), R4 2,248 (its Declares and Imports), Calls +1,347, References +1,482, R2 0,
  R3 0, R5 0, R6 0, R7 0, R9 0. R8 is 1, the pending count above.
- **Byte-identity:**
  - Kit corpus and scratchpad corpus, `step16a` against `step16`, with the default globs, with
    `**/*.lua` and with `**/*.go`: `graph.json` and `headers.json` identical in all six pairs, and
    the scratchpad's graph equals `store-14`'s (`dff8b36a53bb0548…`). Neither corpus holds a `.go`
    file.
  - luarocks with `**/*.lua`: `graph.json` identical to `step16a`'s, so item 0's diffs are the
    only ones since step 15r.
- **Fixture and bench:** not run. The fixture holds no `.go` file and Go is not in its globs.
- **Size:** stripped 66,998,976 → 67,263,536 bytes (+264,560); `tar czf` 14,524,012 → 14,579,325
  (+55,313). Both are far under the 8 MB and 1 MB bars.
- **Build time:** median of five, interleaved with step 0, at load 11.0 falling to 8.9
  (`log/buildtime-step16.txt`). Step 0 ran 1.323 s and step 16 1.483 s, which is 1.121×.
- **Licence reading:** `tree-sitter-go 0.25.0 MIT ok`, ABI 15. Its `LICENSE`: "The MIT License (MIT)
  Copyright (c) 2014 Max Brunsfeld". The queries are written to the engine's contract, not adapted
  from the crate's `tags.scm`, so they carry no notice.
- **Edited tests:** `main.go` leaves the unread examples of
  `the_typescript_family_is_read_by_its_two_grammars_and_an_unknown_extension_by_none` and
  `a_name_either_table_knows_decides_whatever_the_shebang_says`, since `.go` is read now.
  `lang.rs`'s `all_lists_every_language_once` gains Go's slot. No other assertion changed.
- **Platforms:** not verified. The branch was not pushed, so `ci` has not run on it.
- **Gate:**
  - `cargo test --all-targets`: 1,430 passed, 0 failed;
  - `cargo clippy --all-targets -- -D warnings`: clean;
  - `cargo tree --duplicates --depth 0`: no tree-sitter crate.

### Step 16 · review fix `5cd204b`, re-verified

New: `bin/repograph-step16r`, built at `5cd204b`. Parent: `bin/repograph-step16`. The fix names an
unaliased import by the package most of its files declare, unquotes a raw-string import path, and
binds a generic receiver's type parameters in the method.

- **gin** (`--no-dense`, the defaults plus `**/*.go`), `step16` against `step16r`: `graph.json`
  byte-identical (`119169be4bd8aa62`), langdiff `--lang go` R1–R9 all 0
  (`log/langdiff-step16r-gin.txt`). Nothing moves because gin holds none of the three shapes: each
  of its eight package directories declares one package, no import path is a raw string, and no
  method has a generic receiver. The three cases in `src/code/cases/go.rs` pin them.
- **`Locals::shadows`** splits a name on `.` as well as `/` since step 16's item 0. No store moved,
  because no reference the default resolves spells a dotted name in any language today. The case
  `a_local_hides_a_path_through_it_whichever_separator_joins_the_path` in `src/code/cases.rs` pins
  it: a local `a` hides `a`, `a.b.C`, `a/b` and `a.b/c`, and not `ab.C`, `b.a` or `C`, nor anything
  past its scope.
- **Recorded open** (plan, step 16 as built): a `go.mod` edit widens no importer until the next
  `build`; `go.work` and `replace` are not read; `shadows` ignores binding order.

## Step 17 · C

New: `bin/repograph-step17`, built at the tree committed as `77b7d8f` (measured as `step17w`, the
same tree). Parent: `bin/repograph-step16r`, built at `5cd204b`; item 0 is `f22c2a1`, which
changed no store. C is read only when a config or `REPOGRAPH_CODE_GLOBS` names `**/*.c` and `**/*.h`.

- **Corpus:** `redis/hiredis` at `v1.4.1`, `616f2286ba5503f74ae96e720623fa11dbc690af`, 25 `.c` and
  27 `.h`, BSD-3-Clause. It is read as data only: no script of it was run.
- **Parse census, held for Max:** 18 of 52 files hold an ERROR or a MISSING node, 234 nodes, far
  above the 2% bar. By cause:
  - six headers (`alloc.h`, `async.h`, `hiredis.h`, `hiredis_ssl.h`, `read.h`, `sds.h`) carry one
    `MISSING #endif` each, where `#ifdef __cplusplus } #endif` closes the `extern "C" {` block;
    nothing in them drops;
  - macros the grammar cannot see through: the vendored `ffc.h`'s attribute macros (140 nodes),
    `EV_P_` in the libev adapter and example, `G_GNUC_UNUSED` in the glib example, `va_arg(ap, T)`
    in `hiredis.c` and `sds.c`, the test macros in `test.c` and `sds.c`; `ssl.c`'s `} else` split
    across `#ifdef`; `__unused` in `adapters/macosx.h`'s parameters;
  - `adapters/libuv.h`, one ERROR over the whole file, from an `#if … #else` that repeats a
    function's header;
  - `adapters/qt.h` and `examples/example-qt.h`, which are C++.

  Only the last two causes cost definitions: 13 of the census's 692.
- **Cases:** 14 in `src/code/cases/c.rs`, and one contract test in `queries.rs` for `outside`. They
  cover every capture role: functions, function-like macros, bodied and plain typedefs, tagged
  types, members through every declarator shape, an unnamed nested struct's members as the
  holder's, enumerators, a prototype declaring nothing, `static` hidden, includes beside the
  includer and under the roots with `<…>` dropped, the own file first and then the one linked
  definition, a parameter or local hiding a function, `#ifdef` variants as one symbol, `extern "C"`,
  a function body's and a variable's inline types declaring nothing, and citations.
- **What C writes** (`--no-dense`, the defaults plus `**/*.c **/*.h`): 1,140 nodes (52 files, 1,088
  symbols: 609 functions, 70 macros, 81 types, 328 members), Declares 1,088 (842 exported), Imports
  78, Calls 1,192. The pending count is 1, a `LICENSE-2` citation in `ffc.h`.
- **Census recall.** A text census over comment- and string-blanked source, independent of the
  grammar (`s17/census.py`, `s17/tcensus.py` in the scratchpad), every miss hand-read:
  - Functions and macros: 692 definitions, 679 with a symbol. The 13 misses are `libuv.h`'s 11 and
    one function in each C++ header.
  - Types 81 of 82, members 328 of 332: every miss is in `libuv.h`. No member and no top-level
    symbol is outside the census.
  - Calls, under the plan's linker rule (own file, else the one non-`static` definition): 1,197
    pairs, 1,185 with an edge. The 12 misses sit in `libuv.h` and `qt.h`, whose definitions drop;
    five of them land on the file instead, the edges no site explains. Of the 1,197 pairs, two are
    not C calls: `connect(…)` in `qt.h` is `QObject::connect`, and `read()` there is inside Qt's
    `SLOT(…)`.
  - Sites the linker rule claims nothing for: 115 calls of a `static` function defined in another
    file (a header's `static inline`, `sdslen` alone 56, the adapters' `Attach` functions, and
    `dict.c`'s functions, which `async.c` pulls in with `#include "dict.c"`); 20 of `assert`, which
    `async.c` and `test.c` both define; 14 of an adapter's `static` function behind
    `#include <adapters/…>`.
  - Against a truth that follows quoted includes transitively, recall is 0.884 (1,114 of 1,260
    pairs), or 0.942 counting `alloc.h`'s `static inline hi_malloc` and `alloc.c`'s `_WIN32`
    `hi_malloc` as one variant. The other 71 pairs are header statics. This is recorded for step
    18, not gated (plan, step 17 as built).
  - Includes: 81 quoted, 78 resolving to a file of the repository, 78 edges, none missing, none
    extra; the other three name files outside it.
  - `OwnFirst` instead of `OwnThenUnique`: +28 Calls edges, all `assert` reaching both files'
    macros, wrong; none gained.
- **Hand read:**
  - Symbols (seed 17): 50 of 50 right in id, line and end.
  - Five seeded files, counted by hand against the source: `examples/example-ae.c` 4/4,
    `examples/example-libevent-ssl.c` 4/4, `examples/example-libsdevent.c` 5/5,
    `examples/example-ssl.c` 1/1, `win32.h` 4/4 (two function-like macros, two `__inline`
    functions). `example-ae.c`'s five Calls are right; its sixth call, `redisAeAttach`, is an
    adapter's `static` behind an angle include.
  - Declares (seed 17): 50 of 50. Two are `qt.h`'s C++ methods (`delWrite`, `setContext`), which
    stand in the file as functions and are read as C reads them, with no class.
  - Imports (seed 17): 50 of 50, each against its `#include` line.
  - Calls (seed 17): 50 of 50.
- **P and R per kind** (§3.3; P from the hand read and the census, R from the census):

  | kind | edges | sample | P | R |
  |---|---|---|---|---|
  | Calls | 1,192 | 50/50 | 0.9941 (1,185 of 1,192) | 0.9900 (1,185 of 1,197) |
  | Declares, functions and macros | 679 | 50/50 | 1.000 | 0.9812 (679 of 692) |
  | Declares, types | 81 | | 1.000 | 0.9878 (81 of 82) |
  | Declares, members | 328 | | 1.000 | 0.9880 (328 of 332) |
  | Imports | 78 | 50/50 | 1.000 | 1.000 (78 of 78) |

  Calls P counts all seven edges no site explains as wrong: five from `libuv.h`'s file node to the
  right targets, and `qt.h`'s `connect` and `read`. The Declares sample of 50 spans all three rows.
- **Regressions** (`log/langdiff-step17-hiredis.txt`), hiredis without against with the C globs:
  c R1 454, h R1 686 (every new node), R4 478 and 688 (Declares and Imports), Calls +886 and +306,
  R2 0, R3 0, R5 0, R6 0, R7 0, R9 0. R8 is 1, the pending count above.
- **Byte-identity:**
  - Kit corpus and scratchpad corpus, `step16r` against `step17`, with the default globs, with
    `**/*.lua`, with `**/*.go` and with `**/*.c **/*.h`: `graph.json` and `headers.json` identical
    in all eight pairs (`dff8b36a53bb0548`), and the scratchpad's graph equals `store-14`'s. Neither
    corpus holds a `.c` or `.h` file.
  - gin with `**/*.go`: identical to `step16r`'s (`119169be4bd8aa62`).
  - luarocks with `**/*.lua`: identical to `step16r`'s (`368f10ad0de3ba07`), which is step 16a's.
- **Fixture and bench:** not run. C is not in the fixture's globs.
- **Size:** stripped 67,296,488 → 67,940,648 bytes (+644,160); `tar czf` 14,600,535 → 14,669,745
  (+69,210). Both are under the 8 MB and 1 MB bars.
- **Build time:** median of five, interleaved with step 0 (`log/buildtime-step17.txt`): step 0
  1.347 s, step 17 1.502 s, 1.115×, at load 6.3 to 6.4. A first run read 1.233× at load 6.1
  (`log/buildtime-step17-first.txt`, one round at 1.677 s), and a three-way run of seven rounds
  read step 16r at 1.095× and step 17 at 1.118× (`log/buildtime-step17-3way.txt`), so the first was
  noise.
- **Licence reading:** `tree-sitter-c 0.24.2 MIT ok`, ABI 15. Its `LICENSE`: "The MIT License (MIT)
  Copyright (c) 2014 Max Brunsfeld". The queries are written to the engine's contract, not adapted
  from the crate's `tags.scm`, so they carry no notice.
- **Edited tests:** `tests/engine.rs` builds one `ImportPaths::Relative` and moves from
  `root: false`/`true` to `roots: &[]`/`&[""]`, construction only. `lang.rs`'s
  `all_lists_every_language_once` gains C's slot. No assertion changed.
- **Platforms:** not verified. The branch was not pushed, so `ci` has not run on it.
- **Gate:**
  - `cargo test --all-targets`: 1,448 passed, 0 failed;
  - `cargo clippy --all-targets -- -D warnings`: clean;
  - `cargo tree --duplicates --depth 0`: no tree-sitter crate.

## Step 18 · C++

New: `bin/repograph-cpp12`, built at the non-test source committed as `67c09fa`. Parent:
`bin/repograph-cpp-base`, built at `4d262b1`. C++ is read only when a config or
`REPOGRAPH_CODE_GLOBS` names its extensions; the store below names `**/*.cc **/*.h`.

- **Corpus:** `google/leveldb` at `1.23`, `99b3c03b3284f5886f9ef9a4ef703d57373e61be`, 75 `.cc` and
  56 `.h`, BSD-3-Clause. It is read as data only: no script of it was run.
- **`.h` census,** each corpus's headers alone under each grammar (`tests/parse_census.rs`):

  | headers | grammar | files with ERROR or MISSING | ERROR | MISSING |
  |---|---|---|---|---|
  | hiredis, 27 | C | 12 | 60 | 122 |
  | hiredis, 27 | C++ | 12 | 146 | 16 |
  | leveldb, 56 | C | 47 | 1,085 | 60 |
  | leveldb, 56 | C++ | 20 | 111 | 54 |

  C++ parses hiredis's headers with no more ERROR files than C, so `.h` is `Lang::Cpp`.
- **Parse census, gated on losses:** 0 of 131 leveldb files lose a definition. Raw: 32 of 131 hold
  an ERROR or MISSING node (140 and 75). On hiredis after the move: C 25 files, 6 bad; C++ 28 files
  (27 headers and `example-qt.cpp`), 12 bad.
- **Exception, no-hook numbers** (`bin/repograph-cpp-nohook`, the same tree with `hooks: NONE`): 25
  of 131 files lose a definition (19.1%); recall function 1.000, member 0.9054 (651 of 719), method
  0.8917 (1,021 of 1,145), type 0.8810 (222 of 252), Calls 0.8335 (986 of 1,183, 1,000 edges). With
  the hook every kind reads 1.000. On hiredis the hook changes only `qt.h` and `example-qt.h`.
- **Cases:** 8 in `src/code/cases/cpp.rs`: every capture role (in-class method, constructor,
  destructor, operator, pure virtual, deleted method declaring nothing, out-of-line method under a
  namespace and under a template with a nested class, the recovered `MISSING "::"` function, friend
  as the file's, unnamed namespace kept in the file, qualified and nested types, fields under `#if`
  and through every declarator, typedefs, `using`, enumerators), the attribute-macro hook,
  includes, the implicit owner in the own file and in the class's header, recursion not falling
  through, `ns::f`, and `o.f()`, `p->f()` and `A::f()` unresolved.
- **What C++ writes** (`--no-dense`, the defaults plus `**/*.cc **/*.h`): 2,602 nodes (138 files,
  131 of them C++, 2,464 symbols), Declares 2,465 (2,361 exported; `status.h`'s `Status.Status` is
  defined in the class and outside it, one node with two Declares), Imports 487, Calls 1,176. The
  pending count is 6, five of them citations C++ comments and strings add.
- **Census recall.** A text census over comment- and string-blanked source, independent of the
  grammar (`s18/census.py` in the scratchpad), every miss hand-read:
  - function 338 of 338, method 1,145 of 1,145, member 719 of 719, type 252 of 252. Ten symbols
    lie outside the census, all right: a brace-initialised field (`count_`), a test fixture's
    constructor, four `operator()` methods, two `BackgroundWorkItem.function` members, `State.cvar`,
    and a test helper function the census misreads.
  - Calls, under the linker rule with a method's implicit owner first: 1,183 pairs, 1,163 with an
    edge. The 20 misses: the most vexing parse (8: `BytewiseComparator` 5, `RandomSeed` 3), base
    initialisers (9: `EnvWrapper` 5, `Constructor` 4), `ModelDB`'s local class's `Put` and `Delete`
    (2, the census's false pairs), and placement `new Node` (1, the census's).
  - Unclaimed by the rule: 98 `new` expressions, 285 class-qualified calls (`Env::Default()`), 4
    names hidden in another file. No member call collides with a free function of its name.
  - Includes: 523 quoted, 487 resolving to a file of the repository, 487 edges, none missing, none
    extra; the other 36 name files outside it (`port_config.h`, gtest, benchmark).
- **Hand read** (seed 18):
  - Symbols: 50 of 50 right in id, line and end.
  - Five seeded files, counted by hand against the source: `db/repair.cc` 31/31 (its function
    body's `LogReporter` is a local), `helpers/memenv/memenv.cc` 60/60, `include/leveldb/table.h`
    3/3 (the class, its private constructor with a body, `rep_`; its prototypes declare nothing),
    `table/table_builder.cc` 28/28, `util/hash_test.cc` 2/2 (`TEST` and `main`).
  - Declares: 50 of 50. Imports: 50 of 50, each against its `#include` line. Calls: 50 of 50, the
    20 whose site the sampler did not find each found by hand (bare member calls and `TEST`
    bodies after the first).
- **P and R per kind** (§3.3; P from the hand read and the census, R from the census):

  | kind | edges | sample | P | R |
  |---|---|---|---|---|
  | Calls | 1,176 | 50/50 | 0.9983 (1,174 of 1,176) | 0.9831 (1,163 of 1,183) |
  | Declares, functions | 338 | 50/50 | 1.000 | 1.000 (338 of 338) |
  | Declares, methods | 1,145 | | 1.000 | 1.000 (1,145 of 1,145) |
  | Declares, members | 719 | | 1.000 | 1.000 (719 of 719) |
  | Declares, types | 252 | | 1.000 | 1.000 (252 of 252) |
  | Imports | 487 | 50/50 | 1.000 | 1.000 (487 of 487) |

  Of the 13 Calls edges no census pair explains, 11 are right (the fixture constructor's 3 calls,
  `SkipList`'s `compare_` functor member 3, `logging_test.cc`'s helper and its 2 calls, a `Slice(…)`
  construction, `GetLengthPrefixedSlice`), one
  is debatable (`WriteBatchInternal::Sequence` → the `SequenceNumber` typedef, a functional cast)
  and counted wrong, and one is wrong: `corruption_test.cc::TEST_F → skiplist_test.cc::Key`, a
  fixture's member called in a gtest body read as a free function. Counting all 13 as wrong, P is
  0.9889. The Declares sample of 50 spans all four rows.
- **Hiredis before and after** (`**/*.c **/*.h`, `cpp-base` against `cpp12`, step 17's census):
  definitions 679 → 672 of 692, calls 1,185 → 1,178 of 1,197, edges no site explains 7 → 9,
  Imports 78 of 78 both. By file, nodes: `qt.h` −11 +18, `example-qt.h` +6, and 103 of `ffc.h`'s
  changed in body or line only (88 move `line` up one: C++ reads the `ffc_internal ffc_inline`
  line before a signature as part of the definition); edges: `qt.h` −15 +18, `example-qt.h` +6,
  `ffc.h` −1. Every new definition miss is a `qt.h` or `example-qt.h` method now under its class,
  which the C census reads as a function; the call miss is `ffc.h`'s
  `FFC_SIMD_DISABLE_WARNINGS return h(…)`, read as a declaration. Step 17's wrong
  `qt.h → sockcompat.h::connect` is gone, and the new unexplained edges are right method edges.
  `libuv.h` is unchanged. Graph `1396190e7661e0c7` → `ee379ffbd70010d6`.
- **Regressions** (`log/langdiff-step18-leveldb.txt`), leveldb without against with the C++ globs:
  cc R1 1,900, h R1 695 (every new node), R4 2,215 and 737, Calls +1,104 and +72, R2 0, R3 0,
  R5 0, R6 0, R7 0, R9 0. R8 is 1, the pending count above. No edge dangles.
- **Byte-identity,** `cpp-base` against `cpp12`:
  - Kit corpus and scratchpad corpus with every C and C++ extension named: `graph.json` and
    `headers.json` identical (`dff8b36a53bb0548`), and the scratchpad's graph equals `store-14`'s.
  - gin with `**/*.go`: identical (`119169be4bd8aa62`).
  - luarocks with `**/*.lua`: identical (`368f10ad0de3ba07`); with `**/*.c **/*.h` added:
    identical (`58bfff811cf5cb80`); with `.hpp` added it differs by `lua.hpp` alone, a new C++ file.
- **Fixture and bench:** not run. C++ is not in the fixture's globs.
- **Size and build time:** not measured; this step ran no release build.
- **Licence reading:** `tree-sitter-cpp 0.23.4`, `license = "MIT"` in its `Cargo.toml` and
  `tree-sitter.json` (the crate ships no `LICENSE` file), ABI 14. The queries are written to the
  engine's contract, not adapted from the crate's `tags.scm`.
- **Edited tests:** `cases/c.rs`'s `a_c_source_and_a_header_are_read_by_their_extension` expects
  `Lang::Cpp` for a `.h`, and checks C++'s family. No other assertion changed.
- **Platforms:** not verified. The branch was not pushed, so `ci` has not run on it.
- **Gate:**
  - `cargo test --all-targets`: 1,463 passed, 0 failed;
  - `cargo clippy --all-targets -- -D warnings`: clean;
  - `cargo tree --duplicates --depth 0`: no tree-sitter crate.

### Step 18 · review fix `829623c`

New: `bin/repograph-cpp-fix`, built at `829623c`'s source. Parent: `bin/repograph-cpp12`. `b93426c`
between them changes only an operator spelled with `/`, which neither leveldb nor hiredis writes.
The fix resolves qualified calls through their whole qualifier, names a nested out-of-line method by
every class around it, drops a namespace its own file opens from a definition, and repairs two
tokenizer cases of the attribute-macro view. Plan, step 18 as built, has each finding.

- **leveldb, step 18's criterion re-run** (`**/*.cc **/*.h`). The census (`s18f/py/census.py` in the
  scratchpad) gains the fix's rules: an out-of-line definition joins every scope past a leading run
  of namespaces its file opens, and a qualified call drops a leading run of namespaces any file
  opens, then names `path.name` or nothing. Step 18's census left class-qualified calls unclaimed,
  so its Calls row is not comparable; both binaries are counted below under the new census.

  | kind | edges before → after | P before → after | R before → after |
  |---|---|---|---|
  | Calls | 1,176 → 1,388 | 0.9974 → 0.9978 (1,173 → 1,385) | 0.8348 → 0.9856 (1,162 → 1,372 of 1,392) |
  | Declares, functions | 338 | 1.000 | 1.000 (338 of 338) |
  | Declares, methods | 1,145 | 1.000 | 1.000 (1,145 of 1,145) |
  | Declares, members | 719 | 1.000 | 1.000 (719 of 719) |
  | Declares, types | 252 | 1.000 | 1.000 (252 of 252) |
  | Imports | 487 | 1.000 | 1.000 (487 of 487) |

  - Graph diff: Calls +213 −1, Declares +1 −1, one node renamed, nothing else. The renamed node is
    `memtable.cc`'s `MemTable::KeyComparator::operator()`, `KeyComparator.operator()` →
    `MemTable.KeyComparator.operator()`; its one Calls edge and one Declares edge follow it.
  - The 20 Calls misses are step 18's 20, unchanged. Unclaimed by the rule: 435 sites whose
    qualifier names nothing in the repository (`std::` 373, `testing::` 43, `snappy::` 4,
    `benchmark::` 1, and the rest the census's `return ::f()`), 36 whose member two files define
    (`Env::Default()` 34, in `env_posix.cc` and `env_windows.cc`; `DB::Put` and `DB::Delete`, a
    pure virtual in `db.h` and the body in `db_impl.cc`, where the linker rule takes the body), 3
    through the alias `PosixDefaultEnv` or `WindowsDefaultEnv`, 98 `new` expressions, 4 hidden.
  - P: 16 Calls edges no census pair explains: step 18's 13, the `::crc32c::Extend` edge step 18's
    census explained, and 2 new ones, which are right:
    `ModelDB::Put` and `Delete` call `DB::Put` and `DB::Delete` by name, and the body in
    `db_impl.cc` is the one reached. The renamed `operator()`'s edge to `GetLengthPrefixedSlice` is
    right, as before. Wrong in both binaries: `corruption_test.cc::TEST_F → skiplist_test.cc::Key`,
    the `SequenceNumber` functional cast, and `port_stdcxx.h`'s `::crc32c::Extend`, which names the
    external crc32c library's function, not `leveldb::crc32c::Extend` (step 18 counted it right). So
    P before is 1,173 of 1,176, not step 18's 1,174.
- **Hand read** (`log/handread-step18r-leveldb.txt`, seed 18): the one removed Calls edge is the
  rename. The 213 added are sampled to 50, all right, each against its site and the definition it
  reaches: 36 `Status::OK()`, `Corruption`, `NotFound` or `IOError`, 7 `WriteBatchInternal::…`, 3
  `DB::Open`, and one each of `Table::Open`, `EnvWrapper::RemoveFile`, `DB::Put` and
  `EnvWindowsTestHelper::SetReadOnlyMMapLimit`. The sampler found no site for one,
  `memenv_test.cc::TEST_F → DB.Open`: the site is line 225, in the sixth `TEST_F` body, which the
  `TEST_F` node's span does not cover.
- **hiredis, step 17's criterion** (`**/*.c **/*.h`, `.h` as C++), `cpp12` against `cpp-fix`:
  `graph.json` byte-identical (`ee379ffbd70010d6`); definitions 672 of 692, calls 1,178 of 1,197,
  edges no site explains 9, Imports 78 of 78, as step 18.
- **`.h` probe:** a C header using C++ keywords as identifiers, read as C++, keeps 23 of 25
  definitions; it loses functions named `operator` and `template`, and a call `new(3)` reads as a
  `new` expression. Recorded, no fallback.
- **`view()` timing** (debug build, leveldb, 131 files, 880,038 bytes): 47 ms a pass against 255 ms
  of parsing; 104 files borrow the source, 27 need a copy. The second pass is recorded, not removed.
- **Byte-identity,** `cpp12` against `cpp-fix`:
  - kit corpus and scratchpad corpus with every C and C++ extension named: `graph.json` and
    `headers.json` identical (`dff8b36a53bb0548`), and the scratchpad's graph equals `store-14`'s;
  - gin with `**/*.go`: identical (`119169be4bd8aa62`);
  - luarocks with `**/*.lua`: identical (`368f10ad0de3ba07`); with `**/*.c **/*.h` added: identical
    (`58bfff811cf5cb80`).
- **Cases:** `src/code/cases/cpp.rs` 10 tests (one replaced, two added): qualified calls through a
  namespace, a class, a nested class and `::`, with `std::swap`, `testing::Of` and `Missing::OK`
  naming nothing though the repository defines `swap`, `Of` and `OK`; the nested out-of-line method
  and its implicit owner; a namespace-qualified definition in a file opening the namespace.
  `exceptions/attribute_macros.rs` adds three: the borrowed view, the digit separator, and the CRLF
  continuation.
- **Gate:** `cargo test --all-targets` 1,469 passed, 0 failed; `cargo clippy --all-targets -- -D
  warnings` clean. No release build; fixture, bench and platforms not run.

## Step 19 · Ruby

New: `bin/repograph-ruby`, built at the tree committed as `ffeaabf`. Parent: `bin/repograph-step17`,
built at `77b7d8f`. Ruby is read only when a config or `REPOGRAPH_CODE_GLOBS` names `**/*.rb`,
`**/*.rbw`, `**/*.gemspec` and `**/*.rake`.

- **Corpus:** `rack/rack` at `v3.2.7`, `70d2e1046789a70e240e01b9ac0b3ffc9b26d33f`, 93 `.rb` and one
  `.gemspec`, MIT. It is read as data only: no script of it was run.
- **Parse census:** 94 files, 0 with an ERROR or MISSING node.
- **Cases:** 10 in `src/code/cases/ruby.rs`. They cover modules joining with `/` and classes with
  `.`, a reopened class as one symbol, `class << self`, the `private` and `protected` sections and
  `private def`, doc comments above a first method, `require`, `require_relative` and `autoload`
  beside the requirer and under `lib/`, `.rake` and `.gemspec`, a bare call reaching its class and
  the file, a `self.` call its owner, constants read under each container and as written, paths,
  the superclass, a reopened module reached in every file, block parameters, rescued errors and
  loop variables hiding a method, a hidden method out of another file's reach, a constant passing
  over a method spelled like it, and citations.
- **What Ruby writes** (`--no-dense`, the defaults plus the four Ruby globs): 1,158 nodes (94 Ruby
  files, 1,056 symbols: 67 modules, 92 classes, 726 methods, 171 constants), Declares 1,056 (975
  exported), Imports 291, Calls 455, References 802, Extends 12. The pending count gains 11, ten
  `UTF-8` and one `RFC-3330` citation in Ruby files.
- **Census recall.** A text census over comment-, string-, symbol- and heredoc-blanked source,
  nested by indentation, independent of the grammar (`s19/census.py`, `s19/blank.py`,
  `s19/icensus.py` in the scratchpad), every miss hand-read:
  - Definitions: modules 67 of 67, classes 92 of 92, methods 725 of 726, constants 171 of 171. The
    miss is `def YAML.unsafe_load` in `test/psych_fix.rb`, a method on a constant receiver, read by
    neither side's rule. 39 `def value.m` and 13 `class << value` sit on values no name reaches and
    are left out of both. The store has one symbol the census lacks: `test/spec_response.rb::<<`,
    from `alias << call` inside a `class_eval` block, counted wrong. Export: 0 disagreements against
    `private`, `protected` and `public` read from the raw text.
  - Calls: 458 pairs, 454 with an edge. The misses are `stat(…)` twice in `directory.rb` and
    `query_parser()` in `spec_request.rb`, each beside a local of its name, and the call of
    `YAML.unsafe_load`. The store's one edge outside the census, `builder` in `spec_builder.rb`, is
    right; the census misread it.
  - References: 801 pairs, 801 with an edge. The store's one more, `DOCROOT` from
    `defined? DOCROOT`, is right.
  - Extends: 13, 12 with an edge. The miss is `TestProxyRequest < RackRequestTest`, a nested class
    extending the class around it, an edge to the source's container, which the engine drops.
  - Unclaimed: 828 constant references and 8 constant-named bare calls name nothing the repository
    declares (`Minitest`, `File`, `StringIO`; `Array(…)`, `Integer(…)`, `Float(…)`).
  - Requires: 342 literal sites (248 `require_relative`, 51 `require`, 43 `autoload`) and one built
    at run time; 291 reach a file of the repository, 291 edges, none missing, none extra, under the
    step's rule and under Ruby's own (`require_relative` beside, `require` and `autoload` under
    `lib/`).
  - `OwnThenUnique` instead of `OwnFirst`: −69 References, every one to a reopened `Rack` or
    `Rack::Multipart` in another file, all right; none gained.
  - A bare call kept to its innermost container and the top level: −18 Calls, all of
    `Response::Helpers` reaching `Response#get_header`, `set_header` and `has_header?`, right because
    `Response` includes `Helpers`. Not taken.
- **Hand read:**
  - Symbols (seed 19): 50 of 50 right in id, line and end.
  - Five seeded files, counted by hand against the source: `lib/rack/body_proxy.rb` 7 Declares and
    1 Call of 7 and 1; `lib/rack/directory.rb` 19 Declares, 5 Imports, 22 References of 19, 5, 22,
    and 13 Calls of 15, the two misses the `stat = stat(…)` pair above; `test/spec_directory.rb` 3,
    6, 11 and 4 of the same; `test/spec_etag.rb` 3 Declares, 4 Imports, 5 References, 4 Calls of
    the same; `test/spec_sendfile.rb` 5, 4, 7 and 10 of the same.
  - Declares (seed 19): 50 of 50, the three plain ones each after a `private` in its body.
  - Imports (seed 19): 50 of 50, each against its `require_relative` or `autoload` line.
  - Calls (seed 19): 50 of 50. One, `Parser.parse → handle_consume_token`, is made in the instance
    method `parse`, which shares its qualified name with `def self.parse`; the first definition is
    the one symbol, as planned.
  - References (seed 19): 50 of 50. A first sample, before a constant passed over methods, held
    `form_data? → Helpers/POST`, a constant `POST` read as the method `def POST`: wrong. That rule
    moved 3 References to `Rack::POST` and `Rack::GET`, and the census took the same rule.
  - Extends: 12 of 12.
- **P and R per kind** (§3.3; P from the hand read and the census, R from the census):

  | kind | edges | sample | P | R |
  |---|---|---|---|---|
  | Calls | 455 | 50/50 | 1.000 (455 of 455) | 0.9913 (454 of 458) |
  | References | 802 | 50/50 | 1.000 | 1.000 (801 of 801) |
  | Extends | 12 | 12/12 | 1.000 | 0.9231 (12 of 13) |
  | Declares, modules and classes | 159 | 50/50 | 1.000 | 1.000 (159 of 159) |
  | Declares, methods | 726 | | 0.9986 (725 of 726) | 0.9986 (725 of 726) |
  | Declares, constants | 171 | | 1.000 | 1.000 (171 of 171) |
  | Imports | 291 | 50/50 | 1.000 | 1.000 (291 of 291) |

  Extends is thin (|O| under 50), so its row is read whole rather than gated on R. The Declares
  sample of 50 spans all three rows.
- **Regressions** (`log/langdiff-step19-rack.txt`), rack without against with the Ruby globs: rb R1
  1,149 and gemspec R1 1 (every new node), R4 1,346 and 1 (Declares and Imports), Calls +455,
  References +801 and +1, Extends +12, R2 0, R3 0, R5 0, R6 0, R7 0, R9 0. R8 is 1, the pending
  count above.
- **Byte-identity**, `step17` against `ruby`, every build on a copy:
  - Scratchpad corpus and kit corpus, each with its `repograph.toml`, with the default globs and
    with the defaults plus the Ruby globs: `graph.json` identical in all six pairs
    (`dff8b36a53bb0548`, `store-14`'s), `headers.json` identical (`a5470b352b46ebc0`). Neither corpus
    holds a Ruby file.
  - luarocks with `**/*.lua`, and with the Ruby globs too: `368f10ad0de3ba07` in all four builds.
  - gin with `**/*.go`, and with the Ruby globs too: `119169be4bd8aa62` in all four.
  - hiredis with `**/*.c **/*.h`, and with the Ruby globs too: `1396190e7661e0c7` in all four.
- **Fixture and bench:** not run. Ruby is not in the fixture's globs.
- **Licence reading:** `tree-sitter-ruby 0.23.1 MIT ok`, ABI 14. The crate ships no `LICENSE` file;
  its `Cargo.toml` says `license = "MIT"` and its `tree-sitter.json` names Rob Rix. `refs.scm`'s
  locals are adapted from the crate's `locals.scm` and carry its notice, "Copyright (c) 2016 Rob
  Rix, MIT License".
- **Edited tests:** `cases.rs`'s shebang test names `a.hs` instead of `a.rb` as a file ast-grep
  reads and we leave as a file, since `.rb` is now read. `lang.rs`'s
  `all_lists_every_language_once` gains Ruby's slot. No assertion changed.
- **Platforms:** not verified. The branch was not pushed, so `ci` has not run on it.
- **Gate:**
  - `cargo test --all-targets`: 1,460 passed, 0 failed. A first run under load from parallel builds
    failed five `tests/serve.rs` tests on timing; `serve` alone and a second full run passed;

### Step 19 · review fix `6943fdc`

New: `bin/repograph-ruby-fix`, built at `6943fdc`'s source. Parent: `bin/repograph-ruby-rev`, built at
`67e2c99`. The fix keeps `require` and `autoload` to `lib/` and `require_relative` to the requirer's
directory through a new `#set! import` key, takes a path already ending in `.rb` as written, and
stops a value's singleton class declaring its methods after a bare `private`. Plan, step 19 as
built, has each finding.

- **Repro** (`s19r/tiny` in the scratchpad, `**/*.rb`), read by hand against the source:
  - `lib/app/server.rb` with `require "logger"`, `require "app/router.rb"`,
    `require_relative "view.rb"`, `require_relative "app"` and `autoload :Logger, "logger"`, beside
    `lib/app/logger.rb`, `router.rb`, `view.rb` and `lib/app.rb`. `ruby-rev`: Imports to
    `lib/app/logger.rb` and `lib/app.rb`, both wrong. `ruby-fix`: Imports to `lib/app/router.rb` and
    `lib/app/view.rb`, both right.
  - `class << obj; private; def hid; def pub; end` at the top level, `class << other; private def own`
    and `class << self; private; def boot; public; def open; private def secret` in `class A`.
    `ruby-rev` declares `hid`, `pub` and `A.own`, all wrong. `ruby-fix` declares `A.boot` hidden,
    `A.open` exported and `A.secret` hidden, and none of the three.
- **rack, step 19's criterion re-run** (the defaults plus the four Ruby globs, `s19/census.py` and
  `s19/icensus.py`): `graph.json` is byte-identical between the two binaries, so no edge changed
  and there is nothing to hand-read. A text scan of rack finds no `require` or `autoload` of a
  sibling's name that `lib/` lacks, one `require './app.rb'`, in a comment of `builder.rb`, and no
  `private` or `protected` in a `class << value` body.

  | kind | edges | P before → after | R before → after |
  |---|---|---|---|
  | Imports | 291 | 1.000 → 1.000 | 1.000 → 1.000 (291 of 291, under Ruby's rule and under step 19's) |
  | Calls | 455 | 1.000 → 1.000 | 0.9913 → 0.9913 (454 of 458) |
  | Declares, modules and classes | 159 | 1.000 → 1.000 | 1.000 → 1.000 (159 of 159) |
  | Declares, methods | 726 | 0.9986 → 0.9986 | 0.9986 → 0.9986 (725 of 726) |
  | Declares, constants | 171 | 1.000 → 1.000 | 1.000 → 1.000 (171 of 171) |

  Export disagreements 0 in both. The misses are step 19's: the three `stat`/`query_parser` calls
  beside a local and `def YAML.unsafe_load`.
- **Byte-identity**, `ruby-rev` against `ruby-fix`, every build on a copy (`graph.json`, SHA-256
  prefix):
  - leveldb with `**/*.cc **/*.h`: `4daa7236ae432db6`;
  - hiredis with `**/*.c **/*.h`: `c598315d5dbcc0d9`;
  - gin with `**/*.go`: `03a6c87a79dd5bfe`;
  - luarocks with `**/*.lua **/*.c **/*.h`: `5d118eab6ba8aeb6`;
  - Slim with `**/*.php`: `06bfcd802019ae73`;
  - rack with the defaults plus the Ruby globs: `cf4ce541c06dd796`;
  - scratchpad corpus with its `repograph.toml`: `566f40d972ba1d21`, equal to `store-14`'s, and
    `headers.json` `7c73aee1e9db81f6` in both.
- **Cases:** `src/code/cases/ruby.rs` 13 tests (two added): `require` searching `lib/` alone, a
  written `.rb` taken as written and `require_relative` never falling back to `lib/`; a value's
  singleton class declaring nothing with `private` or `private def`, and `class << self` hiding,
  showing and hiding again. One assertion rewritten: `lib/rack/builder.rb`'s `require "rack"` now
  names `lib/rack.rb`, not the `lib/rack/rack.rb` beside it, which pinned the finding. `queries.rs`
  gains a contract test for the key, and `tests/engine.rs` the rooted, relative and suffix rules.
- **Gate:** `cargo test --all-targets` 1,494 passed, 0 failed; `cargo clippy --all-targets -- -D
  warnings` clean. No release build; fixture, bench and platforms not run.

## Step 20 · PHP

New: `bin/repograph-php`, built at the tree committed on `feat/engine-php`. Parent:
`bin/repograph-step17`. PHP is read only when a config or `REPOGRAPH_CODE_GLOBS` names `**/*.php`.

- **Corpus:** `slimphp/Slim` at `4.15.3`, `3675bf6baac66b07032575b7bef4200b60b7974b`, 125 `.php`,
  MIT. It is read as data only: no script of it was run. One root `composer.json`: `Slim\` →
  `Slim`, `Slim\Tests\` → `tests`.
- **Parse census:** 0 of 125 files hold an ERROR or a MISSING node under `LANGUAGE_PHP`, and 0
  under `LANGUAGE_PHP_ONLY`.
- **Cases:** 11 in `src/code/cases/php.rs`: types with members, functions, promoted properties and
  export; PSR-4 by the longest prefix, a prefix with a list of directories, an `autoload-dev` prefix
  and the empty prefix; the nearest `composer.json` deciding even when it maps nothing; resolution
  through `use`, `as`, the namespace's directory and the own class (`$this->`, `self::`,
  `static::`), with `parent::`, a private member and a fully qualified name missing; two classes of
  one name in reach resolving to neither; text before `<?php`; citations; the extension; `widen`
  over the directory and the importers.
- **What PHP writes** (`--no-dense`, the defaults plus `**/*.php`): 1,214 nodes (125 files, 1,089
  symbols: 106 classes, 18 interfaces, 736 methods, 214 properties, 15 constants), Declares 1,089
  (830 exported), Imports 306, Calls 212, References 863 (779 `type`, 84 plain and `impl`), Extends
  53, Implements 22. No edge dangles. The pending count rises by 1, a `PSR-15` citation.
- **Regressions** (`log/langdiff-step20-slim.txt`), Slim without against with the PHP globs: php R1
  1,214 (every new node), R4 1,395 (Declares and Imports), Calls +212, References +863, Extends
  +53, Implements +22, R2 0, R3 0, R5 0, R6 0, R7 0, R9 0. R8 is 1, the pending count above.
- **Census.** A text census over comment- and string-blanked source, independent of the grammar
  (`s20/census.py` in the scratchpad, `log/census-step20-slim.txt`), resolving a short name by
  PHP's rule (the `use` alias, then the own file, then the namespace) and a `use` by the PSR-4
  map, every miss hand-read:
  - Declarations: 1,089 of 1,089 with a symbol, none outside the census, no export disagreement.
  - Imports: 306 of 306, none extra. Six `use Slim\…` lines name classes of other packages
    (`Slim\Http\…`, `Slim\Psr7\…`) with no file here, and write nothing.
  - Calls: 212 of 212, none extra: `$this->m()`, `self::`, `static::` to the own class's method,
    `X::m()` through the class's `use` or namespace, and bare function calls.
  - References: 863 of 863, none extra: type hints, `new X`, `instanceof`, `X::class`, `X::NAME`,
    `self::NAME` and trait uses.
  - Implements: 22 of 22. Extends: 53 of 54, none extra; the miss is `RouteTest extends TestCase`,
    whose file declares `namespace Slim\Tests` from `tests/Routing`, so its directory does not hold
    `tests/TestCase.php`. Composer would not autoload that class by PSR-4 either.
  - Sites no rule claims: 1,375 `$this->m()` of an inherited or PHPUnit method (`prophesize` 451,
    `assertSame` 282), 2,083 `$value->m()`, 509 `X::class` of another package, 179 `X::m()` of
    another package (`Argument::` 177), 247 calls through `use function`, 49 static properties, 4
    `parent::`, 16 `self::` and 5 `X::` sites of an inherited member, 18 `implements`, 6 `extends`
    and 7 trait uses of another package.
- **P and R per kind** (§3.3; P from the hand read and the census, R from the census):

  | kind | edges | sample | P | R |
  |---|---|---|---|---|
  | Declares | 1,089 | 50/50 | 1.000 | 1.000 (1,089 of 1,089) |
  | Imports | 306 | 50/50 | 1.000 | 1.000 (306 of 306) |
  | Calls | 212 | 50/50 | 1.000 | 1.000 (212 of 212) |
  | References | 863 | 50/50 | 1.000 | 1.000 (863 of 863) |
  | Extends | 53 | 50/50 | 1.000 | 0.9815 (53 of 54) |
  | Implements | 22 | 22/22 | 1.000 | 1.000 (22 of 22) |

- **Hand read** (seed 20, `log/samples-step20-slim.txt`):
  - Symbols: 50 of 50 right in id, line and end; every one of the 1,089 ends on a line that closes
    it (`}` or `;`).
  - Five seeded files, counted by hand against the source: `tests/Middleware/ErrorMiddlewareTest.php`
    11/11 symbols and 7/7 imports, `tests/Mocks/CallableTest.php` 5/5 and 1/1,
    `tests/Mocks/MockCustomRequestHandlerInvocationStrategy.php` 3/3 and 1/1,
    `tests/Mocks/MockMiddlewareWithConstructor.php` 4/4 and 0/0 (its six uses are PSR packages),
    `tests/Routing/FastRouteDispatcherTest.php` 17/17 and 2/2.
  - Declares 50 of 50, with export matching the modifier; Imports 50 of 50, each `use` path equal to
    the target's namespace and file name; Calls 50 of 50; References 50 of 50; Extends 50 of 50;
    Implements 22 of 22.
- **Byte-identity** (`log/ident-step20.txt`), `step17` against `php` on copies:
  - Kit corpus and scratchpad corpus, with the default globs and with `**/*.php`, `**/*.lua`,
    `**/*.go` and `**/*.c **/*.h`: `graph.json` and `headers.json` identical in all ten pairs
    (`dff8b36a53bb0548`), and the scratchpad's graph equals `store-14`'s. Neither corpus holds a
    `.php` file.
  - luarocks with `**/*.lua` (`368f10ad0de3ba07`), gin with `**/*.go` (`119169be4bd8aa62`) and
    hiredis with `**/*.c **/*.h` (`1396190e7661e0c7`): identical, and again with `**/*.php` added.
    gin proves the reshaped `ModulePrefix` and `module_of` on Go.
- **Fixture and bench:** not run. PHP is not in the fixture's globs.
- **Size and build time:** not measured, per the wave's rule; the session measures them at merge.
- **Licence reading:** `tree-sitter-php 0.24.2 MIT ok`. Its `LICENSE`: "The MIT License (MIT)
  Copyright (c) 2017 Josh Vera, GitHub". The queries are written to the engine's
  contract, not adapted from the crate's `tags.scm`, so they carry no notice.
- **Edited tests:** `lang.rs`'s `all_lists_every_language_once` gains PHP's slot. No assertion
  changed.
- **Platforms:** not verified. The branch was not pushed, so `ci` has not run on it.
- **Gate:**
  - `cargo test --all-targets`: 1,461 passed, 0 failed;
  - `cargo clippy --all-targets -- -D warnings`: clean;
  - `cargo tree --duplicates --depth 0`: no tree-sitter crate.
