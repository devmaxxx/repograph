# Performance and token-funded accuracy — design (2026-09-05)

The second plan after the levers campaign. The first (`2026-09-05-weak-spots-design.md`) closes
retrieval weak spots without spending tokens at query time; this one takes the other two axes
Max named on 2026-09-05 — the wall time of a fused `ask`, and accuracy bought with model tokens
where the zero-token levers are exhausted. Every number below is read from a transcript under
`$S` (the session scratchpad, `/private/tmp/claude-502/-Users-max-Documents-projects-repograph/244d7285-7f87-48c4-a4bc-379833f916d7/scratchpad`);
the file is named beside each one. Nothing here is implemented.

## Where the shipped `ask` spends its time

`REPOGRAPH_TIMING=1 repograph ask --stale` on the fixture `/Users/max/bench/beauty-crm-502e8a6d`
(binary at `37416b2`, `$S/perf-stages-dense.txt`, `$S/perf-stages-lexical.txt`):

| stage                        | dense arm | lexical arm |
| ---------------------------- | --------: | ----------: |
| graph loaded                 |   17.0 ms |     17.5 ms |
| ids ready                    |    1.1 ms |      1.1 ms |
| questions ready              |    3.6 ms |      3.7 ms |
| vectors loaded               |   16.2 ms |           — |
| model opened                 |  480.3 ms |           — |
| query embedded and searched  |   13.8 ms |           — |
| answered (BM25 built, fused) |   50.5 ms |     53.1 ms |
| total                        |  582.5 ms |     75.5 ms |

Wall time by `/usr/bin/time` agrees: 0.72–0.77 s dense, 0.08 s lexical (`$S/perf-ask-wall.txt`).
Eight tenths of a fused answer is opening the embedding model, and the query itself costs 4–5 ms
(`$S/perf-open-runs.txt`).

### What "model opened" is made of

`Embedder::open` runs `fetch` (the hf-hub cache lookup), then the tokenizer and the ONNX session
in parallel. Measured three times each, release build (`$S/perf-open-parts.txt`):

| part                                          | run 0    | run 1    | run 2    |
| --------------------------------------------- | -------: | -------: | -------: |
| `fetch`, total                                | 299.2 ms | 236.5 ms | 253.4 ms |
| — the four cached files                       |   0.7 ms |   0.1 ms |   0.1 ms |
| — `onnx/model.onnx_data`, absent for e5-small | 238.2 ms | 255.5 ms | 242.6 ms |
| `load_tokenizer` (16.3 MB `tokenizer.json`)   | 230.9 ms | 237.2 ms | 210.9 ms |
| `load_session` (448.5 MB `model.onnx`)        | 216.6 ms | 204.8 ms | 198.5 ms |

The absent file is looked up on the network: hf-hub finds no cached copy and asks
huggingface.co, which answers 404 in a quarter of a second. Every fused `ask` pays that round
trip, and an offline machine pays the connect failure or its timeout instead. It is the single
largest avoidable cost and changes no answer.

The tokenizer and the session cost the same; they already overlap, so open ≈ fetch + max(tokenizer,
session) ≈ 250 + 235 ms, which is the 480 ms the stage shows.

### What does not help: the model file

Session-open variants, same machine, same run (`$S/perf-open-variants.txt`):

| variant                                             | open            | query    | cosine to fp32          |
| --------------------------------------------------- | --------------- | -------- | ----------------------- |
| fp32, graph optimisation Level1 (shipped)           | 134–137 ms      | 5.2 ms   | 1                       |
| fp32, optimisation Disable                          | 96–205 ms       | —        | 1                       |
| fp32, optimisation Level3                           | 109–114 ms      | —        | 1                       |
| fp32 read into memory (44–58 ms) + commit_from_memory | 101–110 ms    | —        | 1                       |
| fp32 with external data (`model.onnx_data`, 448 MB) | 90–256 ms       | 5.2 ms   | 1.0000 ×4               |
| int8 (`model_qint8_avx512_vnni.onnx`, 113 MB)       | 167–188 ms      | 3.4 ms   | 0.9963–0.9976 ×4        |

(The session opened faster in this run than in the parts run — 135 against 200 ms — the machine
was quieter; the ratios are what matter.) Nothing here moves the open by more than the noise:
the session is a quarter of the open, the file format is not the lever, and int8 changes the
vectors (cosine 0.996–0.998 to fp32), so it would have to be measured under the three-way rule
for a gain of 1.5 ms per query and one-off embed time. Not planned.

### The real ceiling: one process per question

After the network round trip is gone, a fused `ask` still opens a 16 MB tokenizer and a 448 MB
session per process — about 235 ms that no file trick removes. `watch` already keeps a graph,
a manifest and (after the first change) the model resident. A resident process that answers
questions is the only way a fused answer approaches the lexical arm's 75 ms.

## Levers

### Performance

**P1 — no network round trip in `fetch`.** Try `onnx/model.onnx_data` only when the cached
`onnx/model.onnx` is a graph-only stub (under 64 MB; the small model's is 448.5 MB, the large
models' stubs are under 2 MB with the weights beside them). Expected: model opened −240 ms,
dense `ask` 582 → ~340 ms, and the dense arm works offline. Answers byte-identical.

**P2 — open the model while the lexical work runs.** Start
`Embedder::open` on a thread right after the vectors are loaded (which name the model), when the
query is not an exact id or symbol and dense is on; join it on the first fused query. The
lexical work it overlaps — ids, questions, three BM25 builds and the fusion — is 55–70 ms, so the
gain is bounded by that. Answers byte-identical.

**P3 — `repograph serve`: a resident answerer.** `watch`'s loop plus a Unix socket at
`.repograph/serve.sock`; `ask` connects when the socket answers a version handshake and falls
back to in-process otherwise (no socket, stale socket file, another binary's version, a
`--no-serve` flag or `REPOGRAPH_NO_SERVE`). The server refreshes the graph the way `ask` does
before every answer (the same `graph_for_ask` contract, `--stale` honoured per request), keeps
the model, vectors and BM25 indexes resident, answers one request at a time, exits after an
idle period. Output is produced by the same `query::render`, so it is byte-identical by
construction and checked by a test that compares socket and in-process output on every recorded
and developer case. Expected: dense `ask` under 100 ms after the first, lexical under 30 ms.

Not planned, with the number that says why: int8 and external-data model files (above);
persisting the BM25 indexes (the three builds are inside the 50 ms "answered" stage and P3 keeps
them resident anyway); `enrich` wall time (already parallel over batches, `--parallel`);
`embed` throughput (103 s once per corpus, README).

### Accuracy for tokens

**A1 — `ask --expand`: the question rewritten by a model before retrieval.** Measured before
planning with a proxy that needs no code change: haiku, through the `ENRICH_COMMAND` shape,
turns the question into (line 1) the same question in the vocabulary a spec would use and (line
2) four to six keyword phrases; the fixture is asked with `--json --seeds 10`, `--stale`, in both
arms, and hits are judged by the `bench::found` rule (id anchors in the five seeds or the
expansion, file anchors in the five seeds). Variants:

- R1 — the rewrite replaces the question;
- R2 — the keywords are appended to the question (one query string, both arms see it);
- R3 — round-robin of the base seeds and the R1 seeds, the first five (a proxy for fusing a
  second query into the lists);
- R4 — a second prompt rewrites the question as two short developer questions (the style the
  generated index holds), replacing it; R5 — round-robin of the base seeds and the R4 seeds.

Selection rule, written in `$S/rulings.txt` before `$S/qrw-full.txt` was read: the variant is
chosen on the held-out 400 alone — the largest sum of dense and lexical hits that loses no more
than it gains in either arm (McNemar, α 0.05, against base); recorded 82 and developer 60 are
validation only. A variant not significantly better than base on the held-out in at least one
arm is recorded and not planned.

One haiku call through `claude -p` costs 2.92 s wall (`$S/perf-haiku-call.txt`), most of it the
CLI's own start-up; so whatever the variant, `--expand` is opt-in with a stated latency, never
the default. Results: the table in the plan's Task A1, from `$S/qrw2-full.txt` — the first run's held-out rows (`$S/qrw-full.txt`) leaked (the held-out questions are generated questions still in the fixture's index; base read 372/400) and were replaced by a run on `$S/bc-ho`, the store with those 400 questions and their dense rows removed, whose base of 107/114 sits beside the LOO dump's 103/109. Verdict by the rule: no variant is significantly better than base on the held-out in either arm — R1/R4 (replacing the question) lose 84–92 of 400, R2/R5 are within noise, R3 is worse in the dense arm (p 0.016). `--expand` is recorded and not planned.

**A2 — a kind router for the seats.** Only after the weak-spots plan's Task 3 (the code seat
gated on a score) is measured: if the gate reaches `where` below 4/9 on the developer suite v2, a
haiku call classifies the question (`where` / `rule` / `cross` / other) and the seat fires on the
class instead of the score. Same cost and latency shape as A1; measured under the same rule. Not
planned until Task 3's number exists.

**Generator and embedder.** Sonnet questions with the kind-aware prompt (G14) and e5-large as
the store's model are the weak-spots plan's Tasks 6 and 7; they are not repeated here.

## Rules, pre-registered

1. A performance change ships only when `dump` of the four arms (`--no-dense` × enriched/raw,
   recorded suite and developer suite) is byte-identical before and after on the fixture, and
   `bench` prints the same lines. Timings in docs are medians of five `REPOGRAPH_TIMING` runs on
   the fixture, both arms, named transcripts.
2. `serve` is byte-identical to in-process by test (every case of `bench/cases.jsonl` and
   `bench/dev-cases.jsonl`, both arms, socket against in-process output) and never answers from a
   graph older than an in-process `ask` would have read: the handshake carries the binary's
   version and the client falls back on any mismatch.
3. A per-question model step is default-off. It ships behind a flag only if the flag arm passes
   the three-way rule against the plain arm — recorded floors held, held-out 400 not
   significantly worse in either arm, developer suite not down in either arm — and its p50 wall
   time and tokens per question are stated in README next to `--rerank`'s.
4. No constant, threshold or variant is chosen by looking at a validation suite; held-outs
   design, recorded and developer suites validate; rules are written before numbers and not
   changed after.
5. Every number in a doc, a commit or a ledger comes from a named transcript under `$S` or a test
   run; token costs are estimates and say so.

## Non-goals

- Changing the default embedder or the generator (Max's decisions, open).
- Persisting BM25 indexes, mmap-ing vectors, a smaller tokenizer: each is inside the noise or
  made moot by P3.
- An MCP server: `serve` is a socket for this binary's own `ask`; other readers keep `watch`.
