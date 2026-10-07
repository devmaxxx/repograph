# feat/reader-refresh-budget

## Tickets
- **#135: Readers answer within 10 s: stale answer plus a detached refresh after a large pull** (GitHub issue · open · milestone 0.6.0)
  - Description: after a ~30-commit pull in beauty-crm (576 files) the first `impact` refreshed
    inline for 69.5 s and the first `ask` embedded 1683 vectors inline for 246.9 s, with no
    progress line — indistinguishable from a hang.
  - Acceptance criteria: the first reader after a pull answers within the budget, not N × embed
    cost; a later `ask` ranks the changed nodes densely once the background refresh finished;
    foreground embeds print progress at least every few seconds.
  - Comments: the owner's comment "Plan (agreed 2026-09-28): a reader never waits longer than
    10 s" is the spec and supersedes the description where they differ:
    1. budget on the reader path (`ask`, `impact`, `trace`, `changes`, `explain`): estimate the
       refresh (changed files from the manifest diff, pending vectors); over budget ⇒ answer from
       the stored index with one stderr line (`index: 576 files behind, refreshing in background`),
       `--json` carries `stale: {files: [...], vectors: n}`; the budget is a config key, default
       10 s; `--stale` stays as it is;
    2. the reader spawns one detached `repograph update`, which takes the store's writer lock; a
       second reader finding the lock held spawns nothing and answers the same way;
    3. `install-agent` installs `post-merge` and `post-checkout` git hooks starting a detached
       update;
    4. a foreground `update`/`build` prints `embedded 400/1683` to stderr.
    Out of scope: making the embed itself faster.
  - Open questions: none blocking — the comment settles what the description left open.

## Files to change
- `src/refresh.rs` (new) — the budget arithmetic, the `Stale` report and its JSON injection, the
  detached spawn (Unix process group, Windows `DETACHED_PROCESS`).
- `src/store.rs` — the writer lock (`.repograph/writer.lock`, `File::try_lock`/`lock`).
- `src/ask.rs` — `graph_for_ask` takes the lock and the deadline and returns what it left behind;
  `Context` carries the deadline, the lock and the stale report; the dense sync is chunked
  against the deadline and stops into a checkpoint.
- `src/index/dense.rs` — `owed` (rows a sync would embed) and `checkpointed` (resume the live scan
  after a stopped sync).
- `src/index/embed.rs` — `embed_with`, a per-forward progress callback.
- `src/main.rs` — `update --detach`, the lock around `build`/`update`, the
  progress line, `graph_for` with the budget, `stale` in every reader's `--json`.
- `src/serve.rs` — restart the answer clock per request.
- `src/config.rs` — `reader_budget` (seconds, default 10; machine file may set it;
  `REPOGRAPH_READER_BUDGET` overrides).
- `src/install_agent.rs` — the two git hooks.
- `repograph.toml`, `README.md` — the key, the line, the hooks, the progress line.
- `tests/reader_budget.rs` (new) — end-to-end over a scratch repository.

## Implementation steps
1. Writer lock in `store.rs`.
2. `reader_budget` config key.
3. `refresh.rs`: `Stale`, `with_stale`, `fits`, `spawn_update`.
4. `graph_for_ask` budgeted; `graph_for` and the four graph readers print the line and inject
   `stale`.
5. `Context`: deadline, lock, stale; dense sync chunked against the deadline; `ask --json` stale.
6. `update --detach` (spawn and return, output to `.repograph/background.log`); every `update` and
   `build`, foreground or spawned, waits for the lock.
7. Progress: `embed_with` + `embedded N/M` every ~2 s in `embed_opened`.
8. `install-agent` git hooks.
9. Docs.

## Tests
- `src/refresh.rs` — `with_stale` keeps an object an object (compact, pretty, empty); `fits`.
- `src/store.rs` — a second try-lock while the first is held reports busy; drop frees it.
- `src/index/dense.rs` — `owed` counts what `sync` would embed; a sync stopped by its callback and
  `checkpointed` answers from old rows and new, and the next sync embeds only the rest.
- `src/install_agent.rs` — hooks written into a fresh repo, idempotent, appended to a foreign hook
  once, honouring `core.hooksPath`.
- `src/config.rs` — default 10, project file, env override.
- `tests/reader_budget.rs` — budget 0 ⇒ `impact --json` answers from the stored graph with
  `stale.files`, one `index:` line, a detached update brings the store in line; lock held ⇒
  no spawn; a fresh store has no `stale` key.

## Decisions
- Free-text task under `TRACKER=none`; branch `feat/reader-refresh-budget` was free locally and on
  origin.
- Built in-thread rather than through subagents: the change is one mechanism threaded through
  several files, and the plan's coherence is the risk, not its size.
- Refresh cost estimate: considered measured per-machine rates kept in the store, a load-average
  scale, and constants; chose constants for the graph (40 ms a file — 576 files took 69.5 s on a
  loaded machine, a quiet one is several times faster) because `apply_diff` cannot be stopped
  halfway, and a deadline for the vectors, measured as they go, because `sync_chunked` can stop
  at any checkpoint: the first chunk's real rate decides, which is what a loaded machine needs.
- `stale` is added to `--json` only when the answer is behind: every existing parser keeps its
  shape, and its presence is the signal.
- `serve` and `watch` do not take the writer lock: they were concurrent writers before and stay
  so (every write is a rename); `serve`'s per-request graph refresh stays synchronous, its dense
  sync gets the budget like a one-shot's.
- A reader that finds the lock held writes nothing at all — not even the stamp cache — so it
  cannot put an older manifest over the writer's.
- The detached update logs to `.repograph/background.log` rather than to nothing, so a failure
  there can be read after the fact.
- Git hooks: one shared block for both events (`$3 = 0` is a file checkout, skipped), appended
  between markers to a hook that exists already, at the path `git rev-parse --git-path hooks`
  names so `core.hooksPath` is honoured.
- The spawned `update` waits on the lock rather than giving up when it is held: a reader may still
  hold the lock in the moment it spawns, and a refresh that quit on finding it held would never run.
  Readers alone use the non-blocking try, so only they skip. No hidden flag separates the two.
