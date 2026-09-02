use crate::config::Config;
use crate::ids::IdMatcher;
use crate::index::dense::{DenseIndex, Embedder};
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

pub fn passes(s: &Summary, dense: bool) -> bool {
    let floor = if dense { 5 } else { 2 };
    s.keyword.0 == s.keyword.1 && s.paraphrase.0 >= floor.min(s.paraphrase.1) && s.code.0 == s.code.1 && s.p90_tokens <= 230
}

// The recorded cases travel inside the binary so a release build benches from any directory.
const BUILT_IN_CASES: &str = include_str!("../bench/cases.jsonl");

pub fn run(repo: &Path, cases: Option<&Path>, no_dense: bool) -> Result<bool> {
    // Resolved before `Config::load` so the override repo's own `repograph.toml` — not the
    // `--repo` one — is what the `IdMatcher` is built from.
    let repo = std::env::var("REPOGRAPH_BENCH_REPO").map(std::path::PathBuf::from).unwrap_or(repo.to_path_buf());
    let cfg = Config::load(&repo)?;
    let store = Store::new(&repo);
    let (graph, _): (Graph, _) = store.load()?;
    if graph.nodes.is_empty() { anyhow::bail!("graph is empty at {} — run build first", repo.display()); }
    let ids = IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
    let dense_idx = DenseIndex::load(&store)?;
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
    let dense_fn = |q: &str, k: usize| -> Vec<String> {
        let mut e = embedder.borrow_mut();
        match e.as_mut().and_then(|e| e.query(q).ok()) { Some(v) => dense_idx.search(&v, k), None => Vec::new() }
    };
    let (cases_path, text) = match cases {
        Some(p) => (p.display().to_string(), std::fs::read_to_string(p).with_context(|| p.display().to_string())?),
        None => ("built-in bench/cases.jsonl".to_string(), BUILT_IN_CASES.to_string()),
    };
    let cases: Vec<Case> = text.lines().filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Case>(l).map_err(anyhow::Error::from))
        .collect::<Result<_>>()?;
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
    if (kw, pf, cd) != (24, 14, 3) {
        anyhow::bail!("{cases_path} has {kw} keyword / {pf} paraphrase / {cd} code cases, expected 24/14/3");
    }
    let opts = Options { seeds: 5, bodies: false, dense: dense_on, json: false };
    let mut summary = Summary::default();
    let mut tokens = Vec::new();
    for case in &cases {
        let words: Vec<String> = case.q.split_whitespace().map(str::to_string).collect();
        let answer = query::ask(&graph, &ids, Some(&dense_fn), &words, &opts);
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
    println!("\nkeyword {}/{}  paraphrase {}/{}  code {}/{}  p90 {} tok  dense={dense_on}",
        summary.keyword.0, summary.keyword.1, summary.paraphrase.0, summary.paraphrase.1, summary.code.0, summary.code.1, summary.p90_tokens);
    Ok(passes(&summary, dense_on))
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
        let at_floor_nodense = Summary { keyword: (24, 24), paraphrase: (2, 14), code: (3, 3), p90_tokens: 230 };
        let at_floor_dense = Summary { keyword: (24, 24), paraphrase: (5, 14), code: (3, 3), p90_tokens: 230 };
        assert!(passes(&at_floor_nodense, false));
        assert!(passes(&at_floor_dense, true));

        // keyword must be exact: one short reddens in both dense arms.
        assert!(!passes(&Summary { keyword: (23, 24), ..at_floor_nodense.clone() }, false));
        assert!(!passes(&Summary { keyword: (23, 24), ..at_floor_dense.clone() }, true));

        // code must be exact: one short reddens in both dense arms.
        assert!(!passes(&Summary { code: (2, 3), ..at_floor_nodense.clone() }, false));
        assert!(!passes(&Summary { code: (2, 3), ..at_floor_dense.clone() }, true));

        // p90: one token over the floor reddens in both dense arms.
        assert!(!passes(&Summary { p90_tokens: 231, ..at_floor_nodense.clone() }, false));
        assert!(!passes(&Summary { p90_tokens: 231, ..at_floor_dense.clone() }, true));

        // paraphrase no-dense floor is 2: one short reddens the `dense: false` call.
        assert!(!passes(&Summary { paraphrase: (1, 14), ..at_floor_nodense.clone() }, false));
        // paraphrase dense floor is 5: one short reddens the `dense: true` call.
        assert!(!passes(&Summary { paraphrase: (4, 14), ..at_floor_dense.clone() }, true));
    }

    #[test]
    fn cases_file_parses_and_has_the_recorded_shape() {
        let text = std::fs::read_to_string(format!("{}/bench/cases.jsonl", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let cases: Vec<Case> = text.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(cases.iter().filter(|c| c.kind == "keyword").count(), 24);
        assert_eq!(cases.iter().filter(|c| c.kind == "paraphrase").count(), 14);
        assert_eq!(cases.iter().filter(|c| c.kind == "code").count(), 3);
    }
}
