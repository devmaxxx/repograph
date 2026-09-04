//! A cross-encoder over the fused candidate list: the question and each candidate's text go
//! through one model together and come out as a relevance score. `ask --rerank` sends that
//! pool to a model command (≈19k input tokens and ~4 s a question, 14/14 on the old paraphrase
//! set); this runs a local model at zero tokens. Opt-in, off every floor until it is measured
//! against them, and loaded through the same `ort` + `tokenizers` path as the embedder.
use anyhow::{anyhow, Context, Result};
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use std::path::{Path, PathBuf};
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

/// Question plus a 120-character snippet is well under this; the cap is the model's.
const MAX_TOKENS: usize = 512;
const BATCH: usize = 16;
/// How many ids the reranker hands back, best first — `ask` shows five seeds.
pub const PICK: usize = 5;

pub struct CrossEncoder { session: Session, tokenizer: Tokenizer, wants_type_ids: bool }

/// Where `optimum-cli export onnx` was told to write: `$HOME/.cache/repograph/reranker`, beside
/// the embedder's own cache, unless the config names another directory.
pub fn default_dir() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".cache").join("repograph").join("reranker"))
}

impl CrossEncoder {
    /// `dir` holds `model.onnx` and `tokenizer.json` as the exporter writes them.
    pub fn open(dir: &Path) -> Result<CrossEncoder> {
        let mut tokenizer = Tokenizer::from_file(dir.join("tokenizer.json"))
            .map_err(|e| anyhow!("{e}")).with_context(|| format!("open reranker tokenizer in {}", dir.display()))?;
        tokenizer.with_truncation(Some(TruncationParams { max_length: MAX_TOKENS, ..Default::default() })).map_err(|e| anyhow!("{e}"))?;
        tokenizer.with_padding(Some(PaddingParams { strategy: PaddingStrategy::BatchLongest, ..Default::default() }));
        let session = Session::builder().map_err(|e| anyhow!("{e}"))?
            .with_optimization_level(GraphOptimizationLevel::Level1).map_err(|e| anyhow!("{e}"))?
            .commit_from_file(dir.join("model.onnx")).map_err(|e| anyhow!("{e}"))
            .with_context(|| format!("open reranker model in {}", dir.display()))?;
        let wants_type_ids = session.inputs().iter().any(|i| i.name() == "token_type_ids");
        Ok(CrossEncoder { session, tokenizer, wants_type_ids })
    }

    /// One score per text, higher is closer: the model's single logit, unnormalised, because
    /// only the order is used.
    pub fn score(&mut self, question: &str, texts: &[String]) -> Result<Vec<f32>> {
        let mut out = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(BATCH) {
            let pairs: Vec<(String, String)> = chunk.iter().map(|t| (question.to_string(), t.clone())).collect();
            let encodings = self.tokenizer.encode_batch(pairs, true).map_err(|e| anyhow!("{e}"))?;
            let batch = encodings.len();
            let len = encodings.first().map_or(0, |e| e.len());
            let mut ids = Vec::with_capacity(batch * len);
            let mut mask = Vec::with_capacity(batch * len);
            let mut types = Vec::with_capacity(batch * len);
            for e in &encodings {
                ids.extend(e.get_ids().iter().map(|&x| x as i64));
                mask.extend(e.get_attention_mask().iter().map(|&x| x as i64));
                types.extend(e.get_type_ids().iter().map(|&x| x as i64));
            }
            let mut inputs = ort::inputs![
                "input_ids" => Tensor::from_array(([batch, len], ids))?,
                "attention_mask" => Tensor::from_array(([batch, len], mask))?,
            ];
            if self.wants_type_ids {
                inputs.push(("token_type_ids".into(), Tensor::from_array(([batch, len], types))?.into()));
            }
            let outputs = self.session.run(inputs)?;
            let logits = outputs.get("logits")
                .or_else(|| (outputs.len() == 1).then(|| &outputs[0]))
                .context("reranker has no logits output")?;
            // [batch, 1]: one logit per (question, text) pair.
            let (_, data) = logits.try_extract_tensor::<f32>()?;
            out.extend(data.iter().copied());
        }
        Ok(out)
    }
}

/// The ids of the `k` best-scoring candidates, best first; a tie keeps the fused order.
pub fn pick(scores: &[f32], candidates: &[(String, String)], k: usize) -> Vec<String> {
    let mut order: Vec<usize> = (0..candidates.len().min(scores.len())).collect();
    order.sort_by(|&a, &b| scores[b].partial_cmp(&scores[a]).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b)));
    order.into_iter().take(k).map(|i| candidates[i].0.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cands() -> Vec<(String, String)> {
        vec![("a".to_string(), String::new()), ("b".to_string(), String::new()), ("c".to_string(), String::new())]
    }

    #[test]
    fn pick_orders_by_score_and_breaks_a_tie_by_fused_order() {
        assert_eq!(pick(&[0.1, 0.9, 0.9], &cands(), 5), vec!["b", "c", "a"]);
        assert_eq!(pick(&[0.1, 0.9, 0.9], &cands(), 1), vec!["b"]);
    }

    #[test]
    fn pick_survives_fewer_scores_than_candidates_and_none_at_all() {
        assert_eq!(pick(&[0.5], &cands(), 5), vec!["a"]);
        assert!(pick(&[], &cands(), 5).is_empty());
    }

    #[test]
    fn a_nan_score_sorts_as_a_tie_not_a_panic() {
        assert_eq!(pick(&[f32::NAN, 1.0, 0.0], &cands(), 3).len(), 3);
    }

    #[test]
    #[ignore = "needs the exported model: set REPOGRAPH_RERANKER_DIR"]
    fn the_model_prefers_the_passage_that_answers_the_question() {
        let dir = std::env::var("REPOGRAPH_RERANKER_DIR").expect("REPOGRAPH_RERANKER_DIR");
        let mut m = CrossEncoder::open(Path::new(&dir)).unwrap();
        let s = m.score("how is a client's phone number stored", &[
            "Phone numbers are normalised to E.164 before they are stored.".into(),
            "The calendar grid uses a five-minute lattice.".into(),
        ]).unwrap();
        assert!(s[0] > s[1], "{s:?}");
    }
}
