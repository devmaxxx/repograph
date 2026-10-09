# Spending tokens on purpose: rerank

Everything else runs at zero model tokens, and stays that way by default. One stage can spend them,
behind an explicit switch, measured on the development corpus.

`enrich`, which asked a model for reader questions once per node and indexed them as lists of their
own, was removed in 0.6.0: under the default embedder it bought one paraphrase case of 30 (19/30
without it, 20/30 with it) for about $2.5 of haiku per corpus and a second store file. A store it
wrote keeps its `questions.json` until deleted; nothing reads it, and `build` and `update` say so
once, naming it and its `questions.bin` mirror. The measurements that shaped it stay in [the history](history.md).

**`ask --rerank`** builds a 200-deep pool — dense passages, BM25 passages and BM25 over the text
files, interleaved — and hands the model each candidate's id, title and the first 120 characters of its text to pick five
from; `--depth` changes how deep, and tokens per question scale with it. `rerank_command` reads the
prompt on stdin and writes the chosen ids one per line; a failing command is reported on stderr and
the answer falls back to the fused order.

What the model is shown decides more than which model it is, and every model read in that seat is
[recorded](history.md#what-the-reranker-is-shown-and-every-model-read-in-that-seat). Where the
arm stood when it was measured, on the 82 cases the floors use, under the small embedder that
preceded the current default and on a store that still carried `enrich`'s questions in its pool:

| | paraphrase | keyword | code | p90 tokens | per question |
| --- | --- | --- | --- | --- | --- |
| `ask` | 15/30 | 40/40 | 12/12 | 220 | 0 tokens, ~0.30 s |
| `--rerank`, sonnet, depth 200, 2 runs | 29/30 | 40/40 | 12/12 | 228–231 | 58,314 B median prompt (metered), ~4.3 s, ≈$0.03 |

Fourteen paraphrase cases gained, none lost, and both runs picked identically down to the one
chronic miss, `FR-MKT-35` — which the [rerank diagnostics](bench/2026-09-09-rerank-diagnostics.md)
found is not in the 200-deep pool at all. That document also has the pool rank of every gained case,
which is what says whether a zero-token lever could reach it.

**`--rerank-local`** is the same pool and the same pick, scored by a local cross-encoder
(`BAAI/bge-reranker-v2-m3`, exported once with `optimum-cli export onnx --model
BAAI/bge-reranker-v2-m3 --task text-classification ~/.cache/repograph/reranker`, ~2.2 GB) at
zero tokens — and **measured and rejected** as a floor candidate on 2026-09-04: 17.9 seconds a
question against a bar of one, and keyword 39/40 on the same 82 cases. Like `--rerank`, it is
answered by a resident [`serve`](serve.md) where there is one, which holds the
cross-encoder between questions instead of opening it again for each.

Both flags ship opt-in and on no floor: a model's pick can vary by one hit between identical runs,
so `--rerank` is measured and never graded ([why](history.md#why---rerank-is-measured-and-never-floored)).
Passing both is an error rather than a silent preference for one.
