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

Measured on the same corpus and the same 38 keyword/paraphrase questions against `graphify`, the
LLM-extracted graph it replaces. That head-to-head is the case set as it stood then; the recorded
set has since grown to 82 cases, which the [Bench](#bench) floors are measured on:

|                           | graphify (the incumbent) | repograph           |
| ------------------------- | ------------------------ | ------------------- |
| paraphrase questions      | 0/14                     | 7/14                |
| keyword questions         | 11/24                    | 24/24               |
| tokens per answer         | 1027-1555                | 197 median, 216 p90 |
| tokens to build the graph | 14,597,195               | 0                   |

Every number above came from running both tools; none is a target. See [Bench](#bench) for the full
floor set and how it was recorded, and
[`docs/adr/ADR-001-paraphrase-recall-was-a-prediction.md`](docs/adr/ADR-001-paraphrase-recall-was-a-prediction.md)
for the one number in this project's history that travelled from a design note into a plan as though
it had been measured, and hadn't been.

## Status

0.5.0 is the version `main` carries; every row below is implemented rather than planned:

| Command                    | State                                                                 |
| -------------------------- | --------------------------------------------------------------------- |
| `build`, `update`          | working; incremental; a no-op `update` is a fixed point               |
| `families`                 | working; reads the built store and the documents, and says which families they define, where each was first defined, how many nodes it holds, and which id-like prefixes were left as text. Nothing to configure and nothing stored — see [Configure](#configure) |
| `ask`, `explain`, `verify` | working: exact id/symbol → BM25 → dense, fused, one hop out           |
| `impact`, `trace`, `changes` | working: callers by depth through barrels, shortest call chain, the diff mapped onto symbols — see [Blast radius](#blast-radius) |
| `bench`                    | working; fails the process if a floor in [Bench](#bench) is missed — floors are keyed by enrichment (a store with `enrich`'s questions and one without) and by the dense embedder (small model, large model); a store under any other model is measured and not graded |
| `import-legacy`            | working; costs recall at query time — see its note in [Bench](#bench) |
| `enrich`, `ask --rerank`   | working; opt-in, the only two stages that spend model tokens — see [Spending tokens on purpose](#spending-tokens-on-purpose) |
| `ask --rerank-local`       | working; opt-in, the same pool picked by a local cross-encoder at zero tokens, measured and rejected as a floor candidate — see [Spending tokens on purpose](#spending-tokens-on-purpose) |
| `embed`                    | working; writes the rows the dense index lacks with the configured model, rewriting it whole when the store was written by another — see [Embeddings](#embeddings) |
| `serve`                    | working; opt-in resident answerer for `ask` — the same bytes, measured 66 ms per fused ask after the first against 326 ms in a fresh process, and 6.8 ms in the lexical arm since the BM25 indexes stopped being rebuilt per question — see [Asking a resident process](#asking-a-resident-process) |

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

The floor is Windows 10 1903, Windows 11 or Server 2022. The ONNX Runtime the Windows binary links
is pyke's DirectML build, which imports DirectML and DirectX 12 at load; those libraries are inbox
from 1903 on, and an older system fails at load rather than at a query. No GPU is used: no execution
provider is registered, so inference runs on the CPU — a GPU-less runner builds a store with
vectors and answers a fused question through a resident `serve`
([run](https://github.com/devmaxxx/repograph/actions/runs/34064881516)).

The binary is not signed. A zip fetched with a browser carries the mark of the web, Explorer's
"Extract All" passes it to the exe, and double-clicking such a copy shows SmartScreen; running it
from a terminal does not, and `Unblock-File .\repograph.exe` clears the mark for good. `gh run
download`, `curl` and npm write no mark at all. Defender scans the model as it lands — 470 MB for
the default `multilingual-e5-small`, 2.2 GB where `embed_model` names the large one — and every
`graph.json` and `vectors.f32` rewrite on close;
`Add-MpPreference -ExclusionPath` on `%USERPROFILE%\.cache\repograph` and on the repository's
`.repograph` is an optional speed-up, not a requirement. An unsigned Rust binary can also draw a
heuristic false positive: restore it from Protection History and add an exclusion.

PowerShell has no `&` job operator, so a resident server is started with
`Start-Process repograph -ArgumentList 'serve','--idle','86400' -WindowStyle Hidden`; `--idle` ends
it, and so does `Stop-Process -Name repograph`. Neither runs the exit that removes the socket file,
which is what the next `serve` sweeps before binding. Stop the server before replacing the binary:
`npm i -g`, `cargo install` and `Expand-Archive -Force` all fail with a sharing error against a
running image.

Keep the repository out of a OneDrive, Dropbox or Google Drive tree. `.repograph` is per machine
and worth nothing to another one, every store write is a rename the client sees as a new file to
upload, and `serve.sock` is a reparse point of a tag no sync client knows — what a given client
does with one is not verified here. A rename over a store file another program holds open is
waited out for about half a second before it is reported, which is what a scanner or an
indexer costs.

Console output is UTF-8. Windows Terminal renders it, and so does Claude Code's Bash tool; but
PowerShell decodes a *piped or captured* native command's output with `[Console]::OutputEncoding`,
which on a Russian-locale system is code page 866 unless the system UTF-8 option or a profile line
sets it otherwise — `repograph ask … | Out-File` and `$x = repograph ask …` are where a Cyrillic
answer turns to mojibake, not the screen. Environment paths must be Windows-form even when they
are set inside Git Bash: `FASTEMBED_CACHE_DIR`, `XDG_CONFIG_HOME` and `REPOGRAPH_CONFIG` reach a
native exe as written, and MSYS converts arguments, not arbitrary values. For a tree deeper than
260 characters, git itself needs `core.longpaths=true` to check it out; repograph follows it from
there.

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
| `verify --json` | `nodes`, `edges`, `nodes_by_kind`, `edges_by_kind`, `dangling`, `undeclared`, `gaps`, `cite_only` |
| `trace --json` | `from`, `to`, `depth`, `path` |
| `prime --json` | `nodes`, `edges`, `enriched`, `questions`, `families`, `model` |

`explain --json` resolves each edge's direction for you — `dir` is `in` or `out` and `other` is the
node at the far end — so a caller never works out which end of an edge it was standing on. One
difference from the text forms is deliberate: a `trace` that finds no path within the depth is an
answer to the question that was asked, so the JSON form prints `"path": null` and exits 0 where the
text form exits non-zero. A caller parsing an object should not have to read an exit code to learn
what the object already says.

### Keeping it fresh

`ask` walks the tree before it answers. Anything edited since the last build is re-extracted in
process and saved, so the graph is never behind the working copy and no `update` has to be
remembered. One line goes to stderr when that happens:

```
refresh: 3 changed, 1 removed
```

A fused query opens the embedding model anyway, so the rows that changed are re-embedded and the
vectors stay in step too. `--no-dense` and the exact-id path open nothing: the lexical graph is
fresh, and the vectors catch up on the next fused query or `update`. Measured on the development
corpus (825 files, 7.5k nodes): the no-change check costs ~10 ms, and a one-file edit costs ~20 ms
lexical, ~40 ms with the re-embedding — the new rows are appended to `vectors.f32` and the rows
they replace are left as holes, so nothing rewrites 50 MB to store a row of 1.5 kB. Holes past a
quarter of the live rows are compacted away by the next refresh, which does rewrite both files.
`REPOGRAPH_TIMING=1` prints the stages.

```bash
repograph ask --stale отмена записи         # answer from the store as it stands, no check
```

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
alone costs about 220 ms of it on the shipped default, 676 ms on `e5-large`. `serve` opens
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

`ask` uses it without being told to, and answers in this process whenever it cannot: no socket, a
socket nobody listens on, a server of another version, a server built from other code under the
same version (the handshake carries the executable's own mtime and size, so a second copy of one
build counts as another), a server started `--no-dense` asked a fused question, a timeout,
`--no-serve`, or `REPOGRAPH_NO_SERVE` in the environment. Each of those prints a line: a question
that quietly costs a cold process, or quietly gets a lexical answer, looks like nothing at all. The
other arm pairing is not a mismatch — a server holding the model answers `--no-dense` lexically,
which is what was asked for. A client never deletes the socket file — a refused connect is also
what a live server with a full backlog gives — so only `serve` removes one, and only the one it
bound itself: told from a replacement's by the socket file's device and inode on unix, and by
NTFS's file reference number on Windows. A server killed outright removes nothing, and the next
one removes the file it left before binding; until then that file at worst costs a client one
refused connect.

The socket path is limited to 108 bytes on Linux and Windows and 104 on macOS; the bytes are UTF-8,
so a Cyrillic user name costs two a letter, and
`C:\Users\Максим\OneDrive - <company>\Documents\projects\beauty-crm\.repograph\serve.sock` is about
100 of the 107 a path may use. A repository deep enough to exceed it cannot start `serve`, and `ask` answers in its own process as it would with no
server at all. On a store neither `enrich` nor `embed` has moved under it, and a server of this
build serving the arm asked for, the answer is the same bytes either way; that is checked on all
142 recorded and developer bench questions, in every pairing of the server's arm with the
client's.

The server refreshes before every answer with the same walk a one-shot `ask` does, and polls
between them like `watch`, so its **graph** is never staler than a fresh process's. Under
`--stale` it skips that walk, as a one-shot does, but still reads the graph back when another
process has written it: `--stale` means the stored graph as it is on disk, resident or not, and a
`--stale` question embeds nothing and writes nothing either way. It answers one question at a
time; a second client waits for the first rather than being turned away.

Both of those reads are decided by `manifest.json`, so a running server does not see the two
commands that leave it alone: `enrich`, which writes `questions.json`, and `embed`, which writes
`vectors.*`. Run either against a repo with a `serve` on it and the resident answers keep the
questions and the vectors they started with — stop the server, or ask with `--no-serve`, to answer
from the new ones.

The configuration is read once, at start-up. Editing `repograph.toml` stops the server after its
next poll — the next `ask` answers in its own process under the new file, and a new `serve` starts
under it too.

Measured on the bench corpus (908 files, 8.3k nodes, enriched), median of eleven, socket and
in-process runs interleaved in one sitting, except where noted:

| | resident | one process |
| --- | --- | --- |
| fused question, dense | 66 ms | 326 ms |
| lexical, indexes rebuilt per question | 54 ms | 106 ms |
| lexical, indexes built once and kept (0.5.0) | **6.8 ms**\* | not re-measured |

\* A different sitting, median of 33 (three blocks of eleven, not the row above's eleven) against a
pre-change base, two fix commits before the binary that shipped. The shipped binary's own socket
median is 6.5 ms.

The first question after a start still pays the model open — 0.26 s, against 0.07 s for the ones
after it. What is left is a process start (5 ms), the socket round trip and the answer itself. The
BM25 indexes are no longer part of that: until 0.5.0 `ask` rebuilt them from the graph on every
question, which was almost all of the remaining 49 ms of the lexical arm's 54 and the one expensive
thing a resident process did not keep. A resident context now builds them on its first fusing answer
and keeps them until the context takes up a moved store, and that arm reads 55.0 ms against 6.8 ms —
medians of 33 over base and head binaries alternating in one sitting, base spread 0.9 ms, which is
the 30 ms this was aimed at. That is its own sitting on the same copy rather than a before-and-after
of the table above, and the one-process column was not re-measured. The stage tables, the levers
behind these numbers and the evidence that the bytes do not move are in
[the perf results](docs/bench/2026-09-06-perf-results.md) and
[the 0.5.0 gap results](docs/bench/2026-09-05-0.5.0-gaps-results.md).

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

Check the graph's health — counts by kind, dangling edges, and ids that are referenced but never
declared, split into gaps inside a declared family (worth chasing) and families that are only ever
cited, such as milestone task ids named from code:

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

## How a question becomes an answer

1. **Exact.** A word that is a known id, or the name of an indexed symbol, wins outright and scores
   above everything else. When every word is an id or a name with an uppercase letter (`asGrosze`,
   `ZERO`), the exact hits are the whole answer; a lowercase word that happens to be a symbol too
   (`money` is a test helper) leads, and the fused retrievers fill the remaining seeds.
2. **Lexical.** BM25 over `id + label + body` for every node, with Snowball stemming — Russian for
   Cyrillic tokens, English otherwise, so `штрафа` and `штрафы` are the same term. Ids survive
   tokenization whole, so `FR-PAY-22` never becomes three tokens.
3. **Dense.** A local embedding of the query, cosine-ranked against every node's stored vector
   (see [Embeddings](#embeddings)). Skipped by `--no-dense`, or when the model cannot be opened —
   no cache and no network (see [Embeddings](#embeddings) for the fallback rules).
4. **Fuse.** The lists are interleaved — rank 1 of each, then rank 2 of each — dense passages
   first, then BM25 over the generated questions (when `enrich` has written any), then BM25 over
   the passages, and the top seeds survive (with `--rerank`, a model picks them from a 200-deep
   pool instead — see [Spending tokens on purpose](#spending-tokens-on-purpose)). Reciprocal rank
   fusion was measured to bury a retriever's second hit under ids both lists merely agreed on; the
   interleave lifted paraphrase recall from 5/14 to 6/14 at +2 tokens p90. The question list was
   measured on 400 held-out generated questions: recall@5 0.445 → 0.515 beside the dense list and
   0.395 → 0.527 without it (exact McNemar p < 0.001 both), keyword and code cases unchanged, the
   no-dense paraphrase cases 3/14 → 5/14, at +3 tokens p90 with embeddings and +15 without — all
   of that on the 14-case set of the day. On the 82 cases recorded since, leading the merge with
   that list cost the no-dense arm two exact seeds — keyword 39/40 without the questions against
   37/40 with them — until 2026-09-05, when the questions list was gated on its own confidence;
   it now reads 39/40 either way. See [Spending tokens on purpose](#spending-tokens-on-purpose)
   and the Bench table below.
5. **Expand.** One hop over `References`, `Implements`, `Declares`, `Links` and `Legacy` edges, in
   both directions, keeping the single neighbour the retrievers ranked best, however far down
   their lists; a neighbour no retriever ranked falls back to its seed's rank. Measured on 400
   held-out generated questions, that choice reads 226 hits against 208 for the seed's rank
   alone (one lost, nineteen gained) and 8/14 against 7/14 on the paraphrase cases, for the same
   one line of output. A second expanded line measured +1 hit per
   extra neighbour against ~+90 tokens per answer. `File` nodes and decorator nodes are never
   expanded _to_ — they are hubs and would drown the answer.
6. **Render.** `ID  path:line  headline`, headline cut to 80 characters.

The lexical index is built in memory rather than stored on disk, and since 0.5.0 it is built once
per context rather than once per question: a one-shot `ask` pays one build, and a resident `serve`
pays one on the first answer that fuses and then keeps it. Nothing lexical is on disk, so there is
no stored lexical state to go stale; the copy a context holds is exactly what can drift from the
graph or the questions, which is why it is dropped rather than refreshed whenever the context takes
up a moved store — not on a `questions.json` rewrite alone. The
build costs about 120 ms on a 7,500-node graph. A later sitting bounds that build, the question
index beside it, their scoring and the fusion at about 49 ms of a 54 ms lexical ask on the bench
corpus at 8.3k nodes, and a later one still reads the same socket answer at 6.8 ms once the indexes
are kept — different sittings on different graphs, not a before and after, on the two documents
cited above.

## What ends up in the graph

**Node kinds:**

| Kind            | What becomes one                                                              |
| --------------- | ----------------------------------------------------------------------------- |
| `Requirement`   | a `<ID> · MUST\|SHOULD\|LATER · title` line, in either dialect; a `· title — MUST` tail is the modality, not the title, and what follows a bold head's closing `**` opens the body |
| `Entity`        | a backticked name inside a requirement's title                                |
| `Invariant`     | an `INV-*` requirement, or a row in a `constitution.yaml`-shaped registry     |
| `Adr`           | an `ADR-*` requirement, or the whole document of a file named after an ADR id |
| `Milestone`     | a `<PREFIX>-M##` requirement, or the whole document of a milestone file       |
| `Task`          | a `- [ ] **T##** …` checklist line inside a milestone file                    |
| `File`          | one per indexed file; owns ids that occur outside any block                   |
| `Symbol`        | a top-level export, class, method, or decorated class member                  |
| `LegacyConcept` | an `import-legacy` node that resolution could not tie to a real node          |

**Edge kinds:**

| Kind          | What creates one                                                                                                            |
| ------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `References`  | an id or backticked entity in prose, a title, a body, a registry row's `basis`, or an id quoted in a code comment or string |
| `Declares`    | a file, milestone, or registry row that owns a node                                                                         |
| `Links`       | a markdown link between two files                                                                                           |
| `Implements`  | a task or registry row and the requirement or gate its text names                                                           |
| `Imports`     | a resolved TypeScript import                                                                                                |
| `ReExports`   | a barrel `export * from`                                                                                                    |
| `Extends`     | a class's `extends` clause                                                                                                  |
| `DecoratedBy` | a decorator application, its first string argument as context                                                               |
| `Legacy`      | an edge carried over by `import-legacy`                                                                                     |
| `Calls`       | a call or `new` whose callee the file can prove: an imported name, a top-level declaration of the same file, `this.member()`, `Static.member()`, or `this.field.member()` through the field's declared type (constructor parameter properties included); a call through a barrel targets the barrel and is resolved by `impact` |

An edge is unique on `(source, target, kind, context, file)` — `file` is part of the key on purpose,
so a relationship that two different files both assert is recorded twice and survives either one
being edited.

From documents it takes requirement blocks in both `**ID · MUST · title**` and `### ID · MUST · title`
forms, ids referenced in prose (including ranges like `FR-RPT-42…48` and slash lists like
`INV-11/12/20`), backticked entity names, markdown links, and `constitution.yaml`-shaped registries.

From TypeScript it takes a `Symbol` per top-level declaration — exported or not, `declare`d,
destructured, overloaded, a namespace or an enum — and per class member, quoted and computed
names included; imports resolved through relative paths, `tsconfig` `paths` (with `baseUrl`) and
`package.json` `exports` (wildcard subpaths included, `main`/`types` as the fallback), plus
`import()` and `require()` calls; decorators with their first string argument as context; and
every id quoted in a comment or string literal, attributed to the top-level function, class
member, `const`, interface or enum that contains it. That last layer is the doc↔code bridge an
AST-only indexer misses entirely. Each construct is pinned by one inline case in
`src/code/cases.rs`; `.claude/skills/extractor-case/` is the loop for adding the next one.

## Blast radius

`impact <symbol>` walks `Calls` and `Extends` edges towards the symbol: `d=1` are the direct
callers ("will break"), `d=2` their callers, and so on to `--depth` (3). A class is walked
through its members, and a caller that imported through a barrel is found because the barrel's
`ReExports` edges are followed back to the declaration. The barrel itself is listed among the
importers: it names the symbol, and a rename reaches it first. `importers` are the files whose
`import` names the symbol, whether or not a call site resolved. The risk line is four fixed thresholds
on the direct count and the file count — `MEDIUM` from 5 direct or 3 files, `HIGH` from 15 or
10, `CRITICAL` from 30 or 25 — printed with the counts, so the label can be argued with. Barrels
count towards the file threshold, so a symbol re-exported by three barrels and called by nobody
now reads `MEDIUM`: the counts beside the label are what say whether that is a real blast radius
or a re-export chain.

```
$ repograph --repo beauty-crm impact StaffService
sym:apps/api/src/modules/staff/staff.service.ts::StaffService  apps/api/src/modules/staff/staff.service.ts:19
d=1  will break (3)
  file:apps/api/test/staffMembership.spec.ts  apps/api/test/staffMembership.spec.ts:1  Calls → sym:apps/api/src/modules/staff/staff.service.ts::StaffService
  sym:apps/api/src/modules/staff/staff.controller.ts::MembershipController.memberships  apps/api/src/modules/staff/staff.controller.ts:56  Calls → sym:apps/api/src/modules/staff/staff.service.ts::StaffService.memberships
  sym:apps/api/src/modules/staff/staff.controller.ts::StaffController.create  apps/api/src/modules/staff/staff.controller.ts:87  Calls → sym:apps/api/src/modules/staff/staff.service.ts::StaffService.create
importers (3): apps/api/src/modules/staff/staff.controller.ts, apps/api/src/modules/staff/staff.module.ts, apps/api/test/staffMembership.spec.ts
risk: MEDIUM — 3 direct, 3 total, 3 files
```

`--down` walks the other way; `trace <from> <to>` is the shortest chain between two symbols.
What the graph cannot prove it does not list: a call through a chained expression, a
destructured method, a callback parameter or a global has no edge, so confirm a "nothing uses
this" with `rg -l` before deleting. A target the graph knows only by name — a member of an
imported value it never saw declared — prints `?` in place of its `path:line`.

`changes` maps `git diff -U0` (staged and unstaged, plus untracked files whole) onto symbol
spans and unions the callers of every touched symbol into one list and one risk line. Run it
before committing; `--base main` before opening a pull request. A hunk outside every symbol —
an import line, a trailing comment — is reported on the file and walks every symbol the file
declares; a hunk in a file the graph does not index at all — a `.kt`, a `.sql`, a lockfile — is
listed as that file with `not indexed` in place of a span, so the answer says the file changed
rather than nothing; what is being changed is never listed as affected by itself. Deleted files do not
appear: their symbols are gone from the graph, and their former callers surface as dangling
edges in `verify`.

```
$ repograph --repo beauty-crm changes
changed: 2 symbols in 1 file
  file:apps/api/src/modules/staff/staff.service.ts  apps/api/src/modules/staff/staff.service.ts:1
  sym:apps/api/src/modules/staff/staff.service.ts::StaffService.create  apps/api/src/modules/staff/staff.service.ts:31-43
affected (depth 2): 3 symbols in 3 files
  d=1  file:apps/api/test/staffMembership.spec.ts  apps/api/test/staffMembership.spec.ts:1  ← sym:apps/api/src/modules/staff/staff.service.ts::StaffService
  d=1  sym:apps/api/src/modules/staff/staff.controller.ts::MembershipController.memberships  apps/api/src/modules/staff/staff.controller.ts:56  ← sym:apps/api/src/modules/staff/staff.service.ts::StaffService.memberships
  d=1  sym:apps/api/src/modules/staff/staff.controller.ts::StaffController.create  apps/api/src/modules/staff/staff.controller.ts:87  ← sym:apps/api/src/modules/staff/staff.service.ts::StaffService.create
risk: MEDIUM — 3 direct, 3 total, 3 files
```

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

`enrich_command` and `rerank_command` are the transport — any program that reads a prompt on stdin.
Which model that program should run is a separate key, `enrich_model` and `rerank_model`, and it is
substituted into the command's `{model}`. Changing model is then a word rather than a rewritten
command line, and a command that names no `{model}` is run exactly as written, its model key unused.
Nothing here assumes a vendor: the names are whatever the configured command understands. Both run
under `sh -c`; on Windows that is Git for Windows' `sh`, taken from `PATH` when it is there and
otherwise found beside `git`, at the bash Claude Code names in `CLAUDE_CODE_GIT_BASH_PATH`, or under
Program Files, with Git's `usr\bin` put on the command's own `PATH` — nothing else in repograph
needs a shell. Without Git for Windows, `enrich` and `ask --rerank` refuse with a line that says
so, and everything else runs.

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

A key the repository names wins even when it names the built-in value: what the file says is what
that repository asked for — with two exceptions, and they run the other way. **`enrich_command` and
`rerank_command` are read from the machine file and never from a repository.** A repository you
cloned is untrusted input, and those two keys are a shell command that would run on your machine the
first time you ran `enrich` or `ask --rerank` in it; a `repograph.toml` that names one gets a line
on stderr saying where the key belongs, and the machine's command — or the built-in — is used. A
repository can still say which model it wants: `enrich_model` and `rerank_model` are substituted
into that command's `{model}`, and because that substitution lands in a shell string, a name that is
not a model name — anything outside letters, digits and `._:/@+-` — is refused the same way, with
the built-in name used instead. What a clone chooses is the model; what runs it is yours.

The machine file may set **only** `enrich_command`, `rerank_command`,
`enrich_model`, `rerank_model`, `reranker_dir` and `resources`, and refuses any other key by
name.
That refusal is deliberate rather than an omission: the corpus-shaped keys describe one
repository's documents, and a global `embed_model` in particular would rewrite every store's
vectors under a model nobody chose for it.

Two keys were removed on 2026-09-09 and neither is one now. `threads` is the one you are more
likely to have, because this file used to tell you to write it: replace `threads = 6` with
`resources = "full"` and `threads = 2` with `resources = "low"`. `priority` is the other: delete
the line, there is no scheduling band any more and the writers run in the normal one on every
platform. Either key left in a `repograph.toml` or a machine file is a hard error naming the file
and the key on every command that loads config, not a setting quietly ignored — both structs are
`deny_unknown_fields`, and a key that parsed and did nothing would leave you believing a number you
wrote is still being read. The same structs are `default`, which is why *adding* `resources` breaks
nothing: a file that does not name it behaves exactly as it did before. `REPOGRAPH_THREADS` and
`REPOGRAPH_PRIORITY` are simply no longer read — files are refused by name, and an unknown
environment variable has never been an error.


Three of those keys name a model and one names a directory, and they are four different jobs
rather than one preference. What each stage asks of a model, and what the answer was measured to
be:

| key | stage | what the model does there | what it has to be good at | measured | pick |
| --- | --- | --- | --- | --- | --- |
| `embed_model` | the dense index — `build`, `update`, `enrich`, `embed`, `watch` write with it, `ask` reads with what the store records | embeds every passage and every query. A sentence embedder named by its Hugging Face id, downloaded once and opened in-process through `ort`: not a command and not an LLM, so no Claude, GPT or local chat model can sit here, and an API embedder would need a transport this key does not have | putting a question and the sentence that answers it near each other, in Russian and English at once, over a 256-token passage | the default `intfloat/multilingual-e5-small` reads paraphrase 15/30 on the enriched fixture, ~0.30 s an `ask`, 470 MB on disk; `intfloat/multilingual-e5-large` reads 22/30 for ~0.8 s, 2.1 GB, and a first build measured in hours rather than minutes once the background band multiplies the embed | the small one, unless paraphrase recall is the job — [ADR-002](docs/adr/ADR-002-two-defaults-multiplied.md) weighs the two |
| `enrich_command` + `enrich_model` | `repograph enrich` | writes twelve everyday questions and a line of synonyms for each requirement-like node, as `id<TAB>question` lines, Russian and English together | being cheap over thousands of nodes, holding a strict line format across a batch, and asking in a reader's words rather than the document's | haiku reads paraphrase 15/30 and 5/9 of the developer suite's `rule` answers; a stronger model reads 17/30 and 2/9 — the register the questions are written in beats the model that writes them ([the G14 diagnostic](docs/bench/2026-09-06-g14-second-diagnostic.md)). 1,971 eligible nodes cost ~$2.5 and 16 minutes at 8-way parallelism; the corpus is 1,996 nodes now | the cheapest model that keeps the format — `haiku` |
| `rerank_command` + `rerank_model` | `ask --rerank` | picks up to five ids out of a 200-deep pool it is shown as `id<TAB>title — 120 characters` | reading a mostly Cyrillic prompt of near-duplicate candidates — **median 58,314 bytes, p90 60,843**, metered over thirty prompts — and answering with ids and nothing else | sonnet on the 82 recorded cases, two runs on 2026-09-09: paraphrase 29/30 both times, keyword 40/40, code 12/12, p90 228 and 231 tokens, ~4.3 s a question. The prompt is measured in **bytes**, because `claude -p --output-format text` returns the picked ids and no usage block; bytes ÷ 4 is ≈14.6k tokens and is a floor on this corpus, where a Cyrillic character is two bytes — so ≈$0.03 a question at Sonnet 5's $2 per million is a floor too. On the older 41-case pool haiku read 11/14 and opus 14/14 paraphrase but 23/24 keyword | `sonnet`: opus buys nothing and costs a keyword hit, haiku loses three paraphrases |
| `reranker_dir` | `ask --rerank-local` | scores the same pool with a local cross-encoder instead of a model command, at zero tokens | the same pick, without a network or an account | measured and rejected as a floor candidate on 2026-09-04: 17.9 seconds a question against a bar of one, keyword 39/40 | not this, unless tokens are impossible |

**The contract a command has to meet** is the same for both stages and names no vendor. `sh -c`
runs it, the whole prompt arrives on stdin, and the answer is read from stdout; the exit status and
the output judge the run, so a command that answers without reading its prompt to the end is fine —
the broken pipe that write hits is ignored on purpose. `{model}` anywhere in the command is replaced
by that stage's model key, and a command naming no `{model}` runs verbatim with the key unused.
From `enrich`'s output only `id<TAB>question` lines for ids in the batch are kept: any other line is
ignored, one line may carry several tab-separated questions around the node's own id, and a line
whose letters are mostly neither Cyrillic nor Latin is dropped. From `--rerank`'s output only lines
that are exactly one of the ids it showed are kept, in the model's order, without repeats and
tolerating a trailing `.`; a failing command is answered with a notice and the fused order rather
than a failed question. Both parsers ignore what they do not recognise, which is what makes a
chattier command survivable rather than fatal. Why the default runs headless Claude Code with
thinking off — the same answers, 4–5× faster — is in
[Spending tokens on purpose](#spending-tokens-on-purpose).

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
Reading one is the same two commands the numbers here came from: `repograph bench --rerank` against
an enriched store measures a `rerank_model`, and a plain `repograph bench` against a copy of the
store that the other model enriched measures an `enrich_model`. A store records the model its
vectors were written with and nothing at all about the model that wrote its questions, so those
copies have to be kept apart by hand — one directory per enriching model — or the comparison
quietly measures a mixture.

### Id families

There is no key for them. A family is the prefix of any id the corpus *defines*, and the documents
are the only place that is written down: the requirement line `**FR-PAY-22 · MUST · <title>**` and
its heading form `## FR-CAL-40 · <title>` (the modality is optional in both), a milestone document
named `BE-M01-….md` or a `## BE-M01 · <title>` head, a row in one of the `registries`. `build` and
`update` read those definitions off the documents they walk; `ask`, `explain`, `bench`, `dump` and
`serve` read the families back off the graph those definitions became. Nothing is configured,
nothing is stored beside the graph, and there is nothing to keep in step with anything.

A default would have been the alternative, and a default is the answer for a repository about which
nothing is known — 49 families read off `beauty-crm` were never that answer for anybody else's
corpus. A list computed once and pinned beside the graph was the other, and it would have been a
second place saying what the documents already say, out of step the first day somebody wrote a
family down without recomputing it.

Every family is matched as `FAMILY-<1–4 digits>`, hyphen included, or `FAMILY-M<2 digits>` for a
milestone; hyphenless labels such as `B1`, `C11` or `S3` are not ids in any repository and cannot
become families. A prefix that is only ever *mentioned* is plain text — `ISO-8601`, `RFC-7231`, a
ticket number, a year — and so is one whose ids are cited but never defined anywhere; that is the
price of the rule, together with there being no way to take a family away by hand. Writing a line
that defines it is how a prefix crosses that line, and re-reading it is a `repograph update`.

On this project's own development corpus the rule reads 54 id families and 5 milestone families
where the list named 49 and 7. Nineteen of them the list never had — twelve `OP-<AREA>`
open-question families, and `HT`, `I`, `P`, `T`, `C`, `W0B` — each defined by a heading like
`### OP-AI-01 · Может ли салон…` that nobody had thought to configure; thirteen the list had are
cited and never defined, `OQ`, `IDEA` and `PREP` among them, and are now text. The rebuild added
159 nodes and dropped 2,238 edges that pointed at ids no document declares. Of the 82 recorded
bench cases exactly one moved: a paraphrase the lexical arm now reaches, 16/30 against 15/30.

```bash
repograph families                          # families, milestones, and everything left as text
```

Run after a build, it prints every family with the number of nodes it holds and the `path:line`
that first defined it, then every id-like prefix no line defines — how often it is written, in how
many files, and one example line. A family the graph still holds and no document defines any more
is listed too, in place of the `path:line`, since the documents and the store being out of step is
the one thing the command exists to show. The prefixes are counted across source files and a
registry's prose as well as the documents, and a head quoted inside a code fence counts as
neither a family nor a mention. Nothing is written to the repository.

An `update` whose documents have gained or lost a family says so — `families: +REQ`, `families:
-AC` — and re-reads the whole tree rather than the edited file alone, source files included,
because a family changes what every file extracts to; a resident `serve` or `watch` does the same
on the poll that applies the change. An update that finds nothing changed reads no documents at
all: a tree that has not moved cannot have moved its families.

`ask`'s own refresh reads ids through the families the graph already holds, so that answering a
question never costs a pass over the whole corpus, and a brand-new family reaches the read path
through the `build` or `update` that derives it.

A `repograph.toml` that still names `id_families` or `milestone_families` parses as it always did,
gets one line on stderr — `id_families is no longer read — families are derived from the documents'
definitions` — and is otherwise unaffected.

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

The model is a property of the store. `embed_model` in `repograph.toml` names what `build`,
`update`, `enrich`, `embed` and `watch` write vectors with; `vectors.json` records it, and `ask`,
`bench` and `dump` open the recorded one, so a store keeps answering with the model that wrote it
whatever the configuration says today. A store written before the field existed records no model
at all, and every one of those is the small model's — a reader opens that one however the default
moves afterwards, so a new default never silently reinterprets an index nobody re-embedded. Only
`build`, `update`, `enrich`, `embed` and `watch` move a store to the configured model. Switching is
one line and one `repograph embed`: rows another model wrote are dropped and the file rewritten.
The claim is on width as well as name, so a store
the earlier `REPOGRAPH_EMBED_MODEL` recipe left holding another model's rows under no recorded name
is re-embedded whole by the next `embed` rather than relabelled over rows it never wrote.
`REPOGRAPH_EMBED_MODEL=<hub id>` outranks both for one command, which is how a copy of a store is
measured under a second model. Query that copy with `ask --stale`, or with its tree unchanged
beside it — `bench` and `dump` read the store as it stands and need neither: an `ask` that
refreshes claims the index for the model the override named, and at the same width that re-embeds
the very rows being measured (a different width the guard refuses, and the answer is lexical-only).
It is the caveat trap 7 of the [runbook](docs/bench/runbook.md) carries. Measured on the fixture,
`intfloat/multilingual-e5-large` reads paraphrase **22/30** against the default's 15/30 with
keyword 40/40 and code 12/12 unchanged, and held-out 103 → 119 of 400 (+19 −3, p = 0.0009). What
that recall costs is the rest of this section: an `ask` in 0.8 s against 0.30 s (the model opens in
676 ms against 220), 1.9 GB resident against 1.7, a 2.1 GB download against 470 MB, and 1,930 s to
embed the fixture's 33,525 rows against 266 s at the default `resources = "balanced"`
([2026-09-09](docs/bench/2026-09-09-normal-band-only-results.md)): a first build is half an hour
under the large model where the default's is four minutes. The large model is one
line and one `repograph embed` away, and a store already on it keeps answering by it whatever this
file says afterwards; [ADR-002](docs/adr/ADR-002-two-defaults-multiplied.md) weighs the two and
says why the cheaper one is the default.

The two open figures are not the same measurement twice. The small model's fell from 418 ms to
220 when the cache lookup stopped asking the hub for a weights file it has never had and waiting
out the 404; the large model's 676 ms is untouched by that, because its weights genuinely do live
beside its graph and the lookup was always a cache hit. The saving is the default's alone, and the
large model pays what it always paid.

Turning the dense stage off altogether is the step below that, and what it costs depends on which
model it replaces. The lexical lists do not know what is configured, so `--no-dense` reads keyword
39/40, paraphrase 14/30, code 12/12 on the fixture's enriched store either way. Against the
default's 40/40, 15/30, 12/12 that is two hits of eighty-two, for a 470 MB download and ~0.2 s an
`ask` saved; against `e5-large`'s 40/40, 22/30, 12/12 it is nine, for 2.1 GB and ~0.7 s. The dense
stage earns its keep in proportion to the model behind it: on the default it is worth one
paraphrase and one keyword, which is why the model and the `--no-dense` switch are one decision
rather than two.

The floors in [Bench](#bench) are keyed by the model the store's rows were written with, since
0.5.0. Only the two dense arms depend on the embedder at all, and `e5-large` has floors of its own
there too, measured on a copy of the fixture re-embedded under it and read twice per arm:
`keyword 40/40  paraphrase 22/30  code 12/12  p90 224` with `enrich`'s questions and
`40/40  17/30  12/12  p90 227` without. A store whose rows were written by any other model is
measured and never graded — the summary line says `model=<name>` and `gated=false`. The numbers and
how they were read are in [the 0.5.0 gap results](docs/bench/2026-09-05-0.5.0-gaps-results.md).

`ask` opens the model only when a fused query needs it, and that open is most of what a fused
answer costs: ~0.30 s and ~1.7 GB on the default model, ~0.8 s and ~1.9 GB on `e5-large` — the
model, not the graph. The bench fixture is a default-model store, and its 0.30 s is 220 ms of open —
a cache lookup, then the 16 MB tokenizer and the 448 MB ONNX session opening concurrently — against
410 ms before the levers below. An exact-id lookup answers in ~50 ms and ~50 MB, and a `--no-dense`
question in ~0.1 s, since neither opens the model or reads the vectors. What is left of the open is
paid once per process, which is what [`serve`](#asking-a-resident-process) is for.
`REPOGRAPH_TIMING=1` prints where an `ask` spends its time, stage by stage; the stage tables and
what each lever bought are in [the perf results](docs/bench/2026-09-06-perf-results.md).

Five embedding-side levers were measured on the same corpus and cases — on the fourteen-case set,
and before `enrich`'s generated questions were in the index — and none moved recall past 6/14: the
larger `MultilingualE5Base` (768-d, ≈1.1 GB, 2.4× the download) scores 6/14 with a different hit
set; `BGEM3` (1024-d, ≈2.1 GB) scores 5/14 at eleven times the embedding time (1,454 s against
132 s); the quantized `ParaphraseMLMiniLML12V2Q` scores 2/14 and drops keyword to 21/24; raising
the passage cut from 256 to 512 tokens scores 5/14 at double the embedding time; a second vector
per node for the label alone, max-scored against the passage vector, scores 6/14 at 1.85× the
embedding time. What none of them had was size: re-measured with the questions in the index, which
is the paragraph above, `e5-large` reads paraphrase 22/30 against 15/30 — as the one line that
buys it and not as the default, for the reasons in
[ADR-002](docs/adr/ADR-002-two-defaults-multiplied.md). The passage cut stays at 256 tokens and the
node keeps one vector.

If the model can't be opened (no cache, no network), the two kinds of caller degrade differently, on
purpose: `ask` and `update` fall back to lexical-only and print one line to stderr saying so, then
exit 0 — a person reading the answer can judge a lexical-only one for what it is. `bench` in dense
mode instead fails the run outright (exit 1): its only output is an exit code, and a silent fallback
graded against the weaker no-dense floor would report green without having measured what it claims to
measure.

On `beauty-crm`'s 6,700 non-`File` nodes, the first embedding pass takes ~103 s on an M3 Pro — rows are
batched by length, so a ten-token label no longer pads out to a 256-token batch (191 s before that,
same vectors to six decimals); a second `update` with nothing changed embeds 0 — only nodes whose
passage hash changed are re-embedded. `build` drops the graph and the manifest and nothing else: the
vectors are reused by content hash and the questions cost tokens, so neither is paid for twice.

## Spending tokens on purpose

Everything above runs at zero model tokens, and stays that way by default. Two stages can spend
them, each behind an explicit switch, each measured on the development corpus:

**`repograph enrich`** asks a model, once per requirement-like node, for twelve questions a reader
might ask to find that node in everyday words plus a line of synonyms — the generated questions are
embedded as rows of their own for the reranker's pool and indexed for BM25 as a list of their own
in every answer. `enrich_command` is any
shell command that reads the prompt on stdin and writes `id<TAB>question` lines; the default is
headless Claude Code with thinking off (`MAX_THINKING_TOKENS=0 claude -p --model {model} …`),
which answers the same and 4–5× faster than with it; `{model}` is filled from `enrich_model` —
see [Choosing a model](#choosing-a-model-and-where-the-choice-lives). Generation is cached by passage hash in
`.repograph/questions.json`, so a later `enrich` pays only for nodes whose text changed. Two
kinds of drift are refused on the way in and cleaned out of an older cache on load: a line
whose letters are mostly neither Cyrillic nor Latin (the generator answered 12 ADR nodes of the
development corpus in Urdu — 144 lines nobody could search for), and several questions
tab-joined into one line around the node's own id (40 lines, each of which the exact stage
answered for free). A node left without questions is asked again by the next `enrich`. On the
corpus as it stood on 2026-09-02, 1,971 eligible nodes took 16 minutes at 8-way parallelism and
roughly $2.5 of Haiku
([ADR-001, Second amendment](docs/adr/ADR-001-paraphrase-recall-was-a-prediction.md)); the corpus
has since grown to 1,996 eligible nodes
([the 0.5.0 gap results](docs/bench/2026-09-05-0.5.0-gaps-results.md)). A node's questions run
about 13 lines.

`enrich --code` extends the pass to code: symbols with a doc comment or a body of their own and
files with a head comment — 3,475 nodes on the corpus, 290 batches — through a prompt of its own
that asks four Russian and four English questions per node and forbids repeating the identifier: a
developer's question is «где проверяется, что запрос принадлежит нужному бизнесу», not
`TenantContextInterceptor`. Entries carry a `c<n>` key in the prompt because the model, asked to
copy a `sym:apps/api/src/…::AvailabilityService` id, copies its label instead — half the batches
came back without a usable line before the key. The code questions are an index of their own,
searched for the `--rerank` pool and nowhere else: the plain `ask` fusion is byte for byte the
fusion of a store without them, a symbol's passage row stays its declaring line, and a file is a
passage nowhere (its head comment in the passage index moved the BM25 statistics against paraphrase,
15/30 → 13/30 on 2026-09-05) and is present through its questions alone. Why they are kept out of
the plain fusion is measured, in three steps: inside the documents' questions index they lifted that
index's average length until the gate admitted it over the passage that held the answer (keyword
39/40 → 38/40 without embeddings); as a list of their own admitted last on the same 0.85 gate they
took two document answers off that same arm's recorded suite, the arm with embeddings unchanged; and
capped at a single seat they read `where` 0/9 → 2/9 on the developer suite and still lost six of 400
held-out questions in each arm, gaining none (p = 0.031). That seat was measured once more under
the coverage admission at a constant of the code list's own — 0.902, the crossover of 800 held-out
questions, half the fixture's document set and half drawn from a code-enriched copy's 3,463 code
entries — and refused again for a different reason: it reads `where` 0/9 → 2/9 in both arms and
takes the code held-out set from 54/400 to 122/400 and 40/400 to 115/400 (p = 0.0000), and it now
clears the held-out clause that stopped it before (five document questions lost per arm, none
gained, p = 0.0625), but it costs one recorded paraphrase case in the lexical arm — the code seed
takes a slot and pushes off the fifth seed the answer was reached from. A store without code
questions is therefore the old index byte for byte; `coverage` still counts documents, so the
floors grade the same store the same way, and the summary line reports `code_questions=` beside it.
What the questions buy, and what a seat for them costs, is measured in
[the developer-questions results](docs/bench/2026-09-05-dev-cases-results.md) and, under the
coverage form, in
[the residue, the seat and the register](docs/bench/2026-09-06-residue-seat-register-results.md).

Two ways of spending the documents' generated questions were measured and rejected: as extra dense
rows pooled with the passages they bury targets (a passage at rank 2 fell to 87 behind other nodes'
questions), and mixed into a node's own BM25 text they cost a keyword hit. What ships for those
questions is the third — a BM25 list of their own, which the plain `ask` fuses alongside the passage
list — when that list has earned its turn. Each list is asked what fraction of the question its
best document actually reached: `best / attainable`, where `attainable` is the idf of every term
the query asked for, priced in that index — a term the index never saw charged at the idf BM25
gives `df = 0`, so a word a list cannot answer lowers its coverage instead of leaving its
denominator. Those fractions are dimensionless, so the two lists compare in one unit whatever
their raw scores are worth, and the questions list joins the fusion when its coverage reaches
0.761 of the passage list's. That constant is the crossover of 400 held-out generated questions in
the `--no-dense` arm. It was re-derived on that same set when the denominator began charging every
term and returned 0.761 again, so the number did not move, but it belongs to the form that ships —
[the residue, the seat and the register](docs/bench/2026-09-06-residue-seat-register-results.md)
and [ADR-001, Amendment 9](docs/adr/ADR-001-paraphrase-recall-was-a-prediction.md). Until that
change an absent term left the sum, and a list's coverage rose with every query term its index
lacked; charging it moved no count in any of the four recorded arms and no case's verdict in
either suite. Until 0.5.0 the admission compared the two raw bests
at a ratio of 0.85, which moved with enrichment coverage and questions per node and was a constant
of one store rather than of BM25 — gaps G8 and G12, closed by
[the coverage admission results](docs/bench/2026-09-06-coverage-admission-results.md) and recorded
in [ADR-001, Amendment 8](docs/adr/ADR-001-paraphrase-recall-was-a-prediction.md). The change reads
paraphrase 15/30 against the ratio's 14/30 in the lexical arm, reproduces the dense arm's four
counts, raises both developer totals and gains one held-out question with embeddings and three
without, neither significantly. Before any admission at all,
an equal turn cost the `--no-dense` arm two exact keyword seeds, 39/40 raw
against 37/40 enriched; with it that arm reads 39/40 either way, paraphrase 7/30 raw against 14/30
enriched, and the held-out set moved by 5 gained and 7 lost, exact McNemar p = 0.77. The arm with
embeddings was 40/40 throughout. (An older reading on the 14-case set — 6/14 paraphrase with the
questions and without — is what this section used to cite for the claim that the plain `ask` ignores
them; it does not.) The questions also carry targets into a deeper candidate pool for `--rerank`:
with them, all six reachable paraphrase misses of that older set sat within the top 100 fused
candidates; without them, two did not.

**`ask --rerank`** builds a 200-deep pool — dense passages, dense questions, BM25 passages, BM25
questions, and, when the store carries questions about code, those as a fifth list, interleaved —
and hands the model each candidate's id, title and the first 120 characters of its text to pick five
from; `--depth` changes how deep, and tokens per question scale with it. `rerank_command` reads the
prompt on stdin and writes the chosen ids one per line; a failing command is reported on stderr and
the answer falls back to the fused order.

What the model is shown decides more than which model it is. Shown titles only, haiku, sonnet and
opus all read 10–11/14 whatever the depth, and a deeper pool made haiku worse; and because a
title is not evidence, the fused top two had to stay pinned ahead of the model's picks or it
dropped a keyword hit. Shown 120 characters of text, sonnet at depth 200 reads 13/14 with the two
pins and 14/14 without them — the pins were the retrievers' guess taking two of the model's five
slots. Haiku with the same prompt reads 11/14; opus 14/14 on paraphrase but 23/24 on keyword, in
two runs of two. Measured on the 41 cases then recorded (`bench --rerank`, one full run each unless
stated; input tokens are the answering model's own, median over the 38 questions) — all but the
last row, which is the 82 cases recorded since. That row's prompt is **metered in bytes** by `bench`
itself, over the thirty paraphrase prompts of
[the rerank diagnostics](docs/bench/2026-09-09-rerank-diagnostics.md): the transport this row was
run through returns the picked ids and no usage block, so the byte figure is a count and the token
figure derived from it is a floor:

|                                                | paraphrase | keyword | code | p90 tokens | model tokens per question | latency per question |
| ---------------------------------------------- | ---------- | ------- | ---- | ---------- | ------------------------- | -------------------- |
| `ask`                                          | 7/14       | 24/24   | 3/3  | 216        | 0                         | ~0.30 s              |
| `--rerank`, haiku, depth 100, titles           | 10/14      | 24/24   | 3/3  | 222        | ≈4,600                    | ~3.5 s               |
| `--rerank`, haiku, depth 100                   | 11/14      | 24/24   | 3/3  | 222        | ≈9,500                    | ~4 s                 |
| `--rerank`, sonnet, depth 100                  | 13/14      | 24/24   | 3/3  | 222        | ≈10,900                   | ~4 s                 |
| `--rerank`, sonnet, depth 200 (default), 3 runs| 14/14      | 24/24   | 3/3  | 221–226    | ≈19,200                   | ~4.3 s               |
| `--rerank`, sonnet, depth 200, 82 cases, 2 runs (2026-09-09) | 29/30 | 40/40 | 12/12 | 228–231 | 58,314 B median, 60,843 B p90 — metered over 30 prompts; ≥14.6k tokens | ~4.3 s |

The last row is the 82-case set the floors are read on, not the 41 the rows above it use, so its
counts are the ones to compare against `ask`'s 15/30, 40/40, 12/12 on the same store. Against that
baseline the flag gains fourteen paraphrase cases and loses none, and both runs picked identically,
down to the one chronic miss: `FR-MKT-35`, the case the reranker has never answered. The p90
straddles the 230-token ceiling — 228 in the first run, 231 in the second, green and red on tokens
alone with every count unmoved — one more reason the arm is measured and left unfloored, rather
than an argument against it. The tokens per question are ≈14.4k, taken as bytes over four from one
captured 57.5 KB prompt: an estimate on mostly Cyrillic text, and a floor on what it costs.

Of the two later changes to what the flag builds, one is now read and the other still is not: a
symbol's 120-character snippet is its doc comment rather than its declaring line, which is what the
82-case row above measures; a store carrying `enrich --code` questions puts them into the pool as a
fifth list, which that row does not exercise — the fixture store carries no code questions, so the
fifth list was empty in both runs. The code-question pool ships unread because `--rerank` is opt-in
and on no floor.

**`--rerank-local`** is the same pool and the same pick, scored by a local cross-encoder
(`BAAI/bge-reranker-v2-m3`, exported to ONNX once with `optimum-cli export onnx --model
BAAI/bge-reranker-v2-m3 --task text-classification ~/.cache/repograph/reranker`, ~2.2 GB) at
zero tokens — and **measured and rejected** as a floor candidate on 2026-09-04: 17.9 seconds a
question against a bar of one, and keyword 39/40. Those two figures are on the 82-case set (30
paraphrase, 40 keyword) — the set that table's last row uses, not the 14/24 arms above it; ADR-001, Amendment 4 has the
rule that was fixed before the run and the case-by-case swing. It ships opt-in and on no floor,
exactly as `--rerank` does, and the two flags are mutually exclusive: passing both is an error,
not a silent preference for one of them.

The `bench` floors apply to the zero-token query path in each of the two states a store can be
in — with `enrich`'s generated questions and without them, each on its own numbers, see
[Bench](#bench); `--rerank` is measured, not
floored, because a model's pick can vary by one hit between identical runs — which is also why the
default is the configuration that read 14/14 three times, not the one that read it once. A
per-question query rewrite by the model was measured too — 20/24 keyword, 6/14 paraphrase,
~3,100 tokens — and rejected: the added synonyms dilute exact matches and find no new targets.

## Bench

`repograph bench [--cases file]` runs the recorded 82 cases (40 keyword + 30 paraphrase + 12 code)
against a built graph and fails the process if any floor is missed. The recorded `bench/cases.jsonl`
is compiled into the binary, so a release build benches from any directory; `--cases` substitutes
any other file — one of the recorded 40/30/12 shape is graded against the floors below, any other
shape is measured and reported with `gated=false`, the way `bench --cases bench/dev-cases.jsonl` is
used throughout [the runbook](docs/bench/runbook.md).

The floors are not one set of numbers but two, because [`enrich`](#spending-tokens-on-purpose) is
optional and paraphrase recall is what it buys. `bench` reads which state the store is in and says
so on its summary line (`dense=true  enriched=true (1996/1996 nodes) model=small`): a store carrying generated
questions on at least 99% of its requirement-like nodes is graded against the enriched floors, any
other — a fresh `build`, or a pass stopped early — against the raw ones. A store that also carries
questions about code prints `code_questions=covered/eligible` beside those fields and is graded by
the same document floors: `enriched` counts documents alone, and the code questions are searched for
the `--rerank` pool rather than in the fusion the floors measure. The bar is a high-water mark and
not every node because equality over ~2 000 nodes is a cliff: one requirement added after
the pass, one node the model skipped past its retry, one entry dropped on load would regrade a
paid-for store to floors five paraphrase points lower, and `bench` says so only through its exit
code. The printed counts stay exact either way.

The dense floors are keyed by the store's embedder as well, because a floor measured on one model
says nothing about another: rows written by the small model or under no name at all are graded
against the small model's numbers, rows written by `e5-large` against `e5-large`'s, and a dense
arm under any third model is measured and never graded — `model=<name>` and `gated=false` on the
summary line, the way another case file is measured and not graded. The lexical arms have no
embedder in them and keep one set of floors whatever the rows are.

The summary line also carries `families=<count>`: how many id families the store's graph is written
in, the ones every id in the run was read through. It is a number to compare between two runs of
the same corpus, not a floor — a rebuild that changes it has changed what every document extracted
to, and the case-by-case lines are where that shows.

| | enriched store | store with no questions |
| --- | --- | --- |
| keyword | 40/40 with embeddings, 39/40 with `--no-dense` | 40/40 with embeddings, 39/40 with `--no-dense` |
| paraphrase, small-model rows (the default) | ≥14/30 with embeddings, ≥11/30 with `--no-dense` | ≥9/30 with embeddings, ≥7/30 with `--no-dense` |
| paraphrase, `e5-large` rows | ≥22/30 with embeddings | ≥17/30 with embeddings |
| code | 12/12 | 12/12 |
| p90 | ≤230 tokens in every arm | ≤230 tokens in every arm |

`e5-large`'s two dense floors are the counts a copy of the fixture re-embedded under it read,
twice per arm, in [the 0.5.0 gap results](docs/bench/2026-09-05-0.5.0-gaps-results.md); the `--no-dense` column is the small
model's and applies to every store, since no embedder is in it.

**Keyword is 39, not 40, in both lexical-only arms.** `FR-PH-43` sits at passage rank 23 and no
lexical path reaches it, enriched or raw. The enriched arm read **37/40** until 2026-09-05, losing
`FR-WH-53` and `W-206` as well, because the generated-questions list took an equal turn in the
fusion on questions it had nothing to say about; the gate described under [Spending tokens on purpose](#spending-tokens-on-purpose)
put it level with the raw store. The floor moved to 39 only once it was level — at 37 it stayed 40,
because 37 was a cost enrichment itself imposed and a floor that blesses one is not a floor. The
measurement is in [G7](docs/bench/next-version-gaps.md).

A raw store is a supported way to run the tool, not a broken one: it answers every code case and,
with embeddings, every keyword case, and `bench` passes on it. What `enrich` buys is the higher
paraphrase bar. A store below the 99% mark — `--limit`, an interrupt, a batch the model never
answered — is graded raw, since the enriched numbers describe a finished pass; the node counts on
the summary line say how far the pass got. The raw floors themselves have no headroom: unlike the
enriched paraphrase floor of 14, which has a point of slack and weeks of runs behind it, 9 and 7
are two runs on one machine on one day sitting flush on the noisiest split, and they are the first
thing to relax if they flap.

The p90 is counted as rendered UTF-8 bytes / 4 — a conservative proxy, since it counts a Cyrillic
answer at roughly double what an equivalent chars/4 reading would give a Latin one. An answer
seeded from a requirement therefore costs more than one seeded from a symbol or a task node, and
the arms differ by a few tokens at p90 according to where their seeds fall; the four sit between
220 and 226, so one ceiling covers them all.

The paraphrase cases are the noisy half, and the set was grown to narrow them: Wilson 95% on 14/30
is 0.30–0.64, against 0.27–0.73 when the same gate rested on 14 cases. It is still a wide interval,
so retrieval changes are judged on a second set:
`repograph dump --queries qs.jsonl --out lists.json` writes,
for every question in a `{"q", "expect", "kind"}` JSONL, the four retriever lists 300 deep (dense
and BM25, over passages and over the generated questions) with their scores, the query vector,
the exact ids and the answer `ask` would give — and, for a question that is itself a stored
generated question, leaves that row out of both question indexes while it is asked. Four hundred
such held-out questions, one per node, give recall@5 a ±5-point interval and a paired exact
McNemar test against the shipped rule; that is the bar a fusion or expansion change has to clear
before the 82 real cases are consulted as the smoke test they are.

Measured, on the shipped binary against `beauty-crm` with its generated questions in the store,
two runs of each arm identical case by case: `keyword 40/40  paraphrase 15/30  code 12/12  p90 220
tok` with embeddings and `keyword 39/40  paraphrase 14/30  code 12/12  p90 215 tok` with
`--no-dense`, both arms green. A store that
`enrich` has never touched — the same store with its questions files removed — reads `keyword
40/40  paraphrase 9/30  code 12/12  p90 221 tok` with embeddings and `keyword 39/40  paraphrase
7/30  code 12/12  p90 226 tok` with `--no-dense`; the raw floors in the table above are those
numbers, measured rather than assumed. Code is floored at the whole 12 and keyword at the whole 40
with embeddings, which is why they are counts rather than fractions; only paraphrase is a fraction
wherever it is graded.

Three of the first fourteen paraphrase cases were rewritten on the way. One asked about withdrawing
consent through a messenger, while the entry it names (`FR-VIS-76`) is about who may leave a
review — «отзыв» meant a review there, not a withdrawal — so no retriever could have answered it.
Two more were under-specified rather than wrong: «export for tax reporting» names the corpus's
DAC7 tax-reporting cluster better than its target, the accountant's export (`FR-PAY-104`), and
«the product's inviolable requirements» fits the individual invariants as well as their registry
(`FR-VIS-01`); a model shown both sets chose between them at random. Each new question still
shares no word with its target line. Asking for one of the 40 keyword cases' ids verbatim returns
its head line first every time, at 68 tokens median — an exact match fills the answer alone instead
of being topped up with fused neighbours, which had cost 174 tokens for the same lookups.

The design note that shaped this architecture predicted paraphrase recall would reach ≥12/14 once
dense retrieval was fused in. It measured at 5/14, 6/14 after the fusion change and 7/14 once three ill-posed cases were
rewritten — a prediction that
did not survive contact with measurement, not a bug; see
[`docs/adr/ADR-001-paraphrase-recall-was-a-prediction.md`](docs/adr/ADR-001-paraphrase-recall-was-a-prediction.md)
for what was ruled out and what wasn't. The floors above are that measurement, and on the 38
questions both tools were ever run against the tool still beats the incumbent on every axis anyone
has measured: 7/14 and 24/24 at 203 median tokens against graphify's 0/14 and 11/24 at 1,027-1,555
tokens, built for 14.6 million tokens instead of zero.

**`import-legacy`'s coverage note.** Folding in a graphify graph costs recall and cost at query time,
not just disk: importing `beauty-crm`'s graphify graph adds 8,577 concept nodes and 23,117 edges and
moves the recorded 82 cases from `40/15/12` to `38/16/12` — two keyword hits traded for one
paraphrase gained, below the binary's own floors
([the three-graph results](docs/bench/2026-09-03-three-graphs-results.md)). That is why `bench`
above is always measured against a legacy-free store, and why `import-legacy` stays a separate,
opt-in step rather than folding into `build`. Two graphify nodes that resolve to the same
requirement collapse onto one node, and the edge between them is dropped rather than kept as a
self-loop; the import prints the count for whatever graph it is run against.

## Measured

A snapshot from 2026-09-02 (commit `b589ca2`), when the development corpus was a TypeScript
monorepo with a Russian-language PRD, 825 indexed files out of 3,599 tracked:

|                                 |                                                                                                                              |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Nodes                           | 7,525 — 3,640 `Symbol`, 1,880 `Requirement`, 1,001 `Task`, 825 `File`, 72 `Entity`, 69 `Milestone`, 20 `Invariant`, 18 `Adr` |
| Edges                           | 27,412                                                                                                                       |
| Graph on disk                   | 10.2 MB JSON                                                                                                                 |
| Graph load                      | ~19 ms                                                                                                                       |
| Lexical index build             | ~120 ms, paid once per context since 0.5.0 rather than once per question — a later sitting bounds that build, the question index beside it, their scoring and the fusion at ~49 ms on the bench corpus at 8.3k nodes ([the perf results](docs/bench/2026-09-06-perf-results.md)), and a later one still reads that socket answer at 6.8 ms with the indexes kept ([the 0.5.0 gap results](docs/bench/2026-09-05-0.5.0-gaps-results.md)); different sittings, not a before and after |
| Tokens spent building the graph and its vectors | 0                                                                                                             |

The corpus has since grown; the bench fixture the numbers below are measured on is 908 files and
about 8.3k nodes
([the 0.5.0 gap results](docs/bench/2026-09-05-0.5.0-gaps-results.md),
[the perf results](docs/bench/2026-09-06-perf-results.md)).

Retrieval on the recorded 82 cases against that current corpus, both arms run twice with identical results:
keyword 40/40, paraphrase 15/30, code 12/12 at 220 p90 tokens with embeddings; 39/40, 14/30, 12/12
at 215 p90 with `--no-dense`, both arms green. Those are the numbers with `enrich`'s generated
questions in the store — the one thing above that was paid for, roughly $2.5 of Haiku, once. The
lexical-only arm read **37/40** until 2026-09-05, when the gate described under
[Spending tokens on purpose](#spending-tokens-on-purpose) put it level with the raw store; the
floor it is held to is 39, which [Bench](#bench) explains. The same corpus indexed and queried at
zero tokens throughout reads 40/40, 9/30, 12/12 at 221 p90 and 39/40, 7/30, 12/12 at 226;
[Bench](#bench) floors each state on its own numbers.

The prior art on the same corpus was an LLM-extracted graph that cost **14.6 million input tokens
over 13 runs** — see the table at the top of this document for how it and repograph compare on the
38 questions of the day. Cost is not the only reason to replace it, but it is the easiest one to
state.

See [Bench](#bench) for the retrieval-quality floors these numbers are held to, and the ADR for the
one figure that didn't hold up on first measurement.

### Resources

Everything above is a reader, and a reader costs a fraction of a second and the model it opened.
The one command that can take a machine over is a writer that has to embed the store whole —
`build`, `update`, `enrich`, `embed` or `watch` on a store whose rows belong to another model.
Measured on the bench fixture's 33,525 rows under `intfloat/multilingual-e5-large`, before and
after the work in [the resource-usage results](docs/bench/2026-09-07-resource-usage-results.md):

| | wall | user | max RSS | peak CPU | threads |
| --- | --- | --- | --- | --- | --- |
| before | 2,582 s (43 min) | 13,342 s | 2.96 GB | 444% | 18, 6 running |
| after | 1,930 s (32 min) | 7,641 s | 2.15 GB | 293% | 8, 4 running |

Both rows were taken at `786b994` on 2026-09-07 in the normal band, which is the only band there
is now, so they stand as written. That is the large model because it is the worst case the tool has
and a repository opts into it: the same 33,525 rows under the default model are
**266 s and 1.45 GB**, re-measured on 2026-09-09
([the levels results](docs/bench/2026-09-09-normal-band-only-results.md)) — the 214 s this file
used to quote here was an *uncapped* row standing in for a four-thread default, and understated it
by about a quarter.

Those are the rebuild's own numbers, and they are the only ones there are: the writers run in the
same scheduling band as everything else, on every platform, and there is no setting that names one.
There was, until 2026-09-09 — `priority = "background"` put a rebuild in the band the machine keeps
for work nobody is waiting on, and it bought a quiet keyboard genuinely well (a compile beside it
slowed 1.7% instead of 15.7%). It was removed because the price was four times the wall, paid by
everyone who ever rebuilt, to buy something only a person sitting at a loaded machine collects.
[The unnoticeable results](docs/bench/2026-09-07-unnoticeable-results.md) are what it measured while
it existed; none of those numbers is carried forward here, because they are the band's.

What a rebuild costs the person beside it is now bounded by two things only, the token budget and
one word:

| `resources` | threads here | wall | peak CPU (mean) | threads / running |
| --- | --- | --- | --- | --- |
| `"full"` | 6 | 168.8 s | 382% (335%) | 12 / 6 |
| `"balanced"` *(the default)* | 4 | 265.7 s | 275% (218%) | 8 / 4 |
| `"low"` | 2 | 358.7 s | 140% (127%) | 4 / 2 |

The fixture's 33,525 rows under the default model, on a twelve-core Apple Silicon machine
([2026-09-09](docs/bench/2026-09-09-normal-band-only-results.md); `balanced` is the median of three
runs, and the machine was carrying other work throughout, which is recorded there). The rule is
fractions of the logical cores with one thread as the floor: `full` a half, `balanced` a third,
`low` a sixth. On four cores or fewer `balanced` and `low` meet at one thread and only `full` still
names a different amount.

`resources` is the only resource lever there is. `threads = N` was the escape hatch until
2026-09-09 and is gone with `priority`; **`full` is now both the most of the machine you can ask
for and the fastest setting the tool has**, which was not true while a hand-written count existed.
`full` resolves to half the logical cores rather than to no cap at all, and that is a measured
choice rather than a literal one: leaving both pools to size themselves gives ORT its six
performance cores and rayon all twelve — 18 threads, 6 running, 178.1 s — against 12 threads,
6 running and 168.8 s when rayon is capped at the same six, for the same 814 user seconds either
way. Twelve tokenizer threads queueing for six cores cost more than they add.

Set it in `repograph.toml`, or once for the machine in `~/.config/repograph/config.toml`, or
`REPOGRAPH_RESOURCES=full` for one run. Readers take the level too, but it is not what it is for:
every reader but `ask --rerank-local` is under a second.

A writer also stops making its own copy of the model's weights: it reads them from the
memory-mapped file instead, which takes the anonymous memory a rebuild holds from 1.63 GB to
0.50 GB — the part the system counts when it decides what to compress, swap or kill. On a machine
with memory to spare that costs nothing; on one that is already short it costs 6–26% of the wall,
because pages the system is free to reclaim are pages it reclaims and the run reads them again.
Readers are unchanged and keep their own copies: they answer one query and leave.

The level follows the machine: `balanced`'s third of the logical cores is one thread on a two- or
four-core box, and on Linux `available_parallelism` honours a container's `--cpus` quota, so a
devcontainer gets a third of what it was given rather than a third of the host. A build server with
nobody at the keyboard wants `resources = "full"`, one line in
`~/.config/repograph/config.toml`; a laptop you are working on wants `"low"`.

On a machine with 8 GB the model is the lever and the level is not: a rebuild under
`embed_model = "intfloat/multilingual-e5-large"` touches about 1.6 GB of weights whatever the level
says, where the default model's are 0.45 GB and the whole store is 266 s.
There is no low-memory flag, because which vectors are on disk is a property of the repository and
not of the laptop that happens to be building them.

Two things bound it, and the first of them now has a name. `resources` caps the ONNX session's
intra-op pool and rayon's global pool, which is what `tokenizers` fans a batch out over; left alone
that is a third of the logical cores, because the runtime otherwise takes every performance core
and holds it for the length of the run. And a batch closes on a padded-token budget rather than on
a count of texts, so the largest shape the runtime ever allocates an arena for is bounded whatever
the corpus's longest passages happen to be — a count of sixty-four bounds nothing, since sixty-four
256-token passages are 16,384 tokens and sixty-four labels are 1,280.

Moving the level is a straight trade, cores against wall time, with no free side: the three rows in
the table above are 6, 4 and 2 threads for 169, 266 and 359 seconds. A long embed saves after every
1,024 rows and says where it is, so an interrupted rebuild resumes from its last checkpoint instead
of starting again:

```
dense: 16384/33525 rows, 18.7 rows/s, ~15 min left
```

The cheapest way out is the default and costs nothing to keep: `intfloat/multilingual-e5-small`
writes the whole store in 266 s and 1.45 GB where `e5-large` takes 1,930 s and 2.15 GB, and
`--no-dense` on the writer opens no model at all. `serve` and `watch` hold the model on purpose —
1.4 GB resident on the default model, about 1.8 GB on `e5-large` — and are idle between refreshes;
`ask --rerank-local` opens a second 2.1 GB session beside the embedder and is the one reader that
reaches 3+ GB.

## Design

The full design note and implementation plan are in
[`docs/superpowers/specs/2026-09-01-repograph-design.md`](docs/superpowers/specs/2026-09-01-repograph-design.md)
and [`docs/superpowers/plans/2026-09-01-repograph.md`](docs/superpowers/plans/2026-09-01-repograph.md).

The plan's deviations table lists four simplifications against the spec, none of which change the
node/edge model or the answer shape: `serde_json` with an atomic rename instead of `rkyv`; a
77-line hand-rolled BM25 over `rust-stemmers` instead of `tantivy`; a flat edge set scanned per
hop instead of `petgraph`; and a line scanner (one regex for a requirement head, `#` lines as block
boundaries, one regex for links) instead of `tree-sitter-md`. One of those four _did_ move a
measured number: the `tantivy` swap cost paraphrase recall, not just code size — see
[`docs/adr/ADR-001-paraphrase-recall-was-a-prediction.md`](docs/adr/ADR-001-paraphrase-recall-was-a-prediction.md).

## License

MIT.
