# The agent surface, read on twelve tasks

What this branch changes is what a coding agent is told about the graph and when. The claim: a
29.5 kB block of prose at the top of every session can be replaced by a 1.3 kB stanza plus a brief
the store writes at `SessionStart`, without the agent finding less. Whether the Bash interceptor —
the graph's top three lines injected beside a search — is worth its own tokens is a separate claim,
and it had a rule committed before any number was read.

The harness is `bench/agent/`: twelve tasks (`tasks.jsonl`) on beauty-crm at `502e8a6d`, each run as
a headless `claude -p --output-format stream-json`, scored on the result text and on the
`modelUsage` the stream reports. `run.sh <config> <model>` swaps a `.claude/` overlay into a
disposable worktree and keeps every byte the agent streams. Configurations: **B** today's surface
verbatim, **C** the planned one (stanza + brief + interception), **C-rem** the same with
`REPOGRAPH_HOOK_INTERCEPT=0`.

## The verdict

| configuration | runs | hits | tokens | cost | asked | grepped | tokens per hit | cost per hit |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| B — today's surface | 3 | 4, 3, 5 /12 | 3.87 M, 3.92 M, 2.90 M | $2.35, $2.36, $2.07 | 23, 13, 12 | 27, 27, 26 | 966,552 · 1,305,210 · 580,168 | $0.587 · $0.787 · $0.413 |
| C — stanza + brief + interception | 2 | 11, 11 /12 | 1.52 M, 1.42 M | $1.12, $1.06 | 17, 14 | 5, 4 | **137,991 · 129,243** | $0.101 · $0.097 |
| C-rem — the same, reminder only | 2 | 11, 9 /12 | 2.13 M, 1.52 M | $1.17, $1.07 | 16, 15 | 6, 7 | 194,011 · 169,036 | $0.107 · $0.119 |

All sonnet, no task capped. Rows in `bench/history/runs.jsonl` as `agent:<config>+sonnet`.

**F1 — priming. Passes.** `tokens(C) ≤ tokens(B)` and `hits(C) ≥ hits(B)` in both runs, and not
narrowly: 1.42–1.52 M against 2.90–3.92 M, 11 hits against 3–5. The stanza and the brief ship. Read
the magnitude with refusal 5 below — B's own instructions do not work in a bare worktree, so part of
that gap is B's install and not B's prose.

**F2 — interception. Ships on by default.** The rule: *interception ships when its tokens per hit
are lower than reminder-only's; the rule was committed before the number was read and is not
renegotiated after.* On sonnet, twice: 137,991 and 129,243 against 194,011 and 169,036 — the two
ranges do not overlap, so the worse C run still beats the better C-rem run — with hits 11/11 against
11/9. The default stays on and `REPOGRAPH_HOOK_INTERCEPT=0` remains the way out.

The rule says *on both models*, and there is no opus half to read: Max's 2026-09-07 budget decision
gives opus one configuration, not two, and the plan that records it says in the same breath that
"the F1–F3 verdicts are sonnet's". So the rule as literally written could not have been satisfied by
the sweep it was written for. The verdict above is sonnet's, twice, and says so.

**F3 — the scout.** Not run. `repo-scout` ships because it costs nothing uninvoked; its measured
pair against C is not in this document.

What the tool counts say is worth more than the verdict: C greps **4–6 times** where B greps 26–27,
and asks the graph 14–17 times where B asks 12–23. The surface did not make the agent work harder;
it moved the same work off `rg` and onto the index.

## What the first reading measured, and how that is known

The 2026-09-09 runs are labelled B, C and C-rem in nobody's memory but this paragraph: they are all
three configuration **B**.

`run.sh` laid the overlay down once, before the task loop, and reset the worktree before every task
with `git checkout -- .` followed by `git clean -fdq -e .repograph`. beauty-crm tracks its own
`.claude/` — 45 files — and its `.claude/CLAUDE.md` is byte-identical to what `overlays/build.sh`
copies out as configuration B. The checkout restored B over the overlay and the clean removed what
the overlay had added, before the first task ran:

```
$ rm -rf $WT/.claude && cp -R overlays/C $WT/.claude && wc -c $WT/.claude/CLAUDE.md
    1295
$ git -C $WT checkout -- . && git -C $WT clean -fdq -e .repograph && wc -c $WT/.claude/CLAUDE.md
   29543
```

Three corroborations, each independent of that one:

- The median first-request input was **45,904 tokens** in the B run, **45,904** in the C run and
  **45,902** in the C-rem run. A 28 kB difference in instructions is about 10,000 tokens; it was not
  there because there was no difference. With the fix it is: C's median first request is **35,705**.
- The `SessionStart` brief (`repograph: 3076 doc nodes …`) appeared in **0 of 36** transcripts. In
  the four runs since the fix it is in every one.
- Both `CLAUDE.md` locations are read. A scratch repository with a marker in `./CLAUDE.md` and
  another in `./.claude/CLAUDE.md`, asked which markers it could see, named both — so the restored
  file was in context, not merely on disk.

The fix is one copy moved: the overlay is laid down **after** the reset, inside the loop, and a
`cmp` against the overlay's own `CLAUDE.md` fails the run rather than measuring the wrong thing
again.

Those three sonnet rows are kept, relabelled `agent:B+sonnet`, because as readings of B they are
sound — and three runs of one unchanged configuration are the only estimate of this harness's noise
that exists.

## The noise floor, from three runs of one configuration

| run | hits | tokens | tokens per hit |
| --- | --- | --- | --- |
| B sonnet #1 | 4/12 | 3,866,207 | 966,552 |
| B sonnet #2 | 3/12 | 3,915,629 | 1,305,210 |
| B sonnet #3 | 5/12 | 2,900,841 | 580,168 |

Hits range 3–5, tokens 2.90–3.92 M, and **tokens per hit spans 2.25×** with nothing changed between
runs. `track.py report` reads three of the twelve tasks as flaky (`req-exact`, `req-words-1`,
`req-words-miss`, two flips each in three runs).

That is the number to hold beside every other number here. It is why F2 is read on two runs per arm
and why its non-overlapping ranges matter more than its ratio: a single pair of runs on this harness
cannot resolve a 2× difference, and two pairs whose ranges do not touch can.

The C arms are quieter than B was — 11, 11, 11, 9 hits across four runs, with ten of the twelve
tasks answered identically in all four. `rename` misses in all four; only `trace-2` and `cross`
move, and only in the fourth run. A surface that answers more also varies less, which is itself a
result.

One opus run, of configuration B (it was taken before the defect was found):

| run | hits | tokens | cost | asked | grepped | tokens per hit |
| --- | --- | --- | --- | --- | --- | --- |
| B opus | 9/12 | 3,090,595 | $5.33 | 41 | 16 | 343,399 |

Opus asked the graph 41 times and grepped 16; sonnet, on the same instructions, asked 12–23 and
grepped 26–27. The two models read the same surface differently, which is worth more than the hit
counts: a stanza tuned on sonnet's behaviour is not tuned on opus's.

## Priming: what a session is told

| configuration | root `CLAUDE.md` | `.claude/CLAUDE.md` | instruction bytes | median first request |
| --- | --- | --- | --- | --- |
| B — today's surface | 7,322 B | 29,543 B | 36,865 B | 45,904 tokens |
| C — stanza + brief | 7,322 B | 1,295 B | 8,617 B | 35,705 tokens |

**10,199 tokens off the first request of every session**, and off every later request in it, because
the prompt is carried forward. The root file is beauty-crm's own, names repograph zero times, and is
constant across arms — which is what makes the comparison a comparison.

The brief that replaces those bytes is **547 B**:

```
$ repograph prime --repo <fixture> --no-dense | wc -c
     547
```

Eight lines: node counts, whether the questions are written, which embedder the vectors belong to,
how many families the documents define, and the five commands. It is regenerated at every
`SessionStart` and after every compaction, so it is current where a prose block is as old as whoever
last edited it.

## Hook latency

Twenty firings each, wall clock from spawn to exit, against a 78 MB store:

| path | what it runs | p50 | p95 |
| --- | --- | --- | --- |
| `PreToolUse` reminder | nothing — a counter file | 32 ms | 35 ms |
| `SessionStart` brief | `prime --no-dense` | 42 ms | 49 ms |
| `PreToolUse` interceptor, no `serve` | `ask --stale --no-dense --seeds 3` | 79 ms | 94 ms |
| `PreToolUse` interceptor, resident `serve` | the same, over the socket | 67 ms | 73 ms |

Warm page cache. The first firing of a session pays the cache instead: `prime` p50 96 ms / p95
125 ms, the interceptor's `ask` p50 193 ms / p95 393 ms. All of it is inside the 5 s the hook allows
itself and the 10 s the settings file allows the hook.

The resident `serve` buys the tail, not the median — 393 ms → 73 ms at p95 — and it buys nothing on
the `SessionStart` path, because `prime` does not go through the socket. What it is for is the
agent's *own* `repograph ask`: 0.08 s resident against 0.77 s after a model drop and ~0.4 s against
a cold process.

## Every refusal

1. **The 2026-09-09 runs measured configuration B three times over.** Stated above with its proof.
   Cost: three sonnet runs and one opus run, about $12.10 of live agent time, spent on a comparison
   that never happened.
2. **The opus arm cannot decide F2, by construction.** The rule reads "on both models"; the budget
   decision that followed it gives opus one configuration. The verdict is sonnet's, as the plan
   recording that decision already said it would be. No opus run of C or C-rem exists, so nothing
   here says the interceptor pays on opus — and opus's own tool counts (41 asks to 16 greps) say it
   uses the surface differently enough that the question is open, not answered by analogy.
3. **No configuration A run.** The control for "what does an agent do with no graph at all" was not
   taken. Nothing here says the graph beats grep; it says what one graph surface costs against
   another.
4. **No scout arm (F3).** `repo-scout` ships uninvoked and unmeasured.
5. **Overlay B's own instructions do not work in the worktree.** 14 of B's 23 repograph invocations
   went through `pnpm exec repograph`, which fails in a bare `git worktree` under beauty-crm's
   `.npmrc`. B's rows are today's surface *minus a working install*, so F1's direction is sound and
   its magnitude is not attributable to prose alone.
6. **`req-exact` is ambiguous.** It asks for the requirement governing one behaviour and expects
   `FR-CAL-93`; `FR-CAL-95` answers the same words defensibly. Its flips across the three B runs are
   partly the task's fault. All four C-family runs hit it, which does not clear it.
7. **`rename` misses in all four C runs and never hit in B either.** Nothing in this document
   explains that task; it is the next thing to read, not a fact about the surface.
8. **Budget caps.** Sonnet runs cap at `--max-budget-usd 0.30` and the opus run at 0.60. Two tasks
   capped in B sonnet #2 and one in the opus run; none in the four C-family runs.
9. **The opus run's scoring step did not run.** `run.sh` was edited while that run was in flight,
   which moved bash's read offset past the tail of the script. The twelve transcripts are complete
   and were scored by hand with the same `score.py`; the row's note says so.
10. **Twelve tasks, no significance claimed.** These are counts read across runs by the history —
    now with a measured noise floor that says how little a single run means.
