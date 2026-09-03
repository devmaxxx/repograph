# Replace GitNexus Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** GitNexus becomes unnecessary in three releases: 0.4.0 answers "who calls this, what breaks if I change it, what does my diff touch, how does A reach B" from repograph's own graph so `beauty-crm` drops its escape hatch now; 0.5.0 adds a self-maintaining registry so any checkout — clone or worktree — is addressed by name, listed with its freshness, and built by seeding from a sibling store; 0.6.0 (own spec, later) adds the languages the other registered repositories need.

**Architecture:** 0.4.0 — a new per-file walk (`src/code/calls.rs`) emits the already-declared `EdgeKind::Calls` from call sites to the symbol an import, a local declaration, or a typed class field proves they reach; symbols gain an end line; `src/impact.rs` walks `Calls`/`Extends` up and down through barrel aliases and labels risk; `src/changes.rs` maps `git diff -U0` hunks onto symbol spans and reuses the upstream walk. 0.5.0 — `src/registry.rs` keeps `~/.config/repograph/registry.json` as a cache of git facts: project key = root commits, rows written on use and pruned on write, identity cached per store so the hot path never spawns git.

**Tech Stack:** Rust 1.98, tree-sitter 0.27.0 / tree-sitter-typescript 0.23.2, clap 4.6.6, serde 1.0.229 / serde_json 1.0.151, postcard 1.1.3 (all pinned already; no new crates — `git` is shelled out).

**Specs:** `docs/superpowers/specs/2026-09-03-replace-gitnexus.md` (0.4.0), `docs/superpowers/specs/2026-09-03-registry-design.md` (0.5.0)

## Releases

| Release | Ships | Unblocks | Tasks |
|---|---|---|---|
| **0.4.0** | `Calls` edges, symbol spans, `impact`, `trace`, `changes` | `beauty-crm` drops the GitNexus escape hatch; its pre-edit and pre-commit rules run on repograph | 1–7 |
| **0.5.0** | nested-checkout guard, registry, `--repo <name>`, seeding, `repos`, global notice hook | worktrees and second clones build in seconds; one hook for every indexed repository | 8–13 |
| **0.6.0** | C#, Kotlin, Python extractors (roadmap — separate spec) | `bonliva-crm-nx`, `beauty-crm/mobile`; then the user-level GitNexus hooks and MCP server go | — |

`beauty-crm` waits on 0.4.0 only. Nothing in Tasks 8–13 changes a 0.4.0 answer, so 0.4.0 is tagged and consumed before Task 8 starts.

## Global Constraints

- No new crates. `git` is invoked as a subprocess (`std::process::Command`), like `enrich` invokes its model command.
- `beauty-crm` (`/Users/max/Documents/projects/beauty-crm`) is the corpus and the consumer. Tasks 1–6 write there only into `.repograph/`; Task 7 edits its tracked files on purpose.
- Every extractor change follows `.claude/skills/extractor-case/SKILL.md`: dump the tree before writing an arm, one inline case per construct in `src/code/cases.rs`, then `verify` counts and `bench` floors on the corpus (`--no-dense build` for counts).
- Bench floors must not move: keyword 40/40, paraphrase ≥14/30 dense / ≥11/30 no-dense, code 12/12, p90 ≤ 230 tokens. `Calls` is not in `query::EXPAND`, so `ask` output must be byte-identical before and after Task 2.
- Comments explain *why*, never *what*; no ticket ids. Code, docs, commits in English. Conventional Commits; the repo hook rejects AI trailers — do not add `Co-Authored-By` or session links. Never combine a heredoc and `git commit` in one shell call: the commit hook rejects the whole compound command and the `git add` in front of it is lost too.
- Edge uniqueness stays `(source, target, kind, context, file)`. Call edges carry an empty context.
- Symbol ids stay `sym:<rel>::<Name>` and `sym:<rel>::<Class>.<member>`; file ids `file:<rel>`.
- The postcard mirror header embeds `CARGO_PKG_VERSION`, so the version bump in Task 1 is what invalidates 0.3.0 mirrors once `Node` gains a field. Do not reorder Task 1.

---

## Release 0.4.0 — beauty-crm off the escape hatch

Fastest path: Tasks 1 → 2 → 3 → 4 → 5 in order (each depends on the previous), Task 6 tags, Task 7 lands in `beauty-crm`. Nothing here is optional: `impact` carries the pre-edit rule, `changes` the pre-commit rule, and `trace` is forty lines in the same module.

### File structure

```
src/model.rs            Node.end, Extraction::node_span                       (Task 1)
src/code/symbols.rs     spans on declarations and members; resolver accessor (Task 1, 2)
src/code/calls.rs       NEW — per-file scope, call-site walk, Calls edges     (Task 2)
src/code/mod.rs         wires calls::scan between symbols and idrefs          (Task 2)
src/code/cases.rs       inline cases for spans and call shapes                (Task 1, 2)
src/impact.rs           NEW — aliases, canonical, upstream, downstream, trace, risk, render (Task 3)
src/query.rs            resolve() becomes pub(crate)                          (Task 3)
src/changes.rs          NEW — diff parsing, hunk→symbol, report, git          (Task 5)
src/main.rs             Impact, Trace, Changes subcommands                    (Task 4, 5)
README.md               Use / edge table / new "Blast radius" section         (Task 4, 5)
Cargo.toml, Cargo.lock, npm/*/package.json   0.4.0                            (Task 1, 6)
beauty-crm: package.json, .claude/CLAUDE.md, .claude/skills/repo-query/SKILL.md,
            .claude/hooks/repograph-notice.mjs, .gitignore, .ignore           (Task 7)
```

---

### Task 1: Start 0.4.0 and give symbols an end line

**Files:**
- Modify: `Cargo.toml:3`, `Cargo.lock` (via `cargo check`)
- Modify: `src/model.rs:11-20` (Node), `src/model.rs:35-41` (Extraction::node)
- Modify: `src/code/symbols.rs:194-250` (`declaration`), `src/code/symbols.rs:252-307` (`class_body`)
- Test: `src/code/cases.rs`

**Interfaces:**
- Produces: `Node.end: u32` (0 when unknown); `Extraction::node_span(kind, id, label, body, file, line, end)`.

- [ ] **Step 1: Bump the version**

`Cargo.toml` line 3: `version = "0.4.0"`. Run `cargo check --quiet` so `Cargo.lock` follows. The npm `package.json` files stay at 0.3.0 until the release task — they are published from a tag, not from the tree.

- [ ] **Step 2: Write the failing cases**

Append to `src/code/cases.rs` under a new `// ---- spans ----` header:

```rust
// ---- spans ----

fn node<'a>(ex: &'a Extraction, id: &str) -> &'a crate::model::Node {
    ex.nodes.iter().find(|n| n.id == id).unwrap_or_else(|| panic!("{id} not extracted"))
}

#[test]
fn a_function_declaration_spans_its_body() {
    let ex = extract("a.ts", "export function f() {\n  return 1;\n}\nconst x = 1;\n");
    let f = node(&ex, "sym:a.ts::f");
    assert_eq!((f.line, f.end), (1, 3));
    let x = node(&ex, "sym:a.ts::x");
    assert_eq!((x.line, x.end), (4, 4));
}

#[test]
fn a_class_and_each_member_carry_their_own_span() {
    let ex = extract("a.ts", "export class A {\n  one() {\n    return 1;\n  }\n\n  two() {}\n}\n");
    assert_eq!((node(&ex, "sym:a.ts::A").line, node(&ex, "sym:a.ts::A").end), (1, 7));
    assert_eq!((node(&ex, "sym:a.ts::A.one").line, node(&ex, "sym:a.ts::A.one").end), (2, 4));
    assert_eq!((node(&ex, "sym:a.ts::A.two").line, node(&ex, "sym:a.ts::A.two").end), (6, 6));
}

#[test]
fn a_decorated_export_class_still_starts_at_its_name_and_ends_at_its_brace() {
    let ex = extract("a.ts", "@Injectable()\nexport class S {\n  run() {}\n}\n");
    let s = node(&ex, "sym:a.ts::S");
    assert_eq!((s.line, s.end), (2, 4));
}

#[test]
fn a_decorator_node_has_no_span() {
    let ex = extract("a.ts", "@Injectable()\nexport class S {}\n");
    assert_eq!(node(&ex, "deco:Injectable").end, 0);
}
```

- [ ] **Step 3: Run them and watch them fail**

Run: `cargo test code::cases::a_function_declaration_spans_its_body code::cases::a_class_and_each_member`
Expected: compile error `no field 'end' on type Node`.

- [ ] **Step 4: Add the field and the constructor**

`src/model.rs`, in `Node` after `line`:

```rust
    pub line: u32,
    /// Last line of the declaration, for mapping a diff hunk onto the symbol it sits in; 0 on
    /// nodes that have no extent of their own (decorators, files, document ids).
    #[serde(default)] pub end: u32,
```

In `impl Extraction`, keep `node` as is (it sets `end: 0`) and add:

```rust
    pub fn node_span(&mut self, kind: NodeKind, id: &str, label: &str, body: &str, file: &str, line: u32, end: u32) {
        self.node(kind, id, label, body, file, line);
        self.nodes.last_mut().expect("node just pushed").end = end;
    }
```

`Node { … }` literals elsewhere (grep `community: None`) need `end: 0` — `model.rs:38` is the only one; `legacy.rs` builds through `Extraction::node` (check with `grep -n "Node {" src/`).

- [ ] **Step 5: Set the span in the scanner**

`src/code/symbols.rs`, in `declaration` replace the `declare` closure body:

```rust
        let end = decl.end_position().row as u32 + 1;
        let mut declare = |name: &str, ex: &mut Extraction| -> String {
            let id = format!("sym:{rel}::{name}");
            ex.node_span(NodeKind::Symbol, &id, name, &signature, rel, line, end);
            ex.edge(file_id, &id, EdgeKind::Declares, ctx, rel);
            created.push(id.clone());
            id
        };
```

In `class_body`, the member node:

```rust
            ex.node_span(NodeKind::Symbol, &id, &format!("{class_name}.{name}"), &signature, rel, m.start_position().row as u32 + 1, m.end_position().row as u32 + 1);
```

The `deco:` nodes keep `ex.node(...)`.

- [ ] **Step 6: Run the cases and the whole suite**

Run: `cargo test --quiet && cargo clippy --all-targets --quiet`
Expected: all pass (327 + 4), clippy clean.

- [ ] **Step 7: Prove the corpus did not move**

```bash
cargo build --release --quiet
target/release/repograph --repo /Users/max/Documents/projects/beauty-crm --no-dense build
target/release/repograph --repo /Users/max/Documents/projects/beauty-crm verify | head -2
```
Expected: `nodes: 8306 {… "Symbol": 4322 …}` and `edges: 29610 {…}` — identical to 0.3.0 (spans add no node and no edge). The `--no-dense build` is what rewrites the mirror under the new header.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml Cargo.lock src/model.rs src/code/symbols.rs src/code/cases.rs
git commit -m "feat(model): record where a symbol ends" -m "A git hunk can only be mapped onto the symbol around it when the symbol has an extent, not a line. Declarations and class members carry end_position(); decorators, files and document ids stay at 0. The version moves to 0.4.0 here so the mirror header retires every 0.3.0 mirror before a field is read out of one."
```

---

### Task 2: Emit `Calls` edges

**Files:**
- Create: `src/code/calls.rs`
- Modify: `src/code/mod.rs:1-33`
- Modify: `src/code/symbols.rs:115-121` (resolver accessor)
- Test: `src/code/cases.rs`

**Interfaces:**
- Consumes: `idrefs::owner(n, rel, src) -> String` (source of an edge), `symbols::is_top_level(Node) -> bool`, `symbols::parse(rel, src)`, `imports::Resolver::resolve(rel, spec) -> Option<String>`.
- Produces: `calls::scan(resolver: &Resolver, rel: &str, source: &str, locals: &BTreeSet<String>, ex: &mut Extraction)`; edges `(owner, target, EdgeKind::Calls, "", rel)` with targets `sym:<file>::<Name>` / `sym:<file>::<Class>.<member>`; `SymbolScanner::resolver(&self) -> &Resolver`.

- [ ] **Step 1: Pin the grammar shapes**

```bash
REPOGRAPH_DUMP='import { S } from "./s.js"; import * as ns from "./n.js";
export class C { private readonly repo: Repo; constructor(private readonly service: S) {}
  run(dto: D) { helper(); const x = new Foo(); Util.go(); ns.fn(); this.other(); return this.service.create(dto); } }
export const handler = () => { helper(); };' cargo test dump_tree -- --ignored --nocapture
```

Confirm (verified 2026-09-03): `call_expression function: (identifier)`; `new_expression constructor: (identifier)`; `call_expression function: (member_expression object: (identifier) property: (property_identifier))`; `… object: (this) …`; `… object: (member_expression object: (this) property: (property_identifier)) property: (property_identifier)`; `public_field_definition (accessibility_modifier) name: (property_identifier) type: (type_annotation (type_identifier))`; `method_definition name: (property_identifier) parameters: (formal_parameters (required_parameter (accessibility_modifier) pattern: (identifier) type: (type_annotation (type_identifier))))`; `import_clause (named_imports (import_specifier name: (identifier)))`; `namespace_import (identifier)`. An `import { a as b }` specifier has an `alias` field; a `Repository<User>` annotation is `generic_type name: (type_identifier)`.

- [ ] **Step 2: Write the failing cases**

Append to `src/code/cases.rs` under `// ---- calls ----`:

```rust
// ---- calls ----

fn calls(ex: &Extraction) -> Vec<(&str, &str)> {
    edges(ex, EdgeKind::Calls).into_iter().map(|(s, t, _)| (s, t)).collect()
}

#[test]
fn a_call_to_an_imported_function_targets_the_symbol_in_its_file() {
    let repo = Repo::new(&[("lib.ts", "export function helper() {}\n")]);
    let ex = repo.extract("a.ts", "import { helper } from './lib';\nexport function run() { helper(); }\n");
    assert_eq!(calls(&ex), vec![("sym:a.ts::run", "sym:lib.ts::helper")]);
}

#[test]
fn new_of_an_imported_class_is_a_call() {
    let repo = Repo::new(&[("foo.ts", "export class Foo {}\n")]);
    let ex = repo.extract("a.ts", "import { Foo } from './foo';\nexport const make = () => new Foo();\n");
    assert_eq!(calls(&ex), vec![("sym:a.ts::make", "sym:foo.ts::Foo")]);
}

#[test]
fn an_aliased_import_is_resolved_by_its_local_name() {
    let repo = Repo::new(&[("lib.ts", "export function helper() {}\n")]);
    let ex = repo.extract("a.ts", "import { helper as h } from './lib';\nexport function run() { h(); }\n");
    assert_eq!(calls(&ex), vec![("sym:a.ts::run", "sym:lib.ts::helper")]);
}

#[test]
fn a_call_to_a_local_top_level_function_stays_in_the_file() {
    let ex = extract("a.ts", "function inner() {}\nexport function run() { inner(); }\n");
    assert_eq!(calls(&ex), vec![("sym:a.ts::run", "sym:a.ts::inner")]);
}

#[test]
fn a_call_to_a_name_the_file_cannot_prove_yields_no_edge() {
    let ex = extract("a.ts", "export function run(cb: () => void) { console.log(1); cb(); fetch('/'); }\n");
    assert!(calls(&ex).is_empty());
}

#[test]
fn this_method_targets_the_enclosing_class_member() {
    let ex = extract("a.ts", "export class A {\n  run() { this.other(); }\n  other() {}\n}\n");
    assert_eq!(calls(&ex), vec![("sym:a.ts::A.run", "sym:a.ts::A.other")]);
}

#[test]
fn a_static_call_on_an_imported_class_targets_its_member() {
    let repo = Repo::new(&[("util.ts", "export class Util { static go() {} }\n")]);
    let ex = repo.extract("a.ts", "import { Util } from './util';\nexport function run() { Util.go(); }\n");
    assert_eq!(calls(&ex), vec![("sym:a.ts::run", "sym:util.ts::Util.go")]);
}

#[test]
fn a_namespace_import_call_targets_the_bare_symbol() {
    let repo = Repo::new(&[("n.ts", "export function fn() {}\n")]);
    let ex = repo.extract("a.ts", "import * as ns from './n';\nexport function run() { ns.fn(); }\n");
    assert_eq!(calls(&ex), vec![("sym:a.ts::run", "sym:n.ts::fn")]);
}

#[test]
fn a_call_through_a_constructor_parameter_property_targets_the_injected_class_member() {
    let repo = Repo::new(&[("s.ts", "export class StaffService { create() {} }\n")]);
    let ex = repo.extract(
        "c.ts",
        "import { StaffService } from './s';\nexport class StaffController {\n  constructor(private readonly service: StaffService) {}\n  create(dto: unknown) { return this.service.create(dto); }\n}\n",
    );
    assert_eq!(calls(&ex), vec![("sym:c.ts::StaffController.create", "sym:s.ts::StaffService.create")]);
}

#[test]
fn a_call_through_a_typed_field_targets_the_field_type_member() {
    let repo = Repo::new(&[("r.ts", "export class Repo { find() {} }\n")]);
    let ex = repo.extract("c.ts", "import { Repo } from './r';\nexport class C {\n  private repo: Repo;\n  run() { this.repo.find(); }\n}\n");
    assert_eq!(calls(&ex), vec![("sym:c.ts::C.run", "sym:r.ts::Repo.find")]);
}

#[test]
fn a_generic_field_type_uses_its_head_name() {
    let repo = Repo::new(&[("r.ts", "export class Repository<T> { find() {} }\n")]);
    let ex = repo.extract("c.ts", "import { Repository } from './r';\nexport class C {\n  constructor(private readonly users: Repository<User>) {}\n  run() { this.users.find(); }\n}\n");
    assert_eq!(calls(&ex), vec![("sym:c.ts::C.run", "sym:r.ts::Repository.find")]);
}

#[test]
fn a_call_through_a_barrel_points_at_the_barrel_and_stays_dangling() {
    let repo = Repo::new(&[("lib/index.ts", "export * from './impl';\n"), ("lib/impl.ts", "export function helper() {}\n")]);
    let ex = repo.extract("a.ts", "import { helper } from './lib';\nexport function run() { helper(); }\n");
    assert_eq!(calls(&ex), vec![("sym:a.ts::run", "sym:lib/index.ts::helper")]);
}

#[test]
fn a_call_outside_any_symbol_is_owned_by_the_file() {
    let repo = Repo::new(&[("lib.ts", "export function boot() {}\n")]);
    let ex = repo.extract("a.ts", "import { boot } from './lib';\nboot();\n");
    assert_eq!(calls(&ex), vec![("file:a.ts", "sym:lib.ts::boot")]);
}

#[test]
fn a_recursive_call_is_not_an_edge_and_a_repeated_call_is_one_edge() {
    let repo = Repo::new(&[("lib.ts", "export function helper() {}\n")]);
    let ex = repo.extract("a.ts", "import { helper } from './lib';\nexport function run(n: number) { helper(); helper(); if (n) run(n - 1); }\n");
    assert_eq!(calls(&ex), vec![("sym:a.ts::run", "sym:lib.ts::helper")]);
}

#[test]
fn a_chained_call_and_super_yield_nothing() {
    let repo = Repo::new(&[("lib.ts", "export function get() { return { then() {} }; }\n")]);
    let ex = repo.extract("a.ts", "import { get } from './lib';\nexport class A extends B {\n  run() { super.run(); get().then(); }\n}\n");
    assert_eq!(calls(&ex), vec![("sym:a.ts::A.run", "sym:lib.ts::get")]);
}
```

- [ ] **Step 3: Run them and watch them fail**

Run: `cargo test code::cases -- calls`
Expected: every `calls` case fails on an empty vector (no `Calls` edge is emitted yet).

- [ ] **Step 4: Expose the resolver**

`src/code/symbols.rs`, in `impl SymbolScanner` after `new`:

```rust
    pub(crate) fn resolver(&self) -> &Resolver { &self.resolver }
```

- [ ] **Step 5: Write `src/code/calls.rs`**

```rust
//! Call edges: the symbol a call site sits in, to the symbol the file can prove it reaches.
//! Proof is an import, a top-level declaration of this file, or the declared type of the class
//! field the call goes through; a name the file only assumes (a global, a parameter, `console`)
//! yields no edge, so a caller list never contains a guess.
use crate::code::idrefs::owner;
use crate::code::imports::Resolver;
use crate::code::symbols::{is_top_level, parse};
use crate::model::{EdgeKind, Extraction};
use std::collections::{BTreeMap, BTreeSet};
use tree_sitter::Node;

fn text<'a>(n: Node, src: &'a [u8]) -> &'a str { n.utf8_text(src).unwrap_or("") }

/// What one file lets a call resolve to, gathered in one pass over its top-level statements.
#[derive(Default)]
struct Scope {
    /// local binding -> (file that declares it, the name it is declared under there); this
    /// file and the same name for its own top-level declarations, `import { a as b }` maps
    /// `b` to `a`
    names: BTreeMap<String, (String, String)>,
    /// `import * as ns` binding -> file
    namespaces: BTreeMap<String, String>,
    /// class name -> field name -> declared type name, from typed fields and constructor
    /// parameter properties — the NestJS injection shape `this.service.create()` goes through
    fields: BTreeMap<String, BTreeMap<String, String>>,
}

/// `T` of `: T` or `: T<…>`; other annotations (unions, literals, arrays) name no class.
fn type_name(annotation: Node, src: &[u8]) -> Option<String> {
    let t = annotation.named_child(0)?;
    match t.kind() {
        "type_identifier" => Some(text(t, src).to_string()),
        "generic_type" => t.child_by_field_name("name").map(|n| text(n, src).to_string()),
        _ => None,
    }
}

fn class_of(n: Node, src: &[u8]) -> Option<String> {
    let mut cur = n;
    while let Some(p) = cur.parent() {
        if matches!(p.kind(), "class_declaration" | "abstract_class_declaration") && is_top_level(p) {
            return p.child_by_field_name("name").map(|c| text(c, src).to_string());
        }
        cur = p;
    }
    None
}

impl Scope {
    fn collect(root: Node, rel: &str, src: &[u8], resolver: &Resolver, locals: &BTreeSet<String>) -> Scope {
        let mut s = Scope::default();
        for name in locals { s.names.insert(name.clone(), (rel.to_string(), name.clone())); }
        let mut cur = root.walk();
        for stmt in root.named_children(&mut cur) {
            match stmt.kind() {
                "import_statement" => s.import(stmt, rel, src, resolver),
                "export_statement" => {
                    if let Some(decl) = stmt.child_by_field_name("declaration") { s.class(decl, src); }
                }
                _ => s.class(stmt, src),
            }
        }
        s
    }

    fn import(&mut self, stmt: Node, rel: &str, src: &[u8], resolver: &Resolver) {
        let Some(source) = stmt.child_by_field_name("source") else { return };
        let spec = text(source, src).trim_matches(|c| c == '\'' || c == '"');
        let Some(target) = resolver.resolve(rel, spec) else { return };
        let mut cur = stmt.walk();
        for clause in stmt.named_children(&mut cur).filter(|c| c.kind() == "import_clause") {
            let mut cc = clause.walk();
            for part in clause.named_children(&mut cc) {
                match part.kind() {
                    // A default import binds whatever name the importer chose; the declaring
                    // file's symbol is a best guess under that name.
                    "identifier" => { let n = text(part, src).to_string(); self.names.insert(n.clone(), (target.clone(), n)); }
                    "namespace_import" => {
                        if let Some(id) = part.named_child(0) { self.namespaces.insert(text(id, src).to_string(), target.clone()); }
                    }
                    "named_imports" => {
                        let mut nc = part.walk();
                        for spec in part.named_children(&mut nc).filter(|s| s.kind() == "import_specifier") {
                            let Some(name) = spec.child_by_field_name("name") else { continue };
                            let local = spec.child_by_field_name("alias").unwrap_or(name);
                            self.names.insert(text(local, src).to_string(), (target.clone(), text(name, src).to_string()));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn class(&mut self, decl: Node, src: &[u8]) {
        if !matches!(decl.kind(), "class_declaration" | "abstract_class_declaration") { return }
        let Some(name) = decl.child_by_field_name("name").map(|n| text(n, src).to_string()) else { return };
        let Some(body) = decl.child_by_field_name("body") else { return };
        let fields = self.fields.entry(name).or_default();
        let mut bc = body.walk();
        for m in body.named_children(&mut bc) {
            match m.kind() {
                "public_field_definition" => {
                    if let (Some(n), Some(t)) = (m.child_by_field_name("name"), m.child_by_field_name("type").and_then(|t| type_name(t, src))) {
                        fields.insert(text(n, src).to_string(), t);
                    }
                }
                "method_definition" if m.child_by_field_name("name").is_some_and(|n| text(n, src) == "constructor") => {
                    let Some(params) = m.child_by_field_name("parameters") else { continue };
                    let mut pc = params.walk();
                    for p in params.named_children(&mut pc) {
                        // Only a parameter with an accessibility modifier becomes a field.
                        let mut kids = p.walk();
                        if !p.named_children(&mut kids).any(|k| k.kind() == "accessibility_modifier") { continue }
                        let pattern = p.child_by_field_name("pattern").filter(|x| x.kind() == "identifier");
                        let ty = p.child_by_field_name("type").and_then(|t| type_name(t, src));
                        if let (Some(n), Some(t)) = (pattern, ty) { fields.insert(text(n, src).to_string(), t); }
                    }
                }
                _ => {}
            }
        }
    }

    fn target(&self, callee: Node, class: Option<&str>, rel: &str, src: &[u8]) -> Option<String> {
        match callee.kind() {
            "identifier" => {
                let (f, n) = self.names.get(text(callee, src))?;
                Some(format!("sym:{f}::{n}"))
            }
            "member_expression" => {
                let obj = callee.child_by_field_name("object")?;
                let prop = text(callee.child_by_field_name("property")?, src);
                match obj.kind() {
                    "this" => Some(format!("sym:{rel}::{}.{prop}", class?)),
                    "identifier" => {
                        let n = text(obj, src);
                        if let Some(f) = self.namespaces.get(n) { return Some(format!("sym:{f}::{prop}")); }
                        let (f, orig) = self.names.get(n)?;
                        Some(format!("sym:{f}::{orig}.{prop}"))
                    }
                    "member_expression" => {
                        let inner = obj.child_by_field_name("object")?;
                        if inner.kind() != "this" { return None }
                        let field = text(obj.child_by_field_name("property")?, src);
                        let ty = self.fields.get(class?)?.get(field)?;
                        let (f, t) = self.names.get(ty)?;
                        Some(format!("sym:{f}::{t}.{prop}"))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// Every call and `new` in the file, as an edge from its owner to what the scope proves it
/// reaches. `locals` are the names this file declares at top level (the scanner already knows
/// them); a self-call is dropped, a repeated call collapses in the extractor's dedup.
pub(crate) fn scan(resolver: &Resolver, rel: &str, source: &str, locals: &BTreeSet<String>, ex: &mut Extraction) {
    let src = source.as_bytes();
    let Some(tree) = parse(rel, src) else { return };
    let root = tree.root_node();
    let scope = Scope::collect(root, rel, src, resolver, locals);
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let mut c = n.walk();
        stack.extend(n.named_children(&mut c));
        let callee = match n.kind() {
            "call_expression" => n.child_by_field_name("function"),
            "new_expression" => n.child_by_field_name("constructor"),
            _ => None,
        };
        let Some(callee) = callee else { continue };
        let class = class_of(n, src);
        let Some(target) = scope.target(callee, class.as_deref(), rel, src) else { continue };
        let from = owner(n, rel, src);
        if from == target { continue }
        ex.edge(&from, &target, EdgeKind::Calls, "", rel);
    }
}
```

- [ ] **Step 6: Wire it into the extractor**

`src/code/mod.rs`:

```rust
pub mod calls;
pub mod idrefs;
pub mod imports;
pub mod symbols;
```

and in `extract`, between the symbol scan and the id scan:

```rust
    fn extract(&self, rel: &str, text: &str) -> Extraction {
        let mut ex = self.symbols.scan(rel, text);
        // Top-level names this file declares; a member id carries a dot and is not one.
        let prefix = format!("sym:{rel}::");
        let locals: std::collections::BTreeSet<String> = ex.nodes.iter()
            .filter_map(|n| n.id.strip_prefix(&prefix))
            .filter(|n| !n.contains('.'))
            .map(str::to_string)
            .collect();
        calls::scan(self.symbols.resolver(), rel, text, &locals, &mut ex);
        idrefs::scan(&self.ids, rel, text, &mut ex);
```

- [ ] **Step 7: Run the cases, then the suite**

Run: `cargo test code::cases -- calls` then `cargo test --quiet && cargo clippy --all-targets --quiet`
Expected: all pass. If `a_chained_call_and_super_yield_nothing` fails on `super.run()`, the callee is `member_expression object: (super)` — the `_ => None` arm already covers it; check the dump.

- [ ] **Step 8: Prove the corpus moved only where it should**

```bash
cargo build --release --quiet
B=/Users/max/Documents/projects/beauty-crm
target/release/repograph --repo $B --no-dense build
target/release/repograph --repo $B verify | head -2
target/release/repograph --repo $B explain StaffService
target/release/repograph --repo $B ask --stale StaffService > /tmp/ask-after.txt
git stash -q && cargo build --release --quiet && target/release/repograph --repo $B ask --stale StaffService > /tmp/ask-before.txt; git stash pop -q && cargo build --release --quiet
diff /tmp/ask-before.txt /tmp/ask-after.txt && echo "ask unchanged"
target/release/repograph --repo $B bench
target/release/repograph --repo $B --no-dense bench
```
Expected: `verify` shows a new `"Calls": N` with N > 0 and every other edge count and every node count identical to Task 1's; `explain StaffService` now has `Calls ← sym:apps/api/src/modules/staff/staff.controller.ts::StaffController.create` lines (and the same for `staff.module.ts` if it constructs it); `ask unchanged`; both bench runs pass their floors. Record N in the commit body.

- [ ] **Step 9: Commit**

```bash
git add src/code/calls.rs src/code/mod.rs src/code/symbols.rs src/code/cases.rs
git commit -m "feat(code): emit a Calls edge for every call the file can prove" -m "A call resolves through an import, a top-level declaration of the same file, or the declared type of the class field it goes through; anything else is left out rather than guessed. On beauty-crm this adds N Calls edges and moves no other count and no bench floor."
```

---

### Task 3: Traversal — aliases, upstream, downstream, trace, risk

**Files:**
- Create: `src/impact.rs`
- Modify: `src/query.rs:173` (`fn resolve` → `pub(crate) fn resolve`)
- Modify: `src/main.rs:1-14` (`mod impact;`)
- Test: `src/impact.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `Graph { nodes: BTreeMap<String, Node>, edges: BTreeSet<Edge> }`, `Edge { source, target, kind, context, file }`, `query::resolve(graph, needle) -> Option<&Node>`.
- Produces:
  - `pub struct Dependent { pub id: String, pub depth: usize, pub kind: EdgeKind, pub via: String }` (Serialize)
  - `pub struct Impact { pub root: String, pub layers: Vec<Vec<Dependent>>, pub importers: Vec<String> }`
  - `pub fn aliases(graph, id) -> Vec<String>`, `pub fn canonical(graph, id) -> Option<String>`
  - `pub fn upstream(graph, root, depth) -> Impact`, `pub fn downstream(graph, root, depth) -> Impact`
  - `pub fn trace(graph, from, to, depth) -> Option<Vec<String>>`
  - `pub fn risk(direct: usize, total: usize, files: usize) -> &'static str`
  - `pub fn files(graph, imp: &Impact) -> BTreeSet<String>`, `pub fn render(graph, imp, direction: &str) -> String`, `pub fn render_json(graph, imp, direction) -> String`

- [ ] **Step 1: Write the failing tests**

Create `src/impact.rs` with only the test module first:

```rust
use crate::model::{Edge, EdgeKind, Graph};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Extraction, NodeKind};

    /// controller.create → service.create; module constructs the service; a barrel re-exports
    /// the service and a worker calls it through the barrel; a job extends the worker.
    fn graph() -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::File, "file:s.ts", "s.ts", "", "s.ts", 1);
        e.node(NodeKind::Symbol, "sym:s.ts::S", "S", "", "s.ts", 3);
        e.node(NodeKind::Symbol, "sym:s.ts::S.create", "S.create", "", "s.ts", 5);
        e.edge("file:s.ts", "sym:s.ts::S", EdgeKind::Declares, "export", "s.ts");
        e.edge("sym:s.ts::S", "sym:s.ts::S.create", EdgeKind::Declares, "", "s.ts");
        e.node(NodeKind::File, "file:c.ts", "c.ts", "", "c.ts", 1);
        e.node(NodeKind::Symbol, "sym:c.ts::C.create", "C.create", "", "c.ts", 9);
        e.edge("file:c.ts", "file:s.ts", EdgeKind::Imports, "S", "c.ts");
        e.edge("sym:c.ts::C.create", "sym:s.ts::S.create", EdgeKind::Calls, "", "c.ts");
        e.node(NodeKind::File, "file:m.ts", "m.ts", "", "m.ts", 1);
        e.edge("file:m.ts", "file:s.ts", EdgeKind::Imports, "S", "m.ts");
        e.edge("file:m.ts", "sym:s.ts::S", EdgeKind::Calls, "", "m.ts");
        e.node(NodeKind::File, "file:index.ts", "index.ts", "", "index.ts", 1);
        e.edge("file:index.ts", "file:s.ts", EdgeKind::ReExports, "*", "index.ts");
        e.node(NodeKind::File, "file:w.ts", "w.ts", "", "w.ts", 1);
        e.node(NodeKind::Symbol, "sym:w.ts::W.run", "W.run", "", "w.ts", 4);
        e.edge("file:w.ts", "file:index.ts", EdgeKind::Imports, "S", "w.ts");
        e.edge("sym:w.ts::W.run", "sym:index.ts::S.create", EdgeKind::Calls, "", "w.ts");
        e.node(NodeKind::Symbol, "sym:w.ts::W", "W", "", "w.ts", 3);
        e.edge("sym:w.ts::W", "sym:w.ts::W.run", EdgeKind::Declares, "", "w.ts");
        e.node(NodeKind::Symbol, "sym:j.ts::J", "J", "", "j.ts", 2);
        e.edge("sym:j.ts::J", "sym:w.ts::W", EdgeKind::Extends, "", "j.ts");
        g.apply(e);
        g
    }

    #[test]
    fn aliases_follow_re_exports_back_to_every_barrel() {
        assert_eq!(aliases(&graph(), "sym:s.ts::S.create"), vec!["sym:index.ts::S.create"]);
        assert_eq!(aliases(&graph(), "sym:s.ts::S"), vec!["sym:index.ts::S"]);
    }

    #[test]
    fn canonical_walks_a_barrel_forward_to_the_declaration() {
        assert_eq!(canonical(&graph(), "sym:index.ts::S.create").as_deref(), Some("sym:s.ts::S.create"));
        assert_eq!(canonical(&graph(), "sym:s.ts::S.create").as_deref(), Some("sym:s.ts::S.create"));
        assert_eq!(canonical(&graph(), "sym:index.ts::Nope"), None);
    }

    #[test]
    fn upstream_of_a_class_reaches_callers_of_its_members_and_through_barrels() {
        let imp = upstream(&graph(), "sym:s.ts::S", 3);
        let d1: Vec<&str> = imp.layers[0].iter().map(|d| d.id.as_str()).collect();
        assert_eq!(d1, vec!["file:m.ts", "sym:c.ts::C.create", "sym:w.ts::W.run"]);
        // J extends W, and W.run is the caller: the subclass inherits the call, the class
        // itself is not listed as a dependent of its own member.
        let d2: Vec<(&str, EdgeKind)> = imp.layers[1].iter().map(|d| (d.id.as_str(), d.kind)).collect();
        assert_eq!(d2, vec![("sym:j.ts::J", EdgeKind::Extends)]);
        assert_eq!(imp.layers.len(), 2);
        assert_eq!(imp.importers, vec!["c.ts", "m.ts", "w.ts"]);
    }

    #[test]
    fn upstream_of_a_member_is_narrower_than_its_class() {
        let imp = upstream(&graph(), "sym:s.ts::S.create", 1);
        let d1: Vec<&str> = imp.layers[0].iter().map(|d| d.id.as_str()).collect();
        assert_eq!(d1, vec!["sym:c.ts::C.create", "sym:w.ts::W.run"]);
    }

    #[test]
    fn depth_caps_the_walk_and_a_dependent_appears_once_at_its_shallowest() {
        let imp = upstream(&graph(), "sym:s.ts::S", 1);
        assert_eq!(imp.layers.len(), 1);
        let deep = upstream(&graph(), "sym:s.ts::S", 9);
        let all: Vec<&str> = deep.layers.iter().flatten().map(|d| d.id.as_str()).collect();
        let set: BTreeSet<&str> = all.iter().copied().collect();
        assert_eq!(all.len(), set.len());
    }

    #[test]
    fn downstream_canonicalises_a_barrel_target() {
        let imp = downstream(&graph(), "sym:w.ts::W", 2);
        let d1: Vec<&str> = imp.layers[0].iter().map(|d| d.id.as_str()).collect();
        assert_eq!(d1, vec!["sym:s.ts::S.create"]);
    }

    #[test]
    fn trace_finds_the_shortest_call_path_and_reports_none_when_there_is_no_path() {
        // J → W by Extends, W → S.create through its member W.run and the barrel alias.
        assert_eq!(trace(&graph(), "sym:j.ts::J", "sym:s.ts::S", 6), Some(vec!["sym:j.ts::J".into(), "sym:w.ts::W".into(), "sym:s.ts::S.create".into()]));
        assert_eq!(trace(&graph(), "sym:s.ts::S", "sym:j.ts::J", 6), None);
        assert_eq!(trace(&graph(), "sym:j.ts::J", "sym:s.ts::S", 1), None);
    }

    #[test]
    fn risk_thresholds_are_the_documented_ones() {
        assert_eq!(risk(0, 0, 0), "LOW");
        assert_eq!(risk(4, 9, 2), "LOW");
        assert_eq!(risk(5, 5, 1), "MEDIUM");
        assert_eq!(risk(1, 1, 3), "MEDIUM");
        assert_eq!(risk(15, 15, 1), "HIGH");
        assert_eq!(risk(2, 40, 10), "HIGH");
        assert_eq!(risk(30, 30, 1), "CRITICAL");
        assert_eq!(risk(1, 60, 25), "CRITICAL");
    }

    #[test]
    fn render_lists_layers_with_path_line_and_ends_with_the_risk() {
        let g = graph();
        let out = render(&g, &upstream(&g, "sym:s.ts::S", 3), "upstream");
        assert!(out.starts_with("sym:s.ts::S  s.ts:3"));
        assert!(out.contains("d=1  will break (3)\n"));
        assert!(out.contains("  sym:c.ts::C.create  c.ts:9  Calls → sym:s.ts::S.create\n"));
        assert!(out.contains("importers (3): c.ts, m.ts, w.ts\n"));
        assert!(out.ends_with("risk: MEDIUM — 3 direct, 4 total, 4 files\n"));
    }

    #[test]
    fn render_json_is_valid_and_carries_the_same_counts() {
        let g = graph();
        let v: serde_json::Value = serde_json::from_str(&render_json(&g, &upstream(&g, "sym:s.ts::S", 3), "upstream")).unwrap();
        assert_eq!(v["root"], "sym:s.ts::S");
        assert_eq!(v["direction"], "upstream");
        assert_eq!(v["layers"][0].as_array().unwrap().len(), 3);
        assert_eq!(v["risk"], "MEDIUM");
    }
}
```

Add `mod impact;` to `src/main.rs`.

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test impact::`
Expected: compile errors — `aliases`, `canonical`, `upstream`, … not found.

- [ ] **Step 3: Implement**

Above the test module in `src/impact.rs`:

```rust
//! Walks over the code edges: who reaches a symbol, what a symbol reaches, and a path between
//! two. Callers import through barrels, so a caller's edge points at `sym:<barrel>::Name`,
//! never at the declaration; every walk therefore treats a symbol and its barrel aliases as one.

/// The edge kinds along which a change to the target reaches the source.
const CODE: [EdgeKind; 2] = [EdgeKind::Calls, EdgeKind::Extends];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Dependent { pub id: String, pub depth: usize, pub kind: EdgeKind, pub via: String }

#[derive(Debug, Default)]
pub struct Impact { pub root: String, pub layers: Vec<Vec<Dependent>>, pub importers: Vec<String> }

fn name_of(id: &str) -> &str { id.rsplit("::").next().unwrap_or(id) }

/// `S.create` is exported as `S`; a barrel names the class, not the member.
fn bare(name: &str) -> &str { name.split('.').next().unwrap_or(name) }

fn exports(e: &Edge, bare: &str) -> bool { e.context == "*" || e.context.split(',').any(|c| c == bare) }

/// Every `sym:<barrel>::<Name>` a caller could have reached this symbol by.
pub fn aliases(graph: &Graph, id: &str) -> Vec<String> {
    let Some(n) = graph.nodes.get(id) else { return Vec::new() };
    let name = name_of(id);
    let mut files = vec![n.file.clone()];
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::new();
    let mut i = 0;
    while i < files.len() {
        let target = format!("file:{}", files[i]);
        for e in graph.edges.iter().filter(|e| e.kind == EdgeKind::ReExports && e.target == target && exports(e, bare(name))) {
            let barrel = e.source.trim_start_matches("file:").to_string();
            if seen.insert(barrel.clone()) {
                out.push(format!("sym:{barrel}::{name}"));
                files.push(barrel);
            }
        }
        i += 1;
    }
    out
}

/// The node a possibly-dangling `sym:<barrel>::<Name>` stands for, following re-exports forward.
pub fn canonical(graph: &Graph, id: &str) -> Option<String> {
    if graph.nodes.contains_key(id) { return Some(id.to_string()) }
    let (file, name) = id.strip_prefix("sym:")?.rsplit_once("::")?;
    let mut files = vec![file.to_string()];
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut i = 0;
    while i < files.len() {
        let source = format!("file:{}", files[i]);
        for e in graph.edges.iter().filter(|e| e.kind == EdgeKind::ReExports && e.source == source && exports(e, bare(name))) {
            let next = e.target.trim_start_matches("file:").to_string();
            let candidate = format!("sym:{next}::{name}");
            if graph.nodes.contains_key(&candidate) { return Some(candidate) }
            if seen.insert(next.clone()) { files.push(next) }
        }
        i += 1;
    }
    None
}

/// The symbols a class declares: its methods and fields.
fn members(graph: &Graph, id: &str) -> Vec<String> {
    graph.edges.iter().filter(|e| e.kind == EdgeKind::Declares && e.source == id && e.target.starts_with("sym:")).map(|e| e.target.clone()).collect()
}

/// `sym:f::C` for `sym:f::C.m`; none for a class or a file.
fn container(id: &str) -> Option<String> {
    let (file, name) = id.strip_prefix("sym:")?.rsplit_once("::")?;
    let (class, _) = name.split_once('.')?;
    Some(format!("sym:{file}::{class}"))
}

/// The root, its members (a class is changed through them) and every alias of each.
fn seeds(graph: &Graph, root: &str) -> Vec<String> {
    let mut out = vec![root.to_string()];
    out.extend(members(graph, root));
    let aliased: Vec<String> = out.iter().flat_map(|s| aliases(graph, s)).collect();
    out.extend(aliased);
    out
}

/// The edges to follow from `at`. Upstream, a member is also reached through its class by a
/// subclass — `extends C` inherits `C.m` — so the class's `Extends` edges count for the member
/// without the class itself being listed. Downstream, a class reaches what its members call.
fn step<'a>(graph: &Graph, by_key: &BTreeMap<&str, Vec<&'a Edge>>, at: &str, up: bool) -> Vec<&'a Edge> {
    let mut out: Vec<&Edge> = by_key.get(at).into_iter().flatten().copied().collect();
    if up {
        if let Some(c) = container(at) {
            out.extend(by_key.get(c.as_str()).into_iter().flatten().copied().filter(|e| e.kind == EdgeKind::Extends));
        }
    } else {
        for m in members(graph, at) { out.extend(by_key.get(m.as_str()).into_iter().flatten().copied()); }
    }
    out
}

fn index<'a>(graph: &'a Graph, up: bool) -> BTreeMap<&'a str, Vec<&'a Edge>> {
    let mut by_key: BTreeMap<&str, Vec<&Edge>> = BTreeMap::new();
    for e in graph.edges.iter().filter(|e| CODE.contains(&e.kind)) {
        by_key.entry(if up { e.target.as_str() } else { e.source.as_str() }).or_default().push(e);
    }
    by_key
}

fn walk(graph: &Graph, root: &str, depth: usize, up: bool) -> Vec<Vec<Dependent>> {
    let by_key = index(graph, up);
    let start = seeds(graph, root);
    let mut seen: BTreeSet<String> = start.iter().cloned().collect();
    let mut frontier = start;
    let mut layers = Vec::new();
    for d in 1..=depth {
        let mut next: Vec<Dependent> = Vec::new();
        for at in &frontier {
            for e in step(graph, &by_key, at, up) {
                let other = if up { e.source.clone() } else { canonical(graph, &e.target).unwrap_or_else(|| e.target.clone()) };
                if seen.insert(other.clone()) {
                    next.push(Dependent { id: other, depth: d, kind: e.kind, via: at.clone() });
                }
            }
        }
        if next.is_empty() { break }
        next.sort_by(|a, b| a.id.cmp(&b.id));
        frontier = next.iter().map(|x| x.id.clone()).collect();
        layers.push(next);
    }
    layers
}

/// Files that import the symbol by name, from its file or any barrel: one hop, the "who
/// imports it" answer that survives even where no call site resolved.
fn importers(graph: &Graph, root: &str) -> Vec<String> {
    let Some(n) = graph.nodes.get(root) else { return Vec::new() };
    let name = bare(name_of(root));
    let mut files: BTreeSet<String> = BTreeSet::from([format!("file:{}", n.file)]);
    for a in aliases(graph, root) {
        if let Some((f, _)) = a.trim_start_matches("sym:").rsplit_once("::") { files.insert(format!("file:{f}")); }
    }
    let mut out: BTreeSet<String> = BTreeSet::new();
    for e in graph.edges.iter().filter(|e| e.kind == EdgeKind::Imports && files.contains(&e.target) && exports(e, name)) {
        out.insert(e.source.trim_start_matches("file:").to_string());
    }
    out.into_iter().collect()
}

pub fn upstream(graph: &Graph, root: &str, depth: usize) -> Impact {
    Impact { root: root.to_string(), layers: walk(graph, root, depth, true), importers: importers(graph, root) }
}

pub fn downstream(graph: &Graph, root: &str, depth: usize) -> Impact {
    Impact { root: root.to_string(), layers: walk(graph, root, depth, false), importers: Vec::new() }
}

/// The shortest chain of code edges from `from` to `to` (or one of its aliases or members),
/// at most `depth` hops.
pub fn trace(graph: &Graph, from: &str, to: &str, depth: usize) -> Option<Vec<String>> {
    let goal: BTreeSet<String> = seeds(graph, to).into_iter().collect();
    let by_source = index(graph, false);
    let mut parent: BTreeMap<String, String> = BTreeMap::new();
    // The walk starts at `from` alone: `step` already reaches through its members, and the
    // printed path then names the class, not the member that happened to make the call.
    let mut frontier: Vec<String> = vec![from.to_string()];
    let mut seen: BTreeSet<String> = frontier.iter().cloned().collect();
    for _ in 0..depth {
        let mut next = Vec::new();
        for at in &frontier {
            for e in step(graph, &by_source, at, false) {
                let other = canonical(graph, &e.target).unwrap_or_else(|| e.target.clone());
                if !seen.insert(other.clone()) { continue }
                parent.insert(other.clone(), at.clone());
                if goal.contains(&other) {
                    let mut path = vec![other];
                    while let Some(p) = parent.get(path.last().unwrap()) { path.push(p.clone()); }
                    path.reverse();
                    return Some(path);
                }
                next.push(other);
            }
        }
        if next.is_empty() { break }
        frontier = next;
    }
    None
}

/// Fixed thresholds, printed beside the counts that produced them so the label can be argued
/// with and the list still used.
pub fn risk(direct: usize, total: usize, files: usize) -> &'static str {
    let _ = total;
    if direct >= 30 || files >= 25 { "CRITICAL" }
    else if direct >= 15 || files >= 10 { "HIGH" }
    else if direct >= 5 || files >= 3 { "MEDIUM" }
    else { "LOW" }
}

pub fn files(graph: &Graph, imp: &Impact) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = imp.importers.iter().cloned().collect();
    for d in imp.layers.iter().flatten() {
        match graph.nodes.get(&d.id) { Some(n) => { out.insert(n.file.clone()); } None => { out.insert(d.id.trim_start_matches("file:").to_string()); } }
    }
    out
}

fn line_of(graph: &Graph, id: &str) -> String {
    match graph.nodes.get(id) { Some(n) => format!("{}:{}", n.file, n.line), None => "?".to_string() }
}

const LAYER_NAMES: [&str; 3] = ["will break", "likely affected", "may need testing"];

pub fn render(graph: &Graph, imp: &Impact, direction: &str) -> String {
    let mut out = format!("{}  {}\n", imp.root, line_of(graph, &imp.root));
    for (i, layer) in imp.layers.iter().enumerate() {
        let name = if direction == "upstream" { LAYER_NAMES.get(i).copied().unwrap_or("transitive") } else { "reaches" };
        out.push_str(&format!("d={}  {name} ({})\n", i + 1, layer.len()));
        for d in layer {
            let arrow = if direction == "upstream" { "→" } else { "←" };
            out.push_str(&format!("  {}  {}  {:?} {arrow} {}\n", d.id, line_of(graph, &d.id), d.kind, d.via));
        }
    }
    if direction == "upstream" {
        if !imp.importers.is_empty() { out.push_str(&format!("importers ({}): {}\n", imp.importers.len(), imp.importers.join(", "))); }
        let direct = imp.layers.first().map_or(0, Vec::len);
        let total: usize = imp.layers.iter().map(Vec::len).sum();
        let files = files(graph, imp).len();
        out.push_str(&format!("risk: {} — {direct} direct, {total} total, {files} files\n", risk(direct, total, files)));
    }
    out
}

pub fn render_json(graph: &Graph, imp: &Impact, direction: &str) -> String {
    let direct = imp.layers.first().map_or(0, Vec::len);
    let total: usize = imp.layers.iter().map(Vec::len).sum();
    let files = files(graph, imp);
    let layers: Vec<Vec<serde_json::Value>> = imp.layers.iter().map(|l| l.iter().map(|d| serde_json::json!({
        "id": d.id, "at": line_of(graph, &d.id), "depth": d.depth, "kind": format!("{:?}", d.kind), "via": d.via,
    })).collect()).collect();
    serde_json::json!({
        "root": imp.root, "at": line_of(graph, &imp.root), "direction": direction, "layers": layers,
        "importers": imp.importers, "files": files, "direct": direct, "total": total, "risk": risk(direct, total, files.len()),
    }).to_string() + "\n"
}
```

`src/query.rs:173`: `pub(crate) fn resolve<'a>(…)`.

- [ ] **Step 4: Run the tests**

Run: `cargo test impact:: && cargo clippy --all-targets --quiet`
Expected: 10 pass, clippy clean. If `render` totals differ from the test's `3 direct, 4 total, 4 files`: layers are `[m.ts, C.create, W.run]` and `[J]` (J reached through W.run's container W by `Extends`) → 4; files = importers {c.ts, m.ts, w.ts} ∪ {j.ts} = 4.

- [ ] **Step 5: Commit**

```bash
git add src/impact.rs src/query.rs src/main.rs
git commit -m "feat(impact): walk callers, callees and paths through barrel aliases" -m "Upstream and downstream BFS over Calls and Extends, a class seeded with its members, a caller's barrel-shaped target resolved to the declaration it stands for. Risk is four fixed thresholds printed with their counts."
```

---

### Task 4: `impact` and `trace` subcommands, README

**Files:**
- Modify: `src/main.rs:33-78` (Cmd enum), `src/main.rs:414-420` (arms next to Explain)
- Modify: `README.md:77-120` (Use), `README.md:271-288` (edge table), new section before `## Configure`

**Interfaces:**
- Consumes: `impact::{upstream, downstream, trace, render, render_json}`, `query::resolve`, `graph_for_ask(repo, cfg, store, stale, timing)`.
- Produces: CLI `impact <SYMBOL> [--depth 3] [--down] [--json] [--stale]`, `trace <FROM> <TO> [--depth 6] [--stale]`.

- [ ] **Step 1: Add the subcommands**

In `enum Cmd`, after `Explain { node: String },`:

```rust
    /// Who reaches a symbol (callers by depth, importing files, a risk line), or with `--down`
    /// what it reaches. A class is walked through its members; a caller that imported through a
    /// barrel is found all the same
    Impact {
        symbol: String,
        #[arg(long, default_value_t = 3)] depth: usize,
        #[arg(long)] down: bool,
        #[arg(long)] json: bool,
        /// Answers from the store as it stands, without bringing it in line with the tree first
        #[arg(long)] stale: bool,
    },
    /// The shortest chain of calls from one symbol to another, or that there is none within the depth
    Trace {
        from: String,
        to: String,
        #[arg(long, default_value_t = 6)] depth: usize,
        #[arg(long)] stale: bool,
    },
```

- [ ] **Step 2: Add a shared loader and the arms**

Above `fn main`, a loader that refreshes the way `ask` does and falls back the way `ask` does:

```rust
/// The graph an answer is read from: refreshed against the tree unless `--stale`, and, when
/// the store cannot be written, the stored one with a warning — the same contract as `ask`.
fn graph_for(repo: &std::path::Path, cfg: &config::Config, stale: bool) -> anyhow::Result<model::Graph> {
    let timing = Timing::new();
    let store = store::Store::new(repo);
    match graph_for_ask(repo, cfg, &store, stale, &timing) {
        Ok((graph, refreshed)) => {
            if let Some(r) = refreshed { eprintln!("refresh: {} changed, {} removed", r.changed, r.removed); }
            Ok(graph)
        }
        Err(err) => { eprintln!("refresh: skipped ({err:#})"); Ok(store.load()?.0) }
    }
}
```

Arms, after `Cmd::Explain`:

```rust
        Cmd::Impact { symbol, depth, down, json, stale } => {
            let graph = graph_for(&repo, &load_cfg()?, stale)?;
            let Some(root) = query::resolve(&graph, &symbol) else { anyhow::bail!("no node matches {symbol}") };
            let (imp, direction) = if down { (impact::downstream(&graph, &root.id, depth), "downstream") } else { (impact::upstream(&graph, &root.id, depth), "upstream") };
            print!("{}", if json { impact::render_json(&graph, &imp, direction) } else { impact::render(&graph, &imp, direction) });
            Ok(())
        }
        Cmd::Trace { from, to, depth, stale } => {
            let graph = graph_for(&repo, &load_cfg()?, stale)?;
            let Some(a) = query::resolve(&graph, &from) else { anyhow::bail!("no node matches {from}") };
            let Some(b) = query::resolve(&graph, &to) else { anyhow::bail!("no node matches {to}") };
            match impact::trace(&graph, &a.id, &b.id, depth) {
                Some(path) => {
                    for (i, id) in path.iter().enumerate() {
                        let at = graph.nodes.get(id).map(|n| format!("{}:{}", n.file, n.line)).unwrap_or_default();
                        println!("{}{id}  {at}", if i == 0 { "" } else { "  → " });
                    }
                    Ok(())
                }
                None => anyhow::bail!("no call path from {} to {} within {depth} hops", a.id, b.id),
            }
        }
```

- [ ] **Step 3: Build and run on the corpus**

```bash
cargo build --release --quiet
B=/Users/max/Documents/projects/beauty-crm
target/release/repograph --repo $B impact StaffService
target/release/repograph --repo $B impact --json StaffService.create | head -c 400; echo
target/release/repograph --repo $B impact --down StaffController
target/release/repograph --repo $B trace StaffController StaffService
target/release/repograph --repo $B impact NoSuchSymbol; echo "exit $?"
```
Expected: `impact StaffService` prints the root line, `d=1  will break (n)` with `sym:apps/api/src/modules/staff/staff.controller.ts::StaffController.create  apps/api/src/modules/staff/staff.controller.ts:<line>  Calls → sym:…::StaffService.create`, an `importers (…)` line containing `apps/api/src/modules/staff/staff.module.ts`, and a `risk:` line. `trace` prints a two- or three-line chain. The unknown symbol exits 1 with `no node matches`.

- [ ] **Step 4: Document**

`README.md` — in **Use**, after the `ask` block and before "Answers are lines of the form":

````markdown
Ask it what depends on a symbol, what a symbol reaches, and how one reaches another:

```bash
repograph impact StaffService               # callers by depth, importing files, a risk line
repograph impact --down StaffController     # what it calls, through injected services and barrels
repograph impact --json --depth 1 asGrosze  # machine-readable; depth 1 is the "will break" list alone
repograph trace StaffController StaffService  # shortest chain of calls between two symbols
```
````

Replace the `Calls` row of the edge table:

```markdown
| `Calls`       | a call or `new` whose callee the file can prove: an imported name, a top-level declaration of the same file, `this.member()`, `Static.member()`, or `this.field.member()` through the field's declared type (constructor parameter properties included); a call through a barrel targets the barrel and is resolved by `impact` |
```

New section before `## Configure`:

````markdown
## Blast radius

`impact <symbol>` walks `Calls` and `Extends` edges towards the symbol: `d=1` are the direct
callers ("will break"), `d=2` their callers, and so on to `--depth` (3). A class is walked
through its members, and a caller that imported through a barrel is found because the barrel's
`ReExports` edges are followed back to the declaration. `importers` are the files whose `import`
names the symbol, whether or not a call site resolved. The risk line is four fixed thresholds
on the direct count and the file count — `MEDIUM` from 5 direct or 3 files, `HIGH` from 15 or
10, `CRITICAL` from 30 or 25 — printed with the counts, so the label can be argued with.

```
$ repograph --repo beauty-crm impact StaffService
sym:apps/api/src/modules/staff/staff.service.ts::StaffService  apps/api/src/modules/staff/staff.service.ts:19
d=1  will break (2)
  file:apps/api/src/modules/staff/staff.module.ts  apps/api/src/modules/staff/staff.module.ts:1  Calls → sym:…::StaffService
  sym:apps/api/src/modules/staff/staff.controller.ts::StaffController.create  apps/api/src/modules/staff/staff.controller.ts:22  Calls → sym:…::StaffService.create
importers (2): apps/api/src/modules/staff/staff.controller.ts, apps/api/src/modules/staff/staff.module.ts
risk: LOW — 2 direct, 2 total, 2 files
```

`--down` walks the other way; `trace <from> <to>` is the shortest chain between two symbols.
What the graph cannot prove it does not list: a call through a chained expression, a
destructured method, a callback parameter or a global has no edge, so confirm a "nothing uses
this" with `rg -l` before deleting.
````

Paste the real output of Step 3 into that example in place of the illustrative one.

- [ ] **Step 5: Test, clippy, commit**

Run: `cargo test --quiet && cargo clippy --all-targets --quiet`
Expected: pass.

```bash
git add src/main.rs README.md
git commit -m "feat(cli): impact and trace, refreshed like ask" -m "impact prints callers by depth with path:line, the importing files and a risk line; --down walks callees; trace prints the shortest call chain. Both refresh the store against the tree first, so the answer is the working tree's."
```

---

### Task 5: `changes` — map the diff onto the graph

**Files:**
- Create: `src/changes.rs`
- Modify: `src/main.rs` (`mod changes;`, Cmd, arm)
- Modify: `README.md` (Use block, Blast radius section)
- Test: `src/changes.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `Node.end`, `impact::{upstream, Dependent, risk}`.
- Produces:
  - `pub struct Hunk { pub file: String, pub start: u32, pub end: u32 }`
  - `pub fn parse(diff: &str) -> Vec<Hunk>`
  - `pub fn touched(graph, hunks) -> Vec<String>`
  - `pub struct Report { pub touched: Vec<String>, pub affected: Vec<Dependent>, pub files: BTreeSet<String>, pub risk: &'static str }`
  - `pub fn report(graph, hunks, depth) -> Report`
  - `pub fn hunks_from_git(repo: &Path, base: &str) -> anyhow::Result<Vec<Hunk>>`
  - `pub fn render(graph, r: &Report) -> String`, `pub fn render_json(graph, r) -> String`
  - CLI `changes [--base HEAD] [--depth 2] [--json] [--stale]`

- [ ] **Step 1: Write the failing tests**

Create `src/changes.rs` with the test module:

```rust
use crate::impact::{self, Dependent};
use crate::model::{Graph, NodeKind};
use std::collections::BTreeSet;
use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EdgeKind, Extraction};

    const DIFF: &str = "diff --git a/s.ts b/s.ts\n--- a/s.ts\n+++ b/s.ts\n@@ -6,2 +6,3 @@ export class S {\n+  // more\n@@ -20 +21,0 @@\n-old\ndiff --git a/new.ts b/new.ts\nnew file mode 100644\n--- /dev/null\n+++ b/new.ts\n@@ -0,0 +1,2 @@\n+a\n+b\ndiff --git a/gone.ts b/gone.ts\n--- a/gone.ts\n+++ /dev/null\n@@ -1,3 +0,0 @@\n-x\n";

    #[test]
    fn parse_takes_new_side_ranges_and_records_a_pure_deletion_as_one_line() {
        assert_eq!(parse(DIFF), vec![
            Hunk { file: "s.ts".into(), start: 6, end: 8 },
            Hunk { file: "s.ts".into(), start: 21, end: 21 },
            Hunk { file: "new.ts".into(), start: 1, end: 2 },
        ]);
    }

    fn graph() -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::File, "file:s.ts", "s.ts", "", "s.ts", 1);
        e.node_span(NodeKind::Symbol, "sym:s.ts::S", "S", "", "s.ts", 3, 12);
        e.node_span(NodeKind::Symbol, "sym:s.ts::S.create", "S.create", "", "s.ts", 5, 8);
        e.node_span(NodeKind::Symbol, "sym:s.ts::S.list", "S.list", "", "s.ts", 9, 11);
        e.node_span(NodeKind::Symbol, "sym:s.ts::helper", "helper", "", "s.ts", 14, 14);
        e.edge("file:s.ts", "sym:s.ts::S", EdgeKind::Declares, "export", "s.ts");
        e.edge("sym:s.ts::S", "sym:s.ts::S.create", EdgeKind::Declares, "", "s.ts");
        e.edge("sym:s.ts::S", "sym:s.ts::S.list", EdgeKind::Declares, "", "s.ts");
        e.node(NodeKind::Symbol, "sym:c.ts::C.create", "C.create", "", "c.ts", 9);
        e.edge("sym:c.ts::C.create", "sym:s.ts::S.create", EdgeKind::Calls, "", "c.ts");
        g.apply(e);
        g
    }

    #[test]
    fn a_hunk_inside_a_member_names_the_member_not_the_class() {
        assert_eq!(touched(&graph(), &[Hunk { file: "s.ts".into(), start: 6, end: 7 }]), vec!["sym:s.ts::S.create"]);
    }

    #[test]
    fn a_hunk_spanning_two_members_names_both() {
        assert_eq!(touched(&graph(), &[Hunk { file: "s.ts".into(), start: 8, end: 9 }]), vec!["sym:s.ts::S.create", "sym:s.ts::S.list"]);
    }

    #[test]
    fn a_hunk_in_the_class_but_outside_every_member_names_the_class() {
        assert_eq!(touched(&graph(), &[Hunk { file: "s.ts".into(), start: 4, end: 4 }]), vec!["sym:s.ts::S"]);
    }

    #[test]
    fn a_hunk_outside_every_symbol_falls_to_the_file() {
        assert_eq!(touched(&graph(), &[Hunk { file: "s.ts".into(), start: 1, end: 1 }]), vec!["file:s.ts"]);
    }

    #[test]
    fn a_whole_file_hunk_names_every_symbol_of_the_file_but_no_containing_class() {
        assert_eq!(touched(&graph(), &[Hunk { file: "s.ts".into(), start: 1, end: u32::MAX }]), vec!["sym:s.ts::S.create", "sym:s.ts::S.list", "sym:s.ts::helper"]);
    }

    #[test]
    fn a_hunk_in_an_unknown_file_is_ignored() {
        assert!(touched(&graph(), &[Hunk { file: "none.ts".into(), start: 1, end: 9 }]).is_empty());
    }

    #[test]
    fn report_unions_the_callers_of_every_touched_symbol() {
        let g = graph();
        let r = report(&g, &[Hunk { file: "s.ts".into(), start: 6, end: 7 }], 2);
        assert_eq!(r.touched, vec!["sym:s.ts::S.create"]);
        assert_eq!(r.affected.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(), vec!["sym:c.ts::C.create"]);
        assert_eq!(r.files, BTreeSet::from(["c.ts".to_string()]));
        assert_eq!(r.risk, "LOW");
    }

    #[test]
    fn render_says_what_changed_and_what_it_reaches() {
        let g = graph();
        let out = render(&g, &report(&g, &[Hunk { file: "s.ts".into(), start: 6, end: 7 }], 2));
        assert_eq!(out, "changed: 1 symbol in 1 file\n  sym:s.ts::S.create  s.ts:5-8\naffected (depth 2): 1 symbol in 1 file\n  d=1  sym:c.ts::C.create  c.ts:9  ← sym:s.ts::S.create\nrisk: LOW — 1 direct, 1 total, 1 file\n");
    }

    #[test]
    fn an_empty_diff_renders_a_clean_report() {
        let g = graph();
        assert_eq!(render(&g, &report(&g, &[], 2)), "changed: 0 symbols\n");
    }
}
```

Add `mod changes;` to `src/main.rs`.

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test changes::`
Expected: compile errors, nothing defined.

- [ ] **Step 3: Implement**

Above the tests in `src/changes.rs`:

```rust
//! The working tree's diff, read onto the graph: which symbols the hunks sit in, and who reaches
//! those. Line numbers come from the new side of `git diff -U0`, so the graph must have been
//! refreshed against the working tree first — `main` does that before calling in.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk { pub file: String, pub start: u32, pub end: u32 }

/// New-side ranges of a zero-context unified diff. A pure deletion (`+c,0`) has no new lines;
/// it is recorded as the line the cut lands on, so the symbol around it still counts as changed.
pub fn parse(diff: &str) -> Vec<Hunk> {
    let mut out = Vec::new();
    let mut file: Option<String> = None;
    for line in diff.lines() {
        if let Some(p) = line.strip_prefix("+++ ") {
            file = p.strip_prefix("b/").map(str::to_string);
            continue;
        }
        let Some(rest) = line.strip_prefix("@@ ") else { continue };
        let Some(f) = &file else { continue };
        let Some(plus) = rest.split_whitespace().find(|w| w.starts_with('+')) else { continue };
        let (c, d) = match plus[1..].split_once(',') {
            Some((c, d)) => (c.parse::<u32>().unwrap_or(0), d.parse::<u32>().unwrap_or(1)),
            None => (plus[1..].parse::<u32>().unwrap_or(0), 1),
        };
        let (start, end) = if d == 0 { (c.max(1), c.max(1)) } else { (c, c + d - 1) };
        out.push(Hunk { file: f.clone(), start, end });
    }
    out
}

/// Symbols whose span meets a hunk; a hunk outside every symbol falls to its file node. A class
/// whose member matched is dropped — the member is the change, the class only contains it.
pub fn touched(graph: &Graph, hunks: &[Hunk]) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for h in hunks {
        let mut any = false;
        for n in graph.nodes.values().filter(|n| n.kind == NodeKind::Symbol && n.file == h.file && n.line > 0) {
            let end = n.end.max(n.line);
            if n.line <= h.end && end >= h.start { out.insert(n.id.clone()); any = true; }
        }
        let file_id = format!("file:{}", h.file);
        if !any && graph.nodes.contains_key(&file_id) { out.insert(file_id); }
    }
    let members: Vec<String> = out.iter().cloned().collect();
    out.retain(|id| !members.iter().any(|m| m.len() > id.len() && m.starts_with(id) && m[id.len()..].starts_with('.')));
    out.into_iter().collect()
}

pub struct Report { pub touched: Vec<String>, pub affected: Vec<Dependent>, pub files: BTreeSet<String>, pub risk: &'static str }

pub fn report(graph: &Graph, hunks: &[Hunk], depth: usize) -> Report {
    let touched = touched(graph, hunks);
    let mut affected: Vec<Dependent> = Vec::new();
    let mut files: BTreeSet<String> = BTreeSet::new();
    for id in &touched {
        let imp = impact::upstream(graph, id, depth);
        files.extend(impact::files(graph, &imp));
        for d in imp.layers.into_iter().flatten() {
            // One row per dependent, at the shallowest depth any touched symbol reaches it.
            match affected.iter_mut().find(|a| a.id == d.id) {
                Some(a) if d.depth < a.depth => *a = d,
                Some(_) => {}
                None => affected.push(d),
            }
        }
    }
    affected.sort_by(|a, b| (a.depth, &a.id).cmp(&(b.depth, &b.id)));
    let direct = affected.iter().filter(|d| d.depth == 1).count();
    let risk = impact::risk(direct, affected.len(), files.len());
    Report { touched, affected, files, risk }
}

fn plural(n: usize, one: &str) -> String { format!("{n} {one}{}", if n == 1 { "" } else { "s" }) }

fn span_of(graph: &Graph, id: &str) -> String {
    match graph.nodes.get(id) {
        Some(n) if n.end > n.line => format!("{}:{}-{}", n.file, n.line, n.end),
        Some(n) => format!("{}:{}", n.file, n.line),
        None => "?".into(),
    }
}

pub fn render(graph: &Graph, r: &Report) -> String {
    if r.touched.is_empty() { return "changed: 0 symbols\n".into() }
    let changed_files: BTreeSet<&str> = r.touched.iter().filter_map(|id| graph.nodes.get(id).map(|n| n.file.as_str())).collect();
    let mut out = format!("changed: {} in {}\n", plural(r.touched.len(), "symbol"), plural(changed_files.len(), "file"));
    for id in &r.touched { out.push_str(&format!("  {id}  {}\n", span_of(graph, id))); }
    let depth = r.affected.iter().map(|d| d.depth).max().unwrap_or(0).max(1);
    out.push_str(&format!("affected (depth {depth}): {} in {}\n", plural(r.affected.len(), "symbol"), plural(r.files.len(), "file")));
    for d in &r.affected {
        let at = graph.nodes.get(&d.id).map(|n| format!("{}:{}", n.file, n.line)).unwrap_or_else(|| d.id.trim_start_matches("file:").to_string());
        out.push_str(&format!("  d={}  {}  {at}  ← {}\n", d.depth, d.id, d.via));
    }
    let direct = r.affected.iter().filter(|d| d.depth == 1).count();
    out.push_str(&format!("risk: {} — {direct} direct, {} total, {}\n", r.risk, r.affected.len(), plural(r.files.len(), "file")));
    out
}

pub fn render_json(graph: &Graph, r: &Report) -> String {
    let touched: Vec<serde_json::Value> = r.touched.iter().map(|id| serde_json::json!({ "id": id, "at": span_of(graph, id) })).collect();
    let affected: Vec<serde_json::Value> = r.affected.iter().map(|d| serde_json::json!({
        "id": d.id, "at": graph.nodes.get(&d.id).map(|n| format!("{}:{}", n.file, n.line)), "depth": d.depth, "kind": format!("{:?}", d.kind), "via": d.via,
    })).collect();
    serde_json::json!({ "touched": touched, "affected": affected, "files": r.files, "risk": r.risk }).to_string() + "\n"
}

fn git(repo: &Path, args: &[&str]) -> anyhow::Result<String> {
    let out = std::process::Command::new("git").arg("-C").arg(repo).args(args).output()?;
    anyhow::ensure!(out.status.success(), "git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Hunks of the working tree against `base` — staged and unstaged alike, plus every untracked
/// file as one hunk over its whole length, so a new file's symbols count as changed too.
pub fn hunks_from_git(repo: &Path, base: &str) -> anyhow::Result<Vec<Hunk>> {
    let mut hunks = parse(&git(repo, &["diff", "-U0", "--no-color", "--no-ext-diff", base, "--", "."])?);
    for f in git(repo, &["ls-files", "--others", "--exclude-standard"])?.lines().filter(|l| !l.is_empty()) {
        hunks.push(Hunk { file: f.to_string(), start: 1, end: u32::MAX });
    }
    Ok(hunks)
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test changes:: && cargo clippy --all-targets --quiet`
Expected: 10 pass. If `parse` mis-reads `@@ -20 +21,0 @@`: the `+` word is `+21,0` → `c=21, d=0` → `(21, 21)`.

- [ ] **Step 5: Add the subcommand**

`enum Cmd`, after `Trace`:

```rust
    /// What the working tree's diff touches and who reaches it: hunks against `--base` (staged,
    /// unstaged and untracked alike) mapped onto symbol spans, then the callers of each
    Changes {
        #[arg(long, default_value = "HEAD")] base: String,
        #[arg(long, default_value_t = 2)] depth: usize,
        #[arg(long)] json: bool,
        #[arg(long)] stale: bool,
    },
```

Arm after `Cmd::Trace`:

```rust
        Cmd::Changes { base, depth, json, stale } => {
            let graph = graph_for(&repo, &load_cfg()?, stale)?;
            let hunks = changes::hunks_from_git(&repo, &base)?;
            let r = changes::report(&graph, &hunks, depth);
            print!("{}", if json { changes::render_json(&graph, &r) } else { changes::render(&graph, &r) });
            Ok(())
        }
```

- [ ] **Step 6: Run on the corpus with a real edit**

```bash
cargo build --release --quiet
B=/Users/max/Documents/projects/beauty-crm
target/release/repograph --repo $B changes            # clean tree
F=$B/apps/api/src/modules/staff/staff.service.ts
cp $F /tmp/staff.service.bak
printf '\n// probe\n' >> $F
sed -i '' 's/^\(\s*async create(\)/\1\/* probe *\/ /' $F
target/release/repograph --repo $B changes
target/release/repograph --repo $B changes --json | head -c 300; echo
target/release/repograph --repo $B changes --base main | head -5
cp /tmp/staff.service.bak $F && rm /tmp/staff.service.bak
target/release/repograph --repo $B changes
```
Expected: first and last runs print `changed: 0 symbols` (the tree is at `main` with no local edits; if `--base main` on a branch lists commits' hunks that is correct). The middle run lists `sym:…::StaffService.create` and `file:apps/api/src/modules/staff/staff.service.ts` (the trailing comment sits outside every symbol), then `d=1  sym:…::StaffController.create …`. The `refresh:` line on stderr proves the graph was brought up to date before mapping. If the `sed` expression does not match the method's first line, edit any line inside `create` by hand instead — the point is one hunk inside one member.

- [ ] **Step 7: Document**

`README.md` — in the **Use** block from Task 4, add:

```bash
repograph changes                           # what the uncommitted diff touches, and who reaches it
repograph changes --base main --depth 1     # the whole branch; depth 1 is the direct callers alone
```

In **Blast radius**, after the `trace` paragraph:

```markdown
`changes` maps `git diff -U0` (staged and unstaged, plus untracked files whole) onto symbol
spans and unions the callers of every touched symbol into one list and one risk line. Run it
before committing; `--base main` before opening a pull request. A hunk outside every symbol —
an import line, a trailing comment — is reported on the file. Deleted files do not appear:
their symbols are gone from the graph, and their former callers surface as dangling edges in
`verify`.
```

- [ ] **Step 8: Test, clippy, commit**

Run: `cargo test --quiet && cargo clippy --all-targets --quiet`
Expected: pass.

```bash
git add src/changes.rs src/main.rs README.md
git commit -m "feat(cli): changes maps the diff onto symbol spans and their callers" -m "git diff -U0 hunks against a base (HEAD by default), untracked files whole, mapped onto the symbols whose spans they meet — a member over its class, the file when no symbol fits — then one upstream walk per touched symbol into one report. The store is refreshed first so the lines are the working tree's."
```

---

### Task 6: Release 0.4.0

**Files:**
- Modify: `README.md:35` (status line), `npm/repograph/package.json:3,32-33`, `npm/repograph-darwin-arm64/package.json:3`, `npm/repograph-linux-x64/package.json:3`

- [ ] **Step 1: Bump what the release reads**

```bash
sed -i '' '35s/Version 0\.3\.0\./Version 0.4.0./' README.md
sed -i '' '3s/0\.3\.0/0.4.0/' npm/repograph-darwin-arm64/package.json npm/repograph-linux-x64/package.json npm/repograph/package.json
sed -i '' '32,33s/0\.3\.0/0.4.0/' npm/repograph/package.json
grep -rn "0\.[34]\.0" Cargo.toml README.md npm/*/package.json
```
Expected: every line reads `0.4.0`; `Cargo.toml` already does from Task 1.

- [ ] **Step 2: Final proof on the corpus**

```bash
cargo test --quiet && cargo clippy --all-targets --quiet && cargo build --release --quiet
B=/Users/max/Documents/projects/beauty-crm
target/release/repograph --repo $B --no-dense build
target/release/repograph --repo $B verify | head -2
target/release/repograph --repo $B bench
target/release/repograph --repo $B --no-dense bench
```
Expected: node counts as in 0.3.0, `Calls` present, floors met.

- [ ] **Step 3: Commit, tag, push**

```bash
git add README.md npm/repograph/package.json npm/repograph-darwin-arm64/package.json npm/repograph-linux-x64/package.json
git commit -m "chore: release 0.4.0"
git tag -a v0.4.0 -m "repograph 0.4.0"
git push origin main && git push origin v0.4.0
GH_TOKEN="$(gh auth token -u devmaxxx)" gh run watch --repo devmaxxx/repograph --exit-status "$(GH_TOKEN="$(gh auth token -u devmaxxx)" gh run list --repo devmaxxx/repograph --workflow release --limit 1 --json databaseId --jq '.[0].databaseId')"
```
Expected: the release workflow goes green; `gh api /user/packages/npm/repograph/versions` lists `0.4.0`. The active `gh` account cannot see the repository — the `devmaxxx` token is required, as above.

---

### Task 7: Move `beauty-crm` off the escape hatch

**Files (all under `/Users/max/Documents/projects/beauty-crm`):**
- Modify: `package.json:44`, `pnpm-lock.yaml` (via `pnpm install`)
- Modify: `.claude/CLAUDE.md:47-53` (paragraph), `.claude/CLAUDE.md:55-70` (list + new section)
- Modify: `.claude/skills/repo-query/SKILL.md:9-10`, `:36`, `:117-125`
- Modify: `.claude/hooks/repograph-notice.mjs:33-51` (`FULL_NOTICE`)
- Modify: `.gitignore:122-123`, `.ignore:4`

- [ ] **Step 1: Take the release**

```bash
cd /Users/max/Documents/projects/beauty-crm
sed -i '' 's/"@devmaxxx\/repograph": "0\.3\.0"/"@devmaxxx\/repograph": "0.4.0"/' package.json
pnpm install
pnpm exec repograph --version
pnpm exec repograph impact StaffService | tail -1
```
Expected: `repograph 0.4.0`; a `risk:` line.

- [ ] **Step 2: Commit the dependency alone**

Touches the lockfile, so the pipeline must run — no `[skip ci]`.

```bash
git add package.json pnpm-lock.yaml
git commit -m "build(deps): take repograph 0.4.0 for impact, trace and changes"
```

- [ ] **Step 3: Rewrite the GitNexus paragraph in `.claude/CLAUDE.md`**

Replace lines 47–53 (the `**GitNexus в этом репозитории выключен**` paragraph) with:

```markdown
**GitNexus здесь больше не нужен** (снят 2026-09-03, индекс и скиллы удалены). Единственное,
ради чего его держали рядом — граф вызовов и радиус поражения — с 0.4.0 отвечает сам
`repograph`: `impact`, `trace`, `changes` ниже. Глобальный MCP-сервер и два хука в
`~/.claude/settings.json` остаются ради других репозиториев и здесь молчат, не найдя `.gitnexus/`.
```

Extend the list under `## Спрашивать граф до чтения файлов` (after the `--rerank` bullet):

```markdown
- `repograph impact <символ>` — кто вызывает, по глубине, с `путь:строка`; файлы-импортёры;
  строка риска. `--down` — что вызывает сам символ. Класс обходится через его методы, вызов
  через barrel находится;
- `repograph trace <от> <до>` — кратчайшая цепочка вызовов между двумя символами;
- `repograph changes` — что задевает текущий diff (staged, unstaged, новые файлы) и кто до
  этого дотягивается; `--base main` — вся ветка.
```

Add a new section right after that list, before `## Свежесть: граф не устаревает`:

```markdown
## Перед правкой символа и перед коммитом

- **Перед правкой функции, класса или метода** — `repograph impact <символ>` и сообщи
  пользователю радиус: прямые вызывающие (`d=1`), число файлов, риск. `HIGH` и `CRITICAL` —
  предупреди до правки.
- **Перед коммитом** — `repograph changes`: список задетых символов и их вызывающих обязан
  совпадать с тем, что ты собирался менять. Перед PR — `repograph changes --base main`.
- **Переименование** — не поиском-заменой: `repograph impact <старое>` даёт файлы, правка
  идёт через рефакторинг редактора или `ast-grep`, `repograph changes` подтверждает объём.
- Граф записывает только доказуемые вызовы (импорт, объявление в том же файле, типизированное
  поле `this.service.x()`); «никто не использует» проверяй `rg -l` перед удалением.
```

- [ ] **Step 4: Update the `repo-query` skill**

Line 9–10, replace `GitNexus is off in this repository and graphify is frozen — both are covered at the end, with what it takes to bring them back.` with `GitNexus is gone from this repository and graphify is frozen — see the end.`

Replace the route row at line 36:

```markdown
| Who calls it? What breaks if it changes? | `repograph impact <Symbol>` — callers by depth, importers, risk; `--down` for callees | ~150–400 tok, ~60 ms |
| How does A reach B? | `repograph trace <A> <B>` | ~80 tok |
| What does my diff touch, and who reaches that? | `repograph changes` (`--base main` for the branch) | ~200 tok |
```

Replace the `**GitNexus is switched off here**` paragraph (lines 117–125) with:

```markdown
**GitNexus is gone.** Its index, skills and `AGENTS.md` block were removed on 2026-09-03; what
it was kept beside repograph for — blast radius and call chains — is `repograph impact`,
`trace` and `changes` since 0.4.0. The user-level MCP server and hooks in `~/.claude/settings.json`
serve other repositories and exit here on the missing `.gitnexus/`.
```

Keep the graphify paragraph as it is.

- [ ] **Step 5: Extend the notice hook**

`.claude/hooks/repograph-notice.mjs`, in `FULL_NOTICE` after the `repograph ask --json …` line:

```js
  '  repograph impact <symbol>     — callers by depth, importing files, a risk line; run it',
  '                                  before editing a symbol; --down for what it calls',
  '  repograph changes             — what the uncommitted diff touches and who reaches it;',
  '                                  run it before committing (--base main before a PR)',
```

Check it still parses: `node .claude/hooks/repograph-notice.mjs < /dev/null; echo $?` → `0`.

- [ ] **Step 6: Drop the ignore lines**

Remove `.gitignore` lines 122–123 (`# gitnexus: …` and `.gitnexus/`) and `.ignore` line 4 (`.gitnexus/`).

- [ ] **Step 7: Verify no reference is left**

```bash
grep -rIn -i gitnexus . --exclude-dir=node_modules --exclude-dir=.git --exclude-dir=graphify-out --exclude-dir=.repograph --exclude-dir=.turbo
```
Expected: only the two "GitNexus is gone / больше не нужен" sentences from Steps 3–4. (`graphify-out/` is the frozen layer and keeps its history.)

- [ ] **Step 8: Commit the docs and hook**

Prose and a hook only — the pipeline reads none of it.

```bash
git add .claude/CLAUDE.md .claude/skills/repo-query/SKILL.md .claude/hooks/repograph-notice.mjs .gitignore .ignore
git commit -m "docs(claude): put the pre-edit and pre-commit rules on repograph impact and changes [skip ci]"
```

---

## Release 0.5.0 — registry, worktrees, one hook

Starts after `v0.4.0` is tagged and `beauty-crm` is on it. Tasks 8 → 9 → 10 → 11 → 12 in order; Task 13 is host setup and can run any time after 12.

### File structure

```
src/walk.rs             filter_entry: skip a subdirectory that holds a .git       (Task 8)
src/registry.rs         NEW — Identity, Checkout, Project, Registry; detect, branch, touch,
                        prune, resolve, freshest_store, Status, render, render_json (Task 9, 11)
src/store.rs            identity(), write_identity(), seed_from(), SEEDED       (Task 10)
src/main.rs             --repo <name>, note_use, seeding in build/update, repos  (Task 10, 11)
README.md               Use / new "Many repositories" section                   (Task 12)
~/.claude/hooks/repograph-notice.mjs, ~/.claude/settings.json                   (Task 13)
```

---

### Task 8: The walker stops at a nested checkout

**Files:**
- Modify: `src/walk.rs:52` (the `WalkBuilder` chain)
- Test: `src/walk.rs` tests module

**Interfaces:**
- Produces: no API change; `walk()` never yields a file under a subdirectory that contains a `.git` entry.

- [ ] **Step 1: Write the failing test**

In the `tests` module of `src/walk.rs`, after `walk_classifies_and_skips`:

```rust
    // A worktree in a visible subdirectory, a vendored repository, a fix-clone dropped into
    // the tree: each has its own `.git` and is somebody else's checkout, not this one's files.
    #[test]
    fn a_nested_checkout_is_not_walked() {
        let d = repo();
        std::fs::create_dir_all(d.path().join("vendor/inner")).unwrap();
        std::fs::write(d.path().join("vendor/inner/.git"), "gitdir: /elsewhere\n").unwrap();
        std::fs::write(d.path().join("vendor/inner/y.ts"), "export const y = 1;\n").unwrap();
        std::fs::write(d.path().join("vendor/z.ts"), "export const z = 1;\n").unwrap();
        let rels: Vec<String> = walk(d.path(), &Config::default(), &Manifest::default()).unwrap().into_iter().map(|e| e.rel).collect();
        assert!(rels.contains(&"vendor/z.ts".to_string()));
        assert!(!rels.iter().any(|r| r.starts_with("vendor/inner/")), "{rels:?}");
    }
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test walk::tests::a_nested_checkout_is_not_walked`
Expected: FAIL — `vendor/inner/y.ts` is walked.

- [ ] **Step 3: Filter the entry**

`src/walk.rs`, replace the builder line:

```rust
    // The root's own `.git` sits at depth 0; any deeper one marks another checkout.
    let walker = ignore::WalkBuilder::new(repo).hidden(true).git_ignore(true)
        .filter_entry(|d| d.depth() == 0 || !(d.file_type().is_some_and(|t| t.is_dir()) && d.path().join(".git").exists()))
        .build();
    for dent in walker {
```

- [ ] **Step 4: Run the walk tests and the corpus**

Run: `cargo test walk:: && cargo build --release --quiet && target/release/repograph --repo /Users/max/Documents/projects/beauty-crm --no-dense build && target/release/repograph --repo /Users/max/Documents/projects/beauty-crm verify | head -2`
Expected: tests pass; `verify` counts unchanged from 0.4.0 (the nested worktree there is under hidden `.claude/`, already skipped).

- [ ] **Step 5: Commit**

```bash
git add src/walk.rs
git commit -m "fix(walk): stop at a subdirectory that is its own checkout" -m "A worktree in a visible directory, a vendored repository or a fix-clone inside the tree carries a .git of its own; its files belong to that checkout's graph, not to this one's."
```

---

### Task 9: `src/registry.rs` — facts git already knows

**Files:**
- Create: `src/registry.rs`
- Modify: `src/main.rs:1-14` (`mod registry;`)
- Test: `src/registry.rs` `#[cfg(test)]`

**Interfaces:**
- Produces:
  - `pub struct Identity { pub key: String, pub name: String, pub main: bool }`, `pub const IDENTITY: &str = "identity.json"`
  - `pub struct Checkout { pub path: String, pub branch: String, pub main: bool, pub last_used: u64 }`, `pub struct Project { pub name: String, pub checkouts: Vec<Checkout> }`, `pub struct Registry { pub projects: BTreeMap<String, Project> }`
  - `pub fn path() -> Result<PathBuf>`, `pub fn detect(repo: &Path) -> Identity`, `pub fn branch(repo: &Path) -> String`
  - `Registry::{load, save, load_from(&Path), save_to(&Path), prune, touch(&Identity, &Path, &str) -> bool, resolve(&str, cwd: &Path) -> Result<PathBuf>, freshest_store(key: &str, except: &Path) -> Option<PathBuf>}`

- [ ] **Step 1: Write the failing tests**

Create `src/registry.rs` with the test module:

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests {
    use super::*;

    fn id(key: &str, name: &str) -> Identity { Identity { key: key.into(), name: name.into(), main: true } }

    fn git_repo() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            let out = std::process::Command::new("git").arg("-C").arg(d.path()).args(args).output().unwrap();
            assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        };
        run(&["init", "-q"]);
        run(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "--allow-empty", "-q", "-m", "root"]);
        d
    }

    fn git_out(repo: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git").arg("-C").arg(repo).args(args).output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[test]
    fn identity_of_a_git_repo_is_its_root_commit_and_its_directory_name() {
        let d = git_repo();
        let i = detect(d.path());
        assert_eq!(i.key, git_out(d.path(), &["rev-list", "--max-parents=0", "HEAD"]));
        assert_eq!(i.name, d.path().file_name().unwrap().to_str().unwrap());
        assert!(i.main);
    }

    #[test]
    fn identity_takes_its_name_from_origin_when_there_is_one() {
        let d = git_repo();
        git_out(d.path(), &["remote", "add", "origin", "git@github-alias:Owner/some-repo.git"]);
        assert_eq!(detect(d.path()).name, "some-repo");
    }

    #[test]
    fn a_linked_worktree_shares_the_key_and_is_not_main() {
        let d = git_repo();
        let wt = tempfile::tempdir().unwrap();
        let wt_path = wt.path().join("wt");
        git_out(d.path(), &["worktree", "add", "-q", wt_path.to_str().unwrap(), "-b", "wt"]);
        let a = detect(d.path());
        let b = detect(&wt_path);
        assert_eq!(a.key, b.key);
        assert!(a.main && !b.main);
        assert_eq!(branch(&wt_path), "wt");
        assert_eq!(branch(d.path()), git_out(d.path(), &["rev-parse", "--abbrev-ref", "HEAD"]));
    }

    #[test]
    fn a_directory_without_history_keys_on_its_path() {
        let d = tempfile::tempdir().unwrap();
        let i = detect(d.path());
        assert_eq!(i.key, format!("path:{}", d.path().display()));
        assert!(branch(d.path()).is_empty());
    }

    #[test]
    fn touch_adds_a_checkout_and_a_repeat_within_the_window_changes_nothing() {
        let d = tempfile::tempdir().unwrap();
        let mut r = Registry::default();
        assert!(r.touch(&id("k1", "alpha"), d.path(), "main"));
        assert!(!r.touch(&id("k1", "alpha"), d.path(), "main"));
        assert!(r.touch(&id("k1", "alpha"), d.path(), "other"));
        let p = &r.projects["k1"];
        assert_eq!(p.name, "alpha");
        assert_eq!(p.checkouts.len(), 1);
        assert_eq!(p.checkouts[0].branch, "other");
    }

    #[test]
    fn a_second_project_with_a_taken_name_gets_a_key_suffix_and_keeps_it() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let mut r = Registry::default();
        r.touch(&id("aaaa1111", "alpha"), a.path(), "main");
        r.touch(&id("bbbb2222", "alpha"), b.path(), "main");
        assert_eq!(r.projects["bbbb2222"].name, "alpha-bbbb");
        r.touch(&id("bbbb2222", "alpha"), b.path(), "dev");
        assert_eq!(r.projects["bbbb2222"].name, "alpha-bbbb");
    }

    #[test]
    fn save_prunes_checkouts_whose_path_is_gone_and_projects_left_empty() {
        let keep = tempfile::tempdir().unwrap();
        let gone = tempfile::tempdir().unwrap();
        let gone_path = gone.path().to_path_buf();
        let file = tempfile::tempdir().unwrap().path().join("registry.json");
        let mut r = Registry::default();
        r.touch(&id("k1", "alpha"), keep.path(), "main");
        r.touch(&id("k1", "alpha"), &gone_path, "wt");
        r.touch(&id("k2", "beta"), &gone_path.join("sub"), "main");
        drop(gone);
        r.save_to(&file).unwrap();
        let back = Registry::load_from(&file).unwrap();
        assert_eq!(back.projects.len(), 1);
        assert_eq!(back.projects["k1"].checkouts.len(), 1);
        assert!(Registry::load_from(&file.with_file_name("absent.json")).unwrap().projects.is_empty());
    }

    fn three_checkouts() -> (Registry, tempfile::TempDir, tempfile::TempDir, tempfile::TempDir) {
        let main = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        let clone = tempfile::tempdir().unwrap();
        let mut r = Registry::default();
        r.touch(&Identity { key: "k".into(), name: "alpha".into(), main: false }, wt.path(), "fix");
        r.touch(&Identity { key: "k".into(), name: "alpha".into(), main: true }, main.path(), "main");
        r.touch(&Identity { key: "k".into(), name: "alpha".into(), main: false }, clone.path(), "main");
        (r, main, wt, clone)
    }

    #[test]
    fn resolve_prefers_the_checkout_containing_cwd_then_main() {
        let (r, main, wt, _clone) = three_checkouts();
        assert_eq!(r.resolve("alpha", &wt.path().join("deep/er")).unwrap(), wt.path());
        assert_eq!(r.resolve("alpha", Path::new("/nowhere")).unwrap(), main.path());
    }

    #[test]
    fn resolve_falls_back_to_the_most_recently_used_when_there_is_no_main() {
        let (mut r, _main, _wt, clone) = three_checkouts();
        for c in &mut r.projects.get_mut("k").unwrap().checkouts { c.main = false; }
        r.projects.get_mut("k").unwrap().checkouts.iter_mut().find(|c| c.path == clone.path().to_string_lossy()).unwrap().last_used = u64::MAX;
        assert_eq!(r.resolve("alpha", Path::new("/nowhere")).unwrap(), clone.path());
    }

    #[test]
    fn resolve_by_branch_and_the_errors_name_what_exists() {
        let (r, _main, wt, _clone) = three_checkouts();
        assert_eq!(r.resolve("alpha@fix", Path::new("/")).unwrap(), wt.path());
        let e = r.resolve("alpha@nope", Path::new("/")).unwrap_err().to_string();
        assert!(e.contains("fix") && e.contains("main"), "{e}");
        let e = r.resolve("zeta", Path::new("/")).unwrap_err().to_string();
        assert!(e.contains("alpha"), "{e}");
        assert!(Registry::default().resolve("zeta", Path::new("/")).unwrap_err().to_string().contains("repograph build"));
    }

    #[test]
    fn freshest_store_is_the_sibling_with_the_newest_manifest_never_self() {
        let (r, main, wt, clone) = three_checkouts();
        for (d, secs) in [(&main, 100u64), (&wt, 300), (&clone, 200)] {
            let dir = d.path().join(".repograph");
            std::fs::create_dir_all(&dir).unwrap();
            let f = dir.join("manifest.json");
            std::fs::write(&f, "{}").unwrap();
            std::fs::File::options().write(true).open(&f).unwrap()
                .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000 + secs)).unwrap();
        }
        assert_eq!(r.freshest_store("k", main.path()).unwrap(), wt.path());
        assert_eq!(r.freshest_store("k", wt.path()).unwrap(), clone.path());
        assert_eq!(r.freshest_store("nope", main.path()), None);
    }
}
```

Add `mod registry;` to `src/main.rs`.

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test registry::`
Expected: compile errors — nothing is defined.

- [ ] **Step 3: Implement**

Above the tests in `src/registry.rs`:

```rust
//! Which checkouts of which projects this machine has built a graph for — a cache of facts git
//! already knows. Every command that opens a store records the checkout it ran in; every write
//! drops checkouts whose path is gone. Nothing is registered by hand, so nothing rots.

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkout { pub path: String, pub branch: String, pub main: bool, pub last_used: u64 }

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project { pub name: String, pub checkouts: Vec<Checkout> }

/// Keyed by project: the sorted root commits of the history, which a clone, a linked worktree
/// and a moved directory share; a directory without history keys on its own path.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Registry { pub projects: BTreeMap<String, Project> }

/// What `build` learns from git once and caches in the store, so no later command spawns git.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity { pub key: String, pub name: String, pub main: bool }

pub const IDENTITY: &str = "identity.json";

/// A checkout is written back no more often than this: the row is a fact, not a heartbeat, and
/// an exact-id `ask` must not pay for a registry write.
const TOUCH_EVERY: u64 = 600;

pub fn path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("REPOGRAPH_REGISTRY") { return Ok(PathBuf::from(p)) }
    let home = std::env::var_os("HOME").context("neither REPOGRAPH_REGISTRY nor HOME is set")?;
    Ok(PathBuf::from(home).join(".config").join("repograph").join("registry.json"))
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn basename(p: &Path) -> String { p.file_name().and_then(|s| s.to_str()).unwrap_or("repo").to_string() }

fn git(repo: &Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git").arg("-C").arg(repo).args(args).output().ok()?;
    if !out.status.success() { return None }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Asks git — the one place that does.
pub fn detect(repo: &Path) -> Identity {
    let Some(roots) = git(repo, &["rev-list", "--max-parents=0", "HEAD"]).filter(|s| !s.is_empty()) else {
        return Identity { key: format!("path:{}", repo.display()), name: basename(repo), main: true };
    };
    let mut roots: Vec<&str> = roots.lines().collect();
    roots.sort_unstable();
    // `git@host:Owner/name.git`, `https://host/Owner/name`, a local path: the last segment.
    let name = git(repo, &["remote", "get-url", "origin"])
        .and_then(|u| u.trim_end_matches('/').trim_end_matches(".git").rsplit(['/', ':']).next().map(str::to_string))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| basename(repo));
    let main = git(repo, &["rev-parse", "--path-format=absolute", "--git-dir", "--git-common-dir"])
        .map(|s| { let mut l = s.lines(); l.next() == l.next() })
        .unwrap_or(true);
    Identity { key: roots.join("+"), name, main }
}

/// The branch from `.git/HEAD`, through a linked worktree's `gitdir:` pointer, without a
/// subprocess; a detached head shows as its first eight hex digits.
pub fn branch(repo: &Path) -> String {
    let dot = repo.join(".git");
    let dir = if dot.is_file() {
        let Ok(text) = std::fs::read_to_string(&dot) else { return String::new() };
        let Some(p) = text.trim().strip_prefix("gitdir:") else { return String::new() };
        let p = Path::new(p.trim());
        if p.is_absolute() { p.to_path_buf() } else { repo.join(p) }
    } else {
        dot
    };
    let Ok(head) = std::fs::read_to_string(dir.join("HEAD")) else { return String::new() };
    match head.trim().strip_prefix("ref: refs/heads/") {
        Some(b) => b.to_string(),
        None => head.trim().chars().take(8).collect(),
    }
}

impl Registry {
    pub fn load_from(p: &Path) -> Result<Registry> {
        match std::fs::read(p) {
            Ok(b) => serde_json::from_slice(&b).with_context(|| format!("parse {}", p.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Registry::default()),
            Err(e) => Err(e).with_context(|| format!("read {}", p.display())),
        }
    }

    /// Prunes, then writes through a temp file so a reader never sees half a registry.
    pub fn save_to(&mut self, p: &Path) -> Result<()> {
        self.prune();
        if let Some(dir) = p.parent() { std::fs::create_dir_all(dir)?; }
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, p).with_context(|| format!("write {}", p.display()))
    }

    pub fn load() -> Result<Registry> { Self::load_from(&path()?) }

    pub fn save(&mut self) -> Result<()> { self.save_to(&path()?) }

    pub fn prune(&mut self) {
        for p in self.projects.values_mut() { p.checkouts.retain(|c| Path::new(&c.path).is_dir()); }
        self.projects.retain(|_, p| !p.checkouts.is_empty());
    }

    fn name_for(&self, id: &Identity) -> String {
        if let Some(p) = self.projects.get(&id.key) { return p.name.clone() }
        if self.projects.values().any(|p| p.name == id.name) {
            format!("{}-{}", id.name, &id.key[..id.key.len().min(4)])
        } else {
            id.name.clone()
        }
    }

    /// Records that `repo` was used now. Returns whether the registry changed.
    pub fn touch(&mut self, id: &Identity, repo: &Path, branch: &str) -> bool {
        let path = repo.to_string_lossy().to_string();
        let name = self.name_for(id);
        let project = self.projects.entry(id.key.clone()).or_insert_with(|| Project { name, checkouts: Vec::new() });
        let t = now();
        match project.checkouts.iter_mut().find(|c| c.path == path) {
            Some(c) if c.branch == branch && t.saturating_sub(c.last_used) < TOUCH_EVERY => false,
            Some(c) => { c.branch = branch.to_string(); c.main = id.main; c.last_used = t; true }
            None => { project.checkouts.push(Checkout { path, branch: branch.to_string(), main: id.main, last_used: t }); true }
        }
    }

    /// `name` or `name@branch` to a checkout: the one containing `cwd`, else the main checkout,
    /// else the most recently used.
    pub fn resolve(&self, needle: &str, cwd: &Path) -> Result<PathBuf> {
        let (name, branch) = match needle.split_once('@') { Some((n, b)) => (n, Some(b)), None => (needle, None) };
        let Some(project) = self.projects.values().find(|p| p.name == name) else {
            let names: Vec<&str> = self.projects.values().map(|p| p.name.as_str()).collect();
            let known = if names.is_empty() { "none — run `repograph build` in one".to_string() } else { names.join(", ") };
            anyhow::bail!("no repository named {name}; registered: {known}");
        };
        let pick = match branch {
            Some(b) => project.checkouts.iter().find(|c| c.branch == b).with_context(|| {
                let branches: Vec<&str> = project.checkouts.iter().map(|c| c.branch.as_str()).collect();
                format!("{name} has no checkout on {b}; branches: {}", branches.join(", "))
            })?,
            None => project.checkouts.iter().find(|c| cwd.starts_with(&c.path))
                .or_else(|| project.checkouts.iter().find(|c| c.main))
                .or_else(|| project.checkouts.iter().max_by_key(|c| c.last_used))
                .context("registered project without checkouts")?,
        };
        Ok(PathBuf::from(&pick.path))
    }

    /// Another checkout of the same project whose store has the newest manifest — what a
    /// fresh checkout seeds from.
    pub fn freshest_store(&self, key: &str, except: &Path) -> Option<PathBuf> {
        self.projects.get(key)?.checkouts.iter()
            .map(|c| PathBuf::from(&c.path))
            .filter(|p| p != except)
            .filter_map(|p| std::fs::metadata(p.join(".repograph").join("manifest.json")).and_then(|m| m.modified()).ok().map(|t| (t, p)))
            .max_by_key(|(t, _)| *t)
            .map(|(_, p)| p)
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test registry:: && cargo clippy --all-targets --quiet`
Expected: 11 pass. If `identity_of_a_git_repo…` fails on the name: `tempfile` directories are named `.tmpXXXX`; the assertion compares against `file_name()`, which is the same string. If `a_linked_worktree…` fails on `main`: print the two lines `rev-parse` returned — on macOS the worktree path may come back under `/private/var` while the common dir is under `/var`; both come from git in one call, so they agree with each other.

- [ ] **Step 5: Commit**

```bash
git add src/registry.rs src/main.rs
git commit -m "feat(registry): a cache of git facts about every checkout, pruned on write" -m "A project is its root commits, so a clone, a worktree and a moved directory are one row; a checkout is recorded when used and dropped when its path is gone. Name resolution prefers the checkout around the working directory, then the main one, then the last used."
```

---

### Task 10: Identity in the store, use noted, worktrees seeded

**Files:**
- Modify: `src/store.rs` (after `wipe`)
- Modify: `src/main.rs:33-36` (`Build`, `Update` gain `--no-seed`), the `Build | Update` arm, `fn main` head, `graph_for`
- Test: `src/store.rs` tests module

**Interfaces:**
- Consumes: `registry::{Identity, IDENTITY, detect, branch, Registry}`.
- Produces: `Store::identity() -> Option<Identity>`, `Store::write_identity(&Identity) -> Result<()>`, `Store::seed_from(other: &Path) -> Result<Vec<&'static str>>`, `pub const SEEDED: [&str; 7]`; `fn note_use(repo: &Path)` in `main.rs`.

- [ ] **Step 1: Write the failing store tests**

In `src/store.rs` tests:

```rust
    #[test]
    fn identity_round_trips_and_is_absent_on_an_older_store() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::new(d.path());
        assert!(s.identity().is_none());
        let id = crate::registry::Identity { key: "k".into(), name: "n".into(), main: false };
        s.write_identity(&id).unwrap();
        assert_eq!(s.identity(), Some(id));
    }

    #[test]
    fn seed_from_copies_the_store_files_that_exist_and_names_them() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let src = Store::new(a.path());
        src.save(&graph("seed"), &crate::walk::Manifest::default()).unwrap();
        src.write_atomic("vectors.f32", &[1, 2, 3]).unwrap();
        let dst = Store::new(b.path());
        let copied = dst.seed_from(a.path()).unwrap();
        assert_eq!(copied, vec!["graph.json", "graph.bin", "manifest.json", "vectors.f32"]);
        assert_eq!(dst.load().unwrap().0.nodes.len(), 1);
        assert!(dst.has("vectors.f32") && !dst.has("questions.json"));
    }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test store::tests::identity_round_trips store::tests::seed_from_copies`
Expected: compile errors — no such methods.

- [ ] **Step 3: Implement in `src/store.rs`**

Above `impl Store`:

```rust
/// What a fresh checkout copies from a sibling's store before its first update: the graph, its
/// mirror, the manifest the diff runs against, the generated questions and the vectors. In this
/// order, so a partial copy is reported in the order a reader would look.
pub const SEEDED: [&str; 7] = ["graph.json", "graph.bin", "manifest.json", "questions.json", "questions.bin", "vectors.f32", "vectors.json"];
```

In `impl Store`, after `wipe`:

```rust
    pub fn identity(&self) -> Option<crate::registry::Identity> {
        self.read_bytes(crate::registry::IDENTITY).ok().flatten().and_then(|b| serde_json::from_slice(&b).ok())
    }

    pub fn write_identity(&self, id: &crate::registry::Identity) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        self.write_atomic(crate::registry::IDENTITY, &serde_json::to_vec(id)?)
    }

    /// Copies another checkout's store here. The manifest diff then re-reads only files whose
    /// hash differs and the dense index re-embeds only rows whose text changed, so a worktree a
    /// few commits away pays seconds, not the full embedding run.
    pub fn seed_from(&self, other: &Path) -> Result<Vec<&'static str>> {
        std::fs::create_dir_all(&self.dir)?;
        let mut copied = Vec::new();
        for name in SEEDED {
            let src = other.join(".repograph").join(name);
            if !src.exists() { continue }
            let tmp = self.dir.join(format!("{name}.tmp"));
            std::fs::copy(&src, &tmp).with_context(|| format!("copy {}", src.display()))?;
            std::fs::rename(&tmp, self.dir.join(name))?;
            copied.push(name);
        }
        Ok(copied)
    }
```

- [ ] **Step 4: Wire `main.rs`**

`enum Cmd`:

```rust
    /// Full build. A checkout of a project another checkout has already built starts from that
    /// store and re-reads only what differs; `--no-seed` starts from nothing
    Build { #[arg(long)] no_seed: bool },
    /// Re-extract only changed files; seeds like `build` when there is no store yet
    Update { #[arg(long)] no_seed: bool },
```

`fn main`, the `wipe` line and the arm:

```rust
    let wipe = matches!(cli.cmd, Cmd::Build { .. });
    match cli.cmd {
        Cmd::Build { no_seed } | Cmd::Update { no_seed } => {
            let cfg = load_cfg()?;
            let store = store::Store::new(&repo);
            if wipe { store.wipe()?; }
            let id = registry::detect(&repo);
            store.write_identity(&id)?;
            let mut reg = registry::Registry::load().unwrap_or_else(|e| { eprintln!("registry: {e:#}"); Default::default() });
            if !no_seed && !store.has("manifest.json") {
                if let Some(from) = reg.freshest_store(&id.key, &repo) {
                    if !store.seed_from(&from)?.is_empty() { eprintln!("seeded from {}", from.display()); }
                }
            }
            let r = run_update(&repo, &cfg, &extractors(&repo, &cfg)?, false)?;
            println!("changed {} removed {} nodes {} edges {}", r.changed, r.removed, r.nodes, r.edges);
            reg.touch(&id, &repo, &registry::branch(&repo));
            if let Err(e) = reg.save() { eprintln!("registry: {e:#}"); }
            embed_all(&repo, cli.no_dense)
        }
```

Above `fn main`:

```rust
/// Records this checkout in the registry, cheaply: no git, a write at most every ten minutes,
/// never an error — a registry problem is one line on stderr, not a failed answer. A store an
/// older release built has no identity yet and is skipped until its next `update`.
fn note_use(repo: &std::path::Path) {
    let Some(id) = store::Store::new(repo).identity() else { return };
    let mut reg = match registry::Registry::load() { Ok(r) => r, Err(e) => { eprintln!("registry: {e:#}"); return } };
    if reg.touch(&id, repo, &registry::branch(repo)) {
        if let Err(e) = reg.save() { eprintln!("registry: {e:#}"); }
    }
}
```

and in `fn main`, right after `repo` is known and before `match cli.cmd`:

```rust
    if !matches!(cli.cmd, Cmd::Build { .. } | Cmd::Update { .. } | Cmd::Bench { .. } | Cmd::Repos { .. }) { note_use(&repo); }
```

(`Cmd::Repos` arrives in Task 11; until then leave it out of the pattern.)

- [ ] **Step 5: Run everything, then measure the hot path**

Run: `cargo test --quiet && cargo clippy --all-targets --quiet && cargo build --release --quiet`

```bash
B=/Users/max/Documents/projects/beauty-crm
target/release/repograph --repo $B --no-dense update            # writes identity.json, registers
cat ~/.config/repograph/registry.json
REPOGRAPH_TIMING=1 target/release/repograph --repo $B ask FR-PAY-22 2>&1 | tail -3
```
Expected: the registry holds one project `beauty-crm` with one checkout on the current branch; the `ask` timing lines are within 1 ms of a 0.4.0 run (`printed` stage total).

- [ ] **Step 6: Seed a worktree and prove the cost**

```bash
B=/Users/max/Documents/projects/beauty-crm
git -C $B worktree add -q /tmp/bc-seed -b tmp/seed-probe
time target/release/repograph --repo /tmp/bc-seed build
target/release/repograph --repo /tmp/bc-seed verify | head -2
target/release/repograph repos 2>/dev/null || cat ~/.config/repograph/registry.json
git -C $B worktree remove --force /tmp/bc-seed && git -C $B branch -D tmp/seed-probe
```
Expected: stderr shows `seeded from /Users/max/Documents/projects/beauty-crm`; `changed 0 removed 0` (same tree) or the few files the branch differs by; `dense: embedded 0 rows`; wall time well under 10 s; `verify` counts equal the parent's. After removal the next `repograph` command's registry save drops the worktree row.

- [ ] **Step 7: Commit**

```bash
git add src/store.rs src/main.rs
git commit -m "feat(store): cache the checkout's identity and seed a fresh store from a sibling" -m "build and update ask git once and keep the answer in identity.json; every other command notes the checkout in the registry from that file, the branch from .git/HEAD, and writes at most every ten minutes. A checkout with no store copies the newest sibling store and lets the manifest diff and the vector sync pay only for the difference."
```

---

### Task 11: `--repo <name>` and `repos`

**Files:**
- Modify: `src/registry.rs` (Status, render, render_json + tests)
- Modify: `src/main.rs` (`fn main` head, `Repos` command, `probe`)
- Test: `src/registry.rs`

**Interfaces:**
- Produces: `pub struct Status { pub store: bool, pub nodes: usize, pub edges: usize, pub changed: usize, pub removed: usize }` (Default, Serialize); `Registry::render(&self, probe: &dyn Fn(&Path) -> Status) -> String`; `Registry::render_json(&self, probe) -> String`; CLI `repos [--json]`; `--repo <path|name|name@branch>`.

- [ ] **Step 1: Write the failing render tests**

In `src/registry.rs` tests:

```rust
    #[test]
    fn render_groups_checkouts_under_their_project_main_first_and_says_how_fresh() {
        let (r, main, wt, clone) = three_checkouts();
        let probe = |p: &Path| -> Status {
            if p == main.path() { Status { store: true, nodes: 8306, edges: 29610, changed: 0, removed: 0 } }
            else if p == wt.path() { Status { store: true, nodes: 8291, edges: 29540, changed: 3, removed: 1 } }
            else { Status::default() }
        };
        let out = r.render(&probe);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "alpha  k");
        assert_eq!(lines[1], format!("  {}  main  main  8306 nodes / 29610 edges  fresh", main.path().display()));
        assert!(lines[2..].contains(&format!("  {}  fix  8291 nodes / 29540 edges  stale: 3 changed, 1 removed", wt.path().display()).as_str()));
        assert!(lines[2..].contains(&format!("  {}  main  no store", clone.path().display()).as_str()));
        let v: serde_json::Value = serde_json::from_str(&r.render_json(&probe)).unwrap();
        assert_eq!(v["projects"]["k"]["checkouts"].as_array().unwrap().len(), 3);
        assert_eq!(v["projects"]["k"]["checkouts"][0]["status"]["nodes"], 8306);
    }

    #[test]
    fn render_of_an_empty_registry_says_so() {
        assert_eq!(Registry::default().render(&|_| Status::default()), "no repositories yet — run `repograph build` in one\n");
    }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test registry::tests::render`
Expected: compile errors — `Status`, `render` missing.

- [ ] **Step 3: Implement in `src/registry.rs`**

```rust
/// What `repos` learns about one checkout's store by opening it; `changed`/`removed` are the
/// same manifest diff `ask` runs before answering.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Status { pub store: bool, pub nodes: usize, pub edges: usize, pub changed: usize, pub removed: usize }

impl Status {
    fn word(&self) -> String {
        if !self.store { return "no store".into() }
        let counts = format!("{} nodes / {} edges", self.nodes, self.edges);
        if self.changed == 0 && self.removed == 0 { format!("{counts}  fresh") }
        else { format!("{counts}  stale: {} changed, {} removed", self.changed, self.removed) }
    }
}

impl Registry {
    fn ordered(&self) -> Vec<(&String, &Project, Vec<&Checkout>)> {
        let mut projects: Vec<(&String, &Project)> = self.projects.iter().collect();
        projects.sort_by(|a, b| a.1.name.cmp(&b.1.name));
        projects.into_iter().map(|(k, p)| {
            let mut cs: Vec<&Checkout> = p.checkouts.iter().collect();
            cs.sort_by(|a, b| b.main.cmp(&a.main).then(a.path.cmp(&b.path)));
            (k, p, cs)
        }).collect()
    }

    pub fn render(&self, probe: &dyn Fn(&Path) -> Status) -> String {
        if self.projects.is_empty() { return "no repositories yet — run `repograph build` in one\n".into() }
        let mut out = String::new();
        for (key, p, cs) in self.ordered() {
            out.push_str(&format!("{}  {}\n", p.name, &key[..key.len().min(8)]));
            for c in cs {
                let main = if c.main { "  main" } else { "" };
                out.push_str(&format!("  {}  {}{main}  {}\n", c.path, c.branch, probe(Path::new(&c.path)).word()));
            }
        }
        out
    }

    pub fn render_json(&self, probe: &dyn Fn(&Path) -> Status) -> String {
        let projects: serde_json::Map<String, serde_json::Value> = self.ordered().into_iter().map(|(key, p, cs)| {
            let checkouts: Vec<serde_json::Value> = cs.iter().map(|c| serde_json::json!({
                "path": c.path, "branch": c.branch, "main": c.main, "last_used": c.last_used, "status": probe(Path::new(&c.path)),
            })).collect();
            (key.clone(), serde_json::json!({ "name": p.name, "checkouts": checkouts }))
        }).collect();
        serde_json::json!({ "projects": projects }).to_string() + "\n"
    }
}
```

- [ ] **Step 4: Wire `main.rs`**

`enum Cmd`, after `Verify`:

```rust
    /// Every checkout this machine has built a graph for, grouped by project, with each store's
    /// counts and whether it is in step with its tree
    Repos { #[arg(long)] json: bool },
```

Above `fn main`:

```rust
/// One checkout's store, opened the way `ask` opens it; a broken config or an unreadable store
/// reads as "no store" rather than failing the whole listing.
fn probe(path: &std::path::Path) -> registry::Status {
    let store = store::Store::new(path);
    if !store.has("manifest.json") { return registry::Status::default() }
    let Ok((graph, manifest)) = store.load() else { return registry::Status::default() };
    let (changed, removed) = match config::Config::load(path).and_then(|cfg| walk::walk(path, &cfg, &manifest)) {
        Ok(entries) => { let d = manifest.diff(&entries); (d.changed.len(), d.removed.len()) }
        Err(_) => (0, 0),
    };
    registry::Status { store: true, nodes: graph.nodes.len(), edges: graph.edges.len(), changed, removed }
}
```

`fn main` head — replace `let repo = cli.repo.canonicalize()?;`:

```rust
    // A directory is a path; anything else is a registered name, `name` or `name@branch`.
    let repo = if cli.repo.is_dir() {
        cli.repo.canonicalize()?
    } else {
        registry::Registry::load()?.resolve(&cli.repo.to_string_lossy(), &std::env::current_dir()?)?.canonicalize()?
    };
```

The arm, after `Cmd::Verify`:

```rust
        Cmd::Repos { json } => {
            let mut reg = registry::Registry::load()?;
            reg.prune();
            print!("{}", if json { reg.render_json(&probe) } else { reg.render(&probe) });
            Ok(())
        }
```

Add `Cmd::Repos { .. }` to the `note_use` exclusion pattern from Task 10.

- [ ] **Step 5: Run and try it from elsewhere**

Run: `cargo test --quiet && cargo clippy --all-targets --quiet && cargo build --release --quiet`

```bash
cd /tmp
/Users/max/Documents/projects/repograph/target/release/repograph repos
/Users/max/Documents/projects/repograph/target/release/repograph --repo beauty-crm ask FR-PAY-22 | head -2
/Users/max/Documents/projects/repograph/target/release/repograph --repo beauty-crm@nope verify; echo "exit $?"
/Users/max/Documents/projects/repograph/target/release/repograph --repo nothing verify; echo "exit $?"
cd - >/dev/null
```
Expected: `repos` prints `beauty-crm  bc97b44e` and its checkout line ending `fresh`; the `ask` answers from `/tmp`; the two bad names exit 1 naming the branches / the names that exist.

- [ ] **Step 6: Commit**

```bash
git add src/registry.rs src/main.rs
git commit -m "feat(cli): address a checkout by name and list every one with its freshness" -m "--repo takes a registered name, or name@branch, from any directory. repos groups checkouts under their project, main first, with the store's counts and the same manifest diff ask runs; --json is what a hook reads."
```

---

### Task 12: README and release 0.5.0

**Files:**
- Modify: `README.md` (Use block; new section "Many repositories" before `## Configure`; status line 35)
- Modify: `Cargo.toml:3`, `Cargo.lock`, `npm/*/package.json`

- [ ] **Step 1: Document**

In **Use**, after the `changes` lines:

```bash
repograph repos                             # every checkout built on this machine, grouped by project, with freshness
repograph --repo beauty-crm ask FR-PAY-22   # a registered name works from any directory; name@branch picks a worktree
```

New section before `## Configure`:

````markdown
## Many repositories

Every `build` or `update` records the checkout it ran in — path, branch, whether it is the main
checkout — under the project's identity, the root commits of its history, in
`~/.config/repograph/registry.json` (`REPOGRAPH_REGISTRY` overrides the path). Nothing is
registered by hand: every command that opens a store refreshes the row, and every write drops
rows whose directory is gone. `repos` lists what is there:

```
$ repograph repos
beauty-crm  bc97b44e
  /Users/max/Documents/projects/beauty-crm  main  main  8306 nodes / 29610 edges  fresh
  /Users/max/Documents/projects/beauty-crm/.claude/worktrees/tasks-coordination  worktree-tasks-coordination  8291 nodes / 29540 edges  stale: 3 changed, 1 removed
```

`--repo <name>` picks the checkout around the current directory, else the main one, else the
most recently used; `--repo <name>@<branch>` picks a worktree by its branch.

A `build` in a checkout that has no store yet — a new worktree, a second clone — copies the
newest sibling store of the same project and then runs the ordinary update, so it re-reads only
the files whose hash differs and re-embeds only the rows whose text changed: seconds, not the
full embedding run. `--no-seed` builds from nothing. A subdirectory that is its own checkout
(a worktree in a visible directory, a vendored repository) is never walked by the parent.
````

Replace the example with the real `repos` output from Task 11 Step 5.

- [ ] **Step 2: Bump, prove, tag**

```bash
sed -i '' '3s/0\.4\.0/0.5.0/' Cargo.toml && cargo check --quiet
sed -i '' '35s/Version 0\.4\.0\./Version 0.5.0./' README.md
sed -i '' '3s/0\.4\.0/0.5.0/' npm/repograph-darwin-arm64/package.json npm/repograph-linux-x64/package.json npm/repograph/package.json
sed -i '' '32,33s/0\.4\.0/0.5.0/' npm/repograph/package.json
cargo test --quiet && cargo clippy --all-targets --quiet && cargo build --release --quiet
B=/Users/max/Documents/projects/beauty-crm
target/release/repograph --repo $B --no-dense build && target/release/repograph --repo $B bench && target/release/repograph --repo $B --no-dense bench
git add Cargo.toml Cargo.lock README.md npm/repograph/package.json npm/repograph-darwin-arm64/package.json npm/repograph-linux-x64/package.json
git commit -m "chore: release 0.5.0"
git tag -a v0.5.0 -m "repograph 0.5.0"
git push origin main && git push origin v0.5.0
```
Expected: floors met; the release workflow green under the `devmaxxx` token as in Task 6.

---

### Task 13: One notice hook for every indexed repository (host setup)

**Files (outside the repository):**
- Create: `~/.claude/hooks/repograph-notice.mjs` (from `beauty-crm/.claude/hooks/repograph-notice.mjs`)
- Modify: `~/.claude/settings.json` (`hooks.PreToolUse`)

- [ ] **Step 1: Copy the hook and make it silent where there is no store**

```bash
cp /Users/max/Documents/projects/beauty-crm/.claude/hooks/repograph-notice.mjs ~/.claude/hooks/repograph-notice.mjs
```

In the copy, replace the line `if (seen === 0) return built ? FULL_NOTICE : BUILD_NOTICE;` with:

```js
  // User-level: a directory without a store is simply not indexed, never nagged.
  if (!built) return null;
  if (seen === 0) return FULL_NOTICE;
```

and delete the now-unused `BUILD_NOTICE` constant. Check: `echo '{"tool_input":{"file_path":"/x/y.ts"},"cwd":"/tmp","session_id":"t"}' | node ~/.claude/hooks/repograph-notice.mjs; echo "exit $?"` → no output, exit 0.

- [ ] **Step 2: Register it beside the GitNexus hooks**

In `~/.claude/settings.json`, add to `hooks.PreToolUse` (keep the GitNexus entries until 0.6.0 covers the repositories they serve):

```json
{
  "matcher": "Read|Grep|Glob",
  "hooks": [
    { "type": "command", "command": "node '/Users/max/.claude/hooks/repograph-notice.mjs'", "timeout": 10 }
  ]
}
```

Validate: `python3 -c "import json;json.load(open('$HOME/.claude/settings.json'))" && echo ok`.

- [ ] **Step 3: See it fire once in a registered repository**

Start a Claude Code session in `beauty-crm`, read any `.ts` file: the full notice appears once (the project-level hook shares the per-session counter in the temp dir, so the two do not double up). In `/tmp`, reading a file shows nothing.

---

## Release 0.6.0 — languages (roadmap, separate spec)

Not planned here; brainstormed and specified on its own when 0.5.0 is out. What is known:

- **Corpora:** `bonliva-crm-nx` — C# ~700 files (`packages/crm-api-client-dotnet`, `apps/bonliva-crm-timereport-api`, `libs-dotnet/*`); `beauty-crm/mobile/shared` — Kotlin 122 files; `bonliva-erp` — a handful of Python files. Order: C#, Kotlin, Python.
- **Shape:** `FileKind::Code` dispatched by extension to a per-language `symbols`/`calls`/`imports` trio; `idrefs` is grammar-agnostic (it walks `comment` and `string` nodes) and is reused as is; `impact`, `changes`, `repos` need nothing. Grammars: `tree-sitter-c-sharp`, `tree-sitter-kotlin`, `tree-sitter-python`, pinned.
- **Config:** `id_families` are `beauty-crm`'s census; the Bonliva repositories get a `repograph.toml` with `ERP`, `CRMDEV`, `BON`. No code.
- **Then, and only then:** the two GitNexus hooks and the `gitnexus` MCP server leave `~/.claude/settings.json` and `~/.claude.json`, and `~/.gitnexus/` is deleted.

---

## Self-review

**Spec coverage (0.4.0).** Call edges → Task 2; spans → Task 1; aliases/canonical/upstream/downstream/trace/risk → Task 3; `impact`/`trace` CLI + README → Task 4; `changes` → Task 5; release → Task 6; `beauty-crm` rules, routes, hook, ignore lines, escape-hatch paragraphs, dependency bump → Task 7. The user-level GitNexus config is documented as out of scope in the spec, not silently skipped. Acceptance items: `verify` `Calls` count (Task 2 Step 8, Task 6 Step 2); `impact StaffService` (Task 4 Step 3); `changes` on an edited method and on a clean tree (Task 5 Step 6); bench floors (Tasks 2, 6); clippy (every task); no `gitnexus` in `beauty-crm` (Task 7 Step 7).

**Placeholders.** None: every step has its code, command and expected output. The one open value is the corpus `Calls` count `N`, which the executor records from `verify` in the Task 2 commit body.

**Type consistency.** `Extraction::node_span(kind, id, label, body, file, line, end)` is defined in Task 1 and used in Tasks 1 and 5. `impact::{Dependent, Impact, aliases, canonical, upstream, downstream, trace, risk, files, render, render_json}` are defined in Task 3 with the signatures Tasks 4 and 5 call. `changes::{Hunk, parse, touched, Report, report, hunks_from_git, render, render_json}` are defined and used in Task 5. `query::resolve` becomes `pub(crate)` in Task 3 and is called from `main.rs` in Task 4. `graph_for(repo, cfg, stale)` is defined in Task 4 and reused in Task 5. `SymbolScanner::resolver()` is added in Task 2 Step 4 before Step 6 uses it.

**Spec coverage (0.5.0, `2026-09-03-registry-design.md`).** Nested checkouts → Task 8; project key, name, checkouts, `TOUCH_EVERY`, prune-on-write, `name`/`name@branch` resolution, freshest sibling → Task 9; identity cached in the store, no git on the hot path, seeding with `--no-seed`, `seeded from` line → Task 10; `--repo <name>`, `repos`/`--json` with live counts and freshness → Task 11; README and tag → Task 12; the one hook → Task 13. Acceptance items: name from `/tmp` and `name@branch` (Task 11 Step 5); seeded worktree under 10 s with equal counts and the row dropped after removal (Task 10 Step 6); hot path within 1 ms (Task 10 Step 5); floors (Task 12 Step 2). Out of scope stays out: no import, no `watch --all`, no MCP.

**Type consistency (0.5.0).** `registry::{Identity, IDENTITY, Checkout, Project, Registry, Status, detect, branch, path}` are defined in Task 9 (Status in Task 11) with the signatures Tasks 10 and 11 call; `Store::{identity, write_identity, seed_from}` and `SEEDED` are defined in Task 10 and used there; `note_use` and `probe` live in `main.rs`; `Cmd::Build { no_seed }` / `Cmd::Update { no_seed }` replace the unit variants in Task 10, and the `wipe` match is updated with them; `Cmd::Repos` is added in Task 11 and only then joins the `note_use` exclusion pattern.
