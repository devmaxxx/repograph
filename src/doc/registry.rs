use crate::ids::IdMatcher;
use crate::model::{EdgeKind, Extraction, Extractor, NodeKind};
use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Registry { #[serde(default)] invariants: Vec<Row> }

#[derive(Deserialize)]
struct Row {
    id: String,
    #[serde(default)] statement: String,
    #[serde(default)] mechanism: String,
    #[serde(default)] test_ref: Option<String>,
    #[serde(default)] basis: Option<String>,
}

pub struct RegistryExtractor { ids: IdMatcher }

impl RegistryExtractor {
    pub fn new(ids: IdMatcher) -> RegistryExtractor { RegistryExtractor { ids } }
}

fn bold_span() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // `(?s)` because the real corpus wraps a statement's bold span across the
    // YAML block's line breaks; a line-anchored regex would silently underfill it.
    RE.get_or_init(|| Regex::new(r"(?s)\*\*(.*?)\*\*").unwrap())
}

/// Every row in the real registry opens with a `**…**` span that runs past the
/// closing `**` into trailing prose, so the label is that span's inner text, not
/// the row's first line. A row without a bold span (none in the corpus today)
/// falls back to the old first-line rule.
fn label_of(statement: &str) -> String {
    match bold_span().captures(statement) {
        Some(c) => c[1].trim().to_string(),
        None => statement.lines().next().unwrap_or("").replace("**", "").trim().to_string(),
    }
}

impl Extractor for RegistryExtractor {
    fn extract(&self, rel: &str, text: &str) -> Extraction {
        let mut ex = Extraction::default();
        let file_id = format!("file:{rel}");
        ex.node(NodeKind::File, &file_id, rel, "", rel, 1);
        let reg = match serde_yaml::from_str::<Registry>(text) {
            Ok(r) => r,
            Err(e) => { eprintln!("registry {rel}: {e}"); return ex; }
        };
        for (i, row) in reg.invariants.iter().enumerate() {
            let label = label_of(&row.statement);
            // Row order is the only line information YAML gives cheaply; good enough for `path:line`.
            ex.node(NodeKind::Invariant, &row.id, &label, row.mechanism.trim(), rel, i as u32 + 1);
            ex.edge(&file_id, &row.id, EdgeKind::Declares, "", rel);
            if let Some(t) = &row.test_ref {
                let target = if t.contains('/') { format!("file:{t}") } else { format!("gate:{t}") };
                ex.edge(&row.id, &target, EdgeKind::Implements, "test_ref", rel);
            }
            if let Some(b) = &row.basis {
                for hit in self.ids.find_all(b) {
                    ex.edge(&row.id, &hit.id, EdgeKind::References, "basis", rel);
                }
            }
        }
        ex
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex() -> Extraction {
        let cfg = crate::config::Config::default();
        let text = std::fs::read_to_string(format!("{}/tests/fixtures/constitution.yaml", env!("CARGO_MANIFEST_DIR"))).unwrap();
        RegistryExtractor::new(IdMatcher::new(&cfg.id_families, &cfg.milestone_families)).extract("docs/constitution.yaml", &text)
    }

    #[test]
    fn rows_become_invariants_with_implements_edges() {
        let ex = ex();
        let n = ex.nodes.iter().find(|n| n.id == "INV-01").unwrap();
        assert_eq!(n.kind, NodeKind::Invariant);
        assert_eq!(n.label, "Ранжирование не зависит от платного признака.");
        assert!(n.body.contains("ranking_input_whitelist_test"));
        assert!(ex.edges.iter().any(|e| e.source == "INV-01" && e.target == "gate:ranking_input_whitelist_test" && e.kind == EdgeKind::Implements));
        assert!(ex.edges.iter().any(|e| e.source == "INV-06" && e.target == "file:packages/db/test/contours.spec.ts" && e.kind == EdgeKind::Implements));
        assert!(ex.edges.iter().any(|e| e.source == "INV-01" && e.target == "N-039" && e.kind == EdgeKind::References));
        assert!(!ex.edges.iter().any(|e| e.source == "INV-02" && e.kind == EdgeKind::Implements));
    }

    #[test]
    fn non_registry_yaml_is_harmless() {
        let cfg = crate::config::Config::default();
        let r = RegistryExtractor::new(IdMatcher::new(&cfg.id_families, &cfg.milestone_families));
        let ex = r.extract("x.yaml", "foo: bar\n");
        assert_eq!(ex.nodes.len(), 1);
        assert!(ex.edges.is_empty());
    }
}
