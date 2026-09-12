# The agent surface — design

> **Superseded, never executed.** This is the 2026-09-06 draft of the agent surface as an MCP server in the
> binary. It was replaced by [`2026-09-06-agent-surface.md`](../plans/2026-09-06-agent-surface.md), merged in #24, which decided that no MCP
> server is built. Nothing below is implemented; it is kept as the record of the alternative that was weighed.

repograph exists so that a coding agent gets a precise answer in a tenth of the characters a
grep-and-read loop spends. Today it is reachable as a CLI shaped for a person. This note decides
what the surface for an agent is, what it costs before a single call, who keeps the store fresh
under it, what an agent is told when the graph has no answer, and — written here before any
number is read — what would be measured to know the surface is worth its cost. The plan that
argues from this note is [`../plans/2026-09-06-agent-integration.md`](../plans/2026-09-06-agent-integration.md).

Nothing in this note is implemented. Every number is one an earlier document or a read of the
fixture measured, and names its source; no number below is a prediction.

## What an agent has today

- **In repograph itself:** nothing. `watch`'s doc comment names "editors, MCP servers" as the
  readers it exists to supply (`src/main.rs:66`); nothing implements one. Three readers take
  `--json` (`ask`, `impact`, `changes`); `explain` and `trace` do not. Every reader refreshes
  the store against the tree before answering, except `explain` and `verify`, which read the
  store as it stands.
- **In `beauty-crm`, by hand:** a `.claude/CLAUDE.md` section with the two rules (impact before
  editing a symbol, `changes` before committing), a `repo-query` skill that routes a question
  to `repograph`, `rg` or `ast-grep` by cost, and a `PreToolUse` hook on `Read|Grep|Glob` that
  prints a notice once per session and a one-line reminder every fortieth call
  (`.claude/hooks/repograph-notice.mjs`). Every one of those calls the binary through
  `Bash(pnpm exec repograph …)`. It works, and it lives in one repository.
- **Before that, GitNexus:** a user-level MCP server of seventeen tools and two hooks that ran
  `augment` on every `Grep`/`Glob`/`Bash` with a 7 s budget. Of the seventeen, `beauty-crm`'s
  rules used five (`impact`, `detect_changes`, `context`, `trace`, `query`,
  [`2026-09-03-replace-gitnexus.md`](2026-09-03-replace-gitnexus.md)). The five schemas this
  session loaded run to roughly 16 KB of description and parameters — about 4,000 tokens for
  five tools of seventeen, an estimate from the loaded text — and the hook's augmentation on
  every grep is what the runbook still warns about. That surface is the yardstick for what an
  agent expects, not a specification.

## The candidates, weighed

| | reaches Claude Code | reaches Codex and others | context cost before a call | who owns the resident process | needs from the binary |
|---|---|---|---|---|---|
| MCP server in the binary | yes, via `.mcp.json` or a plugin | yes — `codex mcp add`, `.cursor/mcp.json`, one config line each | the tool schemas, once per session (billed again per request, cached) | the harness: starts it with the session, closes stdin to stop it | a stdio JSON-RPC loop over `ask::Context` — `serve` with another transport |
| Claude Code plugin (skills, commands, hooks) | yes | no — a second artifact per harness | a skill's one-line description per request; the body when invoked | nobody | nothing; it drives the CLI through `Bash` |
| `SKILL.md` alone | yes | Codex reads `AGENTS.md`, not skills | as above | nobody | nothing |
| hooks alone | yes | Codex has hooks of a different shape | the notice, once; the augmentation, per call | nobody | nothing |
| a machine-readable CLI contract | any harness with a shell | any harness with a shell | zero; the agent must be told the commands exist | nobody, unless someone runs `serve` | `--json` on every reader, a `status` command, stable exit codes |

Two facts from the repository decide it. First, the answers are already agent-shaped: five
lines of `ID  path:line  headline`, 197 tokens median, and the same bytes whether a person or a
script asked (README, Measured). No surface improves the answer; every surface is a way of
being found and a way of being kept fresh. Second, the resident process is where the speed is —
6.8 ms a lexical answer against 55 ms, 66 ms fused against 326 (README, Asking a resident
process) — and in the Bash design nobody owns it: `serve` is opt-in, and the beauty-crm setup
never starts one. In MCP the harness owns the process by construction.

## Decision

**The surface is an MCP server in the binary, `repograph mcp`, exposing the five readers as
five tools; a Claude Code plugin packages it with one skill, one setup command and one reminder
hook; and underneath both, the CLI becomes a machine-readable contract — `--json` on every
reader and a `status` command — so that any harness with a shell reaches the same answers
without a second implementation.** Claude Code is first; Codex is reached by the same server
through its own one-line config and an `AGENTS.md` paragraph, and is smoke-tested in the plan.

What is given up:

- About a thousand tokens of context per session for the five schemas and the server's
  instructions, before any call — five answers' worth. The Bash-and-skill design costs a
  fortieth of that. The ceiling is a test, not a hope (below).
- One resident process per harness session rather than one per store. Two sessions on one
  worktree hold the embedding model twice unless someone runs `serve`, which the MCP server
  uses when it answers; the plan states the memory and does not hide it.
- The GitNexus shapes that were never used here: `cypher`, `rename`, `route_map`, `group_*`,
  `pdg_query`. An agent that expects them finds five tools and a skill that says why.

What would change the choice: the measurement in the last section. If an agent driving the CLI
through `Bash` with the skill finishes the task suite in no more tokens than one driving the MCP
tools, the server stays in the binary as the portable surface and the plugin ships without its
`.mcp.json`, the skill carrying the CLI. The rule that decides is written below and not
re-read after the numbers.

## The seven questions

### 1. Which answers are worth a tool call

An agent that can call a tool calls it on everything, so what a tool costs when it is called
on the wrong thing is the first number. Read on the fixture at `502e8a6d`, `--stale`,
bytes / 4:

| command | narrow target | hub target | measured on |
|---|---|---|---|
| `impact` | `StaffService` 225 tokens, `TenantContextInterceptor` 188 | `DatabaseService` **8,860** (156 lines), `ProblemException` 4,097 | `bench/blast.jsonl` targets |
| `explain` | `StaffService` 136, `FR-PAY-22` 194 | `DatabaseService` 251, a `File` node 219 | the same store |
| `changes` | a one-file edit ≈ 200 (README) | `--base HEAD~30` **94,824** (3,190 lines) | the same store |
| `ask` | 197 median, 216 p90 (README) | `--seeds 8` 297 median (ADR-001) | the 82 cases |
| `trace` | ≤ 7 lines | — | — |

| becomes a tool | stays CLI-only | why |
|---|---|---|
| `ask` (words, `seeds` ≤ 8, `bodies`, `stale`) | `ask --rerank`, `--rerank-local` | ≈19,200 model tokens and ~4 s a question, or 17.9 s a question; an agent would pay it on every question. The skill says when a person reaches for it. |
| `explain` (`limit` edges, default 40) | — | cheap everywhere measured; the cap is for the `File` hub the README already warns is never expanded to |
| `impact` (`depth` ≤ 6, `down`, `limit` lines a layer, default 25) | — | 8,860 tokens on a hub is forty answers; the risk line is computed on the whole walk and printed whatever the cap, so the label an agent acts on is never truncated |
| `trace` (`depth` ≤ 12) | — | a chain or "none"; bounded by construction |
| `changes` (`base`, `depth` ≤ 4, `limit`) | — | 94,824 tokens for a branch is the largest trap in the binary; capped the same way, the counts and the risk line intact |
| — | `build`, `update`, `enrich`, `embed`, `serve`, `watch`, `import-legacy` | writers: minutes to hours, dollars, or a process to own — a person's decision, made in a shell |
| — | `bench`, `dump`, `verify` | measurement and a census; nothing an agent needs mid-task |

The CLI keeps printing everything; the caps are the tools'. A tool answer is the CLI's bytes,
with the cap's `… N more` line where it applied and the notices `ask` prints on stderr ahead
of it, because MCP has no stderr that reaches the model.

### 2. Staleness

- **Every tool call refreshes the store against the tree first**, exactly as `ask` does (walk
  ~10 ms on a quiet tree, ~20–40 ms per edited file, README "Keeping it fresh"), unless the
  call says `stale: true`. Between calls the server polls the way `serve` does, and adopts
  what another process wrote, so a resident graph is never older than what a fresh `ask` would
  load. The refresh is reused, not re-implemented: `serve`'s per-request refresh becomes a
  shared function.
- **What an agent sees when the tree had moved:** the answer's first line is the existing
  notice, `refresh: 3 changed, 1 removed`. When the store cannot be written, `refresh: skipped
  (…)` and the answer that follows may be behind — the same contract `ask` has.
- **A cold repository:** every tool returns the same one-line refusal naming `repograph
  build`, with `isError: true`, and writes nothing. **No tool builds a store.** A lexical build
  is ~1 s per 800 files, but the dense index is ~103 s on the small model and ~2,680 s on the
  default (README, Embeddings) — a person's choice of model and of minutes, made in the shell,
  and the plugin's `/repograph:setup` command walks them through it. The refusal is also what
  keeps the server's poll from building: a poll on an empty manifest would otherwise re-extract
  the whole tree, which is the runbook's trap 7 in reverse.
- **How long an agent waits.** The lexical refresh is bounded by the edit. The vectors are
  not: a fused `ask` after a refresh embeds the rows that changed, inline, at ~80 ms a row on
  the default model (2,680 s over 33,525 rows) and ~3.1 ms on the small one — a branch switch
  touching 500 files is thousands of rows and minutes inside one tool call. The server
  therefore embeds at most `INLINE_ROWS = 128` rows inside a call (≈10 s on the default model,
  ≈0.4 s on the small one), answers from the vectors as they stand for the rest with the notice
  `dense: N rows behind, embedded between calls`, and embeds the remainder in slices of 64
  between requests. A deferred node keeps its previous vector until the new one is written, so
  a changed requirement is still reachable by its old text and always by the fresh lexical
  graph. The one-shot `ask` and `serve` are unchanged: the bound is `None` there.

### 3. The token budget

Two accountings, both stated in the README:

- **Context, once per session:** the harness renders the tool list and the server's
  `instructions` into the system prompt once. Ceilings, enforced by unit tests: the serialised
  `tools` array ≤ 3,200 bytes and the instructions ≤ 800 bytes — 4,000 bytes, about 1,000
  tokens by the bytes / 4 proxy `bench` uses, five answers' worth. The five GitNexus schemas
  loaded above are about four times that for five tools of seventeen.
- **Billing, per request:** the same bytes are re-sent every turn, cached after the first at a
  fraction of the price. The measurement below reads the real figure — the first request's
  input tokens in the MCP arm minus the bare arm — because a harness renders schemas in its own
  format and the JSON is only a proxy.

Claude Code defers MCP tool schemas behind a search when a session carries many tools (this
session's own tool listing shows it); the ceiling is set for the harness that does not.

### 4. Concurrency and lifetime

- **One MCP process per harness session.** It starts with the session, exits when stdin
  closes, and never idles out: the harness is the owner. After `--idle-model` seconds (default
  900) without an `ask`, it closes the embedding model — 1.9 GB on the default — and reopens it
  on the next fused question (676 ms). `impact`, `trace`, `changes`, `explain` and an exact-id
  `ask` never open it.
- **The repository is the nearest ancestor of the process's working directory holding a
  `.repograph/`, else the nearest holding `.git`** — a linked worktree's `.git` is a file and
  is found the same way — so a harness opened in `apps/api` still asks the repository's store
  and two worktrees each get their own. `--repo` overrides it.
- **A running `serve` is used, never started.** The `ask` tool tries the socket first with the
  client `ask` already has (`serve::try_ask`), which refuses another build, another arm and
  another version and falls back to the process's own context; two harness sessions on one
  worktree then share one model if someone ran `serve`. Nothing in the plugin spawns a process
  that outlives the session.
- **Two processes writing one store** is the situation two `ask`s are in today: every write is
  a temp file and a rename, the manifest's stamp tells a reader the store moved, and identical
  edits produce identical bytes. No lock is added; the plan tests two servers taking the same
  edit and the store loading afterwards.
- **Sockets:** the MCP server binds none and removes none; only `serve` does either.

### 5. Failure and honesty

A claim never exceeds its evidence, and an agent reads only the result text, so:

- **Nothing matched:** `no match — an id or a symbol name answers exactly; otherwise try fewer,
  more specific words`, `isError: false`. A missing answer is an answer.
- **An unknown symbol** in `impact`, `trace` or `explain`: `no node matches X`, plus one
  sentence on how symbols are named.
- **The store's state** is said once, where it is cheapest: the server's `instructions` at
  `initialize` and the first result of the session both carry the `status` line — nodes,
  edges, fresh or behind, questions coverage, the vectors' model or `no vectors`. A store
  `enrich` never touched is named there (`questions 0/1996`), with the README's own numbers for
  what that costs paraphrase recall; a `--no-dense` store is named there (`no vectors`), and a
  fused `ask` on it is what the README calls lexical-only.
- **Per-answer degradation** is the notices `ask` already writes — `dense: model unavailable,
  continuing lexical-only`, `refresh: skipped`, `dense: N rows behind` — placed ahead of the
  answer. Nothing is added that the CLI does not already say to a person.
- **What the graph cannot prove it does not list.** The `impact` description carries the
  README's sentence about dynamic dispatch and callbacks, and the instructions carry "confirm a
  'nothing uses this' with rg before deleting".
- **`isError: true`** is reserved for the cases where the tool could not answer at all: no
  store, an unreadable store, a `changes` outside git, bad arguments.

### 6. Harness portability

| Claude Code-specific | common to every harness |
|---|---|
| the plugin layout (`plugin/.claude-plugin/plugin.json`, `skills/`, `commands/`, `hooks/hooks.json`), the marketplace file at the repository root | `repograph mcp` — stdio JSON-RPC, protocol `2025-06-18`, negotiating down to `2024-11-05` |
| the `/repograph:setup` command | the `status --json` line and `--json` on all five readers |
| the `PreToolUse` reminder hook | the risk thresholds, the answer shape, the caps |
| `--plugin-dir` for the measurement | `codex mcp add repograph -- repograph mcp`; an `AGENTS.md` paragraph |

The MCP server is exercised in the plan by a Rust integration test driving it over stdio the way
any harness does, and by a stdlib Python client that checks the 142 recorded and developer
questions answer the CLI's bytes; Codex is smoke-tested by hand where it is installed. The
plugin is the only artifact with a Claude Code shape, and it holds no logic: the skill and the
hook say where the tools are, the command runs the CLI.

### 7. How this is judged

Retrieval quality is already measured — `bench` and the held-out set — and the plan holds it
still (Rule 1 below). The new question is whether an agent using the surface finishes tasks in
fewer tokens, and that is a measurement of agents, not of retrieval:

- **The suite:** 24 tasks on a copy of the fixture, built deterministically from the recorded
  cases — 8 keyword questions rephrased as tasks ("which requirement says …; answer with the id
  and path:line"), 6 code cases ("where is X declared; answer with the path"), 6 `impact`
  targets of the narrow tier ("list the direct callers of X and say whether changing it is
  low, medium or high risk"), 4 `trace` pairs that have a path ("how does A reach B; name every
  symbol on the way"). Truth is the case's own anchor, or `truth.py`'s rg-derived lists, never
  repograph's answer.
- **Three arms**, each `claude -p` headless in the copy with the same read-only tool
  allowance: **A** bare (Read, Grep, Glob, `rg`); **B** the plugin without its `.mcp.json` —
  the skill and the hook, the CLI through `Bash(repograph *)`; **C** the plugin plus the MCP
  server. The agent command is a template, so another harness can be substituted; the parser
  for its usage is written for `claude`'s `stream-json` and says so.
- **What is read per task:** correctness (the anchor in the final answer); context-shaped
  tokens, `input + cache_creation + cache_read + output` summed over the run; billed cost;
  turns; wall time; and, for arm C against A, the first request's input tokens — the surface's
  real cost in the harness's own rendering.

**The rule, written before any run.** One run per arm per task, then:

- (i) an arm is eligible only if its pass count is not below A's;
- (ii) **C is the default surface** if its median per-task ratio against A is ≤ 0.5, it is
  cheaper than A on at least 18 of 24 tasks (exact two-sided binomial, p ≈ 0.023), and its
  median ratio against B is ≤ 1.1;
- (iii) else, if B meets the same two clauses against A, **B is the default**: the plugin
  ships without `.mcp.json`, the server stays in the binary and the README documents it as
  opt-in;
- (iv) else both ship as documented options, no default is claimed, and the README says the
  tenth-of-the-characters claim did not carry from an answer to a task on this suite, with
  the numbers;
- (v) a decisive pair landing between 15 and 17 wins is run a second time and judged on the
  48 pairs at 36 wins; nothing else is re-run.

The 0.5 bar is not the README's tenth: a task has turns the tool cannot touch, and halving the
whole task is the smallest claim that says the tool changed the task rather than one step of
it. The first-request reading is not part of the rule; it is reported beside the JSON ceiling
so the proxy can be judged.

## The rules, in one place

- **Rule 1 (retrieval untouched):** four `dump` files and four `bench` lines on the fixture,
  both arms, byte-identical to the baseline taken before the first edit, after every code
  task. A cap or a renderer that moves an answer is a bug.
- **Rule 2 (the same bytes):** the `ask` tool's result equals `ask --no-serve` stdout byte for
  byte on the 142 recorded and developer questions on the copy, in both arms, notices
  included; `impact`, `explain`, `trace` and `changes` at `limit = 0` (uncapped) equal their
  CLI twins on the blast cases.
- **Rule 3 (the surface ceiling):** `tools` ≤ 3,200 bytes, `instructions` ≤ 800 bytes, as
  tests; the first-request reading reported beside them.
- **Rule 4 (bounded waiting):** on the copy's small-model store, an `ask` after an edit of 60
  files answers in under 5 s and reports the rows it left behind; the default model's bound is
  the arithmetic of the same cap at 80 ms a row and is stated as arithmetic.
- **Rule 5 (the agent bench):** the five clauses above.

## Explicitly out

- **A registry of checkouts and `--repo <name>`** ([`2026-09-03-registry-design.md`](2026-09-03-registry-design.md)):
  the MCP process runs where the harness runs, and the root walk answers the worktree question
  without a file anyone maintains.
- **Starting `serve` from the plugin.** A process that outlives the session is the user's to
  start; the server uses one when it is there.
- **MCP resources and prompts.** Tools carry everything measured; a resource is a second
  surface with a second budget.
- **A hook that runs `changes` before every commit.** It is the GitNexus augmentation in a
  cheaper place, and it costs a call per commit whether or not the agent needed it; measured
  after the surface is, if at all.
- **`rename`, `cypher`, `pdg_query` and the group tools.** Never used in this corpus; nothing
  in the graph answers them.
