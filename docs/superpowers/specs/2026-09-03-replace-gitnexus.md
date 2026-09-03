# Replacing GitNexus with repograph — gap analysis and design

Source: the 2026-09-03 three-graph measurement of `beauty-crm` @ `aba308b1` (82 recorded cases,
`bench/cases.jsonl`; artifact "Три графа beauty-crm"), the GitNexus rules `beauty-crm` ran under
until `d04b19a4`, and the current repograph 0.3.0 graph model.

## What the measurement said

| | repograph 0.3.0 | gitnexus 1.6.9 |
|---|---|---|
| by requirement id | 61/82 (67 after enrich, 79–81 with `--rerank`) | 12/82 — **0 of 70** requirement ids |
| by file | 75/82 | 54/82 |
| code symbols | 12/12 in 74 ms, 118 chars | 12/12 in 1393 ms, 8126 chars |
| median answer | 594 chars · 347 ms | 6008 chars · 1422 ms |
| build | 103 s, 0 tokens, 31 MB | 233 s, 0 tokens, 598 MB |

The artifact's verdict kept GitNexus "beside" repograph for one reason: *blast radius and call
traces*. It indexes code, not the PRD, and its `impact`/`detect_changes` were what
`beauty-crm`'s agent rules leaned on before every edit and every commit.

## What GitNexus actually did in beauty-crm

From the generated `AGENTS.md` block and the six `gitnexus-*` skills (both removed in
`d04b19a4`, 2026-09-03 11:59), the rules an agent had to follow:

| Rule | GitNexus tool | What it needs from a graph |
|---|---|---|
| MUST run impact before editing a symbol; report callers, risk | `impact({target, direction: "upstream"})` | callers by depth, count, files, risk level |
| MUST run `detect_changes()` before committing; `scope: compare, base_ref: main` for regressions | `detect_changes` | git hunks → symbols → their upstream callers |
| MUST warn on HIGH/CRITICAL | risk thresholds | a documented threshold table |
| "Who calls it? callees? flows?" | `context({name})` | in/out edges of one symbol |
| "How does A reach B?" | `trace({from, to})` | shortest call path |
| find execution flows by concept | `query` | covered: `repograph ask` scores 40/40 keywords, 12/12 symbols |
| rename across the call graph | `rename` | an edit tool, not a graph question |
| taint findings | `explain` with `--pdg` | never used in this repository |
| clusters / processes resources | community detection, flow mining | never used by a rule; only the two MUSTs were enforced |

State today: GitNexus is switched off in `beauty-crm` (index deleted, registry row dropped,
skills and `AGENTS.md` block removed). What remains is the escape hatch — "bring it back when a
call graph or a blast radius is the question: `gitnexus analyze -f --embeddings --skip-agents-md`
(233 s)" — in `.claude/CLAUDE.md`, the `repo-query` skill, two ignore-file lines, and the
user-level MCP server + two hooks in `~/.claude/settings.json` that serve ten other repositories.

## What repograph already has

`verify` on `beauty-crm`: 8306 nodes (4322 `Symbol`, 908 `File`), 29 610 edges — `Declares`
7293, `Imports` 1330 (file → file, imported names in `context`), `ReExports` 247, `Extends` 56,
`DecoratedBy` 340. Every symbol has `path:line`. `explain <Symbol>` prints its edges;
`resolve` matches a bare name to `sym:<file>::<Name>`.

Missing, and why the escape hatch exists:

1. **`EdgeKind::Calls` is declared and never emitted.** No extractor walks call sites, so
   `explain StaffService` shows its members and decorator and nothing that uses it.
2. **No symbol spans.** `Node.line` only; a git hunk cannot be mapped to the symbol it sits in.
3. **No traversal beyond one hop.** `query::EXPAND` walks id edges for `ask`; nothing walks
   `Calls`/`Extends` upstream or downstream, nothing follows a barrel to the real declaration.
4. **No git-diff entry point.**

## Design

### Call edges (`src/code/calls.rs`)

One walk per file, the same shape as `idrefs::scan`. A call site is `call_expression`
(`function` field) or `new_expression` (`constructor` field). Its source is
`idrefs::owner` — the top-level function, class member or `const` that contains it, else the
file. Its target is resolved from a per-file scope built once:

| Callee shape (tree-sitter, verified 2026-09-03) | Target |
|---|---|
| `identifier` `Name` imported from `f` or declared top-level here | `sym:f::Name` |
| `new` `identifier` | same |
| `member_expression (this) prop` inside class `C` | `sym:<this file>::C.prop` |
| `member_expression (identifier Name) prop`, `Name` imported/local | `sym:f::Name.prop` |
| same, `Name` is `import * as Name` | `sym:f::prop` |
| `member_expression (member_expression (this) field) prop` where `C` declares `field: T` as a typed field or a constructor parameter property, `T` imported from `f` or local | `sym:f::T.prop` |
| anything else (chained calls, `super`, globals, parameters, `console`) | no edge |

Names that are neither imported nor declared top-level yield nothing: the graph records what
the file can prove, not what a name suggests. A target may be dangling — a call through a
barrel points at `sym:<barrel>::Name`; that is resolved at query time, not build time, so a
barrel edit never forces a re-read of every caller.

The NestJS pattern `constructor(private readonly service: StaffService)` +
`this.service.create(dto)` is the one that matters for `beauty-crm`'s API: without the
field-type rule the graph would see almost no service calls.

Out of scope, deliberately: JSX element usage as a call, type-position references, `implements`
clauses, calls through chained expressions or destructured methods. Each is one more arm in
`target()` and one inline case when a question needs it.

### Symbol spans

`Node.end: u32` (`#[serde(default)]`; 0 for nodes without a span). Set for every top-level
declaration and class member from `end_position()`. Old JSON loads with `end = 0`; the postcard
mirror carries the writing version in its header, so a 0.3.0 mirror is skipped by 0.4.0 and
rewritten on the next read.

### Traversal (`src/impact.rs`)

- `aliases(graph, id)`: every `sym:<barrel>::<Name>` a caller could have used, by walking
  `ReExports` edges backwards from the declaring file (context `*` or naming the symbol).
- `canonical(graph, id)`: the reverse — a dangling `sym:<barrel>::<Name>` walked forward to the
  node that exists.
- `upstream(graph, root, depth)`: seeds = root ∪ its `Declares` members (a class is changed
  through its methods) ∪ their aliases; BFS over `Calls` and `Extends` edges by target; layers
  by depth, each dependent once at its shallowest depth. Plus `importers`: files whose `Imports`
  edge names the symbol — one hop, no BFS, the "who imports it" answer.
- `downstream(graph, root, depth)`: BFS by source, targets canonicalised.
- `trace(graph, from, to, depth)`: BFS forward from `from`'s seeds until `to` or one of its
  aliases/members; the path, or none.
- `risk(direct, total, files)`: `CRITICAL` ≥30 direct or ≥25 files; `HIGH` ≥15 or ≥10;
  `MEDIUM` ≥5 or ≥3; else `LOW`. Fixed numbers, printed with the counts that produced them, so
  a reader can disagree with the label and still use the list.

### Git entry point (`src/changes.rs`)

`git -C <repo> diff -U0 --no-color --no-ext-diff <base> -- .` (default base `HEAD`, so staged
and unstaged both count; `--base main` compares the whole branch and the working tree) plus
`git ls-files --others --exclude-standard` for new files. Hunk new-side ranges map to symbols
whose span meets them; a hunk outside every symbol maps to the file node; a class whose member
matched is dropped in favour of the member. Each touched symbol's `upstream` is unioned into one
report with one risk line. The graph is refreshed first, exactly as `ask` does, so the lines
are the working tree's.

### CLI

```
repograph impact <symbol> [--depth 3] [--down] [--json] [--stale]
repograph trace <from> <to> [--depth 6] [--stale]
repograph changes [--base HEAD] [--depth 2] [--json] [--stale]
```

`explain` gains `Calls` lines for free — that alone answers "who calls it" at depth 1.

### What is dropped, and what replaces it

| GitNexus | Replacement |
|---|---|
| `impact` | `repograph impact` |
| `detect_changes` | `repograph changes` |
| `context` | `repograph explain` (now with `Calls`) + `impact`/`impact --down` |
| `trace` | `repograph trace` |
| `query` | `repograph ask` |
| `rename` | `repograph impact <old>` for the file list, then the editor's rename or `ast-grep`; `repograph changes` to confirm the scope |
| clusters, processes, `cypher`, `pdg_query`, taint | not replaced — no rule in `beauty-crm` used them; the measured question set never asks for them |
| PreToolUse hook injecting `augment` hits (up to 7 s per `Grep`) | already replaced by `repograph-notice.mjs` (a text notice, no query) |

## Scope

**In:** repograph 0.4.0 (the four items above, README, release); `beauty-crm` wiring —
`@devmaxxx/repograph` 0.4.0, `.claude/CLAUDE.md` rules (the two MUSTs move from GitNexus to
`impact`/`changes`), `repo-query` routes, the notice hook text, the two ignore lines, the
escape-hatch paragraphs.

**Out, decision for Max:** the user-level `~/.claude/settings.json` GitNexus hooks, the
`gitnexus` MCP server in `~/.claude.json`, and `~/.gitnexus/registry.json` — all serve ten
`bonliva-*` repositories repograph does not index (not TypeScript-plus-markdown corpora on the
same conventions). They stay until those repositories move; in `beauty-crm` both handlers exit
on the missing `.gitnexus/`.

## Acceptance

- `verify` on `beauty-crm` reports a `Calls` count > 0 and every other count unchanged from
  0.3.0 (`Symbol` 4322, `Declares` 7293, `Imports` 1330, `ReExports` 247, `Extends` 56,
  `DecoratedBy` 340, `References` 18448).
- `repograph impact StaffService` lists `StaffController.create` at d=1 with `path:line` and
  `staff.module.ts` among importers.
- `repograph changes` on a working tree with one edited method names that method and its
  callers; on a clean tree prints `changed: 0 symbols`.
- `bench` floors unchanged: 40/40 · 14/30 dense (11/30 no-dense) · 12/12 · p90 ≤ 230.
- `cargo test`, `cargo clippy --all-targets` clean.
- `beauty-crm` contains no `gitnexus` outside git history.
