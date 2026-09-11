use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum NodeKind { Requirement, Entity, Invariant, Adr, Milestone, Task, File, Symbol, LegacyConcept }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EdgeKind { References, Declares, Links, Implements, Imports, ReExports, Calls, Extends, DecoratedBy, Legacy }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    #[serde(default)] pub body: String,
    pub file: String,
    pub line: u32,
    /// Last line of the declaration, for mapping a diff hunk onto the symbol it sits in; 0 on
    /// nodes that have no extent of their own (decorators, files, document ids).
    #[serde(default)] pub end: u32,
    #[serde(default)] pub files: BTreeSet<String>,
    #[serde(default)] pub community: Option<String>,
}

impl Node {
    /// Symbols and files: the population `enrich --code` asks about and the indexes keep apart.
    pub fn is_code(&self) -> bool { matches!(self.kind, NodeKind::Symbol | NodeKind::File) }

    /// What the retrievers index for this node. A document is its body; a symbol is the line
    /// that declares it, not the comment above it — 4,300 doc comments in the passage index
    /// moved BM25's length and term statistics enough to cost the recorded suite two paraphrases
    /// without seating a single file, so the prose an author wrote about code reaches the index
    /// only through the questions generated from it (`enrich --code`). A file is not indexed.
    pub fn indexed_body(&self) -> &str {
        match self.kind {
            NodeKind::Symbol => self.body.lines().last().unwrap_or(""),
            _ => &self.body,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
    #[serde(default)] pub context: String,
    pub file: String,
}

#[derive(Debug, Default)]
pub struct Extraction { pub nodes: Vec<Node>, pub edges: Vec<Edge> }

impl Extraction {
    pub fn node(&mut self, kind: NodeKind, id: &str, label: &str, body: &str, file: &str, line: u32) {
        self.nodes.push(Node {
            id: id.to_string(), kind, label: label.to_string(), body: body.to_string(),
            file: file.to_string(), line, end: 0, files: BTreeSet::from([file.to_string()]), community: None,
        });
    }

    pub fn node_span(&mut self, kind: NodeKind, id: &str, label: &str, body: &str, file: &str, (line, end): (u32, u32)) {
        self.node(kind, id, label, body, file, line);
        self.nodes.last_mut().expect("node just pushed").end = end;
    }

    pub fn edge(&mut self, source: &str, target: &str, kind: EdgeKind, context: &str, file: &str) {
        self.edges.push(Edge {
            source: source.to_string(), target: target.to_string(), kind,
            context: context.to_string(), file: file.to_string(),
        });
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: BTreeMap<String, Node>,
    pub edges: BTreeSet<Edge>,
    /// Citations of ids in a family no definition declares — `ISO-8601`, a ticket number, a
    /// prefix the corpus cites and never defines. Kept apart so that no reader follows them and
    /// no count reports them, and kept at all so that the day a line defines the family they
    /// are released by `settle` without a document being re-read. A store written before this
    /// field existed reads as holding none.
    #[serde(default)] pub pending: BTreeSet<Edge>,
}

impl Graph {
    pub fn apply(&mut self, ex: Extraction) {
        for n in ex.nodes {
            match self.nodes.get_mut(&n.id) {
                // The first declaring file stays primary; `run_update` re-reads a surviving
                // declarer whenever the primary goes, so `path:line` never mixes two files.
                Some(existing) => { existing.files.extend(n.files); }
                None => { self.nodes.insert(n.id.clone(), n); }
            }
        }
        self.edges.extend(ex.edges);
    }

    /// Every edge sorted to the side of the line its target's family is on: cited-and-declared
    /// in `edges`, cited-and-not in `pending`. Run once after a batch of `apply`s, because only
    /// then is it known which families the batch declared — a file citing `OQ-25` may be read
    /// before the file that defines `OQ-1`.
    pub fn settle(&mut self) {
        let (ids, milestones) = crate::families::of_graph(self);
        let admitted = |target: &str| match crate::families::classify(target) {
            Some(crate::families::Family::Id(f)) => ids.contains_key(f),
            Some(crate::families::Family::Milestone(f)) => milestones.contains_key(f),
            None => true,
        };
        let all: Vec<Edge> = std::mem::take(&mut self.edges).into_iter().chain(std::mem::take(&mut self.pending)).collect();
        for e in all {
            if admitted(&e.target) { self.edges.insert(e); } else { self.pending.insert(e); }
        }
    }

    pub fn remove_file(&mut self, rel: &str) {
        self.edges.retain(|e| e.file != rel);
        self.pending.retain(|e| e.file != rel);
        let mut gone = Vec::new();
        for (id, n) in self.nodes.iter_mut() {
            n.files.remove(rel);
            if n.files.is_empty() {
                gone.push(id.clone());
            } else if n.file == rel {
                n.file = n.files.iter().next().unwrap().clone();
            }
        }
        for id in gone {
            self.nodes.remove(&id);
        }
    }

    pub fn neighbours(&self, id: &str) -> Vec<&Edge> {
        self.edges.iter().filter(|e| e.source == id || e.target == id).collect()
    }

    pub fn dangling(&self) -> Vec<&Edge> {
        self.edges.iter().filter(|e| !self.nodes.contains_key(&e.target)).collect()
    }
}

pub trait Extractor {
    fn extract(&self, rel: &str, text: &str) -> Extraction;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(file: &str) -> Extraction {
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "CancellationPolicy", "", file, 385);
        e.node(NodeKind::Entity, "entity:CancellationPolicy", "CancellationPolicy", "", file, 385);
        e.edge("FR-PAY-22", "entity:CancellationPolicy", EdgeKind::References, "title", file);
        e.edge("FR-PAY-22", "N-151", EdgeKind::References, "body", file);
        e
    }

    fn settled(files: &[&str]) -> Graph {
        let mut g = Graph::default();
        for f in files { g.apply(ex(f)); }
        g.settle();
        g
    }

    #[test]
    fn apply_then_remove_file_restores_empty_graph() {
        let mut g = Graph::default();
        g.apply(ex("docs/06.md"));
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.edges.len(), 2);
        g.remove_file("docs/06.md");
        assert!(g.nodes.is_empty() && g.edges.is_empty() && g.pending.is_empty());
    }

    #[test]
    fn a_citation_of_a_family_no_node_declares_is_held_aside() {
        let g = settled(&["docs/06.md"]);
        // `N-151` is cited and no `N-…` node exists: the edge is kept, and kept out of sight.
        assert_eq!(g.edges.len(), 1, "{:?}", g.edges);
        assert_eq!(g.pending.iter().map(|e| e.target.as_str()).collect::<Vec<_>>(), vec!["N-151"]);
        assert!(g.dangling().is_empty(), "held-aside edges are not dangling: nothing a reader follows points nowhere");
    }

    #[test]
    fn a_family_that_appears_releases_what_was_held_without_a_re_read() {
        let mut g = settled(&["docs/06.md"]);
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "N-001", "first N", "", "docs/n.md", 1);
        g.apply(e);
        g.settle();
        assert!(g.pending.is_empty());
        assert!(g.edges.iter().any(|e| e.target == "N-151"), "released into the visible graph");
        // Still dangling — `N-151` itself is not defined — which is now a gap in a declared family.
        assert_eq!(g.dangling().len(), 1);
    }

    #[test]
    fn a_family_that_vanishes_takes_its_citations_back_out_of_sight() {
        let mut g = settled(&["docs/06.md"]);
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "N-001", "first N", "", "docs/n.md", 1);
        g.apply(e);
        g.settle();
        g.remove_file("docs/n.md");
        g.settle();
        assert_eq!(g.pending.len(), 1);
        assert!(!g.edges.iter().any(|e| e.target == "N-151"));
    }

    #[test]
    fn removing_a_file_drops_the_edges_it_held_aside_too() {
        let mut g = settled(&["docs/06.md", "docs/07.md"]);
        assert_eq!(g.pending.len(), 2);
        g.remove_file("docs/06.md");
        assert_eq!(g.pending.iter().map(|e| e.file.as_str()).collect::<Vec<_>>(), vec!["docs/07.md"]);
    }

    #[test]
    fn a_target_that_is_not_an_id_is_never_held_aside() {
        let g = settled(&["docs/06.md"]);
        assert!(g.edges.iter().any(|e| e.target == "entity:CancellationPolicy"));
    }

    #[test]
    fn node_declared_by_two_files_survives_one_removal() {
        let mut g = Graph::default();
        g.apply(ex("docs/06.md"));
        g.apply(ex("docs/07.md"));
        g.remove_file("docs/06.md");
        let n = &g.nodes["entity:CancellationPolicy"];
        assert_eq!(n.file, "docs/07.md");
        assert_eq!(n.files.len(), 1);
    }

    #[test]
    fn dangling_lists_edges_without_target_node() {
        let mut g = Graph::default();
        g.apply(ex("docs/06.md"));
        let d: Vec<_> = g.dangling().into_iter().map(|e| e.target.as_str()).collect();
        assert_eq!(d, vec!["N-151"]);
    }

    #[test]
    fn neighbours_are_bidirectional() {
        let mut g = Graph::default();
        g.apply(ex("docs/06.md"));
        assert_eq!(g.neighbours("entity:CancellationPolicy").len(), 1);
        assert_eq!(g.neighbours("FR-PAY-22").len(), 2);
    }

    #[test]
    fn neighbours_of_an_unknown_id_is_empty() {
        let mut g = Graph::default();
        g.apply(ex("docs/06.md"));
        assert!(g.neighbours("nope").is_empty());
    }

    #[test]
    fn removing_a_non_primary_file_keeps_the_primary_file_field() {
        let mut g = Graph::default();
        g.apply(ex("docs/06.md"));
        g.apply(ex("docs/07.md"));
        // "docs/06.md" was applied first, so it stays the primary `file`.
        g.remove_file("docs/07.md");
        let n = &g.nodes["entity:CancellationPolicy"];
        assert_eq!(n.file, "docs/06.md");
        assert_eq!(n.files.len(), 1);
    }

    #[test]
    fn reapplying_the_same_extraction_does_not_duplicate_edges_or_nodes() {
        let mut g = Graph::default();
        g.apply(ex("docs/06.md"));
        g.apply(ex("docs/06.md"));
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.edges.len(), 2);
    }

    #[test]
    fn dangling_excludes_edges_whose_target_node_exists() {
        let mut g = Graph::default();
        g.apply(ex("docs/06.md"));
        let dangling: Vec<&str> = g.dangling().into_iter().map(|e| e.target.as_str()).collect();
        assert!(!dangling.contains(&"entity:CancellationPolicy"));
    }
}

#[cfg(test)]
mod indexed_body_tests {
    use super::*;

    #[test]
    fn a_symbol_indexes_its_declaring_line_and_a_document_its_body() {
        let mut e = Extraction::default();
        e.node(NodeKind::Symbol, "sym:a.ts::f", "f", "Ends every session.\nexport function f() {", "a.ts", 3);
        e.node(NodeKind::Requirement, "FR-X-1", "t", "первая строка\nвторая", "d.md", 1);
        e.node(NodeKind::Symbol, "deco:Injectable", "Injectable", "", "a.ts", 3);
        assert_eq!(e.nodes[0].indexed_body(), "export function f() {");
        assert_eq!(e.nodes[1].indexed_body(), "первая строка\nвторая");
        assert_eq!(e.nodes[2].indexed_body(), "");
    }
}

