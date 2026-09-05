# Weak Spots Close Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close, or measure and record as closed-by-rule, the four weak spots left after the 2026-09-05 levers campaign: code unreachable from prose (`where` 0/9), the `rule` kind at 5/9, paraphrase at 15/30 by default, and a measurement too thin on the code side to design on.

**Architecture:** Measurement first (a synthetic code held-out and a larger developer suite), then the code side (a file-level ranking of the code-questions list and a sixth seat gated on the held-outs, never displacing a document), then the generator side (a `where`-shaped code prompt; a kind-aware document prompt — G14 — on haiku and on sonnet), then the one combination Max's default decision needs (best generator + e5-large). Every lever carries its rule before its number; a lever that fails is recorded, not shipped.

**Tech Stack:** Rust (edition as in `Cargo.toml`, toolchain 1.98, `cargo test --release`, clippy `-D warnings`), Python 3.14 stdlib only under `bench/`, bash 3.2 scripts, `claude -p` as the generation command, `bench/heldout.py` and `dump` for held-out measurement.

**Spec:** `docs/superpowers/specs/2026-09-05-weak-spots-design.md` — the evidence, the simulations, the rules.

## Global Constraints

- Work in a fresh worktree `/Users/max/Documents/projects/repograph/.worktrees/weak-spots` on branch `feat/weak-spots` cut from `main` after PRs #1–#4 are merged. Never `cd` to the main checkout `/Users/max/Documents/projects/repograph`; use absolute paths and `git -C`. The release binary is `target/release/repograph` in that worktree; rebuild after every code change and before any measurement.
- Session scratchpad `S=/private/tmp/claude-502/-Users-max-Documents-projects-repograph/244d7285-7f87-48c4-a4bc-379833f916d7/scratchpad`. Store copies: `$S/bc-a1` (corpus copy at `502e8a6d` with source tree, A1 graph, haiku document questions, haiku code questions — the base for every code-side task), `$S/bc-d1` (same, document questions by sonnet, no code questions), `$S/e5large` (fixture store copy embedded with `intfloat/multilingual-e5-large`; store only, no tree — query it with `--stale` only). New copies are made with `cp -R` from `$S/bc-a1` (code side) or from the fixture's corpus worktree (document side) and named in the task.
- The fixture `/Users/max/bench/beauty-crm-502e8a6d` is never `build`-, `update`-, `enrich`- or `embed`-ed; `bench` and `dump` against it read only. Never run `ask` or `bench` against a store directory without its source tree beside it (the refresh empties the graph); a store-only copy is queried with `--stale`.
- The corpus `/Users/max/Documents/projects/beauty-crm` is read-only for git purposes.
- Rules are written before numbers and not changed after. Three-way rule for retrieval levers: (i) recorded suite (`bench/cases.jsonl`, 82) holds every floor in all four arms with p90 ≤ 230 — enriched `40/14/12` with embeddings, `39/11/12` without; raw `40/9/12`, `39/7/12`; (ii) document held-out (`$S/heldout-400-syn.jsonl`, kind `synthetic`) not significantly worse in either arm (paired exact McNemar, α 0.05, `bench/heldout.py compare`, before = `$S/ho-dense-085-before.json` and `$S/ho-lex-085.json`); (iii) developer suite not down in either arm. Design sets (the two held-outs) and validation sets (recorded, developer) are different sets; no constant is chosen by looking at a validation set.
- Held-out dumps use `$S/heldout-400-syn.jsonl` (documents) and `$S/heldout-code-400.jsonl` (code, created in Task 1). `$S/heldout-400.jsonl` is superseded (kind `paraphrase`, reads a false 0.93).
- Model tokens are allowed on store copies only, with the estimate stated before the run and the byte counts after. Never `enrich` the fixture.
- Commit hook: Conventional Commits subjects; prose bodies with the measured numbers; never `Co-Authored-By`, `Claude-Session` or a `claude.ai` URL; never a heredoc and the commit command in one shell command unless the heredoc's first line is the subject (`git commit -F - <<'MSG'` with the subject first works; python heredoc edits go in a separate command; a file whose text mentions the commit command is written with the file tool, not a heredoc).
- Any `gh` call runs `gh auth switch --user devmaxxx && gh …` in the same shell command.
- Comments say why, never what; no ticket ids in code comments; keep tool directives. Test names are sentences in snake_case, as in the codebase.
- Every number written into a doc, a commit or a ledger comes from a transcript or log under `$S` (named) or a test run; only token costs are estimates, and say so.
- Implementers never dispatch subagents. All subagents run on `opus` (Max, 2026-09-05).

---

## File map

- `bench/heldout.py` — `build --code` samples code nodes; `compare --by-file` judges a hit by the seed's file; `fires` reports a gate sweep over two dumps (new subcommand).
- `bench/heldout.jsonl`-style sets under `$S`: `heldout-code-400.jsonl` (new).
- `bench/dev-cases.jsonl` — v2: 99 cases (60 + 20 `where` + 10 `cross` + 9 `rule`); `bench/history/README.md` notes the case-set change.
- `src/index/lexical.rs` — `LexicalIndex::search_files(&self, query, k, per_file) -> Vec<(String, f32)>` (new): symbol hits aggregated per file, returned as `file:` ids.
- `src/query.rs` — `CODE_SEAT_GATE` (new constant, value from Task 3's sweep), `code_seat(...)` (new), `Options.code_seat: bool` (new, default `true`), the sixth seat appended in `ask` after the five; `lexical_lists` unchanged.
- `src/dump.rs` — `bm25_code_files` field (new) so the sweep has the file-level list; `ask` in the dump carries six seeds when the seat fires.
- `src/main.rs` — `--no-code-seat` flag on `ask`/`bench`/`dump` (new) so the four arms can be measured with and without the seat on one binary.
- `src/bench.rs` — anchors that are files already match a File seed (`hit`); unchanged unless Task 3's measurement shows otherwise.
- `src/enrich.rs` — `prompt_code` v2 (two `where`-shaped questions per node), `prompt` v2 by node kind (ADR/INV/NFR shapes) behind `Scope.prompt_version` (new) so the old prompt stays measurable.
- `src/config.rs` — `enrich_prompt = "v1" | "v2"` (new, default `v1` until a task's rule says otherwise).
- `docs/bench/2026-09-06-weak-spots-results.md` (new), `docs/bench/next-version-gaps.md` (verdicts on G12, G13, G14, new gaps), `README.md`, `docs/bench/runbook.md` (the code held-out).
- `bench/history/runs.jsonl` — rows for both suites at the branch's head.

---

### Task 0: Worktree, branch, copies

**Files:**
- Create: worktree `/Users/max/Documents/projects/repograph/.worktrees/weak-spots`, branch `feat/weak-spots`
- Create: `$S/bc-code` (copy of `$S/bc-a1`, the code-side working copy), `$S/bc-doc` (copy of the fixture corpus worktree, the document-side working copy)

- [ ] **Step 1: Cut the branch from the merged main**

```bash
M=/Users/max/Documents/projects/repograph
git -C $M fetch origin main
git -C $M worktree add $M/.worktrees/weak-spots -b feat/weak-spots origin/main
W=$M/.worktrees/weak-spots
git -C $W log --oneline -1
cargo build --release --manifest-path $W/Cargo.toml
cargo test --release --manifest-path $W/Cargo.toml 2>&1 | grep 'test result'
```

Expected: the head is main's merge commit of PR #4 (or later); `412 passed; 0 failed; 2 ignored`.

- [ ] **Step 2: Copies**

```bash
S=/private/tmp/claude-502/-Users-max-Documents-projects-repograph/244d7285-7f87-48c4-a4bc-379833f916d7/scratchpad
cp -R $S/bc-a1 $S/bc-code
rsync -a --exclude node_modules /Users/max/bench/beauty-crm-502e8a6d/ $S/bc-doc/
ls $S/bc-code/.repograph $S/bc-doc/.repograph
$W/target/release/repograph --repo $S/bc-code bench 2>&1 | tail -1
$W/target/release/repograph --repo $S/bc-doc bench 2>&1 | tail -1
```

Expected: both stores carry `graph.bin graph.json manifest.json questions.bin questions.json vectors.f32 vectors.json`; `bc-code` reads `keyword 40/40 paraphrase 15/30 code 12/12 p90 220` (code questions never enter the plain fusion, so it reads exactly the fixture); `bc-doc` reads the same line. Record both lines in `$S/t0-baselines.txt`.

- [ ] **Step 3: Ledger**

No commit; the SDD ledger records the head, the two copies and the two baseline lines.

---

### Task 1: The code held-out — a design set for the code side

**Files:**
- Modify: `bench/heldout.py` (`cmd_build`: `--code`; `hits`: `--by-file`; new `cmd_fires`)
- Modify: `src/dump.rs` (`bm25_code_files` beside `bm25_code`; needs Task 2's `search_files` — see Interfaces)
- Test: `bench/test_heldout.py` (new; unittest, stdlib)
- Create: `$S/heldout-code-400.jsonl`, dumps `$S/hoc-base-{dense,lex}.json`

**Interfaces:**
- Consumes: `LexicalIndex::search_files(&self, query: &str, k: usize, per_file: usize) -> Vec<(String, f32)>` from Task 2 — so **Task 2 is implemented before this task's dump step**; the Python side of this task does not wait for it.
- Produces: `$S/heldout-code-400.jsonl` (kind `synthetic`, `expect` = the code node's id — `sym:…` or `file:…`); `heldout.py compare --by-file A B --at N`; `heldout.py fires DOC_DUMP CODE_DUMP --list bm25_code_files --gates 0.5,0.7,0.85,1.0,1.2` printing, per γ, the firing rate on the document dump and recall@6 on the code dump.

- [ ] **Step 1: Failing tests for the three Python pieces**

```python
# bench/test_heldout.py
import json, tempfile, unittest
from pathlib import Path
import heldout

class BuildCode(unittest.TestCase):
    def test_code_samples_only_code_nodes_and_keeps_the_node_id_as_expect(self):
        with tempfile.TemporaryDirectory() as d:
            store = Path(d)
            (store / "questions.json").write_text(json.dumps({"entries": {
                "FR-1": {"questions": ["doc q"]},
                "sym:a/b.ts::f": {"questions": ["what does f do", "where is f used"]},
                "file:a/b.ts": {"questions": ["what is in b.ts"]},
            }}))
            out = store / "ho.jsonl"
            heldout.cmd_build(heldout.parse(["build", "--store", str(store), "--size", "2", "--seed", "1", "--code", "--out", str(out)]))
            rows = [json.loads(l) for l in out.read_text().splitlines()]
            self.assertEqual({r["expect"] for r in rows}, {"sym:a/b.ts::f", "file:a/b.ts"})
            self.assertTrue(all(r["kind"] == "synthetic" for r in rows))

class HitsByFile(unittest.TestCase):
    def test_a_seed_in_the_expected_files_file_is_a_hit_by_file_and_not_by_id(self):
        dump = {"queries": [{"q": "x", "expect": "sym:a/b.ts::f", "ask": {"seeds": [["sym:a/b.ts::g", 1.0]]}}]}
        self.assertEqual(heldout.hits(dump, 5, by_file=False), {("x", "sym:a/b.ts::f"): False})
        self.assertEqual(heldout.hits(dump, 5, by_file=True), {("x", "sym:a/b.ts::f"): True})

class Fires(unittest.TestCase):
    def test_the_sweep_counts_firings_on_the_document_dump_and_recall_on_the_code_dump(self):
        doc = {"queries": [{"q": "d", "expect": "FR-1", "bm25_passages": [["FR-1", 10.0]], "bm25_code_files": [["file:a/b.ts", 9.0]], "ask": {"seeds": []}}]}
        code = {"queries": [{"q": "c", "expect": "sym:a/b.ts::f", "bm25_passages": [["FR-2", 5.0]], "bm25_code_files": [["file:a/b.ts", 6.0]], "ask": {"seeds": [["FR-2", 1.0]]}}]}
        rows = heldout.sweep(doc, code, "bm25_code_files", [0.85, 1.0], at=6)
        self.assertEqual(rows[0], {"gate": 0.85, "fires": 1, "of": 1, "recall": 1, "code_n": 1})
        self.assertEqual(rows[1], {"gate": 1.0, "fires": 0, "of": 1, "recall": 1, "code_n": 1})

if __name__ == "__main__":
    unittest.main()
```

The `sweep` contract: for each γ, a document query *fires* when `best(bm25_code_files) ≥ γ · best(bm25_passages)`; a code query is a *recall* hit when its own file is among the first `at` seeds of the dump **or** when it fires and the first `bm25_code_files` id not already among the seeds is its file (the seat's own rule, replayed offline). `heldout.parse(argv)` is the argparse parser factored out of `main()`.

- [ ] **Step 2: Run to see them fail**

Run: `cd /Users/max/Documents/projects/repograph/.worktrees/weak-spots/bench && python3 -m unittest test_heldout -v`
Expected: three failures — `parse` and `sweep` undefined, `hits()` takes no `by_file`.

- [ ] **Step 3: Implement**

```python
# bench/heldout.py — the changed pieces

def is_code(node): return node.startswith(("sym:", "file:"))

def file_of(node):
    if node.startswith("sym:") and "::" in node: return node[4:].split("::")[0]
    if node.startswith("file:"): return node[5:]
    return None

def cmd_build(args):
    entries = json.loads((Path(args.store) / "questions.json").read_text())["entries"]
    nodes = sorted(k for k, v in entries.items() if v.get("questions") and (is_code(k) == args.code))
    ...  # unchanged from here: sample, kind "synthetic", expect = node

def hits(dump, at, by_file=False):
    out = {}
    for q in dump["queries"]:
        seeds = [i for i, _ in q["ask"]["seeds"][:at]]
        if by_file and file_of(q["expect"]):
            target = file_of(q["expect"])
            out[(q["q"], q["expect"])] = any(file_of(s) == target for s in seeds)
        else:
            out[(q["q"], q["expect"])] = q["expect"] in seeds
    return out

def best(rows): return rows[0][1] if rows else 0.0

def sweep(doc, code, lst, gates, at):
    rows = []
    for g in gates:
        fires = sum(1 for q in doc["queries"] if q.get(lst) and best(q[lst]) >= g * best(q["bm25_passages"]))
        recall = 0
        for q in code["queries"]:
            seeds = [i for i, _ in q["ask"]["seeds"][:at]]
            target = file_of(q["expect"])
            hit = any(file_of(s) == target for s in seeds)
            if not hit and q.get(lst) and best(q[lst]) >= g * best(q["bm25_passages"]):
                pick = next((i for i, _ in q[lst] if file_of(i) not in {file_of(s) for s in seeds}), None)
                hit = pick is not None and file_of(pick) == target
            recall += hit
        rows.append({"gate": g, "fires": fires, "of": len(doc["queries"]), "recall": recall, "code_n": len(code["queries"])})
    return rows

def cmd_fires(args):
    doc, code = json.loads(Path(args.doc).read_text()), json.loads(Path(args.code_dump).read_text())
    for r in sweep(doc, code, args.list, [float(g) for g in args.gates.split(",")], args.at):
        print(f"gate {r['gate']:.2f}  fires {r['fires']}/{r['of']} ({r['fires']/r['of']:.0%})  code recall@{args.at} {r['recall']}/{r['code_n']}")
```

Wire the flags: `build --code` (store_true), `compare --by-file` (store_true, passed to `hits`), `fires DOC CODE_DUMP --list NAME --gates LIST --at N` (default `at` 6). Keep every existing behaviour byte for byte when the flags are absent.

- [ ] **Step 4: Run the tests**

Run: `python3 -m unittest test_heldout -v` and `python3 -m unittest test_track -v` (the neighbouring suite must stay green)
Expected: PASS ×3; track unchanged.

- [ ] **Step 5: Commit the Python side**

```bash
git -C $W add bench/heldout.py bench/test_heldout.py
git -C $W commit -F - <<'MSG'
feat(bench): a held-out set for the code side, judged by file, and a gate sweep

`heldout.py build --code` samples the code nodes' own questions the way the document set was
sampled; `compare --by-file` counts a seed in the expected node's file, which is what a `where`
question asks; `fires` replays a gate over a document dump and a code dump so the code seat's
constant is chosen on the two held-outs and not on the developer suite.
MSG
```

- [ ] **Step 6: The set and the baseline dumps (after Task 2 lands `search_files` and this task's `dump` field)**

Add to `src/dump.rs`, beside `bm25_code`: `bm25_code_files: Vec<(String, f32)>` filled by `lexical_c.search_files(&q.q, depth, 3)` where `lexical_c` is the code-questions index already built there. Test: extend `dump`'s existing serialisation test so the field is present and, on a graph with one file and two symbols, holds the `file:` id once. Commit `feat(dump): the file-level code list beside the symbol list`.

```bash
python3 $W/bench/heldout.py build --store $S/bc-code/.repograph --size 400 --seed 7 --code --out $S/heldout-code-400.jsonl
B=$W/target/release/repograph
$B --repo $S/bc-code dump --queries $S/heldout-code-400.jsonl --out $S/hoc-base-dense.json
$B --no-dense --repo $S/bc-code dump --queries $S/heldout-code-400.jsonl --out $S/hoc-base-lex.json
$B --repo $S/bc-code dump --queries $S/heldout-400-syn.jsonl --out $S/hod-base-dense.json
$B --no-dense --repo $S/bc-code dump --queries $S/heldout-400-syn.jsonl --out $S/hod-base-lex.json
python3 $W/bench/heldout.py compare --by-file $S/hoc-base-dense.json $S/hoc-base-dense.json --at 5 | head -3
```

Expected: `400 questions from 3463 enriched nodes`; the self-compare prints recall@5 of the shipped fusion on the code held-out (the plain fusion has no code list, so expect a low number — record it; it is the baseline the seat is measured against). Record all four dump summaries in `$S/t1-baselines.txt`.

---

### Task 2: The file-level code list

**Files:**
- Modify: `src/index/lexical.rs` (new `search_files`)
- Test: `src/index/lexical.rs` `mod tests`

**Interfaces:**
- Consumes: `LexicalIndex::search(&self, query: &str, k: usize) -> Vec<(String, f32)>`, `Node::file` (the `hit` builder in `src/query.rs` reads `n.file`), `file:` ids for File nodes.
- Produces: `pub fn search_files(&self, query: &str, k: usize, per_file: usize) -> Vec<(String, f32)>` — the symbol and file hits of `search(query, k * per_file)` aggregated per file (a `file:` hit counts for its own file; a `sym:` hit for the file in its id), each file's score the sum of its `per_file` best hits, sorted descending, returned as `("file:<path>", score)`, at most `k` long.

- [ ] **Step 1: Failing test**

```rust
#[test]
fn search_files_sums_a_files_best_hits_and_returns_file_ids() {
    // Three symbols of `a.ts` each match the query once; `b.ts` has one symbol matching twice as
    // strongly. Per symbol `b.ts` wins; per file with the three best hits summed, `a.ts` wins —
    // which is what a `where` question wants: the file the query keeps landing in.
    let mut g = Graph::default();
    for (id, text) in [("sym:a.ts::x", "refund money"), ("sym:a.ts::y", "refund money"), ("sym:a.ts::z", "refund money"), ("sym:b.ts::w", "refund refund money money")] {
        let mut n = Node::new(id, NodeKind::Symbol); n.file = Some(id[4..].split("::").next().unwrap().to_string()); n.body = text.into(); g.nodes.insert(id.into(), n);
    }
    let mut q = Questions::default();
    for id in ["sym:a.ts::x", "sym:a.ts::y", "sym:a.ts::z", "sym:b.ts::w"] { q.entries.insert(id.into(), Entry { questions: vec!["how is a refund paid".into()], ..Default::default() }); }
    let idx = LexicalIndex::build_code_questions(&g, &q);
    let files = idx.search_files("refund money", 5, 3);
    assert_eq!(files[0].0, "file:a.ts");
    assert_eq!(files.len(), 2);
    assert!(files[0].1 > files[1].1);
}
```

Adapt `Node::new` / `Entry` to the constructors the neighbouring tests use (`src/index/lexical.rs` tests build graphs already; copy their helper).

- [ ] **Step 2: Run to see it fail** — `cargo test --release search_files_sums` — expected: `no method named search_files`.

- [ ] **Step 3: Implement**

```rust
/// Symbol and file hits folded per file: a `where` question wants the file the query keeps landing
/// in, and per-symbol ranking puts the right file first for only 7 of 28 developer-suite anchors
/// while the three best hits per file summed put it within the top five for 16 (2026-09-05).
pub fn search_files(&self, query: &str, k: usize, per_file: usize) -> Vec<(String, f32)> {
    let mut per: std::collections::HashMap<String, Vec<f32>> = std::collections::HashMap::new();
    for (id, score) in self.search(query, k.saturating_mul(per_file).max(k)) {
        let Some(file) = file_of(&id) else { continue };
        per.entry(file.to_string()).or_default().push(score);
    }
    let mut out: Vec<(String, f32)> = per.into_iter().map(|(f, mut s)| {
        s.sort_by(|a, b| b.total_cmp(a));
        (format!("file:{f}"), s.iter().take(per_file).sum())
    }).collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out.truncate(k);
    out
}

fn file_of(id: &str) -> Option<&str> {
    if let Some(rest) = id.strip_prefix("sym:") { return rest.split("::").next(); }
    id.strip_prefix("file:")
}
```

- [ ] **Step 4: Run** — `cargo test --release search_files_sums` PASS; `cargo clippy --release --all-targets -- -D warnings` silent.

- [ ] **Step 5: Commit** — `feat(index): the code list folded per file, the three best hits of a file summed` with the sim numbers (7/28 → 16/28 within the top five) in the body, naming `code-rank-sim.py` and `dev-dump-a3-lex.json`.

---

### Task 3: The code seat — sixth, gated on the held-outs, never displacing a document

**Files:**
- Modify: `src/query.rs` (`Options.code_seat`, `CODE_SEAT_GATE`, `code_seat`, the append in `ask`), `src/main.rs` (`--no-code-seat` on `ask`, `bench`, `dump`), `src/dump.rs` (nothing beyond Task 1's field; the dump's `ask` shows six seeds when the seat fires)
- Test: `src/query.rs` `mod tests`
- Create: `$S/t3-chain.sh`, transcripts `$S/t3-*.txt`, dumps `$S/hoc-seat-*.json`, `$S/hod-seat-*.json`

**Interfaces:**
- Consumes: Task 2's `search_files`; Task 1's `heldout.py fires` and the four baseline dumps; `fuse::interleave`; `exact_seeds`; `Options`.
- Produces: `pub const CODE_SEAT_GATE: f32` (value from Step 2), `Options { code_seat: bool, .. }` (default `true`), `fn code_seat(graph, questions, query, depth, seeds: &[Hit], passages_best: f32) -> Option<Hit>`.

**Rule, pre-registered (spec §Rules):** ships only if all hold — (i) recorded floors in all four arms with p90 ≤ 230 **with the seat on**; (ii) document held-out: `compare --at 5` reads 0 gained 0 lost in each arm (by construction — verified, not assumed), and the code held-out `compare --by-file --at 6` against `hoc-base-*` is significantly better in each arm; (iii) developer suite not down in either arm, and `where` ≥ 2/9 (v2: ≥ 6/29) in both arms. The gate is chosen in Step 2 on the two held-outs only: **the largest γ in {0.5, 0.7, 0.85, 1.0, 1.2} whose code recall@6 is within two points of the best γ's, subject to the document firing rate ≤ 15 %**; if no γ satisfies the firing bound, γ = the smallest that does and the recall is what it is. If p90 exceeds 230 with the seat, one pre-registered fallback is tried: the seat's hit prints as path only (a File hit with no snippet line) — then re-measured once; still over 230 means the lever fails (i) and is recorded.

- [ ] **Step 1: The gate sweep (design set only)**

```bash
S=…; W=…; python3 $W/bench/heldout.py fires $S/hod-base-lex.json $S/hoc-base-lex.json --list bm25_code_files --gates 0.5,0.7,0.85,1.0,1.2 --at 6 | tee $S/t3-sweep-lex.txt
python3 $W/bench/heldout.py fires $S/hod-base-dense.json $S/hoc-base-dense.json --list bm25_code_files --gates 0.5,0.7,0.85,1.0,1.2 --at 6 | tee $S/t3-sweep-dense.txt
```

Apply the rule above to the lexical arm's table (the seat is a lexical list; the dense arm is the confirmation) and write the chosen γ and the two tables into the ledger before any code is written.

- [ ] **Step 2: Failing tests**

```rust
#[test]
fn the_code_seat_is_appended_after_the_five_and_never_replaces_a_document() {
    // Five documents fill the seeds; the code list's best file clears the gate; the answer has six
    // seeds, the first five unchanged, the sixth a `file:` id.
    let (g, q) = graph_with_five_documents_and_a_matching_file();   // helper beside the module's other fixtures
    let with = ask(&g, &ids(&g), &q, None, None, &words("refund money"), &Options { code_seat: true, ..opts() });
    let without = ask(&g, &ids(&g), &q, None, None, &words("refund money"), &Options { code_seat: false, ..opts() });
    assert_eq!(with.seeds.len(), 6);
    assert_eq!(with.seeds[..5].iter().map(|h| &h.id).collect::<Vec<_>>(), without.seeds.iter().map(|h| &h.id).collect::<Vec<_>>());
    assert!(with.seeds[5].id.starts_with("file:"));
}

#[test]
fn the_code_seat_stays_empty_under_the_gate_and_when_its_file_is_already_seated() { /* two cases, same helper, one with a weak code match and one whose seeds already hold a symbol of that file */ }

#[test]
fn the_code_seat_is_not_expanded_from() { /* the expanded line comes from the five, never from the sixth */ }
```

- [ ] **Step 3: Run to see them fail** — `cargo test --release the_code_seat` — expected: `no field code_seat`.

- [ ] **Step 4: Implement**

```rust
pub struct Options { pub seeds: usize, pub bodies: bool, pub dense: bool, pub json: bool, pub depth: usize, pub code_seat: bool }

/// Chosen on the two held-outs on 2026-09-06 (`$S/t3-sweep-lex.txt`): the largest gate whose code
/// recall@6 is within two points of the best, with the document held-out firing on ≤ 15 % of its
/// questions — a seat that fires on a document question is a sixth node's tokens for nothing.
pub const CODE_SEAT_GATE: f32 = <value from Step 1>;

/// A sixth seat for the file the code questions keep landing in. Never one of the five: a seat given
/// to code displaced a document's answer on 6 of 400 held-out questions in each arm (A4, 2026-09-05);
/// appended after them it costs the answer tokens and nothing else, and reaches `where` 4/9 at most
/// on the developer suite because the right file is first in the list for four of thirteen anchors.
fn code_seat(graph: &Graph, questions: &Questions, query: &str, depth: usize, seeds: &[Hit], passages_best: f32) -> Option<Hit> {
    if questions.entries.is_empty() { return None; }
    let files = LexicalIndex::build_code_questions(graph, questions).search_files(query, depth, 3);
    let best = files.first().map(|(_, s)| *s).unwrap_or(0.0);
    if best <= 0.0 || best < CODE_SEAT_GATE * passages_best { return None; }
    let seated: BTreeSet<&str> = seeds.iter().filter_map(|h| h.file.as_deref()).collect();
    let (id, score) = files.into_iter().find(|(id, _)| !seated.contains(&id[5..]))?;
    hit(graph, &id, score, None)
}
```

In `ask`, after the five seeds are filled and only on the fused path (`!whole_question`), when `opts.code_seat` and `rerank.is_none()`: compute `passages_best` once (the plain passage list's first score — `lexical_lists` returns ids only, so either return the score alongside or recompute `LexicalIndex::build(graph).search(&query, 1)`; prefer extending `lexical_lists` to return the passages' best as a second value) and `if let Some(h) = code_seat(…) { answer.seeds.push(h); }`. The expansion step iterates `answer.seeds[..opts.seeds]` only. `--no-code-seat` in `main.rs` sets `code_seat: false` for `ask`, `bench` and `dump`.

- [ ] **Step 5: Run** — the three tests PASS; the whole suite; clippy silent; `cargo build --release`.

- [ ] **Step 6: Measure (validation sets, both arms, seat on; the seat-off arms are the baselines already recorded)**

`$S/t3-chain.sh` (same shape as the earlier chains): `bench` ×4 on `$S/bc-code` (recorded and developer, both arms) → `$S/t3-enr-{dense,lexical}.txt`, `$S/t3-enr-dev-{dense,lexical}.txt`; raw arms on a copy of the fixture without questions? — no: the raw floors are measured on `$S/bc-doc` with `--no-code-seat`? No — raw arms have no questions, so the seat cannot fire; measure the raw arms once on `$S/bc-code` with a `questions.json` moved aside and back (the chain does it) → `$S/t3-raw-*.txt`; dumps of both held-outs in both arms → `$S/hod-seat-*.json`, `$S/hoc-seat-*.json`; then

```bash
python3 $W/bench/heldout.py compare $S/ho-dense-085-before.json $S/hod-seat-dense.json --at 5
python3 $W/bench/heldout.py compare $S/ho-lex-085.json $S/hod-seat-lex.json --at 5
python3 $W/bench/heldout.py compare --by-file $S/hoc-base-dense.json $S/hoc-seat-dense.json --at 6
python3 $W/bench/heldout.py compare --by-file $S/hoc-base-lex.json $S/hoc-seat-lex.json --at 6
for a in dense lexical; do python3 $S/flips.py $S/t0-… $S/t3-enr-dev-$a.txt; done
```

Read the rule; the controller rules. If it passes: commit `feat(query): a sixth seat for the file the code questions keep landing in` with every number in the body. If it fails: keep the code behind `code_seat: false` by default (one commit, `feat(query): the code seat, measured and off`), and the results doc records the numbers.

---

### Task 4: Developer suite v2 — 20 `where`, 10 `cross`, 9 `rule`

**Files:**
- Modify: `bench/dev-cases.jsonl` (append 39 cases), `bench/history/README.md` (the case-set change 60 → 99 and the one `NOT COMPARABLE` it causes)
- Create: `$S/dev-v2-author-{1,2,3}.jsonl` (drafts), `$S/dev-v2-validate.txt`

Three author subagents (opus), each given the corpus path, the kinds' definitions from `docs/bench/2026-09-05-dev-cases-results.md`, a disjoint list of directories (`apps/api/src/modules/*` split three ways plus `packages/*`), and the same instruction the first sixty had: write questions the way a developer asks while implementing, 18–43 words, mixed vocabulary, Russian and English, anchors that are files (`where`), a requirement plus a file (`cross`), or an ADR/INV/NFR (`rule`); **never run the tool**. The controller validates anchors against the graph (`bench --cases` refuses unknown anchors before asking), dedupes against the existing sixty (no shared anchor set), and appends.

- [ ] **Step 1: Dispatch the three authors** (13 cases each: 7 `where`, 3 `cross`, 3 `rule`); collect drafts.
- [ ] **Step 2: Validate** — `$W/target/release/repograph --repo /Users/max/bench/beauty-crm-502e8a6d bench --cases $S/dev-v2-all.jsonl 2>&1 | head -5` must not name an unknown anchor; fix or drop.
- [ ] **Step 3: Append and record** — append to `bench/dev-cases.jsonl` (99 lines), note in `bench/history/README.md`, then record both arms on the fixture: `CASES=bench/dev-cases.jsonl bench/history/run-repograph.sh` (the first run after the change prints `NOT COMPARABLE … 60 → 99` once — expected). Commit `chore(bench): developer suite v2 — 39 more cases, `where` 29, `cross` 25, `rule` 18`, the two summary lines in the body.
- [ ] **Step 4:** Re-read Task 3's rule with v2 numbers if Task 3 has not yet measured (order: Task 4 before Task 3's Step 6 is preferred; the plan's order allows either, the ledger records which).

---

### Task 5: `where`-shaped code questions (prompt v2 for code)

**Files:**
- Modify: `src/enrich.rs` (`prompt_code` v2 behind `Scope.prompt_version`), `src/config.rs` (`enrich_prompt`), `src/main.rs` (`--prompt v1|v2` on `enrich`)
- Test: `src/enrich.rs` `mod tests`
- Create: `$S/bc-code-p2` (copy of `$S/bc-code` with the code questions emptied), transcripts `$S/t5-*`

**Rule:** developer suite v2 `where` + `cross` hits up in both arms against Task 3's numbers on the same binary; recorded floors hold; the document held-out unchanged by construction (code questions never enter the document lists — asserted by the existing test `code_questions_reach_the_reranked_pool_and_never_the_plain_fusion`). Cost estimate before the run: 3,463 nodes × (prompt ≈ 900 chars + answer ≈ 700 chars) on haiku ≈ $3–4; stated in the ledger, byte counts after.

- [ ] **Step 1: Failing test** — `prompt_code_v2_asks_for_two_where_questions_per_node`: the v2 prompt text contains the two shape lines (`Где править, чтобы …` / `Which file do I edit to …`) and the v1 prompt does not.
- [ ] **Step 2: Implement** — v2 = v1 plus: "Two of the eight questions must be the ones a developer asks *before editing*: «Где править, чтобы …» / “Which file do I edit to …”, naming what they would add or change, never the identifier." `Scope { prompt_version: u8 }`; `enrich_prompt` config default `"v1"`.
- [ ] **Step 3: Generate** — `cp -R $S/bc-code $S/bc-code-p2`; empty the code entries of `questions.json` (python: drop keys starting `sym:`/`file:`); `$W/target/release/repograph --repo $S/bc-code-p2 enrich --code --prompt v2 --parallel 8 2>&1 | tee $S/t5-enrich.log` with the `tee` wrapper `enrich_command` so prompts and answers are kept; expect `3463 nodes written, 0 failed`.
- [ ] **Step 4: Measure** — Task 3's chain on `$S/bc-code-p2` (both suites, both arms, both held-outs) → `$S/t5-*`; flips against `$S/t3-*`.
- [ ] **Step 5: Rule, commit** — if it passes, `enrich_prompt` default becomes `v2` for code (config + README); else v2 stays opt-in and is recorded.

---

### Task 6: G14 — the kind-aware document prompt, on haiku and on sonnet

**Files:**
- Modify: `src/enrich.rs` (`prompt` v2 by node kind), `src/config.rs`, `README.md` (the `enrich` paragraph), `docs/bench/runbook.md` (costs)
- Create: `$S/bc-doc-p2h` (fixture corpus copy, document questions regenerated by haiku with v2), `$S/bc-doc-p2s` (same, sonnet), transcripts `$S/t6-*`

**Rule (D1's, plus `rule`):** recorded floors in both enriched arms with p90 ≤ 230; document held-out not significantly worse in either arm (before = fixture dumps); developer suite v2 not down in either arm **and `rule` not down** (v2: ≥ 9/18 … the v1 store's v2 reading, measured in Task 4, is the floor). Ships as the default prompt if haiku-v2 passes; ships as the default generator (`ENRICH_COMMAND` → sonnet) only if sonnet-v2 passes and beats haiku-v2 on paraphrase and held-out. Costs before the runs: haiku ≈ $2.5, sonnet ≈ $13.6 (both measured shapes on 2026-09-05).

- [ ] **Step 1: Failing test** — `prompt_v2_gives_an_adr_the_is_this_allowed_shape_and_a_requirement_the_old_one`.
- [ ] **Step 2: Implement** — for `NodeKind::Adr | Invariant` (and NFR requirements, detected by id prefix `NFR-`): three of the questions must be of the shape «Можно ли … / что запрещает … / какое решение это покрывает» — "Is it allowed to … / what forbids … / which decision covers …" — phrased from the situation a developer is in, never quoting the title. Requirements keep v1's shapes.
- [ ] **Step 3: Generate twice** — `bc-doc-p2h` with haiku, `bc-doc-p2s` with `claude -p --model sonnet`; both through the `tee` wrapper; `embed` each.
- [ ] **Step 4: Measure both** — the D1 chain shape (`$S/d1-chain.sh` with `R=` and names changed) → `$S/t6h-*`, `$S/t6s-*`; flips against `$S/t0-baselines` (recorded) and Task 4's v2 fixture lines (developer).
- [ ] **Step 5: Rule, commit** — as the rule says; every number in the body; README/runbook cost sentences updated only for what ships.

---

### Task 7: The combination Max's decision needs

**Files:**
- Create: `$S/bc-combo` (the best generator store from Task 6 — or `$S/bc-doc` if none passed — with `embed_model = "intfloat/multilingual-e5-large"` in its `repograph.toml`, then `embed`; ≈ 45 min), transcripts `$S/t7-*`

No ship rule: the default embedder is Max's decision; this task measures the configuration he would be choosing. Both suites, dense arm; the document held-out dense arm; the code held-out if Task 3 shipped.

- [ ] **Step 1** copy, config, `embed` (log the time and the row count); **Step 2** bench ×2, dumps, compares against the fixture dumps and against `$S/c1-*` (e5-large alone); **Step 3** a table in the ledger: default / large / best-generator / best-generator+large, with paraphrase, held-out, dev v2 totals, `rule`, `where`, p90, `ask` latency (`time` of one fused `ask`, three runs, median).

---

### Task 8: Docs, history, artifact, PR

**Files:**
- Create: `docs/bench/2026-09-06-weak-spots-results.md`
- Modify: `docs/bench/next-version-gaps.md` (G12/G13/G14 verdicts, new gaps), `README.md`, `docs/bench/runbook.md` (the code held-out, the sweep), `bench/history/runs.jsonl` (both suites at head)

- [ ] **Step 1** results doc: one section per task with the rule restated in one line, the table, the flips, the verdict — numbers only from `$S/t*-*` transcripts and the ledger.
- [ ] **Step 2** gaps doc verdicts; **Step 3** README (`--no-code-seat`, `enrich --prompt`, the code held-out); **Step 4** `bench/history/run-repograph.sh` for both suites; **Step 5** commit `docs(bench): the weak spots, measured — code seat, where-shaped questions, G14, the combination`.
- [ ] **Step 6** artifact: the existing page gets the new rows and the decision table; **Step 7** PR against `main`: `gh auth switch --user devmaxxx && gh pr create --base main …`, body = the results doc's closing section; no trailers.

---

## Self-review

- Spec coverage: W1 → Tasks 1, 2, 3, 5; W2 → Task 6 (fusion variants rejected in the spec, no task); W3 → Tasks 6, 7; W4 → Tasks 1, 4. Rules: every retrieval lever names the three-way rule plus its own clause; generator levers name D1's rule plus `rule`.
- Placeholders: `CODE_SEAT_GATE`'s value is by design read from Step 1 of Task 3 before any code is written; the two helper fixtures in Task 3's tests are named and described, not left as "TBD". Task 4's cases are authored, not planned in advance.
- Types: `search_files(&self, &str, usize, usize) -> Vec<(String, f32)>` in Tasks 1, 2, 3; `Options.code_seat: bool` in Tasks 3 and the file map; `heldout.py` functions `hits(dump, at, by_file)`, `sweep(doc, code, lst, gates, at)`, `parse(argv)` in Task 1 only.
