<div align="center">

# zerograph

**A project knowledge graph that costs zero API tokens to build, keep fresh, and query.**

[![npm](https://img.shields.io/npm/v/zerograph?logo=npm)](https://www.npmjs.com/package/zerograph)
[![CI](https://github.com/devmaxxx/zerograph/actions/workflows/ci.yml/badge.svg)](https://github.com/devmaxxx/zerograph/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-macOS%20arm64%20%7C%20Linux%20x64%20%7C%20Windows%20x64-lightgrey)

[Get started](#get-started) · [See it in action](#see-it-in-action) · [Features](#features) ·
[Benchmarks](#benchmarks) · [How it works](#how-it-works) · [Docs](docs/README.md)

</div>

`zerograph` reads a repository's markdown and code in 24 languages, extracts the structure the authors
already wrote by hand (requirement ids, cross-references, exports, imports, calls, decorators, ids
quoted in comments) and answers questions about it in a few lines of text.

## Why zerograph

- **Zero tokens by default.** Build, refresh and query run locally. `ask --rerank` can spend model
  tokens and is opt-in.
- **A better door, not a better graph.** Exact id, BM25 and local dense retrieval find the entry
  point from a question in ordinary words, in about 200 tokens an answer.
- **Built for agents.** `--json` on every reader, an MCP server, and hooks that keep the graph fresh.

Measured against `graphify`, the LLM-extracted graph it replaces:

| | zerograph | graphify |
| --- | --- | --- |
| Paraphrase questions | **7/14** | 0/14 |
| Keyword questions | **24/24** | 11/24 |
| Tokens per answer (median) | **197** | 1,027-1,555 |
| Tokens to build | **0** | 14.6 M |

Details: [the head-to-head](docs/history.md#the-head-to-head-against-the-graph-it-replaces).

## Get started

```bash
pnpm add -D zerograph                      # prebuilt: macOS arm64, Linux x64, Windows x64
pnpm add -g zerograph                      # or globally with pnpm
npm install -g zerograph                   # or globally with npm
cargo install --path .                     # or from source

zerograph build                            # full build into .zerograph/ (add it to .gitignore)
zerograph ask cancellation policy          # answer in a few lines
```

- Windows needs a few extra steps: [running on Windows](docs/windows.md).
- Upgrading from repograph: the old store, config and variables are read where they are, see
  [coming from repograph](docs/install.md#coming-from-repograph).

## See it in action

```
$ zerograph ask cancellation
sym:packages/db/src/schema/salon/scheduling.ts::CANCELLATION_CONSEQUENCES  packages/db/src/schema/salon/scheduling.ts:110  CANCELLATION_CONSEQUENCES
BE-M10/T05  docs/.../BE-M10-payments-provider-stripe-connect-….md:80  Cancellation policy engine (`packages/domain/policy.ts`): inputs policy + appoin…
  BE-M10  docs/.../BE-M10-payments-provider-stripe-connect-….md:1  BE-M10 Payments provider: Stripe Connect implementation…  ← BE-M10/T05
```

Each line is `ID  path:line  headline`. Top-level lines are the seeds the question matched; an indented
line is a neighbour one hop away, and `←` names the seed it came from.

```bash
zerograph ask FR-PAY-22                    # an exact id or symbol goes straight to the node
zerograph impact StaffService              # callers by depth, importing files, a risk line
zerograph trace StaffController StaffService   # shortest call chain between two symbols
zerograph changes --base main              # what the diff touches, and who reaches it
zerograph explain FR-PAY-22                # one node and everything attached to it
```

## Features

| | |
| --- | --- |
| Build and refresh | `build`, `update`, `watch`, and a self-healing `ask` that re-reads what changed ([keeping fresh](docs/keeping-fresh.md)) |
| Ask | `ask`, `explain`, `impact`, `trace`, `changes`, `verify`, `families`, all with `--json` ([commands](docs/commands.md)) |
| Resident process | `serve` answers a fused question in 66 ms instead of 326 ms ([serve](docs/serve.md)) |
| Agents | `zerograph mcp`, `install-agent`, `prime` ([MCP](docs/mcp.md)) |
| Languages | TypeScript and JavaScript, Kotlin, Java, C#, Rust, Python, Dart, Swift, GraphQL, SQL, Bicep, HCL, Shell, Vue, Lua, Go, C, C++, PHP, Ruby, Scala, Elixir, OCaml ([languages](docs/languages.md)) |
| Optional spend | `ask --rerank`, a model picking seeds from a 200-deep pool per question ([rerank](docs/rerank.md)) |
| Bench | `zerograph bench` fails the process when a recall floor is missed ([benchmarks](docs/benchmarks.md)) |

## Benchmarks

On the recorded 82 cases with the default embedder, at 244 p90 tokens an answer:

| Suite | Cases | Default | `--no-dense` |
| --- | --- | --- | --- |
| Keyword | 40 | 40/40 | 39/40 |
| Paraphrase | 30 | 19/30 | 7/30 |
| Code | 12 | 12/12 | 12/12 |

The graph itself costs nothing to build. Floors, exit codes and the full tables:
[benchmarks](docs/benchmarks.md).

## Configuration

`zerograph.toml` at the repository root, plus an optional machine file at
`~/.config/zerograph/config.toml`. Every key is optional. The globs, `include`, `text_globs`, model
and `resources` keys, and the `ZEROGRAPH_*` environment variables are in
[configuration](docs/configuration.md); embedder choice is in [embeddings](docs/embeddings.md).

## How it works

Exact id or symbol first, then BM25 over passages, then a dense list,
interleaved rank by rank; the surviving seeds expand one hop over the id graph. The graph records only
what a file proves, so a call through a chained expression or a callback has no edge: pair a "nothing
uses this" with `rg -l`. See [how it works](docs/how-it-works.md) and [the graph model](docs/graph-model.md).

## Documentation

All reference material lives in [docs/](docs/README.md):

- **Use it:** [install](docs/install.md), [commands](docs/commands.md),
  [keeping fresh](docs/keeping-fresh.md), [serve](docs/serve.md), [MCP](docs/mcp.md),
  [configuration](docs/configuration.md), [running on Windows](docs/windows.md)
- **Understand it:** [how it works](docs/how-it-works.md), [the graph model](docs/graph-model.md),
  [languages](docs/languages.md), [embeddings](docs/embeddings.md), [rerank](docs/rerank.md)
- **Measure it:** [benchmarks](docs/benchmarks.md), [the measurements](docs/history.md),
  [bench campaigns](docs/bench/), [decisions](docs/adr/)

## Contributing

Issues and pull requests are welcome at
[devmaxxx/zerograph](https://github.com/devmaxxx/zerograph/issues). Before a pull request, run
`cargo test` and `cargo clippy --all-targets -- -D warnings`; a change to retrieval is judged by the
[three-way rule](docs/bench/runbook.md#judging-a-retrieval-change-the-three-way-rule).

## License

[MIT](LICENSE).
