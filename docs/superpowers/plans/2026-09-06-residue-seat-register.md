# The residue, the seat and the register — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land, in the 0.5.0 line, whichever of three changes clears a rule committed before its numbers are read: the coverage denominator that stops rewarding an index for a narrow vocabulary (G8's residue), one seat for the code list on the plain path under a constant of its own (G13), and the documents' enrichment prompt asked for both registers from one generator (G14, retiring L4 as framed). Record every measurement, passed or failed, in a results document, the gaps file, ADR-001 and the run history.

**Architecture:** repograph is a Rust CLI (`src/`) over a `.repograph/` store. Retrieval fuses ranked lists round-robin into five seeds (`src/query.rs::ask`, `src/index/fuse.rs::interleave`). On the plain path the generated-questions BM25 list joins when its *coverage* — best over `LexicalIndex::attainable` — reaches `QUESTIONS_GATE = 0.761` of the passage list's (`src/query.rs:47`); the code-questions list is pooled on the reranked path only. `dump` records every list and the three `attainable_*` denominators per query so `bench/admission.py` can replay the plain fusion offline under any rule and `check` that the replay is the binary. Every retrieval change is replayed first, confirmed live on the read-only fixture and on store copies, judged by the spec's clauses, and recorded.

**Tech Stack:** Rust 2021 (`cargo test --release`, `cargo clippy --release --all-targets -- -D warnings`; CI runs the debug variants), Python 3 stdlib only (`bench/admission.py`, `bench/heldout.py`, `bench/history/track.py`, their `test_*.py` under `unittest`), bash 3.2, the `claude` CLI as the question generator (Task 6 Step 5 only).

**Spec:** [`docs/superpowers/specs/2026-09-06-residue-seat-register-design.md`](../specs/2026-09-06-residue-seat-register-design.md) — the three rules, the two designs weighed for the residue, the seat's constant and what it is derived on, the register's generator and held-out set, and the order. The spec outranks this plan where they disagree. The gaps are in `docs/bench/next-version-gaps.md` (G8, G13, G14); the evidence each rule was written with is `docs/bench/2026-09-06-coverage-admission-results.md` and `docs/bench/2026-09-06-g14-second-diagnostic.md`.

## Where the repository actually is

The local `main` checkout at `/Users/max/Documents/projects/repograph` is **stale at `d87c607`** (PR #6). The real tip is `origin/main` at `32577e2` (PR #13). This plan and its spec were written into the stale checkout's `docs/superpowers/` because that is the working directory; Task 0 creates a fresh worktree off `origin/main` and Task 1 copies both documents into it and commits them there, before any measurement. Nothing is built or measured from the stale checkout, and it is never `cd`'d into. The existing worktrees under `.worktrees/` (`coverage` at `32577e2` on branch `facts`, with untracked files that belong to another session) are not reused.

## Global Constraints

- **Worktree.** `R=/Users/max/Documents/projects/repograph`; `W=$R/.worktrees/rsr`, branch `feat/residue-seat-register` from `origin/main` (`32577e2`), created in Task 0. Every git command is `git -C "$W" …`, every cargo command `--manifest-path "$W/Cargo.toml"`, every path absolute. `B="$W/target/release/repograph"`, rebuilt with `cargo build --release --manifest-path "$W/Cargo.toml"` after every code change and before any measurement; `shasum "$B"` is written beside every set of dumps it produces. The base binary is kept as `$M/repograph-base` for the whole plan.
- **Scratch.** `M=/Users/max/bench/residue-seat-register-2026-09-06`, created in Task 0. Every number written into a doc, a commit, a test comment or a ledger line comes from a named file under `$M` (or the earlier campaigns' `$G=/Users/max/bench/gaps-2026-09-05`, `$O=/Users/max/bench/coverage-2026-09-06`) or from a test run. A token cost is the one estimate allowed and says it is one.
- **The fixture** `F=/Users/max/bench/beauty-crm-502e8a6d` is **read-only**: `bench`, `dump`, `ask --stale`. Never `build`, `update`, `enrich`, `embed`, `serve`, `watch`; never edit its files. Its `git status --porcelain -- ':(exclude)graphify-out'` is empty before and after every task.
- **Store copies** live under `$M` (and `$G` for the two that survive from 2026-09-05) and are the only stores a writer may touch. Before any `enrich` or `embed` on a copy, its `repograph.toml` names `embed_model = "intfloat/multilingual-e5-small"` — the default is the large model and a writer rewrites the rows whole under the configured model. Run `dump` and `bench` on copies, and `ask` only with `--stale` (a copy has no source tree beside it, and a non-`--stale` ask walks the tree, finds nothing and empties the graph — runbook trap 7). A copy whose `questions.json` is written by a script carries no `questions.bin`: a mirror behind an edited JSON is passed over by stamp, but the rule is simpler to keep than to reason about.
- **Rules are written before numbers and never re-read to fit them.** They are the spec's three rules, repeated in each task. The order of operations is `bench/heldout.py`'s header and the spec's: a constant is derived on the held-out dumps and written to a file before `score` opens a suite; live, the held-out compare is read before any bench line. A change that fails is recorded with its numbers and stays out; no constant is tuned after a suite has been read.
- **Held-out sets** are built by `bench/heldout.py build` with the recorded seed 20260905 and kind `synthetic`; `compare` refuses to pair dumps from different arms or question sets.
- **Commit hygiene.** Conventional Commits subjects (`feat(query): …`, `feat(enrich): …`, `chore(bench): …`, `docs(bench): …`); prose bodies with the measured numbers and the files they came from. The hook rejects `Co-Authored-By`, `Claude-Session` and any `claude.ai` link — none in a commit or the PR body. The hook also rejects a single shell command holding both a heredoc and a literal `git commit`: `git commit -F "$M/msg.txt"` after writing the message in a separate command, or `git commit -F - <<'MSG'` with the Conventional subject as the heredoc's first line. In zsh, quote `===` in `echo`.
- **Any `gh` call**: `gh auth switch --user devmaxxx && gh …` in the same command. The PR goes to `devmaxxx/repograph`, base `main`; the executor does not merge.
- **Comments say why, never what**; no ticket ids in code comments; tool directives stay. Test names are `snake_case` sentences in the style of their neighbours.
- **Python is stdlib only**; tests run as `cd "$W/bench" && python3 -m unittest test_admission test_heldout` and `cd "$W/bench/history" && python3 -m unittest test_track`. bash is 3.2.
- **Implementers never dispatch subagents.** Model tokens are spent at exactly one step, Task 6 Step 5, on the copy `$M/bc-r1`.

---

## File Structure

| File | Responsibility | Tasks |
|---|---|---|
| `docs/superpowers/specs/2026-09-06-residue-seat-register-design.md`, this plan | the rules, committed first | 1 |
| `src/index/lexical.rs` | `attainable` charges every query term; doc comments on `Lexical::code_seat` | 2, 5 |
| `src/dump.rs` | one sentence of the `attainable_*` comment | 2 |
| `src/query.rs` | `QUESTIONS_GATE`'s value and comment; `CODE_SEAT`, `CODE_GATE`, `code_seat()`, the seat in `lexical_lists`; tests | 2, 3, 5 |
| `src/ask.rs` | the resident context builds the code index for plain answers when the seat is on; its test | 5 |
| `src/enrich.rs` | the documents' prompt asks for both registers; test | 6 |
| `bench/admission.py`, `bench/test_admission.py` | `score --ho-base`; `--code-c`; `crossover --list code` | 2, 4 |
| `bench/heldout.py`, `bench/test_heldout.py` (new) | `build --only documents\|code` | 4 |
| `bench/history/track.py`, `bench/history/test_track.py` | `record --tag` so a copy's rows never pool with the fixture's | 4 |
| `docs/bench/2026-09-0X-residue-seat-register-results.md` (new) | every measurement of this plan | 7 |
| `docs/bench/next-version-gaps.md`, `docs/adr/ADR-001-…md`, `README.md`, `bench/history/runs.jsonl` | statuses, Amendment 9, prose, rows | 7 |

Task order: 0 → 1 → 2 → 3 → 4 → 5 → 6 → 7. Task 3 runs only if Task 2's offline verdict passes (for design A or its fallback B); Task 5 only if Task 4's does. Task 6 Steps 1–7 (the prompt, the copy, the tokens, the neutral set) may start right after Task 1 and run in the background while Tasks 2–5 proceed — they touch only `src/enrich.rs` and `$M/bc-r1` — but Task 6 Step 8 onward reads nothing until Task 5 has left the final binary.

---

### Task 0: Worktree, base binary, baselines, the copies checked

**Files:**
- Create (outside the repo): `$M/`, `$M/repograph-base`, `$M/base-*.json`, `$M/base-*.txt`, `$M/bc-raw/`, `$M/task0.txt`
- Nothing in the repo changes; no commit.

**Interfaces:**
- Produces: the base binary every "before" is read on; the fixture baselines under the shipped form (`$M/base-{rec,dev,ho}-{dense,lexical}.json`, `$M/base-bench-{rec,dev}-{dense,lexical}-full.txt` and their one-line tails, `$M/base-raw-bench-{dense,lexical}.txt`); the raw copy; the survey of the surviving copies.

- [ ] **Step 1: The worktree, off `origin/main`**

```bash
R=/Users/max/Documents/projects/repograph
git -C "$R" fetch origin
git -C "$R" worktree add "$R/.worktrees/rsr" -b feat/residue-seat-register origin/main
W="$R/.worktrees/rsr"; git -C "$W" log --oneline -1
```

Expected: `32577e2 feat(config): the model is a setting, in the project or on the machine (#13)`. Anything else: `origin/main` moved since this plan was written — stop and report the new tip; the plan's baselines are the shipped form's and a newer tip has to be read first.

- [ ] **Step 2: The base binary, kept**

```bash
M=/Users/max/bench/residue-seat-register-2026-09-06; mkdir -p "$M"
cargo build --release --manifest-path "$W/Cargo.toml" 2>&1 | tail -1
cargo test --release --manifest-path "$W/Cargo.toml" 2>&1 | grep 'test result'
B="$W/target/release/repograph"; cp "$B" "$M/repograph-base"; shasum "$B" "$M/repograph-base" | tee "$M/base-binary.txt"
```

Expected: `444 passed; 0 failed; 2 ignored` and `12 passed` (the `serve` tests); the two shas equal. `B0="$M/repograph-base"` is the base binary for the rest of the plan.

- [ ] **Step 3: Baselines on the fixture, and the proof they are the shipped form's**

```bash
F=/Users/max/bench/beauty-crm-502e8a6d; G=/Users/max/bench/gaps-2026-09-05; O=/Users/max/bench/coverage-2026-09-06
git -C "$F" status --porcelain -- ':(exclude)graphify-out'   # must print nothing
for arm in dense lexical; do
  nd=""; [ $arm = lexical ] && nd="--no-dense"
  "$B0" --repo "$F" $nd dump --queries "$W/bench/cases.jsonl"     --out "$M/base-rec-$arm.json" >/dev/null 2>&1
  "$B0" --repo "$F" $nd dump --queries "$W/bench/dev-cases.jsonl" --out "$M/base-dev-$arm.json" >/dev/null 2>&1
  "$B0" --repo "$F" $nd dump --queries "$G/heldout-400-syn.jsonl" --out "$M/base-ho-$arm.json"  >/dev/null 2>&1
  "$B0" --repo "$F" $nd bench                                    > "$M/base-bench-rec-$arm-full.txt" 2>&1
  "$B0" --repo "$F" $nd bench --cases "$W/bench/dev-cases.jsonl" > "$M/base-bench-dev-$arm-full.txt" 2>&1
  tail -1 "$M/base-bench-rec-$arm-full.txt" > "$M/base-bench-rec-$arm.txt"
  tail -1 "$M/base-bench-dev-$arm-full.txt" > "$M/base-bench-dev-$arm.txt"
  for s in rec dev ho; do cmp "$M/base-$s-$arm.json" "$O/$s-$arm.json" && echo "$s-$arm identical to the shipped form's dump"; done
done 2>&1 | tee "$M/task0-cmp.txt"
cat "$M"/base-bench-*.txt
```

Expected, verbatim from `docs/bench/2026-09-06-coverage-admission-results.md`: recorded `keyword 40/40  paraphrase 15/30  code 12/12  p90 221 tok  dense=true …` and `keyword 39/40  paraphrase 15/30  code 12/12  p90 215 tok  dense=false …`; developer `long 10/15  cross 13/15  multi 10/12  where 0/9  rule 5/9` (dense) and `long 11/15  cross 13/15  multi 9/12  where 0/9  rule 5/9` (lexical); six `cmp` lines silent. A differing dump or line is a retrieval change between `ef3691f` and `32577e2` that nobody recorded — **stop and report**; the spec's baseline table is copied from that document and the rule must be judged against numbers that agree with it.

- [ ] **Step 4: The raw copy, for the raw half of clause R6**

The raw arms have no questions index, so no admission and no `attainable` — untouched by construction. Rule R still names four arms, so the two raw lines are read rather than assumed, on a copy of the fixture's store with nothing `enrich` paid for (the shape `$G/e5large-raw` took):

```bash
mkdir -p "$M/bc-raw/.repograph"
for f in graph.json graph.bin manifest.json vectors.json vectors.f32; do cp "$F/.repograph/$f" "$M/bc-raw/.repograph/"; done
printf 'embed_model = "intfloat/multilingual-e5-small"\n' > "$M/bc-raw/repograph.toml"
for arm in dense lexical; do
  nd=""; [ $arm = lexical ] && nd="--no-dense"
  "$B0" --repo "$M/bc-raw" $nd bench 2>&1 | tail -1 | tee "$M/base-raw-bench-$arm.txt"
done
```

Expected (ADR-001 Amendment 7's raw lines, on the small model): `keyword 40/40  paraphrase 9/30  code 12/12  p90 221 tok  dense=true  enriched=false …` and `keyword 39/40  paraphrase 7/30  code 12/12  p90 226 tok  dense=false  enriched=false …`, both `gated=true` and green. A different reading is not an error to fix — it is the before, written down, and clause R6's raw half is read against it.

- [ ] **Step 5: The surviving copies, the held-out set, the disk**

```bash
python3 - <<'PY'
import json
G="/Users/max/bench/gaps-2026-09-05"; F="/Users/max/bench/beauty-crm-502e8a6d"
f=json.load(open(f"{F}/.repograph/questions.json"))["entries"]
for d in ["bc-d1","bc-a1"]:
    v=json.load(open(f"{G}/{d}/.repograph/vectors.json")); q=json.load(open(f"{G}/{d}/.repograph/questions.json"))["entries"]
    code=[k for k in q if k.startswith(("sym:","file:"))]
    docs_same=sum(1 for k in f if q.get(k,{}).get("questions")==f[k]["questions"])
    print(d, "model", repr(v.get("model")), "dim", v["dim"], "rows", len(v["ids"]), "entries", len(q), "code", len(code), "document entries identical to the fixture's", docs_same, "/", len(f), "toml:", open(f"{G}/{d}/repograph.toml").read().strip())
PY
python3 "$W/bench/heldout.py" build --store "$F/.repograph" --out "$M/heldout-400-syn.jsonl"
cmp "$M/heldout-400-syn.jsonl" "$G/heldout-400-syn.jsonl" && echo "held-out set identical to the campaign's"
df -h /Users/max | tail -1
```

Expected: `bc-d1 model 'intfloat/multilingual-e5-small' dim 384 rows 34802 entries 1996 code 0 document entries identical to the fixture's 0 / 1996` (a second generator's questions, none the fixture's); `bc-a1 model '' dim 384 rows 61169 entries 5459 code 3463 document entries identical to the fixture's 1996 / 1996` (its document questions *are* the fixture's, which Task 4 relies on); both tomls `embed_model = "intfloat/multilingual-e5-small"`; `400 questions from 1996 enriched nodes, seed 20260905`, `cmp` silent; at least 2 GB free (`$M/bc-r1` is ~130 MB, `$M/bc-raw` ~75 MB, the dumps a few hundred MB). A missing copy is **BLOCKED** — `bc-d1` cost ≈ $13.62 to make and `bc-a1` a code enrichment, and re-creating either is the controller's call.

- [ ] **Step 6: The task record**

`$M/task0.txt`: the worktree's commit, the base binary's sha, the eight fixture bench lines, the six `cmp` verdicts, the two raw lines, the copy survey, the held-out check, the free disk. Report DONE with that path.

---

### Task 1: The rules, committed before any number

**Files:**
- Create in the worktree: `docs/superpowers/specs/2026-09-06-residue-seat-register-design.md`, `docs/superpowers/plans/2026-09-06-residue-seat-register.md` (copied from the stale checkout, byte for byte)
- Create (scratch): `$M/spec-commit.txt`

- [ ] **Step 1: Copy, verify, commit**

```bash
cp "$R/docs/superpowers/specs/2026-09-06-residue-seat-register-design.md" "$W/docs/superpowers/specs/"
cp "$R/docs/superpowers/plans/2026-09-06-residue-seat-register.md"        "$W/docs/superpowers/plans/"
grep -n '40/40, paraphrase 15/30\|39/40, paraphrase 15/30\|— \*\*38\*\*' "$W/docs/superpowers/specs/2026-09-06-residue-seat-register-design.md"
git -C "$W" add docs/superpowers/specs/2026-09-06-residue-seat-register-design.md docs/superpowers/plans/2026-09-06-residue-seat-register.md
```

The spec's baseline table must agree with `$M/base-bench-*.txt` (Task 0 Step 3 already required that); if it does not, Task 0 stopped and this task is not reached. Then, in a second command, the commit: subject `docs(specs): the residue, the seat and the register — three rules written before the numbers`, body naming the three items, the order and its reason, and the one point tokens are spent. `git -C "$W" rev-parse --short HEAD | tee "$M/spec-commit.txt"`.

From here on **no clause in the spec is edited**. A clarification a later task needs goes into the results document beside the number it clarifies.

---

### Task 2: G8's residue — the denominator in the binary, the constant derived, the offline verdict

**Files:**
- Modify: `src/index/lexical.rs` (`attainable`; its doc comment; the test `attainable_is_the_sum_of_idf_over_the_query_terms_the_index_holds_each_counted_once` rewritten)
- Modify: `src/dump.rs` (one sentence of the comment above `attainable_questions`)
- Modify: `src/query.rs` (tests only: one new, one rewritten by the new arithmetic, one premise re-checked)
- Modify: `bench/admission.py` (`score --ho-base`, the `recorded_hits` helper), `bench/test_admission.py`
- Create (scratch): `$M/lists.py`, `$M/t2-*.json`, `$M/t2-*.txt`

**Interfaces:**
- Produces: `LexicalIndex::attainable(&self, query: &str) -> f32` with the new meaning (every unique query term, an absent one at df = 0, an empty index attaining 0.0); dump records whose `attainable_*` fields carry it and whose lists are unchanged; `admission.py score --ho <new dumps> --ho-base <baseline dumps>`.
- The constant `C1` in `$M/t2-crossover.txt`. Task 3 copies it into `QUESTIONS_GATE`.

- [ ] **Step 1: Write the failing tests**

In `src/index/lexical.rs`'s `mod tests`, replace `attainable_is_the_sum_of_idf_over_the_query_terms_the_index_holds_each_counted_once` with:

```rust
    #[test]
    fn attainable_charges_every_query_term_once_and_an_absent_one_at_the_idf_of_df_zero() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "правило отмены", "штраф считается по политике отмены", "a.md", 1);
        e.node(NodeKind::Requirement, "FR-PAY-26", "списание штрафа", "штраф списывается автоматически", "a.md", 9);
        e.node(NodeKind::Requirement, "FR-CAL-40", "коды конфликтов", "словарь кодов", "b.md", 1);
        g.apply(e);
        let idx = LexicalIndex::build(&g);
        let n = 3.0_f32;
        let idf = |df: f32| ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
        // «штраф» sits in two documents, «политике» in one, «ъъъ» in none and is charged as a
        // term no document holds — it lowers what the best document could cover instead of
        // leaving the sum; a term the query repeats is one term, as a document of average length
        // holds it once.
        assert!((idx.attainable("штраф политике ъъъ") - (idf(2.0) + idf(1.0) + idf(0.0))).abs() < 1e-6);
        assert!((idx.attainable("штраф штраф") - idf(2.0)).abs() < 1e-6);
        assert!((idx.attainable("ъъъ") - idf(0.0)).abs() < 1e-6);
        assert_eq!(LexicalIndex::build(&Graph::default()).attainable("штраф"), 0.0, "an index over nothing attains nothing");
    }
```

`a_document_of_average_length_holding_a_term_once_scores_exactly_the_attainable` stays as it is: every term of its query is present. In `src/query.rs`'s `mod tests`, after `a_zero_attainable_leaves_that_list_covering_nothing_on_either_side`:

```rust
    #[test]
    fn an_index_that_lacks_a_query_term_covers_less_of_the_query_not_more() {
        // Two indexes over the same two nodes. The passages hold both words of «штраф отмены»;
        // the stored question holds «штраф» alone. Under the shipped denominator the questions
        // list covered as much of the query as the passages did — the term it lacked left its
        // denominator, which is the residue of G8 — and it was seated level with a list that
        // answered twice as much. Now the missing term is charged and it is refused.
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-A", "штраф отмены", "", "a.md", 1);
        e.node(NodeKind::Requirement, "FR-B", "другое", "", "a.md", 5);
        g.apply(e);
        let mut qs = Questions::default();
        qs.entries.insert("FR-A".into(), crate::enrich::Entry { hash: String::new(), questions: vec!["какой штраф".into()] });
        let l = lex(&g, &qs);
        let qi = l.questions.as_ref().unwrap();
        let best = |x: &[(String, f32)]| x.first().map(|(_, s)| *s).unwrap_or(0.0);
        let q = "штраф отмены";
        let (bq, aq, bp, ap) = (best(&qi.search(q, 10)), qi.attainable(q), best(&l.passages.search(q, 10)), l.passages.attainable(q));
        assert!(coverage(bq, aq) < coverage(bp, ap), "questions covered {}, passages {}", coverage(bq, aq), coverage(bp, ap));
        assert!(!admits(bq, aq, bp, ap, QUESTIONS_GATE), "{} against {}", coverage(bq, aq), coverage(bp, ap));
    }
```

Run: `cargo test --release --manifest-path "$W/Cargo.toml" attainable covers_less 2>&1 | tail -5`. Expected: the two new assertions fail (`attainable("ъъъ")` reads 0, the questions list is admitted).

- [ ] **Step 2: Implement design A**

In `src/index/lexical.rs`, replace `attainable` and its doc comment with:

```rust
    /// What the query asked for, priced in this index: the score a document of average length
    /// would get for holding each of the query's terms exactly once — BM25 at tf = 1 and the
    /// mean length, where the term weight `(K1 + 1) / (1 + K1)` is one, so the plain sum of
    /// their idf. Every term of the query is in the sum, a term this index never saw at the idf
    /// BM25 gives df = 0, `ln(2n + 2)`. Until 2026-09-06 an absent term left the sum instead,
    /// and a list's coverage — its best over this figure — then rose with every query term its
    /// index lacked: an index was rewarded for a narrow vocabulary as much as for a good match,
    /// the residue of gap G8 that the coverage admission shipped with. Charging the term keeps
    /// the statistic a property of the query and of one index alone, which is what lets two
    /// lists' coverages compare in one unit (G12); `search` is untouched, so no ranking moves.
    /// An index over no documents attains nothing.
    pub fn attainable(&self, query: &str) -> f32 {
        if self.ids.is_empty() { return 0.0; }
        let n = self.ids.len() as f32;
        let mut seen = std::collections::HashSet::new();
        tokenize(query).into_iter().filter(|t| seen.insert(t.clone()))
            .map(|t| Self::idf(n, self.postings.get(&t).map_or(0, Vec::len)))
            .sum()
    }
```

In `src/dump.rs`, the comment above `attainable_questions` says the empty sum's `-0.0` is what "`attainable` on that still-real, still-empty index gives" — no longer true (an empty index returns `0.0`, and `Lexical::code_index` never keeps an empty one). Cut that sentence; the `-0.0` stays the sign of an *absent* index, which `unwrap_or(-0.0)` still writes and the test `a_raw_store_dumps_no_questions_list_and_the_empty_sum_it_matches` still pins.

- [ ] **Step 3: Run the suite and rewrite what the new arithmetic changed**

Run: `cargo test --release --manifest-path "$W/Cargo.toml" 2>&1 | grep -E 'test result|FAILED|panicked' | head`.

Expected: the two Step 1 tests pass; `what_the_raw_ratio_separated_by_magnitude_the_coverage_admission_does_not` fails, and it should. Its own comment says «считается» "is in FR-PAY-22's body and in no stored question" — a term the questions index lacks, which the shipped denominator dropped and this one charges. On `graph()` (six non-file nodes, `n = 6`, so df = 0 costs `ln 14`), the questions list's coverage of «штраф считается» falls from 0.696 to roughly a third of that (`1.072 / (1.540 + 2.639)`) and the admission from 0.811 to about 0.30, well under the constant; «штраф отмену», both of whose terms both indexes hold, does not move. Rewrite the test as `a_term_the_questions_index_lacks_now_costs_it_the_seat_the_shipped_form_gave_it`: keep the graph and the two queries; assert «штраф отмену» still reads 0.696 against 0.858 and is seated (the arithmetic for a query every index holds is unchanged); assert «штраф считается» is now refused and the plain answer's first seed is `FR-PAY-22`; write the four coverages the run prints into the comment, from the assertion messages, in place of the prose above. Do not touch `QUESTIONS_GATE` to make anything pass.

`code_questions_reach_the_reranked_pool_and_never_the_plain_fusion` asserts its own premise (`admits(...)` for «штраф выйти всех устройств»). Under the new denominator the passage index is charged three Russian terms it lacks and the code index one, so the premise most likely still holds; if it does not, change the query to one the arithmetic admits (`выйти всех устройств`, where the passages have nothing and the code list is alone with something to say), and say so in the comment. `the_questions_list_leads_when_admitted_and_is_absent_when_it_matched_nothing` and `dense_leads_then_the_question_list_then_the_passages` use queries both indexes hold in full and do not move.

Then: `cargo test --release --manifest-path "$W/Cargo.toml" 2>&1 | grep 'test result'` — `445 passed` (444 − 0 + 1) and `12 passed`; `cargo clippy --release --all-targets --manifest-path "$W/Cargo.toml" -- -D warnings 2>&1 | tail -1` clean.

- [ ] **Step 4: `admission.py score --ho-base`**

The replay's held-out compare reads its "before" out of the dump it replays (`rec["ask"]["seeds"]`). For this task the before is the *shipped* binary's dump, so `score` learns to take one. In `bench/admission.py`, extract the pairing into a helper and add the argument:

```python
def recorded_hits(dump):
    """Each question's recorded answer — the binary's own — as whether an anchor sat in the
    first five seeds, keyed the way `heldout.py` keys a pairing."""
    out = {}
    for rec in dump["queries"]:
        key = (rec["q"], tuple(anchors(rec["expect"])))
        out[key] = bool(set(key[1]) & set(ids(rec["ask"]["seeds"][:SEATS])))
    return out
```

In `cmd_score`, the held-out loop pairs `after` (the replay) against `before = recorded_hits(load(base))` when `--ho-base` is given, else against `recorded_hits(dump)` as now; `--ho-base` takes one path per `--ho` path, in order, and a length mismatch or a key-set mismatch is a `SystemExit` naming the two files (a paired test over two different question sets is not a paired test). The printed line names both files: `held-out ho-lexical.json vs base-ho-lexical.json: …`. In `bench/test_admission.py`:

```python
class Recorded(unittest.TestCase):
    def test_recorded_hits_reads_the_first_five_seeds_against_any_anchor(self):
        dump = {"queries": [
            {"q": "a", "expect": "N-1", "ask": {"seeds": [["x", 1.0], ["N-1", 0.5]]}},
            {"q": "b", "expect": ["N-2", "N-3"], "ask": {"seeds": [["N-3", 1.0]]}},
            {"q": "c", "expect": "N-4", "ask": {"seeds": [["1", 1], ["2", 1], ["3", 1], ["4", 1], ["5", 1], ["N-4", 1]]}},
        ]}
        self.assertEqual(a.recorded_hits(dump), {("a", ("N-1",)): True, ("b", ("N-2", "N-3")): True, ("c", ("N-4",)): False})
```

Run: `cd "$W/bench" && python3 -m unittest test_admission 2>&1 | tail -3` — 14 tests, OK.

- [ ] **Step 5: Build, dump, and prove the lists did not move**

```bash
cargo build --release --manifest-path "$W/Cargo.toml" 2>&1 | tail -1; shasum "$B" | tee "$M/t2-binary.txt"
for arm in dense lexical; do
  nd=""; [ $arm = lexical ] && nd="--no-dense"
  "$B" --repo "$F" $nd dump --queries "$W/bench/cases.jsonl"     --out "$M/t2-rec-$arm.json" >/dev/null 2>&1
  "$B" --repo "$F" $nd dump --queries "$W/bench/dev-cases.jsonl" --out "$M/t2-dev-$arm.json" >/dev/null 2>&1
  "$B" --repo "$F" $nd dump --queries "$G/heldout-400-syn.jsonl" --out "$M/t2-ho-$arm.json"  >/dev/null 2>&1
done
cat > "$M/lists.py" <<'PY'
"""The new binary's dump against the shipped one's: every ranked list byte for byte the same, every
attainable at least what it was, and the shift written down per index."""
import json, statistics, sys
base, new = (json.load(open(p)) for p in sys.argv[1:3])
assert base["meta"]["dense"] == new["meta"]["dense"] and len(base["queries"]) == len(new["queries"])
shift = {"passages": [], "questions": [], "code": []}
unchanged = {k: 0 for k in shift}
for a, b in zip(base["queries"], new["queries"]):
    assert a["q"] == b["q"]
    for k in ("exact", "bm25_passages", "bm25_questions", "bm25_code", "dense_passages", "dense_questions"):
        assert a[k] == b[k], f"{k} moved on {a['q'][:60]!r}"
    for k in shift:
        old, cur = a[f"attainable_{k}"], b[f"attainable_{k}"]
        assert cur >= old - 1e-6, f"attainable_{k} fell on {a['q'][:60]!r}: {old} -> {cur}"
        unchanged[k] += cur == old
        if old > 0:
            shift[k].append(cur / old)
print(f"{sys.argv[2].split('/')[-1]}: {len(new['queries'])} queries, every list identical")
for k, v in shift.items():
    if v:
        print(f"  attainable_{k}: unchanged on {unchanged[k]}, new/old median {statistics.median(v):.3f}, max {max(v):.3f}")
PY
for arm in dense lexical; do for s in rec dev ho; do python3 "$M/lists.py" "$M/base-$s-$arm.json" "$M/t2-$s-$arm.json"; done; done 2>&1 | tee "$M/t2-lists.txt"
cd "$W/bench" && python3 admission.py check --store "$F/.repograph" --form coverage --c 0.761 "$M"/t2-rec-*.json "$M"/t2-dev-*.json "$M"/t2-ho-*.json 2>&1 | tee "$M/t2-check-old-constant.txt"
```

Required: six `every list identical` lines with no assertion; the `check` at the *old* constant replicates 82/82, 60/60, 400/400 in both arms — the binary and the replay are still one function, now over the new denominators. A moved list means `search` changed, which this task must not do: stop and fix before anything is derived.

- [ ] **Step 6: The constant, before any suite is opened**

```bash
cd "$W/bench" && python3 admission.py crossover --form coverage --heldout "$M/t2-ho-lexical.json" 2>&1 | tee "$M/t2-crossover.txt"
```

Expected shape: `form=coverage c=<C1>  (400 questions with a questions list, …)` and the four percentages. `C1` is that number to three decimals, as printed. It is written now, and it is the constant whether or not it equals 0.761.

- [ ] **Step 7: The offline verdict**

```bash
C1=$(head -1 "$M/t2-crossover.txt" | sed 's/.*c=\([0-9.]*\).*/\1/'); echo "C1=$C1" | tee "$M/t2-constant.txt"
cd "$W/bench" && python3 admission.py score --store "$F/.repograph" --form coverage --c "$C1" \
  --rec "$M/t2-rec-dense.json" "$M/t2-rec-lexical.json" --dev "$M/t2-dev-dense.json" "$M/t2-dev-lexical.json" \
  --ho "$M/t2-ho-dense.json" "$M/t2-ho-lexical.json" --ho-base "$M/base-ho-dense.json" "$M/base-ho-lexical.json" 2>&1 | tee "$M/t2-score.txt"
```

Read against rule R, clause by clause, into `$M/t2-verdict.txt` (`design A: pass` or `design A: fail (clause N: …)`):

- R0 — the crossover file predates this step: `ls -l --time-style=full-iso "$M/t2-crossover.txt" "$M/t2-score.txt"`.
- R6 (which carries R2) — the recorded lines read `keyword 40/40 paraphrase 15/30 code 12/12` with embeddings and `keyword 39/40 paraphrase 15/30 code 12/12` without, counts identical to `$M/base-bench-rec-*.txt`.
- R3 — developer totals at least 38 and 38, `where 0/9` in both.
- R4 — both held-out lines `not significant` or with `gained ≥ lost`.
- R1 and R5 are live clauses and are read in Task 3.

**Pass:** on to Task 3 with `C1`.

**Fail:** design A is recorded with its numbers, and the fallback is tried once, under the same clauses, in the same order: `attainable_within(&self, query, known: &HashSet<String>)` charges only the query terms in `known`, absent-here ones at df = 0; `Lexical` builds `known` once as the union of the passage and questions indexes' vocabularies and exposes `known(&self)`; `lexical_lists` and `dump` call `attainable_within(query, lex.known())` for all three lists; the code list is charged against the same `known`, which is the code-side residue the spec names as the reason B is second. Repeat Steps 1–7 as `$M/t2b-*` with `C1b`. If B also fails: revert `src/index/lexical.rs`, `src/query.rs`, `src/dump.rs` (`git -C "$W" checkout -- …`), keep the `admission.py` change and commit it alone (`chore(bench): the replay's held-out compare takes a baseline dump`), write `$M/t2-verdict.txt` with both designs' clauses and numbers, and continue at Task 4 under the shipped denominator and `c = 0.761`.

---

### Task 3: G8's residue — live, and the constant in the binary

Runs only if `$M/t2-verdict.txt` says `pass` for A or B. The design below is written as A; under B the same steps apply with `attainable_within`.

**Files:**
- Modify: `src/query.rs` (`QUESTIONS_GATE`'s value and the doc comment's derivation paragraph)
- Create (scratch): `$M/t3-*.json`, `$M/t3-*.txt`, `$M/t3-g13.md`

**Interfaces:**
- Consumes: `C1` from `$M/t2-constant.txt`.
- Produces: the binary Task 4 derives the seat's constant on, and the baseline every later "before" on the fixture is read from.

- [ ] **Step 1: The constant, and the comment that says where it came from**

In `src/query.rs`, `const QUESTIONS_GATE: f64 = 0.761;` becomes `C1` as an `f64` literal copied from `$M/t2-constant.txt`. The doc comment's paragraph "The value is the crossover of the 400 held-out questions …" is rewritten to say: the crossover on the same 400 questions in the same arm, re-derived on 2026-09-0X when `attainable` began charging every query term (`$M/t2-crossover.txt` by path, `docs/bench/2026-09-0X-residue-seat-register-results.md` by name); 0.761 was the same crossover under the denominator that dropped absent terms; neither was rounded or tuned. If `C1` happens to be 0.761, the value line stays and only the paragraph changes.

Run: `cargo test --release --manifest-path "$W/Cargo.toml" 2>&1 | grep 'test result'` — the seat tests were written against the arithmetic, not the constant, and pass unchanged unless `C1` crosses one of the test graph's statistics; if one fails, its comment gets the statistic and the assertion follows the arithmetic, never the other way. Clippy clean. `cargo build --release --manifest-path "$W/Cargo.toml"`; `shasum "$B" | tee "$M/t3-binary.txt"`.

- [ ] **Step 2: Dumps, and the binary is the replay**

```bash
C1=$(cut -d= -f2 "$M/t2-constant.txt")   # or $M/t2b-constant.txt under design B
for arm in dense lexical; do
  nd=""; [ $arm = lexical ] && nd="--no-dense"
  "$B" --repo "$F" $nd dump --queries "$W/bench/cases.jsonl"     --out "$M/t3-rec-$arm.json" >/dev/null 2>&1
  "$B" --repo "$F" $nd dump --queries "$W/bench/dev-cases.jsonl" --out "$M/t3-dev-$arm.json" >/dev/null 2>&1
  "$B" --repo "$F" $nd dump --queries "$G/heldout-400-syn.jsonl" --out "$M/t3-ho-$arm.json"  >/dev/null 2>&1
done
cd "$W/bench" && python3 admission.py check --store "$F/.repograph" --form coverage --c "$C1" "$M"/t3-rec-*.json "$M"/t3-dev-*.json "$M"/t3-ho-*.json 2>&1 | tee "$M/t3-check.txt"
```

Required (R5): 82/82, 60/60, 400/400 replicated in both arms, exit 0.

- [ ] **Step 3: Held-out first**

```bash
for arm in dense lexical; do python3 "$W/bench/heldout.py" compare "$M/base-ho-$arm.json" "$M/t3-ho-$arm.json"; done 2>&1 | tee "$M/t3-heldout.txt"
```

Required (R4): neither arm `significant` with `lost > gained`. Expected: the replay's numbers from `$M/t2-score.txt`, question for question.

- [ ] **Step 4: The four arms, the developer suite, the per-case moves**

```bash
for arm in dense lexical; do
  nd=""; [ $arm = lexical ] && nd="--no-dense"
  "$B" --repo "$F" $nd bench                                    > "$M/t3-bench-rec-$arm-full.txt" 2>&1
  "$B" --repo "$F" $nd bench --cases "$W/bench/dev-cases.jsonl" > "$M/t3-bench-dev-$arm-full.txt" 2>&1
  "$B" --repo "$M/bc-raw" $nd bench 2>&1 | tail -1 > "$M/t3-raw-bench-$arm.txt"
  tail -1 "$M/t3-bench-rec-$arm-full.txt" > "$M/t3-bench-rec-$arm.txt"; tail -1 "$M/t3-bench-dev-$arm-full.txt" > "$M/t3-bench-dev-$arm.txt"
  diff <(grep -E '^\S+ +\S+ +(HIT|miss)' "$M/base-bench-rec-$arm-full.txt" | awk '{print $1, $2, $3}') <(grep -E '^\S+ +\S+ +(HIT|miss)' "$M/t3-bench-rec-$arm-full.txt" | awk '{print $1, $2, $3}') > "$M/t3-case-moves-rec-$arm.txt"
  diff <(grep -E '^\S+ +\S+ +(HIT|miss)' "$M/base-bench-dev-$arm-full.txt" | awk '{print $1, $2, $3}') <(grep -E '^\S+ +\S+ +(HIT|miss)' "$M/t3-bench-dev-$arm-full.txt" | awk '{print $1, $2, $3}') > "$M/t3-case-moves-dev-$arm.txt"
done
cat "$M"/t3-bench-*.txt "$M"/t3-raw-bench-*.txt "$M"/base-bench-rec-*.txt "$M"/base-raw-bench-*.txt | tee "$M/t3-bench.txt"; wc -l "$M"/t3-case-moves-*.txt
```

Required: R1 — every line `gated=true`, exit 0, p90 ≤ 230 (`grep -c 'gated=true' "$M/t3-bench.txt"` reads 8 of the 8 graded lines); R6 — the two enriched recorded lines and the two raw lines carry counts identical to their `base-` counterparts; R3 — each developer line's total (the five numerators summed) at least the base line's, `where 0/9`. The `t3-case-moves-*` files are recorded whatever they hold: a count that did not move with a case that did is information for the results document, not a clause.

- [ ] **Step 5: G13 under the new denominator**

```bash
"$B" --repo "$G/bc-a1" --no-dense dump --queries "$W/bench/dev-cases.jsonl" --out "$M/t3-a1-dev-lexical.json" >/dev/null 2>&1
cd "$W/bench" && python3 admission.py g13 --dump "$M/t3-a1-dev-lexical.json" --store "$G/bc-a1/.repograph" | tee "$M/t3-g13.md"
```

Expected: 27 rows, the `coverage` column now under the charged denominator (the code index lacks many document terms, so its coverages fall from `$O/g13.md`'s); the table is evidence for Task 4 and is not scored against anything.

- [ ] **Step 6: Verdict and commit**

`$M/t3-verdict.txt`: each of R0–R6 with its file and `pass`/`fail`. **Pass:** `git -C "$W" add src/index/lexical.rs src/dump.rs src/query.rs bench/admission.py bench/test_admission.py` and one commit, subject `feat(query): the coverage denominator charges every query term, an absent one at df = 0`, body with the constant and its crossover line, the six replicated dumps, the four fixture arms and two raw lines, both held-out compares, and the attainable-shift medians from `$M/t2-lists.txt`, each with its file. **Fail:** a clause that passed offline and fails live is a disagreement between the replay and the binary, not a verdict on the form — R5 is what would have caught it, so `$M/t3-check.txt` names the query; fix the replication defect, re-run from Step 2, and if the form then fails a clause, revert the three Rust files, commit the Python alone as Task 2's fail branch says, record, and continue at Task 4 under `0.761`.

---

### Task 4: One seat for the code list — the tooling, the constant, the offline verdict

**Files:**
- Modify: `bench/heldout.py` (`is_code`, `build --only`), create `bench/test_heldout.py`
- Modify: `bench/admission.py` (`--code-c` everywhere the code list is admitted; `crossover --list`), `bench/test_admission.py`
- Modify: `bench/history/track.py` (`record --tag`), `bench/history/test_track.py`
- Create (scratch): `$M/heldout-a1-{docs,code,mixed}.jsonl`, `$M/t4-off-*.json`, `$M/t4-*.txt`

**Interfaces:**
- Consumes: the binary Task 3 left (`$B`; `C=$C1`, or `0.761` if the residue did not ship — `C=$(cat "$M/t2-constant.txt" | cut -d= -f2)` or `0.761`, written to `$M/t4-constant-c.txt`).
- Produces: the three held-out files; the seat-off baselines on `bc-a1`; `C_CODE` in `$M/t4-crossover-code.txt`; the offline verdict `$M/t4-verdict.txt` that decides whether Task 5 touches `src/query.rs`.

- [ ] **Step 1: `heldout.py build --only`**

In `bench/heldout.py`, after `AT = 5`:

```python
def is_code(node_id):
    """A code node's entry: `enrich --code` keys them by the graph's `sym:` and `file:` ids, and
    a held-out set for the code list has to be drawn from those alone, as the documents' set is
    drawn from the rest."""
    return node_id.startswith(("sym:", "file:"))
```

In `cmd_build`, the node list becomes
`nodes = sorted(k for k, v in entries.items() if v.get("questions") and (args.only is None or (args.only == "code") == is_code(k)))`,
and the parser gains `b.add_argument("--only", choices=["documents", "code"], default=None, help="draw from the documents' entries or the code's; every entry by default")`. The `print` line names the filter when one is set. New `bench/test_heldout.py`:

```python
#!/usr/bin/env python3
"""`heldout.py`'s pieces that a hand can check: which entries are code, and the paired test."""
import unittest

import heldout as h


class Kinds(unittest.TestCase):
    def test_code_entries_are_the_graph_s_symbol_and_file_ids(self):
        self.assertTrue(h.is_code("sym:apps/a.ts::revoke") and h.is_code("file:apps/a.ts"))
        self.assertFalse(h.is_code("FR-PAY-22") or h.is_code("entity:Money") or h.is_code("ADR-005"))


class McNemar(unittest.TestCase):
    def test_a_wash_is_p_one_and_six_lost_to_none_gained_is_the_price_a4_paid(self):
        before = {k: True for k in "abcdef"}
        self.assertEqual(h.mcnemar(before, dict(before)), (0, 0, 1.0))
        lost, gained, p = h.mcnemar(before, {k: False for k in before})
        self.assertEqual((lost, gained), (6, 0))
        self.assertAlmostEqual(p, 0.03125)


if __name__ == "__main__":
    unittest.main()
```

Run: `cd "$W/bench" && python3 -m unittest test_heldout 2>&1 | tail -3` — 2 tests, OK (the second reproduces A4's `p = 0.031` from six lost and none gained, which is the number the seat has to beat).

- [ ] **Step 2: `admission.py --code-c` and `crossover --list`**

In `bench/admission.py`: `lexical_lists(rec, form, c, code_seat, has_questions, code_c=None)` admits the code list at `c if code_c is None else code_c`; `all_lists`, `replay`, `score_suite`, `cmd_check` and `cmd_score` carry `code_c` through; `common` gains `--code-c` (`type=float, default=None, help="the code seat's own constant; the questions list's --c when absent"`). `cmd_crossover` gains `--list` (`choices=["questions", "code"], default="questions"`): the list and the denominator it reads are `bm25_questions`/`attainable_questions` or `bm25_code`/`attainable_code`, the statistic is that list's over the passages', and the two printed labels say `questions list` or `code list`. The module docstring's "Forms" paragraph gains one sentence: the code seat, when replayed, is admitted at `--code-c` and contributes its first document only. Tests in `bench/test_admission.py`:

```python
class CodeSeat(unittest.TestCase):
    def rec(self):
        return {"bm25_passages": scored(("p", 2.0)), "bm25_questions": scored(("q", 0.1)), "bm25_code": scored(("c1", 1.0), ("c2", 0.5)),
                "attainable_passages": 4.0, "attainable_questions": 4.0, "attainable_code": 2.0}

    def test_the_code_list_takes_its_own_constant_and_one_seat(self):
        # coverage(code) = 0.5 against coverage(passages) = 0.5: level, so admitted at 1.0 and not at 1.1.
        self.assertEqual(a.lexical_lists(self.rec(), "coverage", 1.1, True, True, code_c=1.0), [["p"], ["c1"]])
        self.assertEqual(a.lexical_lists(self.rec(), "coverage", 1.1, True, True), [["p"]])
        self.assertEqual(a.lexical_lists(self.rec(), "coverage", 1.0, True, True, code_c=1.1), [["p"]])

    def test_without_the_seat_the_code_list_is_never_on_the_plain_path(self):
        self.assertEqual(a.lexical_lists(self.rec(), "coverage", 0.0, False, True), [["p"]])
```

(The questions list at `0.1 / 4.0` against `2.0 / 4.0` sits under every constant above 0.05, so it is absent in all four cases — say so in a comment.) Run: `python3 -m unittest test_admission` — 16 tests, OK.

- [ ] **Step 3: `track.py record --tag`**

In `bench/history/track.py`'s `cmd_record`, after `build_row` returns, `if args.tag: row["arm"] += f"+{args.tag}"`; the parser gains `r.add_argument("--tag", default=None, help="appended to the arm name, so a store copy's rows never pool with the fixture's")`. A comment beside it says why: the report's `improved`/`REGRESSED` and chronic readings are per arm, and a copy recorded under the fixture's arm name would read as the fixture moving. Test in `bench/history/test_track.py`, in the style of its neighbours, on `TRANSCRIPT`: recording with `--tag r1` yields `arm == "bench:dense+enriched+r1"` and without it `bench:dense+enriched`. Run: `cd "$W/bench/history" && python3 -m unittest test_track 2>&1 | tail -3` — OK, one more than before.

- [ ] **Step 4: The three held-out files on `bc-a1`**

```bash
python3 "$W/bench/heldout.py" build --store "$G/bc-a1/.repograph" --only documents --out "$M/heldout-a1-docs.jsonl"
cmp "$M/heldout-a1-docs.jsonl" "$G/heldout-400-syn.jsonl" && echo "the documents' set on bc-a1 is the fixture's set"
python3 "$W/bench/heldout.py" build --store "$G/bc-a1/.repograph" --only code --out "$M/heldout-a1-code.jsonl"
cat "$M/heldout-a1-docs.jsonl" "$M/heldout-a1-code.jsonl" > "$M/heldout-a1-mixed.jsonl"; wc -l "$M"/heldout-a1-*.jsonl
python3 -c "
import json; rows=[json.loads(l) for l in open('$M/heldout-a1-code.jsonl')]
print(sum(r['expect'].startswith('file:') for r in rows), 'file anchors,', sum(r['expect'].startswith('sym:') for r in rows), 'symbol anchors,', len({r['expect'] for r in rows}), 'distinct nodes')"
```

Expected: `400 questions from 1996 enriched nodes` and `cmp` silent — `bc-a1`'s document questions are the fixture's, so the same seed draws the same 400 questions; `400 questions from 3463 enriched nodes` for the code set; 800 lines mixed; 400 distinct code nodes. A differing documents' set means `bc-a1`'s questions moved since Task 0's survey said they had not — **stop and report**.

- [ ] **Step 5: Seat-off baselines on `bc-a1`, on the binary Task 3 left**

The seat does not exist in the binary yet, so nothing is set; these dumps are the "before" of every seat clause, and their lists are what the constant is derived from.

```bash
# The questions constant as the binary carries it — C1 (or C1b) if Task 3 shipped, 0.761 if not.
C=$(grep -o 'QUESTIONS_GATE: f64 = [0-9.]*' "$W/src/query.rs" | awk '{print $NF}'); echo "C=$C" | tee "$M/t4-constant-c.txt"
for arm in dense lexical; do
  nd=""; [ $arm = lexical ] && nd="--no-dense"
  "$B" --repo "$G/bc-a1" $nd dump --queries "$W/bench/cases.jsonl"        --out "$M/t4-off-rec-$arm.json"     >/dev/null 2>&1
  "$B" --repo "$G/bc-a1" $nd dump --queries "$W/bench/dev-cases.jsonl"    --out "$M/t4-off-dev-$arm.json"     >/dev/null 2>&1
  "$B" --repo "$G/bc-a1" $nd dump --queries "$M/heldout-a1-docs.jsonl"    --out "$M/t4-off-ho-docs-$arm.json" >/dev/null 2>&1
  "$B" --repo "$G/bc-a1" $nd dump --queries "$M/heldout-a1-code.jsonl"    --out "$M/t4-off-ho-code-$arm.json" >/dev/null 2>&1
  "$B" --repo "$G/bc-a1" $nd bench                                    > "$M/t4-off-bench-rec-$arm-full.txt" 2>&1
  "$B" --repo "$G/bc-a1" $nd bench --cases "$W/bench/dev-cases.jsonl" > "$M/t4-off-bench-dev-$arm-full.txt" 2>&1
  tail -1 "$M/t4-off-bench-rec-$arm-full.txt" > "$M/t4-off-bench-rec-$arm.txt"; tail -1 "$M/t4-off-bench-dev-$arm-full.txt" > "$M/t4-off-bench-dev-$arm.txt"
done
"$B" --repo "$G/bc-a1" --no-dense dump --queries "$M/heldout-a1-mixed.jsonl" --out "$M/t4-off-ho-mixed-lexical.json" >/dev/null 2>&1
cd "$W/bench" && python3 admission.py check --store "$G/bc-a1/.repograph" --form coverage --c "$C" "$M"/t4-off-*.json 2>&1 | tee "$M/t4-off-check.txt"
cat "$M"/t4-off-bench-*.txt
```

Required: every `check` line fully replicated (82, 60, 400, 400 per arm, and 800 mixed) — the replay is the binary on this store too. Expected lines: recorded `code_questions=3463/3475 … gated=true` in both arms with the counts the shipped code reads on this store (the ratio era read `40/15/12` and `39/14/12`; under the coverage form and the new denominator they are whatever they are — the before, written down); developer `where 0/9` in both.

- [ ] **Step 6: The seat's constant, before any suite is opened**

```bash
cd "$W/bench" && python3 admission.py crossover --form coverage --list code --heldout "$M/t4-off-ho-mixed-lexical.json" 2>&1 | tee "$M/t4-crossover-code.txt"
C_CODE=$(head -1 "$M/t4-crossover-code.txt" | sed 's/.*c=\([0-9.]*\).*/\1/'); echo "C_CODE=$C_CODE" | tee "$M/t4-constant-code.txt"
```

Expected shape: `form=coverage c=<C_CODE>  (<n> questions with a code list, …)` — `n` is at most 800, fewer where the code list is empty for a question; then `above c: … code list holds the answer in top five X%, passage list Y%` and the same below. The document questions can only be "right" below the cut and the file-anchored code questions only above it, which is what makes the split meaningful. `C_CODE` is that number to three decimals and is not compared with `C`.

- [ ] **Step 7: The offline verdict**

```bash
cd "$W/bench" && python3 admission.py score --store "$G/bc-a1/.repograph" --form coverage --c "$C" --code-seat --code-c "$C_CODE" \
  --rec "$M/t4-off-rec-dense.json" "$M/t4-off-rec-lexical.json" --dev "$M/t4-off-dev-dense.json" "$M/t4-off-dev-lexical.json" \
  --ho "$M/t4-off-ho-docs-dense.json" "$M/t4-off-ho-docs-lexical.json" "$M/t4-off-ho-code-dense.json" "$M/t4-off-ho-code-lexical.json" 2>&1 | tee "$M/t4-score.txt"
```

`--ho` without `--ho-base` pairs the replay against each dump's own recorded answer, which is the seat-off binary — exactly the before rule S names. Read into `$M/t4-verdict.txt`:

- S0 — `$M/t4-crossover-code.txt` predates `$M/t4-score.txt`.
- S2 — recorded keyword, paraphrase, code each at least `$M/t4-off-bench-rec-*.txt`'s, both arms.
- S3 — developer total at least the seat-off total and `where ≥ 1/9`, both arms.
- S4 — the two `ho-docs` lines: neither `significant` with `lost > gained`.
- Recorded: the two `ho-code` lines (the gain), whatever they say.
- S1 and S5 are live and read in Task 5.

**Pass:** on to Task 5 with `C` and `C_CODE`. **Fail:** `src/query.rs` is not touched for the seat. Commit the tooling alone — `git -C "$W" add bench/heldout.py bench/test_heldout.py bench/admission.py bench/test_admission.py bench/history/track.py bench/history/test_track.py`, subject `chore(bench): held-out sets by kind, a constant of the code list's own in the replay, tagged history rows` — write `$M/t4-verdict.txt` with the clause and the numbers (A4's price under the old gate beside this one's under the coverage form), and continue at Task 6 Step 8.

---

### Task 5: One seat for the code list — in the binary, measured, decided

Runs only if `$M/t4-verdict.txt` says `pass`.

**Files:**
- Modify: `src/query.rs` (`CODE_SEAT`, `CODE_GATE`, `code_seat()`, `code_seat_from`, the seat in `lexical_lists`, `lexical_lists_under` for the tests; tests)
- Modify: `src/ask.rs` (`code_seat` on the plain path; the test `a_plain_answer_is_unaffected_when_a_later_reranked_request_seats_the_code_list`)
- Modify: `src/index/lexical.rs` (the `Lexical::code_seat` and `ensure_code` doc comments)
- Create (scratch): `$M/t5-*.json`, `$M/t5-*.txt`, `$M/cost.py`

**Interfaces:**
- Consumes: `C` (`$M/t4-constant-c.txt`), `C_CODE` (`$M/t4-constant-code.txt`).
- Produces: `pub(crate) fn code_seat() -> bool` read by `ask::Context`; `REPOGRAPH_CODE_SEAT=1|0` for a measurement; `const CODE_SEAT: bool` (`false` until this task's verdict, `true` on a pass); `const CODE_GATE: f64 = C_CODE`.

- [ ] **Step 1: Write the failing tests**

In `src/query.rs`'s `mod tests`:

```rust
    #[test]
    fn the_code_seat_switch_reads_one_and_zero_and_nothing_else() {
        assert_eq!(code_seat_from(None), CODE_SEAT);
        assert!(code_seat_from(Some("1")) && code_seat_from(Some(" 1 ")));
        assert!(!code_seat_from(Some("0")));
        assert_eq!(code_seat_from(Some("yes")), CODE_SEAT, "a value that is neither is ignored, not read as on");
    }

    #[test]
    fn the_code_list_takes_one_seat_after_the_passages_when_admitted_and_none_when_off() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "правило отмены", "штраф считается по политике отмены", "a.md", 1);
        e.node(NodeKind::Symbol, "sym:apps/a.ts::revoke", "revoke", "Ends every session.\nrevoke() {}", "apps/a.ts", 3);
        e.node(NodeKind::Symbol, "sym:apps/a.ts::revokeOne", "revokeOne", "Ends one session.\nrevokeOne() {}", "apps/a.ts", 9);
        g.apply(e);
        let mut qs = Questions::default();
        let entry = |t: &str| crate::enrich::Entry { hash: String::new(), questions: vec![t.into()] };
        qs.entries.insert("sym:apps/a.ts::revoke".into(), entry("как выйти со всех устройств"));
        qs.entries.insert("sym:apps/a.ts::revokeOne".into(), entry("как выйти с одного устройства"));
        // No passage holds a word of the query, so the code list is alone with something to say
        // and is seated under any constant; the seat is its first document and nothing more.
        let query = "выйти всех устройств";
        let on = lexical_lists_under(&lex(&g, &qs), query, 10, false, QUESTIONS_GATE, true, CODE_GATE);
        assert_eq!(on.last().unwrap(), &vec!["sym:apps/a.ts::revoke".to_string()], "{on:?}");
        let off = lexical_lists_under(&lex(&g, &qs), query, 10, false, QUESTIONS_GATE, false, CODE_GATE);
        assert!(off.iter().all(|l| !l.contains(&"sym:apps/a.ts::revoke".to_string())), "{off:?}");
        // Reranked, the whole list is pooled whether or not the seat is on.
        assert_eq!(lexical_lists_under(&lex(&g, &qs), query, 10, true, QUESTIONS_GATE, false, CODE_GATE).last().unwrap().len(), 2);
    }
```

`code_questions_reach_the_reranked_pool_and_never_the_plain_fusion` becomes `code_questions_reach_the_reranked_pool_whole_and_the_plain_fusion_only_through_the_seat`: its plain-path assertion is made under `code_seat = false` through `lexical_lists_under`, so it says what it always said, and its premise assertion admits the code list under `CODE_GATE` rather than `QUESTIONS_GATE`.

Run: `cargo test --release --manifest-path "$W/Cargo.toml" code_seat 2>&1 | tail -3` — compile errors: `CODE_SEAT`, `CODE_GATE`, `code_seat_from`, `lexical_lists_under` do not exist.

- [ ] **Step 2: Implement**

In `src/query.rs`, after `gate_from`:

```rust
/// Whether the code-questions list takes one seat on the plain path. Off until measured on: A4
/// gave the list a seat under the raw-best ratio and read `where` 0/9 → 2/9 on the developer
/// suite at six held-out document questions lost in each arm, none gained, p = 0.031. Under the
/// coverage admission at a constant of the list's own the seat is measured again on the
/// code-enriched copy `bc-a1`, and this value is what that measurement decided
/// (`docs/bench/2026-09-0X-residue-seat-register-results.md`).
const CODE_SEAT: bool = false;

/// The code list's own constant under the coverage admission: the crossover, by the same
/// procedure as `QUESTIONS_GATE`, of 800 held-out questions on `bc-a1` in the `--no-dense` arm —
/// the 400 document questions the fixture's set is and 400 of the code's, one per node — on
/// `coverage(code) / coverage(passages)`. It is not `QUESTIONS_GATE` because a crossover is a
/// procedure over a population and the seat decides on a different one: on document questions
/// the code list never holds the answer, on code questions the passages hold a file never. Not
/// rounded, not tuned; if the seat fails, the seat fails.
const CODE_GATE: f64 = C_CODE;

/// `REPOGRAPH_CODE_SEAT=1` or `0` overrides `CODE_SEAT` for a measurement and for nothing else,
/// so one binary reads a store with the seat and without it. Anything else is ignored.
pub(crate) fn code_seat() -> bool {
    code_seat_from(std::env::var("REPOGRAPH_CODE_SEAT").ok().as_deref())
}

fn code_seat_from(override_: Option<&str>) -> bool {
    match override_.map(str::trim) { Some("1") => true, Some("0") => false, _ => CODE_SEAT }
}
```

with `C_CODE` the literal from `$M/t4-constant-code.txt`. `lexical_lists` becomes `lexical_lists_under(lex, query, depth, reranked, questions_gate(), code_seat(), CODE_GATE)`, and the plain-path tail of `lexical_lists_under` is:

```rust
    let mut lists = Vec::with_capacity(3);
    if admits(best(&generated), questions_index.attainable(query), passages_best, passages_attainable, gate) {
        lists.push(only_ids(generated));
    }
    lists.push(only_ids(passages));
    if seat {
        if let Some(code_index) = &lex.code {
            let code = code_index.search(query, depth);
            // One seat: `fuse::interleave` is a round-robin, so a one-element list takes the slot
            // its rank-one turn gives it and displaces no second document seed.
            if admits(best(&code), code_index.attainable(query), passages_best, passages_attainable, code_gate) {
                lists.push(only_ids(code).into_iter().take(1).collect());
            }
        }
    }
    lists
```

The doc comment on `lexical_lists` ends today with "The questions about code are a third list on the reranked path and on no other: the plain fusion's five seats were measured to be worth more to the documents than to them." — it becomes: on the reranked path whole, and on the plain path as one seat under `CODE_SEAT`, with the A4 price moved to `CODE_SEAT`'s comment. The comment block inside `lexical_lists`'s reranked branch that quotes A4 is shortened to point at `CODE_SEAT`.

In `src/ask.rs` line 277, `let code_seat = rerank.is_some();` becomes `let code_seat = rerank.is_some() || query::code_seat();`, and the comment above it says the plain path builds the list too once the seat is on, paying the build the 0.5.0 gap work made lazy (18.5 ms on a code-enriched store, `docs/bench/2026-09-05-0.5.0-gaps-results.md`) only on a store that has code questions. In `src/index/lexical.rs`, `Lexical::code_seat`'s doc comment ("a list the plain fusion never seats (`lexical_lists`)") and `ensure_code`'s say the same in one clause each.

Run: `cargo test --release --manifest-path "$W/Cargo.toml" 2>&1 | grep 'test result'` — 447 + 12 (445 + 2 new); clippy clean; build; `shasum "$B" | tee "$M/t5-binary.txt"`.

- [ ] **Step 3: Live with the seat on, by the override**

```bash
C=$(cut -d= -f2 "$M/t4-constant-c.txt"); C_CODE=$(cut -d= -f2 "$M/t4-constant-code.txt")
for arm in dense lexical; do
  nd=""; [ $arm = lexical ] && nd="--no-dense"
  REPOGRAPH_CODE_SEAT=1 "$B" --repo "$G/bc-a1" $nd dump --queries "$W/bench/cases.jsonl"     --out "$M/t5-on-rec-$arm.json"     >/dev/null 2>&1
  REPOGRAPH_CODE_SEAT=1 "$B" --repo "$G/bc-a1" $nd dump --queries "$W/bench/dev-cases.jsonl" --out "$M/t5-on-dev-$arm.json"     >/dev/null 2>&1
  REPOGRAPH_CODE_SEAT=1 "$B" --repo "$G/bc-a1" $nd dump --queries "$M/heldout-a1-docs.jsonl" --out "$M/t5-on-ho-docs-$arm.json" >/dev/null 2>&1
  REPOGRAPH_CODE_SEAT=1 "$B" --repo "$G/bc-a1" $nd dump --queries "$M/heldout-a1-code.jsonl" --out "$M/t5-on-ho-code-$arm.json" >/dev/null 2>&1
  REPOGRAPH_CODE_SEAT=1 "$B" --repo "$G/bc-a1" $nd bench                                    > "$M/t5-on-bench-rec-$arm-full.txt" 2>&1
  REPOGRAPH_CODE_SEAT=1 "$B" --repo "$G/bc-a1" $nd bench --cases "$W/bench/dev-cases.jsonl" > "$M/t5-on-bench-dev-$arm-full.txt" 2>&1
  tail -1 "$M/t5-on-bench-rec-$arm-full.txt" > "$M/t5-on-bench-rec-$arm.txt"; tail -1 "$M/t5-on-bench-dev-$arm-full.txt" > "$M/t5-on-bench-dev-$arm.txt"
  grep '^where' "$M/t5-on-bench-dev-$arm-full.txt" > "$M/t5-on-where-$arm.txt"
  # The fixture has no code index: with the seat on it must answer byte for byte as it did without.
  REPOGRAPH_CODE_SEAT=1 "$B" --repo "$F" $nd dump --queries "$W/bench/cases.jsonl" --out "$M/t5-fixture-rec-$arm.json" >/dev/null 2>&1
  cmp "$M/t5-fixture-rec-$arm.json" "$M/t3-rec-$arm.json" && echo "fixture $arm: unmoved by the seat"
done 2>&1 | tee "$M/t5-fixture.txt"
cd "$W/bench" && python3 admission.py check --store "$G/bc-a1/.repograph" --form coverage --c "$C" --code-seat --code-c "$C_CODE" "$M"/t5-on-*.json 2>&1 | tee "$M/t5-check.txt"
```

Required: S1's fixture half — both `cmp` silent (if Task 3 did not ship, compare against `$M/base-rec-$arm.json` instead); S5 — 82/82, 60/60, 400/400, 400/400 replicated in both arms.

- [ ] **Step 4: Held-out first, then the lines**

```bash
for arm in dense lexical; do
  python3 "$W/bench/heldout.py" compare "$M/t4-off-ho-docs-$arm.json" "$M/t5-on-ho-docs-$arm.json"
  python3 "$W/bench/heldout.py" compare "$M/t4-off-ho-code-$arm.json" "$M/t5-on-ho-code-$arm.json"
done 2>&1 | tee "$M/t5-heldout.txt"
cat "$M"/t5-on-bench-*.txt "$M"/t4-off-bench-rec-*.txt "$M"/t4-off-bench-dev-*.txt | tee "$M/t5-bench.txt"; grep -c HIT "$M"/t5-on-where-*.txt
```

Required: S4 — the two `docs` compares not significantly worse; S1 — both recorded lines `gated=true`, p90 ≤ 230; S2 — keyword, paraphrase, code each at least the `t4-off` count per arm; S3 — developer total at least the `t4-off` total per arm and `where` HIT count ≥ 1 in both. Recorded: the two `code` compares.

- [ ] **Step 5: Verdict, the constant's default, the cost**

`$M/t5-verdict.txt`: S0–S5 with files. **Pass:** `CODE_SEAT` becomes `true`, its comment gets the measured `where`, held-out and floor lines; `src/ask.rs`'s test `a_plain_answer_is_unaffected_when_a_later_reranked_request_seats_the_code_list` is rewritten as `a_plain_answer_on_a_code_enriched_store_seats_the_code_list_and_a_reranked_one_pools_it`: the plain answer already contains `revokeAllSessions` (the code list alone has something to say for «выйти устройств»), the reranked one still does, and a second plain answer equals the first — `ensure_code` is still covered by `a_plain_build_skips_the_code_list_and_ensure_code_adds_it_without_changing_what_already_answered` in `lexical.rs`. Rebuild, test, then prove the default is the override:

```bash
for arm in dense lexical; do
  nd=""; [ $arm = lexical ] && nd="--no-dense"
  "$B" --repo "$G/bc-a1" $nd dump --queries "$W/bench/dev-cases.jsonl" --out "$M/t5-default-dev-$arm.json" >/dev/null 2>&1
  cmp "$M/t5-default-dev-$arm.json" "$M/t5-on-dev-$arm.json" && echo "$arm: the default is the measured seat"
done 2>&1 | tee "$M/t5-default.txt"
```

Then the cost, one sitting, alternating, information only — a one-shot lexical `ask` on `bc-a1` now builds the code index on the plain path:

```bash
cat > "$M/cost.py" <<'PY'
"""Wall time of a one-shot lexical ask on bc-a1, seat off against seat on, blocks of eleven alternating, three each."""
import os, statistics, subprocess, sys, time
B, repo = sys.argv[1], sys.argv[2]
def block(seat):
    t = []
    for _ in range(11):
        env = dict(os.environ, REPOGRAPH_NO_SERVE="1", REPOGRAPH_CODE_SEAT=seat)
        s = time.perf_counter(); subprocess.run([B, "--repo", repo, "--no-dense", "ask", "--stale", "штраф за отмену записи"], env=env, capture_output=True, check=True); t.append((time.perf_counter() - s) * 1000)
    return statistics.median(t)
rows = {"0": [], "1": []}
for _ in range(3):
    for seat in ("0", "1"):
        rows[seat].append(block(seat))
for seat, v in rows.items():
    print(f"seat={seat}: block medians {' · '.join(f'{x:.1f}' for x in v)}  median {statistics.median(v):.1f} ms  spread {max(v) - min(v):.1f}")
PY
python3 "$M/cost.py" "$B" "$G/bc-a1" 2>&1 | tee "$M/t5-cost.txt"
```

Expected: seat on above seat off by roughly the 18.5 ms the eager build cost on 2026-09-05, past the off blocks' spread; a resident `serve` pays it once. Reported, not judged. **Fail:** `CODE_SEAT` stays `false` with the numbers in its comment; the ask.rs test keeps its name and premise; no cost sitting.

- [ ] **Step 6: Commit**

`git -C "$W" add src/query.rs src/ask.rs src/index/lexical.rs bench/heldout.py bench/test_heldout.py bench/admission.py bench/test_admission.py bench/history/track.py bench/history/test_track.py` and one commit: subject `feat(query): one seat for the code list on the plain path, at a constant of its own` on a pass, `feat(query): the code seat measured under the coverage admission and left off` on a fail; body with `C_CODE` and its crossover line, the eight replicated dumps, the `where` counts, both document held-out compares and both code ones, the four `bc-a1` lines against their seat-off lines, the fixture `cmp`s, and the cost blocks — each with its file.

---

### Task 6: The register — one generator, both registers (G14)

**Files:**
- Modify: `src/enrich.rs` (`prompt`'s text and doc comment; one test)
- Create (scratch): `$M/bc-r1/`, `$M/t6-*`, `$M/heldout-d1-400.jsonl`, `$M/register-share.py`

**Interfaces:**
- Consumes: `enrich::run`, `parse`, `Questions::save` as they are; the `claude` CLI at `enrich_command`; the local `e5-small` for the rows.
- Produces: `$M/bc-r1`, a copy of the fixture's graph and rows with every document's questions regenerated under the new prompt by the default generator; the neutral held-out set; the register-share table.

Tokens are spent in **Step 5 only**, once, through the copy's `enrich_command`: 1,996 document nodes on the default generator — ≈ $2.50 at D1's per-corpus haiku figure, an estimate; the tee'd prompt and answer bytes are the record it is checked against. Steps 1–7 may run as soon as Task 1 has committed; Step 8 onward waits for Task 5's binary.

- [ ] **Step 1: Write the failing test**

In `src/enrich.rs`'s `mod tests`, after `the_code_prompt_shows_the_path_and_asks_in_both_languages`:

```rust
    #[test]
    fn the_documents_prompt_asks_for_both_registers_and_keeps_its_roles() {
        let mut e = Extraction::default();
        e.node(NodeKind::Adr, "ADR-005", "Границы смены в UTC", "Смена хранится в UTC, не в настенном времени.", "a.md", 1);
        let p = prompt(&[&e.nodes[0]]);
        assert!(p.contains("### ADR-005\nГраницы смены в UTC\nСмена хранится в UTC"), "{p}");
        for phrase in ["two registers, six questions each", "first person allowed", "about to do that very thing",
                       "the entry's own vocabulary", "a customer, a front-desk employee, the business owner and a developer",
                       "id<TAB>synonyms", "id<TAB>text", "never translated"] {
            assert!(p.contains(phrase), "missing {phrase:?}");
        }
    }
```

Run: `cargo test --release --manifest-path "$W/Cargo.toml" both_registers 2>&1 | tail -3` — fails on `two registers`.

- [ ] **Step 2: The prompt**

In `src/enrich.rs`, `prompt`'s doc comment and body become:

```rust
/// Worded and measured on the development corpus (a Russian PRD); the example substitutions
/// and the four reader roles are its, and a rewording is a re-measure. Two registers, six
/// questions each, since 2026-09-06: with every question in the asker's colloquial voice the
/// developer suite's "is this allowed" questions held and paraphrase did not move; with every
/// question in the document's own precise voice paraphrase and held-out rose and four of nine
/// of those answers were lost; both registers together read above either alone on every kind
/// measured. The register, not the shape and not the model, is what retrieval was losing (gap
/// G14, `docs/bench/2026-09-06-g14-second-diagnostic.md`), and one generator asked for both is
/// the lever measured in `docs/bench/2026-09-0X-residue-seat-register-results.md`.
pub fn prompt(nodes: &[&Node]) -> String {
    let mut p = String::from(
        "Below are entries from a product's documentation: an id, a title line and the start of the text.\n\
         For each entry write 12 short questions (3-10 words) that a person who has never read this \
         documentation could ask to find exactly this entry, in two registers, six questions each.\n\
         The first six are in the words the person asking would actually use, as if said aloud to a \
         colleague: everyday words, not the entry's own terms (SLA -> сроки ответа, аудит-лог -> история \
         действий, no-show -> клиент не пришёл), first person allowed (можно ли мне..., а если я..., нам \
         нельзя...?), and where the entry decides or forbids something, the question of a person about \
         to do that very thing. The other six are in the entry's own vocabulary: the precise terms, \
         names and system-side wording the entry itself uses, as the person who wrote it would ask \
         about it. Across all twelve, vary the wording so that no two share their key words, and take \
         the points of view of a customer, a front-desk employee, the business owner and a developer, \
         three questions each, every one about a different detail of the entry. Then add one line \
         `id<TAB>synonyms: ...` with 5-10 everyday synonyms or paraphrases of the entry's key terms, \
         comma-separated. Every entry gets its lines, including one that has only a title. Write \
         every question and synonym in the language the entry itself is written in (a Russian entry \
         gets Russian questions), never translated. Output exactly one question per line, in the form \
         `id<TAB>text`, with no numbering and no commentary.\n\n");
    for n in nodes {
        p.push_str(&format!("### {}\n{}\n\n", n.id, passage(n)));
    }
    p
}
```

The module doc comment's first paragraph ("Questions a reader might ask to reach a node …") gains half a sentence: half in the asker's words, half in the document's. `prompt_code` is untouched — the code register was never the finding.

- [ ] **Step 3: Tests, clippy, the generator's binary**

`cargo test --release --manifest-path "$W/Cargo.toml" 2>&1 | grep 'test result'` (one more unit test than the task before); clippy clean; `cargo build --release --manifest-path "$W/Cargo.toml"`; `shasum "$B" | tee "$M/t6-enrich-binary.txt"` — this binary only generates; nothing is read on it.

- [ ] **Step 4: The copy, with the writer trap closed**

```bash
mkdir -p "$M/bc-r1/.repograph"
for f in graph.json graph.bin manifest.json vectors.json vectors.f32; do cp "$F/.repograph/$f" "$M/bc-r1/.repograph/"; done
printf '{"entries":{}}\n' > "$M/bc-r1/.repograph/questions.json"      # every eligible node stale; no questions.bin
cat > "$M/bc-r1/repograph.toml" <<'TOML'
embed_model = "intfloat/multilingual-e5-small"
enrich_model = "haiku"
enrich_command = "tee -a /Users/max/bench/residue-seat-register-2026-09-06/t6-prompts.txt | MAX_THINKING_TOKENS=0 claude -p --model {model} --output-format text --tools '' --setting-sources '' --no-session-persistence | tee -a /Users/max/bench/residue-seat-register-2026-09-06/t6-outputs.txt"
TOML
ls "$M/bc-r1/.repograph"; test -z "${REPOGRAPH_ENRICH_MODEL:-}" && test ! -e "$HOME/.config/repograph/config.toml" && echo "no override, no machine file"; claude --version
```

Expected: six files and no `questions.bin`; `no override, no machine file`; the CLI's version line, written into `$M/task6.txt`. `enrich_model = "haiku"` is what `src/config.rs` ships as the default; it is named so the record says which generator, not so it changes.

- [ ] **Step 5: The tokens**

```bash
"$B" --repo "$M/bc-r1" enrich --batch 12 --parallel 8 2>&1 | tee "$M/t6-enrich.log" | tail -3
```

Expected: `enrich: 1996 nodes written, 0 dropped, 0 still without questions, 167 batches (0 failed) in <n>s` then `dense: <≈26000> vectors embedded …` on the small model (the passage rows are already there; the new question rows are what is embedded, about a minute). `still without questions` above 0 means the model skipped a node twice: run the same `enrich` once more — it asks only for what is stale — and record both lines; a second miss stays as is. A failing batch (`failed > 0`) is retried the same way. Then the survey and the estimate:

```bash
python3 - <<'PY'
import json, statistics
M="/Users/max/bench/residue-seat-register-2026-09-06"; F="/Users/max/bench/beauty-crm-502e8a6d"
r=json.load(open(f"{M}/bc-r1/.repograph/questions.json"))["entries"]; f=json.load(open(f"{F}/.repograph/questions.json"))["entries"]
print("bc-r1 entries", len(r), "median questions/node", statistics.median(len(v["questions"]) for v in r.values()), "total", sum(len(v["questions"]) for v in r.values()))
print("fixture entries", len(f), "median", statistics.median(len(v["questions"]) for v in f.values()), "total", sum(len(v["questions"]) for v in f.values()))
print("questions identical to the fixture's:", sum(q in set(f.get(k,{}).get("questions",[])) for k,v in r.items() for q in v["questions"]))
v=json.load(open(f"{M}/bc-r1/.repograph/vectors.json")); print("rows", len(v["ids"]), "model", repr(v.get("model")), "dim", v["dim"])
PY
wc -c "$M/t6-prompts.txt" "$M/t6-outputs.txt" | tee "$M/t6-bytes.txt"
```

Expected: 1,996 entries at a median near 13 (twelve questions and a synonyms line), a few hundred identical to the fixture's at most (the same generator, the same passages), rows near the fixture's 34,802, `model 'intfloat/multilingual-e5-small' dim 384`. The estimate is bytes ÷ 4 at the generator's list price, stated as an estimate and as an underestimate for Cyrillic, exactly as D1's was.

- [ ] **Step 6: The neutral held-out set**

```bash
python3 "$W/bench/heldout.py" build --store "$G/bc-d1/.repograph" --out "$M/heldout-d1-400.jsonl"   # bc-d1 holds document entries only, so no --only is needed and Task 4 need not have run
python3 - <<'PY'
import json
M="/Users/max/bench/residue-seat-register-2026-09-06"; G="/Users/max/bench/gaps-2026-09-05"; F="/Users/max/bench/beauty-crm-502e8a6d"
d1=[json.loads(l) for l in open(f"{M}/heldout-d1-400.jsonl")]; fx=[json.loads(l) for l in open(f"{G}/heldout-400-syn.jsonl")]
print("same 400 anchor nodes as the fixture's set:", [r["expect"] for r in d1] == [r["expect"] for r in fx])
for name, path in (("fixture", f"{F}/.repograph/questions.json"), ("bc-r1", f"{M}/bc-r1/.repograph/questions.json")):
    q=json.load(open(path))["entries"]
    print(name, "holds", sum(r["q"] in q.get(r["expect"],{}).get("questions",[]) for r in d1), "of the 400 neutral questions verbatim")
PY
```

Expected: `400 questions from 1996 enriched nodes, seed 20260905`; `True` — `bc-d1` carries the same 1,996 keys, so the seed draws the same nodes; `fixture holds 0` and `bc-r1 holds 0` (or a handful — a coincidence of wording is recorded, not removed; the set stays what the seed drew).

- [ ] **Step 7: The register-share table**

```bash
cat > "$M/register-share.py" <<'PY'
"""The second diagnostic's metric: of a developer question's own words longer than three characters,
the share found anywhere in the expected node's questions, averaged per kind, for each store."""
import json, re, statistics, sys
cases = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
stores = {a.split("=")[0]: json.load(open(a.split("=")[1]))["entries"] for a in sys.argv[2:]}
def share(q, entry):
    words = {w for w in re.findall(r"\w+", q.lower()) if len(w) > 3}
    text = " ".join(entry).lower()
    return sum(w in text for w in words) / len(words) if words else 0.0
print("| kind | n | " + " | ".join(stores) + " |"); print("|---|---|" + "---|" * len(stores))
for kind in ("long", "cross", "multi", "where", "rule"):
    row = []
    for name, q in stores.items():
        vals = []
        for c in cases:
            if c["kind"] != kind: continue
            anchors = c["expect"] if isinstance(c["expect"], list) else [c["expect"]]
            entry = next((q[a]["questions"] for a in anchors if a in q), None)
            if entry is not None: vals.append(share(c["q"], entry))
        row.append(f"{statistics.fmean(vals):.3f}" if vals else "—")
    print(f"| {kind} | {sum(c['kind'] == kind for c in cases)} | " + " | ".join(row) + " |")
PY
python3 "$M/register-share.py" "$W/bench/dev-cases.jsonl" "fixture=$F/.repograph/questions.json" "bc-d1=$G/bc-d1/.repograph/questions.json" "bc-r1=$M/bc-r1/.repograph/questions.json" | tee "$M/t6-register-share.txt"
```

Expected: the fixture and `bc-d1` columns reproduce the second diagnostic's shape — `rule` 0.195 against 0.139, `long` 0.186 against 0.245 (`g14-ranks.txt`'s metric; small differences from that script's tokenisation are noted, not chased); the `bc-r1` column is the mechanism check — on `rule` it should sit at or above the fixture's if the prompt moved register, and wherever it sits is recorded. Information, not a clause.

- [ ] **Step 8: Measure on the final binary — held-out first**

Only after Task 5 has left its binary (`shasum "$B"` equals the last line of `$M/t5-binary.txt`, or `$M/t3-binary.txt` if Task 5 did not run, or `$M/base-binary.txt` if neither shipped — write which into `$M/t6-binary.txt`):

```bash
for arm in dense lexical; do
  nd=""; [ $arm = lexical ] && nd="--no-dense"
  for pair in "fixture:$F" "r1:$M/bc-r1"; do
    tag=${pair%%:*}; store=${pair#*:}
    "$B" --repo "$store" $nd dump --queries "$M/heldout-d1-400.jsonl"   --out "$M/t6-$tag-ho-d1-$arm.json"  >/dev/null 2>&1
    "$B" --repo "$store" $nd dump --queries "$G/heldout-400-syn.jsonl"  --out "$M/t6-$tag-ho-fx-$arm.json"  >/dev/null 2>&1
    "$B" --repo "$store" $nd dump --queries "$W/bench/cases.jsonl"      --out "$M/t6-$tag-rec-$arm.json"    >/dev/null 2>&1
    "$B" --repo "$store" $nd dump --queries "$W/bench/dev-cases.jsonl"  --out "$M/t6-$tag-dev-$arm.json"    >/dev/null 2>&1
  done
  python3 "$W/bench/heldout.py" compare "$M/t6-fixture-ho-d1-$arm.json" "$M/t6-r1-ho-d1-$arm.json"
  python3 "$W/bench/heldout.py" compare "$M/t6-fixture-ho-fx-$arm.json" "$M/t6-r1-ho-fx-$arm.json"
done 2>&1 | tee "$M/t6-heldout.txt"
CFINAL=$(grep -o 'QUESTIONS_GATE: f64 = [0-9.]*' "$W/src/query.rs" | awk '{print $NF}')
SEAT=""; if grep -q 'const CODE_SEAT: bool = true' "$W/src/query.rs"; then SEAT="--code-seat --code-c $(grep -o 'CODE_GATE: f64 = [0-9.]*' "$W/src/query.rs" | awk '{print $NF}')"; fi
cd "$W/bench" && python3 admission.py check --store "$M/bc-r1/.repograph" --form coverage --c "$CFINAL" $SEAT "$M"/t6-r1-rec-*.json "$M"/t6-r1-dev-*.json 2>&1 | tee "$M/t6-check.txt"
```

Required: G4 — the two `ho-d1` compares (the first of each pair) not significantly worse; G5 — `check` replicates 82/82 and 60/60 in both arms on `bc-r1` (pass `--code-seat` only if `CODE_SEAT` shipped `true`; the store has no code index either way). Recorded: the two `ho-fx` compares, with the sentence that the fixture has its home advantage on that set.

- [ ] **Step 9: The lines**

```bash
for arm in dense lexical; do
  nd=""; [ $arm = lexical ] && nd="--no-dense"
  for pair in "fixture:$F" "r1:$M/bc-r1"; do
    tag=${pair%%:*}; store=${pair#*:}
    "$B" --repo "$store" $nd bench                                    > "$M/t6-$tag-bench-rec-$arm-full.txt" 2>&1
    "$B" --repo "$store" $nd bench --cases "$W/bench/dev-cases.jsonl" > "$M/t6-$tag-bench-dev-$arm-full.txt" 2>&1
    tail -1 "$M/t6-$tag-bench-rec-$arm-full.txt" > "$M/t6-$tag-bench-rec-$arm.txt"; tail -1 "$M/t6-$tag-bench-dev-$arm-full.txt" > "$M/t6-$tag-bench-dev-$arm.txt"
    grep -c '^rule .*HIT' "$M/t6-$tag-bench-dev-$arm-full.txt" > "$M/t6-$tag-rule-$arm.txt"
  done
done
cat "$M"/t6-*-bench-*.txt; for f in "$M"/t6-*-rule-*.txt; do echo "$f $(cat "$f")"; done | tee "$M/t6-rule.txt"
```

Required: G1 — both `r1` recorded lines `gated=true`, exit 0, p90 ≤ 230; G2 — `r1`'s keyword, paraphrase and code each at least `fixture`'s per arm; G3 — `r1`'s developer total at least `fixture`'s per arm and `rule` ≥ 5 in both; G6 — at least one of: `r1` paraphrase above `fixture`'s in either arm, an `ho-d1` compare significantly better in either arm, `rule` above 5 in either arm.

- [ ] **Step 10: Verdict and commit**

`$M/t6-verdict.txt`: G1–G6 with files. **Pass:** `git -C "$W" add src/enrich.rs` and one commit, subject `feat(enrich): the documents' prompt asks for both registers`, body with the `enrich` line, the survey, the bytes and the estimate, both held-out compares per arm (which decided and which is recorded), the four `bc-r1` lines beside the fixture's, the `rule` counts and the register-share table — each with its file. **Fail:** `git -C "$W" checkout -- src/enrich.rs`; `$M/bc-r1` stays; the same list goes to Task 7 as the record of the register prompt measured on the default generator and rejected, with the clause. **No effect** (G1–G5 pass, G6 fails): the same revert, recorded under that name.

---

### Task 7: The record — results, gaps, ADR, README, history, PR

**Files:**
- Create: `docs/bench/2026-09-0X-residue-seat-register-results.md` (X the day the last number was read)
- Modify: `docs/bench/next-version-gaps.md` (G8, G13, G14 status lines and the suggested-order rows), `docs/adr/ADR-001-paraphrase-recall-was-a-prediction.md` (Amendment 9, only if Task 3 or Task 5 shipped), `README.md`, `bench/history/runs.jsonl`
- Create (scratch): `$M/t7-*.txt`, `$M/t7-pr.md`

**Interfaces:**
- Consumes: every `$M` file the earlier tasks named and the three verdict files. Every number in the prose is copied from one of them and the file is named beside it, as the coverage results document does.

- [ ] **Step 1: The results document**

In the shape of `docs/bench/2026-09-06-coverage-admission-results.md`: a header naming the branch, the base commit `32577e2`, the spec's commit (`$M/spec-commit.txt`), the fixture, the copies and `$M`; then one section per rule in the order R, S, G, each opening with the rule as pre-registered (copied from the spec, not paraphrased) and closing with pass/fail per clause:

- **R** — the crossover line (`$M/t2-crossover.txt`), the attainable-shift medians per suite and index (`$M/t2-lists.txt`), the six replay lines (`$M/t3-check.txt`), the four fixture arms and the two raw lines against their baselines (`$M/t3-bench.txt`), the per-case moves (`$M/t3-case-moves-*.txt`), both held-out compares (`$M/t3-heldout.txt`), the G13 table under the new denominator pasted whole (`$M/t3-g13.md`). If A failed and B was tried, both with their clauses; if neither shipped, the paragraph says the residue stays named and what each design cost.
- **S** — the three held-out files and the `cmp` proving the documents' set is the fixture's, the seat-off lines (`$M/t4-off-bench-*.txt`), the constant and its crossover line with the four percentages (`$M/t4-crossover-code.txt`), the offline prediction (`$M/t4-score.txt`), the live lines and `where` counts (`$M/t5-bench.txt`, `$M/t5-on-where-*.txt`), the document and code held-out compares (`$M/t5-heldout.txt`), the eight replay lines (`$M/t5-check.txt`), the fixture `cmp`s, the cost blocks (`$M/t5-cost.txt`), and A4's price beside this seat's. If the replay failed, the section says so with the clause and the code seat stays priced.
- **G** — the retirement of L4 as framed in one paragraph with its two diagnostic readings; the copy and the generator (`$M/task6.txt`), the `enrich` line and survey (`$M/t6-enrich.log`), the bytes and the estimate (`$M/t6-bytes.txt`), the neutral set's provenance and the overlap check, the register-share table (`$M/t6-register-share.txt`), both held-out compares per arm with which decided (`$M/t6-heldout.txt`), the four `bc-r1` lines beside the fixture's (`$M/t6-*-bench-*.txt`), the `rule` counts, and the sonnet reading priced and not run.
- **What ships and what stays open** — one table with the three rules, verdict, the deciding number and the file; then the list of everything measured and rejected with the file each rests on; then the test counts (Rust unit, `serve`, the three Python files).

- [ ] **Step 2: The gaps document**

Under `## G8`, `## G13` and `## G14` in `docs/bench/next-version-gaps.md`, a `**Status (2026-09-0X, 0.5.0):**` paragraph in the style of the existing ones — closed for vocabulary / still open with the residue's two designs measured; closed with the seat's numbers / still open with the seat's price under the coverage form; closed with the register prompt / measured and rejected, and in either case L4 as framed retired against the second diagnostic — each linking the results document. In `## Suggested order`, the three rows get the strike-through-and-reason treatment or a one-line "measured, not closed" reason.

- [ ] **Step 3: ADR-001 Amendment 9 — only if the admission changed**

`## Amendment 9 — the denominator charges every term, and a seat for the code list (2026-09-0X)`: what `attainable` dropped and now charges, the re-derived constant with its crossover line, the four arms unmoved, both held-out compares; the seat's shape, its own constant and what it was derived on, the `where` counts, the document held-out price against A4's; and, as Amendment 8 did, what did not survive. If neither Task 3 nor Task 5 shipped, no amendment; the results document carries both failures and the G8/G13 status lines point to it.

- [ ] **Step 4: README**

Each edit names the passage as it reads today so the diff is reviewable:

- **Spending tokens on purpose**, the admission paragraph ("Each list is asked what fraction of the question its best document actually reached: `best / attainable`, where `attainable` is the idf the query could have collected in that index at all."): if R shipped, `attainable` is the idf of every term the query asked, a term the index never saw at df = 0, and the constant is the re-derived one with one sentence on why it moved; if not, unchanged.
- The code paragraph ("The code questions are an index of their own, searched for the `--rerank` pool and nowhere else: the plain `ask` fusion is byte for byte the fusion of a store without them …"): if S shipped, the code list takes one seat on the plain path under the coverage admission at a constant of its own, with the `where` and held-out numbers, the history of A3/A4 kept as history, and "a store without code questions is therefore the old index byte for byte" kept because it is still true; if not, one sentence that the seat was measured under the coverage form and what it cost.
- The enrich paragraph ("**`repograph enrich`** asks a model, once per requirement-like node, for twelve questions a reader might ask to find that node in everyday words plus a line of synonyms"): if G shipped, six in the asker's own words and six in the entry's, why, and the numbers; if not, unchanged, and the "with `rule` as the thing to watch" sentence beside `enrich_model` stays.
- **Asking a resident process** / the perf rows: if S shipped, one sentence that a one-shot lexical `ask` on a store with code questions builds the code index again (`$M/t5-cost.txt`), a resident `serve` once.

- [ ] **Step 5: History rows**

```bash
cd "$W" && NOTE="residue/seat/register: R <shipped|rejected>, S <shipped|rejected>, G <shipped|rejected|no effect>" bench/history/run-repograph.sh 2>&1 | tee "$M/t7-history-rec.txt"
cd "$W" && CASES=bench/dev-cases.jsonl NOTE="same" bench/history/run-repograph.sh 2>&1 | tee "$M/t7-history-dev.txt"
for arm in dense lexical; do
  python3 "$W/bench/history/track.py" record "$M/t5-on-bench-rec-$arm-full.txt" --corpus beauty-crm --corpus-path "$G/bc-a1" --tag a1-seat --note "bc-a1 with the code seat on (Task 5)"
  python3 "$W/bench/history/track.py" record "$M/t5-on-bench-dev-$arm-full.txt" --corpus beauty-crm --corpus-path "$G/bc-a1" --tag a1-seat --note "bc-a1 with the code seat on (Task 5)"
  python3 "$W/bench/history/track.py" record "$M/t6-r1-bench-rec-$arm-full.txt" --corpus beauty-crm --corpus-path "$M/bc-r1" --tag r1 --note "bc-r1, the register prompt on the default generator (Task 6)"
  python3 "$W/bench/history/track.py" record "$M/t6-r1-bench-dev-$arm-full.txt" --corpus beauty-crm --corpus-path "$M/bc-r1" --tag r1 --note "bc-r1, the register prompt on the default generator (Task 6)"
done 2>&1 | tee "$M/t7-history-copies.txt"
```

The fixture rows land under their usual arms with the corpus commit `502e8a6d`; the copies' rows land under `+a1-seat` and `+r1` with no corpus commit (a store copy has no checkout, as the 2026-09-05 large-model rows show), which keeps them out of every cross-arm reading. Skip the `t5-on-*` rows if Task 5 did not run, and record `t4-off-*` instead under `--tag a1`.

- [ ] **Step 6: Commit and PR**

Docs and history in one commit: `git -C "$W" add docs README.md bench/history/runs.jsonl`, subject `docs(bench): the residue, the seat and the register — three rules, three verdicts`, body listing per rule the verdict and the results document's section. Then:

```bash
git -C "$W" push -u origin feat/residue-seat-register
gh auth switch --user devmaxxx && gh pr create --repo devmaxxx/repograph --base main --head feat/residue-seat-register --title "feat: the residue, the seat and the register — 0.5.0 line" --body-file "$M/t7-pr.md"
```

`$M/t7-pr.md` is the results document's "What ships and what stays open" section — the three-rule table, the rejected list, the test counts; no AI trailers, no session links. The executor does not merge.

---

## Placeholder check, done when the plan was written

Every `$M/...` path is a file a step creates before another reads it. The numbers this plan cannot know are named as the files that will hold them and are written into code or prose only by copying from those files: `C1` (`$M/t2-constant.txt`, Task 2 Step 7 — into `QUESTIONS_GATE` in Task 3 Step 1), `C_CODE` (`$M/t4-constant-code.txt`, Task 4 Step 6 — into `CODE_GATE` in Task 5 Step 2), the seat's verdict (`$M/t5-verdict.txt` — into `CODE_SEAT` in Task 5 Step 5), and the results document's date. The four predicted coverages in Task 2 Step 3 are predictions from this plan's arithmetic and are replaced by the run's printed values in the test's comment. No step says "similar to" another; the Rust in Tasks 2, 5 and 6 is written against `src/index/lexical.rs`, `src/query.rs`, `src/ask.rs`, `src/dump.rs` and `src/enrich.rs` as they stand at `32577e2`.
