# The three-graph comparison

One corpus, one commit, the same questions to repograph, graphify and gitnexus.
This file is the protocol; `bench/compare/` is the code that runs it. Re-running it
on a new commit, a new repository or a new version of any tool takes one command
and produces a result file that can be set beside every earlier one.

Three files sit around this one and each answers a different question:

| | |
|---|---|
| [`2026-09-04-repograph-0.5.0-results.md`](2026-09-04-repograph-0.5.0-results.md) | the latest run — repograph alone, 32 blast cases, `rank` and `mrr`, and which comparisons to the run below are not deltas |
| [`2026-09-03-three-graphs-results.md`](2026-09-03-three-graphs-results.md) | the three-tool run, and what was decided because of it |
| [`runbook.md`](runbook.md) | how to run it again — pre-flight, the six traps, the cleanup |
| [`next-version-gaps.md`](next-version-gaps.md) | the weak spots those numbers exposed, each with its gate |

## What gets measured

Two suites, both scored against the repository rather than against each other.

**Retrieval** — `bench/cases.jsonl`, 82 questions with one expected answer each:
40 by keyword, 30 paraphrases (the question shares no word with the requirement)
and 12 by code symbol. The expected answer is a requirement id (`FR-CAL-101`), a
task id (`BE-M17`) or a file path.

**Blast radius** — `bench/blast.jsonl`, 32 cases over the call graph:

| kind | cases | asks |
|---|---|---|
| `impact` | 16 | who breaks if this symbol changes |
| `trace` | 8 | the call chain from A to B, or that there is none |
| `changes` | 8 | what a diff since a base commit touches |

The sixteen `impact` targets are spread by fan-in on purpose. A tier is a count of
referencing files, not a judgement call: narrow is 5 or fewer, wide is 6 to 19, hub is
20 or more. By that cutoff the suite holds two hubs (24 to 48 referencing files),
three wide (6 to 10), eleven narrow (2 to 3) — because a tool
that answers hubs well can still miss a service with two callers, and a mean across
tiers would hide it; six narrow targets were added 2026-09-04 for that reason. A
seventh case, `TenantContextInterceptor`, was filed as a hub when the suite was
first written but has only three referencing files; it was relabelled narrow the
same day, which is why the suite holds two hubs rather than three. The eight
`trace` cases are six real chains and two pairs with no path, so a tool that always
finds something scores 6, not 8. `changes` grew the same day from two bases to
eight, for the same reason: two diffs are too thin to read a delta from.

## How the answer is judged

`bench/compare/truth.py` reads the expectations out of the repository with ripgrep
and a small TypeScript reader. No graph tool is consulted, so a tool that disagrees
with the truth file is wrong about the repository, not about a rival's model.

| suite | truth | hit |
|---|---|---|
| retrieval, strict | the expected id itself | the id appears in the answer |
| retrieval, soft | every file that spells that id | one of those files appears in the answer |
| impact | every file naming the symbol in code — comments and string literals blanked — minus the file declaring it | share of those files the answer names |
| trace | the chain through injected fields, found by breadth-first search | every intermediate name appears, and the tool does not say "no path" |
| changes | the enclosing declaration of every hunk in the diff | share of those symbols the answer names |

The soft criterion exists because graphify and gitnexus answer with files and
symbols rather than requirement ids; without it the comparison would be unfair to
them. The gap between strict and soft is the portrait of a tool: repograph loses
cases on it (right file, neighbouring requirement), gitnexus gains dozens (always
in the right file, never naming the id).

The `impact` truth is references, not calls: an import, a type position and a
constructor injection all count. A file that names a symbol depends on it, and a
blast radius that omits it is a blast radius its reader cannot trust.

## Running it

```bash
# 1. the corpus must be at a known commit with a clean tree
cd ~/Documents/projects/beauty-crm && git rev-parse --short HEAD

# 2. each tool's index must be fresh, and how it got fresh is part of the result
repograph --repo . build                  # or let `ask` refresh it
graphify update <path> --force            # AST layer, semantic cache, 0 tokens
gitnexus analyze -f --embeddings --skip-agents-md

# 3. run both suites against all three
cd ~/Documents/projects/repograph/bench/compare
python3 run.py \
  --repo ~/Documents/projects/beauty-crm \
  --repograph ~/Documents/projects/repograph/target/release/repograph \
  --graphify-graph <path>/graphify-out/graph.json \
  --truth /tmp/truth.json \
  --out ../results/$(date +%F)-beauty-crm.json
```

Useful flags: `--tools repograph` to run one, `--suites blast` to skip the slow
retrieval pass, `--strip-prefix <dir>` when graphify reads a graph built in a
worktree and its paths carry that directory, `--truth <path>` to reuse a truth file
instead of rebuilding it, `--cases <path>` / `--blast <path>` to run against a case
file other than the tracked default — the result header's `case_files` records
which one a run used.

Every tool is invoked through its own command line, without hints:

| | retrieval | impact | trace | changes |
|---|---|---|---|---|
| repograph | `ask` | `impact --depth 3` | `trace` | `changes --base` |
| graphify | `query --graph` | `affected --depth 3` | `path` | — |
| gitnexus | `query -r` | `impact -d upstream --depth 3` | `trace` | `detect-changes -s compare -b` |

## Adding a case

A retrieval case is one line in `bench/cases.jsonl`: `{"kind","q","expect"}`, where
`kind` is `keyword`, `paraphrase` or `code`. A blast case is one line in
`bench/blast.jsonl`:

```json
{"kind":"impact","target":"AuthService","file":"apps/api/src/modules/identity/auth.service.ts","tier":"wide"}
{"kind":"trace","from":"AuthController","to":"DatabaseService","expect":"path","via":["AuthService","IdentityRepository"]}
{"kind":"trace","from":"SystemController","to":"DatabaseService","expect":"none"}
{"kind":"changes","base":"cbc931ba~1","note":"the nearest base whose diff carries TypeScript"}
```

Nothing but the case is written down. The files a target is referenced from, the
chain a trace should follow and the symbols a diff touches are all recomputed from
the repository at run time, so the set survives a refactor: it goes stale only when
a name disappears.

A `changes` case is everything since its base, so it grows as HEAD moves — see the
caveat below on comparing `changes` across runs. The `note` records the mix the
commit itself had when the case was chosen, not the mix a later run will find.

## Adding a corpus

`bench/corpora/<name>/blast.jsonl` (and `cases.jsonl` where the corpus has its own ids and
prose) are run with `--blast` / `--cases`; the result goes to `bench/results/<date>-<name>.json`.
The first one is `beauty-crm-mobile`: eight Kotlin `impact` targets that score 0 on every
repograph before 0.6.0, written down so that the Kotlin extractor is measured on the day it
lands rather than predicted. No language ships without its file here and a result beside it.
This corpus has no Kotlin symbol at hub scale — its widest target has eight referencing
files — so its tiers read wide and narrow only, not lopsided against the same cutoffs the
main suite uses.

## Reading a result file

`bench/results/<date>-<repo>.json` carries the corpus path, the commit, the case
counts, and per tool a `rows` array (one record per case) and a `summary`. Two
result files from different dates are comparable when the commit and the tool
versions in the header say they are — the numbers move with the corpus, not only
with the tools.

Every retrieval row since 2026-09-04 also carries `rank`: 1 + the distinct competing ids (for
a file case, paths) that appear in the answer before the expected one, or `null` when it is
absent. The summary's `mrr` is the mean of `1/rank` over the suite with absences as 0. Strict
says whether the answer is there; MRR says how far down. The 2026-09-03 file predates the field.

## Caveats that belong with every number

- One run per row, no repeats. For a fraction out of 30 the confidence interval is
  wider than the gap between neighbouring rows.
- The strict criterion is a substring; `rank` and `mrr` are what say where in the answer it sat.
- Latency is wall clock for a whole process, so every tool pays its own start-up.
  gitnexus also has an `eval-server` that skips it; the numbers here do not use it,
  because the other two have no equivalent.
- gitnexus prints at most 15 changed symbols from `detect-changes`, then `... and N
  more`. Its MCP payload carries all of them. The CLI number is what the table
  reports; the payload was checked by hand and is quoted in the notes where it differs.
- The case set lives in the repograph repository and was written against it.
  Keywords and code symbols are taken by all three; the paraphrases make the set hard.
- A `changes` case's truth is the diff from its base to the corpus's live HEAD, not a
  fixed window, so its want-totals grow as the corpus moves forward on its own. Two
  runs' `changes` fractions are comparable only when both ran at the same corpus
  commit — it is in every result file's header, and a reader comparing `changes`
  across runs has to check it before comparing anything else.

## When a blast delta counts

One run per row, no repeats, so a rule is stated before a change is measured, not after:

| suite | a change counts when | noise |
|---|---|---|
| impact | mean recall over 16 does not fall, no case falls by more than one file, and no case with fewer than five referencing files may fall at all | one file on a case with five or more referencing files |
| trace | 8/8 stays 8/8 | none — a chain either resolves or it does not |
| changes | `symbols_found / symbols_want` over 8 cases rises by ≥ 0.05 and `files_found / files_want` does not fall | ±1 symbol on one case |

Anything inside the noise column is reported and not argued from.
