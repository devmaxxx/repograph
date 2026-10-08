# Text files on a list of their own — the rule, written before it is measured

**Status:** rule only. Results are appended below once measured; nothing above the results heading
changes after the first number is read.

## Why

`text_globs` is empty by default because turning it on cost recall: on the bench fixture
`text_globs = ["**/*"]` took paraphrase from 15 to 8 of 30 and held-out recall@5 from 100 to 90 of
400. A text node is one document in the same BM25 passage index and the same dense passage list as
the requirements, so a configuration file that shares three words with a question takes a seat of
the five that a requirement would have held.

## The lever

Text nodes leave the passage BM25 index and the dense passage list and ride a list of their own.
The list has two halves, the BM25 rows over an index of text nodes alone and the text nodes the
dense passage list returned, kept in their dense order. Both are seated together, under the same
coverage admission the generated-questions list is seated under: `admits(text_best,
text_attainable, passages_best, passages_attainable, QUESTIONS_GATE)`. A question whose words a
configuration file covers as completely as a requirement covers them seats the text list; a question
about a requirement does not, and the text rows are dropped from that fusion altogether.

`QUESTIONS_GATE` (0.761) is reused as it is. It is not re-derived and not tuned for this list; the
form is judged once, as written. The reranked path seats the text list unconditionally, as it does
the questions list, since there the fused order is a candidate pool and not five seats.

## The rule, written before the measurement

Measured on the fixture copy under the default embedder (embeddinggemma-300m), the same store
read twice with the same binary: once with `text_globs` empty, once with `text_globs = ["**/*"]`.

1. **Recorded suite (82 cases) loses nothing.** Keyword 40/40, paraphrase and code at the counts the
   empty reading gives (20/30 and 12/12), p90 at or under 250 tokens, and no case that hit with
   `text_globs` empty misses with it set.
2. **Dev suite (60 cases) loses nothing.** The count does not fall and no case that hit with
   `text_globs` empty misses with it set.
3. **Gain, reported.** A new set of config-file questions, written into
   `bench/text-cases.jsonl` and committed before the run, each anchored on a text file, is read
   with `text_globs` empty and set. Its gain is reported. Passing rules 1 and 2 decides the
   default; the gain is what the default buys.

Both 1 and 2 hold: ships on by default, `text_globs = ["**/*"]`. Either fails: the separate list
ships behind `text_globs` alone and the default stays empty. The numbers are reported either way,
beside the same readings for the single-list fusion this replaces where the binary at the base
commit can be run.

A hit is `bench`'s: an anchor id among the seeds or the expanded line, or an anchor file among the
seeds.

## Results
