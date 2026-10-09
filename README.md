# repograph

A project knowledge graph that costs **zero API tokens** to build, keep fresh, and query.

`repograph` reads a repository's markdown and code in 15 languages, extracts the structure the authors
already wrote by hand (requirement ids, cross-references, exports, imports, calls, decorators, ids
quoted in comments) and answers questions about it in a few lines of text.

- **Zero tokens by default.** Build, refresh and query run locally. `ask --rerank` can spend model tokens and is opt-in.
- **A better door, not a better graph.** Exact id, BM25 and local dense retrieval find the entry point from a question in ordinary words, in about 200 tokens an answer.
- **Built for agents.** `--json` on every reader, an MCP server, and hooks that keep the graph fresh.

Measured against `graphify`, the LLM-extracted graph it replaces: 7/14 paraphrase and 24/24 keyword at
197 median tokens and zero build cost, against 0/14 and 11/24 at 1,027-1,555 tokens and 14.6 M tokens to
build ([the head-to-head](docs/history.md#the-head-to-head-against-the-graph-it-replaces)).

## Get started

```bash
# ~/.npmrc, once per machine (a classic PAT with read:packages): see docs/install.md
pnpm add -D @devmaxxx/repograph            # prebuilt: macOS arm64, Linux x64, Windows x64
cargo install --path .                     # or from source

repograph build                            # full build into .repograph/ (add it to .gitignore)
repograph ask cancellation policy          # answer in a few lines
```

Windows needs a few extra steps: [running on Windows](docs/windows.md).

## See it in action

```
$ repograph ask cancellation
sym:packages/db/src/schema/salon/scheduling.ts::CANCELLATION_CONSEQUENCES  packages/db/src/schema/salon/scheduling.ts:110  CANCELLATION_CONSEQUENCES
BE-M10/T05  docs/.../BE-M10-payments-provider-stripe-connect-….md:80  Cancellation policy engine (`packages/domain/policy.ts`): inputs policy + appoin…
  BE-M10  docs/.../BE-M10-payments-provider-stripe-connect-….md:1  BE-M10 Payments provider: Stripe Connect implementation…  ← BE-M10/T05
```

Each line is `ID  path:line  headline`. Top-level lines are the seeds the question matched; an indented
line is a neighbour one hop away, and `←` names the seed it came from.

```bash
repograph ask FR-PAY-22                    # an exact id or symbol goes straight to the node
repograph impact StaffService              # callers by depth, importing files, a risk line
repograph trace StaffController StaffService   # shortest call chain between two symbols
repograph changes --base main              # what the diff touches, and who reaches it
repograph explain FR-PAY-22                # one node and everything attached to it
```

## What it does

| | |
| --- | --- |
| Build and refresh | `build`, `update`, `watch`, and a self-healing `ask` that re-reads what changed ([keeping fresh](docs/keeping-fresh.md)) |
| Ask | `ask`, `explain`, `impact`, `trace`, `changes`, `verify`, `families`, all with `--json` ([commands](docs/commands.md)) |
| Resident process | `serve` answers a fused question in 66 ms instead of 326 ms ([serve](docs/serve.md)) |
| Agents | `repograph mcp`, `install-agent`, `prime` ([MCP](docs/mcp.md)) |
| Languages | TypeScript and JavaScript, Kotlin, Java, C#, Rust, Python, Dart, Swift, GraphQL, SQL, Bicep, HCL, Shell, Vue ([languages](docs/languages.md)) |
| Optional spend | `ask --rerank`, a model picking seeds from a 200-deep pool per question ([rerank](docs/rerank.md)) |
| Bench | `repograph bench` fails the process when a recall floor is missed ([benchmarks](docs/benchmarks.md)) |

## Benchmarks

On the recorded 82 cases (40 keyword, 30 paraphrase, 12 code) with the default embedder: keyword 40/40,
paraphrase 19/30, code 12/12 at 244 p90 tokens; with `--no-dense` 39/40, 7/30, 12/12. The graph itself costs nothing to build. Floors, exit codes and the full tables:
[benchmarks](docs/benchmarks.md).

## Configuration

`repograph.toml` at the repository root, plus an optional machine file at
`~/.config/repograph/config.toml`. Every key is optional. The globs, `include`, `text_globs`, model
and `resources` keys, and the `REPOGRAPH_*` environment variables are in
[configuration](docs/configuration.md); embedder choice is in [embeddings](docs/embeddings.md).

## How it works

Exact id or symbol first, then BM25 over passages, then a dense list,
interleaved rank by rank; the surviving seeds expand one hop over the id graph. The graph records only
what a file proves, so a call through a chained expression or a callback has no edge: pair a "nothing
uses this" with `rg -l`. See [how it works](docs/how-it-works.md) and [the graph model](docs/graph-model.md).

## Documentation

All reference material lives in [docs/](docs/README.md):
[install](docs/install.md), [commands](docs/commands.md), [keeping fresh](docs/keeping-fresh.md),
[serve](docs/serve.md), [MCP](docs/mcp.md), [how it works](docs/how-it-works.md),
[languages](docs/languages.md), [configuration](docs/configuration.md), [embeddings](docs/embeddings.md),
[rerank](docs/rerank.md), [benchmarks](docs/benchmarks.md),
[the graph model](docs/graph-model.md), [the measurements](docs/history.md),
[running on Windows](docs/windows.md), [bench campaigns](docs/bench/), [decisions](docs/adr/).

## License

MIT.
