# Generic Code Engine Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan step by step. Each step moves one language and ends at its gate (§5); a step whose gate does not pass does not merge.

**Goal:** One engine reads every language. Detection, parsing, declarations, ids, spans, bodies, citations, path imports and same-name resolution are shared and driven by data. Per-language Rust survives only as a resolve hook, and only where a measurement shows name-only resolution writes wrong edges or loses too many right ones. Max, 2026-10-10: "I don't wanna split code parsing by language, I need unified interfaces to detect and use." After the fourteen existing languages are ported, nine languages repograph does not read today arrive as `Spec`s with queries, data and no hooks, one per step (§6.2): Lua, Go, C, C++, Ruby, PHP, Scala, Elixir and OCaml.

**Architecture:** Each language is a `Spec`: the grammar `lang.rs` already names, two tree-sitter queries compiled once per process from `.scm` files embedded in the binary, a few data fields (comment syntax, body recipe, export rule, visibility, ambiguity) and a `Hooks` table of optional function pointers, which is the successor of PR #170's `Reader`. The engine runs the queries, turns captures into `Facts` (definitions, references, imports, citations, locals), writes nodes and `Declares` edges generically, resolves bare names by the spec's visibility model, and hands the parsed file to whichever hooks the spec names. Old and new paths coexist per language until that language's gate passes. Detection is our own `detect`: overrides, `Lang::of`'s table, then a shebang. Each grammar stays its own pinned crate, and the query runner is ours, seeded from upstream `tags.scm` (§2.8 records the library decisions and why hyperpolyglot fell back to the table).

**Tech Stack:** Rust 2021, `tree-sitter =0.27.0` (`Query`, `QueryCursor`; `streaming-iterator =0.1.9` is already pinned), the grammar crates already in `Cargo.toml`, `bench/probe/graphdiff.py` plus a new scoped classifier, `bench/compare/run.py` for blast clauses, and `repograph bench` on the pinned fixture. §6.2 adds nine grammar crates, checked against crates.io on 2026-10-10 but not yet in `Cargo.toml`.

**Base:** branch `refactor/code-general-pipeline` (PR #170) at `b02f388`. Its `graph.json` and `headers.json` are byte-identical to `store-base` on the 776-file polyglot corpus.

**Supersedes:** the 0.6.0 spec's choice of "a hand walk, not tag queries" (`docs/superpowers/specs/2026-09-11-0.6.0-languages-design.md`). §2.7 explains why the reason given there no longer holds, and lists what queries still cannot say.

**Decisions taken 2026-10-10 (Max):** every recommendation in §9 was accepted:

- doc lines above a signature may change, with one GRAMMAR bump;
- Terraform becomes its own `Lang`;
- all fourteen porting steps run, with a re-read of the numbers at step 9;
- the hook thresholds are P ≥ 0.99 and R ≥ 0.97;
- the private corpora are restored if a copy is found, and otherwise the corpus and cases judge;
- #170 merges first, then one PR per step on `refactor/generic-code-engine`.

Max then asked for:

- other languages, which became §6.2: steps 15–23, one language each, and step 24 to close;
- one library choice per layer, which became §2.8.

Steps 0–14 port; steps 15–24 add.

## Global Constraints

- A missing edge is acceptable; a wrong one is not. The gate (§5) is stricter on added edges than on missing ones.
- Invariants L1–L11 of the 0.6.0 spec hold at every step:
  - no grammar means a file node only (L1);
  - one table picks the grammar (L2);
  - `.` joins members only, and Terraform and Postgres addresses join with `/` (L4);
  - GRAMMAR moves once per release (L5): at step 14, or at step 24 when the new languages ship in the same release;
  - DecoratedBy goes to the declaring symbol (L8);
  - no edge crosses a family (L10).
- `src/code/lang.rs` stays self-contained (model plus grammar crates), because `tests/parse_census.rs` compiles it by path. The spec table sits beside it, never inside it.
- The engine evaluates tree-sitter's built-in predicates (`#eq?`, `#match?`, `#any-of?` and their `#not-` forms) and `#set!` properties, and nothing else. A query that needs a general predicate fails the contract test. This keeps the semantics inside tree-sitter, where they are documented, and out of a predicate dialect of our own.
- A new data field is admitted to `Spec` only when at least two languages use it. Once a field is admitted, adding variants to it is free. Anything only one language needs is a hook. This rule keeps the spec from becoming a per-language switch with extra steps.
- A port reproduces the old reader; it does not improve it. A query captures exactly what the old reader read. Widening a language (new citations, new reference kinds) is a separate change with its own gate.
- A new language (§6.2) arrives as a `Spec` with `hooks: NONE`. It never adds a hook. It may add a variant to a field that is already admitted, or a field that at least two of the new languages use. Either is pinned by cases in its own step and defaults to today's behaviour for every existing language.
- Builds: run `df -h /` first (stop below 10 GiB free), and use `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0` with one shared `CARGO_TARGET_DIR`.
- Writers on the bench fixture run only in `~/bench/beauty-crm-test`, after `bench/probe/reset.sh`. `~/bench/beauty-crm-502e8a6d` is read-only.

## 1 · Where it stands

PR #170 put every language on one `Reader` record (`src/code/reader.rs`): `extract`, `header`, `collect`, `state`, `manifest`, `widen` and `dotted_dirs`, with `..NONE` defaults and type-keyed states. Detection is already one table (`Lang::of`). Everything inside `extract` is still per language. Each family walks its own tree with its own helpers, builds ids, spans and bodies its own way, and resolves its own names.

Excluding cases, that is about 12,400 lines across twelve family directories:

| family | lines |
|---|---|
| Shell | 365 |
| Bicep | 323 |
| HCL | 356 |
| Swift | 195 |
| GraphQL | 356 |
| SQL | 445 |
| Dart | 688 |
| Python | 906 |
| Rust | 1,304 |
| JVM | 3,294 |
| .NET (Razor 817 of it) | 2,706 |
| TypeScript (Vue 238 of it) | 1,495 |

The shared modules add another 1,749 lines (`prose`, `syntax`, `reader`, `imports`, `index`, `mod`, `lang`).

The corpus base graph (`store-base`) has 9,885 nodes, 19,757 edges and 1,623 pending. It splits by file extension as follows. A language with fewer than 50 edges of a kind here cannot decide that kind on the corpus alone (§3.3).

| ext | files | symbols | Declares | Calls | References | Imports | other |
|---|---|---|---|---|---|---|---|
| ts | 174 | 1,522 | 1,462 | 911 | — | 219 | DecoratedBy 1,103, Extends 40, Implements 9, ReExports 26 |
| tsx | 160 | 579 | 578 | 283 | — | 100 | DecoratedBy 2, Extends 4 |
| sh | 107 | 141 | 141 | 192 | 11 | 15 | — |
| py | 54 | 1,123 | 1,123 | 841 | — | 28 | Extends 5 |
| bicep | 44 | 1,379 | 1,379 | — | 4,344 | 66 | — |
| kt | 43 | 460 | 461 | 155 | 73 | 59 | Extends 15 |
| sql | 42 | 1,715 | 1,715 | 145 | 388 | — | — |
| graphql | 42 | 192 | 192 | 69 | 0 | — | — |
| cs | 40 | 498 | 498 | 324 | — | 20 | — |
| rs | 38 | 1,107 | 1,107 | 1,068 | — | 3 | ReExports 2 |
| dart | 10 | 239 | 239 | 76 | — | 18 | — |
| vue | 8 | 46 | 46 | 7 | — | 2 | Extends 4 |
| tf | 6 | 62 | 62 | — | 66 | — | — |
| java | 4 | 35 | 35 | 14 | — | 1 | — |
| swift | 4 | 11 | 11 | — | — | — | — |

The corpus has no Razor files and no HCL files that are not Terraform.

The reviewer named three defects on `b02f388`. All three come from the per-language `extract`, and the engine's front removes them by construction (§4):

- **Vue reads each file twice.** Its `collect` only records the path, but `Resolver::new` reads the source anyway in order to call it.
- **`reader::open(Lang::X, …)` names the grammar a second time.** `reader(lang)` has already chosen the reader by that same `Lang`.
- **TypeScript's `*ex = symbols::scan(…)` replaces the whole extraction.** The file node is discarded and rewritten with the file head as its body. TypeScript also parses the same text three times, once each in `symbols`, `calls` and `idrefs`.

Two readers also reach across files at extract time. Bicep's `modules.rs` parses each module target to list its top-level names, and HCL's `Modules::add` parses sibling `.tf` files. The engine gives both a per-build memo of parsed facts instead (§3.1).

## 2 · The engine

### 2.1 One spec per language

```rust
pub(crate) struct Spec {
    pub queries: &'static [&'static str], // .scm sources in order; TSX lists TypeScript's then its own
    pub view: Option<Lang>,               // whose grammar and queries read an embedded view
    pub comments: CommentSyntax,          // what `changes` uses to tell a comment-only hunk
    pub body: Body,
    pub export: Export,
    pub visibility: Visibility,
    pub ambiguity: Ambiguity,
    pub imports: ImportPaths,             // suffix candidates and roots for path imports
    pub cross_file: bool,                 // serve rebuilds the resolver when such a file changes
    pub dotted_dirs: bool,                // kept from Reader
    pub hooks: Hooks,
}
pub(crate) fn spec(lang: Lang) -> &'static Spec;
```

`src/code/spec.rs` holds the table, with one `match` from `Lang` to a `const` per language. Each field has at least two users:

- `view`: Vue (TypeScript or TSX) and Razor (C#).
- `comments`: every language. It absorbs `changes.rs`'s `comment_syntax` match.
- `body`, `export`, `visibility`, `ambiguity`: every language.
- `imports`: Shell, Bicep and HCL path imports.
- `cross_file`: Shell, Bicep, HCL and Terraform. It absorbs `main.rs`'s `path_family`. The flag is explicit because the set is not the `widen` set: Bicep has no `widen`.
- `dotted_dirs`: kept from `Reader` as it is.

### 2.2 Query files and the capture contract

Queries live in `src/code/queries/<lang>/{definitions,references,imports,locals}.scm`. `src/code/queries.rs` embeds them with `include_str!` (`Cargo.toml` has no `include` list, so `cargo package` takes them). They are never read from disk at runtime. Razor and Vue have no files of their own; they use their view's. A `.gitattributes` line `*.scm text eol=lf` keeps a Windows checkout from changing query text.

| capture | on | meaning |
|---|---|---|
| `@definition.<kind>` | the node whose rows are the symbol's span | one symbol; `<kind>` comes from the closed list below |
| `@name` | inside a definition, scope or reference match | the name's text; repeated `@name` captures join with `/` in source order (`aws_s3_bucket/b`, `schema/table`) |
| `@owner` | inside a definition | an explicit qualifier that replaces containment (Kotlin `fun Foo.bar()` is `Foo.bar`) |
| `@scope.owner` | a container that writes no node | qualifies nested definitions by its `@name` (Swift `extension`, Rust `impl`) |
| `@scope.namespace` | a package or namespace | a header `scope` line; never part of an id |
| `@modifier` | a modifier or keyword token | text compared with `spec.export` |
| `@doc` | a doc node | the doc part of the body when `body.doc` is `Capture` |
| `@reference.<edge>` | a reference site | `<edge>` is `calls`, `references`, `extends`, `implements` or `decorated_by`; the edge context comes from `#set! context` |
| `@receiver` | inside a reference | marks a member access; default resolution leaves it to a hook |
| `@import` with `@import.path`, `@import.name`, `@import.alias` | an import site | `#set! reexport` makes it a ReExports edge |
| `@local.scope`, `@local.definition` | | shadowing, in the nvim locals convention |
| `@cite.comment`, `@cite.string` | comment and string nodes | text scanned with `ids::generic().find_all`, written as References `comment` / `string` |
| `@_…` | | private to a predicate, by tree-sitter convention |

**Definition kinds are a closed list, and the kind decides the join:**

- **Type-like containers join with `.`:** class, struct, interface, enum, trait, protocol, object, record, type, and Bicep's resource.
- **Module-like containers join with `/`:** module (Rust `mod`).
- **Leaves qualify nothing:** function, method, field, property, constant, variable, value, output, block.

A function nested in a function is qualified by its nearest container, if it has one. This matches Shell today, where every `function_definition` is a top-level id.

**Properties** (`#set!`) have a closed key set:

- `prefix` is prepended to the name: `data/`, `var/`, `local/`, `output/`, `module/`, `provider/`.
- `context` is the edge context.
- `export` is `always` or `never`.
- `reexport` marks an import as a re-export.
- `visibility` overrides `spec.visibility` for one reference pattern. Step 19 adds it (§6.2).

Contexts form a closed list, which is the list in use today: `""`, `export`, `arg`, `get`, `*`, `comment`, `string`, `alter`, `impl`, `extend`, `on`, `type`, `variable`, `argument`, `member`.

**The contract test** (`src/code/queries.rs`, `#[test]`) checks every language with queries, and fails unless:

- every query compiles against its grammar;
- every capture name is in the table above;
- every `#set!` key and every `context` value is in its closed list;
- `general_predicates(i)` is empty for every pattern `i`;
- every `@definition.*` and `@scope.*` pattern captures `@name`.

### 2.3 Qualification, ids and Declares

1. **Collect.** Gather the definition and scope matches. When two patterns match the same node with the same role, the pattern that comes first in file order wins. Queries put specific patterns before general ones.
2. **Find the parent.** Sort by `(start byte, Reverse(end byte))` and walk with a stack. A definition's parent is the innermost enclosing container: a container-kind definition or a `@scope.owner`.
3. **Qualify.** `qualified` is the parent's `qualified`, then the parent kind's join, then `prefix`, then `name`. An `@owner` capture replaces the parent's part.
4. **Id and label.** The id is `sym:{rel}::{qualified}`. The label is the text after the first `::`, as `prose::declare` writes it today.
5. **Declares.** The edge runs from the parent's node, or from `file:{rel}` when there is no parent or the parent is a `@scope.owner`. Its context is `export` when the export rule says so, otherwise `""`.
6. **Broken names.** A definition whose name node is missing or has an error is dropped. This is `syntax::broken`'s rule.
7. **Duplicate ids.** The first in source order wins, which is what `finish()` already does (JVM overloads).

This generalizes `prose::Spans::owner`. The same containment gives each reference and citation its source: the innermost definition containing the site, otherwise the file.

**Export** (`spec.export`) is one of:

- `Words { show }`: exported only with a listed keyword (TypeScript `export`, Rust `pub`).
- `Words { hide }`: exported unless a listed keyword is present (Swift `private`/`fileprivate`, Kotlin `private`).
- `Prefix('_')`: a leading underscore hides the name (Python, Dart).
- `All`: everything is exported (GraphQL, HCL).

A pattern's `#set! export` overrides the rule (Bicep `param` and `output`, `@export()`; Terraform `var/` and `output/`). A hidden name is left out of the header's `top`, as `Header.private` does today.

### 2.4 Spans and bodies

A symbol's `line` is the `@definition` node's start row + 1, and its `end` is the end row + 1 (`syntax::text_span`). The query picks the node, and that choice is what puts decorators inside a span or leaves them out: Python's `decorated_definition` versus `function_definition`.

A body is doc lines followed by one signature line. Both passage indexes read only the last line (`Node::indexed_body`), so the signature line is what retrieval sees. The doc lines reach only:

- rerank (`src/rerank.rs:25`);
- `query.rs`'s matching line and explain output (`src/query.rs:261,316`).

The recipe has four fields, and each variant below has at least two users today:

- **`doc`:**
  - `Adjacent { kinds, between }`: comment nodes directly above the definition. `between` lists node kinds allowed in between (Bicep decorators). Shebangs and trailing comments are excluded, as in `prose::body`.
  - `Capture`: the `@doc` node (Python docstring, GraphQL description).
  - `None`.
- **`form`:** `Raw` keeps the markers (Shell, Bicep, HCL). `Stripped` removes `//`, `///`, `/** */` and `#` (JVM, TypeScript, Rust).
- **`cap`:** `Tail` keeps the last 600 bytes (prose). `Head` keeps the first 600 characters (JVM, TypeScript). 600 is `syntax::DOC_CHARS`.
- **`line`:** `First` is the node's first line (prose). `Name` is the line holding `@name` (JVM).

Swift's "first line not starting with `@`" has one user. Its port tries `Name` first. If any Swift body's last line changes, Swift gets a `signature` hook instead of a new variant.

Each port starts by writing its old recipe down in these four terms. A recipe that does not fit becomes that language's hook. It never becomes a new field with one user.

A file node's body is empty, except TypeScript's file head, which is the `head` hook (§3.2).

### 2.5 Facts

```rust
pub(crate) struct Facts {
    pub defs: Vec<Def>,       // id, qualified, name, kind, byte range, line, end, parent, export, body
    pub refs: Vec<Ref>,       // source (def index or file), name, receiver, edge, context, byte range
    pub imports: Vec<Import>, // path, names, alias, reexport, byte range
    pub cites: Vec<Cite>,     // context, byte range
    pub locals: Locals,
}
pub(crate) struct File<'a> { pub lang: Lang, pub rel: &'a str, pub src: &'a str, pub tree: &'a Tree, pub facts: &'a Facts }
```

`Facts` is the interface between the shared part and the hooks. A hook reads the declared names, supertypes and member references it needs from `Facts` instead of walking the tree a second time. It still has the tree for whatever a query cannot say.

### 2.6 Compilation and caching

Each language has two compiled queries:

- **`decl`:** definitions, scopes, doc and imports. Both the collect pass and extract use it.
- **`refs`:** references, locals and citations. Only extract uses it.

The `.scm` sources are concatenated in a fixed order before compiling, so one cursor pass covers each query.

- **When and where they compile.** Lazily, on first use, into `static COMPILED: [OnceLock<Compiled>; N]` indexed by `Lang`. `Query` is `Send + Sync`, and each rayon worker makes one `QueryCursor` per file.
- **What compiling precomputes.** Each capture index maps to a `Role`, and each pattern's properties parse into a `PatternMeta`. The work per match is then integer lookups.
- **Unknown capture names.** A capture name outside the contract panics at compile; the contract test makes that a test failure first.
- **Pathological files.** `QueryCursor::set_match_limit` bounds runaway matching. A file whose cursor reports `did_exceed_match_limit()` is named once on stderr and keeps what it matched. The hand walks are iterative because of 50,000-deep lists; the cursor iterates in C, so depth no longer recurses in Rust, but long flat lists still cost time.

Step 0 times three things: compile time per grammar (Postgres is the largest grammar, +14.6 MB stripped), the deepest corpus file, and the whole corpus build.

### 2.7 What queries can say, and what they cannot

The 0.6.0 spec rejected queries because `tags.scm` gives definitions only: no Extends, no visibility, no field types. That holds for the tags convention, not for queries. With captures of our own:

- `@reference.extends` captures supertypes;
- `@modifier` carries visibility;
- a field's type node is `@reference.references` with `#set! context "type"`.

What queries cannot say, and where it goes instead:

- **The absence of a child** (no `export` keyword). Only `!field` negation exists. An optional `@modifier` plus `spec.export` handle it.
- **Comment adjacency.** Tags does it with the general predicate `#select-adjacent!`. Here it is `Body::Adjacent`, in the engine.
- **Computed paths:**
  - Shell's `$(dirname "$0")/x.sh` and `BASH_SOURCE`;
  - Python package roots;
  - Dart `package:` URIs;
  - tsconfig paths.

  These go in the `import` hook.
- **Anything across files** (partial classes, `part of`, the Rust module tree, `global using`). These go in a `resolve` hook with state.
- **Receiver types** (`a.b()` where `a`'s type is inferred). These go in a `resolve` hook (JVM, C#, TypeScript, Rust, Python, Dart).
- **Name folding** (SQL's unquoted names). This goes in the `fold` hook.

So the engine wins detection, parsing, declarations, ids, spans, bodies, citations, path imports and same-name resolution. The heavy languages keep most of their resolver Rust, which now reads `Facts` instead of a second walk. That is the honest size of the win. The shared part is real, and what stays per language is resolution, which is where languages genuinely differ.

### 2.8 Libraries

Max asked whether existing libraries could do detection and parsing, and decided one choice per layer on 2026-10-10. Each library below was checked that day against crates.io and its published source; nothing was added to `Cargo.toml`. A library enters only where it moves no existing graph, or moves it only within §5.3's accepted classes.

| layer | decided | outcome |
|---|---|---|
| Detection | hyperpolyglot behind our own `detect`, with our overrides on top | hyperpolyglot is unusable (below), so the decision's fallback applies: `Lang::of` plus a shebang rule, behind the same `detect` |
| Grammars | one pinned crate per grammar | as today; no grammar bundle |
| Parsing | tree-sitter | `=0.27.0`, unchanged |
| Extraction | our own query runner, query files seeded from upstream `tags.scm` | §2.2–§2.6; no `tree-sitter-tags` |
| Resolution | the default, with hooks only where §3.3 says | §3 |

**Why hyperpolyglot is unusable.** Its licence and MSRV fit: it is MIT OR Apache-2.0, and edition 2018 with no `rust-version`, so Rust 1.98 builds it. It is unmaintained, and it would also move work the engine has just removed:

- **Releases.** The newest is 0.1.7, of 2020-07-26. The repository's last commit, 2023-05-17, was never released, and its eight open issues (the oldest from 2021) have no answer. `hyperpolyglot_fork 0.1.7` (2025-05-05) bumps `phf` and nothing else.
- **Data.** It carries linguist's data from 2020. It has no Bicep, knows `.tf` as HCL, and knows `.cshtml` only as `HTML+Razor`.
- **API.** Its one public function, `detect(&Path)`, opens and reads the file itself whenever the extension is ambiguous. That is a second read of the kind §1 removes from Vue. It also errors on a path that is not on disk, which `changes` passes for a deleted file. Issues #10, #12 and #16 ask for a text API, or for the detectors to be public.
- **Dependencies.**
  - `clap 2.33` is a normal dependency, because the binary ships in the same package. It brings `atty 0.2.14`, which has RUSTSEC-2021-0145 and RUSTSEC-2024-0375.
  - `serde_yaml 0.8` brings `yaml-rust 0.4.3`, which has RUSTSEC-2024-0320.
  - `pcre2` builds the PCRE2 C library.
  - The classifier's token table is 7.6 MB of generated source.

**The detection seam,** built in step 0:

```rust
pub fn detect(rel: &str, head: Option<&str>) -> Option<Lang>;
```

`detect` tries three sources in order, and the first answer wins:

1. **Overrides.** An ordered list of our own path rules, which win over everything below. In step 0 it holds only what the walk already decides beside `Lang::of`, moved unchanged: manifests that a `hooks.manifest` claims, and the `dotted_dirs` rule. A later rule, for example scripts under `.husky/`, is added with its own case and its own gate.
2. **The table.** `Lang::of(rel)`, unchanged (L2).
3. **Shebang.** It is read only when the table has no answer and the caller passes the text. The interpreter on the first line decides, in direct (`#!/bin/bash`) and `env` (`#!/usr/bin/env -S python3`) form:
   - `sh`, `bash`, `dash` and `zsh` mean Shell;
   - `python`, `python2` and `python3` mean Python;
   - `node`, `deno`, `bun` and `ts-node` mean TypeScript.

   Each new language adds its interpreters in its own step: `lua`, `ruby`, `php`, `scala`, `elixir` and `ocaml`.

Who passes the text:

- `read` and `Resolver::new` hold it and pass `Some`.
- Path-only callers pass `None` and keep today's answer: `index.rs`, `imports.rs`, `main.rs` and `changes.rs` for a deleted file.

The shebang rule moves no existing graph:

- every default glob names an extension, so no extensionless file reaches `detect` in a default build;
- neither the polyglot corpus nor the fixture holds an extensionless file with a shebang (checked 2026-10-10).

It matters only to a user whose globs name extensionless scripts, and step 0's byte-identical gate holds it there.

**A detection library can come later** as a fourth source after the table, with no caller changing. On any file where it disagrees with the overrides or the table, ours wins. The step that adds it lists every disagreement on the polyglot corpus and the fixture in the results document, and its graph gate is step 0's. The maintained candidate is `linguist 0.1.16` (MIT, MSRV 1.88, published 2026-09-08), which Q7 asks about:

- it takes the text as an argument, `disambiguate(path, &str)`;
- its data is current linguist, Bicep included;
- it has content heuristics for `.h`;
- it has no shebang detection.

**Grammars.** Rejected:

- **`tree-sitter-language-pack 1.21.0`** (MIT, 2026-10-04). Its default features download pre-compiled parsers on first use and load them with `libloading`. A build would then depend on the network and on binaries we do not pin.
- **`ast-grep-language 0.50.0`** (MIT, published 2026-10-09). Its tree-sitter requirement, `^0.27.0`, would accept our pin. What it pins is its own grammar set and ranges:
  - `tree-sitter-php ^0.24`, against step 20's 0.25;
  - `tree-sitter-haskell ^0.23`;
  - no Bicep, PostgreSQL or GraphQL crate.

  Its `SupportLang::from_path` reads the extension only, as `Lang::of` does.

**Extraction.** A port's first query file is seeded from the pinned crate's `queries/tags.scm` where one ships:

- shipped: TypeScript, C#, Kotlin (`kotlin-sg`), Java, Rust, Python, Dart, Swift, and all nine new languages;
- not shipped: PostgreSQL, GraphQL, Bash, Bicep and HCL.

The seed is renamed to §2.2's captures and stripped of general predicates (§6.2). It is then extended with `@owner`, `@scope.*`, `@modifier`, locals, imports and citations. For a port, the seed is a draft, not a widening: an upstream pattern that captures more than the old reader read is cut before the step's gate.

Not adopted:

- **`tree-sitter-tags 0.27.1`** (2026-10-08) requires `tree-sitter ^0.27.1`, which the `=0.27.0` pin refuses; 0.27.0 would fit. Either way it returns a flat list of tags: name, kind, span, docs and whether it is a definition. It has no containment, so no qualification and no ids. It has no export rule, imports, reference contexts, properties or bodies of our shape. The capture contract would be rebuilt on top of it.
- **`tree-sitter-loader 0.27.1`** compiles grammars at runtime with the user's C compiler, and also requires `^0.27.1`.

**Also rejected:**

- **tokei 15.0.0** (MIT OR Apache-2.0, 2026-09-06) has a line counter's language table: extension, filename and shebang, with no content heuristics. Its non-optional dependencies include `rayon`, `dashmap`, `grep-searcher` and `clap-cargo`.
- **SCIP indexers** need each language's own toolchain and run out of process.
- **stack-graphs** was archived by GitHub on 2025-09-09 and is no longer supported upstream.

## 3 · Resolution

### 3.1 What the engine resolves by default

The default applies to references that have no `@receiver`, for edge kinds that no hook owns.

1. **Shadowing.** A reference is not resolved when a `@local.definition` binds its name in an enclosing `@local.scope`. This covers Bicep's and Python's shadowing today.
2. **Candidates,** by `spec.visibility`:
   - `File`: definitions in the reference's own file with that qualified name (Bicep).
   - `Imported { transitive }`: the own file, plus files reached by its resolved imports, transitively for Shell's `source`.
   - `Directory`: the own file, plus sibling files of the same language (a Terraform module is a directory).
   - `Family`: the own file, plus the family's `QualifiedIndex` by name, with hidden names excluded (Swift, GraphQL, SQL).
3. **Choice,** by `spec.ambiguity`:
   - `OwnFirst`: a match in the own file shadows the others; otherwise every candidate (Swift Extends).
   - `Unique`: exactly one candidate across the own file and the visible files, otherwise nothing (Shell calls).
   - `All`: every candidate (GraphQL `extend type`).
4. **Write.** The edge runs from the reference's source to `sym:{file}::{name}`, with its kind and context. A self edge is never written. An edge to the source's own container is dropped, as GraphQL does today.

**Imports.** The default import resolution:

- tries `@import.path` relative to the importer's directory, then relative to the repo root when `imports.root` is set, with each `imports.suffixes` candidate;
- keeps a path only when it is inside the repo and in the walk's file set;
- excludes registry references by predicate in the query (Bicep `#not-match? @import.path "^(br|ts)[:/]"`);
- writes Imports file→file with context `*`, or ReExports when the pattern sets `reexport`.

**Headers.** The default header comes from `decl`:

- `scope` from `@scope.namespace`;
- `top` from the top-level definitions that are not hidden;
- `nested` types, members and values from containment.

A family whose header carries more (arities, supertypes, directives) keeps a `header` hook. `headers.json` must not move (§5.3, R7), so no language gains a header in this plan.

**Cross-file reads at extract time.** `Resolver::facts(rel)` parses a file's `decl` facts on first request and memoizes them for the build (a `Mutex<HashMap<String, Arc<Facts>>>`). Bicep's module targets and HCL's sibling files are then parsed once per build rather than once per importer, and `headers.json` is unchanged.

### 3.2 The hook table

```rust
pub(crate) struct Hooks {
    // collect pass, in Resolver::new
    pub register: Option<fn(&mut Resolver, &str)>,                    // path only; the walk does not read the file
    pub collect: Option<fn(&mut Resolver, &File) -> Option<Header>>, // the front parsed it once
    pub header: Option<fn(&File) -> Header>,
    pub manifest: Option<Manifest>,
    pub state: Option<fn() -> Box<dyn Any + Send + Sync>>,
    // extract pass
    pub embed: Option<fn(&str) -> View>,                              // Vue and Razor blanking
    pub head: Option<fn(&File) -> String>,                            // file node body (TypeScript)
    pub fold: Option<fn(&str) -> Cow<str>>,                           // definition and reference names (SQL)
    pub signature: Option<fn(&File, &Def) -> String>,                 // a body line no recipe gives
    pub import: Option<fn(&Resolver, &File, &Import) -> Vec<String>>, // computed and package paths
    pub resolve: Option<fn(&Resolver, &File, &mut Sink)>,
    pub owns: &'static [EdgeKind],                                    // kinds the default leaves to `resolve`
    // update
    pub widen: Option<fn(&[String], &[String], &Graph, &[String]) -> Vec<String>>,
}
pub(crate) const NONE: Hooks = Hooks { register: None, /* … */ owns: &[], widen: None };
```

**A fn table, not a trait.** PR #170 just converged on this shape: `const` statics, `..NONE` defaults, type-keyed states, and no object-safety work. A trait would make every language a type and add nothing the table lacks, because every piece of per-language data is already a state. A language that needs nothing writes `hooks: NONE`.

**`Sink`** wraps `&mut Extraction` and exposes only `node` and `edge`. A hook can append but never replace, which makes the TypeScript overwrite impossible by type.

**Signatures change in two places.** `collect` and `header` take `&File` instead of `(rel, source)`, so they no longer parse for themselves. `register` exists so that Vue's path bookkeeping needs no read.

**Collecting files are still parsed twice per build:** once in the collect pass and once in extract, the same count as today. Keeping trees across the two passes would cost memory in proportion to the repo, so it is out of scope unless a build-time gate fails.

**`owns`** makes the hook criterion concrete. Handing an edge kind back to the default means removing it from `owns`, which is a one-line change measured by §3.3.

### 3.3 When a language keeps a hook

The criterion is applied per language L and per edge kind K in {Calls, References, Extends, Implements, DecoratedBy}. Compare N, L on the new path with K resolved by default, against O, L on the old path. Both come from the same binary through `REPOGRAPH_LEGACY` (§5.2). Compare L's files only.

1. `added` = N ∖ O and `missed` = O ∖ N, with edges compared as `(from, to, kind, context)`.
2. **Hand read.** Every added edge is read by hand when `|added| ≤ 50`. Otherwise a seeded sample of 50 is read and the wrong count is scaled up. An added edge read as right is a find of the new path, not an error.
3. **Precision and recall.** P = 1 − wrong / |N| and R = |N ∩ O| / |O|.
4. **The default keeps K** when all of these hold:
   - P ≥ 0.99;
   - R ≥ 0.97;
   - no edge dangles (R5);
   - L's blast clauses hold at their 0.6.0 bars, where L has public cases (§5.5);
   - the bench has no flip, where L is in the fixture (§5.4).

   Otherwise K goes into L's `hooks.owns`, and the hook must reproduce O exactly.
5. **Thin corpora.** When `|O| < 50` on the corpus, the corpus cannot decide. This covers Swift Extends (0), Java Calls (14), Vue Calls (7), Shell References (11), Python Extends (5), Kotlin Extends (15) and every Razor kind. The default then keeps K only if L's cases (the family's `cases.rs` and `tests/<lang>.rs`) pass with no assertion edited; otherwise K is a hook.
6. **Imports and ReExports are not on this list; they are strict.** A wrong import edge widens the wrong files, and a missing one drops a whole file from impact. The default import resolution is used only where it reproduces O exactly; anywhere else L has an `import` hook.

Each decision is recorded in the results document (§5.6), with the hand-read edges listed.

### 3.4 Expected hooks

This is a prediction, not a decision; §3.3 decides. It sizes the steps.

| lang | predicted hooks | why |
|---|---|---|
| Shell | `import` (`paths.rs`, and runs by path written as References to `file:`), `collect` (scripts state), `widen` | path evaluation; scripts state |
| Bicep | `resolve` owning nothing, writing a module's reach (References to each top-level name of the target) | one user, so not a field |
| HCL, Terraform | `widen` (Terraform); `resolve` only if `Directory` misses Terraform addresses | addresses are `@name` parts with a `prefix` |
| Swift | none | `Family` with `OwnFirst` |
| GraphQL | none | the default header is its header |
| SQL | `fold` | unquoted names fold to lowercase |
| Dart | `collect` (libraries, `part of`, show/hide, pubspec), `import`, `resolve` owning Calls and Extends | library visibility |
| Python | `collect` (packages, `__all__`), `import`, `resolve` owning Calls | attribute types, package roots |
| Rust | `collect` (crate module tree), `import` (`use` flattening), `resolve` owning Calls | impls, fields, macros, derives |
| Kotlin, Java | `header` (arity, supertypes), `resolve` owning Calls | the JVM's own lookup, overloads, arity |
| C# | `collect` (csproj, `global using`, partial types, extension methods), `header`, `resolve` owning Calls | compiler lookup order |
| Razor | `embed`, `collect` (directives), `header`; resolution through C#'s | blanking (L6) |
| TS, TSX | `head`, `import` (tsconfig tiers, package exports), `resolve` owning Calls and DecoratedBy | receivers; `deco:<Name>` is a shared node (the L8 exception) |
| Vue | `embed`, `register`, `resolve` writing the component symbol | the component is not in the script |

## 4 · Detection and the front

```rust
pub fn detect(rel: &str, head: Option<&str>) -> Option<Lang>; // overrides, Lang::of's table (L2), then a shebang (§2.8)
pub(crate) fn spec(lang: Lang) -> &'static Spec;
pub(crate) fn read(resolver: &Resolver, rel: &str, source: &str) -> Extraction;
```

`read` is the only front:

1. **Detect.** `detect(rel, Some(source))`. With no language, the file gets a file node only (L1).
2. **Embed.** With `hooks.embed`, an unread view means a file node only. A read text is parsed with `spec.view`'s grammar and queries. With no view, the file gets a file node, as Vue does today.
3. **Parse once.** A failed parse means a file node only.
4. **File node.** Its body is `hooks.head`'s output, or empty.
5. **Queries.** Run `decl` and `refs` to get `Facts`. Apply `hooks.fold` to every name.
6. **Write.** Definitions, their Declares edges and citations.
7. **Imports.** By default, or through `hooks.import` for each site it handles.
8. **References.** Default resolution for the kinds not in `hooks.owns`.
9. **Hook.** `hooks.resolve`, with a `Sink`.
10. **Finish.** `finish()`.

While a language's spec still holds `legacy: Some(old_extract)`, `read` routes that language to the old reader after step 1. The front owns the parse for old readers too: from step 0, their `extract` takes the `Tree`.

`Resolver::new` reads each walked file once at most. It runs `detect` and `spec`, then:

- `register` with no read;
- or `collect` (read, parse, `decl`);
- and `manifest` for manifests, before sources, as today.

What callers see:

- **`CodeExtractor::extract`** calls `read`.
- **`changes.rs`** replaces its `comment_syntax` match with `spec(lang).comments`.
- **`main.rs`:**
  - `path_family` becomes `spec(lang).cross_file`;
  - the `widen` loop iterates over specs;
  - `files_only_note` is unchanged.
- **`tests/parse_census.rs`** is untouched.
- **The `dump_tree` case** gains `REPOGRAPH_DUMP_CAPTURES=1`, which prints every match's captures and properties. Every port writes its queries with it.

## 5 · The gate

### 5.1 The kit

Step 0 copies the corpus (a 6.6 MB git repo), `store-base` and `repograph-base` from this session's scratchpad to `~/bench/engine-2026-10-10/`, because `/private/tmp` does not survive a reboot. It confirms that `graphdiff.py` reads zero between the copied `store-base` and a fresh build by `repograph-base` there. `K` below is that directory.

### 5.2 The procedure

Every commit on a step runs this. `<exts>` are the step language's extensions.

```bash
df -h /                                                  # stop below 10 GiB free
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test
cargo build --release
B=target/release/repograph
rm -rf $K/corpus/.repograph
REPOGRAPH_LEGACY=<lang> $B --repo $K/corpus build --no-dense && mv $K/corpus/.repograph $K/old-<step>
$B --repo $K/corpus build --no-dense && mv $K/corpus/.repograph $K/new-<step>
python3 -I bench/probe/graphdiff.py $K/store-base/graph.json $K/old-<step>/graph.json   # old path untouched
python3 -I bench/probe/langdiff.py --lang <exts> $K/old-<step> $K/new-<step>
cmp $K/old-<step>/headers.json $K/new-<step>/headers.json
```

`REPOGRAPH_LEGACY` takes a comma-separated list of languages, or `all`, and forces the old path for them. O and N then come from one binary, so the gate does not drift when `main` moves under the branch. When `main` changes graph output, `store-base` is rebuilt with `REPOGRAPH_LEGACY=all` at the new base. Step 14 removes the variable.

`bench/probe/langdiff.py` is new in step 0, with a test beside `test_graphdiff.py`.

- **Input.** It reads two stores and assigns each node, and each edge (by its `from` node), to a language by file extension.
- **Output.** It prints the class counts of §5.3 per language, the added and missed edge lists per kind, and `--sample 50 --seed N` for the hand read.
- **Exit codes.** 0 when every regression count is zero, 1 otherwise.

**Build time** is measured at steps 0, 6, 9, 13 and 14, as the median of three `build --no-dense` runs on the corpus. It must stay at or below 1.20× the step-0 base median.

### 5.3 What a diff may contain

**Regressions.** Each count must read zero:

| | |
|---|---|
| R1 | a node id added or removed |
| R2 | a change in a node's `kind`, `label`, `file`, `line`, `end` or `files` |
| R3 | a change in a body's last line, which is what both indexes read |
| R4 | a Declares, Imports or ReExports edge added or removed |
| R5 | an edge to an id no node declares |
| R6 | any change in a node or edge of another language's files |
| R7 | `headers.json` not byte-identical |
| R8 | a change in the pending count |
| R9 | a change in `community` in a commit whose edges did not change |

**Accepted and reported:**

- body lines above the last line;
- `community` changes in a commit with bounded edge changes.

**Bounded:** Calls, References, Extends, Implements and DecoratedBy from L's files. Each kind is either unchanged, or within §3.3 with its hand read recorded.

**Tests:** every test passes with no assertion edited. An edited assertion is allowed only when the results document lists it under an accepted class.

### 5.4 The bench

The bench runs at the steps whose language is in the fixture: Shell, HCL (`.tf`), SQL, Kotlin and TypeScript (`.ts`, `.tsx`, `.mjs`). It also runs at step 0 (the base) and at step 14. `run-repograph.sh` reads the fixture's own store and does not rebuild it, so the store is built here in the writable worktree:

```bash
BIN=$PWD/$B REPOGRAPH_EMBED_MODEL=<declared> bench/probe/reset.sh
$B --repo ~/bench/beauty-crm-test build --no-dense
python3 -I bench/probe/langdiff.py --lang <exts> $K/fixture-base ~/bench/beauty-crm-test/.repograph
$B --repo ~/bench/beauty-crm-test --no-dense bench
```

Step 0 records `fixture-base` with the base binary and records the base per-case results.

**Floors under `--no-dense`:**

- keyword ≥ 39/40;
- paraphrase ≥ 7/30;
- code 12/12;
- p90 ≤ 250 ms;
- zero per-case flips against the step-0 base.

Steps 13 and 14 also run the dense arm with a full build in the worktree. Its floors are keyword 40/40, paraphrase ≥ 18/30 under gemma (≥ 9 under the small model), code 12/12 and p90 ≤ 250 ms. All twelve code cases are TypeScript, which is why TypeScript goes last.

### 5.5 Blast

`bench/compare/run.py` runs only when a step leaves a bounded kind changed for L. It runs on L's public corpus at its pin, with the clauses and bars in `docs/bench/2026-09-11-0.6.0-results.md`, and no case may read below its base. A step whose graph is identical skips it: an identical graph means identical blast.

| corpus | languages |
|---|---|
| `repograph/shell-blast.jsonl` | Shell |
| `beauty-crm-hcl` | HCL |
| `pawfice` | Dart, Swift |
| `beauty-crm-sql` | SQL |
| `repograph`, `agent-orchestrator` | Rust, Python |
| `beauty-crm-mobile` | Kotlin |

The private corpora for Java, C#, Razor, GraphQL, Bicep and Vue are not on this machine. Decision 5 restores them if a copy turns up; none did on 2026-10-10 (§9). Until one does, the corpus diff and the cases decide alone for those languages, and the results document says the blast clause was not read.

### 5.6 The record

`docs/bench/2026-10-10-generic-engine-results.md` gets one section per step, holding:

- the regression counts (all zero);
- the accepted counts;
- P, R and the hand read for each bounded kind that changed;
- the bench and blast readings;
- the build time.

A row is written before its step merges and is never edited afterwards.

## 6 · Steps

Each step is one language and one PR. A step's last commit passes §5. Sizes are rough. **S** is under 400 lines changed, **M** is 400–1,200, **L** is over 1,200. "Query" is new `.scm` lines and "Rust" is old lines removed.

### 6.1 The existing languages, steps 0–14

**Step 0 · Engine and front, every language still old.** M–L.

Build:

- `spec.rs`, `queries.rs`, and the engine (facts, qualification, bodies, default resolution);
- `detect` with its overrides and the shebang rule, plus cases for each interpreter form (§2.8), about 80 lines;
- the front, `read`;
- `Hooks` replacing `Reader`, whose fields map one to one, with `legacy` holding each old `extract`;
- `Sink`, and `register` for Vue.

Change how files are read:

- The front owns the parse: every `reader::open` and every `Lang::X.parse` inside an `extract` goes away.
- TypeScript's three scans share one tree. Their inputs are the same text and grammar.
- TypeScript's file head moves into `head`.
- Collect-time parses stay until each port.

Wire up callers and tooling:

- `changes.rs` and `main.rs` read the spec;
- the contract test, with one probe query per grammar so it is not vacuous;
- `REPOGRAPH_DUMP_CAPTURES` and `REPOGRAPH_LEGACY`;
- `langdiff.py` and its test.

Record the base:

- the kit copy (§5.1), `fixture-base`, and the base bench;
- the base build time, compile time per grammar, and the time of the deepest file.

Gate: `graph.json` and `headers.json` byte-identical to `store-base`, as #170 required; tests unedited; the bench with zero flips. This step alone closes the three reviewer defects. About 1,200 lines of Rust are added, about 200 removed, plus about 250 lines of Python.

**Step 1 · Shell.** S. Query ~50, Rust −250, hook ~150 kept.

- **Queries:** `function_definition`; a command name as `@reference.calls`; `source` and `.` as `@import`; citations over `comment`, `string`, `raw_string` and `heredoc_body`.
- **Spec:** `Imported { transitive }`, `Unique`, the prose recipe (`Adjacent`, `Raw`, `Tail`, `First`).
- **Hooks:** `import` keeps `paths.rs` and the run-by-path References; `collect` and `widen` stay.
- **Gate:** the corpus decides Calls (192). Cases decide References (11).

**Step 2 · Bicep.** S–M. Query ~70, Rust −250.

- **Queries:** the seven declaration kinds; a child resource as a `.` container; `output/` as a `prefix`; `#set! export` on `param`, `output` and `@export()`; in-file references with locals for shadowing; `module` as `@import` with the registry predicate.
- **Spec:** `File`, `OwnFirst`, prose with `between` set to decorators.
- **Hooks:** module reach in `resolve`, using `Resolver::facts`.
- **Gate:** the corpus decides References (4,344).

**Step 3 · HCL and Terraform.** S–M. Query ~90, Rust −250.

- **Split (decision 2):** `.tf` becomes `Lang::Terraform`, read with the HCL grammar crate and in `Family::Hcl`; `.hcl` stays `Lang::Hcl`. Each language has its own queries and export rule (`#set! export` on `var/` and `output/` for Terraform, `All` for HCL), so no dialect field exists. `Lang::ALL`, `tests/parse_census.rs` and `docs/languages.md` gain the row. Ids, family and headers do not change, so the gate is the same as for any port.
- **Queries:** Terraform addresses as `@name` parts with prefixes (`T/N`, `data/T/N`, `var/`, `local/` per attribute of `locals`, `output/`, `module/`, `provider/`). Non-Terraform blocks as kind plus labels, with `#not-match? "/"` for a skipped label. The first aliased provider wins through first-id-wins.
- **Spec:** `Directory` for Terraform.
- **Hooks:** `widen` stays, on Terraform.
- **Gate:** References (66) is just enough for the corpus to decide. The `beauty-crm-hcl` blast and the fixture's 5 `.tf` files also apply.

**Step 4 · Swift.** S. Query ~60, Rust −180.

- **Queries:** the `class_declaration` variants told apart by their keyword token (`(class_declaration "struct")`), `protocol_declaration`, `extension` as `@scope.owner`, top-level `let`.
- **Spec:** `Words { hide: private, fileprivate }`, `Family`, `OwnFirst`, `Name` for the signature line, or the `signature` hook if that changes R3.
- **Gate:** 11 symbols, so the cases decide everything; plus the `pawfice` blast.

**Step 5 · GraphQL.** S. Query ~60, Rust −300.

- **Queries:** definitions with `Type.field` members, type links, `extend type` as `@reference.references` with `#set! context "extend"`, and citations from descriptions and comments.
- **Spec:** `All` export, `Family`, `All` ambiguity, container edges dropped, the default header.
- **Gate:** Calls (69) by the corpus.

**Step 6 · SQL.** M. Query ~130, Rust −380.

- **Queries:** the Postgres statements that declare (tables, views, functions, types, indexes, triggers, policies); schema and name as `@name` parts; references with their contexts (`alter`, `on`, `type`, …).
- **Spec:** `Family`.
- **Hooks:** `fold`.
- **Timing:** compile time and build time are measured here, because Postgres is the largest grammar.
- **Gate:** Calls (145) and References (388) by the corpus; the `beauty-crm-sql` blast; the fixture's 76 `.sql` files.

**Step 7 · Dart.** M. Query ~100, Rust −350, hook ~330 kept.

- **Queries:** declarations and members; `Prefix('_')`.
- **Hooks:** `collect` (libraries), `import` (`package:`), `resolve` owning Calls and Extends.
- **Gate:** Calls (76) by the corpus; the `pawfice` blast.

**Step 8 · Python.** M. Query ~110, Rust −450, hook ~450 kept.

- **Queries:** `def` and `class`; the docstring as `@doc`; the decorated span, chosen to match the old reader; locals for parameters and comprehensions; citations.
- **Spec:** `Prefix('_')`, with `__all__` in `collect`.
- **Hooks:** `import` (package roots, relative imports), `resolve` owning Calls.
- **Gate:** Calls (841) by the corpus. Extends (5) by the cases.

**Step 9 · Rust.** L. Query ~150, Rust −600, hook ~700 kept.

- **Queries:** items; `impl` as `@scope.owner`; `mod` as a `/` container; `///` doc through `Adjacent` and `Stripped`; `pub` as `Words { show }`; citations.
- **Hooks:** `collect` (the crate module tree), `import` (`use` flattening), `resolve` owning Calls.
- **Gate:** Calls (1,068) by the corpus; the `repograph` and `agent-orchestrator` blasts.
- **Pause (decision 3):** before step 10 starts, the results document gets a re-read of steps 0–9 for Max:
  - Rust lines removed and hook lines kept, against §6's estimates;
  - the build time against the 1.20× bar;
  - P and R for every kind decided so far;
  - which hooks the prediction table (§3.4) got wrong.

  Step 10 starts after that re-read, not before.

**Step 10 · Kotlin.** L. Query ~120, Rust −700. The shared JVM hook is extracted here.

- **Queries:** declarations; `package` as `@scope.namespace`; an extension receiver as `@owner`; modifiers.
- **Hooks:** the JVM `header` (arity, supertypes) and `resolve` owning Calls. References (73) and Extends (15) try the default first.
- **Gate:** the fixture's 126 `.kt` files; the `beauty-crm-mobile` blast.

**Step 11 · Java.** M. Query ~100, Rust −500.

- **Hooks:** shares Kotlin's JVM hook.
- **Gate:** Calls (14), so the cases decide.

**Step 12 · C#, then Razor.** L. Query ~150, Rust −800, hook ~1,100 kept.

- **Queries:** declarations; block and file-scoped namespaces as `@scope.namespace`; members; modifiers.
- **Hooks:** `collect`, `header` and `manifest` for the DotNet state; `resolve` owning Calls (compiler lookup order, partial types).
- **Razor:** `embed` (`blank.rs`), `view: CSharp`, its own `collect` and `header`. There are no corpus files, so the cases decide.

**Step 13 · TypeScript, TSX, then Vue.** L. Query ~220, Rust −700, hook ~600 kept.

- **Queries:** functions, classes and members; interface members; object-literal members declared by their `const`; enums and type aliases; `export`; decorators; `idrefs` as citations. TSX inherits TypeScript's queries.
- **Hooks:** `head`; `import` (tsconfig tiers, package exports, the existing `Resolver::resolve`); `resolve` owning Calls and DecoratedBy. DecoratedBy stays with the hook because of the shared `deco:<Name>` nodes (the L8 exception).
- **Vue:** `embed`, `register`, `view: TypeScript | Tsx`, and the component symbol in `resolve`.
- **Gate:** the full bench, `--no-dense` and dense.

**Step 14 · Remove the old paths.** M, net negative. Remove:

- `legacy`, `Reader`, `REPOGRAPH_LEGACY`;
- the per-family declaration walkers;
- `prose::declare`, `prose::cite`, `idrefs`.

Then:

- Bump `walk::GRAMMAR` to 6, once, if any accepted class read non-zero in any step (decision 1, L5); otherwise leave it. If steps 15–24 ship in the same release, step 24 carries this bump instead, so a release still moves GRAMMAR once.
- Update `docs/languages.md` and `docs/graph-model.md`, and make the 0.6.0 spec's "hand walk" paragraph point here.
- Run the dense bench and the build time.

Expected net for steps 0–14: about 6,000 lines of Rust leave, about 1,300 lines of queries arrive, and about 3,500 lines of hooks remain. Most of those hooks are in JVM, .NET and TypeScript.

### 6.2 New languages, steps 15–24

These are languages repograph does not read today. The engine makes each one a `Spec` with queries, data and `hooks: NONE`. Each joins through its own step's gate. Until step 24, a new language is read only when a config or `REPOGRAPH_CODE_GLOBS` names its globs, as Razor is.

**Selection.** A candidate stays only when its grammar crate passes four checks:

- it depends on `tree-sitter-language ^0.1` rather than on `tree-sitter`, so it links against `tree-sitter =0.27.0`;
- its generated `LANGUAGE_VERSION` lies inside 0.27's window, 13–15 (`include/tree_sitter/api.h`);
- its licence is MIT, Apache-2.0 or a BSD variant, the 0.6.0 licence bar;
- it ships an upstream `tags.scm`, or an equivalent query that names top-level definitions.

The table below was read on 2026-10-10 from crates.io's API and from each crate's published source (`parser.c`, `queries/`, `bindings/rust/lib.rs`). Nothing was added to `Cargo.toml`.

| language | crate, newest | published | licence | ABI | upstream queries | parser sources | verdict |
|---|---|---|---|---|---|---|---|
| Lua | `tree-sitter-lua 0.5.0` (tree-sitter-grammars) | 2026-02-26 | MIT | 15 | tags, locals | 0.35 MB | kept |
| Go | `tree-sitter-go 0.25.0` | 2025-08-29 | MIT | 15 | tags | 1.5 MB | kept |
| C | `tree-sitter-c 0.24.2` | 2026-04-22 | MIT | 15 | tags, definitions only | 3.7 MB | kept |
| C++ | `tree-sitter-cpp 0.23.4` | 2024-11-11 | MIT | 14 | tags, definitions only | 17 MB | kept |
| Ruby | `tree-sitter-ruby 0.23.1` | 2024-11-11 | MIT | 14 | tags, locals | 15 MB | kept |
| PHP | `tree-sitter-php 0.25.1` | 2026-10-06 | MIT | 15 | tags | 14 MB, two parsers | kept; pinned by age (§7) |
| Scala | `tree-sitter-scala 0.26.2` | 2026-08-08 | MIT | 15 | tags, locals | 26 MB | kept |
| Elixir | `tree-sitter-elixir 0.3.5` | 2026-03-02 | Apache-2.0 | 14 | tags | 12 MB | kept |
| OCaml | `tree-sitter-ocaml 0.26.0` | 2026-09-13 | MIT | 15 | tags, locals | 62 MB, three parsers | kept; last, held by the size bar |
| Zig | `tree-sitter-zig 1.1.2` | 2024-12-22 | MIT | 14 | highlights, indents, folds, injections | 5.6 MB | **dropped** |
| Haskell | `tree-sitter-haskell 0.24.1` | 2026-09-30 | MIT | 15 | locals, highlights, injections | 19 MB | **dropped** |

Zig has no tags query, neither in the crate nor on the repository's `master`. Haskell has none either: its `locals.scm` names local bindings, not top-level definitions. Every kept crate's external scanner is C, so MSVC builds it the way it builds today's grammars.

**Upstream tags are a starting point, not the query.** They use general predicates the engine refuses: `#strip!`, `#select-adjacent!`, `#set-adjacent!` and `#is-not? local`. Their capture names also differ from §2.2:

- `@reference.call` becomes `@reference.calls`;
- `@reference.class` and `@reference.type` become `@reference.references` with `context "type"`;
- `@reference.implementation` becomes `@reference.implements`.

Doc adjacency moves to `Body::Adjacent`, and `#is-not? local` moves to `@local.*`. C and C++ tags declare only, so their reference and include patterns are written here. A copied query keeps its upstream licence notice at the top of the `.scm`, and each step adds its crate to the licence reading (0.6.0 results §8).

**What a new language may add.** It never adds a hook. When a language needs something no variant gives, there are three outcomes:

- a new variant of an admitted field: engine code, pinned by that language's cases, and open to every later language;
- a new field, only when at least two of the batch's languages use it (the admission rule);
- a missing edge, which is acceptable by rule.

Every addition defaults to today's behaviour, so the existing languages' specs do not change. Gate 4 below checks that. §2 and §3 describe the engine as step 14 leaves it. The batch is expected to add these to it:

- **`Export::Capitalized`** (Go).
- **`Ambiguity::OwnThenUnique`:** a match in the own file shadows the others; otherwise exactly one candidate, or nothing. C, C++, Lua and PHP functions use it, because a name that is defined twice in a repo (two programs, two examples) is more often two things than one.
- **`ImportPaths::ModulePrefix { manifest, directive }`:** the import path, minus the module path a manifest line declares, read as a repo directory. Go's `go.mod` `module` line; the nearest `go.mod` above the importer wins.
- **`ImportPaths::Scoped { sep }`:** the path splits at its last `sep` into a header scope and a name, resolved through the family index. A wildcard names every file of the scope (PHP `\`, Scala `.`).
- **`ImportPaths::Named { sep }`:** the whole path is a declared qualified name in the family index (Elixir modules).
- **`ImportPaths::Dotted { roots, suffixes }`:** dots become slashes before the path lookup (Lua `require("a.b")`).
- **`ImportPaths::ModuleFile`:** a capitalized module name names a sibling file, so `Foo` is `foo.ml` or `foo.mli` (OCaml).
- **Member lookup through a receiver** matches the member's last name part, so Lua's `m.f` finds `M.f` in the required file.
- **Directory targets.** An import that names a directory writes one Imports edge to each file of that directory in the same language. A spec glob excludes test files (Go packages).
- **A new field, `receivers: Receivers`.**
  - `Hook` is today's rule, and every existing language keeps it: a reference with a `@receiver` is left to a hook.
  - `Imports`: a `@receiver` that is an import's alias, or a declared module's qualified name, resolves its member among that import's or module's files. It resolves only when exactly one of them declares the member.
  - Six users: Lua `m.f`, Go `pkg.F`, PHP `User::find`, Scala `Obj.f`, Elixir `Conn.f` and OCaml `M.f`.
- **A new field, `qualifiers: &[&str]`.** Separators inside one captured `@name`, `@owner` or `@receiver` token become `/`, the module join. A name the grammar writes as one token then reads like nested modules.
  - Elixir's `alias` is a single token with its dots: `Plug.Conn`, checked against the crate's `grammar.json`. It becomes `Plug/Conn`, so `.` still joins members only (L4).
  - Ruby `Foo::Bar` and C++ `ns::Cls` use `::`.
  - Three users. Every existing language has `&[]`.
- **`#set! visibility`** on one reference pattern overrides `spec.visibility` for that pattern. Ruby and PHP bare method calls stay in their file, while class names reach the family. `visibility` joins the closed key set of §2.2.

No new definition kind is needed:

- Ruby modules, C++ namespaces, Elixir modules and OCaml modules are `module`;
- C unions are `struct`;
- PHP traits are `trait`;
- Scala objects are `object`;
- function-like macros are `function`.

**Per language.** Sizes count queries, cases and engine variants; no step removes old code.

**Step 15 · Lua.** S. Query ~60, cases ~150, engine ~150 (`Dotted`, `receivers`, `OwnThenUnique`).

- **Lang and family:** `Lang::Lua` (`.lua`), `Family::Lua`.
- **Queries:**
  - `function M.f()` and `function obj:m()`, with `M` and `obj` as `@owner`;
  - `local function` with `export "never"`;
  - `require("a.b")` as `@import`, with an alias when a `local` binds it;
  - locals from the upstream `locals.scm`.
- **Imports:** `Dotted { roots: ["", "src", "lua"], suffixes: [".lua", "/init.lua"] }`.
- **Resolution:**
  - bare calls use `Imported` with `OwnThenUnique`, because a global function is visible only once its file is required;
  - `m.f()` resolves through `receivers: Imports`.
  - Method calls on values (`obj:m()` where `obj` is not an import) stay missing.
- **Default enough:** yes.
- **Corpus:** `luarocks/luarocks` at `v3.13.0`, 162 `.lua`, MIT.
- **Why first:** it is the smallest grammar, and it proves a language can arrive as data only.

**Step 16 · Go.** M. Query ~90, cases ~250, engine ~150 (`Capitalized`, `ModulePrefix`, directory targets).

- **Lang and family:** `Lang::Go` (`.go`), `Family::Go`.
- **Queries:**
  - functions;
  - methods, with the receiver's type as `@owner` (`T.M`);
  - types, with struct fields and interface methods as members;
  - consts and vars;
  - calls, and type uses as `context "type"`;
  - `import_spec` with its alias.
- **Imports:** `ModulePrefix { manifest: "go.mod", directive: "module" }` with directory targets, excluding `_test.go`. The standard library and modules outside the repo drop.
- **Resolution:**
  - bare names use `Directory` (a package is a directory) with `All`. Build-tag variants (`x_linux.go`, `x_windows.go`) declare one name twice, and both edges are right.
  - `pkg.F` resolves through `receivers: Imports`.
  - `x.M()` on a value needs types and stays missing.
- **Default enough:** yes for package-level code.
- **Corpus:** `gin-gonic/gin` at `v1.12.0`, 98 `.go`, MIT. It has several packages under one module, imported by module path.

**Step 17 · C.** S–M. Query ~70, cases ~200.

- **Lang and family:** `Lang::C` (`.c`, and `.h` pending step 18's census). `Family::C` is shared with C++: they are one link namespace, and `extern "C"` code calls across.
- **Queries:**
  - function definitions (prototypes declare nothing);
  - structs, unions and enums as `.` containers with their fields;
  - typedefs;
  - function-like `#define`s as functions;
  - calls by identifier.
- **Export:** `Words { hide: ["static"] }`.
- **Imports:** `#include "x.h"` relative to the includer, then under the roots `["", "include", "src"]`. `<…>` is a `system_lib_string` node and drops by kind.
- **Resolution:** `Family` with `OwnThenUnique`. A bare call resolves to the file's own definition, else to the one non-static definition in the family. Calls through function pointers and macro-built names stay missing.
- **Default enough:** yes.
- **Corpus:** `redis/hiredis` at `v1.4.1`, 25 `.c` and 27 `.h`, BSD-3-Clause.

**Step 18 · C++.** M. Query ~120, cases ~250, engine ~40 (`qualifiers`).

- **Lang and family:** `Lang::Cpp` (`.cc`, `.cpp`, `.cxx`, `.c++`, `.hh`, `.hpp`, `.hxx`, `.h++`), `Family::C`.
- **Queries:**
  - namespaces as `/` containers;
  - classes and structs as `.` containers, with methods and fields;
  - out-of-line definitions (`void DB::Get()`), with the qualified scope as `@owner` and `qualifiers: ["::"]`. An out-of-line function in a namespace (`void ns::f()`) gets `ns.f` where the in-namespace form gets `ns/f`; the step records that mismatch rather than resolving it;
  - free functions;
  - bare calls and `ns::f` calls.
- **`.h`:** the step reads both grammars' parse census over both corpora's headers: hiredis has 27 C headers, leveldb has 56 C++ headers. `.h` becomes `Lang::Cpp` if the C++ grammar parses hiredis's headers with no more ERROR files than the C grammar does. Otherwise it stays `Lang::C`, and a C++ header named `.h` loses its class members. That limitation is recorded either way.
- **Export:** as C. Anonymous namespaces are not modelled.
- **Resolution:** as C. Member calls (`o.f()`, `p->f()`), overloads after the first, and template instantiations need types and stay missing.
- **Default enough:** for free functions and namespaces, yes. For member-heavy code, recall is low, and a missing edge is acceptable.
- **Corpus:** `google/leveldb` at `1.23`, 75 `.cc` and 56 `.h`, BSD-3-Clause.

**Step 19 · Ruby.** M. Query ~90, cases ~250, engine ~60 (`#set! visibility`).

- **Lang and family:** `Lang::Ruby` (`.rb`, `.rake`, `.gemspec`), `Family::Ruby`.
- **Queries:**
  - modules as `/` containers and classes as `.` containers, with `qualifiers: ["::"]` for `class Foo::Bar`;
  - `def` and `def self.` methods;
  - constants;
  - the superclass as `@reference.extends`;
  - `include` and `extend` as References with the existing contexts `impl` and `extend`;
  - constant references and bare calls;
  - locals from upstream.
- **Export:** `All`. Ruby's `private` is a statement that changes what follows it, and a query cannot carry state across siblings.
- **Imports:** `require_relative` resolves relative to the file, with `.rb`. `require` resolves under the roots `["lib", ""]`. Gems drop.
- **Resolution:**
  - constants use `Family` with `OwnFirst`, because Rails and Zeitwerk autoload without requires;
  - bare method calls use `#set! visibility "file"`;
  - calls with a receiver stay missing.
- **Default enough:** yes.
- **Corpus:** `rack/rack` at `v3.2.7`, 93 `.rb`, MIT.

**Step 20 · PHP.** M. Query ~110, cases ~250, engine ~100 (`Scoped`).

- **Lang and family:** `Lang::Php` (`.php`), `Family::Php`. The grammar is `LANGUAGE_PHP`, which handles PHP mixed with HTML.
- **Queries:**
  - the namespace as `@scope.namespace`;
  - classes, interfaces, traits and enums as `.` containers;
  - methods, properties and class constants;
  - functions;
  - `extends` and `implements`;
  - `new X`, `X::m()` and type hints as references;
  - `use` with aliases;
  - `require` and `include` with a plain string literal.
- **Imports:** `Scoped { sep: "\\" }` for `use`, which is PSR-4 by namespace with no `composer.json` read. A literal `require` path resolves relative to the file.
- **Resolution:**
  - a class name resolves through the `use` aliases, then the file's own namespace scope;
  - a bare function call uses `Family` with `OwnThenUnique`;
  - `X::m()` resolves through `receivers: Imports`;
  - `$o->m()` stays missing.
- **Default enough:** yes.
- **Corpus:** `slimphp/Slim` at `4.15.3`, 125 `.php`, MIT.

**Step 21 · Scala.** M. Query ~110, cases ~250.

- **Lang and family:** `Lang::Scala` (`.scala`, `.sc`), `Family::Scala`.
- **Queries:**
  - package clauses as `@scope.namespace`;
  - classes, traits, objects and enums as `.` containers;
  - `def`, `val`, `var` and `type`;
  - `extends` and `with` as Extends;
  - imports with their selectors and wildcards.
- **Imports:** `Scoped { sep: "." }`. A wildcard names every file of the scope.
- **Resolution:**
  - a type resolves through the imports, then the same package scope;
  - `Obj.f` resolves through `receivers: Imports`;
  - implicits, givens, extension methods and calls on values stay missing.
- **Default enough:** for types and objects, yes.
- **Corpus:** `com-lihaoyi/os-lib` at `0.11.8`, 59 `.scala`, MIT.

**Step 22 · Elixir.** S–M. Query ~80, cases ~200, engine ~50 (`Named`).

- **Lang and family:** `Lang::Elixir` (`.ex`, `.exs`), `Family::Elixir`.
- **Queries:**
  - `defmodule` as a `module`, its `alias` token as `@name` with `qualifiers: ["."]` (`Plug.Conn` becomes `Plug/Conn`), and nested modules as `/` containers;
  - `def` and `defmacro`;
  - `defp` and `defmacrop` with `export "never"`;
  - `defstruct`;
  - `alias`, `import`, `require` and `use` as imports, with `as:`;
  - remote and local calls.
- **Imports:** `Named { sep: "." }`.
- **Resolution:**
  - a local call resolves in its own module;
  - `Plug.Conn.f` and an aliased `Conn.f` resolve through `receivers: Imports`;
  - calls through an `import`ed module use `Imported`;
  - code generated by `use` macros stays missing.
- **Default enough:** yes.
- **Corpus:** `elixir-plug/plug` at `v1.20.3`, 42 `.ex` and 36 `.exs`, Apache-2.0.

**Step 23 · OCaml.** M. Query ~120, cases ~250, engine ~60 (`ModuleFile`).

- **Lang and family:** `Lang::OCaml` (`.ml`) and `Lang::OCamlInterface` (`.mli`), both in `Family::OCaml`. They use `LANGUAGE_OCAML` and `LANGUAGE_OCAML_INTERFACE`; the type grammar is not referenced.
- **Queries:**
  - `let` bindings;
  - `module M = struct` as a `/` container;
  - module types and types, with constructors and record fields as `.` members;
  - classes and methods;
  - `open` and `include` as imports;
  - value paths `M.f`.
- **Imports:** `ModuleFile`. Dune's library wrapping is not read.
- **Resolution:**
  - a bare name resolves in its own file, then in the opened modules (`Imported`);
  - `M.f` resolves through `receivers: Imports`;
  - functors and first-class modules stay missing.
- **Default enough:** yes for module-level code.
- **Size first:** the crate's parser sources are 62 MB, the most after Postgres. The size bar below is read before any query is written.
- **Corpus:** `ocaml/ocaml-re` at `1.14.0`, 71 `.ml` and 26 `.mli`. Its licence is LGPL-2.1 or later with a static-linking exception. The corpus is read, never redistributed or linked.

**Step 24 · Defaults and GRAMMAR.** S.

- **Defaults:** every language whose step passed joins the default `code_globs` (Q8). A language whose step was held stays opt-in.
- **GRAMMAR:** `walk::GRAMMAR` moves once. It goes to 7, or it carries step 14's bump if both ship in one release (L5).
- **Docs:** `docs/languages.md` gets each new language's row.
- **Bench:** the full bench, dense and `--no-dense`. The fixture holds none of the nine languages' files (checked 2026-10-10), so its graph must not move. The bench reading must equal step 14's.

**The gate for steps 15–23.** The bench has no case in these languages and no floor for them (§7, risk 11). They are graded instead by:

1. **Cases.** `src/code/queries/<lang>/cases.rs` must cover:
   - every capture role the queries use;
   - each import shape;
   - each resolution path;
   - one reference the default leaves unresolved on purpose, such as a method call on a value.

   Every test passes, and none is edited. The contract test covers the new queries.
2. **Parse census.** `tests/parse_census.rs` gains the language. On the pinned corpus, at most 2% of the language's files may hold an ERROR or MISSING node, and the results document names each one. Above 2%, the step is held for Max.
3. **Hand-read precision.** The corpus is built with `REPOGRAPH_CODE_GLOBS` set to the defaults plus the new globs, then `build --no-dense`:
   - zero dangling edges (R5);
   - a seeded sample of 50 symbols, read against the source, with zero wrong ids, lines or ends;
   - for five seeded files, the symbol count against a hand count of their declarations, with every difference explained;
   - for every edge kind the language writes, a seeded sample of 50 edges (all of them when there are fewer), with zero wrong. P ≥ 0.99 at n = 50 permits none.

   Counts per kind are reported. Recall is not measured, because there is no truth reader.
4. **No change in the existing languages:**
   - the polyglot corpus's `graph.json` and `headers.json` are byte-identical to the previous step's, built with the new globs added. The corpus holds none of the nine languages' files (checked 2026-10-10), so any byte that moves is an engine addition leaking into an existing language;
   - the fixture's graph, built with the default globs, is byte-identical to step 14's;
   - on the language's own corpus, the builds with and without the new globs differ only in the new language's files (`langdiff.py` R6 = 0).
5. **Cost.** The stripped binary and the compressed release archive are measured against the parent commit. A step that adds more than 8 MB stripped or 1 MB compressed is held for Max.
6. **Platforms.** `ci` is green on `ubuntu-latest`, `macos-14` and `windows-latest`.

**Fetching a corpus**, once, at the step's start, after `df -h /`:

```bash
mkdir -p ~/bench/corpora-public
git clone --filter=blob:none https://github.com/gin-gonic/gin ~/bench/corpora-public/gin
git -C ~/bench/corpora-public/gin -c advice.detachedHead=false checkout v1.12.0
git -C ~/bench/corpora-public/gin rev-parse HEAD
```

The SHA goes into the results document, so a tag moved upstream is caught on the next fetch. Each corpus is fetched the same way into `~/bench/corpora-public/<repo name>`, with its own repo and tag. The clone is read-only apart from the `.repograph/` store each build writes into it.

| step | repo | tag |
|---|---|---|
| 15 | `luarocks/luarocks` | `v3.13.0` |
| 16 | `gin-gonic/gin` | `v1.12.0` |
| 17 | `redis/hiredis` | `v1.4.1` |
| 18 | `google/leveldb` | `1.23` |
| 19 | `rack/rack` | `v3.2.7` |
| 20 | `slimphp/Slim` | `4.15.3` |
| 21 | `com-lihaoyi/os-lib` | `0.11.8` |
| 22 | `elixir-plug/plug` | `v1.20.3` |
| 23 | `ocaml/ocaml-re` | `1.14.0` |

Expected size for steps 15–24: about 850 lines of queries, 2,100 lines of cases and 610 lines of engine additions, with no hooks.

The grammars are estimated at about +20 MB stripped and +1.1 MB compressed. That is scaled from Postgres: 93 MB of parser source became +14.6 MB stripped and +0.8 MB compressed (0.6.0 spec §2 and §9). Each step measures its own grammar instead of trusting the scale. OCaml (~6.5 MB stripped estimated), Scala (~4) and C++ (~2.7) are the ones the bar may hold.

Expected net for the whole plan, steps 0–24:

- about 6,000 lines of Rust leave;
- about 2,150 lines of queries arrive;
- about 2,000 lines of engine arrive (step 0's ~1,200, the detection seam, and the batch's ~610);
- about 3,500 lines of hooks remain;
- nine languages are added.

## 7 · Risks

1. **Query fidelity.** Tree-sitter's match semantics differ from a hand walk in ways that are easy to miss:
   - several patterns can match one node;
   - a text predicate on an optional capture that did not match passes vacuously;
   - quantified captures arrive in an order that must be pinned.

   The strict classes (R1–R4) catch every visible effect. The contract test and `REPOGRAPH_DUMP_CAPTURES` catch most of them before the corpus does.
2. **Knob creep.** Every review rejects a new `Spec` field with one user. A step that wants one adds a hook instead, and a later step that finds a second user promotes it.
3. **Thin corpora.** Swift (11 symbols), Java (35), Vue (46), Razor (0) and non-Terraform HCL (0) decide on cases alone, so a default that is wrong on real code in those languages could ship. Mitigation: the default keeps a kind there only when the cases pass unedited. Otherwise the hook stays. Restoring the private corpora (decision 5) would close the gap; no copy was found on 2026-10-10.
4. **Hooks reading `Facts`.** The heavy resolvers were written against their own `Declared` structs. Adapting them is most of the work in steps 9–13, and the likeliest place for subtle diffs. Kinds a hook owns are gated strictly, so no diff there passes.
5. **Cost.** Two cursor passes per file replace one hand walk, and the Postgres grammar compiles slowly. The build-time gate (1.20×) is read five times, and compile time is measured in step 0.
6. **Pathological files.** Deep or huge lists are bounded by the match limit and named on stderr. The deepest corpus file is timed in step 0.
7. **Base drift.** A fix that lands on `main` in a language not yet ported goes into its old reader. A fix in a ported language goes into its query or hook. `REPOGRAPH_LEGACY` keeps O and N in one binary, so the gate compares like with like; `store-base` is rebuilt when `main` moves the graph.
8. **Coexistence.** Each language carries two paths for one step's lifetime. Step 14 is a step with its own gate, not a cleanup to forget.
9. **GRAMMAR.** It moves once per release: at step 14, or at step 24 when both ship together (decision 1). A release cut in the middle of the migration can leave old doc lines in stores until their files change. Retrieval never reads those lines; rerank and explain do.
10. **The `parse_census` coupling.** For steps 0–14, `lang.rs` gains nothing, and `spec.rs` and `queries.rs` sit beside it. From step 15, each new language adds one `Lang` variant and one grammar crate to `lang.rs`, which `tests/parse_census.rs` compiles by path. The census therefore names the new language in the same commit.
11. **No floor for the new languages.** The 0.6.0 spec says it outright for Go: no repository of ours is written in it. The same holds for all nine. The bench has no case in them, and there is no truth reader and no blast clause. They are graded by their cases, a parse census and a hand-read precision sample of 50 per kind on one public corpus (§6.2). Recall is not measured at all.

    A language can therefore pass with low recall on real code. That is acceptable by the first global constraint, because a missing edge is not a wrong one. What would not be acceptable is a wrong-edge rate the 50-edge sample happened to miss: at P = 0.99, a sample of 50 shows no error 61% of the time. The mitigation is that every engine addition is shared, so a later step's sample re-reads it.
12. **Binary size.** The nine grammars are estimated at +20 MB stripped and +1.1 MB compressed. OCaml's parser sources alone are 62 MB, about 6.5 MB stripped by the Postgres scale, and Scala's and C++'s are 26 MB and 17 MB. The per-step bar (8 MB stripped, 1 MB compressed) holds a step for Max before it merges. The release archive's 0.6.0 bar, `v0.5.3`'s asset + 3.5 MB, is read again at step 24.
13. **Adapting upstream queries.** Upstream `tags.scm` relies on general predicates the engine refuses, and its captures name a different contract. Every adaptation is a place where a pattern can quietly stop matching. The contract test, `REPOGRAPH_DUMP_CAPTURES` and the 50-symbol declaration sample are the guard. A declaration a pattern silently lost shows up only as a missing symbol, so the sample counts symbols per file against a hand count for five files.
14. **`.h` is ambiguous.** One extension is C, C++ or Objective-C, and `Lang::of` cannot read content. Step 18's census picks one grammar for every `.h`, and the other kind of header loses members. Q7's `linguist` would decide per file by content.
15. **Upstream licence notices.** A query copied from a crate keeps its notice. The `cpp`, `ruby` and `lua` crates ship no LICENSE file, so the notice is taken from the grammar repository at the pinned tag. Elixir's grammar is Apache-2.0, whose NOTICE terms are read in step 22.
16. **Version age.** `tree-sitter-php 0.25.1` was four days old on 2026-10-10. Each step pins with `=` the newest version that is at least two weeks old when the step starts, as the 0.6.0 pins did. For PHP that is 0.25.1 by step 20, or 0.24.2 (2025-08-18) if 0.25.1 is yanked.
17. **The engine additions touch shared code.** `receivers`, `qualifiers`, `OwnThenUnique`, the new import shapes and the `visibility` property all live in the engine every language runs. Each defaults to today's behaviour. Gate 4 (§6.2) is what proves that: the polyglot corpus and the fixture stay byte-identical at every step.
18. **Detection stays a table.** Hyperpolyglot, the decided library, is unmaintained (§2.8), so detection keeps `Lang::of` plus shebangs. Content-based disambiguation (`.h`, `.m`, `.pl`) waits for Q7.

## 8 · Out of scope

- Queries or grammars supplied at runtime, and per-repo query overrides.
- Languages beyond §6.2's nine. Zig and Haskell were checked and dropped because neither grammar ships a tags query or an equivalent. Each further language is its own step under §6.2's gate.
- Hooks for the new languages, and per-language resolution beyond the engine additions in §6.2: receiver types, Go interfaces, C++ overloads, Ruby's `private` sections, Scala implicits, Elixir macros, OCaml functors.
- Reading build manifests for the new languages (`composer.json`, `build.sbt`, `mix.exs`, `dune`, `CMakeLists.txt`). `go.mod`'s `module` line is the one exception, read as data.
- Widening a ported language: more citations, more reference kinds, better resolution.
- Cross-family edges (L10), and any change to ids, node kinds, edge kinds or contexts.
- Sharing trees between the collect and extract passes.
- Native grammars for Razor and Vue (L6 stays).
- Doc and text extraction.

## 9 · Decisions and open questions

Questions 1–6 were decided by Max on 2026-10-10, each as recommended. They are recorded here as taken and are not reopened by a step.

1. **Doc lines may change.** They reach rerank (the default for `ask` since #164) and explain, but not retrieval. They are accepted and reported per step. GRAMMAR moves once for them: at step 14, or at step 24 if steps 15–24 ship in the same release.
2. **Terraform is its own `Lang`.** `.tf` maps to `Lang::Terraform`, with the HCL grammar crate and `Family::Hcl`; `.hcl` stays `Lang::Hcl`. Step 3 makes the split.
3. **All fourteen porting steps run.** Step 9 ends with a pause to re-read the numbers (§6.1); step 10 starts only after it.
4. **The hook thresholds are P ≥ 0.99 and R ≥ 0.97,** with P read from a hand read of up to 50 added edges.
5. **The private corpora are restored if a copy is found.** On 2026-10-10 none was:
   - `~/bench/corpora-private` is absent;
   - a search of `~` to depth 4 found no copy;
   - a search of `~/bench` and `~/Documents` to depth 3 found none either.

   Until Max points to one, Java, C#, Razor, GraphQL, Bicep and Vue are judged on the polyglot corpus and their cases (§5.5).
6. **Branching.** #170 merges first. `refactor/generic-code-engine` then branches from `main`, with one PR per step.

Open, for Max:

7. **A content-aware detection library?** Hyperpolyglot, as decided, is unusable (§2.8), so detection stays `Lang::of` plus shebangs. `linguist 0.1.16` fits on licence and MSRV, is maintained, takes the text as an argument, and would decide `.h` per file. It would sit after our table as a fallback, so it could not move an existing file's language. Recommendation: not now. No existing language needs it; step 18's census answers `.h` well enough for one public corpus. Revisit if a `.h` census or a user's repo shows the static choice failing.
8. **Do the new languages join the default globs at step 24?** Joining means every user's next build reads those files, and GRAMMAR moves once. Staying opt-in means a user names the globs, as with Razor. Recommendation: join for each language whose step passed. None of our repositories holds these files, so the fixture and the corpora do not move, and the risk falls only on users who have them. For those users a file node with symbols is better than a bare file node.
