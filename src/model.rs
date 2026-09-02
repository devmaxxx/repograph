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
    #[serde(default)] pub files: BTreeSet<String>,
    #[serde(default)] pub community: Option<String>,
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
            file: file.to_string(), line, files: BTreeSet::from([file.to_string()]), community: None,
        });
    }

    pub fn edge(&mut self, source: &str, target: &str, kind: EdgeKind, context: &str, file: &str) {
        self.edges.push(Edge {
            source: source.to_string(), target: target.to_string(), kind,
            context: context.to_string(), file: file.to_string(),
        });
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: BTreeMap<String, Node>,
    pub edges: BTreeSet<Edge>,
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

    pub fn remove_file(&mut self, rel: &str) {
        self.edges.retain(|e| e.file != rel);
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

    #[test]
    fn apply_then_remove_file_restores_empty_graph() {
        let mut g = Graph::default();
        g.apply(ex("docs/06.md"));
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.edges.len(), 2);
        g.remove_file("docs/06.md");
        assert!(g.nodes.is_empty() && g.edges.is_empty());
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
