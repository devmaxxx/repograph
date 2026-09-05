# The weak spots after the levers campaign — what is left, what was simulated, what to build

Written 2026-09-05 after PRs #1–#4 (`feat/dev-cases` at `c48a9d3` + the broken-pipe fix). Every number
below is either a recorded bench line (`bench/history/runs.jsonl`, `docs/bench/2026-09-05-dev-cases-results.md`)
or an offline simulation over the session's dumps (scripts named per number; all under the session
scratchpad `$S`). Simulations replay the shipped fusion over the retriever lists `dump` wrote 300 deep;
they reproduce the dense arm's recorded line exactly (40/15/12) and the lexical arm's within one case
(38/13 against the real 39/14), so they rank variants but do not ship them.

## Where the shipped configuration stands

| suite | with embeddings | `--no-dense` |
|---|---|---|
| recorded 82 (keyword / paraphrase / code, p90) | 40/40 · 15/30 · 12/12 · 220 | 39/40 · 14/30 · 12/12 · 215 |
| held-out 400 synthetic document questions, recall@5 | 103 (0.258) | 109 (0.273) |
| developer 60 (long / cross / multi / where / rule) | 10 · 12 · 9 · **0/9** · 5/9 = 36 | 11 · 11 · 10 · **0/9** · 5/9 = 37 |

The default embedder is `multilingual-e5-small`; the store is written by haiku's questions; the code
questions (`enrich --code`) live in an index of their own and reach only the `--rerank` pool.

## The four weak spots, with the evidence

### W1 — code is unreachable from prose: `where` 0/9, `cross` file anchors 11/15 absent

Coverage is not the cause. On the copy with code questions (`$S/bc-a1`: 3,463 code nodes, 546 files
with a head comment, 2,917 symbols, median 8 questions, 25,568 in all, 610 distinct files covered) every
one of the 28 `where`/`cross` file anchors carries questions — 11 to 319 per file. Ranking is:

- In the code-questions BM25 list the right file's best symbol stands first for 7 of 28 anchors,
  within the top 3 for 10, within the top 5 for 11 (`code-rank-sim.py` over `dev-dump-a3-*.json`).
  Aggregating symbol hits per file (sum of the three best hits of each file) moves that to 7 / 12 / 16.
  The dense rows of the code questions rank worse (4 / 7 / 8), and reciprocal-rank fusion of the two
  does not beat BM25 alone (5 / 11 / 15).
- A **sixth seat** reserved for the code list — added after the five, never displacing a document —
  leaves the recorded suite and the document held-out unchanged by construction, and reads `where`
  1/9 at gate 1.0, 2/9 at 0.85, 3/9 at 0.5, **4/9 with no gate at all** (`seat6-sim.py`, both arms).
  Four of nine is the ceiling because the right file is at rank 1 for only four of the thirteen
  `where` anchors. The gate's precision is the other half of the problem: at 0.85 the seat fires on
  192 of 400 document questions (every one wasted), at 1.0 on 103; a within-list margin gate
  (best/second ≥ 1.3) fires on 56 and seats nothing; agreement of the top-5 hits on one file
  (`agree 3`) fires on 13 and seats one.
- Swapping the fifth document seed for the code hit is A4 again: held-out 103 → 97 and 109 → 103
  at 0.85, 92 / 98 at 0.5. Displacement stays rejected.

So the lever has three parts, in this order: **(a)** a held-out set on the code side, so the gate and
the file ranking are designed on 400 questions instead of 9 developer cases; **(b)** the file-level
ranking of the code list; **(c)** the sixth seat, gated where the code held-out says it should be and
priced in tokens by the recorded suite's p90 ≤ 230. Behind those, the questions' *shape*: the
`where` questions ask "which file do I edit to add X"; the generated questions describe what a symbol
does. A prompt that also writes "where would you edit to change …" questions is the generator-side
lever, measurable only on the developer suite — which is why the developer suite grows first (W4).

### W2 — five seats over three lists: `multi` shares and `rule` 5/9

Every `rule` miss on the shipped store sits in the BM25 questions list at rank 3–30 and in no passage
list at all (`weak2.py` over `dev-dump-{dense,lex}.json`: INV-14 at 14, INV-16 at 30, ADR-031 at 6–8,
ADR-003 at 3); the `multi` siblings sit at questions-list ranks 2–13 and dense-questions ranks 6–17.
Two fusion changes were simulated on the shipped store's dumps and both are rejected:

| variant (`fusion-rrf-sim.py`) | recorded, dense | held-out dense / lexical | dev rule |
|---|---|---|---|
| round-robin (shipped) | 40/15/12 | 103 / 109 | 4/9 · 4/9 |
| reciprocal-rank fusion k=60 | 40/13/12 | 95 / 104 | 2/9 · 2/9 |
| RRF k=5 (≈ round-robin) | 40/13/12 | 103 / 109 | 2/9 · 3/9 |
| RRF + dense questions list, w=1.0 | 37/17/12 | 93 / — | 2/9 |
| round-robin + dense questions as a fourth list | 39/14/12 | 98 / — | 3/9 |

A sixth seat drawn from the questions list when it dominates the passages (`best_q ≥ γ · best_p`)
costs nothing on the document side by construction and buys held-out +2 to +9 (γ 1.2 → 0.85) and
`multi` +1, `rule` +0 — while firing on 80 to 274 of 400 questions, i.e. paying a sixth node's tokens
on up to 69 % of answers. Not worth a p90 that is already 220 against a ceiling of 230.

Conclusion: the fusion is not the lever for W2. The `rule` answers are reachable only through generated
questions, and those rank 3–30 because the generator writes "what does this ADR decide" questions, not
"is this allowed / what forbids it" ones — the same hole D1 opened wider (sonnet: `rule` 5/9 → 2/9 in
both arms). The lever is G14, the kind-aware prompt.

### W3 — paraphrase: 15/30 and 14/30 by default

Measured and known: `multilingual-e5-large` reads 22/30 with held-out 103 → 119 (p = 0.0009) at
0.8 s an `ask`, +0.2 GB, 26× embed time, a 2.1 GB download — shipped as `embed_model`, default
unchanged; sonnet's questions read 17/30 and 16/30 with held-out 132 / 128 (both significant) and lose
four `rule` answers — not shipped. The combination is unmeasured. The decision on the default is Max's;
this plan produces the missing numbers (G14 with haiku, G14 with sonnet, the best of those with
e5-large) so the decision is between measured configurations.

### W4 — the measurement is thin where the levers are

Nine `where` and fifteen `cross` cases decide the code side; nine `rule` cases decide G14; the code
side has no held-out set at all, so every code-side constant so far was read off the developer
suite, which the rule forbids. Two additions: a synthetic code held-out (400 questions sampled from
the code nodes' own questions, leave-one-out as `dump` already does for kind `synthetic`, judged by
file) and a developer suite v2 with 20 more `where`, 10 more `cross` and 9 more `rule` cases written
the way the first sixty were — three authors, from the corpus, never running the tool.

## Rules, pre-registered

The three-way rule stays for every retrieval lever: recorded floors in all four arms with p90 ≤ 230;
document held-out (400) not significantly worse in either arm (paired exact McNemar, α 0.05, before =
the fixture dumps `$S/ho-dense-085-before.json` / `$S/ho-lex-085.json`); developer suite not down in
either arm. Additions for this plan:

- **Design sets and validation sets are different sets.** The code seat's gate and the file ranking are
  chosen on the two held-outs (code and document); the recorded and developer suites only confirm.
  No constant is chosen by looking at the recorded or developer suite.
- **A lever for `where` must move `where`**: the code seat ships only if `where` ≥ 2/9 in both arms
  of the developer suite (v2: ≥ 6/29) on top of the three-way rule. A seat that fires and reaches
  nothing is tokens for nothing.
- **Generator levers are judged by D1's rule** (recorded floors in both enriched arms; held-out not
  worse; developer suite not down), and additionally `rule` not down for G14, since that is the hole it
  is meant to close.
- **Tokens are money**: each generation run states its estimate from character counts before it runs
  and its actual prompt/answer byte counts after; a run over 3,463 code nodes on haiku is ≈ $3–4,
  on sonnet ≈ $20; a run over 1,996 documents on haiku ≈ $2.5, on sonnet ≈ $13.6 (measured 2026-09-05).

## Not on this list, and why

- Reciprocal-rank fusion, the dense questions list as a fourth list, a sixth seat for the questions
  list — simulated above, rejected.
- A bigger seed budget (six for everyone) — ADR-001; the simulations confirm the price shape.
- Replacing round-robin's equal turns with score-weighted turns — RRF with small k is that, and it
  read exactly round-robin.
- The reranker (`ask --rerank`) — 17.9 s a question with the local model; unchanged.
