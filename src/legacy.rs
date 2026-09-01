use crate::ids::IdMatcher;
use crate::model::{EdgeKind, Extraction, Graph, NodeKind};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

/// A path the walker never yields, so `update` neither removes nor
/// re-extracts the frozen layer imported from a graphify graph.
pub const LEGACY_FILE: &str = "legacy:graphify";

#[derive(Deserialize)]
struct GNode {
    id: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    _origin: Option<String>,
    #[serde(default)]
    source_file: String,
    #[serde(default)]
    community_name: Option<String>,
}

#[derive(Deserialize)]
struct GEdge {
    source: String,
    target: String,
    #[serde(default)]
    relation: String,
    #[serde(default)]
    context: Option<String>,
}

#[derive(Deserialize)]
struct GGraph {
    nodes: Vec<GNode>,
    links: Vec<GEdge>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub edges_seen: usize,
    pub resolved_both: usize,
    pub resolved_one: usize,
    pub concepts_created: usize,
}

fn basename(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

/// Where a graphify node landed: an id already in `graph`, or a freshly
/// minted `LegacyConcept` standing in for one that resolution rules 1-2 miss.
enum Resolved {
    Real(String),
    Concept(String),
}

impl Resolved {
    fn id(&self) -> &str {
        match self {
            Resolved::Real(id) | Resolved::Concept(id) => id,
        }
    }

    fn is_real(&self) -> bool {
        matches!(self, Resolved::Real(_))
    }
}

fn resolve(
    graph: &Graph,
    ids: &IdMatcher,
    by_label: &HashMap<(String, String), String>,
    gn: &GNode,
) -> Resolved {
    if let Some(hit) = ids.find_all(&gn.label).into_iter().find(|h| graph.nodes.contains_key(&h.id)) {
        return Resolved::Real(hit.id);
    }
    let key = (basename(&gn.source_file).to_string(), gn.label.to_lowercase());
    if let Some(id) = by_label.get(&key) {
        return Resolved::Real(id.clone());
    }
    Resolved::Concept(format!("legacy:{}", gn.id))
}

pub fn import(graph: &mut Graph, ids: &IdMatcher, json: &str) -> Result<Report> {
    let g: GGraph = serde_json::from_str(json).context("parse graphify graph.json")?;

    let semantic: HashMap<&str, &GNode> = g
        .nodes
        .iter()
        .filter(|n| n._origin.as_deref() != Some("ast"))
        .map(|n| (n.id.as_str(), n))
        .collect();

    let by_label: HashMap<(String, String), String> = graph
        .nodes
        .values()
        .map(|n| ((basename(&n.file).to_string(), n.label.to_lowercase()), n.id.clone()))
        .collect();

    let mut resolved: HashMap<&str, Resolved> = HashMap::new();
    let mut report = Report::default();
    let mut ex = Extraction::default();

    for e in &g.links {
        let (Some(&sn), Some(&tn)) = (semantic.get(e.source.as_str()), semantic.get(e.target.as_str())) else {
            continue;
        };
        report.edges_seen += 1;

        for (gid, gn) in [(e.source.as_str(), sn), (e.target.as_str(), tn)] {
            if let Entry::Vacant(slot) = resolved.entry(gid) {
                let r = resolve(graph, ids, &by_label, gn);
                match &r {
                    Resolved::Real(id) => {
                        // Prior graph knowledge wins; a legacy import only fills gaps.
                        if let Some(n) = graph.nodes.get_mut(id) {
                            if n.community.is_none() {
                                n.community = gn.community_name.clone();
                            }
                        }
                    }
                    Resolved::Concept(id) => {
                        // A second import re-derives the same "legacy:<gid>" id (see the
                        // `file = LEGACY_FILE` note below for why rules 1-2 can't redirect
                        // it); skip staging once the node already exists so the counter
                        // reports concepts actually created by this run, not ones
                        // re-derived from a prior one.
                        if !graph.nodes.contains_key(id) {
                            report.concepts_created += 1;
                            // `file = LEGACY_FILE` does double duty: it is what keeps
                            // `remove_file` from ever touching a concept (the walker
                            // never yields this path, same as for the edges below), and
                            // it is what keeps a later import from re-resolving this
                            // concept through rule 2 — `by_label`'s key is (basename of
                            // a node's `file`, label), and no real graphify `source_file`
                            // is ever the literal string `LEGACY_FILE`. The graphify
                            // source lives in `body` instead, where it stays searchable.
                            ex.node(NodeKind::LegacyConcept, id, &gn.label, &gn.source_file, LEGACY_FILE, 0);
                            if let Some(n) = ex.nodes.last_mut() {
                                n.community = gn.community_name.clone();
                            }
                        }
                    }
                }
                slot.insert(r);
            }
        }

        let sr = &resolved[e.source.as_str()];
        let tr = &resolved[e.target.as_str()];
        match (sr.is_real(), tr.is_real()) {
            (true, true) => report.resolved_both += 1,
            (false, false) => {}
            _ => report.resolved_one += 1,
        }
        let ctx = match &e.context {
            Some(c) if !c.is_empty() => format!("{}: {c}", e.relation),
            _ => e.relation.clone(),
        };
        ex.edge(sr.id(), tr.id(), EdgeKind::Legacy, &ctx, LEGACY_FILE);
    }

    graph.apply(ex);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "CancellationPolicy", "", "docs/prd/06-payments.md", 385);
        e.node(NodeKind::Requirement, "N-151", "no free-text policies", "", "docs/never-list.md", 3);
        e.node(NodeKind::Requirement, "FR-TOOL-39", "Refund flow", "", "docs/prd/06-payments.md", 500);
        g.apply(e);
        g
    }

    fn run(g: &mut Graph) -> Report {
        let cfg = crate::config::Config::default();
        let ids = IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
        let json = std::fs::read_to_string(format!(
            "{}/tests/fixtures/graphify-graph.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        import(g, &ids, &json).unwrap()
    }

    #[test]
    fn resolves_by_id_then_label_then_creates_concepts() {
        let mut g = base();
        let r = run(&mut g);
        assert_eq!(r, Report { edges_seen: 3, resolved_both: 2, resolved_one: 1, concepts_created: 1 });
        assert!(g.edges.iter().any(|e| e.source == "FR-PAY-22" && e.target == "N-151" && e.kind == EdgeKind::Legacy && e.context == "references"));
        assert!(g.edges.iter().any(|e| e.source == "FR-PAY-22" && e.target == "FR-TOOL-39" && e.context == "conceptually_related_to"));
        let ghost = &g.nodes["legacy:ghost"];
        assert_eq!(ghost.kind, NodeKind::LegacyConcept);
        assert_eq!(ghost.file, LEGACY_FILE);
        assert_eq!(ghost.body, "99-x.md");
        assert_eq!(g.nodes["FR-PAY-22"].community.as_deref(), Some("Payments core"));
        assert!(!g.edges.iter().any(|e| e.target.contains("ast_sym")));
    }

    #[test]
    fn import_is_idempotent_and_survives_update_removal() {
        let mut g = base();
        let r1 = run(&mut g);
        let (n, e) = (g.nodes.len(), g.edges.len());
        let r2 = run(&mut g);
        assert_eq!((g.nodes.len(), g.edges.len()), (n, e));
        // `edges_seen`/`resolved_both`/`resolved_one` are pure functions of the input graph
        // and must reproduce exactly; `concepts_created` legitimately drops to zero on a
        // re-run because every concept from the first run already exists (that is the fix
        // in src/legacy.rs guarding `Resolved::Concept` on `!graph.nodes.contains_key`).
        assert_eq!((r2.edges_seen, r2.resolved_both, r2.resolved_one), (r1.edges_seen, r1.resolved_both, r1.resolved_one));
        assert_eq!(r2.concepts_created, 0);

        // "99-x.md" is `ghost`'s graphify source_file, now stored in `body`, not `file`.
        // Removing it as a repograph-tracked path must not touch the concept at all.
        g.remove_file("99-x.md");
        g.remove_file("docs/prd/06-payments.md");
        assert!(g.nodes.contains_key("legacy:ghost"));
        let ghost = &g.nodes["legacy:ghost"];
        assert_eq!(ghost.file, LEGACY_FILE);
        assert_eq!(ghost.line, 0);
        assert!(g.edges.iter().any(|e| e.kind == EdgeKind::Legacy));
    }
}
