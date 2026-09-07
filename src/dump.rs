//! `repograph dump`: every retriever's ranked list for a set of questions, written once so the
//! retrieval mathematics can be done offline, without the model and without this code.

use crate::bench::Expect;
use crate::config::Config;
use crate::enrich::Questions;
use crate::ids::IdMatcher;
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
    dense_questions: Vec<(String, f32)>,
    bm25_passages: Vec<(String, f32)>,
    bm25_questions: Vec<(String, f32)>,
    bm25_code: Vec<(String, f32)>,
    /// What each query could reach in the index its list came from, the denominator of the
    /// coverage admission (`LexicalIndex::attainable`). Recorded so an admission rule can be
    /// replayed over these lists offline, one binary and no re-run per candidate rule.
    attainable_passages: f32,
    attainable_questions: f32,
    attainable_code: f32,
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
    // `dump` does not come through `main`'s arms, so the pools it owns are capped here.
    let threads = crate::index::embed::threads(cfg.threads);
    crate::cap_pools(threads);
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
        for anchor in q.expect.anchors() {
            if let Some(e) = loo.entries.get_mut(anchor) { e.questions.retain(|t| t != &q.q); }
        }
    }
    // A dump is a diagnostic record of every retriever, not the fusion any one query took, so
    // it always asks for the code list — unlike a resident `Context`, one build here serves
    // every query in the suite.
    let lex = Lexical::build(&graph, &loo, true);
    // `--no-dense` records the lexical-only arm: the dense lists stay empty and `ask` answers
    // without them, exactly as `ask --no-dense` would, so the held-out gate can be read in the
    // arm the floors also grade.
    let mut embedder = if no_dense {
        None
    } else {
        match Embedder::open(&crate::index::embed::resolve(DenseIndex::recorded_model(&store)?.as_deref(), &cfg.embed_model), threads) {
            Ok(e) => Some(e),
            Err(err) => anyhow::bail!("dense: model unavailable ({err:#})"),
        }
    };
    if let Some(e) = embedder.as_mut() {
        // A dump that silently searched 384-d queries against 1024-d rows would record empty
        // dense lists, and the held-out gate would read them as the lexical-only arm.
        let width = e.dim()?;
        if dense_idx.dim > 0 && width != dense_idx.dim {
            anyhow::bail!("{}", crate::index::embed::width_mismatch(dense_idx.dim, e.name(), width));
        }
    }
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
        let answer = query::ask(&graph, &ids, &lex, dense_arm, None, &words, &opts);
        records.push(Record {
            q: q.q.clone(),
            expect: q.expect.clone(),
            kind: q.kind.clone(),
            exact: Exact { ids: exact_ids, whole_question },
            bm25_passages: lex.passages.search(&q.q, depth),
            // A store with no questions never builds this index at all (`Lexical::build`'s
            // `entries.is_empty()` guard) — `build_questions` on empty entries is a real,
            // non-empty index of bare ids, so the guard is what keeps the list out of `ask`'s
            // fusion, and this `[]` records that same absence rather than a list nothing reads.
            bm25_questions: lex.questions.as_ref().map(|i| i.search(&q.q, depth)).unwrap_or_default(),
            bm25_code: lex.code.as_ref().map(|i| i.search(&q.q, depth)).unwrap_or_default(),
            attainable_passages: lex.passages.attainable(&q.q),
            // -0.0, not 0.0: the sign an empty sum takes under `f32`'s `Sum`. `questions`' index
            // is never built at all on a store with none (see `bm25_questions` above), so -0.0
            // here stands for the sum an absent index would give, not one an actual build
            // produced — an index that was built and holds no documents attains a plain 0.0.
            attainable_questions: lex.questions.as_ref().map(|i| i.attainable(&q.q)).unwrap_or(-0.0),
            attainable_code: lex.code.as_ref().map(|i| i.attainable(&q.q)).unwrap_or(-0.0),
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

    // Pins the decision in the two comments above: a graph `enrich` never touched dumps an empty
    // questions list and the -0.0 an absent index's sum matches, not the real ranked list and
    // positive sum a direct `LexicalIndex::build_questions` call over empty entries would give.
    #[test]
    fn a_raw_store_dumps_no_questions_list_and_the_empty_sum_it_matches() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("docs/a.md"), "# A\n\n**FR-PAY-22 · MUST · cancellation window**\n\nbody\n").unwrap();
        let repo = dir.path();
        let cfg = Config::default();
        crate::run_update(repo, &cfg, &crate::extractors(repo, &cfg).unwrap(), true).unwrap();
        let queries_path = dir.path().join("queries.jsonl");
        // The query is the id itself, not a word from the label: an index of bare ids (what
        // `build_questions` over empty entries actually builds) ranks this above the passage
        // that carries it, so this is the shape that would slip past a weaker query untouched.
        std::fs::write(&queries_path, r#"{"q": "FR-PAY-22", "expect": "FR-PAY-22", "kind": "keyword"}"#).unwrap();
        let out_path = dir.path().join("out.json");
        run(repo, &queries_path, &out_path, 5, true).unwrap();
        let text = std::fs::read_to_string(&out_path).unwrap();
        let dump: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(dump["queries"][0]["bm25_questions"], serde_json::json!([]));
        assert!(text.contains(r#""attainable_questions":-0.0"#), "{text}");
    }

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
            dense_questions: vec![],
            bm25_passages: vec![],
            bm25_questions: vec![],
            bm25_code: vec![],
            attainable_passages: 1.5,
            attainable_questions: 0.0,
            attainable_code: 0.0,
            loo_hash: "abc".into(),
            loo_rows: vec![3],
            ask: Ask { seeds: vec![("FR-PAY-22".into(), 1.0)], expanded: vec![("N-151".into(), 0.5, "FR-PAY-22".into())] },
        };
        let v = serde_json::to_value(&record).unwrap();
        for key in ["q", "expect", "kind", "exact", "qvec", "dense_passages", "dense_questions", "bm25_passages", "bm25_questions", "bm25_code", "attainable_passages", "attainable_questions", "attainable_code", "loo_hash", "loo_rows", "ask"] {
            assert!(v.get(key).is_some(), "missing field {key}");
        }
        assert_eq!(v["expect"], "FR-PAY-22");
        assert_eq!(v["exact"]["whole_question"], true);
        assert_eq!(v["ask"]["expanded"][0][2], "FR-PAY-22");
        let text = serde_json::to_string(&record).unwrap();
        assert!(text.contains(r#""attainable_passages":1.5"#), "{text}");
        assert!(text.contains(r#""attainable_questions":0.0"#), "{text}");
        assert!(text.contains(r#""attainable_code":0.0"#), "{text}");
    }
}
