# zerograph

One binary that builds a knowledge graph of a repository's markdown and code in 24 languages at
zero model tokens, keeps it fresh while you work, and answers questions about
it by exact id, keyword or paraphrase.

```bash
npm install --save-dev zerograph     # or: pnpm add -D zerograph
npx zerograph build                   # once per clone, ~1 s per 800 files
npx zerograph ask отмена записи       # refreshes the graph first if files changed
npx zerograph explain FR-PAY-22
```

This package is a launcher: the binary itself comes from a per-platform package pulled in as an
optional dependency (`zerograph-darwin-arm64`, `zerograph-linux-x64`, `zerograph-win32-x64`). On any other platform the launcher explains how to build from
source with `cargo install`.

The dense retriever downloads its embedding model, `embeddinggemma-300m` (about 1.2 GB in the
cache), into `~/.cache/zerograph/fastembed` (`%USERPROFILE%\.cache\zerograph\fastembed` on
Windows) on its first fused query; `--no-dense` never touches it. On the recorded 82 cases it reads
keyword 40/40, paraphrase 19/30 and code 12/12, against 39/40, 7/30 and 12/12 with `--no-dense`.
`embed_model` in `zerograph.toml` picks another model; the measured ones are in `docs/embeddings.md`.

This package was `@devmaxxx/repograph` until 0.6.0. It installs only the `zerograph` command, and
reads a `.repograph/` store, `repograph.toml`, `REPOGRAPH_*` variables and the old model cache
where they are; the first write moves the store to `.zerograph/`. Remove the old package
(`pnpm remove @devmaxxx/repograph`, or `npm uninstall -g @devmaxxx/repograph` if it was installed
globally). The rules are in `docs/install.md`, under "Coming from repograph".

Full documentation, the measured comparison against graphify and GitNexus, and the bench live in
the repository: https://github.com/devmaxxx/zerograph
