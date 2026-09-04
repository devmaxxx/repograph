# Bench Gaps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the six gaps the 2026-09-03 three-graph run measured on repograph 0.4.0, in the order that removes the most missing answer first: the scoring records rank; `changes` stops silently dropping files it cannot parse; the two false impact misses leave the truth file and the one real miss (a re-exporting barrel) leaves `impact`; the blast suite grows to a size a delta can be judged on; the one unmeasured paraphrase lever — a local cross-encoder — is measured; and a second corpus gets a case file before 0.6.0 ships a language for it.

**Architecture:** Two halves that never touch each other's floors. The harness half is Python in `bench/compare/` (a `rank_of` scorer and MRR, string literals stripped from the truth, `--cases`/`--blast` flags, a 32-case blast set, a per-corpus case directory). The binary half is three small Rust changes — `changes::touched` reports an unknown file as `file:<rel>`, `impact::importers` adds the barrels `aliases` already finds, and `src/index/cross.rs` runs `BAAI/bge-reranker-v2-m3` through the same `ort` + `tokenizers` path `embed.rs` uses, behind `ask --rerank-local` — plus a measurement of that last one that decides nothing by itself.

**Tech Stack:** Rust 1.98, ort 2.0.0-rc.13, tokenizers 0.22.2, hf-hub 0.5.0, clap 4.6.6 (all pinned; **no new crates**). Python 3.14 stdlib only (`unittest`, no pytest on this machine). `optimum-cli` in a throwaway venv, once, to export the reranker to ONNX.

**Spec:** `docs/bench/next-version-gaps.md` (the six gaps, G1–G6), argued from `docs/bench/2026-09-03-three-graphs-results.md`; protocol in `docs/bench/three-graphs.md`, day-of steps in `docs/bench/runbook.md`.

## Global Constraints

- **Bench floors do not move in this plan:** keyword 40/40, paraphrase ≥14/30 dense / ≥11/30 no-dense, code 12/12, p90 ≤230 tokens (`src/bench.rs`, `passes`). `ask` output without `--rerank-local` must be byte-identical before and after every task here; Tasks 2 and 4 touch only `changes` and `impact`.
- **No new crates.** The cross-encoder rides `ort`, `tokenizers`, `hf-hub` exactly as `src/index/embed.rs` does. The ONNX export is a Python tool run once outside the build.
- **No version bump here.** 0.5.0's bump belongs to `docs/superpowers/plans/2026-09-03-replace-gitnexus.md` Task 8 (the registry). This plan rides the same tag; whichever lands second does not bump again.
- **The corpus is `beauty-crm` at `/Users/max/Documents/projects/beauty-crm`.** Every measurement in this plan pins the commit it ran at; the plan was written against `2483d932`. A bench run at a different commit is a different number, not a regression — say the commit.
- **`beauty-crm` keeps `graphify-out/` and `.gitnexus/` on disk and runs neither tool.** Every comparison in this plan is `--tools repograph`; a three-tool re-run follows `docs/bench/runbook.md` in a scratch worktree and is not part of this plan.
- Comments explain *why*, never *what*; no ticket ids. Code, docs, commits in English. Conventional Commits. **The repo's commit hook rejects AI trailers and session links** — do not add `Co-Authored-By` or `Claude-Session` lines, whatever the session's attribution reminder says; the hook wins. Never combine a heredoc and `git commit` in one shell call: the hook rejects the whole compound command and the `git add` in front of it is lost too.
- Python in `bench/compare/` is tested with `python3 -m unittest` from that directory; Rust with `cargo test`. Every step that changes a number ends with the command that printed it.

---

## File structure

```
bench/compare/run.py            rank_of, `rank` per row, `mrr` in summary, --cases/--blast     (Task 1, 5)
bench/compare/report.py         "retrieval, MRR" row                                             (Task 1)
bench/compare/truth.py          rg reads the tree not stdin; string literals are not references  (Task 1, 3)
bench/compare/test_run.py       NEW — unittest for rank_of and the case-file shapes               (Task 1, 5, 7)
bench/compare/test_truth.py     NEW — unittest for strip_comments                                (Task 3)
bench/blast.jsonl               20 → 32 cases                                                    (Task 5)
bench/corpora/beauty-crm-mobile/blast.jsonl   NEW — 8 Kotlin impact cases, a baseline           (Task 7)
bench/results/<date>-*.json     one file per measurement                                          (Task 2, 3, 4, 5, 7, 8)
src/changes.rs                  touched() reports an unindexed file; render counts symbols/files (Task 2)
src/impact.rs                   importers() includes re-exporting barrels                         (Task 4)
src/index/cross.rs              NEW — CrossEncoder::open/score, pick                              (Task 6)
src/index/mod.rs                pub mod cross                                                     (Task 6)
src/config.rs                   reranker_dir                                                      (Task 6)
src/main.rs                     Ask/Bench --rerank-local                                          (Task 6)
src/bench.rs                    run() takes rerank_local                                          (Task 6)
repograph.toml                  reranker_dir = ""                                                 (Task 6)
README.md                       Blast radius, Configure, Spending tokens on purpose               (Task 2, 4, 6)
docs/bench/three-graphs.md      rank/MRR, decision rule, 32-case table, adding a corpus            (Task 1, 3, 5, 7)
docs/adr/ADR-001-…md            Amendment 4: the cross-encoder, measured                            (Task 6)
docs/bench/next-version-gaps.md status per gap                                                     (Task 8)
docs/bench/<date>-repograph-0.5.0-results.md   NEW — before/after                                  (Task 8)
```

---

### Task 1: Rank and MRR — the scoring learns where the answer sits (G5)

**Files:**
- Modify: `bench/compare/run.py:22` (after `NO_PATH`), `run.py:86-99` (`score_retrieval`), `run.py:150-165` (`summarise`, retrieval block)
- Modify: `bench/compare/report.py:51-56` (retrieval rows)
- Modify: `bench/compare/truth.py:36-38` (`rg`)
- Modify: `docs/bench/three-graphs.md` — "Reading a result file", "Caveats"
- Create: `bench/compare/test_run.py`

**Interfaces:**
- Produces: `run.rank_of(answer: str, want: str) -> int | None`; retrieval rows gain `"rank"`; `summary["retrieval"]["mrr"]: float`.

- [ ] **Step 1: Write the failing tests**

`bench/compare/test_run.py`:

```python
"""The scorer, on answers copied from real tool output."""

import unittest

from run import rank_of

# `repograph ask "расход виден салону"` on beauty-crm at 2483d932, verbatim.
ANSWER = (
    "FR-AI-138  docs/prd-2026-08-16/prd/07-ai-layer.md:1163  Расход виден салону.\n"
    "NFR-PH-7  docs/prd-2026-08-16/prd/13-phasing.md:1470  Стоимость\n"
    "FR-SEC-92  docs/prd-2026-08-16/prd/12-security-prereqs.md:1144  Внутренний экран\n"
    "  FR-WH-30  docs/prd-2026-08-16/prd/08-marketing-inventory.md:981  Норматив  ← N-137\n"
)
# `repograph ask asGrosze`, verbatim.
CODE = (
    "sym:packages/contracts/src/money.ts::asGrosze  packages/contracts/src/money.ts:31  asGrosze\n"
    "sym:packages/domain/src/money/index.ts::asGrosze  packages/domain/src/money/index.ts:115  asGrosze\n"
)


class RankOf(unittest.TestCase):
    def test_first_line_is_rank_one(self):
        self.assertEqual(rank_of(ANSWER, "FR-AI-138"), 1)

    def test_two_ids_before_it_make_rank_three(self):
        self.assertEqual(rank_of(ANSWER, "FR-SEC-92"), 3)

    def test_a_neighbour_line_counts_its_ids_too(self):
        # FR-AI-138, NFR-PH-7, FR-SEC-92, FR-WH-30 come first: the reader passed four.
        self.assertEqual(rank_of(ANSWER, "N-137"), 5)

    def test_absent_is_none(self):
        self.assertIsNone(rank_of(ANSWER, "FR-CAL-101"))

    def test_a_path_case_counts_paths_not_ids(self):
        self.assertEqual(rank_of(CODE, "packages/contracts/src/money.ts"), 1)
        self.assertEqual(rank_of(CODE, "packages/domain/src/money/index.ts"), 2)

    def test_the_same_token_repeated_is_one_competitor(self):
        self.assertEqual(rank_of("FR-A-1 x\nFR-A-1 y\nFR-B-2\n", "FR-B-2"), 2)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run it to see it fail**

Run: `cd bench/compare && python3 -m unittest test_run -v`
Expected: `ImportError: cannot import name 'rank_of' from 'run'`

- [ ] **Step 3: Add `rank_of` and record it per row**

In `bench/compare/run.py`, after the `NO_PATH` line:

```python
# What competes with an answer for the reader's eye: another id of the same shape, or for a
# file case another path. `BE-M17`, `FR-AI-138`, `INV-16`, `N-137` all match the first;
# `docs/prd/x.md` and `apps/api/src/y.ts` the second.
ID_TOKEN = re.compile(r"\b[A-Z]{1,5}(?:-[A-Z]{1,6})?-[A-Z]?\d{1,4}\b")
PATH_TOKEN = re.compile(r"[\w./-]+/[\w.-]+\.(?:tsx?|kt|md|json|ya?ml|sql)\b")


def rank_of(answer: str, want: str) -> int | None:
    """1 + the distinct competing answers a reader passes before `want`.

    Strict is a substring test and says nothing about where in the answer the id sits — an id
    fifth of five counted the same as first on 2026-09-03. This counts the other ids (for a
    file case, the other paths) that appear before the first occurrence of `want`: first reads
    1, buried behind four neighbours reads 5. Tool-agnostic on purpose: it reads the text every
    tool prints, not a structure only one of them has.
    """
    at = answer.find(want)
    if at < 0:
        return None
    pattern = PATH_TOKEN if "/" in want else ID_TOKEN
    seen: list[str] = []
    for m in pattern.finditer(answer[:at]):
        token = m.group(0)
        if token != want and token not in seen:
            seen.append(token)
    return len(seen) + 1
```

In `score_retrieval`, the row dict gains one key after `"soft"`:

```python
            "strict": want in answer, "soft": bool(named(answer, files)),
            "rank": rank_of(answer, want),
```

In `summarise`, inside `if ret:` before `out["retrieval"] = {`:

```python
        ranks = [r.get("rank") for r in ret]
```

and one more key in the `out["retrieval"]` dict, after `"n"`:

```python
            "mrr": round(statistics.mean(1 / r if r else 0.0 for r in ranks), 3),
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cd bench/compare && python3 -m unittest test_run -v`
Expected: `Ran 6 tests … OK`

- [ ] **Step 5: Report the row**

In `bench/compare/report.py`, after `row("soft, all", ret("soft"))`:

```python
    row("retrieval, MRR", lambda x: x.get("retrieval", {}).get("mrr"))
```

Run: `python3 report.py ../results/2026-09-03-beauty-crm.json | grep MRR`
Expected: `| retrieval, MRR | — | — | — |` — the 2026-09-03 rows carry no `rank`; the scorer stores no answer text, so they cannot be rescored. From this task on they are.

- [ ] **Step 6: Stop `rg` from reading stdin**

While in the harness: `truth.rg` spawns ripgrep with no path argument, and ripgrep searches **stdin** instead of the tree whenever stdin is not a terminal — under a heredoc, a CI step, or `subprocess` from another script it returns nothing and every truth list comes out empty. Found while measuring Task 3's misses. In `bench/compare/truth.py`, `rg`:

```python
def rg(repo: Path, args: list[str]) -> list[str]:
    # stdin detached: ripgrep searches stdin instead of the tree when stdin is not a tty,
    # which turned every truth list empty under a heredoc and would do the same in CI.
    out = subprocess.run(["rg", *args, *EXCLUDE], cwd=repo, capture_output=True, text=True,
                         stdin=subprocess.DEVNULL)
    return [line for line in out.stdout.split("\n") if line]
```

Verify from a heredoc, which is the failing shape:

```bash
cd /Users/max/Documents/projects/beauty-crm && python3 - <<'EOF'
import sys; sys.path.insert(0, "/Users/max/Documents/projects/repograph/bench/compare")
import truth as T; from pathlib import Path
print(len(T.code_files_naming(Path(".").resolve(), "AuthService")))
EOF
```
Expected: `10` (was `0` before the change).

- [ ] **Step 7: Write the definition down**

In `docs/bench/three-graphs.md`, under "Reading a result file", add:

```markdown
Every retrieval row since 2026-09-04 also carries `rank`: 1 + the distinct competing ids (for
a file case, paths) that appear in the answer before the expected one, or `null` when it is
absent. The summary's `mrr` is the mean of `1/rank` over the suite with absences as 0. Strict
says whether the answer is there; MRR says how far down. The 2026-09-03 file predates the field.
```

and replace the caveat "The strict criterion is a substring: an id fifth of five counts the same as first." with "The strict criterion is a substring; `rank` and `mrr` are what say where in the answer it sat."

- [ ] **Step 8: Commit**

```bash
git add bench/compare/run.py bench/compare/report.py bench/compare/truth.py bench/compare/test_run.py docs/bench/three-graphs.md
git commit -m "test(bench): the scorer records rank, and rg reads the tree not stdin"
```

---

### Task 2: `changes` names the file it could not parse (G1, the language-independent half)

**Files:**
- Modify: `src/changes.rs:36-51` (`touched`), `src/changes.rs:96-118` (`span_of`, `render`), `src/changes.rs:120-126` (`render_json`)
- Test: `src/changes.rs` tests module — replace `a_hunk_in_an_unknown_file_is_ignored`
- Modify: `README.md:335-341` ("`changes` maps …" paragraph)

**Interfaces:**
- Produces: `touched` returns `file:<rel>` for a hunk in a file the graph has no node for; `render` header counts symbols and files separately; JSON `touched[].indexed: bool`.

- [ ] **Step 1: Turn the test that pins the wrong behaviour into two that pin the right one**

In `src/changes.rs` tests, delete `a_hunk_in_an_unknown_file_is_ignored` and add:

```rust
    #[test]
    fn a_hunk_in_a_file_the_graph_never_indexed_is_reported_as_that_file() {
        assert_eq!(touched(&graph(), &[Hunk { file: "Foo.kt".into(), start: 1, end: 9 }]), vec!["file:Foo.kt"]);
    }

    #[test]
    fn an_unindexed_file_renders_as_changed_with_no_span_and_reaches_nothing() {
        let g = graph();
        let r = report(&g, &[Hunk { file: "Foo.kt".into(), start: 1, end: 9 }], 2);
        assert_eq!(r.touched, vec!["file:Foo.kt"]);
        assert!(r.affected.is_empty());
        assert_eq!(
            render(&g, &r),
            "changed: 0 symbols in 1 file\n  file:Foo.kt  not indexed\naffected (depth 2): 0 symbols in 0 files\nrisk: LOW — 0 direct, 0 total, 0 files\n"
        );
        let v: serde_json::Value = serde_json::from_str(&render_json(&g, &r)).unwrap();
        assert_eq!(v["touched"][0]["indexed"], false);
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test changes:: 2>&1 | tail -20`
Expected: `a_hunk_in_a_file_the_graph_never_indexed_is_reported_as_that_file` fails with `left: [], right: ["file:Foo.kt"]`; the render test fails the same way.

- [ ] **Step 3: Report the file, count symbols and files apart**

In `touched`, replace the two lines after the inner `for`:

```rust
        // A file the graph never indexed — a `.kt`, a `.sql`, a lockfile — is still a file the
        // diff changed. Reported as its file id, so the answer says "this changed, I cannot say
        // which symbol" instead of saying nothing: on the bench corpus the silence was a third of
        // a large diff (2026-09-03, 11 of 18 files named). A deleted file never gets here — its
        // `+++ /dev/null` yields no hunk — so every id emitted is a file on the new side.
        if !any { out.insert(format!("file:{}", h.file)); }
```

(the old line was `if !any && graph.nodes.contains_key(&file_id) { … }`; `file_id` is now unused — remove its `let`.)

Replace `span_of` and the first three lines of `render`:

```rust
/// A touched id's file: the node's for a symbol or an indexed file, the id itself for a file
/// the graph does not hold.
fn file_of_touched(graph: &Graph, id: &str) -> String {
    match id.strip_prefix("file:") {
        Some(rel) => rel.to_string(),
        None => graph.nodes.get(id).map(|n| n.file.clone()).unwrap_or_default(),
    }
}

fn span_of(graph: &Graph, id: &str) -> String {
    match graph.nodes.get(id) {
        Some(n) if n.end > n.line => format!("{}:{}-{}", n.file, n.line, n.end),
        Some(n) => format!("{}:{}", n.file, n.line),
        None if id.starts_with("file:") => "not indexed".into(),
        None => "?".into(),
    }
}

pub fn render(graph: &Graph, r: &Report) -> String {
    if r.touched.is_empty() { return "changed: 0 symbols\n".into() }
    let symbols = r.touched.iter().filter(|id| !id.starts_with("file:")).count();
    let changed_files: BTreeSet<String> = r.touched.iter().map(|id| file_of_touched(graph, id)).collect();
    let mut out = format!("changed: {} in {}\n", plural(symbols, "symbol"), plural(changed_files.len(), "file"));
```

In `render_json`, the `touched` entries gain a flag:

```rust
    let touched: Vec<serde_json::Value> = r.touched.iter()
        .map(|id| serde_json::json!({ "id": id, "at": span_of(graph, id), "indexed": graph.nodes.contains_key(id) })).collect();
```

- [ ] **Step 4: Run the whole crate's tests**

Run: `cargo test 2>&1 | tail -5`
Expected: `test result: ok` — including `render_says_what_changed_and_what_it_reaches` unchanged (`1 symbol in 1 file`) and `a_hunk_outside_every_symbol_falls_to_the_file` unchanged.

- [ ] **Step 5: Measure the large diff**

```bash
cargo build --release
cd /Users/max/Documents/projects/beauty-crm && git rev-parse --short HEAD
cd /Users/max/Documents/projects/repograph/bench/compare
python3 run.py --repo /Users/max/Documents/projects/beauty-crm \
  --repograph ../../target/release/repograph --tools repograph --suites blast \
  --truth /tmp/truth-$(date +%F).json --out ../results/$(date +%F)-beauty-crm-task2.json
python3 - <<'EOF'
import json, glob
r = json.load(open(sorted(glob.glob("../results/*-task2.json"))[-1]))
for row in r["tools"]["repograph"]["rows"]:
    if row["kind"] == "changes":
        print(row["base"], f"symbols {row['found_symbols']}/{row['want_symbols']}", f"files {row['found_files']}/{row['want_files']}")
EOF
```

Expected: files on the `0f27d2d1` case ≥ the 2026-09-03 value of 11/18 with the unparsed files now named; symbols unchanged at 27/38 — the symbol half is the Kotlin extractor's, 0.6.0. If files land below 18/20 in total, bucket the still-missing files by extension (`git diff --name-only 0f27d2d1 -- . | grep -vE '\.(ts|tsx)$'`) and write the buckets into the results note; the gate in the spec is ≥18/20 and a shortfall is a finding, not a failure to record.

- [ ] **Step 6: Say it in the README**

In `README.md`, the `changes` paragraph of "Blast radius", after "…walks every symbol the file declares;":

```markdown
a hunk in a file the graph does not index at all — a `.kt`, a `.sql`, a lockfile — is listed
as that file with `not indexed` in place of a span, so the answer says the file changed rather
than nothing; what is being changed is never listed as affected by itself.
```

- [ ] **Step 7: Commit**

```bash
git add src/changes.rs README.md bench/results/
git commit -m "feat(changes): a hunk in a file the graph never indexed is reported, not dropped"
```

---

### Task 3: A name inside a string is not a reference (G3, two of three)

**Files:**
- Modify: `bench/compare/truth.py:40-52` (`BLOCK_COMMENT`, `LINE_COMMENT`, `strip_comments`)
- Create: `bench/compare/test_truth.py`
- Modify: `docs/bench/three-graphs.md` — "How the answer is judged" impact row

**Interfaces:**
- Produces: `truth.strip_comments(src) -> str` also blanks the bodies of `'…'`, `"…"` and `` `…` `` literals, quotes and line count preserved.

Measured on `2483d932` (the plan's commit): `impact TenantContextInterceptor` scores 3/4 because `apps/api/src/shared/db/database.service.ts:34` mentions it inside an error message; `impact OutboxPublisher` 6/7 because `apps/api/test/loggingModule.spec.ts:11` is `'PinoLogger:OutboxPublisher'`. Neither file depends on the class; the truth said they did.

- [ ] **Step 1: Write the failing tests**

`bench/compare/test_truth.py`:

```python
"""What counts as a reference, on the lines that were miscounted."""

import unittest

from truth import strip_comments


class StripComments(unittest.TestCase):
    def test_a_name_inside_a_single_quoted_token_is_not_a_reference(self):
        # apps/api/test/loggingModule.spec.ts:11 — a logger's name, not a dependency.
        src = "const OUTBOX_LOGGER_TOKEN = 'PinoLogger:OutboxPublisher';\n"
        self.assertNotIn("OutboxPublisher", strip_comments(src))

    def test_a_name_inside_an_error_message_is_not_a_reference(self):
        # apps/api/src/shared/db/database.service.ts:34 — prose about the interceptor.
        src = 'throw new Error("a route with :businessId behind TenantContextInterceptor");\n'
        self.assertNotIn("TenantContextInterceptor", strip_comments(src))

    def test_a_template_literal_is_blanked_too(self):
        self.assertNotIn("AuthService", strip_comments("log(`${x} AuthService`);\n"))

    def test_code_outside_strings_and_comments_survives_with_its_line_count(self):
        src = (
            "import { AuthService } from './auth.service.js'; // AuthService\n"
            "/* AuthService */\n"
            "new AuthService('x');\n"
        )
        out = strip_comments(src)
        self.assertEqual(out.count("\n"), src.count("\n"))
        self.assertEqual(out.count("AuthService"), 2)

    def test_a_url_in_a_string_is_not_a_line_comment(self):
        self.assertIn("fetch(", strip_comments("fetch('http://x/y');\n"))


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run them to see the first three fail**

Run: `cd bench/compare && python3 -m unittest test_truth -v`
Expected: the three "not a reference" tests fail with `'OutboxPublisher' unexpectedly found …`; the last two pass already.

- [ ] **Step 3: Blank the literals**

In `bench/compare/truth.py`, replace the `BLOCK_COMMENT` … `strip_comments` block:

```python
BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.S)
LINE_COMMENT = re.compile(r"(?<!:)//[^\n]*")
# Single- and double-quoted literals stay on one line; a template literal may not.
STRING_LITERAL = re.compile(r"'(?:[^'\\\n]|\\.)*'|\"(?:[^\"\\\n]|\\.)*\"|`(?:[^`\\]|\\.)*`", re.S)


def strip_comments(src: str) -> str:
    """Prose is not a reference, and neither is a string.

    This corpus writes long docblocks that name the symbols they discuss, so a plain grep
    counts a paragraph about `TenantContextInterceptor` as a file that depends on it. It also
    names symbols in text: `'PinoLogger:OutboxPublisher'` is a logger's name and an error
    message can mention an interceptor. Both counted on 2026-09-03 and put two impact targets
    one file short of 1.0 for a dependency that did not exist. Only code counts. A literal's
    body is blanked and its quotes kept, so line numbers and bracket depth survive; a
    `${…}` inside a template literal is blanked with it, which is the one thing this loses.
    """
    without_comments = LINE_COMMENT.sub("", BLOCK_COMMENT.sub("", src))
    return STRING_LITERAL.sub(lambda m: m.group(0)[0] + m.group(0)[-1], without_comments)
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cd bench/compare && python3 -m unittest test_truth test_run -v`
Expected: `Ran 11 tests … OK`

- [ ] **Step 5: Rebuild the truth and measure**

```bash
cd /Users/max/Documents/projects/repograph/bench/compare
python3 run.py --repo /Users/max/Documents/projects/beauty-crm \
  --repograph ../../target/release/repograph --tools repograph --suites blast \
  --truth /tmp/truth-task3.json --out ../results/$(date +%F)-beauty-crm-task3.json
python3 report.py ../results/$(date +%F)-beauty-crm-task3.json | sed -n '/impact, case by case/,$p'
```

Expected: `TenantContextInterceptor` want 3, recall 1.0; `OutboxPublisher` want 6, recall 1.0; `AuthService` still 0.9 (that one is Task 4); summary `impact, files named` 110/112.

- [ ] **Step 6: Say what the truth counts**

In `docs/bench/three-graphs.md`, the impact row of "How the answer is judged": change "every file naming the symbol, minus the file declaring it" to "every file naming the symbol in code — comments and string literals blanked — minus the file declaring it".

- [ ] **Step 7: Commit**

```bash
git add bench/compare/truth.py bench/compare/test_truth.py docs/bench/three-graphs.md bench/results/
git commit -m "test(bench): a name inside a string literal is not a reference"
```

---

### Task 4: A barrel that re-exports the symbol is an importer (G3, the real one)

**Files:**
- Modify: `src/impact.rs:133-147` (`importers`)
- Test: `src/impact.rs` tests — `upstream_of_a_class_reaches_callers_of_its_members_and_through_barrels`, `render_lists_layers_with_path_line_and_ends_with_the_risk`, one new test
- Modify: `README.md:308-334` ("Blast radius", the importers sentence)

**Interfaces:**
- Produces: `Impact.importers` includes every barrel file `aliases()` finds.

Measured on `2483d932`: `impact AuthService` scores 9/10; the missing file is `apps/api/src/modules/identity/index.ts:15`, `export { AuthService } from './auth.service.js';`. `aliases()` already walks to that barrel — `importers()` uses it only as a *target* to match `Imports` edges against and never lists the barrel itself.

- [ ] **Step 1: Pin the behaviour in the existing test graph**

The test graph already has `file:index.ts` re-exporting `s.ts`. In `src/impact.rs` tests, change the importers assertion in `upstream_of_a_class_reaches_callers_of_its_members_and_through_barrels`:

```rust
        assert_eq!(imp.importers, vec!["c.ts", "index.ts", "m.ts", "w.ts"]);
```

and in `render_lists_layers_with_path_line_and_ends_with_the_risk`:

```rust
        assert!(out.contains("importers (4): c.ts, index.ts, m.ts, w.ts\n"));
        assert!(out.ends_with("risk: MEDIUM — 3 direct, 4 total, 5 files\n"), "{out}");
```

and add:

```rust
    #[test]
    fn a_barrel_that_re_exports_the_symbol_is_an_importer() {
        // Rename S and `export * from './s'` in index.ts is the first thing that breaks.
        assert!(upstream(&graph(), "sym:s.ts::S", 1).importers.contains(&"index.ts".to_string()));
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test impact:: 2>&1 | grep -E "FAILED|panicked|left|right" | head`
Expected: three failures, `left: ["c.ts", "m.ts", "w.ts"]`.

- [ ] **Step 3: List the barrels**

Replace `importers` in `src/impact.rs`:

```rust
/// Files that name the symbol without necessarily calling it: every importer of its name from
/// its file or any barrel, and the barrels themselves — a barrel that re-exports the symbol
/// names it as surely as an importer does, and `export { AuthService } from './auth.service.js'`
/// breaks before any caller when the class is renamed. It was the one file `impact AuthService`
/// left out on the bench corpus (9 of 10, 2026-09-03).
fn importers(graph: &Graph, root: &str) -> Vec<String> {
    let Some(n) = graph.nodes.get(root) else { return Vec::new() };
    let name = bare(name_of(root));
    let mut files: BTreeSet<String> = BTreeSet::from([format!("file:{}", n.file)]);
    let mut out: BTreeSet<String> = BTreeSet::new();
    for a in aliases(graph, root) {
        if let Some((f, _)) = a.trim_start_matches("sym:").rsplit_once("::") {
            files.insert(format!("file:{f}"));
            out.insert(f.to_string());
        }
    }
    for e in graph.edges.iter().filter(|e| e.kind == EdgeKind::Imports && files.contains(&e.target) && exports(e, name)) {
        out.insert(e.source.trim_start_matches("file:").to_string());
    }
    out.into_iter().collect()
}
```

- [ ] **Step 4: Run the whole crate's tests**

Run: `cargo test 2>&1 | tail -5`
Expected: `test result: ok`. `changes::report_unions_the_callers_of_every_touched_symbol` still expects `files == {"c.ts"}` — its graph has no `ReExports` edge, so nothing changes there.

- [ ] **Step 5: Measure**

```bash
cargo build --release
cd /Users/max/Documents/projects/repograph/bench/compare
python3 run.py --repo /Users/max/Documents/projects/beauty-crm \
  --repograph ../../target/release/repograph --tools repograph --suites blast \
  --truth /tmp/truth-task3.json --out ../results/$(date +%F)-beauty-crm-task4.json
python3 report.py ../results/$(date +%F)-beauty-crm-task4.json | grep -E "impact, (mean|files)|AuthService"
```

Expected: `AuthService` 1.0; `impact, files named` 112/112; `impact, mean recall` 1.0. Trace stays 8/8 (`cargo test` covered `trace`; the run confirms it on the corpus).

- [ ] **Step 6: Say it in the README**

In "Blast radius", after the sentence ending "…a caller that imported through a barrel is found because the barrel's re-export is followed back":

```markdown
The barrel itself is listed among the importers: it names the symbol, and a rename reaches it
first.
```

- [ ] **Step 7: Commit**

```bash
git add src/impact.rs README.md bench/results/
git commit -m "fix(impact): a barrel that re-exports the symbol is an importer of it"
```

---

### Task 5: A blast suite a delta can be judged on (G4)

**Files:**
- Modify: `bench/blast.jsonl` (20 → 32 lines)
- Modify: `bench/compare/run.py:203-215` (argparse), `run.py:218-221` (case loading), `run.py:228-231` (report header)
- Modify: `bench/compare/test_run.py` (shape test)
- Modify: `docs/bench/three-graphs.md` — "What gets measured" table, "Adding a case", new section "When a blast delta counts"

**Interfaces:**
- Produces: `run.py --cases <path> --blast <path>` (defaults unchanged); result header carries `"case_files": {"retrieval": …, "blast": …}`.

Chosen from the corpus at `2483d932` — the narrow targets by counting referencing code files with strings and comments blanked (each has two or three), the `changes` bases by reading `git log` for commits whose own diff mixes TypeScript and Kotlin or is TypeScript-only at size:

- [ ] **Step 1: Write the shape test**

Append to `bench/compare/test_run.py`:

```python
import collections
from pathlib import Path

import truth as T

BENCH = Path(__file__).resolve().parent.parent


class BlastShape(unittest.TestCase):
    def test_blast_has_the_recorded_shape(self):
        rows = T.read_jsonl(BENCH / "blast.jsonl")
        self.assertEqual(collections.Counter(r["kind"] for r in rows), {"impact": 16, "trace": 8, "changes": 8})
        tiers = collections.Counter(r["tier"] for r in rows if r["kind"] == "impact")
        self.assertEqual(tiers, {"hub": 3, "wide": 3, "narrow": 10})
        self.assertEqual(len({r["target"] for r in rows if r["kind"] == "impact"}), 16)
```

Run: `cd bench/compare && python3 -m unittest test_run.BlastShape -v`
Expected: fails with `Counter({'impact': 10, 'trace': 8, 'changes': 2}) != …`.

- [ ] **Step 2: Add the twelve cases**

Append to `bench/blast.jsonl`, after the existing `SyncConflictService` line (keep the file grouped: impact, then trace, then changes — move the two existing `changes` lines to the end if needed):

```json
{"kind":"impact","target":"RateLimitInterceptor","file":"apps/api/src/modules/identity/rate-limit.interceptor.ts","tier":"narrow"}
{"kind":"impact","target":"PlatformSessionGuard","file":"apps/api/src/modules/identity/platform-session.guard.ts","tier":"narrow"}
{"kind":"impact","target":"ChangeLogCompactionService","file":"apps/api/src/modules/sync/change-log-compaction.service.ts","tier":"narrow"}
{"kind":"impact","target":"ShutdownService","file":"apps/api/src/shared/lifecycle/shutdown.service.ts","tier":"narrow"}
{"kind":"impact","target":"RbacInterceptor","file":"apps/api/src/shared/rbac/rbac.interceptor.ts","tier":"narrow"}
{"kind":"impact","target":"SystemFacade","file":"apps/api/src/modules/system/system.facade.ts","tier":"narrow"}
```

and after the existing two `changes` lines:

```json
{"kind":"changes","base":"bc9db289~1","note":"polyglot, small when recorded: the commit itself is 3 ts + 2 kt"}
{"kind":"changes","base":"a7acc0f4~1","note":"polyglot: the commit is 8 ts + 2 kt"}
{"kind":"changes","base":"3441991c~1","note":"polyglot: the commit is 9 ts + 7 kt"}
{"kind":"changes","base":"d9521668~1","note":"TypeScript only: 12 files in packages/ui"}
{"kind":"changes","base":"68635fae~1","note":"TypeScript only: 21 files in apps/api sync"}
{"kind":"changes","base":"5c61a72c~1","note":"a merge of 1 628 files, 25 ts + 37 kt among them"}
```

Run: `cd bench/compare && python3 -m unittest test_run -v`
Expected: `OK`.

- [ ] **Step 3: Let a run name its case files**

In `run.py` `main()`, after `ap.add_argument("--repo", …)`:

```python
    ap.add_argument("--cases", default="", help="retrieval cases; default bench/cases.jsonl")
    ap.add_argument("--blast", default="", help="blast cases; default bench/blast.jsonl")
```

replace the two `T.read_jsonl(bench / …)` lines:

```python
    cases_path = Path(args.cases) if args.cases else bench / "cases.jsonl"
    blast_path = Path(args.blast) if args.blast else bench / "blast.jsonl"
    cases = T.read_jsonl(cases_path)
    blast = T.read_jsonl(blast_path)
```

and in the `report` dict, after `"cases": {…}`:

```python
        "case_files": {"retrieval": str(cases_path), "blast": str(blast_path)},
```

- [ ] **Step 4: Measure the 32**

```bash
cd /Users/max/Documents/projects/repograph/bench/compare
python3 run.py --repo /Users/max/Documents/projects/beauty-crm \
  --repograph ../../target/release/repograph --tools repograph --suites blast \
  --truth /tmp/truth-task5.json --out ../results/$(date +%F)-beauty-crm-task5.json
python3 report.py ../results/$(date +%F)-beauty-crm-task5.json
```

Expected: `cases: 82 retrieval, 32 blast`; the impact table has 16 rows; `changes, symbols named` and `files named` are sums over 8 cases. Write the per-case `changes` numbers into the results note — they are the baseline every later delta is read against.

- [ ] **Step 5: State the rule**

In `docs/bench/three-graphs.md`: change the "What gets measured" blast table to `impact 16 / trace 8 / changes 8` with "six narrow targets added 2026-09-04, each with two or three referencing files, because a mean over hubs hides a service with two callers"; under "Adding a case" add "A `changes` case is everything since its base, so it grows as HEAD moves; the `note` records the mix the commit itself had when the case was chosen."; and add a section:

```markdown
## When a blast delta counts

One run per row, no repeats, so a rule is stated before a change is measured, not after:

| suite | a change counts when | noise |
|---|---|---|
| impact | mean recall over 16 does not fall, and no case falls by more than one file | one file on one case |
| trace | 8/8 stays 8/8 | none — a chain either resolves or it does not |
| changes | `symbols_found / symbols_want` over 8 cases rises by ≥ 0.05 and `files_found / files_want` does not fall | ±1 symbol on one case |

Anything inside the noise column is reported and not argued from.
```

- [ ] **Step 6: Commit**

```bash
git add bench/blast.jsonl bench/compare/run.py bench/compare/test_run.py docs/bench/three-graphs.md bench/results/
git commit -m "test(bench): six narrow impact targets, six more diffs, and a rule for reading a delta"
```

---

### Task 6: The one paraphrase lever never measured — a local cross-encoder (G2)

**Files:**
- Create: `src/index/cross.rs`
- Modify: `src/index/mod.rs` (add `pub mod cross;`)
- Modify: `src/config.rs:7-18` (`Config`), `:26-45` (`Default`)
- Modify: `src/main.rs:38-50` (`Cmd::Ask`), `:96` (`Cmd::Bench`), `:392-445` (Ask arm), `:498-499` (Bench arm)
- Modify: `src/bench.rs:56` (`run` signature), `:98-99` (rerank closure)
- Modify: `repograph.toml` (one key), `README.md` "Configure" and "Spending tokens on purpose"
- Modify: `docs/adr/ADR-001-paraphrase-recall-was-a-prediction.md` (Amendment 4)

**Interfaces:**
- Produces: `index::cross::CrossEncoder::open(dir: &Path) -> Result<CrossEncoder>`, `CrossEncoder::score(&mut self, question: &str, texts: &[String]) -> Result<Vec<f32>>`, `index::cross::pick(scores: &[f32], candidates: &[(String, String)], k: usize) -> Vec<String>`, `index::cross::default_dir() -> Result<PathBuf>`; `Config.reranker_dir: String`; `ask --rerank-local`, `bench --rerank-local`; `bench::run(repo, cases, no_dense, rerank, rerank_local, depth)`.
- Consumes: `query::Rerank` (`&dyn Fn(&str, &[(String, String)]) -> Vec<String>`), `rerank::text` for candidate snippets, `rerank::DEPTH`.

The deliverable is a **number**, not a floor. ADR-001 names `bge-reranker-v2-m3` as the one lever unmeasured; everything cheaper is measured and rejected. Adoption is a separate decision, made in Step 9 by the rule written before the run.

- [ ] **Step 1: Export the model once**

The weights repo `BAAI/bge-reranker-v2-m3` carries no ONNX file (checked 2026-09-04: `config.json`, `tokenizer.json`, `tokenizer_config.json` only). Export locally; the venv is throwaway.

```bash
python3 -m venv /tmp/optimum && /tmp/optimum/bin/pip install -q "optimum[exporters]" onnx onnxruntime
mkdir -p ~/.cache/repograph/reranker
/tmp/optimum/bin/optimum-cli export onnx --model BAAI/bge-reranker-v2-m3 --task text-classification ~/.cache/repograph/reranker
ls -la ~/.cache/repograph/reranker
```

Expected: `model.onnx` (about 2.2 GB, fp32), `tokenizer.json`, `config.json`. Record the size and the export time in the ADR amendment.

- [ ] **Step 2: Write the pure test and the model test**

`src/index/cross.rs`:

```rust
//! A cross-encoder over the fused candidate list: the question and each candidate's text go
//! through one model together and come out as a relevance score. `ask --rerank` sends that
//! pool to a model command (≈19k input tokens and ~4 s a question, 14/14 on the old paraphrase
//! set); this runs a local model at zero tokens. Opt-in, off every floor until it is measured
//! against them, and loaded through the same `ort` + `tokenizers` path as the embedder.
use anyhow::{anyhow, Context, Result};
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use std::path::{Path, PathBuf};
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

/// Question plus a 120-character snippet is well under this; the cap is the model's.
const MAX_TOKENS: usize = 512;
const BATCH: usize = 16;
/// How many ids the reranker hands back, best first — `ask` shows five seeds.
pub const PICK: usize = 5;

pub struct CrossEncoder { session: Session, tokenizer: Tokenizer, wants_type_ids: bool }

/// Where `optimum-cli export onnx` was told to write: `$HOME/.cache/repograph/reranker`, beside
/// the embedder's own cache, unless the config names another directory.
pub fn default_dir() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".cache").join("repograph").join("reranker"))
}

impl CrossEncoder {
    /// `dir` holds `model.onnx` and `tokenizer.json` as the exporter writes them.
    pub fn open(dir: &Path) -> Result<CrossEncoder> {
        let mut tokenizer = Tokenizer::from_file(dir.join("tokenizer.json"))
            .map_err(|e| anyhow!("{e}")).with_context(|| format!("open reranker tokenizer in {}", dir.display()))?;
        tokenizer.with_truncation(Some(TruncationParams { max_length: MAX_TOKENS, ..Default::default() })).map_err(|e| anyhow!("{e}"))?;
        tokenizer.with_padding(Some(PaddingParams { strategy: PaddingStrategy::BatchLongest, ..Default::default() }));
        let session = Session::builder().map_err(|e| anyhow!("{e}"))?
            .with_optimization_level(GraphOptimizationLevel::Level1).map_err(|e| anyhow!("{e}"))?
            .commit_from_file(dir.join("model.onnx")).map_err(|e| anyhow!("{e}"))
            .with_context(|| format!("open reranker model in {}", dir.display()))?;
        let wants_type_ids = session.inputs().iter().any(|i| i.name() == "token_type_ids");
        Ok(CrossEncoder { session, tokenizer, wants_type_ids })
    }

    /// One score per text, higher is closer: the model's single logit, unnormalised, because
    /// only the order is used.
    pub fn score(&mut self, question: &str, texts: &[String]) -> Result<Vec<f32>> {
        let mut out = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(BATCH) {
            let pairs: Vec<(String, String)> = chunk.iter().map(|t| (question.to_string(), t.clone())).collect();
            let encodings = self.tokenizer.encode_batch(pairs, true).map_err(|e| anyhow!("{e}"))?;
            let batch = encodings.len();
            let len = encodings.first().map_or(0, |e| e.len());
            let mut ids = Vec::with_capacity(batch * len);
            let mut mask = Vec::with_capacity(batch * len);
            let mut types = Vec::with_capacity(batch * len);
            for e in &encodings {
                ids.extend(e.get_ids().iter().map(|&x| x as i64));
                mask.extend(e.get_attention_mask().iter().map(|&x| x as i64));
                types.extend(e.get_type_ids().iter().map(|&x| x as i64));
            }
            let mut inputs = ort::inputs![
                "input_ids" => Tensor::from_array(([batch, len], ids))?,
                "attention_mask" => Tensor::from_array(([batch, len], mask))?,
            ];
            if self.wants_type_ids {
                inputs.push(("token_type_ids".into(), Tensor::from_array(([batch, len], types))?.into()));
            }
            let outputs = self.session.run(inputs)?;
            let logits = outputs.get("logits")
                .or_else(|| (outputs.len() == 1).then(|| &outputs[0]))
                .context("reranker has no logits output")?;
            // [batch, 1]: one logit per (question, text) pair.
            let (_, data) = logits.try_extract_tensor::<f32>()?;
            out.extend(data.iter().copied());
        }
        Ok(out)
    }
}

/// The ids of the `k` best-scoring candidates, best first; a tie keeps the fused order.
pub fn pick(scores: &[f32], candidates: &[(String, String)], k: usize) -> Vec<String> {
    let mut order: Vec<usize> = (0..candidates.len().min(scores.len())).collect();
    order.sort_by(|&a, &b| scores[b].partial_cmp(&scores[a]).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b)));
    order.into_iter().take(k).map(|i| candidates[i].0.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cands() -> Vec<(String, String)> {
        vec![("a".to_string(), String::new()), ("b".to_string(), String::new()), ("c".to_string(), String::new())]
    }

    #[test]
    fn pick_orders_by_score_and_breaks_a_tie_by_fused_order() {
        assert_eq!(pick(&[0.1, 0.9, 0.9], &cands(), 5), vec!["b", "c", "a"]);
        assert_eq!(pick(&[0.1, 0.9, 0.9], &cands(), 1), vec!["b"]);
    }

    #[test]
    fn pick_survives_fewer_scores_than_candidates_and_none_at_all() {
        assert_eq!(pick(&[0.5], &cands(), 5), vec!["a"]);
        assert!(pick(&[], &cands(), 5).is_empty());
    }

    #[test]
    fn a_nan_score_sorts_as_a_tie_not_a_panic() {
        assert_eq!(pick(&[f32::NAN, 1.0, 0.0], &cands(), 3).len(), 3);
    }

    #[test]
    #[ignore = "needs the exported model: set REPOGRAPH_RERANKER_DIR"]
    fn the_model_prefers_the_passage_that_answers_the_question() {
        let dir = std::env::var("REPOGRAPH_RERANKER_DIR").expect("REPOGRAPH_RERANKER_DIR");
        let mut m = CrossEncoder::open(Path::new(&dir)).unwrap();
        let s = m.score("how is a client's phone number stored", &[
            "Phone numbers are normalised to E.164 before they are stored.".into(),
            "The calendar grid uses a five-minute lattice.".into(),
        ]).unwrap();
        assert!(s[0] > s[1], "{s:?}");
    }
}
```

Add `pub mod cross;` to `src/index/mod.rs`.

- [ ] **Step 3: Run the tests**

Run: `cargo test cross:: 2>&1 | tail -8`
Expected: 3 passed, 1 ignored. Then the model test:

Run: `REPOGRAPH_RERANKER_DIR=$HOME/.cache/repograph/reranker cargo test cross:: -- --ignored 2>&1 | tail -5`
Expected: `1 passed`. If it fails on `logits`, print `session.outputs()` names in the error and fix the lookup — the exporter's `text-classification` head is named `logits`; this step is where that assumption is checked.

- [ ] **Step 4: Config key**

In `src/config.rs`, `Config` gains, after `rerank_command`:

```rust
    /// Directory holding `model.onnx` and `tokenizer.json` for `ask --rerank-local`; empty
    /// means `$HOME/.cache/repograph/reranker`.
    pub reranker_dir: String,
```

and `Default` gains `reranker_dir: String::new(),`. In `repograph.toml`, after `rerank_command`:

```toml
# `ask --rerank-local`: a local cross-encoder over the same 200-deep pool, zero tokens. Empty
# means ~/.cache/repograph/reranker, where `optimum-cli export onnx` was pointed.
reranker_dir = ""
```

Run: `cargo test config:: 2>&1 | tail -3`
Expected: `ok` (the `deny_unknown_fields` round-trip tests still pass; `repograph.toml` parses).

- [ ] **Step 5: Wire the flag**

`src/main.rs`, in `Cmd::Ask` after `rerank: bool`:

```rust
        /// Picks the seeds with a local cross-encoder instead of the model command — the same
        /// pool, zero tokens. Needs the exported model in `reranker_dir`.
        #[arg(long)] rerank_local: bool,
```

`Cmd::Bench` gains `#[arg(long)] rerank_local: bool` beside `rerank`. In the Ask arm, the pattern becomes `Cmd::Ask { words, json, seeds, bodies, rerank, rerank_local, depth, stale }` and the two rerank lines (`main.rs:443-444`) become:

```rust
            let rerank_fn = |q: &str, c: &[(String, String)]| rerank::run(&cfg.rerank_command, q, c);
            let cross = std::cell::RefCell::new(if rerank_local {
                let dir = if cfg.reranker_dir.is_empty() { index::cross::default_dir()? } else { std::path::PathBuf::from(&cfg.reranker_dir) };
                Some(index::cross::CrossEncoder::open(&dir).context("--rerank-local")?)
            } else { None });
            let local_fn = |q: &str, c: &[(String, String)]| -> Vec<String> {
                let mut m = cross.borrow_mut();
                let Some(m) = m.as_mut() else { return Vec::new() };
                let texts: Vec<String> = c.iter().map(|(_, t)| t.clone()).collect();
                match m.score(q, &texts) {
                    Ok(s) => index::cross::pick(&s, c, index::cross::PICK),
                    // Like a failing rerank command: say so and answer from the fused order.
                    Err(e) => { eprintln!("rerank-local: {e:#}; answering from the fused order"); Vec::new() }
                }
            };
            let rerank: Option<query::Rerank> = if rerank_local { Some(&local_fn) } else if rerank { Some(&rerank_fn) } else { None };
```

(`main.rs` spells anyhow paths out — `anyhow::Result` — and imports no trait, so `.context(…)` needs `use anyhow::Context;` beside `use clap::{Parser, Subcommand};` at `main.rs:18`; `index` is a module of `main.rs`, so `index::cross` resolves there and `crate::index::cross` in `bench.rs`.) `query::ask` already switches the pool depth on `rerank.is_some()`, so `--rerank-local` sees the same 200-deep list `--rerank` does.

Bench arm: `Cmd::Bench { cases, rerank, rerank_local, depth } => … bench::run(&repo, cases.as_deref(), cli.no_dense, rerank, rerank_local, depth)`. In `src/bench.rs`, `run` takes `rerank_local: bool` after `rerank`, and lines 98–99 become the same `cross`/`local_fn`/`rerank` triple as above (copy it with `crate::index::cross` paths; `cfg` and `anyhow::Context` are already in scope there). The summary line's suffix gains ` rerank_local=true depth={depth}` when `rerank_local`.

Run: `cargo build --release 2>&1 | tail -3 && cargo test 2>&1 | tail -3`
Expected: builds; `test result: ok`.

- [ ] **Step 6: One question by hand**

```bash
cd /Users/max/Documents/projects/beauty-crm
B=/Users/max/Documents/projects/repograph/target/release/repograph
time $B --repo . ask --rerank-local "как отдаём освободившееся окно тому, кто ждёт"
$B --repo . ask "как отдаём освободившееся окно тому, кто ждёт"
```

Expected: both print five seeds; the first line of the output is the answer to read. Note the wall time — `real` minus the ~1 s the embedder costs is the reranker's share at depth 200. (`FR-CAL-101` is one of the two soft misses; if neither run names it, that is the coverage finding G2 predicts, not a reranker failure.)

- [ ] **Step 7: Measure the three arms**

Write the rule first, in the ADR amendment (Step 9): **the zero-token floor moves only if `--rerank-local` reads paraphrase ≥ 17/30 with keyword 40/40, code 12/12, p90 ≤ 230 and a median under one second per question.** 17 beats the 16/30 the 2026-09-03 store measured by one; the rule is written before the number exists.

```bash
cd /Users/max/Documents/projects/beauty-crm
B=/Users/max/Documents/projects/repograph/target/release/repograph
$B --repo . bench                                    | tail -1     # the control, this store
time $B --repo . bench --rerank-local                | tail -1     # depth 200
time $B --repo . bench --rerank-local --depth 100    | tail -1
time $B --repo . bench --rerank-local --depth 40     | tail -1
```

Record all four lines and the three `real` times (÷ 82 for a per-question median). ADR-001 says six of the old eight misses sat at dense rank 32–81, so 100 is the smallest pool worth reading; 40 is there to show the latency curve.

- [ ] **Step 8: Document the flag**

`README.md` "Configure": add `reranker_dir` to the key table with "directory of the exported cross-encoder for `--rerank-local`; empty = `~/.cache/repograph/reranker`". "Spending tokens on purpose": add a paragraph:

```markdown
**`--rerank-local`** is the same pool and the same pick, scored by a local cross-encoder
(`BAAI/bge-reranker-v2-m3`, exported to ONNX once with `optimum-cli export onnx --model
BAAI/bge-reranker-v2-m3 --task text-classification ~/.cache/repograph/reranker`, ~2.2 GB) at
zero tokens. Measured on the bench corpus on 2026-09-04 — see ADR-001, Amendment 4 — and not on
any floor.
```

- [ ] **Step 9: The ADR amendment, with the numbers**

Append to `docs/adr/ADR-001-paraphrase-recall-was-a-prediction.md`:

```markdown
## Amendment 4 — the cross-encoder, measured (2026-09-04)

The one lever the second amendment left unmeasured. `bge-reranker-v2-m3`, exported to ONNX
(`optimum-cli`, <size> GB, <export time>), loaded through the embedder's own `ort` path, scoring
the same 200-deep pool `--rerank` shows the model command. Rule, written before the run: the
zero-token floor moves only on paraphrase ≥ 17/30 with keyword 40/40, code 12/12, p90 ≤ 230 and
a median under one second a question.

| arm | keyword | paraphrase | code | p90 | s / question |
|---|---|---|---|---|---|
| control (this store, dense) | 40/40 | <n>/30 | 12/12 | <n> | <n> |
| `--rerank-local`, depth 200 | | | | | |
| `--rerank-local --depth 100` | | | | | |
| `--rerank-local --depth 40` | | | | | |

<Decision: adopted as a floor at depth <d> / stays opt-in because <which condition failed>.
One paragraph on which paraphrases moved, in either direction, against the fourteen misses in
`docs/bench/2026-09-03-three-graphs-results.md`.>
```

Fill every `<…>` from Step 7's output. If the rule is met, that is a second change with its own commit (floor in `bench::passes`, the README's Bench list) and is *not* part of this task; say so in the paragraph.

- [ ] **Step 10: Commit**

```bash
git add src/index/cross.rs src/index/mod.rs src/config.rs src/main.rs src/bench.rs repograph.toml README.md docs/adr/ADR-001-paraphrase-recall-was-a-prediction.md
git commit -m "feat(ask): --rerank-local, a cross-encoder over the same pool at zero tokens, measured"
```

---

### Task 7: A case file for the corpus 0.6.0 will parse (G6)

**Files:**
- Create: `bench/corpora/beauty-crm-mobile/blast.jsonl`
- Modify: `bench/compare/test_run.py` (shape test)
- Modify: `docs/bench/three-graphs.md` — new section "Adding a corpus"
- Create: `bench/results/<date>-beauty-crm-mobile.json` (the baseline)

**Interfaces:**
- Consumes: `run.py --blast` (Task 5).
- Produces: a blast file whose expectations a Kotlin extractor will be judged against, and the number it scores today.

Kotlin is not parsed in this release: every case below scores 0 now, and that is the point — the file exists before the language ships, so 0.6.0 is measured rather than predicted. Retrieval cases are left out on purpose: `beauty-crm/mobile` shares `beauty-crm`'s PRD and id families, so the retrieval half is already the main set. `trace` and `changes` cases wait for 0.6.0 too: `truth.di_call_graph` reads TypeScript classes and `TOP_LEVEL` knows `class` but not `fun` or `object`, so a Kotlin arm in `truth.py` lands with the extractor, not before it.

Chosen by counting referencing `.kt` files at `2483d932`: three hubs (10–18 files), two wide (6–8), three narrow (1 each).

- [ ] **Step 1: The shape test**

Append to `bench/compare/test_run.py`'s `BlastShape`:

```python
    def test_mobile_baseline_has_eight_kotlin_impact_cases(self):
        rows = T.read_jsonl(BENCH / "corpora" / "beauty-crm-mobile" / "blast.jsonl")
        self.assertEqual(collections.Counter(r["kind"] for r in rows), {"impact": 8})
        self.assertTrue(all(r["file"].endswith(".kt") for r in rows))
        self.assertEqual(collections.Counter(r["tier"] for r in rows), {"hub": 3, "wide": 2, "narrow": 3})
```

Run: `cd bench/compare && python3 -m unittest test_run.BlastShape -v`
Expected: `FileNotFoundError`.

- [ ] **Step 2: The cases**

`bench/corpora/beauty-crm-mobile/blast.jsonl`:

```json
{"kind":"impact","target":"ReplicaKey","file":"mobile/shared/core/src/commonMain/kotlin/pl/placeholder/crm/storage/ReplicaKey.kt","tier":"hub"}
{"kind":"impact","target":"SnapshotVerifier","file":"mobile/shared/core/src/commonMain/kotlin/pl/placeholder/crm/auth/SnapshotVerifier.kt","tier":"hub"}
{"kind":"impact","target":"SigningKeyStore","file":"mobile/shared/core/src/commonMain/kotlin/pl/placeholder/crm/auth/SigningKeyStore.kt","tier":"hub"}
{"kind":"impact","target":"SessionTokens","file":"mobile/shared/core/src/commonMain/kotlin/pl/placeholder/crm/network/Tokens.kt","tier":"wide"}
{"kind":"impact","target":"ReplicaKeyStore","file":"mobile/shared/core/src/commonMain/kotlin/pl/placeholder/crm/storage/ReplicaKeyStore.kt","tier":"wide"}
{"kind":"impact","target":"SignedPermissionSnapshot","file":"mobile/shared/core/src/commonMain/kotlin/pl/placeholder/crm/auth/PermissionSnapshot.kt","tier":"narrow"}
{"kind":"impact","target":"ParallelLimitRule","file":"mobile/shared/domain/src/commonMain/kotlin/pl/placeholder/crm/domain/availability/AvailabilityOccupancyRules.kt","tier":"narrow"}
{"kind":"impact","target":"LocationClosedRule","file":"mobile/shared/domain/src/commonMain/kotlin/pl/placeholder/crm/domain/availability/AvailabilityOccupancyRules.kt","tier":"narrow"}
```

Run: `cd bench/compare && python3 -m unittest test_run -v`
Expected: `OK`.

- [ ] **Step 3: The baseline**

```bash
cd /Users/max/Documents/projects/repograph/bench/compare
python3 run.py --repo /Users/max/Documents/projects/beauty-crm \
  --repograph ../../target/release/repograph --tools repograph --suites blast \
  --blast ../corpora/beauty-crm-mobile/blast.jsonl \
  --truth /tmp/truth-mobile.json --out ../results/$(date +%F)-beauty-crm-mobile.json
python3 report.py ../results/$(date +%F)-beauty-crm-mobile.json | sed -n '/impact, case by case/,$p'
```

Expected: eight rows, `want` between 1 and 18, recall 0.0 on every one — `impact ReplicaKey` prints that it knows no such symbol. That output is the deliverable. If any row is above 0, read it: a `.kt` file naming a TypeScript symbol of the same name would be the truth counting across languages, and the case needs a different target.

- [ ] **Step 4: Say how a corpus is added**

In `docs/bench/three-graphs.md`, after "Adding a case":

```markdown
## Adding a corpus

`bench/corpora/<name>/blast.jsonl` (and `cases.jsonl` where the corpus has its own ids and
prose) are run with `--blast` / `--cases`; the result goes to `bench/results/<date>-<name>.json`.
The first one is `beauty-crm-mobile`: eight Kotlin `impact` targets that score 0 on every
repograph before 0.6.0, written down so that the Kotlin extractor is measured on the day it
lands rather than predicted. No language ships without its file here and a result beside it.
```

- [ ] **Step 5: Commit**

```bash
git add bench/corpora/beauty-crm-mobile/blast.jsonl bench/compare/test_run.py docs/bench/three-graphs.md bench/results/
git commit -m "test(bench): the Kotlin baseline — eight impact cases that read 0 until 0.6.0"
```

---

### Task 8: Re-run, write the result, mark the gaps

**Files:**
- Create: `docs/bench/<date>-repograph-0.5.0-results.md`
- Modify: `docs/bench/next-version-gaps.md` (a status line per gap)
- Modify: `docs/bench/three-graphs.md` (the "three files" table gains the new result)
- Create: `bench/results/<date>-beauty-crm.json` (the run this document reads)

- [ ] **Step 1: One run, both suites, pinned**

```bash
cd /Users/max/Documents/projects/beauty-crm && git status --porcelain && git rev-parse --short HEAD
cd /Users/max/Documents/projects/repograph/bench/compare
python3 run.py --repo /Users/max/Documents/projects/beauty-crm \
  --repograph ../../target/release/repograph --tools repograph \
  --truth /tmp/truth-final.json --out ../results/$(date +%F)-beauty-crm.json
python3 report.py ../results/$(date +%F)-beauty-crm.json
```

Expected: a clean tree and a commit to write down; 82 retrieval rows with `rank`, 32 blast rows; `impact, files named` 112/112; `trace, all` 8/8; `changes` sums over 8 cases; `retrieval, MRR` a number.

- [ ] **Step 2: The write-up**

`docs/bench/<date>-repograph-0.5.0-results.md`, same shape as the 2026-09-03 file, repograph only, with one before/after table:

```markdown
| | 2026-09-03 (0.4.0 @ 7733bd53) | <date> (0.5.0 @ <sha>) | what moved it |
|---|---|---|---|
| retrieval strict | 68/82 | <n>/82 | nothing in this plan; the store's `enrich` and the commit |
| retrieval MRR | — | <n> | Task 1 records it; first measurement |
| impact files | 110/114 | 112/112 | Task 3 (two strings were not references), Task 4 (the barrel) |
| impact cases | 10 | 16 | Task 5 |
| trace | 8/8 | 8/8 | — |
| changes symbols | 32/43 (2 cases) | <n>/<n> (8 cases) | Task 5 grew the set; Kotlin symbols wait for 0.6.0 |
| changes files | 13/20 (2 cases) | <n>/<n> (8 cases) | Task 2 names unindexed files |
| `--rerank-local` paraphrase | not measured | <n>/30 at <s> s | Task 6; floor unchanged unless the ADR says so |
```

Fill from Step 1 and from `bench/results/<date>-beauty-crm-mobile.json`. Two sums with different denominators are not compared as fractions in prose; the sentence names the case count.

- [ ] **Step 3: Mark the gaps**

In `docs/bench/next-version-gaps.md`, under each `## G…` heading, one status line: `**Status (<date>):** closed by Task N — <the number>` for G1 (files half), G3, G4, G5, G7; `**Status (<date>):** measured — <the ADR's decision>` for G2; `**Status (<date>):** baseline written; the extractor is 0.6.0's` for G1's symbol half and G6. Add the new result file to the table at the top of `three-graphs.md`.

- [ ] **Step 4: Commit**

```bash
git add docs/bench/ bench/results/
git commit -m "docs(bench): what 0.5.0 measures against 0.4.0, and the status of every gap"
```

---

## Self-review

**Spec coverage.** G5 → Task 1 (rank, MRR, the substring caveat). G1 → Task 2 (files named; symbol half explicitly deferred to 0.6.0 with the number that will judge it). G3 → Tasks 3 and 4 (the three files read: two strings, one barrel — no exclusion needed). G4 → Task 5 (16/8/8, decision rule in the protocol). G2 → Task 6 (the unmeasured lever, rule before number, ADR amendment). G6 → Task 7 (case file + baseline; retrieval and trace/changes halves deferred with the reason). "Suggested order" G5 → G1 → G3 → G4 → G2 → G6 is the task order. One deviation from the spec's gates, stated where it applies: G5's "2026-09-03 rows rescored" is impossible — those rows hold no answer text — so the field starts now.

**Placeholders.** The `<…>` slots in Task 6 Step 9 and Task 8 Steps 2–3 are the measurement's own output and are filled by the step that produces them; no other step defers content. Every code step is complete code; every measurement step is the command and what it should print.

**Type consistency.** `rank_of(answer: str, want: str) -> int | None` is defined in Task 1 and used only there. `touched` in Task 2 keeps its signature; `file_of_touched` is private. `importers` in Task 4 keeps its signature. `CrossEncoder::open(&Path)`, `score(&mut self, &str, &[String]) -> Result<Vec<f32>>`, `pick(&[f32], &[(String, String)], usize) -> Vec<String>`, `default_dir() -> Result<PathBuf>` are defined in Task 6 Step 2 and used verbatim in Step 5; `bench::run` gains `rerank_local: bool` in the position Step 5 passes it. `run.py --blast` is added in Task 5 and consumed in Task 7.
