# Embeddings

Dense retrieval embeds by default with `onnx-community/embeddinggemma-300m-ONNX` (768-d, ONNX,
≈1.2 GB on disk, downloaded on first use) run through `ort` directly: the tokenizer and the session open concurrently at optimisation
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
written before the field existed is the small model's. `repograph model <hub id>` switches it —
[below](#choosing-the-model) — and rows another model wrote are dropped and the file rewritten, on
width as well as on name. Naming a model in `repograph.toml` by hand is enough: the first `ask`
after it says so on stderr, answers from the stored vectors, and starts a background `update` that
downloads the model and re-embeds. That holds for the catalogue's models; any other hub id is a
model a cloned repository would have this machine download and load, so the file's word is not
enough for it: `repograph model <hub id>` trusts it on this machine (in `trusted-models` beside the
machine config), and until then the default stands and stderr says why.
`REPOGRAPH_EMBED_MODEL=<hub id>` outranks both for one command, which is how a copy of a store is
measured under a second model — query that copy with `ask --stale`, or with `bench` and `dump`,
which read the store as it stands; a refreshing `ask` would claim the index for the overriding
model. It is trap 7 of the [runbook](bench/runbook.md).

`--no-dense` turns the dense stage off everywhere; on the default model that costs two hits of
eighty-two ([the arithmetic](history.md#what-turning-the-dense-stage-off-costs)).

`ask` opens the model only when a fused query needs it, and that open is most of what a fused
answer costs: ~0.30 s and ~1.7 GB on the default — the model, not
the graph. An exact-id lookup answers in ~50 ms and ~50 MB, a `--no-dense` question in ~0.1 s, since
neither opens the model or reads the vectors. What is left is paid once per process, which is what
[`serve`](serve.md) is for. `REPOGRAPH_TIMING=1` prints the stages.

Five other embedding-side levers were measured on the fourteen-case set, before the generated
questions were in the index, and none moved recall — a larger base model, BGE-M3, a quantized
MiniLM, a 512-token passage cut, a second vector per node
([the readings](history.md#five-embedding-side-levers-all-rejected)). The passage cut stays at
256 tokens and a node keeps one vector; the two models in that list were read again with the
questions in the index, in the table below.

If the model cannot be opened (no cache, no network) the two kinds of caller degrade differently on
purpose: `ask` and `update` fall back to lexical-only, say so on stderr and exit 0, while `bench` in
dense mode fails outright — its only output is an exit code, and a silent fallback graded against
the weaker no-dense floor would report green without having measured what it claims to.

Embedding times, and what a rebuild reuses rather than pays for twice, are in
[the measurements](history.md#the-first-embedding-pass-and-what-a-rebuild-reuses).

### Choosing the model

Nine models were embedded over the same corpus and read on the same 82 cases — one run each, the
whole store re-embedded per model. Keyword and code read 40/40 and 12/12 for every model but the
smallest, which drops one keyword case, so paraphrase is the column the embedder moves. Three of
the nine are offered here; [the readings](bench/2026-09-22-embedders-results.md) hold the rest.

| hub id | dim | paraphrase | embed | vectors | licence |
| --- | --- | --- | --- | --- | --- |
| `onnx-community/embeddinggemma-300m-ONNX` (default) | 768 | **21/30** | 1.0× | 103 MB | Gemma |
| `intfloat/multilingual-e5-small` | 384 | 15/30 | 0.2× | 51 MB | MIT |
| `Snowflake/snowflake-arctic-embed-l-v2.0` | 1024 | **21/30** | 2.7× | 137 MB | Apache-2.0 |

The default is the cheapest of the four models that read 21/30; the small model it replaced reads
15/30. A reader answers with the model that wrote the store, but a writer embeds with the
configured one: a store the small model wrote, in a repository that names no `embed_model`, moves
to the default at its next `update` — one 1.2 GB download and one whole re-embed, in the background
band. Pin `embed_model = "intfloat/multilingual-e5-small"` to stay on the small model.
`Snowflake/snowflake-arctic-embed-l-v2.0` is the upgrade, and the choice under an OSI licence: a
widened arm of 60 paraphrase cases separated the four, and it took 39/60 against the default's
35/60, for 2.7× the embed and 137 MB of vectors where the default writes 103 MB. The conditions, the other seven candidates, the flips and
the caveats are in [the readings](bench/2026-09-22-embedders-results.md).

`repograph model` prints that table with the store's own model marked, and
`repograph model <hub id>` changes it:

```console
$ repograph model Snowflake/snowflake-arctic-embed-l-v2.0
model: intfloat/multilingual-e5-small → Snowflake/snowflake-arctic-embed-l-v2.0
model: 2.1G of files in the hub cache, fetched once
model: 2.7× the default's embed — 1944 s for the bench fixture's 33,525 rows
model: 137 MB of vectors for that corpus, against the other model's 51 MB on it
model: 384-d → 1024-d, so every row is re-embedded and the whole index rewritten, not extended
model: opened in 5.2s, 1024-d vectors
model: embed_model = "Snowflake/snowflake-arctic-embed-l-v2.0" in /repo/repograph.toml
embedded 33525/33525 (14.5 rows/s, ~0 s left)
dense: embedded 33525 rows in 2317.4s
```

The model is opened before anything is written, so an id with no ONNX export fails with the hub's
own error and leaves the repository exactly as it was. `repograph.toml` is then rewritten in place —
only that one value, comments and everything else kept — and the store re-embedded, which
`--no-embed` stops if you would rather pay for it later.

### Licence

The default's weights are published under the [Gemma Terms of Use](https://ai.google.dev/gemma/terms),
not an OSI licence: they carry a prohibited-use policy that Google may update, and anyone who
redistributes the weights passes those terms on. repograph does not redistribute them — the files
are fetched from the Hugging Face hub onto the machine that runs it, so whoever runs repograph is
the one taking the terms. The default stays on gemma because it reads arctic's recall on the
recorded arm for under two fifths of its embed, at 1.3 GB peak RSS ([ADR-003](adr/ADR-003-gemma-is-the-default.md)).

A project whose policy admits only OSI licences pins another model in `repograph.toml`:
`Snowflake/snowflake-arctic-embed-l-v2.0` (Apache-2.0) for the same or better recall, or
`intfloat/multilingual-e5-small` (MIT) for the cheapest embed.
