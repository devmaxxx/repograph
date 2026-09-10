# The rows another vendor would fill, and why none of them is filled

G37 says the README shows Codex and Ollama as command shapes read off their `--help` and never run,
and that two accuracy levers — `e5-large` rows and the sonnet reranker — have never been stacked.
This is the attempt, and it is four refusals. Each one carries the command that would have produced
the row and the exact reason it did not, because a blank is not a row: a reader who cannot tell an
unrun arm from a bad one will assume whichever suits them.

Machine, 2026-09-10: `codex-cli 0.147.0`, `ollama version is 0.33.3`, repograph at `6538c66`.

## What *was* established, for nothing

The two command shapes in the README are still the right shapes on the installed CLIs. Checked
against `--help`, not run:

| claim in the README | `codex exec --help`, 0.147.0 |
| --- | --- |
| no prompt argument ⇒ instructions from stdin | «instructions are read from stdin. If stdin is piped and a prompt is also provided, stdin is appended as a `<stdin>` block» |
| `-m, --model <MODEL>` names the model | present |
| `--skip-git-repo-check` allows a non-repository | present |
| `-o, --output-last-message <FILE>` puts the answer on stdout | present |

| claim in the README | `ollama run --help`, 0.33.3 |
| --- | --- |
| `ollama run MODEL [PROMPT]` | «Usage: ollama run MODEL [PROMPT] [flags]» |
| `--hidethinking` | present — «Hide thinking output (if provided)» |

So the shapes have not rotted. That is the whole of what a `--help` reading can say, and the README
already says it in those words.

## The four rows, and what each would cost to fill

### Codex — the account, not the shape

```bash
rerank_command = "codex exec --skip-git-repo-check -m {model} -o /dev/stdout"
```

```
ERROR: Your access token could not be refreshed because your refresh token was already
used. Please log out and sign in again.
```

Cause, stated plainly because it was self-inflicted: establishing the Codex contract for
`install-agent` (see `agent/codex.md`), a scratch `CODEX_HOME` was given a copy of
`~/.codex/auth.json` and one `codex exec` was run under it. That consumed the refresh token, and the
real login has been unable to refresh since. `codex login status` still reports «Logged in using
ChatGPT» — it reads the file rather than the token, so it is not the check. The repair is `codex
login`, which is the account holder's to run.

**To fill this row:** `codex login`, then the 30 paraphrase cases through the command above. ~30
questions of the account's own quota, at the prompt sizes the [rerank
diagnostics](2026-09-09-rerank-diagnostics.md) metered: median 58,314 bytes a question.

### Ollama — the daemon is installed, the shelf is empty

```bash
$ ollama list
NAME    ID    SIZE    MODIFIED
```

Nothing pulled. The transport cannot be exercised without a model, and pulling one is a multi-
gigabyte download and a machine decision rather than a benchmark decision. Declined on 2026-09-10
rather than taken quietly.

**To fill this row:** `ollama pull <model>`, then `bench --rerank` with `rerank_command = "ollama
run {model} --hidethinking"`. Expect the reading to be about the model and not about repograph: the
prompt is ~58 kB of mostly Cyrillic near-duplicate candidates, which is a hard shape for a small
local model, and a bad row would be evidence about that model at that size — worth having, and
worth labelling as such when it lands.

### `e5-large` under the reranker — the store does not exist

Every store on this machine is 384-dimensional:

```
~/bench/beauty-crm-502e8a6d   dim 384, 33,533 rows
~/bench/perf-2026-09-06       dim 384, 33,533 rows
~/bench/resources-2026-09-07  dim 384, 33,533 rows
/private/tmp/g19-copy         intfloat/multilingual-e5-small, dim 384, 33,525 rows
```

and `~/.cache/repograph/fastembed` holds 578 MB — the small model. `e5-large` is a 2.1 GB download
followed by an embed of 33.5k rows, which the README records as a first build «measured in hours
rather than minutes». That is not a cost this task was given, and it is not a cost worth paying to
find out whether two levers add: the question can be asked far more cheaply on a subset.

**To fill this row, cheaply:** embed a copy under `embed_model = "intfloat/multilingual-e5-large"`
and run the 30 paraphrase cases only, both with and without `--rerank`. The interesting number is
whether the 22/30 that `e5-large` reads alone and the 28/30 the reranker reads over small rows
overlap or stack — and the pool ranks from the diagnostics say where to look: the five gains at
rank ≤ 25 are the ones a better embedder could plausibly move, and the seven past rank 100 are not.

### The code-enriched fifth list — priced, not run

`enrich --code` has never been run on the fixture, so the reranker has never seen the fifth list its
pool is built to take. At the corpus's 5,240 code nodes and the rate the README records for the
document pass (1,971 nodes ≈ $2.5 of haiku), this is roughly $6 of enrichment before a single
reranked question is asked. Declined here for the same reason as the pull: it is a spending
decision, and it belongs to whoever is paying.

## What this means for the README

Nothing changes in the command table: it already says which line is run and which are shapes, and
the `--help` check above confirms the shapes still match their CLIs. What is added is a pointer to
this file, so the next reader who asks «has anyone actually run Codex through this?» gets the
answer «no, and here is exactly what stopped it» instead of inferring one from a gap in a table.
