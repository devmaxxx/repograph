use crate::enrich::Questions;
use crate::ids::IdMatcher;
use crate::index::{fuse, lexical::LexicalIndex};
use crate::model::{EdgeKind, Graph, NodeKind};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// `depth`: how many fused candidates a reranking model is shown; each retriever runs that deep.
pub struct Options { pub seeds: usize, pub bodies: bool, pub dense: bool, pub json: bool, pub depth: usize }

/// Dense retrieval for a question: the passage-row list and the question-row list, each `k` deep.
pub type Dense<'a> = &'a dyn Fn(&str, usize) -> (Vec<String>, Vec<String>);

/// Picks seeds for a question from `(id, label)` candidates, best first.
pub type Rerank<'a> = &'a dyn Fn(&str, &[(String, String)]) -> Vec<String>;

#[derive(Debug, Clone, Serialize)]
pub struct Hit { pub id: String, pub file: String, pub line: u32, pub label: String, pub score: f32, pub via: Option<String> }

#[derive(Debug, Default, Serialize)]
pub struct Answer { pub seeds: Vec<Hit>, pub expanded: Vec<Hit> }

const EXPAND: [EdgeKind; 5] = [EdgeKind::References, EdgeKind::Implements, EdgeKind::Declares, EdgeKind::Links, EdgeKind::Legacy];
// Stays at 1: raising it to 8 buys at most one extra paraphrase hit and takes
// p90 from 218 to 510 tokens, which bench's p90 floor exists to catch.
const MAX_EXPANDED: usize = 1;
/// Fused seeds kept ahead of the reranking model's picks. Zero since the model sees a snippet
/// of each candidate: shown titles only it dropped a keyword hit the retrievers had ranked
/// first, so two seeds were pinned; shown text, the pins were the retrievers' guess taking two
/// of the model's five slots, and unpinning them is what took paraphrase from 13/14 to 14/14.
const PINNED: usize = 0;
/// The generated-questions BM25 list joins the plain-path fusion only when its best score is at
/// least this fraction of the passage list's best. Below it, on 400 held-out questions, the
/// passage list is the one holding the answer (30% in its top five against 21%) and dropping
/// the questions list costs nothing measurable; above it the questions list is the better
/// retriever (24% against 12%). The held-out set never binds the value — exact McNemar against
/// the ungated fusion is p = 0.34 to 0.79 at every value from 0.80 to 0.95 — so the window is
/// set by the 82 recorded cases alone: at 0.80 the keyword case `FR-WH-53` (ratio 0.801) loses
/// its seat and the lexical arm reddens, and at 0.87 a paraphrase case (ratio 0.867) loses its
/// list and the arm with embeddings reads 13/30 under its floor of 14. Ten of the thirty
/// paraphrase ratios sit in (0.85, 0.90), so the window's upper edge is the foot of the very
/// population the gate admits. The window's centre, 0.83, was measured too: identical on the 82
/// in all four arms, but the lexical held-out arm loses three more questions to it (109 → 106,
/// none gained) — a list admitted at a ratio just under 0.85 came in without the answer and
/// took seats. So the value stays at 0.85, with 0.05 of room below and 0.006 above; the nearest
/// paraphrase ratio is 0.856, and any change to the store's questions moves every ratio.
///
/// It is a constant of this store, not of BM25. The two indices share the tokenizer, the
/// formula and the document count, but each normalises length against its own mean (30.6
/// tokens a passage, 51.8 a question document) and weights terms by its own vocabulary (10.8k
/// against 21.8k), so the ratio moves with enrichment coverage and questions per node. A
/// scale-free form — each list's best against its own k-th — is gap G8, not tried.
const QUESTIONS_GATE: f32 = 0.85;

/// `REPOGRAPH_QUESTIONS_GATE` overrides the constant for a measurement and for nothing else:
/// `0` reproduces the ungated fusion that ADR-001 Amendment 6's before-column was read against,
/// which no commit's binary otherwise produces, and any other value re-reads the window above.
fn questions_gate() -> f32 {
    gate_from(std::env::var("REPOGRAPH_QUESTIONS_GATE").ok().as_deref())
}

fn gate_from(override_: Option<&str>) -> f32 {
    override_.and_then(|v| v.trim().parse().ok()).unwrap_or(QUESTIONS_GATE)
}

/// The lexical lists for one question, in fusion order. A store `enrich` never touched has one:
/// an index of id-only documents is shorter than the passages and ranks an id-bearing term above
/// the passage that carries it, so on a raw store the questions list would clear any gate for the
/// wrong reason and cost a build per question to do it. On the reranked path both lists are
/// admitted unconditionally: the fused order there is a candidate pool `depth` deep rather than
/// five seats, so a list there costs the reranking model candidates and not seeds, and Amendment 2
/// measured the questions list as what carries paraphrase targets into that pool. The questions
/// about code are a third list on the reranked path and on no other: the plain fusion's five seats
/// were measured to be worth more to the documents than to them.
fn lexical_lists(graph: &Graph, questions: &Questions, query: &str, depth: usize, reranked: bool) -> Vec<Vec<String>> {
    let only_ids = |scored: Vec<(String, f32)>| -> Vec<String> { scored.into_iter().map(|(id, _)| id).collect() };
    let passages = LexicalIndex::build(graph).search(query, depth);
    if questions.entries.is_empty() { return vec![only_ids(passages)]; }
    let generated = LexicalIndex::build_questions(graph, questions).search(query, depth);
    if reranked {
        let mut pool = vec![only_ids(passages), only_ids(generated)];
        // Not on the plain path. Given a seat there instead — one, on the same gate — the code
        // questions read `where` 0/9 → 2/9 on the developer suite but held-out 103 → 97 and
        // 109 → 103, 0 gained and 6 lost in each arm, p = 0.031 (2026-09-05): five seats are the
        // budget the floors were set on, and a seat given to code is a document question's answer
        // lost. Here the pool is `--depth` deep (200 by default) rather than five seats, so the
        // list costs the reranking model candidates and not seeds; what it is worth to that
        // model is unmeasured.
        let code = LexicalIndex::build_code_questions(graph, questions).search(query, depth);
        if !code.is_empty() { pool.push(only_ids(code)); }
        return pool;
    }
    let best = |l: &[(String, f32)]| l.first().map(|(_, s)| *s).unwrap_or(0.0);
    let floor = questions_gate() * best(&passages);
    let admitted = |l: &[(String, f32)]| best(l) >= floor && best(l) > 0.0;
    let mut lists = Vec::with_capacity(2);
    if admitted(&generated) { lists.push(only_ids(generated)); }
    lists.push(only_ids(passages));
    lists
}

fn hit(graph: &Graph, id: &str, score: f32, via: Option<&str>) -> Option<Hit> {
    let n = graph.nodes.get(id)?;
    Some(Hit { id: n.id.clone(), file: n.file.clone(), line: n.line, label: n.label.clone(), score, via: via.map(str::to_string) })
}

/// Exact hits per word, and whether they answer the whole question: every word matched, and
/// each carries an uppercase letter — `money` and `utf8` are topics as much as names, `asGrosze` only a name.
pub(crate) fn exact_seeds(graph: &Graph, ids: &IdMatcher, words: &[String]) -> (Vec<String>, bool) {
    let mut out = Vec::new();
    let mut whole = true;
    for w in words {
        if ids.is_id(w) && graph.nodes.contains_key(w) {
            out.push(w.clone());
            continue;
        }
        let tail = format!("::{w}");
        let mut syms: Vec<&String> = graph.nodes.values()
            .filter(|n| n.kind == NodeKind::Symbol && (n.label == *w || n.id.ends_with(&tail)))
            .map(|n| &n.id).collect();
        syms.sort();
        whole &= !syms.is_empty() && w.chars().any(char::is_uppercase);
        out.extend(syms.into_iter().cloned());
    }
    let mut seen = BTreeSet::new();
    out.retain(|id| seen.insert(id.clone()));
    let whole = whole && !out.is_empty();
    (out, whole)
}

pub fn ask(graph: &Graph, ids: &IdMatcher, questions: &Questions, dense: Option<Dense>, rerank: Option<Rerank>, words: &[String], opts: &Options) -> Answer {
    let query = words.join(" ");
    let mut answer = Answer::default();
    let (exact, whole_question) = exact_seeds(graph, ids, words);
    let mut ranked: HashMap<String, usize> = HashMap::new();
    // A name duplicated across generated clients (packages/contracts/src/generated/**) must not
    // be able to spend the whole answer budget on itself.
    for id in exact.iter().take(opts.seeds) {
        if let Some(h) = hit(graph, id, 1.0, None) { answer.seeds.push(h); }
    }
    // Topping an exact answer up from fusion only appends neighbours nobody asked for (an id
    // lookup measured 174 tokens with them, 68 without).
    if !whole_question {
        let depth = if rerank.is_some() { opts.depth } else { 20 };
        // Dense passages go first: they are the retriever the paraphrase floor rests on, so they
        // get the odd seed. The BM25 list over the generated questions comes before the one over
        // the passages: on 400 held-out generated questions it lifted recall@5 from 0.445 to
        // 0.515 beside the dense list and from 0.395 to 0.527 without it. Pooled into the
        // passage rows instead it buries targets (a passage at rank 2 fell to 87), so it stays a
        // list of its own. The dense rows over the generated questions add nothing at five seeds
        // and only feed the reranker's pool.
        //
        // On a keyword-shaped question the questions list has little to say — over the forty
        // recorded keyword cases it holds the answer in its top five eight times and lacks it
        // outright ten — yet an equal turn in the round-robin hands it half of five seeds, and
        // the exact passage row goes past the cut. Thinning its turns for every question was
        // measured and rejected (gap G7): on held-out paraphrases it is the retriever doing the
        // work. So it is admitted per question, on how strongly it matched against how strongly
        // the passages did — `QUESTIONS_GATE`, and `lexical_lists` for what the two paths do
        // with it. The raw arms are untouched by construction: a store without questions gets
        // no questions list built at all.
        let mut lists: Vec<Vec<String>> = Vec::new();
        if opts.dense {
            if let Some(d) = dense {
                let (passages, questions_rows) = d(&query, depth);
                lists.push(passages);
                if rerank.is_some() { lists.push(questions_rows); }
            }
        }
        lists.extend(lexical_lists(graph, questions, &query, depth, rerank.is_some()));
        lists.retain(|l| !l.is_empty());
        let mut fused = fuse::interleave(&lists);
        if let Some(r) = rerank {
            let candidates: Vec<(String, String)> = fused.iter().take(depth)
                .map(|(id, _)| (id.clone(), crate::rerank::text(&graph.nodes[id]))).collect();
            let mut order: Vec<String> = candidates.iter().take(PINNED).map(|(id, _)| id.clone()).collect();
            for id in r(&query, &candidates) {
                if !order.contains(&id) { order.push(id); }
            }
            let rest: Vec<(String, f32)> = fused.into_iter().filter(|(id, _)| !order.contains(id)).collect();
            fused = order.iter().enumerate().map(|(rank, id)| (id.clone(), 1.0 / (rank as f32 + 1.0))).chain(rest).collect();
        }
        for (rank, (id, score)) in fused.into_iter().enumerate() {
            if answer.seeds.len() < opts.seeds && !exact.contains(&id) {
                if let Some(h) = hit(graph, &id, score, None) { answer.seeds.push(h); }
            }
            ranked.entry(id).or_insert(rank);
        }
    }

    // The expanded line goes to the seed neighbour the retrievers ranked best, however far
    // down; a neighbour no retriever ranked falls back to its seed's rank. Measured on 400
    // held-out generated questions: 208 → 226 hits over the seed's rank alone (one lost,
    // nineteen gained), 7/14 → 8/14 on the paraphrase cases, same one line of output.
    let seed_ids: Vec<String> = answer.seeds.iter().map(|h| h.id.clone()).collect();
    let mut candidates: Vec<(Option<usize>, f32, &str, &str)> = Vec::new();
    for seed in &answer.seeds {
        for e in graph.neighbours(&seed.id) {
            if !EXPAND.contains(&e.kind) { continue; }
            let other = if e.source == seed.id { &e.target } else { &e.source };
            if seed_ids.contains(other) || other.starts_with("file:") || other.starts_with("deco:") { continue; }
            if candidates.iter().any(|c| c.2 == other) { continue; }
            candidates.push((ranked.get(other).copied(), seed.score * 0.5, other, &seed.id));
        }
    }
    candidates.sort_by(|a, b| {
        a.0.is_none().cmp(&b.0.is_none()).then(a.0.cmp(&b.0)).then(b.1.partial_cmp(&a.1).unwrap()).then(a.2.cmp(b.2))
    });
    answer.expanded = candidates.iter().take(MAX_EXPANDED).filter_map(|(rank, fallback, id, via)| {
        let score = rank.map_or(*fallback, |r| 1.0 / (r as f32 + 1.0));
        hit(graph, id, score, Some(via))
    }).collect();
    answer
}

fn headline(label: &str) -> String {
    let mut s: String = label.chars().take(80).collect();
    if label.chars().count() > 80 { s.push('…'); }
    s
}

pub fn render(answer: &Answer, graph: &Graph, opts: &Options) -> String {
    if opts.json {
        return serde_json::to_string_pretty(answer).unwrap() + "\n";
    }
    let mut out = String::new();
    for h in &answer.seeds {
        out.push_str(&format!("{}  {}:{}  {}\n", h.id, h.file, h.line, headline(&h.label)));
        if opts.bodies {
            if let Some(n) = graph.nodes.get(&h.id) {
                for l in n.body.lines() { out.push_str(&format!("    {l}\n")); }
            }
        }
    }
    for h in &answer.expanded {
        out.push_str(&format!("  {}  {}:{}  {}  ← {}\n", h.id, h.file, h.line, headline(&h.label), h.via.as_deref().unwrap_or("")));
    }
    out
}

pub(crate) fn resolve<'a>(graph: &'a Graph, needle: &str) -> Option<&'a crate::model::Node> {
    if let Some(n) = graph.nodes.get(needle) { return Some(n); }
    let tail = format!("::{needle}");
    let mut c: Vec<&crate::model::Node> = graph.nodes.values().filter(|n| n.id.ends_with(&tail)).collect();
    if c.is_empty() {
        let lower = needle.to_lowercase();
        c = graph.nodes.values().filter(|n| n.label.to_lowercase() == lower).collect();
    }
    c.sort_by(|a, b| a.id.cmp(&b.id));
    c.into_iter().next()
}

pub fn explain(graph: &Graph, needle: &str) -> Option<String> {
    let n = resolve(graph, needle)?;
    let mut out = format!("{}  {}:{}  {:?}  {}\n", n.id, n.file, n.line, n.kind, headline(&n.label));
    if let Some(c) = &n.community { out.push_str(&format!("  community: {c}\n")); }
    let mut edges = graph.neighbours(&n.id);
    edges.sort_by_key(|e| (e.kind == EdgeKind::Legacy, e.kind, e.source.clone(), e.target.clone()));
    for e in edges {
        let (arrow, other) = if e.source == n.id { ("→", &e.target) } else { ("←", &e.source) };
        let ctx = if e.context.is_empty() { String::new() } else { format!("  [{}]", e.context) };
        out.push_str(&format!("  {:?} {arrow} {other}{ctx}\n", e.kind));
    }
    Some(out)
}

pub fn verify(graph: &Graph) -> String {
    let mut nodes: BTreeMap<String, usize> = BTreeMap::new();
    for n in graph.nodes.values() { *nodes.entry(format!("{:?}", n.kind)).or_default() += 1; }
    let mut edges: BTreeMap<String, usize> = BTreeMap::new();
    for e in &graph.edges { *edges.entry(format!("{:?}", e.kind)).or_default() += 1; }
    let dangling = graph.dangling();
    let mut undeclared: Vec<&str> = dangling.iter()
        .filter(|e| !e.target.contains(':'))
        .map(|e| e.target.as_str()).collect();
    undeclared.sort();
    undeclared.dedup();
    // A family that is never declared anywhere (task ids cited from code, say) is a corpus
    // convention, not a broken link; only a gap inside a declared family is worth chasing.
    let declared: BTreeSet<&str> = graph.nodes.keys().filter(|id| !id.contains(':')).map(|id| family(id)).collect();
    let (gaps, cite_only): (Vec<&str>, Vec<&str>) = undeclared.iter().partition(|id| declared.contains(family(id)));
    let cite_only_families: BTreeSet<&str> = cite_only.iter().map(|id| family(id)).collect();
    let sample = |ids: &[&str]| ids.iter().take(10).cloned().collect::<Vec<_>>().join(" ");
    let mut out = String::new();
    out.push_str(&format!("nodes: {}  {:?}\n", graph.nodes.len(), nodes));
    out.push_str(&format!("edges: {}  {:?}\n", graph.edges.len(), edges));
    out.push_str(&format!("dangling edges: {}\n", dangling.len()));
    out.push_str(&format!("undeclared ids: {}  {}\n", undeclared.len(), sample(&undeclared)));
    out.push_str(&format!("  gaps in declared families: {}  {}\n", gaps.len(), sample(&gaps)));
    out.push_str(&format!("  in families never declared: {}  {}\n", cite_only.len(),
        cite_only_families.iter().cloned().collect::<Vec<_>>().join(" ")));
    out
}

fn family(id: &str) -> &str {
    id.trim_end_matches(|c: char| c.is_ascii_digit() || c == '-' || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EdgeKind, Extraction, NodeKind};

    fn graph() -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "`CancellationPolicy` — правило отмены с числами", "штраф считается по политике отмены (`N-151`)", "docs/06.md", 385);
        e.node(NodeKind::Requirement, "FR-PAY-20", "отмена записи клиентом", "клиент отменяет запись", "docs/06.md", 300);
        e.node(NodeKind::Requirement, "N-151", "never: free-text policies", "", "docs/never.md", 12);
        e.node(NodeKind::Entity, "entity:CancellationPolicy", "CancellationPolicy", "", "docs/06.md", 385);
        e.node(NodeKind::Symbol, "sym:packages/contracts/src/money.ts::asGrosze", "asGrosze", "export function asGrosze()", "packages/contracts/src/money.ts", 2);
        e.node(NodeKind::Symbol, "sym:packages/domain/test/money.spec.ts::money", "money", "function money()", "packages/domain/test/money.spec.ts", 50);
        e.node(NodeKind::File, "file:docs/06.md", "docs/06.md", "", "docs/06.md", 1);
        e.edge("FR-PAY-22", "N-151", EdgeKind::References, "body", "docs/06.md");
        e.edge("FR-PAY-22", "entity:CancellationPolicy", EdgeKind::References, "title", "docs/06.md");
        e.edge("file:docs/06.md", "FR-PAY-22", EdgeKind::Declares, "", "docs/06.md");
        e.edge("file:docs/06.md", "FR-PAY-20", EdgeKind::Declares, "", "docs/06.md");
        g.apply(e);
        g
    }

    // A seed with exactly one expandable neighbour, so a cap of 1 cannot hide it.
    fn single_neighbour_graph() -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "`CancellationPolicy` rule", "policy body", "docs/06.md", 385);
        e.node(NodeKind::Entity, "entity:CancellationPolicy", "CancellationPolicy", "", "docs/06.md", 385);
        e.edge("FR-PAY-22", "entity:CancellationPolicy", EdgeKind::References, "title", "docs/06.md");
        g.apply(e);
        g
    }

    // The same symbol name declared under four generated-client flavours, as it is in
    // packages/contracts/src/generated/** — nothing here caps how many exact matches exist.
    fn duplicated_symbol_graph() -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        for flavour in ["a", "b", "c", "d"] {
            let file = format!("packages/contracts/src/generated/{flavour}/client.ts");
            e.node(NodeKind::Symbol, &format!("sym:{file}::buildClientParams"), "buildClientParams", "", &file, 1);
        }
        g.apply(e);
        g
    }

    // A seed whose only expandable neighbour is a File hub, to pin that hubs are never expanded to.
    fn file_hub_only_graph() -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-X", "only points at a file hub", "", "docs/x.md", 1);
        e.node(NodeKind::File, "file:docs/x.md", "docs/x.md", "", "docs/x.md", 1);
        e.edge("FR-X", "file:docs/x.md", EdgeKind::References, "", "docs/x.md");
        g.apply(e);
        g
    }

    fn ids() -> IdMatcher {
        let cfg = crate::config::Config::default();
        IdMatcher::new(&cfg.id_families, &cfg.milestone_families)
    }

    fn opts() -> Options { Options { seeds: 5, bodies: false, dense: false, json: false, depth: crate::rerank::DEPTH } }

    #[test]
    fn exact_id_wins_and_expands_one_hop() {
        let g = graph();
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["FR-PAY-22".to_string()], &opts());
        assert_eq!(a.seeds[0].id, "FR-PAY-22");
        assert_eq!(a.seeds[0].score, 1.0);
        assert_eq!(a.expanded.len(), 1);
        assert_eq!(a.expanded[0].id, "N-151");
        assert_eq!(a.expanded[0].via.as_deref(), Some("FR-PAY-22"));
        let ex: Vec<&str> = a.expanded.iter().map(|h| h.id.as_str()).collect();
        assert!(!ex.contains(&"file:docs/06.md"));
    }

    #[test]
    fn a_lowercase_word_that_is_also_a_symbol_leads_but_still_fuses() {
        let g = graph();
        let dense = |_: &str, _: usize| (vec!["FR-PAY-20".to_string()], Vec::new());
        let a = ask(&g, &ids(), &Questions::default(), Some(&dense), None, &["money".to_string()], &Options { dense: true, ..opts() });
        assert_eq!(a.seeds[0].id, "sym:packages/domain/test/money.spec.ts::money");
        assert!(a.seeds.iter().any(|h| h.id == "FR-PAY-20"));
    }

    #[test]
    fn a_code_shaped_name_is_the_whole_question() {
        let g = graph();
        let dense = |_: &str, _: usize| (vec!["FR-PAY-20".to_string()], Vec::new());
        let a = ask(&g, &ids(), &Questions::default(), Some(&dense), None, &["asGrosze".to_string()], &Options { dense: true, ..opts() });
        assert_eq!(a.seeds.len(), 1);
        assert_eq!(a.seeds[0].id, "sym:packages/contracts/src/money.ts::asGrosze");
    }

    #[test]
    fn a_symbol_asked_twice_is_one_seed() {
        let g = graph();
        let words: Vec<String> = ["asGrosze", "foo", "asGrosze"].iter().map(|s| s.to_string()).collect();
        let a = ask(&g, &ids(), &Questions::default(), None, None, &words, &opts());
        assert_eq!(a.seeds.iter().filter(|h| h.id.ends_with("::asGrosze")).count(), 1);
    }

    #[test]
    fn exact_match_leaves_the_other_seed_slots_empty() {
        let g = graph();
        // "N-151" is also a lexical hit on FR-PAY-22's body; it must arrive by expansion, not as a seed.
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["N-151".to_string()], &opts());
        assert_eq!(a.seeds.len(), 1);
        assert_eq!(a.seeds[0].id, "N-151");
        assert_eq!(a.expanded[0].id, "FR-PAY-22");
    }

    #[test]
    fn backticked_entity_is_reachable_by_expansion() {
        let g = single_neighbour_graph();
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["FR-PAY-22".to_string()], &opts());
        assert_eq!(a.expanded.len(), 1);
        assert_eq!(a.expanded[0].id, "entity:CancellationPolicy");
        assert_eq!(a.expanded[0].via.as_deref(), Some("FR-PAY-22"));
    }

    #[test]
    fn exact_seeds_are_capped_at_options_seeds() {
        let g = duplicated_symbol_graph();
        let mut o = opts();
        o.seeds = 2;
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["buildClientParams".to_string()], &o);
        assert_eq!(a.seeds.len(), 2);
        let ids: Vec<&str> = a.seeds.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, vec![
            "sym:packages/contracts/src/generated/a/client.ts::buildClientParams",
            "sym:packages/contracts/src/generated/b/client.ts::buildClientParams",
        ]);
    }

    #[test]
    fn file_hub_is_never_expanded_to() {
        let g = file_hub_only_graph();
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["FR-X".to_string()], &opts());
        assert!(a.expanded.is_empty());
    }

    #[test]
    fn exact_symbol_name_resolves_to_its_file() {
        let g = graph();
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["asGrosze".to_string()], &opts());
        assert_eq!(a.seeds[0].file, "packages/contracts/src/money.ts");
    }

    #[test]
    fn lexical_query_in_russian_finds_the_requirement() {
        let g = graph();
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["политика".into(), "отмены".into(), "штраф".into()], &opts());
        assert_eq!(a.seeds[0].id, "FR-PAY-22");
    }

    #[test]
    fn the_dense_top_hit_takes_the_first_seed_and_its_second_hit_the_third() {
        let g = graph();
        // Lexical alone ranks FR-PAY-22 first for "штраф"; dense disagrees on both of its slots.
        let dense = |_: &str, _: usize| (vec!["N-151".to_string(), "FR-PAY-20".to_string()], Vec::new());
        let a = ask(&g, &ids(), &Questions::default(), Some(&dense), None, &["штраф".to_string()], &Options { dense: true, ..opts() });
        let order: Vec<&str> = a.seeds.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(&order[..3], ["N-151", "FR-PAY-22", "FR-PAY-20"]);
    }

    fn questions() -> Questions {
        let mut qs = Questions::default();
        qs.entries.insert("FR-PAY-20".into(), crate::enrich::Entry { hash: String::new(), questions: vec!["можно ли аннулировать бронь самому".into()] });
        qs
    }

    #[test]
    fn a_store_without_questions_gets_one_lexical_list_on_either_path() {
        // The old guard was `!entries.is_empty()`; without it a raw store builds an index of
        // id-only documents, and "FR-PAY-22" scores higher there than in the passage that
        // carries it, so the list would clear the gate on the strength of its own shortness.
        let g = graph();
        let none = Questions::default();
        for reranked in [false, true] {
            let lists = lexical_lists(&g, &none, "FR-PAY-22 штраф", 10, reranked);
            assert_eq!(lists.len(), 1, "reranked={reranked}: {lists:?}");
            assert_eq!(lists[0][0], "FR-PAY-22");
        }
    }

    #[test]
    fn the_questions_list_leads_when_admitted_and_is_absent_when_not() {
        let g = graph();
        let mut qs = questions();
        qs.entries.get_mut("FR-PAY-20").unwrap().questions.push("какой штраф за отмену".into());
        // Ratio 0.41 (see the seat test above): passages only.
        let weak = lexical_lists(&g, &qs, "штраф считается", 10, false);
        assert_eq!(weak.len(), 1);
        assert_eq!(weak[0][0], "FR-PAY-22");
        // Ratio above the gate: the questions list first, then the passages.
        let strong = lexical_lists(&g, &qs, "штраф отмену", 10, false);
        assert_eq!(strong.len(), 2);
        assert_eq!((strong[0][0].as_str(), strong[1][0].as_str()), ("FR-PAY-20", "FR-PAY-22"));
        // Reranked: both lists whatever the ratio, passages first — a pool, not five seats.
        let pool = lexical_lists(&g, &qs, "штраф считается", 10, true);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool[0][0], "FR-PAY-22");
    }

    #[test]
    fn code_questions_reach_the_reranked_pool_and_never_the_plain_fusion() {
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
        // Every query word but one is a code question's, and the list would clear the gate the
        // documents' list is held to: the plain fusion is the passages alone all the same. The
        // premise is asserted rather than asserted-by-comment, so a scoring change that made the
        // code list weak would fail here instead of leaving the plain-path check passing for the
        // wrong reason.
        let query = "штраф выйти всех устройств";
        let best = |l: &[(String, f32)]| l.first().map(|(_, s)| *s).unwrap_or(0.0);
        let code_best = best(&LexicalIndex::build_code_questions(&g, &qs).search(query, 10));
        let passages_best = best(&LexicalIndex::build(&g).search(query, 10));
        assert!(code_best >= QUESTIONS_GATE * passages_best,
            "the code list must clear the gate for this test to say anything: {code_best} against {passages_best}");
        let plain = lexical_lists(&g, &qs, query, 10, false);
        assert_eq!(plain.len(), 1, "{plain:?}");
        assert_eq!(plain[0][0], "FR-PAY-22");
        // The pool is `depth` deep, not five seats, so the code list joins it whole and last.
        let pool = lexical_lists(&g, &qs, query, 10, true);
        assert_eq!(pool.len(), 3, "{pool:?}");
        assert_eq!(pool[2], vec!["sym:apps/a.ts::revoke".to_string(), "sym:apps/a.ts::revokeOne".to_string()]);
        // No word of either code question: the list is absent from the pool, not empty.
        assert_eq!(lexical_lists(&g, &qs, "штраф считается", 10, true).len(), 2);
    }

    #[test]
    fn the_gate_override_is_read_only_when_it_parses() {
        assert_eq!(gate_from(None), QUESTIONS_GATE);
        assert_eq!(gate_from(Some("0")), 0.0);
        assert_eq!(gate_from(Some(" 0.9 ")), 0.9);
        assert_eq!(gate_from(Some("high")), QUESTIONS_GATE, "an unparsable override is ignored, not treated as zero");
    }

    #[test]
    fn a_generated_question_seeds_the_plain_answer_without_the_reranker() {
        let g = graph();
        // No label or body contains «аннулировать» or «бронь»; only the stored question does.
        let a = ask(&g, &ids(), &questions(), None, None, &["аннулировать".into(), "бронь".into()], &opts());
        assert_eq!(a.seeds[0].id, "FR-PAY-20");
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["аннулировать".into(), "бронь".into()], &opts());
        assert!(a.seeds.is_empty());
    }

    #[test]
    fn dense_leads_then_the_question_list_then_the_passages() {
        let g = graph();
        // Lexical passages rank FR-PAY-22 first for «штраф отмену»; the question list and dense
        // each bring a node of their own. Both query words sit in the stored question and only
        // one in any passage, so the question list clears the gate (2.14 against 1.32).
        let dense = |_: &str, _: usize| (vec!["N-151".to_string()], Vec::new());
        let mut qs = questions();
        qs.entries.get_mut("FR-PAY-20").unwrap().questions.push("какой штраф за отмену".into());
        let a = ask(&g, &ids(), &qs, Some(&dense), None, &["штраф".into(), "отмену".into()], &Options { dense: true, ..opts() });
        let order: Vec<&str> = a.seeds.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(&order[..3], ["N-151", "FR-PAY-20", "FR-PAY-22"]);
    }

    #[test]
    fn a_weakly_matched_question_list_does_not_take_a_seed_from_the_passages() {
        // «штраф считается» is two words of FR-PAY-22's body and one word of the stored
        // question, so the passage index scores 2.64 against the question index's 1.07 — a ratio
        // of 0.41, well under the gate. Under an equal turn the question row led the answer.
        let g = graph();
        let mut qs = questions();
        qs.entries.get_mut("FR-PAY-20").unwrap().questions.push("какой штраф за отмену".into());
        let a = ask(&g, &ids(), &qs, None, None, &["штраф".into(), "считается".into()], &opts());
        let order: Vec<&str> = a.seeds.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(order, ["FR-PAY-22"], "the weak question list took a seat: {order:?}");

        // The gate is relative. «штраф отмену» is both words of the stored question and one of
        // the passage, 2.14 against 1.32, and the question list leads as before.
        let a = ask(&g, &ids(), &qs, None, None, &["штраф".into(), "отмену".into()], &opts());
        assert_eq!(a.seeds[0].id, "FR-PAY-20");

        // And a list that is the only one with anything to say always clears it.
        let a = ask(&g, &ids(), &qs, None, None, &["аннулировать".into(), "бронь".into()], &opts());
        assert_eq!(a.seeds[0].id, "FR-PAY-20");
    }

    #[test]
    fn the_expanded_line_is_the_neighbour_the_retrievers_ranked_not_the_top_seeds() {
        // Seeds A1..A5 are the dense list; A1's neighbour X is unranked, A5's neighbour Y sits
        // at fused rank 6. The old rule took X (best seed); the retrievers vouch for Y.
        let mut g = Graph::default();
        let mut e = Extraction::default();
        for id in ["A1", "A2", "A3", "A4", "A5", "X", "Y"] {
            e.node(NodeKind::Requirement, id, id, "", "docs/a.md", 1);
        }
        e.edge("A1", "X", EdgeKind::References, "body", "docs/a.md");
        e.edge("A5", "Y", EdgeKind::References, "body", "docs/a.md");
        g.apply(e);
        let dense = |_: &str, _: usize| (["A1", "A2", "A3", "A4", "A5", "Y"].iter().map(|s| s.to_string()).collect(), Vec::new());
        let a = ask(&g, &ids(), &Questions::default(), Some(&dense), None, &["ничего".into()], &Options { dense: true, ..opts() });
        assert_eq!(a.seeds.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), ["A1", "A2", "A3", "A4", "A5"]);
        assert_eq!(a.expanded.len(), 1);
        assert_eq!((a.expanded[0].id.as_str(), a.expanded[0].via.as_deref()), ("Y", Some("A5")));
        // Without a ranked neighbour the seed's own rank decides, as before.
        let dense = |_: &str, _: usize| (["A1", "A2", "A3", "A4", "A5"].iter().map(|s| s.to_string()).collect(), Vec::new());
        let a = ask(&g, &ids(), &Questions::default(), Some(&dense), None, &["ничего".into()], &Options { dense: true, ..opts() });
        assert_eq!((a.expanded[0].id.as_str(), a.expanded[0].via.as_deref()), ("X", Some("A1")));
    }

    #[test]
    fn dense_callback_is_fused_when_present() {
        let g = graph();
        let dense = |_q: &str, _k: usize| (vec!["FR-PAY-20".to_string()], Vec::new());
        let mut o = opts();
        o.dense = true;
        let a = ask(&g, &ids(), &Questions::default(), Some(&dense), None, &["ничего".into(), "похожего".into()], &o);
        assert_eq!(a.seeds[0].id, "FR-PAY-20");
    }

    #[test]
    fn render_shape_is_id_path_line_headline() {
        let g = graph();
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["FR-PAY-22".to_string()], &opts());
        let out = render(&a, &g, &opts());
        let first = out.lines().next().unwrap();
        assert!(first.starts_with("FR-PAY-22  docs/06.md:385  `CancellationPolicy`"), "{first}");
        assert!(out.lines().any(|l| l.starts_with("  N-151  docs/never.md:12  ") && l.ends_with("← FR-PAY-22")));
        assert!(out.len() / 4 < 130);
    }

    #[test]
    fn headline_truncates_by_character_not_byte() {
        let cyrillic: String = "ж".repeat(100);
        let h = headline(&cyrillic);
        assert_eq!(h.chars().count(), 81);
        assert!(h.ends_with('…'));
        assert_eq!(headline(&"ж".repeat(80)), "ж".repeat(80));
    }

    #[test]
    fn a_question_no_retriever_answers_yields_an_empty_answer_not_a_panic() {
        let g = graph();
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["ъъъ".to_string(), "?!".to_string()], &opts());
        assert!(a.seeds.is_empty() && a.expanded.is_empty());
        assert_eq!(render(&a, &g, &opts()), "");
        let a = ask(&g, &ids(), &Questions::default(), None, None, &[String::new()], &opts());
        assert!(a.seeds.is_empty());
    }

    #[test]
    fn explain_groups_edges_by_kind() {
        let g = graph();
        let out = explain(&g, "CancellationPolicy").unwrap();
        assert!(out.starts_with("entity:CancellationPolicy  docs/06.md:385"));
        assert!(out.contains("References ← FR-PAY-22  [title]"));
        assert!(explain(&g, "nope").is_none());
    }

    #[test]
    fn verify_counts_undeclared_and_dangling() {
        let mut g = graph();
        let mut e = Extraction::default();
        e.edge("FR-PAY-20", "FR-PAY-999", EdgeKind::References, "body", "docs/06.md");
        e.edge("FR-PAY-20", "MOB-M01-T3", EdgeKind::References, "body", "docs/06.md");
        g.apply(e);
        let out = verify(&g);
        assert!(out.contains("undeclared ids: 2"));
        assert!(out.contains("gaps in declared families: 1  FR-PAY-999"));
        assert!(out.contains("in families never declared: 1  MOB-M"));
        assert!(out.contains("dangling edges: 2"));
    }

    #[test]
    fn several_ids_in_one_question_all_become_seeds_in_word_order() {
        let g = graph();
        let words: Vec<String> = ["FR-PAY-22", "FR-PAY-20"].iter().map(|s| s.to_string()).collect();
        let a = ask(&g, &ids(), &Questions::default(), None, None, &words, &opts());
        assert_eq!(a.seeds.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), ["FR-PAY-22", "FR-PAY-20"]);
    }

    #[test]
    fn an_id_shaped_word_absent_from_the_graph_yields_no_seeds() {
        let g = graph();
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["FR-PAY-999".to_string()], &opts());
        assert!(a.seeds.is_empty());
        assert!(a.expanded.is_empty());
    }

    #[test]
    fn a_symbol_and_an_id_together_both_become_exact_seeds() {
        let g = graph();
        let words: Vec<String> = ["asGrosze", "FR-PAY-22"].iter().map(|s| s.to_string()).collect();
        let a = ask(&g, &ids(), &Questions::default(), None, None, &words, &opts());
        assert_eq!(a.seeds.len(), 2);
        assert_eq!(a.seeds[0].id, "sym:packages/contracts/src/money.ts::asGrosze");
        assert_eq!(a.seeds[1].id, "FR-PAY-22");
    }

    #[test]
    fn seeds_zero_drops_even_an_exact_match_and_its_expansion() {
        let g = graph();
        let mut o = opts();
        o.seeds = 0;
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["FR-PAY-22".to_string()], &o);
        assert!(a.seeds.is_empty());
        assert!(a.expanded.is_empty());
    }

    #[test]
    fn seeds_larger_than_the_pool_returns_only_what_exists() {
        let g = duplicated_symbol_graph();
        let mut o = opts();
        o.seeds = 100;
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["buildClientParams".to_string()], &o);
        assert_eq!(a.seeds.len(), 4);
    }

    #[test]
    fn render_includes_body_lines_only_when_bodies_is_set() {
        let g = graph();
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["FR-PAY-22".to_string()], &opts());
        let mut with_bodies = opts();
        with_bodies.bodies = true;
        let out = render(&a, &g, &with_bodies);
        assert!(out.lines().any(|l| l.starts_with("    ") && l.contains("штраф")));
        let out = render(&a, &g, &opts());
        assert!(!out.lines().any(|l| l.starts_with("    ")));
    }

    #[test]
    fn json_render_is_valid_json_with_seeds_and_expanded_keys() {
        let g = graph();
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["FR-PAY-22".to_string()], &opts());
        let mut o = opts();
        o.json = true;
        let out = render(&a, &g, &o);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v.get("seeds").is_some());
        assert!(v.get("expanded").is_some());
        let seed = &v["seeds"][0];
        for key in ["id", "file", "line", "label", "score", "via"] {
            assert!(seed.get(key).is_some(), "missing {key}");
        }
    }

    #[test]
    fn explain_of_an_unknown_node_is_none() {
        let g = graph();
        assert!(explain(&g, "not-a-real-id-or-label").is_none());
    }

    #[test]
    fn verify_on_an_empty_graph_reports_zero_everywhere() {
        let g = Graph::default();
        let out = verify(&g);
        assert!(out.contains("nodes: 0"));
        assert!(out.contains("edges: 0"));
        assert!(out.contains("dangling edges: 0"));
        assert!(out.contains("undeclared ids: 0"));
    }

    #[test]
    fn verify_reports_a_single_dangling_edge_as_a_gap_in_a_declared_family() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-WEB-1", "первая", "", "docs/a.md", 1);
        e.edge("FR-WEB-1", "FR-WEB-999", EdgeKind::References, "body", "docs/a.md");
        g.apply(e);
        let out = verify(&g);
        assert!(out.contains("dangling edges: 1"));
        assert!(out.contains("undeclared ids: 1  FR-WEB-999"));
        assert!(out.contains("gaps in declared families: 1  FR-WEB-999"));
    }

    #[test]
    fn a_question_of_only_punctuation_yields_an_empty_answer_not_a_panic() {
        let g = graph();
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["???".to_string(), "!!!".to_string()], &opts());
        assert!(a.seeds.is_empty() && a.expanded.is_empty());
    }

    #[test]
    fn a_question_of_only_whitespace_words_yields_an_empty_answer_not_a_panic() {
        let g = graph();
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["   ".to_string(), "\t".to_string()], &opts());
        assert!(a.seeds.is_empty() && a.expanded.is_empty());
    }

    #[test]
    fn a_single_cyrillic_word_reaches_the_lexical_path() {
        let g = graph();
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["штраф".to_string()], &opts());
        assert_eq!(a.seeds[0].id, "FR-PAY-22");
    }

    #[test]
    fn expansion_skips_a_neighbour_that_is_already_a_seed() {
        let g = graph();
        let words: Vec<String> = ["FR-PAY-22", "N-151"].iter().map(|s| s.to_string()).collect();
        let a = ask(&g, &ids(), &Questions::default(), None, None, &words, &opts());
        assert_eq!(a.seeds.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), ["FR-PAY-22", "N-151"]);
        assert_eq!(a.expanded.len(), 1);
        assert_eq!(a.expanded[0].id, "entity:CancellationPolicy");
        assert_eq!(a.expanded[0].via.as_deref(), Some("FR-PAY-22"));
    }

    #[test]
    fn tied_neighbour_candidates_break_by_id_ascending() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        for id in ["FR-WEB-1", "FR-WEB-30", "FR-WEB-40"] {
            e.node(NodeKind::Requirement, id, id, "", "docs/a.md", 1);
        }
        e.edge("FR-WEB-1", "FR-WEB-40", EdgeKind::References, "body", "docs/a.md");
        e.edge("FR-WEB-1", "FR-WEB-30", EdgeKind::References, "body", "docs/a.md");
        g.apply(e);
        let a = ask(&g, &ids(), &Questions::default(), None, None, &["FR-WEB-1".to_string()], &opts());
        assert_eq!(a.expanded.len(), 1);
        assert_eq!(a.expanded[0].id, "FR-WEB-30");
    }
}
