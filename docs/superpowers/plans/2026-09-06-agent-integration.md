# The Agent Surface — Implementation Plan

> **Superseded, never executed.** This is the 2026-09-06 draft of the agent surface as an MCP server in the
> binary. It was replaced by [`2026-09-06-agent-surface.md`](2026-09-06-agent-surface.md), merged in #24, which decided that no MCP
> server is built. Nothing below is implemented; it is kept as the record of the alternative that was weighed.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make repograph a first-class tool for coding agents without moving a single measured retrieval number: a machine-readable CLI contract (`status`, `--json` on every reader), an MCP server in the binary (`repograph mcp`) exposing the five readers as five capped tools over the resident `ask::Context`, a Claude Code plugin that packages it with one skill, one setup command and one reminder hook, a Codex path through the same server — and an agent benchmark, with its rule written before its numbers, that decides which of the two surfaces is the default.

**Architecture:** repograph is a Rust CLI (`src/`) over a `.repograph/` store. `ask::Context` already holds everything an answer needs across requests and `serve.rs` already refreshes it per request and polls between them over a Unix socket; `mcp.rs` is the same loop with stdin/stdout JSON-RPC as the transport and the four other readers beside `ask`. Every tool answers the bytes its CLI twin prints, capped for hub symbols, notices first. The plugin under `plugin/` holds no logic. The agent bench under `bench/agent/` runs three headless-harness arms on a copy of the fixture and grades against the cases' own anchors.

**Tech Stack:** Rust 2021 edition at `rust-version = "1.98"` (`cargo test --release`, `cargo clippy --release --all-targets -- -D warnings`), the existing dependency set — `serde_json` carries the protocol, no new crate; Python 3 stdlib only (`bench/agent/*.py`, `plugin/hooks/notice.py`); bash 3.2 (`plugin/bin/repograph-mcp`); `claude` 2.1.263 and `codex` 0.147.0 as the harnesses on this machine.

**Spec:** `docs/superpowers/specs/2026-09-06-agent-integration-design.md` — the decision, the seven questions, the five rules. The spec outranks this plan where they disagree.

## Global Constraints

- Work in a worktree of `devmaxxx/repograph` branched from `origin/main` at `32577e2` (0.5.0); name it `feat/agent-surface`. Never `cd` to `/Users/max/Documents/projects/repograph` (a dirty checkout of the same repository): use `git -C "$W"`, `cargo … --manifest-path "$W/Cargo.toml"`, absolute paths. `W` is the worktree path, `B="$W/target/release/repograph"`, rebuilt with `cargo build --release --manifest-path "$W/Cargo.toml"` after every code change and before any measurement.
- Scratch is `A=/Users/max/bench/agent-2026-09-06` (created in Task 0). Every number written into a doc, a commit, a test comment or the README comes from a named file under `$A` or from a test run. A token cost is the one estimate allowed and says it is one.
- The fixture `F=/Users/max/bench/beauty-crm-502e8a6d` is **read-only**: only `bench`, `dump`, `ask --stale`, `impact --stale`, `explain`, `trace --stale`. Never `build`, `update`, `enrich`, `embed`, `serve`, `watch`, `mcp` or a bare `ask` on it, never edit its files. The copy every writer, every `serve` and every agent arm runs on is `C=$A/bc`, made in Task 0 from `S=/Users/max/bench/perf-2026-09-06/bc-perf` (the corpus at `502e8a6d` with a small-model store beside it — a store with no tree beside it empties itself on the first refresh, runbook trap 7).
- **Rules are written before numbers and never re-read to fit them.** They are the spec's "The rules, in one place" and are repeated in the task that applies each. A rule that fails is recorded with its numbers; no cap, ceiling or threshold is tuned after a suite has been read.
- **Rule 1 after every code task:** `dump` of `bench/cases.jsonl` and `bench/dev-cases.jsonl` on the fixture in both arms, and `bench` in both arms, byte-identical to Task 0's baselines. This plan adds readers and caps; it changes no answer.
- Model tokens are spent only in Task 7, only through the machine's configured harness, only on `$C`. Nothing in the tool surface names a model vendor: the plugin's command runs the binary, the bench runs whatever `--agent-command` names.
- Commit hygiene: Conventional Commits (`feat(mcp): …`, `feat(cli): …`, `docs(agents): …`, `chore(bench): …`); prose bodies with the measured numbers and the files they came from. The hook rejects `Co-Authored-By`, `Claude-Session` and any `claude.ai` link — none in a commit or the PR body. It also rejects one shell command holding both a heredoc and a literal `git commit` unless the heredoc's first line is a Conventional subject: `git commit -F - <<'MSG'` with the subject first works; a `python3 - <<'PY'` edit and a `git commit` go in two commands. In zsh, quote `===` in `echo`.
- Any `gh` call: `gh auth switch --user devmaxxx && gh …` in one command. The PR goes to `devmaxxx/repograph`, base `main`; the executor does not merge.
- Comments say why, never what; no ticket ids in code comments; tool directives stay. Test names are `snake_case` sentences in the style of their neighbours. Python is stdlib only, run as `python3 …`; bash is 3.2 (no associative arrays, `${arr[@]+"${arr[@]}"}` for empty arrays under `set -u`).
- Implementers never dispatch subagents.

---

## File Structure

| File | Responsibility | Tasks |
|---|---|---|
| `src/status.rs` (new) | the store in one line: `Status`, `read`, `render`, `render_json` | 1 |
| `src/query.rs` | `explain_capped`, `explain_json` | 1 |
| `src/impact.rs` | `render_capped`, `render_trace`, `render_trace_json` | 1 |
| `src/changes.rs` | `render_capped` | 1 |
| `src/index/dense.rs` | `recorded_rows`; later `sync` bounded by `max_new`, `Synced` | 1, 4 |
| `src/main.rs` | `Status`, `Explain --json`, `Trace --json`, `Mcp` subcommands; `Watcher` fields `pub(crate)` | 1, 2, 3 |
| `src/serve.rs` | `fresh` extracted from `answer`; `adopt_if_moved`, `WAKE` shared | 2 |
| `src/ask.rs` | `Context::graph`, `model_open`, `release_model`; later `inline_rows`, `embed_pending` | 2, 4 |
| `src/mcp.rs` (new) | the protocol loop, `find_root`, the five tools, the ceilings | 3 |
| `tests/mcp.rs` (new) | the server driven over stdio the way a harness drives it | 3 |
| `plugin/` (new) | `.claude-plugin/plugin.json`, `.mcp.json`, `bin/repograph-mcp`, `skills/repograph/SKILL.md`, `commands/setup.md`, `hooks/hooks.json`, `hooks/notice.py`, `hooks/test_notice.py` | 5 |
| `.claude-plugin/marketplace.json` (new, repository root) | the one-entry marketplace pointing at `./plugin` | 5 |
| `README.md` | Status rows for `mcp` and `status`; a "For agents" section | 5, 7 |
| `bench/agent/` (new) | `mcp_client.py`, `tasks.py`, `run.py`, `test_agent.py`, `tasks.jsonl`, `runs.jsonl` | 3, 6, 7 |
| `docs/bench/2026-09-06-agent-surface-results.md` (new), `docs/adr/ADR-002-the-agent-surface.md` (new), `bench/history/runs.jsonl` | the record | 7 |

Task order: 0 → 1 → 2 → 3 → 4 → 5 → 6 → 7. After Task 3 an agent has a working MCP path; after Task 1 a shell-driving agent has the contract. Task 4 is an improvement to Task 3's server and is independent of Tasks 5 and 6; Tasks 5 and 6 are independent of each other.

---

### Task 0: Worktree, scratch, the copy, the baselines, the yardsticks

**Files:**
- Create (outside the repo): `$A/`, `$A/bc/` (the copy), `$A/base-*.json`, `$A/base-*.txt`, `$A/base-shasums.txt`, `$A/yardstick.txt`, `$A/task0.txt`
- Nothing in the repo changes; no commit.

**Interfaces:**
- Produces: the Rule 1 baselines every later task compares against (`$A/base-{rec,dev}-{dense,lexical}.json`, `$A/base-bench-{rec,dev}-{dense,lexical}.txt`), the copy `$C`, and `$A/yardstick.txt` — the hub-symbol sizes the caps in Task 1 are justified by, re-read on this binary.

- [ ] **Step 1: Worktree and scratch**

```bash
cd /Users/max/Documents/projects/repograph/.worktrees/coverage && git fetch origin
git -C /Users/max/Documents/projects/repograph/.worktrees/coverage worktree add -b feat/agent-surface /Users/max/Documents/projects/repograph/.worktrees/agent origin/main
W=/Users/max/Documents/projects/repograph/.worktrees/agent; A=/Users/max/bench/agent-2026-09-06; mkdir -p "$A"
git -C "$W" rev-parse --short HEAD | tee "$A/task0-head.txt"
```

Expected: `32577e2`. A different head means `main` moved; record it and continue — the baselines below are taken on whatever the branch point is.

- [ ] **Step 2: The copy**

```bash
S=/Users/max/bench/perf-2026-09-06/bc-perf; C=/Users/max/bench/agent-2026-09-06/bc
[ -d "$S/.repograph" ] && git -C "$S" rev-parse --short HEAD
cp -R "$S" "$C"
rm -f "$C/.repograph/serve.sock"
ls -la "$C/.repograph" | tee "$A/task0-copy.txt"; git -C "$C" status --porcelain -- ':(exclude)graphify-out' | head
```

Expected: `502e8a6d`; seven store files (`graph.json`, `graph.bin`, `manifest.json`, `questions.json`, `questions.bin`, `vectors.f32`, `vectors.json`) and no socket; a clean status. If `$S` is gone, make the copy from the fixture instead — `cp -R "$F" "$C"` — and say so in `$A/task0.txt`; it is the same corpus and the same store.

- [ ] **Step 3: Rule 1 baselines on the branch-point binary**

```bash
F=/Users/max/bench/beauty-crm-502e8a6d; B="$W/target/release/repograph"
cargo build --release --manifest-path "$W/Cargo.toml" 2>&1 | tail -1; shasum "$B" | tee "$A/base-binary.txt"
for arm in dense lexical; do
  nd=""; [ $arm = lexical ] && nd="--no-dense"
  "$B" --repo "$F" $nd dump --queries "$W/bench/cases.jsonl"     --out "$A/base-rec-$arm.json" >/dev/null 2>&1
  "$B" --repo "$F" $nd dump --queries "$W/bench/dev-cases.jsonl" --out "$A/base-dev-$arm.json" >/dev/null 2>&1
  "$B" --repo "$F" $nd bench                                    2>&1 | tail -1 > "$A/base-bench-rec-$arm.txt"
  "$B" --repo "$F" $nd bench --cases "$W/bench/dev-cases.jsonl" 2>&1 | tail -1 > "$A/base-bench-dev-$arm.txt"
done
shasum "$A"/base-*.json | tee "$A/base-shasums.txt"; cat "$A"/base-bench-*.txt
```

Expected bench lines (the last history rows, `bench/history/runs.jsonl`): recorded `keyword 40/40  paraphrase 15/30  code 12/12  p90 221 tok  dense=true …` and `keyword 39/40  paraphrase 15/30  code 12/12  p90 215 tok  dense=false …`. A different reading is the before, written down, not an error.

- [ ] **Step 4: The yardsticks, re-read on this binary**

The caps in Task 1 rest on these; they were read once while this plan was written and are read again here so the results document cites a file:

```bash
{
  for s in DatabaseService ProblemException StaffService TenantContextInterceptor; do
    out=$("$B" --repo "$F" impact --stale "$s" 2>/dev/null)
    printf 'impact %-26s lines=%4d bytes=%6d tok=%5d  %s\n' "$s" "$(printf '%s\n' "$out" | wc -l)" "${#out}" "$(( ${#out} / 4 ))" "$(printf '%s\n' "$out" | tail -1)"
  done
  for s in DatabaseService FR-PAY-22 "file:apps/api/src/modules/staff/staff.service.ts"; do
    out=$("$B" --repo "$F" explain "$s" 2>/dev/null)
    printf 'explain %-50s lines=%4d bytes=%6d tok=%5d\n' "$s" "$(printf '%s\n' "$out" | wc -l)" "${#out}" "$(( ${#out} / 4 ))"
  done
  out=$("$B" --repo "$F" changes --stale --base 'HEAD~30' 2>/dev/null)
  printf 'changes --base HEAD~30 lines=%d bytes=%d tok=%d  %s\n' "$(printf '%s\n' "$out" | wc -l)" "${#out}" "$(( ${#out} / 4 ))" "$(printf '%s\n' "$out" | tail -1)"
} | tee "$A/yardstick.txt"
ls -la --time-style=full-iso "$F/.repograph" 2>/dev/null || ls -la "$F/.repograph"
```

Expected, as read on 2026-09-06 on the 0.5.0 binary: `impact DatabaseService lines= 156 bytes= 35443 tok= 8860 … risk: CRITICAL — 61 direct, 150 total, 69 files`, `ProblemException … tok= 4097`, `StaffService … tok= 225`, `TenantContextInterceptor … tok= 188`; `explain DatabaseService … tok= 251`, `FR-PAY-22 … tok= 194`, the file node `tok= 219`; `changes --base HEAD~30 lines=3190 bytes=379297 tok=94824 … risk: CRITICAL — 175 direct, 189 total, 103 files`. The store's file times must be unchanged by all of this — every command above is a read.

- [ ] **Step 5: The task record**

`$A/task0.txt`: the head, the copy's listing, the four bench lines, the binary's sha, the yardstick. Report DONE with that path.

---

### Task 1: The CLI contract — `status`, `--json` on every reader, the capped renderers

**Files:**
- Create: `src/status.rs`
- Modify: `src/main.rs` (module list; `Cmd::Status`, `Cmd::Explain { json }`, `Cmd::Trace { json }`; the three match arms), `src/query.rs` (`explain_capped`, `explain_json`), `src/impact.rs` (`render_capped`, `render_trace`, `render_trace_json`), `src/changes.rs` (`render_capped`), `src/index/dense.rs` (`recorded_rows`)
- Create (scratch): `$A/t1-*.json`, `$A/t1-*.txt`

**Interfaces:**
- Produces:
  - `status::Status { root: String, store: bool, nodes: usize, edges: usize, changed: usize, removed: usize, questions: (usize, usize), code_questions: (usize, usize), model: Option<String>, rows: usize, serve: bool }`, `status::read(repo: &Path, cfg: &Config) -> Result<Status>`, `status::render(&Status) -> String` (one line), `status::render_json(&Status) -> String`. Task 3's `initialize` and first result carry `render`'s line; Task 5's hook and command call `status --json`.
  - `query::explain_capped(graph, needle, limit: usize) -> Option<String>` — `explain`'s text with at most `limit` edge lines then `  … N more edges (limit L)`; `query::explain(graph, needle)` becomes `explain_capped(graph, needle, usize::MAX)` and prints the same bytes as before. `query::explain_json(graph, needle) -> Option<String>`.
  - `impact::render_capped(graph, imp, direction, limit) -> String` — `render`'s text with at most `limit` lines per layer then `  … N more (limit L)`; `render` = `render_capped(…, usize::MAX)`. `impact::render_trace(graph, path: &[String]) -> String` (the text `main.rs` prints today), `impact::render_trace_json(graph, from, to, depth, path: Option<&[String]>) -> String`.
  - `changes::render_capped(graph, r, limit) -> String` — the `changed:` and `affected` lists each capped the same way; `render` = `render_capped(…, usize::MAX)`.
  - `DenseIndex::recorded_rows(store) -> Result<usize>` — the row count from `vectors.json` without reading the rows.
- The CLI prints exactly the bytes it printed before for every existing invocation; the new flags and command add bytes only where they are asked for.

- [ ] **Step 1: The failing tests for the capped renderers**

In `src/impact.rs`'s `mod tests`, after `render_json_is_valid_and_carries_the_same_counts`:

```rust
    #[test]
    fn a_capped_render_keeps_the_counts_and_the_risk_line_and_says_how_many_it_left_out() {
        let g = graph();
        let imp = upstream(&g, "sym:s.ts::S", 3);
        let full = render(&g, &imp, "upstream");
        let capped = render_capped(&g, &imp, "upstream", 1);
        assert!(capped.contains("d=1  will break (3)\n  file:m.ts  m.ts:1  Calls → sym:s.ts::S\n  … 2 more (limit 1)\n"), "{capped}");
        assert!(capped.ends_with("risk: MEDIUM — 3 direct, 4 total, 5 files\n"), "the risk line is computed on the whole walk: {capped}");
        assert!(capped.contains("importers (4): c.ts, index.ts, m.ts, w.ts\n"));
        assert_eq!(render_capped(&g, &imp, "upstream", usize::MAX), full, "an uncapped render is the render");
        assert_eq!(render_capped(&g, &imp, "upstream", 3), full, "a cap the layer fits under prints no tail");
    }

    #[test]
    fn a_trace_renders_as_the_command_prints_it_and_as_json_with_or_without_a_path() {
        let g = graph();
        let path = trace(&g, "sym:j.ts::J", "sym:s.ts::S", 6).unwrap();
        assert_eq!(render_trace(&g, &path), "sym:j.ts::J  j.ts:2\n  → sym:w.ts::W  w.ts:3\n  → sym:s.ts::S.create  s.ts:5\n");
        let v: serde_json::Value = serde_json::from_str(&render_trace_json(&g, "sym:j.ts::J", "sym:s.ts::S", 6, Some(&path))).unwrap();
        assert_eq!(v["path"].as_array().unwrap().len(), 3);
        assert_eq!(v["path"][1]["at"], "w.ts:3");
        let none: serde_json::Value = serde_json::from_str(&render_trace_json(&g, "sym:s.ts::S", "sym:j.ts::J", 6, None)).unwrap();
        assert!(none["path"].is_null());
        assert_eq!(none["depth"], 6);
    }
```

In `src/changes.rs`'s `mod tests`, after the last test:

```rust
    #[test]
    fn a_capped_changes_render_caps_both_lists_and_keeps_the_risk_line() {
        // Three touched symbols in one file, each with one caller, so both lists have three lines.
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::File, "file:s.ts", "s.ts", "", "s.ts", 1);
        for (i, name) in ["a", "b", "c"].iter().enumerate() {
            let line = (i as u32) * 10 + 1;
            e.node_span(NodeKind::Symbol, &format!("sym:s.ts::{name}"), name, "", "s.ts", (line, line + 5));
            e.edge("file:s.ts", &format!("sym:s.ts::{name}"), EdgeKind::Declares, "export", "s.ts");
            e.node(NodeKind::Symbol, &format!("sym:c{i}.ts::call_{name}"), &format!("call_{name}"), "", &format!("c{i}.ts"), 1);
            e.edge(&format!("sym:c{i}.ts::call_{name}"), &format!("sym:s.ts::{name}"), EdgeKind::Calls, "", &format!("c{i}.ts"));
        }
        g.apply(e);
        let hunks = vec![Hunk { file: "s.ts".into(), start: 1, end: 30 }];
        let r = report(&g, &hunks, 2);
        let full = render(&g, &r);
        let capped = render_capped(&g, &r, 1);
        assert!(capped.starts_with("changed: 3 symbols in 1 file\n  sym:s.ts::a  s.ts:1-6\n  … 2 more (limit 1)\naffected (depth 2): 3 symbols in 3 files\n  d=1  sym:c0.ts::call_a  c0.ts:1  ← sym:s.ts::a\n  … 2 more (limit 1)\n"), "{capped}");
        assert!(capped.ends_with("risk: MEDIUM — 3 direct, 3 total, 3 files\n"), "{capped}");
        assert_eq!(render_capped(&g, &r, usize::MAX), full);
    }
```

In `src/query.rs`'s `mod tests`, after `json_render_is_valid_json_with_seeds_and_expanded_keys`:

```rust
    #[test]
    fn explain_capped_stops_at_the_limit_and_counts_the_rest_and_json_carries_every_edge() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "cancel", "body", "a.md", 3);
        for i in 0..5 {
            let id = format!("FR-PAY-3{i}");
            e.node(NodeKind::Requirement, &id, "x", "", "a.md", 10 + i as u32);
            e.edge(&id, "FR-PAY-22", EdgeKind::References, "", "a.md");
        }
        g.apply(e);
        let full = explain(&g, "FR-PAY-22").unwrap();
        assert_eq!(full.lines().count(), 6, "{full}");
        let capped = explain_capped(&g, "FR-PAY-22", 2).unwrap();
        assert_eq!(capped.lines().count(), 4, "{capped}");
        assert!(capped.ends_with("  … 3 more edges (limit 2)\n"), "{capped}");
        assert_eq!(explain_capped(&g, "FR-PAY-22", usize::MAX).unwrap(), full);
        let v: serde_json::Value = serde_json::from_str(&explain_json(&g, "FR-PAY-22").unwrap()).unwrap();
        assert_eq!(v["id"], "FR-PAY-22");
        assert_eq!(v["kind"], "Requirement");
        assert_eq!(v["at"], "a.md:3");
        assert_eq!(v["edges"].as_array().unwrap().len(), 5);
        assert_eq!(v["edges"][0]["dir"], "in");
        assert!(explain_json(&g, "nothing").is_none());
    }
```

Run: `cargo test --release --manifest-path "$W/Cargo.toml" capped 2>&1 | tail -5` — expected: compile errors naming `render_capped`, `render_trace`, `explain_capped`.

- [ ] **Step 2: Implement the renderers**

`src/impact.rs` — replace `render` with:

```rust
/// The line a cap leaves in place of the rows it dropped. The count is what the agent acts on
/// when a hub symbol is the target: `impact DatabaseService` on the bench corpus is 156 lines
/// and 8,860 tokens, forty answers' worth, and the risk line already says what those lines
/// would.
fn more(left: usize, limit: usize) -> String { format!("  … {left} more (limit {limit})\n") }

pub fn render(graph: &Graph, imp: &Impact, direction: &str) -> String { render_capped(graph, imp, direction, usize::MAX) }

/// `render` with at most `limit` dependents printed per layer; the layer counts, the importers
/// and the risk line are computed on the whole walk whatever the cap.
pub fn render_capped(graph: &Graph, imp: &Impact, direction: &str, limit: usize) -> String {
    let up = direction == "upstream";
    let mut out = format!("{}  {}\n", imp.root, line_of(graph, &imp.root));
    for (i, layer) in imp.layers.iter().enumerate() {
        let name = if up { LAYER_NAMES.get(i).copied().unwrap_or("transitive") } else { "reaches" };
        out.push_str(&format!("d={}  {name} ({})\n", i + 1, layer.len()));
        for d in layer.iter().take(limit) {
            let arrow = if up { "→" } else { "←" };
            out.push_str(&format!("  {}  {}  {:?} {arrow} {}\n", d.id, line_of(graph, &d.id), d.kind, d.via));
        }
        if layer.len() > limit { out.push_str(&more(layer.len() - limit, limit)); }
    }
    if up {
        if !imp.importers.is_empty() { out.push_str(&format!("importers ({}): {}\n", imp.importers.len(), imp.importers.join(", "))); }
        let direct = imp.layers.first().map_or(0, Vec::len);
        let total: usize = imp.layers.iter().map(Vec::len).sum();
        let files = files(graph, imp).len();
        out.push_str(&format!("risk: {} — {direct} direct, {total} total, {files} files\n", risk(direct, total, files)));
    }
    out
}

/// The chain as `trace` prints it: the first node bare, every next one behind an arrow.
pub fn render_trace(graph: &Graph, path: &[String]) -> String {
    let mut out = String::new();
    for (i, id) in path.iter().enumerate() {
        let at = graph.nodes.get(id).map(|n| format!("{}:{}", n.file, n.line)).unwrap_or_default();
        out.push_str(&format!("{}{id}  {at}\n", if i == 0 { "" } else { "  → " }));
    }
    out
}

/// `path` is null when there is none within `depth`: a script reads one shape either way.
pub fn render_trace_json(graph: &Graph, from: &str, to: &str, depth: usize, path: Option<&[String]>) -> String {
    let hops = path.map(|p| p.iter().map(|id| serde_json::json!({ "id": id, "at": line_of(graph, id) })).collect::<Vec<_>>());
    serde_json::json!({ "from": from, "to": to, "depth": depth, "path": hops }).to_string() + "\n"
}
```

`src/changes.rs` — replace `render` with:

```rust
pub fn render(graph: &Graph, r: &Report) -> String { render_capped(graph, r, usize::MAX) }

/// `render` with each list cut at `limit` lines and a count for the rest: `changes --base
/// HEAD~30` on the bench corpus is 3,190 lines and 94,824 tokens, and the risk line, computed
/// on the whole report, is what those lines were for.
pub fn render_capped(graph: &Graph, r: &Report, limit: usize) -> String {
    if r.touched.is_empty() { return "changed: 0 symbols\n".into() }
    let more = |left: usize| format!("  … {left} more (limit {limit})\n");
    let symbols = r.touched.iter().filter(|id| !id.starts_with("file:")).count();
    let changed_files: BTreeSet<String> = r.touched.iter().map(|id| file_of(graph, id)).collect();
    let mut out = format!("changed: {} in {}\n", plural(symbols, "symbol"), plural(changed_files.len(), "file"));
    for id in r.touched.iter().take(limit) { out.push_str(&format!("  {id}  {}\n", span_of(graph, id))); }
    if r.touched.len() > limit { out.push_str(&more(r.touched.len() - limit)); }
    out.push_str(&format!("affected (depth {}): {} in {}\n", r.depth, plural(r.affected.len(), "symbol"), plural(r.files.len(), "file")));
    for d in r.affected.iter().take(limit) {
        let at = graph.nodes.get(&d.id).map(|n| format!("{}:{}", n.file, n.line)).unwrap_or_else(|| d.id.trim_start_matches("file:").to_string());
        out.push_str(&format!("  d={}  {}  {at}  ← {}\n", d.depth, d.id, d.via));
    }
    if r.affected.len() > limit { out.push_str(&more(r.affected.len() - limit)); }
    let direct = r.affected.iter().filter(|d| d.depth == 1).count();
    out.push_str(&format!("risk: {} — {direct} direct, {} total, {}\n", r.risk, r.affected.len(), plural(r.files.len(), "file")));
    out
}
```

`src/query.rs` — replace `explain` with:

```rust
pub fn explain(graph: &Graph, needle: &str) -> Option<String> { explain_capped(graph, needle, usize::MAX) }

/// One node and at most `limit` of its edges; a `File` node is a hub the answer path never
/// expands to, and an agent's `explain` on one should not cost what the CLI's may.
pub fn explain_capped(graph: &Graph, needle: &str, limit: usize) -> Option<String> {
    let n = resolve(graph, needle)?;
    let mut out = format!("{}  {}:{}  {:?}  {}\n", n.id, n.file, n.line, n.kind, headline(&n.label));
    if let Some(c) = &n.community { out.push_str(&format!("  community: {c}\n")); }
    let edges = sorted_edges(graph, &n.id);
    for e in edges.iter().take(limit) {
        let (arrow, other) = if e.source == n.id { ("→", &e.target) } else { ("←", &e.source) };
        let ctx = if e.context.is_empty() { String::new() } else { format!("  [{}]", e.context) };
        out.push_str(&format!("  {:?} {arrow} {other}{ctx}\n", e.kind));
    }
    if edges.len() > limit { out.push_str(&format!("  … {} more edges (limit {limit})\n", edges.len() - limit)); }
    Some(out)
}

pub fn explain_json(graph: &Graph, needle: &str) -> Option<String> {
    let n = resolve(graph, needle)?;
    let edges: Vec<serde_json::Value> = sorted_edges(graph, &n.id).iter().map(|e| {
        let (dir, other) = if e.source == n.id { ("out", &e.target) } else { ("in", &e.source) };
        serde_json::json!({ "kind": format!("{:?}", e.kind), "dir": dir, "other": other, "context": e.context })
    }).collect();
    Some(serde_json::json!({
        "id": n.id, "kind": format!("{:?}", n.kind), "at": format!("{}:{}", n.file, n.line), "label": n.label,
        "community": n.community, "edges": edges,
    }).to_string() + "\n")
}

/// The edge order `explain` has always printed: legacy last, then by kind, source, target.
fn sorted_edges<'a>(graph: &'a Graph, id: &str) -> Vec<&'a crate::model::Edge> {
    let mut edges = graph.neighbours(id);
    edges.sort_by_key(|e| (e.kind == EdgeKind::Legacy, e.kind, e.source.clone(), e.target.clone()));
    edges
}
```

Run: `cargo test --release --manifest-path "$W/Cargo.toml" 2>&1 | grep 'test result'` — expected `447 passed` in the unit binary (444 + 3; 2 ignored as before), `12 passed` in `tests/serve.rs`. `render_lists_layers_with_path_line_and_ends_with_the_risk` and `render_json_is_valid_and_carries_the_same_counts` still pass: `render` is the uncapped call.

- [ ] **Step 3: The failing tests for `status`**

Create `src/status.rs` with only the tests module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn built_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("docs/a.md"), "# A\n\n**FR-PAY-22 · MUST · cancellation window**\n\nbody\n").unwrap();
        let cfg = crate::config::Config::default();
        crate::run_update(dir.path(), &cfg, &crate::extractors(dir.path(), &cfg).unwrap(), true).unwrap();
        dir
    }

    #[test]
    fn a_repository_without_a_store_reads_as_no_store_and_walks_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let s = read(dir.path(), &crate::config::Config::default()).unwrap();
        assert!(!s.store);
        assert_eq!((s.nodes, s.edges, s.changed), (0, 0, 0));
        let line = render(&s);
        assert!(line.starts_with("no store at "), "{line}");
        assert!(line.contains("repograph build"), "{line}");
        assert!(!dir.path().join(".repograph").exists(), "reading the status must not create the store directory");
    }

    #[test]
    fn a_built_store_reads_its_counts_and_says_fresh_until_the_tree_moves() {
        let dir = built_repo();
        let cfg = crate::config::Config::default();
        let s = read(dir.path(), &cfg).unwrap();
        assert!(s.store);
        assert_eq!(s.nodes, 2, "the requirement and its file");
        assert_eq!((s.changed, s.removed), (0, 0));
        assert_eq!(s.questions, (0, 1));
        assert_eq!(s.model, None);
        assert_eq!(s.rows, 0);
        assert!(!s.serve);
        let line = render(&s);
        assert!(line.starts_with("store: 2 nodes, 1 edges  fresh  questions 0/1  code questions 0/0  no vectors  serve: no"), "{line}");
        std::fs::write(dir.path().join("docs/b.md"), "# B\n\n**FR-PAY-23 · MUST · refund**\n\nbody\n").unwrap();
        let s = read(dir.path(), &cfg).unwrap();
        assert_eq!((s.changed, s.removed), (1, 0));
        assert!(render(&s).contains("behind: 1 changed, 0 removed"), "{}", render(&s));
        // Reading the status never refreshes: the store still has one document.
        assert_eq!(crate::store::Store::new(dir.path()).load().unwrap().0.nodes.len(), 2);
    }

    #[test]
    fn the_json_carries_every_field_the_line_prints_and_names_the_model_when_rows_exist() {
        let dir = built_repo();
        let model = crate::index::embed::DEFAULT_MODEL;
        std::fs::write(dir.path().join(".repograph/vectors.json"), format!(r#"{{"ids":["FR-PAY-22","x"],"hashes":["h","h"],"kinds":[false,true],"dim":4,"model":"{model}"}}"#)).unwrap();
        std::fs::write(dir.path().join(".repograph/vectors.f32"), [0u8; 32]).unwrap();
        let s = read(dir.path(), &crate::config::Config::default()).unwrap();
        assert_eq!(s.model.as_deref(), Some(model));
        assert_eq!(s.rows, 2);
        let v: serde_json::Value = serde_json::from_str(&render_json(&s)).unwrap();
        assert_eq!(v["store"], true);
        assert_eq!(v["nodes"], 2);
        assert_eq!(v["questions"], serde_json::json!([0, 1]));
        assert_eq!(v["model"], model);
        assert_eq!(v["rows"], 2);
        assert!(render(&s).contains(&format!("vectors {model} (2 rows)")), "{}", render(&s));
    }
}
```

Add `mod status;` to `src/main.rs`. Run: `cargo test --release --manifest-path "$W/Cargo.toml" status:: 2>&1 | tail -3` — expected: compile errors, `read` and `render` do not exist.

- [ ] **Step 4: Implement `status`**

Above the tests in `src/status.rs`:

```rust
//! `repograph status`: what a store is, in one line, without opening a model. It is the line
//! the MCP server hands a harness at start and the first result of a session carries, so a
//! reader that never sees stderr still knows whether the answers it gets are fused or lexical,
//! whether `enrich` ever ran, and whether the store is behind the tree.
use crate::{config, enrich, index, serve, store, walk};
use anyhow::Result;
use std::path::Path;

#[derive(Debug, PartialEq, serde::Serialize)]
pub struct Status {
    pub root: String,
    pub store: bool,
    pub nodes: usize,
    pub edges: usize,
    pub changed: usize,
    pub removed: usize,
    pub questions: (usize, usize),
    pub code_questions: (usize, usize),
    pub model: Option<String>,
    pub rows: usize,
    pub serve: bool,
}

/// The store as it stands and how far the tree has moved from it — the same ~10 ms walk `ask`
/// pays, and none of the refresh. A repository without a store is not walked at all: an empty
/// manifest makes every file a change, and hashing a tree to say "no store" is the wrong price.
pub fn read(repo: &Path, cfg: &config::Config) -> Result<Status> {
    let store = store::Store::new(repo);
    let root = repo.display().to_string();
    if !store.has("graph.json") {
        return Ok(Status { root, store: false, nodes: 0, edges: 0, changed: 0, removed: 0, questions: (0, 0), code_questions: (0, 0), model: None, rows: 0, serve: false });
    }
    let (graph, manifest) = store.load()?;
    let diff = manifest.diff(&walk::walk(repo, cfg, &manifest)?);
    let questions = enrich::Questions::load(&store)?;
    let model = index::dense::DenseIndex::recorded_model(&store)?;
    let rows = index::dense::DenseIndex::recorded_rows(&store)?;
    let serve = {
        let path = serve::socket_path(repo);
        path.exists() && std::os::unix::net::UnixStream::connect(&path).is_ok()
    };
    Ok(Status {
        root, store: true, nodes: graph.nodes.len(), edges: graph.edges.len(),
        changed: diff.changed.len(), removed: diff.removed.len(),
        questions: enrich::coverage(&graph, &questions), code_questions: enrich::code_coverage(&graph, &questions),
        model, rows, serve,
    })
}

pub fn render(s: &Status) -> String {
    if !s.store {
        return format!("no store at {}/.repograph — run `repograph build` (about 1 s per 800 files with --no-dense; the vectors take minutes on the small model and longer on the default)\n", s.root);
    }
    let fresh = if s.changed + s.removed == 0 { "fresh".to_string() } else { format!("behind: {} changed, {} removed", s.changed, s.removed) };
    let vectors = match &s.model { Some(m) => format!("vectors {m} ({} rows)", s.rows), None => "no vectors".to_string() };
    format!("store: {} nodes, {} edges  {fresh}  questions {}/{}  code questions {}/{}  {vectors}  serve: {}\n",
        s.nodes, s.edges, s.questions.0, s.questions.1, s.code_questions.0, s.code_questions.1, if s.serve { "yes" } else { "no" })
}

pub fn render_json(s: &Status) -> String { serde_json::to_string(s).unwrap() + "\n" }
```

`recorded_model` returns `None` for a store whose `vectors.f32` is absent and the unnamed model's name for rows without one, which is what the line should say. In `src/index/dense.rs`, after `recorded_model`:

```rust
    /// How many rows `vectors.json` names, without reading them: `status` prints it beside the
    /// model, and 50 MB of floats is the wrong price for a count.
    pub fn recorded_rows(store: &Store) -> Result<usize> {
        #[derive(serde::Deserialize)]
        struct Written { #[serde(default)] ids: Vec<String> }
        let Some(meta) = store.read_bytes("vectors.json")? else { return Ok(0) };
        let w: Written = serde_json::from_slice(&meta).context("vectors.json")?;
        Ok(w.ids.len())
    }
```

`serve::socket_path` is already `pub`. Run: `cargo test --release --manifest-path "$W/Cargo.toml" status:: 2>&1 | grep 'test result'` — expected `3 passed`. If the fresh-store line reads `2 nodes, 1 edges` differently (the `Declares` edge count), copy the printed line into the assertion — the counts are the extractor's and the test pins the shape.

- [ ] **Step 5: Wire the commands**

In `src/main.rs`, `enum Cmd`: add `#[arg(long)] json: bool,` to `Explain` and `Trace`, and after `Verify`:

```rust
    /// The store in one line — counts, freshness against the tree, question coverage, the
    /// vectors' model, whether a `serve` answers — for a hook, a harness or a person deciding
    /// whether to build. Reads only; a store that is behind stays behind.
    Status { #[arg(long)] json: bool },
```

Replace the `Explain`, `Trace` arms and add `Status`:

```rust
        Cmd::Explain { node, json } => {
            let (graph, _) = store::Store::new(&repo).load()?;
            let text = if json { query::explain_json(&graph, &node) } else { query::explain(&graph, &node) };
            match text {
                Some(s) => { print!("{s}"); Ok(()) }
                None => anyhow::bail!("no node matches {node}"),
            }
        }
        // …
        Cmd::Trace { from, to, depth, stale, json } => {
            let graph = graph_for(&repo, &load_cfg()?, stale)?;
            let Some(a) = query::resolve(&graph, &from) else { anyhow::bail!("no node matches {from}") };
            let Some(b) = query::resolve(&graph, &to) else { anyhow::bail!("no node matches {to}") };
            let path = impact::trace(&graph, &a.id, &b.id, depth);
            if json {
                print!("{}", impact::render_trace_json(&graph, &a.id, &b.id, depth, path.as_deref()));
                return Ok(());
            }
            match path {
                Some(p) => { print!("{}", impact::render_trace(&graph, &p)); Ok(()) }
                None => anyhow::bail!("no call path from {} to {} within {depth} hops", a.id, b.id),
            }
        }
        // …
        Cmd::Status { json } => {
            let s = status::read(&repo, &load_cfg()?)?;
            print!("{}", if json { status::render_json(&s) } else { status::render(&s) });
            if !json && !s.store { std::process::exit(1) }
            Ok(())
        }
```

`trace --json` exits 0 with `"path": null` when there is none — a script reads one shape and one code; the text form keeps its non-zero exit for a person's shell.

Run: `cargo test --release --manifest-path "$W/Cargo.toml" 2>&1 | grep 'test result'` and `cargo clippy --release --all-targets --manifest-path "$W/Cargo.toml" -- -D warnings 2>&1 | tail -1`. Expected `450 passed` + `12 passed`, clippy `Finished`.

- [ ] **Step 6: The contract by hand, and Rule 1**

```bash
cargo build --release --manifest-path "$W/Cargo.toml" 2>&1 | tail -1
"$B" --repo "$F" status | tee "$A/t1-status.txt"
"$B" --repo "$F" status --json | python3 -c 'import json,sys; d=json.load(sys.stdin); print(sorted(d))' | tee -a "$A/t1-status.txt"
"$B" --repo "$F" explain --json StaffService | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["at"], len(d["edges"]))' | tee "$A/t1-explain.txt"
"$B" --repo "$F" trace --stale --json StaffController StaffService | tee "$A/t1-trace.txt"
"$B" --repo "$F" trace --stale --json SystemController DatabaseService | tee -a "$A/t1-trace.txt"
"$B" --repo "$F" impact --stale StaffService | tee "$A/t1-impact.txt" | tail -1
```

Expected: the status line reads `store: 8316 nodes, 32601 edges  fresh  questions 1996/1996  code questions 0/3475  vectors intfloat/multilingual-e5-small (34802 rows)  serve: no` (the counts are Task 0's `verify` and the gaps plan's `bc-d1` row; a different count is recorded, not corrected); the JSON keys are the eleven fields; `explain --json StaffService` prints `apps/api/src/modules/staff/staff.service.ts:19` and an edge count; the first trace prints a `path` of three hops, the second `"path":null`. Then Rule 1:

```bash
for arm in dense lexical; do
  nd=""; [ $arm = lexical ] && nd="--no-dense"
  "$B" --repo "$F" $nd dump --queries "$W/bench/cases.jsonl"     --out "$A/t1-rec-$arm.json" >/dev/null 2>&1
  "$B" --repo "$F" $nd dump --queries "$W/bench/dev-cases.jsonl" --out "$A/t1-dev-$arm.json" >/dev/null 2>&1
  "$B" --repo "$F" $nd bench                                    2>&1 | tail -1 > "$A/t1-bench-rec-$arm.txt"
  "$B" --repo "$F" $nd bench --cases "$W/bench/dev-cases.jsonl" 2>&1 | tail -1 > "$A/t1-bench-dev-$arm.txt"
  for s in rec dev; do cmp "$A/base-$s-$arm.json" "$A/t1-$s-$arm.json" && echo "dump $s $arm identical"; cmp "$A/base-bench-$s-$arm.txt" "$A/t1-bench-$s-$arm.txt" && echo "bench $s $arm identical"; done
done 2>&1 | tee "$A/t1-rule1.txt"
```

Expected: eight `identical`. Any difference is **BLOCKED** — a renderer that moves an answer is a bug.

- [ ] **Step 7: Commit**

`git -C "$W" add src/status.rs src/main.rs src/query.rs src/impact.rs src/changes.rs src/index/dense.rs` and one commit, subject `feat(cli): status, --json on every reader, and capped renderers for hub symbols`, body naming the yardstick numbers (`$A/yardstick.txt`), the eight Rule 1 verdicts (`$A/t1-rule1.txt`), and the test count.

---

### Task 2: The refresh shared with `serve`, and the context's three small doors

**Files:**
- Modify: `src/serve.rs` (`fresh` extracted from `answer`; `adopt_if_moved` and `WAKE` become `pub(crate)`), `src/ask.rs` (`Context::graph`, `Context::model_open`, `Context::release_model`; tests), `src/main.rs` (`Watcher`'s `graph` field stays private; nothing else)

**Interfaces:**
- Produces: `serve::fresh(watcher: &mut Watcher, ctx: &mut Context, stale: bool) -> Result<()>` — the per-request refresh `answer` does today, in one place; `serve::WAKE` and `serve::adopt_if_moved` visible to the crate; `Context::graph(&self) -> &Graph`; `Context::model_open(&self) -> bool`; `Context::release_model(&mut self)` — drops the embedder, the warm handle and the vectors; the next fused answer reopens all three lazily through the paths that already exist.
- `serve`'s twelve integration tests are the guard: nothing about the socket's behaviour moves.

- [ ] **Step 1: The failing tests**

In `src/ask.rs`'s `mod tests`, after `lexical_indexes_built_by_an_answer_are_dropped_once_the_context_adopts_a_watcher`:

```rust
    #[test]
    fn a_context_lends_its_graph_and_releases_a_model_it_never_opened_without_complaint() {
        let dir = repo_with_two_docs();
        let cfg = crate::config::Config::load(dir.path()).unwrap();
        let ex = crate::extractors(dir.path(), &cfg).unwrap();
        crate::run_update(dir.path(), &cfg, &ex, true).unwrap();
        let mut ctx = Context::open(dir.path(), &cfg, true, true).unwrap();
        assert!(ctx.graph().nodes.contains_key("FR-PAY-1"));
        assert!(!ctx.model_open());
        ctx.release_model();
        assert!(!ctx.model_open());
        assert!(ctx.answer(&fused(true)).unwrap().contains("FR-PAY-1"), "a release before any answer changes nothing");
    }

    #[test]
    fn releasing_the_model_drops_the_vectors_too_and_the_next_fused_answer_reloads_them() {
        let dir = repo_with_two_docs();
        let cfg = crate::config::Config::load(dir.path()).unwrap();
        let ex = crate::extractors(dir.path(), &cfg).unwrap();
        crate::run_update(dir.path(), &cfg, &ex, true).unwrap();
        vectors_beside_the_graph(dir.path());
        an_unopenable_model();
        let mut ctx = Context::open(dir.path(), &cfg, true, false).unwrap();
        ctx.answer(&fused(true)).unwrap();
        assert!(ctx.dense_idx.borrow().is_some(), "the fused answer loaded the vectors");
        // The model could not be opened, so `embedder` holds `Some(None)`: opened, and empty.
        assert!(ctx.embedder.borrow().is_some());
        ctx.release_model();
        assert!(ctx.dense_idx.borrow().is_none());
        assert!(ctx.embedder.borrow().is_none());
        assert!(!ctx.model_open());
        ctx.answer(&fused(true)).unwrap();
        assert!(ctx.dense_idx.borrow().is_some(), "reloaded lazily by the next fused answer");
    }
```

Run: `cargo test --release --manifest-path "$W/Cargo.toml" release 2>&1 | tail -3` — expected: compile errors naming `graph`, `model_open`, `release_model`.

- [ ] **Step 2: Implement**

In `src/ask.rs`, `impl Context`, after `no_dense`:

```rust
    /// The graph this context answers from, for the readers that walk it rather than search
    /// it — `impact`, `trace`, `changes`, `explain` in a resident process.
    pub fn graph(&self) -> &model::Graph { &self.graph }

    /// Whether an embedding session is held — the 1.9 GB a resident process gives back when
    /// no fused question has come for a while.
    pub fn model_open(&self) -> bool {
        matches!(&*self.embedder.borrow(), Some(Some(_))) || self.warm.borrow().is_some()
    }

    /// The model, the thread that may still be opening it and the vectors, dropped. Each
    /// reopens on the next fused answer through the same lazy path that opened it the first
    /// time, so nothing here decides anything an answer would not decide again; the `resync`
    /// flag is untouched, since rows left behind are still behind.
    pub fn release_model(&mut self) {
        if let Some(handle) = self.warm.borrow_mut().take() { let _ = handle.join(); }
        *self.embedder.borrow_mut() = None;
        *self.dense_idx.borrow_mut() = None;
    }
```

In `src/serve.rs`: make `const WAKE` `pub(crate) const WAKE`, make `fn adopt_if_moved` `pub(crate) fn adopt_if_moved`, and add above `answer`:

```rust
/// The refresh a request gets before its answer, whichever transport it came by. `--stale`
/// skips the walk and not the store: a one-shot answers from whatever is on disk at this
/// instant, so a resident one reads the store back when another process has written it.
/// Otherwise the same refresh a one-shot `ask` does, batch 1, applying any change now.
pub(crate) fn fresh(watcher: &mut crate::Watcher, ctx: &mut ask::Context, stale: bool) -> Result<()> {
    if stale {
        if watcher.reload_if_moved()? { ctx.adopt(watcher, None)?; }
    } else {
        adopt_if_moved(watcher, ctx, 1)?;
    }
    Ok(())
}
```

and in `answer`, replace the `if hello.req.stale { … } else { … }` block (with its two comments) by `fresh(watcher, ctx, hello.req.stale)?;`, keeping the `log(ctx);` line before it. `Watcher::reload_if_moved` and `Watcher::poll` are already `pub(crate)` through the struct's crate visibility; if the compiler says otherwise, mark the two methods `pub(crate)`.

Run: `cargo test --release --manifest-path "$W/Cargo.toml" 2>&1 | grep 'test result'` — expected `452 passed` + `12 passed`; then `cargo clippy --release --all-targets --manifest-path "$W/Cargo.toml" -- -D warnings 2>&1 | tail -1`.

- [ ] **Step 3: Rule 1 and Rule 2's serve half**

Rule 1 as in Task 1 Step 6 with the `t2-` prefix, eight `identical` expected. Then the serve check on the copy — the same bytes through the socket as in-process, lexical arm, the 142 questions:

```bash
C=/Users/max/bench/agent-2026-09-06/bc
python3 - "$W/bench/cases.jsonl" "$W/bench/dev-cases.jsonl" > "$A/questions.txt" <<'PY'
import json, sys
for p in sys.argv[1:]:
    for line in open(p):
        print(json.loads(line)["q"])
PY
wc -l "$A/questions.txt"
("$B" --repo "$C" --no-dense serve --every 3600 --idle 600 > "$A/t2-serve.log" 2>&1 &) ; sleep 3; grep -q "serve: " "$A/t2-serve.log" && echo "serve up"
n=0; same=0; while IFS= read -r q; do
  a=$("$B" --repo "$C" --no-dense ask --stale $q 2>/dev/null); b=$("$B" --repo "$C" --no-dense ask --stale --no-serve $q 2>/dev/null)
  n=$((n+1)); [ "$a" = "$b" ] && same=$((same+1))
done < "$A/questions.txt"; echo "socket == in-process on $same/$n" | tee "$A/t2-rule2-serve.txt"
pkill -f "$C --no-dense serve" ; sleep 1; ls "$C/.repograph/serve.sock" 2>/dev/null || echo "socket removed"
```

Expected: `142` questions, `socket == in-process on 142/142`, the socket removed. (`pkill -f 'repograph serve'` matches nothing — the command line reads `repograph --repo <path> --no-dense serve`, runbook.)

- [ ] **Step 4: Commit**

`git -C "$W" add src/serve.rs src/ask.rs src/main.rs` — subject `refactor(serve): the per-request refresh in one function, and a context that lends its graph and releases its model`, body with the test count and the two rule files.

---

### Task 3: `repograph mcp` — the five tools over stdio

**Files:**
- Create: `src/mcp.rs`, `tests/mcp.rs`, `bench/agent/mcp_client.py`
- Modify: `src/main.rs` (`mod mcp;`, `Cmd::Mcp`, its arm), `src/serve.rs` (nothing — `try_ask`, `fresh`, `adopt_if_moved`, `WAKE`, `VERSION` are reused as they are)
- Create (scratch): `$A/t3-*.txt`

**Interfaces:**
- Consumes: `serve::fresh`, `serve::adopt_if_moved`, `serve::try_ask`, `serve::WAKE`, `Watcher::open`, `ask::Context`, `status::read/render`, the capped renderers of Task 1.
- Produces:
  - `mcp::run(repo: &Path, cfg: &Config, every: u64, idle_model: u64, no_dense: bool) -> Result<()>` — reads newline-delimited JSON-RPC 2.0 on stdin, writes replies on stdout, logs on stderr, returns when stdin closes.
  - `mcp::find_root(start: &Path) -> PathBuf`; `mcp::tools() -> Vec<serde_json::Value>`; `mcp::instructions(status_line: &str) -> String`; the constants `PROTOCOL`, `TOOLS_BYTES = 3_200`, `INSTRUCTIONS_BYTES = 800`, `LINES = 25`.
  - The wire: `initialize` → `{protocolVersion, capabilities: {tools: {}}, serverInfo: {name: "repograph", version}, instructions}`; `notifications/initialized` → nothing; `ping` → `{}`; `tools/list` → `{tools: [...]}`; `tools/call {name, arguments}` → `{content: [{type: "text", text}], isError}`; anything else → `{error: {code: -32601}}`; a line that is not JSON → `{error: {code: -32700}}` with `id: null`; a message without an `id` is a notification and gets no reply.
  - Tool result text = the notices `ask` would print on stderr, one per line, then the CLI twin's bytes (capped where `limit` applies); the first result of a session is prefixed by the `status` line; `no match …` when `ask` prints nothing.
- `tests/serve.rs` stays green untouched.

- [ ] **Step 1: The failing unit tests**

Create `src/mcp.rs` with the module doc and the tests only:

```rust
//! `repograph mcp`: the store answered over the Model Context Protocol on stdin and stdout, for
//! a coding agent's harness. The five tools are the five readers — `ask`, `explain`, `impact`,
//! `trace`, `changes` — and each answers the bytes its CLI twin prints, capped where a hub
//! symbol would otherwise cost an agent thousands of tokens, the notices `ask` prints on stderr
//! placed ahead of the answer because the model reads only the result. The process belongs to
//! the harness: it starts with the session, exits when stdin closes, refreshes before every
//! answer the way `ask` does, polls between answers the way `serve` does, uses a `serve` when
//! one answers, and never builds a store.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_surface_fits_under_its_ceilings_and_names_the_five_readers() {
        let tools = serde_json::to_string(&tools()).unwrap();
        assert!(tools.len() <= TOOLS_BYTES, "tools/list is {} bytes against a ceiling of {TOOLS_BYTES}", tools.len());
        let names: Vec<String> = tools().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
        assert_eq!(names, ["ask", "explain", "impact", "trace", "changes"]);
        for t in tools() {
            assert_eq!(t["inputSchema"]["type"], "object", "{}", t["name"]);
            assert!(t["description"].as_str().unwrap().len() <= 320, "{} describes itself in more than 320 bytes", t["name"]);
        }
        let line = "store: 8316 nodes, 32601 edges  fresh  questions 1996/1996  code questions 0/3475  vectors intfloat/multilingual-e5-large (33525 rows)  serve: no\n";
        let text = instructions(line);
        assert!(text.len() <= INSTRUCTIONS_BYTES, "instructions are {} bytes against a ceiling of {INSTRUCTIONS_BYTES}", text.len());
        assert!(text.contains("impact"), "{text}");
        assert!(text.ends_with(line.trim_end()), "the store line closes the instructions: {text}");
    }

    #[test]
    fn find_root_prefers_a_store_then_a_git_entry_then_the_directory_itself() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(repo.join("apps/api/src")).unwrap();
        assert_eq!(find_root(&repo.join("apps/api/src")), repo.join("apps/api/src"), "nothing above says where the root is");
        // A linked worktree's `.git` is a file; the walk must not ask for a directory.
        std::fs::write(repo.join(".git"), "gitdir: /elsewhere\n").unwrap();
        assert_eq!(find_root(&repo.join("apps/api/src")), repo);
        std::fs::create_dir_all(repo.join("apps/api/.repograph")).unwrap();
        assert_eq!(find_root(&repo.join("apps/api/src")), repo.join("apps/api"), "a store below the git root is the nearer answer");
    }

    #[test]
    fn a_protocol_revision_the_server_knows_is_echoed_and_an_unknown_one_gets_ours() {
        assert_eq!(negotiate(Some("2024-11-05")), "2024-11-05");
        assert_eq!(negotiate(Some("2025-06-18")), "2025-06-18");
        assert_eq!(negotiate(Some("2099-01-01")), PROTOCOL);
        assert_eq!(negotiate(None), PROTOCOL);
    }

    #[test]
    fn integer_arguments_are_clamped_to_their_range_and_a_missing_required_string_is_an_error() {
        let args = serde_json::json!({ "depth": 40, "seeds": 0, "query": "штраф  за отмену" });
        assert_eq!(int_arg(&args, "depth", 3, 1, 6), 6);
        assert_eq!(int_arg(&args, "seeds", 5, 1, 8), 1);
        assert_eq!(int_arg(&args, "limit", LINES, 0, 1000), LINES);
        assert_eq!(str_arg(&args, "query").unwrap(), "штраф  за отмену");
        assert!(str_arg(&args, "symbol").unwrap_err().to_string().contains("symbol"));
        assert!(!bool_arg(&args, "stale"));
    }

    #[test]
    fn a_reply_is_one_json_line_carrying_the_request_id_and_a_notification_gets_none() {
        assert!(reply_line(&serde_json::json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }), |_, _| unreachable!()).is_none());
        let line = reply_line(&serde_json::json!({ "jsonrpc": "2.0", "id": 7, "method": "ping" }), |_, _| Ok(serde_json::json!({}))).unwrap();
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["id"], 7);
        assert_eq!(v["result"], serde_json::json!({}));
        assert!(!line.contains('\n'));
        let err = reply_line(&serde_json::json!({ "jsonrpc": "2.0", "id": "a", "method": "nope" }), |_, _| Err((-32601, "method not found: nope".into()))).unwrap();
        let v: serde_json::Value = serde_json::from_str(&err).unwrap();
        assert_eq!(v["error"]["code"], -32601);
        assert_eq!(v["id"], "a");
    }
}
```

Add `mod mcp;` to `src/main.rs`. Run: `cargo test --release --manifest-path "$W/Cargo.toml" mcp:: 2>&1 | tail -3` — expected: compile errors.

- [ ] **Step 2: The protocol, the surface, the arguments**

Above the tests in `src/mcp.rs`:

```rust
use crate::{ask, changes, config, impact, query, rerank, serve, status, store};
use anyhow::{Context as _, Result};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The revision this server speaks. A client naming an older one it knows is answered in that
/// one: nothing here uses what the later revisions added, and a client refuses a version it
/// did not ask for.
pub const PROTOCOL: &str = "2025-06-18";
const KNOWN: [&str; 3] = ["2024-11-05", "2025-03-26", "2025-06-18"];

/// What a session pays before it asks anything, in bytes, bytes / 4 being the token proxy
/// `bench` uses: the five schemas and the instructions together under a thousand tokens —
/// five answers' worth. The five GitNexus schemas one session loaded ran to about 16 KB for
/// five tools of seventeen. Tests hold both ceilings.
pub const TOOLS_BYTES: usize = 3_200;
pub const INSTRUCTIONS_BYTES: usize = 800;

/// Lines a hub symbol may fill per list before the rest is a count. `impact DatabaseService`
/// on the bench corpus is 156 lines and 8,860 tokens, `changes --base HEAD~30` 3,190 lines
/// and 94,824; the risk line is computed on the whole walk whatever the cap, so what an agent
/// acts on is never the part that was cut.
pub const LINES: usize = 25;

fn negotiate(asked: Option<&str>) -> &'static str {
    asked.and_then(|a| KNOWN.iter().find(|k| **k == a)).copied().unwrap_or(PROTOCOL)
}

/// The repository the harness started this process in: the nearest ancestor holding a store,
/// else the nearest holding a `.git` entry — a linked worktree's is a file and is the root all
/// the same — else the directory itself. A harness opened in `apps/api` still asks the
/// repository's store, and two worktrees of one repository each get their own.
pub fn find_root(start: &Path) -> PathBuf {
    let ancestors: Vec<&Path> = start.ancestors().collect();
    if let Some(p) = ancestors.iter().find(|p| p.join(".repograph").is_dir()) { return p.to_path_buf(); }
    if let Some(p) = ancestors.iter().find(|p| p.join(".git").exists()) { return p.to_path_buf(); }
    start.to_path_buf()
}

pub fn tools() -> Vec<Value> {
    let int = |desc: &str, min: usize, max: usize| json!({ "type": "integer", "minimum": min, "maximum": max, "description": desc });
    let flag = |desc: &str| json!({ "type": "boolean", "description": desc });
    let text = |desc: &str| json!({ "type": "string", "description": desc });
    let tool = |name: &str, desc: &str, props: Value, required: &[&str]| json!({
        "name": name, "description": desc,
        "inputSchema": { "type": "object", "properties": props, "required": required },
    });
    vec![
        tool("ask",
            "Ask the repository graph in words: a requirement id, a symbol name, keywords or a paraphrase, in any language the documents use. Up to `seeds` lines `ID  path:line  headline` plus one neighbour, about 200 tokens. An exact id or symbol name answers outright.",
            json!({ "query": text("The question, as words"), "seeds": int("Answers to return; default 5", 1, 8),
                    "bodies": flag("Print each answer's full text"), "stale": flag("Skip the refresh against the tree") }),
            &["query"]),
        tool("explain",
            "One node — an id, a symbol name or a label — with its kind, path:line and its edges, `limit` at most.",
            json!({ "node": text("Id, symbol name or label"), "limit": int("Edges to print; default 40", 0, 1000) }),
            &["node"]),
        tool("impact",
            "Who reaches a symbol: callers by depth (d=1 will break), importing files, and a risk line with its counts. `down` walks what it calls instead. A call the graph cannot prove — dynamic dispatch, a callback — is not listed; confirm a 'nothing uses this' with rg.",
            json!({ "symbol": text("Symbol name, or sym:<file>::<Name>"), "depth": int("Hops; default 3", 1, 6),
                    "down": flag("Walk callees instead of callers"), "limit": int("Lines per layer; default 25", 0, 1000),
                    "stale": flag("Skip the refresh against the tree") }),
            &["symbol"]),
        tool("trace",
            "The shortest chain of calls from one symbol to another within `depth` hops, or that there is none.",
            json!({ "from": text("Source symbol"), "to": text("Target symbol"), "depth": int("Hops; default 6", 1, 12) }),
            &["from", "to"]),
        tool("changes",
            "What the working tree's diff touches — hunks mapped onto symbols — and who reaches those, with a risk line. `base` is HEAD; pass main for the whole branch.",
            json!({ "base": text("Git ref to diff against; default HEAD"), "depth": int("Hops; default 2", 1, 4),
                    "limit": int("Lines per list; default 25", 0, 1000), "stale": flag("Skip the refresh against the tree") }),
            &[]),
    ]
}

/// What a harness puts in the system prompt: when to call what, in the words the README uses,
/// and the store line so an agent knows from the start whether its answers are fused or
/// lexical and whether `enrich` ever ran.
pub fn instructions(status_line: &str) -> String {
    format!(concat!(
        "repograph: a graph of this repository's requirement documents and TypeScript, built and refreshed at zero model tokens. ",
        "Ask it before reading files. ask: a concept, an id or a symbol name — five `ID  path:line  headline` lines, ~200 tokens. ",
        "explain: one node and its edges. impact: before editing a symbol — callers by depth and a risk line. ",
        "trace: how A reaches B. changes: before committing. Every call refreshes the graph from the tree first. ",
        "What the graph cannot prove it does not list: confirm a \"nothing uses this\" with rg before deleting. ",
        "{}"), status_line.trim_end())
}

fn str_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key).and_then(Value::as_str).filter(|s| !s.trim().is_empty()).ok_or_else(|| anyhow::anyhow!("`{key}` is required"))
}

fn bool_arg(args: &Value, key: &str) -> bool { args.get(key).and_then(Value::as_bool).unwrap_or(false) }

/// Clamped rather than refused: a model that asks for depth 40 wanted "a lot", and the ceiling
/// is the answer to that, not an error it has to read and retry.
fn int_arg(args: &Value, key: &str, default: usize, min: usize, max: usize) -> usize {
    args.get(key).and_then(Value::as_u64).map(|v| (v as usize).clamp(min, max)).unwrap_or(default)
}

/// A cap of 0 means no cap: `limit: 0` is how a caller asks for the CLI's bytes whole.
fn cap(limit: usize) -> usize { if limit == 0 { usize::MAX } else { limit } }

/// One reply line for one message, or none for a notification. `handle` is given the method
/// and the params and answers with a result or a `(code, message)` error.
fn reply_line(msg: &Value, handle: impl FnOnce(&str, &Value) -> std::result::Result<Value, (i64, String)>) -> Option<String> {
    let id = msg.get("id")?.clone();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let body = match handle(method, &params) {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
    };
    Some(body.to_string())
}
```

Run: `cargo test --release --manifest-path "$W/Cargo.toml" mcp:: 2>&1 | grep 'test result'` — expected `5 passed`. If `the_surface_fits_under_its_ceilings…` fails on bytes, shorten a description — never raise the ceiling.

- [ ] **Step 3: The loop and the tools**

Below `reply_line` in `src/mcp.rs`:

```rust
/// A resident context and the watcher that keeps it in step with the tree, opened on the
/// first call that finds a store — a harness may start this process in a repository that is
/// built a minute later, and the refusal until then costs no walk.
struct Resident<'a> { watcher: crate::Watcher<'a>, ctx: ask::Context }

struct Server<'a> {
    repo: &'a Path,
    cfg: &'a config::Config,
    store: store::Store,
    no_dense: bool,
    resident: Option<Resident<'a>>,
    /// The first answer of a session carries the store line; the rest do not pay for it.
    said_store: bool,
    last_ask: Instant,
}

impl<'a> Server<'a> {
    fn open(repo: &'a Path, cfg: &'a config::Config, no_dense: bool) -> Server<'a> {
        Server { repo, cfg, store: store::Store::new(repo), no_dense, resident: None, said_store: false, last_ask: Instant::now() }
    }

    fn status_line(&self) -> String {
        status::read(self.repo, self.cfg).map(|s| status::render(&s)).unwrap_or_else(|e| format!("status unavailable ({e:#})\n"))
    }

    fn initialize(&self, params: &Value) -> Value {
        json!({
            "protocolVersion": negotiate(params.get("protocolVersion").and_then(Value::as_str)),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "repograph", "version": serve::VERSION },
            "instructions": instructions(&self.status_line()),
        })
    }

    /// The resident pair, opened on demand. Opened stale, as `serve` opens its context: the
    /// watcher has just walked the tree and `fresh` owns every later walk.
    fn resident(&mut self) -> Result<&mut Resident<'a>> {
        if self.resident.is_none() {
            let watcher = crate::Watcher::open(self.repo, self.cfg)?;
            let mut ctx = ask::Context::open(self.repo, self.cfg, true, self.no_dense)?;
            for n in ctx.notices() { eprintln!("mcp: {n}"); }
            self.resident = Some(Resident { watcher, ctx });
        }
        Ok(self.resident.as_mut().unwrap())
    }

    /// `(text, is_error)`. `is_error` is for the cases where nothing was answered at all — no
    /// store, a broken one, no git — never for a question the graph answered with nothing.
    fn call(&mut self, name: &str, args: &Value) -> Result<(String, bool)> {
        if !self.store.has("graph.json") {
            // Said here rather than left to the refresh: a poll over an empty manifest would
            // extract the whole tree, and building a store is a person's decision — the model,
            // the minutes and the `.gitignore` line — made in a shell, not inside a tool call.
            let s = status::read(self.repo, self.cfg)?;
            return Ok((format!("{}Nothing here builds it.\n", status::render(&s)), true));
        }
        let stale = bool_arg(args, "stale");
        let mut text = match name {
            "ask" => {
                self.last_ask = Instant::now();
                let words: Vec<String> = str_arg(args, "query")?.split_whitespace().map(str::to_string).collect();
                let req = ask::Request { words, json: false, seeds: int_arg(args, "seeds", 5, 1, 8), bodies: bool_arg(args, "bodies"), rerank: false, rerank_local: false, depth: rerank::DEPTH, stale, no_dense: self.no_dense };
                // A `serve` that answers is the same bytes at a fraction of the cost, and two
                // harness sessions on one worktree then share one model. Its client already
                // refuses another build, another arm and another version, on stderr — this
                // process's log, not the answer.
                match serve::try_ask(self.repo, &req) {
                    Some(reply) => { eprintln!("mcp: ask answered by the resident serve"); lines(reply.stderr) + &reply.stdout }
                    None => {
                        let r = self.resident()?;
                        serve::fresh(&mut r.watcher, &mut r.ctx, stale)?;
                        let out = r.ctx.answer(&req)?;
                        lines(r.ctx.notices()) + &out
                    }
                }
            }
            "explain" => {
                let node = str_arg(args, "node")?.to_string();
                let limit = cap(int_arg(args, "limit", 40, 0, 1000));
                let r = self.resident()?;
                serve::fresh(&mut r.watcher, &mut r.ctx, stale)?;
                let notices = lines(r.ctx.notices());
                notices + &query::explain_capped(r.ctx.graph(), &node, limit).unwrap_or_else(|| no_node(&node))
            }
            "impact" => {
                let symbol = str_arg(args, "symbol")?.to_string();
                let (depth, down, limit) = (int_arg(args, "depth", 3, 1, 6), bool_arg(args, "down"), cap(int_arg(args, "limit", LINES, 0, 1000)));
                let r = self.resident()?;
                serve::fresh(&mut r.watcher, &mut r.ctx, stale)?;
                let notices = lines(r.ctx.notices());
                let graph = r.ctx.graph();
                notices + &match query::resolve(graph, &symbol) {
                    None => no_node(&symbol),
                    Some(root) => {
                        let (imp, direction) = if down { (impact::downstream(graph, &root.id, depth), "downstream") } else { (impact::upstream(graph, &root.id, depth), "upstream") };
                        impact::render_capped(graph, &imp, direction, limit)
                    }
                }
            }
            "trace" => {
                let (from, to) = (str_arg(args, "from")?.to_string(), str_arg(args, "to")?.to_string());
                let depth = int_arg(args, "depth", 6, 1, 12);
                let r = self.resident()?;
                serve::fresh(&mut r.watcher, &mut r.ctx, stale)?;
                let notices = lines(r.ctx.notices());
                let graph = r.ctx.graph();
                notices + &match (query::resolve(graph, &from), query::resolve(graph, &to)) {
                    (None, _) => no_node(&from),
                    (_, None) => no_node(&to),
                    (Some(a), Some(b)) => match impact::trace(graph, &a.id, &b.id, depth) {
                        Some(path) => impact::render_trace(graph, &path),
                        None => format!("no call path from {} to {} within {depth} hops\n", a.id, b.id),
                    },
                }
            }
            "changes" => {
                let base = args.get("base").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("HEAD").to_string();
                let (depth, limit) = (int_arg(args, "depth", 2, 1, 4), cap(int_arg(args, "limit", LINES, 0, 1000)));
                let hunks = match changes::hunks_from_git(self.repo, &base) {
                    Ok(h) => h,
                    Err(e) => return Ok((format!("changes: {e:#}\n"), true)),
                };
                let r = self.resident()?;
                serve::fresh(&mut r.watcher, &mut r.ctx, stale)?;
                let notices = lines(r.ctx.notices());
                let report = changes::report(r.ctx.graph(), &hunks, depth);
                notices + &changes::render_capped(r.ctx.graph(), &report, limit)
            }
            other => anyhow::bail!("unknown tool {other}"),
        };
        if name == "ask" && text.trim().is_empty() {
            text = "no match — an id or a symbol name answers exactly; otherwise try fewer, more specific words\n".to_string();
        }
        if !self.said_store {
            self.said_store = true;
            text = self.status_line() + &text;
        }
        Ok((text, false))
    }

    /// Between requests: the poll `serve` does, and the model given back after a quiet spell.
    fn idle(&mut self, every: u64, idle_model: u64, last_poll: &mut Instant) -> Result<()> {
        if let Some(r) = self.resident.as_mut() {
            if last_poll.elapsed() >= Duration::from_secs(every) {
                *last_poll = Instant::now();
                serve::adopt_if_moved(&mut r.watcher, &mut r.ctx, 1)?;
                for n in r.ctx.notices() { eprintln!("mcp: {n}"); }
            }
            if idle_model > 0 && r.ctx.model_open() && self.last_ask.elapsed() >= Duration::from_secs(idle_model) {
                r.ctx.release_model();
                eprintln!("mcp: no ask for {idle_model}s, model released");
            }
        }
        Ok(())
    }
}

fn lines(notices: Vec<String>) -> String { notices.into_iter().map(|n| n + "\n").collect() }

fn no_node(needle: &str) -> String {
    format!("no node matches {needle} — symbols are named as declared (StaffService, StaffService.create) or as sym:<file>::<Name>; ids as written (FR-PAY-22)\n")
}

pub fn run(repo: &Path, cfg: &config::Config, every: u64, idle_model: u64, no_dense: bool) -> Result<()> {
    // The read blocks on a thread of its own, as `serve`'s accept does, so the loop below can
    // wake for its poll and its idle deadline while no message is waiting. A rendezvous
    // channel keeps it to one message in flight; the harness sends the next after the reply.
    let (tx, rx) = std::sync::mpsc::sync_channel::<String>(0);
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(l) if l.trim().is_empty() => {}
                Ok(l) => if tx.send(l).is_err() { return },
                Err(_) => return,
            }
        }
    });
    let mut server = Server::open(repo, cfg, no_dense);
    let stdout = std::io::stdout();
    let mut last_poll = Instant::now();
    eprintln!("mcp: {} every {every}s, model released after {idle_model}s idle; exits when stdin closes", repo.display());
    loop {
        match rx.recv_timeout(serve::WAKE) {
            Ok(line) => {
                let reply = match serde_json::from_str::<Value>(&line) {
                    Ok(msg) => reply_line(&msg, |method, params| match method {
                        "initialize" => Ok(server.initialize(params)),
                        "ping" => Ok(json!({})),
                        "tools/list" => Ok(json!({ "tools": tools() })),
                        "tools/call" => {
                            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                            match server.call(name, &args) {
                                Ok((text, is_error)) => Ok(json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })),
                                Err(e) => Err((-32602, format!("{e:#}"))),
                            }
                        }
                        other => Err((-32601, format!("method not found: {other}"))),
                    }),
                    Err(e) => Some(json!({ "jsonrpc": "2.0", "id": Value::Null, "error": { "code": -32700, "message": format!("parse error: {e}") } }).to_string()),
                };
                if let Some(text) = reply {
                    let mut out = stdout.lock();
                    writeln!(out, "{text}").context("stdout")?;
                    out.flush().context("stdout")?;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            // Stdin closed: the harness is gone, and so is the reason to hold a model.
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }
        server.idle(every, idle_model, &mut last_poll)?;
    }
}
```

The closure handed to `reply_line` borrows `server` mutably while `tools()` and `initialize` borrow nothing that conflicts; if the borrow checker objects to `server` inside a `FnOnce` alongside the `match`, move the `match method { … }` into a method `Server::dispatch(&mut self, method, params) -> Result<Value, (i64, String)>` and pass `|m, p| server.dispatch(m, p)`.

In `src/main.rs`, `enum Cmd`, after `Watch`:

```rust
    /// Answers `ask`, `explain`, `impact`, `trace` and `changes` over the Model Context Protocol
    /// on stdin and stdout, for a coding agent's harness: refreshes before every answer, polls
    /// between them, uses a `serve` when one answers, never builds a store, exits when stdin
    /// closes. With `--repo` left at its default the repository is the nearest ancestor of the
    /// working directory holding a store, else a `.git`.
    Mcp {
        #[arg(long, default_value_t = 30)] every: u64,
        /// Seconds without an `ask` before the embedding model is closed to give its memory
        /// back; the next fused question opens it again. 0 keeps it open.
        #[arg(long, default_value_t = 900)] idle_model: u64,
    },
```

and the arm, beside `Serve`:

```rust
        Cmd::Mcp { every, idle_model } => {
            let root = if cli.repo == std::path::Path::new(".") { mcp::find_root(&repo) } else { repo.clone() };
            mcp::run(&root, &config::Config::load(&root)?, every, idle_model, cli.no_dense)
        }
```

`repo` was canonicalised from `cli.repo` above; the comparison is on the flag as given. Run `cargo build --release --manifest-path "$W/Cargo.toml" 2>&1 | grep -E "^(warning|error)" | head`; expected none, then `cargo clippy --release --all-targets --manifest-path "$W/Cargo.toml" -- -D warnings 2>&1 | tail -1`.

- [ ] **Step 4: The integration tests**

Create `tests/mcp.rs`:

```rust
//! `repograph mcp` driven over stdin and stdout the way a harness drives it: one JSON-RPC line
//! in, one line out. The repository these tests build has no vectors, so nothing here opens
//! the 448 MB model; the fused arm is exercised for its handshake and its notices.
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

fn repograph() -> Command { Command::new(env!("CARGO_BIN_EXE_repograph")) }

fn repo_with_docs_and_code() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/pay.md"), "**FR-PAY-1 · MUST · Штраф за отмену**\n\nШтраф списывается сам (INV-1).\n\n**INV-1 · MUST · Деньги не сгорают**\n\nОтмена не сжигает деньги.\n").unwrap();
    std::fs::write(dir.path().join("docs/cal.md"), "**FR-CAL-1 · MUST · Перенос визита**\n\nПеренос не считается отменой.\n").unwrap();
    std::fs::write(dir.path().join("s.ts"), "export class S {\n  create() { return 1; }\n}\n").unwrap();
    // Three files that `new` the class: three callers at d=1, so a limit of one leaves two.
    for i in 0..3 { std::fs::write(dir.path().join(format!("c{i}.ts")), "import { S } from './s';\nexport const made = new S();\n").unwrap(); }
    std::fs::write(dir.path().join("repograph.toml"), "id_families = [\"FR-PAY\", \"FR-CAL\", \"INV\"]\n").unwrap();
    let out = repograph().args(["--no-dense", "--repo"]).arg(dir.path()).arg("build").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(out.stderr.as_slice()));
    dir
}

struct Mcp { child: Child, stdin: ChildStdin, stdout: BufReader<ChildStdout>, next: u64 }

impl Mcp {
    fn start(dir: &std::path::Path, extra: &[&str]) -> Mcp {
        let mut child = repograph().args(["--no-dense", "--repo"]).arg(dir).arg("mcp").args(extra)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Mcp { child, stdin, stdout, next: 1 }
    }

    /// One request, one reply. A harness never sends the next before the reply, and neither
    /// does this.
    fn request(&mut self, method: &str, params: serde_json::Value) -> serde_json::Value {
        let id = self.next; self.next += 1;
        writeln!(self.stdin, "{}", serde_json::json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })).unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let v: serde_json::Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("not a JSON line: {line:?} ({e})"));
        assert_eq!(v["id"], id, "{line}");
        v
    }

    fn notify(&mut self, method: &str) {
        writeln!(self.stdin, "{}", serde_json::json!({ "jsonrpc": "2.0", "method": method })).unwrap();
        self.stdin.flush().unwrap();
    }

    fn call(&mut self, tool: &str, args: serde_json::Value) -> (String, bool) {
        let v = self.request("tools/call", serde_json::json!({ "name": tool, "arguments": args }));
        let r = &v["result"];
        (r["content"][0]["text"].as_str().unwrap_or_else(|| panic!("no text in {v}")).to_string(), r["isError"].as_bool().unwrap_or(false))
    }

    /// Closing stdin is how a harness stops its server; the exit is the test that it listens.
    fn close(mut self) -> String {
        drop(self.stdin);
        let out = self.child.wait_with_output().unwrap();
        assert!(out.status.success(), "mcp exited {:?}: {}", out.status, String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stderr).into_owned()
    }
}

fn cli(dir: &std::path::Path, args: &[&str]) -> String {
    let out = repograph().args(["--no-dense", "--repo"]).arg(dir).args(args).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn initialize_lists_five_tools_under_the_ceiling_and_the_first_answer_carries_the_store_line() {
    let dir = repo_with_docs_and_code();
    let mut m = Mcp::start(dir.path(), &[]);
    let init = m.request("initialize", serde_json::json!({ "protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } }));
    assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(init["result"]["serverInfo"]["name"], "repograph");
    let instructions = init["result"]["instructions"].as_str().unwrap();
    assert!(instructions.contains("store: ") && instructions.contains("no vectors"), "{instructions}");
    m.notify("notifications/initialized");
    let list = m.request("tools/list", serde_json::json!({}));
    let tools = list["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.iter().map(|t| t["name"].as_str().unwrap()).collect::<Vec<_>>(), ["ask", "explain", "impact", "trace", "changes"]);
    assert!(serde_json::to_string(tools).unwrap().len() <= 3_200);
    let (first, err) = m.call("explain", serde_json::json!({ "node": "FR-CAL-1" }));
    assert!(!err);
    assert!(first.starts_with("store: "), "the first answer says what the store is: {first}");
    assert!(first.contains("FR-CAL-1  docs/cal.md:1"), "{first}");
    let (second, _) = m.call("explain", serde_json::json!({ "node": "FR-CAL-1" }));
    assert!(!second.starts_with("store: "), "only the first answer pays for the line: {second}");
    assert_eq!(second, cli(dir.path(), &["explain", "FR-CAL-1"]), "explain answers the CLI's bytes");
    m.close();
}

#[test]
fn ask_over_mcp_answers_the_bytes_the_cli_answers_and_sees_an_edit_first() {
    let dir = repo_with_docs_and_code();
    let mut m = Mcp::start(dir.path(), &["--every", "3600"]);
    m.call("explain", serde_json::json!({ "node": "INV-1" }));
    for q in ["штраф", "перенос визита", "FR-CAL-1", "S"] {
        let (got, err) = m.call("ask", serde_json::json!({ "query": q }));
        assert!(!err);
        let words: Vec<&str> = q.split_whitespace().collect();
        let mut args = vec!["ask", "--no-serve"]; args.extend(words);
        assert_eq!(got, cli(dir.path(), &args), "question {q:?}");
    }
    let (none, err) = m.call("ask", serde_json::json!({ "query": "ничего" }));
    assert!(!err && none.starts_with("no match"), "{none}");
    std::fs::write(dir.path().join("docs/new.md"), "**FR-PAY-2 · MUST · Возврат аванса**\n\nАванс возвращается при отмене салоном.\n").unwrap();
    let (after, _) = m.call("ask", serde_json::json!({ "query": "возврат аванса" }));
    assert!(after.starts_with("refresh: 1 changed, 0 removed\n"), "the refresh is the first line of the answer: {after}");
    assert!(after.contains("FR-PAY-2"), "{after}");
    let (again, _) = m.call("ask", serde_json::json!({ "query": "возврат аванса" }));
    assert!(!again.starts_with("refresh"), "a quiet tree prints no refresh line: {again}");
    m.close();
}

#[test]
fn impact_caps_a_layer_at_the_limit_and_keeps_the_risk_line_and_zero_means_uncapped() {
    let dir = repo_with_docs_and_code();
    let mut m = Mcp::start(dir.path(), &[]);
    m.call("explain", serde_json::json!({ "node": "S" }));
    let (capped, _) = m.call("impact", serde_json::json!({ "symbol": "S", "limit": 1 }));
    assert!(capped.contains("d=1  will break (3)\n"), "{capped}");
    assert!(capped.contains("  … 2 more (limit 1)\n"), "{capped}");
    assert!(capped.contains("risk: MEDIUM — 3 direct, 3 total, 3 files\n"), "{capped}");
    let (whole, _) = m.call("impact", serde_json::json!({ "symbol": "S", "limit": 0 }));
    assert_eq!(whole, cli(dir.path(), &["impact", "S"]));
    let (missing, err) = m.call("impact", serde_json::json!({ "symbol": "Nowhere" }));
    assert!(!err && missing.starts_with("no node matches Nowhere"), "{missing}");
    let (trace, _) = m.call("trace", serde_json::json!({ "from": "made", "to": "S" }));
    assert!(trace.starts_with("sym:c0.ts::made") || trace.contains("no call path"), "{trace}");
    m.close();
}

#[test]
fn a_repository_without_a_store_is_refused_by_every_tool_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/a.md"), "**FR-PAY-1 · MUST · x**\n\nbody\n").unwrap();
    let mut m = Mcp::start(dir.path(), &[]);
    let init = m.request("initialize", serde_json::json!({ "protocolVersion": "2025-06-18" }));
    assert!(init["result"]["instructions"].as_str().unwrap().contains("no store at"));
    for (tool, args) in [("ask", serde_json::json!({ "query": "x" })), ("impact", serde_json::json!({ "symbol": "x" })), ("changes", serde_json::json!({}))] {
        let (text, err) = m.call(tool, args);
        assert!(err, "{tool} must refuse: {text}");
        assert!(text.contains("repograph build") && text.contains("Nothing here builds it"), "{text}");
    }
    m.close();
    assert!(!dir.path().join(".repograph").exists(), "a refusal writes nothing");
}

#[test]
fn a_store_built_after_the_server_started_is_answered_without_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/a.md"), "**FR-PAY-1 · MUST · штраф**\n\nbody\n").unwrap();
    std::fs::write(dir.path().join("repograph.toml"), "id_families = [\"FR-PAY\"]\n").unwrap();
    let mut m = Mcp::start(dir.path(), &[]);
    assert!(m.call("ask", serde_json::json!({ "query": "штраф" })).1, "refused before the build");
    cli(dir.path(), &["build"]);
    let (text, err) = m.call("ask", serde_json::json!({ "query": "штраф" }));
    assert!(!err && text.contains("FR-PAY-1"), "{text}");
    m.close();
}

#[test]
fn two_servers_on_one_store_both_see_an_edit_and_the_store_still_loads() {
    let dir = repo_with_docs_and_code();
    let mut a = Mcp::start(dir.path(), &["--every", "3600"]);
    let mut b = Mcp::start(dir.path(), &["--every", "3600"]);
    a.call("ask", serde_json::json!({ "query": "штраф" }));
    b.call("ask", serde_json::json!({ "query": "штраф" }));
    std::fs::write(dir.path().join("docs/new.md"), "**FR-PAY-2 · MUST · Возврат аванса**\n\nАванс возвращается.\n").unwrap();
    let (from_a, _) = a.call("ask", serde_json::json!({ "query": "возврат аванса" }));
    let (from_b, _) = b.call("ask", serde_json::json!({ "query": "возврат аванса" }));
    assert!(from_a.contains("FR-PAY-2") && from_b.contains("FR-PAY-2"), "{from_a}\n{from_b}");
    a.close(); b.close();
    let verify = cli(dir.path(), &["verify"]);
    assert!(verify.starts_with("nodes: "), "{verify}");
    assert!(cli(dir.path(), &["ask", "--no-serve", "возврат", "аванса"]).contains("FR-PAY-2"));
}

#[test]
fn a_resident_serve_answers_the_ask_tool_with_the_same_bytes() {
    let dir = repo_with_docs_and_code();
    let mut serve = repograph().args(["--no-dense", "--repo"]).arg(dir.path()).args(["serve", "--every", "3600", "--idle", "60"])
        .stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    let sock = dir.path().join(".repograph/serve.sock");
    let start = std::time::Instant::now();
    while !sock.exists() { assert!(start.elapsed().as_secs() < 20, "serve never bound"); std::thread::sleep(std::time::Duration::from_millis(50)); }
    let mut m = Mcp::start(dir.path(), &[]);
    m.call("explain", serde_json::json!({ "node": "INV-1" }));
    let (got, _) = m.call("ask", serde_json::json!({ "query": "штраф" }));
    assert_eq!(got, cli(dir.path(), &["ask", "--no-serve", "штраф"]));
    let log = m.close();
    assert!(log.contains("mcp: ask answered by the resident serve"), "{log}");
    serve.kill().unwrap(); let _ = serve.wait();
}

#[test]
fn a_notification_gets_no_reply_an_unknown_method_an_error_and_a_broken_line_a_parse_error() {
    let dir = repo_with_docs_and_code();
    let mut m = Mcp::start(dir.path(), &[]);
    m.notify("notifications/initialized");
    let ping = m.request("ping", serde_json::json!({}));
    assert_eq!(ping["result"], serde_json::json!({}), "the reply to the ping is the first line out: {ping}");
    let unknown = m.request("resources/list", serde_json::json!({}));
    assert_eq!(unknown["error"]["code"], -32601);
    writeln!(m.stdin, "this is not json").unwrap(); m.stdin.flush().unwrap();
    let mut line = String::new(); m.stdout.read_line(&mut line).unwrap();
    let v: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(v["error"]["code"], -32700);
    let bad = m.request("tools/call", serde_json::json!({ "name": "ask", "arguments": {} }));
    assert_eq!(bad["error"]["code"], -32602, "a missing required argument is a protocol error, not an answer: {bad}");
    m.close();
}
```

Run: `cargo test --release --manifest-path "$W/Cargo.toml" --test mcp 2>&1 | grep -E 'test result|panicked|FAILED'` — expected `8 passed`. The `trace` assertion in the third test accepts either shape because whether a top-level `const` initialiser's `new` is attributed to the const is the extractor's business (`src/code/cases.rs`), not this task's; the test that pins the cap is `impact`'s.

- [ ] **Step 5: The stdlib client, and Rule 2 on the copy**

Create `bench/agent/mcp_client.py`:

```python
#!/usr/bin/env python3
"""A harness in fifty lines: starts `repograph mcp`, speaks JSON-RPC over its stdio, and
returns tool results as text. Used by the Rule 2 check (every recorded question answers the
CLI's bytes through the server) and by anyone who wants to see what a harness sees.

    python3 bench/agent/mcp_client.py --bin <repograph> --repo <dir> [--no-dense] ask "штраф за отмену"
"""
import argparse
import json
import subprocess
import sys


class Client:
    def __init__(self, binary, repo, no_dense=False, extra=()):
        args = [binary]
        if no_dense:
            args.append("--no-dense")
        args += ["--repo", repo, "mcp", *extra]
        self.proc = subprocess.Popen(args, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
        self.next_id = 1
        self.init = self.request("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "mcp_client.py", "version": "0"}})
        self.notify("notifications/initialized")

    def request(self, method, params):
        rid, self.next_id = self.next_id, self.next_id + 1
        self.proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "method": method, "params": params}, ensure_ascii=False) + "\n")
        self.proc.stdin.flush()
        line = self.proc.stdout.readline()
        if not line:
            raise RuntimeError("server closed: " + self.proc.stderr.read())
        reply = json.loads(line)
        if reply.get("id") != rid:
            raise RuntimeError(f"reply {reply.get('id')} to request {rid}")
        return reply

    def notify(self, method):
        self.proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": method}) + "\n")
        self.proc.stdin.flush()

    def call(self, tool, **arguments):
        reply = self.request("tools/call", {"name": tool, "arguments": arguments})
        if "error" in reply:
            raise RuntimeError(json.dumps(reply["error"], ensure_ascii=False))
        result = reply["result"]
        return result["content"][0]["text"], bool(result.get("isError"))

    def close(self):
        self.proc.stdin.close()
        self.proc.wait(timeout=30)
        return self.proc.stderr.read()


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin", required=True)
    ap.add_argument("--repo", required=True)
    ap.add_argument("--no-dense", action="store_true")
    ap.add_argument("tool")
    ap.add_argument("arg", nargs="?", default="")
    a = ap.parse_args()
    c = Client(a.bin, a.repo, a.no_dense)
    key = {"ask": "query", "explain": "node", "impact": "symbol"}.get(a.tool)
    text, is_error = c.call(a.tool, **({key: a.arg} if key else {}))
    sys.stdout.write(text)
    c.close()
    sys.exit(1 if is_error else 0)


if __name__ == "__main__":
    main()
```

Then Rule 2 on the copy, both arms, the 142 questions of Task 2 (`$A/questions.txt`), the first answer's store line stripped:

```bash
C=/Users/max/bench/agent-2026-09-06/bc
python3 - "$B" "$C" "$A/questions.txt" "$W/bench/agent/mcp_client.py" <<'PY' 2>&1 | tee "$A/t3-rule2.txt"
import subprocess, sys
sys.path.insert(0, sys.argv[4].rsplit("/", 1)[0])
from mcp_client import Client
binary, repo, qfile = sys.argv[1:4]
for arm, nd in (("lexical", True), ("dense", False)):
    c = Client(binary, repo, no_dense=nd)
    c.call("explain", node="FR-PAY-22")     # consumes the first answer's store line
    same = n = 0
    for q in open(qfile, encoding="utf-8").read().splitlines():
        mcp, _ = c.call("ask", query=q, stale=True)
        cli = subprocess.run([binary] + (["--no-dense"] if nd else []) + ["--repo", repo, "ask", "--stale", "--no-serve", *q.split()], capture_output=True, text=True)
        want = "".join(l + "\n" for l in cli.stderr.splitlines() if l.startswith(("refresh:", "dense:"))) + cli.stdout
        if not want.strip():
            want = "no match — an id or a symbol name answers exactly; otherwise try fewer, more specific words\n"
        n += 1
        same += mcp == want
    print(f"{arm}: mcp == cli on {same}/{n}")
    c.close()
PY
```

Expected: `lexical: mcp == cli on 142/142` and `dense: mcp == cli on 142/142`. The dense arm opens the small model once (0.3 s) and answers 142 questions from it; `--stale` keeps both sides off the walk so the comparison is of the answer and not of who refreshed first. A mismatch is **BLOCKED**: the server answers through `Context::answer` and any difference is a notice placed wrongly.

- [ ] **Step 6: Rule 1, then the manual round trip and Codex**

Rule 1 as in Task 1 Step 6 with the `t3-` prefix; eight `identical` expected. Then the two harnesses, by hand, on the copy:

```bash
cat > "$A/mcp.json" <<JSON
{ "mcpServers": { "repograph": { "command": "$B", "args": ["mcp"] } } }
JSON
cd "$C" && claude -p --output-format json --setting-sources "" --strict-mcp-config --mcp-config "$A/mcp.json" --allowedTools "mcp__repograph" --max-turns 6 \
  "Use the repograph ask tool to find which requirement id covers cancellation fees (штраф за отмену). Reply with the id only." | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["result"]); print("turns", d["num_turns"], "usage", d["usage"])' | tee "$A/t3-claude.txt"
codex mcp add repograph -- "$B" mcp && codex mcp list | tee "$A/t3-codex-list.txt"
cd "$C" && codex exec --skip-git-repo-check "Use the repograph MCP tool 'ask' with query 'штраф за отмену' and reply with the first id it returned." 2>&1 | tail -5 | tee "$A/t3-codex.txt"
codex mcp remove repograph
```

Expected: the Claude answer names `FR-PAY-22` or another `FR-PAY-*` id, with a `usage` object; `codex mcp list` shows `repograph`; the Codex tail carries an id. Either harness failing to connect is recorded with its message and is not a blocker for the Rust work — it is what Task 5's shim and README section then have to answer.

- [ ] **Step 7: Commit**

`git -C "$W" add src/mcp.rs src/main.rs tests/mcp.rs bench/agent/mcp_client.py` — subject `feat(mcp): the five readers over stdio for a coding agent's harness`, body naming the surface bytes as the test read them, the 142/142 lines (`$A/t3-rule2.txt`), the Rule 1 verdicts and the two harness transcripts.

---

### Task 4: Bounded waiting — the vectors embedded in slices, inside a call and between calls

**Files:**
- Modify: `src/index/dense.rs` (`sync` takes `max_new: Option<usize>` and returns `Synced`; tests), `src/ask.rs` (`Context::inline_rows`, `Context::with_inline_rows`, `Context::embed_pending`, `pending`; the resync reads `Synced`), `src/main.rs` (`run_watch`, `embed_all` pass `None` and read `.embedded`), `src/mcp.rs` (`INLINE_ROWS`, `IDLE_ROWS`; the context opened with the cap; `idle` embeds a slice), `src/dump.rs` and `src/bench.rs` only if they call `sync` (they do not)
- Create (scratch): `$A/t4-*.txt`

**Interfaces:**
- Produces: `pub struct Synced { pub embedded: usize, pub deferred: usize }`; `DenseIndex::sync(&mut self, graph, questions, embed, max_new: Option<usize>) -> Result<Synced>` — `None` is today's behaviour exactly; `Some(m)` embeds at most `m` new rows, keeps the previous row of every node whose new row was deferred alive, and reports the rest as `deferred`. `Context::with_inline_rows(self, Option<usize>) -> Context`; `Context::pending(&self) -> bool`; `Context::embed_pending(&mut self, max: usize) -> Result<usize>` — a slice of the rows still behind, only while the model is open, saved when anything was embedded.
- Rule 4 (bounded waiting): on the copy's small-model store, an `ask` after an edit of 60 documents answers in under 5 s and reports the rows it left behind.

- [ ] **Step 1: The failing tests in `dense.rs`**

In `src/index/dense.rs`'s `mod tests`, using whatever graph-building helpers the module already has (`Extraction::node`, a fake `embed` closure returning `vec![1.0, 0.0, 0.0, 0.0]` per text — look at the neighbouring tests and copy their helper names exactly):

```rust
    /// Every row a unit vector on the first axis, so nothing here depends on a model.
    fn unit(texts: &[String]) -> Result<Vec<Vec<f32>>> { Ok(texts.iter().map(|_| vec![1.0, 0.0, 0.0, 0.0]).collect()) }

    fn three_requirements(bodies: [&str; 3]) -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        for (i, b) in bodies.iter().enumerate() {
            e.node(NodeKind::Requirement, &format!("FR-A-{i}"), &format!("label {i}"), b, "a.md", 1 + i as u32);
        }
        g.apply(e);
        g
    }

    #[test]
    fn an_unbounded_sync_is_the_shipped_behaviour_and_reports_no_deferral() {
        let g = three_requirements(["one", "two", "three"]);
        let mut idx = DenseIndex::default();
        let s = idx.sync(&g, &Questions::default(), &mut unit, None).unwrap();
        assert_eq!((s.embedded, s.deferred), (3, 0));
        let s = idx.sync(&g, &Questions::default(), &mut unit, None).unwrap();
        assert_eq!((s.embedded, s.deferred), (0, 0), "a second sync over the same rows embeds nothing");
    }

    #[test]
    fn a_bounded_sync_embeds_the_cap_keeps_the_old_row_of_a_deferred_node_and_finishes_next_time() {
        let g = three_requirements(["one", "two", "three"]);
        let mut idx = DenseIndex::default();
        idx.sync(&g, &Questions::default(), &mut unit, None).unwrap();
        // Two nodes change; only one new row fits under the cap.
        let g2 = three_requirements(["one", "two changed", "three changed"]);
        let s = idx.sync(&g2, &Questions::default(), &mut unit, Some(1)).unwrap();
        assert_eq!((s.embedded, s.deferred), (1, 1));
        // The deferred node still answers by its previous row: three live rows, not two.
        assert_eq!(idx.live.len(), 3, "{:?}", idx.ids);
        let s = idx.sync(&g2, &Questions::default(), &mut unit, Some(1)).unwrap();
        assert_eq!((s.embedded, s.deferred), (1, 0));
        assert_eq!(idx.live.len(), 3);
        let s = idx.sync(&g2, &Questions::default(), &mut unit, Some(1)).unwrap();
        assert_eq!((s.embedded, s.deferred), (0, 0));
    }

    #[test]
    fn a_cap_of_zero_embeds_nothing_and_defers_everything() {
        let g = three_requirements(["one", "two", "three"]);
        let mut idx = DenseIndex::default();
        let s = idx.sync(&g, &Questions::default(), &mut unit, Some(0)).unwrap();
        assert_eq!((s.embedded, s.deferred), (0, 3));
        assert!(idx.ids.is_empty());
    }
```

Run: `cargo test --release --manifest-path "$W/Cargo.toml" bounded_sync 2>&1 | tail -3` — expected: compile errors (`sync` takes three arguments; `Synced` does not exist).

- [ ] **Step 2: Implement the bound**

In `src/index/dense.rs`, above `impl DenseIndex`:

```rust
/// What a sync did and what it left: `deferred` rows are new or changed passages whose vectors
/// are not yet written because the caller bounded the work — a tool call that must answer in
/// seconds on a store whose default model embeds at ~80 ms a row.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Synced { pub embedded: usize, pub deferred: usize }
```

Change `sync`'s signature to `pub fn sync(&mut self, graph: &Graph, questions: &Questions, embed: &mut dyn FnMut(&[String]) -> Result<Vec<Vec<f32>>>, max_new: Option<usize>) -> Result<Synced>` and, between the loop that fills the `todo_*` vectors and `let embedded = todo_ids.len();`, insert:

```rust
        // The bound: what does not fit is left for the next sync, and a node whose new row is
        // deferred keeps its previous rows alive, so a changed requirement still answers by its
        // old text until its new vector is written rather than vanishing from the dense list.
        let deferred = max_new.map_or(0, |m| todo_ids.len().saturating_sub(m));
        if deferred > 0 {
            let keep = max_new.unwrap();
            let by_id: std::collections::HashMap<&str, Vec<usize>> = self.live.iter().fold(std::collections::HashMap::new(), |mut acc, &i| { acc.entry(self.ids[i].as_str()).or_default().push(i); acc });
            for id in &todo_ids[keep..] {
                for &i in by_id.get(id.as_str()).into_iter().flatten() { alive[i] = true; }
            }
            todo_ids.truncate(keep); todo_texts.truncate(keep); todo_hashes.truncate(keep); todo_kinds.truncate(keep);
        }
```

and replace the final `Ok(embedded)` with `Ok(Synced { embedded, deferred })`. The borrow of `self.live` and `self.ids` in `by_id` ends before `alive` is written; if the checker objects, collect `by_id` into owned `String` keys first. Every existing caller becomes `sync(…, None)?.embedded`: `run_watch` and `embed_all` in `src/main.rs`, the resync in `src/ask.rs` (rewritten in Step 4), and any test in `dense.rs` that calls `sync` (append `, None` and `.embedded`).

Run: `cargo test --release --manifest-path "$W/Cargo.toml" dense:: 2>&1 | grep 'test result'` — expected all of the module's tests green, three more than before.

- [ ] **Step 3: The failing tests in `ask.rs`**

In `src/ask.rs`'s `mod tests`:

```rust
    #[test]
    fn embed_pending_is_a_no_op_while_no_model_is_open_and_the_flag_survives_it() {
        let dir = repo_with_two_docs();
        let cfg = crate::config::Config::load(dir.path()).unwrap();
        let ex = crate::extractors(dir.path(), &cfg).unwrap();
        crate::run_update(dir.path(), &cfg, &ex, true).unwrap();
        std::fs::write(dir.path().join("docs/new.md"), "**FR-PAY-2 · MUST · Возврат аванса**\n\nтело\n").unwrap();
        let mut ctx = Context::open(dir.path(), &cfg, false, false).unwrap().with_inline_rows(Some(1));
        assert!(ctx.pending(), "the refresh left rows behind");
        assert_eq!(ctx.embed_pending(64).unwrap(), 0, "nothing to embed with: no model is open");
        assert!(ctx.pending(), "and nothing was decided about them");
        assert_eq!(ctx.inline_rows, Some(1));
    }
```

Run: `cargo test --release --manifest-path "$W/Cargo.toml" embed_pending 2>&1 | tail -3` — expected: compile errors.

- [ ] **Step 4: Implement the context's half**

In `src/ask.rs`, `struct Context`: add `/// Rows a single answer may embed inline; `None` is the one-shot's and `serve`'s unbounded catch-up.\n    inline_rows: Option<usize>,` after `resync`, and `inline_rows: None,` in `open`'s literal. In `impl Context`:

```rust
    /// The same context, answering with at most `rows` vectors embedded inside one answer;
    /// the rest wait for `embed_pending`. A tool call has seconds, and the default model
    /// embeds at ~80 ms a row.
    pub fn with_inline_rows(mut self, rows: Option<usize>) -> Context { self.inline_rows = rows; self }

    /// Whether rows are still behind the graph — a refresh nobody has embedded for yet, or a
    /// bounded answer's leftovers.
    pub fn pending(&self) -> bool { self.resync.get() }

    /// A slice of the rows still behind, embedded and saved — only while the model is already
    /// open, because opening 1.9 GB to catch up in the background is a fused question's
    /// decision and not an idle loop's. Returns how many were embedded; the flag stays set
    /// while any remain.
    pub fn embed_pending(&mut self, max: usize) -> Result<usize, anyhow::Error> {
        if !self.resync.get() { return Ok(0); }
        let Context { store, graph, questions, dense_idx, embedder, resync, notices, .. } = &*self;
        let mut slot = embedder.borrow_mut();
        let Some(Some(emb)) = slot.as_mut() else { return Ok(0) };
        let mut idx = dense_idx.borrow_mut();
        let Some(idx) = idx.as_mut() else { return Ok(0) };
        let s = idx.sync(graph, questions, &mut |texts| emb.embed(texts), Some(max))?;
        if s.embedded > 0 {
            idx.save(store)?;
            notices.borrow_mut().push(format!("refresh: {} vectors embedded between calls", s.embedded));
        }
        resync.set(s.deferred > 0);
        Ok(s.embedded)
    }
```

and in `answer`'s resync block, replace `match idx.sync(graph, questions, &mut |texts| emb.embed(texts)) { Ok(0) => {} Ok(n) => { … } Err(err) => … }` with:

```rust
                    match idx.sync(graph, questions, &mut |texts| emb.embed(texts), self_inline) {
                        Ok(s) => {
                            if s.embedded > 0 {
                                if let Err(err) = idx.save(store) { notices.borrow_mut().push(format!("refresh: vectors not saved ({err:#})")); }
                                notices.borrow_mut().push(format!("refresh: {} vectors embedded", s.embedded));
                            }
                            if s.deferred > 0 {
                                // Left set: the next answer or the idle loop continues from here.
                                resync.set(true);
                                notices.borrow_mut().push(format!("dense: {} rows behind, embedded between calls", s.deferred));
                            }
                        }
                        Err(err) => notices.borrow_mut().push(format!("refresh: vectors unchanged ({err:#})")),
                    }
```

where `let self_inline = self.inline_rows;` is read at the top of `answer`, beside `let no_dense = …`. `resync` is the `Cell` destructured from `&*self` as the other fields are.

Run: `cargo test --release --manifest-path "$W/Cargo.toml" 2>&1 | grep 'test result'` — expected `456 passed` (452 + 3 + 1) + `12` + `8`.

- [ ] **Step 5: The server uses it**

In `src/mcp.rs`, beside `LINES`:

```rust
/// Rows one answer may embed before it answers from the vectors as they stand: ~10 s on the
/// default model at the ~80 ms a row the README's 2,680 s over 33,525 rows works out to, ~0.4 s
/// on the small one. The rest is embedded between calls, `IDLE_ROWS` a wake, so a harness that
/// switched branches gets its answers now and its vectors within minutes.
pub const INLINE_ROWS: usize = 128;
pub const IDLE_ROWS: usize = 64;
```

In `Server::resident`, open the context as `ask::Context::open(self.repo, self.cfg, true, self.no_dense)?.with_inline_rows(Some(INLINE_ROWS))`. In `Server::idle`, after the poll block and before the model-release block:

```rust
            if r.ctx.pending() && r.ctx.model_open() {
                match r.ctx.embed_pending(IDLE_ROWS) {
                    Ok(n) if n > 0 => eprintln!("mcp: {n} vectors embedded between calls"),
                    Ok(_) => {}
                    Err(e) => eprintln!("mcp: embedding between calls failed ({e:#})"),
                }
            }
```

The notices `embed_pending` pushes reach the next answer's text, which is where an agent learns its vectors caught up. Run the unit and integration tests again; the counts are unchanged.

- [ ] **Step 6: Rule 4 on the copy, and Rule 1**

Sixty documents edited on the copy, then one fused `ask` through the server, timed:

```bash
C=/Users/max/bench/agent-2026-09-06/bc
git -C "$C" status --porcelain -- ':(exclude)graphify-out' | wc -l
ls "$C"/docs/prd-2026-08-16/prd/*.md | head -60 > "$A/t4-files.txt"; wc -l "$A/t4-files.txt"
while IFS= read -r f; do printf '\n<!-- t4 -->\n' >> "$f"; done < "$A/t4-files.txt"
python3 - "$B" "$C" "$W/bench/agent/mcp_client.py" <<'PY' 2>&1 | tee "$A/t4-rule4.txt"
import sys, time
sys.path.insert(0, sys.argv[3].rsplit("/", 1)[0])
from mcp_client import Client
c = Client(sys.argv[1], sys.argv[2])
t = time.time(); text, _ = c.call("ask", query="штраф за отмену записи"); dt = time.time() - t
first = [l for l in text.splitlines() if l.startswith(("refresh:", "dense:"))]
print(f"first fused ask after the edit: {dt:.2f} s; notices: {first}")
time.sleep(8)
t = time.time(); text, _ = c.call("ask", query="штраф за отмену записи"); dt = time.time() - t
print(f"second ask: {dt:.2f} s; notices: {[l for l in text.splitlines() if l.startswith(('refresh:', 'dense:'))]}")
print(c.close()[-600:])
PY
git -C "$C" checkout -- docs; "$B" --repo "$C" update --no-dense | tail -1
```

Expected: the first answer under 5 s, its notices reading `refresh: 60 changed, 0 removed`, `refresh: 128 vectors embedded` and `dense: N rows behind, embedded between calls` for some N > 0 (each edited document re-embeds its file row and every requirement whose passage moved — the count is the store's to say); the second answer's notices include `refresh: … vectors embedded between calls` or nothing left behind; the server log shows `mcp: 64 vectors embedded between calls` lines. If the first answer exceeds 5 s on the small model, **Rule 4 failed**: record the time, do not raise the cap, and report. The `checkout` and `update` put the copy back; the vectors it re-embeds are its own.

Then Rule 1 with the `t4-` prefix, eight `identical` expected (`dump` and `bench` never call `sync`).

- [ ] **Step 7: Commit**

`git -C "$W" add src/index/dense.rs src/ask.rs src/main.rs src/mcp.rs` — subject `feat(mcp): vectors embedded in bounded slices, so a tool call answers in seconds after a branch switch`, body with the Rule 4 reading (`$A/t4-rule4.txt`), the 80 ms and 3.1 ms per-row rates and where they come from, and the Rule 1 verdicts.

---

### Task 5: The plugin, the Codex path, and the README's "For agents"

**Files:**
- Create: `plugin/.claude-plugin/plugin.json`, `plugin/.mcp.json`, `plugin/bin/repograph-mcp`, `plugin/skills/repograph/SKILL.md`, `plugin/commands/setup.md`, `plugin/hooks/hooks.json`, `plugin/hooks/notice.py`, `plugin/hooks/test_notice.py`, `.claude-plugin/marketplace.json` (repository root), `plugin/README.md`
- Modify: `README.md` (Status table: `mcp`, `status` rows; the new section "For agents"), `.gitignore` (nothing new unless `plugin/` needs an ignore, which it does not)
- Create (scratch): `$A/t5-*.txt`

**Interfaces:**
- Consumes: `repograph mcp`, `repograph status --json`, the CLI.
- Produces: a plugin installable with `claude plugin marketplace add devmaxxx/repograph` then `claude plugin install repograph@repograph`, or loaded for one session with `claude --plugin-dir <repo>/plugin`; a Codex configuration block; a hook that prints ≤ 400 bytes once per session and ≤ 120 bytes every fortieth `Read|Grep|Glob`.
- The plugin holds no logic: every decision an agent could take is in the binary's answers.

- [ ] **Step 1: The manifest, the marketplace, the MCP declaration, the shim**

`plugin/.claude-plugin/plugin.json`:

```json
{
  "name": "repograph",
  "version": "0.5.0",
  "description": "Ask a repository's requirement docs and TypeScript as a graph, at zero model tokens: an MCP server with ask, explain, impact, trace and changes; a routing skill; a setup command; a one-line reminder hook",
  "author": { "name": "Max Synenko" },
  "homepage": "https://github.com/devmaxxx/repograph",
  "repository": "https://github.com/devmaxxx/repograph",
  "license": "MIT",
  "keywords": ["knowledge-graph", "requirements", "typescript", "mcp", "blast-radius"]
}
```

The version tracks `Cargo.toml`; a release bump PR moves both, as PR #7 moved the crate and the npm packages.

`.claude-plugin/marketplace.json` at the repository root:

```json
{
  "name": "repograph",
  "owner": { "name": "Max Synenko" },
  "plugins": [
    { "name": "repograph", "source": "./plugin", "description": "repograph's MCP server, skill, setup command and reminder hook", "version": "0.5.0" }
  ]
}
```

`plugin/.mcp.json`:

```json
{
  "mcpServers": {
    "repograph": { "command": "${CLAUDE_PLUGIN_ROOT}/bin/repograph-mcp", "args": [] }
  }
}
```

`plugin/bin/repograph-mcp`, `chmod +x`:

```bash
#!/bin/bash
# Finds the repograph binary for the harness that starts this MCP server: REPOGRAPH_BIN, then
# PATH, then the project's own node_modules — what `pnpm exec repograph` would find — so a
# repository that took the npm package needs nothing on PATH. `exec`, not a child: the harness
# owns this process and must see the server exit when it closes stdin. bash 3.2, which is
# what macOS ships.
set -u
if [ -n "${REPOGRAPH_BIN:-}" ] && [ -x "$REPOGRAPH_BIN" ]; then exec "$REPOGRAPH_BIN" mcp "$@"; fi
if command -v repograph >/dev/null 2>&1; then exec repograph mcp "$@"; fi
dir=$PWD
while [ "$dir" != "/" ] && [ -n "$dir" ]; do
  if [ -x "$dir/node_modules/.bin/repograph" ]; then exec "$dir/node_modules/.bin/repograph" mcp "$@"; fi
  dir=$(dirname "$dir")
done
echo "repograph-mcp: no repograph binary — set REPOGRAPH_BIN, put repograph on PATH (cargo install --path . / npm i -g @devmaxxx/repograph), or add @devmaxxx/repograph to this project" >&2
exit 127
```

Check: `REPOGRAPH_BIN="$B" "$W/plugin/bin/repograph-mcp" < /dev/null; echo "exit $?"` from inside `$C` — expected the `mcp: … exits when stdin closes` line on stderr and `exit 0`; `PATH=/usr/bin:/bin "$W/plugin/bin/repograph-mcp" < /dev/null; echo "exit $?"` from `/tmp` — expected the one-line message and `exit 127`.

- [ ] **Step 2: The hook and its tests**

`plugin/hooks/hooks.json`:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Read|Grep|Glob",
        "hooks": [
          { "type": "command", "command": "python3 \"${CLAUDE_PLUGIN_ROOT}/hooks/notice.py\"", "timeout": 5 }
        ]
      }
    ]
  }
}
```

`plugin/hooks/test_notice.py` first:

```python
#!/usr/bin/env python3
"""The reminder hook: once in full, then a line every fortieth call, never for the store's
own files, nothing at all outside a repository that has a store."""
import json
import os
import tempfile
import unittest

import notice


class Notice(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = os.path.join(self.tmp.name, "repo")
        os.makedirs(os.path.join(self.repo, ".repograph"))
        os.makedirs(os.path.join(self.repo, "apps", "api"))
        self.state = os.path.join(self.tmp.name, "state")
        os.makedirs(self.state)

    def tearDown(self):
        self.tmp.cleanup()

    def payload(self, tool="Read", path=None, cwd=None, session="s1"):
        return {"session_id": session, "cwd": cwd or self.repo, "tool_name": tool,
                "tool_input": {"file_path": path or os.path.join(self.repo, "apps/api/x.ts")}}

    def test_the_first_matching_call_of_a_session_gets_the_full_notice_and_the_next_thirty_nine_nothing(self):
        out = notice.respond(self.payload(), self.state)
        self.assertIn("repograph", out["hookSpecificOutput"]["additionalContext"])
        self.assertLessEqual(len(out["hookSpecificOutput"]["additionalContext"].encode()), 400)
        for _ in range(38):
            self.assertIsNone(notice.respond(self.payload(), self.state))
        self.assertIsNone(notice.respond(self.payload(), self.state))
        reminder = notice.respond(self.payload(), self.state)
        self.assertIsNotNone(reminder, "the fortieth call after the notice is reminded")
        self.assertLessEqual(len(reminder["hookSpecificOutput"]["additionalContext"].encode()), 120)

    def test_a_read_of_the_store_itself_or_of_a_file_the_graph_does_not_index_is_ignored(self):
        self.assertIsNone(notice.respond(self.payload(path=os.path.join(self.repo, ".repograph/graph.json")), self.state))
        self.assertIsNone(notice.respond(self.payload(path=os.path.join(self.repo, "logo.png")), self.state))
        self.assertIsNotNone(notice.respond(self.payload(path=os.path.join(self.repo, "docs/a.md")), self.state))

    def test_a_grep_counts_by_its_path_and_a_glob_by_its_pattern(self):
        p = self.payload(tool="Grep"); p["tool_input"] = {"pattern": "x", "path": os.path.join(self.repo, "apps")}
        self.assertIsNotNone(notice.respond(p, self.state))
        p = self.payload(tool="Glob", session="s2"); p["tool_input"] = {"pattern": "**/*.ts"}
        self.assertIsNotNone(notice.respond(p, self.state))

    def test_outside_a_repository_with_a_store_the_hook_says_nothing_but_names_the_build_once_in_a_git_checkout(self):
        bare = os.path.join(self.tmp.name, "bare"); os.makedirs(os.path.join(bare, "src"))
        self.assertIsNone(notice.respond(self.payload(cwd=bare, path=os.path.join(bare, "src/a.ts")), self.state))
        os.makedirs(os.path.join(bare, ".git"))
        out = notice.respond(self.payload(cwd=bare, path=os.path.join(bare, "src/a.ts"), session="s3"), self.state)
        self.assertIn("repograph build", out["hookSpecificOutput"]["additionalContext"])
        self.assertIsNone(notice.respond(self.payload(cwd=bare, path=os.path.join(bare, "src/a.ts"), session="s3"), self.state), "said once")

    def test_a_malformed_payload_is_silence_not_an_error(self):
        self.assertIsNone(notice.respond({}, self.state))
        self.assertIsNone(notice.respond({"tool_input": "not a dict"}, self.state))


if __name__ == "__main__":
    unittest.main()
```

Run: `cd "$W/plugin/hooks" && python3 -m unittest test_notice 2>&1 | tail -3` — expected `ModuleNotFoundError: notice`.

`plugin/hooks/notice.py`:

```python
#!/usr/bin/env python3
"""A PreToolUse reminder: the moment an agent first reaches for a source or planning file is
the moment it is about to answer a question the graph answers in a fifth of the tokens. Full
notice once per session, one line every fortieth matching call so a compacted context is
re-primed, nothing for the store's own files, nothing outside a repository that has a store.
Every path exits 0 and prints nothing on doubt: a hook must never block a tool call.

Stdlib only, since a plugin cannot ask for dependencies; bash 3.2 cannot parse JSON, which
is why this is Python.
"""
import json
import os
import sys
import tempfile

REMINDER_EVERY = 40
INDEXED = {".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".md", ".mdx", ".yaml", ".yml", ".json", ".toml"}

FULL = ("This repository has a repograph graph (.repograph/). Before reading or grepping for a requirement, "
        "an id, a symbol, its callers or what a diff touches, ask it: the repograph MCP tools (ask, explain, "
        "impact, trace, changes) or `repograph ask <words>` in the shell — ~200 tokens an answer, refreshed "
        "from the tree on every call. Skill: repograph.")
BUILD = ("This repository is a git checkout with no repograph store. `repograph build --no-dense` (about "
         "1 s per 800 files) makes `repograph ask` and the repograph MCP tools answer; /repograph:setup walks through it.")
REMIND = "Reminder: ask the repograph graph before grepping (MCP tools or `repograph ask`)."


def root_of(start):
    """The nearest ancestor with a store, else with a `.git` entry (a worktree's is a file), else None."""
    d = os.path.abspath(start)
    while True:
        if os.path.isdir(os.path.join(d, ".repograph")):
            return d, True
        if os.path.exists(os.path.join(d, ".git")):
            return d, False
        parent = os.path.dirname(d)
        if parent == d:
            return None, False
        d = parent


def target_of(payload):
    tool_input = payload.get("tool_input")
    if not isinstance(tool_input, dict):
        return None
    return tool_input.get("file_path") or tool_input.get("path") or tool_input.get("pattern")


def worth_it(target):
    if not target or "/.repograph/" in target or target.endswith("/.repograph"):
        return False
    ext = os.path.splitext(target)[1]
    # A glob pattern names an extension at its tail the same way a path does.
    return ext in INDEXED or ext == ""


def context(text):
    return {"hookSpecificOutput": {"hookEventName": "PreToolUse", "additionalContext": text}}


def respond(payload, state_dir):
    """The hook's answer for one payload, or None for silence. `state_dir` holds one counter
    file per session, so the tests never touch the real temp directory."""
    if not isinstance(payload, dict):
        return None
    session = str(payload.get("session_id") or "")
    target = target_of(payload)
    if not session or not worth_it(target):
        return None
    root, has_store = root_of(payload.get("cwd") or os.getcwd())
    if root is None:
        return None
    counter = os.path.join(state_dir, "repograph-notice-" + "".join(c for c in session if c.isalnum() or c in "-_"))
    try:
        n = int(open(counter).read().strip() or "0")
    except (OSError, ValueError):
        n = 0
    try:
        with open(counter, "w") as f:
            f.write(str(n + 1))
    except OSError:
        return None
    if not has_store:
        return context(BUILD) if n == 0 else None
    if n == 0:
        return context(FULL)
    if n % REMINDER_EVERY == 0:
        return context(REMIND)
    return None


def main():
    try:
        raw = sys.stdin.read()
        payload = json.loads(raw) if raw.strip() else {}
    except (OSError, ValueError):
        return
    out = respond(payload, tempfile.gettempdir())
    if out is not None:
        sys.stdout.write(json.dumps(out))


if __name__ == "__main__":
    main()
```

Run: `cd "$W/plugin/hooks" && python3 -m unittest test_notice 2>&1 | tail -3` — expected `OK`, 5 tests. Then the hook against a real payload: `echo '{"session_id":"x","cwd":"'"$C"'","tool_name":"Read","tool_input":{"file_path":"'"$C"'/apps/api/src/main.ts"}}' | python3 "$W/plugin/hooks/notice.py"; echo " exit $?"` — expected the JSON with the full notice and `exit 0`; a second run prints nothing.

- [ ] **Step 3: The skill**

`plugin/skills/repograph/SKILL.md`:

```markdown
---
name: repograph
description: Use when answering a question about this repository before opening a file — which requirement owns a concept, what an id means, where a symbol lives, who calls it, what a diff touches, how one symbol reaches another. Routes the question to the repograph graph, rg or ast-grep by what it costs, and says what the graph cannot prove.
---

# repograph — ask the repository before reading it

The store in `.repograph/` holds every requirement, ADR, invariant, milestone task, entity and
TypeScript symbol as a node, and every quoted id, import, call, decorator and markdown link as an
edge. It is built and refreshed at zero model tokens, and every call refreshes it against the
tree first, so it is never behind the working copy. Reading a file to find something costs
tokens every time; an answer here is about 200.

The tools are the `repograph` MCP server (`ask`, `explain`, `impact`, `trace`, `changes`) or the
same five commands in the shell (`repograph ask …`; `pnpm exec repograph …` where the npm package
is installed). Same bytes either way.

## The loop

1. **Anchor if you have one.** An id (`FR-CAL-40`, `BE-M17`, `INV-07`, `ADR-021`) or a symbol
   name (`CancellationPolicy`) answers exactly, in ~50 ms, no model.
2. **Otherwise ask in words**, in whatever language the documents use. Five lines of
   `ID  path:line  headline` and one neighbour.
3. **Widen before you doubt it**: `seeds: 8` for more, `bodies: true` for the full requirement
   text instead of opening the file, `explain` for one node and everything attached to it.
4. **Read only the `path:line` the graph printed.** Never open a documents folder to search it.

## Route the question

| Question | Tool | Cost |
| --- | --- | --- |
| Which requirement defines this concept? What does the PRD call it? | `ask` | ~200 tok |
| What is `FR-PAY-22`, what quotes it, what does it quote? | `ask FR-PAY-22`, then `explain` | ~200 tok |
| Where is this symbol declared, what extends or imports it? | `ask <Symbol>` | ~120 tok |
| Who calls it? What breaks if it changes? | `impact <Symbol>`; `down: true` for what it calls | ~200 tok; hubs are capped at 25 lines a layer, the counts and the risk line are whole |
| How does A reach B? | `trace A B` | ~100 tok |
| What does my diff touch, and who reaches that? | `changes`; `base: main` for the whole branch | ~200–600 tok, capped the same way |
| Every occurrence of a literal string | `rg -l` first, `rg -n` on the shortlist | 10–500 tok |
| A syntactic shape, not a string | `ast-grep run -p '<pattern>'` | ~500 tok |

## Before editing a symbol, and before committing

- **Before editing a function, class or method:** `impact <symbol>`, and say what it printed —
  the direct callers (`d=1`), the file count, the risk label. `HIGH` and `CRITICAL` are said
  before the edit, not after. The thresholds are printed with the counts that produced them:
  `MEDIUM` from 5 direct callers or 3 files, `HIGH` from 15 or 10, `CRITICAL` from 30 or 25.
- **Before committing:** `changes`. The symbols and callers it lists must be the ones you meant
  to touch. Before a pull request, `changes` with `base: main`.
- **What the graph cannot prove it does not list.** A call through a chained expression, a
  destructured method, a callback parameter or a global has no edge. Confirm a "nothing uses
  this" with `rg -l` before deleting.

## What the answers say about themselves

- The first answer of a session, and the server's instructions, carry the store line: node and
  edge counts, `fresh` or `behind`, `questions <covered>/<eligible>`, the vectors' model or
  `no vectors`. A store with `questions 0/…` was never enriched: keyword and symbol questions
  are unaffected, paraphrases find fewer targets (measured: 9/30 against 15/30 on the bench
  corpus with the small model). A store with `no vectors` answers lexically only.
- `refresh: N changed, M removed` as the first line means the answer is from a graph just
  brought in line with your edits. `dense: N rows behind, embedded between calls` means a large
  edit's vectors are still being written; the lexical graph is already current.
- `no match` is an answer. Try fewer, more specific words, or an anchor.

## What is not a tool, and why

`repograph ask --rerank` spends ~19,000 model tokens a question and `--rerank-local` 18 s; both
are shell-only and for a person who has seen two misses in a row. `build`, `update`, `enrich`,
`embed`, `serve`, `watch` are writers a person runs; no tool builds a store. If a tool answers
`no store at …`, say so and stop — `/repograph:setup` is the person's next step.

## Pass this on

Any subagent sent into this repository gets the same rule in its prompt: ask the graph before
reading files, and read only the `path:line` it printed.
```

- [ ] **Step 4: The setup command**

`plugin/commands/setup.md`:

```markdown
---
description: Build or check this repository's repograph store — the binary, the store, the model, the ignore line — with the person deciding what it costs
allowed-tools: Bash(repograph *), Bash(pnpm exec repograph *), Bash(npx repograph *), Bash(cargo install *), Bash(git *), Read, Edit
---

# /repograph:setup

Walk the person through making `repograph ask` and the repograph MCP tools answer in this
repository. Every step that spends minutes or disk is theirs to confirm; none is run unasked.

## Steps

1. **The binary.** Run `repograph --version`; if it fails, try `pnpm exec repograph --version`
   and `npx repograph --version`. If none answers, say how to get one — `cargo install --path .`
   from a checkout of `devmaxxx/repograph`, or `pnpm add -D @devmaxxx/repograph` with the
   GitHub Packages line in `~/.npmrc` the README describes — and stop.
2. **The store.** Run `repograph status --json`. If `"store": true`, print the status line,
   say whether it is `fresh` or `behind`, and go to step 5.
3. **Build, lexical first.** Tell the person `repograph build --no-dense` takes about a second
   per 800 files and answers keywords, ids and symbols at once; ask, then run it. Show the
   `changed … nodes … edges …` line.
4. **The vectors, their choice.** Explain the two models with the README's numbers: the default
   `intfloat/multilingual-e5-large` reads paraphrase 22/30 on the bench corpus at ~2,680 s to
   embed 33,525 rows and a 2.1 GB download; `intfloat/multilingual-e5-small` reads 15/30 at
   ~103 s and 470 MB. If they choose the small one, write `embed_model =
   "intfloat/multilingual-e5-small"` into `repograph.toml`. Then, only if they say so, run
   `repograph embed` and report the `dense: embedded … rows in …` line. If they decline, say
   the store answers lexically until they do.
5. **The ignore line.** If `.gitignore` does not contain `.repograph/`, offer to add it.
6. **Enrichment, mentioned once.** Say that `repograph enrich` writes reader questions beside
   every requirement through the configured model command (~$2.5 of a small model on the
   bench corpus, once) and is what lifts paraphrase recall; do not run it.
7. **Done.** Print `repograph status` and one sentence: the MCP tools and `repograph ask` now
   answer here.
```

- [ ] **Step 5: The plugin README and the repository README**

`plugin/README.md` — twenty lines: what the plugin contains (the four files), how to install it (marketplace or `--plugin-dir`), where the binary comes from (the shim's three places), and that the plugin holds no logic.

`README.md`:

- **Status table**: after the `serve` row, `| \`mcp\` | working; the five readers over stdio for a coding agent's harness — capped for hub symbols, notices ahead of the answer, refreshed per call, no store built — see [For agents](#for-agents) |` and `| \`status\` | working; the store in one line, \`--json\` for a hook or a harness |`.
- **Use**: after the `changes` examples, three lines: `repograph explain --json FR-PAY-22`, `repograph trace --json A B`, `repograph status --json`.
- **A new section "For agents"** before "Configure", in this order: (1) two sentences on what the surface is and why (the spec's decision paragraph, shortened); (2) *Claude Code*: `claude plugin marketplace add devmaxxx/repograph`, `claude plugin install repograph@repograph`, or `claude --plugin-dir path/to/repograph/plugin`, and the plain `.mcp.json` block for a project without the plugin; (3) *Codex*: the `[mcp_servers.repograph]` block with `command = "repograph"` and `args = ["mcp"]`, or `codex mcp add repograph -- repograph mcp`, and the `AGENTS.md` paragraph (the skill's "Before editing a symbol" list condensed to five lines); (4) *Any shell*: the five commands with `--json`, `status --json`, exit codes (0 answered — an empty `ask` included; 1 an error named on stderr; 2 a usage error); (5) *What a tool costs before a call*: the two ceilings as bytes and as tokens, and the sentence that Task 7 fills with the measured first-request cost; (6) *What the tools refuse*: no store, `--rerank`, the writers; (7) *Where the caps are*: `impact` and `changes` at 25 lines a list, `explain` at 40 edges, `limit: 0` for the whole, with the yardstick numbers from `$A/yardstick.txt`; (8) *Concurrency*: one process per session, the root walk, a `serve` used when present, `--idle-model`. Every number cites its file.

- [ ] **Step 6: Load it, both harnesses**

```bash
cd "$C" && REPOGRAPH_BIN="$B" claude -p --output-format json --setting-sources "" --strict-mcp-config --plugin-dir "$W/plugin" --allowedTools "mcp__plugin_repograph_repograph,mcp__repograph,Skill" --max-turns 6 \
  "Which requirement id covers cancellation fees (штраф за отмену)? Use the repograph tools; reply with the id and path:line." \
  | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["result"]); print("turns", d["num_turns"], "usage", d["usage"])' | tee "$A/t5-claude-plugin.txt"
cd "$C" && REPOGRAPH_BIN="$B" claude -p --output-format json --setting-sources "" --strict-mcp-config --plugin-dir "$W/plugin" --allowedTools "Read" --max-turns 3 \
  "Read apps/api/src/main.ts and reply with its first line." | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["result"][:200])' | tee "$A/t5-hook.txt"
```

Expected: the first answer names an `FR-PAY-*` id with a path; if the MCP tool name under a plugin differs from both allowed forms, read the transcript with `--output-format stream-json --verbose` once, record the name in `$A/t5-claude-plugin.txt`, and put it into the README's Claude Code paragraph. The second run exercises the hook: with `--verbose` and `stream-json`, the first `Read` is preceded by the notice text in the transcript; record whether it appeared. A hook that did not fire under `--plugin-dir` is recorded, not worked around — it is what the results document reports under "harness facts".

Codex, with the shim:

```bash
codex mcp add repograph -- env REPOGRAPH_BIN="$B" "$W/plugin/bin/repograph-mcp" && cd "$C" && codex exec --skip-git-repo-check "Use the repograph 'impact' tool on StaffService and reply with its risk line." 2>&1 | tail -3 | tee "$A/t5-codex.txt"; codex mcp remove repograph
```

Expected: a `risk: MEDIUM — 3 direct, 3 total, 3 files` line in the tail.

- [ ] **Step 7: Commit**

`git -C "$W" add plugin .claude-plugin README.md` — subject `feat(plugin): a Claude Code plugin around repograph mcp, and the Codex path`, body with the hook test count, the two harness transcripts and the README anchors added.

---

### Task 6: The agent bench — tasks, the runner, the report

**Files:**
- Create: `bench/agent/tasks.py`, `bench/agent/run.py`, `bench/agent/test_agent.py`, `bench/agent/tasks.jsonl` (generated, committed), `bench/agent/README.md`
- Create (scratch): `$A/t6-*.txt`, `$A/plugin-cli/`

**Interfaces:**
- Consumes: `bench/cases.jsonl`, `bench/blast.jsonl`, `bench/compare/truth.py` (`files_naming`, `declaration_of`), the plugin, `claude -p --output-format stream-json`.
- Produces: `tasks.jsonl` rows `{"id", "kind", "prompt", "expect": {...}}`; `run.py run` rows appended to a JSONL, one per (arm, task, run): `{"when", "arm", "task", "kind", "ok", "tokens": {"input", "cache_creation", "cache_read", "output", "total"}, "first_input", "cost_usd", "turns", "seconds", "model", "answer"}`; `run.py report` — per-arm totals and the paired sign tests, in the words of Rule 5.
- The runner's agent command is a template; the parser is `claude`'s and says so. Another harness gets a parser of its own before it gets a number.

- [ ] **Step 1: The tests**

`bench/agent/test_agent.py`:

```python
#!/usr/bin/env python3
"""The pieces of the agent bench that can be checked without spending a token: the task
shapes, the grader, the usage parser, and the sign test."""
import json
import math
import unittest

import run as r
import tasks as t


class Tasks(unittest.TestCase):
    def test_the_suite_is_twenty_four_tasks_in_the_pre_registered_shape(self):
        rows = t.build(t.CASES, t.BLAST)
        kinds = [x["kind"] for x in rows]
        self.assertEqual((kinds.count("keyword"), kinds.count("code"), kinds.count("impact"), kinds.count("trace")), (8, 6, 6, 4))
        self.assertEqual(len({x["id"] for x in rows}), 24)
        for x in rows:
            self.assertTrue(x["prompt"].strip())
            self.assertIn("Reply", x["prompt"])

    def test_building_twice_is_the_same_file(self):
        self.assertEqual(t.build(t.CASES, t.BLAST), t.build(t.CASES, t.BLAST))


class Grading(unittest.TestCase):
    def test_a_keyword_task_passes_on_the_id_and_a_code_task_on_the_path(self):
        self.assertTrue(r.graded({"kind": "keyword", "expect": {"id": "FR-PAY-22"}}, "It is FR-PAY-22 at docs/x.md:12.", None))
        self.assertFalse(r.graded({"kind": "keyword", "expect": {"id": "FR-PAY-22"}}, "FR-PAY-2 and FR-PAY-220.", None))
        self.assertTrue(r.graded({"kind": "code", "expect": {"file": "packages/contracts/src/money.ts"}}, "packages/contracts/src/money.ts", None))

    def test_an_impact_task_passes_on_the_declaring_file_and_one_true_caller_and_fails_on_an_invented_one(self):
        task = {"kind": "impact", "expect": {"file": "s/x.service.ts", "naming": ["s/x.service.ts", "s/x.controller.ts", "s/x.module.ts"]}}
        self.assertTrue(r.graded(task, "declared in s/x.service.ts; called from s/x.controller.ts; risk MEDIUM", None))
        self.assertFalse(r.graded(task, "declared in s/x.service.ts; risk LOW", None), "no caller named")
        self.assertFalse(r.graded(task, "s/x.service.ts is called from s/y.service.ts and s/x.controller.ts", None), "an invented caller fails the task")

    def test_a_trace_task_passes_when_every_via_symbol_is_named(self):
        task = {"kind": "trace", "expect": {"via": ["AuthService", "IdentityRepository"]}}
        self.assertTrue(r.graded(task, "AuthController → AuthService → IdentityRepository → DatabaseService", None))
        self.assertFalse(r.graded(task, "AuthController → DatabaseService directly", None))


class Usage(unittest.TestCase):
    def test_the_stream_is_read_for_the_result_usage_the_first_request_and_the_model(self):
        stream = "\n".join(json.dumps(x) for x in [
            {"type": "system", "subtype": "init", "model": "claude-x"},
            {"type": "assistant", "message": {"model": "claude-x", "usage": {"input_tokens": 10, "cache_creation_input_tokens": 9000, "cache_read_input_tokens": 0, "output_tokens": 50}}},
            {"type": "assistant", "message": {"model": "claude-x", "usage": {"input_tokens": 20, "cache_creation_input_tokens": 100, "cache_read_input_tokens": 9000, "output_tokens": 60}}},
            {"type": "result", "subtype": "success", "num_turns": 2, "total_cost_usd": 0.05, "result": "FR-PAY-22",
             "usage": {"input_tokens": 30, "cache_creation_input_tokens": 9100, "cache_read_input_tokens": 9000, "output_tokens": 110}},
        ])
        u = r.usage_of(stream)
        self.assertEqual(u["tokens"]["total"], 30 + 9100 + 9000 + 110)
        self.assertEqual(u["first_input"], 10 + 9000)
        self.assertEqual((u["turns"], u["cost_usd"], u["model"], u["answer"]), (2, 0.05, "claude-x", "FR-PAY-22"))

    def test_a_stream_without_a_result_is_a_failed_run_not_a_zero(self):
        self.assertIsNone(r.usage_of('{"type":"system","subtype":"init"}\n'))


class SignTest(unittest.TestCase):
    def test_the_exact_two_sided_binomial_matches_the_pre_registered_bar(self):
        self.assertAlmostEqual(r.sign_p(18, 24), 0.0227, places=3)
        self.assertAlmostEqual(r.sign_p(12, 24), 1.0, places=3)
        self.assertAlmostEqual(r.sign_p(36, 48), 0.0007, places=3)


if __name__ == "__main__":
    unittest.main()
```

Run: `cd "$W/bench/agent" && python3 -m unittest test_agent 2>&1 | tail -3` — expected `ModuleNotFoundError`.

- [ ] **Step 2: The tasks**

`bench/agent/tasks.py`:

```python
#!/usr/bin/env python3
"""The 24 agent tasks, built deterministically from the recorded cases so that nobody writes a
task with the answer in mind: the first eight keyword cases, the first six code cases, the first
six narrow-tier impact targets and the first four trace pairs that have a path, in file order.

    python3 bench/agent/tasks.py build --repo <corpus checkout> --out bench/agent/tasks.jsonl

Truth is the case's own anchor, or what `truth.py` reads out of the checkout with rg — never a
repograph answer.
"""
import argparse
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
BENCH = HERE.parent
CASES = BENCH / "cases.jsonl"
BLAST = BENCH / "blast.jsonl"
sys.path.insert(0, str(BENCH / "compare"))

KEYWORD = 8
CODE = 6
IMPACT = 6
TRACE = 4


def rows(path):
    return [json.loads(l) for l in Path(path).read_text(encoding="utf-8").splitlines() if l.strip()]


def build(cases_path, blast_path, repo=None):
    """`repo` is needed only for the impact tasks' rg-derived naming lists; without it the
    lists are left empty, which is what the unit tests build."""
    cases, blast = rows(cases_path), rows(blast_path)
    out = []
    for c in [c for c in cases if c["kind"] == "keyword"][:KEYWORD]:
        out.append({"id": f"k{len(out):02d}", "kind": "keyword",
                    "prompt": f"In this repository's documents, which requirement says this: «{c['q']}»? Reply with the requirement id and its path:line, nothing else.",
                    "expect": {"id": c["expect"]}})
    for c in [c for c in cases if c["kind"] == "code"][:CODE]:
        out.append({"id": f"c{len(out):02d}", "kind": "code",
                    "prompt": f"Where in this repository is this declared: «{c['q']}»? Reply with the file path relative to the repository root, nothing else.",
                    "expect": {"file": c["expect"]}})
    for b in [b for b in blast if b["kind"] == "impact" and b["tier"] == "narrow"][:IMPACT]:
        naming = []
        if repo is not None:
            import truth as T
            naming = sorted(T.files_naming(Path(repo), b["target"]))
        out.append({"id": f"i{len(out):02d}", "kind": "impact",
                    "prompt": f"List the direct callers of `{b['target']}` in this repository — the file of each caller — and say whether changing it is LOW, MEDIUM or HIGH risk. Reply with the declaring file, the caller files, and the risk word.",
                    "expect": {"file": b["file"], "naming": naming}})
    for b in [b for b in blast if b["kind"] == "trace" and b.get("expect") == "path"][:TRACE]:
        out.append({"id": f"t{len(out):02d}", "kind": "trace",
                    "prompt": f"How does `{b['from']}` reach `{b['to']}` through calls in this repository? Reply with the chain of symbol names from the first to the last.",
                    "expect": {"via": b["via"]}})
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("build")
    b.add_argument("--repo", required=True)
    b.add_argument("--out", required=True)
    a = ap.parse_args()
    tasks = build(CASES, BLAST, a.repo)
    Path(a.out).write_text("".join(json.dumps(t, ensure_ascii=False) + "\n" for t in tasks), encoding="utf-8")
    kinds = [t["kind"] for t in tasks]
    print(f"{len(tasks)} tasks: " + ", ".join(f"{k} {kinds.count(k)}" for k in ("keyword", "code", "impact", "trace")))


if __name__ == "__main__":
    main()
```

- [ ] **Step 3: The runner and the report**

`bench/agent/run.py`:

```python
#!/usr/bin/env python3
"""Runs the agent tasks through a headless harness in three arms and reads what each cost.

    A  bare: Read, Grep, Glob and rg — what an agent has without repograph
    B  the plugin without its MCP declaration: the skill and the hook, repograph through Bash
    C  the plugin with the MCP server

    python3 bench/agent/run.py run --copy <corpus copy> --bin <repograph> --plugin <repo>/plugin \
        --tasks bench/agent/tasks.jsonl --arms A,B,C --out bench/agent/runs.jsonl --note "…"
    python3 bench/agent/run.py report bench/agent/runs.jsonl [--when <prefix>]

The agent command is a template with `{prompt_file}`; the usage parser reads `claude`'s
`stream-json` and nothing else. The rule the report is read against is the spec's Rule 5, and
the report prints its clauses in order; it decides nothing.
"""
import argparse
import json
import math
import os
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path

# The prompt goes in on stdin: the task texts carry backticks, which a shell inside double
# quotes would run as commands.
DEFAULT_COMMAND = ('claude -p --output-format stream-json --verbose --max-turns 30 --setting-sources "" '
                   '--strict-mcp-config --disallowedTools "Edit,Write,MultiEdit,NotebookEdit,Task,WebFetch,WebSearch" '
                   '{arm_flags} --allowedTools "{allowed}" < {prompt_file}')
BARE_TOOLS = ["Read", "Grep", "Glob", "Bash(rg *)", "Bash(cat *)", "Bash(ls *)", "Bash(git log *)", "Bash(git show *)"]


def arm_setup(arm, plugin, binary, scratch):
    """The flags and the allowed tools for one arm. B and C load the same plugin directory,
    copied without `.mcp.json`; C adds the server through `--mcp-config`, so a tool in C is
    the server and not the plugin's own declaration — one registration, one name."""
    allowed = list(BARE_TOOLS)
    flags = []
    if arm in ("B", "C"):
        cli = Path(scratch) / "plugin-cli"
        if not cli.exists():
            shutil.copytree(plugin, cli, ignore=shutil.ignore_patterns(".mcp.json"))
        flags += ["--plugin-dir", str(cli)]
        allowed += ["Bash(repograph *)", "Skill"]
    if arm == "C":
        mcp = Path(scratch) / "mcp.json"
        mcp.write_text(json.dumps({"mcpServers": {"repograph": {"command": binary, "args": ["mcp"]}}}))
        flags += ["--mcp-config", str(mcp)]
        allowed += ["mcp__repograph"]
    return " ".join(flags), ",".join(allowed)


def usage_of(stream):
    """The result message's usage (the sum over the run), the first request's input tokens
    (the surface's real cost in the harness's own rendering), the model and the answer."""
    result, first, model = None, None, None
    for line in stream.splitlines():
        try:
            m = json.loads(line)
        except ValueError:
            continue
        if m.get("type") == "assistant" and first is None:
            u = m.get("message", {}).get("usage", {})
            first = u.get("input_tokens", 0) + u.get("cache_creation_input_tokens", 0) + u.get("cache_read_input_tokens", 0)
            model = m.get("message", {}).get("model")
        if m.get("type") == "result":
            result = m
    if result is None:
        return None
    u = result.get("usage", {})
    tokens = {"input": u.get("input_tokens", 0), "cache_creation": u.get("cache_creation_input_tokens", 0),
              "cache_read": u.get("cache_read_input_tokens", 0), "output": u.get("output_tokens", 0)}
    tokens["total"] = sum(tokens.values())
    return {"tokens": tokens, "first_input": first or 0, "turns": result.get("num_turns"), "cost_usd": result.get("total_cost_usd"),
            "model": model or result.get("model"), "answer": result.get("result", "")}


def whole_word(needle, text):
    return re.search(r"(?<![\w-])" + re.escape(needle) + r"(?![\w-])", text) is not None


def graded(task, answer, repo):
    """Whether the final answer reaches the task's truth. `repo` is unused today and kept so a
    grader that has to read the checkout has somewhere to do it."""
    e, k = task["expect"], task["kind"]
    if k == "keyword":
        return whole_word(e["id"], answer)
    if k == "code":
        return e["file"] in answer
    if k == "impact":
        if e["file"] not in answer:
            return False
        named = [f for f in re.findall(r"[\w./-]+\.(?:ts|tsx|kt)\b", answer) if f != e["file"]]
        callers = [f for f in named if any(n.endswith(f) or f.endswith(n) for n in e["naming"])]
        invented = [f for f in named if not any(n.endswith(f) or f.endswith(n) for n in e["naming"])]
        return bool(callers) and not invented
    if k == "trace":
        return all(whole_word(v, answer) for v in e["via"])
    raise SystemExit(f"unknown kind {k}")


def sign_p(wins, n):
    """Exact two-sided binomial at p = 1/2: the pre-registered bar is 18 of 24 (p ≈ 0.023)."""
    k = min(wins, n - wins)
    tail = sum(math.comb(n, i) for i in range(0, k + 1)) / 2 ** n
    return min(1.0, 2 * tail)


def cmd_run(a):
    tasks = [json.loads(l) for l in Path(a.tasks).read_text(encoding="utf-8").splitlines() if l.strip()]
    scratch = Path(a.scratch or tempfile.mkdtemp(prefix="repograph-agent-"))
    scratch.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, PATH=str(Path(a.bin).parent) + os.pathsep + os.environ.get("PATH", ""), REPOGRAPH_BIN=a.bin)
    with open(a.out, "a", encoding="utf-8") as out:
        for arm in a.arms.split(","):
            arm_flags, allowed = arm_setup(arm, a.plugin, a.bin, scratch)
            for task in tasks:
                prompt_file = scratch / f"{arm}-{task['id']}.prompt"
                prompt_file.write_text(task["prompt"], encoding="utf-8")
                cmd = a.agent_command.format(arm_flags=arm_flags, allowed=allowed, prompt_file=prompt_file)
                t = time.time()
                p = subprocess.run(cmd, shell=True, cwd=a.copy, env=env, capture_output=True, text=True, timeout=a.timeout)
                seconds = time.time() - t
                (scratch / f"{arm}-{task['id']}.stream").write_text(p.stdout, encoding="utf-8")
                u = usage_of(p.stdout)
                row = {"when": a.when, "arm": arm, "task": task["id"], "kind": task["kind"], "note": a.note, "seconds": round(seconds, 1),
                       "ok": None if u is None else graded(task, u["answer"], a.copy), "failed": u is None}
                if u is not None:
                    row.update({"tokens": u["tokens"], "first_input": u["first_input"], "turns": u["turns"], "cost_usd": u["cost_usd"], "model": u["model"], "answer": u["answer"][:400]})
                else:
                    row["stderr"] = p.stderr[-800:]
                out.write(json.dumps(row, ensure_ascii=False) + "\n")
                out.flush()
                print(f"{arm} {task['id']} {task['kind']:8} {'ok ' if row['ok'] else 'MISS' if row['ok'] is False else 'FAIL'} "
                      f"{(row.get('tokens') or {}).get('total', 0):>8} tok  {row.get('turns') or 0:>2} turns  {seconds:5.1f}s")


def cmd_report(a):
    rows = [json.loads(l) for l in Path(a.runs).read_text(encoding="utf-8").splitlines() if l.strip()]
    if a.when:
        rows = [r for r in rows if r["when"].startswith(a.when)]
    by = {}
    for r in rows:
        by.setdefault(r["arm"], {}).setdefault(r["task"], []).append(r)
    arms = sorted(by)
    for arm in arms:
        runs = [r for rs in by[arm].values() for r in rs]
        ok = sum(1 for r in runs if r.get("ok"))
        tok = [r["tokens"]["total"] for r in runs if r.get("tokens")]
        first = [r["first_input"] for r in runs if r.get("first_input")]
        cost = [r["cost_usd"] for r in runs if r.get("cost_usd") is not None]
        print(f"arm {arm}: {ok}/{len(runs)} correct  median tokens {statistics.median(tok) if tok else 0:.0f}  "
              f"median first request {statistics.median(first) if first else 0:.0f}  total cost ${sum(cost):.2f}  failed runs {sum(1 for r in runs if r.get('failed'))}")
    for x, y in (("C", "A"), ("B", "A"), ("C", "B")):
        if x not in by or y not in by:
            continue
        pairs = []
        for task in sorted(set(by[x]) & set(by[y])):
            for rx, ry in zip(by[x][task], by[y][task]):
                if rx.get("tokens") and ry.get("tokens"):
                    pairs.append(rx["tokens"]["total"] / ry["tokens"]["total"])
        if not pairs:
            continue
        wins = sum(1 for p in pairs if p < 1)
        print(f"{x}/{y}: median ratio {statistics.median(pairs):.2f}  {x} cheaper on {wins}/{len(pairs)}  exact binomial p = {sign_p(wins, len(pairs)):.4f}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run")
    r.add_argument("--copy", required=True)
    r.add_argument("--bin", required=True)
    r.add_argument("--plugin", required=True)
    r.add_argument("--tasks", required=True)
    r.add_argument("--arms", default="A,B,C")
    r.add_argument("--out", required=True)
    r.add_argument("--note", default="")
    r.add_argument("--scratch")
    r.add_argument("--timeout", type=int, default=900)
    r.add_argument("--agent-command", default=DEFAULT_COMMAND)
    r.add_argument("--when", default=datetime.now(timezone.utc).isoformat(timespec="seconds"))
    r.set_defaults(fn=cmd_run)
    p = sub.add_parser("report")
    p.add_argument("runs")
    p.add_argument("--when")
    p.set_defaults(fn=cmd_report)
    a = ap.parse_args()
    a.fn(a)


if __name__ == "__main__":
    main()
```

Run: `cd "$W/bench/agent" && python3 -m unittest test_agent -v 2>&1 | tail -4` — expected `OK`, 9 tests. If `sign_p(18, 24)` reads a fourth decimal off from `0.0227`, print the value and pin the test to it at three places — the bar is 18 wins, the p is what the arithmetic says.

- [ ] **Step 4: Build the task file and read it**

```bash
cd "$W" && python3 bench/agent/tasks.py build --repo "$C" --out bench/agent/tasks.jsonl | tee "$A/t6-tasks.txt"
python3 - "$W/bench/agent/tasks.jsonl" <<'PY' | tee -a "$A/t6-tasks.txt"
import json, sys
for l in open(sys.argv[1], encoding="utf-8"):
    t = json.loads(l)
    print(t["id"], t["kind"], json.dumps(t["expect"], ensure_ascii=False)[:100], "|", t["prompt"][:70])
PY
```

Expected: `24 tasks: keyword 8, code 6, impact 6, trace 4`; every impact task's `naming` list is non-empty and holds its own `file`. An impact target whose `naming` list is only its declaring file has no caller rg can see — replace nothing; the task stays and reads as a task no arm can pass, which the report shows as such.

- [ ] **Step 5: A dry run of two tasks in every arm**

Two tasks, three arms, six runs — under a dollar, and the check that the harness facts the runner assumes hold on this machine:

```bash
head -1 "$W/bench/agent/tasks.jsonl" > "$A/t6-two.jsonl"; grep '"kind": "impact"' "$W/bench/agent/tasks.jsonl" | head -1 >> "$A/t6-two.jsonl"
cd "$W" && python3 bench/agent/run.py run --copy "$C" --bin "$B" --plugin "$W/plugin" --tasks "$A/t6-two.jsonl" --arms A,B,C --out "$A/t6-dry.jsonl" --scratch "$A/t6-scratch" --note "dry run" 2>&1 | tee "$A/t6-dry.txt"
python3 bench/agent/run.py report "$A/t6-dry.jsonl" | tee -a "$A/t6-dry.txt"
grep -l '"name":"mcp__repograph' "$A"/t6-scratch/C-*.stream | wc -l
grep -l 'repograph ask\|repograph impact' "$A"/t6-scratch/B-*.stream | wc -l
grep -l 'This repository has a repograph graph' "$A"/t6-scratch/B-*.stream | wc -l
```

Expected: six rows with `tokens` and no `failed`; in arm C at least one stream names an `mcp__repograph…` tool call; in arm B at least one stream shows a `repograph …` Bash call; the hook's notice appears in B's streams. Any of the three counts at `0` is a harness fact to settle before Task 7 — the tool's name under `--mcp-config` (read it from the stream and put it into `BARE_TOOLS`'s `allowed` list), the plugin loading under `--setting-sources ""` (drop that flag from `DEFAULT_COMMAND` for every arm if the skill did not load and say so in the README), the hook firing under `--plugin-dir`. Each is recorded in `$A/t6-dry.txt` with what was changed, and the change is to the runner and the README, never to a task or to the rule.

- [ ] **Step 6: `bench/agent/README.md` and the commit**

Twenty lines: what the three arms are, how truth is read, the one command to run the suite, the one to report it, that `runs.jsonl` is append-only in the way `bench/history/runs.jsonl` is, and the sentence that the rule lives in the spec.

`git -C "$W" add bench/agent` — subject `chore(bench): an agent bench — 24 tasks, three harness arms, a paired report`, body with the task counts, the dry run's six rows and any harness fact it settled.

---

### Task 7: The measurement, the verdict, and the record

**Files:**
- Create: `docs/bench/2026-09-06-agent-surface-results.md`, `docs/adr/ADR-002-the-agent-surface.md`
- Modify: `bench/agent/runs.jsonl` (the run's rows), `bench/history/runs.jsonl` (two rows, the retrieval smoke test), `README.md` (the numbers Task 5 left as sentences; the default the rule chose), `plugin/.mcp.json` (kept, moved or removed by the rule), `docs/bench/next-version-gaps.md` (one row under "What is explicitly not on this list")
- Create (scratch): `$A/t7-*.txt`

**Interfaces:**
- Consumes: every `$A` transcript, the dry run's harness facts, Rule 5 as the spec states it.
- Produces: the verdict, and a README whose every number names a file.

- [ ] **Step 1: The rule, copied before the run**

Copy the spec's Rule 5 — the five clauses — verbatim into `$A/t7-rule.txt` before anything below runs. The results document quotes that file.

- [ ] **Step 2: The run**

Twenty-four tasks, three arms, one run each — 72 headless sessions. At the dry run's per-run cost it is an estimate of $10–25 in model tokens and one to two hours of wall time; say the real total from the report afterwards.

```bash
cd "$W" && python3 bench/agent/run.py run --copy "$C" --bin "$B" --plugin "$W/plugin" --tasks bench/agent/tasks.jsonl --arms A,B,C --out bench/agent/runs.jsonl --scratch "$A/t7-scratch" --note "agent surface, first full run" 2>&1 | tee "$A/t7-run.txt"
python3 bench/agent/run.py report bench/agent/runs.jsonl | tee "$A/t7-report.txt"
```

Between arms nothing is rebuilt; the copy is not reset (no arm may write, and `git -C "$C" status --porcelain -- ':(exclude)graphify-out'` before and after, saved to `$A/t7-copy-status.txt`, is the proof). A run that `failed` (no result message — a timeout, a harness error) is re-run once for that (arm, task) with `--arms <arm> --tasks <one-line file>`; a second failure stays a failure and the pair is excluded from the sign test, which the report already does.

- [ ] **Step 3: Read the report against the rule**

In the order of the clauses, writing each verdict into `$A/t7-verdict.txt`:

- (i) pass counts per arm from the `arm X: n/24 correct` lines — B and C eligible only at ≥ A's;
- (ii) `C/A: median ratio r  C cheaper on w/24  p` — C is the default if r ≤ 0.5 and w ≥ 18, and `C/B` median ≤ 1.1;
- (iii) else `B/A` on the same two clauses — B is the default;
- (iv) else neither;
- (v) if the decisive pair's wins land in 15–17, the second run: `--arms X,Y --note "second run, clause (v)"`, then `report` on both runs together and 36 of 48.

Then the surface reading beside it: `median first request` for C minus A — the cost of the schemas as the harness rendered them — recorded next to the JSON proxy (`$A/t3-surface.txt`, the bytes the unit test read, written now: `cargo test --release --manifest-path "$W/Cargo.toml" the_surface_fits -- --nocapture 2>&1 | grep bytes`).

- [ ] **Step 4: Apply the verdict**

- C the default: nothing moves; the README's "For agents" says the plugin's MCP server is on by default and quotes the ratio, the wins and the p.
- B the default: `git -C "$W" mv plugin/.mcp.json plugin/mcp/mcp.json`; the setup command gains a step 8 that offers `claude mcp add repograph -- repograph mcp`; the README says the skill and the CLI are the default and the server is one command away, with the numbers.
- Neither: the README's paragraph says what was measured and claims no default; both files stay where they are.

Retrieval's smoke test, after the last code change on the branch:

```bash
cd "$W" && NOTE="agent surface: status, --json, capped renderers, mcp, bounded embedding — no retrieval change" bench/history/run-repograph.sh 2>&1 | tee "$A/t7-history.txt"
```

Expected: two rows in `bench/history/runs.jsonl` with the same counts as Task 0's baselines, green.

- [ ] **Step 5: The results document**

`docs/bench/2026-09-06-agent-surface-results.md`, in the shape of `2026-09-05-0.5.0-gaps-results.md`: a header naming the branch, the base commit, the fixture, the copy and the harness versions (`claude --version`, `codex --version`); then sections in this order, each opening with the rule as pre-registered (copied from `$A/t7-rule.txt` and the spec, not paraphrased) and closing with pass/fail per clause:

- **The yardsticks** (`$A/yardstick.txt`) and the caps chosen from them;
- **Rule 1** — the verdict lines from `t1`, `t2`, `t3`, `t4` (thirty-two `identical`);
- **Rule 2** — `$A/t2-rule2-serve.txt`, `$A/t3-rule2.txt`;
- **Rule 3** — the bytes the test read, and the first-request reading, C − A;
- **Rule 4** — `$A/t4-rule4.txt`, the two per-row rates and the arithmetic for the default model;
- **Rule 5** — the report verbatim (`$A/t7-report.txt`), the per-task table (task, kind, A/B/C tokens, ok), the clauses in order and the verdict, the total cost as read; the second run if clause (v) applied;
- **Harness facts** — everything the dry run and the manual round trips settled (`$A/t3-claude.txt`, `$A/t3-codex.txt`, `$A/t5-*.txt`, `$A/t6-dry.txt`): tool names, which flags loaded the plugin, whether the hook fired, what Codex showed;
- **What ships and what stays open** — one paragraph per task, and the list of what was measured and not adopted (a second arm, a cap, a flag) with the file each rests on.

- [ ] **Step 6: ADR-002**

`docs/adr/ADR-002-the-agent-surface.md`, in ADR-001's shape — **Context** (the three surfaces an agent had, in three sentences, with the GitNexus yardstick), **Decision** (the spec's decision paragraph, the verdict of Rule 5 with its numbers, what was given up), **Consequences** (the ceilings as tests, the caps, no tool builds, one process per session, what would reopen it: a harness that renders schemas at a cost the first-request reading did not show, or a second corpus where the suite reads differently). Status: `Accepted, 2026-09-06` with the clause that decided.

- [ ] **Step 7: README, gaps, history, commit, PR**

- README "For agents": every sentence Task 5 left for this task, filled from `$A/t7-report.txt` and `$A/t3-surface.txt`; the Status rows unchanged.
- `docs/bench/next-version-gaps.md`, under "What is explicitly not on this list": one bullet — *A hook that runs `changes` before every commit* — with the spec's reason and a pointer to the results document.
- Commit the docs and the history in one: `git -C "$W" add docs README.md bench/agent/runs.jsonl bench/history/runs.jsonl plugin` — subject `docs(agents): the agent surface measured — the surface cost, the bounded wait, and which default the rule chose`, body listing the five rules and their verdicts with the results document's sections.
- Push and open the PR:

```bash
git -C "$W" push -u origin feat/agent-surface
gh auth switch --user devmaxxx && gh pr create --repo devmaxxx/repograph --base main --head feat/agent-surface --title "feat: the agent surface — status, --json, repograph mcp, the plugin, the agent bench" --body-file "$A/t7-pr.md"
```

`$A/t7-pr.md` is the results document's "What ships and what stays open" with a table of the five rules (verdict, the deciding number, the file) and the test counts (unit, `tests/serve.rs`, `tests/mcp.rs`, the two Python suites); no AI trailers, no session links. The executor does not merge.

---

## Placeholder check, done when the plan was written

Every `$A/...` path is a file a step creates before another reads it. The numbers this plan cannot know are named as the files that will hold them — the surface bytes (`$A/t3-surface.txt`, read from the unit test), the Rule 4 timing (`$A/t4-rule4.txt`), the Rule 5 report (`$A/t7-report.txt`) — and each is written into prose only by copying from that file. The four harness facts the runner assumes (the MCP tool's name under `--mcp-config` and under `--plugin-dir`, the plugin loading under `--setting-sources ""`, the hook firing under `--plugin-dir`, Codex's `mcp add` syntax) are checked by Task 3 Step 6, Task 5 Step 6 and Task 6 Step 5 before Task 7 spends anything, and a fact that fails changes the runner or the README, never a task or a rule. No step says "similar to" another; the Rust in Tasks 1–4 is written against `src/query.rs`, `src/impact.rs`, `src/changes.rs`, `src/serve.rs`, `src/ask.rs` and `src/index/dense.rs` as they stand at `32577e2`. The test counts expected along the way — 444 → 447 → 450 → 452 → 456 in the unit binary, 12 in `tests/serve.rs`, 8 in `tests/mcp.rs`, 5 and 9 in the two Python suites — are what `cargo test --release` read at `32577e2` plus what each task adds; a count that differs by more than the task's own tests is a test that was lost, not a number to record.
