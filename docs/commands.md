# Commands

## Status

0.6.0 is the version `main` carries. Its default globs read TypeScript and JavaScript, Kotlin, Java,
C#, Rust, Python, Dart, Swift, GraphQL, SQL, Bicep, HCL, Shell and Vue; every command below is
implemented rather than planned:
`build` (a full re-read that replaces the stored graph only when it saves, so one interrupted
leaves the previous store answering) and `update` (incremental; a no-op `update` is a fixed point), `families`, `ask`, `explain`,
`verify`, `impact`, `trace`, `changes`, `embed`, `watch`, `serve`, `mcp`, `prime`, `install-agent`,
`import-legacy`, `dump` and `bench`. Three spend model tokens and all three are opt-in: `enrich`,
`ask --rerank`, and `ask --rerank-local` (zero tokens, a local cross-encoder, measured and rejected
as a floor candidate). `bench` fails the process when a floor in [Bench](benchmarks.md#bench) is missed; floors
are keyed by enrichment and by the store's embedder, and a store under any other model is measured
and not graded.

`--no-dense` skips the embedding stage everywhere it could apply — `build`, `update`, `enrich`,
`embed`, `watch`, `serve`, `ask`, `bench`, `dump`. Without it, those commands use local embeddings
once the model is cached (see [Embeddings](embeddings.md)).

## Use

Build the graph once, then keep it fresh incrementally:

```bash
repograph --repo /path/to/project build     # full build
repograph --repo /path/to/project update    # re-extract only changed files
```

`--repo` defaults to the current directory. State lives in `<repo>/.repograph/`; add it to
`.gitignore`.

After a first build, read which id families the documents defined and what was left as text:

```bash
repograph families                          # every family, its nodes, and the line that defines it
repograph families --json                   # the same numbers for a script
```

Ask it something:

```bash
repograph ask cancellation policy
repograph ask FR-PAY-22                     # an exact id short-circuits straight to the node
repograph ask asGrosze                      # so does an exact symbol name (a plain lowercase word does so only alone)
repograph ask "FR-PAY-22 refund"            # quoted or not, the same words find the same ids and names
repograph ask --bodies отмена записи        # print full requirement bodies, not just headlines
repograph ask --json отмена записи          # machine-readable
repograph ask --seeds 8 отмена записи       # widen the search beyond the default of 5; costs more tokens
repograph ask --no-dense отмена записи      # lexical only, no embedding query
```

Ask it what depends on a symbol, what a symbol reaches, and how one reaches another:

```bash
repograph impact StaffService               # callers by depth, importing files, a risk line
repograph impact --down StaffController     # what it calls, through injected services and barrels
repograph impact --json --depth 1 asGrosze  # machine-readable; depth 1 is the "will break" list alone
repograph trace StaffController StaffService  # shortest chain of calls between two symbols
repograph changes                           # what the uncommitted diff touches, and who reaches it
repograph changes --base main --depth 1     # the whole branch; depth 1 is the direct callers alone
```

`impact` and `trace` resolve a name the way `explain` does, except that when a document node and
code share it they take the code, production code before a test (a `test/` or `e2e/` path, a
`*.test.*`, `*.spec.*` or `*.stories.*` file), and name on stderr three of the candidates they
passed over and how many more there are.

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

### Answering a program instead of a person

Every reader takes `--json`, and every one of them answers with an object rather than a bare array,
so a field can be added without breaking a parser written against the version before it:

| Command | Top-level keys |
| --- | --- |
| `ask --json` | `seeds`, `expanded` |
| `impact --json` | `root`, `at`, `direction`, `risk`, `direct`, `total`, `files`, `importers`, `layers` |
| `changes --json` | `risk`, `touched`, `affected`, `files` |
| `families --json` | `families`, `milestones`, `mention_only` |
| `explain --json` | `id`, `kind`, `label`, `file`, `line`, `community`, `edges` |
| `verify --json` | `nodes`, `edges`, `nodes_by_kind`, `edges_by_kind`, `dangling`, `undeclared`, `gaps`, `cite_only`, `held_aside`, `held_aside_prefixes` |
| `trace --json` | `from`, `to`, `depth`, `path` |
| `prime --json` | `nodes`, `edges`, `enriched`, `questions`, `families`, `model` |
| `model --json` | `store`, `configured`, `configured_from`, `this_run`, `agrees`, `recommended` |

`explain --json` resolves each edge's direction for you — `dir` is `in` or `out` and `other` is the
node at the far end — so a caller never works out which end of an edge it was standing on. `file`
and `line` say where the edge is written, as the text form's trailing `path:line` does, so two
edges from one id that two documents declare read as two: the line is that of the declaration the
edge sits in, and `null` when no node declared in that file is an end of it. Each
row of `impact` and `changes`, and each step of a `trace` path, carries `passes`: `true` when the
edge is an identifier handed to a call rather than called, which the text forms print as `Passes`.
`kind` stays the graph's own edge kind, `Calls`, so a reader filtering on it keeps those rows. One
difference from the text forms is deliberate: a `trace` that finds no path within the depth is an
answer to the question that was asked, so the JSON form prints `"path": null` and exits 0 where the
text form exits **3**. The two are the same answer in two shapes: a caller parsing an object should
not have to read an exit code to learn what the object already says, and a shell script reading the
text form should not have to tell that answer from the `1` an unknown symbol exits with.

Inspect one node and everything attached to it:

```bash
repograph explain <id|symbol|label>         # tries an exact id, then a symbol name, then a label
repograph explain FR-PAY-22
repograph explain asGrosze
```

`explain` resolves its argument as an id — as typed, then without the punctuation around it, then
ignoring case, as `ask` reads an id in a question — then as a symbol name, then as a
case-insensitive label match. Like every other reader it brings the store in line with the tree first; `--stale`
answers from the store as it stands.

Check the graph's health — counts by kind, dangling edges, how many citations are held aside because
no line defines their prefix, and ids that are referenced but never declared, split into gaps inside
a declared family (worth chasing) and shapes the dialect does not read as ids at all:

```bash
repograph verify
```

Fold in a graphify graph's model-only edges as a frozen legacy layer:

```bash
repograph import-legacy path/to/graphify/graph.json
```

This prints how many of the source graph's edges resolved to real nodes on both ends, on one end, or
needed a new `LegacyConcept`. It is opt-in and lossy by construction — see its coverage note under
[Bench](benchmarks.md#bench) before running it on a store you plan to benchmark.

Run the recorded benchmark:

```bash
repograph bench                      # bench/cases.jsonl: 40 keyword + 30 paraphrase + 12 code
repograph bench --cases other.jsonl  # any shape: the 40/30/12 shape is graded, any other is measured ungated
repograph dump --queries qs.jsonl --out lists.json   # every retriever's ranked list per question, 300 deep
```
