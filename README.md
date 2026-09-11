# repograph

A project knowledge graph that costs **zero API tokens** to build, keep fresh, and query.

`repograph` reads a repository's markdown and TypeScript, extracts the structure the authors already
wrote by hand — requirement ids, cross-references, invariants, milestones, exports, imports,
decorators, and the ids quoted inside code comments — and answers questions about it in a few lines
of text.

## What it is

It is built around one claim: **the hard part is not building a better graph, it is building a better
door onto it.** A repository's requirement ids already form a dense, human-authored graph. What was
missing was a way in that finds the right entry point from a question phrased in ordinary words.

Measured against `graphify`, the LLM-extracted graph it replaces, on the 38 questions of the day:
0/14 paraphrase and 11/24 keyword at 1,027–1,555 tokens an answer and 14.6 M tokens to build,
against 7/14, 24/24, 197 median tokens and zero. The table and its caveats are in
[the measurements](docs/history.md#the-head-to-head-against-the-graph-it-replaces); the recorded set
has since grown to the 82 cases [Bench](#bench) floors.

## Status

0.5.1 is the version `main` carries, and every command below is implemented rather than planned:
`build` and `update` (incremental; a no-op `update` is a fixed point), `families`, `ask`, `explain`,
`verify`, `impact`, `trace`, `changes`, `embed`, `watch`, `serve`, `prime`, `install-agent`,
`import-legacy`, `dump` and `bench`. Three spend model tokens and all three are opt-in: `enrich`,
`ask --rerank`, and `ask --rerank-local` (zero tokens, a local cross-encoder, measured and rejected
as a floor candidate). `bench` fails the process when a floor in [Bench](#bench) is missed; floors
are keyed by enrichment and by the store's embedder, and a store under any other model is measured
and not graded.

`--no-dense` skips the embedding stage everywhere it could apply — `build`, `update`, `enrich`,
`embed`, `watch`, `serve`, `ask`, `bench`, `dump`. Without it, those commands use local embeddings
once the model is cached (see [Embeddings](#embeddings)).

## Install

The packages live in GitHub Packages, which serves no anonymous reads — every consumer
authenticates, public package or not. Once per machine, in `~/.npmrc`:

```
@devmaxxx:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=<classic PAT with read:packages>
```

The token must be a **classic** personal access token; the npm registry does not accept
fine-grained ones. A project-level `.npmrc` can carry the first line, but not the second: pnpm
stopped expanding `${ENV_VAR}` in project files in 11.5.3, so a committed `_authToken` would
either be a literal secret or silently ignored.

Then:

```bash
pnpm add -D @devmaxxx/repograph    # npm: prebuilt binary for macOS arm64, Linux x64 and Windows x64
cargo install --path .             # from source; Rust 1.98, pinned in rust-toolchain.toml
```

The npm package is a launcher: the binary comes from `@devmaxxx/repograph-darwin-arm64`,
`@devmaxxx/repograph-linux-x64` or `@devmaxxx/repograph-win32-x64`, pulled in as an optional
dependency, so a lockfile written on one platform installs on the others. All three are under the
same scope, so the one registry line above covers them. Tagged releases (`v*`) build all three
binaries as GitHub release assets (a tarball for the unix ones, a zip holding `repograph.exe` for
Windows) and cut the npm packages from those same files (`.github/workflows/release.yml`,
publishing with the repository's own `GITHUB_TOKEN`; `scripts/npm-pack.sh` does the same by
hand).

### On Windows

Windows 10 1903, Windows 11 or Server 2022, CPU inference, an unsigned binary, and a handful of
platform-specific traps — SmartScreen on a browser-fetched zip, Defender scanning the model as it
lands, `Start-Process` instead of `&`, PowerShell's code page on a *piped* answer, and keeping the
repository out of a OneDrive tree. All of it is in [running on Windows](docs/windows.md).

## Use

Build the graph once, then keep it fresh incrementally:

```bash
repograph --repo /path/to/project build     # full build
repograph --repo /path/to/project update    # re-extract only changed files
```

`--repo` defaults to the current directory. State lives in `<repo>/.repograph/`; add it to
`.gitignore`.

After a first build, read which id families the documents defined and what was left as text:

```bash
repograph families                          # every family, its nodes, and the line that defines it
repograph families --json                   # the same numbers for a script
```

Ask it something:

```bash
repograph ask cancellation policy
repograph ask FR-PAY-22                     # an exact id short-circuits straight to the node
repograph ask asGrosze                      # so does an exact symbol name
repograph ask --bodies отмена записи        # print full requirement bodies, not just headlines
repograph ask --json отмена записи          # machine-readable
repograph ask --seeds 8 отмена записи       # widen the search beyond the default of 5; costs more tokens
repograph ask --no-dense отмена записи      # lexical only, no embedding query
```

Ask it what depends on a symbol, what a symbol reaches, and how one reaches another:

```bash
repograph impact StaffService               # callers by depth, importing files, a risk line
repograph impact --down StaffController     # what it calls, through injected services and barrels
repograph impact --json --depth 1 asGrosze  # machine-readable; depth 1 is the "will break" list alone
repograph trace StaffController StaffService  # shortest chain of calls between two symbols
repograph changes                           # what the uncommitted diff touches, and who reaches it
repograph changes --base main --depth 1     # the whole branch; depth 1 is the direct callers alone
```

Answers are lines of the form:

```
ID  path:line  headline
  ID  path:line  headline  ← seed-id
```

Top-level lines are the seeds the query matched. An indented line is a neighbour reached by one hop
over the id graph, and the `←` names the seed it came from. A real run, against `beauty-crm`:

```
$ repograph --repo beauty-crm ask cancellation
sym:packages/db/src/schema/salon/scheduling.ts::CANCELLATION_CONSEQUENCES  packages/db/src/schema/salon/scheduling.ts:110  CANCELLATION_CONSEQUENCES
BE-M10/T05  docs/prd-2026-08-16/plans/milestones/backend/BE-M10-payments-provider-stripe-connect-implementation-deposits-car.md:80  Cancellation policy engine (`packages/domain/policy.ts`): inputs policy + appoin…
PLAT-M12/T09  docs/prd-2026-08-16/plans/milestones/platform/PLAT-M12-billing.md:48  Cancel semantics N-141: immediate stop of future charges, accrued usage as dated…
PLAT-M12/T04  docs/prd-2026-08-16/plans/milestones/platform/PLAT-M12-billing.md:43  Panel subscription page: plan, next invoice, cancel with end-of-access date, exp…
BE-M10/T07  docs/prd-2026-08-16/plans/milestones/backend/BE-M10-payments-provider-stripe-connect-implementation-deposits-car.md:82  EOD fee charger worker: `chargeOffSession` for due fees → intent(purpose=cancell…
  BE-M10  docs/prd-2026-08-16/plans/milestones/backend/BE-M10-payments-provider-stripe-connect-implementation-deposits-car.md:1  BE-M10 Payments provider: Stripe Connect implementation, deposits, card-on-file …  ← BE-M10/T05
```

### Answering a program instead of a person

Every reader takes `--json`, and every one of them answers with an object rather than a bare array,
so a field can be added without breaking a parser written against the version before it:

| Command | Top-level keys |
| --- | --- |
| `ask --json` | `seeds`, `expanded` |
| `impact --json` | `root`, `at`, `direction`, `risk`, `direct`, `total`, `files`, `importers`, `layers` |
| `changes --json` | `risk`, `touched`, `affected`, `files` |
| `families --json` | `families`, `milestones`, `mention_only` |
| `explain --json` | `id`, `kind`, `label`, `file`, `line`, `community`, `edges` |
| `verify --json` | `nodes`, `edges`, `nodes_by_kind`, `edges_by_kind`, `dangling`, `undeclared`, `gaps`, `cite_only`, `held_aside`, `held_aside_prefixes` |
| `trace --json` | `from`, `to`, `depth`, `path` |
| `prime --json` | `nodes`, `edges`, `enriched`, `questions`, `families`, `model` |

`explain --json` resolves each edge's direction for you — `dir` is `in` or `out` and `other` is the
node at the far end — so a caller never works out which end of an edge it was standing on. One
difference from the text forms is deliberate: a `trace` that finds no path within the depth is an
answer to the question that was asked, so the JSON form prints `"path": null` and exits 0 where the
text form exits **3**. The two are the same answer in two shapes: a caller parsing an object should
not have to read an exit code to learn what the object already says, and a shell script reading the
text form should not have to tell that answer from the `1` an unknown symbol exits with.

### Keeping it fresh

`ask` walks the tree before it answers. Anything edited since the last build is re-extracted in
process and saved, so the graph is never behind the working copy and no `update` has to be
remembered. One line goes to stderr when that happens:

```
refresh: 3 changed, 1 removed
```

A fused query opens the embedding model anyway, so the rows that changed are re-embedded and the
vectors stay in step too. `--no-dense` and the exact-id path open nothing: the lexical graph is
fresh, and the vectors catch up on the next fused query or `update` — after an `update --no-dense`
from a commit hook too, because `vectors.json` records the graph its rows were last synced against.
Measured on the development
corpus (825 files, 7.5k nodes): the no-change check costs ~10 ms, and a one-file edit costs ~20 ms
lexical, ~40 ms with the re-embedding — the new rows are appended to `vectors.f32` and the rows
they replace are left as holes, so nothing rewrites 50 MB to store a row of 1.5 kB. Holes past a
quarter of the live rows are compacted away by the next refresh, which does rewrite both files.
`REPOGRAPH_TIMING=1` prints the stages.

An upgrade is the one refresh that reads everything. The manifest records which generation of the
extractor's grammar the graph beside it was read by, and where a newer generation is reading, a hash
settles nothing: a file nobody has touched may hold a citation the older grammar never looked for.
So the first `update`, `watch` poll or refreshing `ask` after such an upgrade says on stderr that it
is re-reading the whole tree and why, writes the new generation forward, and leaves every update
after it incremental again. A release that does not change the grammar costs no walk at all.

```bash
repograph ask --stale отмена записи         # answer from the store as it stands, no check
```

`--stale` never pays for that walk and never repairs it: it answers from the store as it stands,
which is the whole of what it promises.

Readers that do not refresh themselves — an editor plugin, an MCP server — can be kept supplied by
a poller instead:

```bash
repograph watch                  # poll every 30 s, apply what changed, embed it
repograph watch --every 15       # a tighter cadence
repograph watch --batch 5        # save once five files are waiting, not on every one
repograph watch --no-dense       # lexical only, leaves the 1.4 GB model unopened
```

A refresh rewrites the whole graph, so `--batch` is there to spend that once on a burst of edits
rather than once per file. Fewer than `--batch` files waiting are carried to the next poll, at most
three times in a row, so a lone edit lands within four polls whatever the batch says;
`REPOGRAPH_TIMING=1` reports each deferral.

It prints one line per refresh and exits on Ctrl-C; every store write is a temp file and a rename,
so interrupting it cannot leave half a graph behind. An idle poll is the walk and nothing else —
21 ms of CPU on the development corpus, under a tenth of a percent of a core at the default
cadence.

### Asking a resident process

Most of a fused `ask` is the process opening things it then throws away: the embedding model
alone costs about 220 ms of it on the shipped default. `serve` opens
them once and answers over a Unix socket — on Windows too, where the same socket file has existed
since Windows 10 1803 and is protected the way the repository directory is:

```bash
repograph serve                  # .repograph/serve.sock, poll every 30 s, exit after 30 min idle
repograph serve --idle 86400     # a day rather than half an hour before it gives up
repograph serve --idle-model 60  # forget the model after a minute unasked; the process stays
repograph serve --no-dense       # lexical only; a fused `ask` is told so and answers in its own process
repograph ask --no-serve отмена  # answer here even while one is listening
```

Two idles, one clock. `--idle` ends the process; `--idle-model` (default 300 s) drops only the
model weights and keeps everything that answers without them — the graph, the ids, the lexical
indexes, the vectors — so a server left up overnight is cheap to leave up. On a 33.5k-row store the
resident size goes 908.5 MB → 28.8 MB on the first drop and 277.6 MB on later ones (the allocator
keeps some of what the reopen took), and the first fused `ask` after a drop pays the open: 0.771 s
against 0.083 s warm.

The socket lives at `.repograph/serve.sock` — unless that path would be longer than a Unix socket
name may be (104 bytes on macOS, including the terminating NUL), in which case it goes in the
temporary directory as `repograph-<hash of the repository path>.sock`. A repository under a deep
enough path could not run `serve` at all before that fallback, and both the server and every client
compute the name the same way, so nothing has to be told which one is in use — the startup line
prints it. A `serve` that is killed with `SIGTERM` or `SIGINT` takes its socket file with it,
because it leaves through the same exit as an idle timeout; on Windows there is no equivalent
signal and the file is left for the next `serve`, which removes a dead socket before it binds.

`ask` uses it without being told to and answers in this process whenever it cannot — no socket, a
socket nobody listens on, a server of another version or another build of the same version, a
`--no-dense` server asked a fused question, a timeout, `--no-serve`, or `REPOGRAPH_NO_SERVE`. Each
of those prints a line: a question that quietly costs a cold process, or quietly gets a lexical
answer, looks like nothing at all. A client never deletes the socket file, since a refused connect
is also what a live server with a full backlog gives; only `serve` removes one, and only the one it
bound itself — told from a replacement's by device and inode on unix, by NTFS's file reference
number on Windows.

The socket name is bounded by the platform's own limit — 104 bytes on macOS, 108 elsewhere — which
is why the fallback exists; the arithmetic of a deep Windows path, and the check that a resident
answer is byte-identical to a cold one across all 142 recorded questions, are in
[the measurements](docs/history.md#the-socket-path-and-the-same-bytes-either-way).

The server refreshes before every answer with the same walk a one-shot `ask` does, and polls
between them like `watch`, so its **graph** is never staler than a fresh process's; `--stale` skips
that walk and reads the stored graph as it is on disk. It answers one question at a time, and a
second client waits rather than being turned away. Both reads are decided by `manifest.json`, so a
running server does not see `enrich` writing `questions.json` or `embed` writing `vectors.*` — stop
it, or ask with `--no-serve`, to answer from the new ones.

The configuration is read once, at start-up. Editing `repograph.toml` stops the server after its
next poll — the next `ask` answers in its own process under the new file, and a new `serve` starts
under it too.

Measured on the bench corpus, median of eleven with socket and in-process runs interleaved: a fused
question is **66 ms** resident against 326 ms in a fresh process, and the lexical arm **6.8 ms**
against 106 ms once the BM25 indexes stopped being rebuilt per question. The first question after a
start still pays the model open. The full tables, and the evidence that the answer bytes do not
move, are in [the measurements](docs/history.md#the-resident-answer-measured).

Git hooks are the free version of the same thing, for a repository whose changes arrive by pull:

```sh
# .git/hooks/post-merge, .git/hooks/post-checkout, .git/hooks/post-commit
#!/bin/sh
exec repograph --repo "$(git rev-parse --show-toplevel)" update
```

`chmod +x` each of them. `post-checkout` and `post-merge` cover a branch switch and a pull;
`post-commit` covers your own work, which the self-healing `ask` already handles.

Inspect one node and everything attached to it:

```bash
repograph explain <id|symbol|label>         # tries an exact id, then a symbol name, then a label
repograph explain FR-PAY-22
repograph explain asGrosze
```

`explain` resolves its argument as an exact id, then as a symbol name, then as a case-insensitive
label match.

Check the graph's health — counts by kind, dangling edges, how many citations are held aside because
no line defines their prefix, and ids that are referenced but never declared, split into gaps inside
a declared family (worth chasing) and shapes the dialect does not read as ids at all:

```bash
repograph verify
```

Fold in a graphify graph's model-only edges as a frozen legacy layer:

```bash
repograph import-legacy path/to/graphify/graph.json
```

This prints how many of the source graph's edges resolved to real nodes on both ends, on one end, or
needed a new `LegacyConcept`. It is opt-in and lossy by construction — see its coverage note under
[Bench](#bench) before running it on a store you plan to benchmark.

Run the recorded benchmark:

```bash
repograph bench                      # bench/cases.jsonl: 40 keyword + 30 paraphrase + 12 code
repograph bench --cases other.jsonl  # any shape: the 40/30/12 shape is graded, any other is measured ungated
repograph dump --queries qs.jsonl --out lists.json   # every retriever's ranked list per question, 300 deep
```

## How a question becomes an answer, and what ends up in the graph

Exact id or symbol first, then BM25 over passages and over `enrich`'s generated questions, then a
dense list, interleaved rank by rank; the surviving seeds expand one hop over the id graph, and the
answer is rendered as `ID  path:line  headline`. The node kinds, the edge kinds, what the
TypeScript and markdown extractors take, and what `impact`, `trace` and `changes` can and cannot
prove are in [the graph model](docs/graph-model.md).

Two things worth knowing before you trust an answer: the graph records only what a file proves, so
a call through a chained expression or a callback has no edge and a "nothing uses this" wants an
`rg -l` beside it; and `impact`'s risk label is four fixed thresholds printed with the counts they
came from, so it can be argued with.

## Configure

`repograph.toml` at the repository root — this file also doubles as the worked example, set to its
own defaults. Every key is optional; a repository with no `repograph.toml` gets `Config::default()`
in full, not an empty config:

| Key                  | Default                                                                                     |
| -------------------- | ------------------------------------------------------------------------------------------- |
| `doc_globs`          | `["**/*.md"]`                                                                               |
| `code_globs`         | `["**/*.ts", "**/*.tsx"]`                                                                   |
| `skip`               | `["**/node_modules/**", "**/dist/**", "**/TRACKER.md", "graphify-out/**", ".repograph/**"]` |
| `registries`         | `["docs/constitution.yaml"]`                                                                |
| `enrich_command`     | **machine file only** — headless `claude -p --model {model}` with thinking off, see [Spending tokens on purpose](#spending-tokens-on-purpose) |
| `rerank_command`     | **machine file only** — the same command, with `rerank_model` in its `{model}`               |
| `enrich_model`       | `haiku` — whatever goes in `enrich_command`'s `{model}`                                      |
| `rerank_model`       | `sonnet` — the same for `rerank_command`                                                    |
| `reranker_dir`       | directory of the exported cross-encoder for `--rerank-local`; empty = `~/.cache/repograph/reranker` |
| `embed_model`        | `intfloat/multilingual-e5-small`; the model the vectors are written with — see [Embeddings](#embeddings) |
| `resources`          | `"balanced"` = a third of the logical cores; `"low"` a sixth, `"full"` a half — how much of the machine a run may take, see [Resources](#resources) |

### Choosing a model, and where the choice lives

`enrich_command` and `rerank_command` are the transport — any program that reads a prompt on stdin —
and which model that program runs is a separate key, substituted into the command's `{model}`.
Changing model is a word rather than a rewritten command line; a command naming no `{model}` runs
exactly as written. Nothing here assumes a vendor. Both run under `sh -c`; on Windows that is Git
for Windows' `sh`, found on `PATH`, beside `git`, at `CLAUDE_CODE_GIT_BASH_PATH` or under Program
Files, with Git's `usr\bin` put on the command's own `PATH`. Without Git for Windows, `enrich` and
`ask --rerank` refuse with a line that says so and everything else runs.

Which model to run is usually a property of the machine — what is installed, what the account may
spend — rather than of the corpus, so it can be set once for every repository. Three layers, each
beating the one below it:

| Layer | Where |
| --- | --- |
| the run | `REPOGRAPH_ENRICH_MODEL`, `REPOGRAPH_RERANK_MODEL`, `REPOGRAPH_RESOURCES` |
| the repository | `repograph.toml` |
| the machine | `$REPOGRAPH_CONFIG`, else `$XDG_CONFIG_HOME/repograph/config.toml`, else `~/.config/repograph/config.toml` (`%USERPROFILE%\.config\repograph\config.toml` on Windows) |

```toml
# ~/.config/repograph/config.toml — every repository on this machine, unless it says otherwise
enrich_model = "haiku"
rerank_model = "sonnet"
```

A key the repository names wins even when it names the built-in value — with two exceptions, and
they run the other way. **`enrich_command` and `rerank_command` are read from the machine file and
never from a repository.** A repository you cloned is untrusted input, and those two keys are a
shell command that would run on your machine the first time you ran `enrich` or `ask --rerank` in
it; a `repograph.toml` that names one gets a line on stderr saying where the key belongs, and the
machine's command — or the built-in — is used. A repository may still say which *model* it wants,
and because that name lands in a shell string, anything outside letters, digits and `._:/@+-` is
refused the same way. What a clone chooses is the model; what runs it is yours.

The machine file may set **only** `enrich_command`, `rerank_command`, `enrich_model`,
`rerank_model`, `reranker_dir` and `resources`, and refuses any other key by name: the
corpus-shaped keys describe one repository's documents, and a global `embed_model` would rewrite
every store's vectors under a model nobody chose for it.

`threads` and `priority` were removed on 2026-09-09. Replace `threads = 6` with
`resources = "full"` and `threads = 2` with `resources = "low"`; delete `priority`, there is no
scheduling band any more. Either key left in a config file is a hard error naming the file and the
key, not a setting quietly ignored — both structs are `deny_unknown_fields`, and a key that parsed
and did nothing would leave you believing a number you wrote is still read.

Three of those keys name a model and one names a directory, and they are four different jobs.
`enrich_model` writes the questions (`haiku`: the register the questions are written in beats the
model that writes them); `rerank_model` picks five ids out of a 200-deep pool (`sonnet`: opus buys
nothing and costs a keyword hit, haiku loses three paraphrases); `embed_model` is the store's own
and is weighed in [ADR-002](docs/adr/ADR-002-two-defaults-multiplied.md); `reranker_dir` is the
local cross-encoder, measured and rejected as a floor candidate. Every one of those readings, with
what each stage asks of a model, is in
[the measurements](docs/history.md#what-each-stage-asks-of-a-model-and-every-answer-measured).

**The contract a command has to meet** is the same for both stages and names no vendor. `sh -c`
runs it, the prompt arrives whole on stdin, the answer is read from stdout, and `{model}` anywhere
in the command is replaced by that stage's model key. Both parsers keep only what they recognise — `id<TAB>question` lines for ids in the batch from
`enrich`, ids it actually showed from `--rerank`, in the model's order — so a chattier command is
survivable rather than fatal, and a failing one is answered with a notice and the fused order. A
command that answers without reading its prompt to the end is fine too: the broken pipe the write
hits is ignored on purpose. The default runs headless Claude Code with thinking off: the same
answers, 4–5× faster.

**Another vendor is a different command line, not a different repograph.** Only the first of these
is what this repository runs; the second is verified against that CLI's `--help` on this machine and
the rest are shapes. All of them go in the **machine** file — `~/.config/repograph/config.toml` —
because a command a cloned repository names is a command it runs on your machine:

```toml
# ~/.config/repograph/config.toml
# The shipped default: headless Claude Code, thinking off.
rerank_command = "MAX_THINKING_TOKENS=0 claude -p --model {model} --output-format text --tools \"\" --setting-sources \"\" --no-session-persistence"
rerank_model = "sonnet"

# OpenAI's Codex CLI. Read from `codex exec --help` here and not run: with no prompt argument
# the instructions are read from stdin, `-m, --model <MODEL>` names the model, and
# `--skip-git-repo-check` allows a directory that is not a repository. Plain stdout is the run's
# own transcript, so `-o, --output-last-message <FILE>` pointed at /dev/stdout is what guarantees
# the answer reaches stdout at all; the parsers drop the transcript lines around it.
# rerank_command = "codex exec --skip-git-repo-check -m {model} -o /dev/stdout"
# rerank_model = "<a model that install has>"

# A model on the machine, through Ollama — a shape. `ollama run --help` documents
# `ollama run MODEL [PROMPT]` and the thinking switches; nothing was run here, this machine has
# no daemon up and no model pulled, and whether a piped prompt needs the argument omitted is
# for whoever has one to confirm.
# rerank_command = "ollama run {model} --hidethinking"
# rerank_model = "<a pulled model>"

# Anything else — any API, any account: five lines that read stdin and print the answer, and the
# model stays a word in a config file.
# rerank_command = "python3 tools/rerank-via-some-api.py --model {model}"
```

No non-Claude model has been read on these cases, so none of the rows above is a claim about one.
An attempt on 2026-09-10 filled none of them and wrote down why — an account, an empty Ollama
shelf, a 2.1 GB download, ~$6 of enrichment — in
[the cross-vendor refusals](docs/bench/2026-09-10-cross-vendor-refusals.md), which also re-checks
both shapes against the installed CLIs.
Reading one is the same two commands the numbers here came from: `bench --rerank` against an
enriched store measures a `rerank_model`, and a plain `bench` against a copy the other model
enriched measures an `enrich_model`. A store records the model its *vectors* were written with and
nothing about the model that wrote its questions, so those copies are kept apart by hand — one
directory per enriching model — or the comparison quietly measures a mixture.

### Id families

There is no key for them. A family is the prefix of any id the corpus *defines*, and the documents
are the only place that is written down: the requirement line `**FR-PAY-22 · MUST · <title>**` and
its heading form `## FR-CAL-40 · <title>` (the modality is optional in both), a milestone document
named `BE-M01-….md` or a `## BE-M01 · <title>` head, a row in one of the `registries`. `build` and
`update` read those definitions off the documents they walk, and the graph they write is where the
set is recorded. No reader builds a list of its own: every id is read by one grammar, and which
prefixes are families is a question asked of the graph after every file is read rather than of each
line while it is scanned. Nothing is configured, nothing is stored beside the graph, and there is
nothing to keep in step with anything.

Why derived rather than configured or pinned beside the graph is in
[the measurements](docs/history.md#id-families-why-they-are-derived-and-not-configured).

Every id is matched as `FAMILY-<1–4 digits>`, hyphen included, or `FAMILY-M<2 digits>` for a
milestone, whatever the prefix; hyphenless labels such as `B1` or `S3` are not ids in any repository
and cannot become families. A citation of a prefix no line defines — `ISO-8601`, a ticket number, a
year — is found like any other and then held aside: no reader follows it, no count reports it, and
`verify` says how many are waiting and under which prefixes. Writing a line that defines the prefix
is how it crosses that line, and the next `repograph update` releases what was held into the graph.

```bash
repograph families                          # families, milestones, and everything left as text
```

Run after a build, it prints every family with its node count, how many definitions stand behind it
and the `path:line` its first node stands at, then every id-like prefix no line defines, with how
often, under how many distinct ids, and where it is written. Two of those columns exist to put the
report's own edge first: `defs` counts declaring-file × id pairs — summed over the family's nodes,
so a node two documents declare counts twice — the rows sort by it ascending, and
a family with exactly one — one line, which may have been a mistake — reads `defined once`; `ids`
counts the distinct ids written under an undefined prefix, and the mention half sorts by it, because
twenty ids nobody defines is a vocabulary where twenty mentions of one id is a citation repeated.
Both are in `families --json` too, beside the fields that were already there. Both
halves are read off the graph and the tree beside it, so a definition edited away since the last
update still shows the line it was read from, and the command says on stderr how many files the
store is behind. Nothing is written to the repository.

An `update` whose documents gained or lost a family says so — `families: +REQ`, `families: -AC` —
and re-reads nothing else: the one file that changed is read, and the citations the graph was
holding for the new family are released where they lie. A resident `serve` or `watch` says the same
on the poll that applies it, and `ask`'s own refresh never costs a pass over the whole corpus. A
`repograph.toml` still naming `id_families` parses, gets one line on stderr, and is otherwise
unaffected.

## Embeddings

Dense retrieval embeds by default with `intfloat/multilingual-e5-small` (384-d, ONNX, ≈470 MB on
disk) run through `ort` directly: the tokenizer and the session open concurrently at optimisation
level 1,
which halves model-open time against the library default. The files are a one-time Hugging Face
download cached under `FASTEMBED_CACHE_DIR` if that is set, else `~/.cache/repograph/fastembed`
(`%USERPROFILE%\.cache\repograph\fastembed` on Windows; the layout is the hub client's, so a
cache populated by an earlier release is reused as is — where the client links each file into its
snapshot, or on a Windows account without symlink rights moves it there). Every
command that touches the dense stage — `build`, `update`, `enrich`, `embed`, `watch`, `ask`,
`bench`, `dump`, `serve` — reuses the cache; there are no further network calls once it is
populated. `--no-dense` skips the download and the embedding stage everywhere.

A rebuild can add nodes the questions do not cover — a document that grew, or a corpus that started
defining a family it only cited before — and `enrich` has never seen those. `build` and `update`
print how many requirement-like nodes are without questions whenever the store has questions for
some others, so a rebuild ends by naming its own next step: `repograph enrich`. A store nobody has
enriched prints nothing, because there is nothing to say.

The model is a property of the store. `embed_model` names what `build`, `update`, `enrich`, `embed`
and `watch` write vectors with; `vectors.json` records it, and `ask`, `bench` and `dump` open the
recorded one — so a store keeps answering with the model that wrote it whatever the configuration
says today, and a new default never silently reinterprets an index nobody re-embedded. A store
written before the field existed is the small model's. Switching is one line and one `repograph
embed`: rows another model wrote are dropped and the file rewritten, on width as well as on name.
`REPOGRAPH_EMBED_MODEL=<hub id>` outranks both for one command, which is how a copy of a store is
measured under a second model — query that copy with `ask --stale`, or with `bench` and `dump`,
which read the store as it stands; a refreshing `ask` would claim the index for the overriding
model. It is trap 7 of the [runbook](docs/bench/runbook.md).

`--no-dense` turns the dense stage off everywhere; on the default model that costs two hits of
eighty-two ([the arithmetic](docs/history.md#what-turning-the-dense-stage-off-costs)).

`ask` opens the model only when a fused query needs it, and that open is most of what a fused
answer costs: ~0.30 s and ~1.7 GB on the default — the model, not
the graph. An exact-id lookup answers in ~50 ms and ~50 MB, a `--no-dense` question in ~0.1 s, since
neither opens the model or reads the vectors. What is left is paid once per process, which is what
[`serve`](#asking-a-resident-process) is for. `REPOGRAPH_TIMING=1` prints the stages.

Five other embedding-side levers were measured and none moved recall — a larger base model, BGE-M3,
a quantized MiniLM, a 512-token passage cut, a second vector per node
([the readings](docs/history.md#five-embedding-side-levers-all-rejected)). The passage cut stays at
256 tokens and a node keeps one vector.

If the model cannot be opened (no cache, no network) the two kinds of caller degrade differently on
purpose: `ask` and `update` fall back to lexical-only, say so on stderr and exit 0, while `bench` in
dense mode fails outright — its only output is an exit code, and a silent fallback graded against
the weaker no-dense floor would report green without having measured what it claims to.

Embedding times, and what a rebuild reuses rather than pays for twice, are in
[the measurements](docs/history.md#the-first-embedding-pass-and-what-a-rebuild-reuses).

## Spending tokens on purpose

Everything above runs at zero model tokens, and stays that way by default. Two stages can spend
them, each behind an explicit switch, each measured on the development corpus:

**`repograph enrich`** asks a model, once per requirement-like node, for twelve questions a reader
might ask to find that node in everyday words, plus a line of synonyms. Those questions are embedded
as rows of their own for the reranker's pool and indexed for BM25 as a list of their own in every
answer. Generation is cached by passage hash in `.repograph/questions.json`, so a later `enrich`
pays only for nodes whose text changed, and a node left without questions is asked again by the next
run. Two kinds of drift are refused on the way in and cleaned out of an older cache: a line whose
letters are mostly neither Cyrillic nor Latin, and several questions tab-joined around the node's
own id. On the corpus of 2026-09-02, 1,971 eligible nodes took 16 minutes at 8-way parallelism and
roughly $2.5 of haiku; the corpus is 1,996 eligible nodes now.

`enrich --code` extends the pass to symbols with a doc comment or a body of their own and files
with a head comment — 3,475 nodes on the corpus — through a prompt that asks four Russian and four
English questions per node and forbids repeating the identifier. Those code questions are an index
of their own, searched for the `--rerank` pool and nowhere else: a store carrying them fuses byte
for byte like a store without them. Why they are kept out of the plain fusion was measured three
times and is [recorded](docs/history.md#enrich---code-the-fifth-list-and-why-it-stays-out-of-the-plain-fusion).

The documents' generated questions join the fusion as a BM25 list of their own, and only when that
list has earned its turn: each list is asked what fraction of the query its best document actually
reached, and the questions list is admitted at 0.761 of the passage list's coverage. Two other ways
of spending those questions were measured and rejected. The constant, the two rejections and what
the admission moved are
[recorded](docs/history.md#three-ways-to-spend-the-generated-questions-and-the-one-that-ships).

**`ask --rerank`** builds a 200-deep pool — dense passages, dense questions, BM25 passages, BM25
questions, and, when the store carries questions about code, those as a fifth list, interleaved —
and hands the model each candidate's id, title and the first 120 characters of its text to pick five
from; `--depth` changes how deep, and tokens per question scale with it. `rerank_command` reads the
prompt on stdin and writes the chosen ids one per line; a failing command is reported on stderr and
the answer falls back to the fused order.

What the model is shown decides more than which model it is, and every model read in that seat is
[recorded](docs/history.md#what-the-reranker-is-shown-and-every-model-read-in-that-seat). Where the
arm stands today, on the 82 cases the floors use:

| | paraphrase | keyword | code | p90 tokens | per question |
| --- | --- | --- | --- | --- | --- |
| `ask` | 15/30 | 40/40 | 12/12 | 220 | 0 tokens, ~0.30 s |
| `--rerank`, sonnet, depth 200, 2 runs | 29/30 | 40/40 | 12/12 | 228–231 | 58,314 B median prompt (metered), ~4.3 s, ≈$0.03 |

Fourteen paraphrase cases gained, none lost, and both runs picked identically down to the one
chronic miss, `FR-MKT-35` — which the [rerank diagnostics](docs/bench/2026-09-09-rerank-diagnostics.md)
found is not in the 200-deep pool at all. That document also has the pool rank of every gained case,
which is what says whether a zero-token lever could reach it.

**`--rerank-local`** is the same pool and the same pick, scored by a local cross-encoder
(`BAAI/bge-reranker-v2-m3`, exported once with `optimum-cli export onnx --model
BAAI/bge-reranker-v2-m3 --task text-classification ~/.cache/repograph/reranker`, ~2.2 GB) at
zero tokens — and **measured and rejected** as a floor candidate on 2026-09-04: 17.9 seconds a
question against a bar of one, and keyword 39/40 on the same 82 cases.

Both flags ship opt-in and on no floor: a model's pick can vary by one hit between identical runs,
so `--rerank` is measured and never graded ([why](docs/history.md#why---rerank-is-measured-and-never-floored)).
Passing both is an error rather than a silent preference for one.

## Bench

`repograph bench [--cases file]` runs the recorded 82 cases (40 keyword + 30 paraphrase + 12 code)
against a built graph and fails the process if any floor is missed. The recorded `bench/cases.jsonl`
is compiled into the binary, so a release build benches from any directory; `--cases` substitutes
any other file — one of the recorded 40/30/12 shape is graded against the floors below, any other
shape is measured and reported with `gated=false`, the way `bench --cases bench/dev-cases.jsonl` is
used throughout [the runbook](docs/bench/runbook.md).

The floors are two sets, not one, because [`enrich`](#spending-tokens-on-purpose) is optional and
paraphrase recall is what it buys. `bench` reads which state the store is in and says so on its
summary line (`dense=true  enriched=true (1996/1996 nodes) model=small`): a store carrying questions
on at least 99% of its requirement-like nodes is graded against the enriched floors, anything else
against the raw ones. The bar is a high-water mark rather than every node because equality over
~2,000 nodes is a cliff — one node the model skipped would regrade a paid-for store five paraphrase
points lower, and `bench` would say so through its exit code alone.

The dense floors are keyed by the store's embedder too, since a floor measured on one model says
nothing about another: small-model rows (or rows under no name) are graded against the small model's
numbers, and rows under any other model are measured and never graded. The lexical arms
have no embedder in them and keep one set whatever the rows are. The summary line also carries
`code_questions=<covered>/<eligible>` on a store that carries questions about code, which are
searched for the `--rerank` pool rather than in the fusion the floors measure. Beneath it, on a line
of its own, `anchors  <kind> <reached>/<wanted> …` says how much of each answer was reached, not only
whether it was: a case that keeps its verdict and loses two of its three anchors moves that line and
nothing else. `bench --repeat N` runs the suite N times, judges every run on the floors, and prints
a median beneath them.

The exit status says which of three things happened, so a harness never has to read the sentence on
stderr: **0** — the suite answered and every floor was met; **3** — the suite answered and a floor
was missed, which is a verdict on the answers and a reading of the reader; **1** — nothing was
measured, which is an empty graph, a case file that does not parse, a built-in case set of the wrong
shape, or a dense width mismatch. The verdict is **3** and not 2 because 2 is written above the
command and says nothing about a suite: `clap` exits 2 on a usage error, and `npm`'s launcher exits
2 when no platform binary is installed. A `--repeat` run takes the worst of its runs: every run has
to meet the floors, not the median of them.

| | enriched store | store with no questions |
| --- | --- | --- |
| keyword | 40/40 with embeddings, 39/40 with `--no-dense` | 40/40 with embeddings, 39/40 with `--no-dense` |
| paraphrase, small-model rows (the default) | ≥14/30 with embeddings, ≥11/30 with `--no-dense` | ≥9/30 with embeddings, ≥7/30 with `--no-dense` |
| code | 12/12 | 12/12 |
| p90 | ≤230 tokens in every arm | ≤230 tokens in every arm |

The `--no-dense` column applies to every store, since no embedder is in it.

Where each of those floors came from, why keyword is 39 and not 40 in the lexical arms, how the p90
is counted, and the 400-question held-out set a retrieval change has to clear before the 82 cases
are consulted, are in [the measurements](docs/history.md#the-bench-floors-case-by-case).

## Measured

The bench fixture is 908 files and about 8.3k nodes. On the recorded 82 cases, both arms run twice
with identical results: **keyword 40/40, paraphrase 15/30, code 12/12 at 220 p90 tokens** with
embeddings, and **39/40, 14/30, 12/12 at 215 p90** with `--no-dense`, both green. Those are the
numbers with `enrich`'s generated questions in the store — the one thing paid for, roughly $2.5 of
haiku, once. The same corpus at zero tokens throughout reads 40/40, 9/30, 12/12 at 221 and 39/40,
7/30, 12/12 at 226; [Bench](#bench) floors each state on its own numbers.

The graph itself costs nothing to build: 7,525 nodes and 27,412 edges on the corpus of 2026-09-02,
10.2 MB on disk, ~19 ms to load, zero model tokens. The full snapshot, the prior art it replaced and
what that cost are in [the measurements](docs/history.md#the-corpus-as-it-was-measured-and-the-prior-art).

### Resources

Every reader costs a fraction of a second and the model it opened. The one command that can take a
machine over is a writer that has to embed a store whole — `build`, `update`, `enrich`, `embed` or
`watch` on rows another model wrote. One word bounds it:

| `resources` | threads | wall | peak CPU (mean) |
| --- | --- | --- | --- |
| `"full"` | 6 | 168.8 s | 382% (335%) |
| `"balanced"` *(the default)* | 4 | 265.7 s | 275% (218%) |
| `"low"` | 2 | 358.7 s | 140% (127%) |

The fixture's 33,525 rows under the default model on a twelve-core Apple Silicon machine
([2026-09-09](docs/bench/2026-09-09-normal-band-only-results.md)). The rule is fractions of the
logical cores with one thread as the floor: `full` a half, `balanced` a third, `low` a sixth — so
the level follows the machine, and a container gets a fraction of its quota rather than of the
host. Set it in `repograph.toml`, once for the machine in `~/.config/repograph/config.toml`, or
`REPOGRAPH_RESOURCES=full` for one run. A build server wants `full`; a laptop you are working on
wants `low`.

`resources` is the only resource lever there is: `threads` and `priority` were removed on
2026-09-09, and `full` is now both the most of the machine you can ask for and the fastest setting
the tool has. A long embed checkpoints every 1,024 rows and says where it is, so an interrupt
resumes rather than restarts. What a rebuild costs the person beside it, why `full` is half the
logical cores rather than no cap, and what the removed scheduling band bought and cost are in
[the measurements](docs/history.md#what-a-rebuild-costs-the-machine).

## Design

The full design note and implementation plan are in
[`docs/superpowers/specs/2026-09-01-repograph-design.md`](docs/superpowers/specs/2026-09-01-repograph-design.md)
and [`docs/superpowers/plans/2026-09-01-repograph.md`](docs/superpowers/plans/2026-09-01-repograph.md).
The plan's deviations table lists four simplifications against the spec, none of which change the
node/edge model or the answer shape; one of them moved a measured number, and
[ADR-001](docs/adr/ADR-001-paraphrase-recall-was-a-prediction.md) says which.

### Where the rest of it lives

This file is what you need in order to run the tool. The arguments behind it are beside it:

| | |
| --- | --- |
| [the graph model](docs/graph-model.md) | node and edge kinds, what the extractors take, how a question becomes an answer, what `impact` and `changes` can and cannot prove |
| [the measurements](docs/history.md) | every reading that decided a default: the embedder, the fusion, the floors, the reranker, what a rebuild costs |
| [running on Windows](docs/windows.md) | the platform floor, SmartScreen, Defender, PowerShell |
| [`docs/bench/`](docs/bench/) | the campaign documents, the runbook, and the open gap ledger |
| [`docs/adr/`](docs/adr/) | the two decisions that outlived their arguments |

## License

MIT.
