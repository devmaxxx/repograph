//! A file no doc or code glob claims, read as text (spec §11): one node, its whole text the body.
//! No node below it and no edge out of it — what a YAML key or a workflow's script means is L10's
//! question, not this reader's.

use crate::model::{Extraction, Extractor, NodeKind};

#[allow(dead_code)] // Wired into the walk's dispatch by the next commit.
pub struct TextExtractor;

impl Extractor for TextExtractor {
    fn extract(&self, rel: &str, text: &str) -> Extraction {
        let mut ex = Extraction::default();
        ex.node(NodeKind::Text, &format!("file:{rel}"), rel, text, rel, 1);
        ex
    }
}

#[cfg(test)]
mod tests {
    use super::TextExtractor;
    use crate::model::{Extractor, NodeKind};

    #[test]
    fn a_text_file_is_one_text_node_holding_its_text_and_nothing_else() {
        let ex = TextExtractor.extract("ops/deploy.yaml", "strategy: blue-green\n");
        assert_eq!(ex.nodes.len(), 1);
        let n = &ex.nodes[0];
        assert_eq!((n.kind, n.id.as_str(), n.label.as_str(), n.file.as_str()), (NodeKind::Text, "file:ops/deploy.yaml", "ops/deploy.yaml", "ops/deploy.yaml"));
        assert_eq!(n.body, "strategy: blue-green\n");
        assert!(ex.edges.is_empty());
    }
}
