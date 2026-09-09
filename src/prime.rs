//! `repograph prime`: what a coding agent should be told about this repository at the start of a
//! session, in a few hundred tokens rather than a few thousand. Every number is read off the store
//! rather than claimed by prose — whether the questions are written, which embedder the vectors
//! belong to, how many families the documents define — because a brief that is wrong about the
//! store sends an agent to a command that will answer badly, which is worse than saying nothing.
//! Reads; never refreshes. A brief that rebuilds is a brief nobody can afford at session start.
use crate::enrich::{self, Questions};
use crate::model::{Graph, NodeKind};

pub struct Brief {
    pub docs: usize,
    pub code: usize,
    pub edges: usize,
    pub covered: usize,
    pub eligible: usize,
    pub families: usize,
    pub model: Option<String>,
}

/// Everything the brief says, from the store alone. `model` is the embedder the rows were written
/// by — `None` for a store with no rows for a name to be wrong about.
pub fn brief(graph: &Graph, questions: &Questions, families: usize, model: Option<&str>) -> Brief {
    let count = |f: fn(&NodeKind) -> bool| graph.nodes.values().filter(|n| f(&n.kind)).count();
    let (covered, eligible) = enrich::coverage(graph, questions);
    Brief {
        docs: count(|k| !matches!(k, NodeKind::File | NodeKind::Symbol)),
        code: count(|k| matches!(k, NodeKind::File | NodeKind::Symbol)),
        edges: graph.edges.len(),
        covered,
        eligible,
        families,
        model: model.map(str::to_string),
    }
}

impl Brief {
    /// The line a `SessionStart` hook prints. Facts first, then the five commands and the one rule:
    /// an agent that reads only the first line still learns whether this store can answer it.
    pub fn text(&self) -> String {
        let enriched = enrich::enriched(self.covered, self.eligible);
        format!(
            "repograph: {} doc nodes, {} code nodes, {} edges, {} id families\n\
             enriched={enriched} ({}/{} nodes)  model={}\n\
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
            self.covered,
            self.eligible,
            self.model.as_deref().unwrap_or("unnamed"),
        )
    }

    /// The same facts for a hook that would rather not parse prose.
    pub fn json(&self) -> String {
        format!(
            "{{\"nodes\":{{\"doc\":{},\"code\":{}}},\"edges\":{},\"enriched\":{},\
             \"questions\":{{\"covered\":{},\"eligible\":{}}},\"families\":{},\"model\":{}}}",
            self.docs,
            self.code,
            self.edges,
            enrich::enriched(self.covered, self.eligible),
            self.covered,
            self.eligible,
            self.families,
            match &self.model {
                Some(m) => format!("\"{m}\""),
                None => "null".to_string(),
            }
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
    /// contract and not a preference. The README next door is 92 kB; nothing but a failing test
    /// keeps this from growing into it.
    #[test]
    fn the_brief_fits_in_its_own_budget() {
        let b = brief(&graph(), &Questions::default(), 54, Some("intfloat/multilingual-e5-large"));
        let t = b.text();
        assert!(t.len() <= BUDGET, "the brief is {} bytes:\n{t}", t.len());
        assert!(t.lines().count() <= 12, "{t}");
    }

    /// A brief that claims an enriched store when the store is raw sends the agent to a command
    /// that will answer badly. Every number here is one the store can be asked for.
    #[test]
    fn the_brief_states_the_store_it_actually_read() {
        let b = brief(&graph(), &Questions::default(), 3, None);
        let t = b.text();
        assert!(t.contains("enriched=false (0/2 nodes)"), "{t}");
        assert!(t.contains("model=unnamed"), "a store with no recorded model says so: {t}");
        assert!(t.contains("2 doc nodes, 1 code nodes"), "{t}");
        assert!(b.json().contains("\"enriched\":false"), "{}", b.json());
        assert!(b.json().contains("\"model\":null"), "{}", b.json());
    }

    #[test]
    fn a_named_model_and_a_full_store_read_as_themselves() {
        let g = graph();
        let mut q = Questions::default();
        for id in ["FR-PAY-22", "FR-PAY-26"] {
            q.entries.insert(id.into(), crate::enrich::Entry { hash: String::new(), questions: vec!["q".into()] });
        }
        let b = brief(&g, &q, 54, Some("intfloat/multilingual-e5-small"));
        assert!(b.text().contains("enriched=true (2/2 nodes)"), "{}", b.text());
        assert!(b.json().contains("\"model\":\"intfloat/multilingual-e5-small\""), "{}", b.json());
        assert!(b.json().contains("\"families\":54"), "{}", b.json());
    }
}
