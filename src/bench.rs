use crate::config::Config;
use crate::enrich::{self, coverage, Questions};
use crate::ids::IdMatcher;
use crate::index::dense::DenseIndex;
use crate::index::embed::Embedder;
use crate::model::Graph;
use crate::query::{self, Answer, Options};
use crate::store::Store;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// What a case expects the answer to reach: one node id or file path, or several when the
/// honest answer to the question is more than one place.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Expect { One(String), Many(Vec<String>) }

impl Expect {
    pub fn anchors(&self) -> &[String] {
        match self { Expect::One(a) => std::slice::from_ref(a), Expect::Many(v) => v }
    }

    /// The per-case line and the history key: anchors joined by `+`, a character neither an
    /// id nor a path contains.
    pub fn key(&self) -> String { self.anchors().join("+") }
}

impl From<&str> for Expect {
    fn from(a: &str) -> Self { Expect::One(a.to_string()) }
}

#[derive(Debug, Deserialize)]
pub struct Case { pub kind: String, pub q: String, pub expect: Expect }

/// Hits per kind, in the order the case file introduces the kinds, so the summary line reads
/// in the order a person wrote the file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary { pub by_kind: Vec<(String, (usize, usize))>, pub p90_tokens: usize }

impl Summary {
    #[cfg(test)]
    pub fn recorded(keyword: (usize, usize), paraphrase: (usize, usize), code: (usize, usize), p90_tokens: usize) -> Summary {
        let by_kind = vec![("keyword".to_string(), keyword), ("paraphrase".to_string(), paraphrase), ("code".to_string(), code)];
        Summary { by_kind, p90_tokens }
    }

    /// `(hits, cases)` for one kind; a kind the file never named reads `(0, 0)`.
    pub fn kind(&self, kind: &str) -> (usize, usize) {
        self.by_kind.iter().find(|(k, _)| k == kind).map(|(_, c)| *c).unwrap_or((0, 0))
    }

    #[cfg(test)]
    pub fn with(mut self, kind: &str, count: (usize, usize)) -> Summary {
        *self.slot(kind) = count;
        self
    }

    fn slot(&mut self, kind: &str) -> &mut (usize, usize) {
        if let Some(i) = self.by_kind.iter().position(|(k, _)| k == kind) { return &mut self.by_kind[i].1; }
        self.by_kind.push((kind.to_string(), (0, 0)));
        &mut self.by_kind.last_mut().unwrap().1
    }
}

/// How many of the case's anchors the answer reaches, over how many it has. An anchor that
/// names a node counts anywhere in the answer, seeds or expansion; a file path counts only
/// among the seeds, as `code` cases always have — the expansion names neighbours, and a file
/// reached only as the neighbour of the wrong seed is not a file the developer was pointed at.
/// `is_id` is the graph's knowledge of which strings are node ids, so the case file never has
/// to say which shape an anchor has.
pub fn found(case: &Case, answer: &Answer, is_id: &dyn Fn(&str) -> bool) -> (usize, usize) {
    let anchors = case.expect.anchors();
    let reached = anchors.iter().filter(|a| {
        if is_id(a) {
            answer.seeds.iter().chain(answer.expanded.iter()).any(|h| &h.id == *a)
        } else {
            answer.seeds.iter().any(|h| &h.file == *a)
        }
    }).count();
    (reached, anchors.len())
}

/// One anchor reached is a hit: the developer has an entry point. `found` says how complete it was.
pub fn hit(case: &Case, answer: &Answer, is_id: &dyn Fn(&str) -> bool) -> bool {
    found(case, answer, is_id).0 >= 1
}

/// Which embedder's floors a dense arm is graded against. The lexical arms have no embedder and
/// read one set whatever the store's rows were written by; a dense arm under a model with no
/// floors of its own is measured and not graded, the way another case file is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Floors { Small, Large, None }

pub fn floors_for(model: Option<&str>) -> Floors {
    match model {
        None => Floors::Small,
        Some(m) if m == crate::index::embed::UNNAMED_MODEL => Floors::Small,
        Some(m) if m == crate::index::embed::DEFAULT_MODEL => Floors::Large,
        Some(_) => Floors::None,
    }
}

/// `(enriched, dense, floors) → (keyword, paraphrase)`. Every number is one the recorded cases
/// measured, never a target: the small model's four on the fixture (the paragraphs below), the
/// large model's two dense arms on its copy of the same store, each read twice and agreeing both
/// times (`$G/t5-large-enriched-{1,2}.txt`, `$G/t5-large-raw-{1,2}.txt`). The lexical rows carry
/// `Floors::Small` and are read for every model: no embedder is in them.
/// `bench/history/track.py` reads this table out of the source; keep the rows one per line.
const FLOORS: [(bool, bool, Floors, usize, usize); 6] = [
    (true, true, Floors::Small, 40, 14),
    (true, false, Floors::Small, 39, 11),
    (false, true, Floors::Small, 40, 9),
    (false, false, Floors::Small, 39, 7),
    (true, true, Floors::Large, 40, 22),
    (false, true, Floors::Large, 40, 17),
];

pub fn passes(s: &Summary, dense: bool, enriched: bool, floors: Floors) -> bool {
    // Every floor is the number the recorded cases measure; only the token ceiling is rounded,
    // up to the next ten, and the small model's four p90s (220 to 226) and the large model's two
    // (224, 227) all fit under that one. A count equal to its total is an exact floor: `run`
    // grades against these only when the case file has the recorded 40/30/12 shape.
    //
    // `enrich` spends model tokens and is optional, so the store it has never touched is graded
    // on what it reads rather than on what the enriched store calibrated: paraphrase measures 9
    // with embeddings and 7 without, against the 14 and 11 the questions buy. Paraphrase is the
    // split the arms disagree on most — three of its questions are reached by the dense passage
    // list and by neither lexical list.
    //
    // Keyword is 39 in both lexical-only arms, not 40: `FR-PH-43` sits at passage rank 23 and no
    // lexical path reaches it, enriched or raw. The enriched arm read 40 on an earlier corpus
    // commit and 37 once the questions list took an equal turn in the fusion; gating that list
    // on its own confidence put it back level with the raw store, and level is the floor. It was
    // not lowered while it stood at 37, because 37 was a cost enrichment itself imposed.
    //
    // The raw pair has no headroom, unlike the enriched one: 9 and 7 are two runs on one machine
    // on one day sitting flush on the noisiest split, while 14 has a point of slack and weeks of
    // runs under it. If the raw floors flap, they are the first thing to relax.
    //
    // A dense arm has no floors of its own once the store's embedder is neither the small model
    // nor the large one — those numbers were never measured, so grading them would be inventing a
    // bar. `run` still prints what it found; it just cannot say pass or fail.
    if dense && floors == Floors::None { return false; }
    let key = if dense { floors } else { Floors::Small };
    let Some(&(_, _, _, keyword, paraphrase)) = FLOORS.iter().find(|r| r.0 == enriched && r.1 == dense && r.2 == key) else { return false };
    s.kind("keyword").0 >= keyword && s.kind("paraphrase").0 >= paraphrase && s.kind("code").0 >= 12 && s.p90_tokens <= 230
}

// The recorded cases travel inside the binary so a release build benches from any directory.
const BUILT_IN_CASES: &str = include_str!("../bench/cases.jsonl");

/// The shape the floors were measured on. A file of this shape is graded; any other file is
/// measured and reported, and its exit code claims nothing.
const RECORDED_SHAPE: [(&str, usize); 3] = [("keyword", 40), ("paraphrase", 30), ("code", 12)];

fn parse_cases(text: &str) -> Result<Vec<Case>> {
    text.lines().filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Case>(l).map_err(anyhow::Error::from))
        .collect()
}

/// Kinds in order of first appearance, each with its count.
fn shape(cases: &[Case]) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for c in cases {
        match out.iter_mut().find(|(k, _)| *k == c.kind) {
            Some((_, n)) => *n += 1,
            None => out.push((c.kind.clone(), 1)),
        }
    }
    out
}

fn is_recorded_shape(cases: &[Case]) -> bool {
    let mut got = shape(cases);
    got.sort();
    let mut want: Vec<(String, usize)> = RECORDED_SHAPE.iter().map(|(k, n)| (k.to_string(), *n)).collect();
    want.sort();
    got == want
}

/// Every anchor must be a node id or a file some node declares. A mistyped anchor would
/// otherwise score as a miss for as long as nobody read the transcript, and a weak spot that is
/// really a typo is the one kind this suite must not report.
fn check_anchors(cases_path: &str, cases: &[Case], graph: &Graph) -> Result<()> {
    let files: HashSet<&str> = graph.nodes.values().map(|n| n.file.as_str()).collect();
    for c in cases {
        for a in c.expect.anchors() {
            if !graph.nodes.contains_key(a) && !files.contains(a.as_str()) {
                anyhow::bail!("{cases_path}: {:?} expects {a:?}, which is neither a node id nor a file any node declares", c.q);
            }
        }
    }
    Ok(())
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
    // The floors a dense arm is graded against follow the store, not the configured default: a
    // store built under a model with no floors of its own is measured and not graded.
    let recorded = DenseIndex::recorded_model(&store)?;
    let floors = floors_for(recorded.as_deref());
    let questions = Questions::load(&store)?;
    // One build serves every case in the run, so the code list's cost is paid once regardless
    // of whether any case reranks — unlike a resident `Context`, there is nothing to save here.
    let lex = crate::index::lexical::Lexical::build(&graph, &questions, true);
    // The threshold decides the floors; the counts printed on the summary line stay exact.
    let (covered, eligible) = coverage(&graph, &questions);
    let enriched = enrich::enriched(covered, eligible);
    // Code questions are a configuration of their own and the summary line says so; the floors
    // read `enriched`, which counts documents alone.
    let (code_covered, code_eligible) = enrich::code_coverage(&graph, &questions);
    let code_note = if code_covered > 0 { format!(" code_questions={code_covered}/{code_eligible}") } else { String::new() };
    // `ask` degrading to lexical-only on a missing model is fine — a person reading the answer
    // sees the stderr notice and can judge it. `bench` speaks only through its exit code, so a
    // dense run that silently falls back and then grades against the weaker no-dense floor
    // would report green without ever having checked what it claims to check.
    let mut embedder = if no_dense {
        None
    } else {
        let model = crate::index::embed::resolve(recorded.as_deref(), &cfg.embed_model);
        match Embedder::open(&model) {
            Ok(e) => Some(e),
            Err(err) => anyhow::bail!("dense: model unavailable ({err:#})"),
        }
    };
    if !no_dense && dense_idx.ids.is_empty() {
        anyhow::bail!("dense index is empty at {} — run `repograph update` first", repo.display());
    }
    if let Some(e) = embedder.as_mut() {
        // A bench that silently searched 384-d queries against 1024-d rows would read empty
        // dense lists as a lexical-only run and grade it against the wrong floors.
        let width = e.dim()?;
        if dense_idx.dim > 0 && width != dense_idx.dim {
            anyhow::bail!("{}", crate::index::embed::width_mismatch(dense_idx.dim, e.name(), width));
        }
    }
    let dense_on = !no_dense;
    let embedder = std::cell::RefCell::new(embedder);
    let dense_fn = |q: &str, k: usize| -> (Vec<String>, Vec<String>) {
        let mut e = embedder.borrow_mut();
        match e.as_mut().and_then(|e| e.query(q).ok()) { Some(v) => dense_idx.search(&v, k), None => (Vec::new(), Vec::new()) }
    };
    let built_in = cases.is_none();
    let (cases_path, suite, text) = match cases {
        Some(p) => {
            let suite = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string());
            (p.display().to_string(), suite, std::fs::read_to_string(p).with_context(|| p.display().to_string())?)
        }
        None => ("built-in bench/cases.jsonl".to_string(), "built-in".to_string(), BUILT_IN_CASES.to_string()),
    };
    let cases: Vec<Case> = parse_cases(&text)?;
    if cases.is_empty() { anyhow::bail!("{cases_path} holds no cases"); }
    // The recorded case set is a constant, not an input: a truncated or edited copy of it would
    // otherwise still pass, since every floor is relative to whatever total showed up. Another
    // file is another suite — measured, printed, never graded against floors it did not earn.
    let shape_ok = is_recorded_shape(&cases);
    if built_in && !shape_ok {
        let got = shape(&cases).iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(" / ");
        anyhow::bail!("{cases_path} has {got} cases, expected 40 keyword / 30 paraphrase / 12 code");
    }
    // A dense arm under a model with no floors of its own is still worth running — the case
    // file's shape earned grading, the store's embedder just never measured any. `gated` says so
    // through the exit code rather than a bail, so the run still prints what it found.
    let gated = shape_ok && !(dense_on && floors == Floors::None);
    check_anchors(&cases_path, &cases, &graph)?;
    let is_id = |a: &str| graph.nodes.contains_key(a);
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
        let answer = query::ask(&graph, &ids, &lex, Some(&dense_fn), rerank, &words, &opts);
        let rendered = query::render(&answer, &graph, &opts);
        let tok = rendered.len() / 4;
        tokens.push(tok);
        let (reached, want) = found(case, &answer, &is_id);
        let ok = hit(case, &answer, &is_id);
        let slot = summary.slot(&case.kind);
        slot.1 += 1;
        if ok { slot.0 += 1; }
        println!("{:<10} {:<12} {} {reached}/{want} {:>4} tok  {}", case.kind, case.expect.key(), if ok { "HIT " } else { "miss" }, tok, case.q);
    }
    tokens.sort_unstable();
    summary.p90_tokens = tokens.get(tokens.len() * 9 / 10).copied().unwrap_or(0);
    let counts = summary.by_kind.iter().map(|(k, (h, t))| format!("{k} {h}/{t}")).collect::<Vec<_>>().join("  ");
    // Named after the store's rows, not the CLI's configured default, so a copy re-embedded under
    // another model reads its own name rather than borrowing the store it was copied from.
    let model_field = match floors {
        Floors::Small => "small".to_string(),
        Floors::Large => "large".to_string(),
        Floors::None => recorded.clone().unwrap_or_default(),
    };
    println!("\n{counts}  p90 {} tok  dense={dense_on}  enriched={enriched} ({covered}/{eligible} nodes) model={model_field}{code_note}{}  suite={suite} gated={gated}",
        summary.p90_tokens, match (rerank_local, rerank.is_some()) {
            (true, _) => format!(" rerank_local=true depth={depth}"),
            (false, true) => format!(" rerank=true depth={depth}"),
            _ => String::new(),
        });
    Ok(!gated || passes(&summary, dense_on, enriched, floors))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::Hit;

    fn h(id: &str, file: &str) -> Hit { Hit { id: id.into(), file: file.into(), line: 1, label: String::new(), score: 1.0, via: None } }

    fn case(kind: &str, expect: Expect) -> Case { Case { kind: kind.into(), q: String::new(), expect } }

    fn many(anchors: &[&str]) -> Expect { Expect::Many(anchors.iter().map(|a| a.to_string()).collect()) }

    // Node ids in these fixtures never contain a slash; paths always do.
    fn is_id(a: &str) -> bool { !a.contains('/') }

    #[test]
    fn hit_rules() {
        let a = Answer { seeds: vec![h("sym:x.ts::f", "packages/x.ts")], expanded: vec![h("FR-PAY-22", "d.md")] };
        assert!(hit(&case("paraphrase", "FR-PAY-22".into()), &a, &is_id));
        assert!(hit(&case("code", "packages/x.ts".into()), &a, &is_id));
        assert!(!hit(&case("keyword", "FR-PAY-23".into()), &a, &is_id));

        // A substring of a real id is not a match: ids compare whole-string, not `contains`.
        assert!(!hit(&case("keyword", "PAY-22".into()), &a, &is_id));
        // Ids compare verbatim, not case-folded.
        assert!(!hit(&case("keyword", "fr-pay-22".into()), &a, &is_id));

        // A code case's expected path is only a suffix of the hit's file: `file` compares
        // whole-string, not `ends_with`.
        let nested = Answer { seeds: vec![h("sym:packages/nested/x.ts::f", "packages/nested/x.ts")], expanded: vec![] };
        assert!(!hit(&case("code", "x.ts".into()), &nested, &is_id));
        // A path only reachable through `expanded` does not count: paths score `seeds` only.
        let expanded_only = Answer { seeds: vec![], expanded: vec![h("sym:packages/x.ts::f", "packages/x.ts")] };
        assert!(!hit(&case("code", "packages/x.ts".into()), &expanded_only, &is_id));
    }

    #[test]
    fn an_anchor_matches_on_the_axis_its_shape_names_and_never_on_the_other() {
        // The hit's id and its file are both present; an id anchor must not be satisfied by the
        // file and a path anchor must not be satisfied by the id, or a case would score on a
        // coincidence between two unrelated strings.
        let a = Answer { seeds: vec![h("packages/x.ts", "FR-PAY-22")], expanded: vec![] };
        assert!(!hit(&case("keyword", "FR-PAY-22".into()), &a, &is_id), "an id anchor read off the file field");
        assert!(!hit(&case("code", "packages/x.ts".into()), &a, &is_id), "a path anchor read off the id field");
    }

    #[test]
    fn a_case_with_several_anchors_reports_how_many_it_reached() {
        let a = Answer { seeds: vec![h("FR-PAY-20", "p.md"), h("sym:x.ts::f", "packages/x.ts")], expanded: vec![h("FR-PAY-22", "d.md")] };
        let c = case("multi", many(&["FR-PAY-20", "FR-PAY-22", "FR-PAY-99"]));
        assert_eq!(found(&c, &a, &is_id), (2, 3));
        assert!(hit(&c, &a, &is_id), "one anchor reached is an entry point");

        // Mixed shapes: the id counts from the expansion, the path only from the seeds.
        let cross = case("cross", many(&["FR-PAY-22", "packages/x.ts"]));
        assert_eq!(found(&cross, &a, &is_id), (2, 2));
        let expanded_path = Answer { seeds: vec![], expanded: vec![h("FR-PAY-22", "d.md"), h("sym:x.ts::f", "packages/x.ts")] };
        assert_eq!(found(&cross, &expanded_path, &is_id), (1, 2));

        // Nothing reached is a miss, whatever the anchor count.
        assert_eq!(found(&case("multi", many(&["FR-A-1", "FR-A-2"])), &a, &is_id), (0, 2));
        assert!(!hit(&case("multi", many(&["FR-A-1", "FR-A-2"])), &a, &is_id));
    }

    #[test]
    fn the_graph_decides_which_anchors_are_ids() {
        // A task id carries a slash and would otherwise read as a path; the graph knows better.
        let a = Answer { seeds: vec![], expanded: vec![h("BE-M01/T01", "docs/m.md")] };
        let graph_says_id = |x: &str| x == "BE-M01/T01";
        assert!(hit(&case("keyword", "BE-M01/T01".into()), &a, &graph_says_id));
        assert!(!hit(&case("keyword", "BE-M01/T01".into()), &a, &is_id), "as a path it is looked for among seed files");
    }

    #[test]
    fn expect_parses_as_one_string_or_a_list() {
        let one: Case = serde_json::from_str(r#"{"kind":"keyword","q":"a","expect":"FR-X-1"}"#).unwrap();
        assert_eq!(one.expect.anchors(), ["FR-X-1"]);
        assert_eq!(one.expect.key(), "FR-X-1");
        let several: Case = serde_json::from_str(r#"{"kind":"multi","q":"a","expect":["FR-X-1","packages/x.ts"]}"#).unwrap();
        assert_eq!(several.expect.anchors(), ["FR-X-1", "packages/x.ts"]);
        assert_eq!(several.expect.key(), "FR-X-1+packages/x.ts");
        assert!(serde_json::from_str::<Case>(r#"{"kind":"multi","q":"a","expect":7}"#).is_err());
    }

    #[test]
    fn only_the_recorded_shape_is_gated() {
        let mut recorded = Vec::new();
        for (kind, n) in RECORDED_SHAPE { for _ in 0..n { recorded.push(case(kind, "X".into())); } }
        assert!(is_recorded_shape(&recorded));
        // Order of kinds in the file does not matter, the counts do.
        recorded.reverse();
        assert!(is_recorded_shape(&recorded));

        let mut short = recorded.iter().map(|c| case(&c.kind, "X".into())).collect::<Vec<_>>();
        short.pop();
        assert!(!is_recorded_shape(&short), "one case short of the recorded shape");
        let mut extra_kind = recorded.iter().map(|c| case(&c.kind, "X".into())).collect::<Vec<_>>();
        extra_kind.push(case("long", "X".into()));
        assert!(!is_recorded_shape(&extra_kind), "a fourth kind is another suite");
        assert!(!is_recorded_shape(&[case("long", "X".into()), case("cross", "X".into())]));
    }

    #[test]
    fn the_shape_keeps_the_order_the_file_introduced() {
        let cases = [case("long", "X".into()), case("cross", "X".into()), case("long", "X".into())];
        assert_eq!(shape(&cases), [("long".to_string(), 2), ("cross".to_string(), 1)]);
    }

    #[test]
    fn a_summary_counts_kinds_in_file_order_and_reads_absent_kinds_as_zero() {
        let s = Summary::default().with("long", (3, 5)).with("cross", (1, 2));
        assert_eq!(s.by_kind, [("long".to_string(), (3, 5)), ("cross".to_string(), (1, 2))]);
        assert_eq!(s.kind("cross"), (1, 2));
        assert_eq!(s.kind("keyword"), (0, 0));
        // `recorded` is the three graded kinds in their fixed order.
        assert_eq!(Summary::recorded((40, 40), (14, 30), (12, 12), 220).kind("paraphrase"), (14, 30));
    }

    #[test]
    fn floors() {
        // Fixtures sit exactly on each floor so a boundary shifted by one in either direction
        // reddens the corresponding call; a fixture comfortably clear of the floor (the
        // original mistake) would not notice such a shift.
        let at_floor_nodense = Summary::recorded((39, 40), (11, 30), (12, 12), 230);
        let at_floor_dense = Summary::recorded((40, 40), (14, 30), (12, 12), 230);
        assert!(passes(&at_floor_nodense, false, true, Floors::Small));
        assert!(passes(&at_floor_dense, true, true, Floors::Small));

        // keyword: one short of its floor reddens either arm — 38 without embeddings, 39 with.
        assert!(!passes(&at_floor_nodense.clone().with("keyword", (38, 40)), false, true, Floors::Small));
        assert!(!passes(&at_floor_dense.clone().with("keyword", (39, 40)), true, true, Floors::Small));

        // code must be exact: one short reddens in both dense arms.
        assert!(!passes(&at_floor_nodense.clone().with("code", (11, 12)), false, true, Floors::Small));
        assert!(!passes(&at_floor_dense.clone().with("code", (11, 12)), true, true, Floors::Small));

        // p90: one token over the shared ceiling reddens either arm.
        assert!(!passes(&Summary { p90_tokens: 231, ..at_floor_nodense.clone() }, false, true, Floors::Small));
        assert!(!passes(&Summary { p90_tokens: 231, ..at_floor_dense.clone() }, true, true, Floors::Small));

        // paraphrase no-dense floor is 11: one short reddens the `dense: false` call.
        assert!(!passes(&at_floor_nodense.clone().with("paraphrase", (10, 30)), false, true, Floors::Small));
        // paraphrase dense floor is 14: one short reddens the `dense: true` call.
        assert!(!passes(&at_floor_dense.clone().with("paraphrase", (13, 30)), true, true, Floors::Small));

        // A summary of another suite has none of the graded kinds and reads as every floor
        // missed, which is why `run` never grades one.
        assert!(!passes(&Summary::default().with("long", (15, 15)), true, true, Floors::Small));

        // The large model's dense arms are graded on their own measured numbers (the 0.5.0 gap
        // results, L3); its lexical arms are the small model's, because no embedder is in them.
        let large_enriched = Summary::recorded((40, 40), (22, 30), (12, 12), 230);
        assert!(passes(&large_enriched, true, true, Floors::Large));
        assert!(!passes(&large_enriched.clone().with("paraphrase", (21, 30)), true, true, Floors::Large));
        assert!(passes(&large_enriched, true, true, Floors::Small), "the small floors are the lower bar and the large store clears them, which is what made them the wrong bar");
        assert!(passes(&at_floor_nodense, false, true, Floors::Large), "lexical arms do not read the model");
        assert!(!passes(&at_floor_nodense.clone().with("keyword", (38, 40)), false, true, Floors::Large));
        // A model with no floors of its own is measured and never graded.
        assert!(!passes(&large_enriched, true, true, Floors::None));
        // The large model's raw store (no questions paid for) measures its own floor too
        // ($G/t5-large-raw-1.txt).
        let large_raw = Summary::recorded((40, 40), (17, 30), (12, 12), 230);
        assert!(passes(&large_raw, true, false, Floors::Large));
        assert!(!passes(&large_raw.clone().with("paraphrase", (16, 30)), true, false, Floors::Large));
    }

    #[test]
    fn a_store_without_questions_is_graded_on_the_numbers_it_reads() {
        // Exactly what a store `enrich` has never touched measures on the corpus, so a shift of
        // one in either direction is visible here.
        let raw_dense = Summary::recorded((40, 40), (9, 30), (12, 12), 221);
        let raw_nodense = Summary::recorded((39, 40), (7, 30), (12, 12), 226);
        assert!(passes(&raw_dense, true, false, Floors::Small));
        assert!(passes(&raw_nodense, false, false, Floors::Small));

        // The same run against a store that paid for its questions is a failure, which is what
        // keeps the enriched bar a bar.
        assert!(!passes(&raw_dense, true, true, Floors::Small));
        assert!(!passes(&raw_nodense, false, true, Floors::Small));

        // One short of each raw floor reddens, the `--no-dense` keyword floor of 39 included.
        assert!(!passes(&raw_dense.clone().with("paraphrase", (8, 30)), true, false, Floors::Small));
        assert!(!passes(&raw_nodense.clone().with("paraphrase", (6, 30)), false, false, Floors::Small));
        assert!(!passes(&raw_dense.clone().with("keyword", (39, 40)), true, false, Floors::Small));
        assert!(!passes(&raw_nodense.clone().with("keyword", (38, 40)), false, false, Floors::Small));
        assert!(!passes(&raw_dense.clone().with("code", (11, 12)), true, false, Floors::Small));
        assert!(!passes(&Summary { p90_tokens: 231, ..raw_nodense.clone() }, false, false, Floors::Small));
    }

    #[test]
    fn the_floors_follow_the_model_the_rows_were_written_by() {
        use crate::index::embed::{DEFAULT_MODEL, UNNAMED_MODEL};
        assert_eq!(floors_for(None), Floors::Small, "a store with no vectors is graded lexically, on floors the model never enters");
        assert_eq!(floors_for(Some(UNNAMED_MODEL)), Floors::Small);
        assert_eq!(floors_for(Some(DEFAULT_MODEL)), Floors::Large);
        assert_eq!(floors_for(Some("BAAI/bge-m3")), Floors::None);
    }

    #[test]
    fn cases_file_parses_and_has_the_recorded_shape() {
        let cases = parse_cases(BUILT_IN_CASES).unwrap();
        assert!(is_recorded_shape(&cases));
        assert!(cases.iter().all(|c| c.expect.anchors().len() == 1), "every recorded case expects one place");
    }

    #[test]
    fn keyword_hit_counts_a_seed_or_an_expanded_entry() {
        let a = Answer { seeds: vec![h("FR-PAY-22", "d.md")], expanded: vec![h("FR-PAY-20", "e.md")] };
        assert!(hit(&case("keyword", "FR-PAY-22".into()), &a, &is_id));
        assert!(hit(&case("keyword", "FR-PAY-20".into()), &a, &is_id));
    }

    #[test]
    fn paraphrase_hit_counts_a_seed_or_an_expanded_entry() {
        let a = Answer { seeds: vec![h("FR-PAY-22", "d.md")], expanded: vec![h("FR-PAY-20", "e.md")] };
        assert!(hit(&case("paraphrase", "FR-PAY-22".into()), &a, &is_id));
        assert!(hit(&case("paraphrase", "FR-PAY-20".into()), &a, &is_id));
    }

    #[test]
    fn code_hit_only_counts_a_seed_never_an_expanded_entry() {
        let a = Answer { seeds: vec![h("sym:x.ts::f", "packages/x.ts")], expanded: vec![h("sym:y.ts::g", "packages/y.ts")] };
        assert!(hit(&case("code", "packages/x.ts".into()), &a, &is_id));
        assert!(!hit(&case("code", "packages/y.ts".into()), &a, &is_id));
    }

    #[test]
    fn anchors_are_checked_against_the_graph_before_anything_is_asked() {
        use crate::model::{Node, NodeKind};
        let mut graph = Graph::default();
        let node = |id: &str, file: &str| Node {
            id: id.into(), kind: NodeKind::Requirement, label: String::new(), body: String::new(), file: file.into(), line: 1, end: 0,
            files: ["docs/y.md".to_string()].into_iter().collect(), community: None,
        };
        graph.nodes.insert("FR-X-1".into(), node("FR-X-1", "docs/x.md"));
        graph.nodes.insert("sym:a".into(), node("sym:a", "packages/a.ts"));
        let good = [case("cross", many(&["FR-X-1", "packages/a.ts"])), case("long", "FR-X-1".into())];
        assert!(check_anchors("f", &good, &graph).is_ok());
        let bad_id = [case("long", "FR-X-2".into())];
        let err = check_anchors("f", &bad_id, &graph).unwrap_err().to_string();
        assert!(err.contains("FR-X-2"), "{err}");
        let bad_path = [case("where", "packages/b.ts".into())];
        assert!(check_anchors("f", &bad_path, &graph).is_err());
        // A file some node lists as a secondary location is not the file a hit reports.
        let also_listed = [case("where", "docs/y.md".into())];
        assert!(check_anchors("f", &also_listed, &graph).is_err());
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
