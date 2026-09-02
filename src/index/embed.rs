//! The e5-small embedder over its own ONNX session.
//!
//! fastembed opened the same files behind a private session builder: tokenizer first, then a
//! session forced to the heaviest graph optimisation. Measured on the cached model, the
//! tokenizer takes ~0.35 s to deserialise (a 16 MB unigram vocabulary) and the session ~0.25 s
//! at Level1 — Level3 costs up to 0.4 s more to open and buys nothing on a 256-token query. Opening
//! the two concurrently caps model open at the slower of them, which halves a fused `ask`.

use anyhow::{anyhow, Context, Result};
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use std::path::{Path, PathBuf};
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

const MODEL: &str = "intfloat/multilingual-e5-small";
const MAX_TOKENS: usize = 256;
const BATCH: usize = 64;

pub struct Embedder {
    session: Session,
    tokenizer: Tokenizer,
    wants_type_ids: bool,
}

/// The `.fastembed_cache`-under-cwd default of the hub client re-downloads 470 MB per directory
/// `repograph` is run from and fails outright on a read-only one.
fn cache_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("FASTEMBED_CACHE_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME").context("neither FASTEMBED_CACHE_DIR nor HOME is set")?;
    Ok(PathBuf::from(home).join(".cache").join("repograph").join("fastembed"))
}

struct Files { model: PathBuf, tokenizer: PathBuf, pad_token: String, pad_id: u32 }

/// Cache hits never touch the network; the first run downloads with a progress bar.
fn fetch() -> Result<Files> {
    let api = hf_hub::api::sync::ApiBuilder::new().with_cache_dir(cache_dir()?).with_progress(true).build()?;
    let repo = api.model(MODEL.to_string());
    let get = |f: &str| repo.get(f).with_context(|| format!("fetch {MODEL}/{f}"));
    let model = get("onnx/model.onnx")?;
    let tokenizer = get("tokenizer.json")?;
    let config: serde_json::Value = serde_json::from_slice(&std::fs::read(get("config.json")?)?)?;
    let tok_config: serde_json::Value = serde_json::from_slice(&std::fs::read(get("tokenizer_config.json")?)?)?;
    let pad_token = tok_config["pad_token"].as_str().context("tokenizer_config.json: pad_token")?.to_string();
    let pad_id = config["pad_token_id"].as_u64().unwrap_or(0) as u32;
    Ok(Files { model, tokenizer, pad_token, pad_id })
}

fn load_tokenizer(files: &Files) -> Result<Tokenizer> {
    let mut tk = Tokenizer::from_file(&files.tokenizer).map_err(|e| anyhow!("{e}"))?;
    tk.with_truncation(Some(TruncationParams { max_length: MAX_TOKENS, ..Default::default() })).map_err(|e| anyhow!("{e}"))?;
    tk.with_padding(Some(PaddingParams {
        strategy: PaddingStrategy::BatchLongest,
        pad_token: files.pad_token.clone(),
        pad_id: files.pad_id,
        ..Default::default()
    }));
    Ok(tk)
}

fn load_session(model: &Path) -> Result<Session> {
    let mut builder = Session::builder().map_err(|e| anyhow!("{e}"))?
        .with_optimization_level(GraphOptimizationLevel::Level1).map_err(|e| anyhow!("{e}"))?;
    builder.commit_from_file(model).map_err(|e| anyhow!("{e}"))
}

fn normalise(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 { for x in v { *x /= norm; } }
}

impl Embedder {
    pub fn open() -> Result<Embedder> {
        let files = fetch()?;
        let session = std::thread::scope(|s| {
            let tokenizer = s.spawn(|| load_tokenizer(&files));
            let session = load_session(&files.model).context("open embedding model")?;
            let tokenizer = tokenizer.join().map_err(|_| anyhow!("tokenizer thread panicked"))?.context("open tokenizer")?;
            Ok::<_, anyhow::Error>((session, tokenizer))
        });
        let (session, tokenizer) = session?;
        let wants_type_ids = session.inputs().iter().any(|i| i.name() == "token_type_ids");
        Ok(Embedder { session, tokenizer, wants_type_ids })
    }

    /// Texts arrive already e5-prefixed (`dense::rows`): passages as `passage: `, generated
    /// questions as `query: `.
    pub fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut out = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(BATCH) {
            out.extend(self.forward(chunk)?);
        }
        Ok(out)
    }

    pub fn query(&mut self, text: &str) -> Result<Vec<f32>> {
        Ok(self.forward(&[format!("query: {text}")])?.remove(0))
    }

    fn forward(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let encodings = self.tokenizer
            .encode_batch(texts.iter().map(String::as_str).collect(), true)
            .map_err(|e| anyhow!("{e}"))?;
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
            "attention_mask" => Tensor::from_array(([batch, len], mask.clone()))?,
        ];
        if self.wants_type_ids {
            inputs.push(("token_type_ids".into(), Tensor::from_array(([batch, len], types))?.into()));
        }
        let outputs = self.session.run(inputs)?;
        let hidden = outputs.get("last_hidden_state")
            .or_else(|| (outputs.len() == 1).then(|| &outputs[0]))
            .context("model has no last_hidden_state output")?;
        let (shape, data) = hidden.try_extract_tensor::<f32>()?;
        let dim = *shape.get(2).context("last_hidden_state is not [batch, tokens, dim]")? as usize;
        Ok(pool(data, &mask, len, dim))
    }
}

/// Mean pooling over the attention mask, then L2-normalised — the model card's recipe.
fn pool(hidden: &[f32], mask: &[i64], len: usize, dim: usize) -> Vec<Vec<f32>> {
    let batch = mask.len().checked_div(len).unwrap_or(0);
    (0..batch).map(|b| {
        let mut v = vec![0f32; dim];
        let mut n = 0f32;
        for t in 0..len {
            if mask[b * len + t] == 0 { continue; }
            n += 1.0;
            for (x, y) in v.iter_mut().zip(&hidden[(b * len + t) * dim..][..dim]) { *x += y; }
        }
        for x in &mut v { *x /= n.max(1.0); }
        normalise(&mut v);
        v
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pooling_averages_only_unmasked_tokens_and_normalises() {
        // Two texts of two tokens each, dim 2; the second text has one padded position.
        let hidden = [1.0, 0.0, 0.0, 1.0, /* text 2 */ 3.0, 4.0, 100.0, 100.0];
        let mask = [1, 1, 1, 0];
        let v = pool(&hidden, &mask, 2, 2);
        let r = 0.5f32.sqrt();
        assert!((v[0][0] - r).abs() < 1e-6 && (v[0][1] - r).abs() < 1e-6);
        assert!((v[1][0] - 0.6).abs() < 1e-6 && (v[1][1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn an_all_masked_row_is_a_zero_vector_not_a_nan() {
        let v = pool(&[1.0, 1.0], &[0], 1, 2);
        assert_eq!(v, vec![vec![0.0, 0.0]]);
    }
}
