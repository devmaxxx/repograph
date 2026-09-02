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

Version 0.1.0. Every row below is implemented, not planned:

| Command                    | State                                                                 |
| -------------------------- | --------------------------------------------------------------------- |
| `build`, `update`          | working; incremental; a no-op `update` is a fixed point               |
| `ask`, `explain`, `verify` | working: exact id/symbol → BM25 → dense, fused, one hop out           |
| `bench`                    | working; fails the process if a floor in [Bench](#bench) is missed    |
| `import-legacy`            | working; costs recall at query time — see its note in [Bench](#bench) |
| `enrich`, `ask --rerank`   | working; opt-in, the only two stages that spend model tokens — see [Spending tokens on purpose](#spending-tokens-on-purpose) |

`--no-dense` skips the embedding stage everywhere it could apply — `build`, `update`, `ask`, `bench`.
Without it, those commands use local embeddings once the model is cached (see [Embeddings](#embeddings)).

## Install

```bash
pnpm add -D @devmaxxx/repograph    # npm: prebuilt binary for macOS arm64 and Linux x64
cargo install --path .             # from source; Rust 1.98, pinned in rust-toolchain.toml
```

The npm package is a launcher: the binary comes from `@devmaxxx/repograph-darwin-arm64` or
`@devmaxxx/repograph-linux-x64`, pulled in as an optional dependency, so a lockfile written on
one platform installs on the other. Tagged releases (`v*`) build both binaries as GitHub release
assets and cut the npm packages from those same files (`.github/workflows/release.yml`;
`scripts/npm-pack.sh` does the same by hand).

## Use

Build the graph once, then keep it fresh incrementally:

```bash
repograph --repo /path/to/project build     # full build
repograph --repo /path/to/project update    # re-extract only changed files
```

`--repo` defaults to the current directory. State lives in `<repo>/.repograph/`; add it to
`.gitignore`.

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
lexical, ~80 ms with the re-embedding. `REPOGRAPH_TIMING=1` prints the stages.

```bash
repograph ask --stale отмена записи         # answer from the store as it stands, no check
```

Readers that do not refresh themselves — an editor plugin, an MCP server — can be kept supplied by
a poller instead:

```bash
repograph watch                  # poll every 30 s, apply what changed, embed it
repograph watch --every 15       # a tighter cadence
repograph watch --batch 5        # save once five files are waiting, not on every one
repograph watch --no-dense       # lexical only, leaves the 1.3 GB model unopened
```

A refresh rewrites the whole graph, so `--batch` is there to spend that once on a burst of edits
rather than once per file. Fewer than `--batch` files waiting are carried to the next poll, at most
three times in a row, so a lone edit lands within four polls whatever the batch says;
`REPOGRAPH_TIMING=1` reports each deferral.

It prints one line per refresh and exits on Ctrl-C; every store write is a temp file and a rename,
so interrupting it cannot leave half a graph behind. An idle poll is the walk and nothing else —
21 ms of CPU on the development corpus, under a tenth of a percent of a core at the default
cadence.

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
repograph bench --cases other.jsonl  # a different case file, same 40/30/12 shape
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
   no-dense paraphrase cases 3/14 → 5/14, at +3 tokens p90 with embeddings and +15 without.
5. **Expand.** One hop over `References`, `Implements`, `Declares`, `Links` and `Legacy` edges, in
   both directions, keeping the single neighbour the retrievers ranked best, however far down
   their lists; a neighbour no retriever ranked falls back to its seed's rank. Measured on 400
   held-out generated questions, that choice reads 226 hits against 208 for the seed's rank
   alone (one lost, nineteen gained) and 8/14 against 7/14 on the paraphrase cases, for the same
   one line of output. A second expanded line measured +1 hit per
   extra neighbour against ~+90 tokens per answer. `File` nodes and decorator nodes are never
   expanded _to_ — they are hubs and would drown the answer.
6. **Render.** `ID  path:line  headline`, headline cut to 80 characters.

The lexical index is rebuilt in memory on every `ask` rather than stored on disk. It costs about
120 ms on a 7,500-node graph, and in exchange there is no lexical state that can ever go stale
relative to the graph.

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
| `Calls`       | declared in the model, but no extractor emits it yet — reserved, not measured                                               |

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
| `id_families`        | see below — 49 strict families                                                              |
| `milestone_families` | `["BE", "FE", "PLAT", "SYNC", "OPS", "AI", "MOB"]`                                          |
| `enrich_command`     | headless `claude -p --model haiku` with thinking off — see [Spending tokens on purpose](#spending-tokens-on-purpose) |
| `rerank_command`     | the same with `--model sonnet`                                                              |

`id_families` and `milestone_families` default to the strict list `beauty-crm`'s census settled on —
they are this project's development corpus, not a generic default. Every family is matched as
`FAMILY-<1–4 digits>`, hyphen included; hyphenless labels such as `B1`, `C11` or `S3` collide with
ordinary prose and are deliberately not families at all. A different repository should replace both
lists with its own:

```toml
id_families = [
  "FR-DM", "FR-CAL", "FR-VIS", "FR-PAY", "FR-PH", "FR-SEC", "FR-APP", "FR-MKT",
  "FR-AI", "FR-CRM", "FR-SHELL", "FR-TOOL", "FR-SVC", "FR-LIFE", "FR-WH", "FR-RPT",
  "FR-MIG", "FR-WEB", "FR-OPS", "FR-STAFF",
  "NFR-PH", "NFR-MKT", "NFR-MIG", "NFR-PAY", "NFR-DM", "NFR-RPT", "NFR-WEB", "NFR-SVC", "NFR-STAFF", "NFR",
  "AC-DM", "AC-VIS", "INV", "ADR", "OD", "OQ", "N", "R", "M", "W", "D", "G",
  "PREP", "CAL", "OR", "MON", "SEAM", "SG", "IDEA",
]
```

A requirement line is recognised as `<ID> · MUST|SHOULD|LATER · <title>`, in either the bold or the
heading form; the modality is optional.

## Embeddings

Dense retrieval embeds with `intfloat/multilingual-e5-small` (384-d, ONNX, ≈470 MB on disk) run
through `ort` directly: the tokenizer and the session open concurrently at optimisation level 1,
which halves model-open time against the library default. The files are a one-time Hugging Face
download cached under `FASTEMBED_CACHE_DIR` if that is set, else `~/.cache/repograph/fastembed`
(the layout is the hub client's, so a cache populated by an earlier release is reused as is). Every
command that touches the dense stage — `build`, `update`, `ask`, `bench` —
reuses the cache; there are no further network calls once it is populated. `--no-dense` skips the
download and the embedding stage everywhere.

`ask` opens the model only when a fused query needs it: an exact id or symbol lookup answers in
~30 ms and ~50 MB, a fused query in ~0.4 s and ~1.4 GB — the model, not the graph; an exact-id
lookup answers in ~50 ms and a `--no-dense` question in ~0.1 s, since neither opens the model or
reads the vectors. `REPOGRAPH_TIMING=1` prints where an `ask` spends its time, stage by stage.

Five embedding-side levers were measured on the same corpus and cases and none moved recall past
6/14: the larger `MultilingualE5Base` (768-d, ≈1.1 GB, 2.4× the download) scores 6/14 with a
different hit set; `BGEM3` (1024-d, ≈2.1 GB) scores 5/14 at eleven times the embedding time
(1,454 s against 132 s); the quantized `ParaphraseMLMiniLML12V2Q` scores 2/14 and drops keyword to
21/24; raising the passage cut from 256 to 512 tokens scores 5/14 at double the embedding time; a
second vector per node for the label alone, max-scored against the passage vector, scores 6/14 at
1.85× the embedding time. The small model at 256 tokens, one vector per node, stays.

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
headless Claude Code with thinking off (`MAX_THINKING_TOKENS=0 claude -p --model haiku …`), which
answers the same and 4–5× faster than with it. Generation is cached by passage hash in
`.repograph/questions.json`, so a later `enrich` pays only for nodes whose text changed. Two
kinds of drift are refused on the way in and cleaned out of an older cache on load: a line
whose letters are mostly neither Cyrillic nor Latin (the generator answered 12 ADR nodes of the
development corpus in Urdu — 144 lines nobody could search for), and several questions
tab-joined into one line around the node's own id (40 lines, each of which the exact stage
answered for free). A node left without questions is asked again by the next `enrich`. On the
1,971 eligible nodes of the corpus it took 16 minutes at 8-way parallelism and roughly $2.5 of
Haiku; a node's questions run about 13 lines.

On their own the questions buy nothing at five seeds — 6/14 paraphrase with them and without, on
the case set before the three rewrites below — and
that is why the plain `ask` never consults them: as extra dense rows pooled with the passages they
bury targets (a passage at rank 2 fell to 87 behind other nodes' questions), mixed into the BM25
text they cost a keyword hit, and as separate lists they change no seed. What they do is carry
targets into a deeper candidate pool: with them, all six reachable paraphrase misses sit within the
top 100 fused candidates; without them, two do not.

**`ask --rerank`** builds a 200-deep pool — dense passages, dense questions, BM25 passages, BM25
questions, interleaved — and hands the model each candidate's id, title and the first 120
characters of its text to pick five from; `--depth` changes how deep, and tokens per question
scale with it. `rerank_command` reads the prompt on stdin and writes the chosen ids one per line;
a failing command is reported on stderr and the answer falls back to the fused order.

What the model is shown decides more than which model it is. Shown titles only, haiku, sonnet and
opus all read 10–11/14 whatever the depth, and a deeper pool made haiku worse; and because a
title is not evidence, the fused top two had to stay pinned ahead of the model's picks or it
dropped a keyword hit. Shown 120 characters of text, sonnet at depth 200 reads 13/14 with the two
pins and 14/14 without them — the pins were the retrievers' guess taking two of the model's five
slots. Haiku with the same prompt reads 11/14; opus 14/14 on paraphrase but 23/24 on keyword, in
two runs of two. Measured on the 41 cases then recorded (`bench --rerank`, one full run each unless
stated; input tokens are the answering model's own, median over the 38 questions):

|                                                | paraphrase | keyword | code | p90 tokens | model tokens per question | latency per question |
| ---------------------------------------------- | ---------- | ------- | ---- | ---------- | ------------------------- | -------------------- |
| `ask`                                          | 7/14       | 24/24   | 3/3  | 216        | 0                         | ~0.4 s               |
| `--rerank`, haiku, depth 100, titles           | 10/14      | 24/24   | 3/3  | 222        | ≈4,600                    | ~3.5 s               |
| `--rerank`, haiku, depth 100                   | 11/14      | 24/24   | 3/3  | 222        | ≈9,500                    | ~4 s                 |
| `--rerank`, sonnet, depth 100                  | 13/14      | 24/24   | 3/3  | 222        | ≈10,900                   | ~4 s                 |
| `--rerank`, sonnet, depth 200 (default), 3 runs| 14/14      | 24/24   | 3/3  | 221–226    | ≈19,200                   | ~4.3 s               |

The `bench` floors apply to the zero-token path; `--rerank` is measured, not
floored, because a model's pick can vary by one hit between identical runs — which is also why the
default is the configuration that read 14/14 three times, not the one that read it once. A
per-question query rewrite by the model was measured too — 20/24 keyword, 6/14 paraphrase,
~3,100 tokens — and rejected: the added synonyms dilute exact matches and find no new targets.

## Bench

`repograph bench [--cases file]` runs the recorded 82 cases (40 keyword + 30 paraphrase + 12 code)
against a built graph and fails the process if any floor is missed. The recorded `bench/cases.jsonl`
is compiled into the binary, so a release build benches from any directory; `--cases` substitutes a
different file of the same shape:

- keyword 40/40
- paraphrase ≥14/30 with embeddings, ≥11/30 with `--no-dense`
- code 12/12
- p90 ≤230 tokens in both arms, counted as rendered UTF-8 bytes / 4 — a conservative proxy, since
  it counts a Cyrillic answer at roughly double what an equivalent chars/4 reading would give a
  Latin one. The no-dense arm seeds more of its answers from the generated questions, whose
  Cyrillic requirement headlines cost more bytes than a symbol or task node's; that is worth three
  tokens at p90 here, so one ceiling covers both arms

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
two runs of each arm agreeing to the case: `keyword 40/40  paraphrase 14/30  code 12/12  p90 225
tok` with embeddings, 195 median; `keyword 40/40  paraphrase 11/30  code 12/12  p90 228 tok` with
`--no-dense`, 200 median. A store that `enrich` has never touched reads 40/40, 9/30, 12/12 and
39/40, 7/30, 12/12: the floors presume the questions, and without them even a keyword case goes.
Every keyword and code case hits in both arms, which is why those two floors are exact rather than
a fraction; only paraphrase is graded on a count.

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
not just disk: importing `beauty-crm`'s graphify graph measured keyword dropping 24/24 → 23/24 and
dense p90 rising 218 → 233 tokens on the 41 cases then recorded, which breaches the p90 floor above. That is why `bench` above is
always measured against a legacy-free store, and why `import-legacy` stays a separate, opt-in step
rather than folding into `build`. Two graphify nodes that resolve to the same requirement collapse
onto one node, and the edge between them is dropped rather than kept as a self-loop — 592 of the
20,415 links on `beauty-crm`'s graph; the import prints the count.

## Measured

On its development corpus — a TypeScript monorepo with a Russian-language PRD, 825 indexed files
out of 3,599 tracked:

|                                 |                                                                                                                              |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Nodes                           | 7,525 — 3,640 `Symbol`, 1,880 `Requirement`, 1,001 `Task`, 825 `File`, 72 `Entity`, 69 `Milestone`, 20 `Invariant`, 18 `Adr` |
| Edges                           | 27,412                                                                                                                       |
| Graph on disk                   | 10.2 MB JSON                                                                                                                 |
| Graph load                      | ~19 ms                                                                                                                       |
| Lexical index rebuild           | ~120 ms                                                                                                                      |
| Tokens spent building any of it | 0                                                                                                                            |

Retrieval on the recorded 82 cases against that graph, both arms run twice with identical results:
keyword 40/40, paraphrase 14/30, code 12/12 at 195 median and 225 p90 tokens with embeddings;
40/40, 11/30, 12/12 at 200 median and 228 p90 with `--no-dense`.

The prior art on the same corpus was an LLM-extracted graph that cost **14.6 million input tokens
over 13 runs** and, measured on the 38 questions of the day, answered 0 of 14 paraphrase queries at
~1,555 tokens per answer and 11 of 24 keyword queries at ~1,027. Cost is not the only reason to
replace it, but it is the easiest one to state.

See [Bench](#bench) for the retrieval-quality floors these numbers are held to, and the ADR for the
one figure that didn't hold up on first measurement.

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
