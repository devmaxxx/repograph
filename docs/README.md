# repograph documentation

| page | what is in it |
| --- | --- |
| [install](install.md) | the GitHub Packages registry line, npm and cargo, the platform packages, and the release workflow |
| [commands](commands.md) | every command with examples, `--json` shapes, exit codes, `explain`, `verify`, `import-legacy`, `bench` |
| [keeping fresh](keeping-fresh.md) | the self-healing `ask`, `reader_budget`, detached refreshes, `watch`, git hooks |
| [serve](serve.md) | the resident process, its idles, socket path and measured latency |
| [MCP server](mcp.md) | `repograph mcp`, the tools it exposes, `install-agent --claude` |
| [how it works](how-it-works.md) | how a question becomes an answer, what the graph records, the design notes |
| [languages](languages.md) | what each grammar reads and does not read |
| [configuration](configuration.md) | `repograph.toml`, the machine file, environment variables, id families, `resources` |
| [embeddings](embeddings.md) | the default model, the cache, choosing and switching models |
| [enrich and rerank](enrich-and-rerank.md) | the two opt-in stages that spend model tokens |
| [benchmarks](benchmarks.md) | `bench`, the floors, exit codes, measured numbers |
| [the graph model](graph-model.md) | node and edge kinds, what the extractors take, what `impact` and `changes` can and cannot prove |
| [the measurements](history.md) | every reading that decided a default |
| [running on Windows](windows.md) | the platform floor, SmartScreen, Defender, PowerShell |
| [`bench/`](bench/) | the campaign documents, the runbook, and the open gap ledger |
| [`adr/`](adr/) | the decisions that outlived their arguments |
