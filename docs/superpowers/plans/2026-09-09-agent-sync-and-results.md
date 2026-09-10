# Agent Sync and Result Quality Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One agent-facing surface, installed identically for Claude Code and Codex, that costs a session a few hundred tokens instead of thousands; a uniform machine-readable output on every command an agent calls; an answer that arrives in milliseconds because a resident process holds the indexes; and the three accuracy levers the 82-case reranker run left unread, measured before any of them is sold as a default.

**Architecture:** Three layers, and each is testable without the next. **The surface** is a directory `agent/` in this repository — one hook script dispatched by event, one skill, one subagent definition, one stanza, one installer — from which `repograph install-agent` writes a consuming repository's `.claude/` **or** `.codex/`, so the two harnesses are two installs of one source. **The contract** is Rust: `prime` for the session brief, `--json` on the four commands that lack it, and a stable JSON shape an agent can parse without reading prose. **The results** are bench work: the reranker's own p90 bar, the pool-rank histogram behind its fourteen gains, a metered token count, and rows for the models nobody has run.

**Tech Stack:** Rust 2021 pinned at 1.98.0, dependencies `=`-pinned, no new crates. Node ≥ 18 for the hooks (already required by the npm launcher). Python 3 for scoring, in the `bench/` style. Claude Code 2.1.263, Codex CLI 0.147.0.

**Spec:** `docs/superpowers/plans/2026-09-06-agent-surface.md` — its *Decision* section is the spec for the surface, and it is 1,035 lines of argument this plan does not repeat. **That plan is the predecessor, not a duplicate: Tasks 0–8 of it stand as written, subject to the deltas in the next section.** What this plan adds is everything decided or measured since it was written, plus the accuracy and speed work it deliberately left out. Gaps referenced: G24, G26, G28, G29, G30, G31, G37 in `docs/bench/next-version-gaps.md` at `origin/main` (`1e185ed`).

## Global Constraints

- **`main` is behind.** The clone's `main` is `fec9496`; `origin/main` is `1e185ed`. Fast-forward before cutting anything. All line numbers are read on `1e185ed`.
- **The fixture `~/bench/beauty-crm-502e8a6d` is read-only.** Every command against it is `--stale --no-dense`, or `bench`/`dump`/`verify`/`explain`/`families`. A task that needs a writer copies the store first, and a copy destined for `enrich`/`embed` has `embed_model` set to `intfloat/multilingual-e5-small` **before** the writer runs.
- **Hooks never open the dense model.** Every hook invocation carries `--no-dense` and `--stale`. A hook that writes the store is confined to consuming repositories and to the harness worktree; nothing a hook runs touches the fixture.
- **No model token is spent by a task that does not name its ceiling first.** Each measuring task states the number of questions and the model before it runs, and caps the harness with `--max-budget-usd`.
- **Token sizes** of command output are UTF-8 bytes ÷ 4, the README's proxy, which under-counts Cyrillic by roughly half; where a model reports its own count, that number wins and the method is named beside it.
- **ADR-001 applies:** a floor is committed before its number is read.
- **Task 1 of `2026-09-09-critical-defects.md` changes the config trust boundary** — a project's `repograph.toml` may no longer name `enrich_command`/`rerank_command`. Anything this plan installs that configures a transport writes the **machine** file. Land that plan first, or write the installer against the new rule and note the ordering in the PR.
- Comments say why, never what. No ticket ids. Conventional Commits subjects. The commit hook rejects AI attribution trailers, session links, and any single shell command carrying both a heredoc and `git commit` unless the heredoc's first line is the subject — so plan chunks and skill text are written with the Write tool, never appended by heredoc.
- Any `gh` call runs `gh auth switch --user devmaxxx && gh …` in the same shell command.
- Agent-facing text (stanza, brief, skill, hook lines) is English: one text serves both harnesses.

---

## What moved under the predecessor plan, and what each delta costs it

The 2026-09-06 plan was written against `868f4c1`. Five things landed since, and each one changes a line of it. This table is the whole of the rebase; Task 0 applies it.

| Landed | Where the predecessor assumed otherwise | The delta |
|---|---|---|
| **PR #21 — families derived from definitions** (`9db1f0f`) | The stanza and brief describe `id_families` as a setting; `verify` reports "in families never declared" | The brief reports the **derived** family count and cites `repograph families`; no installed text tells an agent to configure families, and `repograph.toml` printing a notice for those two keys is expected output, not an error |
| **PR #22 — `core.quotepath=false` on `changes`** (`1e185ed`) | The PostToolUse risk hook was drafted against a `changes` that silently dropped any file with a non-ASCII path | The risk hook is now correct on a Cyrillic-named file, which is most of the beauty-crm corpus's *content* and none of its paths; the harness gains one case with such a path so the hook is proved on it |
| **PR #20 — `threads` became `resources`** (`6b78837`, breaking) | Any installed config or doc naming `threads` | The installer never writes `threads`; the skill's performance paragraph names `resources` |
| **PR #13 + #21 — `enrich_model` / `rerank_model` are config, sonnet reranks** | The predecessor's scout task picks a model in prose | The scout reads the configured model; the stanza names no model at all |
| **Windows binary shipped** (`cfd4c40`) | Hooks and the installer are POSIX-shaped | `install-agent` writes the same JSON on Windows; the hook script is Node and already portable; any hook line that shells out uses the launcher, not `sh` |

---

## File Structure

- `agent/hook.mjs` — one script, dispatched on `SessionStart`, `PreToolUse`, `PostToolUse`, `SubagentStart`. All gates live here.
- `agent/stanza.md` — the ten lines that go in `CLAUDE.md` / `AGENTS.md`.
- `agent/rule.txt` — the three lines a subagent is handed.
- `agent/skill/SKILL.md` — routing only: which command answers which question.
- `agent/subagent.md` — the scout definition.
- `agent/install.rs` is **not** a file: `repograph install-agent` embeds the four texts above with `include_str!` and writes them, so a version of the binary and the surface it installs cannot disagree.
- `src/prime.rs` — the session brief, text and JSON.
- `bench/agent/` — the harness: tasks, runner, scorer (predecessor Task 1).

---

### Task 0: Sync, branch, and apply the delta table to the predecessor plan

**Files:**
- Modify: `docs/superpowers/plans/2026-09-06-agent-surface.md`

- [ ] **Step 1: Fast-forward and cut the branch**

```bash
git -C /Users/max/Documents/projects/repograph fetch origin && git -C /Users/max/Documents/projects/repograph checkout main && git -C /Users/max/Documents/projects/repograph merge --ff-only origin/main && git checkout -b feat/agent-surface
```

- [ ] **Step 2: Write the delta into the predecessor plan**

Insert the delta table above into `docs/superpowers/plans/2026-09-06-agent-surface.md` immediately after its *Global Constraints*, under the heading `## Rebased 2026-09-09`, with one sentence saying the plan's Tasks 0–8 stand and the table is what changed beneath them. Use the Write tool.

- [ ] **Step 3: Record the tip the branch was cut from**

Add one line under that heading naming `git rev-parse --short origin/main` at the time of the cut.

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/plans/2026-09-06-agent-surface.md && git commit -m "docs(plans): the agent surface plan, rebased on what landed since it was written"
```

---

### Task 1: `repograph prime` — the session brief, and its ceiling

**Files:**
- Create: `src/prime.rs`
- Modify: `src/main.rs` (`Cmd::Prime`, the dispatch arm, `mod prime;`)
- Test: `src/prime.rs`'s own `mod tests`

**Interfaces:**
- Produces: `pub fn brief(graph: &Graph, questions: &Questions, families: &Derived, model: Option<&str>) -> Brief` and `impl Brief { fn text(&self) -> String; fn json(&self) -> String }`.
- Produces: CLI `repograph prime [--json]`, which reads the store and writes nothing. Never refreshes: a brief that rebuilds is a brief nobody can afford at `SessionStart`.

**Why a ceiling is a test and not a wish.** The predecessor measured today's priming at ≈3,850–6,150 tokens a session and drafted the brief at 530 bytes. The README is 92,687 bytes on `1e185ed` — ≈23k tokens if an agent ever reads it whole. The only thing that keeps a brief from growing into that is a test that fails when it does.

- [ ] **Step 1: Write the failing tests**

```rust
    /// The brief is read at every session start and after every compaction, so its size is a
    /// contract and not a preference. 600 bytes is the drafted 530 plus room for a long model name.
    #[test]
    fn the_brief_fits_in_its_own_budget() {
        let b = brief(&graph(), &Questions::default(), &derived(), Some("intfloat/multilingual-e5-small"));
        let t = b.text();
        assert!(t.len() <= 600, "the brief is {} bytes:\n{t}", t.len());
        assert!(t.lines().count() <= 12, "{t}");
    }

    /// A brief that claims an enriched store when the store is raw sends the agent to a command
    /// that will answer badly, which is worse than saying nothing.
    #[test]
    fn the_brief_states_the_store_it_actually_read() {
        let b = brief(&graph(), &Questions::default(), &derived(), None);
        assert!(b.text().contains("enriched=false"), "{}", b.text());
        assert!(b.text().contains("model=unnamed"), "a store with no recorded model says so");
        let j: serde_json::Value = serde_json::from_str(&b.json()).unwrap();
        assert_eq!(j["enriched"], serde_json::json!(false));
        assert!(j["nodes"].is_object() && j["families"].is_number());
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --manifest-path Cargo.toml prime::tests`
Expected: FAIL — the module does not exist.

- [ ] **Step 3: Implement the brief**

The text form, one line per fact, in this order: node counts by kind; edge count; `enriched=<bool> (<covered>/<eligible>)`; `model=<embed_model or "unnamed">`; `families=<n>`; then the five commands with their one-line jobs (`ask`, `impact`, `changes`, `trace`, `explain`) and the single rule the predecessor's `rule.txt` carries. Every number comes from the store; nothing is claimed that was not read.

- [ ] **Step 4: Wire the subcommand**

`Cmd::Prime { #[arg(long)] json: bool }`, dispatched to `prime::brief(...)` with `--stale` semantics always: the brief never refreshes.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --manifest-path Cargo.toml prime::tests`
Expected: PASS.

- [ ] **Step 6: Read it on the fixture and record the byte count**

```bash
/Users/max/Documents/projects/repograph/target/release/repograph --repo ~/bench/beauty-crm-502e8a6d --no-dense prime | tee /private/tmp/prime-fixture.txt | wc -c
```

Expected: ≤ 600. Record the number and the text in the PR body.

- [ ] **Step 7: Commit**

```bash
git add src/prime.rs src/main.rs && git commit -m "feat(cli): a session brief the store can vouch for"
```

---

### Task 2: `--json` on every command an agent calls

**Files:**
- Modify: `src/main.rs` (`Explain`, `Trace`, `Verify`, `Families` arms)
- Modify: `src/impact.rs` or wherever `explain`/`trace` render, adding a serializable shape
- Test: `tests/cli.rs` (or the crate's existing integration test file)

**Interfaces:**
- Produces: `--json` on `explain`, `trace` and `verify`, matching the shape `ask --json`, `impact --json`, `changes --json` and `families --json` already use: an object with named arrays, never a bare array, so a field can be added without breaking a parser.

**Why this is usability and not decoration.** An agent that must parse prose re-reads the prose on every call and gets it wrong when a label changes. Four of the ten commands already answer in JSON; the other three are the ones a debugging session calls most.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn every_reader_answers_in_json() {
    let repo = fixture_copy();
    for args in [
        vec!["explain", "FR-PAY-22", "--json"],
        vec!["trace", "AvailabilityService", "DatabaseService", "--json"],
        vec!["verify", "--json"],
    ] {
        let out = repograph(&repo, &args);
        let v: serde_json::Value = serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|e| panic!("{args:?} did not answer in JSON: {e}\n{}", String::from_utf8_lossy(&out.stdout)));
        assert!(v.is_object(), "{args:?} answered with a bare value");
    }
}
```

Reuse the integration harness the repository already has for CLI tests; if none exists, add `tests/cli.rs` with a `repograph()` helper that runs `env!("CARGO_BIN_EXE_repograph")` against a temporary store built from `tests/fixtures/`.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --manifest-path Cargo.toml --test cli every_reader_answers_in_json`
Expected: FAIL — `explain` rejects `--json` as an unknown argument.

- [ ] **Step 3: Implement**

Add the flag to each arm and a `#[derive(Serialize)]` view struct per command beside its renderer. Do not serialize the internal `Graph` types: a view struct is what keeps the store's layout out of an agent's parser.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --manifest-path Cargo.toml --test cli`
Expected: PASS.

- [ ] **Step 5: Document the shapes**

One table in the README's Commands section: command, flag, top-level keys. Nothing more — the shapes are the test's business.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/impact.rs README.md tests/cli.rs && git commit -m "feat(cli): explain, trace and verify answer in JSON like the rest"
```

---

### Task 3: The surface — `agent/`, the hook, the installer (predecessor Tasks 2 and 6, with the deltas applied)

**Files:**
- Create: `agent/hook.mjs`, `agent/stanza.md`, `agent/rule.txt`, `agent/skill/SKILL.md`, `agent/subagent.md`
- Modify: `src/main.rs` (`Cmd::InstallAgent { target: Claude | Codex | Both, dir: Option<PathBuf> }`)
- Create: `src/install_agent.rs`
- Test: `src/install_agent.rs`'s own `mod tests`; `agent/hook.test.mjs` run by `node --test`

**Interfaces:**
- Consumes: `prime` from Task 1 — the `SessionStart` branch of the hook runs `repograph prime` and prints its text.
- Produces: `repograph install-agent --claude|--codex|--both [--dir <repo>]`, writing `.claude/settings.json` hook entries + `.claude/skills/repograph/SKILL.md` + the stanza appended to `CLAUDE.md`, or the Codex equivalents, and printing every path it wrote or left alone. Idempotent: a second run changes nothing and says so.

**This task implements the predecessor's Decision §1–§3 as written.** The gates, the 12-injection bound, the `agent_id` keying and the Bash tokenizer are all argued there; do not re-derive them. What follows is only what the delta table changed and what the predecessor left as prose.

- [ ] **Step 1: Write the failing hook tests**

```javascript
// agent/hook.test.mjs
import { test } from 'node:test';
import assert from 'node:assert';
import { queryFromBash, shouldInject } from './hook.mjs';

test('a grep for a word is a question; a path is not', () => {
  assert.deepEqual(queryFromBash('rg -n "отмена записи" apps/'), ['отмена', 'записи']);
  assert.equal(queryFromBash('rg apps/api/src/availability.service.ts'), null);
  assert.deepEqual(queryFromBash('grep -R withTenant .'), ['withTenant']);
});

test('the session budget is a hard bound', () => {
  const state = { count: 12, seen: new Set() };
  assert.equal(shouldInject(state, 'anything'), false);
});

test('a repeated query is asked once', () => {
  const state = { count: 0, seen: new Set() };
  assert.equal(shouldInject(state, 'отмена записи'), true);
  state.seen.add('отмена записи');
  assert.equal(shouldInject(state, 'отмена записи'), false);
});
```

- [ ] **Step 2: Run them to verify they fail**

Run: `node --test agent/hook.test.mjs`
Expected: FAIL — the module does not exist.

- [ ] **Step 3: Write `agent/hook.mjs`**

One `switch` on the event name. `SessionStart` prints the brief. `PreToolUse` on `Bash` tokenizes and injects under the predecessor's gates. `PostToolUse` on `Edit|Write` runs `changes --depth 1 --json` and injects one risk line at MEDIUM or above. `SubagentStart` returns `hookSpecificOutput.additionalContext` = `rule.txt`. Every invocation adds `--stale --no-dense`; every failure is silence.

- [ ] **Step 4: Run the hook tests to verify they pass**

Run: `node --test agent/hook.test.mjs`
Expected: PASS.

- [ ] **Step 5: Write the failing installer test**

```rust
    #[test]
    fn installing_twice_writes_the_same_tree_and_says_it_changed_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let first = install(dir.path(), Target::Claude).unwrap();
        assert!(dir.path().join(".claude/settings.json").exists());
        assert!(dir.path().join(".claude/skills/repograph/SKILL.md").exists());
        assert!(first.written > 0);
        let second = install(dir.path(), Target::Claude).unwrap();
        assert_eq!(second.written, 0, "a second install is a no-op");
    }

    /// A repository with hooks of its own keeps them: the installer merges its four entries in and
    /// never rewrites the file wholesale.
    #[test]
    fn an_existing_settings_file_keeps_its_own_hooks() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::write(dir.path().join(".claude/settings.json"), r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"echo mine"}]}]}}"#).unwrap();
        install(dir.path(), Target::Claude).unwrap();
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join(".claude/settings.json")).unwrap()).unwrap();
        assert!(v["hooks"]["Stop"].is_array(), "the repository's own hook survived");
        assert!(v["hooks"]["SessionStart"].is_array());
    }
```

- [ ] **Step 6: Run them to verify they fail, implement, and run again**

Run: `cargo test --manifest-path Cargo.toml install_agent::tests`
Expected: FAIL, then PASS after `src/install_agent.rs` embeds the texts with `include_str!` and merges the settings file.

- [ ] **Step 7: Commit**

```bash
git add agent src/install_agent.rs src/main.rs && git commit -m "feat(agent): one surface, installed by the binary that answers it"
```

---

### Task 4: Codex parity, verified against the CLI rather than assumed

**Files:**
- Modify: `src/install_agent.rs` (the `Target::Codex` branch)
- Create: `agent/codex.md` — the one page recording what Codex's hook contract actually is, with the command that read it
- Test: `src/install_agent.rs`'s `mod tests`

**Interfaces:**
- Consumes: `install(dir, Target::Codex)` from Task 3.
- Produces: the Codex install, whatever shape Step 1 finds. **No file under `.codex/` is written from memory of a format.**

- [ ] **Step 1: Read the contract before writing to it**

```bash
codex --version && codex --help && codex exec --help
```

and, for the hook and instruction-file contract, dispatch a `bond:DocsExplorer` agent with: *"Codex CLI 0.147.0 — where does it read project instructions from (AGENTS.md? .codex/?), does it support lifecycle hooks, and if so what is the config file and the event names? Return the file paths, the event names, and a source link."* Write the answer, with its source, into `agent/codex.md`. If Codex has no hook mechanism, that is the finding and Step 3 changes shape.

- [ ] **Step 2: Write the failing test, from what Step 1 found**

```rust
    #[test]
    fn the_codex_install_writes_what_codex_reads() {
        let dir = tempfile::tempdir().unwrap();
        install(dir.path(), Target::Codex).unwrap();
        // The paths here are the ones agent/codex.md records; if that file says Codex reads only
        // AGENTS.md, this asserts the stanza landed there and nothing else was written.
        assert!(dir.path().join("AGENTS.md").exists());
        let stanza = std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
        assert!(stanza.contains("repograph ask"), "{stanza}");
        assert!(stanza.len() <= 1_400, "the stanza is {} bytes", stanza.len());
    }
```

- [ ] **Step 3: Implement the branch and run the test**

Run: `cargo test --manifest-path Cargo.toml install_agent::tests`
Expected: PASS.

- [ ] **Step 4: Prove it on a real Codex run**

In the harness worktree (predecessor Task 0), install for Codex and run one task through `codex exec`, recording whether the stanza reached the model and whether it called `repograph`. One task, one model, the ceiling stated first.

- [ ] **Step 5: Commit**

```bash
git add agent/codex.md src/install_agent.rs && git commit -m "feat(agent): the Codex install, written against the contract it was read from"
```

---

### Task 5: The answer arrives in milliseconds — `serve` autostart and the idle drop (G26, G24)

**Files:**
- Modify: `src/serve.rs` (drop the model after an idle period, keep the process)
- Modify: `agent/hook.mjs` (start a `serve` in the background on `SessionStart` when none is listening)
- Test: `src/serve.rs`'s `mod tests`

**Interfaces:**
- Consumes: `socket_path` from the critical-defects plan's Task 4 — a deep-path repository must be servable before a hook may rely on `serve`.
- Produces: `serve --idle-model <secs>` (default 300), independent of `--idle` (default 1800, which exits the process). The first ask after a drop pays the open and says so on stderr.

**The numbers this task is judged by.** `serve` idle holds **1.39 GB** on the small model and holds it for the life of the process; `watch` drops to **129.5 MB** between refreshes, a 7.2×. A fused `ask` through the socket is **0.41 s** cold in-process and lexical through a resident one is **6.8 ms**. The hook's whole latency budget is a 5 s timeout, so a resident `serve` is the difference between a gate that fires and one that times out on a busy machine.

- [ ] **Step 1: Write the failing test**

```rust
    /// The process stays; the weights do not. `--idle` exits, `--idle-model` forgets — a resident
    /// answer is worth 0.4 s to a person and 1.4 GB is not worth holding for the hours between.
    #[test]
    fn the_model_is_dropped_after_its_own_idle_and_reopened_on_the_next_question() {
        let mut s = Resident::for_test();
        assert!(s.embedder_open());
        s.tick(Duration::from_secs(301));
        assert!(!s.embedder_open(), "the weights went and the process stayed");
        assert!(!s.should_exit(), "only --idle exits");
        s.answer("отмена записи");
        assert!(s.embedder_open(), "the next question paid the open");
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --manifest-path Cargo.toml serve::tests::the_model_is_dropped`
Expected: FAIL — no such knob.

- [ ] **Step 3: Implement**

`serve` already counts the time since the last question that arrived (`--idle`). Add a second threshold that drops the embedder instead of exiting, and reopen lazily in the answer path. The lexical indexes stay resident: they are what makes the 6.8 ms answer and they are not the 1.4 GB.

- [ ] **Step 4: Run the test to verify it passes, then measure**

```bash
cd /private/tmp/serve-copy && /Users/max/Documents/projects/repograph/target/release/repograph serve --idle 1800 --idle-model 60 & sleep 2; /Users/max/Documents/projects/repograph/target/release/repograph --repo /private/tmp/serve-copy ask отмена записи >/dev/null; sleep 5; ps -o rss= -p $(pgrep -f "repograph serve"); sleep 70; ps -o rss= -p $(pgrep -f "repograph serve")
```

Expected: the second reading under 0.1 GB, matching the 129.5 MB `watch` reads. Record both, and the first-ask latency after the drop.

- [ ] **Step 5: Autostart from the hook**

On `SessionStart`, when `socket_path` has no listener and the store exists, spawn `repograph serve --idle 1800 --idle-model 300` detached, stdout and stderr to `/dev/null`, and never wait on it. A failed spawn is silence.

- [ ] **Step 6: Commit**

```bash
git add src/serve.rs agent/hook.mjs docs/bench/next-version-gaps.md && git commit -m "perf(serve): the weights go, the process stays, the answer stays resident"
```

---

### Task 6: What the reranker is actually buying (G30, G29, G31, G28)

**Files:**
- Modify: `src/bench.rs` (per-case pool rank of the expected id under `--rerank`; the metered prompt size)
- Create: `docs/bench/2026-09-09-rerank-diagnostics.md`
- Test: `src/bench.rs`'s `mod tests`

**Interfaces:**
- Consumes: `bench --repeat` and the anchor column from the critical-defects plan's Task 6, if that plan landed first; if not, this task runs each arm twice by hand and says so.
- Produces: two new columns on a `--rerank` case line — `pool=<rank of the expected id in the 200-deep pool, or ->` and `prompt=<bytes>` — and a summary line carrying median and p90 prompt bytes.

**Why the diagnostic comes before any lever.** The reranker reads paraphrase **29/30 against 15/30**, twice, with zero flips — and **twelve of its fourteen gains are cases no zero-token arm has ever reached**. Whether those twelve are reachable without a model is a property of *where in the 200-deep pool they sit*, which no row records. A fusion change judged against a case sitting at pool rank 140 is a change judged against the model, not against itself. G2's "nothing cheaper left to try" was written before this list existed.

- [ ] **Step 1: Write the failing test**

```rust
    /// The rank of the expected id inside the pool the reranker was shown. A case the model found
    /// at rank 6 is one a sixth seat could seat; a case at 140 is the model's alone.
    #[test]
    fn the_pool_rank_of_the_expected_id_is_recorded() {
        let pool = vec!["FR-CAL-1".to_string(), "FR-MKT-35".to_string(), "FR-PAY-22".to_string()];
        assert_eq!(pool_rank(&pool, "FR-MKT-35"), Some(2));
        assert_eq!(pool_rank(&pool, "FR-SVC-50"), None);
    }
```

- [ ] **Step 2: Run it to verify it fails, implement, run again**

Run: `cargo test --manifest-path Cargo.toml bench::tests::the_pool_rank`
Expected: FAIL, then PASS.

- [ ] **Step 3: Meter the prompt**

`rerank` already builds the prompt it sends. Record its byte length per question; print median and p90 on the summary line with the method named (`bytes`, and the model's own count where the command returns one). This is G31's whole lever: a cost that is a measurement rather than a division.

- [ ] **Step 4: Run the diagnostic, ceiling stated first**

30 paraphrase cases, one arm, sonnet at depth 200, ≈$0.03 a question ⇒ **≈$0.90, capped at $2**:

```bash
cd ~/bench/beauty-crm-502e8a6d && /Users/max/Documents/projects/repograph/target/release/repograph bench --rerank --depth 200 2>&1 | tee /private/tmp/rerank-diag.txt | tail -3
```

- [ ] **Step 5: Write the histogram and the two verdicts**

`docs/bench/2026-09-09-rerank-diagnostics.md` carries: the fourteen gained cases with their pool ranks as a histogram; `FR-MKT-35`'s rank in each of the four lists (G29 — in the pool means the lever is the 120-character snippet, outside means it is depth or fusion); the metered prompt median and p90 against the README's ≈19k estimate; and a proposed p90 bar **of the reranked arm's own**, set from the two runs that read 228 and 231 (G28), written down before the next run reads it.

- [ ] **Step 6: Commit**

```bash
git add src/bench.rs docs/bench/2026-09-09-rerank-diagnostics.md docs/bench/next-version-gaps.md && git commit -m "test(bench): where the reranker's gains sit, and what its prompt actually costs"
```

---

### Task 7: A row for every model an agent might actually run (G37)

**Files:**
- Modify: `README.md` (the `--rerank` table)
- Create: `docs/bench/2026-09-09-cross-vendor-results.md`

**Interfaces:**
- Consumes: Task 6's columns, so each row carries pool rank and prompt bytes as well as hits and p90.

**What has never been read.** The README shows Codex and Ollama as command shapes taken from their `--help` and never run. Two accuracy levers have never been stacked: `e5-large` rows (paraphrase 22/30) and the sonnet reranker (29/30 over small rows). And the reranker has never been run over a store carrying `enrich --code` questions — the fifth list the pool is built to take.

- [ ] **Step 1: Prepare the copies**

```bash
for tag in ollama codex e5large codeenriched; do cp -R ~/bench/beauty-crm-502e8a6d /private/tmp/xv-$tag; done && grep -n embed_model /private/tmp/xv-*/repograph.toml
```

Confirm each copy's `embed_model` before any writer runs on it: `xv-e5large` is the e5-large store's copy, the rest are the small model's.

- [ ] **Step 2: Run the four arms, each with its ceiling stated**

Ollama and the code-enriched arm are wall-priced, not dollar-priced; `codex exec` is capped the way Task 6 caps sonnet. Each run writes its own tagged output file under `/private/tmp/`.

- [ ] **Step 3: Write the rows, including the refusals**

A configuration that cannot be run — a model not installed, a CLI that rejects the prompt shape — gets a row saying so with the command and the error. A blank is not a row.

- [ ] **Step 4: Update the README table and commit**

```bash
git add README.md docs/bench/2026-09-09-cross-vendor-results.md && git commit -m "test(bench): the reranker read on models other than the one that shipped it"
```

---

### Task 8: The README is not the agent's document

**Files:**
- Modify: `README.md`
- Create: `docs/history.md`

**Why.** `README.md` is 92,687 bytes on `1e185ed` — ≈23k tokens. It is the only document an agent finds by default, and most of it is campaign history: model comparisons, measurement narratives, gap arguments. The stanza and the brief only work if the document behind them is the one an agent should read.

- [ ] **Step 1: Measure before cutting**

```bash
wc -c README.md && awk '/^## /{print NR, $0}' README.md
```

- [ ] **Step 2: Move history out**

Every section that argues a past measurement moves to `docs/history.md` with its numbers intact and a link from the README. What stays: install, the commands and their flags, the JSON shapes (Task 2), Configure (with Task 1 of the critical plan's trust rule), Embeddings, `serve`, and the current results table.

- [ ] **Step 3: Prove nothing was lost**

```bash
wc -c README.md docs/history.md && git show origin/main:README.md | wc -c
```

Expected: the two together within a few hundred bytes of the original; the README itself under 30,000.

- [ ] **Step 4: Commit**

```bash
git add README.md docs/history.md && git commit -m "docs: the README is what a reader needs now, the history is beside it"
```

---

### Task 9: The harness verdict, the review, and the PR

**Files:**
- Create: `bench/agent/` (predecessor Task 1)
- Create: `docs/bench/2026-09-09-agent-surface-results.md`

- [ ] **Step 1: Run the harness on overlays B and C**

Execute the predecessor's Task 1 and Task 3 as written: the same tasks through configuration B (stanza + brief, reminder only) and configuration C (stanza + brief + interception), both models, tokens and hit rate from the harness's own `modelUsage`.

- [ ] **Step 2: Apply the predecessor's committed rule**

Interception ships when its **tokens per hit are lower than reminder-only's on both models**; otherwise reminder-only ships and the interceptor stays behind `REPOGRAPH_HOOK_INTERCEPT=0`. The rule was committed before the number was read; it is not renegotiated after.

- [ ] **Step 3: Write the results document**

Priming tokens before and after, per session; hit rate; tokens per hit for both configurations; the brief's byte count; the p95 hook latency with and without a resident `serve`; and every refusal.

- [ ] **Step 4: Full suite, lint, review**

```bash
cargo test --manifest-path /Users/max/Documents/projects/repograph/Cargo.toml && cargo clippy --manifest-path /Users/max/Documents/projects/repograph/Cargo.toml --all-targets -- -D warnings
```

Then `/code-review` with `--fix` at the level the router reads off the diff; apply, re-run, commit the applied findings separately.

- [ ] **Step 5: Open the PR as a draft**

```bash
gh auth switch --user devmaxxx && gh pr create --draft --title "feat(agent): one surface for two harnesses, a brief the store vouches for, and what the reranker actually buys" --body-file /private/tmp/agent-surface-pr.md
```

The body carries the priming numbers before and after, the harness verdict against the committed rule, the JSON shapes added, the `serve` idle readings, and the reranker diagnostics — each with the command that produced it.
