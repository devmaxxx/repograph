# Agent Surface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Claude Code and Codex use `repograph` for what it is good at — the entry point, the blast radius, the diff — while the text that tells them to costs a few hundred tokens a session instead of the ~7,800 it costs today, the graph's answer reaches an agent that never decided to ask, every subagent inherits the rule mechanically rather than by prose, and a cheap model can run the graph while an expensive one reasons. Claude Code first, Codex on the same files. Every choice below is judged by a harness that runs real agents on the pinned fixture and records the tokens the harness itself reports; nothing in this plan ships on an estimate.

**Architecture:** repograph is a Rust CLI (`src/`) over a `.repograph/` store; the agent-facing surface today is three files in the consuming repository — `.claude/CLAUDE.md`, `.claude/skills/repo-query/SKILL.md`, `.claude/hooks/repograph-notice.mjs` — plus prose asking the model to pass the rule on. This plan moves that surface into this repository under `agent/` (one hook script dispatched by event, one skill, one subagent definition, one ten-line stanza, one installer), so Claude's `.claude/` and Codex's `.codex/` + `.agents/` are two installs of one source. Four hook events do the work: `SessionStart` prints a repository brief; `PreToolUse` on `Bash|Grep|Glob` answers a search with the graph's top three lines under strict gates; `PostToolUse` on `Edit|Write` injects a risk line when a touched symbol's callers cross MEDIUM; `SubagentStart` hands every subagent the three-line rule. Rust changes are additive flags and two subcommands (`prime`, `install-agent`), ordered last.

**Tech Stack:** Rust 2021 pinned at 1.98.0, dependencies `=`-pinned, no new crates. Node ≥ 18 for the hooks (already required by the notice hook and the npm launcher). Claude Code 2.1.263 (`claude -p --output-format stream-json` for the harness), Codex CLI 0.147.0. Python 3 for scoring, in the `bench/` style. bash 3.2 for scripts.

**Spec:** this document's *Decision* section. No separate spec was written; the decision and its consequences are argued here because they are the plan.

## Rebased 2026-09-09

This plan's Tasks 0–8 stand as written. Five things landed under it since it was written against `868f4c1`, and each one changes a line of it; the table is the whole of the rebase, and [`2026-09-09-agent-sync-and-results.md`](2026-09-09-agent-sync-and-results.md) carries the work this plan does not have — the Codex contract read rather than assumed, the resident-answer latency, the reranker diagnostics, and the README the stanza points at. The branch was cut at `0def05e`, which is `fix/critical-defects` stacked on `origin/main` at `1e185ed`.

| Landed | Where this plan assumed otherwise | The delta |
|---|---|---|
| **PR #21 — families derived from definitions** (`9db1f0f`) | The stanza and brief describe `id_families` as a setting; `verify` reports "in families never declared" | The brief reports the **derived** family count and cites `repograph families`; no installed text tells an agent to configure families, and a `repograph.toml` notice for those two keys is expected output, not an error |
| **PR #22 — `core.quotepath=false` on `changes`** (`1e185ed`) | The PostToolUse risk hook was drafted against a `changes` that silently dropped any file with a non-ASCII path | The risk hook is correct on a Cyrillic-named file now; the harness gains one case with such a path so it is proved rather than assumed |
| **PR #20 — `threads` became `resources`** (`6b78837`, breaking) | Any installed config or doc naming `threads` | The installer never writes `threads`; the skill's performance paragraph names `resources` |
| **PR #13 + #21 — `enrich_model` / `rerank_model` are config, sonnet reranks** | The scout task picks a model in prose | The scout reads the configured model; the stanza names no model at all |
| **PR #17 + #23 — a Windows binary, and the config trust boundary** (`cfd4c40`) | Hooks and the installer are POSIX-shaped, and a project file could name a transport | `install-agent` writes the same JSON on Windows; the hook script is Node and already portable; and anything that configures a transport writes the **machine** file, because a project's `repograph.toml` may no longer name `enrich_command` or `rerank_command` |

## Global Constraints

- The worktree is `/Users/max/Documents/projects/repograph/.worktrees/agent-surface`, branch `feat/agent-surface`, cut from `origin/main`. `main` carries `868f4c1` (#14) at the time of writing; Task 0 records the tip it actually found. This plan file lives on `docs/weak-spots-residue-seat-register` and is not part of the feature branch's diff.
- **The fixture `~/bench/beauty-crm-502e8a6d` is read-only.** Every command against it is `--stale --no-dense` (or `bench`/`dump`/`verify`/`explain`, which do not refresh), never `build`/`update`/`enrich`/`embed`, never a hook that writes. Agents are never run *in* the fixture: the harness runs them in a disposable linked worktree of the beauty-crm repository at the same commit, with the fixture's `.repograph/` copied in (Task 0). Runbook trap 7 does not apply there — the copy has its own identical tree beside it — but the copy is still never written by a hook (hooks pass `--stale`; the one exception, the PostToolUse risk hook, is argued in the Decision and confined to the harness worktree and to consuming repositories).
- **Hooks never open the dense model.** Every hook invocation carries `--no-dense`. A `serve` that happens to be up answers a `--no-dense` question lexically, so nothing changes when one is running.
- **No model token is spent by a task that does not name its ceiling.** The harness caps each agent run with `--max-budget-usd`; the escalate measurement runs the model only on the subset the offline gate selected and states the count first; the repo-scout measurement is the harness again. Enrichment is never re-run.
- **Numbers.** Everything measured during planning is labelled with the command that produced it (the *Measured during planning* ledger). Token sizes of command output are UTF-8 bytes / 4, the README's proxy, which counts Cyrillic at roughly double; the harness measures tokens the model was actually billed (`modelUsage` in the result message). A number this plan cannot measure now is written as "to be measured in Task N". ADR-001 applies: a floor is committed before its number is read.
- The GitNexus user-level hooks (`~/.claude/settings.json`, `~/.codex/hooks.json`) and its MCP server stay until 0.6.0 covers C#/Kotlin/Python. `graphify-out/` and `.gitnexus/` in beauty-crm stay on disk and unused; nothing here proposes touching them.
- No MCP server (Decision §6). No Windows transport work (its own plan). No new language extractors (0.6.0).
- Versions: `main` is 0.5.0. Tasks 0–5 and 9 change no Rust and ship as 0.5.x docs/hooks/bench work; Tasks 6–8 add flags and subcommands and ship in the current version line as well — Max, 2026-09-07: «включить в текущую версию» — so the whole plan lands as 0.5.x, and nothing in it bumps `Cargo.toml`.
- The commit hook rejects AI attribution trailers and session links, and rejects any single shell command that contains both a heredoc and the text `git commit` unless the heredoc's first line is a Conventional Commits subject: edits and commits go in separate commands; `git commit -F - <<'MSG'` works when the first heredoc line is the subject. It also reads any heredoc whose body mentions a commit step, so a plan chunk or a skill paragraph that names the commit command is written with the Write tool, never appended by heredoc. Subjects are Conventional Commits (`feat(agent): …`, `test(bench): …`, `docs: …`, `feat(cli): …`; the hook's type list is feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert, so bench work commits as `test(bench)`).
- Comments say why, never what; no ticket ids; tool directives stay. Agent-facing text (stanza, skill, hook, brief) is English: the hook and the skill already are, and one text serves both harnesses.
- Any `gh` call runs `gh auth switch --user devmaxxx && gh …` in the same shell command.
- Implementers never dispatch subagents. (The harness spawns `claude -p` processes; that is the measurement, not delegation.)

---

## Decision: the agent surface

Seven dimensions were to be decided. Two measurements taken during planning reshape all seven, so they come first.

**The interception surface is Bash, not Grep/Glob.** A pass over Max's 38 recorded beauty-crm sessions (`~/.claude/projects/-Users-max-Documents-projects-beauty-crm/*.jsonl`, 28 with tool calls, 3,317 `tool_use` blocks) found **Grep 0, Glob 0, Read 35, Bash 2,727** — of the Bash calls, **933 contain `rg` or `grep`** (median 20 per session, max 113). The reason is in this session's own system prompt: under auto mode the harness says "do your work through the Bash tool wherever it can accomplish the job … search with grep and find … rather than using the dedicated Read, Edit, or Write tools." Today's notice hook matches `Read|Grep|Glob`, so it is nearly dead: its full text appears in **2 of 38** transcripts, its reminder **2 times** in total. `repograph` itself was run 227 times across **11 of 28** sessions, median **0** per session (one session, a bench sitting, accounts for 134). Any priming or interception that does not parse Bash commands is priming for a tool the agent does not use here. GitNexus's hook already parses `rg`/`grep` out of Bash; that part of its design is right and is kept.

**Prose propagation to subagents fails seven times in eight.** 160 subagent transcripts under those sessions: Bash 4,149, Edit 550, Read 518; **121 of 160 ran `rg`/`grep`** (1,404 calls); **16 ran `repograph`** (45 calls). Of the 146 `Agent` spawns in the parent transcripts, **19** carried the word `repograph` in the prompt (13%); of the 20 subagent transcripts whose prompt carried it, **16 ran the tool**; of the 140 whose prompt did not, **0** did. The rule works when it arrives and almost never arrives. Mechanical delivery (a hook) is therefore the whole of dimension 3, and "pass this on" prose is dropped from the stanza's job description.

### 1. Priming cost

What a session pays today, bytes/4: `.claude/CLAUDE.md` 29,543 B ≈ **7,385 tok**, of which the repograph sections (everything before `## Скиллы`) are 13,771 B ≈ **3,442 tok**; the `repo-query` skill 9,247 B ≈ **2,311 tok** when invoked (its one-line description is in every session's skill list regardless); the notice hook's full text 1,611 B ≈ **402 tok** once, its reminder 89 B ≈ 22 tok every 40th watched call. Roughly 3,850 tokens of repograph text per session before the skill, 6,150 with it — re-read after every compaction, since CLAUDE.md is re-attached and the skill body is not.

Candidates:

- **(a) Keep the sections, trim them.** Rejected: the sections carry history (graphify's numbers, the import-legacy measurement, model comparisons) that belongs in this repository's docs, not in an agent's every turn. Trimming still leaves the wrong thing in the wrong place.
- **(b) A ten-line stanza in CLAUDE.md, the rest on demand in the skill.** The stanza drafted in this plan is 1,014 B ≈ **253 tok** (`$S/stanza.md`, `wc -c`); it carries the five commands, the output shape, the depth-1 and `--base main` defaults, the cost of the wide flags, and a pointer to the skill. The skill is rewritten to carry routing only (target ≤ 4,000 B ≈ 1,000 tok, measured in Task 2), with the measurement history moved to `docs/bench/`.
- **(c) A SessionStart brief produced by repograph itself.** A `repograph prime` line set drafted here is 530 B ≈ **132 tok**: node counts by kind, edge count, enrichment coverage, the vectors' model, the id families, the five commands and the one rule. It is *repository-specific* where the stanza is generic, it fires on `startup|resume|clear|compact` — so it comes back after a compaction, which the skill body does not — and it is authoritative about the store's actual state (enriched or not, which embedder), which prose can only claim. Until the Rust subcommand exists the hook prints the first two lines of `verify` (58 ms, measured) plus the fixed command lines — the same information, the same size.

**Picked: (b) + (c).** Stanza ≈ 253 + brief ≈ 132 + reminder 22 ≈ **~410 tokens of priming per session** against ~3,850–6,150 today — draft sizes in bytes/4; Task 2 Step 2 records the shipped bytes and the harness measures what a session is billed. The notice hook's full text goes: the brief has taken its job. The hook keeps `BUILD_NOTICE` (store missing, once) and the 22-token reminder, and its counter now counts Bash calls that touch watched files or run `rg`/`grep`, since those are the calls that exist. What survives compaction: the stanza (CLAUDE.md is re-read), the brief (`SessionStart` with matcher `compact`), the reminder cadence; the skill body does not, and does not need to — the stanza is enough to act on.

*Judged by:* the harness (Decision §7), configuration C against B: total billed tokens per task and hit rate, both models. Floor F1 in Task 3.

### 2. Answering instead of grepping

GitNexus intercepts `Grep|Glob|Bash` and injects graph context on every search, ungated except for a 3-character minimum, so the model never has to decide to ask. Its answer for a hub is large and the injection sits in the transcript for the rest of the session; a Bash-heavy session at 20 `rg` calls would carry twenty injections. Token arithmetic: an injection is paid once as output of the hook and then on every later turn as input (cache-read, but counted against the context and against compaction). So the gate is worth more than the answer's size, and the answer's size must be bounded regardless.

Candidates:

- **(a) Remind, never intercept (today).** Measured dead above: fires in 2 of 38 sessions; the median session asks the graph 0 times and greps 20.
- **(b) Intercept and deny the grep, telling the agent to ask instead.** Rejected. A third of the searches an agent runs are for literals the graph does not index — an env var, a string in a test, a config key — and a denied tool call costs a whole extra turn (the agent's reply plus a new call), which is more than any injection.
- **(c) Intercept, answer beside the grep, under gates.** The hook runs `repograph --repo <root> --no-dense ask --stale --seeds 3 <words>` and injects the seed lines (no neighbour line), each cut to 160 characters. Measured on the fixture (`$S/measure*.sh`): the injection is **89–107 tok** for word queries, **21–36 tok** for a symbol or an id; the call costs **0.14 s** cold (median of 7; `serve`, if up, answers in 6.8 ms), well inside a 5 s hook timeout. The gates, each written to keep an injection out of the transcript rather than to put one in:
  - the pattern yields ≥ 1 word of ≥ 4 letters (Latin or Cyrillic) after regex metacharacters and path separators are stripped; a pattern with `/` and no space is a path and is skipped; at most 6 words are sent;
  - a single identifier-shaped token (`asGrosze`, `FR-PAY-22`, `withTenant`) is sent as is — the exact stage answers it in 60 ms and 20–40 tokens, which is the best case;
  - once per distinct normalised query per session (a marker file under `$TMPDIR/repograph-hook/<session>/`), and **at most 12 injections per session** — a bound chosen so the worst case is 12 × ~107 ≈ 1,300 tokens, a third of today's CLAUDE.md sections; the harness reports the count actually reached;
  - never when the command itself runs `repograph`, never on `.repograph/` paths, never when the store is missing (the BUILD notice covers that once), never when `ask` returns nothing or fails — silence is the default;
  - Bash only when the command contains `rg` or `grep` (the GitNexus tokenizer: first non-flag token, flags with values skipped); `Grep`'s `pattern`; `Glob`'s longest word-like segment. `Read` is not a search and is not intercepted.
  - `--stale` on purpose: the hook never writes the store, at the price of not seeing an edit made since the last refresh — the agent's own next `ask` refreshes, and a hint one edit behind is still a hint.

**Picked: (c), shipped only if the harness says so.** The reminder-only form (a) is the fallback the same hook script provides (`REPOGRAPH_HOOK_INTERCEPT=0`), and Task 3 runs both. The rule, committed here: interception ships when its **tokens per hit are lower than reminder-only's on both models**; if it buys hits at a higher total cost per hit, reminder-only ships and the interceptor stays in the script behind the variable. An injection every search would have made the graph a tax; a gated one is a measured bet.

**PostToolUse on `Edit|Write` — blast radius without asking.** After an edit to a `.ts`/`.tsx` file the hook runs `repograph --repo <root> --no-dense changes --depth 1 --json` (the uncommitted diff mapped onto symbols; 0.34 s on the fixture, measured, whose dirty tree is 2,441 tracked `graphify-out/` files — a normal working tree is smaller), reads `risk`, `touched` and the depth-1 `affected` rows, and injects one line when risk is MEDIUM or above, once per (file, risk) per session: `repograph changes: risk CRITICAL — 61 direct callers of DatabaseService.withTenant in 48 files; \`repograph changes --depth 1\` lists them.` — about 40 tokens. This is the one hook that may refresh the store: `changes` maps hunks onto spans and needs the edited file's new spans, so it performs the same lexical refresh the agent's next `ask` would (~20 ms per file), writes `graph.json` by rename, and a running `serve` picks the write up by manifest stamp. It is confined to consuming repositories and the harness worktree; nothing runs it against the fixture. *Judged by:* the harness's `precommit` task and a hook unit test on a synthetic hub edit (Task 2); it is cheap enough (40 tokens, MEDIUM+ only) that F1 covers it.

### 3. Subagent propagation

Verified from the hooks reference during planning: hooks from settings files "also run inside subagents. When a subagent calls a tool, tool events such as `PreToolUse` and `PostToolUse` fire the same configured hooks as in the main conversation, and the input carries the `agent_id` and `agent_type`"; `Agent` is a built-in tool name a `PreToolUse` matcher can match (Agent SDK hooks reference, matcher row: "Built-in tools include `Bash`, `Read`, `Write`, `Edit`, `Glob`, `Grep`, `WebFetch`, `Agent`"); `updatedInput` "still applies and flows through the normal permission evaluation" when `permissionDecision` is omitted; `SubagentStart` is an event matched on agent type, and the search results for its output contract say it "can inject context into the subagent via `hookSpecificOutput.additionalContext`" (the hooks page itself truncated before that section in every fetch — Task 2 Step 5 verifies it by experiment, five minutes, no tokens beyond one haiku subagent).

Three layers, all in the one hook script, each independent of the others:

1. **`SubagentStart` (matcher `.*`) → `additionalContext` = the three-line rule**, 321 B ≈ **80 tok** per subagent (`$S/rule.txt`). Primary. Verified in Task 2; if the harness version does not deliver it, layer 2 carries the load.
2. **`PreToolUse` on `Agent` → `updatedInput.prompt = prompt + "\n\n" + rule`.** Fallback, same 80 tokens, lands in the subagent's first user message instead of its context. Off by default once layer 1 is verified, so the rule is not delivered twice.
3. **The notice counter keyed by `session_id` *and* `agent_id`.** Today's hook keys its once-per-session counter by `session_id` alone; a subagent shares the parent's, so a subagent's first watched call finds the count already past zero and gets nothing — which is a second reason the 140 unprimed subagents ran `rg`. With `agent_id` in the key, every subagent's first `rg` earns the 22-token reminder whatever else failed. Zero cost when layers 1 or 2 already delivered — the reminder is one line.

**A cheap model on the graph: `.claude/agents/repo-scout.md`.** `model: haiku`, `tools: Bash, Read`, `maxTurns: 6`, a prompt that says: answer with `repograph ask`/`impact --depth 1`/`trace`/`explain` under `--no-dense`, read a file only at a `path:line` the graph printed, return ≤ 300 tokens of `id  path:line — one clause` and nothing else. Subagent transcripts are billed apart from the parent and only the summary returns ("The subagent does that work in its own context and returns only the summary"), and `modelUsage` on the parent's result splits the cost by model, so the trade is measurable: the parent (sonnet in the harness) spends ~300 tokens reading a digest instead of ~2–4k reading `impact` on a hub itself, and the ~2–4k are paid at haiku's price ($1/$5 per MTok against sonnet's $2/$10, the skill's price table cached 2026-06-24). Codex has no project-agent file of this shape; the scout is Claude-only and the plan says so.

*Judged by:* Task 4 — the 12 harness tasks with the scout available against without, sonnet parent; floor F3: cost per hit not higher, hits not lower. The scout ships as a file either way (it costs nothing unless invoked); the stanza recommends it only if F3 passes.

### 4. Model routing inside repograph

Already shipped: `enrich_model=haiku`, `rerank_model=sonnet`, both per machine (`~/.config/repograph/config.toml`) or per repository; the transport is `enrich_command`/`rerank_command` with `{model}` substituted. The measured facts this section leans on and does not re-measure: `--rerank` on sonnet at depth 200 reads 14/14 three times at ≈19,200 input tokens per question, haiku 11/14, opus 14/14 but 23/24 keyword (README, *Spending tokens on purpose*); enrichment on haiku equals sonnet and opus (15/14/15 of 30) at $2.5 once. Nothing here changes a default: sonnet stays the rerank model because it is the configuration that read 14/14 three times; haiku is one line in the machine config for whoever prefers the plan accounting, and the skill says so.

**`ask --escalate` — spend the reranker only on a real miss.** The zero-token path already knows how confident it is: the coverage admission computes, per query, `best/attainable` for the passage list and the questions list (`src/query.rs` `coverage`/`admits`, constant `QUESTIONS_GATE = 0.761`), and `dump` records `attainable_*` and every list's scores, so an escalation gate of the form *"escalate when both lexical coverages are below c_e and no exact hit exists"* can be **priced offline first, at zero tokens**, from the dumps that already exist (`~/bench/residue-seat-register-2026-09-06/base-*-ho-*.json`, `base-*-rec-*.json`, 400 held-out and 82 recorded in both arms). What the offline replay yields: for each candidate c_e, how many questions escalate and how many of those the plain answer actually missed — the gate's *precision*. The rule, committed before the numbers: **c_e is the largest value at which at least half of the escalated held-out questions are plain-`ask` misses** (a reranker call that lands on an answer already right is a wasted 19k tokens; below one in two the flag is a tax). Only then is the model run, on the escalated subset of the 82 recorded cases alone — so the count and the ceiling are known before the first call — and the README table reads escalations, rescued, tokens per rescued case. The flag is opt-in like `--rerank`; the skill recommends it only if the rule passed; whether it may ever be a default is Max's (Open questions). *Judged by:* Task 8's pre-registered clauses.

**Codex users.** `codex exec` reads the prompt from stdin when the argument is `-`, takes `-m <model>`, `-o <file>` for the last message, `--ephemeral` (no session files), `-s read-only`, `--skip-git-repo-check`. The command line to document (TOML literal string; verified in Task 5 by a one-word round trip, the first and only Codex tokens this plan spends):

```toml
rerank_command = 'f=$(mktemp); codex exec - --ephemeral --skip-git-repo-check -s read-only -m {model} -o "$f" >/dev/null 2>&1; cat "$f"; rm -f "$f"'
rerank_model = "gpt-5.6-sol"
```

`claude -p` stays the default. `enrich_command` takes the same shape.

**`--brief` for `ask`: rejected.** `ask` is already 82–246 tokens (six queries, `$S/measure.sh`); the wide flags are what cost — `--seeds 8` 332 vs 244, `--bodies` 1,026–1,659 (4.6–6.7×), `--json` 146–486 (1.8–2.0×). The lever is to *say so* in the stanza (done) and to cap `impact`/`changes` (§5), not to add a flag to the command that is already small.

### 5. Output shape per command for agents

Measured on the fixture, `--stale --no-dense`, bytes/4 (`$S/measure2.sh`, `$S/measure3.sh`, `$S/measure4.sh`):

| command | case | depth 1 | depth 2 | depth 3 | `--json` |
|---|---|---|---|---|---|
| `impact` | `DatabaseService` — CRITICAL, 61 direct, 48 files | **3,852** | 6,962 | 8,936 | 4,968 (d1) · 11,116 (d3) |
| `impact` | `cn` — CRITICAL, 81 direct, 26 files | 3,386 | 3,386 | 3,386 | 4,387 |
| `impact` | `ProblemException` — CRITICAL, 36 direct | 2,163 | 3,841 | 4,133 | 2,799 (d1) · 5,188 (d3) |
| `impact` | `AuthService` — HIGH, 16 direct | 1,009 | 1,009 | 1,009 | 1,263 |
| `impact` | `StaffService` — MEDIUM, 3 direct | 227 | 227 | 227 | 296 |
| `impact` | `asGrosze` — MEDIUM, 7 direct | 335 | 383 | 431 | 444 (d1) · 546 (d3) |
| `changes` | live branch of the working clone, 13 files, docs/CI | 328 | 328 | 328 | 429 |
| `changes` | fixture against `cbc931ba~1` — 98 files, 21 symbols changed, 51 affected | 4,329 | 4,602 | — | 6,335 |
| `changes` | fixture against `a7acc0f4~1` — 350 files, 513 symbols changed | 25,058 | 25,542 | — | 33,767 |
| `explain` | `FR-PAY-22` · `DatabaseService` · `BE-M10` · `FR-CAL-40` | 208 · 257 · 749 · **1,583** | | | |
| `trace` · `verify` | | 60–132 · 161 | | | |

Three things the table decides:

- **`impact --depth 1` is the agent default.** On the hub that matters most, depth 3 is 2.3× depth 1 for callers-of-callers the agent will not read; on every non-hub the depths are equal. The stanza, the skill and the scout say `--depth 1`; `--depth 3` stays the command's default for people.
- **A hub still needs a cap.** 61 direct callers is 3,852 tokens the agent needs three of, plus the risk line. `impact --limit N` (per layer, deterministic: the layer's rows in the order already rendered, then `  … and K more in F files`) and `changes --brief` (the `changed:` list collapsed to `N symbols in F files` — on the 350-file diff that list is 513 of 917 lines, and the agent wrote the diff — plus the depth-1 `affected` rows up to the same limit, plus the risk line) are additive Rust flags (Task 7). Default output is byte-identical, which the blast suite and a golden test hold. The design cap: `--limit 12` on `DatabaseService` and `--brief` on the 350-file diff each read **≤ 25 % of the uncapped bytes** — a rule this plan commits to and Task 7 verifies, not a prediction of the number.
- **`--json` is discouraged for agents.** 1.8–2.0× on `ask`, 1.24–1.3× on `impact`, 1.3–1.5× on `changes`, measured; an agent reads `path:line` off the text form as well as off JSON. The stanza says so; the hook consumes JSON where a script needs a field and never shows it to the model.
- `explain` on a hub (`FR-CAL-40`, 110 lines) is 19× `ask FR-CAL-40`. The skill routes: `ask <id>` first (82 tokens), `explain` when the neighbourhood is the question.

### 6. Codex parity, and why not MCP

Verified from the Codex references during planning: hooks load from `~/.codex/hooks.json` and `<repo>/.codex/hooks.json` ("Project-local hooks load only when the project `.codex/` layer is trusted" — beauty-crm is `trust_level = "trusted"` in `~/.codex/config.toml`); events include `SessionStart`, `PreToolUse`, `PostToolUse`, `SubagentStart`; tool matchers are `Bash`, `apply_patch`, `Edit`, `Write` — **Codex has no Grep or Glob tool, so the Bash parser is the whole interception surface there**; the output contract is the same `hookSpecificOutput` with `additionalContext`/`updatedInput`; hooks are trusted by content hash (`hooks.state … trusted_hash`), so a changed script is skipped until re-trusted. Skills load from `.agents/skills` in the working directory, its parents, the repository root and `$HOME/.agents/skills` — not `~/.codex/skills`, which holds only the bundled `.system` set — and the skills *list* is budgeted at 2 % of the context or 8,000 characters. `AGENTS.md` at the repository root and `~/.codex/AGENTS.md` are the instruction files.

**One source of truth.** `agent/` in this repository holds the hook (`agent/hooks/repograph-hook.mjs`, one script, dispatched on `hook_event_name` like GitNexus's), the skill (`agent/skills/repo-query/SKILL.md`), the scout (`agent/agents/repo-scout.md`), the stanza (`agent/stanza.md`, fenced by `<!-- repograph:begin/end -->` markers so re-installing replaces rather than appends) and the two wiring fragments (`agent/hooks/claude.json`, `agent/hooks/codex.json`). **The installer is a Node script first, a Rust subcommand second.** `agent/install.mjs --claude|--codex [--global] [--command "pnpm exec repograph"] [--dry-run]` ships in the zero-Rust batch: copies the files, merges the hook entries into `settings.json`/`hooks.json` keyed by the command path (idempotent: a second run changes nothing and says so), writes the skill into `.agents/skills/repo-query/` with a symlink from `.claude/skills/repo-query` when `.agents/skills/` exists (beauty-crm's own convention, reversed: the source lives where both tools read, the mirror is the symlink), inserts the stanza between markers in `CLAUDE.md` (`--claude`) and `AGENTS.md` (`--codex`), and prints every path with `wrote`/`unchanged`. `repograph install-agent` (Task 6) embeds the same files with `include_str!` — the reason to have it at all is that npm and cargo users get no `agent/` directory, and that the brief, the skill and the binary then move together — and its output tree is tested byte-equal to the script's on the same input.

**MCP: recommended against, with the number.** The incumbent's own server answers `tools/list` with **17 tools, 43,589 B ≈ 10,897 tokens** of schema (`impact` alone 9,918 B ≈ 2,479; measured over stdio during planning, `$S/gitnexus-tools.jsonl`). How much of that a session pays depends on the harness: Claude Code's documentation says MCP tools are deferred by default and loaded on demand through tool search (code.claude.com, *How Claude Code works*, fetched 2026-09-06), so the schema is charged from the first turn that searches it in and on every turn after, not from the first turn of the session; Codex documents no deferral, so there it is the whole figure from the first turn. A repograph server with five tools would be smaller either way — but the typed schema buys nothing a Bash call lacks here: the commands are five, their syntax fits in the 253-token stanza, the output is already shaped for a model, `serve` already is the resident process an MCP server would be, and Max ruled on 2026-09-03 ("smart but simple/fast, no MCP"). The stanza costs 253 tokens once; a five-tool schema costs its size on every turn after it is loaded. What would change the verdict, stated so it can be checked: a client that cannot run a shell at all, or a harness reading in which agents holding the stanza fail to call repograph on **≥ 30 % of the tasks where it would have answered** — then a typed tool is the next lever to *measure*, not to assume. Task 3 records that fraction (`asked` per task) so the reconsideration has its number.

### 7. The harness that makes the rest honest

`bench/agent/`: **12 tasks** on the fixture's corpus (`tasks.jsonl`), run through `claude -p --output-format stream-json --verbose` — the result message carries `usage`, `total_cost_usd` and `modelUsage` (per model: `inputTokens`, `outputTokens`, `cacheReadInputTokens`, `cacheCreationInputTokens`, `costUSD`; subagent work is *included* in `total_cost_usd` and `modelUsage` and *excluded* from `usage`, per the SDK cost-tracking reference), and the stream carries every `tool_use`, so the harness also counts how many times the agent ran `repograph` and how many times `rg`. Models `haiku` and `sonnet` (the resolved id is read from the `system/init` event). Three configurations as `.claude/` overlays in a disposable linked worktree of beauty-crm at `502e8a6d` — **A** no repograph (no binary on `PATH`, a CLAUDE.md without the repograph sections, no hooks, no skill), **B** today's surface (the fixture's `CLAUDE.md`, `settings.json`, the three hooks, the `repo-query` skill, verbatim), **C** the planned surface (`agent/install.mjs --claude` output on A's CLAUDE.md), and **C-rem** = C with `REPOGRAPH_HOOK_INTERCEPT=0`. The non-repograph skill set is the same in every overlay (none), so the skill list costs the same everywhere. Scoring: `hit` = every expected substring present in the final result text; tokens = the `modelUsage` buckets summed; `cost` = `total_cost_usd`; `turns` = `num_turns`; `asked`, `grepped` from the stream. Rows go to `bench/history/runs.jsonl` with `source: "agent"`, `suite: "agent"`, one row per (config, model) per run, cases keyed `<kind>/<task-id>`, never graded (`gated: false`) — the floors in this plan are read by the results document, not by `track.py`.

**Why 12 and not 82.** Each run is a live agent at real prices. One run of one configuration on one model is 12 invocations, each capped at `--max-budget-usd 0.30`: a **$3.60 ceiling**. The parent model is sonnet throughout — haiku as a *parent* is not a workload Max runs, and haiku's part is the scout inside a sonnet parent (Max, 2026-09-07) — with one opus reading of the shipped configuration, because opus is what Max's sessions run. The sweep the tasks actually make — A and B once (Task 1), C and C-rem twice (Task 3), C-scout twice (Task 4), all on sonnet — is 8 runs, 96 invocations, a **$28.80 ceiling**, plus **C once on opus** at a cap set in Task 3 Step 3b from one measured task (12 invocations × that cap, named in `$M/task3.txt` before the run); each task's file names its share before it runs. The opus run is a reading, not a verdict: it has no opus baseline beside it, so it is recorded as counts — hits, `asked`, `grepped`, cost — and the F1–F3 verdicts are sonnet's. The actual cost is what each run's `total_cost_usd` sums to and is the number the results document carries; the only figure known now is a lower bound on B's first turn — the fixture's CLAUDE.md alone is ≥ 7,385 tokens of input, about $0.015 on sonnet. With 12 tasks no significance is claimed; like the 82 cases these are counts, a smoke test with its fragility read across runs by the history, and the held-out analogue for the one retrieval change (escalate) is the 400-question dump set, not an agent run.

**Codex** is measured last and separately (`codex exec --json` emits JSONL events; whether they carry usage is verified in Task 5 before any run) — the plan's floors are Claude's, by Max's priority.

## Measured during planning

Every number above, with the command that produced it. `$S` is the planning scratchpad; the scripts are committed to `bench/agent/planning/` by Task 1 so the numbers can be re-read.

| Claim | Command / source |
|---|---|
| Grep 0, Glob 0, Read 35, Bash 2,727 (933 with `rg`/`grep`), `repograph` 227 in 11/28 sessions, `Agent` 146 (19 with the rule), full notice in 2/38 transcripts | Python pass over `~/.claude/projects/-Users-max-Documents-projects-beauty-crm/*.jsonl` counting `tool_use` blocks by name (`bench/agent/planning/transcripts.py`, Task 1) |
| 160 subagents: 121 ran `rg`/`grep`, 16 ran `repograph`, 16 of 20 with the rule, 0 of 140 without | the same pass over `**/agent-*.jsonl` |
| CLAUDE.md 29,543 B; its repograph sections 13,771 B; SKILL.md 9,247 B; FULL_NOTICE 1,611 B; REMINDER 89 B; BUILD_NOTICE 214 B | `wc -c`; the notice strings extracted from the hook source by regex; a fake payload run of the hook printed 1,712 B |
| stanza 1,014 B, rule 321 B, brief 530 B | `wc -c $S/stanza.md $S/rule.txt $S/prime.txt` (drafts, reproduced in Task 2) |
| `ask` 82–246 tok; `--json` 146–486; `--bodies` 1,026–1,659; `--seeds 8` 332 | `$S/measure.sh`: six queries, `ask --stale --no-dense`, bytes/4 |
| interceptor injection 89–107 tok (words), 21–36 (symbol/id); 0.14 s | `ask --stale --no-dense --seeds 3`, seed lines only, `cut -c1-160`; `/usr/bin/time -p`, median of 7 |
| lexical cold `ask` 0.11 s (stages: ids 25.7 ms, questions +28.4, lexical +49.9, answered +1.3); exact id 0.06 s; `impact --depth 1` 0.03 s; `verify` 0.058 s; `changes --json` 0.34 s | `REPOGRAPH_TIMING=1`; `/usr/bin/time -p`, medians of 7 (3 for `changes`) |
| the `impact`/`changes`/`explain` table | `$S/measure2.sh`, `$S/measure3.sh`, `$S/measure4.sh` on the fixture; the wide `changes` bases are the eight recorded in `bench/blast.jsonl`; the live-branch row is the working clone `~/Documents/projects/beauty-crm` (`ci/local-run-before-merge`, 15 commits, read with `--stale`) |
| most-called symbols (cn 81, DatabaseService 60, salon 57, tenantTable 45, ProblemException 36 distinct callers) | Python over `.repograph/graph.json`: `Calls`/`Extends` edges grouped by target class |
| GitNexus MCP: 17 tools, 43,589 B | JSON-RPC `initialize` + `tools/list` over stdio to `node ~/.npm/_npx/…/gitnexus/dist/cli/index.js mcp`, saved as `$S/gitnexus-tools.jsonl` |
| hooks: run inside subagents with `agent_id`/`agent_type`; `Agent` is a matchable tool; `updatedInput` applies without a decision; `SessionStart` matchers `startup|resume|clear|compact|fork`; `SubagentStart` accepts `additionalContext` | code.claude.com hooks reference and Agent SDK hooks reference (fetched 2026-09-06); the `SubagentStart` claim is from search results over the same docs, the page truncated in every fetch — **verified by experiment in Task 2 Step 5** |
| Codex: hook paths, events, matchers `Bash|apply_patch|Edit|Write`, output contract, trust hash; skills from `.agents/skills`, list budget 2 % / 8,000 chars; `codex exec -` reads stdin, `-m`, `-o`, `--ephemeral`, `-s read-only`, `--json` | learn.chatgpt.com hooks and build-skills pages (fetched 2026-09-06); `codex exec --help` (0.147.0) |
| `claude -p`: `--output-format json/stream-json`, `--max-budget-usd`, no `--max-turns` in 2.1.263, `--bare` skips CLAUDE.md and hooks but does not use the subscription login (so the harness does not use it), `--setting-sources user,project,local`, `--agents`, `--append-system-prompt-file` | `claude -p --help`; the headless and cost-tracking references |
| prices: Haiku 4.5 $1/$5, Sonnet 5 $2/$10, Opus 5 $5/$25 per MTok | the `claude-api` skill's model table, cached 2026-06-24 — the harness reads `total_cost_usd` and does not use these |
| fixture: 4,336 tracked files, 72 MB without indexes; `.repograph/` 78 MB; both e5 models cached | `git ls-files | wc -l`, `du -sh`, `ls ~/.cache/repograph/fastembed` |

---

## File Structure

| File | Responsibility | Tasks |
|---|---|---|
| `agent/hooks/repograph-hook.mjs` | one hook, four events: SessionStart brief, PreToolUse notice/interceptor, PostToolUse risk line, SubagentStart rule (+ the `Agent` `updatedInput` fallback) | 2 |
| `agent/hooks/repograph-hook.test.mjs` | `node --test`: the gates, the dedup, the caps, the fail-open contract, on fake payloads and a fake `repograph` | 2 |
| `agent/hooks/claude.json`, `agent/hooks/codex.json` | the `hooks` fragments each harness merges | 2 |
| `agent/skills/repo-query/SKILL.md` | routing only, ≤ 4,000 B | 2 |
| `agent/agents/repo-scout.md` | the haiku scout | 4 |
| `agent/stanza.md` | the ten lines between markers | 2 |
| `agent/install.mjs`, `agent/install.test.mjs` | idempotent installer, Claude and Codex, project and global, `--dry-run`; its tests | 2 |
| `bench/agent/tasks.jsonl`, `bench/agent/run.sh`, `bench/agent/score.py`, `bench/agent/overlays/{A,B,C}/`, `bench/agent/planning/*` | the harness, its configurations, the planning scripts | 1 |
| `bench/history/track.py` | accepts `source: "agent"` rows (an `agent` subcommand that reads `score.py` output) | 1 |
| `src/main.rs`, `src/prime.rs` (new) | `repograph prime` | 6 |
| `src/main.rs`, `src/install.rs` (new) | `repograph install-agent`, `include_str!` over `agent/` | 6 |
| `src/impact.rs`, `src/changes.rs`, `src/main.rs` | `impact --limit`, `changes --brief` | 7 |
| `src/query.rs`, `src/ask.rs`, `src/main.rs`, `bench/escalate.py` (new) | `ask --escalate`, its offline replay | 8 |
| `README.md`, `docs/bench/2026-09-0X-agent-surface-results.md` (new), `docs/bench/next-version-gaps.md`, `bench/history/runs.jsonl` | the story, the numbers, the gaps, the history | 9 |
| beauty-crm (Max's repository, its own commit): `.claude/CLAUDE.md`, `AGENTS.md`, `.claude/settings.json`, `.claude/hooks/`, `.agents/skills/repo-query/`, `.claude/skills/repo-query` → symlink, `.codex/hooks.json`, `.claude/agents/repo-scout.md` | the install | 5 |

---

### Task 0: Worktree, baseline counts, and the agents' worktree

**Files:**
- Create (outside the repo): `$M/` = `/Users/max/bench/agent-surface-2026-09-07/`, `$M/task0.txt`, the linked worktree `$M/wt`
- Nothing in the repo changes; no commit.

**Interfaces:**
- Produces: `$M/wt` — beauty-crm at `502e8a6d` with the fixture's `.repograph/` beside it; `$M/bin/repograph`; the test counts every later task is held to.

- [ ] **Step 1: The feature worktree**

```bash
R=/Users/max/Documents/projects/repograph
git -C "$R" fetch origin
git -C "$R" worktree add "$R/.worktrees/agent-surface" -b feat/agent-surface origin/main
W="$R/.worktrees/agent-surface"; git -C "$W" log --oneline -1 | tee -a /dev/null
cd "$W" && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && cargo test --release 2>&1 | grep 'test result'
```

Expected: the tip is `868f4c1` or a later `main`; record it. `444 passed; 0 failed; 2 ignored` and `12 passed` (the counts the 2026-09-06 plans recorded at `868f4c1`); a different count is a moved tip — record it as the baseline, it is not a failure.

- [ ] **Step 2: The agents' worktree — the corpus, never the fixture**

```bash
M=/Users/max/bench/agent-surface-2026-09-07; mkdir -p "$M/bin"
F=/Users/max/bench/beauty-crm-502e8a6d
git -C /Users/max/Documents/projects/beauty-crm worktree add --detach "$M/wt" 502e8a6d
cp -R "$F/.repograph" "$M/wt/.repograph"
ln -sf "$W/target/release/repograph" "$M/bin/repograph"
cd "$M/wt" && PATH="$M/bin:$PATH" repograph --no-dense ask --stale FR-PAY-22 | head -1
PATH="$M/bin:$PATH" repograph --no-dense ask FR-PAY-22 2>&1 >/dev/null | grep -c refresh; echo "(expected 0: same tree, nothing to refresh)"
git -C "$M/wt" status --porcelain | grep -v '^?? .repograph' | wc -l
```

Expected: `FR-PAY-22  docs/prd-2026-08-16/prd/06-payments.md:385 …`; a non-`--stale` ask prints no `refresh:` line — the copied manifest matches the identical tree, so nothing is rewritten; the worktree is clean apart from the untracked store. The fixture was read once (`cp -R`) and not written. `git worktree add` of the beauty-crm repository is the same mechanism the fixture itself uses; the new worktree is disposable and is removed by Task 9.

- [ ] **Step 3: One headless call, to see the fields before spending on 96**

```bash
cd "$M/wt" && PATH="$M/bin:$PATH" claude -p "Run \`repograph --no-dense ask --stale FR-PAY-22\` and reply with the path:line it printed, nothing else." \
  --model haiku --output-format stream-json --verbose --no-session-persistence --allowedTools "Bash" --max-budget-usd 0.05 \
  --setting-sources project > "$M/task0-probe.jsonl" 2>"$M/task0-probe.err"
python3 - "$M/task0-probe.jsonl" <<'PY'
import json,sys
init=res=None; tools=[]
for l in open(sys.argv[1]):
    o=json.loads(l)
    if o.get('type')=='system' and o.get('subtype')=='init': init=o
    if o.get('type')=='result': res=o
    if o.get('type')=='assistant':
        tools+=[b['name'] for b in o['message']['content'] if b.get('type')=='tool_use']
print('model', init and init.get('model')); print('tools', tools)
print('result', (res or {}).get('result','')[:120]); print('cost', res.get('total_cost_usd'), 'turns', res.get('num_turns'))
print('modelUsage keys', {k:sorted(v.keys()) for k,v in (res.get('modelUsage') or {}).items()})
PY
```

Expected: `model` names a haiku id; `tools` is `['Bash']`; the result contains `06-payments.md:385`; `modelUsage` has one model with `inputTokens, outputTokens, cacheReadInputTokens, cacheCreationInputTokens, costUSD` (the names the cost-tracking reference gives — if the CLI spells them otherwise, `score.py` in Task 1 uses the spelling seen here, and `$M/task0.txt` records it). `--setting-sources project` keeps the user-level hooks (GitNexus's) out of the probe — the worktree has no `.claude/` yet, so nothing project-level loads either; the harness passes the same flag with the overlay as the worktree's `.claude/`, and Task 1 Step 6 confirms that loads the overlay and nothing from `~/.claude`.

Also here, before any Rust changes: `cp "$W/target/release/repograph" "$M/repograph-base"` — the binary Task 7's byte-identity check compares against.

- [ ] **Step 4: The task record**

`$M/task0.txt`: the feature worktree's tip, the two test counts, the probe's model id and field spellings, the probe's `total_cost_usd`. Report DONE with that path.

---

### Task 1: The harness — tasks, runner, scorer, overlays A and B

**Files:**
- Create: `bench/agent/tasks.jsonl`, `bench/agent/run.sh`, `bench/agent/score.py`, `bench/agent/overlays/A/CLAUDE.md`, `bench/agent/overlays/README.md`, `bench/agent/planning/transcripts.py`, `bench/agent/planning/measure.sh`
- Modify: `bench/history/track.py` (an `agent` subcommand), `bench/history/README.md` (a paragraph)

**Interfaces:**
- Produces: `run.sh <config> <model> [--repeat N]` → `$M/runs/<stamp>-<config>-<model>/<task>.jsonl` + `summary.json`; `score.py <run-dir>` → one JSON row; `track.py agent <summary.json> --note …` → a `runs.jsonl` row with `source: "agent"`.
- Consumes: `$M/wt`, `$M/bin/repograph`, the overlays.

Background the brief cannot know: the fixture's `.claude/` has 29 skills (7 own, 22 symlinks into `.agents/skills`), five hooks and a `settings.json` that also sets `autoCompactWindow`; overlay B copies `CLAUDE.md`, `settings.json`, `hooks/repograph-notice.mjs`, `hooks/precompact-brief.mjs`, `hooks/compact-state.mjs` and `skills/repo-query/` — the repograph surface as it is — and nothing else, so the skill list is `repo-query` alone in B and C and empty in A. `run.sh` swaps `$M/wt/.claude` per configuration (`rm -rf` + `cp -R` of the overlay; the worktree is disposable) and resets the working tree between tasks (`git -C $M/wt checkout -- . && git clean -fd -e .repograph`), since the `precommit` task edits a file.

- [ ] **Step 1: The twelve tasks**

`bench/agent/tasks.jsonl`, one object per line: `{"id","kind","prompt","expect":[…all-of substrings…],"setup":[…shell lines run in $M/wt before the task…]}`. The expectations below were pinned during planning against the fixture (`$S/pin.sh`); the two marked *pin* are read in Step 2 and written in before the first run.

| id | kind | prompt (verbatim to the agent) | expect |
|---|---|---|---|
| `req-exact` | find-req | Which PRD requirement says the cancellation policy is a rule with numbers, not text? Answer with path:line. | `06-payments.md:385` |
| `req-words-1` | find-req | В каком виде мы храним денежные суммы, чтобы не терять копейки, и где это решено? Ответь path:line. | `ADR-004`, *pin: the path `ask ADR-004` prints* |
| `req-words-2` | find-req | Где требование о том, что клиент должен сразу понять, что с ним говорит робот? path:line. | `FR-AI-21`, *pin: its path* |
| `req-words-miss` | find-req | Где в документах объяснена разница между моментом на временной шкале и временем на стене? path:line. | `ADR-005` (a recorded lexical miss: the task measures what the agent does when the graph does not answer) |
| `who-calls` | who-calls | Who calls `StaffService.create`? Give path:line for each caller. | `staff.controller.ts:87` |
| `hub-callers` | who-calls | How many direct callers does `DatabaseService` have, and what risk label does that give? | `61`, `CRITICAL` |
| `rename` | safe-to-rename | I want to rename `asGrosze`. List every call site with path:line. | `runner.ts:184`, `runner.ts:187`, `runner.ts:205`, `runner.ts:255`, `money.spec.ts:37` |
| `trace-1` | trace-the-flow | How does `AuthController` reach `DatabaseService`? Name the chain with path:line for each hop. | `auth.service.ts:141`, `identity.repository.ts:96` |
| `trace-2` | trace-the-flow | Does `AvailabilityController` call `AvailabilityService` directly? Which method, path:line? | `availability.service.ts:103` |
| `precommit` | pre-commit-review | Review my uncommitted change before I commit: which symbol did it touch and who is affected? | `StaffService.create`, `staff.controller.ts:87`; setup: `sed -i '' '35s/$/ \/\/ reviewed/' apps/api/src/modules/staff/staff.service.ts` (line 35 is inside `StaffService.create`, span 31–43 per the README's `changes` example; Step 2 confirms with `changes --depth 1`) |
| `where-symbol` | find-the-symbol | Where is `TenantContextInterceptor` declared, and which requirement id does its comment cite? | `tenant-context.interceptor.ts:38`, `FR-DM-01` |
| `cross` | doc-to-code | Which code implements FR-CAL-41, the permission matrix code × channel × parallel_policy? Give the file. | `packages/domain/src/availability/force.ts` (the developer suite's `cross` expectation; `ask FR-CAL-41` alone prints the requirement and ADR-008 — Step 2 records whether `explain FR-CAL-41` reaches the file, and the task stays either way: it is G9, measured from the agent's side) |

- [ ] **Step 2: Pin the two paths and the precommit rows**

```bash
cd "$M/wt" && PATH="$M/bin:$PATH"
repograph --no-dense ask --stale ADR-004 | head -1; repograph --no-dense ask --stale FR-AI-21 | head -1
repograph --no-dense explain FR-CAL-41 | grep -c force.ts
sed -i '' '35s/$/ \/\/ reviewed/' apps/api/src/modules/staff/staff.service.ts
repograph --no-dense changes --depth 1; git checkout -- apps/api/src/modules/staff/staff.service.ts
```

Expected: two `path:line`s to write into `tasks.jsonl`; a `0` or `1` for `force.ts`, recorded in `overlays/README.md`; `changes` prints `changed: 1 symbol in 1 file`, `sym:…StaffService.create  …staff.service.ts:31-43`, `d=1 … StaffController.create  …staff.controller.ts:87`, `risk: LOW — 1 direct…` (or `MEDIUM` — write down what it prints). The refresh that `changes` performs writes the *worktree's* store; the checkout restores the file and the next refresh restores the graph.

- [ ] **Step 3: Overlays A and B**

```bash
F=/Users/max/bench/beauty-crm-502e8a6d; O="$W/bench/agent/overlays"; mkdir -p "$O/A" "$O/B/hooks" "$O/B/skills"
# A: the fixture's CLAUDE.md with the repograph sections removed — everything before '## Скиллы', except the H1.
python3 - "$F/.claude/CLAUDE.md" "$O/A/CLAUDE.md" <<'PY'
import sys; t=open(sys.argv[1]).read(); i=t.find('## Скиллы')
head=t.splitlines()[0]; open(sys.argv[2],'w').write(head+'\n\n'+t[i:])
PY
# B: today's repograph surface, verbatim.
cp "$F/.claude/CLAUDE.md" "$F/.claude/settings.json" "$O/B/"
cp "$F/.claude/hooks/repograph-notice.mjs" "$F/.claude/hooks/precompact-brief.mjs" "$F/.claude/hooks/compact-state.mjs" "$O/B/hooks/"
cp -R "$F/.claude/skills/repo-query" "$O/B/skills/"
wc -c "$O/A/CLAUDE.md" "$O/B/CLAUDE.md"
```

Expected: A is ~15.8 kB (29,543 − 13,771), B is 29,543 B. `overlays/README.md` says what each overlay is and that C is produced by `agent/install.mjs` in Task 3, never hand-edited.

- [ ] **Step 4: `run.sh`**

```bash
#!/usr/bin/env bash
# One configuration, one model, all tasks: the agent runs in the disposable worktree with the
# overlay as its .claude/, and every byte it streams is kept, because the tokens, the tool calls
# and the answer are all read from that stream afterwards.
set -euo pipefail
CONFIG=$1; MODEL=$2; REPEAT=${3:-1}
M=${M:-/Users/max/bench/agent-surface-2026-09-07}; WT="$M/wt"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; REPO="$(cd "$HERE/../.." && pwd)"
# C-rem is C with the interceptor off: the overlay directory is C's, the switch is the environment.
OVERLAY_NAME=$CONFIG; [ "$CONFIG" = "C-rem" ] && OVERLAY_NAME=C && export REPOGRAPH_HOOK_INTERCEPT=0
OVERLAY="$HERE/overlays/$OVERLAY_NAME"; [ -d "$OVERLAY" ] || { echo "no overlay $OVERLAY_NAME" >&2; exit 2; }
# A has no binary on PATH; every other configuration has the one this branch built.
case "$CONFIG" in A) BINPATH="" ;; *) BINPATH="$M/bin:" ;; esac
STAMP=$(date -u +%Y%m%dT%H%M%SZ); OUT="$M/runs/$STAMP-$CONFIG-$MODEL"; mkdir -p "$OUT"
rm -rf "$WT/.claude"; cp -R "$OVERLAY" "$WT/.claude"
# One TSV line per task — id, prompt, setup — read back with tabs as the only separator; prompts
# carry backticks and Cyrillic and no tabs.
python3 -c '
import json,sys
for l in open(sys.argv[1]):
    if l.strip():
        t=json.loads(l); print(t["id"], t["prompt"], " && ".join(t.get("setup",[])) or ":", sep="\t")
' "$HERE/tasks.jsonl" > "$OUT/tasks.tsv"
for r in $(seq "$REPEAT"); do
  while IFS=$'\t' read -r id prompt setup; do
    git -C "$WT" checkout -- . && git -C "$WT" clean -fdq -e .repograph
    (cd "$WT" && eval "$setup")
    (cd "$WT" && PATH="$BINPATH$PATH" claude -p "$prompt" --model "$MODEL" \
        --output-format stream-json --verbose --no-session-persistence \
        --setting-sources project --allowedTools "Bash,Read,Grep,Glob" --disallowedTools "Edit,Write,MultiEdit,NotebookEdit" \
        --max-budget-usd "${CAP:-0.30}" > "$OUT/$id.r$r.jsonl" 2> "$OUT/$id.r$r.err") || echo "$id r$r: exit $?" >> "$OUT/failures.txt"
  done < "$OUT/tasks.tsv"
done
git -C "$WT" checkout -- . && git -C "$WT" clean -fdq -e .repograph
python3 "$HERE/score.py" "$OUT" --tasks "$HERE/tasks.jsonl" --config "$CONFIG" --model "$MODEL" > "$OUT/summary.json"
python3 -c "import json,sys; s=json.load(open('$OUT/summary.json')); print(s['config'], s['model'], 'hits', s['hits'], '/', s['tasks'], 'tokens', s['tokens'], 'cost', round(s['cost'],4), 'asked', s['asked'], 'grepped', s['grepped'])"
echo "$OUT"
```

The `--setting-sources project` line is what keeps `~/.claude/settings.json`'s GitNexus hooks out of the run and lets the overlay's `settings.json` in; Step 6 proves it. `~/.claude/CLAUDE.md` (the user memory file, 2,252 B) is loaded by every configuration alike, since memory files are not a setting source; it names no tool and cancels out. A task that ends in `error_max_budget_usd` is recorded as `budget_hit` and counted as a miss: under the same cap, a configuration that runs out of budget on a task another answers is a finding about that configuration, not noise.

- [ ] **Step 5: `score.py`**

Reads every `<task>.r<n>.jsonl`: `system/init` → resolved model; each `assistant` message's `tool_use` blocks → `asked` (a Bash command containing `repograph`), `grepped` (a Bash command matching `\b(rg|grep)\b`, or a Grep/Glob call), `reads` (Read calls); the `result` message → `result` text, `num_turns`, `total_cost_usd`, `modelUsage` summed into `input`, `output`, `cache_read`, `cache_create` (field spellings from `$M/task0.txt`); `hit` = every `expect` substring in the result text; `budget_hit` = `result.subtype == "error_max_budget_usd"`. Emits `{"config","model","resolved_model","tasks","hits","tokens","cost","asked","grepped","per_task":{id:{hit,tokens,cost,turns,asked,grepped,reads,budget_hit}}}` — `tokens` is the sum of all four buckets over all models, because cache reads are what a transcript costs to *carry* and that is the quantity this plan is about. Run: `python3 bench/agent/score.py "$M/runs/<probe>"` on Task 0's probe directory (copy the probe file in as `probe.r1.jsonl` with a one-line `tasks.jsonl`) → a row with `hits 1`.

- [ ] **Step 6: The setting-sources proof, then A and B on sonnet**

```bash
cd "$W" && bench/agent/run.sh B sonnet 2>&1 | tail -2
grep -l 'GitNexus' "$M"/runs/*-B-sonnet/*.jsonl | wc -l; echo "(expected 0: the user-level hook did not run)"
grep -l 'repograph knowledge graph in .repograph' "$M"/runs/*-B-sonnet/*.jsonl | wc -l; echo "(the notice hook's full text: how many tasks it reached under today's surface)"
bench/agent/run.sh A sonnet | tail -2
```

Expected: no `GitNexus` string in any B transcript; the count of tasks the full notice reached, recorded; two summaries. This is the baseline: **A** is what an agent does with no graph, **B** is today. Record the two `total_cost_usd` sums in `$M/task1.txt` — that sum is the first honest "cost per run" figure, and Task 9 writes it into the results document.

- [ ] **Step 7: `track.py agent`, the planning scripts, commit**

`bench/history/track.py`: an `agent` subcommand that reads a `summary.json`, and appends a row `{"arm": "agent:<config>+<model>", "source": "agent", "suite": "agent", "gated": false, "metrics": {"hits": [h, n], "tokens": t, "cost": c, "asked": a, "grepped": g}, "cases": {"<kind>/<id>": 1.0|0.0}, "tool_commit", "tool_dirty", "corpus": "beauty-crm", "corpus_commit": "502e8a6d", "when", "note"}`; `report` treats `suite == "agent"` rows like the dev suite (measured, never graded) and lists chronically missed tasks the same way. Add the test in `bench/history/test_track.py`: a summary fixture round-trips and `report` names a task missed in two consecutive agent rows.

`bench/agent/planning/transcripts.py` is the planning pass (the two Python scripts above, merged, taking the project directory as an argument); `bench/agent/planning/measure.sh` is `$S/measure*.sh` merged, parameterised by `FIXTURE` and the binary. Both run and reproduce the ledger's numbers (the transcript counts will have grown by the sessions since; the output-size numbers reproduce exactly on the fixture).

```bash
cd "$W" && python3 -m pytest -q bench/history/test_track.py 2>&1 | tail -1 && bash -n bench/agent/run.sh && python3 bench/agent/planning/transcripts.py ~/.claude/projects/-Users-max-Documents-projects-beauty-crm | head -8
for c in A B; do python3 bench/history/track.py agent "$(ls -d "$M"/runs/*-$c-sonnet | tail -1)/summary.json" --note "agent-surface baseline, $c"; done
git -C "$W" add bench/agent bench/history/track.py bench/history/test_track.py bench/history/README.md bench/history/runs.jsonl
git -C "$W" commit -F - <<'MSG'
test(bench): twelve agent tasks on the fixture, run through headless Claude and scored on billed tokens

Three configurations as .claude/ overlays in a disposable worktree of the corpus: no repograph, today's
surface, and the planned one. The scorer reads the stream the CLI emits: the result's modelUsage for
tokens and cost, the tool_use blocks for how often the agent asked the graph and how often it grepped.
The baseline rows for A and B on sonnet are recorded; the planning scripts that produced the
plan's session-transcript and output-size numbers are kept beside them.
MSG
```

Expected: tests green; the first four `agent:` rows in `runs.jsonl`.

---

### Task 2: `agent/` — the hook, the skill, the stanza, the installer

**Files:**
- Create: `agent/hooks/repograph-hook.mjs`, `agent/hooks/repograph-hook.test.mjs`, `agent/hooks/claude.json`, `agent/hooks/codex.json`, `agent/skills/repo-query/SKILL.md`, `agent/stanza.md`, `agent/install.mjs`, `agent/README.md`
- Modify: nothing else in the repo.

**Interfaces:**
- Produces: the four-event hook with the contract *never block, never throw, silence by default*; the installer that Task 3 runs to make overlay C and Task 5 runs on beauty-crm.
- Consumes: `repograph` on `PATH` or at `<root>/node_modules/.bin/repograph` (the lefthook resolution, reused verbatim); `REPOGRAPH_HOOK_INTERCEPT` (`0` turns the interceptor off), `REPOGRAPH_HOOK_AGENT_INPUT` (`1` turns the `Agent` `updatedInput` fallback on), `REPOGRAPH_HOOK_DEBUG` (stderr diagnostics, silent otherwise — Codex validates hook output, as GitNexus's comment on issue #1913 records).

- [ ] **Step 1: The failing tests first**

`agent/hooks/repograph-hook.test.mjs`, `node --test`. It spawns the hook with a fake payload on stdin, `PATH` pointing at a fake `repograph` (`$TMP/bin/repograph`, a shell script that echoes canned lines and records its argv to a file), `TMPDIR` fresh per test, and `CLAUDE_PROJECT_DIR`/`cwd` set to a temp root that has or lacks `.repograph/manifest.json`. Cases, each an assertion on stdout JSON or on its absence:

- `session_start_prints_the_brief_and_nothing_where_there_is_no_store` — `SessionStart` with `source: "startup"`: output has `hookEventName: "SessionStart"` and `additionalContext` beginning `repograph` (the fake prints `verify`'s two lines), ≤ 700 bytes; with no store: no output.
- `bash_rg_with_a_word_asks_once_with_three_seeds_and_only_seed_lines` — `Bash` `rg -n "cancellation policy" apps/` → argv recorded is `--repo <root> --no-dense ask --stale --seeds 3 cancellation policy`; `additionalContext` has exactly the fake's three top-level lines, none indented, each ≤ 160 chars; the same payload again → no output (dedup).
- `an_identifier_is_sent_as_is` — `rg asGrosze` → words `asGrosze`; `rg FR-PAY-22` → `FR-PAY-22`.
- `paths_short_words_and_repograph_commands_are_skipped` — `rg foo`, `rg src/app/`, `grep -r "a.b*" x`, `rg -l 'TODO' .repograph/`, `repograph ask x | rg y` → no output, the fake never ran.
- `grep_and_glob_tools_are_parsed` — `Grep` `{pattern: "tenant isolation"}` asks; `Glob` `{pattern: "**/tenant-*.ts"}` asks with `tenant`; `Glob` `{pattern: "**/*.ts"}` is silent.
- `the_twelfth_injection_is_the_last` — 13 distinct patterns → 12 outputs.
- `an_empty_or_failing_ask_is_silence` — the fake prints nothing / exits 1 → no output.
- `intercept_off_leaves_the_reminder_cadence` — `REPOGRAPH_HOOK_INTERCEPT=0`, 80 watched calls with a store present: no output on the first (the brief has that job now), the 89-byte reminder on the 40th and the 80th, nothing else.
- `a_subagent_gets_its_own_first_call` — two payloads with the same `session_id`, different `agent_id`: each first watched call under `INTERCEPT=0` counts from zero (the reminder logic keyed per agent).
- `an_edit_to_a_hub_injects_one_risk_line_once` — `PostToolUse` `Edit` on `x.ts`, the fake `changes --json` returns `{"risk":"CRITICAL","touched":[{"id":"sym:a.ts::DatabaseService.withTenant"}],"affected":[…61 depth-1 rows…],"files":[…48…]}` → one line naming `CRITICAL`, `61`, `withTenant`, `48 files`; again → nothing; risk `LOW` → nothing; an edit to `x.md` → the fake never ran.
- `subagent_start_carries_the_rule` — `SubagentStart` → `additionalContext` equals `agent/stanza.md`'s rule paragraph (the hook reads its text from a constant that the test compares to the file, so the two cannot drift).
- `agent_tool_prompt_is_extended_only_when_asked` — `PreToolUse` `Agent` with `REPOGRAPH_HOOK_AGENT_INPUT=1` → `updatedInput.prompt` ends with the rule and no `permissionDecision`; without the variable → no output.
- `malformed_input_is_silence_with_exit_zero` — garbage stdin → exit 0, no stdout.

Run: `node --test agent/hooks/` — every test fails on a missing file.

- [ ] **Step 2: The hook**

`agent/hooks/repograph-hook.mjs`. The shape, with the parts that carry the decisions written out; the rest is the notice hook's existing code (payload reading, watched extensions, the per-session file counter, the stdout-without-exit note) moved over unchanged.

```js
#!/usr/bin/env node
/**
 * The one repograph hook, dispatched on the event Claude Code or Codex names in the payload.
 *
 *  SessionStart   → a repository brief, so the graph's state and its five commands are in
 *                   context at start and again after a compaction, without a 7k-token CLAUDE.md.
 *  PreToolUse     → Bash/Grep/Glob searches get the graph's top three lines beside them, under
 *                   gates written to keep an injection out rather than put one in: the injection
 *                   stays in the transcript for the rest of the session. Read is not a search.
 *  PostToolUse    → an Edit/Write to code gets one risk line when the touched symbol's direct
 *                   callers cross MEDIUM. The one call here that may refresh the store.
 *  SubagentStart  → every subagent gets the three-line rule; prose asking the parent to pass it
 *                   on reached 19 of 146 subagents in the recorded sessions.
 *
 * Contract: exit 0 always, stdout only when there is something to say, stderr only under
 * REPOGRAPH_HOOK_DEBUG. Every repograph call is --no-dense; every call but `changes` is --stale.
 */
import { existsSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createHash } from 'node:crypto';

const MAX_INJECTIONS = 12;      // 12 × ~107 tokens: a third of the CLAUDE.md sections this replaces
const SEEDS = 3;                // measured 89–107 tokens for words, 21–36 for an id or a symbol
const LINE_CUT = 160;
const REMINDER_EVERY = 40;
const ASK_TIMEOUT_MS = 5000;    // measured 0.14 s cold; serve answers in 7 ms

const RULE = /* the paragraph from agent/stanza.md, verbatim */ '…';

function binary(root) {
  // The lefthook resolution: PATH first, the package's own bin second, and nothing else — a hook
  // that installed anything would not be a hook.
  const local = join(root, 'node_modules', '.bin', 'repograph');
  return process.env.REPOGRAPH_BIN || (existsSync(local) ? local : 'repograph');
}

function run(root, args) {
  const r = spawnSync(binary(root), ['--repo', root, '--no-dense', ...args], { encoding: 'utf8', timeout: ASK_TIMEOUT_MS, stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
  return r.status === 0 ? r.stdout : '';
}

/** Words worth asking, or null: ≥ 4 letters each, six at most, a lone identifier or id kept whole. */
function queryWords(pattern) {
  if (!pattern || /\//.test(pattern) && !/\s/.test(pattern)) return null;   // a path
  const bare = pattern.replace(/[\\^$.*+?()[\]{}|]/g, ' ').trim();
  if (/^[A-Za-z][A-Za-z0-9_]*$|^[A-Z]+(?:-[A-Z]+)*-\d{1,4}$/.test(bare) && bare.length >= 4) return [bare];
  const words = bare.split(/[^\p{L}\p{N}_-]+/u).filter((w) => /\p{L}{4,}/u.test(w)).slice(0, 6);
  return words.length ? words : null;
}

/** The pattern a tool call searches for; GitNexus's Bash tokenizer, kept as it is. */
function searchPattern(tool, input) { /* Grep: input.pattern; Glob: longest word-like segment; Bash: first non-flag token after rg|grep, flags-with-values skipped; null otherwise */ }

function once(sessionKey, kind, key) {
  const dir = join(tmpdir(), 'repograph-hook', sessionKey);
  const file = join(dir, `${kind}-${createHash('sha1').update(key).digest('hex').slice(0, 16)}`);
  if (existsSync(file)) return false;
  try { mkdirSync(dir, { recursive: true }); writeFileSync(file, ''); } catch { /* unwritable tmp: answer this once, never dedup */ }
  return true;
}

function intercept(payload, root, sessionKey) {
  if (process.env.REPOGRAPH_HOOK_INTERCEPT === '0') return null;
  const input = payload.tool_input || {};
  if (payload.tool_name === 'Bash' && /\brepograph\b/.test(input.command || '')) return null;
  const words = queryWords(searchPattern(payload.tool_name, input));
  if (!words || JSON.stringify(input).includes('.repograph/')) return null;
  if (!once(sessionKey, 'ask', words.join(' ').toLowerCase())) return null;
  if (count(sessionKey, 'injections') >= MAX_INJECTIONS) return null;
  const lines = run(root, ['ask', '--stale', '--seeds', String(SEEDS), ...words]).split('\n').filter((l) => l && !l.startsWith('  ')).map((l) => l.slice(0, LINE_CUT));
  if (!lines.length) return null;
  bump(sessionKey, 'injections');
  return [`repograph ask ${words.join(' ')} →`, ...lines, '(read only these path:line; `repograph ask` for more)'].join('\n');
}

function riskLine(payload, root, sessionKey) {
  const file = (payload.tool_input || {}).file_path || '';
  if (!/\.tsx?$/.test(file)) return null;
  let r; try { r = JSON.parse(run(root, ['changes', '--depth', '1', '--json'])); } catch { return null; }
  if (!r || !['MEDIUM', 'HIGH', 'CRITICAL'].includes(r.risk)) return null;
  if (!once(sessionKey, 'risk', `${file}:${r.risk}`)) return null;
  const direct = r.affected.filter((d) => d.depth === 1).length;
  const touched = r.touched.filter((t) => t.indexed !== false).map((t) => t.id.split('::').pop()).slice(0, 3).join(', ');
  return `repograph changes: risk ${r.risk} — ${direct} direct callers of ${touched} in ${r.files.length} files; \`repograph changes --depth 1\` lists them.`;
}

function brief(root) {
  const v = run(root, ['verify']).split('\n').slice(0, 2).join('\n');   // until `repograph prime` exists
  if (!v) return null;
  return `${v}\nask <words|ID|Symbol> · impact --depth 1 <Symbol> · changes --base main · trace A B · explain <id>\nask before reading; read only the path:line printed; say HIGH/CRITICAL before the edit`;
}

const handlers = {
  SessionStart: (p, root) => (built(root) ? brief(root) : null),
  SubagentStart: (p, root) => (built(root) ? RULE : null),
  PreToolUse: (p, root, key) => {
    if (p.tool_name === 'Agent') return process.env.REPOGRAPH_HOOK_AGENT_INPUT === '1' ? { updatedInput: { ...p.tool_input, prompt: `${p.tool_input.prompt}\n\n${RULE}` } } : null;
    if (!built(root)) return isWatched(p.tool_input) && once(key, 'build', 'once') ? BUILD_NOTICE : null;
    return intercept(p, root, key) ?? reminder(p, key);
  },
  PostToolUse: (p, root, key) => (built(root) ? riskLine(p, root, key) : null),
};
```

`main` reads the payload, derives `root` (`payload.cwd || CLAUDE_PROJECT_DIR || process.cwd()`), `sessionKey = `${session_id}-${agent_id || 'main'}`` (layer 3 of §3), calls the handler, and writes `{"hookSpecificOutput":{"hookEventName":<event>, …}}` — `additionalContext` for a string, the object itself for `updatedInput` — without `process.exit`, for the Windows-pipe reason the notice hook records. `count(sessionKey, name)`/`bump(sessionKey, name)` are the notice hook's per-session file counter with a name in the file name, so `injections` and `watched` count apart. `reminder` is today's every-40th logic over watched calls, now counting Bash calls whose command names a watched extension or runs `rg`/`grep`, keyed by `sessionKey`; `built(root)` is `existsSync(join(root, '.repograph', 'manifest.json'))`; `isWatched` and `BUILD_NOTICE` move over unchanged.

Run: `node --test agent/hooks/` → all green. Then the real thing, once, against the agents' worktree:

```bash
cd "$M/wt" && for ev in SessionStart SubagentStart; do printf '{"hook_event_name":"%s","source":"startup","cwd":"%s","session_id":"t1"}' $ev "$M/wt" | PATH="$M/bin:$PATH" node "$W/agent/hooks/repograph-hook.mjs" | python3 -c "import json,sys; o=json.load(sys.stdin); c=o['hookSpecificOutput']['additionalContext']; print(len(c.encode()),'B'); print(c)"; done
printf '{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"rg -n \\"cancellation policy\\" apps/"},"cwd":"%s","session_id":"t1"}' "$M/wt" | PATH="$M/bin:$PATH" node "$W/agent/hooks/repograph-hook.mjs" | python3 -c "import json,sys; c=json.load(sys.stdin)['hookSpecificOutput']['additionalContext']; print(len(c.encode()),'B'); print(c)"
rm -rf "${TMPDIR:-/tmp}/repograph-hook/t1-main"
```

Expected: the brief ≤ 700 B; the rule 321 B; the interception ≤ 500 B with three `path:line` lines from the corpus. Record the three sizes in `$M/task2.txt` — these are the priming figures the results document cites, replacing the drafts.

- [ ] **Step 3: The wiring fragments, the stanza, the skill**

`agent/hooks/claude.json`:

```json
{ "hooks": {
  "SessionStart": [{ "matcher": "startup|resume|clear|compact", "hooks": [{ "type": "command", "command": "node \"$CLAUDE_PROJECT_DIR/.claude/hooks/repograph-hook.mjs\"", "timeout": 10 }] }],
  "SubagentStart": [{ "matcher": ".*", "hooks": [{ "type": "command", "command": "node \"$CLAUDE_PROJECT_DIR/.claude/hooks/repograph-hook.mjs\"", "timeout": 5 }] }],
  "PreToolUse": [{ "matcher": "Bash|Grep|Glob|Read|Agent", "hooks": [{ "type": "command", "command": "node \"$CLAUDE_PROJECT_DIR/.claude/hooks/repograph-hook.mjs\"", "timeout": 10 }] }],
  "PostToolUse": [{ "matcher": "Edit|Write|MultiEdit", "hooks": [{ "type": "command", "command": "node \"$CLAUDE_PROJECT_DIR/.claude/hooks/repograph-hook.mjs\"", "timeout": 10 }] }]
} }
```

`agent/hooks/codex.json`: three of the four events — no PreToolUse interceptor on Codex (Max, 2026-09-07: brief, skill and risk line only; the interceptor's verdict is read on Claude and not carried over blind) — `SessionStart`, `"apply_patch|Edit|Write"` on PostToolUse, `SubagentStart`, and the command `node .codex/hooks/repograph-hook.mjs` (Codex sets no project-dir variable; the session's `cwd` is the repository). The installer rewrites the path for `--global` (`~/.claude/hooks/…`, `~/.codex/hooks/…`).

`agent/stanza.md`: the draft measured at 1,014 B, between `<!-- repograph:begin -->` / `<!-- repograph:end -->`, with `{{command}}` where the invocation name goes (`repograph` by default, `pnpm exec repograph` for beauty-crm). The `RULE` constant in the hook is its fourth-bullet paragraph rewritten as the three-line rule; the test compares them.

`agent/skills/repo-query/SKILL.md`: frontmatter `name`, `description` (the current one, shortened to one sentence — Codex budgets the list); body ≤ 4,000 B: the loop (anchor → words → widen → spend a model only on a miss → read only the `path:line`), the route table with the measured costs from this plan's ledger (`ask` 80–250, `impact --depth 1`, `changes --base main`, `trace`, `rg -l` first for a literal), the output shape, "freshness is not your problem" in three lines, the two paid stages in five lines (haiku for `enrich`, sonnet for `--rerank` at ~19k tokens; the machine config line for a cheaper reranker), verify-before-believing in three lines, and the scout (Task 4) in one. The measurement history that the current skill carries (GitNexus's 0/70, graphify's import, the 82-case table) moves to `docs/bench/2026-09-0X-agent-surface-results.md` in Task 9 with a link. `wc -c agent/skills/repo-query/SKILL.md` ≤ 4000.

- [ ] **Step 4: The installer**

`agent/install.mjs --claude|--codex [--root <dir>] [--global] [--command "<name>"] [--dry-run]`:

- `--claude`: `.claude/hooks/repograph-hook.mjs` (copy), `.claude/settings.json` merged: each event's array gains the fragment's entry unless an entry whose command contains `repograph-hook.mjs` is already there (then it is replaced — that is what makes a re-run after a hook change idempotent and what keeps `autoCompactWindow` and the other hooks untouched; the old `repograph-notice.mjs` entry, if present, is removed and the file deleted, since the brief has its job), the skill into `.agents/skills/repo-query/SKILL.md` with `.claude/skills/repo-query` a relative symlink when `.agents/skills/` exists, else into `.claude/skills/repo-query/SKILL.md`; the stanza into `CLAUDE.md` (root `CLAUDE.md` if it exists, else `.claude/CLAUDE.md`, created if neither) between the markers, `{{command}}` substituted; `.claude/agents/repo-scout.md` once Task 4 adds it.
- `--codex`: `.codex/hooks/repograph-hook.mjs`, `.codex/hooks.json` merged the same way, the skill as above, the stanza into `AGENTS.md`.
- `--global`: `~/.claude/hooks/` + `~/.claude/settings.json`, or `~/.codex/hooks/` + `~/.codex/hooks.json`; no stanza, no skill (a global skill would fire in repositories with no store — `~/.agents/skills` is possible but not written here).
- Prints one line per path: `wrote`, `unchanged`, `replaced`, `removed`. `--dry-run` prints the same lines and touches nothing. Exit 0 on success, 2 on a JSON it cannot parse (it never overwrites a settings file it could not read).

Tests, in `agent/hooks/repograph-hook.test.mjs`'s sibling `agent/install.test.mjs`: a fresh temp root gets the full tree; a second run prints `unchanged` for every path and changes no byte (`find -newer`); a root with an existing `settings.json` carrying other hooks keeps them; a root with `.agents/skills/` gets the symlink; `--codex` writes `.codex/hooks.json` with the `Bash` matcher; `--dry-run` writes nothing.

Run: `node --test agent/` → green.

- [ ] **Step 5: SubagentStart, verified in a session — the one claim the docs did not settle**

```bash
cd "$M/wt" && rm -rf .claude && node "$W/agent/install.mjs" --claude --root "$M/wt" --command repograph >/dev/null
PATH="$M/bin:$PATH" claude -p "Spawn one general-purpose subagent with the prompt: 'Reply with the exact text of any repository rule or note you were given about a knowledge graph, or NONE.' Return its reply verbatim." \
  --model haiku --output-format json --no-session-persistence --setting-sources project --max-budget-usd 0.05 | python3 -c "import json,sys; print(json.load(sys.stdin)['result'][:600])"
```

Expected: the subagent's reply quotes the rule (`repograph ask <words|ID|Symbol>` …). **If it says NONE:** set `REPOGRAPH_HOOK_AGENT_INPUT=1` in the fragment's `PreToolUse` entry (`"command": "REPOGRAPH_HOOK_AGENT_INPUT=1 node …"`), drop the `SubagentStart` entry, re-run; the reply must then quote the rule from its prompt. Record which layer delivered in `$M/task2.txt`; the fragment ships with that layer on and the other off. Either way layer 3 (the per-agent reminder) is in the script and tested.

- [ ] **Step 6: Commit**

```bash
cd "$W" && node --test agent/ 2>&1 | tail -3 && wc -c agent/stanza.md agent/skills/repo-query/SKILL.md
git -C "$W" add agent/
git -C "$W" commit -F - <<'MSG'
feat(agent): one hook for four events, a ten-line stanza, a routing-only skill, and an installer

SessionStart prints the repository's brief, PreToolUse answers a Bash/Grep/Glob search with the graph's
top three lines under gates that keep injections out of the transcript, PostToolUse on an edit injects
one risk line at MEDIUM and above, SubagentStart hands every subagent the rule prose delivered to 19 of
146. The installer writes the same files into .claude/ or .codex/ + .agents/, merges the hook entries
idempotently, and puts the stanza between markers in CLAUDE.md or AGENTS.md.
MSG
```

---

### Task 3: Overlay C, the harness verdict on priming and interception

**Files:**
- Create: `bench/agent/overlays/C/` (generated), `$M/task3.txt`
- Modify: `bench/history/runs.jsonl`

**Interfaces:**
- Produces: the F1 and F2 verdicts; the interceptor's default in `agent/hooks/claude.json` follows F2.

- [ ] **Step 1: Generate C, never hand-edit it**

```bash
cd "$W" && O=bench/agent/overlays; rm -rf "$O/C" && mkdir -p "$O/C-src" && cp "$O/A/CLAUDE.md" "$O/C-src/CLAUDE.md"
node agent/install.mjs --claude --root "$O/C-src" --command repograph && mv "$O/C-src/.claude" "$O/C" && mv "$O/C-src/CLAUDE.md" "$O/C/CLAUDE.md" && rm -rf "$O/C-src"
ls -R "$O/C" && wc -c "$O/C/CLAUDE.md"
```

Expected: `C/CLAUDE.md` = A's plus the stanza (~15.8 kB + 1,014 B), `C/hooks/repograph-hook.mjs`, `C/settings.json` with four events, `C/skills/repo-query/SKILL.md` (no `.agents/` in the overlay root, so a plain copy). `overlays/README.md` records the command that made C.

- [ ] **Step 2: The rule, written before the runs**

Append to `$M/task3.txt` before running anything:

> F1 — priming. For each model, over the 12 tasks: `tokens(C) ≤ tokens(B)` and `hits(C) ≥ hits(B)`, in both of two runs. Pass → the stanza and brief ship (Task 5). Fail on tokens → find which tasks carry the excess from `per_task`; if it is the interceptor's injections, F2 decides; if it is the stanza or the brief, they are cut and re-run once. Fail on hits → the tasks lost are named; a lost hit that B answered through the CLAUDE.md sections' text is a sentence to put back into the stanza, once, and re-run.
> F2 — interception. `tokens_per_hit(C) < tokens_per_hit(C-rem)` on both models, both runs → interception ships on by default. Otherwise reminder-only ships (`REPOGRAPH_HOOK_INTERCEPT=0` in the fragment) and the interceptor stays in the script.
> Recorded regardless: `asked`, `grepped` and the injections count per task (the §6 reconsideration number is the share of tasks with `asked == 0` and `hit == 0` where B or C-rem hit).

- [ ] **Step 3: Run C and C-rem on sonnet, twice**

```bash
cd "$W" && for r in 1 2; do for c in C C-rem; do bench/agent/run.sh $c sonnet | tail -2; done; done
for d in "$M"/runs/*-C*-*; do python3 bench/history/track.py agent "$d/summary.json" --note "agent-surface, $(basename $d)"; done
python3 - <<'PY'
import json,glob,collections
rows=[json.loads(l) for l in open('/Users/max/Documents/projects/repograph/bench/history/runs.jsonl') if l.strip()]
agent=[r for r in rows if r.get('source')=='agent']
by=collections.defaultdict(list)
for r in agent: by[r['arm']].append(r)
for arm,rs in sorted(by.items()):
    for r in rs: m=r['metrics']; print(f"{arm:24s} hits {m['hits'][0]:2d}/{m['hits'][1]} tokens {m['tokens']:8d} cost {m['cost']:.4f} asked {m['asked']:3d} grepped {m['grepped']:3d}  {r['when'][:16]}")
PY
```

Expected: four new rows (C ×2, C-rem ×2) beside the two baseline rows; the table above is what `$M/task3.txt` records, then the F1/F2 verdicts in the words of Step 2, then the per-task table for any failed clause. Cost: four runs, ceiling $14.40; the actual sum of `total_cost_usd` goes in the file.

- [ ] **Step 3b: One reading of C on opus — the cap first, then the run**

Opus is the model Max's sessions run; the harness's `$0.30` cap was sized for sonnet and would cut opus runs short, which reads as misses. So the cap is measured before it is set: one task, the cheapest-looking one (`FR-PAY-22`-style exact id), at a cap of `1.00`, and its `total_cost_usd` read.

```bash
cd "$W" && CAP=1.00 REPEAT=1 bench/agent/run.sh C opus --only 01 | tail -2   # run.sh gains --only <id> here: one task, same scorer
python3 -c "import json; print(json.load(open('$(ls -d "$M"/runs/*-C-opus | tail -1)/summary.json'))['cost'])"
```

Write to `$M/task3.txt` before the full run: `opus cap = 4 × that cost, rounded up to $0.10, floor $0.30`, and the resulting ceiling `12 × cap`. Then:

```bash
cd "$W" && CAP=<the cap> REPEAT=1 bench/agent/run.sh C opus | tail -2
python3 bench/history/track.py agent "$(ls -d "$M"/runs/*-C-opus | tail -1)/summary.json" --note "agent-surface, opus reading of C, no opus baseline"
```

Expected: one row `agent:C+opus`; `$M/task3.txt` gains a paragraph with hits, `asked`, `grepped`, the cost sum and how many tasks hit the cap (`failures.txt` and the result's `stop_reason`). No verdict is read from it: F1–F3 are sonnet's, this is what the shipped configuration does on the model that will use it, beside sonnet's C rows for the eye.

- [ ] **Step 4: Apply the verdict to the fragment; commit**

If F2 failed: in `agent/hooks/claude.json` and `codex.json`, the PreToolUse command becomes `REPOGRAPH_HOOK_INTERCEPT=0 node …` with a comment-free README line in `agent/README.md` saying what was measured. Then:

```bash
cd "$W" && node --test agent/ 2>&1 | tail -1
git -C "$W" add agent/ bench/agent/overlays bench/history/runs.jsonl
git -C "$W" commit -F - <<'MSG'
test(bench): the planned surface against today's — sonnet twice, one opus reading

Overlay C is the installer's output on a CLAUDE.md without the repograph sections. Five rows recorded
beside the baseline; the priming and interception verdicts are read by the rules written before the
runs and applied to the hook's default.
MSG
```

---

### Task 4: The scout — a cheap model on the graph

**Files:**
- Create: `agent/agents/repo-scout.md`, `bench/agent/overlays/C-scout/` (generated: C plus the agent)
- Modify: `agent/install.mjs` (installs the agent under `--claude`), `agent/install.test.mjs`, `agent/stanza.md` (one line, only if F3 passes), `bench/history/runs.jsonl`

- [ ] **Step 1: The agent file**

```markdown
---
name: repo-scout
description: Answers "where is / who calls / how does A reach B / what does my diff touch" from the repograph knowledge graph at zero model cost on the parent's side. Send it a question about this repository before reading files yourself; it returns id  path:line lines and one clause each, nothing else.
tools: Bash, Read
model: haiku
maxTurns: 6
---

You answer one question about this repository from its repograph graph, and you return only what the graph proved.

1. Run `repograph --no-dense ask --stale <words|ID|Symbol>` first — an exact id or symbol name answers fastest. For callers: `repograph --no-dense impact --stale --depth 1 <Symbol>`. For a chain: `repograph --no-dense trace --stale A B`. For the uncommitted diff: `repograph --no-dense changes --depth 1`.
2. Read a file only at a `path:line` the graph printed, and only the lines around it, when the question cannot be answered from the graph's line alone.
3. Reply in at most 300 tokens: one line per finding, `id  path:line — one clause`; end with the risk line when `impact` or `changes` printed one. If the graph found nothing, say `graph: nothing for <words>` and stop — the parent decides what to do next. Never summarise the codebase, never suggest edits.
```

- [ ] **Step 2: The rule, then the runs**

Append to `$M/task4.txt` first:

> F3 — scout. Sonnet parent, overlay C-scout (C plus the agent) against C, two runs: `cost_per_hit(C-scout) ≤ cost_per_hit(C)` and `hits(C-scout) ≥ hits(C)`. Pass → the stanza gains one line ("send `repo-scout` for a who-calls or trace question") and the installer writes the agent by default. Fail → the file ships (it costs nothing uninvoked), the stanza does not mention it, the README states the measured pair. Also recorded: how many tasks invoked the scout at all (`Agent` tool_use with `subagent_type: repo-scout` in the stream), and `modelUsage`'s haiku/sonnet split per task.

```bash
cd "$W" && O=bench/agent/overlays; rm -rf "$O/C-scout" && cp -R "$O/C" "$O/C-scout" && mkdir -p "$O/C-scout/agents" && cp agent/agents/repo-scout.md "$O/C-scout/agents/"
for r in 1 2; do bench/agent/run.sh C-scout sonnet | tail -2; done
for d in "$M"/runs/*-C-scout-sonnet; do python3 bench/history/track.py agent "$d/summary.json" --note "agent-surface, scout"; done
```

`score.py` needs one addition for this task: `scouted` = count of `Agent` tool_use blocks whose input names `repo-scout`, and `per_model` = the raw `modelUsage` map kept per task. Expected: two rows; `$M/task4.txt` carries the F3 arithmetic. Cost ceiling $7.20.

- [ ] **Step 3: Installer, stanza line if earned, commit**

`agent/install.mjs --claude` copies `agent/agents/repo-scout.md` to `.claude/agents/repo-scout.md` (test: present after install, `unchanged` on the second run). If F3 passed, `agent/stanza.md`'s fourth bullet gains `; for a who-calls or trace question, send the \`repo-scout\` subagent`, and Step 3 of Task 2's size record is updated.

```bash
cd "$W" && node --test agent/ 2>&1 | tail -1
git -C "$W" add agent/ bench/agent bench/history/runs.jsonl
git -C "$W" commit -F - <<'MSG'
feat(agent): a haiku scout that runs the graph and returns three hundred tokens

Measured on the twelve tasks with a sonnet parent, twice, against the same surface without it; the
stanza recommends it only if cost per hit did not rise.
MSG
```

---

### Task 5: beauty-crm, Codex, and the machine

**Files (in Max's beauty-crm repository, `/Users/max/Documents/projects/beauty-crm`, on a branch Max names; nothing here is committed by the implementer):**
- Modify: `.claude/CLAUDE.md`, `.claude/settings.json`, `.gitignore` (one line if the symlink direction needs it)
- Create: `AGENTS.md`, `.codex/hooks.json`, `.codex/hooks/repograph-hook.mjs`, `.claude/hooks/repograph-hook.mjs`, `.agents/skills/repo-query/SKILL.md`, `.claude/agents/repo-scout.md`
- Replace: `.claude/skills/repo-query/` (directory) → symlink to `../../.agents/skills/repo-query`
- Remove: `.claude/hooks/repograph-notice.mjs` (the installer does this)
- Files in this repository: `README.md` gains the Codex command line (Task 9 writes the section; this task verifies the line).

Background the brief cannot know: beauty-crm's `.gitignore` un-ignores `.claude/skills/` for the symlinks that point *into* `.agents/skills`; the reversed direction is the same shape (a symlink under `.claude/skills/`), so it needs no new rule — Step 3 checks. `repograph` there is `pnpm exec repograph` (`node_modules/.bin`), which the hook's `binary()` resolves without the `pnpm` shim.

- [ ] **Step 1: Dry run, then install for Claude**

```bash
cd /Users/max/Documents/projects/beauty-crm && git status --short | head -3
node "$W/agent/install.mjs" --claude --command "pnpm exec repograph" --dry-run
node "$W/agent/install.mjs" --claude --command "pnpm exec repograph"
git status --short
```

Expected: the dry run lists every path with its verb and writes nothing; the install writes them; `.claude/CLAUDE.md` now has the stanza between markers — **and the implementer then deletes, by hand, the sections the stanza replaces**: everything after the H1 `# beauty-crm — правила работы с графом знаний` up to and including `## Что граф стоит`, and the `## Хуки в репозитории` table's `repograph-notice.mjs` row (the other two rows stay; add a row for `repograph-hook.mjs` naming its four events). What stays of the file: the H1, the stanza, `## Хуки в репозитории`, `## Автосжатие контекста`, `## Скиллы`, `## Чекбоксы вех`, `## Закрытие задачи — трейлером`, `## Парковка CI`, `## Как работать в этом репозитории`. The measurement paragraphs (graphify's numbers, the import-legacy rollback, the model comparison) move to a new `docs/tooling/repograph.md` in beauty-crm with a one-line pointer from the stanza's last bullet, so nothing Max wrote is lost. `wc -c .claude/CLAUDE.md` — record it; the target is under 17 kB.

- [ ] **Step 2: Codex**

```bash
node "$W/agent/install.mjs" --codex --command "pnpm exec repograph"
cat .codex/hooks.json | python3 -c "import json,sys; d=json.load(sys.stdin); print({k:[e['matcher'] for e in v] for k,v in d['hooks'].items()})"
head -5 AGENTS.md; ls -la .claude/skills/repo-query .agents/skills/repo-query/SKILL.md
```

Expected: matchers `Bash` / `apply_patch|Edit|Write` / `startup|resume|clear|compact` / `.*`; `AGENTS.md` begins with the stanza and ends with one line pointing at `.claude/CLAUDE.md` for the non-repograph rules; the skill is one file under `.agents/skills/` with `.claude/skills/repo-query` a symlink to it.

Then, in a Codex session in beauty-crm (interactive — Codex trusts a hook by hash from `/hooks`): trust the two new hook entries; run `rg -n "cancellation policy"` through the agent once and confirm an injection appears (or, if F2 turned interception off, that nothing appears and the session brief did). Record in `$M/task5.txt` which of the four Codex events fired — `SubagentStart` in particular, since Codex's page lists it and nothing here has exercised it.

- [ ] **Step 3: The Codex reranker line, one word round trip**

```bash
printf 'Reply with the single word pong and nothing else.\n' | sh -c 'f=$(mktemp); codex exec - --ephemeral --skip-git-repo-check -s read-only -m gpt-5.6-sol -o "$f" >/dev/null 2>&1; cat "$f"; rm -f "$f"'
```

Expected: `pong` on stdout alone. This is the command line the README documents for `rerank_command`/`enrich_command` (Decision §4); if `codex exec` needs `--json` or another flag to keep stdout clean, the line is corrected here and only here, and the README carries the corrected one.

- [ ] **Step 4: Global hooks (Max, 2026-09-07: global and per-repository)**

`node "$W/agent/install.mjs" --claude --global` and `--codex --global`; the user-level entry fires in every repository with a `.repograph/` store and is silent elsewhere (the hook's `built()` guard), beside the GitNexus entries, which stay. The Codex global fragment is the same three-event `codex.json`. Verify: open a session in a repository without a store and confirm no hook output; in beauty-crm confirm one brief and no duplicate (project and global entries both fire — the script dedups on `session_id`, tested in Task 2).

- [ ] **Step 5: See it once, and hand over**

Start a Claude Code session in beauty-crm: the brief appears at start; run one `rg` through the agent; edit one line inside `DatabaseService.withTenant` and revert it — the CRITICAL risk line appears once. `$M/task5.txt` records what was seen. The branch in beauty-crm is left for Max to review and commit; the implementer reports the file list.

---

### Task 6 (Rust): `repograph prime` and `repograph install-agent`

**Files:**
- Create: `src/prime.rs`, `src/install.rs`
- Modify: `src/main.rs` (two `Cmd` arms, `mod` lines), `agent/hooks/repograph-hook.mjs` (`brief()` calls `prime` when the binary has it, `verify` otherwise), `agent/hooks/repograph-hook.test.mjs`, `README.md` (Task 9 writes the prose; this task adds the two command lines to the status table)

**Interfaces:**
- `prime`: prints ≤ 700 B: line 1 `repograph <version> · .repograph/ · <N> nodes: <kind counts, descending> · <E> edges · questions <covered>/<eligible> · vectors <model or none>`; line 2 `id families: <families with ≥ 1 declared node, in config order> · milestones <list>`; lines 3–4 the fixed command and rule lines from the draft. Reads the store as it stands (no refresh, like `verify`), exits 1 with `graph is empty — run repograph build` on an empty graph.
- `install-agent --claude|--codex [--global] [--command <name>] [--dry-run]`: `include_str!` over `agent/hooks/repograph-hook.mjs`, `agent/hooks/claude.json`, `agent/hooks/codex.json`, `agent/skills/repo-query/SKILL.md`, `agent/agents/repo-scout.md`, `agent/stanza.md`; the same tree, the same merge rules, the same verbs as `agent/install.mjs`.

- [ ] **Step 1: Failing tests**

`src/prime.rs` `#[cfg(test)]`: a two-requirement temp repo built with `run_update` → `render(&graph, &questions, &dense_model, &cfg)` contains `2 nodes`, `Requirement 2`, `questions 0/2`, `vectors none`, `id families: FR-PAY`, the command line, and is ≤ 700 bytes; a graph with 20 kinds still fits (kinds beyond the eighth are folded into `+N more`). `src/install.rs` `#[cfg(test)]`: temp root, `--claude` writes the six paths, second run reports `unchanged` for all and changes no mtime; an existing `settings.json` with a foreign `PreToolUse` entry keeps it; `--codex` writes `.codex/hooks.json` with the `Bash` matcher; `--dry-run` writes nothing; **and** the tree `install-agent --claude` writes is byte-equal, file by file, to what `node agent/install.mjs --claude` writes on an identical root (the test shells out to `node` and is `#[ignore]`d where `node` is absent, with the reason in the attribute).

Run: `cargo test --release prime:: install::` → compile errors.

- [ ] **Step 2: Implement**

`src/prime.rs`: `pub fn run(repo: &Path) -> anyhow::Result<()>` — `Store::load`, `enrich::Questions::load`, the eligible/covered counts the way `bench.rs` computes them (extract that computation into `enrich::coverage(&graph, &questions) -> (usize, usize)` if it is not already a function; `bench.rs` then calls it), the vectors' recorded model via the `DenseIndex` reader `bench` uses for its `model=` field, the families from `config::Config::load` filtered to those with a declared node. `src/install.rs`: `const HOOK: &str = include_str!("../agent/hooks/repograph-hook.mjs");` and the five others; `serde_json::Value` for the settings merge (unknown keys preserved); `--command` substitution into the stanza; the symlink only on unix (`#[cfg(unix)]`; on Windows the skill is copied to both places, and the README's Windows line says so).

`main.rs`: `Cmd::Prime`, `Cmd::InstallAgent { claude: bool, codex: bool, global: bool, command: String, dry_run: bool }` (clap: exactly one of `claude`/`codex` required via `required = true` on an `ArgGroup`).

The hook's `brief()`: try `prime`; on a non-zero exit (an older binary without the subcommand) fall back to the two `verify` lines. Test both branches with the fake binary.

- [ ] **Step 3: Green, sizes, commit**

```bash
cd "$W" && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && cargo test --release 2>&1 | grep 'test result'
target/release/repograph --repo /Users/max/bench/beauty-crm-502e8a6d prime | tee "$M/task6-prime.txt" | wc -c
node --test agent/ 2>&1 | tail -1
git -C "$W" add src/prime.rs src/install.rs src/main.rs src/enrich.rs src/bench.rs agent/hooks/ README.md
git -C "$W" commit -F - <<'MSG'
feat(cli): prime prints a repository brief; install-agent writes the agent surface from the binary

The brief is what the SessionStart hook injects: counts, enrichment, the vectors' model, the families
and the five commands, from the store as it stands. install-agent embeds the agent/ files so npm and
cargo users get the same hook, skill, scout and stanza the repository ships, byte for byte.
MSG
```

Expected: unit tests +N, clippy clean; `prime` on the fixture ≤ 700 B (record the number: it replaces the 530 B draft in the results document).

---

### Task 7 (Rust): `impact --limit`, `changes --brief`

**Files:**
- Modify: `src/impact.rs`, `src/changes.rs`, `src/main.rs`, `agent/stanza.md` and `agent/skills/repo-query/SKILL.md` (the recommended forms), `agent/hooks/repograph-hook.mjs` (`riskLine` unchanged — it reads JSON)

**Interfaces:**
- `impact --limit N` (default `0` = unlimited, byte-identical output): each layer prints its first `N` rows in the order already rendered, then `  … and K more in F files` where `F` counts the distinct files among the omitted rows; `importers` likewise `importers (M): a, b, c, … and K more`; the risk line unchanged. `--json` ignores `--limit` (a script wants all rows).
- `changes --brief`: the `changed:` header stays; the per-symbol list becomes one line per file `  <file>: N symbols` (files without an indexed symbol keep their `not indexed` line); `affected` prints depth-1 rows only, up to 12, then the `… and K more` tail; the risk line unchanged. `--json` unaffected.

- [ ] **Step 1: The rule and the failing tests**

`$M/task7.txt` first: *the capped forms read ≤ 25 % of the uncapped bytes on `impact --limit 12 DatabaseService` (3,852 tok at depth 1) and `changes --brief --base a7acc0f4~1` (25,058 tok) on the fixture; the uncapped output is byte-identical before and after; `bench --cases bench/blast.jsonl` reads the same counts.*

Tests in `src/impact.rs`: `limit_keeps_the_first_rows_and_says_how_many_it_dropped` on the existing `graph()` fixture (a layer of 3 with `--limit 2` → two rows and `… and 1 more in 1 file`); `limit_zero_is_byte_identical` (render with 0 equals today's render). In `src/changes.rs`: `brief_collapses_the_changed_list_per_file_and_keeps_depth_one` on the existing `DIFF` fixture. Run → compile errors on the new parameters.

- [ ] **Step 2: Implement, measure, commit**

`render(graph, imp, direction, limit)` and `render_brief(graph, r)`; `main.rs` passes the flags. Then:

```bash
cd "$W" && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && cargo test --release 2>&1 | grep 'test result'
F=/Users/max/bench/beauty-crm-502e8a6d; B=target/release/repograph
for l in 0 12; do printf 'impact DatabaseService --limit %s  ' $l; $B --repo $F --no-dense impact --stale --depth 1 --limit $l DatabaseService | wc -c; done
printf 'changes a7acc0f4~1 plain  '; $B --repo $F --no-dense changes --stale --depth 1 --base a7acc0f4~1 | wc -c
printf 'changes a7acc0f4~1 brief  '; $B --repo $F --no-dense changes --stale --depth 1 --base a7acc0f4~1 --brief | wc -c
for s in DatabaseService asGrosze StaffService; do cmp <($B --repo $F --no-dense impact --stale $s) <("$M/repograph-base" --repo $F --no-dense impact --stale $s) && echo "impact $s uncapped: byte-identical"; done
cmp <($B --repo $F --no-dense changes --stale --base cbc931ba~1) <("$M/repograph-base" --repo $F --no-dense changes --stale --base cbc931ba~1) && echo "changes uncapped: byte-identical"
$B --repo $F --no-dense bench --cases bench/blast.jsonl 2>&1 | tail -1
```

Expected: `--limit 0` = 15,408 B (the planning number); `--limit 12` and `--brief` each ≤ 25 % of their uncapped bytes — the two ratios go into `$M/task7.txt` and the README; byte-identical uncapped; the blast suite's summary unchanged. If a ratio misses 25 %, the limit is not lowered to meet it: the plan records the ratio the cap achieved and the stanza's recommendation carries the measured number.

`agent/stanza.md`: `impact --depth 1 --limit 12 <Symbol>` and `changes --brief --base main`; the skill's route table too; the hook's tests still pass (the interceptor does not call these).

```bash
git -C "$W" add src/impact.rs src/changes.rs src/main.rs agent/
git -C "$W" commit -F - <<'MSG'
feat(impact): --limit caps a layer with a counted tail; changes --brief folds the diff the agent wrote

A hub's sixty-one callers are four thousand tokens an agent needs three of; a wide diff's changed list is
five hundred lines the agent produced itself. Default output is byte for byte what it was.
MSG
```

---

### Task 8 (Rust): `ask --escalate`, priced offline before a token is spent

**Files:**
- Create: `bench/escalate.py`
- Modify: `src/query.rs` (the escalation predicate beside `admits`), `src/ask.rs` (`Request.escalate`; escalation runs the `--rerank` path only when the predicate fires), `src/main.rs` (`--escalate`, conflicts with `--rerank`/`--rerank-local`), `src/serve.rs` (the `Hello` carries the request; a resident process honours the flag the same way), `README.md` (Task 9), `agent/skills/repo-query/SKILL.md` (one line, only on pass)

**Interfaces:**
- Predicate `escalates(exact_hits, cov_passages, cov_questions, c_e) -> bool`: no exact hit, and both coverages below `c_e` (a list that has no index — a raw store's questions list — reads coverage 0 and does not veto). `c_e` is a constant in `query.rs` with the derivation in its doc comment, like `QUESTIONS_GATE`.
- `bench/escalate.py crossover <ho-dump.json>…` prints, for `c_e` from 0.30 to 0.95 in 0.05 steps: escalated count, of which plain-`ask` misses (precision), misses not escalated (recall); `score <rec-dump.json> --c <c_e>` lists the escalated recorded cases. The replay reads `bm25_passages`/`bm25_questions` bests and `attainable_*` from the dumps the way `bench/admission.py` does, and `ask`'s recorded answer for the miss verdict.

- [ ] **Step 1: The rules, committed first**

`$M/task8.txt`, before any replay:

> E1 — the constant. `c_e` = the largest step at which precision ≥ 0.5 on the 400 held-out questions in the **lexical** arm (`base-ho-lexical.json`, the arm every hook and most agents run); read the dense arm too and record it, the constant does not move for it. If no step reaches 0.5, the flag does not ship and the results document says what precision the best step reached.
> E2 — recall. At that `c_e`, at least half of the held-out misses are escalated; below that the flag would spend on the easy misses and skip the hard ones — recorded, not gating.
> E3 — the live subset. The escalated recorded cases at `c_e` (both arms, deduplicated) are listed with their count **before** the model runs; ceiling = count × 19.2k input tokens on sonnet; `bench --escalate` runs once per arm; rescued = escalated misses that the reranked answer hits; the README table carries escalations / rescued / tokens per rescued case from the run's own `total` line. The flag ships if rescued ≥ half of the escalated misses; the skill recommends it only then.
> E4 — nothing else moves. `bench` in both arms without the flag reads the four counts it reads today (the predicate is not on the plain path).

- [ ] **Step 2: The replay**

```bash
cd "$W" && D=/Users/max/bench/residue-seat-register-2026-09-06
python3 bench/escalate.py crossover "$D/base-ho-lexical.json" | tee "$M/task8-crossover-lexical.txt"
python3 bench/escalate.py crossover "$D/base-ho-dense.json" | tee "$M/task8-crossover-dense.txt"
```

Expected: two tables; `c_e` read from the lexical one by E1; the dense one recorded. If the dumps predate the residue change (`attainable` charging absent terms), regenerate both held-out dumps on the current binary first (`repograph --repo $F --no-dense dump --queries $D/heldout-400-syn.jsonl --out $M/ho-lexical.json`, and without `--no-dense`; `dump` reads the store and writes nothing to it) — `bench/escalate.py check` verifies the replay against each dump's own recorded `ask` answer the way `admission.py check` does, and refuses to go on otherwise.

- [ ] **Step 3: The subset, then the binary, then the live run**

```bash
python3 bench/escalate.py score "$D/base-rec-lexical.json" --c <c_e> | tee "$M/task8-subset-lexical.txt" | tail -1
python3 bench/escalate.py score "$D/base-rec-dense.json" --c <c_e> | tee "$M/task8-subset-dense.txt" | tail -1
```

Write the two counts and the ceiling into `$M/task8.txt`. Then the Rust: the predicate with its tests (`escalates_only_without_an_exact_hit_and_below_both_coverages`), `Request.escalate`, the branch in `Context::answer` that sets `rerank` when the predicate fires (the fusion has already computed both coverages for the admission — expose them from `query::ask`'s lexical stage rather than recomputing), `bench --escalate` reporting `escalated N  rescued M` on its summary line, `serve` passing the flag through. `cargo clippy`, `cargo test --release` green; `bench` without the flag in both arms unchanged (E4):

```bash
for a in "" "--no-dense"; do target/release/repograph --repo $F $a bench 2>&1 | tail -1; done
target/release/repograph --repo $F --no-dense bench --escalate 2>&1 | tail -1 | tee -a "$M/task8-live.txt"
target/release/repograph --repo $F bench --escalate 2>&1 | tail -1 | tee -a "$M/task8-live.txt"
```

Expected: the two plain lines equal today's (`40/40 15/30 12/12 p90 220` dense, `39/40 14/30 12/12 p90 215` lexical); the two `--escalate` lines carry `escalated N  rescued M` with `N` equal to Step 3's count. `bench` reads the store; `--escalate` inside it runs the configured `rerank_command` on the escalated questions only — sonnet, `claude -p`, the ceiling stated. E3 decides. Commit either way, the verdict in the body:

```bash
git -C "$W" add src/query.rs src/ask.rs src/main.rs src/serve.rs src/bench.rs bench/escalate.py
git -C "$W" commit -F - <<'MSG'
feat(ask): --escalate spends the reranker only where both lexical coverages say the answer is weak

The constant is the crossover on the 400 held-out questions under a rule written first: the largest
value at which half of what escalates is a real miss. Priced offline from the recorded dumps, then run
live on the escalated recorded cases alone.
MSG
```

---

### Task 9: The story in words, the numbers in their document, the history, the cleanup

**Files:**
- Create: `docs/bench/2026-09-0X-agent-surface-results.md`
- Modify: `README.md`, `docs/bench/next-version-gaps.md`, `bench/history/README.md`, `agent/README.md`
- Remove (outside the repo): `$M/wt` (the agents' worktree)

- [ ] **Step 1: The results document**

Sections, each with the file under `$M` it reads from: what a session paid (the ledger's priming bytes, then `$M/task2.txt`'s measured brief/rule/injection bytes and `$M/task6-prime.txt`); the transcript census (the two counts tables, with the caveat that the sessions mix purposes and that auto mode routes searches through Bash); the harness — tasks, configurations, the twelve rows per model with hits/tokens/cost/asked/grepped, the F1/F2/F3 verdicts in the words of their rules, the actual dollars per run; the output-shape table and the Task 7 ratios; the escalate crossover tables, subset counts and the live line; what was rejected and why (deny-the-grep, `ask --brief`, MCP with its 10,897-token schema and the deferral caveat, the `--json` forms for agents); what is not measured (Codex runs; opus/fable parents; more than 12 tasks). The GitNexus/graphify history paragraphs that leave the skill land here under *The two neighbours*, verbatim.

- [ ] **Step 2: README**

A new section **For agents** after *Asking a resident process*: the stanza as it ships (fenced), `repograph install-agent --claude|--codex [--global]` with what it writes, the four hook events in one table with the token each costs (measured), the scout in two sentences with its verdict, the Codex `rerank_command` line, `prime`'s output as a real run on the fixture, `impact --limit`/`changes --brief` with their ratios, `--escalate` with its table or its rejection line. The status table gains `prime`, `install-agent`, `ask --escalate` rows. The *Spending tokens on purpose* section links the escalate table. `docs/bench/next-version-gaps.md`: a G18 entry — *the agent surface is measured on 12 tasks, one corpus, two models; Codex unmeasured* — with the harness as the instrument.

- [ ] **Step 3: History, cleanup, commit**

```bash
cd "$W" && python3 bench/history/track.py report | tail -20
git -C /Users/max/Documents/projects/beauty-crm worktree remove "$M/wt" --force && git -C /Users/max/Documents/projects/beauty-crm worktree prune
ls /Users/max/bench/beauty-crm-502e8a6d/.repograph/graph.json && git -C /Users/max/bench/beauty-crm-502e8a6d status --porcelain -- ':(exclude)graphify-out' | wc -l; echo "(expected 0: the fixture was never written)"
git -C "$W" add README.md docs/bench/ bench/history/README.md agent/README.md
git -C "$W" commit -F - <<'MSG'
docs: the agent surface — what a session pays, what the harness read, what shipped and what did not
MSG
```

`$M` itself stays (the raw streams are the evidence; `bench/history/README.md` names the directory the way the other measurement directories are named).

---

### Task 10: The pull request

**Files:** none.

```bash
gh auth switch --user devmaxxx && gh pr create --repo devmaxxx/repograph --base main --head feat/agent-surface \
  --title "feat(agent): the agent surface — one hook, one stanza, one installer, measured on twelve agent tasks" \
  --body-file - <<'BODY'
## What

The text that tells Claude Code and Codex to use repograph shrinks from ~3,850–6,150 tokens a session to
the measured figure in the results document; a `SessionStart` brief comes back after every compaction; a
gated `PreToolUse` interceptor answers Bash/Grep/Glob searches with the graph's top three lines (shipped
on or off by the harness's verdict); a `PostToolUse` hook injects one risk line on a MEDIUM+ edit;
`SubagentStart` delivers the rule that prose delivered to 19 of 146 subagents. `agent/` is the one
source for both harnesses; `install-agent` embeds it in the binary. `prime`, `impact --limit`,
`changes --brief`, `ask --escalate` are the Rust half, each with its measurement.

## Why these and not others

Argued in `docs/superpowers/plans/2026-09-06-agent-surface.md` (Decision). In one line each: Grep/Glob
are 0 of 3,317 tool calls in the recorded sessions, so the surface is Bash; the notice hook fired in 2
of 38 sessions; an MCP schema costs up to 10,897 tokens a session, deferred or not, for nothing a Bash call lacks; `impact` on a
hub is 3,852 tokens at depth 1 and the agent needs three of them.

## Evidence

`docs/bench/2026-09-0X-agent-surface-results.md`; raw streams under `/Users/max/bench/agent-surface-2026-09-07/`;
history rows `agent:*` in `bench/history/runs.jsonl`.

## Not done here

beauty-crm's own install (its branch, Max's commit); Codex runs through the harness; the version bump.
BODY
```

Expected: a PR URL. Report DONE with it and the paths under `$M`.

---

## Placeholder check, done when the plan was written

Every `$M/...` path is a file a step creates before another reads it. The twelve tasks' expectations were read from the fixture during planning except the two marked *pin*, which Task 1 Step 2 reads before the first run. The two test counts (`444`/`12`) are the 2026-09-06 plans' at `868f4c1` and are confirmed or replaced by Task 0 Step 1. The `modelUsage` field spellings come from the cost-tracking reference and are confirmed by Task 0 Step 3 before `score.py` depends on them. The `SubagentStart` output contract is the one documentation claim the fetches could not quote, and Task 2 Step 5 settles it in a session before the fragment ships. Every floor (F1–F3, E1–E4, the 25 % cap) is written into a `$M/taskN.txt` before its numbers are read. The hook code in Task 2 is written against the payload fields the notice hook already consumes and the output contract GitNexus's hook already uses in both harnesses; the Rust in Tasks 6–8 against `src/main.rs`, `src/impact.rs`, `src/changes.rs`, `src/query.rs`, `src/ask.rs`, `src/bench.rs`, `src/dump.rs` as read at `e0b2f5c`. No step says "similar to" another. Nothing writes the fixture; Task 9 Step 3 checks.

## Decisions (Max, 2026-09-07)

The five questions the plan carried were put to Max and answered; the tasks above are edited to match.

1. **Hook install: global and per-repository.** `install-agent --global` for both harnesses, beside the GitNexus entries; beauty-crm also gets the project install. Task 5 Step 4 is unconditional.
2. **Codex gets the brief, the skill and the risk line — no interceptor.** `agent/hooks/codex.json` carries `SessionStart`, `PostToolUse`, `SubagentStart` and no `PreToolUse` entry; the interceptor's F2 verdict is read on Claude and not carried to Codex.
3. **`--escalate` spends only behind its flag.** Opt-in like `--rerank`; the skill may recommend it on a miss (E3 decides the wording); `ask` never escalates on its own.
4. **The Rust half ships in the current version line.** Tasks 6–8 land as 0.5.x with the rest; no minor bump in this plan; 0.6.0 stays the languages' release.
5. **The C family runs twice — on sonnet; haiku leaves the parent seat; opus reads C once.** Max, 2026-09-07: haiku as a parent is not a workload he runs, so the harness parent is sonnet on every overlay (F1–F3 as written, two runs), haiku stays the scout's model, and the shipped configuration C is run once on opus at a cap measured on one task first (Task 3 Step 3b) — a reading beside sonnet's rows, not a verdict. Sonnet ceiling $28.80 plus 12 × the opus cap.
