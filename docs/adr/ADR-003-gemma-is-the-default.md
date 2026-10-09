# ADR-003 · embeddinggemma-300m is the default embedder

**Status:** Accepted, 2026-10-08. Supersedes the embedder half of ADR-002; its principle stands.

## Context

ADR-002 put the small model back as the default because the large one's price landed on a first
build: 1,930 s of foreground embed, hours in the background band, and a 2.1 GB download. It named
what would reverse it: the price of the extra recall falling.

It fell. Four models read 21/30 on the paraphrase arm where `intfloat/multilingual-e5-small` reads
15/30 ([the readings](../bench/2026-09-22-embedders-results.md)), and the cheapest of them is
`onnx-community/embeddinggemma-300m-ONNX`:

| | e5-small | embeddinggemma-300m | arctic-embed-l-v2.0 |
| --- | --- | --- | --- |
| paraphrase, 82-case suite | 15/30 | **21/30** | **21/30** |
| paraphrase, widened arm of 60 | — | 35/60 | **39/60** |
| whole store embedded, 33,525 rows | 144 s | 733 s | 1,944 s |
| peak RSS on that embed | 1.8 GB | **1.3 GB** | — |
| vectors / hub cache | 51 MB / 578 MB | 103 MB / 1.2 GB | 137 MB / 2.1 GB |
| licence | MIT | Gemma terms | Apache-2.0 |

On the fixture rebuilt 2026-10-08 the enriched dense arm read keyword 40/40, paraphrase 20/30,
code 12/12, p90 244 tokens under gemma, against 40 / 14 / 12, p90 237 under the small model.

## Decision

**`DEFAULT_MODEL` is `onnx-community/embeddinggemma-300m-ONNX`.** Six more paraphrases for 5× the
small model's embed — under two fifths of the 1,930 s the large model asked when ADR-002 refused it — and
less memory than the small model at peak. Arctic stays the documented upgrade for more recall or an
OSI licence.

`UNNAMED_MODEL` stays the small model: a store with no recorded model was written by it.

## Consequences

- A reader still opens the model that wrote the store. A writer embeds with the configured one, so
  a small-model store in a repository that names no `embed_model` moves to gemma at its next
  `update`: one 1.2 GB download and one whole re-embed in the background band. Pinning
  `embed_model = "intfloat/multilingual-e5-small"` keeps it where it is.
- A model named in `repograph.toml` that differs from the store's is switched to without a manual
  `repograph model`: the next `ask` says so on stderr, answers from the stored vectors, and starts a
  background `update`.
- `bench` gains `Floors::Gemma` for the enriched dense arm only, at 40/19: one paraphrase under
  the 20/30 it read on two rebuilds, the slack the small model's floor has. Its raw
  dense arm was never measured and is not graded. The small model keeps its four rows, and the
  fixture's store is still the small model's.
- The licence is Gemma's terms, not an OSI one, and it was weighed and accepted for the default:
  repograph does not redistribute the weights — the hub download puts them on the user's machine,
  so the user takes the terms. A project that cannot take them pins the small model or arctic
  ([Embeddings](../embeddings.md#licence)).
