# Next-Version Levers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close nine gaps of `docs/bench/next-version-gaps.md` on one branch — G23, G15, G35, G39, G40, G34, G38, G32, G19 — each against the gate its own ledger section states, with every measurement's pass/fail clause written and committed before the number is read, and no recorded floor moved.

**Architecture:** G39 lands first and G34 is written against the shape G39 leaves. Extraction reads ids through one generic grammar (`ids::generic()`), the family set becomes a view over the graph (`families::of_graph`), and citations of prefixes no definition declares are kept aside on the graph (`Graph::pending`, partitioned by `Graph::settle`) so no reader sees them and no document is re-read when a family appears. With the second reader of the grammar gone, G34's "one owner" holds by construction and its property test is written once — every id the extractor writes classifies to the family the view counts it in, and a no-op `update` re-extracts nothing — instead of a `derive(..).against(..)` test whose subject the next task would delete. G39's own gate, the visible graph on the corpus copy byte-identical to `main`'s build and the four recorded arms reproducing the ledger's baseline, is what makes a G39 regression visible, and it is read against the corpus and the recorded numbers rather than against the grammar's opinion of itself. The cost family (G23, G19, G35) is read through one vendored harness, `bench/probe/`, whose bars are proved by running the same binary through it twice before any change is judged. Where G39 fails its gate the branch stops there (see "Where the branch stops"). Every writer on the branch runs in one locked worktree, `~/bench/beauty-crm-test`, put back to the pinned fixture's state by `bench/probe/reset.sh` before each arm and copied out of before the next; the fixture `~/bench/beauty-crm-502e8a6d` is read and never written; so the tasks that touch the corpus run one at a time, in order, and never side by side (see "One writable directory").

**Tech Stack:** Rust 2021 on toolchain 1.98.0, dependencies `=`-pinned in `Cargo.toml` (no new crate is needed by any task; `regex 1.13.1`, `serde`, `postcard` and `tempfile` are already in the lock). Python 3 (stdlib only) for `bench/history/track.py` and the new `bench/probe/judge.py`. Bash 3.2, `/usr/bin/time -l`, `top -l`, `perl -MTime::HiRes`, `caffeinate` for the probe scripts — macOS only, like the kit they replace.

**Spec:** `docs/bench/next-version-gaps.md` at `main` (`d688e56`) — sections `## G23` (line 1137), `## G32` (1456), `## G34` (1509), `## G35` (1542), `## G38` (1644), `## G39` (1661), `## G40` (1688), `## G15` (683), `## G19` (950). Each task quotes its gap's own **Gate** as the acceptance criterion; where this plan reads a gate differently from its letter (G40 after G39, G19's control), the task says so and the ledger entry written in Task 12 records it.

> **⚠ The probe scripts in this document are the initial draft. `bench/probe/` in the tree is authoritative.**
>
> Every fenced script block below — Task 0's `arms.sh`, Task 1's `judge.py`, `readers.sh`, `embed.sh`, `measure.sh`, `quiet.sh`, `reset.sh`, `graphdiff.py`, `README.md` and the embed summary row it prints — is the draft as first written. The files in `bench/probe/` are authoritative from `a53e50c` onwards (`arms.sh`, which landed earlier, from `0bb5ed3`), and every commit named below corrected a defect that is still present in the blocks here. **No executor may re-extract a script from this document** — not with `awk`, not by hand, not "to check". Read `bench/probe/` and edit it in place. The agent that ran Task 0 did extract a block from this document with `awk` and install it byte-for-byte, which is why this note exists; nothing below is ticked or changed by it, and no step is retired.
>
> - `0bb5ed3` — `arms.sh` gets the shebang the draft block omits.
> - `994eae3` — a row that measured a failure is refused rather than averaged: `rc=` on `measure.sh`'s summary line, `judge.py` refusing it, `readers.sh`, `embed.sh`, `arms.sh` and `graphdiff.py` following.
> - `deceb5c` — an arm that fails a floor is still a recorded arm: `arms.sh` stops dropping it; `judge.py` refuses a medians file that parsed to nothing.
> - `8b642d8` — `quiet.sh` counts `node` like the other three and exempts only its own ancestor chain.
> - `8061a6f` — `judge.py compare` judges peak CPU, and leaves a row the sampler never caught unjudged instead of green.
> - `765761f` — the review's findings on the probe: `measure.sh` reads each `time` field off the report's whole shape, `readers.sh` and `embed.sh` follow, `judge.py` refuses an unreadable path by name.
> - `7bd637a` — `quiet.sh` counts busyness and not presence; the printed line names which rule counted what.
> - `ad05b04` — `embed.sh` builds its row through `summary_row`, carries `samples=` (the draft's row below prints no such field) and refuses a row the sampler never read.
> - `6ed52ab` — `readers.sh` stops swallowing the medians verdict through a `| tee`, and resolves `$F` before the `cd` so the `cn.ts` restore cannot miss.
> - `12dcb9a` — a refused `embed.sh` kills the run instead of waiting it out, and says what it left half-written in the store.
> - `79a0ef1` — `judge.py`'s `MIN_N` argument prints the usage line instead of a traceback, and a floor of zero is refused.
> - `db523da` — `embed.sh` exits 3 where the store is left partial, `readers.sh` makes a slash-bearing binary path absolute, `judge.py` asks `isdecimal` where it meant `isdigit`.
> - `fa1e2d6` — a `bench` row that missed a floor is a reading and carries `floors_missed=1`; every other non-zero exit is still refused, and the refusal quotes what the row said.
> - **the embedder declaration left the tree** (the commit carrying this bullet) — `reset.sh` no longer writes `$WT/repograph.toml` and its `clean` no longer spares one, so a reset leaves an empty `git status --porcelain`; which weights a row is read under is declared as `REPOGRAPH_EMBED_MODEL=intfloat/multilingual-e5-small` in the environment, named once in the new `bench/probe/embedder.sh`, and `reset.sh`, `readers.sh`, `embed.sh` and `arms.sh` each refuse with exit 2 when it is absent or names another model. **Every command below that calls one of those four must carry the declaration**; a bare `bench/probe/reset.sh` now refuses. Three places in this document still describe the file `reset.sh` used to write and were left as first written, like the blocks: the numbered `reset.sh` rule under **One writable directory**, Task 10's **Interfaces** line, and Task 10 Step 2's `grep embed_model $WT/repograph.toml` with the `embed_model = …` line in its **Expected**. The tree is authoritative over all three.

## Global Constraints

- Rust 2021, toolchain 1.98.0 (`rust-toolchain.toml`). Every dependency is `=`-pinned in `Cargo.toml`; a new dependency needs a reason in this plan and an exact pin. This plan adds none.
- Gates on every task: `cargo test` green and `cargo clippy --all-targets -- -D warnings` clean. Where a task touches `agent/`: `node --test agent/hook.test.mjs` (no task here does). Where it touches `bench/history/`: `python3 -m unittest discover -s bench/history`. Where it touches `bench/probe/`: `python3 -m unittest discover -s bench/probe`. Where it touches `bench/compare/`'s inputs (the `families --json` shape): `python3 -m unittest discover -s bench/compare`.
- CI runs `macos-14`, `ubuntu-latest` and `windows-latest`. Every new test that shells out or writes a path is written to pass on Windows: paths through `Path::join`, no `/tmp`, no backslash assumptions, the binary through `env!("CARGO_BIN_EXE_repograph")`.
- Conventional Commits subjects (`feat(ids):`, `refactor(families):`, `perf(embed):`, `test(bench):`, `docs(bench):`). **No AI attribution trailers and no session links in commit messages.** The repository's commit hook also rejects any single shell command that contains both a heredoc and the words `git commit` unless the heredoc's first line is a Conventional subject — so file edits and commits go in separate commands, and any file whose text mentions committing is written with the Write tool.
- Comment hygiene: comments say *why*, never *what*. A rustdoc `///` on a public item states the contract and stays.
- **One beauty-crm worktree. Every test runs there. Removal is blocked.** Max's standing rule, and this plan creates no worktree of the corpus. Two directories exist, both `git worktree lock`ed so `git worktree remove` refuses them, both detached at `502e8a6d`: `FIX=~/bench/beauty-crm-502e8a6d`, the pinned fixture, read-only for ever — what may run against it is `bench`, `dump`, `verify`, `explain`, which load the store as it stands and never walk the tree, and `ask`, `impact`, `trace`, `changes` with `--stale`, which return before the walk (a walk without `--stale` records stamps into `manifest.json` and rewrites any mirror whose magic it did not recognise, and that is a write; a `--stale` ask never resyncs the dense index, so a dense reader is as safe as a `--no-dense` one); never `build`, `update`, `enrich`, `embed`, `derive`, `watch`, `serve`, `import-legacy`. And `WT=~/bench/beauty-crm-test`, the one writable target: every writer on this branch runs there and nowhere else, and so do the reader rows, for the two reasons "One writable directory" states. Every command line in this plan spells one of the two names.
- **The embedder is declared in the environment, not in the tree.** `REPOGRAPH_EMBED_MODEL=intfloat/multilingual-e5-small` is exported for every `bench/probe/` script that runs a writer or a reader (`reset.sh`, `readers.sh`, `embed.sh`, `arms.sh`), and each refuses with exit 2 when it is absent or names another model rather than defaulting — only the small model is cached on this machine (`~/.cache/repograph/fastembed/models--intfloat--multilingual-e5-small`), and the built-in default has moved before (`e5-large` between `35357c1` and 2026-09-07, ADR-002). The variable outranks what a store records, so a restored store is read and embedded with the weights the reading names; and no `repograph.toml` is left in `$WT`, whose tree an untracked file of ours would make two changed paths for every `changes` row instead of the one it touched. `bench/probe/embedder.sh` is the one place that names the model.
- **Reset before every arm, copy out before the next.** `bench/probe/reset.sh` (Task 1) puts `$WT` back to the fixture's state — tree reverted and cleaned to an empty `git status --porcelain`, the fixture's store copied in whole, the stamps settled by one `--no-dense update` — and refuses any directory that is not the locked worktree at `502e8a6d`. Whatever a comparison reads is copied to `~/bench/levers-2026-09-10/log/<task>/<arm>/` before anything else runs in `$WT`, and the comparison reads the copies. Those copies are store images and not places to run anything: an `ask` against a store with no tree beside it empties the graph (runbook trap 7). `$WT` is a real worktree with the tree beside the store, so the trap does not apply there.
- **A gate is written before its number.** Task 0 commits `docs/bench/2026-09-10-next-version-levers-results.md` with every numeric clause in it; each measuring task pastes its reading under its clause and changes no clause. A clause that a reading fails is recorded as failed, and the ledger entry says so; nothing is widened after the fact.
- **No floor moves.** The four recorded arms (built-in suite and `bench/dev-cases.jsonl`, dense and `--no-dense`) on the pinned store must read the ledger's 2026-09-09 baseline table exactly — counts, p90 and the anchor line — after every task that touches a read path (Tasks 4, 5, 8). `bench`'s `passes()` and `FLOORS` are not edited by any task.
- The standing rule for this repository: every task that changed code ends with `/code-review high --fix`, the findings applied, tested and committed before the task is reported done. Task 12 runs one more over the whole branch.
- Any `gh` call runs as `gh auth switch --user devmaxxx && gh …` in the same shell command. This plan pushes nothing; the branch is left local.

## Out of scope, in one line

G1 and G6 (wait on 0.6.0's Kotlin/C#/Python extractor), G2 (the ledger records nothing cheaper left to try), G13/G17/G14/G37/G22/G24/G25 (money, hardware under memory pressure, or a 2.1 GB download — this branch is the code levers), G33 (closed as accepted); and the large-model cadence of G19, which is the same 2.1 GB download.

## Where the branch stops

G39 is the one row in the ledger that could grow the store, and the ledger's own order says it is "measured before it is taken". Its measurement is Task 6. If Task 6 fails clause (a) (the visible graph is not identical to `main`'s), clause (b) (`graph.json` grows past 110%), or clause (e) (a reader row leaves the bars of Task 1's control), the executor stops after recording the reading in the results document and the ledger — Tasks 7, 8 and 9 are written for the graph G39 leaves and are not attempted on the graph it did not. Tasks 10 (G32) and 11 (G19) do not depend on G39 and are run in either case, on `main`'s extractor if G39 was reverted (`git revert` of Tasks 4–5, one commit each, recorded).

## File Structure

New:

- `bench/probe/README.md` — what each script measures, the quiet precondition, the bars, macOS-only.
- `bench/probe/measure.sh` — one command under `/usr/bin/time -l` with a `top` sampler (the kit's `measure.sh`, paths removed).
- `bench/probe/quiet.sh` — the quiet-machine precondition; exits 1 with the reading when unmet.
- `bench/probe/reset.sh` — the one writable worktree put back to the fixture's state before a writer arm; refuses any directory that is not the locked worktree at `502e8a6d`. Tested by `bench/probe/test_reset.py`.
- `bench/probe/readers.sh` — the reader suite: n runs per row, every row `--stale`, in the writable worktree straight after a reset.
- `bench/probe/embed.sh` — a whole-store embed under the sampler with every stderr line wall-clock-stamped; refuses the pinned fixture by path.
- `bench/probe/judge.py` — medians, the control verdict, the compare verdict, the cadence reading. Tested by `bench/probe/test_judge.py`.
- `bench/probe/graphdiff.py` — are two `graph.json` files the same visible graph, and how much is held aside. Tested by `bench/probe/test_graphdiff.py`.
- `bench/probe/queries10.jsonl` — the ten `dump` queries the reader suite has always used (vendored from the kit).
- `docs/bench/2026-09-10-next-version-levers-results.md` — every gate, then every reading.
- `docs/superpowers/plans/2026-09-10-next-version-levers.md` — this file.

Modified:

- `src/ids.rs` — `IdMatcher::new(families, milestones)` and the never-matching alternation go; `ids::generic()` is the one matcher.
- `src/doc/requirements.rs` — `RequirementScanner::new()` takes no matcher; after Task 8 it owns `FAMILY`, `MILESTONE` and the head regex.
- `src/doc/mod.rs`, `src/doc/registry.rs`, `src/code/mod.rs`, `src/code/idrefs.rs` — extractors take no matcher; `registry::declared_ids` goes.
- `src/model.rs` — `Graph.pending`, `Graph::settle`, `remove_file` covers pending.
- `src/store.rs` — mirror magic `RGM2`; one test that a store written before `pending` existed reads whole.
- `src/families.rs` — `derive`, `Derived`, `against`, `matcher`, `keys`, `from_graph`, `graph_families`, `test_matcher` go; `of_graph`, `classify`, `line`, `moved`, `survey`, the report with two new columns stay.
- `src/main.rs` — `extractors(repo)`; `run_update`, `Watcher`, `apply_diff` stop deriving; `Timing` stages on `update`; `SYNC_CHUNK` in tokens; the bench line loses `families=`.
- `src/ask.rs` — `graph_for_ask` stops deriving, gains a stage; `Context` loses `ids`.
- `src/query.rs` — `exact_seeds(graph, words)`, `ask(graph, lex, …)`; `verify` reports what is held aside.
- `src/legacy.rs`, `src/bench.rs`, `src/dump.rs`, `src/prime.rs` — callers of the above.
- `src/index/dense.rs` — `ChunkBudget { tokens, max_rows, ramp }`, `chunk_ends(lens, b)`, `sync_chunked(.., measure, ..)`, the ratio probe.
- `src/index/embed.rs` — `Embedder::token_lengths` becomes `pub`.
- `src/walk.rs` — one assertion that no `rel` carries a backslash.
- `bench/history/track.py`, `bench/history/test_track.py`, `bench/history/README.md` — the anchors line enters the history.
- `tests/families.rs` — the integration test follows the report's new shape.
- `README.md` — Id families, Bench (anchors, `--repeat`, `families=` retired), the `update` timing line, `verify`'s held-back line.
- `docs/bench/next-version-gaps.md` — a status line under each of the nine gaps.
- `docs/bench/runbook.md` — points at `bench/probe/`.

## Measurement rule for this branch

Every before/after read on this branch is the four arms — `bench` and `bench --cases bench/dev-cases.jsonl`, each with and without `--no-dense` — with the per-case lines, the summary line and the `anchors` line kept in `~/bench/levers-2026-09-10/log/` and recorded into `bench/history/runs.jsonl` with a `--tag`. "Identical to the baseline" means the ledger's 2026-09-09 table under G15:

| suite | arm | counts | anchors |
| --- | --- | --- | --- |
| recorded | lexical | keyword 39/40, paraphrase 15/30, code 12/12, p90 215 | 39/40, 15/30, 12/12 |
| recorded | dense | keyword 40/40, paraphrase 15/30, code 12/12, p90 221 | 40/40, 15/30, 12/12 |
| developer | lexical | long 11/15, cross 13/15, multi 9/12, where 0/9, rule 5/9, p90 239 | 11/15, 13/30, 16/39, 0/13, 5/9 |
| developer | dense | long 10/15, cross 13/15, multi 10/12, where 0/9, rule 5/9, p90 239 | 10/15, 13/30, 13/39, 0/13, 5/9 |

The four arms are run by one script so nobody types them differently twice:

```bash
# arms.sh <binary> <repo> <logdir> <label>  — the four arms, transcripts kept, recorded with a tag
B=$1; R=$2; L=$3; T=$4; H=/Users/max/Documents/projects/repograph/bench/history
mkdir -p "$L"
for suite in rec dev; do
  for arm in dense lexical; do
    nd=""; [ "$arm" = lexical ] && nd="--no-dense"
    cases=""; [ "$suite" = dev ] && cases="--cases /Users/max/Documents/projects/repograph/bench/dev-cases.jsonl"
    REPOGRAPH_NO_SERVE=1 "$B" --repo "$R" $nd bench $cases > "$L/$T-$suite-$arm.txt" 2>&1 || true
    tail -3 "$L/$T-$suite-$arm.txt"
    python3 "$H/track.py" record "$L/$T-$suite-$arm.txt" --corpus beauty-crm --corpus-path "$R" --tag "$T" --note "next-version-levers $T"
  done
done
```

Task 0 writes it to `bench/probe/arms.sh`. Its `<repo>` is `$FIX` for every read of the baseline (Tasks 6, 10 and 12) and `$WT` only for G32's rebuilt and enriched reads (Task 10).

## One writable directory

Two directories, both `git worktree lock`ed so `git worktree remove` refuses them, and nothing on this branch creates a third:

- `FIX=~/bench/beauty-crm-502e8a6d` — the pinned fixture. Read-only, for ever. The four arms of every baseline read are taken here (`bench` loads the store as it stands), and nothing else on this branch touches it except as `reset.sh`'s `rsync` source.
- `WT=~/bench/beauty-crm-test` — the one writable target, the corpus tree with no store of its own until `reset.sh` writes one. Every writer on this branch runs here: G35's `update` (Task 3), G39's builds and updates (Task 6), G34's and G38's builds (Tasks 8, 9), G32's rebuild and `enrich` (Task 10), G19's embeds (Task 11). So do the reader rows (Tasks 1 and 6), for two reasons that are the machine's and not a preference. The branch's binary writes its mirrors under a magic (`RGM2`, Task 4) that the fixture's `graph.bin` (`RGM1`) does not carry, and a mirror can only be written where writing is allowed — read on the fixture, the branch would parse 12 MB of JSON on every row that `main` reads in postcard, and the cheap rows (`impact` at 0.04 s) would leave the 10% bar for a reason that is not the change's. And the fixture's `git status` is dirty by design (`graphify-out/` is tracked and rebuilt in place: four files, a 41 MB `graph.json` among them, an 8,027-line `-U0` diff), which the `changes` row would read on every run and a clean tree would not. What G23's lever asked for — a pinned index the rows are taken against, which no writer in the session touches — the rows still get, and more strictly than before: the store under them is the locked fixture's, byte for byte, copied in moments before, every row is `--stale`, and Task 1 checksums the store before and after both suites to say that nothing wrote.

**Arms are sequential and destructive.** With one directory there is no side by side: the branch's `build` overwrites `main`'s `graph.json`, and the next reset overwrites the enriched store. Two disciplines follow, and every task that touches the corpus is written with both as numbered steps:

1. **`bench/probe/reset.sh` before every arm** (Task 1 writes it, with its test). It refuses anything that is not the locked, detached worktree at `502e8a6d`; reverts tracked edits and removes untracked files; copies the fixture's store in whole (`rsync -a --delete`); writes `repograph.toml` with `embed_model = "intfloat/multilingual-e5-small"`; and runs one `--no-dense update` through `repograph-main` to settle the stamps — the copied manifest carries the fixture's mtimes, which match nothing in `$WT`, so that first walk hashes every file, finds every hash unchanged, records this tree's stamps, and every later walk is stat-only; on an unchanged tree `repograph-main` takes the no-op path and derives nothing, so the settle reads `changed 0` and is not a re-read. Where an arm needs an empty store rather than the fixture's, the task says so after the reset: `rm -rf $WT/.repograph` for a build from nothing (a `build` wipes only the graph and the manifest and keeps the questions and the vectors, so "from nothing" has to be said); `embed.sh` removes the vectors itself. Where rows are read through a branch binary, one warm non-stale `--no-dense ask FR-PAY-22` follows the reset: it walks the settled tree (stat-only, nothing changed, no stamp written), passes over the `RGM1` mirror, writes the branch's own `RGM2` mirrors of the unchanged graph and questions, and pays that rewrite outside the rows.
2. **Artefacts copied out before the next arm.** Whatever a comparison reads is copied to `~/bench/levers-2026-09-10/log/<task>/<arm>/` immediately after the arm that wrote it and before anything else runs in `$WT`, and the comparison reads the copies: `log/g39/main/graph.json` against `log/g39/branch/graph.json` (Task 6), `log/g39/branch/graph.json` against `log/g34/t8/graph.json` (Task 8), the enriched store to `log/g32/after/store/` (Task 10) — the store Max may pin next, which the next reset would otherwise destroy. This is the single most likely way an executor loses a reading, so it is a numbered step in each of those tasks, never a parenthesis.

**So Tasks 1, 3, 6, 8, 9, 10 and 11 run one at a time, in the plan's order, never in parallel and never interleaved** — no dispatch of two of them to two agents. A task interrupted mid-arm resets before it resumes rather than trusting what is on disk, and re-runs the arm from its first step. Tasks 2, 4, 5 and 7 touch no corpus and are unaffected.

`~/bench/levers-2026-09-10/{bin,log}` hold the built `repograph-main` and the transcripts — not corpus worktrees, and outside the rule. `~/bench/levers-2026-09-10/main-src` is a detached worktree of *this* repository at `d688e56`, checked out to build `main`'s binary from: the rule is about the corpus a store is built from, and a checkout of repograph is not that.

---

### Task 0: Branch, baseline, the two directories, and every gate written down

**Files:**
- Create: `docs/bench/2026-09-10-next-version-levers-results.md`
- Create: `bench/probe/arms.sh`
- Modify: none

**Interfaces:**
- Consumes: `FIX=~/bench/beauty-crm-502e8a6d` and `WT=~/bench/beauty-crm-test`, both locked worktrees of the corpus at `502e8a6d`, as the rule leaves them — this task creates no worktree of the corpus.
- Produces: `~/bench/levers-2026-09-10/bin/repograph-main` built at `d688e56`; `~/bench/levers-2026-09-10/log/`; `~/bench/levers-2026-09-10/main-src`, a detached worktree of this repository.

- [x] **Step 1: Branch off `main` and prove the baseline is green**

```bash
cd /Users/max/Documents/projects/repograph && git checkout main && git rev-parse --short HEAD
```

Expected: `d688e56`. Anything else: stop and report — this plan's line numbers are read on `d688e56`.

```bash
cd /Users/max/Documents/projects/repograph && git checkout -b feat/next-version-levers && cargo test 2>&1 | tail -3 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1
```

Expected: every test binary `test result: ok`; clippy prints nothing but `Finished`.

- [x] **Step 2: The scratch directory and `main`'s binary**

```bash
mkdir -p ~/bench/levers-2026-09-10/log ~/bench/levers-2026-09-10/bin
git -C /Users/max/Documents/projects/repograph worktree add --detach ~/bench/levers-2026-09-10/main-src d688e56
cargo build --release --manifest-path ~/bench/levers-2026-09-10/main-src/Cargo.toml 2>&1 | tail -1
cp ~/bench/levers-2026-09-10/main-src/target/release/repograph ~/bench/levers-2026-09-10/bin/repograph-main
~/bench/levers-2026-09-10/bin/repograph-main --version
```

Expected: `repograph 0.5.0`. `main-src` is a worktree of repograph, not of beauty-crm: the rule is about the corpus a store is built from — one writable copy of that tree — and a checkout of this repository's `main` to build a binary from is not that. It is removed after the merge (Task 12).

- [x] **Step 3: The two directories, as the rule leaves them**

```bash
FIX=~/bench/beauty-crm-502e8a6d; WT=~/bench/beauty-crm-test
git -C ~/Documents/projects/beauty-crm worktree list | grep -E 'beauty-crm-(502e8a6d|test) '
git -C "$FIX" rev-parse --short HEAD; git -C "$WT" rev-parse --short HEAD
ls "$FIX/.repograph"; ls "$WT/.repograph" 2>&1; git -C "$WT" status --porcelain | wc -l
```

Expected: two lines, each ending `502e8a6d (detached HEAD) locked`; `502e8a6d` twice; the fixture's store lists `graph.bin graph.json manifest.json questions.bin questions.json vectors.f32 vectors.json`; `$WT/.repograph` is `No such file or directory`; the status count is `0`. Anything else — a directory missing, unlocked, at another commit, `$WT` already holding a store or a dirty tree — is reported, not repaired here: nothing on this branch creates, removes or unlocks a worktree of the corpus. `$WT`'s first store is written by `bench/probe/reset.sh` the first time an arm runs (Task 1's control), and every arm after that starts the same way. Neither directory holds a `repograph.toml`, and neither is left to the built-in default: which embedder a row was read under is declared in the environment as `REPOGRAPH_EMBED_MODEL=intfloat/multilingual-e5-small` and refused when absent, so `$WT`'s status count stays `0` for the `changes` rows and a store restored under a moved default is still read with the weights the reading names.

- [x] **Step 4: Write the four-arms script**

Write `bench/probe/arms.sh` with the content under "Measurement rule for this branch" above, then:

```bash
chmod +x /Users/max/Documents/projects/repograph/bench/probe/arms.sh
```

- [x] **Step 5: Write the results document with every gate in it**

Write `docs/bench/2026-09-10-next-version-levers-results.md`:

```markdown
# Next-version levers — what the branch measured

Every gate in this document was written and committed before the run it judges (Task 0 of
[the plan](../superpowers/plans/2026-09-10-next-version-levers.md)); readings are pasted under
their gate and no clause is edited after a number is read. A clause a reading fails is recorded
as failed. `main` is `d688e56`; the branch binary in each section is named by its commit.

Machine: (filled in by Task 0 step 6). Directories: the pinned fixture `~/bench/beauty-crm-502e8a6d`
(locked, read-only — the four arms of every baseline read are taken there, and nothing is ever
written there) and the one writable worktree `~/bench/beauty-crm-test` (locked — every writer runs
there, and every reader row is read there straight after `bench/probe/reset.sh` has copied the
fixture's store in). What each arm wrote, copied out before the next arm ran:
`~/bench/levers-2026-09-10/log/<task>/<arm>/`. Transcripts: `~/bench/levers-2026-09-10/log/`.

## 1 · G23 — the reader bars, read through their own control

**Gate.** `bench/probe/quiet.sh` passes (idle ≥ 85%, 1-minute load < 3.0, AC power, no `cargo`,
`rustc`, `node` or another `repograph` running); `bench/probe/readers.sh` at n = 5 per row, twice
through the same binary (`repograph-main`), in `~/bench/beauty-crm-test` straight after one
`bench/probe/reset.sh`, every row `--stale`, with `shasum` over the store's files identical before
the first suite and after the second (nothing wrote under them); for every row
|median wall A − median wall B| ≤ 10% of A and |median max RSS A − B| ≤ 5% of A. A row outside
at n = 5 is re-run once at n = 9; a row still outside is recorded as *not judgeable at this shape
on this machine* and stays in the table with that word — it is not widened and it is not dropped.
The passing rows' medians become the reference every later reader reading on this branch is
judged against, under the same 10% / 5%.

**The index under the rows.** G23's lever asked for a pinned index — the reader rows taken against
a copy no writer in the session touches. On this branch that index is the locked fixture's store,
byte for byte: `reset.sh` copies it from `~/bench/beauty-crm-502e8a6d` into the one writable
worktree moments before the suite, every row reads the store as it stands (`--stale` on every
command that could walk; `bench` and `dump` never walk), and the checksums say nothing moved. The
cause this replaces is the fixture's own index growing 33,526 → 33,554 rows mid-suite on
2026-09-07 because `watch` and `update` ran against it. The fixture is never written now — it is
locked, and the rule allows it readers only — and no row of the suite can write anywhere.

**Reading.**

## 2 · G15 — the anchor line in the history

**Rule.** Every before/after on this branch carries the `anchors` line and is recorded through
`track.py`, which reads it. The four arms on the pinned fixture `~/bench/beauty-crm-502e8a6d`
reproduce the 2026-09-09 baseline exactly — counts, p90, anchors — after every task that touches a
read path.

**Reading.**

## 3 · G35 — what `derive` costs on an update that changes a file

**Gate.** Median of five `families derived` steps under `REPOGRAPH_TIMING=1` on a one-file
`--no-dense update` in `~/bench/beauty-crm-test` after `reset.sh`, recorded beside `tree walked`
from the same runs. The ledger expects it under the walk's own number; it is recorded whichever
side it lands.

**Reading.**

## 4 · G39 — extraction by one grammar, the family set a view

**Gate.** In `~/bench/beauty-crm-test`, each build from an empty store after `reset.sh`, each
build's `graph.json` copied to `log/g39/<main|branch>/` before the next build runs: (a) `graph.json`
written by `repograph-main build` and by the branch's `build` are the same visible graph —
`bench/probe/graphdiff.py` over the two copies reports `nodes: same` and `edges: same`; (b) the
branch's `graph.json` is at most 110% of `main`'s in bytes, and the number of edges held aside
(`pending`) is recorded; (c) an `update` after adding `docs/oq.md` defining `OQ-1` prints
`changed 1 removed 0`, prints `families: +OQ`, re-reads no other file, and its `--no-dense` wall is
under 0.5 s as the median of five; (d) `--no-dense build` wall on the branch is within 10% of
`main`'s, medians of three; (e) every reader row of §1 through the branch binary — read in the same
directory as §1's reference after a reset and one warm `--no-dense ask` that writes the branch's own
mirror (the fixture's is `RGM1`, the branch's `RGM2`, and a mirror can only be written where writing
is allowed) — is within §1's bars of the reference. Read path: the four arms on the pinned fixture
read the baseline table of §2 exactly. Failing (a), (b) or (e) stops the branch at this task.

**Reading.**

## 5 · G40 — a corpus that defines no ids

**Gate.** A build over documents with no definition line yields file nodes only, an empty family
line, its `ISO-8601` mention held aside and counted by `verify`, and `ask ISO-8601` answering from
retrieval rather than an exact seed. The never-matching alternation is gone from `src/ids.rs`.

**Reading.**

## 6 · G34 — one owner, the property test, the corpus

**Gate.** The property test is green on macOS, Linux and Windows CI; a build on the branch after
this task, in `~/bench/beauty-crm-test` from an empty store, has the same node and edge counts as
the build Task 6 copied to `log/g39/branch/graph.json`, and every node label that differs is a
heading that lost a CommonMark closing `#` sequence, each listed. `families=` is
gone from the bench summary line and `track.py` still reads every transcript in
`bench/history/runs.jsonl`'s history.

**Reading.**

## 7 · G38 — the report's own edge

**Gate.** `repograph families` in `~/bench/beauty-crm-test` as Task 8's build left it puts `OQ`
first in the mention-only section and every family with exactly one definition carries
`defined once`; `bench/compare` still reads the JSON.

**Reading.**

## 8 · G32 — the fixture copy, rebuilt and enriched

**Gate.** *Before* — the four arms on the pinned fixture `~/bench/beauty-crm-502e8a6d` read the
baseline table of §2 exactly; *rebuilt* — in `~/bench/beauty-crm-test` after `reset.sh`, `build`
prints the family line and ``N requirement-like nodes have no questions — run `repograph enrich` ``
with N = eligible − covered from the bench line, and both recorded arms print `enriched=false`;
*after* — one `enrich` (≈ $0.20 of haiku) brings coverage to ≥ 99%, both recorded arms print
`enriched=true gated=true` and exit 0, the developer arms are recorded, and the enriched store is
copied to `log/g32/after/store/` before anything else runs in the directory. Coverage still under
99% after a second `enrich` is a failed gate, recorded with the count the model declined.

**Reading.**

## 9 · G19 — a chunk budgeted in tokens

**Gate.** Control: `repograph-main embed` in `~/bench/beauty-crm-test` after `reset.sh`, the
vectors removed before each run by `bench/probe/embed.sh`, three runs under it and `caffeinate -di`,
wall / user / max RSS / peak CPU and the stamped cadence. Candidate: the branch binary, three runs
in the same directory after a second reset and one warm `--no-dense ask` for the branch's own
mirror, vectors removed before each.
Pass when, on the candidate's median run, every interval is under 60 s including the first (from
process start to the first `dense: N/M rows` line), the longest interval is at most 1.3 × the
median interval, the median wall lies inside the control's [min, max] or within 10% of the
control's median (whichever is wider), median max RSS within 5% and median peak CPU within 10% of
the control's. Small model only; the large-model cadence is the 2.1 GB download this branch does
not take. A candidate that misses a clause is recorded, and the bar is not moved.

**Reading.**

## 10 · What did not close
```

- [x] **Step 6: Fill in the machine line**

```bash
sysctl -n hw.model hw.ncpu hw.memsize && sw_vers -productVersion
```

Replace `(filled in by Task 0 step 6)` in the results document with the four values on one line, e.g. `Mac15,6, 12 cores, 32 GB, macOS 26.x`.

- [x] **Step 7: Commit**

```bash
cd /Users/max/Documents/projects/repograph && git add bench/probe/arms.sh docs/bench/2026-09-10-next-version-levers-results.md && git commit -m "docs(bench): the gates of the next-version levers, written before any run"
```

---

### Task 1: G23 — the reader bars vendored, and proved by their own control

> **⚠ These scripts already exist and every fenced block in this task is out of date.** `bench/probe/` in the tree is authoritative from `a53e50c` onwards; the blocks below are the initial draft, and each of the commits listed in the note under **Spec** at the top of this document corrected a defect that is still in them — the ones this task's own files carry are `994eae3`, `deceb5c`, `8b642d8`, `8061a6f`, `765761f`, `7bd637a`, `ad05b04`, `6ed52ab`, `12dcb9a`, `79a0ef1`, `db523da` and `fa1e2d6`. **Do not re-extract a script from this document**, with `awk` or otherwise: installing a block byte-for-byte reinstalls every one of those defects, which is how the trap was found. Read the files, edit them in place, and leave the blocks below as the record of what was first written.

**Files:**
- Create: `bench/probe/README.md`, `bench/probe/measure.sh`, `bench/probe/quiet.sh`, `bench/probe/reset.sh`, `bench/probe/test_reset.py`, `bench/probe/readers.sh`, `bench/probe/embed.sh`, `bench/probe/judge.py`, `bench/probe/test_judge.py`, `bench/probe/graphdiff.py`, `bench/probe/test_graphdiff.py`, `bench/probe/queries10.jsonl`
- Modify: `docs/bench/2026-09-10-next-version-levers-results.md` (§1 reading)

**Interfaces:**
- Consumes: `bin/repograph-main` from Task 0; `FIX` and `WT` as Task 0 Step 3 found them.
- Produces: `reset.sh` (reads `FIX`, `WT`, `PIN`, `BIN` from the environment, defaults to this machine's paths and `bin/repograph-main`; exit 2 and nothing touched when `$WT` is not the locked, detached worktree at `$PIN` or is `$FIX`; exit 1 when the settle walk changed anything or its counts differ from the fixture's `graph.json`); `readers.sh BIN WORKTREE LOG [N]` (ten rows, every row `--stale`); `embed.sh NAME BIN WORKTREE LOG` (refuses the fixture by path); `judge.py medians <summary.txt>` → lines `row wall=<s> maxrss=<GB> peak_cpu=<%> n=<k>`; `judge.py control <A> <B>` (exit 1 on any row outside 10%/5%); `judge.py compare <ref> <new>` (same bars, deltas signed); `judge.py cadence <stamped.err>` → `first=<s> max=<s> median=<s> ratio=<x> n=<k>`; `graphdiff.py <a.json> <b.json>` → `nodes: same|differ (…)`, `edges: same|differ (…)`, `pending: <n>`, `bytes: <a> <b> <ratio>`. Later tasks (6, 8, 11) call these.

**The decision, stated:** the kit at `~/bench/resources-2026-09-07` is vendored into `bench/probe/`, because the bars it sets are what every cost-family reading is judged against, and a harness nobody can review in a diff is part of the gap it measures. What is vendored is the scripts and the ten queries; the store copies and 41 MB legacy graph stay out of tree, and the `import-legacy` row is dropped from the reader suite because it writes the store — the one row that could move the index under the suite, which is the cause G23 names. Two more things change because the suite reads the fixture's store where the rule puts writers (see "One writable directory"): every row is `--stale` — the four `ask` rows, `impact`, `trace`, `changes` — so no row can walk, and the suite writes nothing, which Step 8 proves with the store's checksums; and the kit's `ask-stale` row goes, since it is now the same command as `ask-fused`. The walk a non-stale reader pays is not in the suite any more; its number is Task 3's `tree walked` stage. `changes` keeps its one touched line in `packages/ui/src/lib/cn.ts`, copied aside with `cp -p` and moved back, so the tree is put back to the byte and to the nanosecond stamp (`cp -p` keeps it on this machine; a plain `cp` would leave a new mtime behind).

- [x] **Step 1: Write the failing tests for `judge.py`**

Write `bench/probe/test_judge.py`:

```python
import unittest

import judge

SUMMARY = """\
ask-fused-1  wall=0.61s user=0.40s sys=0.20s maxrss=1.55GB peak_cpu=120% peak_threads=8 samples=1
ask-fused-2  wall=0.75s user=0.41s sys=0.21s maxrss=1.56GB peak_cpu=118% peak_threads=8 samples=1
ask-fused-3  wall=0.60s user=0.40s sys=0.20s maxrss=1.36GB peak_cpu=121% peak_threads=8 samples=1
impact-1  wall=0.04s user=0.02s sys=0.01s maxrss=0.05GB peak_cpu=0% peak_threads=1 samples=0
impact-2  wall=0.05s user=0.02s sys=0.01s maxrss=0.05GB peak_cpu=0% peak_threads=1 samples=0
impact-3  wall=0.04s user=0.02s sys=0.01s maxrss=0.05GB peak_cpu=0% peak_threads=1 samples=0
"""

STAMPED = """\
100.00 start
100.40 dense: model open in 0.4s
100.90 dense: 128/33525 rows, 256.0 rows/s, ~2 min left
103.90 dense: 512/33525 rows, 170.7 rows/s, ~3 min left
109.90 dense: 1536/33525 rows, 170.7 rows/s, ~3 min left
118.20 dense: 33525/33525 rows, 180.0 rows/s, ~0 min left
118.30 dense: embedded 33525 rows in 18.3s
118.31        18.31 real        60.00 user         2.00 sys
118.31           1740000000  maximum resident set size
"""


class Medians(unittest.TestCase):
    def test_runs_of_one_row_are_grouped_and_the_median_taken(self):
        m = judge.medians(SUMMARY.splitlines())
        self.assertEqual(m["ask-fused"], {"wall": 0.61, "maxrss": 1.55, "peak_cpu": 120.0, "n": 3})
        self.assertEqual(m["impact"]["wall"], 0.04)

    def test_an_even_count_takes_the_upper_middle_like_bench_does(self):
        self.assertEqual(judge.median([1.0, 2.0, 3.0, 4.0]), 3.0)

    def test_a_medians_file_reads_back_as_written(self):
        import os
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "m.txt")
            with open(p, "w") as f:
                f.write("ask-fused wall=0.61 maxrss=1.55 peak_cpu=120.0 n=5\n")
            self.assertEqual(judge.read_medians(p)["ask-fused"], {"wall": 0.61, "maxrss": 1.55, "peak_cpu": 120.0, "n": 5})

    def test_a_summary_file_of_runs_reads_as_medians_too(self):
        import os
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "s.txt")
            with open(p, "w") as f:
                f.write(SUMMARY)
            self.assertEqual(judge.read_medians(p)["impact"]["n"], 3)


class Control(unittest.TestCase):
    def test_the_same_binary_twice_inside_the_bars_passes(self):
        a = {"ask-fused": {"wall": 0.60, "maxrss": 1.50, "peak_cpu": 120.0, "n": 5}}
        b = {"ask-fused": {"wall": 0.63, "maxrss": 1.55, "peak_cpu": 121.0, "n": 5}}
        rows = judge.control(a, b)
        self.assertEqual(rows, [("ask-fused", 0.05, 0.0333, True)])

    def test_a_row_outside_either_bar_fails_and_names_which(self):
        a = {"dump10": {"wall": 0.70, "maxrss": 1.36, "peak_cpu": 100.0, "n": 5}}
        b = {"dump10": {"wall": 0.90, "maxrss": 1.36, "peak_cpu": 100.0, "n": 5}}
        (row, wall, rss, ok), = judge.control(a, b)
        self.assertFalse(ok)
        self.assertAlmostEqual(wall, 0.2857, places=4)
        self.assertEqual(rss, 0.0)

    def test_a_row_missing_from_one_side_is_reported_not_skipped(self):
        with self.assertRaises(SystemExit):
            judge.control({"a": {"wall": 1, "maxrss": 1, "peak_cpu": 1, "n": 1}}, {})


class Cadence(unittest.TestCase):
    def test_intervals_are_read_from_the_stamps_not_from_rows_per_second(self):
        c = judge.cadence(STAMPED.splitlines())
        self.assertEqual(c["first"], 0.9)
        self.assertEqual(c["intervals"], [0.9, 3.0, 6.0, 8.3])
        self.assertEqual(c["max"], 8.3)
        self.assertEqual(c["median"], 6.0)
        self.assertAlmostEqual(c["ratio"], 8.3 / 6.0, places=3)

    def test_the_first_interval_counts_from_process_start_not_from_model_open(self):
        lines = ["50.00 start", "80.00 dense: model open in 30.0s", "85.00 dense: 128/9 rows, 1.0 rows/s, ~1 min left"]
        self.assertEqual(judge.cadence(lines)["first"], 35.0)

    def test_a_transcript_with_no_progress_line_is_refused(self):
        with self.assertRaises(SystemExit):
            judge.cadence(["1.00 start", "2.00 dense: embedded 0 rows in 1.0s"])


class Verdict(unittest.TestCase):
    def test_cadence_verdict_needs_every_interval_under_the_bar_and_a_flat_tail(self):
        ok, why = judge.cadence_ok({"first": 0.9, "intervals": [0.9, 3.0, 6.0, 8.3], "max": 8.3, "median": 6.0, "ratio": 8.3 / 6.0}, bar=60.0, tail=1.3)
        self.assertFalse(ok)
        self.assertIn("1.383", why)
        ok, _ = judge.cadence_ok({"first": 0.9, "intervals": [5.0, 6.0], "max": 6.0, "median": 6.0, "ratio": 1.0}, bar=60.0, tail=1.3)
        self.assertTrue(ok)


if __name__ == "__main__":
    unittest.main()
```

- [x] **Step 2: Run them to see them fail**

```bash
cd /Users/max/Documents/projects/repograph && python3 -m unittest discover -s bench/probe 2>&1 | tail -3
```

Expected: `ModuleNotFoundError: No module named 'judge'`.

- [x] **Step 3: Write `judge.py`**

Write `bench/probe/judge.py`:

```python
#!/usr/bin/env python3
"""The arithmetic behind the cost-family bars, in one reviewable place.

A single reading of a reader row cannot be judged: on 2026-09-07 the same command through the
same binary read 0.61 s and 0.75 s, and max RSS bounced between 1.36 and 1.56 GB on both sides of
a control. So a row is a median of n runs, and a bar is usable only once the same binary run
through the suite twice reads inside it. This file is what says so, and the tests beside it are
what a reader checks instead of the shell that produced a table.
"""

import re
import statistics
import sys
from pathlib import Path

WALL_BAR = 0.10
RSS_BAR = 0.05
CPU_BAR = 0.10

# `NAME-i  wall=0.61s user=… maxrss=1.55GB peak_cpu=120% …` from measure.sh; the unit suffixes
# are stripped so the numbers compare, and `NAME-i` is `NAME` with its run index.
RUN = re.compile(r"^(\S+?)-(\d+)\s+(.*)$")
FIELD = re.compile(r"(\w+)=([0-9.]+)")
STAMP = re.compile(r"^(\d+\.\d+)\s+(.*)$")
PROGRESS = re.compile(r"^dense: (\d+)/(\d+) rows")


def median(xs):
    """The upper middle on an even count, the way `bench`'s p90 and `median` are taken."""
    s = sorted(xs)
    return s[len(s) // 2]


def fields(text):
    return {k: float(v) for k, v in FIELD.findall(text)}


def medians(lines):
    runs = {}
    for line in lines:
        m = RUN.match(line.strip())
        if not m:
            continue
        runs.setdefault(m.group(1), []).append(fields(m.group(3)))
    out = {}
    for row, rs in runs.items():
        out[row] = {
            "wall": median(r["wall"] for r in rs),
            "maxrss": median(r["maxrss"] for r in rs),
            "peak_cpu": median(r["peak_cpu"] for r in rs),
            "n": len(rs),
        }
    return out


# `row wall=0.61 maxrss=1.55 peak_cpu=120.0 n=5`: what `medians` prints, and what `control` and
# `compare` read back. A run line carries `-<i>` after the row and a `user=` field; a medians
# line carries neither, so the two shapes cannot be confused for each other.
MEDIAN_LINE = re.compile(r"^(\S+)\s+(wall=[0-9.]+ maxrss=[0-9.]+ peak_cpu=[0-9.]+ n=\d+)$")


def read_medians(path):
    lines = Path(path).read_text().splitlines()
    out = {}
    for line in lines:
        m = MEDIAN_LINE.match(line.strip())
        if m:
            f = fields(m.group(2))
            out[m.group(1)] = {"wall": f["wall"], "maxrss": f["maxrss"], "peak_cpu": f["peak_cpu"], "n": int(f["n"])}
    return out or medians(lines)


def spread(a, b):
    return 0.0 if a == 0 else abs(b - a) / a


def control(a, b, wall_bar=WALL_BAR, rss_bar=RSS_BAR):
    """The same binary twice: every row's spread against the bars it will later judge with."""
    out = []
    for row in a:
        if row not in b:
            raise SystemExit(f"{row}: present in one control run and not the other — the suites differ")
        w = round(spread(a[row]["wall"], b[row]["wall"]), 4)
        r = round(spread(a[row]["maxrss"], b[row]["maxrss"]), 4)
        out.append((row, w, r, w <= wall_bar and r <= rss_bar))
    return out


def compare(ref, new, wall_bar=WALL_BAR, rss_bar=RSS_BAR):
    """A candidate against the reference, deltas signed so a reader sees which way it moved."""
    out = []
    for row in ref:
        if row not in new:
            raise SystemExit(f"{row}: in the reference and not in the candidate")
        w = round((new[row]["wall"] - ref[row]["wall"]) / ref[row]["wall"], 4) if ref[row]["wall"] else 0.0
        r = round((new[row]["maxrss"] - ref[row]["maxrss"]) / ref[row]["maxrss"], 4) if ref[row]["maxrss"] else 0.0
        out.append((row, w, r, abs(w) <= wall_bar and abs(r) <= rss_bar))
    return out


def cadence(lines):
    """Intervals between progress lines, from wall-clock stamps `embed.sh` puts on stderr.

    Stamps and not `rows/s`: a laptop that slept mid-run does not advance the rate, and the run
    that closed G19 on 2026-09-09 had to reconstruct its intervals for exactly that reason.
    """
    start = None
    marks = []
    for line in lines:
        m = STAMP.match(line.rstrip())
        if not m:
            continue
        t, rest = float(m.group(1)), m.group(2)
        if rest == "start":
            start = t
        elif PROGRESS.match(rest):
            marks.append(t)
    if start is None or not marks:
        raise SystemExit("no `start` stamp or no progress line — the run did not embed anything")
    intervals = [round(marks[0] - start, 2)] + [round(b - a, 2) for a, b in zip(marks, marks[1:])]
    med = median(intervals)
    return {"first": intervals[0], "intervals": intervals, "max": max(intervals), "median": med,
            "ratio": max(intervals) / med if med else float("inf")}


def cadence_ok(c, bar=60.0, tail=1.3):
    if c["max"] >= bar:
        return False, f"an interval of {c['max']} s is not under {bar} s"
    if c["ratio"] > tail:
        return False, f"the longest interval is {c['ratio']:.3f} × the median, over {tail}"
    return True, "every interval under the bar and the tail flat"


def print_table(rows, head):
    print(f"| row | {head[0]} | {head[1]} | |")
    print("|---|---|---|---|")
    for row, w, r, ok in rows:
        print(f"| {row} | {w:+.1%} | {r:+.1%} | {'ok' if ok else 'OUTSIDE'} |")
    return all(ok for _, _, _, ok in rows)


def main(argv):
    if len(argv) < 3:
        raise SystemExit("usage: judge.py medians SUMMARY | control A B | compare REF NEW | cadence STAMPED")
    cmd = argv[1]
    if cmd == "medians":
        for row, m in read_medians(argv[2]).items():
            print(f"{row} wall={m['wall']} maxrss={m['maxrss']} peak_cpu={m['peak_cpu']} n={m['n']}")
        return 0
    if cmd == "control":
        return 0 if print_table(control(read_medians(argv[2]), read_medians(argv[3])), ("wall spread", "RSS spread")) else 1
    if cmd == "compare":
        return 0 if print_table(compare(read_medians(argv[2]), read_medians(argv[3])), ("wall Δ", "RSS Δ")) else 1
    if cmd == "cadence":
        c = cadence(Path(argv[2]).read_text().splitlines())
        ok, why = cadence_ok(c)
        print(f"first={c['first']} max={c['max']} median={c['median']} ratio={c['ratio']:.3f} n={len(c['intervals'])}")
        print(f"intervals={c['intervals']}")
        print(("ok: " if ok else "OUTSIDE: ") + why)
        return 0 if ok else 1
    raise SystemExit(f"unknown command {cmd}")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
```

- [x] **Step 4: Run the tests to see them pass**

```bash
cd /Users/max/Documents/projects/repograph && python3 -m unittest discover -s bench/probe 2>&1 | tail -3
```

Expected: `OK`.

- [x] **Step 5: Write the failing test for `graphdiff.py`, then the script**

Write `bench/probe/test_graphdiff.py`:

```python
import json
import os
import tempfile
import unittest

import graphdiff


def graph(nodes, edges, pending=None):
    g = {"nodes": {n: {"id": n, "kind": "Requirement", "label": n, "file": "docs/a.md", "line": 1} for n in nodes},
         "edges": [{"source": s, "target": t, "kind": "References", "context": "body", "file": "docs/a.md"} for s, t in edges]}
    if pending is not None:
        g["pending"] = [{"source": s, "target": t, "kind": "References", "context": "body", "file": "docs/a.md"} for s, t in pending]
    return g


class Diff(unittest.TestCase):
    def write(self, d, name, g):
        p = os.path.join(d, name)
        with open(p, "w") as f:
            json.dump(g, f)
        return p

    def test_the_same_visible_graph_with_edges_held_aside_reads_same(self):
        with tempfile.TemporaryDirectory() as d:
            a = self.write(d, "a.json", graph(["FR-1"], [("FR-1", "FR-2")]))
            b = self.write(d, "b.json", graph(["FR-1"], [("FR-1", "FR-2")], pending=[("FR-1", "ISO-8601")]))
            r = graphdiff.diff(a, b)
            self.assertEqual((r["nodes_same"], r["edges_same"], r["pending"]), (True, True, 1))
            self.assertGreater(r["bytes_ratio"], 1.0)

    def test_a_missing_or_extra_edge_is_named(self):
        with tempfile.TemporaryDirectory() as d:
            a = self.write(d, "a.json", graph(["FR-1"], [("FR-1", "FR-2")]))
            b = self.write(d, "b.json", graph(["FR-1"], [("FR-1", "FR-3")]))
            r = graphdiff.diff(a, b)
            self.assertFalse(r["edges_same"])
            self.assertEqual(r["edges_only_a"], ["FR-1 -> FR-2 [References/body] docs/a.md"])
            self.assertEqual(r["edges_only_b"], ["FR-1 -> FR-3 [References/body] docs/a.md"])

    def test_a_node_whose_label_changed_is_named_with_both_labels(self):
        with tempfile.TemporaryDirectory() as d:
            ga, gb = graph(["FR-1"], []), graph(["FR-1"], [])
            gb["nodes"]["FR-1"]["label"] = "title"
            a, b = self.write(d, "a.json", ga), self.write(d, "b.json", gb)
            r = graphdiff.diff(a, b)
            self.assertFalse(r["nodes_same"])
            self.assertEqual(r["nodes_changed"], [("FR-1", "label", "FR-1", "title")])


if __name__ == "__main__":
    unittest.main()
```

Run: `python3 -m unittest discover -s bench/probe` → `ModuleNotFoundError: No module named 'graphdiff'`. Then write `bench/probe/graphdiff.py`:

```python
#!/usr/bin/env python3
"""Are two `graph.json` files the same visible graph, and how much does one hold aside.

Written for G39's gate — an extractor rewritten to read ids by one generic grammar has to write
the graph every reader sees byte-for-byte as before, and the citations it now keeps aside are the
whole of the growth — but general: `nodes` and `edges` are compared field by field, `pending` is
counted, and the sizes are reported as a ratio.
"""

import json
import os
import sys


def edge_key(e):
    return f"{e['source']} -> {e['target']}" + (f" [{e['kind']}/{e.get('context', '')}] {e['file']}")


def diff(a_path, b_path):
    a, b = json.load(open(a_path)), json.load(open(b_path))
    ea, eb = {edge_key(e) for e in a["edges"]}, {edge_key(e) for e in b["edges"]}
    changed = []
    for nid in sorted(set(a["nodes"]) & set(b["nodes"])):
        na, nb = a["nodes"][nid], b["nodes"][nid]
        for k in sorted(set(na) | set(nb)):
            if na.get(k) != nb.get(k):
                changed.append((nid, k, na.get(k), nb.get(k)))
    return {
        "nodes_same": set(a["nodes"]) == set(b["nodes"]) and not changed,
        "nodes_only_a": sorted(set(a["nodes"]) - set(b["nodes"])),
        "nodes_only_b": sorted(set(b["nodes"]) - set(a["nodes"])),
        "nodes_changed": changed,
        "edges_same": ea == eb,
        "edges_only_a": sorted(ea - eb),
        "edges_only_b": sorted(eb - ea),
        "pending": len(b.get("pending", [])),
        "pending_a": len(a.get("pending", [])),
        "bytes_ratio": os.path.getsize(b_path) / os.path.getsize(a_path),
        "counts": (len(a["nodes"]), len(a["edges"]), len(b["nodes"]), len(b["edges"])),
    }


def main(argv):
    if len(argv) != 3:
        raise SystemExit("usage: graphdiff.py A.json B.json")
    r = diff(argv[1], argv[2])
    na, ea, nb, eb = r["counts"]
    print(f"nodes: {'same' if r['nodes_same'] else 'differ'} ({na} vs {nb}; only in A {len(r['nodes_only_a'])}, only in B {len(r['nodes_only_b'])}, changed {len(r['nodes_changed'])})")
    for nid, k, va, vb in r["nodes_changed"][:20]:
        print(f"  {nid}.{k}: {va!r} -> {vb!r}")
    print(f"edges: {'same' if r['edges_same'] else 'differ'} ({ea} vs {eb}; only in A {len(r['edges_only_a'])}, only in B {len(r['edges_only_b'])})")
    for e in (r["edges_only_a"][:10] + r["edges_only_b"][:10]):
        print(f"  {e}")
    print(f"pending: {r['pending']} (A held {r['pending_a']})")
    print(f"bytes: ratio {r['bytes_ratio']:.3f}")
    return 0 if r["nodes_same"] and r["edges_same"] else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
```

Run the tests: expected `OK`.

- [x] **Step 6: Write the failing test for `reset.sh`, then the script**

Write `bench/probe/test_reset.py`. It pins the contract on a throwaway repository — what the script refuses, and what a reset leaves behind — with the settle walk a stub that prints what `update` prints: the script's own job ends where the binary's begins, and the counts it checks are read off that line. The whole class is skipped off POSIX, because the script is bash for a macOS kit and CI never discovers `bench/probe`; the skip keeps `discover` green anywhere.

```python
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
RESET = HERE / "reset.sh"
SMALL = 'embed_model = "intfloat/multilingual-e5-small"\n'
FIXTURE_GRAPH = '{"nodes": {"FR-1": {}}, "edges": []}'


def git(*args, cwd):
    return subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True, text=True).stdout


@unittest.skipUnless(os.name == "posix", "bash, git worktree and rsync: the macOS kit's own platform")
class Reset(unittest.TestCase):
    """The contract of reset.sh on a throwaway repository: what it refuses, and what a reset
    leaves behind. The settle walk is a stub that prints what `update` prints — the script's
    own job ends where the binary's begins, and the counts it checks are read off that line."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.repo = self.tmp / "repo"
        self.repo.mkdir()
        git("init", "-q", cwd=self.repo)
        git("config", "user.email", "t@t", cwd=self.repo)
        git("config", "user.name", "t", cwd=self.repo)
        (self.repo / "docs").mkdir()
        (self.repo / "docs/a.md").write_text("# a\n")
        (self.repo / ".gitignore").write_text(".repograph/\n")
        git("add", ".", cwd=self.repo)
        git("commit", "-q", "-m", "one", cwd=self.repo)
        self.sha = git("rev-parse", "HEAD", cwd=self.repo).strip()
        self.wt = self.tmp / "wt"
        git("worktree", "add", "-q", "--detach", str(self.wt), self.sha, cwd=self.repo)
        git("worktree", "lock", str(self.wt), cwd=self.repo)
        self.fix = self.tmp / "fix"
        (self.fix / ".repograph").mkdir(parents=True)
        (self.fix / ".repograph/graph.json").write_text(FIXTURE_GRAPH)
        (self.fix / ".repograph/manifest.json").write_text('{"files": {}}')
        (self.fix / ".repograph/vectors.f32").write_bytes(b"v")
        self.calls = self.tmp / "calls"
        self.bin = self.tmp / "repograph-stub"
        self.stub("changed 0 removed 0 nodes 1 edges 0")

    def stub(self, line):
        self.bin.write_text(f'#!/bin/sh\necho "$@" >> "{self.calls}"\necho "{line}"\n')
        self.bin.chmod(0o755)

    def run_reset(self, **env):
        e = {**os.environ, "FIX": str(self.fix), "WT": str(self.wt), "PIN": self.sha[:8], "BIN": str(self.bin), **env}
        return subprocess.run(["bash", str(RESET)], env=e, capture_output=True, text=True)

    def dirty(self):
        (self.wt / "docs/a.md").write_text("# edited\n")
        (self.wt / "docs/oq.md").write_text("**OQ-1**\n")
        (self.wt / ".repograph").mkdir(exist_ok=True)
        (self.wt / ".repograph/graph.json").write_text("{}")
        (self.wt / ".repograph/left-by-an-arm").write_text("x")
        (self.wt / "repograph.toml").write_text('embed_model = "wrong"\n')

    def test_a_dirty_tree_and_a_foreign_store_are_put_back(self):
        self.dirty()
        r = self.run_reset()
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertEqual((self.wt / "docs/a.md").read_text(), "# a\n")
        self.assertFalse((self.wt / "docs/oq.md").exists())
        self.assertEqual((self.wt / ".repograph/graph.json").read_text(), FIXTURE_GRAPH)
        self.assertEqual((self.wt / ".repograph/vectors.f32").read_bytes(), b"v")
        self.assertFalse((self.wt / ".repograph/left-by-an-arm").exists(), "--delete: what an arm left is gone")
        self.assertEqual((self.wt / "repograph.toml").read_text(), SMALL)
        self.assertEqual(self.calls.read_text(), f"--repo {self.wt} --no-dense update\n")
        self.assertIn("settle  changed 0 removed 0 nodes 1 edges 0", r.stdout)

    def test_an_unlocked_worktree_is_refused_and_nothing_is_touched(self):
        git("worktree", "unlock", str(self.wt), cwd=self.repo)
        self.dirty()
        r = self.run_reset()
        self.assertEqual(r.returncode, 2, r.stdout + r.stderr)
        self.assertIn("not locked", r.stderr)
        self.assertEqual((self.wt / "docs/a.md").read_text(), "# edited\n")
        self.assertFalse(self.calls.exists())

    def test_a_worktree_at_another_commit_is_refused(self):
        r = self.run_reset(PIN="0000000")
        self.assertEqual(r.returncode, 2, r.stderr)
        self.assertIn("is not at 0000000", r.stderr)

    def test_the_fixture_cannot_be_named_as_the_target(self):
        r = self.run_reset(FIX=str(self.wt))
        self.assertEqual(r.returncode, 2, r.stderr)
        self.assertIn("same directory", r.stderr)

    def test_a_directory_that_is_no_worktree_is_refused(self):
        plain = self.tmp / "plain"
        plain.mkdir()
        r = self.run_reset(WT=str(plain))
        self.assertEqual(r.returncode, 2, r.stderr)
        self.assertIn("not a registered git worktree", r.stderr)

    def test_a_fixture_without_a_store_is_refused(self):
        (self.fix / ".repograph/graph.json").unlink()
        r = self.run_reset()
        self.assertEqual(r.returncode, 2, r.stderr)
        self.assertIn("not a store to restore from", r.stderr)

    def test_a_settle_that_changes_anything_or_miscounts_fails(self):
        self.stub("changed 1 removed 0 nodes 1 edges 0")
        r = self.run_reset()
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("changed something", r.stderr)
        self.stub("changed 0 removed 0 nodes 2 edges 0")
        r = self.run_reset()
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("fixture holds 1 0", r.stderr)


if __name__ == "__main__":
    unittest.main()
```

Run: `python3 -m unittest discover -s bench/probe` → seven errors, each `No such file or directory: 'bench/probe/reset.sh'` from `subprocess`. Then write `bench/probe/reset.sh`:

```bash
#!/bin/bash
# reset.sh — the one writable beauty-crm worktree put back to a known state before a writer arm.
#
# One directory holds every writer on this branch, so arms are sequential and each starts from
# whatever the last one left unless something puts the fixture's state back. This does: tracked
# edits reverted and untracked files removed (the store and repograph.toml excepted — both are
# rewritten below), the pinned fixture's store copied in whole, repograph.toml written so that no
# enrich or embed reaches for weights this machine does not have, and one --no-dense update to
# settle the stamps: the copied manifest carries the fixture's mtimes, which match nothing here,
# so that first walk hashes every file, finds every hash unchanged, records this tree's stamps,
# and every later walk is stat-only. `repograph-main` takes the no-op path on an unchanged tree
# and derives nothing, which is why the settle reads `changed 0` and is not a re-read.
#
# Refuses with exit 2 and nothing touched unless WT is the locked, detached worktree at PIN and
# is not FIX — so a mistyped path cannot silently become the target. Exit 1 when the settle walk
# changed anything or its counts differ from the fixture's graph.json: the tree is not the one
# the store was built from, and nothing measured on it would be comparable.
#
# usage: reset.sh     (FIX, WT, PIN and BIN from the environment; the defaults are this machine's)
set -u
FIX=${FIX:-$HOME/bench/beauty-crm-502e8a6d}
WT=${WT:-$HOME/bench/beauty-crm-test}
PIN=${PIN:-502e8a6d}
BIN=${BIN:-$HOME/bench/levers-2026-09-10/bin/repograph-main}

refuse() { echo "reset: refusing — $1" >&2; exit 2; }

fix=$(cd "$FIX" 2>/dev/null && pwd -P) || refuse "FIX=$FIX is not a directory"
wt=$(cd "$WT" 2>/dev/null && pwd -P) || refuse "WT=$WT is not a directory"
[ "$wt" != "$fix" ] || refuse "WT and FIX are the same directory ($wt)"
[ -f "$fix/.repograph/graph.json" ] || refuse "$fix/.repograph/graph.json is missing — not a store to restore from"
[ -x "$BIN" ] || refuse "BIN=$BIN is not executable"
block=$(git -C "$wt" worktree list --porcelain 2>/dev/null | awk -v w="$wt" 'BEGIN{RS=""; FS="\n"} $1 == "worktree " w {print; exit}')
[ -n "$block" ] || refuse "$wt is not a registered git worktree"
printf '%s\n' "$block" | grep -q "^HEAD $PIN" || refuse "$wt is not at $PIN (HEAD $(printf '%s\n' "$block" | awk '/^HEAD/ {print substr($2, 1, 8)}'))"
printf '%s\n' "$block" | grep -q '^detached$' || refuse "$wt is on a branch, not detached at $PIN"
printf '%s\n' "$block" | grep -q '^locked' || refuse "$wt is not locked — lock it, or this is the wrong directory"

# `reset --hard` rather than `checkout -- .`: a staged edit survives the latter.
git -C "$wt" reset -q --hard HEAD || exit 1
git -C "$wt" clean -fdq -e .repograph -e repograph.toml || exit 1
left=$(git -C "$wt" status --porcelain | grep -v '^?? repograph.toml$')
[ -z "$left" ] || { echo "reset: the tree did not come clean:" >&2; echo "$left" >&2; exit 1; }
rsync -a --delete "$fix/.repograph/" "$wt/.repograph/" || exit 1
printf 'embed_model = "intfloat/multilingual-e5-small"\n' > "$wt/repograph.toml"
echo "reset: $wt at $PIN, tree clean, store restored from $fix:"
ls -l "$wt/.repograph" | awk 'NR > 1 {printf "  %10s  %s\n", $5, $9}'
out=$("$BIN" --repo "$wt" --no-dense update) || exit 1
echo "reset: settle  $out"
case "$out" in "changed 0 removed 0 "*) ;; *) echo "reset: the settle walk changed something — this tree is not the fixture's: $out" >&2; exit 1;; esac
want=$(python3 -c 'import json, sys; g = json.load(open(sys.argv[1])); print(len(g["nodes"]), len(g["edges"]))' "$fix/.repograph/graph.json")
got=$(echo "$out" | awk '/^changed/ {print $6, $8}')
[ "$got" = "$want" ] || { echo "reset: nodes/edges $got after the settle, fixture holds $want" >&2; exit 1; }
```

```bash
chmod +x /Users/max/Documents/projects/repograph/bench/probe/reset.sh && cd /Users/max/Documents/projects/repograph && python3 -m unittest discover -s bench/probe 2>&1 | tail -1
```

Expected: `OK`. The script and the seven tests were run on this machine on 2026-09-10 against a throwaway repository, and the guards were run against the real `$WT` with a mismatched pin and a missing binary: exit 2 both times, status count still `0`. `.repograph/` is in the corpus's `.gitignore`, so `clean` would spare it without the `-e`; `repograph.toml` is not, so its `-e` is load-bearing. `rsync` on this machine is openrsync, which takes `-a --delete`. Note that the settle rewrites `graph.json`, `graph.bin` (`RGM1`) and `manifest.json` — `apply_diff` saves on a no-op — so after a reset the store's bytes are `repograph-main`'s serialisation of the fixture's graph, not the fixture's own bytes; nothing in this plan compares `$WT`'s `graph.json` to the fixture's.

- [x] **Step 7: Vendor the scripts**

```bash
cp ~/bench/resources-2026-09-07/queries10.jsonl /Users/max/Documents/projects/repograph/bench/probe/queries10.jsonl
```

Write `bench/probe/measure.sh` (the kit's, unchanged in what it measures):

```bash
#!/bin/bash
# measure.sh NAME LOGDIR -- cmd args...
# One command under /usr/bin/time -l (wall, user, sys, max RSS) with `top` sampled every second
# for instantaneous CPU% and thread count. Writes NAME.time, NAME.samples, NAME.out and appends
# one summary line to LOGDIR/summary.txt. macOS only: `-l` and `top -l` are Darwin's.
set -u
NAME=$1; LOG=$2; shift 2; [ "$1" = "--" ] && shift
mkdir -p "$LOG"
/usr/bin/time -l "$@" >"$LOG/$NAME.out" 2>"$LOG/$NAME.time" &
WRAP=$!
sleep 0.3
# `time` forks the measured command; a command that finished inside 0.3 s leaves no samples.
PID=$(pgrep -P $WRAP | head -1)
[ -z "$PID" ] && PID=$WRAP
: >"$LOG/$NAME.samples"
while kill -0 "$PID" 2>/dev/null; do
  top -l 2 -s 1 -pid "$PID" -stats pid,cpu,mem,th 2>/dev/null | tail -1 >>"$LOG/$NAME.samples"
done
wait $WRAP
PEAK_CPU=$(awk '{gsub("%","",$2); if ($2+0>m) m=$2+0} END{print m+0}' "$LOG/$NAME.samples")
PEAK_TH=$(awk '{split($4,a,"/"); if (a[1]+0>m) m=a[1]+0} END{print m+0}' "$LOG/$NAME.samples")
RSS=$(awk '/maximum resident set size/{printf "%.2f", $1/1073741824}' "$LOG/$NAME.time")
WALL=$(awk '/real/{print $1}' "$LOG/$NAME.time")
USR=$(awk '/real/{print $3}' "$LOG/$NAME.time")
SYS=$(awk '/real/{print $5}' "$LOG/$NAME.time")
N=$(wc -l <"$LOG/$NAME.samples" | tr -d ' ')
echo "$NAME  wall=${WALL}s user=${USR}s sys=${SYS}s maxrss=${RSS}GB peak_cpu=${PEAK_CPU}% peak_threads=${PEAK_TH} samples=${N}" | tee -a "$LOG/summary.txt"
```

Write `bench/probe/quiet.sh`:

```bash
#!/bin/bash
# The precondition every probe row is read under, stated the way each row states the idle
# baseline it is read against. Exit 1 with the reading when the machine is not quiet: a reading
# taken on a busy laptop measures the laptop, which is what G23 found.
set -u
IDLE=$(top -l 2 -s 1 | grep 'CPU usage' | tail -1 | sed 's/.*, \([0-9.]*\)% idle.*/\1/')
LOAD1=$(sysctl -n vm.loadavg | awk '{print $2}')
AC=$(pmset -g batt | head -1 | grep -c "AC Power")
# A build or another store's run is what moves a reading; the agent harness that launched this
# script is a `node` process and is not, so it is not in the pattern.
BUSY=$(pgrep -l 'cargo|rustc|repograph' | wc -l | tr -d ' ')
echo "quiet: idle=${IDLE}% load1=${LOAD1} ac=${AC} busy_processes=${BUSY}"
ok=1
awk -v i="$IDLE" 'BEGIN{exit !(i+0 >= 85)}' || { echo "  not quiet: idle ${IDLE}% < 85%"; ok=0; }
awk -v l="$LOAD1" 'BEGIN{exit !(l+0 < 3.0)}' || { echo "  not quiet: load1 ${LOAD1} >= 3.0"; ok=0; }
[ "$AC" = "1" ] || { echo "  not quiet: on battery"; ok=0; }
[ "$BUSY" = "0" ] || { echo "  not quiet: $BUSY cargo/rustc/repograph processes running"; pgrep -l 'cargo|rustc|repograph'; ok=0; }
[ "$ok" = "1" ]
```

On 2026-09-10 at planning time this machine read `idle=61.52% load1=6.86 ac=0` — a reading the script refuses, which is the point.

Write `bench/probe/readers.sh`:

```bash
#!/bin/bash
# readers.sh BINARY WORKTREE LOGDIR [N]
# The reader suite, N runs per row (default 5), in the one writable worktree straight after
# reset.sh has put the pinned fixture's store there, the resident server bypassed so every row is
# a cold process. Rows are the 2026-09-07 suite less `import-legacy`, which writes the store, and
# less `ask-stale`, which is every ask row now: every command that could walk the tree is read
# `--stale`, so no row refreshes stamps, rewrites a mirror or resyncs the index under the rows
# after it — the index moving under the suite is the cause G23 named — and the suite writes
# nothing, which the plan proves with the store's checksums before and after. `changes` needs a
# diff to read, so it touches one source file for the row's duration and moves the original back
# with its stamp (`cp -p`). Writes LOGDIR/summary.txt (one line per run) and LOGDIR/medians.txt.
set -u
B=$1; F=$2; L=$3; N=${4:-5}
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$HERE/measure.sh"
export REPOGRAPH_NO_SERVE=1
mkdir -p "$L"
# Not a pipeline: bash 3.2 has no pipefail here and a `| tee` would hand back tee's exit code.
if ! "$HERE/quiet.sh" > "$L/quiet.txt" 2>&1; then cat "$L/quiet.txt"; echo "refusing to measure on a machine that is not quiet" >&2; exit 2; fi
cat "$L/quiet.txt" | tee -a "$L/summary.txt"
cd "$F" || exit 2
Q="как отменить запись и кто платит штраф"
CN=packages/ui/src/lib/cn.ts
for i in $(seq 1 "$N"); do
  "$M" ask-fused-$i "$L" -- "$B" --repo "$F" ask --stale $Q
  "$M" ask-nodense-$i "$L" -- "$B" --repo "$F" --no-dense ask --stale $Q
  "$M" ask-rerank-local-$i "$L" -- "$B" --repo "$F" ask --stale --rerank-local $Q
  "$M" ask-exact-id-$i "$L" -- "$B" --repo "$F" ask --stale FR-PAY-22
  "$M" impact-$i "$L" -- "$B" --repo "$F" impact --stale cn --depth 3
  "$M" trace-$i "$L" -- "$B" --repo "$F" trace --stale main cn --depth 6
  cp -p "$CN" "$CN.orig"
  printf '\n// touched for the changes measurement\n' >> "$CN"
  "$M" changes-$i "$L" -- "$B" --repo "$F" changes --stale --depth 2
  mv "$CN.orig" "$CN"
  "$M" bench-nodense-$i "$L" -- "$B" --repo "$F" --no-dense bench
  "$M" bench-dense-$i "$L" -- "$B" --repo "$F" bench
  "$M" dump10-$i "$L" -- "$B" --repo "$F" dump --queries "$HERE/queries10.jsonl" --out "$L/dump10.json" --depth 300
done
python3 "$HERE/judge.py" medians "$L/summary.txt" | tee "$L/medians.txt"
```

Write `bench/probe/embed.sh`:

```bash
#!/bin/bash
# embed.sh NAME BINARY WORKTREE LOGDIR
# A whole-store embed under the sampler, the store's vectors removed first, every stderr line
# stamped with the wall clock so the cadence is read from when lines arrived and not from the
# rate the line itself quotes. `caffeinate -di` keeps the machine and the display awake: the run
# that closed G19 lost its intervals to a laptop that slept, and `judge.py cadence` refuses a
# transcript with no `start` stamp.
set -u
NAME=$1; B=$2; F=$3; L=$4
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# The one script here that deletes store files, so it refuses the pinned fixture by path: nothing
# writes there, and a mistyped argument is how that rule would get broken.
case "$(cd "$F" && pwd -P)" in "$HOME/bench/beauty-crm-502e8a6d") echo "refusing: $F is the pinned fixture" >&2; exit 2;; esac
mkdir -p "$L"
if ! "$HERE/quiet.sh" > "$L/$NAME.quiet" 2>&1; then cat "$L/$NAME.quiet"; echo "refusing to measure on a machine that is not quiet" >&2; exit 2; fi
cat "$L/$NAME.quiet" | tee -a "$L/summary.txt"
rm -f "$F/.repograph/vectors.f32" "$F/.repograph/vectors.json"
STAMP='perl -MTime::HiRes=time -ne '"'"'printf "%.2f %s", time, $_'"'"''
perl -MTime::HiRes=time -e 'printf "%.2f start\n", time' > "$L/$NAME.err"
caffeinate -di /usr/bin/time -l "$B" --repo "$F" embed >"$L/$NAME.out" 2> >(eval "$STAMP" >> "$L/$NAME.err") &
WRAP=$!
sleep 0.3
PID=$(pgrep -f "^$B --repo $F embed" | head -1)
[ -z "$PID" ] && PID=$WRAP
: >"$L/$NAME.samples"
while kill -0 "$PID" 2>/dev/null; do
  top -l 2 -s 1 -pid "$PID" -stats pid,cpu,mem,th 2>/dev/null | tail -1 >>"$L/$NAME.samples"
done
wait $WRAP
sleep 0.5
PEAK_CPU=$(awk '{gsub("%","",$2); if ($2+0>m) m=$2+0} END{print m+0}' "$L/$NAME.samples")
RSS=$(awk '/maximum resident set size/{printf "%.2f", $2/1073741824}' "$L/$NAME.err")
WALL=$(awk '/real/{print $2}' "$L/$NAME.err")
USR=$(awk '/real/{print $4}' "$L/$NAME.err")
echo "$NAME  wall=${WALL}s user=${USR}s maxrss=${RSS}GB peak_cpu=${PEAK_CPU}%" | tee -a "$L/summary.txt"
python3 "$HERE/judge.py" cadence "$L/$NAME.err" | tee -a "$L/summary.txt"
```

The awk fields are one to the right of `measure.sh`'s because every line in `$NAME.err` begins with a stamp. `pgrep -f` finds the binary rather than `time`, so the sampler follows the process that embeds.

Write `bench/probe/README.md`:

```markdown
# The cost-family probe

What every reader and writer costs, measured the same way every time. This directory is the
2026-09-07 kit brought into the tree so the bars it sets can be reviewed in a diff; the pinned
fixture, the one writable worktree and the legacy graph stay under `~/bench/`.

Two directories, both locked: `~/bench/beauty-crm-502e8a6d`, the pinned fixture, read-only for ever
— `bench` and `dump` read its store as it stands, nothing walks it, nothing writes it; and
`~/bench/beauty-crm-test`, the one writable worktree, where every writer and every reader row runs.

- `quiet.sh` — the precondition every row is read under: ≥ 85% idle, 1-minute load < 3.0, AC power,
  no `cargo`/`rustc`/`node`/other `repograph` running. Every other script runs it first and refuses.
- `reset.sh` — the writable worktree put back to the fixture's state: tree reverted and cleaned,
  store copied in, `repograph.toml` written, stamps settled by one `--no-dense update`. Refuses any
  directory that is not the locked worktree at `502e8a6d`. Before every arm, reader suites included.
- `measure.sh NAME LOG -- cmd…` — one command: wall, user, sys, max RSS, sampled peak CPU and threads.
- `readers.sh BIN WORKTREE LOG [N]` — the ten reader rows, N runs each, every row `--stale`, medians
  in `LOG/medians.txt`. In the writable worktree straight after `reset.sh`; the suite writes nothing,
  and the store's checksums before and after are how that is shown.
- `embed.sh NAME BIN WORKTREE LOG` — a whole-store embed with stamped stderr, for the cadence. It
  removes the vectors first, so it refuses the pinned fixture by path.
- `arms.sh BIN REPO LOG TAG` — the four bench arms, recorded into `bench/history/runs.jsonl`; `REPO`
  is the fixture for a baseline read.
- `judge.py` — medians, the twice-through-one-binary control, a candidate against a reference, the
  cadence reading. `graphdiff.py` — two `graph.json` files, same visible graph or not.

The bars: a reader row is judged as a median of n ≥ 5, within 10% wall and 5% max RSS of its
reference, and only once `judge.py control` has shown the same binary twice inside those bars on
this machine (`docs/bench/2026-09-10-next-version-levers-results.md` §1 is the first such control).
A whole-store embed is judged as a median of three against a control of three. macOS only.
```

```bash
chmod +x /Users/max/Documents/projects/repograph/bench/probe/*.sh
```

- [ ] **Step 8: Run the control — the same binary, twice, over a store that must not move**

Wait for the machine to be quiet (`bench/probe/quiet.sh` until it exits 0 — plug in the charger, close the editor's indexers), then, as numbered: (1) reset — `$WT`'s first store, the fixture's; (2) checksum every file of it; (3) suite A; (4) suite B, with no reset between, because a reset between would hide a write and the point is that there is none; (5) checksum again and compare; (6) judge.

```bash
cd /Users/max/Documents/projects/repograph && FIX=~/bench/beauty-crm-502e8a6d; WT=~/bench/beauty-crm-test; export FIX WT
L=~/bench/levers-2026-09-10/log/g23; mkdir -p $L
bench/probe/reset.sh && shasum $WT/.repograph/* > $L/store-before.sha
bench/probe/readers.sh ~/bench/levers-2026-09-10/bin/repograph-main $WT $L/A 5 && bench/probe/readers.sh ~/bench/levers-2026-09-10/bin/repograph-main $WT $L/B 5
shasum $WT/.repograph/* > $L/store-after.sha && cmp $L/store-before.sha $L/store-after.sha && echo "the suite wrote nothing"
python3 bench/probe/judge.py control $L/A/medians.txt $L/B/medians.txt
```

Expected: the reset's `settle  changed 0 removed 0 nodes <…> edges <…>` line; about 2 × (5 × 40 s) of wall including five `--rerank-local` rows of ~32 s each per suite; `cmp` silent and `the suite wrote nothing` printed — seven files, same names, same sums; a table of ten rows with `ok` in every row. `cmp` printing a difference is a row that wrote, and that row is found (`ls -lt $WT/.repograph | head -3` names the file) before anything is judged — nothing in the suite may write, and a suite that did has no reading. A row `OUTSIDE`: re-run both suites once at n = 9 for that row alone (`readers.sh … 9` runs every row; that is the honest re-run, take it), judge again; a row still outside is written into §1 as *not judgeable at this shape on this machine* — the bar is not widened.

- [ ] **Step 9: Record the reading**

Paste under §1 **Reading** in the results document: the reset's settle line, the `quiet:` lines of both runs, the `cmp` outcome, the `judge.py control` table, and the medians of run A as the reference table (`cat $L/A/medians.txt` reformatted as a markdown table with columns row / wall / max RSS / peak CPU / n). Copy `$L/A/medians.txt` to `~/bench/levers-2026-09-10/log/readers-reference.txt`.

- [ ] **Step 10: Tests and commit**

```bash
cd /Users/max/Documents/projects/repograph && python3 -m unittest discover -s bench/probe 2>&1 | tail -1 && git add bench/probe docs/bench/2026-09-10-next-version-levers-results.md && git commit -m "feat(bench): the cost-family probe in the tree, and a control that proves its bars"
```

**Fails to close when:** any reader row is outside 10% / 5% on the same binary twice at n = 9 on a machine `quiet.sh` accepted. That row is recorded as not judgeable and the gap stays open for it; the rows that pass close it for themselves.

---

### Task 2: G15 — the anchor line enters the history

**Files:**
- Modify: `bench/history/track.py` (after `SUMMARY`, in `parse_bench`, `build_row`, `metric_moves`)
- Modify: `bench/history/test_track.py`
- Modify: `bench/history/README.md` (the "A second suite" section)
- Modify: `README.md` (the Bench section, after the `families=` sentence at line 628)

**Interfaces:**
- Produces: history rows carry `"anchors": {kind: [reached, want]}` (or `null` on a transcript without the line); `report` prints `anchors/<kind>: 13/30 -> 15/30` moves.

`bench` already prints the line and `--repeat` (2026-09-09); what is open is that nothing reads it back, so the ledger's "the next campaign's rule names those totals among the counts that must hold" has nowhere to hold them.

- [x] **Step 1: Write the failing tests**

In `bench/history/test_track.py`, extend `TRANSCRIPT` and `DEV_TRANSCRIPT` with an anchors line after the summary line (the exact shape `bench` prints: `anchors  keyword 37/40  paraphrase 15/30  code 12/12`):

```python
TRANSCRIPT = """\
keyword    FR-AI-138    HIT  1/1  143 tok  расход виден салону
keyword    FR-WH-53     miss 0/1  210 tok  склад списание
paraphrase FR-PH-43     miss 0/1  198 tok  как клиент платит телефоном
code       apps/api/src/main.ts HIT  1/1   88 tok  где точка входа
paraphrase W-206        HIT  1/1  512 tok  предупреждение о переносе

keyword 37/40  paraphrase 15/30  code 12/12  p90 220 tok  dense=true  enriched=true (1996/1996 nodes)  suite=built-in gated=true
anchors  keyword 37/40  paraphrase 15/30  code 12/12
"""
```

and in `DEV_TRANSCRIPT` after its summary line: `anchors  long 1/1  cross 0/2  multi 1/3  where 1/1`. Add to `ParseBench`:

```python
    def test_the_anchor_line_is_read_per_kind(self):
        p = track.parse_bench(DEV_TRANSCRIPT)
        self.assertEqual(p["anchors"], {"long": [1, 1], "cross": [0, 2], "multi": [1, 3], "where": [1, 1]})

    def test_a_transcript_without_the_anchor_line_records_none(self):
        self.assertIsNone(track.parse_bench(OLD_TRANSCRIPT)["anchors"])

    def test_the_row_carries_the_anchors(self):
        row = track.build_row(track.parse_bench(DEV_TRANSCRIPT), "beauty-crm", "502e8a6d", "", "abc", False, floor_table=FLOOR_TABLE)
        self.assertEqual(row["anchors"]["multi"], [1, 3])

    def test_anchor_moves_are_reported_beside_metric_moves(self):
        prev = {"metrics": {"multi": [9, 12]}, "anchors": {"multi": [16, 39]}}
        latest = {"metrics": {"multi": [9, 12]}, "anchors": {"multi": [13, 39]}}
        self.assertEqual(track.metric_moves(prev, latest), [("anchors/multi", "16/39", "13/39")])

    def test_a_row_without_anchors_moves_nothing(self):
        prev = {"metrics": {"multi": [9, 12]}}
        latest = {"metrics": {"multi": [9, 12]}, "anchors": {"multi": [13, 39]}}
        self.assertEqual(track.metric_moves(prev, latest), [])
```

`FLOOR_TABLE` — use whatever name the file already gives the parsed floors fixture in `test_a_row_for_an_ungated_run_carries_no_floors_and_no_verdict` (read that test and pass its `floor_table=` argument the same way).

- [x] **Step 2: Run to see them fail**

```bash
cd /Users/max/Documents/projects/repograph && python3 -m unittest discover -s bench/history 2>&1 | tail -3
```

Expected: `KeyError: 'anchors'` and an `AssertionError` on `metric_moves`.

- [x] **Step 3: Implement**

In `track.py`, after `SUMMARY`:

```python
# `anchors  <kind> <reached>/<want> …` beneath the summary, on a line of its own so that the
# summary's own regex — and every transcript recorded through it — did not have to change.
ANCHORS = re.compile(r"^anchors\s+((?:\S+ \d+/\d+\s*)+)$")
```

In `parse_bench`, after `metrics["p90_tokens"] = …`:

```python
    anchors = [ANCHORS.match(l.strip()) for l in text.splitlines()]
    anchors = [m for m in anchors if m]
    anchor_totals = {kind: [int(r), int(w)] for kind, r, w in KIND.findall(anchors[-1].group(1))} if anchors else None
```

and add `"anchors": anchor_totals,` to the returned dict. In `build_row`, add `"anchors": parsed.get("anchors"),` after `"metrics"`. Replace `metric_moves`:

```python
def metric_moves(prev, latest):
    out = []
    for k, v in latest.get("metrics", {}).items():
        old = prev.get("metrics", {}).get(k)
        if old is None or old == v:
            continue
        fmt = (lambda x: f"{x[0]}/{x[1]}") if isinstance(v, list) else str
        out.append((k, fmt(old), fmt(v)))
    # An answer that keeps its verdict and loses two of its three anchors moves no count above;
    # the anchor totals are the one place that move is visible, so they are reported beside it.
    for k, v in (latest.get("anchors") or {}).items():
        old = (prev.get("anchors") or {}).get(k)
        if old is None or old == v:
            continue
        out.append((f"anchors/{k}", f"{old[0]}/{old[1]}", f"{v[0]}/{v[1]}"))
    return out
```

- [x] **Step 4: Run to see them pass**

```bash
cd /Users/max/Documents/projects/repograph && python3 -m unittest discover -s bench/history 2>&1 | tail -1
```

Expected: `OK`.

- [x] **Step 5: Documentation**

In `bench/history/README.md`, at the end of the "A second suite" paragraph, add:

```markdown
`bench` prints one more line beneath the summary — `anchors  cross 13/30  multi 16/39 …`, anchors
reached over anchors wanted per kind — and each row records it under `anchors`; `report` prints an
`anchors/<kind>` move beside the counts when it changes. It is a column and not a floor: the first
reading with a baseline to set one from is the 2026-09-09 table under G15 in
`docs/bench/next-version-gaps.md`.
```

In `README.md`, replace the sentence at line 628–630 that begins `The summary line also carries` and ends `the fusion the floors measure.` with:

```markdown
The summary line also carries `code_questions=<covered>/<eligible>` on a store that carries
questions about code, which are searched for the `--rerank` pool rather than in the fusion the
floors measure. Beneath it, on a line of its own, `anchors  <kind> <reached>/<wanted> …` says how
much of each answer was reached, not only whether it was: a case that keeps its verdict and loses
two of its three anchors moves that line and nothing else. `bench --repeat N` runs the suite N
times, judges every run on the floors, and prints a median beneath them.
```

(The `families=<count>` clause is retired by Task 8; this edit removes it from the sentence now because Task 8 removes it from the line — if Task 8 is not reached, Task 12 restores the clause.)

- [x] **Step 6: Commit**

```bash
cd /Users/max/Documents/projects/repograph && git add bench/history/track.py bench/history/test_track.py bench/history/README.md README.md && git commit -m "feat(bench): the anchor line enters the run history"
```

**Fails to close when:** a transcript in `bench/history/runs.jsonl`'s history stops parsing (`python3 -m unittest discover -s bench/history` covers the old shapes) — there is no measurement clause; the rule is applied by Tasks 6 and 10.

---

### Task 3: G35 — what `derive` costs on an update that changes a file

**Files:**
- Modify: `src/main.rs` (`apply_diff` signature, `run_update`, `Watcher::poll`, the `Cmd::Build | Cmd::Update` arm)
- Modify: `src/ask.rs` (`graph_for_ask`)
- Modify: `README.md` (the Configure/Use paragraph at line 177, `REPOGRAPH_TIMING=1 prints the stages.`)
- Modify: `docs/bench/2026-09-10-next-version-levers-results.md` (§3 reading)

**Interfaces:**
- Produces: `apply_diff(repo, store, graph, entries, diff, also, ex, timing: &ask::Timing)`; `run_update` prints stages `graph loaded`, `tree walked`, `families derived`, `extracted`, `saved` under `REPOGRAPH_TIMING=1`; `graph_for_ask` prints `families derived` on the bootstrap path.

The no-op case is closed (1.00 → 0.08 s); what has no number is the `update` that does change a file and still derives over the whole corpus. The stages `extracted` and `saved` outlive Task 5 (which deletes `families derived`); the number is what this task is for.

- [ ] **Step 1: Write the failing test**

`Timing` prints only under the environment variable and a test cannot set one safely (see `CACHE_OVERRIDE` in `src/index/embed.rs`), so the test pins the seam rather than the output: `apply_diff` takes a `Timing`. In `src/main.rs`'s `mod tests`, add:

```rust
    #[test]
    fn an_update_is_timed_through_the_same_stages_an_ask_is() {
        let dir = doc_repo(ONE);
        let (repo, cfg) = (dir.path(), config::Config::default());
        let timing = ask::Timing::new();
        let store = store::Store::new(repo);
        let (mut graph, manifest) = store.load().unwrap();
        let entries = walk::walk(repo, &cfg, &manifest).unwrap();
        let diff = manifest.diff(&entries);
        let ex = extractors(repo, families::derive(repo, &entries).unwrap().matcher()).unwrap();
        let r = apply_diff(repo, &store, &mut graph, &entries, &diff, &[], &ex, &timing).unwrap();
        assert_eq!(r.changed, 1);
    }
```

- [ ] **Step 2: Run to see it fail**

```bash
cd /Users/max/Documents/projects/repograph && cargo test an_update_is_timed 2>&1 | grep -E 'error\[|expected' | head -3
```

Expected: `error[E0061]: this function takes 7 arguments but 8 arguments were supplied`.

- [ ] **Step 3: Implement**

In `src/main.rs`, change `apply_diff`'s signature to end `…, also: &[walk::Entry], ex: &Extractors, timing: &ask::Timing) -> anyhow::Result<UpdateReport>` and inside it, after the `for e in work…` loop closes, before `store.save(...)`:

```rust
    timing.stage("extracted");
    store.save(graph, &walk::Manifest::from_entries(entries))?;
    timing.stage("saved");
```

In `run_update`:

```rust
pub fn run_update(repo: &std::path::Path, cfg: &config::Config, wipe: bool) -> anyhow::Result<UpdateReport> {
    let timing = ask::Timing::new();
    let store = store::Store::new(repo);
    if wipe { store.wipe()?; }
    let (mut graph, manifest) = store.load()?;
    timing.stage("graph loaded");
    let entries = walk::walk(repo, cfg, &manifest)?;
    let diff = manifest.diff(&entries);
    timing.stage("tree walked");
```

and after `let derived = families::derive(repo, &entries)?;` add `timing.stage("families derived");`. The final call becomes `apply_diff(repo, &store, &mut graph, &entries, &diff, &also, &extractors(repo, ids)?, &timing)`. In `Watcher::poll`, the `apply_diff` call gains `, &ask::Timing::new()`. In `src/ask.rs` `graph_for_ask`, the `derive` arm becomes:

```rust
        true => { let m = crate::families::derive(repo, &entries)?.matcher(); timing.stage("families derived"); m }
```

and its `apply_diff` call gains `, timing` as the last argument.

- [ ] **Step 4: Run, clippy, and the whole suite**

```bash
cd /Users/max/Documents/projects/repograph && cargo test 2>&1 | grep -E 'test result|FAILED' && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1
```

Expected: every `test result: ok`; clippy clean.

- [ ] **Step 5: The measurement**

As numbered: (1) reset — `$WT`'s store is then the fixture's, built under the 49-family list, and the reset's own settle walk is a no-op that derives nothing; (2) one changed-tree `update`, which derives 54 + 5, prints `families: +…` and re-reads the tree — today's behaviour and not the number wanted; (3) the one-file update, five times, timed; (4) the file put back. Nothing on disk is compared in this task — `$L/stages.txt` is the whole artefact — so nothing is copied out, and the next task that touches the corpus resets before it starts.

```bash
cd /Users/max/Documents/projects/repograph && cargo build --release 2>&1 | tail -1 && cp target/release/repograph ~/bench/levers-2026-09-10/bin/repograph-t3
FIX=~/bench/beauty-crm-502e8a6d; WT=~/bench/beauty-crm-test; export FIX WT
B=~/bench/levers-2026-09-10/bin/repograph-t3; L=~/bench/levers-2026-09-10/log/g35; mkdir -p $L
bench/probe/reset.sh
D=docs/prd-2026-08-16/prd/06-payments.md
printf '\n' >> $WT/$D && $B --repo $WT --no-dense update 2>&1 | tail -2
: > $L/stages.txt
for i in 1 2 3 4 5; do
  printf '\n' >> $WT/$D
  REPOGRAPH_TIMING=1 $B --repo $WT --no-dense update 2>&1 >/dev/null | grep '^timing' >> $L/stages.txt
done
git -C $WT checkout -- $D && $B --repo $WT --no-dense update | tail -1
python3 - $L/stages.txt <<'PY'
import re, sys, statistics
rows = {}
for line in open(sys.argv[1]):
    m = re.match(r'timing:\s+([\d.]+) ms\s+\(\+\s*([\d.]+) ms\)\s+(.*)', line)
    if m: rows.setdefault(m.group(3), []).append(float(m.group(2)))
for stage, v in rows.items():
    print(f"{stage:20} step median {statistics.median(v):7.1f} ms   n={len(v)}")
PY
```

Expected: the first `update` prints `families: +…` once (the settle); each measured run prints `changed 1 removed 0`; five `families derived` steps and five `tree walked` steps; the last `update` after the checkout prints `changed 1 removed 0` (the revert is itself a change). Note the document is under `docs/prd-2026-08-16/prd/` on this corpus — confirm with `ls $WT/docs/prd-2026-08-16/prd/06-payments.md` first (it is there at `502e8a6d`) and pick another `.md` under `docs/` if the path differs.

- [ ] **Step 6: Record**

Under §3 **Reading** in the results document: a two-row table `stage | step median (ms) | n` for `tree walked` and `families derived`, and one sentence saying which side of the walk's number it landed on. In `README.md` at line 177, after `REPOGRAPH_TIMING=1 prints the stages.`, add: ``The same variable prints an `update`'s stages — graph loaded, tree walked, extracted, saved — on stderr.``

- [ ] **Step 7: Commit**

```bash
cd /Users/max/Documents/projects/repograph && git add src/main.rs src/ask.rs README.md docs/bench/2026-09-10-next-version-levers-results.md && git commit -m "perf(update): the stages of an update under REPOGRAPH_TIMING, and what derive costs"
```

**Fails to close when:** no number could be read — a run that printed no `families derived` line. Landing above the walk's number is a finding, not a failure; it is recorded and Task 5 removes the cost either way.

---

### Task 4: G39 (a) — one generic grammar, and a graph that holds undeclared citations aside

**Files:**
- Modify: `src/ids.rs` (whole file: constructor, alternation, tests)
- Modify: `src/families.rs` (`FAMILY` visibility; delete `matcher`, `keys`, `from_graph`, `graph_families`, `test_matcher`, `Derived::matcher`, `Derived::families`, `Families`; `Scan::mentions` and `Scan::hits`; tests)
- Modify: `src/doc/requirements.rs` (`RequirementScanner::new()`, the `ids` field, tests)
- Modify: `src/doc/mod.rs`, `src/doc/registry.rs`, `src/doc/cases.rs`, `src/code/mod.rs`, `src/code/idrefs.rs`
- Modify: `src/model.rs` (`Graph.pending`, `Graph::settle`, `remove_file`, tests)
- Modify: `src/store.rs` (`MIRROR_MAGIC`, one test)
- Modify: `src/query.rs` (`exact_seeds`, `ask`, `verify`, `verify_json`, tests)
- Modify: `src/legacy.rs` (`resolve`, `import`, tests), `src/bench.rs:321-323,388`, `src/dump.rs:62`, `src/ask.rs:83,103,136-137,334` and the `query::ask` call, `src/main.rs` (`extractors`, `run_update`, `Watcher`, `Cmd::Prime`, `Cmd::ImportLegacy`, tests)

**Interfaces:**
- Consumes: `apply_diff(.., also, ex, timing)` from Task 3.
- Produces: `ids::generic() -> &'static IdMatcher`; `RequirementScanner::new()`, `DocExtractor::new()`, `CodeExtractor::new(resolver)`, `RegistryExtractor` (a unit struct), `idrefs::scan(rel, source, ex)`; `extractors(repo: &Path) -> Result<Extractors>`; `query::exact_seeds(graph, words)`, `query::ask(graph, lex, dense, rerank, words, opts)`; `legacy::import(graph, json)`; `Graph.pending: BTreeSet<Edge>`, `Graph::settle(&mut self)`; `families::FAMILY` is `pub(crate)`; `verify` prints a `held aside:` line and `verify_json` carries `held_aside` and `held_aside_prefixes`.

What this task changes and what it does not: the definition grammar (the head line, the milestone file, the ADR file, the registry row) already reads any prefix — it is the *reference* grammar that today reads only the prefixes a list-then-derivation admitted. After this task both read the same generic shape, an `Extraction` carries every citation it finds, and `Graph::settle` keeps citations of families no node declares in `pending`, where no reader, no count and no `dangling()` sees them. The `families:` lines on `build`/`update` and the whole-tree re-read stay as they are until Task 5.

- [ ] **Step 1: The failing tests — grammar**

Replace `src/ids.rs`'s `mod tests` helper and two of its tests:

```rust
    fn m() -> &'static IdMatcher { generic() }

    #[test]
    fn hyphenless_labels_are_not_ids_and_a_one_letter_family_is() {
        // `B1`, `C11`, `S3` have no hyphen before the digits and are labels in any corpus; `I-015`
        // has the shape `N-151` has on the bench corpus, and which prefixes are families is the
        // graph's question now, not this matcher's.
        assert_eq!(ids("вариант B1, C11 и S3; I-015 тоже"), vec!["I-015"]);
    }

    #[test]
    fn a_bare_suffix_is_never_an_id() {
        // The property the old empty-list sentinel existed to protect, now a property of the
        // grammar: the family slot needs a letter.
        assert!(ids("see -M01 and -12").is_empty());
        assert!(ids("-11…13, -1/2").is_empty());
        assert!(!m().is_id("-12"));
    }
```

Delete `ambiguous_families_do_not_match` and `empty_family_lists_match_nothing_rather_than_every_bare_suffix`. Every other test in that module stays as written; they pass unchanged under the generic matcher (verify by reading each: `plain_ids_in_prose`, `word_boundaries_hold`, ranges, slash lists, milestones, offsets, `is_id_is_exact`, `nfr_and_fr_ops…`, descending range, wide range, digit width, repeated id, whole-text id).

- [ ] **Step 2: The failing tests — the graph holds aside**

In `src/model.rs`'s `mod tests`, add (the `ex` helper already writes `FR-PAY-22 → N-151` with no `N` node):

```rust
    fn settled(files: &[&str]) -> Graph {
        let mut g = Graph::default();
        for f in files { g.apply(ex(f)); }
        g.settle();
        g
    }

    #[test]
    fn a_citation_of_a_family_no_node_declares_is_held_aside() {
        let g = settled(&["docs/06.md"]);
        // `N-151` is cited and no `N-…` node exists: the edge is kept, and kept out of sight.
        assert_eq!(g.edges.len(), 1, "{:?}", g.edges);
        assert_eq!(g.pending.iter().map(|e| e.target.as_str()).collect::<Vec<_>>(), vec!["N-151"]);
        assert!(g.dangling().is_empty(), "held-aside edges are not dangling: nothing a reader follows points nowhere");
    }

    #[test]
    fn a_family_that_appears_releases_what_was_held_without_a_re_read() {
        let mut g = settled(&["docs/06.md"]);
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "N-001", "first N", "", "docs/n.md", 1);
        g.apply(e);
        g.settle();
        assert!(g.pending.is_empty());
        assert!(g.edges.iter().any(|e| e.target == "N-151"), "released into the visible graph");
        // Still dangling — `N-151` itself is not defined — which is now a gap in a declared family.
        assert_eq!(g.dangling().len(), 1);
    }

    #[test]
    fn a_family_that_vanishes_takes_its_citations_back_out_of_sight() {
        let mut g = settled(&["docs/06.md"]);
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "N-001", "first N", "", "docs/n.md", 1);
        g.apply(e);
        g.settle();
        g.remove_file("docs/n.md");
        g.settle();
        assert_eq!(g.pending.len(), 1);
        assert!(!g.edges.iter().any(|e| e.target == "N-151"));
    }

    #[test]
    fn removing_a_file_drops_the_edges_it_held_aside_too() {
        let mut g = settled(&["docs/06.md", "docs/07.md"]);
        assert_eq!(g.pending.len(), 2);
        g.remove_file("docs/06.md");
        assert_eq!(g.pending.iter().map(|e| e.file.as_str()).collect::<Vec<_>>(), vec!["docs/07.md"]);
    }

    #[test]
    fn a_target_that_is_not_an_id_is_never_held_aside() {
        let g = settled(&["docs/06.md"]);
        assert!(g.edges.iter().any(|e| e.target == "entity:CancellationPolicy"));
    }
```

`apply_then_remove_file_restores_empty_graph` asserts `g.edges.len() == 2` before any settle — it stays true, since `apply` alone holds nothing aside; add `assert!(g.pending.is_empty())` to its final assertion.

- [ ] **Step 3: The failing tests — a store from before, and what `verify` says**

In `src/store.rs`'s `mod tests`, add:

```rust
    /// A store written by a release whose `Graph` had no `pending`: its JSON reads with none
    /// held aside, and its mirror — a postcard image of the older struct under the older magic
    /// — is passed over rather than decoded into the wrong shape.
    #[test]
    fn a_store_written_before_pending_existed_reads_whole() {
        #[derive(serde::Serialize)]
        struct OldGraph { nodes: std::collections::BTreeMap<String, crate::model::Node>, edges: std::collections::BTreeSet<crate::model::Edge> }
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let mut g = Graph::default();
        let mut e = crate::model::Extraction::default();
        e.node(crate::model::NodeKind::Requirement, "FR-1", "x", "", "docs/a.md", 1);
        e.edge("FR-1", "FR-2", crate::model::EdgeKind::References, "body", "docs/a.md");
        g.apply(e);
        let old = OldGraph { nodes: g.nodes.clone(), edges: g.edges.clone() };
        std::fs::create_dir_all(dir.path().join(".repograph")).unwrap();
        store.write_atomic("graph.json", &serde_json::to_vec(&old).unwrap()).unwrap();
        store.write_atomic("manifest.json", b"{\"files\":{}}").unwrap();
        // The older release's mirror: its magic, its stamp, its struct.
        let now = stamp(&dir.path().join(".repograph/graph.json")).unwrap();
        let mut bytes = mirror_header(now).to_vec();
        bytes[..4].copy_from_slice(b"RGM1");
        bytes.extend(postcard::to_stdvec(&old).unwrap());
        store.write_atomic("graph.bin", &bytes).unwrap();

        let (read, _, source) = store.load_traced().unwrap();
        assert_eq!(source, Source::Json, "the old mirror is passed over, not misread");
        assert_eq!((read.nodes, read.edges), (g.nodes, g.edges));
        assert!(read.pending.is_empty());
    }
```

Read the existing test `a_mirror_from_another_release_is_passed_over` and `a_saved_graph_loads_from_its_mirror_and_matches_the_json` first to reuse their setup helpers where the module has them (the helper that writes a manifest, the way `stamp` is reached) — the test above is written against the functions the file shows at lines 17–37 and 118.

In `src/query.rs`'s `mod tests`, beside `verify_counts_undeclared_and_dangling`, add:

```rust
    #[test]
    fn verify_reports_what_is_held_aside_apart_from_what_dangles() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-1", "x", "", "docs/a.md", 1);
        e.edge("FR-1", "FR-9", EdgeKind::References, "body", "docs/a.md");
        e.edge("FR-1", "ISO-8601", EdgeKind::References, "body", "docs/a.md");
        e.edge("FR-1", "OQ-25", EdgeKind::References, "body", "docs/a.md");
        g.apply(e);
        g.settle();
        let text = verify(&g);
        assert!(text.contains("dangling edges: 1\n"), "{text}");
        assert!(text.contains("held aside: 2 edges to ids in 2 prefixes no line defines  ISO OQ\n"), "{text}");
        assert!(text.contains("gaps in declared families: 1  FR-9"), "{text}");
        let json: serde_json::Value = serde_json::from_str(&verify_json(&g)).unwrap();
        assert_eq!(json["held_aside"], 2);
        assert_eq!(json["held_aside_prefixes"], serde_json::json!(["ISO", "OQ"]));
        assert_eq!(json["cite_only"], serde_json::json!([]), "nothing cite-only is left where a reader can see it");
    }
```

- [ ] **Step 4: Run to see them fail**

```bash
cd /Users/max/Documents/projects/repograph && cargo test 2>&1 | grep -E '^error' | sort | uniq -c | head
```

Expected: `cannot find function \`generic\``, `no field \`pending\``, `no method named \`settle\``.

- [ ] **Step 5: `src/ids.rs` — the one matcher**

Replace `alternation`, `IdMatcher::new` and the `pub fn new` block with:

```rust
use std::sync::OnceLock;

static GENERIC: OnceLock<IdMatcher> = OnceLock::new();

/// The one grammar every id is read by: any prefix the family slot admits, the digits after the
/// hyphen. Which prefixes are families is no longer the matcher's question — the graph answers
/// it after extraction, from the definitions it holds — so a citation of a prefix no line defines
/// is found here like any other and held aside by `Graph::settle` rather than never found.
pub fn generic() -> &'static IdMatcher {
    GENERIC.get_or_init(|| IdMatcher::build(crate::families::FAMILY, crate::families::MILESTONE))
}

impl IdMatcher {
    fn build(fam: &str, ms: &str) -> IdMatcher {
        let single = Regex::new(&format!(r"(?:{fam})-\d{{1,4}}|(?:{ms})-M\d{{2}}")).unwrap();
        // `FR-RPT-42…48`, `R-1601…R-1603`, `INV-11..13`, `N-1—N-3`
        let range = Regex::new(&format!(
            r"((?:{fam})-)(\d{{1,4}})\s*(?:…|\.\.|—|–)\s*(?:(?:{fam})-)?(\d{{1,4}})"
        )).unwrap();
        let slash = Regex::new(&format!(r"((?:{fam})-)(\d{{1,4}}(?:/\d{{1,4}})+)")).unwrap();
        IdMatcher { single, range, slash }
    }
```

`single_pattern`, `is_id`, `find_all` and `bounded` stay as they are. Delete the doc comment above the old `alternation` (the sentinel it explained is gone). In `src/families.rs`, make `const FAMILY` `pub(crate) const FAMILY`.

- [ ] **Step 6: The extractors take no family set**

`src/doc/requirements.rs`: remove the `ids: IdMatcher` field; `pub fn new() -> RequirementScanner` reads `let id = crate::ids::generic().single_pattern();`; in `scan`, `self.ids.is_id(name)` → `crate::ids::generic().is_id(name)` and both `self.ids.find_all(..)` → `crate::ids::generic().find_all(..)`; add

```rust
impl Default for RequirementScanner {
    fn default() -> Self { Self::new() }
}
```

and the tests' helper becomes `RequirementScanner::new().scan(rel, text)`. Add one case beside `edges_are_unique_per_key`:

```rust
    #[test]
    fn a_citation_of_a_prefix_nothing_defines_is_extracted_like_any_other() {
        // Whether `ISO` is a family is decided on the graph, after every file is read; the
        // extractor's job is to miss nothing.
        let ex = scan("docs/x.md", "**FR-PAY-22 · MUST · a**\n\nдаты по ISO-8601\n");
        assert!(has(&ex, "FR-PAY-22", "ISO-8601", EdgeKind::References));
    }
```

`src/doc/mod.rs`: `pub struct DocExtractor { req: requirements::RequirementScanner }`, `pub fn new() -> DocExtractor { DocExtractor { req: requirements::RequirementScanner::new() } }`, plus `impl Default`. `src/doc/cases.rs:8`: `DocExtractor::new().extract(rel, text)`.

`src/doc/registry.rs`: `pub struct RegistryExtractor;` — delete `new` and the `ids` field; in `extract`, `self.ids.find_all(b)` → `crate::ids::generic().find_all(b)`; tests construct `RegistryExtractor` directly. Rewrite the comment at line 183 (`go through the IdMatcher, so an unconfigured family on the row itself passes straight through`) to say that a row's own id is a definition whatever its prefix, and only its `basis:` citations are read through the grammar. `declared_ids` stays for now (Task 5 deletes it with its last caller).

`src/code/idrefs.rs`: `pub fn scan(rel: &str, source: &str, ex: &mut Extraction)` with `crate::ids::generic().find_all(t)`; tests call `scan(rel, src, &mut ex)`. `src/code/mod.rs`: `CodeExtractor { symbols }`, `new(resolver)`, `idrefs::scan(rel, text, &mut ex)`; delete `use crate::ids::IdMatcher`.

`src/main.rs`:

```rust
pub(crate) fn extractors(repo: &std::path::Path) -> anyhow::Result<Extractors> {
    let resolver = code::imports::Resolver::new(repo)?;
    Ok(Extractors {
        doc: Box::new(doc::DocExtractor::new()),
        code: Box::new(code::CodeExtractor::new(resolver)),
        registry: Box::new(doc::registry::RegistryExtractor),
    })
}
```

- [ ] **Step 7: The graph**

In `src/model.rs`, `Graph` becomes:

```rust
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: BTreeMap<String, Node>,
    pub edges: BTreeSet<Edge>,
    /// Citations of ids in a family no definition declares — `ISO-8601`, a ticket number, a
    /// prefix the corpus cites and never defines. Kept apart so that no reader follows them and
    /// no count reports them, and kept at all so that the day a line defines the family they
    /// are released by `settle` without a document being re-read. A store written before this
    /// field existed reads as holding none.
    #[serde(default)] pub pending: BTreeSet<Edge>,
}
```

Add to `impl Graph`:

```rust
    /// Every edge sorted to the side of the line its target's family is on: cited-and-declared
    /// in `edges`, cited-and-not in `pending`. Run once after a batch of `apply`s, because only
    /// then is it known which families the batch declared — a file citing `OQ-25` may be read
    /// before the file that defines `OQ-1`.
    pub fn settle(&mut self) {
        let (ids, milestones) = crate::families::of_graph(self);
        let admitted = |target: &str| match crate::families::classify(target) {
            Some(crate::families::Family::Id(f)) => ids.contains_key(f),
            Some(crate::families::Family::Milestone(f)) => milestones.contains_key(f),
            None => true,
        };
        let all: Vec<Edge> = std::mem::take(&mut self.edges).into_iter().chain(std::mem::take(&mut self.pending)).collect();
        for e in all {
            if admitted(&e.target) { self.edges.insert(e); } else { self.pending.insert(e); }
        }
    }
```

and in `remove_file`, after `self.edges.retain(|e| e.file != rel);`, add `self.pending.retain(|e| e.file != rel);`.

In `src/store.rs`, `const MIRROR_MAGIC: &[u8; 4] = b"RGM2";` with the comment: a `Graph` that gained a field is a new mirror shape, and a magic that says so is one fewer thing that depends on postcard failing at the right byte.

- [ ] **Step 8: The readers**

`src/query.rs`: `pub(crate) fn exact_seeds(graph: &Graph, words: &[String]) -> (Vec<String>, bool)` using `crate::ids::generic().is_id(w)`; `pub fn ask(graph: &Graph, lex: &Lexical, dense: Option<Dense>, rerank: Option<Rerank>, words: &[String], opts: &Options) -> Answer` calling `exact_seeds(graph, words)`; delete `use crate::ids::IdMatcher;` and the tests' `fn ids()` helper, and fix every `ask(`/`exact_seeds(` call in the module's tests. In `verify`, after the `dangling edges:` line:

```rust
    let held: BTreeSet<&str> = graph.pending.iter().map(|e| family(&e.target)).collect();
    out.push_str(&format!("held aside: {} edges to ids in {} prefixes no line defines  {}\n",
        graph.pending.len(), held.len(), held.iter().cloned().collect::<Vec<_>>().join(" ")));
```

and in `verify_json`, the same two values as `held_aside: usize` and `held_aside_prefixes: Vec<&'a str>` on `Out`. The `cite_only` partition stays — it is computed over `edges` and reads empty once citations of undeclared families live in `pending`, which is what the new test asserts.

`src/legacy.rs`: `fn resolve(graph, by_label, gn)` with `crate::ids::generic().find_all(&gn.label)`; `pub fn import(graph: &mut Graph, json: &str) -> Result<Report>`; delete the `ids` parameters, the `use crate::ids::IdMatcher`, and the tests' `ids()` helpers.

`src/bench.rs`: at lines 321–323 keep `counted` and `family_count`, delete `let ids = …`; the `query::ask` call loses `&ids`. `src/dump.rs`: delete line 62 and the argument. `src/ask.rs`: delete the `ids` field (:103), its construction and the `ids ready` stage (:136–137), the reset after a resync (:334), the `derive`/`from_graph` match in `graph_for_ask` (:82–85 — the extractors need no families now; `crate::extractors(repo)?`), and the `ids` argument to `query::ask`. `src/main.rs`: `Cmd::Prime` reads `let families = families::of_graph(&graph).0.len();`; `Cmd::ImportLegacy` drops `ids`; `Watcher` loses `ex_families` and `extract_under` (its `open` builds `extractors(repo)?` once; `poll` drops the `extract_under` call and keeps `derived.against`); `run_update` becomes the shape shown in Task 3 with `derived.matcher()` gone:

```rust
    let bootstrap = graph.nodes.is_empty();
    let quiet = !bootstrap && diff.changed.is_empty() && diff.removed.is_empty();
    let mut also: Vec<walk::Entry> = Vec::new();
    // A tree that has not moved cannot have moved its families either. Reading every document to
    // confirm it is a whole build's worth of I/O on the no-op update a commit hook fires.
    if !quiet {
        let derived = families::derive(repo, &entries)?;
        timing.stage("families derived");
        match bootstrap {
            true => eprintln!("{}", derived.line()),
            false => {
                let moved = derived.against(&graph);
                if !moved.is_empty() {
                    eprintln!("families: {}", moved.join(", "));
                    also = rest_of_tree(&entries, &diff);
                }
            }
        }
    }
    apply_diff(repo, &store, &mut graph, &entries, &diff, &also, &extractors(repo)?, &timing)
```

In `apply_diff`, insert `graph.settle();` immediately before `timing.stage("extracted");`. The test `an_ask_after_an_edit_answers_from_the_edited_file` drops its `ids` line and argument; `an_update_is_timed_through_the_same_stages_an_ask_is` (Task 3) builds `extractors(repo).unwrap()`.

- [ ] **Step 9: `src/families.rs` — delete the list-shaped API, keep the scan**

Delete `matcher`, `keys`, `from_graph`, `graph_families`, `test_matcher`, `Derived::matcher`, `Derived::families`, the `Families` alias, `Scan.matchers`, `Scan::hits`, `Scan.id`, `Scan.milestone`, and `use crate::ids::{bounded, IdMatcher}` (keep `bounded` only if still used; it is not — `find_all` applies it). Rewrite `Scan::mentions`:

```rust
    fn mentions(&mut self, rel: &str, line_no: u32, line: &str) {
        if self.tally == Mentions::Skip { return; }
        // The extractor's own reading of the line — ranges and slash lists expanded, boundaries
        // applied — so a mention is counted the way the graph would have cited it.
        let mut per_prefix: BTreeMap<String, usize> = BTreeMap::new();
        for hit in crate::ids::generic().find_all(line) {
            if let Some(Family::Id(f) | Family::Milestone(f)) = classify(&hit.id) {
                *per_prefix.entry(f.to_string()).or_default() += 1;
            }
        }
        for (prefix, n) in per_prefix {
            self.record(prefix, n, rel, line_no, line);
        }
    }
```

Tests: `the_graph_reports_the_same_families_its_documents_defined` loses its `from_graph(&g).find_all(..)` assertion (the read matcher is gone; what the graph admits is `Graph::settle`'s test in `model.rs`); `a_long_prefix_and_a_three_part_one_survive_the_round_trip` builds `crate::doc::DocExtractor::new()`; the rest stand.

- [ ] **Step 10: Run everything**

```bash
cd /Users/max/Documents/projects/repograph && cargo test 2>&1 | grep -E 'test result|FAILED|panicked' && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1
```

Expected: every `test result: ok`; clippy clean. `tests/families.rs` still passes as written: the `families:` lines still come from `derive`, `ISO` is still absent from them, the `+NEW` update still re-reads. If `dead_code` names `declared_ids`, it still has its caller in `Scan::registry`; if it names `bounded`, delete the import.

- [ ] **Step 11: Commit**

```bash
cd /Users/max/Documents/projects/repograph && git add -A src && git commit -m "feat(ids): one generic grammar for every id, and a graph that holds undeclared citations aside"
```

**Fails to close when:** a test in Step 1–3 cannot be made green without changing what a fixture extracts to in `nodes` or `edges` (the `cases.rs` and `requirements.rs` suites are the fixtures' pinned shape). The corpus gate is Task 6's.

---

### Task 5: G39 (b) — the writers stop deriving, and the report reads the graph

**Files:**
- Modify: `src/families.rs` (delete `derive`, `Derived`, `Site`-from-scan, `Scan::define`, `Scan::registry`'s definition half, `survey`'s definition half; add `Families`, `line`, `moved`, graph-sourced rows; `run` gains the behind-the-tree line)
- Modify: `src/main.rs` (`run_update`, `Watcher::poll`, `apply_diff` loses `also`, delete `rest_of_tree`)
- Modify: `src/ask.rs` (`apply_diff` call)
- Modify: `src/doc/registry.rs` (delete `declared_ids`)
- Modify: `tests/families.rs`
- Modify: `README.md` (the "Id families" section, lines 452–486)

**Interfaces:**
- Consumes: `Graph::settle`, `ids::generic()` from Task 4.
- Produces: `families::Families = (BTreeMap<String, usize>, BTreeMap<String, usize>)` (what `of_graph` returns); `families::line(&Families) -> String`; `families::moved(before: &Families, after: &Families) -> Vec<String>`; `families::survey(repo, entries) -> Result<Vec<Mention>>`; `families::report(graph: &Graph, cited: Vec<Mention>) -> Report` with `Row { family, nodes, defined: Site }`; `apply_diff(repo, store, graph, entries, diff, ex, timing)`.

- [ ] **Step 1: The failing integration test**

In `tests/families.rs`, change `REQ` so the requirement cites a prefix nothing defines *yet*:

```rust
const REQ: &str = "# Требования\n\n**REQ-7 · MUST · Отмена визита за сутки**\n\nОтмена возможна за сутки, задача NEW-5.\n\nсм. REQ-7, даты по ISO-8601\n";
```

and rewrite the middle of `a_repository_gets_its_families_from_the_documents_and_says_when_they_move` from the no-op update onward:

```rust
    // A no-op update is a fixed point, families included: nothing is re-read and nothing is said.
    let (ok, out, err) = repograph(repo, &["update"]);
    assert!(ok, "{out}{err}");
    assert!(out.starts_with("changed 0 removed 0"), "{out}");
    assert!(!err.contains("families"), "{err}");

    // Before any line defines `NEW`, the citation is held aside: `verify` counts it and nothing
    // a reader follows reaches it.
    let (ok, out, err) = repograph(repo, &["verify"]);
    assert!(ok, "{out}{err}");
    assert!(out.contains("held aside: 2 edges to ids in 2 prefixes no line defines  ISO NEW"), "{out}");
    let (ok, out, _) = repograph(repo, &["explain", "REQ-7"]);
    assert!(ok && !out.contains("NEW-5"), "a held-aside citation is not an answer: {out}");

    // A family the corpus grows: the one file that defines it is read, and the citations the
    // graph was holding are released — `req.md` is not touched and not re-read.
    std::fs::write(repo.join("docs/new.md"), "**NEW-1 · MUST · Новое правило**\n\nтело\n").unwrap();
    let (ok, out, err) = repograph(repo, &["update"]);
    assert!(ok, "{out}{err}");
    assert!(out.starts_with("changed 1 removed 0"), "{out}");
    assert!(err.contains("families: +NEW"), "{err}");
    let (ok, out, _) = repograph(repo, &["explain", "REQ-7"]);
    assert!(ok && out.contains("NEW-5"), "released without a re-read: {out}");
    let (ok, out, _) = repograph(repo, &["verify"]);
    assert!(ok && out.contains("held aside: 1 edges to ids in 1 prefixes no line defines  ISO"), "{out}");
    let (ok, out, err) = repograph(repo, &["ask", "NEW-1"]);
    assert!(ok, "{out}{err}");
    assert!(out.lines().next().unwrap().starts_with("NEW-1"), "{out}");

    // And a file that still names the old keys: one line, and not one family different.
    std::fs::write(repo.join("repograph.toml"), "id_families = [\"FR-PAY\"]\n").unwrap();
    let (ok, out, err) = repograph(repo, &["families"]);
    assert!(ok, "{out}{err}");
    assert!(err.contains("id_families is no longer read"), "{err}");
    assert!(families(&out, "REQ").contains("docs/req.md:3") && families(&out, "NEW").contains("docs/new.md:1"), "{out}");
    assert!(!out.contains("FR-PAY"), "a key nobody reads names no family: {out}");

    // The definition edited away with no update since. The graph is the report's source, so the
    // family is still there with its node; what the report can say is that the store is behind
    // the tree, which is the one state it exists to expose.
    std::fs::write(repo.join("docs/new.md"), "тело без определения\n").unwrap();
    let (ok, out, err) = repograph(repo, &["families"]);
    assert!(ok, "{out}{err}");
    assert!(families(&out, "NEW").contains("docs/new.md:1"), "{out}");
    assert!(err.contains("families: the store is 1 file behind the tree — run `repograph update`"), "{err}");
    let (ok, json, err) = repograph(repo, &["families", "--json"]);
    assert!(ok, "{json}{err}");
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let new = v["families"].as_array().unwrap().iter().find(|f| f["family"] == "NEW").unwrap();
    assert!(new["defined"]["file"] == "docs/new.md" && new["nodes"] == 1, "{new}");
    // And after the update the family is gone with its definition, and its citation is held again.
    let (ok, out, err) = repograph(repo, &["update"]);
    assert!(ok && err.contains("families: -NEW"), "{out}{err}");
    let (ok, out, _) = repograph(repo, &["verify"]);
    assert!(ok && out.contains("held aside: 2 edges"), "{out}");
```

The `ISO`-row and `Not families` assertions earlier in the test stand. `every_shape_the_extractor_defines_a_node_with_defines_a_family` stands as written — every assertion in it is about the graph or the mention tally.

- [ ] **Step 2: Run to see it fail**

```bash
cd /Users/max/Documents/projects/repograph && cargo test --test families 2>&1 | grep -E 'panicked|assertion' | head -3
```

Expected: the `explain REQ-7` after `docs/new.md` fails first if the re-read path is still there (it passes by accident — the re-read also releases the edge); the definitive red is `families: the store is 1 file behind the tree`, which nothing prints yet.

- [ ] **Step 3: `src/families.rs`**

Delete `Derived`, `derive`, `Scan.definition`, `Scan.ids`, `Scan.milestones`, `Scan::define`, the definition half of `Scan::doc` (the `milestone_file`/`adr_file` inserts) and of `Scan::registry` (keep its `tally_lines`), `Scan::finish`'s definition filter, `names`, `rows(defined, counts)`, and `report(derived, graph)`. Add:

```rust
/// Every family the graph's own nodes are written in, ids and milestones apart, with how many
/// nodes each holds. What `of_graph` returns, and the whole of what a writer compares before and
/// after an apply to say which families moved.
pub type Families = (BTreeMap<String, usize>, BTreeMap<String, usize>);

/// What a build says it found. The whole list where it is short and its head with a count where
/// it is not: a build's stderr is read at a glance, and a large corpus names dozens.
pub fn line(f: &Families) -> String {
    const SHOWN: usize = 20;
    let listed = |m: &BTreeMap<String, usize>| {
        let v: Vec<&str> = m.keys().map(String::as_str).collect();
        match (v.len(), v.len() > SHOWN) {
            (0, _) => "(none)".to_string(),
            (n, true) => format!("{} … +{}", v[..SHOWN].join(", "), n - SHOWN),
            _ => v.join(", "),
        }
    };
    format!("families: {} · milestones: {}", listed(&f.0), listed(&f.1))
}

/// Families the graph gained or lost between two readings of it, each as the word an update
/// prints. Empty is the ordinary case.
pub fn moved(before: &Families, after: &Families) -> Vec<String> {
    let mut out = Vec::new();
    for (was, is, mark) in [(&before.0, &after.0, ""), (&before.1, &after.1, " milestone")] {
        for f in is.keys().filter(|f| !was.contains_key(*f)) { out.push(format!("+{f}{mark}")); }
        for f in was.keys().filter(|f| !is.contains_key(*f)) { out.push(format!("-{f}{mark}")); }
    }
    out
}
```

`Scan` keeps only the tally (`seen`, `mentions`, `record`, `tally_lines`, `doc` with the fence guard, `code`, `registry` = `tally_lines`), `finish` returns every prefix's `Mention` sorted as today, and `survey(repo, entries) -> Result<Vec<Mention>>` is the only scan entry point; `scan_tree`'s `Mentions::Skip` branch and the enum go with `derive`. The report:

```rust
/// Whether a node is written in this family, in the id form or the milestone form as asked.
fn in_family(id: &str, family: &str, milestone: bool) -> bool {
    match classify(id) {
        Some(Family::Id(f)) => !milestone && f == family,
        Some(Family::Milestone(f)) => milestone && f == family,
        None => false,
    }
}

/// Where a family is defined: its first node in path-then-line order, since the graph keeps a
/// node's primary declaring file and line.
fn site_of(graph: &Graph, family: &str, milestone: bool) -> Site {
    let n = graph.nodes.values()
        .filter(|n| !n.is_code() && in_family(&n.id, family, milestone))
        .min_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)))
        .expect("a counted family has a node");
    Site { file: n.file.clone(), line: n.line, text: crate::query::headline(&n.label) }
}

fn rows(graph: &Graph, counts: &BTreeMap<String, usize>, milestone: bool) -> Vec<Row> {
    counts.iter().map(|(family, nodes)| Row { family: family.clone(), nodes: *nodes, defined: site_of(graph, family, milestone) }).collect()
}

pub fn report(graph: &Graph, cited: Vec<Mention>) -> Report {
    let (ids, milestones) = of_graph(graph);
    let mention_only = cited.into_iter().filter(|m| !ids.contains_key(&m.prefix) && !milestones.contains_key(&m.prefix)).collect();
    Report { families: rows(graph, &ids, false), milestones: rows(graph, &milestones, true), mention_only }
}

pub fn run(repo: &Path, cfg: &crate::config::Config, json: bool) -> Result<()> {
    let (graph, manifest) = Store::new(repo).load()?;
    if graph.nodes.is_empty() { anyhow::bail!("graph is empty — run `repograph build`"); }
    let entries = crate::walk::walk(repo, cfg, &manifest)?;
    // The graph is the report's source, so a definition edited since the last update is not in
    // it; the tree being ahead of the store is said once, in words, rather than left for a
    // reader to infer from a row that looks a day old.
    let diff = manifest.diff(&entries);
    let behind = diff.changed.len() + diff.removed.len();
    if behind > 0 {
        eprintln!("families: the store is {behind} file{} behind the tree — run `repograph update`", if behind == 1 { "" } else { "s" });
    }
    let r = report(&graph, survey(repo, &entries)?);
    match json {
        true => println!("{}", serde_json::to_string_pretty(&r)?),
        false => print!("{}", render(&r)),
    }
    Ok(())
}
```

`Row.defined` becomes `pub defined: Site` and `defined_at` takes `&Site`. Delete `crate::doc::registry::declared_ids` and its test. The inline tests: delete the definition-scan tests (`a_bold_head_and_a_heading_define_and_prose_does_not` becomes a mention-tally test asserting `["AC", "FR-PAY", "OQ", "REQ"]` are all *cited*; `a_line_that_only_opens_with_an_id_defines_nothing` asserts `ISO`, `RFC`, `UTF`, `REQ` are cited; `a_head_inside_a_code_fence_defines_nothing` asserts a fenced line contributes no mention; `a_prefix_written_only_in_code_or_only_in_a_registry_is_still_counted` asserts `INV`, `N`, `NEW`, `TCK` cited), delete `a_derivation_for_a_build_counts_no_mentions_at_all`, `a_milestone_document_defines_its_family_by_its_name_and_by_a_head`, `an_adr_document_defines_the_adr_family_by_its_name`, `a_registry_row_defines_its_prefix`, `what_a_build_derives_is_what_its_graph_declares`, `a_family_gained_and_one_lost_are_both_named` (its replacement below), `the_build_line_names_the_families_and_elides_a_long_list` (replacement below); rewrite `a_family_the_graph_holds_and_no_document_defines_is_a_row_of_its_own` as `every_family_the_graph_holds_is_a_row_with_its_first_node_as_its_site`; keep `a_range_and_a_slash_list_count_every_id_they_stand_for`, `an_example_line_is_cut_on_a_character_boundary`, `the_graph_reports_the_same_families_its_documents_defined`, `a_task_is_written_in_its_milestones_family`, and `a_long_prefix_and_a_three_part_one_survive_the_round_trip` with its `against` assertion replaced by `assert_eq!(of_graph(&g).0.keys().collect::<Vec<_>>(), vec!["FR-PAY-EU", "SECURITY"])`. Add:

```rust
    #[test]
    fn a_family_gained_and_one_lost_are_both_named() {
        let before = of_graph(&graph_with(&["AC-3", "REQ-7"]));
        let after = of_graph(&graph_with(&["REQ-7", "BE-M01"]));
        assert_eq!(moved(&before, &after), vec!["-AC", "+BE milestone"]);
    }

    #[test]
    fn the_build_line_names_the_families_and_elides_a_long_list() {
        assert_eq!(line(&of_graph(&graph_with(&["REQ-1", "BE-M01"]))), "families: REQ · milestones: BE");
        let ids: Vec<String> = (0..25).map(|i| format!("F{i:02}-1")).collect();
        let g = graph_with(&ids.iter().map(String::as_str).collect::<Vec<_>>());
        let l = line(&of_graph(&g));
        assert!(l.starts_with("families: F00, F01,"), "{l}");
        assert!(l.contains("… +5 · milestones: (none)"), "{l}");
    }
```

- [ ] **Step 4: `src/main.rs` and `src/ask.rs`**

`apply_diff` loses the `also` parameter and `rest_of_tree` is deleted; `stale` and `work` are built from `diff` alone, and `let moved = !diff.changed.is_empty() || !diff.removed.is_empty();`. `run_update`:

```rust
    let bootstrap = graph.nodes.is_empty();
    let before = families::of_graph(&graph);
    let r = apply_diff(repo, &store, &mut graph, &entries, &diff, &extractors(repo)?, &timing)?;
    let after = families::of_graph(&graph);
    // A family that appeared or vanished changes which citations are visible, and `settle` has
    // already moved them; what is left to do is say so, since a reader of the next `ask` will
    // see edges that were not there before.
    match bootstrap {
        true => eprintln!("{}", families::line(&after)),
        false => {
            let moved = families::moved(&before, &after);
            if !moved.is_empty() { eprintln!("families: {}", moved.join(", ")); }
        }
    }
    Ok(r)
```

`Watcher::poll` does the same around its `apply_diff` (without the bootstrap arm — a watcher opens a built store). `graph_for_ask`'s call becomes `crate::apply_diff(repo, store, &mut graph, &entries, &diff, &crate::extractors(repo)?, timing)?`. Delete the doc comments that described the re-read (`Re-extracts what the diff names … the whole tree, when a family has appeared or vanished under it` on `apply_diff`; `The store brought in line with the tree. The extractors are built here rather than passed in because the families come between the walk and them` on `run_update`) and write what is true now: the extractors read every id, and the graph decides after the apply which are visible.

- [ ] **Step 5: Run everything**

```bash
cd /Users/max/Documents/projects/repograph && cargo test 2>&1 | grep -E 'test result|FAILED|panicked' && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1
```

Expected: green and clean. The `families:` stderr line on a build must read exactly as before for the same corpus (`families: REQ · milestones: (none)` in the integration test).

- [ ] **Step 6: README**

In `README.md`'s "Id families" section replace the paragraph that begins `An \`update\` whose documents gained or lost a family says so` (lines 480–486) with:

```markdown
Every citation is read whatever its prefix, and the graph decides after each build or update which
of them a reader may follow: a citation of a family some line defines is an edge; a citation of a
prefix nothing defines — `ISO-8601`, a ticket number — is held aside, counted by `repograph verify`
as `held aside`, and reached by nothing else. An `update` whose documents gained or lost a family
says so — `families: +REQ`, `families: -AC` — and re-reads nothing: the one file that changed is
read, and the citations the graph was holding for the new family are released from where they were.
A `repograph.toml` still naming `id_families` parses, gets one line on stderr, and is otherwise
unaffected. `repograph families` reads the graph and, when the tree has moved since the last update,
says so on stderr rather than guessing which definitions still exist.
```

and in the same section change `A prefix that is only ever *mentioned* — \`ISO-8601\`, a ticket number, a year — is plain text, and so is one whose ids are cited but never defined.` to `A prefix that is only ever *mentioned* — \`ISO-8601\`, a ticket number, a year — is held aside rather than linked, and so is one whose ids are cited but never defined.`

- [ ] **Step 7: Commit**

```bash
cd /Users/max/Documents/projects/repograph && git add -A src tests README.md && git commit -m "refactor(families): the family set is a view over the graph, and no writer re-reads the tree"
```

**Fails to close when:** the integration test's `changed 1 removed 0` after `docs/new.md` cannot be met without a re-read — that would mean `settle` is not releasing held edges, and the design is wrong rather than the test.

---

### Task 6: G39 (c) — the corpus: same visible graph, how much is held, what an update costs

**Files:**
- Modify: `docs/bench/2026-09-10-next-version-levers-results.md` (§4 reading, §2 reading)

**Interfaces:**
- Consumes: `bench/probe/graphdiff.py`, `reset.sh`, `readers.sh`, `judge.py`, `arms.sh` (Task 1); `FIX` and `WT`; `~/bench/levers-2026-09-10/log/readers-reference.txt` (Task 1).
- Produces: `~/bench/levers-2026-09-10/log/g39/main/graph.json` and `log/g39/branch/graph.json` — the two builds, copied out; Task 8 reads the second as its *before*.

- [ ] **Step 1: Build the binary**

```bash
cd /Users/max/Documents/projects/repograph && cargo build --release 2>&1 | tail -1 && cp target/release/repograph ~/bench/levers-2026-09-10/bin/repograph-t5 && git rev-parse --short HEAD
```

Record the short hash as the section's binary name.

- [ ] **Step 2: Clause (a) and (b) — the same visible graph, and the growth**

Two builds in one directory, so the second overwrites the first; as numbered: (1) reset; (2) empty the store — a `build` wipes only the graph and the manifest and would keep the fixture's questions and vectors, and "from nothing" has to be said; (3) `main`'s build; (4) **copy its `graph.json` out to `$L/main/` before anything else runs** — the next build destroys it; (5) empty the store again; (6) the branch's build; (7) copy out to `$L/branch/`; (8) diff the two copies, never the directory.

```bash
cd /Users/max/Documents/projects/repograph && FIX=~/bench/beauty-crm-502e8a6d; WT=~/bench/beauty-crm-test; export FIX WT
L=~/bench/levers-2026-09-10/log/g39; mkdir -p $L/main $L/branch
bench/probe/reset.sh && rm -rf $WT/.repograph
~/bench/levers-2026-09-10/bin/repograph-main --repo $WT --no-dense build 2>&1 | tail -3 && cp $WT/.repograph/graph.json $L/main/graph.json
rm -rf $WT/.repograph
~/bench/levers-2026-09-10/bin/repograph-t5 --repo $WT --no-dense build 2>&1 | tail -3 && cp $WT/.repograph/graph.json $L/branch/graph.json
ls -l $L/main/graph.json $L/branch/graph.json
python3 bench/probe/graphdiff.py $L/main/graph.json $L/branch/graph.json | tee $L/graphdiff.txt
```

Expected: both builds print the same `families: … · milestones: …` line (54 ids, 5 milestones on this corpus), the same ``repograph: N requirement-like nodes have no questions — run `repograph enrich` …`` with the same N (an emptied store has no questions; this is not G32's number), and the same `nodes N edges M`; `ls` shows two files of about 12 MB; `graphdiff.py` prints `nodes: same`, `edges: same`, `pending: <n>` and `bytes: ratio 1.0xx` with the ratio ≤ 1.100. The `pending` count is the store's growth and is recorded whatever it is; the ledger's 2,238 is the lower bound (that count was under the 49-list matcher; the generic one also cites the 96 mention-only prefixes).

- [ ] **Step 3: Clause (d) — the build's wall**

Each timed build is from an empty store like the two above, so the six are alike:

```bash
for i in 1 2 3; do rm -rf $WT/.repograph; bench/probe/measure.sh build-main-$i $L -- ~/bench/levers-2026-09-10/bin/repograph-main --repo $WT --no-dense build; done
for i in 1 2 3; do rm -rf $WT/.repograph; bench/probe/measure.sh build-t5-$i $L -- ~/bench/levers-2026-09-10/bin/repograph-t5 --repo $WT --no-dense build; done
python3 bench/probe/judge.py medians $L/summary.txt
```

Expected: `build-t5` wall median within 10% of `build-main`'s (the 2026-09-09 reading of `--no-dense build` is 1.04 s). The generic regex is larger than a fixed alternation and the settle is one pass over 60-odd thousand edges; this is where either would show. The artefacts are `measure.sh`'s files in `$L`; the store in `$WT` is not compared.

- [ ] **Step 4: Clause (c) — one definition, one file**

The corpus cites `OQ` 1,771 times and defines it nowhere. The last build above is the branch's, so the store in `$WT` is settled under it — and the tree is clean, since nothing here has touched it.

```bash
B=~/bench/levers-2026-09-10/bin/repograph-t5
$B --repo $WT verify | grep -E 'held aside|dangling|gaps' | tee $L/verify-before.txt
: > $L/oq-stages.txt
for i in 1 2 3 4 5; do
  printf '**OQ-1 · MUST · открытый вопрос**\n\nтело\n' > $WT/docs/oq.md
  bench/probe/measure.sh oq-add-$i $L -- env REPOGRAPH_TIMING=1 $B --repo $WT --no-dense update
  grep -E '^(changed|families)' $L/oq-add-$i.out $L/oq-add-$i.time; grep '^timing' $L/oq-add-$i.time >> $L/oq-stages.txt
  [ $i = 1 ] && { $B --repo $WT verify | grep -E 'held aside|dangling|gaps' | tee $L/verify-after.txt; $B --repo $WT explain ADR-023 | grep -c 'OQ-25' | tee $L/oq-25-visible.txt; }
  rm $WT/docs/oq.md
  $B --repo $WT --no-dense update 2>&1 | grep -E '^(changed|families)'
done
python3 bench/probe/judge.py medians $L/summary.txt | grep oq-add
```

Expected on every add: stdout `changed 1 removed 0`, stderr `families: +OQ`, the `timing:` stages with `extracted` the only stage that grew and no stage named `families derived`; on every remove: `changed 0 removed 1` and `families: -OQ`. `verify-after.txt`'s `held aside` count is `verify-before.txt`'s minus the OQ citations, its `gaps in declared families` count grew by the same number, and `oq-25-visible.txt` reads `1` or more (`docs/adr/ADR-023-conformance-policy-and-day-bounds.md` cites `OQ-25`, so the `ADR-023` node's `References → OQ-25  [prose]` line is visible only once `OQ` is declared; `explain` prints one `References → <id>` line per edge). The `oq-add` wall median is under 0.5 s.

- [ ] **Step 5: Clause (e) and the read path**

Wait for `quiet.sh`. As numbered: (1) reset — the store is the fixture's again, its `graph.bin` as `repograph-main`'s settle wrote it (`RGM1`), the same store §1's reference was read over; (2) one warm non-stale `--no-dense ask` through the branch binary — it walks the settled tree (stat-only, nothing changed, no stamp written), passes over the `RGM1` mirror it does not read, and writes its own `RGM2` beside the unchanged `graph.json`, plus the questions mirror, so the rows read a mirror the way the reference's rows did and the rewrite is paid outside the rows; (3) the suite; (4) the four arms — on the fixture, where `bench` loads the store as it stands, and where the counts do not care that the branch parses the fixture's JSON while `main` reads its mirror.

```bash
cd /Users/max/Documents/projects/repograph && FIX=~/bench/beauty-crm-502e8a6d; WT=~/bench/beauty-crm-test; export FIX WT
B=~/bench/levers-2026-09-10/bin/repograph-t5; L=~/bench/levers-2026-09-10/log/g39
bench/probe/reset.sh
REPOGRAPH_NO_SERVE=1 $B --repo $WT --no-dense ask FR-PAY-22 > /dev/null 2>&1 && head -c 4 $WT/.repograph/graph.bin; echo
bench/probe/readers.sh $B $WT $L/readers 5 && python3 bench/probe/judge.py compare ~/bench/levers-2026-09-10/log/readers-reference.txt $L/readers/medians.txt | tee $L/readers-compare.txt
bench/probe/arms.sh $B $FIX $L/arms g39-read
```

Expected: `head -c 4` prints `RGM2`; every reader row `ok` against the reference; the four `arms.sh` transcripts read the baseline table under "Measurement rule for this branch" exactly — counts, p90, and the `anchors` line — and `track.py record` prints four `green`/`measured, no floors` lines.

- [ ] **Step 6: Record**

Under §4 **Reading**: the `graphdiff.txt` output, naming the two copies it compared (`log/g39/main/graph.json`, `log/g39/branch/graph.json`); the build medians as a two-row table; the `oq-add` medians, the `verify` before/after lines and the `oq-25-visible` reading; the `readers-compare.txt` table with the sentence that the rows were read in `~/bench/beauty-crm-test` after a reset and a warm `ask`, like the reference; and under §2 **Reading**: the four arm summary lines and anchor lines beside the baseline, read on the fixture. Under §10, one line per clause that failed, if any. If (a), (b) or (e) failed: stop after the commit below and report — see "Where the branch stops". `$WT` is left as the last arm left it; Task 8 (or Task 10, if the branch stopped) resets before it starts.

- [ ] **Step 7: Commit**

```bash
cd /Users/max/Documents/projects/repograph && git add docs/bench/2026-09-10-next-version-levers-results.md bench/history/runs.jsonl && git commit -m "docs(bench): generic extraction read on the corpus — the visible graph, what is held, what an update costs"
```

**Fails to close when:** clause (a) — one node or one visible edge differs from `main`'s build; clause (b) — `graph.json` past 110%; clause (e) — a reader row outside the bars. Each is a recorded rejection of the design, and the branch stops here.

---

### Task 7: G40 — a corpus that defines no ids, as a state a reader can test

**Files:**
- Create: none
- Modify: `tests/families.rs` (one test), `docs/bench/2026-09-10-next-version-levers-results.md` (§5 reading)

**Interfaces:**
- Consumes: `Graph::pending`, `verify`'s `held aside` line, `families::line`.

The ledger's lever is `Option<IdMatcher>`, `None` when the corpus defines no ids. After Task 4 there is no matcher built from a family list to be `None`: the one matcher is the grammar, and "this corpus declares no ids" is `families::of_graph` being empty — which `build` prints as `families: (none) · milestones: (none)`, `families` prints as `(none)`, and `prime` prints as `0 id families`. The sentinel `[^\s\S]` is gone with the constructor (Task 4, `a_bare_suffix_is_never_an_id`). What this task adds is the test the gate asks for: a fixture with no definitions reads the same graph it reads today.

- [ ] **Step 1: The failing test**

In `tests/families.rs`:

```rust
/// A repository whose documents define nothing: file nodes, an empty family line, and every
/// id-shaped mention held aside where no reader follows it. The state the old never-matching
/// alternation stood in for, now a state the store carries.
#[test]
fn a_corpus_that_defines_no_ids_reads_as_one() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join("docs")).unwrap();
    std::fs::write(repo.join("docs/notes.md"), "# Заметки\n\nдаты по ISO-8601, см. RFC-7231 и -M01\n").unwrap();

    let (ok, out, err) = repograph(repo, &["build"]);
    assert!(ok, "{out}{err}");
    assert!(err.contains("families: (none) · milestones: (none)"), "{err}");
    assert!(out.starts_with("changed 1 removed 0 nodes 1 edges 0"), "one file node and nothing a reader follows: {out}");

    let (ok, out, err) = repograph(repo, &["verify"]);
    assert!(ok, "{out}{err}");
    assert!(out.contains("held aside: 2 edges to ids in 2 prefixes no line defines  ISO RFC"), "{out}");
    assert!(out.contains("dangling edges: 0\n"), "{out}");

    let (ok, out, err) = repograph(repo, &["families"]);
    assert!(ok, "{out}{err}");
    assert!(out.contains("families                                  nodes  defined\n  (none)"), "{out}");
    assert!(mention(&out, "ISO").contains("docs/notes.md:3"), "{out}");

    // An id-shaped word that is no node is a search term, not an exact seed.
    let (ok, out, err) = repograph(repo, &["ask", "ISO-8601"]);
    assert!(ok, "{out}{err}");
    assert!(!out.lines().next().unwrap_or("").starts_with("ISO-8601"), "{out}");
}
```

The `families` header line width: read `render`'s `NAME` constant (36) and `section`'s format before pinning the exact string — the assertion above is written for `{:<36}{:>7}  defined` and must match what `render` prints byte for byte; adjust the literal to the real header rather than the header to the literal.

- [ ] **Step 2: Run to see it fail, then pass**

```bash
cd /Users/max/Documents/projects/repograph && cargo test --test families a_corpus_that_defines_no_ids 2>&1 | grep -E 'test result|panicked'
```

Expected on the first run: a failure naming which line differs (most likely the `families` header literal); fix the literal to what `render` prints, not the other way around, and re-run to `ok`. If `verify` prints a different count, the grammar found a third prefix — `M` from `-M01`? — it must not, by `a_bare_suffix_is_never_an_id`; a third prefix is a defect in Task 4 and is fixed there.

- [ ] **Step 3: Record and commit**

Under §5 **Reading**: the four assertions' actual lines from a local run (`cargo test --test families a_corpus -- --nocapture` after adding a temporary `eprintln!` is not needed — copy them from the test's own literals once green) and one sentence: the constructor is gone, so the `Option` the ledger named has nothing to wrap; the state is the graph's.

```bash
cd /Users/max/Documents/projects/repograph && git add tests/families.rs docs/bench/2026-09-10-next-version-levers-results.md && git commit -m "test(families): a corpus that defines no ids is a state the store carries"
```

**Fails to close when:** the build over a definition-free corpus writes anything but file nodes into `nodes`, or anything into `edges`.

---

### Task 8: G34 — one owner for the grammar, the property test, the edge cases, and `families=` retired

**Files:**
- Modify: `src/doc/requirements.rs` (owns `FAMILY`, `MILESTONE`; the closing-`#` strip; tests)
- Modify: `src/families.rs` (imports the two constants), `src/ids.rs` (`generic()` reads them from `doc::requirements`)
- Modify: `src/walk.rs` (one assertion in an existing test)
- Modify: `src/main.rs` (one test), `src/bench.rs:321-322,447`
- Modify: `bench/history/test_track.py` (one transcript shape)
- Modify: `docs/bench/2026-09-10-next-version-levers-results.md` (§6 reading)

**Interfaces:**
- Produces: `crate::doc::requirements::{FAMILY, MILESTONE}` as `pub(crate) const`; the bench summary line without `families=`.

After Task 5 there is one reader of the definition grammar — `RequirementScanner` — and one of the id grammar — `ids::generic()` — but the two constants they share still live in `families.rs`, a module that no longer defines anything. The owner the ledger names is `doc::requirements`. The property that remains to pin, with the fixed point gone, is that what the extractor writes, `classify` reads back — the one seam `of_graph`, `settle` and the report all stand on.

- [ ] **Step 1: The failing tests — the property, the edge cases, the no-op update, the path**

In `src/doc/requirements.rs`'s `mod tests`:

```rust
    /// The property the family view rests on: every id the extractor writes a node for, and every
    /// id-shaped target it writes an edge to, is one `classify` reads a family off — so the graph
    /// can count, settle and report them without a second grammar to keep in step. Over the
    /// fixtures always; over a corpus when `REPOGRAPH_CORPUS` names one.
    #[test]
    fn every_id_the_extractor_writes_classifies() {
        use crate::families::classify;
        let mut docs: Vec<(String, String)> = ["03-calendar.md", "06-payments.md", "BE-M01-foundation.md"].iter()
            .map(|n| (format!("docs/{n}"), fixture(n))).collect();
        if let Some(corpus) = std::env::var_os("REPOGRAPH_CORPUS") {
            let repo = std::path::PathBuf::from(corpus);
            let entries = crate::walk::walk(&repo, &crate::config::Config::default(), &crate::walk::Manifest::default()).unwrap();
            for e in entries.iter().filter(|e| e.kind == crate::walk::FileKind::Doc) {
                if let Ok(text) = std::fs::read_to_string(repo.join(&e.rel)) { docs.push((e.rel.clone(), text)); }
            }
        }
        let scanner = RequirementScanner::new();
        let mut ids_written = 0;
        for (rel, text) in &docs {
            let ex = scanner.scan(rel, text);
            for n in ex.nodes.iter().filter(|n| !n.id.contains(':')) {
                assert!(classify(&n.id).is_some(), "{rel}: node {} has no family", n.id);
                ids_written += 1;
            }
            for e in ex.edges.iter().filter(|e| crate::ids::generic().is_id(&e.target)) {
                assert!(classify(&e.target).is_some(), "{rel}: edge target {} has no family", e.target);
            }
        }
        assert!(ids_written > 0);
    }

    #[test]
    fn a_quoted_a_commented_and_an_indented_head_define_nothing() {
        let ex = scan("docs/x.md", "> **FR-1 · MUST · quoted**\n<!-- **FR-2 · MUST · commented** -->\n   **FR-3 · MUST · indented**\n");
        assert!(ex.nodes.iter().all(|n| n.id.starts_with("file:")), "{:?}", ex.nodes.iter().map(|n| &n.id).collect::<Vec<_>>());
    }

    #[test]
    fn a_heading_head_loses_its_closing_hash_sequence() {
        let ex = scan("docs/x.md", "## FR-CAL-40 · Единый словарь ##\n\nтело\n");
        assert_eq!(ex.nodes.iter().find(|n| n.id == "FR-CAL-40").unwrap().label, "Единый словарь");
        // A bold head is not a heading; a `#` in its title is its own.
        let ex = scan("docs/x.md", "**FR-CAL-41 · issue #**\n");
        assert_eq!(ex.nodes.iter().find(|n| n.id == "FR-CAL-41").unwrap().label, "issue #");
    }

    #[test]
    fn a_milestone_path_is_read_with_forward_slashes_only() {
        assert!(milestone_file().is_match("docs/plans/BE-M01-foundation.md"));
        assert!(!milestone_file().is_match(r"docs\plans\BE-M01-foundation.md"), "walk normalises to `/`; the grammar never sees a backslash");
    }
```

In `src/walk.rs`'s `mod tests`, in the first test that walks a nested directory (the one at line 134 that builds `entries`), add after its existing assertions: `assert!(entries.iter().all(|e| !e.rel.contains('\\')), "{:?}", entries.iter().map(|e| &e.rel).collect::<Vec<_>>());` — meaningful on the Windows runner, harmless elsewhere.

In `src/main.rs`'s `mod tests`:

```rust
    #[test]
    fn a_no_op_update_re_extracts_nothing_and_writes_the_same_graph() {
        let dir = doc_repo(ONE);
        let (repo, cfg) = (dir.path(), config::Config::default());
        built(repo, &cfg);
        let before = std::fs::read(repo.join(".repograph/graph.json")).unwrap();
        let r = run_update(repo, &cfg, false).unwrap();
        assert_eq!((r.changed, r.removed), (0, 0));
        assert_eq!(std::fs::read(repo.join(".repograph/graph.json")).unwrap(), before, "byte-identical: nothing was re-extracted into it");
    }
```

In `bench/history/test_track.py`, add a transcript whose summary tail carries the 2026-09-09 shape and pin that it still reads:

```python
WITH_FAMILIES = """\
keyword 40/40  paraphrase 15/30  code 12/12  p90 221 tok  dense=true  enriched=true (1996/1996 nodes) model=small families=59  suite=built-in gated=true
"""
```

```python
    def test_a_transcript_from_the_days_the_line_carried_families_still_reads(self):
        p = track.parse_bench(WITH_FAMILIES)
        self.assertEqual((p["model"], p["suite"], p["gated"]), ("small", "built-in", True))
```

- [ ] **Step 2: Run to see them fail**

```bash
cd /Users/max/Documents/projects/repograph && cargo test a_heading_head_loses 2>&1 | grep -E 'panicked|left|right' | head -3
```

Expected: `left: "Единый словарь ##"`. The others pass already or fail to compile until Step 3 — `every_id_the_extractor_writes_classifies` passes on the fixtures today, which is the point: it is the invariant, not a change.

- [ ] **Step 3: Implement**

Move `FAMILY` and `MILESTONE` (with their doc comments, rewritten for their new home: the id slot of the definition grammar and the milestone slot, read by the head regex here, by `ids::generic()` and by `families::classify`) into `src/doc/requirements.rs` as `pub(crate) const`; in `src/families.rs` add `use crate::doc::requirements::{FAMILY, MILESTONE};` and delete the two definitions; in `src/ids.rs` `generic()` reads `crate::doc::requirements::FAMILY` and `MILESTONE`. `milestone_file()` already reads `MILESTONE` — drop the `crate::families::` path. In `RequirementScanner::scan`, where a head is pushed:

```rust
            if let Some(c) = self.head.captures(line) {
                let tail = c.get(4).map(|m| m.as_str().trim().trim_end_matches('*').trim()).unwrap_or("");
                let mut title = c[3].trim();
                // CommonMark closes an ATX heading with an optional `#` sequence that is not part
                // of its text; a bold head has no such sequence, so its `#` stays.
                if line.starts_with('#') { title = title.trim_end_matches('#').trim_end(); }
                heads.push((i, c[1].to_string(), title.to_string(), tail.to_string()));
            }
```

In `src/bench.rs`, delete lines 321–322 (`counted`, `family_count`) and remove ` families={}` and its `family_count` argument from the `println!` at line 447, so the line ends `model={model_field}{code_note}{}  suite={suite} gated={gated}`. Check `tests/json_surface.rs` and `tests/users_day.rs` for any assertion on `families=` in a bench line (`grep -n 'families=' tests/`); none is expected.

- [ ] **Step 4: Run everything, three suites**

```bash
cd /Users/max/Documents/projects/repograph && cargo test 2>&1 | grep -E 'test result|FAILED|panicked' && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && python3 -m unittest discover -s bench/history 2>&1 | tail -1
```

Expected: green, clean, `OK`. Then the corpus half of the property test, in the writable worktree after a reset — the test walks the tree with an empty manifest and reads documents; it opens no store and writes nothing, and the reset is what makes the tree trustworthy after Task 6's `oq.md` cycles:

```bash
cd /Users/max/Documents/projects/repograph && FIX=~/bench/beauty-crm-502e8a6d; WT=~/bench/beauty-crm-test; export FIX WT
bench/probe/reset.sh && REPOGRAPH_CORPUS=$WT cargo test --release every_id_the_extractor_writes_classifies -- --nocapture 2>&1 | grep -E 'test result|panicked'
```

Expected: `ok`.

- [ ] **Step 5: The corpus gate**

*Before* is the build Task 6 copied out — `log/g39/branch/graph.json`, `repograph-t5`'s build from an empty store; the store Task 6 left in `$WT` has been through five `oq` add-and-remove cycles and a settle, and is not trusted as a baseline. *After* is a build from an empty store by `repograph-t8`, copied out before the diff. As numbered: (1) reset; (2) empty the store; (3) build; (4) copy out; (5) diff the copies.

```bash
cd /Users/max/Documents/projects/repograph && cargo build --release 2>&1 | tail -1 && cp target/release/repograph ~/bench/levers-2026-09-10/bin/repograph-t8
FIX=~/bench/beauty-crm-502e8a6d; WT=~/bench/beauty-crm-test; export FIX WT
L=~/bench/levers-2026-09-10/log/g34; mkdir -p $L/t8
bench/probe/reset.sh && rm -rf $WT/.repograph
~/bench/levers-2026-09-10/bin/repograph-t8 --repo $WT --no-dense build 2>&1 | tail -1 && cp $WT/.repograph/graph.json $L/t8/graph.json
python3 bench/probe/graphdiff.py ~/bench/levers-2026-09-10/log/g39/branch/graph.json $L/t8/graph.json | tee $L/graphdiff.txt
```

Expected: `edges: same`; `nodes:` either `same` or `differ` with `changed N` where every listed change is `<id>.label: "… ##" -> "…"` — a heading that lost its closing sequence — and no node only in A or only in B. Any other difference is a defect in Step 3. `$WT` now holds `repograph-t8`'s build over a clean tree, which is what Task 9 reads.

- [ ] **Step 6: Record and commit**

Under §6 **Reading**: the three CI platform results (from the next push's checks, or `cargo test` locally with the note that CI confirms Windows), the `graphdiff.txt` output with every label change listed, and the sentence that `families=` left the bench line at this commit.

```bash
cd /Users/max/Documents/projects/repograph && git add -A src bench/history/test_track.py docs/bench/2026-09-10-next-version-levers-results.md && git commit -m "refactor(doc): the id grammar has one owner, the property it rests on has a test, and the bench line loses families="
```

**Fails to close when:** the property test is red on any platform, or the corpus diff carries a change that is not a stripped closing `#`.

---

### Task 9: G38 — definitions per family, ids per mention-only prefix, and the sort that puts the edge first

**Files:**
- Modify: `src/families.rs` (`Row.definitions`, `Mention.ids`, `Tally.ids`, `record`, `rows`, `finish`'s sort, `render`, tests)
- Modify: `tests/families.rs` (column indexes)
- Modify: `docs/bench/2026-09-10-next-version-levers-results.md` (§7 reading)

**Interfaces:**
- Consumes: `report(graph, cited)` from Task 5.
- Produces: `Row { family, nodes, definitions, defined }`, `Mention { prefix, mentions, ids, files, example }`; JSON gains `definitions` and `ids` (additive — `bench/compare/run.py` reads `family` only).

- [ ] **Step 1: The failing tests**

In `src/families.rs`'s `mod tests`:

```rust
    fn declared(files: &[(&str, &[&str])]) -> Graph {
        let mut g = Graph::default();
        for (file, ids) in files {
            let mut e = Extraction::default();
            e.node(NodeKind::File, &format!("file:{file}"), file, "", file, 1);
            for id in *ids {
                e.node(NodeKind::Requirement, id, "label", "тело", file, 1);
                e.edge(&format!("file:{file}"), id, crate::model::EdgeKind::Declares, "", file);
            }
            g.apply(e);
        }
        g
    }

    #[test]
    fn a_family_defined_once_sorts_first_and_is_marked() {
        let g = declared(&[("docs/a.md", &["REQ-1", "REQ-2", "REQ-3"]), ("docs/b.md", &["OD-1"]), ("docs/c.md", &["REQ-1"])]);
        let r = report(&g, Vec::new());
        assert_eq!(r.families.iter().map(|f| (f.family.as_str(), f.nodes, f.definitions)).collect::<Vec<_>>(),
            vec![("OD", 1, 1), ("REQ", 3, 4)], "REQ-1 is declared by two files, so four definitions over three nodes");
        let text = render(&r);
        let od = text.lines().find(|l| l.trim_start().starts_with("OD")).unwrap();
        assert!(od.ends_with("defined once"), "{od}");
        assert!(!text.lines().find(|l| l.trim_start().starts_with("REQ")).unwrap().contains("defined once"));
    }

    #[test]
    fn mention_only_prefixes_sort_by_distinct_ids_then_by_mentions() {
        let d = one("см. OQ-1, OQ-2, OQ-3 и OQ-1 ещё раз\nсм. ISO-8601, ISO-8601, ISO-8601, ISO-8601, ISO-8601\nRFC-7231\n");
        assert_eq!(d.iter().map(|m| (m.prefix.as_str(), m.ids, m.mentions)).collect::<Vec<_>>(),
            vec![("OQ", 3, 4), ("ISO", 1, 5), ("RFC", 1, 1)]);
    }
```

(`one(text)` returns the survey's `Vec<Mention>` after Task 5; if the helper was renamed there, use its name.)

In `tests/families.rs`, the row `REQ` now reads `REQ  <nodes>  <definitions>  <defined>  [defined once]`: change `assert_eq!((req[1], req[2]), ("1", "docs/req.md:3"), …)` to `assert_eq!((req[1], req[2], req[3]), ("1", "1", "docs/req.md:3"), …)`, and add `assert!(families(&out, "REQ").ends_with("defined once"), "{out}");` beside it. In `every_shape_the_extractor_defines_a_node_with_defines_a_family`, `nodes(name)` still reads field 1.

- [ ] **Step 2: Run to see them fail**

```bash
cd /Users/max/Documents/projects/repograph && cargo test --lib families:: 2>&1 | grep -E '^error|panicked' | head -3
```

Expected: `no field \`definitions\``, `no field \`ids\``.

- [ ] **Step 3: Implement**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mention {
    pub prefix: String,
    pub mentions: usize,
    /// Distinct ids written under the prefix. A prefix cited a thousand times as one id and one
    /// cited a thousand times as a hundred ids are different things to look at.
    pub ids: usize,
    pub files: usize,
    pub example: Option<Site>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    pub family: String,
    pub nodes: usize,
    /// Declaring-file × id pairs: a node defined in two files counts twice, a family with one is
    /// one line that may have been a mistake, and the report puts it first for that reason.
    pub definitions: usize,
    pub defined: Site,
}
```

`Tally` gains `ids: BTreeSet<String>`; `Scan::mentions` collects the hit ids per prefix (`per_prefix: BTreeMap<String, Vec<String>>`) and `record(prefix, ids: Vec<String>, rel, line_no, line)` does `t.mentions += ids.len(); t.ids.extend(ids);`. `finish` builds `Mention { ids: t.ids.len(), .. }` and sorts `by(|a, b| b.ids.cmp(&a.ids).then(b.mentions.cmp(&a.mentions)).then(a.prefix.cmp(&b.prefix)))`. `rows`:

```rust
fn rows(graph: &Graph, counts: &BTreeMap<String, usize>, milestone: bool) -> Vec<Row> {
    let mut out: Vec<Row> = counts.iter().map(|(family, nodes)| {
        let definitions = graph.edges.iter()
            .filter(|e| e.kind == crate::model::EdgeKind::Declares && e.source.starts_with("file:") && in_family(&e.target, family, milestone))
            .count();
        Row { family: family.clone(), nodes: *nodes, definitions, defined: site_of(graph, family, milestone) }
    }).collect();
    out.sort_by(|a, b| a.definitions.cmp(&b.definitions).then(a.nodes.cmp(&b.nodes)).then(a.family.cmp(&b.family)));
    out
}
```

`render`'s `section` prints `{:<NAME$}{:>7}{:>6}  {}` with heads `nodes`, `defs`, `defined`, each row `…{nodes:>7}{definitions:>6}  {file:line}` and `  defined once` appended when `definitions == 1`; the mention section prints `written`, `ids`, `files`, `e.g.` with `{:>7}{:>6}{:>7}`. Keep `NAME` at 36.

- [ ] **Step 4: Run everything**

```bash
cd /Users/max/Documents/projects/repograph && cargo test 2>&1 | grep -E 'test result|FAILED|panicked' && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && python3 -m unittest discover -s bench/compare 2>&1 | tail -1
```

Expected: green, clean, `OK` (the compare harness reads `families --json`'s `family` field, unchanged). Task 7's header literal in `tests/families.rs` (`families … nodes  defined`) changes shape here — update it to what `render` prints now.

- [ ] **Step 5: The corpus gate**

In `$WT` as Task 8 Step 5 left it — `repograph-t8`'s build over a clean tree. `families` reads the graph, walks the tree and writes nothing, so it needs a store in step with its tree and nothing else; if anything has run in the directory since Task 8 Step 5 (an interrupted task, a stray command), repeat that step's reset, `rm -rf` and build first rather than trusting what is on disk.

```bash
cd /Users/max/Documents/projects/repograph && cargo build --release 2>&1 | tail -1 && cp target/release/repograph ~/bench/levers-2026-09-10/bin/repograph-t9
WT=~/bench/beauty-crm-test; export WT
L=~/bench/levers-2026-09-10/log/g38; mkdir -p $L
~/bench/levers-2026-09-10/bin/repograph-t9 --repo $WT families > $L/families.txt 2>$L/families.err
awk '/^mention-only prefixes/{p=1; next} p && /^  /{print; n++} n==5{exit}' $L/families.txt
grep -c 'defined once' $L/families.txt; grep 'defined once' $L/families.txt
```

Expected: the first mention-only row is `OQ`; the `defined once` grep lists every family with exactly one definition (the ledger's "marks every family with one definition"). `families.err` is empty — the store is in step with its tree after Task 8's build. `$L/families.txt` is the artefact; nothing in `$WT` is compared.

- [ ] **Step 6: Record and commit**

Under §7 **Reading**: the top five mention-only rows and the `defined once` list.

```bash
cd /Users/max/Documents/projects/repograph && git add -A src tests docs/bench/2026-09-10-next-version-levers-results.md && git commit -m "feat(families): definitions per family and ids per undefined prefix, sorted so the edge reads first"
```

**Fails to close when:** `OQ` is not the first mention-only row on the corpus, or a single-definition family lacks the mark.

---

### Task 10: G32 — the fixture copy rebuilt, enriched, and read in all four arms before and after

**Files:**
- Modify: `docs/bench/2026-09-10-next-version-levers-results.md` (§8 reading)
- Modify: `bench/history/runs.jsonl` (twelve rows through `track.py`)

**Interfaces:**
- Consumes: `bench/probe/arms.sh`, `reset.sh` (which writes `$WT`'s `repograph.toml` with `embed_model = "intfloat/multilingual-e5-small"`); `FIX` and `WT`; `claude` on `PATH` for the default `enrich_command` (there is no `~/.config/repograph/config.toml` on this machine, so the built-in `claude -p --model haiku …` runs; ≈ $0.20 for ~150 nodes).
- Produces: `~/bench/levers-2026-09-10/log/g32/after/store/` — the enriched, rebuilt store, copied out as an image before the next reset destroys it.

The shipped half (`enrich::unenriched_note`, the README sentence) is on `main`. This is the fixture half: the recorded before/after pair the ledger asks for, the *before* on the fixture and the rebuild in the writable worktree, the pinned fixture untouched.

- [ ] **Step 1: The binary and the before**

The *before* arms are the baseline read once more, on the fixture — `bench` loads the store as it stands and writes nothing:

```bash
cd /Users/max/Documents/projects/repograph && cargo build --release 2>&1 | tail -1 && cp target/release/repograph ~/bench/levers-2026-09-10/bin/repograph-t10
FIX=~/bench/beauty-crm-502e8a6d; WT=~/bench/beauty-crm-test; export FIX WT
B=~/bench/levers-2026-09-10/bin/repograph-t10; L=~/bench/levers-2026-09-10/log/g32; mkdir -p $L/after
bench/probe/arms.sh $B $FIX $L g32-before 2>&1 | grep -E '^(keyword|long|anchors|bench)'
```

Expected: four summary lines and four anchor lines equal to the baseline table under "Measurement rule for this branch"; `enriched=true (1996/1996 nodes)` on each.

- [ ] **Step 2: The rebuild**

As numbered: (1) reset — the store is the fixture's, questions and vectors included, which a `build` keeps (it wipes the graph and the manifest only), so the rebuilt store's coverage is the fixture's questions over the rebuilt nodes; the `embed_model` line is the reset's; (2) the build; (3) the arms, in the writable worktree this time, since the rebuilt store is what they read.

```bash
cd /Users/max/Documents/projects/repograph && FIX=~/bench/beauty-crm-502e8a6d; WT=~/bench/beauty-crm-test; export FIX WT
B=~/bench/levers-2026-09-10/bin/repograph-t10; L=~/bench/levers-2026-09-10/log/g32
bench/probe/reset.sh && grep embed_model $WT/repograph.toml
$B --repo $WT build 2>$L/build.err | tee $L/build.out; cat $L/build.err
```

Expected: `embed_model = "intfloat/multilingual-e5-small"`; on stderr `families: … · milestones: …` (54 ids, 5 milestones), then ``repograph: N requirement-like nodes have no questions — run `repograph enrich` to search them``, then the dense sync's `dense: model open in …` and its progress; on stdout `changed 9<…> removed 0 nodes <…> edges <…>` and `dense: embedded <…> rows in <…>s`. Record N.

```bash
bench/probe/arms.sh $B $WT $L g32-rebuilt 2>&1 | grep -E '^(keyword|long|anchors|bench)'
```

Expected: `enriched=false (<covered>/<eligible> nodes)` on each line with `eligible − covered == N` and coverage between 92% and 99% (the 2026-09-09 reading was 1996/2146); the recorded arms print `gated=true` and are graded on the raw floors, which they are expected to clear (lexical paraphrase read 16/30 on that day's rebuild).

- [ ] **Step 3: Enrich, and the after**

```bash
$B --repo $WT enrich 2>$L/enrich.err | tee $L/enrich.out; tail -3 $L/enrich.err
```

Expected: `enrich: <written> nodes written, <dropped> dropped, <left> still without questions, <batches> batches (0 failed) in <…>s`, then the dense sync of the new rows. `left` is nodes the model declined; coverage is `(eligible − left) / eligible`.

```bash
bench/probe/arms.sh $B $WT $L g32-after 2>&1 | grep -E '^(keyword|long|anchors|bench)'
```

Expected: `enriched=true (<covered>/<eligible> nodes)` with `covered * 100 >= eligible * 99`, both recorded arms `gated=true` and `track.py` printing `green`; the developer arms `measured, no floors`. If coverage is under 99%: run `enrich` once more and read the arms again; still under — record `left` under §10 as the failed clause with the ids the model declined (`python3 -c` over `questions.json` against the eligible nodes is not needed: `enrich`'s own `still without questions` count is the number).

Then, **immediately, before anything else runs in the directory** — Task 11's first act is a reset, which destroys this store — copy it out:

```bash
rsync -a $WT/.repograph/ $L/after/store/ && du -sh $L/after/store && ls $L/after/store
```

Expected: about 80 MB; the seven store files, `questions.json` among them. `$L/after/store/` is a store image and not a place to run anything — an `ask` against a store with no tree beside it empties the graph (runbook trap 7). To read it again, `rsync` it into `$WT` after a reset.

- [ ] **Step 4: Record and commit**

Under §8 **Reading**: one table, rows = suite × arm, columns = before / rebuilt / after, each cell the counts, p90, anchors and coverage, with the note that *before* was read on the fixture and the other two in the writable worktree; the `build.err` and `enrich.err` tails verbatim; N and the cost (from the `claude` CLI's own report if it prints one, else "≈ $0.20 by the ledger's estimate, <batches> batches"); and where the enriched store's image is. Under §2 **Reading**: the `g32-before` lines as a second read of the baseline.

```bash
cd /Users/max/Documents/projects/repograph && git add docs/bench/2026-09-10-next-version-levers-results.md bench/history/runs.jsonl && git commit -m "docs(bench): the fixture copy rebuilt and enriched, read in four arms before and after"
```

**Fails to close when:** coverage stays under 99% after two `enrich` runs, or the before arms do not read the baseline. A rebuilt-and-enriched arm under an enriched floor is recorded as that store's reading; no floor moves and the pinned fixture is not rebuilt — whether the enriched store at `log/g32/after/store/` becomes the next fixture is a decision for Max, not this branch.

---

### Task 11: G19 — a checkpoint budgeted in the unit a forward is

**Files:**
- Modify: `src/index/dense.rs` (`ChunkBudget`, `chunk_ends`, `sync`, `sync_chunked`, the ratio probe, tests)
- Modify: `src/index/embed.rs` (`token_lengths` visibility)
- Modify: `src/main.rs` (`SYNC_CHUNK`, `embed_all`)
- Modify: `docs/bench/2026-09-10-next-version-levers-results.md` (§9 reading)

**Interfaces:**
- Consumes: `bench/probe/embed.sh`, `judge.py cadence`, `reset.sh`; `FIX` and `WT`.
- Produces: `ChunkBudget { tokens: usize, max_rows: usize, ramp: usize }`; `chunk_ends(lens: &[usize], b: ChunkBudget) -> Vec<usize>`; `DenseIndex::sync_chunked(&mut self, graph, questions, embed, measure: &mut dyn FnMut(&[String]) -> Result<Vec<usize>>, budget, after_chunk)`; `Embedder::token_lengths` is `pub`.

The ledger closed G19 on 2026-09-09 with a budget in *characters* and kept the change; what is still open is the unit — a character is not what a forward is budgeted in (`token_batches` counts padded tokens, and a 300-token Cyrillic passage and a 60-token Latin one have the same byte length), and the large-model cadence, which this branch cannot measure. The control is `main`'s own run on this machine, not the band-era 1,930 s the ledger first recorded: that number was a property of a scheduler setting removed on 2026-09-09.

- [ ] **Step 1: The failing tests**

In `src/index/dense.rs`'s `mod tests`, change the test at line 400 to build `ChunkBudget { tokens: 600_000, max_rows: 1024, ramp: 16 }` over token lengths rather than texts (its `texts` become `lens: Vec<usize>` of the same values it derived from `chars().count()` before), give every `sync_chunked` call a `&mut |t: &[String]| Ok(t.iter().map(|s| s.len()).collect::<Vec<_>>())` after its `embed` closure, and add:

```rust
    #[test]
    fn a_chunk_is_closed_by_tokens_not_by_rows() {
        // Two long rows fill the budget; the short one after them is its own chunk.
        assert_eq!(chunk_ends(&[600, 600, 10], ChunkBudget { tokens: 1000, max_rows: 1024, ramp: 1 }), vec![2, 3]);
        // A single row over the budget still gets a chunk; the alternative is one that never ends.
        assert_eq!(chunk_ends(&[5000, 10], ChunkBudget { tokens: 1000, max_rows: 1024, ramp: 1 }), vec![1, 2]);
    }

    #[test]
    fn the_lengths_are_measured_once_and_only_when_a_budget_asks_for_them() {
        let mut idx = DenseIndex::default();
        let mut fake = |t: &[String]| Ok(t.iter().map(|_| vec![1.0, 0.0]).collect());
        let mut measured = 0;
        let mut lens = |t: &[String]| { measured += 1; Ok(t.iter().map(|s| s.len()).collect()) };
        idx.sync_chunked(&wide(0), &Questions::default(), &mut fake, &mut lens, ChunkBudget::rows(4), &mut |_, _| Ok(())).unwrap();
        assert_eq!(measured, 0, "a row budget never tokenises");
        let mut idx = DenseIndex::default();
        idx.sync_chunked(&wide(0), &Questions::default(), &mut fake, &mut lens, ChunkBudget { tokens: 8, max_rows: 1024, ramp: 1 }, &mut |_, _| Ok(())).unwrap();
        assert_eq!(measured, 1, "one pass over every row to plan the chunks, before the first forward");
    }
```

(`wide(n)` and `DenseIndex::default()` are the helpers the existing tests at lines 412–455 use; read them and match their names.)

- [ ] **Step 2: Run to see them fail**

```bash
cd /Users/max/Documents/projects/repograph && cargo test --lib dense:: 2>&1 | grep -E '^error' | head -3
```

Expected: `no field \`tokens\``, `this method takes 5 arguments but 6 arguments were supplied`.

- [ ] **Step 3: Implement**

`src/index/dense.rs`:

```rust
/// How much work one checkpoint covers, in the unit a forward is budgeted in. Tokens rather than
/// rows because a row is not a unit of work — 1,024 short questions and 1,024 long passages are
/// the same count and a minute apart — and tokens rather than characters because a character is
/// not one either: a 300-token Cyrillic passage and a 60-token Latin one have the same byte
/// length, and `token_batches` pads and bounds a forward in tokens. The lengths cost one
/// tokenizer pass over the rows to be embedded before the first forward; that pass is the
/// planning, and it is skipped when a row count is all that was asked for. `ramp` divides the
/// budget for the first chunk and halves its way out after each, so the first line does not wait
/// for the run's fixed start-up *and* a full chunk on top of it.
#[derive(Clone, Copy, Debug)]
pub struct ChunkBudget { pub tokens: usize, pub max_rows: usize, pub ramp: usize }

impl ChunkBudget {
    /// A fixed number of rows and no ramp: what a test or a one-chunk sync asks for.
    pub const fn rows(n: usize) -> Self { ChunkBudget { tokens: usize::MAX, max_rows: n, ramp: 1 } }
}

/// Where each checkpoint falls, as end offsets into the rows whose token counts are `lens`. A
/// single row over budget on its own still gets a chunk; the alternative is a chunk that never ends.
pub fn chunk_ends(lens: &[usize], b: ChunkBudget) -> Vec<usize> {
    let mut ends = Vec::new();
    let mut divisor = b.ramp.max(1);
    let (mut tokens, mut rows) = (0usize, 0usize);
    for (i, &len) in lens.iter().enumerate() {
        tokens = tokens.saturating_add(len);
        rows += 1;
        if tokens >= b.tokens / divisor || rows >= (b.max_rows / divisor).max(1) {
            ends.push(i + 1);
            divisor = (divisor / 2).max(1);
            tokens = 0;
            rows = 0;
        }
    }
    if !lens.is_empty() && ends.last() != Some(&lens.len()) { ends.push(lens.len()); }
    ends
}
```

`sync` passes `&mut |t: &[String]| Ok(vec![0; t.len()])` as `measure`; `sync_chunked` gains `measure: &mut dyn FnMut(&[String]) -> Result<Vec<usize>>` after `embed` and plans with

```rust
        let lens = if budget.tokens == usize::MAX { vec![0; todo_texts.len()] } else { measure(&todo_texts)? };
        let plan = chunk_ends(&lens, budget);
```

`grep -n 'sync_chunked(' src` must show only `dense.rs` and `main.rs` — `watch`, `serve` and `ask` resync through `sync`. In `src/index/embed.rs`, `fn token_lengths` becomes `pub fn token_lengths`. In `src/main.rs`, `embed_all` shares the embedder between the two closures through a `RefCell` (the measure borrows it shared, the forward mutably, and `sync_chunked` calls them in turn, never together):

```rust
    let emb = std::cell::RefCell::new(emb);
    let n = dense.sync_chunked(&graph, &questions,
        &mut |texts| emb.borrow_mut().embed(texts),
        &mut |texts| emb.borrow().token_lengths(texts),
        SYNC_CHUNK, &mut |idx, p| { … unchanged … })?;
```

(`emb.dim()?` is called before the wrap, as today.) Add the instrument that sizes the constant, in `dense.rs`'s `mod tests`:

```rust
    /// Not a test of anything: the instrument that sizes `SYNC_CHUNK`, run by hand against a
    /// store copy — `REPOGRAPH_PROBE_STORE=<repo> cargo test --release -- --ignored --nocapture chars_per_token`.
    /// It prints the corpus's characters-per-token ratio for the small model, which is what turns
    /// the recorded 600,000-character budget into a token budget of the same size.
    #[test]
    #[ignore]
    fn chars_per_token_on_a_store() {
        let Some(repo) = std::env::var_os("REPOGRAPH_PROBE_STORE") else { return };
        let store = Store::new(std::path::Path::new(&repo));
        let (graph, _) = store.load().unwrap();
        let questions = Questions::load(&store).unwrap();
        let texts: Vec<String> = graph.nodes.values().flat_map(|n| rows(n, &questions)).collect();
        let emb = crate::index::embed::Embedder::open(crate::index::embed::DEFAULT_MODEL, 4, crate::index::embed::Weights::Mapped).unwrap();
        let lens = emb.token_lengths(&texts).unwrap();
        let (chars, tokens): (usize, usize) = (texts.iter().map(|t| t.chars().count()).sum(), lens.iter().sum());
        eprintln!("rows={} chars={chars} tokens={tokens} chars/token={:.3}", texts.len(), chars as f64 / tokens as f64);
    }
```

- [ ] **Step 4: Size the constant from the corpus**

After a reset, so the rows the ratio is read over are the fixture's — Task 10 left its enriched, rebuilt store in the directory, whose rows are more and different, and the recorded 600,000-character budget was sized on the fixture's shape. The probe opens the store through `Store::load` and `Questions::load` and writes nothing.

```bash
cd /Users/max/Documents/projects/repograph && FIX=~/bench/beauty-crm-502e8a6d; WT=~/bench/beauty-crm-test; export FIX WT
bench/probe/reset.sh && REPOGRAPH_PROBE_STORE=$WT cargo test --release -- --ignored --nocapture chars_per_token 2>&1 | grep '^rows='
```

Expected: one line, `chars/token` between 2.0 and 4.0 for this Cyrillic-and-Latin corpus under e5's tokenizer; outside that band the probe is reading the wrong thing (a store with no questions, the wrong model) — stop and check. Set `SYNC_CHUNK` in `src/main.rs` to `ChunkBudget { tokens: T, max_rows: 1024, ramp: 16 }` where `T = 600_000 / ratio` rounded to the nearest 10,000, and rewrite its comment: the same amount of work the recorded 600,000-character chunk was on this corpus, in the unit a forward is budgeted in, with the ratio and the date it was read.

- [ ] **Step 5: Run everything**

```bash
cd /Users/max/Documents/projects/repograph && cargo test 2>&1 | grep -E 'test result|FAILED|panicked' && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1
```

Expected: green, clean.

- [ ] **Step 6: The control, then the candidate — three each, on a quiet machine, awake**

As numbered: (1) reset; (2) the control's three runs — `embed.sh` removes the vectors before each; (3) reset again, so both binaries start from the same bytes; (4) one warm non-stale `--no-dense ask` through the branch binary for its own `RGM2` mirrors — `embed` loads the graph and the questions through their mirrors, and without this the candidate would parse 12 MB of JSON at start-up that the control does not, inside the `first` interval and the RSS; (5) the candidate's three runs. Every artefact is what `embed.sh` writes into `$L` — `.err`, `.out`, `.samples`, `.quiet` and the summary lines — nothing in `$WT` is compared, so nothing is copied out.

```bash
cd /Users/max/Documents/projects/repograph && cargo build --release 2>&1 | tail -1 && cp target/release/repograph ~/bench/levers-2026-09-10/bin/repograph-t11
FIX=~/bench/beauty-crm-502e8a6d; WT=~/bench/beauty-crm-test; export FIX WT
L=~/bench/levers-2026-09-10/log/g19; mkdir -p $L
bench/probe/reset.sh
for i in 1 2 3; do bench/probe/embed.sh control-$i ~/bench/levers-2026-09-10/bin/repograph-main $WT $L; done
bench/probe/reset.sh && REPOGRAPH_NO_SERVE=1 ~/bench/levers-2026-09-10/bin/repograph-t11 --repo $WT --no-dense ask FR-PAY-22 > /dev/null 2>&1 && head -c 4 $WT/.repograph/graph.bin; echo
for i in 1 2 3; do bench/probe/embed.sh tokens-$i ~/bench/levers-2026-09-10/bin/repograph-t11 $WT $L; done
python3 bench/probe/judge.py medians $L/summary.txt; grep -E '^(control|tokens)-[123] ' $L/summary.txt
```

`head -c 4` prints `RGM2` before the candidate's runs. About 4–5 minutes a run at the default `balanced` (the 2026-09-09 reading was 265.7 s, range 230.7–292.0), so half an hour in all; `embed.sh` refuses to start on a machine that is not quiet and keeps it awake once it does. Then the cadence of the candidate's median-wall run (pick it by `wall=` from the three `tokens-` lines):

```bash
python3 bench/probe/judge.py cadence $L/tokens-<median>.err; python3 bench/probe/judge.py cadence $L/control-<median>.err
```

Expected on the candidate: `ok:` with `first` under 60, `max` under 60, `ratio` ≤ 1.300; the control's own cadence recorded beside it (the character budget read 0.5–8.3 s intervals on 2026-09-09, ratio about 1.63 by its own numbers). Then the three clauses by hand from the six summary lines: candidate median wall inside [min, max] of the control's three or within 10% of the control median, whichever is wider; median max RSS within 5%; median peak CPU within 10%.

- [ ] **Step 7: Record and commit**

Under §9 **Reading**: a six-row table (run, wall, user, max RSS, peak CPU, first interval, max interval, ratio), the two `judge.py cadence` outputs, the ratio and `T` from Step 4, and the verdict per clause. If a clause failed, it is written under §10 too and `SYNC_CHUNK` stays as sized — the bar is not moved to meet it.

```bash
cd /Users/max/Documents/projects/repograph && git add -A src docs/bench/2026-09-10-next-version-levers-results.md && git commit -m "perf(embed): a checkpoint budgeted in tokens, the unit a forward is budgeted in"
```

**Fails to close when:** any interval, the first included, reaches 60 s; the tail ratio exceeds 1.3; or wall, RSS or CPU leave the control's bars. Each is recorded, and the token budget is kept or reverted on the reading — reverted only if a resource clause failed, since a cadence clause failing on the small model would fail on the character budget too and says nothing the ledger did not.

---

### Task 12: The ledger, the README, the runbook, and the closing review

**Files:**
- Modify: `docs/bench/next-version-gaps.md` (a status paragraph under G23, G32, G34, G35, G38, G39, G40, G15, G19; the three "Suggested order" tables)
- Modify: `README.md` (the `verify` paragraph at lines 287–289; the "Id families" `repograph families` bullet)
- Modify: `docs/bench/runbook.md` (the reader-suite section at lines 225–275 points at `bench/probe/`)
- Modify: `docs/bench/2026-09-10-next-version-levers-results.md` (§10)

- [ ] **Step 1: The ledger**

Under each gap's heading, after its last paragraph, add one paragraph beginning `**Status (2026-09-10, \`feat/next-version-levers\`).**` that states: what shipped (the commit subject), the reading against the gate with the numbers from the results document, and whether the row is closed, measured-and-rejected, or open with what is left. The wording for the two rows whose gate this plan read differently from its letter:

- G40: `Closed by removal. The lever named \`Option<IdMatcher>\`; after G39 there is no matcher built from a family list to be \`None\` — \`ids::generic()\` is the grammar, and "this corpus declares no ids" is \`families::of_graph\` reading empty, which \`build\` prints as \`families: (none)\`. The never-matching alternation is gone with the constructor; the state is pinned by \`a_corpus_that_defines_no_ids_reads_as_one\` and \`a_bare_suffix_is_never_an_id\`.`
- G19: `The unit is tokens now (\`SYNC_CHUNK\` at <T> tokens, sized from a measured <ratio> characters per token on the fixture). Control and candidate are three runs each of \`main\`'s binary and the branch's on this machine at \`balanced\` — not the band-era 1,930 s / 293% / 2.15 GB, which were a scheduler setting removed on 2026-09-09. Cadence: first <s>, longest <s>, ratio <x> against 1.3; wall / RSS / CPU: <numbers> against the control's. The large-model cadence remains unmeasured: 2.1 GB this branch does not download.`

In the three "Suggested order" tables, strike the rows that closed (`~~**G23** …~~` with `closed 2026-09-10 — …` in the "why here" cell, in the style the tables already use) and reword the rows that stayed open.

- [ ] **Step 2: README and runbook**

In `README.md`'s `verify` paragraph (lines 287–289), after `cited, such as milestone task ids named from code:` add a sentence: `; and, on a line of its own, how many citations are held aside because no line defines their prefix — \`ISO-8601\`, a ticket number — which no reader follows and which \`repograph families\` lists as mention-only.` In the "Id families" section's command block change the comment on `repograph families` to `# families with their definitions, milestones, and every prefix left as text with the ids it cites`.

In `docs/bench/runbook.md`, replace the `perf-time.sh` block and the paragraph above it (lines 246–275) with:

```markdown
Only then, the clock — through `bench/probe/`, which is the 2026-09-07 kit in the tree:
`bench/probe/reset.sh` puts the pinned fixture's store into the one writable worktree
`~/bench/beauty-crm-test` and refuses any other directory; `bench/probe/readers.sh <binary>
<worktree> <log> 5` then reads the ten reader rows five times each there, every row `--stale` so
nothing writes under them; `judge.py compare <reference> <medians>` judges them against the reference
`docs/bench/2026-09-10-next-version-levers-results.md` §1 recorded; and `quiet.sh` refuses to start
on a machine that is not idle, plugged in, and free of builds. The bars are 10% wall and 5% max RSS
on medians of five, and they are usable because the same binary read inside them twice over a store
whose checksums did not move. The fixture itself is read by `bench` and `dump` only.
`bench/probe/README.md` has the rest.
```

- [ ] **Step 3: The closing review**

Run `/code-review high --fix` over the branch (`main..HEAD`), apply the findings, then:

```bash
cd /Users/max/Documents/projects/repograph && cargo test 2>&1 | grep -E 'test result|FAILED' && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && python3 -m unittest discover -s bench/history 2>&1 | tail -1 && python3 -m unittest discover -s bench/probe 2>&1 | tail -1 && python3 -m unittest discover -s bench/compare 2>&1 | tail -1
```

Expected: green, clean, `OK` three times. A finding that changes a read path re-runs `bench/probe/arms.sh <binary> ~/bench/beauty-crm-502e8a6d <log> review-read` — on the fixture, like every baseline read — and the four lines must read the baseline.

- [ ] **Step 4: Commit**

```bash
cd /Users/max/Documents/projects/repograph && git add -A docs README.md && git commit -m "docs(bench): nine ledger rows read against their gates on feat/next-version-levers"
```

If the review's fixes touched code, they are committed first under their own `fix:` subject naming what the review found, in the style of `fix: five the review found — …` on `main`.

- [ ] **Step 5: Leave the directories**

Nothing is removed, and nothing is unlocked. `~/bench/beauty-crm-502e8a6d` was never written. `~/bench/beauty-crm-test` stays locked, holding whatever Task 11's last embed left — the next campaign's first act is a reset, so its state at the end of this branch is nobody's concern. `~/bench/levers-2026-09-10/` stays: `log/g32/after/store/` is the enriched, rebuilt store Max may pin next (a store image — to read it, `rsync` it into `~/bench/beauty-crm-test` after a reset; never `ask` it where it lies), and the logs are what the results document cites. `~/bench/levers-2026-09-10/main-src` is a worktree of this repository, not of the corpus, and can be removed with `git worktree remove ~/bench/levers-2026-09-10/main-src` once the branch is merged. Nothing is pushed.

---

## Self-review

**Spec coverage.** G23 → Task 1 (n runs and a median: `readers.sh` + `judge.py medians`; quiet precondition: `quiet.sh`; pinned index: the locked fixture's store, copied into the one writable worktree by `reset.sh` and read by rows none of which can write — every row `--stale`, the store's checksums identical before and after; the gate's twice-through-one-binary control: Step 8). G15 → Task 2 (the line is read into the history; the rule names the anchor totals: "Measurement rule", applied in Tasks 6 and 10). G35 → Task 3 (the stage and the number; the band half is moot, the band is gone). G39 → Tasks 4–6 (generic grammar, prefix recorded as the family the graph classifies rather than a field on the node — `classify(id)` is a pure function of the id and a stored copy would be a second thing to keep in step; the family set as `of_graph`; the store's growth measured as `pending`; the one-file update; the read path). G40 → Task 4 (the constructor and the sentinel removed) and Task 7 (the state pinned; the ledger note that `Option` had nothing left to wrap). G34 → Task 8 (one owner; the property test; the five unmeasured cases — blockquote, HTML comment, indented, trailing `#`, backslash path; the no-op update re-extracting nothing; `families=` retired with its parser checked). G38 → Task 9. G32 → Task 10 (the shipped half is on `main`; the fixture half here, on the copy, with `embed_model` set in Task 0). G19 → Task 11 (tokens, the control on today's binary, the large model out of scope by the one line under "Out of scope").

**The one thing the scan card says that this plan does not do.** The card reads `schema=graph node gains an id prefix (G39)`. The node does not gain a field: `families::classify` reads the prefix off the id, and `Graph::settle` reads it at the moment it partitions, so a stored prefix would be a copy of a pure function's result and a second thing to migrate. What the graph gains is `pending`, and Task 4's store test is the "a store built by the previous version still reads" test the card asks for, with the mirror magic bumped so the answer does not depend on postcard failing at the right byte.

**Placeholder scan.** No `TBD`/`TODO`/"similar to Task N"; the two values an executor fills from a measurement — `T` in Task 11 and the machine line in Task 0 — carry the formula or the command that produces them. Task 7's `families` header literal is the one string written to be corrected against `render`'s output, and the step says so.

**Type consistency.** `Families = (BTreeMap<String, usize>, BTreeMap<String, usize>)` (Task 5) is what `of_graph` returns (unchanged since `main`) and what `line`, `moved` and `settle` consume; `Row.defined: Site` (Task 5) is what Task 9's `rows` builds and `defined_at(&Site)` prints; `apply_diff(repo, store, graph, entries, diff, ex, timing)` (Task 5) is the shape Tasks 6, 8 and 11 call through `run_update`; `sync_chunked(graph, questions, embed, measure, budget, after_chunk)` (Task 11) is the only chunked caller's shape in `embed_all`; `ids::generic()` returns `&'static IdMatcher` everywhere it is named (Tasks 4, 5, 8); `judge.py`'s `medians` output is `read_medians`'s input by the `MEDIAN_LINE` regex (Task 1) and `compare`'s two arguments in Task 6 are both files of that shape. `readers.sh BIN WORKTREE LOG [N]` and `embed.sh NAME BIN WORKTREE LOG` take the worktree Tasks 1, 6 and 11 pass as `$WT`; `reset.sh` reads `FIX`, `WT`, `PIN` and `BIN` from the environment, and every block that calls it exports the first two on its own first line, so no block depends on another's shell. `arms.sh`'s `<repo>` is `$FIX` in Tasks 6, 10 (before) and 12 and `$WT` in Task 10 (rebuilt, after). The two copied-out paths Task 8 compares — `log/g39/branch/graph.json` and `log/g34/t8/graph.json` — are the names Task 6 Step 2 and Task 8 Step 5 write.

**Directories.** No task creates, removes or unlocks a worktree of the corpus. `$FIX` is named only in `arms.sh` calls (`bench`, which never walks), in `reset.sh`'s `rsync` source, and in Task 0's check; every `build`, `update`, `enrich`, `embed`, `ask` without `--stale`, and every reader row, names `$WT`. Tasks 1, 3, 6, 8, 9, 10 and 11 each begin with `reset.sh` before their first arm, and Tasks 6, 8 and 10 copy out before the next arm runs.
