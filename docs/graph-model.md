# What the graph holds, and how a question reaches it

Moved out of the README on 2026-09-10 so that the document an agent finds first is the one it needs
to *use* the tool. Nothing here changed in the move; this is the model behind the answers and the
semantics of the three walking commands.

## How a question becomes an answer

1. **Exact.** A word that is a known id, or the name of an indexed symbol, wins outright and scores
   above everything else. When every word is an id or a name with an uppercase letter (`asGrosze`,
   `ZERO`), the exact hits are the whole answer; a lowercase word that happens to be a symbol too
   (`money` is a test helper) leads, and the fused retrievers fill the remaining seeds.
2. **Lexical.** BM25 over `id + label + body` for every node, with Snowball stemming — Russian for
   Cyrillic tokens, English otherwise, so `штрафа` and `штрафы` are the same term. Ids survive
   tokenization whole, so `FR-PAY-22` never becomes three tokens.
3. **Dense.** A local embedding of the query, cosine-ranked against every node's stored vector
   (see [Embeddings](../README.md#embeddings)). Skipped by `--no-dense`, or when the model cannot be opened —
   no cache and no network (see [Embeddings](../README.md#embeddings) for the fallback rules).
4. **Fuse.** The lists are interleaved — rank 1 of each, then rank 2 of each — dense passages
   first, then BM25 over the generated questions (when `enrich` has written any), then BM25 over
   the passages, and the top seeds survive (with `--rerank`, a model picks them from a 200-deep
   pool instead — see [Spending tokens on purpose](../README.md#spending-tokens-on-purpose)). Reciprocal rank
   fusion was measured to bury a retriever's second hit under ids both lists merely agreed on; the
   interleave lifted paraphrase recall from 5/14 to 6/14 at +2 tokens p90. The question list was
   measured on 400 held-out generated questions: recall@5 0.445 → 0.515 beside the dense list and
   0.395 → 0.527 without it (exact McNemar p < 0.001 both), keyword and code cases unchanged, the
   no-dense paraphrase cases 3/14 → 5/14, at +3 tokens p90 with embeddings and +15 without — all
   of that on the 14-case set of the day. On the 82 cases recorded since, leading the merge with
   that list cost the no-dense arm two exact seeds — keyword 39/40 without the questions against
   37/40 with them — until 2026-09-05, when the questions list was gated on its own confidence;
   it now reads 39/40 either way. See [Spending tokens on purpose](../README.md#spending-tokens-on-purpose)
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

