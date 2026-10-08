# MCP server

```bash
repograph mcp                    # a stdio MCP server (protocol 2025-06-18) over the current repository
claude mcp add repograph -- repograph mcp   # what `install-agent --claude` runs, once
```

`install-agent --claude` registers the server through the `claude` CLI, leaves one that is already
registered alone, and prints the command to run by hand where `claude` is not on `PATH`. The
protocol is newline-delimited JSON-RPC on stdout and nothing else; diagnostics go to stderr. A tool
that fails returns an `isError` result and the server carries on.

| tool | does |
| --- | --- |
| `ask` | `question` (plus `seeds`, `bodies`, `rerank`, `rerank_local`, `depth`, `stale`) to the answer `ask` prints, with the low-confidence line when the retrievers disagree |
| `explain`, `impact`, `trace`, `changes` | the commands of those names, with their flags; an answer given while files are behind the tree says so |
| `status` | nodes and edges, enrich coverage, files behind the tree, the store's model against the configured one, vector rows owed, and whether a background refresh is running with its last progress line from `.repograph/background.log` |
| `reindex` | starts a detached `update` and returns at once; says so when one is already running |
| `switch_model` | `model <hub id>` in the background. Only catalogued models and ones trusted on this machine with `repograph model <id>`; any other id is refused, because the caller is an agent and a model is code this machine downloads |

`status` reports progress while the writer lock is held and nothing once the refresh is over. Each
read tool loads the store per call, or asks a running `serve` for `ask`.
