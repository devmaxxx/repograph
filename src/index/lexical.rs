use crate::enrich::Questions;
use crate::model::{Graph, NodeKind};
use rust_stemmers::{Algorithm, Stemmer};
use std::collections::HashMap;

const K1: f32 = 1.2;
const B: f32 = 0.75;

pub struct LexicalIndex {
    ids: Vec<String>,
    lengths: Vec<f32>,
    avg_len: f32,
    /// term → (doc index, term frequency)
    postings: HashMap<String, Vec<(usize, u32)>>,
}

fn stemmers() -> &'static (Stemmer, Stemmer) {
    static S: std::sync::OnceLock<(Stemmer, Stemmer)> = std::sync::OnceLock::new();
    S.get_or_init(|| (Stemmer::create(Algorithm::Russian), Stemmer::create(Algorithm::English)))
}

pub fn tokenize(text: &str) -> Vec<String> {
    let (ru, en) = stemmers();
    let lower = text.to_lowercase();
    let mut out = Vec::new();
    // Hyphens stay inside a token so `fr-pay-22` is one term; every other
    // non-alphanumeric byte splits.
    for raw in lower.split(|c: char| !(c.is_alphanumeric() || c == '-')) {
        let t = raw.trim_matches('-');
        if t.chars().count() < 2 { continue; }
        if t.contains('-') || t.chars().any(|c| c.is_ascii_digit()) {
            out.push(t.to_string());
        } else if t.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)) {
            out.push(ru.stem(t).into_owned());
        } else {
            out.push(en.stem(t).into_owned());
        }
    }
    out
}

impl LexicalIndex {
    pub fn build(graph: &Graph) -> LexicalIndex {
        Self::build_with(graph, |n| format!("{} {} {}", n.id, n.label, n.body))
    }

    /// The generated questions alone: mixed into the passage text they cost a keyword hit.
    pub fn build_questions(graph: &Graph, questions: &Questions) -> LexicalIndex {
        Self::build_with(graph, |n| format!("{} {}", n.id, questions.get(&n.id).join(" ")))
    }

    fn build_with(graph: &Graph, text: impl Fn(&crate::model::Node) -> String) -> LexicalIndex {
        let mut ids = Vec::new();
        let mut lengths = Vec::new();
        let mut postings: HashMap<String, Vec<(usize, u32)>> = HashMap::new();
        for n in graph.nodes.values().filter(|n| n.kind != NodeKind::File) {
            let doc = ids.len();
            ids.push(n.id.clone());
            let toks = tokenize(&text(n));
            lengths.push(toks.len() as f32);
            let mut tf: HashMap<String, u32> = HashMap::new();
            for t in toks { *tf.entry(t).or_default() += 1; }
            for (t, c) in tf { postings.entry(t).or_default().push((doc, c)); }
        }
        let avg_len = if lengths.is_empty() { 1.0 } else { lengths.iter().sum::<f32>() / lengths.len() as f32 };
        LexicalIndex { ids, lengths, avg_len, postings }
    }

    pub fn search(&self, query: &str, k: usize) -> Vec<(String, f32)> {
        let n = self.ids.len() as f32;
        let mut scores: HashMap<usize, f32> = HashMap::new();
        for term in tokenize(query) {
            let Some(list) = self.postings.get(&term) else { continue };
            let idf = ((n - list.len() as f32 + 0.5) / (list.len() as f32 + 0.5) + 1.0).ln();
            for (doc, tf) in list {
                let tf = *tf as f32;
                let norm = K1 * (1.0 - B + B * self.lengths[*doc] / self.avg_len);
                *scores.entry(*doc).or_default() += idf * (tf * (K1 + 1.0)) / (tf + norm);
            }
        }
        let mut ranked: Vec<(String, f32)> = scores.into_iter().map(|(d, s)| (self.ids[d].clone(), s)).collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        ranked.truncate(k);
        ranked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Extraction, NodeKind};

    #[test]
    fn russian_inflections_share_a_stem() {
        assert_eq!(tokenize("штрафа"), tokenize("штрафы"));
        assert_eq!(tokenize("отмены"), tokenize("отмена"));
        assert_eq!(tokenize("cancellations"), tokenize("cancellation"));
    }

    #[test]
    fn ids_survive_as_one_token_and_case_folds() {
        assert_eq!(tokenize("См. FR-PAY-22!"), vec!["см".to_string(), "fr-pay-22".to_string()]);
    }

    #[test]
    fn hyphenated_word_without_digits_stays_one_token() {
        // No digit in this one, unlike an id: pins the hyphen branch on its own
        // instead of riding along on the digit check.
        assert_eq!(tokenize("long-standing"), vec!["long-standing".to_string()]);
    }

    #[test]
    fn bm25_ranks_the_body_match_first() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "правило отмены", "штраф считается по политике отмены", "a.md", 1);
        e.node(NodeKind::Requirement, "FR-PAY-26", "списание штрафа", "штраф списывается автоматически", "a.md", 9);
        e.node(NodeKind::Requirement, "FR-CAL-40", "коды конфликтов", "словарь кодов", "b.md", 1);
        e.node(NodeKind::File, "file:a.md", "a.md", "", "a.md", 1);
        g.apply(e);
        let idx = LexicalIndex::build(&g);
        let hits = idx.search("политика отмен штрафы", 5);
        assert_eq!(hits[0].0, "FR-PAY-22");
        assert_eq!(hits[1].0, "FR-PAY-26");
        assert_eq!(hits.len(), 2);
        assert!(idx.search("file", 5).is_empty());
    }

    #[test]
    fn a_generated_question_reaches_its_node_through_the_questions_index_only() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "правило отмены", "штраф считается по политике отмены", "a.md", 1);
        e.node(NodeKind::Requirement, "FR-PAY-26", "списание штрафа", "штраф списывается автоматически", "a.md", 9);
        g.apply(e);
        let mut q = Questions::default();
        q.entries.insert("FR-PAY-26".into(), crate::enrich::Entry { hash: String::new(), questions: vec!["когда деньги уходят сами".into()] });
        let hits = LexicalIndex::build_questions(&g, &q).search("деньги уходят", 5);
        assert_eq!(hits.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), vec!["FR-PAY-26"]);
        assert!(LexicalIndex::build(&g).search("деньги уходят", 5).is_empty());
    }

    #[test]
    fn bm25_score_is_pinned_to_the_stated_constants() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "правило отмены", "штраф считается по политике отмены", "a.md", 1);
        e.node(NodeKind::Requirement, "FR-PAY-26", "списание штрафа", "штраф списывается автоматически", "a.md", 9);
        e.node(NodeKind::Requirement, "FR-CAL-40", "коды конфликтов", "словарь кодов", "b.md", 1);
        e.node(NodeKind::File, "file:a.md", "a.md", "", "a.md", 1);
        g.apply(e);
        let idx = LexicalIndex::build(&g);
        let hits = idx.search("политика отмен штрафы", 5);
        // Measured with K1 = 1.2, B = 0.75; a tolerance this tight catches
        // either constant drifting to a materially different value.
        assert!((hits[0].1 - 2.565_525).abs() < 0.001, "top score {} moved off the K1/B baseline", hits[0].1);
    }
}
