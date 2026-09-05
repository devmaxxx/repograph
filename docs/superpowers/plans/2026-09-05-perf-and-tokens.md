# Performance and Token-Funded Accuracy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cut a fused `ask` from 582 ms to under 100 ms after the first question without changing a single answer, and measure the one token-funded query-time lever that survives its held-out rule.

**Architecture:** Three performance steps, each byte-identical by rule — drop the network round trip hidden in the model fetch (P1), open the model while the lexical work runs (P2), and a resident `serve` process behind a Unix socket that `ask` uses when it answers and ignores when it does not (P3, split into the refactor that makes the answer reusable, the server and client, and the measurement). Then the token lever: `ask --expand`, planned only if the proxy measured before this plan (Task A1's table) passed the pre-registered rule, and a kind router (A2) that waits for the weak-spots plan's Task 3 number.

**Tech Stack:** Rust (edition as in `Cargo.toml`, toolchain 1.98, `cargo test --release`, clippy `-D warnings`), `ort` 2.0.0-rc.13, `hf-hub` 0.5.0, `tokenizers` 0.22.2, `serde_json`, Unix domain sockets from `std::os::unix::net`, Python 3.14 stdlib only under `bench/`, bash 3.2, `claude -p` as the model command.

**Spec:** `docs/superpowers/specs/2026-09-05-perf-and-tokens-design.md` — the stage timings, the open split, the rejected file variants, the rules.

## Global Constraints

- Work in a fresh worktree `/Users/max/Documents/projects/repograph/.worktrees/perf` on branch `feat/perf` cut from `main` (`29693d2` or later). Never `cd` to the main checkout `/Users/max/Documents/projects/repograph` (dirty with unrelated work); use `git -C` and `--manifest-path`. Bash 3.2; zsh quirks (`echo ===`, `set --`) avoided.
- Session scratchpad `S=/private/tmp/claude-502/-Users-max-Documents-projects-repograph/244d7285-7f87-48c4-a4bc-379833f916d7/scratchpad`. Baseline dumps and timings are written there before the first code change (Task 0) and every later comparison reads them.
- The fixture `/Users/max/bench/beauty-crm-502e8a6d` is never `build`-, `update`-, `enrich`- or `embed`-ed; `ask --stale`, `bench` and `dump` against it read only. `serve` is never started against the fixture (it refreshes and writes); it is measured on `$S/bc-perf`, a copy with its source tree.
- Rule 1 (spec): a performance change ships only when the four `dump`s (`--no-dense` × recorded/developer suite, enriched store) are byte-identical to Task 0's and `bench` prints the same lines; timings are medians of five `REPOGRAPH_TIMING` runs, both arms, saved as named transcripts under `$S`.
- Rule 2 (spec): `serve` output equals in-process output for every case of `bench/cases.jsonl` and `bench/dev-cases.jsonl` in both arms, by test on a small repo and by transcript on `$S/bc-perf`; the client falls back on any handshake mismatch.
- Rule 3 (spec): a per-question model step is default-off and ships behind a flag only under the three-way rule against the plain arm, with p50 wall time and tokens per question in README.
- No constant is chosen by looking at a validation suite; rules are written before numbers and never changed after.
- Commit hook: Conventional Commits subjects; prose bodies with the measured numbers; never `Co-Authored-By`, `Claude-Session` or a `claude.ai` URL; never a heredoc and the commit command in one shell command unless the heredoc's first line is the subject (`git commit -F - <<'MSG'` with the subject first works); ledger and findings files that mention the commit command are written with the Write tool.
- Any `gh` call runs `gh auth switch --user devmaxxx && gh …` in the same shell command.
- Comments say why, never what; no ticket ids in code comments; keep tool directives. Test names are sentences in snake_case, as in the codebase.
- Every number written into a doc, a commit or a ledger comes from a transcript or log under `$S` (named) or a test run; token costs are estimates and say so.
- Implementers never dispatch subagents. All subagents run on `opus` (Max, 2026-09-05).

---

## File map

- `src/index/embed.rs` — `EXTERNAL_DATA_STUB` (new const), `keeps_weights_beside(len) -> bool` (new), `fetch_from(cache: &Path, endpoint: Option<&str>, model) -> Result<Files>` (new; `fetch` calls it with `cache_dir()?` and `None`).
- `src/main.rs` — the `Cmd::Ask` arm shrinks to `ask::Context::open` + `answer` (Task 3); `Cmd::Serve { every, batch, idle }` (new); `--no-serve` on `Cmd::Ask` (new); `warm` model thread in the ask context.
- `src/ask.rs` (new) — `Request` (serde), `Context` (graph, ids, questions, dense index, embedder, cross-encoder, timing), `Context::open(repo, cfg, stale, no_dense) -> Result<Context>`, `Context::answer(&mut self, req: &Request) -> Result<String>`; the text `Cmd::Ask` printed before, byte for byte.
- `src/serve.rs` (new) — `socket_path(store) -> PathBuf`, `Hello`, `Reply` (serde), `run(repo, cfg, every, batch, idle, no_dense) -> Result<()>`, `try_ask(repo, req) -> Option<Reply>` (client; `None` means answer in-process).
- `tests/serve.rs` (new) — end-to-end: build a two-file repo, `serve`, `ask` through the socket against `ask --no-serve`.
- `bench/history/run-repograph.sh` — unchanged; `REPOGRAPH_TIMING` transcripts are taken by hand.
- `README.md` — `serve` row in Status, the timing paragraph, `--expand` if A1 ships; `docs/bench/runbook.md` — how to take the timing transcripts; `docs/bench/2026-09-06-perf-results.md` (new).
- `bench/history/runs.jsonl` — rows for both suites at the branch head (identical lines by rule).

---

### Task 0: Worktree, copy, baselines

**Files:**
- Create: worktree `/Users/max/Documents/projects/repograph/.worktrees/perf`, branch `feat/perf`
- Create: `$S/bc-perf` (the fixture's corpus with its source tree), `$S/perf0-*.txt|json` baselines

- [ ] **Step 1: Cut the branch**

```bash
M=/Users/max/Documents/projects/repograph
git -C $M fetch origin main
git -C $M worktree add $M/.worktrees/perf -b feat/perf origin/main
W=$M/.worktrees/perf
git -C $W log --oneline -1
cargo build --release --manifest-path $W/Cargo.toml
cargo test --release --manifest-path $W/Cargo.toml 2>&1 | grep 'test result'
```

Expected: the head is `29693d2` or a later main; `413 passed; 0 failed; 2 ignored` or more passed, never fewer.

- [ ] **Step 2: The copy and the baseline dumps**

```bash
S=/private/tmp/claude-502/-Users-max-Documents-projects-repograph/244d7285-7f87-48c4-a4bc-379833f916d7/scratchpad
F=/Users/max/bench/beauty-crm-502e8a6d
B=$W/target/release/repograph
rsync -a --exclude node_modules $F/ $S/bc-perf/
for arm in dense lexical; do
  nd=""; [ $arm = lexical ] && nd="--no-dense"
  (cd $F && $B dump $nd > $S/perf0-rec-$arm.json)
  (cd $F && $B dump $nd --cases bench/dev-cases.jsonl > $S/perf0-dev-$arm.json) 2>/dev/null || (cd $F && $B dump $nd $W/bench/dev-cases.jsonl > $S/perf0-dev-$arm.json)
  (cd $F && $B bench $nd 2>&1 | tail -1 > $S/perf0-bench-rec-$arm.txt)
  (cd $F && $B bench $nd --cases $W/bench/dev-cases.jsonl 2>&1 | tail -1 > $S/perf0-bench-dev-$arm.txt)
done
shasum $S/perf0-*.json
```

Expected: four dump files and four bench lines; `perf0-bench-rec-dense.txt` reads `keyword 40/40 paraphrase 14/30 code 12/12` with p90 ≤ 230, `perf0-bench-rec-lexical.txt` `39/40 11/30 12/12` or the `14/30` the recorded rows at `6a9b5e6` show for the enriched store. (`dump`'s case-file flag: read `src/main.rs` `Cmd::Dump` for its exact spelling before running; the first form that works is the one to keep in the ledger.)

- [ ] **Step 3: The baseline timings — five runs per arm**

```bash
cat > $S/perf-time.sh <<'EOF'
#!/bin/bash
# usage: perf-time.sh <binary> <repo> <label>   — five REPOGRAPH_TIMING runs per arm, medians printed
B=$1; R=$2; L=$3; S=/private/tmp/claude-502/-Users-max-Documents-projects-repograph/244d7285-7f87-48c4-a4bc-379833f916d7/scratchpad
for arm in dense lexical; do
  nd=""; [ $arm = lexical ] && nd="--no-dense"
  : > $S/$L-$arm.txt
  for i in 1 2 3 4 5; do
    (cd $R && REPOGRAPH_TIMING=1 $B ask --stale $nd 'штраф за отмену записи' 2>&1 >/dev/null | grep '^timing' >> $S/$L-$arm.txt)
  done
  python3 -P - "$S/$L-$arm.txt" <<'PY'
import re, sys, statistics
rows = {}
for line in open(sys.argv[1]):
    m = re.match(r'timing:\s+([\d.]+) ms\s+\(\+\s*([\d.]+) ms\)\s+(.*)', line)
    if m: rows.setdefault(m.group(3), []).append((float(m.group(1)), float(m.group(2))))
for stage, v in rows.items():
    print(f"{stage:32} total median {statistics.median(x for x,_ in v):8.1f} ms   step median {statistics.median(y for _,y in v):7.1f} ms   n={len(v)}")
PY
done
EOF
chmod +x $S/perf-time.sh
$S/perf-time.sh $B $F perf0
```

Expected: a `model opened` step median near 480 ms and a `printed` total near 580 ms in the dense arm; `printed` near 75 ms in the lexical arm (the spec's single-run numbers were 480.3 and 582.5 / 75.5).

- [ ] **Step 4: Ledger**

No commit; the SDD ledger records the head, the copy, the four dump hashes and the two median tables.

---

### Task 1 (P1): No network round trip in `fetch`

**Files:**
- Modify: `src/index/embed.rs:60-76` (`fetch`), tests at the end of the file
- Test: `src/index/embed.rs` unit tests

**Interfaces:**
- Consumes: `hf_hub::api::sync::ApiBuilder` (`with_cache_dir`, `with_endpoint`), `Files { model, tokenizer, pad_token, pad_id }`.
- Produces: `fetch_from(cache: &Path, endpoint: Option<&str>, model: &str) -> Result<Files>`; `keeps_weights_beside(model_len: u64) -> bool`; `EXTERNAL_DATA_STUB: u64`.

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module of `src/index/embed.rs` (create `mod tests` at the end if the file has none):

```rust
#[cfg(test)]
mod fetch_tests {
    use super::*;

    /// A cache in hf-hub's layout for one model, with the model file sized as asked; the other
    /// three files are small and valid enough for `fetch` to read them.
    fn cache_with_model_of(dir: &std::path::Path, model_len: u64) {
        let root = dir.join("models--intfloat--multilingual-e5-small");
        std::fs::create_dir_all(root.join("refs")).unwrap();
        std::fs::write(root.join("refs/main"), "abc").unwrap();
        let snap = root.join("snapshots/abc");
        std::fs::create_dir_all(snap.join("onnx")).unwrap();
        let model = std::fs::File::create(snap.join("onnx/model.onnx")).unwrap();
        model.set_len(model_len).unwrap();
        std::fs::write(snap.join("tokenizer.json"), "{}").unwrap();
        std::fs::write(snap.join("config.json"), r#"{"pad_token_id": 1}"#).unwrap();
        std::fs::write(snap.join("tokenizer_config.json"), r#"{"pad_token": "<pad>"}"#).unwrap();
    }

    #[test]
    fn a_model_that_holds_its_weights_is_fetched_without_the_network() {
        let dir = tempfile::tempdir().unwrap();
        cache_with_model_of(dir.path(), EXTERNAL_DATA_STUB);
        // An endpoint nothing listens on: any lookup that leaves the cache fails at once.
        let files = fetch_from(dir.path(), Some("http://127.0.0.1:9"), "intfloat/multilingual-e5-small").unwrap();
        assert!(files.model.ends_with("onnx/model.onnx"));
        assert_eq!(files.pad_token, "<pad>");
        assert_eq!(files.pad_id, 1);
    }

    #[test]
    fn a_graph_only_stub_asks_for_the_weights_beside_it() {
        let dir = tempfile::tempdir().unwrap();
        cache_with_model_of(dir.path(), 1 << 20);
        // The stub's data file is not cached and the endpoint is dead, so the lookup fails; the
        // fetch still answers, as it did before, and the session open reports the missing file.
        let files = fetch_from(dir.path(), Some("http://127.0.0.1:9"), "intfloat/multilingual-e5-small").unwrap();
        assert!(files.model.ends_with("onnx/model.onnx"));
    }

    #[test]
    fn the_stub_threshold_separates_the_small_models_file_from_a_graph_only_one() {
        assert!(!keeps_weights_beside(448 << 20));
        assert!(!keeps_weights_beside(EXTERNAL_DATA_STUB));
        assert!(keeps_weights_beside(EXTERNAL_DATA_STUB - 1));
        assert!(keeps_weights_beside(2 << 20));
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --release --manifest-path $W/Cargo.toml fetch_tests 2>&1 | tail -5`
Expected: compile error — `fetch_from`, `keeps_weights_beside`, `EXTERNAL_DATA_STUB` not found.

- [ ] **Step 3: Implement**

Replace `fetch` in `src/index/embed.rs` with:

```rust
/// A graph-only `model.onnx` keeps its weights in `model.onnx_data` beside it and is a few MB;
/// the small model's 448 MB file holds them itself. Asking the hub for a data file a model never
/// had is a network round trip on every fused query — 240 ms measured, and a failure offline.
const EXTERNAL_DATA_STUB: u64 = 64 << 20;

fn keeps_weights_beside(model_len: u64) -> bool { model_len < EXTERNAL_DATA_STUB }

/// Cache hits never touch the network; the first run downloads with a progress bar.
fn fetch(model: &str) -> Result<Files> { fetch_from(&cache_dir()?, None, model) }

fn fetch_from(cache: &Path, endpoint: Option<&str>, model: &str) -> Result<Files> {
    let mut builder = hf_hub::api::sync::ApiBuilder::new().with_cache_dir(cache.to_path_buf()).with_progress(true);
    if let Some(e) = endpoint { builder = builder.with_endpoint(e.to_string()); }
    let api = builder.build()?;
    let name = model.to_string();
    let repo = api.model(name.clone());
    let get = |f: &str| repo.get(f).with_context(|| format!("fetch {name}/{f}"));
    let model = get("onnx/model.onnx")?;
    // The larger models keep their weights beside the graph; the session resolves the file by
    // its relative name, so it has to be fetched into the same snapshot. Only a stub-sized graph
    // can have one, so the small model never pays the lookup.
    let len = std::fs::metadata(&model).map(|m| m.len()).unwrap_or(0);
    if keeps_weights_beside(len) { let _ = repo.get("onnx/model.onnx_data"); }
    let tokenizer = get("tokenizer.json")?;
    let config: serde_json::Value = serde_json::from_slice(&std::fs::read(get("config.json")?)?)?;
    let tok_config: serde_json::Value = serde_json::from_slice(&std::fs::read(get("tokenizer_config.json")?)?)?;
    let pad_token = tok_config["pad_token"].as_str().context("tokenizer_config.json: pad_token")?.to_string();
    let pad_id = config["pad_token_id"].as_u64().unwrap_or(0) as u32;
    Ok(Files { model, tokenizer, pad_token, pad_id })
}
```

If `with_progress` is not a method of `ApiBuilder` in 0.5.0 under this name, keep whatever the current `fetch` calls — the only change is the endpoint and the guarded lookup.

- [ ] **Step 4: Run the tests**

Run: `cargo test --release --manifest-path $W/Cargo.toml fetch_tests 2>&1 | tail -5` then the whole suite `cargo test --release --manifest-path $W/Cargo.toml 2>&1 | grep 'test result'` and `cargo clippy --release --manifest-path $W/Cargo.toml -- -D warnings`.
Expected: 3 passed; the suite's count is Task 0's plus 3; clippy clean.

- [ ] **Step 5: Measure — Rule 1**

```bash
cargo build --release --manifest-path $W/Cargo.toml
for arm in dense lexical; do nd=""; [ $arm = lexical ] && nd="--no-dense"
  (cd $F && $B dump $nd > $S/perf1-rec-$arm.json); cmp $S/perf0-rec-$arm.json $S/perf1-rec-$arm.json && echo "rec $arm identical"
  (cd $F && $B dump $nd <the dev-cases form from Task 0> > $S/perf1-dev-$arm.json); cmp $S/perf0-dev-$arm.json $S/perf1-dev-$arm.json && echo "dev $arm identical"
done
$S/perf-time.sh $B $F perf1
```

Expected: four `identical` lines; `model opened` step median about 240 ms lower than Task 0's; the lexical arm unchanged within noise.

- [ ] **Step 6: Commit**

```bash
git -C $W add src/index/embed.rs
git -C $W commit -F - <<'MSG'
perf(embed): stop asking the hub for a data file the small model never had

Every fused ask fetched onnx/model.onnx_data for a model whose 448 MB
graph holds its own weights; hf-hub found nothing cached and made a
network round trip that answered 404 in about 240 ms (perf-open-parts.txt:
238-256 ms of a 236-299 ms fetch). The lookup now happens only when the
cached model.onnx is a graph-only stub under 64 MB. Dumps of both suites
in both arms are byte-identical; model opened fell from <perf0 median> to
<perf1 median> ms on the fixture.
MSG
```

Fill the two medians from `$S/perf0-dense.txt` and `$S/perf1-dense.txt` before committing.

---

### Task 2 (P2): Open the model while the lexical work runs

**Files:**
- Modify: `src/main.rs:426-470` (the `Cmd::Ask` arm — vectors load, embedder slot), `src/main.rs:359-367` (`open_embedder`)
- Test: `src/query.rs` has `exact_seeds` tests; add one in `src/main.rs`'s tests only if `warm_model` becomes a function (below)

**Interfaces:**
- Consumes: `query::exact_seeds(graph, ids, words) -> (Vec<String>, bool)` (pub(crate)); `index::dense::DenseIndex::load(store)`, `.model_of_rows()`; `index::embed::resolve(recorded, configured)`; `open_embedder(no_dense, model)`.
- Produces: `fn warm_model(dense: bool, whole: bool, model: &str) -> Option<std::thread::JoinHandle<Option<index::embed::Embedder>>>` in `src/main.rs`.

- [ ] **Step 1: The decision as a function, with its test**

Add to `src/main.rs` after `open_embedder`:

```rust
/// The model opens on a thread while ids, questions and the BM25 indexes are built — the
/// 55–70 ms of lexical work a fused answer does anyway. A question that exact ids or symbols
/// answer whole never opens it, as before; with `--no-dense` or no vectors nothing starts.
fn warm_model(dense: bool, whole: bool, model: &str) -> Option<std::thread::JoinHandle<Option<index::embed::Embedder>>> {
    if !dense || whole { return None; }
    let model = model.to_string();
    Some(std::thread::spawn(move || open_embedder(false, &model)))
}
```

And in `src/main.rs`'s test module (create `#[cfg(test)] mod tests` at the end if none):

```rust
#[test]
fn the_model_is_not_warmed_for_an_exact_answer_or_a_lexical_arm() {
    assert!(warm_model(false, false, "any").is_none());
    assert!(warm_model(true, true, "any").is_none());
}
```

(The positive case opens the real model and is not a unit test; Step 4 measures it.)

- [ ] **Step 2: Run the test**

Run: `cargo test --release --manifest-path $W/Cargo.toml the_model_is_not_warmed 2>&1 | tail -3`
Expected: 1 passed.

- [ ] **Step 3: Wire it into the `Cmd::Ask` arm**

In the arm, after `timing.stage("questions ready");` and before the `embedder` slot, load the vectors and start the thread:

```rust
            let opts = query::Options { seeds, bodies, dense: !cli.no_dense && index::dense::DenseIndex::present(&store), json, depth };
            let (_, whole) = query::exact_seeds(&graph, &ids, &words);
            let dense_idx: std::cell::RefCell<Option<index::dense::DenseIndex>> = std::cell::RefCell::new(None);
            let warm = std::cell::Cell::new(None);
            if opts.dense && !whole {
                let idx = index::dense::DenseIndex::load(&store).unwrap_or_else(|err| { eprintln!("dense: index unreadable, continuing lexical-only ({err:#})"); Default::default() });
                timing.stage("vectors loaded");
                let model = index::embed::resolve(idx.model_of_rows().as_deref(), &cfg.embed_model);
                warm.set(warm_model(true, whole, &model));
                dense_idx.replace(Some(idx));
            }
            let embedder: std::cell::RefCell<Option<Option<index::embed::Embedder>>> = std::cell::RefCell::new(None);
```

Delete the later `let opts = …` line (it moved up) and change the embedder slot inside `dense_fn` to take the warmed handle first:

```rust
                let e = slot.get_or_insert_with(|| {
                    let e = match warm.take() {
                        Some(handle) => handle.join().unwrap_or_else(|_| { eprintln!("dense: model thread panicked, continuing lexical-only"); None }),
                        None => {
                            let model = index::embed::resolve(idx.model_of_rows().as_deref(), &cfg.embed_model);
                            open_embedder(cli.no_dense, &model)
                        }
                    };
                    timing.stage("model opened");
                    e
                });
```

The `dense_idx` `get_or_insert_with` stays as it is: when the warm path loaded the index it finds it; on the exact-answer path nothing was loaded and it loads lazily as before. `Embedder` must be `Send` for the thread — `ort::session::Session` and `tokenizers::Tokenizer` are; if the compiler disagrees, the field that is not is named in the error and the task stops with `BLOCKED` rather than wrapping it in a lock.

- [ ] **Step 4: Tests, clippy, Rule 1**

Run: `cargo test --release --manifest-path $W/Cargo.toml 2>&1 | grep 'test result'`; `cargo clippy --release --manifest-path $W/Cargo.toml -- -D warnings`; then the dump comparison and `$S/perf-time.sh $B $F perf2` exactly as in Task 1 Step 5 with `perf2` file names.
Expected: all identical; the dense `printed` total median lower than `perf1`'s by 40–70 ms; the lexical arm unchanged; an exact-id question (`(cd $F && REPOGRAPH_TIMING=1 $B ask --stale FR-PAY-104)`) shows no `model opened` stage.

- [ ] **Step 5: Commit**

```bash
git -C $W add src/main.rs
git -C $W commit -F - <<'MSG'
perf(ask): open the model while the lexical work runs

The tokenizer and the session take about 235 ms together and used to
start only on the first fused query, after ids, questions and the BM25
indexes were built. They now start on a thread as soon as the vectors
name the model, unless exact ids or symbols answer the question whole
or dense is off. Dumps unchanged; printed fell from <perf1> to <perf2>
ms in the dense arm on the fixture.
MSG
```

---

### Task 3 (P3a): The answer as a reusable context

**Files:**
- Create: `src/ask.rs`
- Modify: `src/main.rs` (`mod ask;`, the `Cmd::Ask` arm shrinks to build a `Request`, open a `Context`, print)
- Test: `src/ask.rs` unit tests

**Interfaces:**
- Consumes: everything the `Cmd::Ask` arm uses today (`graph_for_ask`, `load_cfg`, `Store`, `IdMatcher`, `Questions::load_traced`, `DenseIndex`, `Embedder`, `rerank::run`, `index::cross`, `query::ask`, `query::render`, `Timing`, `warm_model`, `open_embedder`).
- Produces:

```rust
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Request { pub words: Vec<String>, pub json: bool, pub seeds: usize, pub bodies: bool, pub rerank: bool, pub rerank_local: bool, pub depth: usize, pub stale: bool, pub no_dense: bool }

pub struct Context { /* graph, ids, questions, store, cfg, dense index slot, embedder slot, cross-encoder slot, resync flag, timing */ }

impl Context {
    /// The store read the way `ask` reads it: refreshed against the tree unless `stale`, the
    /// stored graph with a warning when the store cannot be written.
    pub fn open(repo: &Path, cfg: &config::Config, stale: bool, no_dense: bool) -> anyhow::Result<Context>;
    /// The text `ask` prints for this request — `render`'s output, byte for byte.
    pub fn answer(&mut self, req: &Request) -> anyhow::Result<String>;
    /// Notices `ask` used to print on stderr for this answer (refresh lines, dense fallbacks), drained.
    pub fn notices(&mut self) -> Vec<String>;
}
```

`Cmd::Ask` becomes: build `Request` from the flags, `Context::open`, `answer`, print stdout, print notices to stderr, flush, exit 0. `graph_for_ask`, `Timing`, `open_embedder` and `warm_model` move into `src/ask.rs` (pub(crate)) so `serve` can use them; `run_watch` keeps calling `open_embedder` through the module path.

- [ ] **Step 1: Write the failing test**

In `src/ask.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn repo_with_two_docs() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("docs/pay.md"), "# FR-PAY-1 Штраф за отмену\n\nШтраф списывается сам.\n").unwrap();
        std::fs::write(dir.path().join("docs/cal.md"), "# FR-CAL-1 Перенос визита\n\nПеренос не считается отменой.\n").unwrap();
        std::fs::write(dir.path().join("repograph.toml"), "id_families = [\"FR-PAY\", \"FR-CAL\"]\n").unwrap();
        dir
    }

    #[test]
    fn a_context_answers_the_same_text_twice_and_for_an_exact_id() {
        let dir = repo_with_two_docs();
        let cfg = crate::config::Config::load(dir.path()).unwrap();
        let ex = crate::extractors(dir.path(), &cfg).unwrap();
        crate::run_update(dir.path(), &cfg, &ex, true).unwrap();
        let mut ctx = Context::open(dir.path(), &cfg, true, true).unwrap();
        let req = Request { words: vec!["штраф".into()], json: false, seeds: 5, bodies: false, rerank: false, rerank_local: false, depth: crate::rerank::DEPTH, stale: true, no_dense: true };
        let first = ctx.answer(&req).unwrap();
        let second = ctx.answer(&req).unwrap();
        assert_eq!(first, second);
        assert!(first.contains("FR-PAY-1"), "{first}");
        let exact = ctx.answer(&Request { words: vec!["FR-CAL-1".into()], ..req }).unwrap();
        assert!(exact.starts_with("FR-CAL-1"), "{exact}");
    }
}
```

(If `run_update` or `extractors` are private to `main.rs`, make them `pub(crate)` — the test lives in the same crate.)

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test --release --manifest-path $W/Cargo.toml a_context_answers 2>&1 | tail -5`
Expected: compile error, `ask::Context` unknown.

- [ ] **Step 3: Move the arm into `src/ask.rs`**

Create `src/ask.rs` with `Request`, `Context` and the moved functions. `Context::open` does what the arm does up to `questions ready` (plus the Task 2 warm start for the first request when `!stale`-independent: warm on `open` only if the first request is fused — `open` cannot know, so `answer` starts the warm thread on its first call when `whole` is false, then keeps the embedder). `answer` holds the `dense_fn`, `rerank_fn`, `local_fn` closures and the `query::ask` + `query::render` calls, appending what used to be `eprintln!` lines to `self.notices` instead of printing. The `resync` flag is set by `open` when the refresh reported changes and cleared by the first fused answer, as today; `serve` sets it again after every refresh (Task 4).

The `Cmd::Ask` arm:

```rust
        Cmd::Ask { words, json, seeds, bodies, rerank, rerank_local, depth, stale, no_serve } => {
            let req = ask::Request { words, json, seeds, bodies, rerank, rerank_local, depth, stale, no_dense: cli.no_dense };
            let cfg = load_cfg()?;
            let mut ctx = ask::Context::open(&repo, &cfg, stale, cli.no_dense)?;
            let text = ctx.answer(&req)?;
            for n in ctx.notices() { eprintln!("{n}"); }
            print!("{text}");
            use std::io::Write;
            std::io::stdout().flush()?;
            ctx.timing().stage("printed");
            std::process::exit(0)
        }
```

(`no_serve` is added in Task 4; until then the pattern has no such field.) The order of stderr lines against stdout changes for a human reader only when a notice is printed — `refresh:` lines used to come before the answer; keep that by printing notices before `print!`.

- [ ] **Step 4: Tests, clippy, Rule 1**

Run the suite, clippy, the four dump comparisons (`perf3` names) and `$S/perf-time.sh $B $F perf3`.
Expected: identical dumps; the stage table equal to `perf2`'s within noise (a refactor).

- [ ] **Step 5: Commit**

```bash
git -C $W add src/ask.rs src/main.rs
git -C $W commit -F - <<'MSG'
refactor(ask): the answer as a context that can answer again

The ask arm becomes ask::Context::open plus Context::answer, so a
resident process can hold the graph, the vectors, the model and the
indexes and answer many requests with the code path a one-shot ask
runs. Output is the same bytes: dumps of both suites in both arms are
identical to the pre-refactor ones, stage medians within noise.
MSG
```

---

### Task 4 (P3b): `serve` and the client in `ask`

**Files:**
- Create: `src/serve.rs`, `tests/serve.rs`
- Modify: `src/main.rs` (`mod serve;`, `Cmd::Serve`, `--no-serve` on `Cmd::Ask`, the client call in the arm), `src/ask.rs` (`Context::refresh(&mut self) -> Result<Option<UpdateReport>>` for the server's pre-answer refresh)

**Interfaces:**
- Consumes: `ask::{Request, Context}`, `Watcher` (moved to `src/serve.rs` or made `pub(crate)` in `main.rs`), `store::Store::new(repo).dir` for the socket path.
- Produces:

```rust
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub fn socket_path(repo: &Path) -> PathBuf;               // <repo>/.repograph/serve.sock
#[derive(serde::Serialize, serde::Deserialize)] pub struct Hello { pub v: String, pub req: ask::Request }
#[derive(serde::Serialize, serde::Deserialize)] pub struct Reply { pub v: String, pub stdout: String, pub stderr: Vec<String> }
/// The answer the resident process gives, or None when the request must be answered here:
/// no socket, a socket nobody listens on (unlinked), a reply from another version, a timeout.
pub fn try_ask(repo: &Path, req: &ask::Request) -> Option<Reply>;
pub fn run(repo: &Path, cfg: &config::Config, every: u64, batch: usize, idle: u64, no_dense: bool) -> anyhow::Result<()>;
```

Protocol: one line of JSON (`Hello`) from the client, one line of JSON (`Reply`) from the server, then close. Read and write timeouts on the client of 30 s (a reranked answer takes ~4 s); the connect itself fails at once when nothing listens. The server answers one connection at a time on the listener's thread; between connections it polls the tree every `every` seconds with `Watcher::poll(batch)`; before each answer whose request is not `stale` it runs `Watcher::poll(1)` (the same walk-and-apply `ask`'s refresh does) and, when that refreshed, sets the context's resync flag and reloads the questions if their stamp moved. It exits after `idle` seconds without a request, and on exit or start removes a socket file nobody answers.

- [ ] **Step 1: The end-to-end test, failing**

`tests/serve.rs`:

```rust
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn repograph() -> Command { Command::new(env!("CARGO_BIN_EXE_repograph")) }

fn repo_with_docs() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/pay.md"), "# FR-PAY-1 Штраф за отмену\n\nШтраф списывается сам (INV-1).\n\n# INV-1 Деньги не сгорают\n\nОтмена не сжигает деньги.\n").unwrap();
    std::fs::write(dir.path().join("docs/cal.md"), "# FR-CAL-1 Перенос визита\n\nПеренос не считается отменой.\n").unwrap();
    std::fs::write(dir.path().join("repograph.toml"), "id_families = [\"FR-PAY\", \"FR-CAL\", \"INV\"]\n").unwrap();
    let out = repograph().args(["--no-dense", "--repo"]).arg(dir.path()).arg("build").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    dir
}

fn ask(dir: &std::path::Path, extra: &[&str], words: &[&str]) -> (String, String) {
    let out = repograph().args(["--no-dense", "--repo"]).arg(dir).arg("ask").args(extra).args(words).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    (String::from_utf8(out.stdout).unwrap(), String::from_utf8(out.stderr).unwrap())
}

fn wait_for_socket(dir: &std::path::Path) {
    let sock = dir.join(".repograph/serve.sock");
    let start = Instant::now();
    while !sock.exists() {
        assert!(start.elapsed() < Duration::from_secs(20), "serve never opened its socket");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn the_socket_answers_the_bytes_the_process_answers_and_sees_an_edit() {
    let dir = repo_with_docs();
    let questions = [vec!["штраф"], vec!["перенос", "визита"], vec!["FR-CAL-1"], vec!["ничего"]];
    let direct: Vec<_> = questions.iter().map(|q| ask(dir.path(), &["--no-serve"], q).0).collect();
    let mut server = repograph().args(["--no-dense", "--repo"]).arg(dir.path()).args(["serve", "--every", "1", "--idle", "60"]).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    wait_for_socket(dir.path());
    for (q, want) in questions.iter().zip(&direct) {
        let (got, err) = ask(dir.path(), &[], q);
        assert_eq!(&got, want, "question {q:?}");
        assert!(err.contains("serve:") , "the client says it answered through the socket: {err}");
    }
    let json_direct = ask(dir.path(), &["--no-serve", "--json"], &["штраф"]).0;
    assert_eq!(ask(dir.path(), &["--json"], &["штраф"]).0, json_direct);
    std::fs::write(dir.path().join("docs/new.md"), "# FR-PAY-2 Возврат аванса\n\nАванс возвращается при отмене салоном.\n").unwrap();
    let (after, _) = ask(dir.path(), &[], &["возврат", "аванса"]);
    assert!(after.contains("FR-PAY-2"), "the server refreshed before answering: {after}");
    server.kill().unwrap();
    let _ = server.wait();
}

#[test]
fn a_socket_nobody_listens_on_is_removed_and_the_question_answered_here() {
    let dir = repo_with_docs();
    let sock = dir.path().join(".repograph/serve.sock");
    std::os::unix::net::UnixListener::bind(&sock).unwrap();
    // The listener is dropped at once: the file stays, nothing accepts.
    let (out, err) = ask(dir.path(), &[], &["штраф"]);
    assert!(out.contains("FR-PAY-1"));
    assert!(!err.contains("serve:"), "{err}");
    assert!(!sock.exists(), "a dead socket file is unlinked");
}

#[test]
fn a_reply_from_another_version_is_ignored() {
    let dir = repo_with_docs();
    let sock = dir.path().join(".repograph/serve.sock");
    let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
    let fake = std::thread::spawn(move || {
        use std::io::{BufRead, Write};
        let (mut s, _) = listener.accept().unwrap();
        let mut line = String::new();
        std::io::BufReader::new(s.try_clone().unwrap()).read_line(&mut line).unwrap();
        writeln!(s, r#"{{"v":"0.0.0","stdout":"WRONG\n","stderr":[]}}"#).unwrap();
    });
    let (out, _) = ask(dir.path(), &[], &["штраф"]);
    assert!(out.contains("FR-PAY-1") && !out.contains("WRONG"), "{out}");
    fake.join().unwrap();
}
```

The fake-listener test's `UnixListener::bind` inside `repo_with_docs`'s `.repograph` requires the directory to exist — `build` created it.

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test --release --manifest-path $W/Cargo.toml --test serve 2>&1 | tail -5`
Expected: the binary rejects `--no-serve` / `serve` (unknown argument) — three failures.

- [ ] **Step 3: Implement `src/serve.rs`**

```rust
//! `repograph serve`: the store, the vectors, the model and the indexes held by one process
//! that answers `ask` over a Unix socket. A fused answer costs a process 480 ms to open the
//! model (measured on the bench corpus) and 5 ms to use it; a resident one pays the open once.
use crate::{ask, config, walk};
use anyhow::{Context as _, Result};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const IO_TIMEOUT: Duration = Duration::from_secs(30);
const ACCEPT_TICK: Duration = Duration::from_millis(50);

pub fn socket_path(repo: &Path) -> PathBuf { repo.join(".repograph").join("serve.sock") }

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Hello { pub v: String, pub req: ask::Request }

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Reply { pub v: String, pub stdout: String, pub stderr: Vec<String> }

/// The resident answer, or None when this process has to answer: no socket, one nobody
/// listens on (unlinked here, so the next start binds cleanly), another version, a timeout,
/// or a reply that does not parse.
pub fn try_ask(repo: &Path, req: &ask::Request) -> Option<Reply> {
    let path = socket_path(repo);
    if !path.exists() { return None; }
    let mut stream = match UnixStream::connect(&path) {
        Ok(s) => s,
        Err(_) => { let _ = std::fs::remove_file(&path); return None; }
    };
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok()?;
    let hello = serde_json::to_string(&Hello { v: VERSION.into(), req: req.clone() }).ok()?;
    writeln!(stream, "{hello}").ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    let reply: Reply = serde_json::from_str(&line).ok()?;
    if reply.v != VERSION { return None; }
    Some(reply)
}

pub fn run(repo: &Path, cfg: &config::Config, every: u64, batch: usize, idle: u64, no_dense: bool) -> Result<()> {
    let path = socket_path(repo);
    if path.exists() && UnixStream::connect(&path).is_ok() { anyhow::bail!("another serve answers at {}", path.display()); }
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).with_context(|| format!("bind {}", path.display()))?;
    listener.set_nonblocking(true)?;
    let _guard = Unlink(path.clone());
    let mut watcher = crate::Watcher::open(repo, cfg)?;
    let mut ctx = ask::Context::open(repo, cfg, true, no_dense)?;
    eprintln!("serve: {} every {every}s, batch {batch}, idle {idle}s; Ctrl-C stops", path.display());
    let (mut last_poll, mut last_request) = (Instant::now(), Instant::now());
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                last_request = Instant::now();
                if let Err(e) = answer(stream, &mut watcher, &mut ctx) { eprintln!("serve: {e:#}"); }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(ACCEPT_TICK),
            Err(e) => return Err(e).context("accept"),
        }
        if last_poll.elapsed() >= Duration::from_secs(every) {
            last_poll = Instant::now();
            if let crate::Polled::Refreshed(r) = watcher.poll(batch)? { ctx.adopt(&watcher, Some(r))?; }
        }
        if last_request.elapsed() >= Duration::from_secs(idle) { eprintln!("serve: idle for {idle}s, exiting"); return Ok(()); }
    }
}

fn answer(stream: UnixStream, watcher: &mut crate::Watcher, ctx: &mut ask::Context) -> Result<()> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let hello: Hello = serde_json::from_str(&line).context("hello")?;
    let mut stream = stream;
    if hello.v != VERSION {
        writeln!(stream, "{}", serde_json::to_string(&Reply { v: VERSION.into(), stdout: String::new(), stderr: vec![] })?)?;
        return Ok(());
    }
    if !hello.req.stale {
        // The same refresh a one-shot ask does before answering: batch 1 applies any change now.
        if let crate::Polled::Refreshed(r) = watcher.poll(1)? { ctx.adopt(watcher, Some(r))?; }
    }
    let stdout = ctx.answer(&hello.req)?;
    let reply = Reply { v: VERSION.into(), stdout, stderr: ctx.notices() };
    writeln!(stream, "{}", serde_json::to_string(&reply)?)?;
    Ok(())
}

struct Unlink(PathBuf);
impl Drop for Unlink { fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); } }
```

`Context::adopt(&mut self, w: &Watcher, refreshed: Option<UpdateReport>)` (new in `src/ask.rs`) replaces the context's graph with a clone of the watcher's (`w.graph.clone()` — `Graph` is `Clone`; if it is not, derive it), reloads the questions when `store.stamp("questions.json")` moved, pushes the `refresh: N changed, M removed` notice and sets the resync flag so the next fused answer re-embeds the moved rows exactly as a one-shot `ask` would. `Watcher` and `Polled` become `pub(crate)` in `main.rs` (or move to `src/serve.rs` with `run_watch`).

`Cmd::Serve`:

```rust
    /// Answers `ask` from a resident process over `.repograph/serve.sock`: the model, the
    /// vectors and the indexes open once. Refreshes before every answer the way `ask` does,
    /// polls between them like `watch`, exits after `--idle` seconds without a question.
    Serve {
        #[arg(long, default_value_t = 30)] every: u64,
        #[arg(long, default_value_t = 1)] batch: usize,
        #[arg(long, default_value_t = 1800)] idle: u64,
    },
```

`Cmd::Ask` gains `/// Answers in this process even when a `serve` is listening.` `#[arg(long)] no_serve: bool,` and the arm starts with:

```rust
            if !no_serve {
                if let Some(reply) = serve::try_ask(&repo, &req) {
                    for n in reply.stderr { eprintln!("{n}"); }
                    eprintln!("serve: answered by the resident process");
                    print!("{}", reply.stdout);
                    use std::io::Write;
                    std::io::stdout().flush()?;
                    std::process::exit(0)
                }
            }
```

`REPOGRAPH_NO_SERVE` set in the environment counts as `--no-serve` (bench and dump never use the socket: `bench::run` and `dump` build their own contexts and do not call `try_ask`).

- [ ] **Step 4: Run the tests**

Run: `cargo test --release --manifest-path $W/Cargo.toml --test serve 2>&1 | tail -8`, then the whole suite and clippy.
Expected: 3 passed; suite green; clippy clean. If the end-to-end test is flaky on the socket wait, the wait loop's 20 s cap is the only knob — never a sleep.

- [ ] **Step 5: Rule 1 on the fixture, Rule 2 on the copy**

```bash
cargo build --release --manifest-path $W/Cargo.toml
# Rule 1: dumps and in-process timings unchanged (perf4 names, as Task 1 Step 5)
# Rule 2: socket against in-process, every case, both arms, on the copy — never the fixture
C=$S/bc-perf
(cd $C && $B serve --idle 600 > $S/serve4.log 2>&1 &)
sleep 3
python3 -P - <<'PY'
import json, subprocess, os
S = os.environ["S"]; C = os.environ["C"]; B = os.environ["B"]; W = os.environ["W"]
def ask(words, arm, extra):
    a = [B, "--repo", C] + (["--no-dense"] if arm == "lexical" else []) + ["ask", "--stale"] + extra + words
    return subprocess.run(a, capture_output=True, text=True).stdout
same = diff = 0
for suite in [f"{W}/bench/cases.jsonl", f"{W}/bench/dev-cases.jsonl"]:
    for line in open(suite):
        if not line.strip(): continue
        q = json.loads(line)["q"].split()
        for arm in ["dense", "lexical"]:
            a, b = ask(q, arm, ["--no-serve"]), ask(q, arm, [])
            if a == b: same += 1
            else: diff += 1; print("DIFF", arm, q)
print(f"identical {same} differing {diff}")
PY
pkill -f 'repograph serve' || true
```

Expected: `identical 284 differing 0` (142 cases × 2 arms). Then the timing through the socket:

```bash
(cd $C && $B serve --idle 600 > $S/serve4.log 2>&1 &); sleep 3
for arm in dense lexical; do nd=""; [ $arm = lexical ] && nd="--no-dense"
  : > $S/perf4-serve-$arm.txt
  for i in 1 2 3 4 5 6; do (cd $C && /usr/bin/time -p $B $nd ask --stale 'штраф за отмену записи' >/dev/null 2>>$S/perf4-serve-$arm.txt); done
  grep real $S/perf4-serve-$arm.txt
done
pkill -f 'repograph serve' || true
```

Expected: after the first (which opens the model in the server), dense `real` under 0.10 s and lexical under 0.05 s. The first-run value is recorded as well.

- [ ] **Step 6: Commit**

```bash
git -C $W add src/serve.rs src/ask.rs src/main.rs tests/serve.rs
git -C $W commit -F - <<'MSG'
feat(serve): a resident process answers ask over a Unix socket

serve holds the graph, the vectors, the model and the indexes and
answers ask through .repograph/serve.sock; ask connects when a server
of the same version listens and answers in-process otherwise (no
socket, a dead one — unlinked — another version, a timeout, --no-serve
or REPOGRAPH_NO_SERVE). The server refreshes before every non-stale
answer with the walk a one-shot ask does, polls between requests like
watch and exits after --idle seconds. Socket and in-process output are
the same bytes on all 142 recorded and developer cases in both arms
(perf4 transcript); a fused ask through the socket reads <perf4 dense
median> s against <perf1 dense> s in-process on the copy.
MSG
```

---

### Task 5 (P3c): The measurement doc and the runbook

**Files:**
- Create: `docs/bench/2026-09-06-perf-results.md`
- Modify: `README.md` (Status table: `serve` row; the `--no-dense` sentence gains `serve`; the paragraph that says opening the model costs ~0.6 s gets the measured split and what `serve` does about it), `docs/bench/runbook.md` (the timing transcript recipe, `perf-time.sh` inline)

- [ ] **Step 1: The results doc**

Sections, each with its transcript name: the Task 0 stage table (medians of five, both arms); the open split (`$S/perf-open-parts.txt`); the file variants (`$S/perf-open-variants.txt`) and why none shipped; P1, P2, P3 stage tables after each; the socket timings; the Rule 1 and Rule 2 evidence (dump hashes equal, `identical 284 differing 0`); what is not done (persisting BM25, mmap, int8) with the number. Every number copied from the named file, none typed from memory.

- [ ] **Step 2: README and runbook**

README Status gains `| `serve` | working; opt-in resident answerer for `ask`; same bytes, measured <n> ms per fused ask after the first |`; the `--no-dense` list adds `serve`. The runbook gains a "Timing an ask" recipe: the `perf-time.sh` script inline and the rule that a performance change is judged on dumps first.

- [ ] **Step 3: Commit**

```bash
git -C $W add docs/bench/2026-09-06-perf-results.md README.md docs/bench/runbook.md
git -C $W commit -F - <<'MSG'
docs(bench): the ask timings before and after P1-P3, and how to take them
MSG
```

---

### Task A1: `ask --expand` — planned only if the proxy passed

**Pre-registered selection (from `$S/rulings.txt`, written before the results were read):** the variant is chosen on the held-out 400 alone — the largest sum of dense and lexical hits that loses no more than it gains in either arm (McNemar α 0.05 against base); recorded 82 and developer 60 are validation only. A variant not significantly better than base on the held-out in at least one arm is recorded and not planned.

**The proxy's results** (`$S/qrw-full.txt`, script `$S/qrw.py`, rewrites cached in `$S/qrw-rewrites.json`; haiku through the `ENRICH_COMMAND` shape; the fixture asked with `--stale --json --seeds 10`; hits by the `bench::found` rule):

| variant | held-out 400 dense (`bc-ho`) | held-out 400 lexical (`bc-ho`) | recorded 82 dense | recorded 82 lexical | developer 60 dense | developer 60 lexical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| base (the question as asked) | 107/400 | 114/400 | 68/82 | 65/82 | 34/60 | 36/60 |
| R1 rewrite replaces | 40/400 (−84/+17, p 0.000) | 44/400 (−92/+22, p 0.000) | 18/82 (−52/+2, p 0.000) | 14/82 (−52/+1, p 0.000) | 26/60 (−17/+9, p 0.169) | 20/60 (−20/+4, p 0.002) |
| R2 keywords appended | 101/400 (−42/+36, p 0.572) | 108/400 (−42/+36, p 0.572) | 52/82 (−20/+4, p 0.002) | 47/82 (−21/+3, p 0.000) | 32/60 (−6/+4, p 0.754) | 33/60 (−7/+4, p 0.549) |
| R3 round-robin base + R1 | 93/400 (−22/+8, p 0.016) | 104/400 (−16/+6, p 0.052) | 64/82 (−5/+1, p 0.219) | 63/82 (−3/+1, p 0.625) | 34/60 (−4/+4, p 1.000) | 37/60 (−2/+3, p 1.000) |
| R4 two developer questions replace | 63/400 (−71/+27, p 0.000) | 54/400 (−85/+25, p 0.000) | 31/82 (−40/+3, p 0.000) | 19/82 (−49/+3, p 0.000) | 27/60 (−12/+5, p 0.143) | 27/60 (−18/+9, p 0.122) |
| R5 round-robin base + R4 | 96/400 (−21/+10, p 0.071) | 110/400 (−14/+10, p 0.541) | 68/82 (−2/+2, p 1.000) | 62/82 (−4/+1, p 0.375) | 34/60 (−4/+4, p 1.000) | 39/60 (−1/+4, p 0.375) |

Per kind on the validation suites (hits per kind, `base` then the variants that were not significantly worse on the held-out):

- cases-dense: base code 12/12 keyword 40/40 paraphrase 16/30; R2 code 12/12 keyword 28/40 paraphrase 12/30; R3 code 12/12 keyword 38/40 paraphrase 14/30; R5 code 12/12 keyword 39/40 paraphrase 17/30
- cases-lexical: base code 12/12 keyword 38/40 paraphrase 15/30; R2 code 12/12 keyword 23/40 paraphrase 12/30; R3 code 12/12 keyword 37/40 paraphrase 14/30; R5 code 12/12 keyword 37/40 paraphrase 13/30
- dev-cases-dense: base cross 11/15 long 10/15 multi 9/12 rule 4/9 where 0/9; R2 cross 11/15 long 10/15 multi 9/12 rule 2/9 where 0/9; R3 cross 9/15 long 12/15 multi 10/12 rule 3/9 where 0/9; R5 cross 11/15 long 10/15 multi 9/12 rule 4/9 where 0/9
- dev-cases-lexical: base cross 10/15 long 12/15 multi 10/12 rule 4/9 where 0/9; R2 cross 11/15 long 11/15 multi 8/12 rule 3/9 where 0/9; R3 cross 9/15 long 12/15 multi 12/12 rule 4/9 where 0/9; R5 cross 11/15 long 12/15 multi 11/12 rule 5/9 where 0/9

The first run (`$S/qrw-full.txt`) asked the held-out on the fixture itself and read base 372/400 dense, 377/400 lexical: the held-out questions are generated questions still in the fixture's index, so those rows are void and only `bc-ho` (the store with all 400 removed from `questions.json` and their dense rows marked free, built by `$S/mkho.py`) counts — its base of 107/114 sits beside the LOO dump's 103/109. Rewrite cost over the 542 questions, two prompts: 474,905 bytes of prompts and 316,325 of answers for the first, 459,187 and 201,178 for the second — about 362,898 tokens, ≈ $0.2 on haiku at list price (an estimate).

**Verdict:** no variant is significantly better than base on the held-out in either arm. Replacing the question (R1, R4) loses most of what the generated-questions index answers — the index already holds question-shaped paraphrases, and a rewrite in spec vocabulary or in another developer's words moves the query away from them; on the recorded suite R1 and R4 also drop the exact-phrase keyword cases (8/40 and 18/40 of 40). Appending keywords (R2) is indistinguishable from base on the held-out (p 0.57 both arms) and loses keyword cases on the recorded suite (28/40, 23/40). Fusing a second query's seeds (R3, R5) is within noise on the held-out (R3 worse in the dense arm, p 0.016) and equal to base on the recorded and developer suites (R5: 68/82, 62/82; 34/60, 39/60, p 0.38). By the pre-registered rule `--expand` is recorded and not planned; no code, no flag. What would change the verdict is a rewrite that targets the passages rather than the questions (the dense passages list) — a different lever, to be proxied the same way before any code, and still opt-in at 2.9 s per call.

---

### Task A2: A kind router for the seats — waits for the weak-spots plan's Task 3

Not started until `docs/superpowers/plans/2026-09-05-weak-spots-close.md` Task 3 has its number. If the score-gated code seat reaches `where` below 4/9 on the developer suite v2 in both arms, this task is written as a plan section then: a haiku call classifies the question (`where` / `rule` / `cross` / other) through the `ENRICH_COMMAND` shape, the seat fires on the class instead of the score, measured under Rule 3 with the same proxy-first approach as A1 (classify the held-out questions first, count how often the class agrees with the case's kind, only then write code). Cost per question is one short haiku call (about 150 tokens; ~$0.0003 at list price — an estimate) and the CLI's 2.9 s, so it is opt-in like A1.

---

### Task 6: History, artifact, PR

**Files:**
- Modify: `bench/history/runs.jsonl` (four rows at the branch head via `bench/history/run-repograph.sh` — identical numbers, by rule), the artifact `$S/blast-radius.html` (statuses of this plan's rows), `$S/pr-perf.md` (PR body)

- [ ] **Step 1: History**

```bash
REPO=$W $W/bench/history/run-repograph.sh 2>&1 | tail -4
CASES=bench/dev-cases.jsonl REPO=$W $W/bench/history/run-repograph.sh 2>&1 | tail -4
git -C $W add bench/history/runs.jsonl
git -C $W commit -F - <<'MSG'
chore(history): both suites at the perf branch head, numbers unchanged by rule
MSG
```

Expected: the recorder prints the same readings as the rows at `6a9b5e6` (the developer suite prints one `NOT COMPARABLE 59 → 60` once, from the key fix in `60eaa99`).

- [ ] **Step 2: Push, PR**

```bash
git -C $W push -u origin feat/perf
gh auth switch --user devmaxxx && gh pr create --repo devmaxxx/repograph --base main --head feat/perf --title "perf: ask without the network round trip, the model opened early, and serve" --body-file $S/pr-perf.md
```

The PR body: what changed, the stage tables before/after (from the results doc), Rule 1 and Rule 2 evidence, what was measured and not shipped, no attribution trailers, no session links.

- [ ] **Step 3: The artifact**

Set each row of this plan on the board to its status (`выполнено` / `улучшено` / `не получилось` / `на доработку`), republish to the same URL.

---

### Task 7: Release (Max's decision)

Not executed by this plan. When Max says so: bump `Cargo.toml` and the three `npm/*/package.json` to `0.5.0`, `git tag v0.5.0`, and the README status line names 0.5.0 as released. The plan records the versions to touch; the decision and the tag are his.

---

## Self-review

- **Spec coverage:** P1 → Task 1; P2 → Task 2; P3 → Tasks 3–5; A1 → Task A1 (measured before the plan, planned or recorded by its rule); A2 → its own gate; rules 1–5 → Global Constraints and each task's measurement step; non-goals stated in the spec, repeated in Task 5's "not done" section.
- **Placeholders:** Task A1's table and verdict are filled from `$S/qrw2-full.txt`; the two `<perfN median>` slots in commit messages are filled by the implementer from the named transcripts at commit time, as the steps say.
- **Type consistency:** `ask::Request` fields match `Cmd::Ask`'s flags plus `no_dense`; `Context::open(repo, cfg, stale, no_dense)`, `answer(&Request) -> Result<String>`, `notices() -> Vec<String>`, `adopt(&Watcher, Option<UpdateReport>)` are used with those names in Tasks 3 and 4; `serve::{Hello, Reply, try_ask, run, socket_path, VERSION}` as declared; `warm_model(dense, whole, model)` as declared in Task 2 and used by `Context` in Task 3.
