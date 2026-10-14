# @devmaxxx/zerograph

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
npm install --save-dev @devmaxxx/zerograph     # or: pnpm add -D @devmaxxx/zerograph
npx zerograph build                             # once per clone, ~1 s per 800 files
npx zerograph ask отмена записи                 # refreshes the graph first if files changed
npx zerograph explain FR-PAY-22
```

This package is a launcher: the binary itself comes from a per-platform package pulled in as an
optional dependency (`@devmaxxx/zerograph-darwin-arm64`, `@devmaxxx/zerograph-linux-x64`,
`@devmaxxx/zerograph-win32-x64`). On any other platform the launcher explains how to build from
source with `cargo install`.

The dense retriever downloads a 470 MB embedding model into `~/.cache/zerograph/fastembed`
(`%USERPROFILE%\.cache\zerograph\fastembed` on Windows) on its first fused query; `--no-dense`
never touches it, and `embed_model = "intfloat/multilingual-e5-large"` in `zerograph.toml` swaps it
for a 2.1 GB one that reads paraphrase 22/30 against the default's 15/30.

This package was `@devmaxxx/repograph` until 0.6.0. It still installs a `repograph` command beside
`zerograph`, and reads a `.repograph/` store, `repograph.toml`, `REPOGRAPH_*` variables and the old
model cache where they are; the first write moves the store to `.zerograph/`. The rules are in
`docs/install.md`, under "Coming from repograph".

Full documentation, the measured comparison against graphify and GitNexus, and the bench live in
the repository: https://github.com/devmaxxx/repograph
