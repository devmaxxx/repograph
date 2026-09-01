# repograph

A project knowledge graph that costs **zero API tokens** to build, keep fresh, and query.

`repograph` reads a repository's markdown and TypeScript, extracts the structure the authors already
wrote by hand — requirement ids, cross-references, invariants, milestones, exports, imports,
decorators, and the ids quoted inside code comments — and answers questions about it in a few lines
of text.

It is built around one claim: **the hard part is not building a better graph, it is building a better
door onto it.** A repository's requirement ids already form a dense, human-authored graph. What was
missing was a way in that finds the right entry point from a question phrased in ordinary words.

## Status

Version 0.1, under active development. What works today:

| Command                    | State                           |
| -------------------------- | ------------------------------- |
| `build`, `update`          | working                         |
| `ask`, `explain`, `verify` | working, lexical retrieval only |
| `bench`                    | not implemented yet             |
| `import-legacy`            | not implemented yet             |

Local embeddings are planned but **not wired yet**: `ask` currently runs the exact-id and BM25 stages
only, so `--no-dense` is presently the only behaviour. The `bench` harness that measures recall and
answer cost against recorded cases is likewise still to come.

## Install

```bash
cargo install --path .
```

Requires Rust 1.98 (pinned in `rust-toolchain.toml`).

## Use

Build the graph once, then keep it fresh incrementally:

```bash
repograph --repo /path/to/project build     # full build
repograph --repo /path/to/project update    # re-extract only changed files
```

`--repo` defaults to the current directory. State lives in `<repo>/.repograph/`; add it to
`.gitignore`.

Ask it something:

```bash
repograph ask cancellation policy
repograph ask FR-PAY-22                     # an exact id short-circuits straight to the node
repograph ask asGrosze                      # so does an exact symbol name
repograph ask --bodies отмена записи        # print full requirement bodies, not just headlines
repograph ask --json отмена записи          # machine-readable
repograph ask --seeds 8 отмена записи       # widen the search; costs more tokens
```

Answers are lines of the form:

```
ID  path:line  headline
  ID  path:line  headline  ← seed-id
```

Top-level lines are the seeds the query matched. An indented line is a neighbour reached by one hop
over the id graph, and the `←` names the seed it came from.

Inspect one node and everything attached to it:

```bash
repograph explain FR-PAY-22
repograph explain asGrosze
```

`explain` resolves its argument as an exact id, then as a symbol name, then as a case-insensitive
label match.

Check the graph's health — counts by kind, ids that are referenced but never declared, dangling
edges:

```bash
repograph verify
```

## How a question becomes an answer

1. **Exact.** A word that is a known id, or the name of an indexed symbol, wins outright and scores
   above everything else.
2. **Lexical.** BM25 over `id + label + body` for every node, with Snowball stemming — Russian for
   Cyrillic tokens, English otherwise, so `штрафа` and `штрафы` are the same term. Ids survive
   tokenization whole, so `FR-PAY-22` never becomes three tokens.
3. **Fuse.** Reciprocal rank fusion (k = 60) merges the retrievers; the top seeds survive.
4. **Expand.** One hop over `References`, `Implements`, `Declares`, `Links` and `Legacy` edges, in
   both directions. `File` nodes and decorator nodes are never expanded _to_ — they are hubs and
   would drown the answer.
5. **Render.** `ID  path:line  headline`, headline cut to 80 characters.

The lexical index is rebuilt in memory on every `ask` rather than stored on disk. It costs about
120 ms on a 7,500-node graph, and in exchange there is no lexical state that can ever go stale
relative to the graph.

## What ends up in the graph

**Node kinds:** `Requirement`, `Entity`, `Invariant`, `Adr`, `Milestone`, `Task`, `File`, `Symbol`,
`LegacyConcept`.

**Edge kinds:** `References`, `Declares`, `Links`, `Implements`, `Imports`, `ReExports`, `Calls`,
`Extends`, `DecoratedBy`, `Legacy`.

An edge is unique on `(source, target, kind, context, file)` — `file` is part of the key on purpose,
so a relationship that two different files both assert is recorded twice and survives either one
being edited.

From documents it takes requirement blocks in both `**ID · MUST · title**` and `### ID · MUST · title`
forms, ids referenced in prose (including ranges like `FR-RPT-42…48` and slash lists like
`INV-11/12/20`), backticked entity names, markdown links, and `constitution.yaml`-shaped registries.

From TypeScript it takes a `Symbol` per top-level export, class, method and decorated member;
imports resolved through relative paths, `tsconfig` `paths` and `package.json` `exports`; decorators
with their first string argument as context; and every id quoted in a comment or string literal,
attributed to its enclosing symbol. That last layer is the doc↔code bridge an AST-only indexer
misses entirely.

## Configure

`repograph.toml` at the repository root. Every key is optional; the defaults are the ones this
repository ships:

```toml
doc_globs  = ["**/*.md"]
code_globs = ["**/*.ts", "**/*.tsx"]
skip       = ["**/node_modules/**", "**/dist/**"]
registries = ["docs/constitution.yaml"]

# Id prefixes to recognise. Deliberately strict — ambiguous one-letter families
# collide with ordinary prose and are left out.
id_families       = ["FR-PAY", "INV", "ADR"]
milestone_families = ["BE", "FE", "PLAT"]
```

A repository with no config still gets code extraction plus markdown headings.

A requirement line is recognised as `<ID> · MUST|SHOULD|LATER · <title>`, in either the bold or the
heading form; the modality is optional.

## Measured

On its development corpus — a 1,300-file TypeScript monorepo with a Russian-language PRD:

|                                 |                                                                                                                              |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Nodes                           | 7,515 — 3,631 `Symbol`, 1,880 `Requirement`, 1,001 `Task`, 824 `File`, 72 `Entity`, 69 `Milestone`, 20 `Invariant`, 18 `Adr` |
| Edges                           | 27,085                                                                                                                       |
| Graph on disk                   | 10.2 MB JSON                                                                                                                 |
| Graph load                      | ~19 ms                                                                                                                       |
| Lexical index rebuild           | ~120 ms                                                                                                                      |
| Tokens spent building any of it | 0                                                                                                                            |

The prior art on the same corpus was an LLM-extracted graph that cost **14.6 million input tokens
over 13 runs** and, measured on the same questions, answered 0 of 14 paraphrase queries at ~1,555
tokens per answer and 11 of 24 keyword queries at ~1,027. Cost is not the only reason to replace it,
but it is the easiest one to state.

**Targets, not yet results.** The `bench` harness is unbuilt, so treat these as the floors v0.1 is
aiming at rather than claims it has met: 24/24 keyword, ≥7/14 paraphrase without embeddings, ≥12/14
with them, and a 90th-percentile answer cost at or under 130 tokens.

## Design

The full design note and the implementation plan are in
[`docs/superpowers/specs/2026-09-01-repograph-design.md`](docs/superpowers/specs/2026-09-01-repograph-design.md)
and [`docs/superpowers/plans/`](docs/superpowers/plans/), including the measurements that chose this
architecture over the alternatives and the deviations taken during implementation.

## License

MIT.
