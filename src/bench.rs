use crate::config::Config;
use crate::enrich::{coverage, Questions};
use crate::ids::IdMatcher;
use crate::index::dense::DenseIndex;
use crate::index::embed::Embedder;
use crate::model::Graph;
use crate::query::{self, Answer, Options};
use crate::store::Store;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Case { pub kind: String, pub q: String, pub expect: String }

#[derive(Clone, Debug, Default)]
pub struct Summary { pub keyword: (usize, usize), pub paraphrase: (usize, usize), pub code: (usize, usize), pub p90_tokens: usize }

pub fn hit(case: &Case, answer: &Answer) -> bool {
    let all = answer.seeds.iter().chain(answer.expanded.iter());
    if case.kind == "code" {
        return answer.seeds.iter().any(|h| h.file == case.expect);
    }
    all.into_iter().any(|h| h.id == case.expect)
}

pub fn passes(s: &Summary, dense: bool, enriched: bool) -> bool {
    // Every floor is the number the recorded cases measure; only the token ceiling is rounded,
    // up to the next ten, and the four p90s (221 to 228) fit under that one. A count equal to
    // its total is an exact floor: `run` refuses a case file that is not 40/30/12 before any of
    // this is read.
    //
    // `enrich` spends model tokens and is optional, so the store it has never touched is graded
    // on what it reads rather than on what the enriched store calibrated: paraphrase measures 9
    // with embeddings and 7 without, against the 14 and 11 the questions buy, and the lexical-only
    // arm reaches 39 of the 40 keyword cases, so even that floor is not exact there. Paraphrase is
    // the split the arms disagree on most — three of its questions are reached by the dense
    // passage list and by neither lexical list.
    let (keyword, paraphrase) = match (enriched, dense) {
        (true, true) => (40, 14),
        (true, false) => (40, 11),
        (false, true) => (40, 9),
        (false, false) => (39, 7),
    };
    s.keyword.0 >= keyword && s.paraphrase.0 >= paraphrase && s.code.0 >= 12 && s.p90_tokens <= 230
}

// The recorded cases travel inside the binary so a release build benches from any directory.
const BUILT_IN_CASES: &str = include_str!("../bench/cases.jsonl");

fn parse_cases(text: &str) -> Result<Vec<Case>> {
    text.lines().filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Case>(l).map_err(anyhow::Error::from))
        .collect()
}

pub fn run(repo: &Path, cases: Option<&Path>, no_dense: bool, rerank: bool, rerank_local: bool, depth: usize) -> Result<bool> {
    // Resolved before `Config::load` so the override repo's own `repograph.toml` — not the
    // `--repo` one — is what the `IdMatcher` is built from.
    let repo = std::env::var("REPOGRAPH_BENCH_REPO").map(std::path::PathBuf::from).unwrap_or(repo.to_path_buf());
    let cfg = Config::load(&repo)?;
    let store = Store::new(&repo);
    let (graph, _): (Graph, _) = store.load()?;
    if graph.nodes.is_empty() { anyhow::bail!("graph is empty at {} — run build first", repo.display()); }
    let ids = IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
    let dense_idx = DenseIndex::load(&store)?;
    let questions = Questions::load(&store)?;
    // A graph with nothing to enrich cannot be told from one nobody has enriched, and the
    // recorded cases expect the corpus's requirements either way, so it is graded as raw.
    let (covered, eligible) = coverage(&graph, &questions);
    let enriched = eligible > 0 && covered == eligible;
    // `ask` degrading to lexical-only on a missing model is fine — a person reading the answer
    // sees the stderr notice and can judge it. `bench` speaks only through its exit code, so a
    // dense run that silently falls back and then grades against the weaker no-dense floor
    // would report green without ever having checked what it claims to check.
    let embedder = if no_dense {
        None
    } else {
        match Embedder::open() {
            Ok(e) => Some(e),
            Err(err) => anyhow::bail!("dense: model unavailable ({err:#})"),
        }
    };
    if !no_dense && dense_idx.ids.is_empty() {
        anyhow::bail!("dense index is empty at {} — run `repograph update` first", repo.display());
    }
    let dense_on = !no_dense;
    let embedder = std::cell::RefCell::new(embedder);
    let dense_fn = |q: &str, k: usize| -> (Vec<String>, Vec<String>) {
        let mut e = embedder.borrow_mut();
        match e.as_mut().and_then(|e| e.query(q).ok()) { Some(v) => dense_idx.search(&v, k), None => (Vec::new(), Vec::new()) }
    };
    let (cases_path, text) = match cases {
        Some(p) => (p.display().to_string(), std::fs::read_to_string(p).with_context(|| p.display().to_string())?),
        None => ("built-in bench/cases.jsonl".to_string(), BUILT_IN_CASES.to_string()),
    };
    let cases: Vec<Case> = parse_cases(&text)?;
    for c in &cases {
        if !matches!(c.kind.as_str(), "keyword" | "paraphrase" | "code") {
            anyhow::bail!("unrecognised case kind {:?} (expect {:?})", c.kind, c.expect);
        }
    }
    // The recorded case set is a constant, not an input: a truncated or edited file would
    // otherwise still pass, since every floor below is relative to whatever total showed up.
    let (kw, pf, cd) = (
        cases.iter().filter(|c| c.kind == "keyword").count(),
        cases.iter().filter(|c| c.kind == "paraphrase").count(),
        cases.iter().filter(|c| c.kind == "code").count(),
    );
    if (kw, pf, cd) != (40, 30, 12) {
        anyhow::bail!("{cases_path} has {kw} keyword / {pf} paraphrase / {cd} code cases, expected 40/30/12");
    }
    let opts = Options { seeds: 5, bodies: false, dense: dense_on, json: false, depth };
    let rerank_fn = |q: &str, c: &[(String, String)]| crate::rerank::run(&cfg.rerank_command, q, c);
    let cross = std::cell::RefCell::new(if rerank_local {
        let dir = if cfg.reranker_dir.is_empty() { crate::index::cross::default_dir()? } else { std::path::PathBuf::from(&cfg.reranker_dir) };
        Some(crate::index::cross::CrossEncoder::open(&dir).context("--rerank-local")?)
    } else { None });
    let local_fn = |q: &str, c: &[(String, String)]| -> Vec<String> {
        let mut m = cross.borrow_mut();
        let Some(m) = m.as_mut() else { return Vec::new() };
        let texts: Vec<String> = c.iter().map(|(_, t)| t.clone()).collect();
        match m.score(q, &texts) {
            Ok(s) => crate::index::cross::pick(&s, c, crate::index::cross::PICK),
            // Like a failing rerank command: say so and answer from the fused order.
            Err(e) => { eprintln!("rerank-local: {e:#}; answering from the fused order"); Vec::new() }
        }
    };
    let rerank: Option<query::Rerank> = if rerank_local { Some(&local_fn) } else if rerank { Some(&rerank_fn) } else { None };
    let mut summary = Summary::default();
    let mut tokens = Vec::new();
    for case in &cases {
        let words: Vec<String> = case.q.split_whitespace().map(str::to_string).collect();
        let answer = query::ask(&graph, &ids, &questions, Some(&dense_fn), rerank, &words, &opts);
        let rendered = query::render(&answer, &graph, &opts);
        let tok = rendered.len() / 4;
        tokens.push(tok);
        let ok = hit(case, &answer);
        let slot = match case.kind.as_str() { "keyword" => &mut summary.keyword, "paraphrase" => &mut summary.paraphrase, _ => &mut summary.code };
        slot.1 += 1;
        if ok { slot.0 += 1; }
        println!("{:<10} {:<12} {} {:>4} tok  {}", case.kind, case.expect, if ok { "HIT " } else { "miss" }, tok, case.q);
    }
    tokens.sort_unstable();
    summary.p90_tokens = tokens.get(tokens.len() * 9 / 10).copied().unwrap_or(0);
    println!("\nkeyword {}/{}  paraphrase {}/{}  code {}/{}  p90 {} tok  dense={dense_on}  enriched={enriched} ({covered}/{eligible} nodes){}",
        summary.keyword.0, summary.keyword.1, summary.paraphrase.0, summary.paraphrase.1, summary.code.0, summary.code.1, summary.p90_tokens, match (rerank_local, rerank.is_some()) {
            (true, _) => format!(" rerank_local=true depth={depth}"),
            (false, true) => format!(" rerank=true depth={depth}"),
            _ => String::new(),
        });
    Ok(passes(&summary, dense_on, enriched))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::Hit;

    fn h(id: &str, file: &str) -> Hit { Hit { id: id.into(), file: file.into(), line: 1, label: String::new(), score: 1.0, via: None } }

    #[test]
    fn hit_rules() {
        let a = Answer { seeds: vec![h("sym:x.ts::f", "packages/x.ts")], expanded: vec![h("FR-PAY-22", "d.md")] };
        assert!(hit(&Case { kind: "paraphrase".into(), q: String::new(), expect: "FR-PAY-22".into() }, &a));
        assert!(hit(&Case { kind: "code".into(), q: String::new(), expect: "packages/x.ts".into() }, &a));
        assert!(!hit(&Case { kind: "keyword".into(), q: String::new(), expect: "FR-PAY-23".into() }, &a));

        // A substring of a real id is not a match: ids compare whole-string, not `contains`.
        assert!(!hit(&Case { kind: "keyword".into(), q: String::new(), expect: "PAY-22".into() }, &a));
        // Ids compare verbatim, not case-folded.
        assert!(!hit(&Case { kind: "keyword".into(), q: String::new(), expect: "fr-pay-22".into() }, &a));
        // A keyword/paraphrase case matching a hit's file rather than its id does not count.
        assert!(!hit(&Case { kind: "keyword".into(), q: String::new(), expect: "packages/x.ts".into() }, &a));

        // A code case's expected path is only a suffix of the hit's file: `file` compares
        // whole-string, not `ends_with`.
        let nested = Answer { seeds: vec![h("sym:packages/nested/x.ts::f", "packages/nested/x.ts")], expanded: vec![] };
        assert!(!hit(&Case { kind: "code".into(), q: String::new(), expect: "x.ts".into() }, &nested));
        // A code case only reachable through `expanded` does not count: code scores `seeds` only.
        let expanded_only = Answer { seeds: vec![], expanded: vec![h("sym:packages/x.ts::f", "packages/x.ts")] };
        assert!(!hit(&Case { kind: "code".into(), q: String::new(), expect: "packages/x.ts".into() }, &expanded_only));
    }

    #[test]
    fn floors() {
        // Fixtures sit exactly on each floor so a boundary shifted by one in either direction
        // reddens the corresponding call; a fixture comfortably clear of the floor (the
        // original mistake) would not notice such a shift.
        let at_floor_nodense = Summary { keyword: (40, 40), paraphrase: (11, 30), code: (12, 12), p90_tokens: 230 };
        let at_floor_dense = Summary { keyword: (40, 40), paraphrase: (14, 30), code: (12, 12), p90_tokens: 230 };
        assert!(passes(&at_floor_nodense, false, true));
        assert!(passes(&at_floor_dense, true, true));

        // keyword must be exact: one short reddens in both dense arms.
        assert!(!passes(&Summary { keyword: (39, 40), ..at_floor_nodense.clone() }, false, true));
        assert!(!passes(&Summary { keyword: (39, 40), ..at_floor_dense.clone() }, true, true));

        // code must be exact: one short reddens in both dense arms.
        assert!(!passes(&Summary { code: (11, 12), ..at_floor_nodense.clone() }, false, true));
        assert!(!passes(&Summary { code: (11, 12), ..at_floor_dense.clone() }, true, true));

        // p90: one token over the shared ceiling reddens either arm.
        assert!(!passes(&Summary { p90_tokens: 231, ..at_floor_nodense.clone() }, false, true));
        assert!(!passes(&Summary { p90_tokens: 231, ..at_floor_dense.clone() }, true, true));

        // paraphrase no-dense floor is 11: one short reddens the `dense: false` call.
        assert!(!passes(&Summary { paraphrase: (10, 30), ..at_floor_nodense.clone() }, false, true));
        // paraphrase dense floor is 14: one short reddens the `dense: true` call.
        assert!(!passes(&Summary { paraphrase: (13, 30), ..at_floor_dense.clone() }, true, true));
    }

    #[test]
    fn a_store_without_questions_is_graded_on_the_numbers_it_reads() {
        // Exactly what a store `enrich` has never touched measures on the corpus, so a shift of
        // one in either direction is visible here.
        let raw_dense = Summary { keyword: (40, 40), paraphrase: (9, 30), code: (12, 12), p90_tokens: 221 };
        let raw_nodense = Summary { keyword: (39, 40), paraphrase: (7, 30), code: (12, 12), p90_tokens: 226 };
        assert!(passes(&raw_dense, true, false));
        assert!(passes(&raw_nodense, false, false));

        // The same run against a store that paid for its questions is a failure, which is what
        // keeps the enriched bar a bar.
        assert!(!passes(&raw_dense, true, true));
        assert!(!passes(&raw_nodense, false, true));

        // One short of each raw floor reddens, the `--no-dense` keyword floor of 39 included.
        assert!(!passes(&Summary { paraphrase: (8, 30), ..raw_dense.clone() }, true, false));
        assert!(!passes(&Summary { paraphrase: (6, 30), ..raw_nodense.clone() }, false, false));
        assert!(!passes(&Summary { keyword: (39, 40), ..raw_dense.clone() }, true, false));
        assert!(!passes(&Summary { keyword: (38, 40), ..raw_nodense.clone() }, false, false));
        assert!(!passes(&Summary { code: (11, 12), ..raw_dense.clone() }, true, false));
        assert!(!passes(&Summary { p90_tokens: 231, ..raw_nodense.clone() }, false, false));
    }

    #[test]
    fn cases_file_parses_and_has_the_recorded_shape() {
        let text = std::fs::read_to_string(format!("{}/bench/cases.jsonl", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let cases: Vec<Case> = text.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(cases.iter().filter(|c| c.kind == "keyword").count(), 40);
        assert_eq!(cases.iter().filter(|c| c.kind == "paraphrase").count(), 30);
        assert_eq!(cases.iter().filter(|c| c.kind == "code").count(), 12);
    }

    #[test]
    fn keyword_hit_counts_a_seed_or_an_expanded_entry() {
        let a = Answer { seeds: vec![h("FR-PAY-22", "d.md")], expanded: vec![h("FR-PAY-20", "e.md")] };
        assert!(hit(&Case { kind: "keyword".into(), q: String::new(), expect: "FR-PAY-22".into() }, &a));
        assert!(hit(&Case { kind: "keyword".into(), q: String::new(), expect: "FR-PAY-20".into() }, &a));
    }

    #[test]
    fn paraphrase_hit_counts_a_seed_or_an_expanded_entry() {
        let a = Answer { seeds: vec![h("FR-PAY-22", "d.md")], expanded: vec![h("FR-PAY-20", "e.md")] };
        assert!(hit(&Case { kind: "paraphrase".into(), q: String::new(), expect: "FR-PAY-22".into() }, &a));
        assert!(hit(&Case { kind: "paraphrase".into(), q: String::new(), expect: "FR-PAY-20".into() }, &a));
    }

    #[test]
    fn code_hit_only_counts_a_seed_never_an_expanded_entry() {
        let a = Answer { seeds: vec![h("sym:x.ts::f", "packages/x.ts")], expanded: vec![h("sym:y.ts::g", "packages/y.ts")] };
        assert!(hit(&Case { kind: "code".into(), q: String::new(), expect: "packages/x.ts".into() }, &a));
        assert!(!hit(&Case { kind: "code".into(), q: String::new(), expect: "packages/y.ts".into() }, &a));
    }

    #[test]
    fn cases_file_parsing_skips_blank_lines() {
        let text = "\n{\"kind\":\"keyword\",\"q\":\"a\",\"expect\":\"X\"}\n\n   \n{\"kind\":\"code\",\"q\":\"b\",\"expect\":\"Y\"}\n";
        let cases = parse_cases(text).unwrap();
        assert_eq!(cases.iter().map(|c| c.kind.as_str()).collect::<Vec<_>>(), ["keyword", "code"]);
    }

    #[test]
    fn cases_file_of_only_blank_lines_yields_no_cases() {
        assert!(parse_cases("\n\n   \n").unwrap().is_empty());
    }

    #[test]
    fn cases_file_parsing_rejects_a_malformed_line() {
        let good = "{\"kind\":\"keyword\",\"q\":\"a\",\"expect\":\"X\"}\n";
        assert!(parse_cases(&format!("{good}not json\n")).is_err(), "invalid JSON");
        assert!(parse_cases(&format!("{good}{{\"kind\":\"keyword\",\"q\":\"a\"}}\n")).is_err(), "missing required field");
    }
}
