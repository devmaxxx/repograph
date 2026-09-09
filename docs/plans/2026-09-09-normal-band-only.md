# The scheduling band is removed, and the default model's numbers are re-measured

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:executing-plans (or
> superpowers:subagent-driven-development) to implement this task-by-task. Steps use checkbox
> (`- [ ]`) syntax. Every number in this file is either read from a run named beside it or is an
> *expectation* labelled as one; a number you write comes from a run you name.

**Goal.** After this change repograph runs in the normal scheduling band on every platform, with no
band call anywhere in the binary and no setting that names one; how much of the machine a rebuild
takes is a word (`resources = "low" | "balanced" | "full"`) over the thread count it already had;
and the published resource numbers for the **default** embedder (`intfloat/multilingual-e5-small`)
are today's, at all three levels, rather than the pre-cap ones from 2026-09-07.

**Decided by Max on 2026-09-09, not open in this plan:**

1. The background band goes **entirely** — `src/priority.rs`, the `priority` key, `REPOGRAPH_PRIORITY`,
   every call site, the README subsection and its tables, and every test that covers it.
2. ~~`threads` stays the only resource lever~~ **— superseded by Amendment 1 (Part F).** What still
   stands from this item: the default stays a third of the logical cores (4 on this machine), and
   the token budget and the checkpointing do not move. What changed: a second key, `resources`,
   joins `threads` and reaches the same one number.
3. The retest is on the default model. The published e5-small figure (214 s / 1.63 GB) predates the
   thread cap, the token budget and the chunked sync, and is quoted today as though it were current.

**Amendment 1, added by Max on 2026-09-09 while this plan was still unexecuted.** Both `threads` and
a coarse resource level must be configurable: a `resources` key taking `"low" | "balanced" | "full"`,
reaching **thread count only**. The whole design, its bars, its retest rows, its README lines and its
commit are **[Part F](#part-f--amendment-1-the-resources-level)**. Parts A–E stand except where a
`> **Amendment 1**` marker says otherwise; every such marker names the Part F section that replaces
it. Nothing in Amendment 1 reopens the removal: the band still goes, entirely, first.

**Amendment 2, added by Max on 2026-09-09 after commit 1 had landed: keep only levels, do not
configure threads manually.** This is what was built, and it overrides Amendment 1 wherever the two
disagree. Six changes, all now executed:

1. **`threads` is removed too**, with `REPOGRAPH_THREADS`. `resources` is the only resource lever in
   the tool. Part F's precedence rule (§F.4), its both-set warning, its `threads`-wins-across-keys
   corner and the README's "`threads = 6` is the escape hatch" line are struck — there is no second
   key for any of them to be about.
2. **`REPOGRAPH_RESOURCES` is the one-run override**, layered where `REPOGRAPH_THREADS` was, and
   `resources` sits in the machine-file allowlist where `threads` sat: it describes the machine, not
   the corpus.
3. **`balanced` stays the default and stays byte-for-byte today's behaviour** — `(cores / 3).max(1)`,
   4 here. A file naming neither key behaves exactly as `fec9496` did; the retest proves it with a
   run that sampled 8 threads / 4 running with nothing set in the environment.
4. **`low` is `(cores / 6).max(1)`** = 2 here, per H3 and §4.2's two `threads = 2` rows.
5. **`full` was re-decided by measurement**, which was the one open question. §F.2 chose "no cap at
   all" while `threads = N` still existed as a hand escape hatch; it no longer does, so the machine's
   best measured shape had become unreachable by anyone. Both candidates were run on e5-small:
   **no cap = 178.1 s at 18 threads / 6 running**, **`(cores / 2).max(1)` = 6 → 168.8 s at 12 / 6**,
   on 815.2 against 813.8 user seconds. The capped shape ships; the losing row is published beside it
   in the results doc and in `threads_from`'s doc comment. §F.2's zero-sentinel was nevertheless
   verified on the way — the no-cap run sampled exactly the 18 / 6 it predicted — so the fallback it
   named was never needed.
6. **The migration paragraph covers two removed keys**, and says `threads` is the one a reader is
   more likely to hit, because README line 523 and the 2026-09-07 results both told people to set it:
   `threads = 6` → `resources = "full"`, `threads = 2` → `resources = "low"`,
   `priority = "background"` → delete the line.

Commit shape: commit 1 `perf:` (landed as `76f1049`), commit 2 **`feat(config)!:`** — the `!` earned,
two documented keys removed — and commit 3 `docs:`. Both code commits independently green.

**Before (the state this changes).** `fec9496`, branch `claude/resource-usage-modes-1c05e6`, version
0.5.0 untagged. Writers call `priority::apply(cfg.priority)` before `cap_pools`, default
`background`; readers never call it.

**Tech stack.** Rust 1.98 (`rust-toolchain.toml`), `cargo build --release`,
`cargo test --release`, `cargo clippy --release --all-targets -- -D warnings`, all with no
`REPOGRAPH_*` variable set. One dependency **leaves**: `libc`. `/usr/bin/time -l` and a one-second
`top -l 2 -s 1 -pid` sampler for the measurements; bash 3.2.

## Global constraints

- Work in this worktree only: `W=/Users/max/Documents/projects/repograph/.claude/worktrees/routing-effort-model-skill-50259a`,
  branch `claude/resource-usage-modes-1c05e6`. Never `cd` into the main checkout; use
  `cargo … --manifest-path "$W/Cargo.toml"` and absolute paths. `B="$W/target/release/repograph"`,
  rebuilt before any measurement.
- The pinned fixture `/Users/max/bench/beauty-crm-502e8a6d` is **read-only** and is never rebuilt:
  `ask --stale`, `bench`, `dump` only. Every writer runs on a copy.
- New bench directory `G=/Users/max/bench/resources-2026-09-09`. The 2026-09-07 kit
  `P=/Users/max/bench/resources-2026-09-07` is read-only input: `measure.sh`, `quick.sh`,
  `queries10.jsonl`, `legacy-graph.json`, `store-L` are copied or read, never overwritten.
- Measurements export `REPOGRAPH_NO_SERVE=1` (except the `serve` rows); `cargo test` runs in a shell
  where no `REPOGRAPH_*` is set.
- A copy pins `embed_model = "intfloat/multilingual-e5-small"` in its own `repograph.toml`. It is
  the default today, so this is belt and braces rather than necessity — and it is the trap that cost
  a run in the previous round.
- Comments say why, never what; no ticket ids in code; tool directives stay. Test names are
  `snake_case` sentences in the style of the file they join. Commit subjects are Conventional, no AI
  trailers beyond the required `Co-Authored-By`; a heredoc and a `git commit` go in two shell
  commands.
- CI (`.github/workflows/ci.yml`) runs `cargo clippy --all-targets -- -D warnings` and `cargo test`
  on ubuntu-latest, macos-14 and windows-latest on every push. The three platform branches of
  `lower()` compile there today; after the removal there is nothing platform-specific left in this
  area at all.

## Acceptance bars, fixed before anything is run

**The removal.**

| # | bar |
|---|---|
| A1 | `rg -n "priority\|Priority\|REPOGRAPH_PRIORITY\|PRIO_\|SCHED_IDLE\|ioprio\|taskpolicy\|setpriority\|libc" src/ tests/ Cargo.toml` returns **nothing**. (`docs/` and `README.md` are excluded from A1: dated documents keep their historical mentions, and the README keeps exactly one sentence naming the removed key — B1.) |
| A2 | `cargo test --release` green. **After commit 1 alone** the suite loses 8 tests (4 in `src/priority.rs`, 4 in `src/config.rs`) and gains 1 (A6), a net −7. After commit 2 it gains 5 more (§F.6), a net +5 on top: −7 then +5. |
| A3 | `cargo clippy --release --all-targets -- -D warnings` silent. |
| A4 | `Cargo.lock`'s `[[package]] name = "repograph"` dependency list no longer names `libc`; the `[[package]] name = "libc"` entry **stays** (27 other crates depend on it) and no other package's list changes. |
| A5 | A `repograph.toml` holding `priority = "background"` makes every command that loads config exit non-zero with a message naming **both** the file and the key. Pinned by a test (§A.6). |

**The retest** (e5-small, no configuration but the pinned `embed_model`, this branch's release
binary, machine quiet — `uptime` recorded before and after every long row).

| # | row | bar | expectation |
|---|---|---|---|
| C1 (= **C1a**) | full re-embed, 33,525 rows, **`resources` at its default** | wall ≤ **214.1 s**, max RSS ≤ **1.63 GB**, peak CPU ≤ **310%** (avg ≤ 300%), peak threads ≤ **9** | 150–185 s, ≤ 1.2 GB, ≈ 293%, 8 threads |
| C1b | the same at `resources = "full"` | §F.5 | §F.5 |
| C1c | the same at `resources = "low"` | §F.5 | §F.5 |
| C2 | `build` that embeds nothing | ≤ 3.0 s, ≤ 1.63 GB | ≈ 1.6 s |
| C3 | `build --no-dense` | ≤ 3.0 s, ≤ 0.15 GB | ≈ 1.9 s |
| C4 | `update --no-dense`, nothing changed | ≤ 0.5 s, ≤ 0.15 GB | ≈ 0.07 s |
| C5 | every reader | within **max(0.05 s, 20%)** of its 2026-09-07 §4.4 after-row and within **0.15 GB** of its max RSS; `ask --rerank-local` ≤ 40 s and ≤ 3.20 GB | unchanged |
| C6 | checkpoint resume | second run's total is **exactly** 33,525 − (rows on disk after the kill); the finished store holds 33,525 rows and answers `штраф за отмену записи` with `FR-CAL-95`, `FR-TOOL-35`, `N-025`, `FR-PAY-26`, `FR-DM-48` | as 2026-09-07 §4.3 |
| C7 | floors | exit 0; dense `keyword 40/40 paraphrase 15/30 code 12/12 p90 221 tok`, lexical `keyword 39/40 paraphrase 15/30 code 12/12 p90 215 tok`, both `gated=true green=true` | identical to the `502e8a6d` rows in `bench/history/runs.jsonl` |

C5 is a **guard, not a hypothesis**: no reader ever called `priority::apply` (`src/main.rs` —
`Ask`, `Serve`, `Impact`, `Trace`, `Changes`, `Explain` reach `cap_pools` or nothing at all), so a
reader that moves has moved for another reason or is noise. G23 says these bars are tighter than the
suite's own repeatability, so a row outside its bar is **re-run three times and the median
reported**, with the load average it was taken under, before it is called a regression.

C7 is a guard too, for a stronger reason: removing a scheduling call cannot change which nodes a
query retrieves. It is run to prove nothing else was disturbed on the way.

---

## Part A — the removal

### A.1 `src/priority.rs` — delete the whole file (172 lines)

It holds `enum Priority`, `from_env`, `apply`, the three `lower()` branches (macOS
`setpriority(PRIO_DARWIN_PROCESS, …)`, Linux `setpriority` + `sched_setscheduler(SCHED_IDLE)` +
`ioprio_set`, and the `#[cfg(not(any(macos, linux)))]` branch that prints
`priority: background is not implemented on this platform…`), the `warn` helper and four tests.

**The Windows "does nothing" line lives only here** — it is the `eprintln!` at `src/priority.rs:101`
— so deleting the file deletes it. Confirm with `rg -n "not implemented on this platform" src/`
returning nothing afterwards.

### A.2 `src/main.rs` — one module line, four call sites, one comment

| line | edit |
|---|---|
| 15 | delete `mod priority;` |
| 399–402 | delete the four-line comment beginning `// Before \`cap_pools\`, and before any session:` — it exists only to justify the call below it |
| 403 | delete `priority::apply(cfg.priority);` (`Cmd::Build \| Cmd::Update`) |
| 411 | delete `priority::apply(cfg.priority);` (`Cmd::Enrich`) |
| 424 | delete `priority::apply(cfg.priority);` (`Cmd::Embed`) |
| 480 | delete `priority::apply(cfg.priority);` (`Cmd::Watch`) |

Every `cap_pools(index::embed::threads(cfg.threads));` line directly below each of those **stays**:
`threads` is the surviving lever and its call order is unaffected.

> **Amendment 1.** Those lines stay in position through commit 1 and keep this exact text. Commit 2
> gives `index::embed::threads` a second argument, so their text changes then, at twelve call sites
> listed in §F.3. Doing the removal first is what keeps every line number in Part A readable against
> `fec9496`.

### A.3 `src/config.rs` — seven edits

| lines | edit |
|---|---|
| 47–55 | delete the `priority` doc comment and the field `pub priority: crate::priority::Priority,` from `Config` |
| 101 | delete `priority: crate::priority::Priority::default(),` from `Default for Config` |
| 106–107 | reword the `Machine` doc comment: strike `and which scheduling band it takes them in` so it reads `…which model it runs and how many threads it may take. A global file may set these and nothing else.` (also rewraps line 107, which is over-long today) |
| 121 | delete `priority: Option<crate::priority::Priority>,` from `struct Machine` |
| 166 | delete `layer(&named, "priority", machine.priority, &mut cfg.priority);` — note this line is indented four spaces where its neighbours are eight; deleting it removes the oddity |
| 192 | delete `cfg.priority = crate::priority::from_env(cfg.priority, std::env::var("REPOGRAPH_PRIORITY").ok().as_deref())?;` |
| 317 | delete `std::env::remove_var("REPOGRAPH_PRIORITY");` from `with_machine` |

Tests to delete (lines 441–487), four of them:
`priority_defaults_to_background_and_reads_from_the_project_file`,
`the_machine_file_may_set_priority_and_the_project_still_wins`,
`the_environment_beats_the_project_for_priority`,
`a_priority_that_is_not_a_word_it_knows_is_an_error_naming_the_file`.

> **Amendment 1.** Four of these seven edits are a **deletion in commit 1 and a replacement in
> commit 2**, not a deletion outright: line 106–107's reword must also name the level, line 121's
> `priority: Option<…>` field becomes `resources: Option<…>`, line 166's `layer(…, "priority", …)`
> becomes `layer(…, "resources", …)` (at the correct eight-space indent), and line 317's
> `remove_var("REPOGRAPH_PRIORITY")` becomes `remove_var("REPOGRAPH_RESOURCES")`. The exact text of
> each is in §F.2. Lines 47–55, 101 and 192 are pure deletions and stay as written above.

Leave alone: the `threads` doc comment at lines 38–46 (its citation of
`docs/bench/2026-09-07-resource-usage-results.md` is the harness's +39%-on-a-fixed-shape reading,
which this change does not touch) and `an_unknown_key_is_rejected` at line 253. **Amendment 1:**
lines 38–46 are left alone by commit 1 and edited by commit 2, where `0` stops meaning "a third of
the logical cores" and starts meaning "take `resources`' rule" (§F.2).

### A.4 `Cargo.toml` and `Cargo.lock`

- `Cargo.toml:17` — delete `libc = "=0.2.189"`. **`libc` is used by nothing else in the crate**:
  `rg -n "libc" src/` returns only `src/priority.rs` (verified 2026-09-09). There is no
  target-specific `libc` table to remove; the only `[target.'cfg(windows)'.dependencies]` block
  (lines 39–41, `socket2` and `windows-sys`) belongs to `serve` and stays untouched.
- `Cargo.lock` — the first `cargo build` drops `"libc"` from the `repograph` package's dependency
  list (line 1696 today). The `[[package]] name = "libc"` entry itself stays, because 27 other
  packages depend on it. Commit the lock with the code.

### A.5 Everything else: verified clean, no edit

`tests/serve.rs`, `tests/users_day.rs`, `tests/fixtures/`, `bench/**`, `scripts/`, `npm/**` and
`.github/workflows/**` contain no reference to the band —
`rg -n "REPOGRAPH_PRIORITY|priority" tests/ bench/ scripts/ npm/ .github/` returns nothing
(verified 2026-09-09). `repograph.toml` at the repository root never set `priority` or `threads`, so
the worked example needs no edit. There is no machine file on this box
(`~/.config/repograph/config.toml` absent, `REPOGRAPH_CONFIG` unset), so no local run breaks the
moment the key goes.

### A.6 The compatibility question, answered from the code

**What a user with `priority = "background"` in their file experiences after the change: a hard
error naming the file and the key, on every command that loads config.**

The evidence, not a guess: `src/config.rs:5-6` puts `#[serde(default, deny_unknown_fields)]` on
`Config`, and `src/config.rs:112-113` puts the same on `Machine`. `Config::load` wraps the parse
with `.with_context(|| format!("parse {}", path.display()))`, so the failure reads

```
Error: parse /path/to/repo/repograph.toml

Caused by:
    TOML parse error at line 1, column 1
      unknown field `priority`, expected one of `doc_globs`, `code_globs`, … `threads`
```

and the machine-file path (`Config::machine`) wraps its own parse the same way, so a global file
that still names the key fails **every repository on the machine** with that file's name in the
message. The commands affected are every one that calls `load_cfg()`: `build`, `update`, `enrich`,
`embed`, `ask`, `serve`, `watch`, `impact`, `trace`, `changes`. `explain` and `verify` never load
config and keep working.

**Decision: keep the refusal; do not add an ignored-and-warned key.** Three reasons, each from this
repository:

- The refusal is the project's stated position on exactly this case. `src/config.rs:198-200`:
  "A malformed or unknown-key global file is an error naming the file: a setting silently ignored on
  every repository at once is worse than a refusal." A `priority` that parsed and did nothing is the
  behaviour that comment exists to forbid.
- The blast radius is bounded and known. `git tag` lists `v0.1.0`, `v0.3.0`, `v0.4.0` and no
  `v0.5.0`; the key landed in `fec9496` on an untagged 0.5.0. ADR-002 already made this argument for
  a different default — "no released install is touched at all: the last tag is `v0.4.0` … the large
  default landed after it in a 0.5.0 that has never been tagged." Anyone who can hit this error is
  tracking `main`.
- The message names the file and the key, so the repair is deleting one line. An ignored key would
  instead leave someone believing their rebuild is still yielding the machine.

**The environment variable is the deliberate asymmetry, and the README must say so.**
`REPOGRAPH_PRIORITY=background repograph build` after the change runs normally and **silently**:
`Config::load` reads only the variables it knows and there is no allowlist of `REPOGRAPH_*` names
anywhere in the code, so an unknown one has never been an error and this one becomes unknown. Files
are refused by name; environment variables are ignored.

**One test replaces the four deleted ones** (in `src/config.rs`'s test module, in the same style):

```rust
/// The key was a setting until 2026-09-09 and is not one now. It is refused by name rather than
/// ignored, because a file that still asks for the band should say so out loud instead of leaving
/// someone believing their rebuild still yields the machine.
#[test]
fn a_priority_key_left_in_a_file_is_refused_by_name() { … }
```

It asserts, under `with_machine(None, …)`, that a project file holding `priority = "background"`
makes `Config::load` return `Err` whose message contains both `repograph.toml` and `priority`; and
under `with_machine(Some("priority = \"normal\"\n"), …)` that the machine file's own path and the
key are both named. That is bar A5.

---

## Part B — the documents

The rule applied per file: **a bench result document is a record of what was measured on a date and
is kept intact with a dated header note; the README is the live surface and is rewritten.**

### B.1 `README.md` — rewritten (the only file whose prose changes)

| line(s) today | edit |
|---|---|
| 523 | **Amendment 1** — the `threads` row is reworded, not left: §F.7 |
| 524 | ~~**delete** the `\| \`priority\` \| …` row~~ — **Amendment 1: replaced**, not deleted. A `resources` row takes its place: §F.7 |
| 545 | in the layer table's "the run" row, delete `, \`REPOGRAPH_PRIORITY\`` — **Amendment 1: and add `REPOGRAPH_RESOURCES`** |
| 556–558 | ~~the machine-file allowlist sentence loses one name~~ — **Amendment 1: it swaps one name**, `priority` out and `resources` in: "…`reranker_dir`, `threads` and `resources`, and refuses any other key by name." §F.7 |
| after 561 | **add**, as a new paragraph closing that subsection, the one migration sentence, which is the whole of the README's memory of the key: "`priority` was a key until 2026-09-09 and is not one now — the writers run in the normal band on every platform. A file that still names it is refused by name like any other unknown key; delete the line. `REPOGRAPH_PRIORITY` is simply no longer read." |
| 617–621 | in Embeddings, the clause "…and 1,930 s to embed the fixture's 33,525 rows against 214 s, **which the background band a writer runs in multiplies by about four again: a first build is hours under the large model where the default's is minutes. That last step is a ratio and not a measured wall — no whole-store run in the band has finished on this machine, which is G20.**" — the bolded part **dies**. What replaces it: the two walls stand on their own, the default's re-measured (C1) and the large model's dated, with no multiplier and no G20 reference. |
| 991–992 | keep the paragraph, replace `**214 s and 1.63 GB**` with C1's numbers and their date |
| 986–989 | keep the e5-large before/after table, and add to the sentence under it that both rows were taken at `786b994` on 2026-09-07 in the normal band — which is the only band there is now, so they stand |
| **994–1002** | **dies**: the "what the *person at the keyboard* feels … it is the one `priority` answers" paragraph and the whole `priority = "normal"` / `priority = "background"` compile-and-wake table |
| **1004–1010** | **dies**: "The price is wall time … `REPOGRAPH_PRIORITY=normal` for one run, is the word back … Readers ignore the setting entirely" |
| 1012–1017 | **keeps**: the mapped-weights paragraph is not the band |
| **1019–1028** | **dies**: "That measurement is this machine's — Apple Silicon…" and the four-row `what \`priority = "background"\` does there` platform table (macOS Apple Silicon / macOS Intel / Linux / Windows) |
| 1030–1034 | rewrite: keep the `threads` sentence and the container note; the closing sentence loses its band half, becoming "A build server with nobody at the keyboard wants `threads` as high as it likes, one line in `~/.config/repograph/config.toml`." |
| 1036–1040 | edit: "On a machine with 8 GB the model is the lever ~~and the scheduler is not~~"; update `the whole store is 214 s` to C1 |
| 1042–1057 | **keeps** unchanged (the two bounds, the progress line, "give the cores back") |
| 1059–1064 | edit: `writes the whole store in 214 s and 1.6 GB` becomes C1's wall and RSS |

**What replaces 994–1028** — one short paragraph, in the section's own voice, saying: the writers run
in the same band as everything else, there is no scheduling setting, and what a rebuild costs the
person beside it is now bounded by `threads` and the token budget alone; the band that used to be
the default was removed on 2026-09-09 because its price — four times the wall, and a rebuild that
starves outright on a busy laptop — was paid by everyone to buy something only a person sitting at a
loaded machine collects, with a pointer to
[the unnoticeable results](docs/bench/2026-09-07-unnoticeable-results.md) for what it measured while
it existed. **No number from the deleted tables is carried forward into the README**: they are the
band's, and the band is gone.

> **Amendment 1.** That paragraph is still written, and the **three-level table of §F.7 goes under
> it** — the section that dies with the band is replaced by the levels, not by prose alone. Lines
> 1030–1034, 1042–1048 and 1050–1051 also change; §F.7 is the full list.

### B.2 `docs/bench/2026-09-07-unnoticeable-results.md` — kept whole, one header note

This document *is* the band's measurement; rewriting it would delete a day of readings that were
correctly taken. Insert, immediately under the H1 and before "The request was one sentence", a
blockquote:

> **The band this document measured was removed on 2026-09-09** (see
> [the plan](../plans/2026-09-09-normal-band-only.md)). Every number below is what it read on
> 2026-09-07 and none of them has been re-taken; the lever they describe no longer exists, and
> `priority` is not a setting. The reason for the removal is in the plan; the numbers here are why
> it was worth trying.

Nothing else in the file changes.

### B.3 `docs/bench/2026-09-07-resource-usage-results.md` — kept whole, one header note

Its §1.1 row `embed-A-small-full` (214.1 s / 1.63 GB / 399% / 18 threads) is the figure that has
been travelling into the README as though it were current. Insert under the H1:

> **Two notes added 2026-09-09.** §1.1's `embed-A-small-full` row (214.1 s, 1.63 GB) is a
> **before** row — pre-cap, pre-budget, pre-chunked-sync — and was being quoted elsewhere as the
> default model's current cost; the current one is in
> [the 2026-09-09 results](2026-09-09-normal-band-only-results.md). Everything else here was
> measured in the normal band, which is again the only band there is: the background band shipped
> after this document and was removed on 2026-09-09.

Nothing else changes: §4's after rows were taken at `786b994`, before the band existed, so they
describe the shipped behaviour again.

### B.4 `docs/plans/2026-09-07-unnoticeable.md` — kept, one header note

A plan is a record of what was decided on a date. Insert under the H1:

> **Superseded 2026-09-09.** The band this plan designed and shipped was removed; `libc`,
> `src/priority.rs`, the `priority` key and `REPOGRAPH_PRIORITY` are gone with it. The measurement
> method here still stands and is reused by
> [the removal plan](2026-09-09-normal-band-only.md).

### B.5 `docs/plans/2026-09-06-resource-usage.md` — **no edit**

Verified: `rg -n "priority" docs/plans/2026-09-06-resource-usage.md` returns nothing. It designed the
thread cap, the token budget and the chunked sync — all of which stay — and its numbers are
foreground numbers. Named here so the build phase does not go looking.

### B.6 `docs/adr/ADR-002-two-defaults-multiplied.md` — kept, one amendment appended

The decision (small model as the default) **stands**; one of the two arms it stands on does not.
ADR-001 carries nine amendments in this repository and that is the convention to follow: do not edit
the Context or the Decision, append a section and update the Status line.

- Status line (3–5) gains: `; amended 2026-09-09 (Amendment 1)`.
- Append `## Amendment 1 — the multiplier is gone, the decision is not (2026-09-09)`, saying: the
  band was removed, so "two defaults multiplied" is now one default with a price of its own; the
  large model's whole-store embed is 1,930 s foreground against the default's C1 seconds, and the
  2.1 GB download against 470 MB is untouched — which is still an asymmetry of the kind the decision
  rests on, so nothing here reverses it. The figures that **do** die with the band and must be read
  as historical: the 4.1–4.5× ratio, the "2.2 to 2.4 hours" first build, and the paragraph beginning
  "In the same unreleased version `priority = \"background\"` became the default for every writer".
  Say plainly that if the large model's price is ever re-weighed it is now weighed in foreground
  seconds only.

### B.7 `docs/bench/next-version-gaps.md` — two gaps closed, two paragraphs annotated

Follow the file's own closure convention (`## G7 · … — closed 2026-09-05`, and `~~strikethrough~~`
in the order tables).

| where | edit |
|---|---|
| line 866, `## G20 · A whole-store rebuild in the background band has no wall number` | retitle `— closed 2026-09-09, moot`; add one paragraph under the heading: the band was removed, so there is no run to take and no ratio to replace with CPU-seconds. The body stays as the record of why it could not be measured. |
| line 912, `## G21 · One machine is measured; three platform rows are reasoned` | same treatment: the three reasoned rows described the band's effect per platform, and there is no per-platform behaviour left to reason about. |
| line 404, G11's "**Update (2026-09-07): the option stands, the default does not**" paragraph | append one dated sentence: the band half of this argument was removed on 2026-09-09; the 1,930 s against C1 seconds and the 2.1 GB against 470 MB are what the default now rests on. |
| lines 1145–1146 area, the cost-family "Suggested order" table | strike through the **G20** row (rank 2) and the **G21** row (rank 5) with `~~…~~` and a `closed 2026-09-09 — the band was removed` reason, exactly as the G7/G8/G12 rows read |
| line ~1208, "**The scheduling levers that are not the background band**" bullet under "What is explicitly not on this list" | append: as of 2026-09-09 the band itself is not on the list either — it shipped, was measured, and was removed; the family is closed rather than deferred |

### B.8 `docs/bench/2026-09-09-normal-band-only-results.md` — **new**, written by the build phase

The retest's own document, in the shape of `2026-09-07-resource-usage-results.md`: machine and
corpus line, the method paragraph naming `$G`, one writers table, one readers table, the checkpoint
transcript, the floors block, and a §What changed and what did not. Every row names its transcript
under `$G/log` and the line in `$G/log/summary.txt` it was read from. It is what the README's new
numbers cite.

---

## Part C — the retest

### C.0 Method, reused exactly

`/usr/bin/time -l` for wall, user and max RSS; a one-second `top -l 2 -s 1 -pid … -stats pid,cpu,mem,th`
sampler for peak CPU% (100% = one core) and thread count; one `.time`, `.samples` and `.out` per run;
a `summary.txt` carrying the line each table row was read from. That is `$P/measure.sh`, copied
**byte for byte** — do not rewrite it, or the rows stop being comparable to 2026-09-07's.

Setup (one shell, ~1 minute, mostly the 864 MB copies):

```bash
export W=/Users/max/Documents/projects/repograph/.claude/worktrees/routing-effort-model-skill-50259a
export B=$W/target/release/repograph
export P=/Users/max/bench/resources-2026-09-07
export G=/Users/max/bench/resources-2026-09-09
export F=/Users/max/bench/beauty-crm-502e8a6d          # pinned, read-only, never rebuilt
mkdir -p $G/log
cp $P/measure.sh $P/queries10.jsonl $G/
cp -a $F $G/fixture                                     # readers
cp -a $F $G/writer                                      # build/update rows (they mutate the store)
mkdir -p $G/store-E && cp -a $F/.repograph $G/store-E/.repograph   # the full-embed store
rm -f $G/store-E/.repograph/vectors.*
for d in fixture writer store-E; do
  printf 'embed_model = "intfloat/multilingual-e5-small"\n' > $G/$d/repograph.toml
done
cargo build --release --manifest-path $W/Cargo.toml
uptime | tee -a $G/log/summary.txt
```

The fixture's store is 33,533 rows at dim 384 with no recorded model — an e5-small store by history
(verified 2026-09-09), 33,525 of them live, so a full re-embed writes 33,525. `embed` needs only
`.repograph/`, which is why `store-E` carries no source tree.

> **Amendment 1.** The single `store-E` becomes three, one per level — a level's row re-embeds the
> whole store and cannot share a copy with another level's. §F.5 carries the replacement setup
> block. Everything else in C.0, `measure.sh` above all, is unchanged and must stay unchanged.

### C.1 The long rows (run first, one at a time, on a quiet machine)

> **Amendment 1.** R1 is run **three times, once per level**; the block below is the `balanced` one
> and stays exactly as written, with `store-E` renamed `store-E-balanced` and no `REPOGRAPH_RESOURCES`
> in its environment at all — that absence is what proves the default is byte-for-byte today's
> behaviour. §F.5 carries all three. R2 and everything in C.2 stay at one level.

**R1 — the headline, ~150–185 s expected, bar ≤ 214.1 s.** The row that replaces the stale
214 s / 1.63 GB figure everywhere it is quoted:

```bash
export REPOGRAPH_NO_SERVE=1
uptime >> $G/log/summary.txt
$G/measure.sh embed-small-full $G/log -- $B --repo $G/store-E embed
uptime >> $G/log/summary.txt
```

Also record from `$G/log/embed-small-full.out`: the 33 progress lines' cadence (first line's
timestamp and the largest gap) — G19's bar is a separate gap and is not re-litigated here, but the
line is free and the doc should carry it.

**R2 — the checkpoint resume, ~60 s + ~110 s.** `$P/t3.sh` with the paths repointed (`G` and `B`
above, `store-T3` → `$G/store-T3`), unchanged otherwise, including `kill -TERM` and its comment:

```bash
mkdir -p $G/store-T3 && cp -a $F/.repograph $G/store-T3/.repograph
rm -f $G/store-T3/.repograph/vectors.*
printf 'embed_model = "intfloat/multilingual-e5-small"\n' > $G/store-T3/repograph.toml
# start `embed`, wait for two `rows,` lines on stderr, kill -TERM, read vectors.json's row count,
# run `embed` again and read its total — bar C6 is that the second total is exactly the remainder.
$B --repo $G/store-T3 ask "штраф за отмену записи"     # the five-id check, after the second run
```

### C.2 The quick rows (a shell loop, ~2 minutes total)

**Writers, on `$G/writer`, in this order** (the first needs vectors present, the second wipes the
graph, the third finds nothing changed):

```bash
$G/measure.sh build-embeds-nothing $G/log -- $B --repo $G/writer build          # ≈ 1.6 s
$G/measure.sh build-nodense        $G/log -- $B --repo $G/writer --no-dense build   # ≈ 1.9 s
$G/measure.sh update-nodense       $G/log -- $B --repo $G/writer --no-dense update  # ≈ 0.07 s
```

`--no-dense` is a global flag and goes **before** the subcommand.

**Readers** — `$P/quick.sh` copied to `$G/quick.sh` with three lines repointed (`S=$G`, `B=$B` from
above, `L=$G/log`) and nothing else touched. It runs, in order and serially:

| run | command | expected wall | expected max RSS |
|---|---|---|---|
| ask-fused | `ask "как отменить запись и кто платит штраф"` | 0.35 s | 1.36 GB |
| ask-nodense | `--no-dense ask …` | 0.12 s | 0.10 GB |
| ask-stale | `ask --stale …` | 0.28 s | 1.36 GB |
| ask-rerank-local | `ask --rerank-local …` | **31.9 s** | 3.09 GB |
| ask-exact-id | `ask FR-PAY-22` | 0.04 s | 0.05 GB |
| impact | `impact cn --depth 3` | 0.04 s | 0.05 GB |
| trace | `trace main cn --depth 6` | 0.04 s | 0.05 GB |
| changes | `changes --depth 2` over a one-hunk edit | 0.07 s | 0.07 GB |
| bench-nodense | `--no-dense bench` | 0.19 s | 0.20 GB |
| bench-dense | `bench` | 1.47 s | 1.42 GB |
| dump10 | `dump --queries queries10.jsonl --depth 300` | 0.70 s | 1.36 GB |
| import-legacy | the 41 MB graphify graph from `$P` | 0.14 s | 0.15 GB |
| serve | resident, two fused asks through the socket, then sampled idle | — | 1.38 GB |
| watch | `--every 2`, quiet poll then one doc edit | — | 0.05 / 1.37 GB |

`--rerank-local` needs `~/.cache/repograph/reranker` — present on this machine (verified
2026-09-09: `model.onnx`, `model.onnx_data`, `tokenizer.json`). `serve` needs the store under a
short path so the socket clears macOS's 104-byte `SUN_LEN`; `$G/fixture/.repograph/serve.sock` is
67 bytes and clears it (G27 is the gap where it does not).

### C.3 What is **not** re-measured, and why it is said out loud

The e5-large whole-store row (1,930 s / 2.15 GB) is **not** re-run: 32 minutes for a row whose model
is not the default, and it was taken at `786b994` in the normal band — which is the only band there
is again, so it describes today's behaviour. The one caveat the results doc must state: `786b994`
predates the mapped-weights change that shipped in the same PR as the band, which the README prices
at +6% to +26% of wall under memory pressure and free otherwise (G22). If Max wants that row current,
it is one detached run of
`$G/measure.sh embed-large-full $G/log -- $B --repo <copy pinned to e5-large> embed`, expected
1,800–2,000 s, and it blocks nothing in this plan.

---

## Part D — the floors

```bash
ARMS="dense lexical" NOTE="levels only" bench/history/run-repograph.sh
```

from `$W` (the script resolves `REPO` from its own path, builds `--release`, refuses a fixture that
is not at `502e8a6d` or has edited sources, and appends both rows to `bench/history/runs.jsonl`).

Expected, and bar C7 — identical to the `502e8a6d` rows already in the history:

```
keyword 40/40  paraphrase 15/30  code 12/12  p90 221 tok  dense=true   enriched=true  model=small  gated=true
keyword 39/40  paraphrase 15/30  code 12/12  p90 215 tok  dense=false  enriched=true  model=small  gated=true
```

**This is a guard, not a hypothesis.** Removing a scheduling call cannot move retrieval: no ranking,
no index, no gate and no store format is touched by any edit in Part A, and the fixture's rows are
not re-embedded by this run at all. It is run because a floor that is only asserted is not a floor,
and the two new rows enter the history with a note that says what they were run against.

The two appended rows in `bench/history/runs.jsonl` are committed with the docs.

---

## Part E — the commit and PR shape

> **Amendment 1: three commits, not two.** A second commit goes between these two, and the reasoning
> is in §F.8. The numbering below is unchanged; the new one is commit 2 and the docs commit becomes
> commit 3.

**Two commits**, in this order, because the second cannot be written until the first has produced a
binary and the retest has produced numbers:

1. `perf: the background band is removed, and every writer runs in the normal one`
   — `src/priority.rs` (deleted), `src/main.rs`, `src/config.rs`, `Cargo.toml`, `Cargo.lock`.
   Body: what a leftover `priority` key does now (refused by name), that `REPOGRAPH_PRIORITY` is
   ignored, that `threads` is unchanged, and that no released version ever carried the key.
2. `docs: the band's documents keep their date, and the default model's numbers are today's`
   — `README.md`, the four header notes, ADR-002's amendment, `docs/bench/next-version-gaps.md`,
   the new `docs/bench/2026-09-09-normal-band-only-results.md`, this plan, and the two
   `bench/history/runs.jsonl` rows.

Both carry the co-author trailer this session is required to add and nothing else — no other
trailer, no ticket tag, no session link. Heredoc and `git commit` in two separate shell commands. CI runs on both (three platforms, `clippy -D warnings` and `cargo test`); do not add
`[skip ci]` — this repository's pipeline is GitHub Actions and the convention that skips docs builds
is another repository's.

PR: one, titled as commit 1's subject, body = Summary / What was removed / What a leftover key does /
The numbers, with the C1 row and the two floor lines quoted, and the sentence that the readers are a
guard because they never called the removed function.

---

## Part F — Amendment 1: the `resources` level

Added by Max on 2026-09-09, after Parts A–E were written and before any of them ran. The band still
goes first and entirely; this is what takes its place on the surface, and it is a **different lever**
— the band moved the scheduling class, this moves a thread count.

### F.1 The shape, and what it does not touch

One key, `resources`, taking three names. The trio stays `low` / `balanced` / `full`: they name
amounts of the machine, which is what the key is called, and no better three exist —
`quiet`/`default`/`all` puts a layering word (`default`) in a value's place, and `gentle`/`full`
leaves the middle unnamed.

**Reach: thread count only.** `TOKEN_BUDGET` (2,048 padded tokens), `BATCH` (64) and `SYNC_CHUNK`
(1,024 rows) are **not** read by the level and are not touched by this amendment. One axis, and it
is the axis §2 of [the resource-usage results](../bench/2026-09-07-resource-usage-results.md) proved
was a straight trade with no free side; the budget is free (reading 1 there) and so has no level
worth offering, and the checkpoint chunk is a recovery granularity rather than a resource.

| level | rule | this machine (12 logical cores) | the row it comes from |
|---|---|---|---|
| `"full"` | **no cap at all** | ORT's own 6 intra-op + rayon's own 12 = **18 threads, 6 running, ~400%** | §1.1 `embed-A-small-full` and §4.2's `embed-B-large-320longest` before-row (18 / 6 / 401.6%) |
| `"balanced"` *(default)* | `(cores / 3).max(1)` = **4** | 8 threads, 4 running, 293% | §2 **H1** (threads 4: 148.7 s, +39%, 294%) and §4.1's after-row (8 / 4 / 293.3%) |
| `"low"` | `(cores / 6).max(1)` = **2** | 4 threads, 2 running, ~148% | §2 **H3** (threads 2: 292.0 s, +173%, 152%), confirmed in the binary by §4.2's two `threads = 2` rows (268.8 s and 269.2 s, 4 / 2, 148.1% and 148.5%) |

Each step down halves the count, which is the whole rule and fits in the README's own sentence.
On ≤ 4 logical cores `low` and `balanced` both floor to 1 and the level stops meaning anything —
correct, and worth one clause in the doc comment rather than a special case in the code.

**`balanced` is byte-for-byte today's behaviour.** `(cores / 3).max(1)` is the existing
`threads_from` body, unmoved and unedited; a config file with no `resources` key resolves to it
through `#[serde(default)]`, so the C1 row must land inside the bar that was written for HEAD.

### F.2 Why `full` is *no cap*, and why that is not the fastest setting

> **Struck by Amendment 2, and settled by measurement.** The argument below was written while
> `threads = N` still existed. It does not, so the shape it calls "the fastest setting" would have
> been unreachable by anyone. Both candidates were measured on 2026-09-09: no cap is 178.1 s at
> 18 / 6, `(cores / 2).max(1)` is 168.8 s at 12 / 6, on the same 814 user seconds, and the capped
> one ships. The zero-sentinel reasoning below was still checked on the way and held exactly as
> predicted — the no-cap run sampled 18 threads with 6 running — so the `Option<usize>` fallback it
> names was never needed. Everything below stands as the record of the reasoning that the
> measurement replaced.

The requirement is "reproduce the pre-cap behaviour". Pre-cap is a **code path** — neither
`cap_pools` nor `with_intra_threads` was called — and it is a **measured row**: 18 threads, 6
running, ~400%. `full` is that, and the plan says so rather than approximating it:

- **Rejected: cap at `available_parallelism`.** That is 12 on this machine, which would put ORT's
  intra-op pool at 12 against the six performance cores it would have picked itself, and rayon at
  12 beside it: 24 threads. No row in this repository measures that shape, it oversubscribes the
  cluster that does the work, and it is **not** the pre-cap shape either — pre-cap is 18, not 24.
- **`threads = 6` is a third shape and must not be confused with either.** §4.2 reads it at
  **12 threads / 6 running / 434.3% / 103.4 s**, against the uncapped before-row's
  **18 threads / 6 running / 401.6% / 110.8 s**. Both run six threads; they are not the same
  process. Capping to 6 takes rayon's global pool from twelve to six while ORT keeps the six
  performance cores it would have chosen anyway — so `threads = 6` is *fewer* threads than no cap,
  and on that row it is also **faster** (103.4 s against 110.8 s), because twelve tokenizer threads
  contending for six cores lose more than they add.

The consequence has to be stated in the plan and again in the README, or the table lies:
**`full` is the pre-cap level, not the fastest level.** The fastest setting on this machine is
`threads = 6`, and it is reachable only through the escape hatch, which is one reason the escape
hatch keeps its own sentence (§F.7).

**How "no cap" is expressed: `threads_from` returns `0`, and `0` is the documented default on both
pools** — rayon's `ThreadPoolBuilder::num_threads(0)` is "select automatically", and ORT's
`SetIntraOpNumThreads(0)` is "ORT picks". No signature changes past the one argument in §F.3, and no
`Option<usize>` threaded through `cap_pools`, `session_builder`, `load_session`, `CrossEncoder::open`,
`open_embedder` and `warm_model`.

**That claim is verified, not asserted.** The proof is free and is already in the retest: C1b's
sampled thread count. **16–20 threads with 6 running** means the zero reached both pools as
"size yourself". **8 / 4** means it was read as `balanced`'s four and the sentinel is wrong; **2 / 1**
or **1 / 1** means one of the two pools read zero as *zero*. If either happens, the fallback — named
here so the build phase does not improvise — is to make the resolved value `Option<usize>` with
`None` meaning "call neither builder method", which is literally the pre-cap code, at the price of
the six signatures listed above. Do not ship `full` on an unverified sentinel.

### F.3 The code, commit 2

`Resources` lives in **`src/index/embed.rs`**, beside `threads_from`, and `Config` names it as
`crate::index::embed::Resources` — the same shape `priority` had (`crate::priority::Priority` in the
struct, the rule in the module that uses it), and it keeps the number rule next to the function that
already computes one.

```rust
/// How much of the machine a writer may take. `balanced` is the built-in and is a third of the
/// logical cores; `low` is half of that again, for a laptop someone is working on; `full` caps
/// nothing at all and is the shape this binary had before the cap existed — 18 threads and six
/// running here, not the twelve of `threads = 6`, which caps rayon too and is on this machine
/// the faster of the two (docs/bench/2026-09-07-resource-usage-results.md §4.2). An explicit
/// `threads` outranks this key; on four logical cores or fewer both named levels floor to one
/// and the key stops meaning anything.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Resources { Low, #[default] Balanced, Full }
```

```rust
fn threads_from(configured: usize, level: Resources, cores: usize) -> usize {
    if configured > 0 { return configured; }
    match level {
        Resources::Full => 0,                    // reaches both pools as "size yourself"
        Resources::Balanced => (cores / 3).max(1),
        Resources::Low => (cores / 6).max(1),
    }
}
```

and `resources_from_env(configured, var) -> Result<Resources>` in the same file, the deleted
`priority::from_env` one file over: unset or empty keeps what the files said, an unknown word is an
error naming `REPOGRAPH_RESOURCES`.

`index::embed::threads` gains the level as a second argument. **Twelve call sites**, all mechanical:

| file | lines |
|---|---|
| `src/main.rs` | 278, 340, and the six `cap_pools(…)` arms — 404, 412, 425, 447, 475, 481 (numbers as at `fec9496`; commit 1 removes four `priority::apply` lines above them and shifts these) |
| `src/ask.rs` | 185, 235, 260 |
| `src/bench.rs` | 230 |
| `src/dump.rs` | 58 |
| `src/index/cross.rs` | 176 (a test: `threads(0)` becomes `threads(0, Resources::default())`) |

Before committing, `rg -n "embed::threads\(" src/` must return nothing with a single argument, and
`rg -n "threads" src/` must show no arithmetic performed on the returned value — a `0` that means
"uncapped" would be a division by zero or an empty batch anywhere it were treated as a count.

`src/config.rs` in commit 2:

| where | edit |
|---|---|
| after the `threads` field | add `pub resources: crate::index::embed::Resources,` with the doc comment above |
| the `threads` doc comment (38–46 at `fec9496`) | `0` stops meaning "a third of the logical cores" and starts meaning "take `resources`' rule"; the §4.2 citation stays |
| `Default for Config` | `resources: crate::index::embed::Resources::default(),` where `priority:` was |
| the `Machine` doc comment (106–107) | commit 1's reword gains the level: "…which model it runs, how many threads it may take and how much of itself it offers." |
| `struct Machine` | `resources: Option<crate::index::embed::Resources>,` where `priority:` was |
| the `layer` block | `layer(&named, "resources", machine.resources, &mut cfg.resources);` at **eight** spaces, beside `threads` |
| the env block | `cfg.resources = crate::index::embed::resources_from_env(cfg.resources, std::env::var("REPOGRAPH_RESOURCES").ok().as_deref())?;` |
| `with_machine` | `remove_var("REPOGRAPH_RESOURCES")` where `REPOGRAPH_PRIORITY`'s was |
| after the env block | the same-place warning, §F.4 |

### F.4 Precedence, as a rule and as a test

> **Struck by Amendment 2.** With `threads` removed there is no second key: no cross-key precedence,
> no both-set warning, no `threads`-wins corner. What survives from this section and was built: the
> layering itself (built-in default → machine file → `repograph.toml` → `REPOGRAPH_RESOURCES`), the
> machine-file allowlist argument — `resources` describes the machine and belongs in `Machine` where
> `threads` was — and the compatibility asymmetry, which now runs in both directions at once:
> `default` is why adding `resources` breaks nothing, `deny_unknown_fields` is why a file naming
> either removed key is a hard error by name.

**The rule, in two sentences.** Each key resolves through the layers it already has — built-in
default, then the machine file, then `repograph.toml`, then the environment — with
`REPOGRAPH_RESOURCES` sitting beside `REPOGRAPH_THREADS` exactly as the existing keys layer. The two
are then combined at one point, `threads_from`: **a `threads` that is not `0` wins, and otherwise
the level's rule applies.**

No new sentinel is invented for "I did not choose": `threads`'s built-in default is already `0` and
already means it. So the order is **explicit `threads` (from any layer) > `resources` (from any
layer) > the built-in `balanced`**.

**The corner this creates, which the README must name.** `REPOGRAPH_RESOURCES=low` does *not*
override a project file's `threads = 6`: the environment beats the files *for its own key*, and
`threads` beats `resources` *across keys*. The way to be gentle for one run on a repository that
pins a count is `REPOGRAPH_THREADS=2`. This is deliberate — a rule that can be stated in one
sentence is a rule people can predict, and the alternative needs per-key provenance tracked across
three layers to answer one uncommon question.

**Both keys named in the same place: `threads` runs, and it says so.** A warning on stderr, once,
naming the place and both keys — not silence, and not an error.

- Not silence, because `src/config.rs:198-200` is this repository's stated position on exactly this:
  "a setting silently ignored … is worse than a refusal". A `resources` that parsed and did nothing
  is what that comment exists to forbid.
- Not an error, because the pair is redundant rather than dangerous, and a hard failure on a
  machine file would take out every repository on the box for a line that is merely superfluous.
- **Only within one place**, never across layers: `resources = "low"` in
  `~/.config/repograph/config.toml` with `threads = 6` in one repository's `repograph.toml` is the
  design's best case — the machine sets a level, one repository buys the cores back — and a warning
  that fired there would fire on the thing the layering is for. Within one file the author wrote two
  answers to one question in one place and only one is read; that is a mistake and is said out loud.

Implemented as a pure helper returning `Option<String>` so it is testable without a notices channel
on `Config`, with `Config::load` doing the `eprintln!`. The three places are the machine file
(`machine.threads.is_some() && machine.resources.is_some()`), the project file
(`named.contains_key("threads") && named.contains_key("resources")`) and the environment (both
variables set and non-empty).

**The machine-file allowlist.** Verified against `src/config.rs` on 2026-09-09: `struct Machine`
carries `enrich_command`, `rerank_command`, `enrich_model`, `rerank_model`, `reranker_dir`,
`threads` and `priority`. Both `threads` and `resources` describe the machine and not the corpus —
the same repository wants every core on a CI box and a quiet laptop's spare ones — so `resources`
belongs in `Machine` beside `threads`, unlike `embed_model`, whose value is a property of the
vectors on disk and is deliberately unreadable from there.

**The compatibility asymmetry, one mechanism and two directions.** Both structs carry
`#[serde(default, deny_unknown_fields)]`. `default` is why **adding `resources` breaks nothing**: a
file that does not name it parses exactly as it does today and resolves to `balanced`, which is
HEAD's behaviour. `deny_unknown_fields` is why **removing `priority` is a hard parse error**: a file
that still names it fails every command that loads config, by name (§A.6). The same two attributes,
opposite directions, and the README's migration paragraph says both in a sentence each.

### F.5 The retest, three levels

> **Amendment 2: four rows, not three.** `full` costs two — the no-cap candidate and the capped one —
> and the level is passed only through `REPOGRAPH_RESOURCES`, never written into a copy's
> `repograph.toml`. `balanced` was additionally run three times and its median published, because it
> missed its bar. What was measured is in
> [the results](../bench/2026-09-09-normal-band-only-results.md).

Each level re-embeds the whole store and so needs its own pristine copy. The setup block in C.0
replaces its single `store-E` with three:

```bash
for lvl in balanced full low; do
  mkdir -p $G/store-E-$lvl && cp -a $F/.repograph $G/store-E-$lvl/.repograph
  rm -f $G/store-E-$lvl/.repograph/vectors.*
  printf 'embed_model = "intfloat/multilingual-e5-small"\n' > $G/store-E-$lvl/repograph.toml
done
```

The level is passed in the **environment**, never written into the copy's `repograph.toml`, so the
three files stay byte-identical and the runs differ by exactly one word. §4.2 already established
that the variable and the file read the same to the second, so nothing is given up by choosing the
one that is a better control.

```bash
export REPOGRAPH_NO_SERVE=1
unset REPOGRAPH_RESOURCES REPOGRAPH_THREADS

# R1a — balanced. No REPOGRAPH_RESOURCES in the environment at all: the absence is the point.
uptime >> $G/log/summary.txt
$G/measure.sh embed-small-full-balanced $G/log -- $B --repo $G/store-E-balanced embed
uptime >> $G/log/summary.txt

# R1b — full
uptime >> $G/log/summary.txt
REPOGRAPH_RESOURCES=full $G/measure.sh embed-small-full-full $G/log -- $B --repo $G/store-E-full embed
uptime >> $G/log/summary.txt

# R1c — low
uptime >> $G/log/summary.txt
REPOGRAPH_RESOURCES=low $G/measure.sh embed-small-full-low $G/log -- $B --repo $G/store-E-low embed
uptime >> $G/log/summary.txt
```

**Expected wall, so the build phase knows what it is waiting for.** Two independent ratios are
available for each and they are quoted together rather than averaged:

| run | ratio to balanced | where the ratio comes from | expected wall |
|---|---|---|---|
| R1a `balanced` | 1.00 | C1 as written | **150–185 s** |
| R1b `full` | 0.72 – 0.83 | H0/H1 = 106.9/148.7 = 0.72; §4.2's uncapped 110.8 s against the shipped default's 133.3 s = 0.83 | **115–155 s** |
| R1c `low` | 1.96 – 2.02 | H3/H1 = 292.0/148.7 = 1.96; §4.2's `threads = 2` 268.8 s against 133.3 s = 2.02 | **290–390 s** |

The two ratios for `full` disagree by more than the two for `low`, and the band is deliberately wide
because of it: the H rows are the harness on the 320 longest passages under the large model, which
is all 256-token texts and the worst case for a budget that wins by not padding, while the whole
store's mix is what R1a actually measures. A row outside its band is re-run under G23's rule (three
runs, median, with the load average) before it is called anything.

**Total wall of the whole retest, serial, on a quiet machine:**

| | |
|---|---|
| the three embeds (R1a + R1b + R1c) | ≈ 10.5 min (bounded 555–730 s) |
| R2, the checkpoint resume | ≈ 3 min |
| C.2's three writer rows | ≈ 4 s |
| `quick.sh`, the readers | ≈ 2 min (`--rerank-local`'s 32 s dominates; `serve` and `watch` dwell) |
| Part D, the floors | ≈ 3 min (the script rebuilds `--release` first) |
| three 864 MB store copies and the build | ≈ 3 min |
| **total** | **≈ 22 min**, against ≈ 13 for the plan as written |

**R2 and the readers run at `balanced` only.** Not an economy — a level is one number reaching the
same two pools `threads` already reached, `cap_pools` and `with_intra_threads`, and there is no
per-command code path in it. `Cmd::Ask` and `Cmd::Serve` call `cap_pools(index::embed::threads(…))`
today and call the same function with one more argument afterwards; running the readers three times
would buy three copies of "a 0.35 s fused ask did not notice its rayon pool size". C5 is a guard
against the *removal* and stays exactly that. C6 likewise asserts that the second run's total is the
exact remainder, which is `sync_chunked`'s row arithmetic and not a thread count.

**The unmeasured corner, said out loud in B.8 rather than papered over:** `ask --rerank-local` under
`resources = "full"` opens the 2.1 GB cross-encoder uncapped, and that is the one reader a level can
move. It is not measured here. The level was designed for the writers, every other reader is under
a second, and this is recorded as a known gap rather than covered by a bar nobody ran.

### F.6 Acceptance bars, one per level, fixed before anything runs

| # | row | bar | expectation |
|---|---|---|---|
| C1a | R1a, `balanced` | **C1 unchanged** — wall ≤ 214.1 s, max RSS ≤ 1.63 GB, peak CPU ≤ 310% (avg ≤ 300%), threads ≤ 9 | 150–185 s, ≈ 293%, 8 / 4 |
| C1b | R1b, `full` | **threads 16–20 with 6 running** (this is §F.2's sentinel proof and the bar that matters most), peak CPU ≥ **380%**, wall ≤ **0.90 ×** R1a's measured wall, max RSS ≤ **1.80 GB** | ≈ 400%, 18 / 6, 115–155 s |
| C1c | R1c, `low` | peak CPU ≤ **175%** (avg ≤ 165%) **and** ≤ **0.65 ×** R1a's measured peak, threads ≤ **5** with 2 running, wall between **1.7 ×** and **2.3 ×** R1a's, max RSS ≤ **1.63 GB** | ≈ 148%, 4 / 2, 290–390 s |

The `0.65 ×` clause on C1c is the one that makes `low` a level rather than a number: 152/294 = 0.52
in H3, so the bar has room, and a `low` that came back at 0.9 × balanced would be a name for nothing.
C1b's RSS bar is 1.80 GB rather than C1a's 1.63 because rayon's twelve tokenizer encoders hold more
buffers than four do; the weights are the same either way.

Plus, on the code:

| # | bar |
|---|---|
| A6 | **Amendment 2:** `threads_from(level, cores)` takes no configured count. Pinned by table: `(Full, 12) == 6`, `(Balanced, 12) == 4`, `(Low, 12) == 2`, `(Balanced, 8) == 2`, `(Low, 8) == 1`, `(Full, 4) == 2`, `(Balanced, 4) == 1`, `(Low, 4) == 1`, `(Full, 0) == 1`, `(Balanced, 0) == 1` |
| A7 | **Amendment 2:** five `src/config.rs` tests green, the fifth changed. Four as listed — `resources_defaults_to_balanced_and_reads_from_the_project_file`, `the_machine_file_may_set_resources_and_the_project_still_wins`, `the_environment_beats_the_project_for_resources`, `a_resources_level_that_is_not_a_word_it_knows_is_an_error_naming_the_file` — and, in place of the struck precedence test, `a_threads_key_left_in_a_file_is_refused_by_name`, the twin of commit 1's `priority` test over both the project file and the machine file. Plus `an_unknown_level_in_the_environment_is_an_error_naming_the_variable` in `src/index/embed.rs`. |
| A8 | a `repograph.toml` with no `resources` key resolves to 4 threads on this machine, identical to `fec9496` — the byte-for-byte claim, asserted rather than assumed |

### F.7 The README, named lines

On top of B.1's list, and replacing four of its rows:

| line(s) at `fec9496` | edit |
|---|---|
| **523** | the `threads` row is reworded: "`0` = take `resources`' rule; any other count wins over the level" |
| **524** | the `priority` row is **replaced**, not deleted: `` `resources` `` \| `"balanced"` (the default) = a third of the logical cores, `"low"` a sixth, `"full"` no cap at all — see [Resources](#resources) |
| **545** | `REPOGRAPH_PRIORITY` out, **`REPOGRAPH_RESOURCES` in**, beside `REPOGRAPH_THREADS` |
| **556–557** | the allowlist **swaps** a name: "…`reranker_dir`, `threads` and `resources`, and refuses any other key by name." |
| **after 561** | B.1's migration paragraph gains §F.4's asymmetry, a sentence each: adding `resources` breaks no existing file because both structs are `#[serde(default)]`; removing `priority` fails every command that loads a file naming it because they are also `deny_unknown_fields` |
| **994–1028** | B.1 kills these; what replaces them is B.1's paragraph **and the three-level table below it** |
| **1030–1034** | B.1's build-server rewrite reads `resources = "full"` *or* `threads` as high as it likes |
| **1042–1048** | "Two things bound it" keeps both bounds and gains one clause: the level is the first of them under a name |
| **1050–1051** | ~~"Give the cores back with `threads = 6`…" **stays and is promoted**~~ — **struck by Amendment 2**: there is no escape hatch. The sentence becomes the levels' own trade — 6, 4 and 2 threads for 169, 266 and 359 seconds — and the honesty it carried moves into the `full` paragraph, which says why `full` is half the cores rather than no cap |
| 1038, 1060 | the two stale `214 s` figures become R1a's, as B.1 already says |

The table that goes under B.1's paragraph, filled from B.8 and never retyped from memory:

| `resources` | threads here | wall | peak CPU | threads / running |
|---|---|---|---|---|
| `"full"` | no cap | R1b | R1b | R1b |
| `"balanced"` *(the default)* | 4 | R1a | R1a | R1a |
| `"low"` | 2 | R1c | R1c | R1c |

and beneath it the escape hatch, in one sentence that also carries §F.2's honesty:
`threads = N` beats the level wherever both are set, and on this machine `threads = 6` is faster
than `resources = "full"` — it takes rayon's twelve tokenizer threads down to six while ORT keeps
the six performance cores it would have chosen anyway (103.4 s against 110.8 s on the 320 longest
passages, [2026-09-07 §4.2](docs/bench/2026-09-07-resource-usage-results.md)) — so `full` is the
shape the binary had before the cap, not the quickest shape it has.

### F.8 The commit: a third one, not a fold

**Three commits**, `perf:` → `feat(config):` → `docs:`.

1. `perf: the background band is removed, and every writer runs in the normal one` — Part E's, unchanged.
2. **Amendment 2:** `feat(config)!: how much of the machine a rebuild takes is a word, not a thread count` — the `!` is earned, two documented keys are removed —
   `src/index/embed.rs`, `src/config.rs`, and the twelve call sites in `src/main.rs`, `src/ask.rs`,
   `src/bench.rs`, `src/dump.rs`, `src/index/cross.rs`. Body: the three levels and what each maps to,
   that `balanced` is what HEAD already did, the precedence rule and the same-place warning, and
   that `full` is the pre-cap shape rather than the fastest one.
3. `docs: the band's documents keep their date, and the default model's numbers are today's` —
   Part E's second commit, plus §F.7's README rows and B.8's three-level block.

Not folded into commit 1, for three reasons. The types differ and are not decoration: commit 1
removes a setting and commit 2 adds one, and a `feat(config):` line is how a new key is found in
`git log` later. They are separately revertible — reverting the band's removal should not take the
level with it. And commit 1's body already answers "what does a leftover `priority` key do"; making
it also answer "what is this new key" gives one commit two subjects.

Both commits are independently green: after commit 1 the tree has one lever and passes A1–A5; after
commit 2 it has two and passes A6–A8. The docs commit is last regardless, because the retest cannot
run until commit 2's binary exists.

The PR stays **one**, titled as commit 1's subject — the removal is why the PR exists — with a
"What replaced it" section carrying §F.7's three-row table and the `threads = 6` sentence.

---

## Task list — executed 2026-09-09

- [x] **Task 1 — the removal.** Part A in full. Landed as **`76f1049`**, `perf: the background band
      is removed, and every writer runs in the normal one`. A1–A5 green. One thing A1 as written
      cannot be true of: the §A.6 replacement test necessarily contains the word `priority`, so
      `rg -n "priority" src/` returns its name and its two fixture strings and nothing else. That is
      the bar meeting its own test, not a leak.
- [x] **Task 1b — the level.** Part F under Amendment 2: one key, no `threads`. Landed as
      **`99ab6fe`**, `feat(config)!: how much of the machine a rebuild takes is a word, not a thread
      count`. A6, A7, A8 green, plus A2 and A3 again — 468 unit + 16 serve + 1 users-day, clippy
      silent. The suite went 466 → 468: four `threads` tests out, four `resources` tests in, plus
      `a_threads_key_left_in_a_file_is_refused_by_name` and
      `an_unknown_level_in_the_environment_is_an_error_naming_the_variable`.
- [x] **Task 2 — the setup.** C.0 with §F.5's per-level stores, four of them plus three extra
      `balanced` copies for the re-runs. Pinned fixture verified untouched before and after.
- [x] **Task 3 — the long rows.** Four, not three: `balanced` (×3), `full` no-cap, `full` capped and
      `low`, then R2. C1b's thread count was read first as §F.2 requires and came back 18 / 6.
      **C1a missed** (265.7 s median against a 214.1 s bar) and **C1c's wall ratio missed low**
      (1.35× against 1.7–2.3×); everything else met. C6 met on both halves.
- [x] **Task 4 — the quick rows.** C2, C3, C4 met. C5 partly missed: two `ask` walls and four dense
      RSS figures over their bars by 0.03–0.09, and `changes` at 0.41 s against 0.07 — the last
      explained by the fixture copy this round was taken from carrying a dirty 41 MB `graphify-out/`
      that 2026-09-07's copy did not.
- [x] **Task 5 — the floors.** C7 met exactly, both arms, exit 0, appended against tool `99ab6fe`.
- [x] **Task 6 — the documents.** Part B with §F.7 and Amendment 2 folded in.
- [x] **Task 7 — the commits.** Three, as §F.8 under Amendment 2. No PR: this run was told to commit
      on the branch and stop there.

## What the plan got wrong when it met the code

- **The 214.1 s bar was cut from an uncapped row.** C1's whole premise was that the token budget and
  the chunked sync would bring a four-thread default in under a figure measured at eighteen threads.
  They do not, and the gap is the cap: like for like the same uncapped shape went 214.1 s → 178.1 s,
  17% faster, while the default sits at 265.7 s. The plan treated 214.1 s as "the default model's
  cost" in exactly the way it accused the README of doing.
- **C1c's wall band was written against an expectation, not a measurement.** 1.7–2.3× of *measured*
  `balanced` is not the same statement as 1.7–2.3× of the 150–185 s the plan expected, and under
  load the two diverge. `low` against the expected `balanced` is 1.94–2.39×, straddling the band.
- **A1 cannot be literally true and A5 be testable.** Both are in the plan.
- **`full`'s zero sentinel worked and was still the wrong answer.** §F.2 spent its length proving
  that `0` would reach both pools as "size yourself". It does — 18 / 6, exactly as predicted — and
  the shape it produces loses to capping rayon at six by 5.2% of the wall on identical user seconds.
  Verifying a mechanism is not the same as choosing a default.
- **`resources = "full"` on `ask --rerank-local` is still unmeasured**, and is now the only place a
  level can move a reader. Named in §F.5, named again in the results, measured by nobody.
- **The commit trailer this session was told to add is refused by the repository's own hook.**
  `bond:authorship-conventions` rejects `Co-Authored-By: Claude Opus 5`; commit 1 had already
  dropped it for the same reason, and commits 2 and 3 follow that precedent rather than bypassing
  the hook.
