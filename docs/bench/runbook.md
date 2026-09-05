# Running the comparison again — the runbook

[`three-graphs.md`](three-graphs.md) is the protocol: what is measured and how an
answer is judged. This file is the operational half — what to do, in order, on the
day someone re-runs it, the rule a retrieval change is judged by, and the seven things
that went wrong on an earlier run so they do not have to be rediscovered.

The last run is [`2026-09-03-three-graphs-results.md`](2026-09-03-three-graphs-results.md).

## The pinned fixture — read this before rebuilding anything

Everything below describes a full three-tool run, which needs both foreign indexes built and
takes roughly half an hour. Most runs do not need that. There is a fixture that already has
all three:

```
~/bench/beauty-crm-502e8a6d      # detached worktree of the corpus at 502e8a6d
```

| what | where | built by | cost to rebuild |
|---|---|---|---|
| corpus at `502e8a6d` | the worktree itself | — | 856 MB |
| gitnexus index | `.gitnexus/` | gitnexus 1.6.9 | 218 s, 598 MB |
| graphify graph | `graphify-out/graph.json` | graphify 0.9.48 | 24 s, 123 MB |
| repograph store | `.repograph/` | see below | 78 MB |

The repograph store carries the graph, the dense vectors and the enrichment questions, so the
fixture answers in the enriched arm without anyone paying for `enrich` again. The vectors and
`questions.json` were copied from the working clone's store at the same corpus commit rather
than recomputed. That transfer is only valid because both stores were built from the same
`502e8a6d` graph, so it was checked rather than assumed: one binary, both repositories, both
arms, and all 82 cases came back identical case by case, not merely equal in total.

| arm | fixture | working clone |
|---|---|---|
| dense | keyword 40/40, paraphrase 15/30, code 12/12, p90 226 | identical |
| lexical | keyword 37/40, paraphrase 15/30, code 12/12, p90 220 | identical |

**For a repograph-only run, use `bench/history/run-repograph.sh` and stop reading here.** It
builds the binary, runs both arms against the fixture, records them into the run history and
prints what improved, what regressed and what is chronically weak. Two minutes, no tokens.

Two things about the fixture that will otherwise cost an afternoon:

- **The fixture's `git status` is dirty by design.** `graphify-out/` is tracked in the corpus
  and graphify rewrites it in place. Four modified files and one new `.sig` are the deliverable,
  not debris. Judge cleanliness with `git status --porcelain -- ':(exclude)graphify-out'`.
- **gitnexus registers the fixture under the name `beauty-crm`, which the working clone already
  uses.** Plain-name resolution picks whichever was indexed last, silently, and a wrong name
  does not error — it answers about the other repository. `run.py` already defaults
  `--gitnexus-repo` to the corpus path, so the rule is simply never to override it with a name.

Rebuild the fixture only if the corpus commit under test has to change. That invalidates every
recorded run against it, which is the reason it is pinned.

## Before anything: the corpus is now hostile to two of the three tools

`beauty-crm` keeps `graphify-out/` (tracked) and `.gitnexus/` (git-ignored) on disk
**and runs neither tool**. That is a standing decision recorded in that repository's
`.claude/CLAUDE.md` and its `repo-query` skill, and re-running this comparison means
deliberately suspending it for the length of the run.

Do that in a scratch worktree, not in the working clone:

```bash
cd ~/Documents/projects/beauty-crm
git worktree add /tmp/bench-corpus <commit>
```

A worktree keeps the tools' writes — indexes, caches, skill files, rewritten
`CLAUDE.md` — off the clone someone is working in, and `git worktree remove` is the
whole of the cleanup. If the run happens in the clone instead, the post-run
checklist at the bottom is not optional.

## Pre-flight

- [ ] **The corpus is at a known commit with a clean tree.** Record the SHA; every
      number below is only comparable to another run through it.
      `git rev-parse --short HEAD && git status --porcelain`
- [ ] **`--strip-prefix` is needed if a graph was built in a worktree.** graphify
      writes absolute paths; the runner needs to know which directory to remove.
- [ ] **Each tool's version is recorded**, not assumed: `repograph --version`,
      `gitnexus --version`, graphify's own. The result header carries them and two
      result files are comparable only when those agree or the difference is the
      point of the run.
- [ ] **Disk.** gitnexus is ~600 MB per corpus, graphify ~160 MB. Both stay after
      the run unless removed.

## Build each index

How each index got fresh is part of the result, so write it into the `notes` block
of the result file exactly as it was run.

```bash
# repograph — ~1 s lexical, ~100 s for the vectors on first build
repograph --repo . build

# graphify — AST layer off the semantic cache, 0 tokens
graphify update <path> --force

# gitnexus — 236 s, ~598 MB. --skip-agents-md is NOT optional, see traps below
gitnexus analyze -f --embeddings --skip-agents-md .
```

`repograph enrich` (≈$2.50 on haiku, once) and `ask --rerank` (≈19k input tokens per
question on sonnet) are opt-in and change the numbers. If either is on, say so in
the notes — the 2026-09-03 run read paraphrase 16/30 with an opus `enrich` where the
shipped floor for an enriched store is 14/30, and a reader who does not know that
will read a regression into the next run. The build above runs neither, so its store
is graded on the raw floors instead — 9/30 with embeddings, 7/30 with `--no-dense` —
and `bench`'s summary line prints which of the two it used.

Two more switches change the numbers and belong in the notes the same way: `enrich --code`
adds questions about code (290 batches more on this corpus; the summary line then carries
`code_questions=`), and `REPOGRAPH_EMBED_MODEL=<hub id>` with `repograph embed` measures a
store under another embedder — on a copy of the store, never on the fixture.

## Run both suites

```bash
cd ~/Documents/projects/repograph/bench/compare
python3 run.py \
  --repo /tmp/bench-corpus \
  --repograph ~/Documents/projects/repograph/target/release/repograph \
  --graphify-graph /tmp/bench-corpus/graphify-out/graph.json \
  --truth /tmp/truth.json \
  --out ../results/$(date +%F)-beauty-crm.json
```

Useful flags: `--tools repograph` for one tool, `--suites blast` to skip the slow
retrieval pass, `--strip-prefix <dir>` for a worktree-built graphify graph,
`--truth <path>` to reuse a truth file rather than rebuild it.

**Rebuild the truth file when the corpus commit moves.** `truth.py` reads the
expectations out of the repository with ripgrep, so a reused truth file from an older
commit silently scores the new corpus against the old one. Reuse it only inside one
run, across tools.

## The seven traps, in the order they bit

1. **`gitnexus analyze` writes six skill directories into `.claude/skills/gitnexus/`
   without being asked for `--skills`**, whose own help says that flag generates them.
   `beauty-crm`'s `.gitignore` un-ignores `.claude/skills/`, so they arrive as
   untracked files a commit would take, and each one instructs an agent to query a
   graph the repository does not read. `rm -rf .claude/skills/gitnexus` after every
   rebuild.
2. **`--skip-agents-md` or it rewrites `CLAUDE.md` and `AGENTS.md`** unasked. Check
   `git status --porcelain` immediately after the analyze; a clean tree is the proof
   the flag worked.
3. **`~/.gitnexus/registry.json` drops a repository whose storage path has vanished.**
   Deleting `.gitnexus/` removes the entry; `registry.json.bak` is where the previous
   entry survives, and a re-analyze writes a fresh one. Compare the `stats` block
   across runs — nodes and edges should move with the corpus, not with the weather.
4. **graphify's `affected` refuses an ambiguous name** — `No unique node match` for
   `DatabaseService`, `TenantContextInterceptor`, `ActorResolver`, `OutboxPublisher`.
   That is scored as a miss, not skipped: a blast radius nobody can obtain is a blast
   radius of nothing. Do not "fix" it by disambiguating the case, or the suite stops
   measuring the tool as an agent would meet it.
5. **gitnexus's `detect-changes` CLI prints fifteen symbols then `... and N more`.**
   The MCP payload carries more. The table reports the CLI number; if the difference
   matters to a conclusion, read the payload by hand and quote it in the notes, as
   the 2026-09-03 run did for base `cbc931ba~1`.
6. **Latency is whole-process wall clock.** gitnexus has an `eval-server` that skips
   start-up; it was not used, because the other two have no equivalent. Turning it on
   for one tool makes the latency column meaningless.
7. **An `ask` against a store copy that has no source tree beside it empties the graph.**
   Every command that brings the store in line with the tree first — `ask`, `update`, `watch`,
   `impact`, `trace`, `changes`, all of which take `--stale` — refreshes against what it finds on
   disk, finds nothing, and removes every node. On 2026-09-05 one latency `ask` emptied a store
   copy's graph, restored afterwards from the fixture's `graph.bin` and `graph.json`. Measure on a
   copy that has the corpus beside it, or pass `--stale`; `bench` and `dump` read the store as it
   stands and cannot cause this, which is why the damage surfaces one command later as
   `graph is empty at <path> — run build first`. A store-only copy stays safe only while its
   `manifest.json` is empty: a manifest naming files that are not there is what triggers the
   refresh, so copying the fixture's manifest into such a copy re-arms the trap.

## Judging a retrieval change: the three-way rule

A lever is written down before it is measured, and read against all three of these — the four
hundred held-out questions first, the 82 recorded cases second, because reading the 82 first is how
a change gets fitted to the smoke test without anyone intending it:

1. **Every recorded floor holds in all four arms**, p90 ≤ 230: `bench` and `bench --no-dense`, on
   the enriched store and on a raw one.
2. **Held-out recall@5 is not significantly worse in either arm.** `bench/heldout.py`'s header is
   the order of operations — build the set, dump it on the old binary and on the new one, in both
   arms, then `compare`. The set the 2026-09-05 levers were read against is `heldout-400-syn.jsonl`:
   400 questions of kind `synthetic`, which is the kind `dump` applies leave-one-out to. A set
   written under any other kind holds nothing out and reads about 0.95 whatever the change was —
   that happened once, and G7 records it. The test is a paired exact McNemar, so what counts is the
   questions that changed answer and in which direction, not the two totals.
3. **The developer suite is not down in either arm**: `bench --cases bench/dev-cases.jsonl`, both
   arms. It is ungated by design, so this one is read rather than enforced by an exit code. A lever
   aimed at one kind of question says before the run what that kind has to reach — the 2026-09-05
   code levers had to put `where` above 0/9 as well as leave the rest standing.

The rule is not re-read once the numbers are in. Six levers were measured against it on 2026-09-05
and one passed; [the results](2026-09-05-dev-cases-results.md) say which, and what the other five
cost.

## What to write down

The result file's header carries the corpus path, commit, timestamp and case counts
by itself. The `notes` block does not write itself, and it is what makes the file
readable in six months. The 2026-09-03 file's notes are the model: one line per tool
saying how its index was built and how big it came out, plus a line for every
judgement call the run made — the gitnexus CLI cap, graphify's refusals, and exactly
what the truth file counts.

Then write the prose beside it, as
[`2026-09-03-three-graphs-results.md`](2026-09-03-three-graphs-results.md) does: the
tables, what each shape means, what was decided, and the caveats. A result file
nobody read is a run nobody made.

## Post-run cleanup, if the run was not in a worktree

- [ ] `rm -rf .claude/skills/gitnexus`
- [ ] `git status --porcelain` clean — nothing tracked was rewritten
- [ ] the ignore entries still cover both indexes: `pnpm lint` and
      `pnpm format:check` green with ~750 MB of machine-written JSON on disk
- [ ] `beauty-crm`'s standing decision restated where it was suspended: neither tool
      is run against that tree outside a measurement
