<!-- repograph:begin -->

## repograph

This repository carries a `repograph` index of its documents and its TypeScript. It answers by
meaning, at no model tokens, and it is faster than reading files to find out where something is.

- `{{command}} ask <words>` — what the docs and the code say about a concept. Seeds first, then
  one-hop neighbours indented under the seed they came from.
- `{{command}} impact <Symbol> --depth 1` — who calls it, in how many files, and a risk word.
- `{{command}} changes --base main` — your working diff mapped onto symbols and their callers.
- `{{command}} trace <A> <B>` — the call chain between two symbols, or that there is none.
- `{{command}} explain <id>` — one node, where it is written, and its neighbours.

Every one takes `--json`. `ask` and the rest bring the index in line with the tree themselves, so
freshness is not your problem; `--stale` skips that when you want the answer now.

**Ask the graph before you grep for a concept, and read only the `path:line` it prints.** Grep is
still right for a literal — an env var, a string in a test, a config key. Two stages cost model
tokens and are opt-in: `enrich` writes the questions once, `ask --rerank` picks seeds from a deep
pool per question. Everything above is free.
<!-- repograph:end -->
