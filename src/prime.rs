//! `repograph prime`: what a coding agent should be told about this repository at the start of a
//! session, in a few hundred tokens rather than a few thousand. Every number is read off the store
//! rather than claimed by prose — which embedder the vectors belong to, how many families the documents define — because a brief that is wrong about the
//! store sends an agent to a command that will answer badly, which is worse than saying nothing.
//! Reads; never refreshes. A brief that rebuilds is a brief nobody can afford at session start.
use crate::model::{Graph, NodeKind};

pub struct Brief {
    pub docs: usize,
    pub code: usize,
    pub edges: usize,
    pub families: usize,
    pub model: Option<String>,
}

/// Everything the brief says, from the store alone. `model` is the embedder the rows were written
/// by — `None` for a store with no rows for a name to be wrong about.
pub fn brief(graph: &Graph, families: usize, model: Option<&str>) -> Brief {
    let count = |f: fn(&NodeKind) -> bool| graph.nodes.values().filter(|n| f(&n.kind)).count();
    Brief {
        docs: count(|k| !matches!(k, NodeKind::File | NodeKind::Symbol | NodeKind::Text)),
        code: count(|k| matches!(k, NodeKind::File | NodeKind::Symbol)),
        edges: graph.edges.len(),
        families,
        model: model.map(str::to_string),
    }
}

impl Brief {
    /// The line a `SessionStart` hook prints. Facts first, then the five commands and the one rule:
    /// an agent that reads only the first line still learns whether this store can answer it.
    pub fn text(&self) -> String {
        format!(
            "repograph: {} doc nodes, {} code nodes, {} edges, {} id families\n\
             model={}\n\
             ask <words> — what the docs and code say about it, by meaning not by grep\n\
             impact <symbol> — who calls it, how far, how risky a change is\n\
             changes — what the working diff touches and who reaches it\n\
             trace <from> <to> — the call chain between two symbols\n\
             explain <id> — one node, its neighbours and where it is written\n\
             Ask the graph before grepping for a concept; grep is still right for a literal.\n",
            self.docs,
            self.code,
            self.edges,
            self.families,
            self.model.as_deref().unwrap_or("unnamed"),
        )
    }

    /// The same facts for a hook that would rather not parse prose.
    pub fn json(&self) -> String {
        format!(
            "{{\"nodes\":{{\"doc\":{},\"code\":{}}},\"edges\":{},\"families\":{},\"model\":{}}}",
            self.docs,
            self.code,
            self.edges,
            self.families,
            // Through a JSON writer rather than quoted by hand: `embed_model` is never checked
            // for shell safety the way the two command models are — it never reaches a shell — so
            // a name carrying a quote would otherwise make this object unparseable, and the hook
            // that reads it would go silent rather than fail.
            serde_json::to_string(&self.model).unwrap_or_else(|_| "null".to_string())
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Extraction;

    /// Bytes the text form may take. It is read at every session start and after every compaction,
    /// so its size is a contract: the drafted 530 plus room for a long model name and a family
    /// count that grew. The test below is the only thing standing between this and the README.
    const BUDGET: usize = 600;

    fn graph() -> Graph {
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "отмена", "тело", "a.md", 1);
        e.node(NodeKind::Requirement, "FR-PAY-26", "возврат", "тело", "a.md", 9);
        e.node(NodeKind::File, "file:a.ts", "a.ts", "The auth surface.", "a.ts", 1);
        let mut g = Graph::default();
        g.apply(e);
        g
    }

    /// The brief is read at every session start and after every compaction, so its size is a
    /// contract and not a preference. The README next door is 45 kB; nothing but a failing test
    /// keeps this from growing into it.
    #[test]
    fn the_brief_fits_in_its_own_budget() {
        let b = brief(&graph(), 54, Some("Alibaba-NLP/gte-multilingual-base"));
        let t = b.text();
        assert!(t.len() <= BUDGET, "the brief is {} bytes:\n{t}", t.len());
        assert!(t.lines().count() <= 12, "{t}");
    }

    /// A brief that misstates the store sends the agent to a command that will answer badly. Every number here is one the store can be asked for.
    #[test]
    fn the_brief_states_the_store_it_actually_read() {
        let b = brief(&graph(), 3, None);
        let t = b.text();
        assert!(t.contains("model=unnamed"), "a store with no recorded model says so: {t}");
        assert!(t.contains("2 doc nodes, 1 code nodes"), "{t}");
        assert!(b.json().contains("\"model\":null"), "{}", b.json());
    }

    #[test]
    fn a_named_model_reads_as_itself() {
        let b = brief(&graph(), 54, Some("intfloat/multilingual-e5-small"));
        assert!(b.text().contains("model=intfloat/multilingual-e5-small"), "{}", b.text());
        assert!(b.json().contains("\"model\":\"intfloat/multilingual-e5-small\""), "{}", b.json());
        assert!(b.json().contains("\"families\":54"), "{}", b.json());
    }

    #[test]
    fn a_text_file_is_counted_as_neither_a_document_nor_code() {
        use crate::model::{Extraction, Graph, NodeKind};
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-1", "cancel", "", "a.md", 1);
        e.node(NodeKind::Text, "file:ops.yaml", "ops.yaml", "deploy: blue\n", "ops.yaml", 1);
        g.apply(e);
        let b = brief(&g, 0, None);
        assert_eq!((b.docs, b.code), (1, 0));
    }
}
