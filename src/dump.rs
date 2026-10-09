//! `repograph dump`: every retriever's ranked list for a set of questions, written once so the
//! retrieval mathematics can be done offline, without the model and without this code.

use crate::bench::Expect;
use crate::config::Config;
use crate::index::{dense::DenseIndex, embed::Embedder, lexical::Lexical};
use crate::query::{self, Options};
use crate::store::Store;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// `expect` is `bench`'s own type: `dump` is the offline record of the suites `bench` scores, so
/// the two have to agree on what a case expects, list-valued anchors included.
#[derive(Debug, Deserialize)]
struct Query { q: String, expect: Expect, kind: String }

#[derive(Serialize)]
struct Exact { ids: Vec<String>, whole_question: bool }

#[derive(Serialize)]
struct Ask { seeds: Vec<(String, f32)>, expanded: Vec<(String, f32, String)> }

#[derive(Serialize)]
struct Record {
    q: String,
    expect: Expect,
    kind: String,
    exact: Exact,
    qvec: Vec<f32>,
    dense_passages: Vec<(String, f32)>,
    bm25_passages: Vec<(String, f32)>,
    /// Empty on a store without text nodes, as `Lexical::build` leaves the text index unbuilt.
    bm25_text: Vec<(String, f32)>,
    /// What each query could reach in the index its list came from, the denominator of the
    /// coverage admission (`LexicalIndex::attainable`). Recorded so an admission rule can be
    /// replayed over these lists offline, one binary and no re-run per candidate rule.
    attainable_passages: f32,
    attainable_text: f32,
    ask: Ask,
}

#[derive(Serialize)]
struct Meta { store: String, depth: usize, rows: usize, dim: usize, queries: usize, dense: bool }

#[derive(Serialize)]
struct Dump { meta: Meta, queries: Vec<Record> }

pub fn run(repo: &Path, queries: &Path, out: &Path, depth: usize, no_dense: bool) -> Result<()> {
    let cfg = Config::load(repo)?;
    // `dump` does not come through `main`'s arms, so the pools it owns are capped here.
    let threads = crate::index::embed::threads(cfg.resources);
    crate::cap_pools(threads);
    let store = Store::new(repo);
    let (graph, _) = store.load()?;
    if graph.nodes.is_empty() { anyhow::bail!("graph is empty at {} — run build first", repo.display()); }
    let dense_idx = DenseIndex::load(&store)?;
    if !no_dense && dense_idx.ids.is_empty() { anyhow::bail!("dense index is empty at {} — run `repograph update` first", repo.display()); }
    let text = std::fs::read_to_string(queries).with_context(|| queries.display().to_string())?;
    let queries: Vec<Query> = text.lines().filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Query>(l).map_err(anyhow::Error::from))
        .collect::<Result<_>>()?;
    let lex = Lexical::build(&graph);
    // `--no-dense` records the lexical-only arm: the dense lists stay empty and `ask` answers
    // without them, exactly as `ask --no-dense` would, in the arm the floors also grade.
    let mut embedder = if no_dense {
        None
    } else {
        match Embedder::open(&crate::index::embed::resolve(DenseIndex::recorded_model(&store)?.as_deref(), &cfg.embed_model), threads, crate::index::embed::Weights::Packed) {
            Ok(e) => Some(e),
            Err(err) => anyhow::bail!("dense: model unavailable ({err:#})"),
        }
    };
    if let Some(e) = embedder.as_mut() {
        // A dump that silently searched 384-d queries against 1024-d rows would record empty
        // dense lists that read as the lexical-only arm.
        let width = e.dim()?;
        if dense_idx.dim > 0 && width != dense_idx.dim {
            anyhow::bail!("{}", crate::index::embed::width_mismatch(dense_idx.dim, e.name(), width));
        }
    }
    let opts = Options { seeds: 5, bodies: false, dense: !no_dense, json: false, depth: crate::rerank::DEPTH };
    let mut records = Vec::with_capacity(queries.len());
    for q in &queries {
        let words: Vec<String> = q.q.split_whitespace().map(str::to_string).collect();
        let (exact_ids, whole_question) = query::exact_seeds(&graph, &words);
        let qvec = match embedder.as_mut() { Some(e) => e.query(&q.q)?, None => Vec::new() };
        let dense_passages = if no_dense { Vec::new() } else { dense_idx.search_scored(&qvec, depth) };
        // The same vector `ask` would embed for this question, so the recorded answer is the
        // binary's own and an offline replication can be checked against it query by query.
        let dense_fn = |_: &str, k: usize| dense_idx.search(&qvec, k);
        let dense_arm: Option<query::Dense> = if no_dense { None } else { Some(&dense_fn) };
        let answer = query::ask(&graph, &lex, dense_arm, None, &words, &opts);
        records.push(Record {
            q: q.q.clone(),
            expect: q.expect.clone(),
            kind: q.kind.clone(),
            exact: Exact { ids: exact_ids, whole_question },
            bm25_passages: lex.passages.search(&q.q, depth),
            bm25_text: lex.text.as_ref().map(|i| i.search(&q.q, depth)).unwrap_or_default(),
            attainable_passages: lex.passages.attainable(&q.q),
            // -0.0, not 0.0: the sign an empty sum takes under `f32`'s `Sum`, so a store without
            // text nodes records the sum an absent index would give.
            attainable_text: lex.text.as_ref().map(|i| i.attainable(&q.q)).unwrap_or(-0.0),
            dense_passages,
            qvec,
            ask: Ask {
                seeds: answer.seeds.iter().map(|h| (h.id.clone(), h.score)).collect(),
                expanded: answer.expanded.iter().map(|h| (h.id.clone(), h.score, h.via.clone().unwrap_or_default())).collect(),
            },
        });
        eprintln!("dump: {:<10} {:<14} {}", q.kind, q.expect.key(), q.q);
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
        assert_eq!((q.q.as_str(), q.expect.key().as_str(), q.kind.as_str()), ("как отменить?", "FR-PAY-22", "keyword"));
    }

    // Half the developer suite anchors a question on more than one place; a dump that joined
    // those into one string would lose the anchors the offline retrieval maths scores against.
    #[test]
    fn a_query_line_takes_one_anchor_or_several_and_writes_back_the_shape_it_read() {
        let one: Query = serde_json::from_str(r#"{"q": "a", "expect": "FR-PAY-22", "kind": "keyword"}"#).unwrap();
        assert_eq!(one.expect.anchors(), ["FR-PAY-22"]);
        assert_eq!(serde_json::to_value(&one.expect).unwrap(), serde_json::json!("FR-PAY-22"));

        let many: Query = serde_json::from_str(r#"{"q": "b", "expect": ["FR-PAY-22", "packages/pay/refund.ts"], "kind": "multi"}"#).unwrap();
        assert_eq!(many.expect.anchors(), ["FR-PAY-22", "packages/pay/refund.ts"]);
        assert_eq!(serde_json::to_value(&many.expect).unwrap(), serde_json::json!(["FR-PAY-22", "packages/pay/refund.ts"]));
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
            bm25_passages: vec![],
            bm25_text: vec![],
            attainable_passages: 1.5,
            attainable_text: 0.0,
            ask: Ask { seeds: vec![("FR-PAY-22".into(), 1.0)], expanded: vec![("N-151".into(), 0.5, "FR-PAY-22".into())] },
        };
        let v = serde_json::to_value(&record).unwrap();
        for key in ["q", "expect", "kind", "exact", "qvec", "dense_passages", "bm25_passages", "bm25_text", "attainable_passages", "attainable_text", "ask"] {
            assert!(v.get(key).is_some(), "missing field {key}");
        }
        assert_eq!(v["expect"], "FR-PAY-22");
        assert_eq!(v["exact"]["whole_question"], true);
        assert_eq!(v["ask"]["expanded"][0][2], "FR-PAY-22");
        let text = serde_json::to_string(&record).unwrap();
        assert!(text.contains(r#""attainable_passages":1.5"#), "{text}");
    }
}
