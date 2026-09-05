//! `repograph dump`: every retriever's ranked list for a set of questions, written once so the
//! retrieval mathematics can be done offline, without the model and without this code.

use crate::config::Config;
use crate::enrich::Questions;
use crate::ids::IdMatcher;
use crate::index::{dense::DenseIndex, embed::Embedder, lexical::LexicalIndex};
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
    bm25_code: Vec<(String, f32)>,
    loo_hash: String,
    loo_rows: Vec<usize>,
    ask: Ask,
}

#[derive(Serialize)]
struct Meta { store: String, depth: usize, rows: usize, dim: usize, queries: usize, dense: bool }

#[derive(Serialize)]
struct Dump { meta: Meta, queries: Vec<Record> }

pub fn run(repo: &Path, queries: &Path, out: &Path, depth: usize, no_dense: bool) -> Result<()> {
    let cfg = Config::load(repo)?;
    let store = Store::new(repo);
    let (graph, _) = store.load()?;
    if graph.nodes.is_empty() { anyhow::bail!("graph is empty at {} — run build first", repo.display()); }
    let ids = IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
    let dense_idx = DenseIndex::load(&store)?;
    if !no_dense && dense_idx.ids.is_empty() { anyhow::bail!("dense index is empty at {} — run `repograph update` first", repo.display()); }
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
    let lexical_c = LexicalIndex::build_code_questions(&graph, &loo);
    // `--no-dense` records the lexical-only arm: the dense lists stay empty and `ask` answers
    // without them, exactly as `ask --no-dense` would, so the held-out gate can be read in the
    // arm the floors also grade.
    let mut embedder = if no_dense {
        None
    } else {
        match Embedder::open() {
            Ok(e) => Some(e),
            Err(err) => anyhow::bail!("dense: model unavailable ({err:#})"),
        }
    };
    let opts = Options { seeds: 5, bodies: false, dense: !no_dense, json: false, depth: crate::rerank::DEPTH };
    let mut records = Vec::with_capacity(queries.len());
    for q in &queries {
        let words: Vec<String> = q.q.split_whitespace().map(str::to_string).collect();
        let (exact_ids, whole_question) = query::exact_seeds(&graph, &ids, &words);
        let qvec = match embedder.as_mut() { Some(e) => e.query(&q.q)?, None => Vec::new() };
        let loo_hash = blake3::hash(format!("query: {}", q.q).as_bytes()).to_hex().to_string();
        let loo_rows: Vec<usize> = dense_idx.hashes.iter().enumerate().filter(|(_, h)| **h == loo_hash).map(|(i, _)| i).collect();
        let (dense_passages, dense_questions) = if no_dense { (Vec::new(), Vec::new()) } else { dense_idx.search_scored(&qvec, depth, Some(&loo_hash)) };
        // The same vector `ask` would embed for this question, so the recorded answer is the
        // binary's own and the offline replication can be checked against it query by query.
        // It answers from the same held-out indices the four lists come from: a synthetic
        // query answered over the full store finds its own row, and `heldout.py compare`
        // would then be scoring the leak rather than the binary.
        let dense_fn = |_: &str, k: usize| {
            let (p, g) = dense_idx.search_scored(&qvec, k, Some(&loo_hash));
            (p.into_iter().map(|(id, _)| id).collect(), g.into_iter().map(|(id, _)| id).collect())
        };
        let dense_arm: Option<query::Dense> = if no_dense { None } else { Some(&dense_fn) };
        let answer = query::ask(&graph, &ids, &loo, dense_arm, None, &words, &opts);
        records.push(Record {
            q: q.q.clone(),
            expect: q.expect.clone(),
            kind: q.kind.clone(),
            exact: Exact { ids: exact_ids, whole_question },
            bm25_passages: lexical.search(&q.q, depth),
            bm25_questions: lexical_q.search(&q.q, depth),
            bm25_code: lexical_c.search(&q.q, depth),
            dense_passages,
            dense_questions,
            loo_hash,
            loo_rows,
            qvec,
            ask: Ask {
                seeds: answer.seeds.iter().map(|h| (h.id.clone(), h.score)).collect(),
                expanded: answer.expanded.iter().map(|h| (h.id.clone(), h.score, h.via.clone().unwrap_or_default())).collect(),
            },
        });
        eprintln!("dump: {:<10} {:<14} {}", q.kind, q.expect, q.q);
    }
    let dump = Dump {
        meta: Meta { store: repo.join(".repograph").display().to_string(), depth, rows: dense_idx.ids.len(), dim: dense_idx.dim, queries: records.len(), dense: !no_dense },
        queries: records,
    };
    std::fs::write(out, serde_json::to_vec(&dump)?).with_context(|| out.display().to_string())?;
    println!("dump: {} queries, {} deep, {} rows × {}, dense={} → {}", dump.meta.queries, depth, dump.meta.rows, dump.meta.dim, dump.meta.dense, out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_line_deserializes_its_three_required_fields() {
        let q: Query = serde_json::from_str(r#"{"q": "как отменить?", "expect": "FR-PAY-22", "kind": "keyword"}"#).unwrap();
        assert_eq!((q.q.as_str(), q.expect.as_str(), q.kind.as_str()), ("как отменить?", "FR-PAY-22", "keyword"));
    }

    #[test]
    fn a_query_line_missing_a_field_errors_naming_it() {
        let err = serde_json::from_str::<Query>(r#"{"q": "x", "kind": "keyword"}"#).unwrap_err();
        assert!(err.to_string().contains("expect"), "{err}");
    }

    // The dump is read offline by retrieval-math scripts (see module doc), so its field names
    // are a contract independent of this binary; a silent rename here would break them quietly.
    #[test]
    fn a_record_serializes_with_the_names_the_offline_scripts_read() {
        let record = Record {
            q: "q".into(),
            expect: "FR-PAY-22".into(),
            kind: "keyword".into(),
            exact: Exact { ids: vec!["FR-PAY-22".into()], whole_question: true },
            qvec: vec![0.1, 0.2],
            dense_passages: vec![("FR-PAY-22".into(), 0.9)],
            dense_questions: vec![],
            bm25_passages: vec![],
            bm25_questions: vec![],
            bm25_code: vec![],
            loo_hash: "abc".into(),
            loo_rows: vec![3],
            ask: Ask { seeds: vec![("FR-PAY-22".into(), 1.0)], expanded: vec![("N-151".into(), 0.5, "FR-PAY-22".into())] },
        };
        let v = serde_json::to_value(&record).unwrap();
        for key in ["q", "expect", "kind", "exact", "qvec", "dense_passages", "dense_questions", "bm25_passages", "bm25_questions", "bm25_code", "loo_hash", "loo_rows", "ask"] {
            assert!(v.get(key).is_some(), "missing field {key}");
        }
        assert_eq!(v["exact"]["whole_question"], true);
        assert_eq!(v["ask"]["expanded"][0][2], "FR-PAY-22");
    }
}
