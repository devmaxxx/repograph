# Spending tokens on purpose: enrich and rerank

Everything above runs at zero model tokens, and stays that way by default. Two stages can spend
them, each behind an explicit switch, each measured on the development corpus:

**`repograph enrich`** asks a model, once per requirement-like node, for twelve questions a reader
might ask to find that node in everyday words, plus a line of synonyms. Those questions are embedded
as rows of their own for the reranker's pool and indexed for BM25 as a list of their own in every
answer. Generation is cached by passage hash in `.repograph/questions.json`, so a later `enrich`
pays only for nodes whose text changed, and a node left without questions is asked again by the next
run. Two kinds of drift are refused on the way in: a line whose letters are mostly in a script this
run did not ask for, and several questions tab-joined around the node's own id. The scripts a run
accepts are Latin and Cyrillic — identifiers and product names are Latin whatever the documents
are written in — plus the script of every language named below, so a Chinese corpus keeps its
Chinese and a Russian one still drops a generator that wandered into Urdu. Loading an older cache
splits the tab-joined lines and drops mojibake, but judges no script: the run that wrote an entry
named its own languages, and a later load knows nothing about that run. A reply with no id line
anywhere is logged as unparseable rather than as nodes the model skipped, and one that wrote its
tabs as the two characters `\t` — what the generator does when it believes it is writing a file —
is unescaped and read once before the batch is retried at full price. A batch that still comes back short
explains itself only in the reply `enrich` has already read: `enrich --keep-raw <dir>` (or
`REPOGRAPH_ENRICH_KEEP=<dir>`) leaves its prompt and reply as `<first id>.in` and `.out`, its retry
as `<first id>.retry.in` and `.out`, and writes nothing for a clean batch. On the corpus of 2026-09-02, 1,971 eligible nodes took 16 minutes at 8-way parallelism and
roughly $2.5 of haiku; the corpus is 1,996 eligible nodes now.

Every entry gets its twelve questions and its synonyms in every language the documents use — set
in `enrich_languages`, or detected from the documents themselves when that key is empty, counting
letters by script and naming any script that carries a twentieth of them. A detected list is
pinned in `questions.json` by the run that used it and read back by the next one: the list is part
of every entry's staleness hash, so a corpus with a second script sitting near that twentieth
would otherwise regenerate whole — 180 batches of haiku on the bench corpus — the day one document
moved it over the line. `enrich --detect-languages` reads the documents again and pays for what
changes; naming `enrich_languages` outranks both. Before that the questions
followed each entry's own language, which left a bilingual corpus's English half reachable only
from an English question: on beauty-crm, 168 of 2,147 entries came out English while the readers
ask in Russian, and its two ADR paraphrase cases were not in the 200-deep pool at all, so no
reranker could reach them either. Measured on a copy of the pinned
beauty-crm store: with the ADRs' questions in English both cases are outside the pool and
paraphrase reads 13/30; with a full set in Russian and English they sit at ranks 1 and 3 of the
questions' BM25 list and paraphrase reads 16/30, keyword 40/40 and code 11/11 unmoved. The second
language is paid for — 1,984 entries took 1,885 s, `questions.json` went from 3.0 MB to 4.8 MB and
the dense index from 33,533 rows to 62,874 — and a store enriched before the list existed
regenerates every entry whose own language is not the whole list, so a corpus in one language
regenerates nothing. `repograph install-agent` writes the detected list into
`repograph.toml` once, so the choice is a line a repository can see and edit; no writing command
does, because a binary released before the key existed refuses to parse a file that carries it.

`enrich --code` extends the pass to symbols with a doc comment or a body of their own and files
with a head comment — 3,475 nodes on the corpus — through a prompt that asks four Russian and four
English questions per node and forbids repeating the identifier. Those code questions are an index
of their own, searched for the `--rerank` pool and nowhere else: a store carrying them fuses byte
for byte like a store without them. Why they are kept out of the plain fusion was measured three
times and is [recorded](history.md#enrich---code-the-fifth-list-and-why-it-stays-out-of-the-plain-fusion).

The documents' generated questions join the fusion as a BM25 list of their own, and only when that
list has earned its turn: each list is asked what fraction of the query its best document actually
reached, and the questions list is admitted at 0.761 of the passage list's coverage. Two other ways
of spending those questions were measured and rejected. The constant, the two rejections and what
the admission moved are
[recorded](history.md#three-ways-to-spend-the-generated-questions-and-the-one-that-ships).

**`ask --rerank`** builds a 200-deep pool — dense passages, dense questions, BM25 passages, BM25
questions, and, when the store carries questions about code, those as a fifth list, interleaved —
and hands the model each candidate's id, title and the first 120 characters of its text to pick five
from; `--depth` changes how deep, and tokens per question scale with it. `rerank_command` reads the
prompt on stdin and writes the chosen ids one per line; a failing command is reported on stderr and
the answer falls back to the fused order.

What the model is shown decides more than which model it is, and every model read in that seat is
[recorded](history.md#what-the-reranker-is-shown-and-every-model-read-in-that-seat). Where the
arm stood when it was measured, on the 82 cases the floors use and under the small embedder that
preceded the current default:

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
