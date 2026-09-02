use crate::enrich::Questions;
use crate::ids::IdMatcher;
use crate::index::{fuse, lexical::LexicalIndex};
use crate::model::{EdgeKind, Graph, NodeKind};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

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
/// Fused seeds kept ahead of the reranking model's picks.
const PINNED: usize = 2;

fn hit(graph: &Graph, id: &str, score: f32, via: Option<&str>) -> Option<Hit> {
    let n = graph.nodes.get(id)?;
    Some(Hit { id: n.id.clone(), file: n.file.clone(), line: n.line, label: n.label.clone(), score, via: via.map(str::to_string) })
}

/// Exact hits per word, and whether they answer the whole question: every word matched, and
/// each carries an uppercase letter — `money` and `utf8` are topics as much as names, `asGrosze` only a name.
fn exact_seeds(graph: &Graph, ids: &IdMatcher, words: &[String]) -> (Vec<String>, bool) {
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
        // get the odd seed when the two lists are interleaved. The generated questions add
        // nothing at five seeds (measured 6/14 with and without) and cost a keyword seed in the
        // no-dense answer, but they carry targets into the reranker's pool (FR-AI-102 enters at
        // 30, absent otherwise), so they are lists of their own there and nowhere else. Pooled
        // into the passage rows they bury targets: a passage at rank 2 fell to 87.
        let mut lists: Vec<Vec<String>> = Vec::new();
        if opts.dense {
            if let Some(d) = dense {
                let (passages, generated) = d(&query, depth);
                lists.push(passages);
                if rerank.is_some() { lists.push(generated); }
            }
        }
        lists.push(LexicalIndex::build(graph).search(&query, depth).into_iter().map(|(id, _)| id).collect());
        if rerank.is_some() {
            lists.push(LexicalIndex::build_questions(graph, questions).search(&query, depth).into_iter().map(|(id, _)| id).collect());
        }
        lists.retain(|l| !l.is_empty());
        let mut fused = fuse::interleave(&lists);
        if let Some(r) = rerank {
            // The fused top two stay in front: they carry the exact evidence the model is not
            // shown (it sees titles), and with the whole say it dropped a keyword hit and a
            // paraphrase hit the retrievers had ranked second.
            let candidates: Vec<(String, String)> = fused.iter().take(depth)
                .map(|(id, _)| (id.clone(), graph.nodes[id].label.clone())).collect();
            let mut order: Vec<String> = candidates.iter().take(PINNED).map(|(id, _)| id.clone()).collect();
            for id in r(&query, &candidates) {
                if !order.contains(&id) { order.push(id); }
            }
            let rest: Vec<(String, f32)> = fused.into_iter().filter(|(id, _)| !order.contains(id)).collect();
            fused = order.iter().enumerate().map(|(rank, id)| (id.clone(), 1.0 / (rank as f32 + 1.0))).chain(rest).collect();
        }
        for (id, score) in fused {
            if answer.seeds.len() >= opts.seeds { break; }
            if exact.contains(&id) { continue; }
            if let Some(h) = hit(graph, &id, score, None) { answer.seeds.push(h); }
        }
    }

    let seed_ids: Vec<String> = answer.seeds.iter().map(|h| h.id.clone()).collect();
    let mut expanded: Vec<Hit> = Vec::new();
    for seed in &answer.seeds {
        for e in graph.neighbours(&seed.id) {
            if !EXPAND.contains(&e.kind) { continue; }
            let other = if e.source == seed.id { &e.target } else { &e.source };
            if seed_ids.contains(other) || other.starts_with("file:") || other.starts_with("deco:") { continue; }
            if expanded.iter().any(|h| &h.id == other) { continue; }
            if let Some(h) = hit(graph, other, seed.score * 0.5, Some(&seed.id)) { expanded.push(h); }
        }
    }
    expanded.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap().then(a.id.cmp(&b.id)));
    expanded.truncate(MAX_EXPANDED);
    answer.expanded = expanded;
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

fn resolve<'a>(graph: &'a Graph, needle: &str) -> Option<&'a crate::model::Node> {
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
}
