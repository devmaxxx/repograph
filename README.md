# repograph

A project knowledge graph that costs **zero API tokens** to build, keep fresh, and query.

`repograph` reads a repository's markdown and TypeScript, extracts the structure the authors already
wrote by hand — requirement ids, cross-references, invariants, milestones, exports, imports,
decorators, and the ids quoted inside code comments — and answers questions about it in a few lines
of text.

## What it is

It is built around one claim: **the hard part is not building a better graph, it is building a better
door onto it.** A repository's requirement ids already form a dense, human-authored graph. What was
missing was a way in that finds the right entry point from a question phrased in ordinary words.

Measured on the same corpus and the same 38 keyword/paraphrase questions against `graphify`, the
LLM-extracted graph it replaces:

|                           | graphify (the incumbent) | repograph           |
| ------------------------- | ------------------------ | ------------------- |
| paraphrase questions      | 0/14                     | 5/14                |
| keyword questions         | 11/24                    | 24/24               |
| tokens per answer         | 1027-1555                | 202 median, 218 p90 |
| tokens to build the graph | 14,597,195               | 0                   |

Every number above came from running both tools; none is a target. See [Bench](#bench) for the full
floor set and how it was recorded, and
[`docs/adr/0001-paraphrase-recall-was-a-prediction.md`](docs/adr/0001-paraphrase-recall-was-a-prediction.md)
for the one number in this project's history that travelled from a design note into a plan as though
it had been measured, and hadn't been.

## Status

Version 0.1.0. Every row below is implemented, not planned:

| Command                    | State                                                                 |
| -------------------------- | --------------------------------------------------------------------- |
| `build`, `update`          | working; incremental; a no-op `update` is a fixed point               |
| `ask`, `explain`, `verify` | working: exact id/symbol → BM25 → dense, fused, one hop out           |
| `bench`                    | working; fails the process if a floor in [Bench](#bench) is missed    |
| `import-legacy`            | working; costs recall at query time — see its note in [Bench](#bench) |

`--no-dense` skips the embedding stage everywhere it could apply — `build`, `update`, `ask`, `bench`.
Without it, those commands use local embeddings once the model is cached (see [Embeddings](#embeddings)).

## Install

```bash
cargo install --path .
```

Requires Rust 1.98 (pinned in `rust-toolchain.toml`).

Tagged releases (`v*`) also publish prebuilt binaries for `aarch64-apple-darwin` and
`x86_64-unknown-linux-gnu` as GitHub release assets, built by `.github/workflows/release.yml`.

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
repograph ask --seeds 8 отмена записи       # widen the search beyond the default of 5; costs more tokens
repograph ask --no-dense отмена записи      # lexical only, no embedding query
```

Answers are lines of the form:

```
ID  path:line  headline
  ID  path:line  headline  ← seed-id
```

Top-level lines are the seeds the query matched. An indented line is a neighbour reached by one hop
over the id graph, and the `←` names the seed it came from. A real run, against `beauty-crm`:

```
$ repograph --repo beauty-crm ask cancellation
sym:packages/db/src/schema/salon/scheduling.ts::CANCELLATION_CONSEQUENCES  packages/db/src/schema/salon/scheduling.ts:110  CANCELLATION_CONSEQUENCES
BE-M10/T05  docs/prd-2026-08-16/plans/milestones/backend/BE-M10-payments-provider-stripe-connect-implementation-deposits-car.md:80  Cancellation policy engine (`packages/domain/policy.ts`): inputs policy + appoin…
PLAT-M12/T09  docs/prd-2026-08-16/plans/milestones/platform/PLAT-M12-billing.md:48  Cancel semantics N-141: immediate stop of future charges, accrued usage as dated…
PLAT-M12/T04  docs/prd-2026-08-16/plans/milestones/platform/PLAT-M12-billing.md:43  Panel subscription page: plan, next invoice, cancel with end-of-access date, exp…
BE-M10/T07  docs/prd-2026-08-16/plans/milestones/backend/BE-M10-payments-provider-stripe-connect-implementation-deposits-car.md:82  EOD fee charger worker: `chargeOffSession` for due fees → intent(purpose=cancell…
  BE-M10  docs/prd-2026-08-16/plans/milestones/backend/BE-M10-payments-provider-stripe-connect-implementation-deposits-car.md:1  BE-M10 Payments provider: Stripe Connect implementation, deposits, card-on-file …  ← BE-M10/T05
```

Inspect one node and everything attached to it:

```bash
repograph explain <id|symbol|label>         # tries an exact id, then a symbol name, then a label
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

Fold in a graphify graph's model-only edges as a frozen legacy layer:

```bash
repograph import-legacy path/to/graphify/graph.json
```

This prints how many of the source graph's edges resolved to real nodes on both ends, on one end, or
needed a new `LegacyConcept`. It is opt-in and lossy by construction — see its coverage note under
[Bench](#bench) before running it on a store you plan to benchmark.

Run the recorded benchmark:

```bash
repograph bench                      # bench/cases.jsonl: 24 keyword + 14 paraphrase + 3 code
repograph bench --cases other.jsonl  # a different case file, same 24/14/3 shape
```

## How a question becomes an answer

1. **Exact.** A word that is a known id, or the name of an indexed symbol, wins outright and scores
   above everything else.
2. **Lexical.** BM25 over `id + label + body` for every node, with Snowball stemming — Russian for
   Cyrillic tokens, English otherwise, so `штрафа` and `штрафы` are the same term. Ids survive
   tokenization whole, so `FR-PAY-22` never becomes three tokens.
3. **Dense.** A local embedding of the query, cosine-ranked against every node's stored vector
   (see [Embeddings](#embeddings)). Skipped by `--no-dense` or when no model is cached.
4. **Fuse.** Reciprocal rank fusion (k = 60) merges the retrievers; the top seeds survive.
5. **Expand.** One hop over `References`, `Implements`, `Declares`, `Links` and `Legacy` edges, in
   both directions. `File` nodes and decorator nodes are never expanded _to_ — they are hubs and
   would drown the answer.
6. **Render.** `ID  path:line  headline`, headline cut to 80 characters.

The lexical index is rebuilt in memory on every `ask` rather than stored on disk. It costs about
120 ms on a 7,500-node graph, and in exchange there is no lexical state that can ever go stale
relative to the graph.

## What ends up in the graph

**Node kinds:**

| Kind            | What becomes one                                                              |
| --------------- | ----------------------------------------------------------------------------- |
| `Requirement`   | a `<ID> · MUST\|SHOULD\|LATER · title` line, in either dialect                |
| `Entity`        | a backticked name inside a requirement's title                                |
| `Invariant`     | an `INV-*` requirement, or a row in a `constitution.yaml`-shaped registry     |
| `Adr`           | an `ADR-*` requirement, or the whole document of a file named after an ADR id |
| `Milestone`     | a `<PREFIX>-M##` requirement, or the whole document of a milestone file       |
| `Task`          | a `- [ ] **T##** …` checklist line inside a milestone file                    |
| `File`          | one per indexed file; owns ids that occur outside any block                   |
| `Symbol`        | a top-level export, class, method, or decorated class member                  |
| `LegacyConcept` | an `import-legacy` node that resolution could not tie to a real node          |

**Edge kinds:**

| Kind          | What creates one                                                                                                            |
| ------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `References`  | an id or backticked entity in prose, a title, a body, a registry row's `basis`, or an id quoted in a code comment or string |
| `Declares`    | a file, milestone, or registry row that owns a node                                                                         |
| `Links`       | a markdown link between two files                                                                                           |
| `Implements`  | a task or registry row and the requirement or gate its text names                                                           |
| `Imports`     | a resolved TypeScript import                                                                                                |
| `ReExports`   | a barrel `export * from`                                                                                                    |
| `Extends`     | a class's `extends` clause                                                                                                  |
| `DecoratedBy` | a decorator application, its first string argument as context                                                               |
| `Legacy`      | an edge carried over by `import-legacy`                                                                                     |
| `Calls`       | declared in the model, but no extractor emits it yet — reserved, not measured                                               |

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

`repograph.toml` at the repository root — this file also doubles as the worked example, set to its
own defaults. Every key is optional; a repository with no `repograph.toml` gets `Config::default()`
in full, not an empty config:

| Key                  | Default                                                                                     |
| -------------------- | ------------------------------------------------------------------------------------------- |
| `doc_globs`          | `["**/*.md"]`                                                                               |
| `code_globs`         | `["**/*.ts", "**/*.tsx"]`                                                                   |
| `skip`               | `["**/node_modules/**", "**/dist/**", "**/TRACKER.md", "graphify-out/**", ".repograph/**"]` |
| `registries`         | `["docs/constitution.yaml"]`                                                                |
| `id_families`        | see below — 47 strict families                                                              |
| `milestone_families` | `["BE", "FE", "PLAT", "SYNC", "OPS", "AI"]`                                                 |

`id_families` and `milestone_families` default to the strict list `beauty-crm`'s census settled on —
they are this project's development corpus, not a generic default. A one-letter or short family
(`B1`, `C11`, `S3`) collides with ordinary prose and is deliberately left out; a different repository
should replace both lists with its own families:

```toml
id_families = [
  "FR-DM", "FR-CAL", "FR-VIS", "FR-PAY", "FR-PH", "FR-SEC", "FR-APP", "FR-MKT",
  "FR-AI", "FR-CRM", "FR-SHELL", "FR-TOOL", "FR-SVC", "FR-LIFE", "FR-WH", "FR-RPT",
  "FR-MIG", "FR-WEB", "FR-OPS", "FR-STAFF",
  "NFR-PH", "NFR-MKT", "NFR-MIG", "NFR-PAY", "NFR-DM", "NFR-RPT", "NFR-WEB", "NFR-SVC", "NFR-STAFF", "NFR",
  "AC-DM", "AC-VIS", "INV", "ADR", "OD", "OQ", "N", "R", "M", "W", "D", "G",
  "PREP", "CAL", "OR", "MON", "SEAM", "SG", "IDEA",
]
```

A requirement line is recognised as `<ID> · MUST|SHOULD|LATER · <title>`, in either the bold or the
heading form; the modality is optional.

## Embeddings

Dense retrieval embeds with `fastembed`'s `MultilingualE5Small` (`intfloat/multilingual-e5-small`,
384-d, ONNX, ≈450 MB) — a one-time download cached under `FASTEMBED_CACHE_DIR` (or fastembed's own
default cache directory if that is unset). Every command that touches the dense stage — `build`,
`update`, `ask`, `bench` — reuses that cache; there are no further network calls after the first one.
`--no-dense` skips the download and the embedding stage everywhere.

If the model can't be opened (no cache, no network), the two kinds of caller degrade differently, on
purpose: `ask` and `update` fall back to lexical-only and print one line to stderr saying so, then
exit 0 — a person reading the answer can judge a lexical-only one for what it is. `bench` in dense
mode instead fails the run outright (exit 1): its only output is an exit code, and a silent fallback
graded against the weaker no-dense floor would report green without having measured what it claims to
measure.

On `beauty-crm`'s 6,691 non-`File` nodes, the first embedding pass took ~135 s on an M3 Pro; a second
`update` with nothing changed embeds 0 — only nodes whose passage hash changed are re-embedded.

## Bench

`repograph bench [--cases file]` runs the recorded 41 cases (24 keyword + 14 paraphrase + 3 code,
`bench/cases.jsonl` by default) against a built graph and fails the process if any floor is missed:

- keyword 24/24
- paraphrase ≥5/14 with embeddings, ≥2/14 with `--no-dense`
- code 3/3
- p90 ≤230 tokens, counted as rendered UTF-8 bytes / 4 — a conservative proxy, since it counts a
  Cyrillic answer at roughly double what an equivalent chars/4 reading would give a Latin one

Measured, on the shipped binary against `beauty-crm`: `keyword 24/24  paraphrase 5/14  code 3/3
p90 218 tok` (median 202) with embeddings; `keyword 24/24  paraphrase 2/14  code 3/3  p90 216 tok`
with `--no-dense`.

The design note that shaped this architecture predicted paraphrase recall would reach ≥12/14 once
dense retrieval was fused in. It measured at 5/14 — a prediction that did not survive contact with
measurement, not a bug; see
[`docs/adr/0001-paraphrase-recall-was-a-prediction.md`](docs/adr/0001-paraphrase-recall-was-a-prediction.md)
for what was ruled out and what wasn't. The floors above are that measurement, and the tool still
beats the incumbent on every axis anyone has ever measured: 5/14 and 24/24 at 202 median tokens
against graphify's 0/14 and 11/24 at 1,027-1,555 tokens, built for 14.6 million tokens instead of
zero.

**`import-legacy`'s coverage note.** Folding in a graphify graph costs recall and cost at query time,
not just disk: importing `beauty-crm`'s graphify graph measured keyword dropping 24/24 → 23/24 and
dense p90 rising 218 → 233 tokens, which breaches the p90 floor above. That is why `bench` above is
always measured against a legacy-free store, and why `import-legacy` stays a separate, opt-in step
rather than folding into `build`.

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

See [Bench](#bench) for the retrieval-quality floors these numbers are held to, and the ADR for the
one figure that didn't hold up on first measurement.

## Design

The full design note and implementation plan are in
[`docs/superpowers/specs/2026-09-01-repograph-design.md`](docs/superpowers/specs/2026-09-01-repograph-design.md)
and [`docs/superpowers/plans/2026-09-01-repograph.md`](docs/superpowers/plans/2026-09-01-repograph.md).

The plan's deviations table lists four simplifications against the spec, none of which change the
node/edge model or the answer shape: `serde_json` with an atomic rename instead of `rkyv`; a
~120-line hand-rolled BM25 over `rust-stemmers` instead of `tantivy`; adjacency lists instead of
`petgraph`; and a line scanner (one regex for a requirement head, `#` lines as block boundaries, one
regex for links) instead of `tree-sitter-md`. One of those four _did_ move a measured number: the
`tantivy` swap cost paraphrase recall, not just code size — see
[`docs/adr/0001-paraphrase-recall-was-a-prediction.md`](docs/adr/0001-paraphrase-recall-was-a-prediction.md).

## License

MIT.
