# ADR-002 · Two defaults multiplied, and the embedder goes back to the small model

**Status:** Accepted, 2026-09-07. Reverses the default set at `35357c1` (2026-09-05, "the large
model is the default"). ADR-001 and its nine amendments stand; so does every measurement behind the
default this reverses.

## Context

`intfloat/multilingual-e5-large` became the default embedder at `35357c1` on a reading nobody has
questioned since and nothing here questions. On the pinned `beauty-crm` fixture it reads
**paraphrase 22/30** where `intfloat/multilingual-e5-small` reads 15/30, with keyword 40/40 and code
12/12 identical on both, and held-out 103 → 119 of 400 (+19 −3, p = 0.0009)
([the dev-cases results](../bench/2026-09-05-dev-cases-results.md)). Across the 82 recorded cases
that is 74/82 against 67/82: seven paraphrases, and nothing else in the suite.

What the seven cost, every figure read from a run recorded in this repository:

| | `e5-small` | `e5-large` | measured in |
| --- | --- | --- | --- |
| paraphrase · keyword · code, enriched fixture | 15/30 · 40/40 · 12/12 | **22/30** · 40/40 · 12/12 | [dev cases](../bench/2026-09-05-dev-cases-results.md), [0.5.0 gaps](../bench/2026-09-05-0.5.0-gaps-results.md) |
| held-out, 400 questions | 103 | **119** (+19 −3, p = 0.0009) | [dev cases](../bench/2026-09-05-dev-cases-results.md) |
| weights on disk | 470 MB | 2.1 GB | the hub cache, and [Embeddings](../../README.md#embeddings) |
| fp32 weights a rebuild touches | 0.45 GB | 1.63 GB | [the unnoticeable results](../bench/2026-09-07-unnoticeable-results.md) §5.5 |
| whole store embedded, 33,525 rows, foreground | **214 s** | 1,930 s | [what every command costs](../bench/2026-09-07-resource-usage-results.md) §1.1 |
| max RSS on that embed | 1.63 GB | 2.15 GB | the same table |
| `ask --stale` | 0.31 s, 1.43 GB | 1.18 s, 1.74 GB | the same document, §1.2 |
| `bench`, the 82 cases | 1.53 s, 1.54 GB | 4.81 s, 1.74 GB | the same table |

**The argument that is new, and the only reason this was reopened.** When the large model was made
the default, a whole-store embed cost 32 minutes. In the same unreleased version
`priority = "background"` became the default for every writer, because a rebuild has nobody waiting
on it and the band hands the person at the keyboard their machine back — their own six-thread
compile slows by 1.7% instead of 15.7%
([the unnoticeable results](../bench/2026-09-07-unnoticeable-results.md) §4.1). The band's price is
wall time, **4.1–4.5×** on the runs that were taken.

Nobody has finished a whole-store rebuild in the band on this machine: three attempts starved on a
working laptop, which is [G20](../bench/next-version-gaps.md). So the product is a ratio and not a
wall, and it is written here as one — 4.1–4.5× of 1,930 s is **2.2 to 2.4 hours**, against a quarter
of an hour for the small model's 214 s. The one partial reading agrees on the order of magnitude and
on nothing finer: the pre-change binary reached 7,168 of 33,525 rows in 34.7 minutes in the band,
its rate still climbing (§4.4 of the same document). A first build is smaller than a whole-store
re-embed — it writes the 7,408 passages and not the 26,117 generated questions, which do not exist
until `enrich` runs — and how much smaller is not measured, because rows are not equal cost and the
passages are the long ones. Hours rather than minutes, either way the ratio is taken.

**Two defaults were decided separately and they multiply.** Neither was wrong on its own evidence.
The embedder was chosen against recall and priced in foreground seconds; the band was chosen against
what a person feels and priced in a ratio nobody had yet applied to it. Their product — what someone
waits through on the first build after installing the tool — was no one's measurement, and it is
about four times the number the embedder decision was taken on.

## Decision

**The default `embed_model` returns to `intfloat/multilingual-e5-small`.
`intfloat/multilingual-e5-large` stays fully supported as one line of `repograph.toml`.** Nothing
about the large model is removed, deprecated or made harder to reach: its floors stay in `bench`
keyed by the embedder, `vectors.json` still records it, and every reader still opens whatever model
wrote the store it is reading.

**The principle this rests on.** A default is the answer given to someone about whom nothing is
known. Someone installing repograph for the first time does not know whether they will ask
paraphrased questions; they do know they are waiting on a 2.1 GB download and a first build measured
in hours. The cost is immediate, visible and paid by everyone. The benefit is deferred, discovered
later, and collected only by the subset whose questions need it. Where the two are that asymmetric,
the cheap side belongs in the default and the expensive side belongs in a documented line.

## The counter-argument, which is strong and is not disposed of

Paraphrase recall is what this project is for. ADR-001 exists because a paraphrase number travelled
from a design note into a plan as though it had been measured; the README's headline table sells
that axis against graphify; every gap round since has been an attempt to move it. This decision
hands back seven of the fifteen paraphrases the large model gained — half the gain, on the one axis
the project is about — and it does so for wall time and disk, which are the cheapest things a person
has.

Three facts make that acceptable. They are the whole of the defence, and if one of them stops
holding this decision should be retaken:

- **15/30 is not a poor number; it is the number this project was built and compared on.** graphify
  reads 0/14 on the head-to-head set. Every floor in `bench`'s small-model column was measured on
  the small model, and the tool shipped against those floors.
- **The switching cost is bounded, and nobody is stranded.** A store records the model its vectors
  were written with and every reader opens the recorded one, so an existing large-model store keeps
  answering with the large model after this change, with nothing to do and nothing to re-embed.
  Moving is one line and one `repograph embed`; changing your mind twice pays both embeddings —
  214 s + 1,930 s, under 36 minutes of foreground work on the whole fixture. And no released install
  is touched at all: the last tag is `v0.4.0`, whose embedder is the small model by a hard-coded
  constant, and the large default landed after it in a 0.5.0 that has never been tagged. This
  reverses a default that has not yet reached anyone.
- **The large model is one line away, and the line sits where the cost is met.**
  [Embeddings](../../README.md#embeddings) states both models' recall and both models' prices beside
  each other rather than naming a winner.

What would reverse this again: paraphrase recall on the small model turning out to be the binding
constraint for a real corpus in someone's hands, or the large model's price falling.

## Consequences

- `DEFAULT_MODEL` (`src/index/embed.rs`) is the small model; `UNNAMED_MODEL` keeps holding the same
  string as a **separate** constant. A store written before the field existed holds small-model rows
  as a matter of history, not as a matter of what today's default happens to be, and tying the two
  together would make an old 384-d store re-embed itself whole the next time the default moves.
- **The floors do not move, and no store changes how it is graded.** `bench` keys the dense floors
  by the model the rows were written with (0.5.0), so a store under the new default is graded
  against the small model's ≥14/30 enriched and ≥9/30 raw — the floors every recorded run before
  2026-09-05 was graded against — and a large-model store against ≥22/30 and ≥17/30, exactly as
  before. `bench.rs` pins the large model's name by literal rather than to `DEFAULT_MODEL` for this
  reason; the comment there anticipated this flip before it happened.
- `repograph.toml` follows the default rather than pinning the large model. That file's stated job
  is to set every key to the shipped default so it can double as the README's worked example, and a
  pin would have made the example a lie about the tool it configures.
- The README states the large model's numbers wherever it used to say "the default", with the model
  named. No measurement was deleted to make the flip read cleanly.
- **What could make the whole trade-off moot, and what it now costs to find out.** The hub carries
  `onnx/model_qint8_avx512_vnni.onnx` for both models — 562 MB for the large one, **118 MB for the
  small** — and the small model's copy is already in `~/.cache/repograph/fastembed`. Measuring int8
  on the default therefore costs no download at all, where [G25](../bench/next-version-gaps.md) was
  written as though it cost 562 MB; that sentence is corrected. If int8 held the recall at a
  fraction of the weights, the memory floor under every reading in both resource rounds would move
  and the size question would be worth reopening from the other end. **Nothing here measured it.**
  G25 stays deferred with the three questions it already lists: arm64 kernels for an AVX-512 build,
  vectors that differ from the fp32 ones, and a weight file the store does not record.
