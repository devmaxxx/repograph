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

impl LexicalIndex {
    pub fn build(graph: &Graph) -> LexicalIndex {
        Self::build_with(graph, |n| n.kind != NodeKind::File, |n| format!("{} {} {}", n.id, n.label, n.indexed_body()))
    }

    /// The documents' questions, and them alone: mixed into the passage text they cost a keyword
    /// hit. A symbol is an id-only document here and a file is absent from the index altogether,
    /// both as they were before `enrich --code` existed. The questions about code are an index of
    /// their own because putting them here moved this one's BM25 statistics: 3,463 one-token
    /// symbol documents became sixty-token ones, the average length rose, length normalisation
    /// lifted every document's score by a quarter while the passage scores the gate compares
    /// against stayed put, and a keyword case that had kept the questions list out at 0.80
    /// admitted it at 1.03.
    pub fn build_questions(graph: &Graph, questions: &Questions) -> LexicalIndex {
        Self::build_with(graph, |n| n.kind != NodeKind::File,
                         |n| if n.is_code() { n.id.clone() } else { format!("{} {}", n.id, questions.get(&n.id).join(" ")) })
    }

    /// The code nodes' questions — symbols and files `enrich --code` has asked about. A file is
    /// a passage nowhere but is present here: the developer's "which file" question wants the
    /// file itself.
    pub fn build_code_questions(graph: &Graph, questions: &Questions) -> LexicalIndex {
        Self::build_with(graph, |n| n.is_code() && !questions.get(&n.id).is_empty(),
                         |n| format!("{} {}", n.id, questions.get(&n.id).join(" ")))
    }

    fn build_with(graph: &Graph, keep: impl Fn(&crate::model::Node) -> bool, text: impl Fn(&crate::model::Node) -> String + Sync) -> LexicalIndex {
        use rayon::prelude::*;
        let nodes: Vec<_> = graph.nodes.values().filter(|n| keep(n)).collect();
        // Stemming is the cost — three quarters of a no-dense answer on a 7,500-node graph
        // when done one document at a time — and every document stems independently.
        let docs: Vec<(String, HashMap<String, u32>, f32)> = nodes.par_iter().map(|n| {
            let toks = tokenize(&text(n));
            let len = toks.len() as f32;
            let mut tf: HashMap<String, u32> = HashMap::new();
            for t in toks { *tf.entry(t).or_default() += 1; }
            (n.id.clone(), tf, len)
        }).collect();
        let mut ids = Vec::with_capacity(docs.len());
        let mut lengths = Vec::with_capacity(docs.len());
        let mut postings: HashMap<String, Vec<(usize, u32)>> = HashMap::new();
        for (doc, (id, tf, len)) in docs.into_iter().enumerate() {
            ids.push(id);
            lengths.push(len);
            for (t, c) in tf { postings.entry(t).or_default().push((doc, c)); }
        }
        let avg_len = if lengths.is_empty() { 1.0 } else { lengths.iter().sum::<f32>() / lengths.len() as f32 };
        LexicalIndex { ids, lengths, avg_len, postings }
    }

    /// BM25's idf for a term seen in `df` of this index's `n` documents.
    fn idf(n: f32, df: usize) -> f32 { ((n - df as f32 + 0.5) / (df as f32 + 0.5) + 1.0).ln() }

    /// An index built over a store that carries no code questions is an index over nothing;
    /// `Lexical::build` checks this before treating its empty list as a list that lost, rather
    /// than a list that was never in contention.
    pub fn is_empty(&self) -> bool { self.ids.is_empty() }

    /// What the query could reach in this index: the score of a document of average length that
    /// holds each of the query's terms exactly once, which BM25 makes the plain sum of their idf
    /// — at tf = 1 and the mean length the term weight `(K1 + 1) / (1 + K1)` is one. A term this
    /// index never saw adds nothing here, as it adds nothing to any document's score. A list's
    /// best over this figure says how much of the query the best document answered, and unlike
    /// the best score itself it compares across indices: each index normalises length against
    /// its own mean and weights a term by its own vocabulary, so two indices' raw scores are in
    /// two units and their ratio moves when either population does (gaps G8 and G12).
    pub fn attainable(&self, query: &str) -> f32 {
        let n = self.ids.len() as f32;
        let mut seen = std::collections::HashSet::new();
        tokenize(query).into_iter().filter(|t| seen.insert(t.clone()))
            .filter_map(|t| self.postings.get(&t).map(|list| Self::idf(n, list.len())))
            .sum()
    }

    pub fn search(&self, query: &str, k: usize) -> Vec<(String, f32)> {
        let n = self.ids.len() as f32;
        let mut scores: HashMap<usize, f32> = HashMap::new();
        for term in tokenize(query) {
            let Some(list) = self.postings.get(&term) else { continue };
            let idf = Self::idf(n, list.len());
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

/// The BM25 indexes an answer fuses, built once from a graph and its questions and kept by
/// whoever answers more than one question over them: a resident `serve`, `bench` over its
/// cases, `dump` over a suite. Nothing here is written to disk — the indexes are term statistics
/// over every document and a rebuild is the cheapest correct update — so only that on-disk half
/// is stale-free; the copy a `Context` keeps in memory is exactly the state that can drift from
/// the graph or the questions, which is why it gets dropped whenever `Context::adopt` takes up a
/// moved store — not on a `questions.json` rewrite alone, which reaches `adopt` only if a
/// document changed alongside it. What changed is who pays for the build: the lexical arm
/// through the socket spent almost all of 49 of its 54 ms rebuilding these per question
/// (`docs/bench/2026-09-06-perf-results.md`).
pub struct Lexical {
    pub passages: LexicalIndex,
    /// Absent on a store `enrich` never touched — see `build_questions`.
    pub questions: Option<LexicalIndex>,
    /// Absent when no code node carries a question, or when nobody asked `build` for one yet.
    pub code: Option<LexicalIndex>,
    /// Whether `code` was ever asked for. A plain answer's first build passes `false`: on a
    /// store `enrich --code` touched, building it unasked cost 18.5 ms of a one-shot lexical
    /// `ask`'s 128.3 ms median, for a list the plain fusion never seats (`lexical_lists`) —
    /// pre-change vs head, medians of 33 (`docs/bench/2026-09-05-0.5.0-gaps-results.md`).
    /// `ensure_code` flips this once a `--rerank` on the same context needs the list after all.
    code_seat: bool,
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
        Lexical { passages: LexicalIndex::build(&Graph::default()), questions: None, code: None, code_seat: false }
    }

    /// `code_seat` is the caller's promise that it can use a code list at all: `dump` and `bench`
    /// always can (one build serves a whole run, so the cost above is paid once regardless), and
    /// a `Context` can only once a request is reranked — `lexical_lists` never reads `code` on
    /// the plain path.
    pub fn build(graph: &Graph, questions: &Questions, code_seat: bool) -> Lexical {
        let passages = LexicalIndex::build(graph);
        if questions.entries.is_empty() { return Lexical { passages, questions: None, code: None, code_seat }; }
        let code = if code_seat { Self::code_index(graph, questions) } else { None };
        Lexical { passages, questions: Some(LexicalIndex::build_questions(graph, questions)), code, code_seat }
    }

    fn code_index(graph: &Graph, questions: &Questions) -> Option<LexicalIndex> {
        let code = LexicalIndex::build_code_questions(graph, questions);
        if code.is_empty() { None } else { Some(code) }
    }

    /// Builds the code list a plain-first `build` skipped, for a context whose next request
    /// turns out to be reranked — without discarding `passages`/`questions`, which already
    /// answered the plain ones fine. A no-op once `code_seat` is already true.
    pub fn ensure_code(&mut self, graph: &Graph, questions: &Questions) {
        if self.code_seat { return; }
        self.code = if self.questions.is_none() { None } else { Self::code_index(graph, questions) };
        self.code_seat = true;
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
    fn code_questions_are_an_index_of_their_own_and_the_documents_index_keeps_a_symbol_as_its_id() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "правило отмены", "штраф считается по политике отмены", "a.md", 1);
        e.node(NodeKind::File, "file:apps/a.ts", "a.ts", "Sessions and their revocation.", "apps/a.ts", 1);
        e.node(NodeKind::Symbol, "sym:apps/a.ts::revoke", "revoke", "Ends every session.\nrevoke() {}", "apps/a.ts", 3);
        g.apply(e);
        let mut q = Questions::default();
        let entry = |t: &str| crate::enrich::Entry { hash: String::new(), questions: vec![t.into()] };
        q.entries.insert("file:apps/a.ts".into(), entry("где выйти со всех устройств"));
        q.entries.insert("sym:apps/a.ts::revoke".into(), entry("как завершить чужую сессию"));
        let code = LexicalIndex::build_code_questions(&g, &q);
        assert_eq!(code.search("выйти со всех устройств", 5)[0].0, "file:apps/a.ts");
        assert_eq!(code.search("завершить сессию", 5)[0].0, "sym:apps/a.ts::revoke");
        assert!(code.search("штраф", 5).is_empty());
        let docs = LexicalIndex::build_questions(&g, &q);
        assert!(docs.search("выйти устройств завершить сессию", 5).is_empty(), "code questions never enter the documents' index");
        assert_eq!(docs.search("sym:apps/a.ts::revoke", 5)[0].0, "sym:apps/a.ts::revoke", "a symbol stays an id-only document there");
        // The file's own id retrieves the symbol that shares its path tokens and never the file.
        assert!(docs.search("file:apps/a.ts", 5).iter().all(|(id, _)| id != "file:apps/a.ts"), "a file is absent from the documents' index");
        assert!(LexicalIndex::build(&g).search("revocation", 5).is_empty());
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
        // Measured with K1 = 1.2, B = 0.75; a tolerance this tight catches
        // either constant drifting to a materially different value.
        assert!((hits[0].1 - 2.565_525).abs() < 0.001, "top score {} moved off the K1/B baseline", hits[0].1);
    }

    #[test]
    fn attainable_is_the_sum_of_idf_over_the_query_terms_the_index_holds_each_counted_once() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "правило отмены", "штраф считается по политике отмены", "a.md", 1);
        e.node(NodeKind::Requirement, "FR-PAY-26", "списание штрафа", "штраф списывается автоматически", "a.md", 9);
        e.node(NodeKind::Requirement, "FR-CAL-40", "коды конфликтов", "словарь кодов", "b.md", 1);
        g.apply(e);
        let idx = LexicalIndex::build(&g);
        let n = 3.0_f32;
        let idf = |df: f32| ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
        // «штраф» sits in two documents, «политике» in one, «ъъъ» in none; a term the query
        // repeats is one term, as a document of average length holds it once.
        assert!((idx.attainable("штраф политике ъъъ") - (idf(2.0) + idf(1.0))).abs() < 1e-6);
        assert!((idx.attainable("штраф штраф") - idf(2.0)).abs() < 1e-6);
        assert_eq!(idx.attainable("ъъъ"), 0.0);
        assert_eq!(LexicalIndex::build(&Graph::default()).attainable("штраф"), 0.0);
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
        // No code node carries a question, so the code index is an index over nothing.
        assert!(LexicalIndex::build_code_questions(&g, &Questions::default()).is_empty());
    }

    #[test]
    fn a_store_without_questions_builds_the_passage_index_alone() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "штраф", "штраф", "a.md", 1);
        g.apply(e);
        let lex = Lexical::build(&g, &Questions::default(), true);
        assert!(!lex.passages.is_empty());
        assert!(lex.questions.is_none() && lex.code.is_none());
    }

    #[test]
    fn a_store_with_document_questions_and_no_code_questions_builds_two_indexes() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-26", "списание штрафа", "штраф списывается", "a.md", 9);
        e.node_span(NodeKind::Symbol, "sym:apps/a.ts::revoke", "revoke", "Ends every session.\nrevoke() {}", "apps/a.ts", (3, 3));
        g.apply(e);
        let mut q = Questions::default();
        q.entries.insert("FR-PAY-26".into(), crate::enrich::Entry { hash: String::new(), questions: vec!["когда деньги уходят сами".into()] });
        let lex = Lexical::build(&g, &q, true);
        assert_eq!(lex.questions.as_ref().map(|i| i.search("деньги уходят", 5)[0].0.clone()), Some("FR-PAY-26".to_string()));
        assert!(lex.code.is_none(), "no code node carries a question, so there is no code index to search");
        q.entries.insert("sym:apps/a.ts::revoke".into(), crate::enrich::Entry { hash: String::new(), questions: vec!["как выйти со всех устройств".into()] });
        let lex = Lexical::build(&g, &q, true);
        assert_eq!(lex.code.as_ref().map(|i| i.search("выйти устройств", 5)[0].0.clone()), Some("sym:apps/a.ts::revoke".to_string()));
    }

    #[test]
    fn a_plain_build_skips_the_code_list_and_ensure_code_adds_it_without_changing_what_already_answered() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-26", "списание штрафа", "штраф списывается", "a.md", 9);
        e.node_span(NodeKind::Symbol, "sym:apps/a.ts::revoke", "revoke", "Ends every session.\nrevoke() {}", "apps/a.ts", (3, 3));
        g.apply(e);
        let mut q = Questions::default();
        q.entries.insert("FR-PAY-26".into(), crate::enrich::Entry { hash: String::new(), questions: vec!["когда деньги уходят сами".into()] });
        q.entries.insert("sym:apps/a.ts::revoke".into(), crate::enrich::Entry { hash: String::new(), questions: vec!["как выйти со всех устройств".into()] });
        let mut lex = Lexical::build(&g, &q, false);
        assert!(lex.code.is_none(), "a plain build never asks for a list `lexical_lists` would not seat anyway");
        let passages_before = lex.passages.search("штраф", 5);
        let questions_before = lex.questions.as_ref().map(|i| i.search("деньги уходят", 5));
        lex.ensure_code(&g, &q);
        assert_eq!(lex.code.as_ref().map(|i| i.search("выйти устройств", 5)[0].0.clone()), Some("sym:apps/a.ts::revoke".to_string()));
        assert_eq!(lex.passages.search("штраф", 5), passages_before, "a later --rerank gets the code list without losing what already answered fine");
        assert_eq!(lex.questions.as_ref().map(|i| i.search("деньги уходят", 5)), questions_before);
    }
}
