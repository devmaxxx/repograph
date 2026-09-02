//! `repograph dump`: every retriever's ranked list for a set of questions, written once so the
//! retrieval mathematics can be done offline, without the model and without this code.

use crate::config::Config;
use crate::enrich::Questions;
use crate::ids::IdMatcher;
use crate::index::{dense::DenseIndex, embed::Embedder, lexical::{self, LexicalIndex}};
use crate::query::{self, Options};
use crate::store::Store;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Deserialize)]
struct Query { q: String, expect: String, kind: String }

#[derive(Serialize)]
struct Exact { ids: Vec<String>, whole_question: bool }

#[derive(Serialize)]
struct Ask { seeds: Vec<(String, f32)>, expanded: Vec<(String, f32, String)> }

#[derive(Serialize)]
struct Record {
    q: String,
    expect: String,
    kind: String,
    exact: Exact,
    qvec: Vec<f32>,
    dense_passages: Vec<(String, f32)>,
    dense_questions: Vec<(String, f32)>,
    bm25_passages: Vec<(String, f32)>,
    bm25_questions: Vec<(String, f32)>,
    loo_hash: String,
    loo_rows: Vec<usize>,
    ask: Ask,
    /// Shipped Snowball tokens of the question and of the target's indexed text, whatever
    /// `REPOGRAPH_BM25` says: the stem-mismatch measurement needs the baseline stems.
    q_stems: Vec<String>,
    expect_stems: Vec<String>,
    /// The tokens the active variant actually scored the question by.
    q_tokens: Vec<String>,
    bm25_params: String,
}

#[derive(Serialize)]
struct Meta { store: String, depth: usize, rows: usize, dim: usize, queries: usize }

#[derive(Serialize)]
struct Dump { meta: Meta, queries: Vec<Record> }

pub fn run(repo: &Path, queries: &Path, out: &Path, depth: usize) -> Result<()> {
    let cfg = Config::load(repo)?;
    let store = Store::new(repo);
    let (graph, _) = store.load()?;
    if graph.nodes.is_empty() { anyhow::bail!("graph is empty at {} — run build first", repo.display()); }
    let ids = IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
    let dense_idx = DenseIndex::load(&store)?;
    if dense_idx.ids.is_empty() { anyhow::bail!("dense index is empty at {} — run `repograph update` first", repo.display()); }
    let questions = Questions::load(&store)?;
    let text = std::fs::read_to_string(queries).with_context(|| queries.display().to_string())?;
    let queries: Vec<Query> = text.lines().filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Query>(l).map_err(anyhow::Error::from))
        .collect::<Result<_>>()?;
    // A synthetic query is a stored question of its target, so its text leaves the target's
    // document before the lexical questions index is built. One build serves every query
    // because each node is sampled at most once; the dense list drops the row per query instead.
    let mut loo = questions.clone();
    for q in queries.iter().filter(|q| q.kind == "synthetic") {
        if let Some(e) = loo.entries.get_mut(&q.expect) { e.questions.retain(|t| t != &q.q); }
    }
    let lexical = LexicalIndex::build(&graph);
    let lexical_q = LexicalIndex::build_questions(&graph, &loo);
    let mut embedder = match Embedder::open() {
        Ok(e) => e,
        Err(err) => anyhow::bail!("dense: model unavailable ({err:#})"),
    };
    let opts = Options { seeds: 5, bodies: false, dense: true, json: false, depth: crate::rerank::DEPTH };
    let mut records = Vec::with_capacity(queries.len());
    for q in &queries {
        let words: Vec<String> = q.q.split_whitespace().map(str::to_string).collect();
        let (exact_ids, whole_question) = query::exact_seeds(&graph, &ids, &words);
        let qvec = embedder.query(&q.q)?;
        let loo_hash = blake3::hash(format!("query: {}", q.q).as_bytes()).to_hex().to_string();
        let loo_rows: Vec<usize> = dense_idx.hashes.iter().enumerate().filter(|(_, h)| **h == loo_hash).map(|(i, _)| i).collect();
        let (dense_passages, dense_questions) = dense_idx.search_scored(&qvec, depth, Some(&loo_hash));
        // The same vector `ask` would embed for this question, so the recorded answer is the
        // binary's own and the offline replication can be checked against it query by query.
        let dense_fn = |_: &str, k: usize| dense_idx.search(&qvec, k);
        let answer = query::ask(&graph, &ids, &questions, Some(&dense_fn), None, &words, &opts);
        records.push(Record {
            q: q.q.clone(),
            expect: q.expect.clone(),
            kind: q.kind.clone(),
            exact: Exact { ids: exact_ids, whole_question },
            bm25_passages: lexical.search(&q.q, depth),
            bm25_questions: lexical_q.search(&q.q, depth),
            dense_passages,
            dense_questions,
            loo_hash,
            loo_rows,
            qvec,
            ask: Ask {
                seeds: answer.seeds.iter().map(|h| (h.id.clone(), h.score)).collect(),
                expanded: answer.expanded.iter().map(|h| (h.id.clone(), h.score, h.via.clone().unwrap_or_default())).collect(),
            },
            q_stems: lexical::stems(&q.q),
            expect_stems: graph.nodes.get(&q.expect).map(|n| lexical::stems(&format!("{} {} {}", n.id, n.label, n.body))).unwrap_or_default(),
            q_tokens: lexical::tokenize(&q.q),
            bm25_params: format!("{:?}", lexical::params()),
        });
        eprintln!("dump: {:<10} {:<14} {}", q.kind, q.expect, q.q);
    }
    let dump = Dump {
        meta: Meta { store: repo.join(".repograph").display().to_string(), depth, rows: dense_idx.ids.len(), dim: dense_idx.dim, queries: records.len() },
        queries: records,
    };
    std::fs::write(out, serde_json::to_vec(&dump)?).with_context(|| out.display().to_string())?;
    println!("dump: {} queries, {} deep, {} rows × {} → {}", dump.meta.queries, depth, dump.meta.rows, dump.meta.dim, out.display());
    Ok(())
}
