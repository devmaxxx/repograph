# What the reranker is buying, where it sits, and what its prompt costs

Four gaps asked the same question from different sides: G30 (where the gains sit), G29 (the one
paraphrase never picked), G31 (a cost that was an estimate), G28 (an arm on its own ceiling). One
instrumented bench answers all four, because all four are properties of the request the model was
shown and nothing had ever recorded that request.

## Method, and what it cost

`bench` now prints two diagnostics beneath each reranked case — `pool=<rank>/<depth>` and
`prompt=<bytes>` — and a `rerank prompt:` line with median and p90 beneath the summary. Both are
read where the request is made: the pool in the order the reranker was handed it, and the prompt
built a second time so that what is metered is the string that went out rather than a model of it.
They are separate lines rather than columns because `bench/history/track.py` reads the case line's
question as everything after the token count, and a column there would have been recorded as part
of the question in every transcript from here on.

Three runs on the pinned fixture (`~/bench/beauty-crm-502e8a6d`, read-only — `bench` loads the
store and saves nothing), the 30 paraphrase cases cut out of the 82-case suite, sonnet at depth
200:

| run | arm | result | p90 |
| --- | --- | --- | --- |
| free | dense + lexical, no model | **15/30** | 234 tok |
| rerank 1 | sonnet, depth 200 | **28/30** | 231 tok |
| rerank 2 | sonnet, depth 200 | **28/30** | 231 tok |

Two model runs at ~14.6k input tokens a question ⇒ ≈$1.75 of the $2 ceiling the plan set. The
recorded 82-case runs read paraphrase 29/30 twice; this suite reads 28/30 twice, the same 28 both
times, so the difference is between suites and not between runs — the 82-case runs' extra hit is
one this cut does not contain.

## G30 · Where the gains sit — answered

Thirteen cases the model reaches that the zero-token arm does not, zero lost, and their pool ranks:

| pool rank of the gained case | cases |
| --- | --- |
| 1–5 | 0 |
| 6–10 | 1 — `INV-16` at 6 |
| 11–25 | 4 — `FR-CAL-101` 14, `N-109` 18, `FR-CRM-11` 22, `FR-STAFF-45` 23 |
| 26–50 | 0 |
| 51–100 | 1 — `FR-MIG-17` at 51 |
| 101–200 | 7 — 109, 122, 135, 152, 155, 173, 196 |

And the contrast that makes the histogram mean something: **every case both arms hit sits at rank
1–8** (1, 1, 2, 2, 2, 2, 3, 4, 4, 4, 4, 5, 5, 5, 8). The free arm's reach and the model's gains
barely overlap — one case, `INV-16` at rank 6, sits on the boundary.

So G2's «nothing cheaper left to try» is half wrong and half right, and the halves are countable.
**Five of the thirteen sit at rank ≤ 25**: a sixth seat, a second expanded line, a fusion change
that lifts a rank-14 candidate four places — those are levers that could plausibly seat them, and
a zero-token proposal should name which of these five it targets. **Eight sit at rank ≥ 51, seven
of them past 100.** Nothing that reorders a five-seat answer reaches rank 152. Those eight are the
model's alone, and a fusion change judged against them is a change judged against the model.

## G29 · `FR-MKT-35` — answered, and it is the other lever

«что видит посетитель каталога, если салон не хочет публиковать цену» reads `pool=-/200` in both
runs: **the expected id is not in the 200-deep pool at all.** G29 asked which of two levers this
case wants, and the answer is unambiguous — not the 120-character snippet, which the model never
saw for this id, but depth or fusion. No reranker at any depth this pool is built to can pick a
candidate that is not in it.

That also retires the snippet-policy idea *for this case*. It may still be right for others; this
case cannot be the evidence for it.

## G31 · The prompt, metered — closed

Thirty prompts, measured rather than divided:

| | bytes |
| --- | --- |
| min | 54,015 |
| median | **58,314** |
| p90 | **60,843** |
| max | 62,008 |

Identical byte-for-byte between the two runs, which is what a deterministic pool and a fixed
snippet width should produce, and is itself the check that the meter reads the request and not the
answer.

The README carried «≈14.4k (one prompt, estimate)» from a single captured prompt of 57,506 bytes.
The median now reads 58,314 — the single capture was representative to within 1.4%, which is worth
saying plainly: the estimate was not wrong, it was unverified, and those are different failures.
The number that changes is the older «≈19k tokens» from the 41-case pool, which this corpus does
not support.

**Bytes and not tokens, with the method named:** `rerank_command` runs `claude -p --output-format
text`, whose output is the picked ids and nothing else — the transport returns no usage block, so
this process cannot read the model's own count without changing what the command prints and how
`rerank::parse` reads it. Bytes ÷ 4 (≈14.6k tokens at the median) is a **floor** on a corpus that
is mostly Cyrillic, where a character is two bytes and a token is frequently one character. The
true count is higher; how much higher is a question for a transport that reports it.

## G28 · The three tokens have a cause — and a bar, written before the next run

The two reranked runs are identical everywhere the retrievers reach: **the same 28 hits, the same
pool rank for all 30 cases, the same prompt bytes for all 30 cases.** And yet **21 of the 30 cases
moved their rendered token count**, by −18 to +25:

| case | run 1 → run 2 | pool rank |
| --- | --- | --- |
| `FR-WH-18` | 173 → 198 | 152 |
| `FR-DM-66` | 173 → 197 | 4 |
| `FR-OPS-24` | 182 → 206 | 4 |
| `FR-RPT-13` | 195 → 214 | 173 |
| `FR-SEC-21` | 182 → 164 | 122 |
| `FR-SHELL-52` | 246 → 224 | 2 |

Retrieval is deterministic and did not move. What moved is **which five ids the model picked and
in what order**, and therefore which neighbours and headlines the renderer expanded. G28 asked
whether the three tokens at p90 were pick order, a notice or a longer headline; they are pick
order, and the arm's per-case spread at fixed input is ±25 tokens, an order of magnitude larger
than the 3 tokens that turned a run red.

**The bar, committed now, before the next run reads it** (ADR-001: a floor is written before its
number is read). On the graded 82-case suite the free dense arm reads p90 **221** and the reranked
arm **228** and **231** — the model's picks render seven to ten tokens longer at p90, against a
230 ceiling that was set for the free arm. The paraphrase-only cut used here reads 234 free and
231 reranked, which is that cut's own scale and not comparable to the graded suite's.

> `--rerank` grades against a p90 of **240 tokens** on the 82-case suite — its own bar, not the
> free arm's 230.

240 is the highest reranked reading plus nine: it absorbs the ±25 per-case wobble that pick order
alone produces at the p90 slot, and still fails an arm whose rendered answers grow by ten tokens
across the board. A bar tighter than that grades the model's word order. A bar past 250 stops
being a bar — it would pass a reranked answer that rendered thirty tokens longer than any yet
seen, which is the change this measurement exists to catch.

What this does **not** settle: whether the pick order should be pinned to the fused order instead,
which would remove the wobble rather than budget for it. That is a change to what the reader sees,
and it is a separate argument from the bar.
