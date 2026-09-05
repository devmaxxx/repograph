# The three-graph comparison — result, 2026-09-04

One corpus, one commit, the same 114 questions to three tools, superseding
[`2026-09-03-three-graphs-results.md`](2026-09-03-three-graphs-results.md). The protocol is
[`three-graphs.md`](three-graphs.md); the runbook is [`runbook.md`](runbook.md); the raw rows are
[`../../bench/results/2026-09-04-three-graphs.json`](../../bench/results/2026-09-04-three-graphs.json).

| | |
|---|---|
| corpus | `beauty-crm` at `502e8a6d`, in a scratch worktree (`/tmp/bench-corpus-3g`), removed after the run |
| run at | 2026-09-04 15:28 |
| cases | 82 retrieval (40 keyword · 30 paraphrase · 12 code), 32 blast (16 impact · 8 trace · 8 changes) |
| repograph | branch `bench/three-graphs-2026-09-04` @ `7479a02` (`--version` prints `0.4.0`; the crate version is separate work) — `repograph --repo . build`, no `enrich`, zero tokens — 8 316 nodes / 32 601 edges |
| graphify | 0.9.48 — `graphify update . --force` in the worktree, 0 tokens off the warm semantic cache — 26 634 nodes / 50 947 edges |
| gitnexus | 1.6.9 — `gitnexus analyze -f --embeddings --skip-agents-md .`, local embeddings, 0 tokens — 26 801 nodes / 43 745 edges |

All three built at zero LLM cost. That was a deliberate choice for this run, not a default any of
the three ships with a docs guarantee for repograph — see the paraphrase caveat below.

## A harness defect this run found and worked around

`bench/compare/run.py`'s default `--gitnexus-repo` is the corpus directory's basename. gitnexus
registers a repository by a name it derives itself — here, `beauty-crm`, for both the real clone
and this scratch worktree — not by directory name. A worktree named anything else, which the
runbook mandates precisely so the real clone is not disturbed, matches no registered name.
`impact`/`trace`/`detect-changes` then print a JSON `{"error": "...not found..."}` and exit 1;
`query` crashes with an uncaught exception. `run.py` captures either as the tool's answer
regardless of exit code. Worse, the error JSON echoes the query back — `"to": {"name":
"AvailabilityService"}` — so the substring check that scores `trace` recorded two false-positive
hits before this was caught.

The first, discarded attempt at this run scored gitnexus 0/82 retrieval, 0.0 impact recall, and a
trace hit count corrupted by that echo. It was not close to a real result and is not in the result
file. This was worked around, without touching `bench/compare/`, by passing
`--gitnexus-repo /private/tmp/bench-corpus-3g` (the worktree's absolute path), which resolves
unambiguously between the two repositories gitnexus has registered under the same name. Every
number below is from that corrected invocation. It was reported as a harness defect and fixed in
`8e40a19`, after this run: `run.py` defaults `--gitnexus-repo` to the corpus's absolute path. See
the full run notes for the reproduction.

## Retrieval — 82 questions

| | keyword | paraphrase | code | **strict** | soft | mrr | median ms | median chars |
|---|---|---|---|---|---|---|---|---|
| **repograph** | **40/40** | 9/30 | **12/12** | **61/82** | 76/82 | **0.624** | **346** | **593** |
| graphify | 22/40 | 1/30 | 9/12 | 32/82 | 45/82 | 0.312 | 1 467 | 6 688 |
| gitnexus | 0/40 | 0/30 | 12/12 | 12/82 | 61/82 | 0.061 | 1 424 | 6 246 |

The shape repeats: gitnexus is 0 strict on prose and 12/12 on code, never in the wrong file (61
soft) and never naming an id. graphify loses on both criteria. repograph leads strict and soft by
a wide margin and answers in roughly a tenth of the characters.

**repograph's 9/30 paraphrase is not a regression — it is the documented zero-token number.**
This store was built with plain `repograph --repo . build`: no `enrich`. README states plainly
that "a store that `enrich` has never touched reads 40/40, 9/30, 12/12" — the shipped 14/30 floor
presumes enrich-generated questions in the store. `repograph bench` run directly against this
store confirms the same 9/30 and exits with "bench floors not met". The 2026-09-03 run and the
2026-09-04 repograph-only run both used an opus `enrich` pass; this run deliberately did not, so
that all three tools' build costs stay comparable at zero tokens each. See **Comparability**
below — this is the single largest reason repograph's retrieval numbers here cannot be read
against either earlier document.

**MRR is a first measurement for all three tools; the old run has no rows to rescore against.**
repograph's 0.624 sits well above graphify's 0.312 and gitnexus's 0.061 — consistent with the
strict/soft shape above, and additionally showing that when repograph is wrong it is closer to
right: paraphrase MRR 0.158 despite only 9/30 strict, because many of the 21 misses still surface
a competing but related id ahead of nothing. gitnexus's 0.061 overall is almost entirely its
code-kind 0.42; every prose row scores 0 rank, consistent with 0 strict.

## Blast radius — 32 cases

### impact, 16 targets by fan-in

Recall is the share of referencing files (comments and string literals blanked) that the answer
names.

| | mean recall | files found / wanted | median ms |
|---|---|---|---|
| **repograph** | **1.000** | **123 / 123** | **42** |
| graphify | 0.750 | 57 / 123 | 392 |
| gitnexus | 0.687 | 78 / 123 | 919 |

By tier (2 hub, 3 wide, 11 narrow):

| | hub | wide | narrow |
|---|---|---|---|
| **repograph** | **1.000** (72/72) | **1.000** (25/25) | **1.000** (26/26) |
| graphify | 0.500 (24/72) | 0.333 (10/25) | 0.909 (23/26) |
| gitnexus | 0.698 (47/72) | 0.478 (12/25) | 0.743 (19/26) |

graphify's `affected` refused an ambiguous name for 4 of the 16 targets — `DatabaseService`
(hub), `TenantContextInterceptor` (narrow), `ActorResolver` (wide), `OutboxPublisher` (wide), the
identical four names it refused on 2026-09-03. Each is scored 0 recall, not skipped: a blast
radius nobody can obtain is a blast radius of nothing. The other 12 targets all scored 1.000 for
graphify — when it resolves a name at all, it is thorough.

gitnexus refuses nothing but is partial everywhere: every one of the 16 targets returned a
nonzero, sub-1.0 recall.

Every narrow repograph case is 1.000, holding the pattern from 2026-09-03 that repograph's
recall does not thin out on symbols with few callers.

### trace, 8 pairs — six real chains, two with no path

| | hit | real chains | correct refusals | median ms |
|---|---|---|---|---|
| **repograph** | **8/8** | **6/6** | **2/2** | **42** |
| graphify | 5/8 | 3/6 | 2/2 | 1 428 |
| gitnexus | 2/8 | 0/6 | 2/2 | 916 |

These are the identical 8 cases scored on 2026-09-03, and every tool's number is unchanged: 8/8,
5/8, 2/8. gitnexus still answers `no_path` to every real chain and collects its two points from
the pairs where that is correct.

### changes, 8 diffs, base → the corpus's live HEAD (`502e8a6d`)

graphify has no equivalent command.

| | symbols found / wanted | files found / wanted | median ms |
|---|---|---|---|
| **repograph** | **1 783 / 1 844** | **498 / 498** | **866** |
| gitnexus | 68 / 1 844 | 10 / 498 | 2 332 |

Every `changes` total here is measured against `502e8a6d`, not a fixed window — the eight bases
range from a two-file diff (`cbc931ba~1`) to a 341-file diff (`a7acc0f4~1`), and every one grows
as the corpus moves further past this commit. These totals compare to nothing measured at a
different HEAD, including nothing in the 2026-09-03 file, which used two bases against a different
commit entirely.

**gitnexus's `detect-changes` CLI still caps its "Changed symbols" list at 15, and `-l 500` does
not change that** — confirmed directly: `-l 5` prints 5 items, `-l 500` still prints exactly 15,
then `... and N more`. The MCP payload for base `cbc931ba~1` was read by hand, as the 2026-09-03
run did for the same base: it lists all 127 changed symbols and names 20 of the 21 the truth file
wants (every one but `HEAVY`), against 0/21 from the CLI's first-15 window — the real code symbols
sit behind roughly 78 Markdown section titles the same diff touches. The scored table above uses
the CLI number, per protocol; the other 7 bases were not hand-checked, but the shape — large
diffs bury code symbols under doc sections before the 15-item cutoff — is likely the same.

## Cost to build

| | build | size on disk | tokens to build |
|---|---|---|---|
| **repograph** | 139 s (1 s lexical + 135 s embedding 7 408 rows) | 32 M | **0** |
| graphify | 23 s, off a warm semantic cache | 123 M | 0 |
| gitnexus | 215 s | 598 M | 0 (local embeddings) |

All three built at zero tokens for this run — a deliberate choice, not a fixed floor for any of
them: repograph's `enrich` (opus, ≈$2.50) is opt-in and was left off, which is also why its
paraphrase number below the shipped floor is expected rather than a regression (see above).

## What this run confirms, and what changed

**Nothing about which tool this project uses changed.** repograph leads every measured axis in
this run exactly as it did on 2026-09-03: strict retrieval, soft retrieval, impact recall on every
tier, trace, and the only tool with a `changes` command worth reading. The eight `trace` cases,
the one measurement unaffected by any denominator change or build-configuration difference, are
numerically identical to 2026-09-03: 8/8, 5/8, 2/8.

**What is new, not a delta:**
- MRR, for all three tools, on both suites — no prior rows exist to rescore.
- The blast suite's six new narrow `impact` targets and six new `changes` bases graphify and
  gitnexus had never been asked about. graphify refused none of the six new narrow targets (all
  scored 1.000) but is thinner on the three original wide/hub targets it does resolve at all.
- The impact truth's comment/string-literal blanking, which the task brief that started this run
  says moved the old 10-target denominator from 114 to 111 — not independently re-derived here,
  since that was a different, smaller case file; recorded as background for why this run exists,
  not as a number this run reproduces.

## Comparability to 2026-09-03 — read this before comparing any number

| | comparable? | why |
|---|---|---|
| retrieval, repograph | **no** | this store has no `enrich`; 2026-09-03's did (opus). The 61/82 vs 68/82 gap is a build-configuration difference, not a regression |
| retrieval, graphify / gitnexus | **no** | same 82 questions, but scored with `rank`/mrr added and against a corpus one commit further on; graphify and gitnexus's stores were also rebuilt fresh, not reused |
| retrieval MRR, all three | **first measurement** | 2026-09-03 predates the field; its rows cannot be rescored — no answer text was stored |
| impact | **no** | 16 targets against 111-corrected truth, vs 10 targets against the old (114-count) truth. Denominator changed twice, independently |
| trace | **yes** | identical 8 cases, unaffected by the truth fix or by any build-configuration choice; every tool's number matches 2026-09-03 exactly |
| changes | **no** | 8 bases vs 2, and every base's truth is measured to the corpus's *live* HEAD — `502e8a6d` here, a different commit than 2026-09-03 used. Two `changes` totals are comparable only at the same corpus commit |
| cost to build | **partially** | repograph and gitnexus built the same way (zero tokens); graphify's number here (23 s off a warm cache) matches 2026-09-03's description, not a fresh cold-cache timing |

## Caveats that travel with these numbers

- One run per row, no repeats — the general caveat in `three-graphs.md` applies in full, including
  the `ID_TOKEN` over-match note that can make a published MRR pessimistic by an unmeasured amount.
- Latency is whole-process wall clock; gitnexus's `eval-server` was not used, matching the runbook.
- The `changes` truth did not blank comments/strings, though the `impact` truth did — a harness
  limitation carried over from `three-graphs.md`, fixed in `3f950ea`, after this run; the symbol
  fractions above were scored before it and are not restated.
- The gitnexus repo-resolution defect above is specific to running against a scratch worktree
  whose directory name does not match the name gitnexus itself assigns the repository. A run
  against the corpus's own working clone would not hit it — but running against the working clone
  is exactly what the runbook forbids, to avoid writing index artifacts into a repository someone
  is working in. The workaround (`--gitnexus-repo <absolute path>`) was a per-invocation flag at
  run time; `run.py` defaults it to the corpus's absolute path since `8e40a19`, after this run, so
  a later invocation does not pass it.
- graphify's four refusals were not "fixed" by disambiguating the name, per the runbook's explicit
  instruction — they are scored as the zero-recall misses they are for an agent that cannot
  disambiguate them either.
