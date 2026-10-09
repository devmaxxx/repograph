use crate::model::{Graph, NodeKind};
use rust_stemmers::{Algorithm, Stemmer};
use std::collections::HashMap;

const K1: f32 = 1.2;
const B: f32 = 0.75;
/// A pair's share of the BM25 weight it would carry as a term. Below one, so standing together
/// reorders rows that already hold both words and never outweighs a word a row lacks.
const PAIR: f32 = 0.25;

pub struct LexicalIndex {
    ids: Vec<String>,
    lengths: Vec<f32>,
    avg_len: f32,
    /// term → (doc index, term frequency)
    postings: HashMap<String, Vec<(usize, u32)>>,
    /// Adjacent term pairs, keyed `"a b"`: BM25 scores words one at a time, so a row that holds
    /// «отчёты склада» as written ranked under rows holding both words apart.
    pairs: HashMap<String, Vec<(usize, u32)>>,
}

fn pairs_of(toks: &[String]) -> impl Iterator<Item = String> + '_ {
    toks.windows(2).map(|w| format!("{} {}", w[0], w[1]))
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
    // non-alphanumeric byte splits. An identifier stays one term: splitting `revokeAllSessions`
    // into its words was measured and seated no file while costing a paraphrase, because the
    // identifiers quoted in every requirement's text lengthened those documents too.
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

fn passage_text(n: &crate::model::Node) -> String { format!("{} {} {}", n.id, n.label, n.indexed_body()) }

impl LexicalIndex {
    /// The passages: every document-side node but a file's own, and not the text files, which are
    /// an index of their own (`build_text`) so that a configuration file is never a rival for the
    /// passages' BM25 statistics or their seats.
    pub fn build(graph: &Graph) -> LexicalIndex {
        Self::build_with(graph, |n| !matches!(n.kind, NodeKind::File | NodeKind::Text), passage_text)
    }

    /// The text nodes alone, text as the passages carry it. Its coverage is read against the
    /// passages' by the admission in `query`, so it is scored in its own index and not pooled.
    pub fn build_text(graph: &Graph) -> LexicalIndex {
        Self::build_with(graph, |n| n.kind == NodeKind::Text, passage_text)
    }

    fn build_with(graph: &Graph, keep: impl Fn(&crate::model::Node) -> bool, text: impl Fn(&crate::model::Node) -> String + Sync) -> LexicalIndex {
        use rayon::prelude::*;
        let nodes: Vec<_> = graph.nodes.values().filter(|n| keep(n)).collect();
        // Stemming is the cost — three quarters of a no-dense answer on a 7,500-node graph
        // when done one document at a time — and every document stems independently.
        type Doc = (String, HashMap<String, u32>, HashMap<String, u32>, f32);
        let docs: Vec<Doc> = nodes.par_iter().map(|n| {
            let toks = tokenize(&text(n));
            let len = toks.len() as f32;
            let mut pf: HashMap<String, u32> = HashMap::new();
            for p in pairs_of(&toks) { *pf.entry(p).or_default() += 1; }
            let mut tf: HashMap<String, u32> = HashMap::new();
            for t in toks { *tf.entry(t).or_default() += 1; }
            (n.id.clone(), tf, pf, len)
        }).collect();
        let mut ids = Vec::with_capacity(docs.len());
        let mut lengths = Vec::with_capacity(docs.len());
        let mut postings: HashMap<String, Vec<(usize, u32)>> = HashMap::new();
        let mut pairs: HashMap<String, Vec<(usize, u32)>> = HashMap::new();
        for (doc, (id, tf, pf, len)) in docs.into_iter().enumerate() {
            ids.push(id);
            lengths.push(len);
            for (t, c) in tf { postings.entry(t).or_default().push((doc, c)); }
            for (p, c) in pf { pairs.entry(p).or_default().push((doc, c)); }
        }
        let avg_len = if lengths.is_empty() { 1.0 } else { lengths.iter().sum::<f32>() / lengths.len() as f32 };
        LexicalIndex { ids, lengths, avg_len, postings, pairs }
    }

    /// BM25's idf for a term seen in `df` of this index's `n` documents.
    fn idf(n: f32, df: usize) -> f32 { ((n - df as f32 + 0.5) / (df as f32 + 0.5) + 1.0).ln() }

    /// A store with no text file builds its text index over nothing; `Lexical::build` checks this
    /// before keeping an empty list as one that lost, rather than one never in contention.
    pub fn is_empty(&self) -> bool { self.ids.is_empty() }

    /// What the query asked for, priced in this index: the score a document of average length
    /// would get for holding each of the query's terms exactly once — BM25 at tf = 1 and the
    /// mean length, where the term weight `(K1 + 1) / (1 + K1)` is one, so the plain sum of
    /// their idf. Every term of the query is in the sum, a term this index never saw at the idf
    /// BM25 gives df = 0, `ln(2n + 2)`. Until 2026-09-06 an absent term left the sum instead,
    /// and a list's coverage — its best over this figure — then rose with every query term its
    /// index lacked: an index was rewarded for a narrow vocabulary as much as for a good match,
    /// the residue of gap G8 that the coverage admission shipped with. Charging the term keeps
    /// the statistic a property of the query and of one index alone, which is what lets two
    /// lists' coverages compare in one unit (G12); `search` is untouched, so no ranking moves.
    /// The query's adjacent pairs are priced the same way at `PAIR`, since `search` pays them: a
    /// list whose best row held two words together would otherwise cover more than it was asked.
    /// An index over no documents attains nothing.
    pub fn attainable(&self, query: &str) -> f32 {
        if self.ids.is_empty() { return 0.0; }
        let n = self.ids.len() as f32;
        let terms = tokenize(query);
        let mut seen = std::collections::HashSet::new();
        let words: f32 = terms.iter().filter(|t| seen.insert((*t).clone()))
            .map(|t| Self::idf(n, self.postings.get(t).map_or(0, Vec::len)))
            .sum();
        let pairs: f32 = pairs_of(&terms).filter(|p| seen.insert(p.clone()))
            .map(|p| PAIR * Self::idf(n, self.pairs.get(&p).map_or(0, Vec::len)))
            .sum();
        words + pairs
    }

    pub fn search(&self, query: &str, k: usize) -> Vec<(String, f32)> {
        let n = self.ids.len() as f32;
        let mut scores: HashMap<usize, f32> = HashMap::new();
        let terms = tokenize(query);
        let mut add = |list: &[(usize, u32)], weight: f32| {
            let idf = Self::idf(n, list.len());
            for (doc, tf) in list {
                let tf = *tf as f32;
                let norm = K1 * (1.0 - B + B * self.lengths[*doc] / self.avg_len);
                *scores.entry(*doc).or_default() += weight * idf * (tf * (K1 + 1.0)) / (tf + norm);
            }
        };
        for term in &terms {
            if let Some(list) = self.postings.get(term) { add(list, 1.0); }
        }
        for pair in pairs_of(&terms) {
            if let Some(list) = self.pairs.get(&pair) { add(list, PAIR); }
        }
        let mut ranked: Vec<(String, f32)> = scores.into_iter().map(|(d, s)| (self.ids[d].clone(), s)).collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        ranked.truncate(k);
        ranked
    }
}

/// The BM25 indexes an answer fuses, built once from a graph and kept by whoever answers more
/// than one question over them: a resident `serve`, `bench` over its cases, `dump` over a suite.
/// Nothing here is written to disk — the indexes are term statistics over every document and a
/// rebuild is the cheapest correct update — so the copy a `Context` keeps in memory is exactly
/// the state that can drift from the graph, which is why it gets dropped whenever
/// `Context::adopt` takes up a moved store. What changed is who pays for the build: the lexical
/// arm through the socket spent almost all of 49 of its 54 ms rebuilding these per question
/// (`docs/bench/2026-09-06-perf-results.md`).
pub struct Lexical {
    pub passages: LexicalIndex,
    /// Absent when the graph holds no text node, which is every store with `text_globs` empty.
    pub text: Option<LexicalIndex>,
}

impl Lexical {
    /// What `Context::answer` hands `query::ask` for a question the exact ids or symbols answer
    /// whole: `lexical_lists` is never reached on that path (`!whole_question`), so this is
    /// never read — built fresh and cheap rather than kept, since caching it would starve the
    /// next question that does fuse.
    pub fn empty() -> Lexical {
        // Built through the same constructor as every other index, not hand-rolled: `build`'s
        // empty-corpus case already carries the `avg_len: 1.0` divide-by-zero guard, pinned by
        // `empty_index_unknown_terms_and_empty_query_all_answer_empty` below.
        Lexical { passages: LexicalIndex::build(&Graph::default()), text: None }
    }

    pub fn build(graph: &Graph) -> Lexical {
        let passages = LexicalIndex::build(graph);
        let text = Some(LexicalIndex::build_text(graph)).filter(|t| !t.is_empty());
        Lexical { passages, text }
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
    fn two_query_words_standing_together_outrank_the_same_words_apart() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-WH-11", "склад", "отчёты приходят письмом, остатки склада на экране", "a.md", 1);
        e.node(NodeKind::Requirement, "FR-WH-53", "отчёты", "отчёты склада и остатки на экране письмом", "a.md", 9);
        g.apply(e);
        let ranked = LexicalIndex::build(&g).search("отчёты склада", 2);
        assert_eq!(ranked[0].0, "FR-WH-53", "{ranked:?}");
        assert!(ranked[0].1 > ranked[1].1, "{ranked:?}");
    }

    #[test]
    fn empty_index_unknown_terms_and_empty_query_all_answer_empty() {
        let empty = LexicalIndex::build(&Graph::default());
        assert!(empty.search("отмена", 5).is_empty());
        assert_eq!(empty.avg_len, 1.0, "no documents must not divide by zero");
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "правило отмены", "штраф", "a.md", 1);
        g.apply(e);
        let idx = LexicalIndex::build(&g);
        assert!(idx.search("", 5).is_empty());
        assert!(idx.search("ъъъ !!!", 5).is_empty());
        assert!(idx.search("отмена", 0).is_empty());
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
    fn a_leading_bom_does_not_become_a_spurious_token() {
        assert_eq!(tokenize("\u{FEFF}штраф"), vec!["штраф".to_string()]);
    }

    #[test]
    fn purely_numeric_tokens_bypass_stemming() {
        // Digits route through the id branch, not the stemmers, so "42" and "421" stay distinct.
        assert_eq!(tokenize("42 421"), vec!["42".to_string(), "421".to_string()]);
    }

    #[test]
    fn a_token_mixing_cyrillic_and_latin_letters_uses_the_russian_stemmer() {
        // Any Cyrillic letter routes the whole token to the Russian stemmer, not the English one.
        let (ru, _) = stemmers();
        assert_eq!(tokenize("аbC"), vec![ru.stem("аbc").into_owned()]);
    }

    #[test]
    fn k_larger_than_the_corpus_returns_every_document_once() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "штраф", "штраф", "a.md", 1);
        e.node(NodeKind::Requirement, "FR-PAY-26", "штраф", "штраф", "a.md", 9);
        g.apply(e);
        let hits = LexicalIndex::build(&g).search("штраф", 1000);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn tied_bm25_scores_break_ties_by_id_ascending() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-99", "штраф", "штраф", "a.md", 1);
        e.node(NodeKind::Requirement, "FR-PAY-11", "штраф", "штраф", "a.md", 9);
        g.apply(e);
        let hits = LexicalIndex::build(&g).search("штраф", 5);
        assert_eq!(hits.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), vec!["FR-PAY-11", "FR-PAY-99"]);
        assert_eq!(hits[0].1, hits[1].1);
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
        // Measured with K1 = 1.2, B = 0.75 and PAIR = 0.25 — «политике отмены» stands together in
        // the top row, worth 0.44 of this; a tolerance this tight catches any of the three
        // drifting to a materially different value.
        assert!((hits[0].1 - 3.008_275).abs() < 0.001, "top score {} moved off the K1/B baseline", hits[0].1);
    }

    #[test]
    fn attainable_charges_every_query_term_once_and_an_absent_one_at_the_idf_of_df_zero() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "правило отмены", "штраф считается по политике отмены", "a.md", 1);
        e.node(NodeKind::Requirement, "FR-PAY-26", "списание штрафа", "штраф списывается автоматически", "a.md", 9);
        e.node(NodeKind::Requirement, "FR-CAL-40", "коды конфликтов", "словарь кодов", "b.md", 1);
        g.apply(e);
        let idx = LexicalIndex::build(&g);
        let n = 3.0_f32;
        let idf = |df: f32| ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
        // «штраф» sits in two documents, «политике» in one, «ъъъ» in none and is charged as a
        // term no document holds — it lowers what the best document could cover instead of
        // leaving the sum; a term the query repeats is one term, as a document of average length
        // holds it once. Each adjacent pair is charged at `PAIR` the same way: the first query's
        // pairs stand together nowhere, and «штраф штраф» does in FR-PAY-26, whose label ends
        // where its body begins.
        assert!((idx.attainable("штраф политике ъъъ") - (idf(2.0) + idf(1.0) + idf(0.0) + 2.0 * PAIR * idf(0.0))).abs() < 1e-5);
        assert!((idx.attainable("штраф штраф") - (idf(2.0) + PAIR * idf(1.0))).abs() < 1e-5);
        assert!((idx.attainable("ъъъ") - idf(0.0)).abs() < 1e-6);
        assert_eq!(LexicalIndex::build(&Graph::default()).attainable("штраф"), 0.0, "an index over nothing attains nothing");
    }

    #[test]
    fn a_document_of_average_length_holding_a_term_once_scores_exactly_the_attainable() {
        // Three documents of four tokens each, so every one is of average length; the term sits
        // once in one of them, and BM25 at tf = 1 and the mean length reduces to the idf.
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-A-1", "aa", "штраф bb", "a.md", 1);
        e.node(NodeKind::Requirement, "FR-A-2", "cc", "dd ee", "a.md", 5);
        e.node(NodeKind::Requirement, "FR-A-3", "ff", "gg hh", "a.md", 9);
        g.apply(e);
        let idx = LexicalIndex::build(&g);
        let hits = idx.search("штраф", 5);
        assert_eq!(hits[0].0, "FR-A-1");
        assert!((hits[0].1 - idx.attainable("штраф")).abs() < 1e-6, "{} against {}", hits[0].1, idx.attainable("штраф"));
    }

    #[test]
    fn an_index_over_no_documents_says_so() {
        assert!(LexicalIndex::build(&Graph::default()).is_empty());
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "штраф", "штраф", "a.md", 1);
        g.apply(e);
        assert!(!LexicalIndex::build(&g).is_empty());
    }

    #[test]
    fn a_store_builds_the_passage_index_and_no_text_index_without_text_files() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "штраф", "штраф", "a.md", 1);
        g.apply(e);
        let lex = Lexical::build(&g);
        assert!(!lex.passages.is_empty());
        assert!(lex.text.is_none());
    }

    #[test]
    fn a_text_file_is_an_index_of_its_own() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Text, "file:ops/deploy.yaml", "ops/deploy.yaml", "strategy: blue-green\nwindow: saturday\n", "ops/deploy.yaml", 1);
        e.node(NodeKind::Requirement, "FR-1", "cancel", "a visit is cancelled", "a.md", 1);
        g.apply(e);
        assert!(LexicalIndex::build(&g).search("strategy", 5).is_empty(), "the passages never rank a text file");
        assert_eq!(LexicalIndex::build_text(&g).search("strategy", 5)[0].0, "file:ops/deploy.yaml");
    }
}
