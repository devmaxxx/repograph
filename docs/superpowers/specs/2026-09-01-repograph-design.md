# repograph — a standalone Rust tool for tracking a project's knowledge graph

New repository at `/Users/max/Documents/projects/repograph`. `beauty-crm` is its first consumer
and its benchmark corpus, referenced by path — never vendored, never modified by this plan.

## Context

graphify's semantic layer in `beauty-crm` cost **14,597,195 input tokens over 13 runs** and,
measured this session, buys nothing at query time: its start-node pick is lexical, so on 14 true
paraphrase questions it scores **0/14 at 1555 tok/answer**; on 24 keyword questions 11/24 at
1027 tok against `rq.sh ask`'s 24/24 at 594 tok; re-running its 13 misses at an 8000-token
budget recovered **0**. Its 20,158 model-only cross-document edges are reachable only through
`explain` on a node you already know. Beside it sit a 27 MB versioned `graph.json` needing a
custom git merge driver, a 380 MB GitNexus index covering only code, and `/graphify --update` —
referenced by five files and existing as none.

Goal: one Rust binary, `repograph`, that builds and incrementally updates a project graph at
**0 API tokens forever**, answers at ≤130 tokens p90, and beats graphify on every measured axis.

Decisions taken by Max 2026-09-01: a one-time local embedding-model download is allowed;
graphify is to be replaced fully with its model-only edges frozen as a legacy layer; distribution
via an npm wrapper. Since the tool now lives in its own repo, **nothing in `beauty-crm` is
touched by this plan** — that integration is a later, separately-approved milestone (§Later).

## Measurements that drive the design

Paraphrase benchmark: 14 questions written to share no words with the target requirement line.

| approach | paraphrase | keyword (24) | answer cost | build tokens |
|---|---|---|---|---|
| graphify (LLM graph, BFS d2) | 0/14 | 11/24 | 1027–1555 tok | 14.6M |
| `rq.sh ask` (headlines + ids, rg) | 0/14 | 24/24 | 594–1080 tok | 0 |
| BM25 over requirement **bodies**, Russian stemming (prototyped) | 4/14 @5 | — | — | 0 |
| BM25 seed + 1-hop over the hand-written id graph (prototyped) | **7/14** | — | median 57, p90 128, max 378 tok | 0 |
| + local multilingual embeddings, RRF-fused | target ≥12/14 | 24/24 | same shape | 0 |

Corpus measured in `beauty-crm`: 1319 indexable files; `docs/` 159 md / 7.4 MB / 56,931 lines;
542 `.ts/.tsx` / 76,852 lines / 1964 top-level exports / 385 decorator applications (4 repo-local:
`RequireAction`, `Audited`, `RateLimited`, `OnDomainEvent`); **1632 requirement lines in 40 files
across two dialects** — `**ID · MUST · …**` (679) and `### ID · MUST · …` (918), separator
U+00B7; 34,836 id occurrences / 2962 distinct / 20 families, of which **3457 sit in code across
621 files** (comments, test titles); 426 markdown links, 417 intra-repo; 624 tables;
`constitution.yaml` = 20 CI-verified doc→code edges; 7 FR ids referenced but never declared;
355 files contain Cyrillic, 97 of them `.ts`; 16+ byte-identical generated files.

## Alternatives, and the silver bullet

| option | paraphrase | keyword | build tokens | freshness | why not alone |
|---|---|---|---|---|---|
| graphify | 0/14 | 11/24 | 14.6M | manual, stale since 08-26 | lexical door on an LLM graph |
| GitNexus | n/a | code only | 0 | manual, drifted 9 days | 380 MB, no docs, hid a file for 9 days |
| `rq.sh` (rg + awk) | 0/14 | 24/24 | 0 | 0.7 s rebuild | headlines only — no bodies, no symbols, no stemming |
| tantivy alone | ~4/14 | 24/24 | 0 | incremental | no graph: cannot walk a hit to what it implements |
| embeddings alone | est. 8–10/14 | weak on exact ids | 0 | incremental | fuzzy on exact `FR-PAY-22` asks; no structure |
| **repograph — exact → BM25(ru) → dense, RRF-fused, then 1-hop expansion** | ≥12/14 | 24/24 | 0 | <1 s per save | — |

The silver bullet is not a better graph; it is a better **door**. The hand-written id graph
already *is* the structure — graphify re-derived it for 14.6M tokens and then could not enter it.
Three cheap retrievers fused by reciprocal rank, then one hop over the structure the authors
wrote by hand, is where every measurement points. Everything else exists to keep that at 0 tokens
and fresh.

## Repository layout

```
repograph/                        # git init, no remote yet (publishing is a separate ask)
  Cargo.toml                      # workspace, one binary crate
  README.md  LICENSE  .gitignore
  repograph.toml                  # the tool's own config, and the worked example
  src/
    main.rs        clap: build | update | ask | explain | verify | bench | import-legacy
    config.rs      repograph.toml: doc globs, code globs, id families, requirement dialects
    walk.rs        ignore::WalkBuilder, blake3 per file, manifest diff
    store.rs       rkyv snapshot + manifest in <repo>/.repograph/
    doc/           tree-sitter-md: requirement blocks, id refs, links, tables, yaml registries
    code/          tree-sitter-typescript: exports, imports, classes, decorators, calls,
                   ids inside comments and string literals
    legacy.rs      one-shot import of a graphify graph.json's model-only edges
    index/
      lexical.rs   tantivy, SimpleTokenizer + Stemmer(Russian) + Stemmer(English)
      dense.rs     fastembed MultilingualE5Small, brute-force cosine (~4k vectors)
      fuse.rs      reciprocal rank fusion, exact-id short-circuit
    query.rs       seed → 1-hop expand → rank → `ID  path:line  headline`
  tests/           unit + golden-file tests
  bench/
    cases.jsonl    24 keyword + 14 paraphrase + code probes, each with a floor
    run.rs         `repograph bench --repo <path>`; corpus path from REPOGRAPH_BENCH_REPO
```

`repograph.toml` keeps the tool generic — id families, requirement-line dialects and doc/code
globs are configuration, not code. Defaults match `beauty-crm`'s conventions; a repo without a
config still gets code extraction plus markdown headings.

## Verified crates (docs-explorer, 2026-09-01 — pin these)

| crate | version | role | note |
|---|---|---|---|
| `tree-sitter` | 0.27.0 | parser runtime | grammars bind via `tree-sitter-language ^0.1.8` |
| `tree-sitter-typescript` | 0.23.2 | TS/TSX grammar | 2024 release; deps confirm `tree-sitter-language ^0.1` compatibility |
| `tree-sitter-md` | 0.5.3 | markdown grammar | one framework for both halves |
| `tantivy` | 0.26.1 | BM25 | `Language::Russian` is a built-in `Stemmer` variant; k1/b hardcoded at 1.2/0.75 |
| `petgraph` | 0.8.3 | in-memory multigraph, BFS | ships no Louvain — communities come only from the legacy import |
| `rkyv` | 0.8.18 | zero-copy snapshot | 0.8 API is `rancor`-based; **`bincode` is unmaintained — do not use** |
| `ignore` / `globset` | 0.4.33 / 0.4.20 | .gitignore-aware walk | `build_parallel()` |
| `blake3` | 1.8.7 | content hash | collapses the 16 identical generated files |
| `fastembed` | 6.0.2 | local ONNX embeddings | `EmbeddingModel::MultilingualE5Small`, dim 384, `intfloat/multilingual-e5-small`, `onnx/model.onnx` ≈450 MB, cache via `FASTEMBED_CACHE_DIR`, offline after first fetch, `try_new_from_user_defined` for vendored files |
| `clap` | 4.6.6 | CLI derive | |
| `serde` / `serde_json` / `toml` | 1.0.151 / current | config, legacy import, `--json` | |
| `rayon` | 1.12.0 | parallel extraction | |

Rejected deliberately: `oxc` 0.148 (weekly churn; its speed is irrelevant at 542 files), `swc`
(heaviest dep graph), `hnsw_rs`/`usearch` (4k vectors → brute force in <5 ms), `cozo` (3 years
stale) and `kuzu` (company pivoted, repo archived), `gix`/`git2` (blake3 over content suffices).

## Model

Nodes: `Requirement(id)`, `Entity(name)`, `Invariant`, `Adr`, `Milestone`, `Task`, `File`,
`Symbol(path::name)`, `LegacyConcept`. Each carries `source_file`, `line`, `label`, `body`.

Edges, unique on `(source, target, kind, context)` — the key `merge-ast.py` converged on:
`references`, `declares`, `links`, `implements`, `imports`, `re_exports`, `calls`, `extends`,
`decorated_by`, `legacy(relation)`.

## Extraction rules

**Docs.** Requirement head `^(#{1,6} |\*\*|\s*[-*] \*\*)?<ID> · (MUST|SHOULD|LATER)? · <title>`,
both dialects; body runs to the next head or heading. Id regex is the strict 20-family union from
the census, with `\b`; ranges (`R-1601…R-1603`, prefix-dropped `FR-RPT-42…48`) and slash lists
(`INV-11/12/20`) expand. Ambiguous families (`B1`, `C11`, `S3`, `I-015`) are **not** extracted —
the census shows them colliding with ordinary prose. Backticked spans in a title become `Entity`
references. Markdown links become `links` edges. `constitution.yaml`-shaped registries become
`Invariant` nodes with `implements` edges to their `test_ref`. Generated files (`TRACKER.md`) are
skipped by config — it duplicates 1066 checkboxes.

**Code.** A `Symbol` per top-level export, class, method and decorated member; a `File` per file.
Imports resolve through relative paths, tsconfig `paths`, `package.json` `exports` subpath maps,
and barrels (`export * from` followed transitively). Decorators produce `decorated_by` edges
carrying their argument as context, so `RequireAction('x')` reaches the permission id. Ids found
in comments and string literals produce `references` edges — that is the 3457-occurrence doc↔code
layer an AST-only indexer misses entirely. Paths are handled as bytes (Cyrillic filenames); a
`\0` byte in a `.ts`-globbed file is skipped, not fatal.

**Legacy.** `repograph import-legacy <graph.json>` takes edges whose endpoints are both
non-`_origin:"ast"` and share no id, resolving each endpoint to a real node by id-in-label, then
by `(basename, label)`, else creating a `LegacyConcept`. `community_name` is copied onto resolved
nodes. Unresolved count is printed, so the loss is a number rather than a surprise.

## Retrieval

`repograph ask <words…> [--json] [--budget N] [--bodies] [--no-dense]`
1. Exact — a token matching the id regex or an exact symbol name wins outright.
2. Lexical — tantivy over `label + body`, Russian and English stemming, top 20.
3. Dense — e5-small query embedding (with the `"query: "` prefix the model card specifies — to be
   confirmed at implementation), cosine over all node vectors, top 20. Skipped with `--no-dense`
   or when no model is cached, printing one line to say so.
4. Reciprocal-rank fusion (k=60) → top 5 seeds.
5. One hop over `references|implements|declares|links|legacy`, neighbours ranked by seed score.
6. Print `ID  path:line  headline` — the shape measured at 57 tok median. `--bodies` prints full
   requirement bodies (261 tok median).

`repograph explain <node>` — the node and its edges grouped by kind, legacy edges last.

## Incremental update

`repograph update` walks, hashes, diffs the manifest, re-extracts only changed files and replaces
their nodes and edges wholesale — correct here because nothing in the graph is model-derived.
Only changed nodes are re-embedded; tantivy does delete-by-file then re-add; the rkyv snapshot is
written to a temp file and renamed (`merge-ast.py` writes non-atomically — a known gap not
repeated). Expected: parsing in seconds, a one-time ~1–2 min embedding pass over ~4k nodes on an
M3 Pro (to be measured), incremental saves well under 1 s without dense.

## Phases — each ends green, `beauty-crm` used read-only as the corpus

| # | deliverable | done when | effort |
|---|---|---|---|
| 0 | `cargo new`, git init, config, walk+hash+manifest, rkyv store, no-op extractors | `repograph update --repo ../beauty-crm` twice is a fixed point | 1 d |
| 1 | doc extractor: requirements (both dialects), id graph, registries, milestones, links | nodes/edges ≥ `rq.sh index`'s 1935/11447; the 7 undeclared FR ids surface | 2 d |
| 2 | code extractor: exports, imports+barrels+aliases, decorators, ids in comments | `asGrosze`, `problemDetailsOf`, `vitestBase` resolve to the files `rg` finds | 2.5 d |
| 3 | tantivy + exact + expansion, `ask`/`explain` | keyword 24/24; paraphrase ≥7/14 with `--no-dense`; p90 ≤130 tok | 1.5 d |
| 4 | fastembed dense + RRF | paraphrase ≥12/14 on `bench/cases.jsonl` | 1 d |
| 5 | `import-legacy` | ≥90% of the 20,158 edges resolve to real nodes; `explain FR-PAY-22` shows them | 0.5 d |
| 6 | README, `repograph.toml` docs, CI (`cargo test` + `bench`), release binaries on tag | a clean clone builds and benches on another machine | 1 d |

Total ≈9 days. Phase 4 is the one gated on a measurement that does not exist yet, so it is
sequenced late and its fallback (`MultilingualE5Base`, 768-d) is named in advance.

## Later — integrating into beauty-crm (separate plan, separate approval)

Not started here, listed so the boundary is explicit: npm wrapper with per-platform
`optionalDependencies` (the esbuild/biome pattern, no postinstall — `beauty-crm`'s pnpm
`allowBuilds` allowlist would otherwise fail the install); a `"repograph"` script in the root
`package.json` (a protected path — authorised, but deferred to that plan); replacing
`graphify-autoupdate.mjs` and `graphify-notice.mjs`; folding `.claude/skills/repo-query/` into
`repograph ask`; deleting `graphify-out/` (2005 files, 53 MB) and the git merge driver; an ADR.

## Risks

- **Embedding quality on Russian PRD prose is a target, not a measurement.** ≥12/14 is the phase-4
  gate; the fallback is `MultilingualE5Base` before anything exotic.
- tantivy's `SimpleTokenizer` behaviour on Cyrillic is not documented — phase 3 carries a unit
  test that `Language::Russian` stems «начислений» and «начисляется» to one term.
- `tree-sitter-typescript` 0.23.2 dates from 2024; `satisfies` (33 uses) and decorators are in
  that grammar, and tree-sitter's error recovery yields the rest of a file when a newer syntax
  fails — which is why it was chosen over `oxc`.
- Legacy resolution by label is lossy by construction; the remainder is counted, not hidden.

## Verification

`cargo test` covers the id regex (ranges, slash lists, the ambiguous families that must *not*
match), both requirement dialects, barrel resolution, the Russian stemmer, RRF ordering and the
atomic write. `repograph bench --repo ../beauty-crm` runs the 38 recorded cases against their
floors and fails the build if recall or token cost regresses. Parity spot-checks against today's
answers: `ask cancellation` → FR-PAY-20, FR-PAY-22, N-151; a master's working hours → FR-DM-101.
End to end: add an export to a `.ts` in `beauty-crm`, `repograph update`, `explain` finds it;
revert, update, it is gone and `FR-PAY-22` still resolves.
