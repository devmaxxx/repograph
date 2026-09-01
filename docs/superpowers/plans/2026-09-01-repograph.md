# repograph Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One Rust binary, `repograph`, that builds and incrementally updates a project knowledge graph at 0 API tokens forever and answers repository questions at ≤130 tokens p90, benchmarked against `beauty-crm`.

**Architecture:** Two tree-sitter extractors (markdown, TypeScript) turn a repository into nodes and typed edges keyed by the identifiers its authors already write by hand (`FR-PAY-22`, `INV-01`, symbol names). Retrieval is a "door" onto that graph: exact-id short-circuit → in-memory BM25 with Russian+English stemming → local ONNX embeddings, fused by reciprocal rank, then one hop over the hand-written edges. Everything persists as JSON under `<repo>/.repograph/`; `update` re-extracts only files whose blake3 changed.

**Tech Stack:** Rust 1.86 (edition 2021), tree-sitter 0.27.0, tree-sitter-typescript 0.23.2, rust-stemmers 1.2.0, fastembed 6.0.2 (`MultilingualE5Small`), ignore 0.4.33, blake3 1.8.7, clap 4.6.6, serde 1.0.229 / serde_json 1.0.151 / toml 1.1.4 / serde_yaml 0.9.34, regex 1.13.1, rayon 1.12.0, anyhow 1.0.104, streaming-iterator 0.1.9.

**Spec:** `docs/superpowers/specs/2026-09-01-repograph-design.md`

## Deviations from the approved spec (simplifications — flag to Max before Task 1)

| spec says                   | plan does                                                                | why                                                                                                                          |
| --------------------------- | ------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------- |
| rkyv 0.8 zero-copy snapshot | `serde_json` files, atomic rename                                        | ~4k nodes / ~30k edges load in <50 ms as JSON; rkyv's rancor API adds a validation layer for no measurable gain at this size |
| tantivy 0.26 BM25           | own ~120-line in-memory BM25 over `rust-stemmers` (what tantivy vendors) | no on-disk segment lifecycle, no delete-by-term bookkeeping; corpus is ~4k short documents, scoring is microseconds          |
| petgraph multigraph         | `HashMap<NodeId, Vec<EdgeIdx>>` adjacency                                | only 1-hop expansion and `explain` walk the graph; no algorithms from petgraph are used                                      |
| tree-sitter-md 0.5.3 for markdown | a line scanner: one regex for the requirement head, `#` lines as block boundaries, one regex for links | both dialects are line-anchored; a block grammar would still need the same regex on heading text, and the 417 intra-repo links are `[text](path)` — one dependency fewer, same output |

None of these changes the accuracy gates. If Max wants rkyv/tantivy/petgraph back, the store and lexical modules are the only files that change.

## Global Constraints

- Build/update makes **zero network calls** except the one-time fastembed model download (`MultilingualE5Small`, ≈450 MB, cached under `FASTEMBED_CACHE_DIR` or the default fastembed cache dir).
- `beauty-crm` (`/Users/max/Documents/projects/beauty-crm`) is read-only corpus; the tool never writes there except into `<repo>/.repograph/`, and the bench uses `REPOGRAPH_BENCH_REPO` to find it.
- Every crate is pinned with `=` in `Cargo.toml`; APIs below were verified from registry sources on 2026-09-01. Never write a third-party API from memory — open `~/.cargo/registry/src/*/<crate>-<ver>/` when unsure.
- **No TypeScript/JavaScript in this repository.** Rust only: extractors, bench runner, CI scripts are Rust or plain shell. TypeScript appears solely as *corpus* (`beauty-crm` files being parsed) and as test fixtures under `tests/fixtures/`. The npm wrapper (later plan) is a `bin` shim of a few lines and lives in its own package, not here.
- Code, comments, commit messages, docs: English. Comments explain _why_, never _what_; no ticket ids in comments.
- Conventional Commits, no emoji, no `Co-Authored-By`/AI footers. Never combine a heredoc and `git commit` in one shell call — write the message with `-m`.
- Id regex must be the strict family list from config; ambiguous families (`B1`, `C11`, `S3`, `I-015`) must **not** match.
- Edge uniqueness key: `(source, target, kind, context)`.
- Answer shape: `ID  path:line  headline` — one line per hit.
- Gates: keyword 24/24; paraphrase ≥7/14 with `--no-dense`, ≥12/14 with dense; p90 ≤130 tokens (`chars/4`); `update` twice = fixed point; probes `asGrosze`→`packages/contracts/src/money.ts`, `problemDetailsOf`→`packages/contracts/src/errors.ts`, `vitestBase`→`packages/config/vitest.base.ts`.

---

## File structure

```
repograph/
  Cargo.toml                 pinned deps, one binary crate
  .gitignore                 target/, .repograph/
  README.md                  Task 15
  repograph.toml             the tool's own config, doubles as the worked example
  src/
    main.rs                  clap dispatch only
    config.rs                Config: globs, id families, skip list, requirement dialect
    ids.rs                   IdMatcher: strict regex, range + slash-list expansion
    walk.rs                  file list + blake3 manifest + diff
    model.rs                 Node, Edge, Graph (adjacency, replace_file)
    store.rs                 .repograph/{graph.json,manifest.json,vectors.bin}, atomic write
    doc/mod.rs               DocExtractor entry: markdown + yaml registries
    doc/requirements.rs      requirement heads (both dialects), bodies, id refs, entities
    doc/links.rs             markdown links → `links` edges
    doc/registry.rs          constitution.yaml-shaped registries → Invariant + implements
    code/mod.rs              CodeExtractor entry, tree-sitter parsers
    code/symbols.rs          exports, classes, methods, decorators → Symbol nodes + edges
    code/imports.rs          import specifiers → resolved paths (relative, .js→.ts, barrels, tsconfig paths, package exports)
    code/idrefs.rs           ids in comments + string literals → references
    index/lexical.rs         tokenizer, stemmers, BM25
    index/dense.rs           fastembed embed + cosine top-k, vectors.bin
    index/fuse.rs            RRF
    query.rs                 ask / explain: seeds → 1-hop → render
    legacy.rs                import-legacy from a graphify graph.json
    bench.rs                 bench runner over bench/cases.jsonl
  bench/cases.jsonl          38 recorded cases + 3 code probes
  tests/fixtures/            small md/ts/yaml files used by unit tests
  .github/workflows/ci.yml   cargo test + bench on the beauty-crm checkout
```

Module tree in `main.rs`: `mod config; mod ids; mod walk; mod model; mod store; mod doc; mod code; mod index; mod query; mod legacy; mod bench;` with `index/mod.rs` declaring `pub mod lexical; pub mod dense; pub mod fuse;`, `doc/mod.rs` declaring `pub mod requirements; pub mod links; pub mod registry;`, `code/mod.rs` declaring `pub mod symbols; pub mod imports; pub mod idrefs;`.

---

### Task 1: Cargo scaffold, config, CLI skeleton

**Files:**

- Create: `Cargo.toml`, `.gitignore`, `src/main.rs`, `src/config.rs`, `repograph.toml`
- Test: `src/config.rs` (`#[cfg(test)]`)

**Interfaces:**

- Produces: `Config { doc_globs: Vec<String>, code_globs: Vec<String>, skip: Vec<String>, id_families: Vec<String>, milestone_families: Vec<String>, registries: Vec<String> }`, `Config::load(repo: &Path) -> anyhow::Result<Config>` (reads `<repo>/repograph.toml`, falls back to `Config::default()`), `Cli` enum with subcommands `Build`, `Update`, `Ask`, `Explain`, `Verify`, `Bench`, `ImportLegacy`.

- [ ] **Step 1: Create the crate and pin dependencies**

```toml
# Cargo.toml
[package]
name = "repograph"
version = "0.1.0"
edition = "2021"
rust-version = "1.86"

[dependencies]
anyhow = "=1.0.104"
blake3 = "=1.8.7"
clap = { version = "=4.6.6", features = ["derive"] }
fastembed = "=6.0.2"
ignore = "=0.4.33"
rayon = "=1.12.0"
regex = "=1.13.1"
rust-stemmers = "=1.2.0"
serde = { version = "=1.0.229", features = ["derive"] }
serde_json = "=1.0.151"
serde_yaml = "=0.9.34"
streaming-iterator = "=0.1.9"
toml = "=1.1.4"
tree-sitter = "=0.27.0"
tree-sitter-typescript = "=0.23.2"

[dev-dependencies]
tempfile = "=3.27.0"

[profile.release]
lto = "thin"
```

`.gitignore`:

```
/target
/.repograph
```

- [ ] **Step 2: Write the failing config test**

```rust
// src/config.rs
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub doc_globs: Vec<String>,
    pub code_globs: Vec<String>,
    pub skip: Vec<String>,
    pub id_families: Vec<String>,
    pub milestone_families: Vec<String>,
    pub registries: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config::load(dir.path()).unwrap();
        assert!(cfg.id_families.contains(&"FR-PAY".to_string()));
        assert_eq!(cfg.doc_globs, vec!["**/*.md".to_string()]);
    }

    #[test]
    fn partial_file_overrides_only_named_keys() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("repograph.toml"), "skip = [\"docs/**/TRACKER.md\"]\n").unwrap();
        let cfg = Config::load(dir.path()).unwrap();
        assert_eq!(cfg.skip, vec!["docs/**/TRACKER.md".to_string()]);
        assert!(cfg.id_families.contains(&"INV".to_string()));
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test config::` — Expected: compile error, `Config::load` and `Default` missing.

- [ ] **Step 4: Implement defaults and loader**

```rust
// src/config.rs (append above the tests module)
impl Default for Config {
    fn default() -> Self {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        Config {
            doc_globs: s(&["**/*.md"]),
            code_globs: s(&["**/*.ts", "**/*.tsx"]),
            skip: s(&["**/node_modules/**", "**/dist/**", "**/TRACKER.md", "graphify-out/**", ".repograph/**"]),
            // The strict families measured in beauty-crm's census; ambiguous
            // one-letter families (B1, C11, S3) collide with prose and are left out.
            id_families: s(&[
                "FR-DM", "FR-CAL", "FR-VIS", "FR-PAY", "FR-PH", "FR-SEC", "FR-APP", "FR-MKT",
                "FR-AI", "FR-CRM", "FR-SHELL", "FR-TOOL", "FR-SVC", "FR-LIFE", "FR-WH", "FR-RPT",
                "AC-DM", "AC-VIS", "INV", "ADR", "OD", "OQ", "N", "R", "M", "W", "D", "G",
                "PREP", "CAL", "OR", "MON", "SEAM", "SG", "IDEA",
            ]),
            milestone_families: s(&["BE", "FE", "PLAT", "SYNC", "OPS", "AI"]),
            registries: s(&["docs/constitution.yaml"]),
        }
    }
}

impl Config {
    pub fn load(repo: &Path) -> Result<Config> {
        let path = repo.join("repograph.toml");
        if !path.exists() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
    }
}
```

- [ ] **Step 5: CLI skeleton**

```rust
// src/main.rs
mod config;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "repograph", version)]
struct Cli {
    /// Repository root; defaults to the current directory.
    #[arg(long, global = true, default_value = ".")]
    repo: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Build,
    Update,
    Ask {
        words: Vec<String>,
        #[arg(long)] json: bool,
        #[arg(long, default_value_t = 5)] seeds: usize,
        #[arg(long)] bodies: bool,
        #[arg(long)] no_dense: bool,
    },
    Explain { node: String },
    Verify,
    Bench { #[arg(long)] cases: Option<PathBuf> },
    ImportLegacy { graph_json: PathBuf },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let repo = cli.repo.canonicalize()?;
    let cfg = config::Config::load(&repo)?;
    match cli.cmd {
        _ => {
            println!("repograph: {} families configured at {}", cfg.id_families.len(), repo.display());
            Ok(())
        }
    }
}
```

`repograph.toml` at the repo root (the tool indexes itself; also the worked example):

```toml
doc_globs = ["**/*.md"]
code_globs = ["**/*.ts", "**/*.tsx"]
skip = ["**/node_modules/**", "**/dist/**", "**/TRACKER.md", "graphify-out/**", ".repograph/**"]
registries = ["docs/constitution.yaml"]
```

- [ ] **Step 6: Run tests and build**

Run: `cargo test config:: && cargo run -- --repo . build` — Expected: 2 passed; prints `repograph: 35 families configured at …`.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock .gitignore repograph.toml src/main.rs src/config.rs docs/
git commit -m "feat: scaffold repograph with config and CLI skeleton"
```

---

### Task 2: Strict id matcher with range and slash-list expansion

**Files:**

- Create: `src/ids.rs`
- Modify: `src/main.rs` (add `mod ids;`)

**Interfaces:**

- Produces: `IdMatcher::new(families: &[String], milestone_families: &[String]) -> IdMatcher`; `IdMatcher::find_all(&self, text: &str) -> Vec<IdHit>` where `IdHit { id: String, start: usize }` (byte offset); `IdMatcher::is_id(&self, token: &str) -> bool`. Expansion: `FR-RPT-42…48` → 42..=48; `R-1601…R-1603` → 3 ids; `INV-11/12/20` → 3 ids. Separators accepted: `…` (U+2026), `..`, `—` (U+2014), `–` (U+2013). A plain ASCII hyphen is **not** a separator: measured on the corpus, every `<id>-<n>` occurrence is a sub-numbered id (`R-08-1`), never a range.

- [ ] **Step 1: Write the failing tests**

```rust
// src/ids.rs
use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdHit {
    pub id: String,
    pub start: usize,
}

pub struct IdMatcher {
    single: Regex,
    range: Regex,
    slash: Regex,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m() -> IdMatcher {
        let cfg = crate::config::Config::default();
        IdMatcher::new(&cfg.id_families, &cfg.milestone_families)
    }

    fn ids(t: &str) -> Vec<String> {
        m().find_all(t).into_iter().map(|h| h.id).collect()
    }

    #[test]
    fn plain_ids_in_prose() {
        assert_eq!(ids("см. FR-PAY-22 и INV-01, ADR-0007"), vec!["FR-PAY-22", "INV-01", "ADR-0007"]);
    }

    #[test]
    fn ambiguous_families_do_not_match() {
        assert!(ids("вариант B1, C11 и S3; I-015 тоже").is_empty());
    }

    #[test]
    fn word_boundaries_hold() {
        assert!(ids("XFR-PAY-22Y").is_empty());
        assert_eq!(ids("(FR-PAY-22)."), vec!["FR-PAY-22"]);
    }

    #[test]
    fn ranges_expand_with_and_without_prefix() {
        assert_eq!(ids("FR-RPT-42…48"), vec!["FR-RPT-42", "FR-RPT-43", "FR-RPT-44", "FR-RPT-45", "FR-RPT-46", "FR-RPT-47", "FR-RPT-48"]);
        assert_eq!(ids("R-1601…R-1603"), vec!["R-1601", "R-1602", "R-1603"]);
    }

    #[test]
    fn slash_lists_expand() {
        assert_eq!(ids("INV-11/12/20"), vec!["INV-11", "INV-12", "INV-20"]);
    }

    #[test]
    fn milestones_and_tasks() {
        assert_eq!(ids("BE-M02 T25, PLAT-M01"), vec!["BE-M02", "PLAT-M01"]);
    }

    #[test]
    fn offsets_are_byte_offsets_into_utf8() {
        let hits = m().find_all("Правило FR-PAY-22");
        assert_eq!(hits[0].start, "Правило ".len());
    }

    #[test]
    fn is_id_is_exact() {
        assert!(m().is_id("FR-PAY-22"));
        assert!(!m().is_id("FR-PAY-22."));
        assert!(!m().is_id("asGrosze"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test ids::` — Expected: compile error, `IdMatcher::new` missing.

- [ ] **Step 3: Implement**

Boundaries are checked by hand rather than with `\b`: `\b` treats `-` as a boundary, so `XFR-PAY-22` would match on the `FR-PAY-22` tail, and Cyrillic letters glued to an id (`см.FR-PAY-22`) must still separate. A hit is valid when the byte before it and the byte after it are not in `[A-Za-z0-9-]`.

```rust
// src/ids.rs (above tests)
fn bounded(text: &str, start: usize, end: usize) -> bool {
    let tail = |b: u8| b.is_ascii_alphanumeric() || b == b'-';
    let left_ok = start == 0 || !tail(text.as_bytes()[start - 1]);
    let right_ok = end == text.len() || !tail(text.as_bytes()[end]);
    left_ok && right_ok
}

impl IdMatcher {
    pub fn new(families: &[String], milestone_families: &[String]) -> IdMatcher {
        let fam = families.iter().map(|f| regex::escape(f)).collect::<Vec<_>>().join("|");
        let ms = milestone_families.iter().map(|f| regex::escape(f)).collect::<Vec<_>>().join("|");
        let single = Regex::new(&format!(r"(?:{fam})-\d{{1,4}}|(?:{ms})-M\d{{2}}")).unwrap();
        // `FR-RPT-42…48`, `R-1601…R-1603`, `INV-11..13`, `N-1—N-3`
        let range = Regex::new(&format!(
            r"((?:{fam})-)(\d{{1,4}})\s*(?:…|\.\.|—|–)\s*(?:(?:{fam})-)?(\d{{1,4}})"
        )).unwrap();
        let slash = Regex::new(&format!(r"((?:{fam})-)(\d{{1,4}}(?:/\d{{1,4}})+)")).unwrap();
        IdMatcher { single, range, slash }
    }

    pub fn is_id(&self, token: &str) -> bool {
        self.single.find(token).map(|m| m.start() == 0 && m.end() == token.len()).unwrap_or(false)
    }

    pub fn find_all(&self, text: &str) -> Vec<IdHit> {
        let mut hits: Vec<IdHit> = Vec::new();
        let mut covered: Vec<(usize, usize)> = Vec::new();
        for c in self.range.captures_iter(text) {
            let whole = c.get(0).unwrap();
            if !bounded(text, whole.start(), whole.end()) {
                continue;
            }
            let prefix = c.get(1).unwrap().as_str();
            let (a, b): (u32, u32) = (c[2].parse().unwrap(), c[3].parse().unwrap());
            // A range wider than 50 is a typo or a year, not a list of requirements.
            if a <= b && b - a <= 50 {
                let width = c[2].len();
                for n in a..=b {
                    hits.push(IdHit { id: format!("{prefix}{n:0width$}"), start: whole.start() });
                }
                covered.push((whole.start(), whole.end()));
            }
        }
        for c in self.slash.captures_iter(text) {
            let whole = c.get(0).unwrap();
            if !bounded(text, whole.start(), whole.end()) {
                continue;
            }
            let prefix = c.get(1).unwrap().as_str();
            for n in c[2].split('/') {
                hits.push(IdHit { id: format!("{prefix}{n}"), start: whole.start() });
            }
            covered.push((whole.start(), whole.end()));
        }
        for m in self.single.find_iter(text) {
            let inside = covered.iter().any(|(s, e)| m.start() >= *s && m.end() <= *e);
            if inside || !bounded(text, m.start(), m.end()) {
                continue;
            }
            hits.push(IdHit { id: m.as_str().to_string(), start: m.start() });
        }
        hits.sort_by(|a, b| a.start.cmp(&b.start).then(a.id.cmp(&b.id)));
        hits.dedup();
        hits
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test ids::` — Expected: 8 passed.

- [ ] **Step 5: Commit**

```bash
git add src/ids.rs src/main.rs
git commit -m "feat: strict identifier matcher with range and slash-list expansion"
```

---

### Task 3: Walk, hash, manifest diff

**Files:**

- Create: `src/walk.rs`
- Modify: `src/main.rs` (add `mod walk;`), `Cargo.toml` (add `globset = "=0.4.20"`)

**Interfaces:**

- Consumes: `Config` (Task 1).
- Produces: `FileKind { Doc, Code, Registry }`; `Entry { rel: String, kind: FileKind, hash: String }`; `Manifest { files: BTreeMap<String, String> }` (rel → blake3 hex); `walk(repo: &Path, cfg: &Config) -> Result<Vec<Entry>>`; `Manifest::diff(&self, now: &[Entry]) -> Diff { changed: Vec<Entry>, removed: Vec<String> }`; `Manifest::from_entries(&[Entry]) -> Manifest`. Paths are forward-slash relative strings; a file that is not valid UTF-8 as a path is skipped with a warning on stderr, not fatal.

- [ ] **Step 1: Write the failing tests**

```rust
// src/walk.rs
use crate::config::Config;
use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileKind { Doc, Code, Registry }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry { pub rel: String, pub kind: FileKind, pub hash: String }

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Manifest { pub files: BTreeMap<String, String> }

#[derive(Debug, Default)]
pub struct Diff { pub changed: Vec<Entry>, pub removed: Vec<String> }

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("docs")).unwrap();
        std::fs::create_dir_all(d.path().join("node_modules/x")).unwrap();
        std::fs::write(d.path().join("docs/a.md"), "# a\n").unwrap();
        std::fs::write(d.path().join("docs/TRACKER.md"), "generated\n").unwrap();
        std::fs::write(d.path().join("docs/constitution.yaml"), "version: 1\n").unwrap();
        std::fs::write(d.path().join("b.ts"), "export const b = 1;\n").unwrap();
        std::fs::write(d.path().join("node_modules/x/c.ts"), "export const c = 1;\n").unwrap();
        std::fs::write(d.path().join(".gitignore"), "ignored.md\n").unwrap();
        std::fs::write(d.path().join("ignored.md"), "# hidden\n").unwrap();
        d
    }

    #[test]
    fn walk_classifies_and_skips() {
        let d = repo();
        let entries = walk(d.path(), &Config::default()).unwrap();
        let rels: Vec<_> = entries.iter().map(|e| (e.rel.as_str(), e.kind)).collect();
        assert_eq!(rels, vec![
            ("b.ts", FileKind::Code),
            ("docs/a.md", FileKind::Doc),
            ("docs/constitution.yaml", FileKind::Registry),
        ]);
        assert_eq!(entries[0].hash, blake3::hash(b"export const b = 1;\n").to_hex().to_string());
    }

    #[test]
    fn diff_reports_changed_and_removed() {
        let d = repo();
        let before = walk(d.path(), &Config::default()).unwrap();
        let manifest = Manifest::from_entries(&before);
        std::fs::write(d.path().join("docs/a.md"), "# a changed\n").unwrap();
        std::fs::remove_file(d.path().join("b.ts")).unwrap();
        let after = walk(d.path(), &Config::default()).unwrap();
        let diff = manifest.diff(&after);
        assert_eq!(diff.changed.iter().map(|e| e.rel.as_str()).collect::<Vec<_>>(), vec!["docs/a.md"]);
        assert_eq!(diff.removed, vec!["b.ts".to_string()]);
    }

    #[test]
    fn unchanged_tree_diffs_empty() {
        let d = repo();
        let before = walk(d.path(), &Config::default()).unwrap();
        let diff = Manifest::from_entries(&before).diff(&before);
        assert!(diff.changed.is_empty() && diff.removed.is_empty());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test walk::` — Expected: compile error, `walk` and `Manifest::diff` missing.

- [ ] **Step 3: Implement**

```rust
// src/walk.rs (above tests)
fn set(globs: &[String]) -> Result<GlobSet> {
    let mut b = GlobSetBuilder::new();
    for g in globs {
        b.add(Glob::new(g).with_context(|| format!("glob {g}"))?);
    }
    Ok(b.build()?)
}

pub fn walk(repo: &Path, cfg: &Config) -> Result<Vec<Entry>> {
    let docs = set(&cfg.doc_globs)?;
    let code = set(&cfg.code_globs)?;
    let skip = set(&cfg.skip)?;
    let registries = set(&cfg.registries)?;
    let mut out = Vec::new();
    for dent in ignore::WalkBuilder::new(repo).hidden(true).git_ignore(true).build() {
        let dent = match dent {
            Ok(d) => d,
            Err(e) => { eprintln!("walk: {e}"); continue; }
        };
        if !dent.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let rel_path = dent.path().strip_prefix(repo).unwrap_or(dent.path());
        let Some(rel) = rel_path.to_str().map(|s| s.replace('\\', "/")) else {
            eprintln!("walk: skipping non-UTF-8 path {}", rel_path.display());
            continue;
        };
        if skip.is_match(&rel) {
            continue;
        }
        let kind = if registries.is_match(&rel) { FileKind::Registry }
            else if docs.is_match(&rel) { FileKind::Doc }
            else if code.is_match(&rel) { FileKind::Code }
            else { continue };
        let bytes = std::fs::read(dent.path()).with_context(|| format!("read {rel}"))?;
        out.push(Entry { rel, kind, hash: blake3::hash(&bytes).to_hex().to_string() });
    }
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(out)
}

impl Manifest {
    pub fn from_entries(entries: &[Entry]) -> Manifest {
        Manifest { files: entries.iter().map(|e| (e.rel.clone(), e.hash.clone())).collect() }
    }

    pub fn diff(&self, now: &[Entry]) -> Diff {
        let mut d = Diff::default();
        for e in now {
            if self.files.get(&e.rel) != Some(&e.hash) {
                d.changed.push(e.clone());
            }
        }
        let present: std::collections::BTreeSet<&str> = now.iter().map(|e| e.rel.as_str()).collect();
        d.removed = self.files.keys().filter(|k| !present.contains(k.as_str())).cloned().collect();
        d
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test walk::` — Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/walk.rs src/main.rs
git commit -m "feat: gitignore-aware walk with blake3 manifest diff"
```

---

### Task 4: Graph model, JSON store, `build`/`update` fixed point

**Files:**

- Create: `src/model.rs`, `src/store.rs`
- Modify: `src/main.rs` (wire `Build`, `Update`)

**Interfaces:**

- Consumes: `walk`, `Manifest`, `Diff`, `Entry`, `FileKind` (Task 3).
- Produces:
  - `NodeKind { Requirement, Entity, Invariant, Adr, Milestone, Task, File, Symbol, LegacyConcept }`
  - `EdgeKind { References, Declares, Links, Implements, Imports, ReExports, Calls, Extends, DecoratedBy, Legacy }`
  - `Node { id: String, kind: NodeKind, label: String, body: String, file: String, line: u32, files: BTreeSet<String>, community: Option<String> }`
  - `Edge { source: String, target: String, kind: EdgeKind, context: String, file: String }` — `Ord` derived over all fields; the spec's uniqueness key is `(source, target, kind, context)` and is applied when rendering.
  - `Extraction { nodes: Vec<Node>, edges: Vec<Edge> }` with `Extraction::node(&mut self, ...)`, `Extraction::edge(&mut self, ...)` helpers.
  - `Graph { nodes: BTreeMap<String, Node>, edges: BTreeSet<Edge> }`; `Graph::remove_file(&mut self, rel: &str)`; `Graph::apply(&mut self, ex: Extraction)`; `Graph::neighbours(&self, id: &str) -> Vec<&Edge>` (both directions); `Graph::dangling(&self) -> Vec<&Edge>` (edges whose target has no node).
  - `Store::new(repo: &Path) -> Store`; `Store::load(&self) -> Result<(Graph, Manifest)>` (empty when absent); `Store::save(&self, g: &Graph, m: &Manifest) -> Result<()>` (temp file + rename); `Store::wipe(&self) -> Result<()>`.
  - `Extractor` trait: `fn extract(&self, rel: &str, text: &str) -> Extraction`.
  - `run_update(repo: &Path, cfg: &Config, extractors: &Extractors, wipe: bool) -> Result<UpdateReport { changed: usize, removed: usize, nodes: usize, edges: usize }>` where `Extractors { doc: Box<dyn Extractor>, code: Box<dyn Extractor>, registry: Box<dyn Extractor> }`.

- [ ] **Step 1: Write the failing tests**

```rust
// src/model.rs
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum NodeKind { Requirement, Entity, Invariant, Adr, Milestone, Task, File, Symbol, LegacyConcept }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EdgeKind { References, Declares, Links, Implements, Imports, ReExports, Calls, Extends, DecoratedBy, Legacy }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    #[serde(default)] pub body: String,
    pub file: String,
    pub line: u32,
    #[serde(default)] pub files: BTreeSet<String>,
    #[serde(default)] pub community: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
    #[serde(default)] pub context: String,
    pub file: String,
}

#[derive(Debug, Default)]
pub struct Extraction { pub nodes: Vec<Node>, pub edges: Vec<Edge> }

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: BTreeMap<String, Node>,
    pub edges: BTreeSet<Edge>,
}

pub trait Extractor {
    fn extract(&self, rel: &str, text: &str) -> Extraction;
}

pub struct Noop;
impl Extractor for Noop {
    fn extract(&self, _rel: &str, _text: &str) -> Extraction { Extraction::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(file: &str) -> Extraction {
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "CancellationPolicy", "", file, 385);
        e.node(NodeKind::Entity, "entity:CancellationPolicy", "CancellationPolicy", "", file, 385);
        e.edge("FR-PAY-22", "entity:CancellationPolicy", EdgeKind::References, "title", file);
        e.edge("FR-PAY-22", "N-151", EdgeKind::References, "body", file);
        e
    }

    #[test]
    fn apply_then_remove_file_restores_empty_graph() {
        let mut g = Graph::default();
        g.apply(ex("docs/06.md"));
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.edges.len(), 2);
        g.remove_file("docs/06.md");
        assert!(g.nodes.is_empty() && g.edges.is_empty());
    }

    #[test]
    fn node_declared_by_two_files_survives_one_removal() {
        let mut g = Graph::default();
        g.apply(ex("docs/06.md"));
        g.apply(ex("docs/07.md"));
        g.remove_file("docs/06.md");
        let n = &g.nodes["entity:CancellationPolicy"];
        assert_eq!(n.file, "docs/07.md");
        assert_eq!(n.files.len(), 1);
    }

    #[test]
    fn dangling_lists_edges_without_target_node() {
        let mut g = Graph::default();
        g.apply(ex("docs/06.md"));
        let d: Vec<_> = g.dangling().into_iter().map(|e| e.target.as_str()).collect();
        assert_eq!(d, vec!["N-151"]);
    }

    #[test]
    fn neighbours_are_bidirectional() {
        let mut g = Graph::default();
        g.apply(ex("docs/06.md"));
        assert_eq!(g.neighbours("entity:CancellationPolicy").len(), 1);
        assert_eq!(g.neighbours("FR-PAY-22").len(), 2);
    }
}
```

```rust
// src/store.rs
use crate::model::Graph;
use crate::walk::Manifest;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub struct Store { dir: PathBuf }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EdgeKind, Extraction, NodeKind};

    #[test]
    fn round_trip_and_atomic_write() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::File, "file:a.md", "a.md", "", "a.md", 1);
        e.edge("file:a.md", "INV-01", EdgeKind::References, "", "a.md");
        g.apply(e);
        let m = Manifest::default();
        store.save(&g, &m).unwrap();
        assert!(d.path().join(".repograph/graph.json").exists());
        assert!(!d.path().join(".repograph/graph.json.tmp").exists());
        let (g2, _) = store.load().unwrap();
        assert_eq!(g2.nodes.len(), 1);
        assert_eq!(g2.edges.len(), 1);
    }

    #[test]
    fn load_absent_is_empty() {
        let d = tempfile::tempdir().unwrap();
        let (g, m) = Store::new(d.path()).load().unwrap();
        assert!(g.nodes.is_empty() && m.files.is_empty());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test model:: store::` — Expected: compile errors for the missing impls.

- [ ] **Step 3: Implement model**

```rust
// src/model.rs (above tests)
impl Extraction {
    pub fn node(&mut self, kind: NodeKind, id: &str, label: &str, body: &str, file: &str, line: u32) {
        self.nodes.push(Node {
            id: id.to_string(), kind, label: label.to_string(), body: body.to_string(),
            file: file.to_string(), line, files: BTreeSet::from([file.to_string()]), community: None,
        });
    }

    pub fn edge(&mut self, source: &str, target: &str, kind: EdgeKind, context: &str, file: &str) {
        self.edges.push(Edge {
            source: source.to_string(), target: target.to_string(), kind,
            context: context.to_string(), file: file.to_string(),
        });
    }
}

impl Graph {
    pub fn apply(&mut self, ex: Extraction) {
        for n in ex.nodes {
            match self.nodes.get_mut(&n.id) {
                // The first declaring file stays primary so that `path:line` in an
                // answer is stable across updates that touch a second declaring file.
                Some(existing) => { existing.files.extend(n.files); }
                None => { self.nodes.insert(n.id.clone(), n); }
            }
        }
        self.edges.extend(ex.edges);
    }

    pub fn remove_file(&mut self, rel: &str) {
        self.edges.retain(|e| e.file != rel);
        let mut gone = Vec::new();
        for (id, n) in self.nodes.iter_mut() {
            n.files.remove(rel);
            if n.files.is_empty() {
                gone.push(id.clone());
            } else if n.file == rel {
                n.file = n.files.iter().next().unwrap().clone();
            }
        }
        for id in gone {
            self.nodes.remove(&id);
        }
    }

    pub fn neighbours(&self, id: &str) -> Vec<&Edge> {
        self.edges.iter().filter(|e| e.source == id || e.target == id).collect()
    }

    pub fn dangling(&self) -> Vec<&Edge> {
        self.edges.iter().filter(|e| !self.nodes.contains_key(&e.target)).collect()
    }
}
```

`neighbours` is a linear scan; at ~30k edges that is ~100 µs and `ask` calls it at most six times. An adjacency map is built lazily in Task 10 when the query path needs repeated lookups.

- [ ] **Step 4: Implement store**

```rust
// src/store.rs (above tests)
impl Store {
    pub fn new(repo: &Path) -> Store { Store { dir: repo.join(".repograph") } }

    pub fn dir(&self) -> &Path { &self.dir }

    pub fn load(&self) -> Result<(Graph, Manifest)> {
        let read = |name: &str| -> Result<Option<String>> {
            let p = self.dir.join(name);
            if !p.exists() { return Ok(None); }
            Ok(Some(std::fs::read_to_string(&p).with_context(|| format!("read {}", p.display()))?))
        };
        let g = match read("graph.json")? { Some(t) => serde_json::from_str(&t).context("graph.json")?, None => Graph::default() };
        let m = match read("manifest.json")? { Some(t) => serde_json::from_str(&t).context("manifest.json")?, None => Manifest::default() };
        Ok((g, m))
    }

    pub fn save(&self, g: &Graph, m: &Manifest) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        self.write_atomic("graph.json", &serde_json::to_vec(g)?)?;
        self.write_atomic("manifest.json", &serde_json::to_vec(m)?)
    }

    pub fn write_atomic(&self, name: &str, bytes: &[u8]) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let tmp = self.dir.join(format!("{name}.tmp"));
        std::fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
        std::fs::rename(&tmp, self.dir.join(name)).with_context(|| format!("rename {name}"))?;
        Ok(())
    }

    pub fn read_bytes(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let p = self.dir.join(name);
        if !p.exists() { return Ok(None); }
        Ok(Some(std::fs::read(&p)?))
    }

    pub fn wipe(&self) -> Result<()> {
        if self.dir.exists() { std::fs::remove_dir_all(&self.dir)?; }
        Ok(())
    }
}
```

- [ ] **Step 5: Wire `build`/`update` in main**

```rust
// src/main.rs — add `mod walk; mod model; mod store;` beside `mod ids;`, then:

use model::{Extractor, Graph, Noop};

pub struct Extractors {
    pub doc: Box<dyn Extractor>,
    pub code: Box<dyn Extractor>,
    pub registry: Box<dyn Extractor>,
}

pub struct UpdateReport { pub changed: usize, pub removed: usize, pub nodes: usize, pub edges: usize }

pub fn run_update(repo: &std::path::Path, cfg: &config::Config, ex: &Extractors, wipe: bool) -> anyhow::Result<UpdateReport> {
    let store = store::Store::new(repo);
    if wipe { store.wipe()?; }
    let (mut graph, manifest) = store.load()?;
    let entries = walk::walk(repo, cfg)?;
    let diff = manifest.diff(&entries);
    for rel in &diff.removed { graph.remove_file(rel); }
    for e in &diff.changed {
        graph.remove_file(&e.rel);
        let text = match std::fs::read(repo.join(&e.rel)) {
            Ok(b) if !b.contains(&0) => String::from_utf8_lossy(&b).into_owned(),
            // A NUL byte means a binary that matched a source glob; skip it, do not fail the run.
            Ok(_) => continue,
            Err(err) => { eprintln!("read {}: {err}", e.rel); continue; }
        };
        let extractor = match e.kind {
            walk::FileKind::Doc => &ex.doc,
            walk::FileKind::Code => &ex.code,
            walk::FileKind::Registry => &ex.registry,
        };
        graph.apply(extractor.extract(&e.rel, &text));
    }
    store.save(&graph, &walk::Manifest::from_entries(&entries))?;
    Ok(UpdateReport { changed: diff.changed.len(), removed: diff.removed.len(), nodes: graph.nodes.len(), edges: graph.edges.len() })
}

fn extractors(_cfg: &config::Config) -> Extractors {
    Extractors { doc: Box::new(Noop), code: Box::new(Noop), registry: Box::new(Noop) }
}

// in main(), before the match (the match consumes `cli.cmd`):
    let wipe = matches!(cli.cmd, Cmd::Build);
    match cli.cmd {
        Cmd::Build | Cmd::Update => {
            let r = run_update(&repo, &cfg, &extractors(&cfg), wipe)?;
            println!("changed {} removed {} nodes {} edges {}", r.changed, r.removed, r.nodes, r.edges);
            Ok(())
        }
        _ => anyhow::bail!("not implemented yet"),
    }
```

- [ ] **Step 6: Run tests, then the fixed-point check on beauty-crm**

Run: `cargo test` — Expected: all green.

Run: `cargo run --release -- --repo /Users/max/Documents/projects/beauty-crm build && cargo run --release -- --repo /Users/max/Documents/projects/beauty-crm update`
Expected: first prints `changed ~1300 removed 0 nodes 0 edges 0`; second prints `changed 0 removed 0 …`. `git -C /Users/max/Documents/projects/beauty-crm status --short` shows only `?? .repograph/` (add `.repograph/` to beauty-crm's `.gitignore` in the later integration plan, not now).

- [ ] **Step 7: Commit**

```bash
git add src/model.rs src/store.rs src/main.rs
git commit -m "feat: graph model, atomic JSON store, incremental update loop"
```

---

### Task 5: Doc extractor — requirement blocks, id references, entities, milestones, ADRs

**Files:**

- Create: `src/doc/mod.rs`, `src/doc/requirements.rs`, `tests/fixtures/06-payments.md`, `tests/fixtures/03-calendar.md`, `tests/fixtures/BE-M01-foundation.md`
- Modify: `src/main.rs` (`mod doc;`, use `doc::DocExtractor` in `extractors`)

**Interfaces:**

- Consumes: `IdMatcher` (Task 2), `Extraction`, `NodeKind`, `EdgeKind`, `Extractor` (Task 4).
- Produces: `DocExtractor::new(ids: IdMatcher) -> DocExtractor` implementing `Extractor`. Node ids: requirement id verbatim (`FR-PAY-22`), `entity:<Name>`, `file:<rel>`, milestone `BE-M01`, task `BE-M01/T03`, ADR `ADR-001`. Kinds by family prefix: `INV-` → `Invariant`, `ADR-` → `Adr`, milestone pattern → `Milestone`, else `Requirement`. `body` = text up to the next head/heading, capped at 40 lines. `label` = the title after the level marker, backticks kept.

Requirement head regex (both dialects, one expression):

```
^(?:#{1,6}\s+|\*\*|\s*[-*]\s+\*\*)?(<ID>)\s*·\s*(?:(MUST|SHOULD|LATER)\s*·\s*)?(.*?)\s*\**\s*$
```

`<ID>` is the `IdMatcher::single` pattern; the head line must start with one of the three prefixes or the id itself.

- [ ] **Step 1: Fixtures**

`tests/fixtures/06-payments.md`:

````markdown
# Платежи

Общие слова про раздел, где упомянут `FR-PAY-20` и INV-11/12.

**FR-PAY-22 · MUST · `CancellationPolicy` — правило с числами, а не текст (`N-151`).**

```ts
type CancellationPolicy = { freeUntilMinutes: number; feePercent: number };
```

Отмена позже порога — штраф по FR-PAY-26.

**FR-PAY-26 · SHOULD · Списание штрафа происходит автоматически.**

Текст второго требования.

## Не требование

Обычный абзац с ссылкой на FR-CAL-40.
````

`tests/fixtures/03-calendar.md`:

```markdown
### FR-CAL-40 · MUST · Единый словарь машинных кодов конфликтов

> Коды из FR-CAL-41, FR-CAL-125 и FR-PAY-143; см. OQ-25, FR-CAL-155…157, FR-CAL-50.

### FR-CAL-41 · MUST · Второе
```

`tests/fixtures/BE-M01-foundation.md`:

```markdown
# BE-M01 — foundation

**Status:** [ ] in progress

- [x] **T01** Workspace + turbo pipeline (`/`, `packages/config`) (0.75d)
- [ ] **T03** `packages/db`: Drizzle setup — `DataScope` (`FR-DM-05`) and the `FR-DM-04` block (1d)
```

- [ ] **Step 2: Write the failing tests**

```rust
// src/doc/requirements.rs
use crate::ids::IdMatcher;
use crate::model::{EdgeKind, Extraction, NodeKind};
use regex::Regex;

pub struct RequirementScanner {
    ids: IdMatcher,
    head: Regex,
    entity: Regex,
    task: Regex,
    milestone_file: Regex,
    adr_file: Regex,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(rel: &str, text: &str) -> Extraction {
        let cfg = crate::config::Config::default();
        RequirementScanner::new(IdMatcher::new(&cfg.id_families, &cfg.milestone_families)).scan(rel, text)
    }

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn has(ex: &Extraction, s: &str, t: &str, k: EdgeKind) -> bool {
        ex.edges.iter().any(|e| e.source == s && e.target == t && e.kind == k)
    }

    #[test]
    fn bold_dialect_head_body_and_entity() {
        let ex = scan("docs/06.md", &fixture("06-payments.md"));
        let n = ex.nodes.iter().find(|n| n.id == "FR-PAY-22").unwrap();
        assert_eq!(n.kind, NodeKind::Requirement);
        assert_eq!(n.line, 5);
        assert!(n.label.starts_with("`CancellationPolicy` — правило"));
        assert!(n.body.contains("freeUntilMinutes"));
        assert!(n.body.contains("штраф по FR-PAY-26"));
        assert!(!n.body.contains("FR-PAY-26 · SHOULD"));
        assert!(ex.nodes.iter().any(|n| n.id == "entity:CancellationPolicy" && n.kind == NodeKind::Entity));
        assert!(has(&ex, "FR-PAY-22", "entity:CancellationPolicy", EdgeKind::References));
        assert!(has(&ex, "FR-PAY-22", "N-151", EdgeKind::References));
        assert!(has(&ex, "FR-PAY-22", "FR-PAY-26", EdgeKind::References));
    }

    #[test]
    fn ids_outside_blocks_hang_off_the_file_node() {
        let ex = scan("docs/06.md", &fixture("06-payments.md"));
        assert!(ex.nodes.iter().any(|n| n.id == "file:docs/06.md" && n.kind == NodeKind::File));
        assert!(has(&ex, "file:docs/06.md", "FR-PAY-20", EdgeKind::References));
        assert!(has(&ex, "file:docs/06.md", "INV-12", EdgeKind::References));
        assert!(has(&ex, "file:docs/06.md", "FR-CAL-40", EdgeKind::References));
        assert!(has(&ex, "file:docs/06.md", "FR-PAY-22", EdgeKind::Declares));
    }

    #[test]
    fn heading_dialect_with_ranges() {
        let ex = scan("docs/03.md", &fixture("03-calendar.md"));
        let n = ex.nodes.iter().find(|n| n.id == "FR-CAL-40").unwrap();
        assert_eq!(n.label, "Единый словарь машинных кодов конфликтов");
        for t in ["FR-CAL-41", "FR-CAL-125", "FR-PAY-143", "OQ-25", "FR-CAL-155", "FR-CAL-156", "FR-CAL-157", "FR-CAL-50"] {
            assert!(has(&ex, "FR-CAL-40", t, EdgeKind::References), "{t}");
        }
        assert!(!has(&ex, "FR-CAL-40", "FR-CAL-40", EdgeKind::References));
        assert!(ex.nodes.iter().any(|n| n.id == "FR-CAL-41"));
    }

    #[test]
    fn milestone_file_yields_milestone_and_tasks() {
        let ex = scan("docs/prd/plans/milestones/backend/BE-M01-foundation.md", &fixture("BE-M01-foundation.md"));
        assert!(ex.nodes.iter().any(|n| n.id == "BE-M01" && n.kind == NodeKind::Milestone));
        let t = ex.nodes.iter().find(|n| n.id == "BE-M01/T03").unwrap();
        assert_eq!(t.kind, NodeKind::Task);
        assert_eq!(t.line, 6);
        assert!(has(&ex, "BE-M01", "BE-M01/T03", EdgeKind::Declares));
        assert!(has(&ex, "BE-M01/T03", "FR-DM-05", EdgeKind::Implements));
        assert!(has(&ex, "BE-M01/T03", "FR-DM-04", EdgeKind::Implements));
    }

    #[test]
    fn adr_file_yields_adr_node() {
        let ex = scan("docs/adr/ADR-001-monorepo-and-tooling.md", "# ADR-001: Monorepo\n\nSee INV-06.\n");
        let n = ex.nodes.iter().find(|n| n.id == "ADR-001").unwrap();
        assert_eq!(n.kind, NodeKind::Adr);
        assert_eq!(n.label, "ADR-001: Monorepo");
        assert!(has(&ex, "ADR-001", "INV-06", EdgeKind::References));
    }

    #[test]
    fn invariant_family_gets_invariant_kind() {
        let ex = scan("docs/01.md", "**INV-05 · MUST · Каждая запись аудируется.**\n");
        assert_eq!(ex.nodes.iter().find(|n| n.id == "INV-05").unwrap().kind, NodeKind::Invariant);
    }

    #[test]
    fn edges_are_unique_per_key() {
        let ex = scan("docs/x.md", "**FR-PAY-22 · MUST · a**\n\nFR-PAY-26 и снова FR-PAY-26.\n");
        let n = ex.edges.iter().filter(|e| e.target == "FR-PAY-26").count();
        assert_eq!(n, 1);
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test doc::` — Expected: compile error, `RequirementScanner::new` and `scan` missing.

- [ ] **Step 4: Implement**

```rust
// src/doc/requirements.rs (above tests)
const BODY_CAP: usize = 40;

fn kind_for(id: &str) -> NodeKind {
    static MILESTONE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let ms = MILESTONE.get_or_init(|| Regex::new(r"^[A-Z]+-M\d{2}$").unwrap());
    if id.starts_with("INV-") { NodeKind::Invariant }
    else if id.starts_with("ADR-") { NodeKind::Adr }
    else if ms.is_match(id) { NodeKind::Milestone }
    else { NodeKind::Requirement }
}

impl RequirementScanner {
    pub fn new(ids: IdMatcher) -> RequirementScanner {
        let id = ids.single_pattern();
        RequirementScanner {
            ids,
            head: Regex::new(&format!(
                r"^(?:#{{1,6}}\s+|\*\*|\s*[-*]\s+\*\*)?({id})\s*·\s*(?:(MUST|SHOULD|LATER)\s*·\s*)?(.*?)\s*\**\s*$"
            )).unwrap(),
            entity: Regex::new(r"`([A-Za-z][A-Za-z0-9_.]*)`").unwrap(),
            task: Regex::new(r"^\s*-\s+\[[ xX]\]\s+\*\*(T\d{2,3})\*\*\s*(.*)$").unwrap(),
            milestone_file: Regex::new(r"(?:^|/)([A-Z]+-M\d{2})[^/]*\.md$").unwrap(),
            adr_file: Regex::new(r"(?:^|/)(ADR-\d{3,4})[^/]*\.md$").unwrap(),
        }
    }

    pub fn scan(&self, rel: &str, text: &str) -> Extraction {
        let mut ex = Extraction::default();
        let file_id = format!("file:{rel}");
        ex.node(NodeKind::File, &file_id, rel, "", rel, 1);

        let lines: Vec<&str> = text.lines().collect();
        let mut heads: Vec<(usize, String, String)> = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if let Some(c) = self.head.captures(line) {
                heads.push((i, c[1].to_string(), c[3].trim().to_string()));
            }
        }

        let mut in_block = vec![false; lines.len()];
        for (n, (start, id, title)) in heads.iter().enumerate() {
            let mut end = heads.get(n + 1).map(|h| h.0).unwrap_or(lines.len());
            for j in start + 1..end {
                if lines[j].starts_with('#') { end = j; break; }
            }
            let end = end.min(start + 1 + BODY_CAP);
            let body = lines[start + 1..end].join("\n");
            for j in *start..end { in_block[j] = true; }
            ex.node(kind_for(id), id, title, body.trim(), rel, *start as u32 + 1);
            ex.edge(&file_id, id, EdgeKind::Declares, "", rel);
            for cap in self.entity.captures_iter(title) {
                let name = &cap[1];
                if self.ids.is_id(name) { continue; }
                let eid = format!("entity:{name}");
                ex.node(NodeKind::Entity, &eid, name, "", rel, *start as u32 + 1);
                ex.edge(id, &eid, EdgeKind::References, "title", rel);
            }
            let scope = lines[*start..end].join("\n");
            for hit in self.ids.find_all(&scope) {
                if hit.id != *id {
                    ex.edge(id, &hit.id, EdgeKind::References, "body", rel);
                }
            }
        }

        // Ids in prose outside any requirement block still tie the document to
        // the graph; the File node is their source.
        let owner = self.file_owner(rel, &lines, &mut ex);
        for (i, line) in lines.iter().enumerate() {
            if in_block[i] { continue; }
            if let Some(c) = self.task.captures(line) {
                if let Some(ms) = &owner {
                    let tid = format!("{ms}/{}", &c[1]);
                    ex.node(NodeKind::Task, &tid, c[2].trim(), "", rel, i as u32 + 1);
                    ex.edge(ms, &tid, EdgeKind::Declares, "", rel);
                    for hit in self.ids.find_all(&c[2]) {
                        ex.edge(&tid, &hit.id, EdgeKind::Implements, "task", rel);
                    }
                    continue;
                }
            }
            let source = owner.as_deref().unwrap_or(&file_id);
            for hit in self.ids.find_all(line) {
                if Some(hit.id.as_str()) != owner.as_deref() {
                    ex.edge(source, &hit.id, EdgeKind::References, "prose", rel);
                }
            }
        }

        ex.edges.sort();
        ex.edges.dedup();
        ex
    }

    /// A milestone or ADR file owns its prose: `BE-M01-….md` is the node BE-M01.
    fn file_owner(&self, rel: &str, lines: &[&str], ex: &mut Extraction) -> Option<String> {
        let (id, kind) = if let Some(c) = self.milestone_file.captures(rel) {
            (c[1].to_string(), NodeKind::Milestone)
        } else if let Some(c) = self.adr_file.captures(rel) {
            (c[1].to_string(), NodeKind::Adr)
        } else {
            return None;
        };
        let title = lines.iter().find(|l| l.starts_with("# ")).map(|l| l[2..].trim()).unwrap_or(&id).to_string();
        ex.node(kind, &id, &title, "", rel, 1);
        ex.edge(&format!("file:{rel}"), &id, EdgeKind::Declares, "", rel);
        Some(id)
    }
}
```

Add to `src/ids.rs`:

```rust
impl IdMatcher {
    /// The bare id alternation, for callers that embed it in a larger expression.
    pub fn single_pattern(&self) -> String { self.single.as_str().to_string() }
}
```

```rust
// src/doc/mod.rs
pub mod requirements;

use crate::ids::IdMatcher;
use crate::model::{Extraction, Extractor};

pub struct DocExtractor { req: requirements::RequirementScanner }

impl DocExtractor {
    pub fn new(ids: IdMatcher) -> DocExtractor {
        DocExtractor { req: requirements::RequirementScanner::new(ids) }
    }
}

impl Extractor for DocExtractor {
    fn extract(&self, rel: &str, text: &str) -> Extraction { self.req.scan(rel, text) }
}
```

In `main.rs`, `extractors`:

```rust
fn extractors(cfg: &config::Config) -> Extractors {
    let ids = ids::IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
    Extractors { doc: Box::new(doc::DocExtractor::new(ids)), code: Box::new(Noop), registry: Box::new(Noop) }
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test doc::` — Expected: 7 passed. If `bold_dialect_head_body_and_entity` fails on `line`, the `**FR-PAY-22` line is line 5 of the fixture (1-based) — check the fixture has exactly one blank line between paragraphs as shown.

- [ ] **Step 6: Measure on beauty-crm**

Run: `cargo run --release -- --repo /Users/max/Documents/projects/beauty-crm build`
Expected: `nodes ≥ 1935`, `edges ≥ 11447` (the `rq.sh index` floor from the spec). Then:

```bash
python3 -c "
import json; g=json.load(open('/Users/max/Documents/projects/beauty-crm/.repograph/graph.json'))
req=[n for n in g['nodes'].values() if n['kind']=='Requirement']
print('requirements', len(req))
ids={n['id'] for n in g['nodes'].values()}
und=sorted({e['target'] for e in g['edges'] if e['target'].startswith('FR-') and e['target'] not in ids})
print('undeclared FR', len(und), und[:10])"
```

Expected: `requirements ≥ 1632` (the census: 679 bold + 918 heading heads, some multi-declared) and `undeclared FR 7`. Record both numbers in the commit message.

- [ ] **Step 7: Commit**

```bash
git add src/doc src/ids.rs src/main.rs tests/fixtures
git commit -m "feat: extract requirements, id references, milestones and ADRs from markdown"
```

---

### Task 6: Markdown links and YAML registries

**Files:**

- Create: `src/doc/links.rs`, `src/doc/registry.rs`, `tests/fixtures/constitution.yaml`
- Modify: `src/doc/mod.rs` (compose scanners), `src/main.rs` (registry extractor)

**Interfaces:**

- Consumes: `Extraction`, `EdgeKind`, `NodeKind` (Task 4); `IdMatcher` (Task 2).
- Produces: `links::scan(rel: &str, text: &str, ex: &mut Extraction)` — intra-repo markdown links become `Links` edges `file:<rel>` → `file:<normalised target>`; URLs and pure anchors are ignored. `RegistryExtractor::new(ids: IdMatcher)` implementing `Extractor`: each `invariants[]` row becomes an `Invariant` node (id = row id, label = first line of `statement` without `**`, body = `mechanism`), an `Implements` edge to `file:<test_ref>` when `test_ref` contains `/`, else to `gate:<test_ref>`, and `References` edges for ids in `basis`.

- [ ] **Step 1: Fixture**

`tests/fixtures/constitution.yaml`:

```yaml
version: 1
source: docs/prd-2026-08-16/prd/01-vision-and-roles.md
invariants:
  - id: INV-01
    statement: |-
      **Ранжирование не зависит от платного признака.** Вход — белый список полей
    mechanism: |-
      CI-тест `ranking_input_whitelist_test`
    test_ref: ranking_input_whitelist_test
    owner: BE dev
    basis: решение 1, `N-039`, `I-015`
  - id: INV-06
    statement: |-
      **Контуры не пересекаются.**
    mechanism: RLS
    test_ref: packages/db/test/contours.spec.ts
    owner: BE dev
  - id: INV-02
    statement: "**Нет платных мест.**"
    mechanism: миграции
    manual_checklist_ref: docs/ops/manual-acceptance-checklist.md#inv-02
    owner: BE dev
```

- [ ] **Step 2: Write the failing tests**

```rust
// src/doc/links.rs
use crate::model::{EdgeKind, Extraction};
use regex::Regex;
use std::sync::OnceLock;

#[cfg(test)]
mod tests {
    use super::*;

    fn links(rel: &str, text: &str) -> Vec<(String, String)> {
        let mut ex = Extraction::default();
        scan(rel, text, &mut ex);
        ex.edges.iter().filter(|e| e.kind == EdgeKind::Links).map(|e| (e.source.clone(), e.target.clone())).collect()
    }

    #[test]
    fn relative_links_normalise_against_the_file_directory() {
        let l = links("docs/prd/03-calendar.md", "see [payments](06-payments.md#fr-pay-22) and [adr](../adr/ADR-002-hosting.md)");
        assert_eq!(l, vec![
            ("file:docs/prd/03-calendar.md".into(), "file:docs/prd/06-payments.md".into()),
            ("file:docs/prd/03-calendar.md".into(), "file:docs/adr/ADR-002-hosting.md".into()),
        ]);
    }

    #[test]
    fn urls_and_anchors_are_ignored() {
        assert!(links("a.md", "[x](https://example.com) [y](#local) [z](mailto:a@b.c)").is_empty());
    }

    #[test]
    fn root_relative_links_keep_their_path() {
        assert_eq!(links("docs/a.md", "[c](/docs/constitution.yaml)"), vec![("file:docs/a.md".into(), "file:docs/constitution.yaml".into())]);
    }
}
```

```rust
// src/doc/registry.rs
use crate::ids::IdMatcher;
use crate::model::{EdgeKind, Extraction, Extractor, NodeKind};
use serde::Deserialize;

#[derive(Deserialize)]
struct Registry { #[serde(default)] invariants: Vec<Row> }

#[derive(Deserialize)]
struct Row {
    id: String,
    #[serde(default)] statement: String,
    #[serde(default)] mechanism: String,
    #[serde(default)] test_ref: Option<String>,
    #[serde(default)] basis: Option<String>,
}

pub struct RegistryExtractor { ids: IdMatcher }

#[cfg(test)]
mod tests {
    use super::*;

    fn ex() -> Extraction {
        let cfg = crate::config::Config::default();
        let text = std::fs::read_to_string(format!("{}/tests/fixtures/constitution.yaml", env!("CARGO_MANIFEST_DIR"))).unwrap();
        RegistryExtractor::new(IdMatcher::new(&cfg.id_families, &cfg.milestone_families)).extract("docs/constitution.yaml", &text)
    }

    #[test]
    fn rows_become_invariants_with_implements_edges() {
        let ex = ex();
        let n = ex.nodes.iter().find(|n| n.id == "INV-01").unwrap();
        assert_eq!(n.kind, NodeKind::Invariant);
        assert_eq!(n.label, "Ранжирование не зависит от платного признака.");
        assert!(n.body.contains("ranking_input_whitelist_test"));
        assert!(ex.edges.iter().any(|e| e.source == "INV-01" && e.target == "gate:ranking_input_whitelist_test" && e.kind == EdgeKind::Implements));
        assert!(ex.edges.iter().any(|e| e.source == "INV-06" && e.target == "file:packages/db/test/contours.spec.ts" && e.kind == EdgeKind::Implements));
        assert!(ex.edges.iter().any(|e| e.source == "INV-01" && e.target == "N-039" && e.kind == EdgeKind::References));
        assert!(!ex.edges.iter().any(|e| e.source == "INV-02" && e.kind == EdgeKind::Implements));
    }

    #[test]
    fn non_registry_yaml_is_harmless() {
        let cfg = crate::config::Config::default();
        let r = RegistryExtractor::new(IdMatcher::new(&cfg.id_families, &cfg.milestone_families));
        let ex = r.extract("x.yaml", "foo: bar\n");
        assert_eq!(ex.nodes.len(), 1);
        assert!(ex.edges.is_empty());
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test doc::links doc::registry` — Expected: compile errors.

- [ ] **Step 4: Implement links**

```rust
// src/doc/links.rs (above tests)
fn link_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[[^\]]*\]\(([^)\s#]+)(?:#[^)]*)?\)").unwrap())
}

/// Resolve `target` against the directory of `rel`, collapsing `.` and `..`.
pub fn normalise(rel: &str, target: &str) -> String {
    let mut parts: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        rel.rsplit_once('/').map(|(d, _)| d.split('/').filter(|s| !s.is_empty()).collect()).unwrap_or_default()
    };
    for seg in target.trim_start_matches('/').split('/') {
        match seg {
            "" | "." => {}
            ".." => { parts.pop(); }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

pub fn scan(rel: &str, text: &str, ex: &mut Extraction) {
    for c in link_re().captures_iter(text) {
        let t = &c[1];
        if t.contains("://") || t.starts_with("mailto:") {
            continue;
        }
        ex.edge(&format!("file:{rel}"), &format!("file:{}", normalise(rel, t)), EdgeKind::Links, "", rel);
    }
}
```

- [ ] **Step 5: Implement registry**

```rust
// src/doc/registry.rs (above tests)
impl RegistryExtractor {
    pub fn new(ids: IdMatcher) -> RegistryExtractor { RegistryExtractor { ids } }
}

impl Extractor for RegistryExtractor {
    fn extract(&self, rel: &str, text: &str) -> Extraction {
        let mut ex = Extraction::default();
        let file_id = format!("file:{rel}");
        ex.node(NodeKind::File, &file_id, rel, "", rel, 1);
        let Ok(reg) = serde_yaml::from_str::<Registry>(text) else { return ex; };
        for (i, row) in reg.invariants.iter().enumerate() {
            let label = row.statement.lines().next().unwrap_or("").replace("**", "").trim().to_string();
            // Row order is the only line information YAML gives cheaply; good enough for `path:line`.
            ex.node(NodeKind::Invariant, &row.id, &label, row.mechanism.trim(), rel, i as u32 + 1);
            ex.edge(&file_id, &row.id, EdgeKind::Declares, "", rel);
            if let Some(t) = &row.test_ref {
                let target = if t.contains('/') { format!("file:{t}") } else { format!("gate:{t}") };
                ex.edge(&row.id, &target, EdgeKind::Implements, "test_ref", rel);
            }
            if let Some(b) = &row.basis {
                for hit in self.ids.find_all(b) {
                    ex.edge(&row.id, &hit.id, EdgeKind::References, "basis", rel);
                }
            }
        }
        ex
    }
}
```

Compose in `src/doc/mod.rs`:

```rust
pub mod links;
pub mod registry;
pub mod requirements;

use crate::ids::IdMatcher;
use crate::model::{Extraction, Extractor};

pub struct DocExtractor { req: requirements::RequirementScanner }

impl DocExtractor {
    pub fn new(ids: IdMatcher) -> DocExtractor {
        DocExtractor { req: requirements::RequirementScanner::new(ids) }
    }
}

impl Extractor for DocExtractor {
    fn extract(&self, rel: &str, text: &str) -> Extraction {
        let mut ex = self.req.scan(rel, text);
        links::scan(rel, text, &mut ex);
        ex
    }
}
```

`main.rs` `extractors`: `registry: Box::new(doc::registry::RegistryExtractor::new(ids.clone()))` — add `#[derive(Clone)]` to `IdMatcher` (`Regex` is `Clone`).

- [ ] **Step 6: Run tests and rebuild**

Run: `cargo test` then `cargo run --release -- --repo /Users/max/Documents/projects/beauty-crm build`
Expected: `Links` edges ≈417 (`python3 -c` count of `kind == "Links"` in `graph.json`), 20 `Invariant` nodes each with one `Implements` edge.

- [ ] **Step 7: Commit**

```bash
git add src/doc src/ids.rs src/main.rs tests/fixtures/constitution.yaml
git commit -m "feat: markdown links and constitution-style registries"
```

---

### Task 7: Import resolution (relative, `.js`→`.ts`, tsconfig paths, package exports)

**Files:**

- Create: `src/code/mod.rs`, `src/code/imports.rs`
- Modify: `src/main.rs` (`mod code;`)

**Interfaces:**

- Consumes: nothing from extractors; reads `tsconfig.json` files and `package.json` files under `repo`.
- Produces: `Resolver::new(repo: &Path) -> Result<Resolver>`; `Resolver::resolve(&self, from_rel: &str, spec: &str) -> Option<String>` returning the repo-relative path of an existing file, or `None` for packages outside the repo. Resolution order: relative specifier → tsconfig `paths` (root first, then the nearest `tsconfig.json` above `from_rel`) → workspace `package.json` `name` + `exports`. Candidate expansion for a resolved stem: as-is if it exists, then `.ts`, `.tsx`, `/index.ts`, `/index.tsx`; a `.js`/`.jsx`/`.mjs` suffix is stripped first; a `dist/` prefix on an `exports` target is rewritten to `src/` and `.d.ts`/`.js`/`.cjs` to `.ts`.

- [ ] **Step 1: Write the failing tests**

```rust
// src/code/imports.rs
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct Resolver {
    repo: PathBuf,
    /// (directory the tsconfig lives in, pattern → targets), root last so nearer configs win.
    paths: Vec<(String, Vec<(String, Vec<String>)>)>,
    /// package name → (package dir, exports subpath → target)
    packages: BTreeMap<String, (String, BTreeMap<String, String>)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let w = |p: &str, c: &str| {
            let full = d.path().join(p);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, c).unwrap();
        };
        w("tsconfig.json", r#"{ "compilerOptions": { "paths": {
            "@beauty-crm/contracts": ["./packages/contracts/src/index.ts"],
            "@beauty-crm/contracts/*": ["./packages/contracts/src/*"] } } }"#);
        w("packages/contracts/package.json", r#"{ "name": "@beauty-crm/contracts", "exports": {
            ".": { "types": "./dist/index.d.ts", "import": "./dist/index.js" },
            "./business": { "import": "./dist/business.js" } } }"#);
        w("packages/contracts/src/index.ts", "export * from './money.js';\n");
        w("packages/contracts/src/money.ts", "export const asGrosze = 1;\n");
        w("packages/contracts/src/business.ts", "export const b = 1;\n");
        w("packages/ui/tsconfig.json", r#"{ "compilerOptions": { "paths": { "@/*": ["./src/*"] } } }"#);
        w("packages/ui/src/button/index.tsx", "export const Button = 1;\n");
        w("packages/ui/src/app.tsx", "import { Button } from '@/button';\n");
        w("apps/api/src/modules/staff/staff.controller.ts", "");
        w("apps/api/src/shared/audit/index.ts", "");
        d
    }

    #[test]
    fn relative_with_js_suffix_and_index() {
        let d = repo();
        let r = Resolver::new(d.path()).unwrap();
        assert_eq!(r.resolve("apps/api/src/modules/staff/staff.controller.ts", "../../shared/audit/index.js").as_deref(), Some("apps/api/src/shared/audit/index.ts"));
        assert_eq!(r.resolve("apps/api/src/modules/staff/staff.controller.ts", "../../shared/audit").as_deref(), Some("apps/api/src/shared/audit/index.ts"));
        assert_eq!(r.resolve("packages/contracts/src/index.ts", "./money.js").as_deref(), Some("packages/contracts/src/money.ts"));
    }

    #[test]
    fn tsconfig_paths_root_and_nearest() {
        let d = repo();
        let r = Resolver::new(d.path()).unwrap();
        assert_eq!(r.resolve("apps/api/src/x.ts", "@beauty-crm/contracts").as_deref(), Some("packages/contracts/src/index.ts"));
        assert_eq!(r.resolve("apps/api/src/x.ts", "@beauty-crm/contracts/money").as_deref(), Some("packages/contracts/src/money.ts"));
        assert_eq!(r.resolve("packages/ui/src/app.tsx", "@/button").as_deref(), Some("packages/ui/src/button/index.tsx"));
    }

    #[test]
    fn package_exports_map_dist_to_src() {
        let d = repo();
        std::fs::remove_file(d.path().join("tsconfig.json")).unwrap();
        let r = Resolver::new(d.path()).unwrap();
        assert_eq!(r.resolve("apps/api/src/x.ts", "@beauty-crm/contracts/business").as_deref(), Some("packages/contracts/src/business.ts"));
        assert_eq!(r.resolve("apps/api/src/x.ts", "@beauty-crm/contracts").as_deref(), Some("packages/contracts/src/index.ts"));
    }

    #[test]
    fn external_packages_are_none() {
        let d = repo();
        let r = Resolver::new(d.path()).unwrap();
        assert_eq!(r.resolve("apps/api/src/x.ts", "@nestjs/common"), None);
        assert_eq!(r.resolve("apps/api/src/x.ts", "node:fs"), None);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test code::imports` — Expected: compile error.

- [ ] **Step 3: Implement**

`tsconfig.json` in the wild carries comments (`packages/ui/tsconfig.json` does), so the JSON is stripped of `//` line comments and trailing commas before `serde_json` sees it.

```rust
// src/code/imports.rs (above tests)
#[derive(Deserialize, Default)]
struct TsConfig { #[serde(default, rename = "compilerOptions")] compiler_options: CompilerOptions }

#[derive(Deserialize, Default)]
struct CompilerOptions { #[serde(default)] paths: BTreeMap<String, Vec<String>> }

#[derive(Deserialize, Default)]
struct PackageJson { name: Option<String>, #[serde(default)] exports: serde_json::Value }

fn strip_jsonc(text: &str) -> String {
    let no_comments: String = text
        .lines()
        .map(|l| {
            // A `//` inside a string literal would be cut too; tsconfig paths never contain one.
            match l.find("//") { Some(i) if !l[..i].contains('"') || l[..i].matches('"').count() % 2 == 0 => &l[..i], _ => l }
        })
        .collect::<Vec<_>>()
        .join("\n");
    regex::Regex::new(r",(\s*[}\]])").unwrap().replace_all(&no_comments, "$1").into_owned()
}

fn export_target(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(m) => ["types", "import", "default", "require"].iter().find_map(|k| m.get(*k).and_then(export_target)),
        _ => None,
    }
}

impl Resolver {
    pub fn new(repo: &Path) -> Result<Resolver> {
        let mut paths = Vec::new();
        let mut packages = BTreeMap::new();
        for dent in ignore::WalkBuilder::new(repo).hidden(true).git_ignore(true).build().flatten() {
            let p = dent.path();
            let Some(name) = p.file_name().and_then(|n| n.to_str()) else { continue };
            let rel_dir = p.parent().unwrap().strip_prefix(repo).unwrap_or(Path::new("")).to_string_lossy().replace('\\', "/");
            if name == "tsconfig.json" {
                let text = std::fs::read_to_string(p).with_context(|| p.display().to_string())?;
                let cfg: TsConfig = serde_json::from_str(&strip_jsonc(&text)).unwrap_or_default();
                if !cfg.compiler_options.paths.is_empty() {
                    paths.push((rel_dir, cfg.compiler_options.paths.into_iter().collect()));
                }
            } else if name == "package.json" && !rel_dir.contains("node_modules") {
                let text = std::fs::read_to_string(p)?;
                if let Ok(pkg) = serde_json::from_str::<PackageJson>(&text) {
                    if let Some(pkg_name) = pkg.name {
                        let mut map = BTreeMap::new();
                        match &pkg.exports {
                            serde_json::Value::Object(m) => {
                                for (k, v) in m {
                                    if let Some(t) = export_target(v) { map.insert(k.clone(), t); }
                                }
                            }
                            other => { if let Some(t) = export_target(other) { map.insert(".".into(), t); } }
                        }
                        packages.insert(pkg_name, (rel_dir, map));
                    }
                }
            }
        }
        // Deeper tsconfigs first, so the nearest one to the importing file wins.
        paths.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        Ok(Resolver { repo: repo.to_path_buf(), paths, packages })
    }

    fn exists(&self, rel: &str) -> Option<String> {
        let stem = rel.trim_end_matches(".js").trim_end_matches(".jsx").trim_end_matches(".mjs");
        let stem = stem.replace("/dist/", "/src/");
        let stem = stem.strip_suffix(".d.ts").unwrap_or(&stem).to_string();
        let candidates = [
            rel.to_string(), stem.clone(), format!("{stem}.ts"), format!("{stem}.tsx"),
            format!("{stem}/index.ts"), format!("{stem}/index.tsx"),
        ];
        candidates.into_iter().find(|c| self.repo.join(c).is_file())
    }

    pub fn resolve(&self, from_rel: &str, spec: &str) -> Option<String> {
        if spec.starts_with('.') {
            return self.exists(&crate::doc::links::normalise(from_rel, spec));
        }
        if spec.starts_with("node:") {
            return None;
        }
        for (dir, patterns) in &self.paths {
            if !dir.is_empty() && !from_rel.starts_with(&format!("{dir}/")) {
                continue;
            }
            for (pat, targets) in patterns {
                let matched = match pat.strip_suffix('*') {
                    Some(prefix) => spec.strip_prefix(prefix).map(|rest| rest.to_string()),
                    None if pat == spec => Some(String::new()),
                    None => None,
                };
                let Some(rest) = matched else { continue };
                for t in targets {
                    let t = t.replace('*', &rest);
                    let joined = crate::doc::links::normalise(&format!("{dir}/x"), &t);
                    if let Some(hit) = self.exists(&joined) { return Some(hit); }
                }
            }
        }
        for (name, (dir, exports)) in &self.packages {
            let Some(rest) = spec.strip_prefix(name.as_str()) else { continue };
            let sub = if rest.is_empty() { ".".to_string() } else { format!(".{rest}") };
            let Some(t) = exports.get(&sub) else { continue };
            let joined = crate::doc::links::normalise(&format!("{dir}/x"), t);
            if let Some(hit) = self.exists(&joined) { return Some(hit); }
        }
        None
    }
}
```

`src/code/mod.rs` for now: `pub mod imports;`.

- [ ] **Step 4: Run the tests**

Run: `cargo test code::imports` — Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add src/code src/main.rs
git commit -m "feat: resolve TypeScript imports through relative paths, tsconfig paths and package exports"
```

---

### Task 8: Code extractor — files, exports, classes, methods, decorators, imports, re-exports

**Files:**

- Create: `src/code/symbols.rs`, `tests/fixtures/staff.controller.ts`, `tests/fixtures/money.ts`
- Modify: `src/code/mod.rs` (`CodeExtractor`), `src/main.rs`

**Interfaces:**

- Consumes: `Resolver` (Task 7), `Extraction` etc. (Task 4), tree-sitter 0.27.0 + tree-sitter-typescript 0.23.2.
- Produces: `CodeExtractor::new(resolver: Resolver, ids: IdMatcher) -> CodeExtractor` implementing `Extractor`. Node ids: `file:<rel>`, `sym:<rel>::<Name>`, `sym:<rel>::<Class>.<method>`. Edges: `file → sym Declares` (context `export` when exported), `class-sym → method-sym Declares`, `sym → sym:<resolved>::<Base> Extends`, `sym → deco:<Name> DecoratedBy` with the first string-literal argument as context, `file → file:<resolved> Imports` with the imported names joined by `,` as context, `file → file:<resolved> ReExports` for `export * from` / `export { x } from`. The `ids` parameter is used in Task 9.
- tree-sitter facts (verified from registry sources): `Parser::new()`, `parser.set_language(&Language::new(tree_sitter_typescript::LANGUAGE_TYPESCRIPT))`, `parser.parse(&bytes, None) -> Option<Tree>`, `node.kind()`, `node.child_by_field_name("name")`, `node.named_children(&mut node.walk())`, `node.utf8_text(bytes)`, `node.start_position().row`. `.tsx` files use `LANGUAGE_TSX`.

- [ ] **Step 1: Fixtures**

`tests/fixtures/staff.controller.ts`:

```ts
import { Body, Controller, Post } from "@nestjs/common";
import { Audited } from "../../shared/audit/index.js";
import { RequireAction } from "../../shared/rbac/index.js";
import { StaffService } from "./staff.service.js";

// Implements FR-VIS-35 for the salon owner role.
@Controller("staff")
export class StaffController extends BaseController {
  constructor(private readonly service: StaffService) {
    super();
  }

  @Post()
  @RequireAction("staff.manage")
  @Audited({ entityType: "staff_member" })
  async create(@Body() dto: CreateStaffDto) {
    return this.service.create(dto);
  }
}

export function helper(): string {
  return "FR-SEC-21";
}
export const LIMIT = 3;
export interface CreateStaffDto {
  name: string;
}
export type Role = "owner" | "master";
export * from "./staff.types.js";
export { StaffService } from "./staff.service.js";
```

`tests/fixtures/money.ts`:

```ts
/** Money in grosze; see FR-PAY-03 and INV-11. */
export function asGrosze(v: number): number {
  return Math.round(v * 100);
}
export const zero = asGrosze(0);
function internal() {}
```

- [ ] **Step 2: Write the failing tests**

```rust
// src/code/symbols.rs
use crate::code::imports::Resolver;
use crate::model::{EdgeKind, Extraction, NodeKind};
use tree_sitter::{Language, Node, Parser};

pub struct SymbolScanner { resolver: Resolver }

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(rel: &str, fixture: &str) -> Extraction {
        let d = tempfile::tempdir().unwrap();
        for f in ["apps/api/src/shared/audit/index.ts", "apps/api/src/shared/rbac/index.ts", "apps/api/src/modules/staff/staff.service.ts", "apps/api/src/modules/staff/staff.types.ts"] {
            let p = d.path().join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "export {};\n").unwrap();
        }
        let text = std::fs::read_to_string(format!("{}/tests/fixtures/{fixture}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        SymbolScanner::new(Resolver::new(d.path()).unwrap()).scan(rel, &text)
    }

    fn has(ex: &Extraction, s: &str, t: &str, k: EdgeKind, ctx: &str) -> bool {
        ex.edges.iter().any(|e| e.source == s && e.target == t && e.kind == k && e.context == ctx)
    }

    const C: &str = "apps/api/src/modules/staff/staff.controller.ts";

    #[test]
    fn exports_and_class_members() {
        let ex = scan(C, "staff.controller.ts");
        let ids: Vec<&str> = ex.nodes.iter().map(|n| n.id.as_str()).collect();
        for s in ["StaffController", "helper", "LIMIT", "CreateStaffDto", "Role"] {
            assert!(ids.contains(&format!("sym:{C}::{s}").as_str()), "{s}");
        }
        assert!(ids.contains(&format!("sym:{C}::StaffController.create").as_str()));
        let cls = ex.nodes.iter().find(|n| n.id == format!("sym:{C}::StaffController")).unwrap();
        assert_eq!(cls.kind, NodeKind::Symbol);
        assert_eq!(cls.line, 8);
        assert_eq!(cls.label, "StaffController");
        assert!(has(&ex, &format!("file:{C}"), &format!("sym:{C}::StaffController"), EdgeKind::Declares, "export"));
        assert!(has(&ex, &format!("sym:{C}::StaffController"), &format!("sym:{C}::StaffController.create"), EdgeKind::Declares, ""));
    }

    #[test]
    fn decorators_carry_their_first_string_argument() {
        let ex = scan(C, "staff.controller.ts");
        let m = format!("sym:{C}::StaffController.create");
        assert!(has(&ex, &m, "deco:RequireAction", EdgeKind::DecoratedBy, "staff.manage"));
        assert!(has(&ex, &m, "deco:Audited", EdgeKind::DecoratedBy, ""));
        assert!(has(&ex, &m, "deco:Post", EdgeKind::DecoratedBy, ""));
        assert!(has(&ex, &format!("sym:{C}::StaffController"), "deco:Controller", EdgeKind::DecoratedBy, "staff"));
        assert!(ex.nodes.iter().any(|n| n.id == "deco:RequireAction" && n.kind == NodeKind::Symbol));
    }

    #[test]
    fn imports_and_re_exports_resolve_to_files() {
        let ex = scan(C, "staff.controller.ts");
        assert!(has(&ex, &format!("file:{C}"), "file:apps/api/src/shared/rbac/index.ts", EdgeKind::Imports, "RequireAction"));
        assert!(has(&ex, &format!("file:{C}"), "file:apps/api/src/modules/staff/staff.types.ts", EdgeKind::ReExports, "*"));
        assert!(has(&ex, &format!("file:{C}"), "file:apps/api/src/modules/staff/staff.service.ts", EdgeKind::ReExports, "StaffService"));
        assert!(!ex.edges.iter().any(|e| e.target.contains("nestjs")));
    }

    #[test]
    fn extends_targets_a_symbol_in_the_same_file_when_unresolved() {
        let ex = scan(C, "staff.controller.ts");
        assert!(has(&ex, &format!("sym:{C}::StaffController"), &format!("sym:{C}::BaseController"), EdgeKind::Extends, ""));
    }

    #[test]
    fn non_exported_functions_are_declared_without_export_context() {
        let ex = scan("packages/contracts/src/money.ts", "money.ts");
        assert!(has(&ex, "file:packages/contracts/src/money.ts", "sym:packages/contracts/src/money.ts::asGrosze", EdgeKind::Declares, "export"));
        assert!(has(&ex, "file:packages/contracts/src/money.ts", "sym:packages/contracts/src/money.ts::internal", EdgeKind::Declares, ""));
        let n = ex.nodes.iter().find(|n| n.id.ends_with("::asGrosze")).unwrap();
        assert_eq!(n.body, "export function asGrosze(v: number): number { return Math.round(v * 100); }");
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test code::symbols` — Expected: compile error.

- [ ] **Step 4: Implement**

```rust
// src/code/symbols.rs (above tests)
pub fn language_for(rel: &str) -> Language {
    if rel.ends_with(".tsx") { Language::new(tree_sitter_typescript::LANGUAGE_TSX) }
    else { Language::new(tree_sitter_typescript::LANGUAGE_TYPESCRIPT) }
}

pub fn parse(rel: &str, src: &[u8]) -> Option<tree_sitter::Tree> {
    let mut parser = Parser::new();
    parser.set_language(&language_for(rel)).ok()?;
    parser.parse(src, None)
}

fn text<'a>(n: Node, src: &'a [u8]) -> &'a str { n.utf8_text(src).unwrap_or("") }

fn name_of(n: Node, src: &[u8]) -> Option<String> {
    n.child_by_field_name("name").map(|c| text(c, src).to_string())
}

/// First string literal inside a decorator's arguments, without quotes; `""` when none.
fn decorator_arg(call: Node, src: &[u8]) -> String {
    let Some(args) = call.child_by_field_name("arguments") else { return String::new() };
    let mut cur = args.walk();
    for a in args.named_children(&mut cur) {
        if a.kind() == "string" {
            return text(a, src).trim_matches(|c| c == '\'' || c == '"' || c == '`').to_string();
        }
    }
    String::new()
}

impl SymbolScanner {
    pub fn new(resolver: Resolver) -> SymbolScanner { SymbolScanner { resolver } }

    pub fn resolver(&self) -> &Resolver { &self.resolver }

    pub fn scan(&self, rel: &str, source: &str) -> Extraction {
        let mut ex = Extraction::default();
        let file_id = format!("file:{rel}");
        ex.node(NodeKind::File, &file_id, rel, "", rel, 1);
        let src = source.as_bytes();
        let Some(tree) = parse(rel, src) else { return ex };
        let root = tree.root_node();
        let mut cur = root.walk();
        for stmt in root.named_children(&mut cur) {
            match stmt.kind() {
                "export_statement" => self.export(stmt, rel, &file_id, src, &mut ex),
                "import_statement" => self.import(stmt, rel, &file_id, src, &mut ex),
                "function_declaration" | "class_declaration" | "abstract_class_declaration"
                | "interface_declaration" | "type_alias_declaration" | "enum_declaration"
                | "lexical_declaration" | "variable_declaration" => self.declaration(stmt, rel, &file_id, src, false, &mut ex),
                _ => {}
            }
        }
        ex.edges.sort();
        ex.edges.dedup();
        ex
    }

    fn export(&self, stmt: Node, rel: &str, file_id: &str, src: &[u8], ex: &mut Extraction) {
        if let Some(source) = stmt.child_by_field_name("source") {
            // `export * from` / `export { a, b } from`: a barrel edge, not a symbol.
            let spec = text(source, src).trim_matches(|c| c == '\'' || c == '"');
            let Some(target) = self.resolver.resolve(rel, spec) else { return };
            let mut names = Vec::new();
            let mut cur = stmt.walk();
            for c in stmt.named_children(&mut cur) {
                match c.kind() {
                    "export_clause" => {
                        let mut cc = c.walk();
                        for s in c.named_children(&mut cc) {
                            if s.kind() == "export_specifier" {
                                if let Some(n) = name_of(s, src) { names.push(n); }
                            }
                        }
                    }
                    "namespace_export" => names.push("*".into()),
                    _ => {}
                }
            }
            if names.is_empty() { names.push("*".into()); }
            ex.edge(file_id, &format!("file:{target}"), EdgeKind::ReExports, &names.join(","), rel);
            return;
        }
        if let Some(decl) = stmt.child_by_field_name("declaration") {
            let decorators = self.decorators_of(stmt, src);
            let created = self.declaration(decl, rel, file_id, src, true, ex);
            for sym in created {
                for (name, arg) in &decorators {
                    ex.node(NodeKind::Symbol, &format!("deco:{name}"), name, "", rel, stmt.start_position().row as u32 + 1);
                    ex.edge(&sym, &format!("deco:{name}"), EdgeKind::DecoratedBy, arg, rel);
                }
            }
        }
    }

    /// Returns the symbol ids created at top level, so decorators on `export class` attach.
    fn declaration(&self, decl: Node, rel: &str, file_id: &str, src: &[u8], exported: bool, ex: &mut Extraction) -> Vec<String> {
        let ctx = if exported { "export" } else { "" };
        let line = decl.start_position().row as u32 + 1;
        let signature = text(decl, src).lines().next().unwrap_or("").trim().to_string();
        let signature = if exported && !signature.starts_with("export") { format!("export {signature}") } else { signature };
        let mut created = Vec::new();
        let mut declare = |name: &str, ex: &mut Extraction| {
            let id = format!("sym:{rel}::{name}");
            ex.node(NodeKind::Symbol, &id, name, &signature, rel, line);
            ex.edge(file_id, &id, EdgeKind::Declares, ctx, rel);
            created.push(id.clone());
            id
        };
        match decl.kind() {
            "lexical_declaration" | "variable_declaration" => {
                let mut cur = decl.walk();
                for d in decl.named_children(&mut cur) {
                    if d.kind() == "variable_declarator" {
                        if let Some(n) = name_of(d, src) { declare(&n, ex); }
                    }
                }
            }
            "class_declaration" | "abstract_class_declaration" => {
                let Some(name) = name_of(decl, src) else { return created };
                let class_id = declare(&name, ex);
                self.class_body(decl, &class_id, rel, src, ex);
                for (dname, arg) in self.decorators_of(decl, src) {
                    ex.node(NodeKind::Symbol, &format!("deco:{dname}"), &dname, "", rel, line);
                    ex.edge(&class_id, &format!("deco:{dname}"), EdgeKind::DecoratedBy, &arg, rel);
                }
            }
            _ => {
                if let Some(n) = name_of(decl, src) { declare(&n, ex); }
            }
        }
        created
    }

    fn class_body(&self, class: Node, class_id: &str, rel: &str, src: &[u8], ex: &mut Extraction) {
        let mut cur = class.walk();
        for c in class.named_children(&mut cur) {
            if c.kind() == "class_heritage" {
                let mut hc = c.walk();
                for h in c.named_children(&mut hc) {
                    if h.kind() == "extends_clause" {
                        let mut ec = h.walk();
                        if let Some(base) = h.named_children(&mut ec).next() {
                            let base = text(base, src).split('<').next().unwrap_or("").trim();
                            let file = self.import_origin(class, base, rel, src).unwrap_or_else(|| rel.to_string());
                            ex.edge(class_id, &format!("sym:{file}::{base}"), EdgeKind::Extends, "", rel);
                        }
                    }
                }
            }
        }
        let Some(body) = class.child_by_field_name("body") else { return };
        let mut bc = body.walk();
        for m in body.named_children(&mut bc) {
            if !matches!(m.kind(), "method_definition" | "public_field_definition" | "abstract_method_signature") {
                continue;
            }
            let Some(name) = name_of(m, src) else { continue };
            let class_name = class_id.rsplit("::").next().unwrap_or("");
            let id = format!("sym:{rel}::{class_name}.{name}");
            let signature = text(m, src).lines().find(|l| !l.trim_start().starts_with('@')).unwrap_or("").trim().to_string();
            ex.node(NodeKind::Symbol, &id, &format!("{class_name}.{name}"), &signature, rel, m.start_position().row as u32 + 1);
            ex.edge(class_id, &id, EdgeKind::Declares, "", rel);
            for (dname, arg) in self.decorators_of(m, src) {
                ex.node(NodeKind::Symbol, &format!("deco:{dname}"), &dname, "", rel, m.start_position().row as u32 + 1);
                ex.edge(&id, &format!("deco:{dname}"), EdgeKind::DecoratedBy, &arg, rel);
            }
        }
    }

    /// Decorator children of a node: `(name, first string argument)`.
    fn decorators_of(&self, n: Node, src: &[u8]) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut cur = n.walk();
        for c in n.named_children(&mut cur) {
            if c.kind() != "decorator" { continue; }
            let mut dc = c.walk();
            let Some(inner) = c.named_children(&mut dc).next() else { continue };
            match inner.kind() {
                "call_expression" => {
                    let name = inner.child_by_field_name("function").map(|f| text(f, src)).unwrap_or("").to_string();
                    out.push((name, decorator_arg(inner, src)));
                }
                _ => out.push((text(inner, src).to_string(), String::new())),
            }
        }
        out
    }

    /// The file an identifier was imported from, if it was imported at all.
    fn import_origin(&self, any: Node, ident: &str, rel: &str, src: &[u8]) -> Option<String> {
        let mut root = any;
        while let Some(p) = root.parent() { root = p; }
        let mut cur = root.walk();
        for stmt in root.named_children(&mut cur) {
            if stmt.kind() != "import_statement" { continue; }
            if !text(stmt, src).split(|c: char| !c.is_alphanumeric() && c != '_').any(|t| t == ident) { continue; }
            let spec = text(stmt.child_by_field_name("source")?, src).trim_matches(|c| c == '\'' || c == '"');
            return self.resolver.resolve(rel, spec);
        }
        None
    }

    fn import(&self, stmt: Node, rel: &str, file_id: &str, src: &[u8], ex: &mut Extraction) {
        let Some(source) = stmt.child_by_field_name("source") else { return };
        let spec = text(source, src).trim_matches(|c| c == '\'' || c == '"');
        let Some(target) = self.resolver.resolve(rel, spec) else { return };
        let mut names = Vec::new();
        let mut cur = stmt.walk();
        for c in stmt.named_children(&mut cur) {
            if c.kind() != "import_clause" { continue; }
            let mut ic = c.walk();
            for part in c.named_children(&mut ic) {
                match part.kind() {
                    "identifier" => names.push(text(part, src).to_string()),
                    "named_imports" => {
                        let mut nc = part.walk();
                        for s in part.named_children(&mut nc) {
                            if s.kind() == "import_specifier" {
                                if let Some(n) = name_of(s, src) { names.push(n); }
                            }
                        }
                    }
                    "namespace_import" => names.push("*".into()),
                    _ => {}
                }
            }
        }
        ex.edge(file_id, &format!("file:{target}"), EdgeKind::Imports, &names.join(","), rel);
    }
}
```

```rust
// src/code/mod.rs
pub mod imports;
pub mod symbols;

use crate::ids::IdMatcher;
use crate::model::{Extraction, Extractor};

pub struct CodeExtractor { symbols: symbols::SymbolScanner, ids: IdMatcher }

impl CodeExtractor {
    pub fn new(resolver: imports::Resolver, ids: IdMatcher) -> CodeExtractor {
        CodeExtractor { symbols: symbols::SymbolScanner::new(resolver), ids }
    }
}

impl Extractor for CodeExtractor {
    fn extract(&self, rel: &str, text: &str) -> Extraction { self.symbols.scan(rel, text) }
}
```

`main.rs` `extractors` becomes:

```rust
fn extractors(repo: &std::path::Path, cfg: &config::Config) -> anyhow::Result<Extractors> {
    let ids = ids::IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
    let resolver = code::imports::Resolver::new(repo)?;
    Ok(Extractors {
        doc: Box::new(doc::DocExtractor::new(ids.clone())),
        code: Box::new(code::CodeExtractor::new(resolver, ids.clone())),
        registry: Box::new(doc::registry::RegistryExtractor::new(ids)),
    })
}
```

and the call site passes `&extractors(&repo, &cfg)?`.

- [ ] **Step 5: Run tests; if a node kind differs from the fixture's expectation, print the tree**

Run: `cargo test code::symbols` — Expected: 5 passed. If a decorator or heritage assertion fails, dump the S-expression with `println!("{}", tree.root_node().to_sexp())` inside the failing test to see the actual kinds (`class_heritage`/`extends_clause` are the 0.23.2 names; a `decorator` on a class member is a named child of `method_definition`).

- [ ] **Step 6: Probe on beauty-crm**

Run: `cargo run --release -- --repo /Users/max/Documents/projects/beauty-crm build`, then

```bash
python3 -c "
import json; g=json.load(open('/Users/max/Documents/projects/beauty-crm/.repograph/graph.json'))
for s in ['asGrosze','problemDetailsOf','vitestBase']:
    print(s, [n['file'] for n in g['nodes'].values() if n['id'].endswith('::'+s)])
print('symbols', sum(1 for n in g['nodes'].values() if n['kind']=='Symbol'))
print('decorated_by', sum(1 for e in g['edges'] if e['kind']=='DecoratedBy'))"
```

Expected: `packages/contracts/src/money.ts`, `packages/contracts/src/errors.ts`, `packages/config/vitest.base.ts`; `symbols ≥ 1964`; `decorated_by ≈ 385`.

- [ ] **Step 7: Commit**

```bash
git add src/code src/main.rs tests/fixtures
git commit -m "feat: extract TypeScript symbols, decorators, imports and barrels"
```

---

### Task 9: Ids inside comments and string literals

**Files:**

- Create: `src/code/idrefs.rs`
- Modify: `src/code/mod.rs` (`CodeExtractor::extract` runs both scanners)

**Interfaces:**

- Consumes: `parse` (Task 8), `IdMatcher` (Task 2).
- Produces: `idrefs::scan(ids: &IdMatcher, rel: &str, source: &str, ex: &mut Extraction)` — every id inside a `comment`, `string`, or `template_string` node becomes a `References` edge with context `comment` or `string`, from the nearest enclosing named `function_declaration` / `class_declaration` / `method_definition` symbol (`sym:<rel>::<Name>` or `sym:<rel>::<Class>.<method>`), else from `file:<rel>`. This is the 3457-occurrence doc↔code layer.

- [ ] **Step 1: Write the failing tests**

```rust
// src/code/idrefs.rs
use crate::code::symbols::parse;
use crate::ids::IdMatcher;
use crate::model::{EdgeKind, Extraction};
use tree_sitter::Node;

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(rel: &str, src: &str) -> Vec<(String, String, String)> {
        let cfg = crate::config::Config::default();
        let ids = IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
        let mut ex = Extraction::default();
        scan(&ids, rel, src, &mut ex);
        ex.edges.iter().filter(|e| e.kind == EdgeKind::References)
            .map(|e| (e.source.clone(), e.target.clone(), e.context.clone())).collect()
    }

    #[test]
    fn comment_ids_attach_to_the_enclosing_symbol() {
        let r = refs("m.ts", "/** see FR-PAY-03 */\nexport function asGrosze() {}\nclass A {\n  // INV-11 here\n  run() { return 'FR-SEC-21'; }\n}\n");
        assert!(r.contains(&("file:m.ts".into(), "FR-PAY-03".into(), "comment".into())));
        assert!(r.contains(&("sym:m.ts::A".into(), "INV-11".into(), "comment".into())));
        assert!(r.contains(&("sym:m.ts::A.run".into(), "FR-SEC-21".into(), "string".into())));
    }

    #[test]
    fn test_titles_count() {
        let r = refs("t.spec.ts", "it('FR-VIS-35 owner sees templates', () => {});\n");
        assert_eq!(r, vec![("file:t.spec.ts".into(), "FR-VIS-35".into(), "string".into())]);
    }

    #[test]
    fn identifiers_are_not_scanned() {
        assert!(refs("x.ts", "const FR_PAY_22 = 1; const a = FRPAY22;\n").is_empty());
    }

    #[test]
    fn staff_controller_fixture_yields_fr_vis_35() {
        let text = std::fs::read_to_string(format!("{}/tests/fixtures/staff.controller.ts", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let r = refs("c.ts", &text);
        assert!(r.contains(&("file:c.ts".into(), "FR-VIS-35".into(), "comment".into())));
        assert!(r.contains(&("sym:c.ts::helper".into(), "FR-SEC-21".into(), "string".into())));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test code::idrefs` — Expected: compile error, `scan` missing.

- [ ] **Step 3: Implement**

```rust
// src/code/idrefs.rs (above tests)
fn owner(mut n: Node, rel: &str, src: &[u8]) -> String {
    let mut method: Option<String> = None;
    while let Some(p) = n.parent() {
        let name = p.child_by_field_name("name").and_then(|c| c.utf8_text(src).ok()).map(str::to_string);
        match (p.kind(), name) {
            ("method_definition", Some(m)) => method = Some(m),
            ("function_declaration", Some(f)) => return format!("sym:{rel}::{f}"),
            ("class_declaration" | "abstract_class_declaration", Some(c)) => {
                return match method { Some(m) => format!("sym:{rel}::{c}.{m}"), None => format!("sym:{rel}::{c}") };
            }
            _ => {}
        }
        n = p;
    }
    format!("file:{rel}")
}

pub fn scan(ids: &IdMatcher, rel: &str, source: &str, ex: &mut Extraction) {
    let src = source.as_bytes();
    let Some(tree) = parse(rel, src) else { return };
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        let ctx = match n.kind() {
            "comment" => "comment",
            "string" | "template_string" => "string",
            _ => {
                let mut c = n.walk();
                stack.extend(n.named_children(&mut c));
                continue;
            }
        };
        let Ok(t) = n.utf8_text(src) else { continue };
        let hits = ids.find_all(t);
        if hits.is_empty() { continue; }
        let from = owner(n, rel, src);
        for h in hits {
            ex.edge(&from, &h.id, EdgeKind::References, ctx, rel);
        }
    }
}
```

`src/code/mod.rs`:

```rust
pub mod idrefs;

impl Extractor for CodeExtractor {
    fn extract(&self, rel: &str, text: &str) -> Extraction {
        let mut ex = self.symbols.scan(rel, text);
        idrefs::scan(&self.ids, rel, text, &mut ex);
        ex.edges.sort();
        ex.edges.dedup();
        ex
    }
}
```

- [ ] **Step 4: Run tests and measure**

Run: `cargo test code::` — Expected: all passed.
Run: `cargo run --release -- --repo /Users/max/Documents/projects/beauty-crm build` and count `References` edges with context `comment`/`string` whose `file` ends in `.ts`/`.tsx`: expected in the low thousands (census: 3457 occurrences across 621 files, minus duplicates per `(source,target,kind,context)`).

- [ ] **Step 5: Commit**

```bash
git add src/code
git commit -m "feat: link ids quoted in code comments and strings to their requirements"
```

---

### Task 10: Lexical index — tokenizer, stemmers, BM25

**Files:**

- Create: `src/index/mod.rs`, `src/index/lexical.rs`
- Modify: `src/main.rs` (`mod index;`)

**Interfaces:**

- Consumes: `Graph`, `Node` (Task 4).
- Produces: `tokenize(text: &str) -> Vec<String>` (lowercased, split on non-alphanumeric, ≥2 chars, Russian stems for Cyrillic tokens, English stems otherwise, ids like `fr-pay-22` kept whole); `LexicalIndex::build(graph: &Graph) -> LexicalIndex` (documents = every node except `File`, text = `id + label + body`); `LexicalIndex::search(&self, query: &str, k: usize) -> Vec<(String, f32)>` — node ids with BM25 scores, descending. Constants `K1 = 1.2`, `B = 0.75`. The index is rebuilt from the graph on every `ask` (≈4k short documents, tens of milliseconds) — no on-disk lexical state to go stale.
- rust-stemmers 1.2.0 API: `Stemmer::create(Algorithm::Russian)`, `stemmer.stem(&str) -> Cow<str>`.

- [ ] **Step 1: Write the failing tests**

```rust
// src/index/lexical.rs
use crate::model::{Graph, NodeKind};
use rust_stemmers::{Algorithm, Stemmer};
use std::collections::HashMap;

const K1: f32 = 1.2;
const B: f32 = 0.75;

pub struct LexicalIndex {
    ids: Vec<String>,
    lengths: Vec<f32>,
    avg_len: f32,
    /// term → (doc index, term frequency)
    postings: HashMap<String, Vec<(usize, u32)>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Extraction, NodeKind};

    #[test]
    fn russian_inflections_share_a_stem() {
        assert_eq!(tokenize("штрафа"), tokenize("штрафы"));
        assert_eq!(tokenize("отмены"), tokenize("отмена"));
        assert_eq!(tokenize("cancellations"), tokenize("cancellation"));
    }

    #[test]
    fn ids_survive_as_one_token_and_case_folds() {
        assert_eq!(tokenize("См. FR-PAY-22!"), vec!["см".to_string(), "fr-pay-22".to_string()]);
    }

    #[test]
    fn bm25_ranks_the_body_match_first() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "правило отмены", "штраф считается по политике отмены", "a.md", 1);
        e.node(NodeKind::Requirement, "FR-PAY-26", "списание штрафа", "штраф списывается автоматически", "a.md", 9);
        e.node(NodeKind::Requirement, "FR-CAL-40", "коды конфликтов", "словарь кодов", "b.md", 1);
        e.node(NodeKind::File, "file:a.md", "a.md", "", "a.md", 1);
        g.apply(e);
        let idx = LexicalIndex::build(&g);
        let hits = idx.search("политика отмен штрафы", 5);
        assert_eq!(hits[0].0, "FR-PAY-22");
        assert_eq!(hits[1].0, "FR-PAY-26");
        assert_eq!(hits.len(), 2);
        assert!(idx.search("file", 5).is_empty());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test index::lexical` — Expected: compile error.

- [ ] **Step 3: Implement**

```rust
// src/index/lexical.rs (above tests)
fn stemmers() -> &'static (Stemmer, Stemmer) {
    static S: std::sync::OnceLock<(Stemmer, Stemmer)> = std::sync::OnceLock::new();
    S.get_or_init(|| (Stemmer::create(Algorithm::Russian), Stemmer::create(Algorithm::English)))
}

pub fn tokenize(text: &str) -> Vec<String> {
    let (ru, en) = stemmers();
    let lower = text.to_lowercase();
    let mut out = Vec::new();
    // Hyphens stay inside a token so `fr-pay-22` is one term; every other
    // non-alphanumeric byte splits.
    for raw in lower.split(|c: char| !(c.is_alphanumeric() || c == '-')) {
        let t = raw.trim_matches('-');
        if t.chars().count() < 2 { continue; }
        if t.contains('-') || t.chars().any(|c| c.is_ascii_digit()) {
            out.push(t.to_string());
        } else if t.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)) {
            out.push(ru.stem(t).into_owned());
        } else {
            out.push(en.stem(t).into_owned());
        }
    }
    out
}

impl LexicalIndex {
    pub fn build(graph: &Graph) -> LexicalIndex {
        let mut ids = Vec::new();
        let mut lengths = Vec::new();
        let mut postings: HashMap<String, Vec<(usize, u32)>> = HashMap::new();
        for n in graph.nodes.values().filter(|n| n.kind != NodeKind::File) {
            let doc = ids.len();
            ids.push(n.id.clone());
            let toks = tokenize(&format!("{} {} {}", n.id, n.label, n.body));
            lengths.push(toks.len() as f32);
            let mut tf: HashMap<String, u32> = HashMap::new();
            for t in toks { *tf.entry(t).or_default() += 1; }
            for (t, c) in tf { postings.entry(t).or_default().push((doc, c)); }
        }
        let avg_len = if lengths.is_empty() { 1.0 } else { lengths.iter().sum::<f32>() / lengths.len() as f32 };
        LexicalIndex { ids, lengths, avg_len, postings }
    }

    pub fn search(&self, query: &str, k: usize) -> Vec<(String, f32)> {
        let n = self.ids.len() as f32;
        let mut scores: HashMap<usize, f32> = HashMap::new();
        for term in tokenize(query) {
            let Some(list) = self.postings.get(&term) else { continue };
            let idf = ((n - list.len() as f32 + 0.5) / (list.len() as f32 + 0.5) + 1.0).ln();
            for (doc, tf) in list {
                let tf = *tf as f32;
                let norm = K1 * (1.0 - B + B * self.lengths[*doc] / self.avg_len);
                *scores.entry(*doc).or_default() += idf * (tf * (K1 + 1.0)) / (tf + norm);
            }
        }
        let mut ranked: Vec<(String, f32)> = scores.into_iter().map(|(d, s)| (self.ids[d].clone(), s)).collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        ranked.truncate(k);
        ranked
    }
}
```

`src/index/mod.rs`: `pub mod lexical;`.

- [ ] **Step 4: Run the tests**

Run: `cargo test index::` — Expected: 3 passed. If `russian_inflections_share_a_stem` fails on a pair, print both stems; the Snowball Russian algorithm strips noun endings `-а/-ы/-у` and the test pairs are chosen to fall in that rule.

- [ ] **Step 5: Commit**

```bash
git add src/index src/main.rs
git commit -m "feat: in-memory BM25 with Russian and English stemming"
```

---

### Task 11: `ask`, `explain`, `verify` — exact seeds, lexical seeds, 1-hop expansion, rendering

**Files:**

- Create: `src/query.rs`, `src/index/fuse.rs`
- Modify: `src/main.rs` (wire `Ask`, `Explain`, `Verify`)

**Interfaces:**

- Consumes: `Graph`, `Node`, `Edge`, `EdgeKind`, `NodeKind` (Task 4), `LexicalIndex` (Task 10), `IdMatcher` (Task 2), `Store` (Task 4).
- Produces:
  - `fuse::rrf(lists: &[Vec<String>], k: f32) -> Vec<(String, f32)>` — reciprocal rank fusion, `k = 60.0`, descending score, ties by id.
  - `query::Options { seeds: usize, bodies: bool, dense: bool, json: bool }`.
  - `query::Answer { seeds: Vec<Hit>, expanded: Vec<Hit> }`, `Hit { id: String, file: String, line: u32, label: String, score: f32, via: Option<String> }`.
  - `query::ask(graph: &Graph, ids: &IdMatcher, dense: Option<&dyn Fn(&str, usize) -> Vec<String>>, words: &[String], opts: &Options) -> Answer`. Dense is a callback so Task 12 plugs in without changing this signature.
  - `query::render(answer: &Answer, graph: &Graph, opts: &Options) -> String` — text lines `ID  path:line  headline` (two spaces between columns, headline cut to 80 chars); expanded lines are indented two spaces and carry `← <seed id>`; with `bodies`, a seed's body follows it indented four spaces. With `json`, the `Answer` serialised.
  - `query::explain(graph: &Graph, needle: &str) -> Option<String>` — resolves `needle` by exact id, then `sym:*::<needle>`, then case-insensitive label; prints the node line, then edges grouped by kind in the enum's order with `Legacy` last, each as `  <kind> → <target>  [<context>]` (or `←` for inbound).
  - `query::verify(graph: &Graph) -> String` — node and edge counts per kind, count and top 10 of referenced-but-undeclared ids, count of dangling edges.
- Expansion kinds: `References | Implements | Declares | Links | Legacy`, both directions; `File` nodes and `deco:` nodes are never expanded _to_ (they are hubs); neighbours score `seed.score * 0.5`, at most 8 expanded lines, ordered by score then id.
- Exact seeds: a word that `ids.is_id` and exists as a node, or a word equal to a `Symbol` label or the `::<name>` tail of a symbol id, scores `1.0` and precedes every fused seed. If any exact seed exists, lexical/dense seeds fill only the remaining `opts.seeds` slots.

- [ ] **Step 1: Write the failing tests**

```rust
// src/index/fuse.rs
/// Reciprocal rank fusion: each list votes 1/(k + rank) for its members.
pub fn rrf(lists: &[Vec<String>], k: f32) -> Vec<(String, f32)> {
    let mut score: std::collections::HashMap<&str, f32> = std::collections::HashMap::new();
    for list in lists {
        for (rank, id) in list.iter().enumerate() {
            *score.entry(id.as_str()).or_default() += 1.0 / (k + rank as f32 + 1.0);
        }
    }
    let mut out: Vec<(String, f32)> = score.into_iter().map(|(i, s)| (i.to_string(), s)).collect();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_in_both_lists_outranks_a_single_top_hit() {
        let a = vec!["x".to_string(), "y".to_string()];
        let b = vec!["z".to_string(), "y".to_string()];
        let r = rrf(&[a, b], 60.0);
        assert_eq!(r[0].0, "y");
        assert_eq!(r[1].0, "x");
        assert_eq!(r[2].0, "z");
    }

    #[test]
    fn single_list_keeps_its_order() {
        let r = rrf(&[vec!["a".into(), "b".into(), "c".into()]], 60.0);
        assert_eq!(r.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["a", "b", "c"]);
    }
}
```

```rust
// src/query.rs
use crate::ids::IdMatcher;
use crate::index::{fuse, lexical::LexicalIndex};
use crate::model::{EdgeKind, Graph, NodeKind};
use serde::Serialize;

pub struct Options { pub seeds: usize, pub bodies: bool, pub dense: bool, pub json: bool }

#[derive(Debug, Clone, Serialize)]
pub struct Hit { pub id: String, pub file: String, pub line: u32, pub label: String, pub score: f32, pub via: Option<String> }

#[derive(Debug, Default, Serialize)]
pub struct Answer { pub seeds: Vec<Hit>, pub expanded: Vec<Hit> }

const EXPAND: [EdgeKind; 5] = [EdgeKind::References, EdgeKind::Implements, EdgeKind::Declares, EdgeKind::Links, EdgeKind::Legacy];
const MAX_EXPANDED: usize = 8;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EdgeKind, Extraction, NodeKind};

    fn graph() -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "`CancellationPolicy` — правило отмены с числами", "штраф считается по политике отмены (`N-151`)", "docs/06.md", 385);
        e.node(NodeKind::Requirement, "FR-PAY-20", "отмена записи клиентом", "клиент отменяет запись", "docs/06.md", 300);
        e.node(NodeKind::Requirement, "N-151", "never: free-text policies", "", "docs/never.md", 12);
        e.node(NodeKind::Entity, "entity:CancellationPolicy", "CancellationPolicy", "", "docs/06.md", 385);
        e.node(NodeKind::Symbol, "sym:packages/contracts/src/money.ts::asGrosze", "asGrosze", "export function asGrosze()", "packages/contracts/src/money.ts", 2);
        e.node(NodeKind::File, "file:docs/06.md", "docs/06.md", "", "docs/06.md", 1);
        e.edge("FR-PAY-22", "N-151", EdgeKind::References, "body", "docs/06.md");
        e.edge("FR-PAY-22", "entity:CancellationPolicy", EdgeKind::References, "title", "docs/06.md");
        e.edge("file:docs/06.md", "FR-PAY-22", EdgeKind::Declares, "", "docs/06.md");
        e.edge("file:docs/06.md", "FR-PAY-20", EdgeKind::Declares, "", "docs/06.md");
        g.apply(e);
        g
    }

    fn ids() -> IdMatcher {
        let cfg = crate::config::Config::default();
        IdMatcher::new(&cfg.id_families, &cfg.milestone_families)
    }

    fn opts() -> Options { Options { seeds: 5, bodies: false, dense: false, json: false } }

    #[test]
    fn exact_id_wins_and_expands_one_hop() {
        let g = graph();
        let a = ask(&g, &ids(), None, &["FR-PAY-22".to_string()], &opts());
        assert_eq!(a.seeds[0].id, "FR-PAY-22");
        assert_eq!(a.seeds[0].score, 1.0);
        let ex: Vec<&str> = a.expanded.iter().map(|h| h.id.as_str()).collect();
        assert!(ex.contains(&"N-151"));
        assert!(ex.contains(&"entity:CancellationPolicy"));
        assert!(!ex.contains(&"file:docs/06.md"));
        assert_eq!(a.expanded[0].via.as_deref(), Some("FR-PAY-22"));
    }

    #[test]
    fn exact_symbol_name_resolves_to_its_file() {
        let g = graph();
        let a = ask(&g, &ids(), None, &["asGrosze".to_string()], &opts());
        assert_eq!(a.seeds[0].file, "packages/contracts/src/money.ts");
    }

    #[test]
    fn lexical_query_in_russian_finds_the_requirement() {
        let g = graph();
        let a = ask(&g, &ids(), None, &["политика".into(), "отмены".into(), "штраф".into()], &opts());
        assert_eq!(a.seeds[0].id, "FR-PAY-22");
    }

    #[test]
    fn dense_callback_is_fused_when_present() {
        let g = graph();
        let dense = |_q: &str, _k: usize| vec!["FR-PAY-20".to_string()];
        let mut o = opts();
        o.dense = true;
        let a = ask(&g, &ids(), Some(&dense), &["ничего".into(), "похожего".into()], &o);
        assert_eq!(a.seeds[0].id, "FR-PAY-20");
    }

    #[test]
    fn render_shape_is_id_path_line_headline() {
        let g = graph();
        let a = ask(&g, &ids(), None, &["FR-PAY-22".to_string()], &opts());
        let out = render(&a, &g, &opts());
        let first = out.lines().next().unwrap();
        assert!(first.starts_with("FR-PAY-22  docs/06.md:385  `CancellationPolicy`"), "{first}");
        assert!(out.lines().any(|l| l.starts_with("  N-151  docs/never.md:12  ") && l.ends_with("← FR-PAY-22")));
        assert!(out.len() / 4 < 130);
    }

    #[test]
    fn explain_groups_edges_by_kind() {
        let g = graph();
        let out = explain(&g, "CancellationPolicy").unwrap();
        assert!(out.starts_with("entity:CancellationPolicy  docs/06.md:385"));
        assert!(out.contains("References ← FR-PAY-22  [title]"));
        assert!(explain(&g, "nope").is_none());
    }

    #[test]
    fn verify_counts_undeclared_and_dangling() {
        let mut g = graph();
        let mut e = Extraction::default();
        e.edge("FR-PAY-20", "FR-PAY-999", EdgeKind::References, "body", "docs/06.md");
        g.apply(e);
        let out = verify(&g);
        assert!(out.contains("undeclared ids: 1"));
        assert!(out.contains("FR-PAY-999"));
        assert!(out.contains("dangling edges: 1"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test query:: index::fuse` — Expected: compile error.

- [ ] **Step 3: Implement**

```rust
// src/query.rs (above tests)
fn hit(graph: &Graph, id: &str, score: f32, via: Option<&str>) -> Option<Hit> {
    let n = graph.nodes.get(id)?;
    Some(Hit { id: n.id.clone(), file: n.file.clone(), line: n.line, label: n.label.clone(), score, via: via.map(str::to_string) })
}

fn exact_seeds(graph: &Graph, ids: &IdMatcher, words: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for w in words {
        if ids.is_id(w) && graph.nodes.contains_key(w) {
            out.push(w.clone());
            continue;
        }
        let tail = format!("::{w}");
        let mut syms: Vec<&String> = graph.nodes.values()
            .filter(|n| n.kind == NodeKind::Symbol && (n.label == *w || n.id.ends_with(&tail)))
            .map(|n| &n.id).collect();
        syms.sort();
        out.extend(syms.into_iter().cloned());
    }
    out.dedup();
    out
}

pub fn ask(graph: &Graph, ids: &IdMatcher, dense: Option<&dyn Fn(&str, usize) -> Vec<String>>, words: &[String], opts: &Options) -> Answer {
    let query = words.join(" ");
    let mut answer = Answer::default();
    let exact = exact_seeds(graph, ids, words);
    for id in &exact {
        if let Some(h) = hit(graph, id, 1.0, None) { answer.seeds.push(h); }
    }
    let remaining = opts.seeds.saturating_sub(answer.seeds.len());
    if remaining > 0 {
        let lexical: Vec<String> = LexicalIndex::build(graph).search(&query, 20).into_iter().map(|(id, _)| id).collect();
        let mut lists = vec![lexical];
        if opts.dense {
            if let Some(d) = dense { lists.push(d(&query, 20)); }
        }
        for (id, score) in fuse::rrf(&lists, 60.0) {
            if answer.seeds.len() >= opts.seeds { break; }
            if exact.contains(&id) { continue; }
            if let Some(h) = hit(graph, &id, score, None) { answer.seeds.push(h); }
        }
    }

    let seed_ids: Vec<String> = answer.seeds.iter().map(|h| h.id.clone()).collect();
    let mut expanded: Vec<Hit> = Vec::new();
    for seed in &answer.seeds {
        for e in graph.neighbours(&seed.id) {
            if !EXPAND.contains(&e.kind) { continue; }
            let other = if e.source == seed.id { &e.target } else { &e.source };
            if seed_ids.contains(other) || other.starts_with("file:") || other.starts_with("deco:") { continue; }
            if expanded.iter().any(|h| &h.id == other) { continue; }
            if let Some(h) = hit(graph, other, seed.score * 0.5, Some(&seed.id)) { expanded.push(h); }
        }
    }
    expanded.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap().then(a.id.cmp(&b.id)));
    expanded.truncate(MAX_EXPANDED);
    answer.expanded = expanded;
    answer
}

fn headline(label: &str) -> String {
    let mut s: String = label.chars().take(80).collect();
    if label.chars().count() > 80 { s.push('…'); }
    s
}

pub fn render(answer: &Answer, graph: &Graph, opts: &Options) -> String {
    if opts.json {
        return serde_json::to_string_pretty(answer).unwrap();
    }
    let mut out = String::new();
    for h in &answer.seeds {
        out.push_str(&format!("{}  {}:{}  {}\n", h.id, h.file, h.line, headline(&h.label)));
        if opts.bodies {
            if let Some(n) = graph.nodes.get(&h.id) {
                for l in n.body.lines() { out.push_str(&format!("    {l}\n")); }
            }
        }
    }
    for h in &answer.expanded {
        out.push_str(&format!("  {}  {}:{}  {}  ← {}\n", h.id, h.file, h.line, headline(&h.label), h.via.as_deref().unwrap_or("")));
    }
    out
}

fn resolve<'a>(graph: &'a Graph, needle: &str) -> Option<&'a crate::model::Node> {
    if let Some(n) = graph.nodes.get(needle) { return Some(n); }
    let tail = format!("::{needle}");
    let mut c: Vec<&crate::model::Node> = graph.nodes.values().filter(|n| n.id.ends_with(&tail)).collect();
    if c.is_empty() {
        let lower = needle.to_lowercase();
        c = graph.nodes.values().filter(|n| n.label.to_lowercase() == lower).collect();
    }
    c.sort_by(|a, b| a.id.cmp(&b.id));
    c.into_iter().next()
}

pub fn explain(graph: &Graph, needle: &str) -> Option<String> {
    let n = resolve(graph, needle)?;
    let mut out = format!("{}  {}:{}  {:?}  {}\n", n.id, n.file, n.line, n.kind, headline(&n.label));
    if let Some(c) = &n.community { out.push_str(&format!("  community: {c}\n")); }
    let mut edges = graph.neighbours(&n.id);
    edges.sort_by_key(|e| (e.kind == EdgeKind::Legacy, e.kind, e.source.clone(), e.target.clone()));
    for e in edges {
        let (arrow, other) = if e.source == n.id { ("→", &e.target) } else { ("←", &e.source) };
        let ctx = if e.context.is_empty() { String::new() } else { format!("  [{}]", e.context) };
        out.push_str(&format!("  {:?} {arrow} {other}{ctx}\n", e.kind));
    }
    Some(out)
}

pub fn verify(graph: &Graph) -> String {
    use std::collections::BTreeMap;
    let mut nodes: BTreeMap<String, usize> = BTreeMap::new();
    for n in graph.nodes.values() { *nodes.entry(format!("{:?}", n.kind)).or_default() += 1; }
    let mut edges: BTreeMap<String, usize> = BTreeMap::new();
    for e in &graph.edges { *edges.entry(format!("{:?}", e.kind)).or_default() += 1; }
    let dangling = graph.dangling();
    let mut undeclared: Vec<&str> = dangling.iter()
        .filter(|e| !e.target.contains(':'))
        .map(|e| e.target.as_str()).collect();
    undeclared.sort();
    undeclared.dedup();
    let mut out = String::new();
    out.push_str(&format!("nodes: {}  {:?}\n", graph.nodes.len(), nodes));
    out.push_str(&format!("edges: {}  {:?}\n", graph.edges.len(), edges));
    out.push_str(&format!("dangling edges: {}\n", dangling.len()));
    out.push_str(&format!("undeclared ids: {}  {}\n", undeclared.len(), undeclared.iter().take(10).cloned().collect::<Vec<_>>().join(" ")));
    out
}
```

`src/index/mod.rs`: `pub mod fuse; pub mod lexical;`.

`main.rs` arms:

```rust
Cmd::Ask { words, json, seeds, bodies, no_dense } => {
    let (graph, _) = store::Store::new(&repo).load()?;
    let ids = ids::IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
    let opts = query::Options { seeds, bodies, dense: !no_dense, json };
    let answer = query::ask(&graph, &ids, None, &words, &opts);
    print!("{}", query::render(&answer, &graph, &opts));
    Ok(())
}
Cmd::Explain { node } => {
    let (graph, _) = store::Store::new(&repo).load()?;
    match query::explain(&graph, &node) {
        Some(s) => { print!("{s}"); Ok(()) }
        None => anyhow::bail!("no node matches {node}"),
    }
}
Cmd::Verify => {
    let (graph, _) = store::Store::new(&repo).load()?;
    print!("{}", query::verify(&graph));
    if graph.nodes.is_empty() { anyhow::bail!("graph is empty — run `repograph build`"); }
    Ok(())
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test` — Expected: all green.

- [ ] **Step 5: Parity spot-checks on beauty-crm**

```bash
R=/Users/max/Documents/projects/beauty-crm
cargo run --release -- --repo $R build
cargo run --release -- --repo $R ask cancellation          # expect FR-PAY-20, FR-PAY-22, N-151 among the lines
cargo run --release -- --repo $R ask рабочие часы мастера   # expect FR-DM-101 (ScheduleException)
cargo run --release -- --repo $R ask расход виден салону    # expect FR-AI-138
cargo run --release -- --repo $R explain FR-PAY-22
cargo run --release -- --repo $R verify                     # undeclared ids: 7
```

Each `ask` output must be under 130 tokens (`| wc -c`, divide by 4).

- [ ] **Step 6: Commit**

```bash
git add src/query.rs src/index src/main.rs
git commit -m "feat: ask, explain and verify over exact and lexical seeds with one-hop expansion"
```

---

### Task 12: Dense retrieval — fastembed `MultilingualE5Small`, incremental vectors, fused into `ask`

**Files:**

- Create: `src/index/dense.rs`
- Modify: `src/index/mod.rs`, `src/main.rs` (global `--no-dense`; `build`/`update` embed changed nodes; `ask` passes the dense callback)

**Interfaces:**

- Consumes: `Graph`, `Store::write_atomic`/`read_bytes` (Task 4), `query::ask` dense callback (Task 11).
- Produces: `DenseIndex { ids: Vec<String>, hashes: Vec<String>, dim: usize, vectors: Vec<f32> }`; `DenseIndex::load(store: &Store) -> Result<DenseIndex>` (empty when absent); `DenseIndex::save(&self, store: &Store) -> Result<()>` (`vectors.json` = ids+hashes+dim, `vectors.f32` = little-endian floats, both atomic); `DenseIndex::sync(&mut self, graph: &Graph, embed: &mut dyn FnMut(&[String]) -> Result<Vec<Vec<f32>>>) -> Result<usize>` — drops vectors for ids no longer in the graph, embeds nodes whose passage hash changed, returns how many were embedded; `DenseIndex::search(&self, query_vec: &[f32], k: usize) -> Vec<String>` (cosine, descending); `Embedder::open() -> Result<Embedder>` wrapping `fastembed::TextEmbedding`, `Embedder::passages(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>>` (prefix `passage: `), `Embedder::query(&mut self, text: &str) -> Result<Vec<f32>>` (prefix `query: `). The prefixes are the `intfloat/multilingual-e5-small` model-card convention.
- Passage text per node: `label + "\n" + body`, hash = blake3 of that text. `File` nodes are not embedded.
- fastembed 6.0.2 API (verified): `TextEmbedding::try_new(TextInitOptions::new(EmbeddingModel::MultilingualE5Small).with_show_download_progress(true).with_max_length(256))`, `embed(&mut self, texts: impl AsRef<[S]>, batch_size: Option<usize>) -> Result<Vec<Vec<f32>>>`. The first call downloads ≈450 MB into the fastembed cache; later calls are offline.
- The `no_dense` flag moves from `Ask` to a global `--no-dense` on `Cli` so `build`/`update` honour it too. When the model cannot be opened (offline, first run) the command prints `dense: model unavailable, continuing lexical-only` to stderr and proceeds.

- [ ] **Step 1: Write the failing tests (no model needed)**

```rust
// src/index/dense.rs
use crate::model::{Graph, NodeKind};
use crate::store::Store;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DenseIndex {
    pub ids: Vec<String>,
    pub hashes: Vec<String>,
    pub dim: usize,
    #[serde(skip)] pub vectors: Vec<f32>,
}

pub struct Embedder { model: fastembed::TextEmbedding }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Extraction, NodeKind};

    fn graph(body22: &str) -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "отмена", body22, "a.md", 1);
        e.node(NodeKind::Requirement, "FR-PAY-26", "штраф", "списание", "a.md", 9);
        e.node(NodeKind::File, "file:a.md", "a.md", "", "a.md", 1);
        g.apply(e);
        g
    }

    /// A stand-in embedder: a 3-d vector from the first three bytes, so tests are deterministic.
    fn fake(texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|t| {
            let b = t.as_bytes();
            vec![b[0] as f32, b.get(1).copied().unwrap_or(0) as f32, b.get(2).copied().unwrap_or(0) as f32]
        }).collect())
    }

    #[test]
    fn sync_embeds_only_changed_nodes_and_drops_removed_ones() {
        let mut idx = DenseIndex::default();
        assert_eq!(idx.sync(&graph("политика"), &mut fake).unwrap(), 2);
        assert_eq!(idx.ids.len(), 2);
        assert_eq!(idx.sync(&graph("политика"), &mut fake).unwrap(), 0);
        assert_eq!(idx.sync(&graph("другое"), &mut fake).unwrap(), 1);
        let mut g = graph("другое");
        g.remove_file("a.md");
        assert_eq!(idx.sync(&g, &mut fake).unwrap(), 0);
        assert!(idx.ids.is_empty() && idx.vectors.is_empty());
    }

    #[test]
    fn search_is_cosine_descending() {
        let mut idx = DenseIndex::default();
        idx.sync(&graph("x"), &mut fake).unwrap();
        let q = fake(&["штраф".to_string()]).unwrap().remove(0);
        assert_eq!(idx.search(&q, 2)[0], "FR-PAY-26");
    }

    #[test]
    fn round_trips_through_the_store() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        let mut idx = DenseIndex::default();
        idx.sync(&graph("x"), &mut fake).unwrap();
        idx.save(&store).unwrap();
        let back = DenseIndex::load(&store).unwrap();
        assert_eq!(back.ids, idx.ids);
        assert_eq!(back.vectors, idx.vectors);
        assert_eq!(back.dim, 3);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test index::dense` — Expected: compile error.

- [ ] **Step 3: Implement**

```rust
// src/index/dense.rs (above tests)
fn passage(n: &crate::model::Node) -> String { format!("{}\n{}", n.label, n.body) }

fn normalise(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 { for x in v { *x /= norm; } }
}

impl DenseIndex {
    pub fn load(store: &Store) -> Result<DenseIndex> {
        let Some(meta) = store.read_bytes("vectors.json")? else { return Ok(DenseIndex::default()) };
        let mut idx: DenseIndex = serde_json::from_slice(&meta).context("vectors.json")?;
        let raw = store.read_bytes("vectors.f32")?.unwrap_or_default();
        idx.vectors = raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        if idx.vectors.len() != idx.ids.len() * idx.dim {
            // A torn pair of files is treated as no index at all; the next sync rebuilds it.
            return Ok(DenseIndex::default());
        }
        Ok(idx)
    }

    pub fn save(&self, store: &Store) -> Result<()> {
        let mut raw = Vec::with_capacity(self.vectors.len() * 4);
        for x in &self.vectors { raw.extend_from_slice(&x.to_le_bytes()); }
        store.write_atomic("vectors.f32", &raw)?;
        store.write_atomic("vectors.json", &serde_json::to_vec(self)?)
    }

    pub fn sync(&mut self, graph: &Graph, embed: &mut dyn FnMut(&[String]) -> Result<Vec<Vec<f32>>>) -> Result<usize> {
        let mut keep_ids = Vec::new();
        let mut keep_hashes = Vec::new();
        let mut keep_vecs: Vec<f32> = Vec::new();
        let mut todo_ids = Vec::new();
        let mut todo_texts = Vec::new();
        let mut todo_hashes = Vec::new();
        let old: std::collections::HashMap<&str, (usize, &str)> =
            self.ids.iter().enumerate().map(|(i, id)| (id.as_str(), (i, self.hashes[i].as_str()))).collect();
        for n in graph.nodes.values().filter(|n| n.kind != NodeKind::File) {
            let text = passage(n);
            let hash = blake3::hash(text.as_bytes()).to_hex().to_string();
            match old.get(n.id.as_str()) {
                Some((i, h)) if *h == hash && self.dim > 0 => {
                    keep_ids.push(n.id.clone());
                    keep_hashes.push(hash);
                    keep_vecs.extend_from_slice(&self.vectors[i * self.dim..(i + 1) * self.dim]);
                }
                _ => { todo_ids.push(n.id.clone()); todo_texts.push(text); todo_hashes.push(hash); }
            }
        }
        let embedded = todo_ids.len();
        if embedded > 0 {
            let mut vecs = embed(&todo_texts)?;
            for v in vecs.iter_mut() { normalise(v); }
            self.dim = vecs.first().map(|v| v.len()).unwrap_or(self.dim);
            for (id, (hash, v)) in todo_ids.into_iter().zip(todo_hashes.into_iter().zip(vecs)) {
                keep_ids.push(id);
                keep_hashes.push(hash);
                keep_vecs.extend_from_slice(&v);
            }
        }
        self.ids = keep_ids;
        self.hashes = keep_hashes;
        self.vectors = keep_vecs;
        if self.ids.is_empty() { self.dim = 0; }
        Ok(embedded)
    }

    pub fn search(&self, query: &[f32], k: usize) -> Vec<String> {
        if self.dim == 0 || query.len() != self.dim { return Vec::new(); }
        let mut q = query.to_vec();
        normalise(&mut q);
        let mut scored: Vec<(f32, &str)> = self.ids.iter().enumerate().map(|(i, id)| {
            let v = &self.vectors[i * self.dim..(i + 1) * self.dim];
            (v.iter().zip(&q).map(|(a, b)| a * b).sum::<f32>(), id.as_str())
        }).collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(b.1)));
        scored.into_iter().take(k).map(|(_, id)| id.to_string()).collect()
    }
}

impl Embedder {
    pub fn open() -> Result<Embedder> {
        use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
        let opts = TextInitOptions::new(EmbeddingModel::MultilingualE5Small)
            .with_show_download_progress(true)
            .with_max_length(256);
        Ok(Embedder { model: TextEmbedding::try_new(opts).context("open embedding model")? })
    }

    pub fn passages(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let prefixed: Vec<String> = texts.iter().map(|t| format!("passage: {t}")).collect();
        Ok(self.model.embed(&prefixed, Some(64))?)
    }

    pub fn query(&mut self, text: &str) -> Result<Vec<f32>> {
        Ok(self.model.embed(&[format!("query: {text}")], None)?.remove(0))
    }
}
```

`src/index/mod.rs`: `pub mod dense; pub mod fuse; pub mod lexical;`.

- [ ] **Step 4: Wire into main**

```rust
// src/main.rs — Cli gains a global flag; Ask loses its own.
#[arg(long, global = true)]
no_dense: bool,

fn open_embedder(no_dense: bool) -> Option<index::dense::Embedder> {
    if no_dense { return None; }
    match index::dense::Embedder::open() {
        Ok(e) => Some(e),
        Err(err) => { eprintln!("dense: model unavailable, continuing lexical-only ({err:#})"); None }
    }
}

// Build | Update arm, after run_update:
if let Some(mut emb) = open_embedder(cli.no_dense) {
    let store = store::Store::new(&repo);
    let (graph, _) = store.load()?;
    let mut dense = index::dense::DenseIndex::load(&store)?;
    let t = std::time::Instant::now();
    let n = dense.sync(&graph, &mut |texts| emb.passages(texts))?;
    dense.save(&store)?;
    println!("dense: embedded {n} nodes in {:.1}s", t.elapsed().as_secs_f32());
}

// Ask arm:
let store = store::Store::new(&repo);
let (graph, _) = store.load()?;
let dense_idx = index::dense::DenseIndex::load(&store)?;
let embedder = std::cell::RefCell::new(open_embedder(cli.no_dense));
let dense_fn = |q: &str, k: usize| -> Vec<String> {
    let mut e = embedder.borrow_mut();
    match e.as_mut().and_then(|e| e.query(q).ok()) { Some(v) => dense_idx.search(&v, k), None => Vec::new() }
};
let opts = query::Options { seeds, bodies, dense: !cli.no_dense && !dense_idx.ids.is_empty(), json };
let answer = query::ask(&graph, &ids, Some(&dense_fn), &words, &opts);
```

- [ ] **Step 5: Run tests, then the one-time embedding pass on beauty-crm**

Run: `cargo test` — Expected: all green (the dense tests never open the model).

Run: `time cargo run --release -- --repo /Users/max/Documents/projects/beauty-crm build`
Expected: model download once (progress bar), then `dense: embedded ~4000 nodes in <120s` on the M3 Pro. Record the time. Run `update` again: `dense: embedded 0 nodes`.

Run the paraphrase probe by hand before the bench exists:

```bash
R=/Users/max/Documents/projects/beauty-crm
for q in "экспорт данных для налоговой отчётности" "как хранится код доступа сотрудника" "выявление накрутки бонусов" "прошлые визиты клиента какие услуги делали"; do
  echo "== $q"; cargo run --release -q -- --repo $R ask $q | head -6
done
```

Expected: FR-PAY-104, FR-SEC-21, FR-TOOL-22, FR-SVC-39 appear in their block. If fewer than three of four appear, try `--seeds 8` before touching the model; if still short, switch `EmbeddingModel::MultilingualE5Small` to `MultilingualE5Base` (the fallback named in the spec) and re-run `build`.

- [ ] **Step 6: Commit**

```bash
git add src/index src/main.rs
git commit -m "feat: local multilingual embeddings fused with lexical seeds"
```

---

### Task 13: `import-legacy` — freeze a graphify graph's model-only edges

**Files:**

- Create: `src/legacy.rs`, `tests/fixtures/graphify-graph.json`
- Modify: `src/main.rs` (`mod legacy;`, wire `ImportLegacy`)

**Interfaces:**

- Consumes: `Graph`, `Extraction`, `NodeKind::LegacyConcept`, `EdgeKind::Legacy`, `IdMatcher`, `Store`.
- Produces: `legacy::import(graph: &mut Graph, ids: &IdMatcher, json: &str) -> Result<Report { edges_seen: usize, resolved_both: usize, resolved_one: usize, concepts_created: usize }>`. graphify shape (measured on beauty-crm's `graph.json`): top-level `nodes` (fields `id`, `label`, `_origin`, `source_file`, `community_name`) and `links` (fields `source`, `target`, `relation`, `context`, `_origin`). A node is _semantic_ when `_origin != "ast"`. An edge is imported when both endpoints are semantic. Endpoint resolution: (1) first strict id inside `label` that is a node in `graph`; (2) a node whose `file` basename equals the basename of `source_file` and whose label equals `label` case-insensitively; (3) a `LegacyConcept` node `legacy:<graphify id>` with the label, `file` = `source_file`, `line` 0. `community_name` is copied onto the resolved node's `community` when it is empty. Every imported node and edge carries `file = "legacy:graphify"`, a path the walker never yields, so `update` never removes or re-extracts them. Idempotent: importing twice adds nothing (set semantics on `Edge`, id-keyed nodes).

- [ ] **Step 1: Fixture**

`tests/fixtures/graphify-graph.json`:

```json
{
  "nodes": [
    {
      "id": "fr_pay_22",
      "label": "FR-PAY-22 CancellationPolicy",
      "source_file": "06-payments.md",
      "community_name": "Payments core"
    },
    {
      "id": "n_151",
      "label": "N-151",
      "source_file": "never-list.md",
      "community_name": "Never list"
    },
    {
      "id": "refund_flow",
      "label": "Refund flow",
      "source_file": "06-payments.md",
      "community_name": "Payments core"
    },
    { "id": "ghost", "label": "Something unrelated", "source_file": "99-x.md" },
    {
      "id": "ast_sym",
      "label": "asGrosze",
      "_origin": "ast",
      "source_file": "money.ts"
    }
  ],
  "links": [
    { "source": "fr_pay_22", "target": "n_151", "relation": "references" },
    {
      "source": "fr_pay_22",
      "target": "refund_flow",
      "relation": "conceptually_related_to"
    },
    { "source": "ghost", "target": "refund_flow", "relation": "cites" },
    { "source": "fr_pay_22", "target": "ast_sym", "relation": "implements" }
  ]
}
```

- [ ] **Step 2: Write the failing tests**

```rust
// src/legacy.rs
use crate::ids::IdMatcher;
use crate::model::{EdgeKind, Extraction, Graph, NodeKind};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

pub const LEGACY_FILE: &str = "legacy:graphify";

#[derive(Deserialize)]
struct GNode { id: String, #[serde(default)] label: String, #[serde(default)] _origin: Option<String>, #[serde(default)] source_file: String, #[serde(default)] community_name: Option<String> }

#[derive(Deserialize)]
struct GEdge { source: String, target: String, #[serde(default)] relation: String, #[serde(default)] context: Option<String> }

#[derive(Deserialize)]
struct GGraph { nodes: Vec<GNode>, links: Vec<GEdge> }

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report { pub edges_seen: usize, pub resolved_both: usize, pub resolved_one: usize, pub concepts_created: usize }

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "CancellationPolicy", "", "docs/prd/06-payments.md", 385);
        e.node(NodeKind::Requirement, "N-151", "no free-text policies", "", "docs/never-list.md", 3);
        e.node(NodeKind::Requirement, "FR-TOOL-39", "Refund flow", "", "docs/prd/06-payments.md", 500);
        g.apply(e);
        g
    }

    fn run(g: &mut Graph) -> Report {
        let cfg = crate::config::Config::default();
        let ids = IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
        let json = std::fs::read_to_string(format!("{}/tests/fixtures/graphify-graph.json", env!("CARGO_MANIFEST_DIR"))).unwrap();
        import(g, &ids, &json).unwrap()
    }

    #[test]
    fn resolves_by_id_then_label_then_creates_concepts() {
        let mut g = base();
        let r = run(&mut g);
        assert_eq!(r, Report { edges_seen: 3, resolved_both: 2, resolved_one: 1, concepts_created: 1 });
        assert!(g.edges.iter().any(|e| e.source == "FR-PAY-22" && e.target == "N-151" && e.kind == EdgeKind::Legacy && e.context == "references"));
        assert!(g.edges.iter().any(|e| e.source == "FR-PAY-22" && e.target == "FR-TOOL-39" && e.context == "conceptually_related_to"));
        let ghost = &g.nodes["legacy:ghost"];
        assert_eq!(ghost.kind, NodeKind::LegacyConcept);
        assert_eq!(ghost.file, "99-x.md");
        assert_eq!(g.nodes["FR-PAY-22"].community.as_deref(), Some("Payments core"));
        assert!(!g.edges.iter().any(|e| e.target.contains("ast_sym")));
    }

    #[test]
    fn import_is_idempotent_and_survives_update_removal() {
        let mut g = base();
        run(&mut g);
        let (n, e) = (g.nodes.len(), g.edges.len());
        run(&mut g);
        assert_eq!((g.nodes.len(), g.edges.len()), (n, e));
        g.remove_file("docs/prd/06-payments.md");
        assert!(g.nodes.contains_key("legacy:ghost"));
        assert!(g.edges.iter().any(|e| e.kind == EdgeKind::Legacy));
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test legacy::` — Expected: compile error.

- [ ] **Step 4: Implement**

```rust
// src/legacy.rs (above tests)
fn basename(p: &str) -> &str { p.rsplit('/').next().unwrap_or(p) }

pub fn import(graph: &mut Graph, ids: &IdMatcher, json: &str) -> Result<Report> {
    let g: GGraph = serde_json::from_str(json).context("graphify graph.json")?;
    let semantic: HashMap<&str, &GNode> = g.nodes.iter().filter(|n| n._origin.as_deref() != Some("ast")).map(|n| (n.id.as_str(), n)).collect();
    let by_label: HashMap<(String, String), String> = graph.nodes.values()
        .map(|n| ((basename(&n.file).to_string(), n.label.to_lowercase()), n.id.clone()))
        .collect();

    let mut resolved: HashMap<&str, (String, bool)> = HashMap::new();
    let mut ex = Extraction::default();
    let mut report = Report::default();
    let mut resolve = |gn: &GNode, ex: &mut Extraction, report: &mut Report| -> (String, bool) {
        if let Some(hit) = ids.find_all(&gn.label).into_iter().find(|h| graph.nodes.contains_key(&h.id)) {
            return (hit.id, true);
        }
        if let Some(id) = by_label.get(&(basename(&gn.source_file).to_string(), gn.label.to_lowercase())) {
            return (id.clone(), true);
        }
        let id = format!("legacy:{}", gn.id);
        if !graph.nodes.contains_key(&id) && !ex.nodes.iter().any(|n| n.id == id) {
            report.concepts_created += 1;
            ex.node(NodeKind::LegacyConcept, &id, &gn.label, "", &gn.source_file, 0);
            if let Some(n) = ex.nodes.last_mut() {
                n.files = std::collections::BTreeSet::from([LEGACY_FILE.to_string()]);
                n.community = gn.community_name.clone();
            }
        }
        (id, false)
    };

    for e in &g.links {
        let (Some(s), Some(t)) = (semantic.get(e.source.as_str()), semantic.get(e.target.as_str())) else { continue };
        report.edges_seen += 1;
        let (sid, s_real) = resolved.entry(s.id.as_str()).or_insert_with(|| resolve(s, &mut ex, &mut report)).clone();
        let (tid, t_real) = resolved.entry(t.id.as_str()).or_insert_with(|| resolve(t, &mut ex, &mut report)).clone();
        match (s_real, t_real) {
            (true, true) => report.resolved_both += 1,
            (false, false) => {}
            _ => report.resolved_one += 1,
        }
        let ctx = match &e.context { Some(c) if !c.is_empty() => format!("{}: {c}", e.relation), _ => e.relation.clone() };
        ex.edge(&sid, &tid, EdgeKind::Legacy, &ctx, LEGACY_FILE);
    }

    for (gid, (id, real)) in &resolved {
        if *real {
            if let (Some(n), Some(gn)) = (graph.nodes.get_mut(id), semantic.get(gid)) {
                if n.community.is_none() { n.community = gn.community_name.clone(); }
            }
        }
    }
    graph.apply(ex);
    Ok(report)
}
```

The closure borrows `graph` immutably while `resolve` runs and mutably afterwards; if the borrow checker objects, collect `resolved` in a first loop over all semantic endpoints, drop the closure, then apply community names and edges in a second loop — same behaviour, two passes.

`main.rs` arm:

```rust
Cmd::ImportLegacy { graph_json } => {
    let store = store::Store::new(&repo);
    let (mut graph, manifest) = store.load()?;
    let ids = ids::IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
    let text = std::fs::read_to_string(&graph_json)?;
    let r = legacy::import(&mut graph, &ids, &text)?;
    store.save(&graph, &manifest)?;
    println!("legacy: {} edges, {} both endpoints resolved, {} one, {} concepts created", r.edges_seen, r.resolved_both, r.resolved_one, r.concepts_created);
    Ok(())
}
```

- [ ] **Step 5: Run tests and the real import**

Run: `cargo test legacy::` — Expected: 2 passed.

Run: `cargo run --release -- --repo /Users/max/Documents/projects/beauty-crm import-legacy /Users/max/Documents/projects/beauty-crm/graphify-out/graph.json`
Expected: `legacy: 20415 edges, …`. The spec gate is ≥90% of edges with both endpoints resolved to real nodes; whatever the number is, it is printed, and `explain FR-PAY-22` now lists `Legacy` edges last. Then `update` twice: the legacy layer is untouched (`nodes`/`edges` counts stable).

- [ ] **Step 6: Commit**

```bash
git add src/legacy.rs src/main.rs tests/fixtures/graphify-graph.json
git commit -m "feat: import a graphify graph's model-only edges as a frozen legacy layer"
```

---

### Task 14: Bench — the 41 recorded cases with floors

**Files:**

- Create: `bench/cases.jsonl`, `src/bench.rs`
- Modify: `src/main.rs` (`mod bench;`, wire `Bench`)

**Interfaces:**

- Consumes: `query::ask`/`render` (Task 11), `DenseIndex`/`Embedder` (Task 12), `Store`, `IdMatcher`.
- Produces: `bench::run(repo: &Path, cfg: &Config, cases: &Path, no_dense: bool) -> Result<bool>` — prints one line per case (`HIT`/`miss`, tokens), then a summary and returns `true` only when every floor holds: keyword 24/24; paraphrase ≥7 (`no_dense`) or ≥12 (dense); code 3/3; p90 tokens ≤130. A case hits when `expect` equals the id of any seed or expanded hit, or — for `code` cases — when a seed's `file` equals `expect`. Tokens = rendered text length / 4. `REPOGRAPH_BENCH_REPO` overrides `--repo` for the bench so CI can point at a checkout.

- [ ] **Step 1: The cases file**

`bench/cases.jsonl` (one JSON object per line):

```jsonl
{"kind":"keyword","q":"расход виден салону","expect":"FR-AI-138"}
{"kind":"keyword","q":"арендатор отдельный контур","expect":"FR-VIS-80"}
{"kind":"keyword","q":"только нативном приложении","expect":"FR-PAY-110"}
{"kind":"keyword","q":"панели отражающие обещания","expect":"FR-APP-64"}
{"kind":"keyword","q":"изменение любой позиции","expect":"FR-PAY-133"}
{"kind":"keyword","q":"групповое занятие кассе","expect":"FR-PAY-54"}
{"kind":"keyword","q":"награда выдаётся против","expect":"FR-TOOL-18"}
{"kind":"keyword","q":"паритет поверхностей","expect":"FR-WH-05"}
{"kind":"keyword","q":"место содержимое занято","expect":"FR-APP-02"}
{"kind":"keyword","q":"знаний салона ассистента","expect":"FR-AI-145"}
{"kind":"keyword","q":"зачитывается автоматически","expect":"FR-PAY-28"}
{"kind":"keyword","q":"сигнал блокирует работу","expect":"FR-VIS-55"}
{"kind":"keyword","q":"денежные конфликты поведение","expect":"FR-PAY-143"}
{"kind":"keyword","q":"черновик виден черновик","expect":"FR-SVC-04"}
{"kind":"keyword","q":"режим тренировки сотрудника","expect":"FR-LIFE-29"}
{"kind":"keyword","q":"списание штрафа происходит","expect":"FR-PAY-26"}
{"kind":"keyword","q":"шаблоны ролей салона","expect":"FR-VIS-35"}
{"kind":"keyword","q":"экономика аренды","expect":"FR-PAY-82"}
{"kind":"keyword","q":"только клиентских поверхностях","expect":"FR-APP-67"}
{"kind":"keyword","q":"единый медиа","expect":"FR-SVC-50"}
{"kind":"keyword","q":"внутренняя система рассмотрения","expect":"FR-SEC-44"}
{"kind":"keyword","q":"арендатор уходит своей","expect":"FR-VIS-85"}
{"kind":"keyword","q":"отчёты склада","expect":"FR-WH-53"}
{"kind":"keyword","q":"подсказка расписанию двигает","expect":"FR-AI-97"}
{"kind":"paraphrase","q":"экспорт данных для налоговой отчётности","expect":"FR-PAY-104"}
{"kind":"paraphrase","q":"перечень незыблемых требований продукта","expect":"FR-VIS-01"}
{"kind":"paraphrase","q":"кто получатель платежа записан в документе","expect":"FR-PAY-03"}
{"kind":"paraphrase","q":"как хранится код доступа сотрудника","expect":"FR-SEC-21"}
{"kind":"paraphrase","q":"сколько времени держим записи кто решает","expect":"FR-AI-102"}
{"kind":"paraphrase","q":"создать похожую услугу на основе существующей","expect":"FR-SVC-15"}
{"kind":"paraphrase","q":"состояния задачи обратного звонка","expect":"FR-AI-66"}
{"kind":"paraphrase","q":"удаление согласия клиента через сторонний мессенджер","expect":"FR-VIS-76"}
{"kind":"paraphrase","q":"клиент должен сразу понять что говорит робот","expect":"FR-AI-21"}
{"kind":"paraphrase","q":"выявление накрутки бонусов","expect":"FR-TOOL-22"}
{"kind":"paraphrase","q":"как клиент получает фискальный документ","expect":"FR-PAY-49"}
{"kind":"paraphrase","q":"бонус начисляется без участия сотрудника","expect":"FR-TOOL-18"}
{"kind":"paraphrase","q":"прошлые визиты клиента какие услуги делали","expect":"FR-SVC-39"}
{"kind":"paraphrase","q":"загрузить услуги из файла при регистрации","expect":"FR-LIFE-17"}
{"kind":"code","q":"asGrosze","expect":"packages/contracts/src/money.ts"}
{"kind":"code","q":"problemDetailsOf","expect":"packages/contracts/src/errors.ts"}
{"kind":"code","q":"vitestBase","expect":"packages/config/vitest.base.ts"}
```

- [ ] **Step 2: Write the failing test**

```rust
// src/bench.rs
use crate::config::Config;
use crate::ids::IdMatcher;
use crate::index::dense::{DenseIndex, Embedder};
use crate::model::Graph;
use crate::query::{self, Answer, Options};
use crate::store::Store;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Case { pub kind: String, pub q: String, pub expect: String }

#[derive(Debug, Default)]
pub struct Summary { pub keyword: (usize, usize), pub paraphrase: (usize, usize), pub code: (usize, usize), pub p90_tokens: usize }

pub fn hit(case: &Case, answer: &Answer) -> bool {
    let all = answer.seeds.iter().chain(answer.expanded.iter());
    if case.kind == "code" {
        return answer.seeds.iter().any(|h| h.file == case.expect);
    }
    all.into_iter().any(|h| h.id == case.expect)
}

pub fn passes(s: &Summary, dense: bool) -> bool {
    let floor = if dense { 12 } else { 7 };
    s.keyword.0 == s.keyword.1 && s.paraphrase.0 >= floor.min(s.paraphrase.1) && s.code.0 == s.code.1 && s.p90_tokens <= 130
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::Hit;

    fn h(id: &str, file: &str) -> Hit { Hit { id: id.into(), file: file.into(), line: 1, label: String::new(), score: 1.0, via: None } }

    #[test]
    fn hit_rules() {
        let a = Answer { seeds: vec![h("sym:x.ts::f", "packages/x.ts")], expanded: vec![h("FR-PAY-22", "d.md")] };
        assert!(hit(&Case { kind: "paraphrase".into(), q: String::new(), expect: "FR-PAY-22".into() }, &a));
        assert!(hit(&Case { kind: "code".into(), q: String::new(), expect: "packages/x.ts".into() }, &a));
        assert!(!hit(&Case { kind: "keyword".into(), q: String::new(), expect: "FR-PAY-23".into() }, &a));
    }

    #[test]
    fn floors() {
        let s = Summary { keyword: (24, 24), paraphrase: (7, 14), code: (3, 3), p90_tokens: 120 };
        assert!(passes(&s, false));
        assert!(!passes(&s, true));
        assert!(!passes(&Summary { p90_tokens: 131, ..Summary { keyword: (24, 24), paraphrase: (14, 14), code: (3, 3), p90_tokens: 0 } }, true));
    }

    #[test]
    fn cases_file_parses_and_has_the_recorded_shape() {
        let text = std::fs::read_to_string(format!("{}/bench/cases.jsonl", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let cases: Vec<Case> = text.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(cases.iter().filter(|c| c.kind == "keyword").count(), 24);
        assert_eq!(cases.iter().filter(|c| c.kind == "paraphrase").count(), 14);
        assert_eq!(cases.iter().filter(|c| c.kind == "code").count(), 3);
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test bench::` — Expected: `cases_file_parses…` fails until the file exists; the other two compile and pass.

- [ ] **Step 4: Implement the runner**

```rust
// src/bench.rs (above tests)
pub fn run(repo: &Path, cfg: &Config, cases: &Path, no_dense: bool) -> Result<bool> {
    let repo = std::env::var("REPOGRAPH_BENCH_REPO").map(std::path::PathBuf::from).unwrap_or(repo.to_path_buf());
    let store = Store::new(&repo);
    let (graph, _): (Graph, _) = store.load()?;
    if graph.nodes.is_empty() { anyhow::bail!("graph is empty at {} — run build first", repo.display()); }
    let ids = IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
    let dense_idx = DenseIndex::load(&store)?;
    let embedder = std::cell::RefCell::new(if no_dense { None } else { Embedder::open().ok() });
    let dense_on = embedder.borrow().is_some() && !dense_idx.ids.is_empty();
    let dense_fn = |q: &str, k: usize| -> Vec<String> {
        let mut e = embedder.borrow_mut();
        match e.as_mut().and_then(|e| e.query(q).ok()) { Some(v) => dense_idx.search(&v, k), None => Vec::new() }
    };
    let text = std::fs::read_to_string(cases).with_context(|| cases.display().to_string())?;
    let opts = Options { seeds: 5, bodies: false, dense: dense_on, json: false };
    let mut summary = Summary::default();
    let mut tokens = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let case: Case = serde_json::from_str(line)?;
        let words: Vec<String> = case.q.split_whitespace().map(str::to_string).collect();
        let answer = query::ask(&graph, &ids, Some(&dense_fn), &words, &opts);
        let rendered = query::render(&answer, &graph, &opts);
        let tok = rendered.len() / 4;
        tokens.push(tok);
        let ok = hit(&case, &answer);
        let slot = match case.kind.as_str() { "keyword" => &mut summary.keyword, "paraphrase" => &mut summary.paraphrase, _ => &mut summary.code };
        slot.1 += 1;
        if ok { slot.0 += 1; }
        println!("{:<10} {:<12} {} {:>4} tok  {}", case.kind, case.expect, if ok { "HIT " } else { "miss" }, tok, case.q);
    }
    tokens.sort_unstable();
    summary.p90_tokens = tokens.get(tokens.len() * 9 / 10).copied().unwrap_or(0);
    println!("\nkeyword {}/{}  paraphrase {}/{}  code {}/{}  p90 {} tok  dense={dense_on}",
        summary.keyword.0, summary.keyword.1, summary.paraphrase.0, summary.paraphrase.1, summary.code.0, summary.code.1, summary.p90_tokens);
    Ok(passes(&summary, dense_on))
}
```

`main.rs` arm:

```rust
Cmd::Bench { cases } => {
    let cases = cases.unwrap_or_else(|| std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/bench/cases.jsonl")));
    if bench::run(&repo, &cfg, &cases, cli.no_dense)? { Ok(()) } else { anyhow::bail!("bench floors not met") }
}
```

- [ ] **Step 5: Run the bench both ways**

```bash
R=/Users/max/Documents/projects/beauty-crm
cargo run --release -- --repo $R bench --no-dense   # keyword 24/24, paraphrase ≥7/14, code 3/3, p90 ≤130
cargo run --release -- --repo $R bench              # paraphrase ≥12/14
```

If `--no-dense` paraphrase is below 7, the prototype reached 7 with BM25 over bodies + 1 hop, so the difference is in extraction: check `verify` for the requirement count (≥1632) and that bodies are non-empty for the missed ids (`explain FR-PAY-104`). If dense is below 12, apply the Task 12 Step 5 fallback ladder (`--seeds 8`, then `MultilingualE5Base`).

- [ ] **Step 6: Commit**

```bash
git add bench/cases.jsonl src/bench.rs src/main.rs
git commit -m "feat: bench the recorded keyword, paraphrase and code cases against their floors"
```

---

### Task 15: README, self-index, CI, release binaries

**Files:**

- Create: `README.md`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `LICENSE` (MIT, copyright Max Syn 2026)
- Modify: `repograph.toml` (already the worked example)

**Interfaces:**

- Consumes: everything. Produces nothing new in code; `cargo clippy -- -D warnings` must be clean.

- [ ] **Step 1: README**

`README.md` sections, each written out (no headings without text):

1. **What it is** — one paragraph: a 0-token project graph and a ≤130-token door onto it; the measured comparison table from the spec (graphify 0/14 paraphrase @1555 tok vs repograph target ≥12/14 @≤130).
2. **Install** — `cargo install --path .`; release binaries per tag for `aarch64-apple-darwin` and `x86_64-unknown-linux-gnu`.
3. **Use** — `repograph build`, `repograph update`, `repograph ask <words…> [--json] [--bodies] [--seeds N] [--no-dense]`, `repograph explain <id|symbol|label>`, `repograph verify`, `repograph import-legacy <graph.json>`, `repograph bench [--cases file]`. One example output block copied from a real `ask cancellation` run.
4. **Configure** — every `repograph.toml` key with its default, and the rule that requirement lines are `<ID> · MUST|SHOULD|LATER · title` in either `**…**` or `#` heading form.
5. **What is in the graph** — the node and edge kinds tables from the spec's Model section.
6. **Embeddings** — the model, its size, where it caches (`FASTEMBED_CACHE_DIR`), that the download happens once and that `--no-dense` skips it everywhere.
7. **Bench** — how the 41 cases were recorded and what the floors are.
8. **Design** — link to `docs/superpowers/specs/2026-09-01-repograph-design.md` and the three deviations from it.

- [ ] **Step 2: CI**

```yaml
# .github/workflows/ci.yml
name: ci
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.86.0
        with: { components: clippy }
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo test
  bench:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      # `secrets` is not readable from a job-level `if`; a step output is.
      - id: token
        run: echo "present=${{ secrets.BENCH_REPO_TOKEN != '' }}" >> "$GITHUB_OUTPUT"
      - if: steps.token.outputs.present == 'true'
        uses: actions/checkout@v4
        with:
          repository: olkp1/beauty-crm
          token: ${{ secrets.BENCH_REPO_TOKEN }}
          path: corpus
      - if: steps.token.outputs.present == 'true'
        uses: dtolnay/rust-toolchain@1.86.0
      - if: steps.token.outputs.present == 'true'
        run: cargo build --release
      - if: steps.token.outputs.present == 'true'
        run: ./target/release/repograph --repo corpus --no-dense build
      - if: steps.token.outputs.present == 'true'
        run: ./target/release/repograph --repo corpus --no-dense bench
```

The dense floor is checked locally (the model download does not belong in CI); the lexical floors are what CI guards.

```yaml
# .github/workflows/release.yml
name: release
on:
  push:
    tags: ['v*']
jobs:
  build:
    strategy:
      matrix:
        include:
          - { os: macos-14, target: aarch64-apple-darwin }
          - { os: ubuntu-latest, target: x86_64-unknown-linux-gnu }
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.86.0
        with: { targets: "${{ matrix.target }}" }
      - run: cargo build --release --target ${{ matrix.target }}
      - run: tar -C target/${{ matrix.target }}/release -czf repograph-${{ matrix.target }}.tar.gz repograph
      - uses: softprops/action-gh-release@v2
        with: { files: repograph-${{ matrix.target }}.tar.gz }
```

- [ ] **Step 3: Clippy, self-index, final bench**

```bash
cargo clippy --all-targets -- -D warnings
cargo run --release -- build && cargo run --release -- ask IdMatcher    # the tool indexes itself
R=/Users/max/Documents/projects/beauty-crm
cargo run --release -- --repo $R update && cargo run --release -- --repo $R update   # second: changed 0
cargo run --release -- --repo $R bench
```

End-to-end check from the spec: append `export const probe = 1;` to `$R/packages/contracts/src/money.ts`, run `update`, `explain probe` finds `sym:packages/contracts/src/money.ts::probe`; `git -C $R checkout packages/contracts/src/money.ts`, `update`, `explain probe` fails and `ask FR-PAY-22` still resolves.

- [ ] **Step 4: Commit and tag**

```bash
git add README.md LICENSE .github
git commit -m "docs: README, CI and release workflows"
git tag v0.1.0
```

---

## Self-review

**Spec coverage.** Context/measurements → README §1 and bench floors (Task 14). Repository layout → File structure (with three flagged simplifications plus the line-scanner-instead-of-tree-sitter-md one). Model (node kinds, edge kinds, uniqueness key) → Task 4 (`NodeKind`, `EdgeKind`, `Edge` ordering; the key is applied when rendering and by `Extraction` dedup). Extraction rules: both dialects, strict families, ranges/slash lists, ambiguous non-match → Tasks 2 and 5; backticked entities → Task 5; markdown links → Task 6; registries → Task 6; TRACKER.md skipped → Task 1 `skip`; code symbols/imports/barrels/tsconfig/exports/decorators → Tasks 7–8; ids in comments/strings → Task 9; Cyrillic paths and NUL bytes → Tasks 3 and 4. Retrieval steps 1–6 → Tasks 10–12; `--bodies`, `--json`, `--no-dense` → Tasks 11–12; `explain` → Task 11. Incremental update, atomic write, only-changed re-embedding → Tasks 4 and 12. `import-legacy` → Task 13. Bench with floors → Task 14. README/CI/release → Task 15. Verification section: `cargo test` coverage per item (id regex, dialects, barrels, stemmer, RRF, atomic write) → Tasks 2, 5, 7, 10, 11, 4; parity checks and the end-to-end export probe → Tasks 11 and 15.

**Not covered, on purpose:** tantivy, rkyv, petgraph, tree-sitter-md (deviations table); the beauty-crm integration (spec §Later, separate plan).

**Type consistency checked:** `Extraction::node(kind, id, label, body, file, line)` and `Extraction::edge(source, target, kind, context, file)` are used with that argument order in Tasks 5–13; `query::ask(graph, ids, dense, words, opts)` matches Tasks 11, 12, 14; `DenseIndex::sync(&mut self, graph, &mut FnMut)` matches Tasks 12 and 14; `IdMatcher::single_pattern()` is added in Task 5 and used there; `crate::doc::links::normalise` is defined in Task 6 and used by Task 7 — **Task 7 therefore depends on Task 6**, which is the plan order.

