# Lexical adjacency: a bonus for two query words that stand together

## The miss

On the fixture rebuilt 2026-10-08 (`~/bench/beauty-crm-502e8a6d`, questions copied from the main
beauty-crm store, 2144/2146 enriched) the lexical arm reads keyword 38/40 under a floor of 39, and
the 0.5.4-era binary reads the same 38/40 on that corpus: the drop is the questions, not the code.
Both misses are near ones. `keyword/FR-WH-53` («отчёты склада») sits at rank 4 in the passage list
and 5 in the question list, and fuses to sixth. `keyword/FR-PH-43` («критерий готовности рыночному
запуску») sits at rank 4 in the question list. Each target's questions hold two of the query's
words side by side — «synonyms: отчёты склада», «критерий готовности к запуску» — and BM25 scores
words one at a time, so the rows that beat them hold the same words apart.

## The lever

`LexicalIndex` keeps a second postings map over adjacent stem pairs, and `search` adds a pair's
BM25 weight, scaled by a constant below one, when two consecutive query terms stand together in a
row. `attainable` is unchanged.

## The rule, written before the measurement

On the rebuilt fixture, against the 2026-10-08 rows recorded for ef69664:

1. Lexical keyword reaches at least 39/40.
2. No keyword or code case is lost in either arm, dense or lexical.
3. Paraphrase moves by no more than one case down in either arm.
4. Dev cases (entry points) do not fall in either arm.
5. p90 stays at or under 250 tokens in both arms.

Any one failing means the lever does not ship.

## Result

Pending.
