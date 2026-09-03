# The three-graph comparison

One corpus, one commit, the same questions to repograph, graphify and gitnexus.
This file is the protocol; `bench/compare/` is the code that runs it. Re-running it
on a new commit, a new repository or a new version of any tool takes one command
and produces a result file that can be set beside every earlier one.

## What gets measured

Two suites, both scored against the repository rather than against each other.

**Retrieval** — `bench/cases.jsonl`, 82 questions with one expected answer each:
40 by keyword, 30 paraphrases (the question shares no word with the requirement)
and 12 by code symbol. The expected answer is a requirement id (`FR-CAL-101`), a
task id (`BE-M17`) or a file path.

**Blast radius** — `bench/blast.jsonl`, 20 cases over the call graph:

| kind | cases | asks |
|---|---|---|
| `impact` | 10 | who breaks if this symbol changes |
| `trace` | 8 | the call chain from A to B, or that there is none |
| `changes` | 2 | what a diff since a base commit touches |

The ten `impact` targets are spread by fan-in on purpose — three hubs (18 to 58
referencing files), three wide, four narrow — because a tool that answers hubs well
can still miss a service with two callers, and the mean would hide it. The eight
`trace` cases are six real chains and two pairs with no path, so a tool that always
finds something scores 6, not 8.

## How the answer is judged

`bench/compare/truth.py` reads the expectations out of the repository with ripgrep
and a small TypeScript reader. No graph tool is consulted, so a tool that disagrees
with the truth file is wrong about the repository, not about a rival's model.

| suite | truth | hit |
|---|---|---|
| retrieval, strict | the expected id itself | the id appears in the answer |
| retrieval, soft | every file that spells that id | one of those files appears in the answer |
| impact | every file naming the symbol, minus the file declaring it | share of those files the answer names |
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
instead of rebuilding it.

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

## Reading a result file

`bench/results/<date>-<repo>.json` carries the corpus path, the commit, the case
counts, and per tool a `rows` array (one record per case) and a `summary`. Two
result files from different dates are comparable when the commit and the tool
versions in the header say they are — the numbers move with the corpus, not only
with the tools.

## Caveats that belong with every number

- One run per row, no repeats. For a fraction out of 30 the confidence interval is
  wider than the gap between neighbouring rows.
- The strict criterion is a substring: an id fifth of five counts the same as first.
- Latency is wall clock for a whole process, so every tool pays its own start-up.
  gitnexus also has an `eval-server` that skips it; the numbers here do not use it,
  because the other two have no equivalent.
- gitnexus prints at most 15 changed symbols from `detect-changes`, then `... and N
  more`. Its MCP payload carries all of them. The CLI number is what the table
  reports; the payload was checked by hand and is quoted in the notes where it differs.
- The case set lives in the repograph repository and was written against it.
  Keywords and code symbols are taken by all three; the paraphrases make the set hard.
