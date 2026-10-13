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
