use crate::enrich::Questions;
use crate::model::{Graph, NodeKind};
use rust_stemmers::{Algorithm, Stemmer};
use std::collections::HashMap;

const K1: f32 = 1.2;
const B: f32 = 0.75;
/// BM25+ lower bound on a matched term's contribution (Lv & Zhai 2011).
const PLUS_DELTA: f32 = 1.0;
/// BM25L shift of the length-normalised term frequency (Lv & Zhai 2011).
const L_DELTA: f32 = 0.5;
const GRAM: usize = 4;
/// Token class markers: a character n-gram and an adjacent-stem bigram are scored with their
/// own weights, so they carry a prefix a stem or an id can never start with.
const GRAM_MARK: char = '\u{1}';
const BIGRAM_MARK: char = '\u{2}';

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Score { Bm25, Plus, L }

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tok { Stem, Gram, Both }

/// One experiment's constants. `REPOGRAPH_BM25="k1=0.9,b=0.5,label=2,score=plus,tok=both,gw=0.5,dfmax=0.2,bigram=0.3"`
/// overrides any subset; unset, every field is the shipped behaviour.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub k1: f32,
    pub b: f32,
    /// How many times the label is repeated in the indexed text (a repetition BM25F).
    pub label: usize,
    pub score: Score,
    pub tok: Tok,
    /// Weight of a character-gram term's contribution.
    pub gw: f32,
    /// Query terms whose document frequency exceeds this share of the index are dropped.
    pub dfmax: f32,
    /// Weight of an adjacent-stem bigram's contribution; zero emits no bigrams.
    pub bigram: f32,
    /// Character n-gram length for `Tok::Gram` / `Tok::Both`.
    pub gram: usize,
}

impl Default for Params {
    fn default() -> Self {
        Params { k1: K1, b: B, label: 1, score: Score::Bm25, tok: Tok::Stem, gw: 1.0, dfmax: 1.0, bigram: 0.0, gram: GRAM }
    }
}

impl Params {
    fn parse(spec: &str) -> Params {
        let mut p = Params::default();
        for kv in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let (k, v) = kv.split_once('=').unwrap_or_else(|| panic!("REPOGRAPH_BM25: `{kv}` is not key=value"));
            let num = || v.parse::<f32>().unwrap_or_else(|_| panic!("REPOGRAPH_BM25: {k}={v} is not a number"));
            match k {
                "k1" => p.k1 = num(),
                "b" => p.b = num(),
                "label" => p.label = num() as usize,
                "gw" => p.gw = num(),
                "dfmax" => p.dfmax = num(),
                "bigram" => p.bigram = num(),
                "gram" => p.gram = num() as usize,
                "score" => p.score = match v { "bm25" => Score::Bm25, "plus" => Score::Plus, "l" => Score::L, _ => panic!("REPOGRAPH_BM25: score={v}") },
                "tok" => p.tok = match v { "stem" => Tok::Stem, "gram" => Tok::Gram, "both" => Tok::Both, _ => panic!("REPOGRAPH_BM25: tok={v}") },
                _ => panic!("REPOGRAPH_BM25: unknown key {k}"),
            }
        }
        p
    }
}

pub fn params() -> &'static Params {
    static P: std::sync::OnceLock<Params> = std::sync::OnceLock::new();
    P.get_or_init(|| std::env::var("REPOGRAPH_BM25").map(|s| Params::parse(&s)).unwrap_or_default())
}

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

/// Lower-cased words of `text`: hyphens stay inside a word so `fr-pay-22` is one term, every
/// other non-alphanumeric byte splits, and a word shorter than two characters is dropped.
pub fn words(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    lower.split(|c: char| !(c.is_alphanumeric() || c == '-'))
        .map(|raw| raw.trim_matches('-'))
        .filter(|t| t.chars().count() >= 2)
        .map(str::to_string)
        .collect()
}

fn is_name(word: &str) -> bool {
    word.contains('-') || word.chars().any(|c| c.is_ascii_digit())
}

fn stem(word: &str) -> String {
    let (ru, en) = stemmers();
    if word.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)) { ru.stem(word).into_owned() } else { en.stem(word).into_owned() }
}

/// Character `n`-grams of a word, the word itself when it is shorter.
fn grams(word: &str, n: usize) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() <= n {
        return vec![format!("{GRAM_MARK}{word}")];
    }
    chars.windows(n).map(|w| { let mut s = String::from(GRAM_MARK); s.extend(w); s }).collect()
}

pub fn tokenize_with(text: &str, p: &Params) -> Vec<String> {
    let mut out = Vec::new();
    let mut prev: Option<String> = None;
    for w in words(text) {
        let primary = if is_name(&w) { Some(w.clone()) } else if p.tok == Tok::Gram { None } else { Some(stem(&w)) };
        if !is_name(&w) && p.tok != Tok::Stem {
            out.extend(grams(&w, p.gram));
        }
        if let Some(t) = primary {
            if p.bigram > 0.0 {
                if let Some(a) = &prev { out.push(format!("{BIGRAM_MARK}{a} {t}")); }
            }
            prev = Some(t.clone());
            out.push(t);
        }
    }
    out
}

pub fn tokenize(text: &str) -> Vec<String> {
    tokenize_with(text, params())
}

/// The shipped tokenizer regardless of `REPOGRAPH_BM25`: Snowball stems, names whole.
pub fn stems(text: &str) -> Vec<String> {
    tokenize_with(text, &Params::default())
}

fn term_weight(term: &str, p: &Params) -> f32 {
    match term.chars().next() {
        Some(GRAM_MARK) => p.gw,
        Some(BIGRAM_MARK) => p.bigram,
        _ => 1.0,
    }
}

impl LexicalIndex {
    pub fn build(graph: &Graph) -> LexicalIndex {
        let p = params();
        Self::build_with(graph, |n| {
            let mut text = n.id.clone();
            for _ in 0..p.label { text.push(' '); text.push_str(&n.label); }
            text.push(' ');
            text.push_str(&n.body);
            text
        })
    }

    /// The generated questions alone: mixed into the passage text they cost a keyword hit.
    pub fn build_questions(graph: &Graph, questions: &Questions) -> LexicalIndex {
        Self::build_with(graph, |n| format!("{} {}", n.id, questions.get(&n.id).join(" ")))
    }

    fn build_with(graph: &Graph, text: impl Fn(&crate::model::Node) -> String + Sync) -> LexicalIndex {
        use rayon::prelude::*;
        let nodes: Vec<_> = graph.nodes.values().filter(|n| n.kind != NodeKind::File).collect();
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

    pub fn search(&self, query: &str, k: usize) -> Vec<(String, f32)> {
        let p = params();
        let n = self.ids.len() as f32;
        let mut scores: HashMap<usize, f32> = HashMap::new();
        for term in tokenize(query) {
            let Some(list) = self.postings.get(&term) else { continue };
            if list.len() as f32 > p.dfmax * n { continue; }
            let idf = ((n - list.len() as f32 + 0.5) / (list.len() as f32 + 0.5) + 1.0).ln();
            let weight = term_weight(&term, p) * idf;
            for (doc, tf) in list {
                let tf = *tf as f32;
                let len_ratio = 1.0 - p.b + p.b * self.lengths[*doc] / self.avg_len;
                let contribution = match p.score {
                    Score::Bm25 => (tf * (p.k1 + 1.0)) / (tf + p.k1 * len_ratio),
                    Score::Plus => (tf * (p.k1 + 1.0)) / (tf + p.k1 * len_ratio) + PLUS_DELTA,
                    Score::L => { let c = tf / len_ratio; (p.k1 + 1.0) * (c + L_DELTA) / (p.k1 + c + L_DELTA) }
                };
                *scores.entry(*doc).or_default() += weight * contribution;
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
    fn default_params_are_the_shipped_constants() {
        let p = Params::default();
        assert_eq!((p.k1, p.b, p.label, p.gw, p.dfmax, p.bigram, p.gram), (1.2, 0.75, 1, 1.0, 1.0, 0.0, 4));
        assert_eq!((p.score, p.tok), (Score::Bm25, Tok::Stem));
        assert_eq!(tokenize_with("политика отмены FR-PAY-22", &p), stems("политика отмены FR-PAY-22"));
    }

    #[test]
    fn spec_parses_every_key_and_rejects_the_rest() {
        let p = Params::parse("k1=0.9, b=0.5,label=3,score=l,tok=both,gw=0.5,dfmax=0.2,bigram=0.3,gram=3");
        assert_eq!((p.k1, p.b, p.label, p.gw, p.dfmax, p.bigram, p.gram), (0.9, 0.5, 3, 0.5, 0.2, 0.3, 3));
        assert_eq!((p.score, p.tok), (Score::L, Tok::Both));
        assert!(std::panic::catch_unwind(|| Params::parse("kk=1")).is_err());
    }

    #[test]
    fn grams_and_bigrams_carry_their_markers_and_names_stay_whole() {
        let p = Params { tok: Tok::Gram, ..Params::default() };
        let toks = tokenize_with("штрафы FR-PAY-22", &p);
        assert_eq!(toks, vec![format!("{GRAM_MARK}штра"), format!("{GRAM_MARK}траф"), format!("{GRAM_MARK}рафы"), "fr-pay-22".to_string()]);
        let p = Params { tok: Tok::Both, bigram: 0.5, ..Params::default() };
        let toks = tokenize_with("политика отмены", &p);
        let (a, b) = (stem("политика"), stem("отмены"));
        assert!(toks.contains(&a) && toks.contains(&b), "{toks:?}");
        assert!(toks.contains(&format!("{BIGRAM_MARK}{a} {b}")), "{toks:?}");
        assert!(toks.iter().filter(|t| t.starts_with(GRAM_MARK)).count() == 5 + 3);
        assert_eq!(term_weight(&format!("{GRAM_MARK}штра"), &p), 1.0);
        assert_eq!(term_weight(&format!("{BIGRAM_MARK}a b"), &p), 0.5);
        assert_eq!(term_weight("штраф", &p), 1.0);
        let p = Params { tok: Tok::Gram, gram: 3, ..Params::default() };
        assert_eq!(tokenize_with("штрафы", &p).len(), 4);
    }
}
