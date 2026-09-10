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

/// The ids the rows of a registry declare, in the order `extract` would build them into nodes.
/// The family scan reads them from here so that one place knows the registry's shape: a row's own
/// id is never weighed against a family list, so every row defines the family it is written in.
pub fn declared_ids(text: &str) -> Vec<String> {
    serde_yaml::from_str::<Registry>(text)
        .map(|r| r.invariants.into_iter().map(|row| row.id).collect())
        .unwrap_or_default()
}

pub struct RegistryExtractor;

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
                for hit in crate::ids::generic().find_all(b) {
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
        let text = std::fs::read_to_string(format!("{}/tests/fixtures/constitution.yaml", env!("CARGO_MANIFEST_DIR"))).unwrap();
        RegistryExtractor.extract("docs/constitution.yaml", &text)
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
        let r = RegistryExtractor;
        let ex = r.extract("x.yaml", "foo: bar\n");
        assert_eq!(ex.nodes.len(), 1);
        assert!(ex.edges.is_empty());
    }

    #[test]
    fn label_of_falls_back_to_the_trimmed_first_line_without_a_bold_span() {
        assert_eq!(label_of("No bold marker here\nsecond line"), "No bold marker here");
        assert_eq!(label_of("**stray unclosed marker\nsecond line"), "stray unclosed marker");
    }

    #[test]
    fn label_of_takes_the_bold_spans_inner_text_even_across_lines() {
        assert_eq!(label_of("**Ranked\nacross two lines.** trailing prose"), "Ranked\nacross two lines.");
    }

    #[test]
    fn rows_declare_nodes_with_sequential_line_numbers_and_declares_edges() {
        let yaml = "invariants:\n  - id: INV-A\n    statement: \"**A.**\"\n  - id: INV-B\n    statement: \"**B.**\"\n";
        let ex = RegistryExtractor.extract("docs/x.yaml", yaml);
        let a = ex.nodes.iter().find(|n| n.id == "INV-A").unwrap();
        let b = ex.nodes.iter().find(|n| n.id == "INV-B").unwrap();
        assert_eq!((a.line, b.line), (1, 2));
        assert!(ex.edges.iter().any(|e| e.source == "file:docs/x.yaml" && e.target == "INV-A" && e.kind == EdgeKind::Declares));
        assert!(ex.edges.iter().any(|e| e.source == "file:docs/x.yaml" && e.target == "INV-B" && e.kind == EdgeKind::Declares));
    }

    #[test]
    fn a_row_without_a_test_ref_has_no_implements_edge() {
        let yaml = "invariants:\n  - id: INV-A\n    statement: \"**A.**\"\n";
        let ex = RegistryExtractor.extract("docs/x.yaml", yaml);
        assert!(!ex.edges.iter().any(|e| e.kind == EdgeKind::Implements));
    }

    #[test]
    fn a_row_missing_the_required_id_field_yields_only_the_file_node() {
        let yaml = "invariants:\n  - statement: \"**Untitled.**\"\n";
        let ex = RegistryExtractor.extract("docs/x.yaml", yaml);
        assert_eq!(ex.nodes.len(), 1);
        assert_eq!(ex.nodes[0].kind, NodeKind::File);
        assert!(ex.edges.is_empty());
    }

    // The `id` field is what fails the row above; a row that parses but carries no `statement`
    // is a different case — it is kept, just with an empty label.
    #[test]
    fn a_row_with_no_statement_field_is_kept_with_an_empty_label() {
        let yaml = "invariants:\n  - id: INV-A\n";
        let ex = RegistryExtractor.extract("docs/x.yaml", yaml);
        let n = ex.nodes.iter().find(|n| n.id == "INV-A").unwrap();
        assert_eq!(n.label, "");
    }

    // One row failing to deserialize fails `serde_yaml::from_str::<Registry>` for the whole
    // document, not just that row — every other row is dropped along with it.
    #[test]
    fn a_single_malformed_row_drops_every_row_in_the_registry_not_just_itself() {
        let yaml = "invariants:\n  - id: INV-A\n    statement: \"**A.**\"\n  - statement: \"**no id.**\"\n  - id: INV-C\n    statement: \"**C.**\"\n";
        let ex = RegistryExtractor.extract("docs/x.yaml", yaml);
        assert_eq!(ex.nodes.len(), 1);
        assert_eq!(ex.nodes[0].kind, NodeKind::File);
    }

    // Nothing here dedups by id: the extractor trusts the registry file and declares whatever
    // rows it's given, one node and one `Declares` edge per row.
    #[test]
    fn duplicate_ids_in_the_registry_produce_a_node_and_declares_edge_per_row() {
        let yaml = "invariants:\n  - id: INV-DUP\n    statement: \"**First.**\"\n  - id: INV-DUP\n    statement: \"**Second.**\"\n";
        let ex = RegistryExtractor.extract("docs/x.yaml", yaml);
        assert_eq!(ex.nodes.iter().filter(|n| n.id == "INV-DUP").count(), 2);
        assert_eq!(ex.edges.iter().filter(|e| e.target == "INV-DUP" && e.kind == EdgeKind::Declares).count(), 2);
    }

    // A row's own id is a definition whatever prefix it is written in — the node is declared from
    // the row, not looked up — and only the citations inside `basis` are read through the grammar.
    #[test]
    fn a_row_declares_its_invariant_whatever_prefix_the_id_is_written_in() {
        let yaml = "invariants:\n  - id: FR-X-1\n    statement: \"**A prefix no other line defines.**\"\n";
        let ex = RegistryExtractor.extract("docs/x.yaml", yaml);
        let n = ex.nodes.iter().find(|n| n.id == "FR-X-1").unwrap();
        assert_eq!(n.kind, NodeKind::Invariant);
    }

    // Whether `FR-X` is a family is the graph's question, asked after every file is read, so the
    // citation is extracted here and sorted to its side of the line by `Graph::settle`.
    #[test]
    fn a_basis_reference_to_a_prefix_nothing_defines_is_still_an_edge() {
        let yaml = "invariants:\n  - id: INV-A\n    statement: \"**A.**\"\n    basis: \"по FR-X-1\"\n";
        let ex = RegistryExtractor.extract("docs/x.yaml", yaml);
        assert!(ex.edges.iter().any(|e| e.source == "INV-A" && e.target == "FR-X-1" && e.kind == EdgeKind::References));
    }

    #[test]
    fn a_basis_string_with_several_ids_produces_an_edge_for_each() {
        let yaml = "invariants:\n  - id: INV-A\n    statement: \"**A.**\"\n    basis: \"по FR-WEB-1 и FR-WEB-2\"\n";
        let ex = RegistryExtractor.extract("docs/x.yaml", yaml);
        let refs: Vec<&str> = ex.edges.iter().filter(|e| e.source == "INV-A" && e.kind == EdgeKind::References).map(|e| e.target.as_str()).collect();
        assert_eq!(refs, ["FR-WEB-1", "FR-WEB-2"]);
    }
}
