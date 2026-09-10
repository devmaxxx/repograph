use crate::index::{fuse, lexical::Lexical};
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
/// How much more of the query the generated-questions BM25 list must have covered, against the
/// passage list, to join the plain-path fusion. Each list is first asked about itself — what
/// fraction of what the query could reach in that index its best document actually reached,
/// `best / attainable` — and the admission compares those two coverages. A coverage is
/// dimensionless, so the two lists arrive in one unit whatever their raw scores are worth. The
/// shipped ratio compared the raw bests instead, and the two indices normalise length against
/// their own means (30.6 tokens a passage, 51.8 a question document) and weight terms by their
/// own vocabularies (10.8k against 21.8k), so its value moved with enrichment coverage and
/// questions per node and was a constant of one store rather than of BM25 — gaps G8 and G12.
///
/// The value is the crossover of the 400 held-out questions in the `--no-dense` arm: the split
/// that puts the most of them on the side of the list actually holding their answer in its top
/// five. It was re-derived on 2026-09-06, on the same 400 questions in the same arm, when
/// `attainable` began charging every query term rather than dropping the ones its index lacked
/// (`/Users/max/bench/residue-seat-register-2026-09-06/t2-crossover.txt`,
/// `docs/bench/2026-09-06-residue-seat-register-results.md`). It returned 0.761 — the same value
/// the dropping denominator's crossover returned, on the earlier evidence
/// (`/Users/max/bench/gaps-2026-09-05/admission-verdict.txt`) — so the literal did not move,
/// but it is a constant of the form that ships, derived under it. Neither derivation was
/// rounded, tuned or compared with the other: each was taken before either suite was opened, and
/// if the form fails, the form fails. The rules the two forms were judged against, written before
/// either was run, and the measurements that judged them:
/// `docs/superpowers/specs/2026-09-06-coverage-admission-design.md` and
/// `docs/superpowers/specs/2026-09-06-residue-seat-register-design.md`.
const QUESTIONS_GATE: f64 = 0.761;

/// `REPOGRAPH_QUESTIONS_GATE` overrides the constant for a measurement and for nothing else:
/// `0` reproduces the ungated fusion that ADR-001 Amendment 6's before-column was read against,
/// which no commit's binary otherwise produces, and any other value re-reads the coverage
/// crossover above — no longer the ratio of two raw bests that the name once meant.
fn questions_gate() -> f64 {
    gate_from(std::env::var("REPOGRAPH_QUESTIONS_GATE").ok().as_deref())
}

fn gate_from(override_: Option<&str>) -> f64 {
    override_.and_then(|v| v.trim().parse().ok()).unwrap_or(QUESTIONS_GATE)
}

/// What this list's best document answered of the query, over what the query could have reached
/// in that index at all. An index holding none of the query's terms attains nothing and its list
/// covers nothing, which keeps the zero out of the denominator.
fn coverage(best: f32, attainable: f32) -> f64 {
    if attainable > 0.0 { f64::from(best) / f64::from(attainable) } else { 0.0 }
}

/// Whether a list is seated beside the passages under the coverage admission. The arithmetic is
/// `f64` over the `f32` scores because the constant was derived by `bench/admission.py` in double
/// precision, and that replay is checked against this function query by query — in `f32` the two
/// would be different functions at the boundary.
fn admits(best: f32, attainable: f32, passages_best: f32, passages_attainable: f32, c: f64) -> bool {
    if best <= 0.0 { return false; }
    let theirs = coverage(passages_best, passages_attainable);
    // A passage list that covered nothing of the query is no bar to clear, rather than a
    // division by zero that would refuse every list standing behind it.
    theirs <= 0.0 || coverage(best, attainable) / theirs >= c
}

/// The lexical lists for one question, in fusion order. A store `enrich` never touched has one:
/// an index of id-only documents is shorter than the passages and ranks an id-bearing term above
/// the passage that carries it, so on a raw store the questions list would be admitted for the
/// wrong reason and cost a build per question to do it. On the reranked path both lists are
/// admitted unconditionally: the fused order there is a candidate pool `depth` deep rather than
/// five seats, so a list there costs the reranking model candidates and not seeds, and Amendment 2
/// measured the questions list as what carries paraphrase targets into that pool. The questions
/// about code are a third list on the reranked path and on no other: the plain fusion's five seats
/// were measured to be worth more to the documents than to them.
fn lexical_lists(lex: &Lexical, query: &str, depth: usize, reranked: bool) -> Vec<Vec<String>> {
    let only_ids = |scored: Vec<(String, f32)>| -> Vec<String> { scored.into_iter().map(|(id, _)| id).collect() };
    let passages = lex.passages.search(query, depth);
    let Some(questions_index) = &lex.questions else { return vec![only_ids(passages)]; };
    let generated = questions_index.search(query, depth);
    if reranked {
        let mut pool = vec![only_ids(passages), only_ids(generated)];
        // Not on the plain path. Given a seat there instead — one, on the same admission — the
        // code questions read `where` 0/9 → 2/9 on the developer suite but held-out 103 → 97 and
        // 109 → 103, 0 gained and 6 lost in each arm, p = 0.031 (2026-09-05): five seats are the
        // budget the floors were set on, and a seat given to code is a document question's answer
        // lost. Here the pool is `--depth` deep (200 by default) rather than five seats, so the
        // list costs the reranking model candidates and not seeds; what it is worth to that
        // model is unmeasured.
        if let Some(code_index) = &lex.code {
            let code = code_index.search(query, depth);
            if !code.is_empty() { pool.push(only_ids(code)); }
        }
        return pool;
    }
    let best = |l: &[(String, f32)]| l.first().map(|(_, s)| *s).unwrap_or(0.0);
    let (passages_best, passages_attainable) = (best(&passages), lex.passages.attainable(query));
    let mut lists = Vec::with_capacity(2);
    if admits(best(&generated), questions_index.attainable(query), passages_best, passages_attainable, questions_gate()) {
        lists.push(only_ids(generated));
    }
    lists.push(only_ids(passages));
    lists
}

fn hit(graph: &Graph, id: &str, score: f32, via: Option<&str>) -> Option<Hit> {
    let n = graph.nodes.get(id)?;
    Some(Hit { id: n.id.clone(), file: n.file.clone(), line: n.line, label: n.label.clone(), score, via: via.map(str::to_string) })
}

/// Exact hits per word, and whether they answer the whole question: every word matched, and
/// each carries an uppercase letter — `money` and `utf8` are topics as much as names, `asGrosze` only a name.
pub(crate) fn exact_seeds(graph: &Graph, words: &[String]) -> (Vec<String>, bool) {
    let mut out = Vec::new();
    let mut whole = true;
    for w in words {
        if crate::ids::generic().is_id(w) && graph.nodes.contains_key(w) {
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

pub fn ask(graph: &Graph, lex: &Lexical, dense: Option<Dense>, rerank: Option<Rerank>, words: &[String], opts: &Options) -> Answer {
    let query = words.join(" ");
    let mut answer = Answer::default();
    let (exact, whole_question) = exact_seeds(graph, words);
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
        // work. So it is admitted per question, on how much of the question its best document
        // covered against how much the passages' best covered — `QUESTIONS_GATE`, and
        // `lexical_lists` for what the two paths do with it. The raw arms are untouched by
        // construction: a store without questions gets no questions list built at all.

        // The builds now happen once in the caller, not here — the overlap with the model open
        // lives there too, in `Context::answer`, where `Lexical::build` runs before this
        // function is even called. What this order still decides is the seat: dense passages,
        // then the dense question rows on the reranked path, then these.
        let lexical = lexical_lists(lex, &query, depth, rerank.is_some());
        let mut lists: Vec<Vec<String>> = Vec::new();
        if opts.dense {
            if let Some(d) = dense {
                let (passages, questions_rows) = d(&query, depth);
                lists.push(passages);
                if rerank.is_some() { lists.push(questions_rows); }
            }
        }
        lists.extend(lexical);
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

/// How much of a line is shown where one is quoted: a seed's label, a family's defining line.
pub(crate) const HEADLINE: usize = 80;

pub(crate) fn headline(label: &str) -> String {
    let mut s: String = label.chars().take(HEADLINE).collect();
    if label.chars().count() > HEADLINE { s.push('…'); }
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

/// `explain` for a reader that would rather not parse prose: the node, then one entry per edge
/// with the direction already resolved, so a caller never has to know which end of an edge the
/// node was. A view struct rather than the graph's own types — a store layout is not an interface.
pub fn explain_json(graph: &Graph, needle: &str) -> Option<String> {
    #[derive(serde::Serialize)]
    struct Edge<'a> { kind: String, dir: &'a str, other: &'a str, context: &'a str }
    #[derive(serde::Serialize)]
    struct Out<'a> {
        id: &'a str, kind: String, label: &'a str, file: &'a str, line: u32,
        community: Option<&'a str>, edges: Vec<Edge<'a>>,
    }
    let n = resolve(graph, needle)?;
    let mut edges = graph.neighbours(&n.id);
    edges.sort_by_key(|e| (e.kind == EdgeKind::Legacy, e.kind, e.source.clone(), e.target.clone()));
    let out = Out {
        id: &n.id,
        kind: format!("{:?}", n.kind),
        label: &n.label,
        file: &n.file,
        line: n.line,
        community: n.community.as_deref(),
        edges: edges.iter().map(|e| {
            let (dir, other) = if e.source == n.id { ("out", e.target.as_str()) } else { ("in", e.source.as_str()) };
            Edge { kind: format!("{:?}", e.kind), dir, other, context: &e.context }
        }).collect(),
    };
    serde_json::to_string(&out).ok()
}

/// `verify`'s own numbers, the same partitions under names instead of indentation.
pub fn verify_json(graph: &Graph) -> String {
    let mut nodes: BTreeMap<String, usize> = BTreeMap::new();
    for n in graph.nodes.values() { *nodes.entry(format!("{:?}", n.kind)).or_default() += 1; }
    let mut edges: BTreeMap<String, usize> = BTreeMap::new();
    for e in &graph.edges { *edges.entry(format!("{:?}", e.kind)).or_default() += 1; }
    let dangling = graph.dangling();
    let mut undeclared: Vec<&str> = dangling.iter().filter(|e| !e.target.contains(':')).map(|e| e.target.as_str()).collect();
    undeclared.sort();
    undeclared.dedup();
    let declared: BTreeSet<&str> = graph.nodes.keys().filter(|id| !id.contains(':')).map(|id| family(id)).collect();
    let (gaps, cite_only): (Vec<&str>, Vec<&str>) = undeclared.iter().partition(|id| declared.contains(family(id)));
    let held: BTreeSet<&str> = graph.pending.iter().map(|e| family(&e.target)).collect();
    #[derive(serde::Serialize)]
    struct Out<'a> {
        nodes: usize, edges: usize,
        nodes_by_kind: BTreeMap<String, usize>, edges_by_kind: BTreeMap<String, usize>,
        dangling: usize, undeclared: Vec<&'a str>, gaps: Vec<&'a str>, cite_only: Vec<&'a str>,
        held_aside: usize, held_aside_prefixes: Vec<&'a str>,
    }
    let out = Out {
        nodes: graph.nodes.len(), edges: graph.edges.len(),
        nodes_by_kind: nodes, edges_by_kind: edges,
        dangling: dangling.len(), undeclared, gaps, cite_only,
        held_aside: graph.pending.len(), held_aside_prefixes: held.into_iter().collect(),
    };
    serde_json::to_string(&out).unwrap_or_else(|_| "{}".to_string())
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
    let held: BTreeSet<&str> = graph.pending.iter().map(|e| family(&e.target)).collect();
    out.push_str(&format!("held aside: {} edges to ids in {} prefixes no line defines  {}\n",
        graph.pending.len(), held.len(), held.iter().cloned().collect::<Vec<_>>().join(" ")));
    out.push_str(&format!("undeclared ids: {}  {}\n", undeclared.len(), sample(&undeclared)));
    out.push_str(&format!("  gaps in declared families: {}  {}\n", gaps.len(), sample(&gaps)));
    out.push_str(&format!("  in families never declared: {}  {}\n", cite_only.len(),
        cite_only_families.iter().cloned().collect::<Vec<_>>().join(" ")));
    out
}

/// The family an id is written in, the way `repograph families` and the read matcher both read
/// it — two groupings over one store would have `verify` contradict the command that reports on
/// the same thing. An id in no family at all is its own group: the second half of the partition
/// is for citations of prefixes nothing declares, and a shape the dialect does not recognise is
/// exactly that.
fn family(id: &str) -> &str {
    match crate::families::classify(id) {
        Some(crate::families::Family::Id(f) | crate::families::Family::Milestone(f)) => f,
        None => id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enrich::Questions;
    use crate::index::lexical::LexicalIndex;
    use crate::model::{EdgeKind, Extraction, NodeKind};

    fn lex(g: &Graph, qs: &Questions) -> Lexical { Lexical::build(g, qs, true) }

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

    fn opts() -> Options { Options { seeds: 5, bodies: false, dense: false, json: false, depth: crate::rerank::DEPTH } }

    #[test]
    fn exact_id_wins_and_expands_one_hop() {
        let g = graph();
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["FR-PAY-22".to_string()], &opts());
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
        let a = ask(&g, &lex(&g, &Questions::default()), Some(&dense), None, &["money".to_string()], &Options { dense: true, ..opts() });
        assert_eq!(a.seeds[0].id, "sym:packages/domain/test/money.spec.ts::money");
        assert!(a.seeds.iter().any(|h| h.id == "FR-PAY-20"));
    }

    #[test]
    fn a_code_shaped_name_is_the_whole_question() {
        let g = graph();
        let dense = |_: &str, _: usize| (vec!["FR-PAY-20".to_string()], Vec::new());
        let a = ask(&g, &lex(&g, &Questions::default()), Some(&dense), None, &["asGrosze".to_string()], &Options { dense: true, ..opts() });
        assert_eq!(a.seeds.len(), 1);
        assert_eq!(a.seeds[0].id, "sym:packages/contracts/src/money.ts::asGrosze");
    }

    #[test]
    fn a_symbol_asked_twice_is_one_seed() {
        let g = graph();
        let words: Vec<String> = ["asGrosze", "foo", "asGrosze"].iter().map(|s| s.to_string()).collect();
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &words, &opts());
        assert_eq!(a.seeds.iter().filter(|h| h.id.ends_with("::asGrosze")).count(), 1);
    }

    #[test]
    fn exact_match_leaves_the_other_seed_slots_empty() {
        let g = graph();
        // "N-151" is also a lexical hit on FR-PAY-22's body; it must arrive by expansion, not as a seed.
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["N-151".to_string()], &opts());
        assert_eq!(a.seeds.len(), 1);
        assert_eq!(a.seeds[0].id, "N-151");
        assert_eq!(a.expanded[0].id, "FR-PAY-22");
    }

    #[test]
    fn backticked_entity_is_reachable_by_expansion() {
        let g = single_neighbour_graph();
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["FR-PAY-22".to_string()], &opts());
        assert_eq!(a.expanded.len(), 1);
        assert_eq!(a.expanded[0].id, "entity:CancellationPolicy");
        assert_eq!(a.expanded[0].via.as_deref(), Some("FR-PAY-22"));
    }

    #[test]
    fn exact_seeds_are_capped_at_options_seeds() {
        let g = duplicated_symbol_graph();
        let mut o = opts();
        o.seeds = 2;
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["buildClientParams".to_string()], &o);
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
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["FR-X".to_string()], &opts());
        assert!(a.expanded.is_empty());
    }

    #[test]
    fn exact_symbol_name_resolves_to_its_file() {
        let g = graph();
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["asGrosze".to_string()], &opts());
        assert_eq!(a.seeds[0].file, "packages/contracts/src/money.ts");
    }

    #[test]
    fn lexical_query_in_russian_finds_the_requirement() {
        let g = graph();
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["политика".into(), "отмены".into(), "штраф".into()], &opts());
        assert_eq!(a.seeds[0].id, "FR-PAY-22");
    }

    #[test]
    fn the_dense_top_hit_takes_the_first_seed_and_its_second_hit_the_third() {
        let g = graph();
        // Lexical alone ranks FR-PAY-22 first for "штраф"; dense disagrees on both of its slots.
        let dense = |_: &str, _: usize| (vec!["N-151".to_string(), "FR-PAY-20".to_string()], Vec::new());
        let a = ask(&g, &lex(&g, &Questions::default()), Some(&dense), None, &["штраф".to_string()], &Options { dense: true, ..opts() });
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
            let lists = lexical_lists(&lex(&g, &none), "FR-PAY-22 штраф", 10, reranked);
            assert_eq!(lists.len(), 1, "reranked={reranked}: {lists:?}");
            assert_eq!(lists[0][0], "FR-PAY-22");
        }
    }

    #[test]
    fn the_questions_list_leads_when_admitted_and_is_absent_when_it_matched_nothing() {
        let g = graph();
        let mut qs = questions();
        qs.entries.get_mut("FR-PAY-20").unwrap().questions.push("какой штраф за отмену".into());
        // «считается» is in FR-PAY-22's body and in no stored question, so the questions index
        // scores nothing and is refused. This is the only refusal a two-node graph can produce:
        // see the test below for why the raw ratio's other refusals do not survive the change.
        let weak = lexical_lists(&lex(&g, &qs), "считается", 10, false);
        assert_eq!(weak.len(), 1);
        assert_eq!(weak[0][0], "FR-PAY-22");
        // Ratio above the gate: the questions list first, then the passages.
        let strong = lexical_lists(&lex(&g, &qs), "штраф отмену", 10, false);
        assert_eq!(strong.len(), 2);
        assert_eq!((strong[0][0].as_str(), strong[1][0].as_str()), ("FR-PAY-20", "FR-PAY-22"));
        // Reranked: both lists whatever the ratio, passages first — a pool, not five seats.
        let pool = lexical_lists(&lex(&g, &qs), "штраф считается", 10, true);
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
        // Every query word but one is a code question's, and the list would clear the admission
        // the documents' list is held to: the plain fusion is the passages alone all the same.
        // The premise is asserted rather than asserted-by-comment, so a scoring change that made
        // the code list weak would fail here instead of leaving the plain-path check passing for
        // the wrong reason.
        let query = "штраф выйти всех устройств";
        let best = |l: &[(String, f32)]| l.first().map(|(_, s)| *s).unwrap_or(0.0);
        let code_index = LexicalIndex::build_code_questions(&g, &qs);
        let passages_index = LexicalIndex::build(&g);
        let code_best = best(&code_index.search(query, 10));
        let passages_best = best(&passages_index.search(query, 10));
        assert!(admits(code_best, code_index.attainable(query), passages_best, passages_index.attainable(query), QUESTIONS_GATE),
            "the code list must be admissible for this test to say anything: {code_best} against {passages_best}");
        let plain = lexical_lists(&lex(&g, &qs), query, 10, false);
        assert_eq!(plain.len(), 1, "{plain:?}");
        assert_eq!(plain[0][0], "FR-PAY-22");
        // The pool is `depth` deep, not five seats, so the code list joins it whole and last.
        let pool = lexical_lists(&lex(&g, &qs), query, 10, true);
        assert_eq!(pool.len(), 3, "{pool:?}");
        assert_eq!(pool[2], vec!["sym:apps/a.ts::revoke".to_string(), "sym:apps/a.ts::revokeOne".to_string()]);
        // No word of either code question: the list is absent from the pool, not empty.
        assert_eq!(lexical_lists(&lex(&g, &qs), "штраф считается", 10, true).len(), 2);
    }

    #[test]
    fn the_gate_override_is_read_only_when_it_parses() {
        assert_eq!(gate_from(None), QUESTIONS_GATE);
        assert_eq!(gate_from(Some("0")), 0.0);
        assert_eq!(gate_from(Some(" 0.9 ")), 0.9);
        assert_eq!(gate_from(Some("high")), QUESTIONS_GATE, "an unparsable override is ignored, not treated as zero");
        // The documented meaning of the override's one special value: at zero every list that
        // matched anything is seated, which is the ungated fusion ADR-001 Amendment 6 measured.
        assert!(admits(0.001, 9.0, 5.0, 1.0, gate_from(Some("0"))));
        assert!(!admits(0.0, 9.0, 5.0, 1.0, gate_from(Some("0"))));
    }

    #[test]
    fn an_empty_questions_list_is_never_seated_not_even_where_the_passages_are_no_bar() {
        // An empty list is handed a best of zero by `lexical_lists`, and a passage list that
        // covered nothing is otherwise no bar at all: the two ends meet here and the seat still
        // goes nowhere, because a list with nothing to say cannot be the one holding the answer.
        assert!(!admits(0.0, 0.0, 0.0, 0.0, QUESTIONS_GATE));
        assert!(!admits(0.0, 4.0, 1.0, 2.0, QUESTIONS_GATE));
    }

    #[test]
    fn a_zero_attainable_leaves_that_list_covering_nothing_on_either_side() {
        // An index holding none of the query's terms attains nothing. As the admitted list that
        // is a coverage of zero and it loses; as the passage list it is no bar rather than a
        // division by zero. `-0.0` is the sign `f32`'s empty sum takes and reads the same way.
        assert!(!admits(1.0, 0.0, 1.0, 2.0, QUESTIONS_GATE));
        assert!(!admits(1.0, -0.0, 1.0, 2.0, QUESTIONS_GATE));
        assert!(admits(0.1, 4.0, 9.9, 0.0, QUESTIONS_GATE));
        assert!(admits(0.1, 4.0, 9.9, -0.0, QUESTIONS_GATE));
    }

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

    #[test]
    fn a_passage_list_whose_best_is_zero_is_no_bar_at_all() {
        // Nothing scored in the passage index, so its coverage is zero however much the query
        // could have reached there, and the questions list takes the seat unopposed.
        assert!(admits(0.01, 8.0, 0.0, 4.0, QUESTIONS_GATE));
    }

    #[test]
    fn the_coverage_seats_what_the_ratio_of_raw_bests_refuses_and_refuses_what_it_seats() {
        // The form's whole point, in two lines. A questions list at half the passages' raw best
        // has covered twice as much of the question when the passage index could have offered
        // four times more; a list level on raw score has covered a fifth as much when its own
        // index could have offered five times more. The raw bests cannot tell these apart —
        // they are scores from two indices that were never in the same unit (G12).
        let ratio = |best: f32, passages_best: f32| best >= 0.85 * passages_best;
        assert!(!ratio(1.0, 2.0), "the ratio refuses this list");
        assert!(admits(1.0, 2.0, 2.0, 8.0, QUESTIONS_GATE), "its coverage, 0.50 against 0.25, seats it");
        assert!(ratio(2.0, 2.0), "the ratio seats this one");
        assert!(!admits(2.0, 10.0, 2.0, 2.0, QUESTIONS_GATE), "its coverage, 0.20 against 1.00, refuses it");
    }

    #[test]
    fn a_generated_question_seeds_the_plain_answer_without_the_reranker() {
        let g = graph();
        // No label or body contains «аннулировать» or «бронь»; only the stored question does.
        let a = ask(&g, &lex(&g, &questions()), None, None, &["аннулировать".into(), "бронь".into()], &opts());
        assert_eq!(a.seeds[0].id, "FR-PAY-20");
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["аннулировать".into(), "бронь".into()], &opts());
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
        let a = ask(&g, &lex(&g, &qs), Some(&dense), None, &["штраф".into(), "отмену".into()], &Options { dense: true, ..opts() });
        let order: Vec<&str> = a.seeds.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(&order[..3], ["N-151", "FR-PAY-20", "FR-PAY-22"]);
    }

    #[test]
    fn a_term_the_questions_index_lacks_now_costs_it_the_seat_the_shipped_form_gave_it() {
        // The two queries the raw ratio ranked furthest apart on this graph. Under the shipped
        // denominator — which dropped a term the index did not hold instead of charging it —
        // both read 0.696 against 0.858 and were admitted at 0.811, because each query has one
        // term that one of the two indices lacks and dropping it flattered whichever index that
        // was. Charged, the two queries separate, and in opposite directions.
        //
        // «штраф считается»: «считается» is in FR-PAY-22's body and in no stored question, so it
        // is the questions index that pays. Its coverage falls to 0.256 against the passages'
        // 0.858 and the admission to 0.299 — under the constant, refused, and the plain answer is
        // the passage the query actually names.
        //
        // «штраф отмену»: the missing term is the passage index's, so the arithmetic runs the
        // other way — the questions list keeps 0.696 while the passages fall to 0.316 and the
        // admission rises to 2.200. Charging the term does not favour one list; it charges
        // whichever index was being flattered.
        let g = graph();
        let mut qs = questions();
        qs.entries.get_mut("FR-PAY-20").unwrap().questions.push("какой штраф за отмену".into());
        let l = lex(&g, &qs);
        let qi = l.questions.as_ref().unwrap();
        let best = |x: &[(String, f32)]| x.first().map(|(_, s)| *s).unwrap_or(0.0);
        let read = |q: &str| {
            let (bq, aq) = (best(&qi.search(q, 10)), qi.attainable(q));
            let (bp, ap) = (best(&l.passages.search(q, 10)), l.passages.attainable(q));
            (coverage(bq, aq), coverage(bp, ap), admits(bq, aq, bp, ap, QUESTIONS_GATE))
        };
        let (cq, cp, seated) = read("штраф считается");
        assert!((cq - 0.256).abs() < 5e-4 && (cp - 0.858).abs() < 5e-4, "questions {cq}, passages {cp}");
        assert!(!seated, "the questions list covered {cq} of what it was asked against the passages' {cp}");
        let (cq, cp, seated) = read("штраф отмену");
        assert!((cq - 0.696).abs() < 5e-4 && (cp - 0.316).abs() < 5e-4, "questions {cq}, passages {cp}");
        assert!(seated, "the questions list covered {cq} against the passages' {cp}");

        let a = ask(&g, &lex(&g, &qs), None, None, &["штраф".into(), "считается".into()], &opts());
        let order: Vec<&str> = a.seeds.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(order[0], "FR-PAY-22", "{order:?}");

        // The gate is relative. «штраф отмену» is both words of the stored question and one of
        // the passage, 2.14 against 1.32, and the question list leads as before.
        let a = ask(&g, &lex(&g, &qs), None, None, &["штраф".into(), "отмену".into()], &opts());
        assert_eq!(a.seeds[0].id, "FR-PAY-20");

        // And a list that is the only one with anything to say always clears it.
        let a = ask(&g, &lex(&g, &qs), None, None, &["аннулировать".into(), "бронь".into()], &opts());
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
        let a = ask(&g, &lex(&g, &Questions::default()), Some(&dense), None, &["ничего".into()], &Options { dense: true, ..opts() });
        assert_eq!(a.seeds.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), ["A1", "A2", "A3", "A4", "A5"]);
        assert_eq!(a.expanded.len(), 1);
        assert_eq!((a.expanded[0].id.as_str(), a.expanded[0].via.as_deref()), ("Y", Some("A5")));
        // Without a ranked neighbour the seed's own rank decides, as before.
        let dense = |_: &str, _: usize| (["A1", "A2", "A3", "A4", "A5"].iter().map(|s| s.to_string()).collect(), Vec::new());
        let a = ask(&g, &lex(&g, &Questions::default()), Some(&dense), None, &["ничего".into()], &Options { dense: true, ..opts() });
        assert_eq!((a.expanded[0].id.as_str(), a.expanded[0].via.as_deref()), ("X", Some("A1")));
    }

    #[test]
    fn dense_callback_is_fused_when_present() {
        let g = graph();
        let dense = |_q: &str, _k: usize| (vec!["FR-PAY-20".to_string()], Vec::new());
        let mut o = opts();
        o.dense = true;
        let a = ask(&g, &lex(&g, &Questions::default()), Some(&dense), None, &["ничего".into(), "похожего".into()], &o);
        assert_eq!(a.seeds[0].id, "FR-PAY-20");
    }

    #[test]
    fn render_shape_is_id_path_line_headline() {
        let g = graph();
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["FR-PAY-22".to_string()], &opts());
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
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["ъъъ".to_string(), "?!".to_string()], &opts());
        assert!(a.seeds.is_empty() && a.expanded.is_empty());
        assert_eq!(render(&a, &g, &opts()), "");
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &[String::new()], &opts());
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
        // An id the dialect recognises no family for is its own group, printed whole.
        assert!(out.contains("in families never declared: 1  MOB-M01-T3\n"), "{out}");
        assert!(out.contains("dangling edges: 2"));
    }

    #[test]
    fn verify_reports_what_is_held_aside_apart_from_what_dangles() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-1", "x", "", "docs/a.md", 1);
        e.edge("FR-1", "FR-9", EdgeKind::References, "body", "docs/a.md");
        e.edge("FR-1", "ISO-8601", EdgeKind::References, "body", "docs/a.md");
        e.edge("FR-1", "OQ-25", EdgeKind::References, "body", "docs/a.md");
        g.apply(e);
        g.settle();
        let text = verify(&g);
        assert!(text.contains("dangling edges: 1\n"), "{text}");
        assert!(text.contains("held aside: 2 edges to ids in 2 prefixes no line defines  ISO OQ\n"), "{text}");
        assert!(text.contains("gaps in declared families: 1  FR-9"), "{text}");
        let json: serde_json::Value = serde_json::from_str(&verify_json(&g)).unwrap();
        assert_eq!(json["held_aside"], 2);
        assert_eq!(json["held_aside_prefixes"], serde_json::json!(["ISO", "OQ"]));
        assert_eq!(json["cite_only"], serde_json::json!([]), "nothing cite-only is left where a reader can see it");
    }

    #[test]
    fn several_ids_in_one_question_all_become_seeds_in_word_order() {
        let g = graph();
        let words: Vec<String> = ["FR-PAY-22", "FR-PAY-20"].iter().map(|s| s.to_string()).collect();
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &words, &opts());
        assert_eq!(a.seeds.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), ["FR-PAY-22", "FR-PAY-20"]);
    }

    #[test]
    fn an_id_shaped_word_absent_from_the_graph_yields_no_seeds() {
        let g = graph();
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["FR-PAY-999".to_string()], &opts());
        assert!(a.seeds.is_empty());
        assert!(a.expanded.is_empty());
    }

    #[test]
    fn a_symbol_and_an_id_together_both_become_exact_seeds() {
        let g = graph();
        let words: Vec<String> = ["asGrosze", "FR-PAY-22"].iter().map(|s| s.to_string()).collect();
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &words, &opts());
        assert_eq!(a.seeds.len(), 2);
        assert_eq!(a.seeds[0].id, "sym:packages/contracts/src/money.ts::asGrosze");
        assert_eq!(a.seeds[1].id, "FR-PAY-22");
    }

    #[test]
    fn seeds_zero_drops_even_an_exact_match_and_its_expansion() {
        let g = graph();
        let mut o = opts();
        o.seeds = 0;
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["FR-PAY-22".to_string()], &o);
        assert!(a.seeds.is_empty());
        assert!(a.expanded.is_empty());
    }

    #[test]
    fn seeds_larger_than_the_pool_returns_only_what_exists() {
        let g = duplicated_symbol_graph();
        let mut o = opts();
        o.seeds = 100;
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["buildClientParams".to_string()], &o);
        assert_eq!(a.seeds.len(), 4);
    }

    #[test]
    fn render_includes_body_lines_only_when_bodies_is_set() {
        let g = graph();
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["FR-PAY-22".to_string()], &opts());
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
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["FR-PAY-22".to_string()], &opts());
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
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["???".to_string(), "!!!".to_string()], &opts());
        assert!(a.seeds.is_empty() && a.expanded.is_empty());
    }

    #[test]
    fn a_question_of_only_whitespace_words_yields_an_empty_answer_not_a_panic() {
        let g = graph();
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["   ".to_string(), "\t".to_string()], &opts());
        assert!(a.seeds.is_empty() && a.expanded.is_empty());
    }

    #[test]
    fn a_single_cyrillic_word_reaches_the_lexical_path() {
        let g = graph();
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["штраф".to_string()], &opts());
        assert_eq!(a.seeds[0].id, "FR-PAY-22");
    }

    #[test]
    fn expansion_skips_a_neighbour_that_is_already_a_seed() {
        let g = graph();
        let words: Vec<String> = ["FR-PAY-22", "N-151"].iter().map(|s| s.to_string()).collect();
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &words, &opts());
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
        let a = ask(&g, &lex(&g, &Questions::default()), None, None, &["FR-WEB-1".to_string()], &opts());
        assert_eq!(a.expanded.len(), 1);
        assert_eq!(a.expanded[0].id, "FR-WEB-30");
    }

}
