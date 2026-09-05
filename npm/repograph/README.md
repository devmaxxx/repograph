# @devmaxxx/repograph

One binary that builds a knowledge graph of a repository's markdown requirement docs and
TypeScript code at zero model tokens, keeps it fresh while you work, and answers questions about
it by exact id, keyword or paraphrase.

It is published to GitHub Packages, which serves no anonymous reads, so once per machine
`~/.npmrc` needs a scope and a classic personal access token carrying `read:packages`:

```
@devmaxxx:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=<token>
```

```bash
npm install --save-dev @devmaxxx/repograph     # or: pnpm add -D @devmaxxx/repograph
npx repograph build                             # once per clone, ~1 s per 800 files
npx repograph ask отмена записи                 # refreshes the graph first if files changed
npx repograph explain FR-PAY-22
```

This package is a launcher: the binary itself comes from a per-platform package pulled in as an
optional dependency (`@devmaxxx/repograph-darwin-arm64`, `@devmaxxx/repograph-linux-x64`). On any
other platform the launcher explains how to build from source with `cargo install`.

The dense retriever downloads a 2.1 GB embedding model into `~/.cache/repograph/fastembed` on its
first fused query; `--no-dense` never touches it, and `embed_model =
"intfloat/multilingual-e5-small"`
in `repograph.toml` swaps it for a 470 MB one that reads paraphrase 15/30 against the default's
22/30.

Full documentation, the measured comparison against graphify and GitNexus, and the bench live in
the repository: https://github.com/devmaxxx/repograph
