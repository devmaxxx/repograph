---
name: extractor-case
description: Add or fix a TypeScript/markdown extraction case in repograph — pin the grammar shape, write the failing inline case, fix, then prove the corpus and the bench floors did not move.
---

# extractor-case

Every construct the extractor handles is pinned by one inline case in `src/code/cases.rs`
(code) or the `tests` module of the `src/doc/*.rs` file it concerns (docs). A case is a
source string and the nodes or edges it must yield — never a shared fixture, so a reader sees
the construct and its expected shape together.

## Procedure

1. **Pin the grammar shape before guessing it.** tree-sitter node kinds and field names are
   not obvious (`export * as ns` is a `namespace_export`, a method decorator is a sibling of
   the `method_definition`, `declare const` wraps a `lexical_declaration` in an
   `ambient_declaration`). Print the S-expression:

   ```bash
   REPOGRAPH_DUMP='export * as ns from "./lib";' cargo test dump_tree -- --ignored --nocapture
   ```

   A `.tsx` shape needs `REPOGRAPH_DUMP_TSX=1`.

2. **Write the case first and watch it fail.** Helpers in `cases.rs`: `extract(rel, src)` for
   a file with no neighbours, `Repo::new(&[(path, contents)])` when an import must resolve,
   `ids`, `edges(ex, kind)` and `declares(ex, name)` to assert on. One construct per case;
   name it after the behaviour, not the construct (`ids_in_an_arrow_const_attach_to_the_const`).

   ```bash
   cargo test code::cases
   ```

3. **Fix in the narrowest place.** Member naming lives in `symbols::member_name`, binding
   patterns in `symbols::bindings`, ownership of an id reference in `idrefs::owner`,
   top-levelness in `symbols::is_top_level`, resolution in `imports::Resolver::resolve`.
   Add the arm; do not add a second walk.

4. **Prove nothing else moved.**

   ```bash
   cargo test && cargo clippy --all-targets
   cargo build --release
   target/release/repograph --repo <corpus> --no-dense build   # vectors survive a build
   target/release/repograph --repo <corpus> verify | head -3   # node and edge counts by kind
   target/release/repograph --repo <corpus> bench              # floors: 24/24, 7/14, 3/3
   target/release/repograph --repo <corpus> --no-dense bench   # floors: 24/24, 3/14, 3/3
   ```

   Compare the `verify` counts with the previous build's: a fix that names more symbols moves
   `Symbol` and `Declares`; one that re-attributes references moves `References` without
   moving `Symbol`. A count that moved for no reason you can name is a regression to find, not
   a number to record.

5. **Commit the cases with the fix**, one Conventional Commit, the failing cases named in the
   body by what they showed.

## What not to do

- Do not widen a case until it passes; the case states the wanted behaviour, the code meets it.
- Do not read `tree-sitter-typescript`'s grammar from memory — dump the tree.
- Do not run `build` without `--no-dense` to check counts: with vectors present it costs
  nothing, but without them it re-embeds the corpus (three minutes) for a count you can get in
  four seconds.
