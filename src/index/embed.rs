//! The e5 embedder over its own ONNX session, opened by hub id.
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

pub const DEFAULT_MODEL: &str = "intfloat/multilingual-e5-large";

/// What a store that records no model at all was written with. Pinned to the name rather than to
/// `DEFAULT_MODEL`: every store written before the field existed holds small-model rows, and that
/// stays true however the default moves afterwards. Tying the two together would tell a reader
/// that yesterday's 384-d store is today's default, and re-embed it whole to find out otherwise.
pub const UNNAMED_MODEL: &str = "intfloat/multilingual-e5-small";

/// The model a command opens: `REPOGRAPH_EMBED_MODEL` when set — a measurement's switch that
/// outranks both files — else what the store's vectors were written with, else the configured
/// one. A reader passes the recorded model so a store answers with what wrote it; a writer
/// passes `None` and takes the configuration, and `DenseIndex::written_by` drops rows another
/// model wrote before the sync.
pub fn resolve(recorded: Option<&str>, configured: &str) -> String {
    resolve_from(std::env::var("REPOGRAPH_EMBED_MODEL").ok().as_deref(), recorded, configured)
}

fn resolve_from(override_: Option<&str>, recorded: Option<&str>, configured: &str) -> String {
    let named = |m: Option<&str>| m.map(str::trim).filter(|m| !m.is_empty()).map(str::to_string);
    named(override_).or_else(|| named(recorded)).unwrap_or_else(|| configured.to_string())
}

/// What `ask`, `bench` and `dump` say when the model they opened cannot search the store's rows.
/// One sentence for the three of them: each keeps its own consequence — a warning and a
/// lexical-only answer, or a refused run — but a caller wording the width differently from the
/// others would send a reader looking for three separate faults.
pub fn width_mismatch(dim: usize, model: &str, got: usize) -> String {
    format!("the store's vectors are {dim}-d and {model} gives {got}-d — run `repograph embed`")
}
const MAX_TOKENS: usize = 256;
const BATCH: usize = 64;

pub struct Embedder {
    session: Session,
    tokenizer: Tokenizer,
    wants_type_ids: bool,
    name: String,
    dim: Option<usize>,
}

/// A cache chosen from inside the process, consulted before the environment. It exists because a
/// test cannot reach for `set_var`: a `setenv` racing another thread's `getenv` is undefined
/// behaviour, and this binary reads the environment on every `ask`. Nothing outside the test
/// binary sets it, so a real run resolves its cache exactly as it always has.
static CACHE_OVERRIDE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

#[cfg(test)]
pub(crate) fn set_cache_dir(dir: PathBuf) {
    let _ = CACHE_OVERRIDE.set(dir);
}

/// The `.fastembed_cache`-under-cwd default of the hub client re-downloads the model per
/// directory `repograph` is run from — 2.1 GB for the default one — and fails outright on a
/// read-only one.
fn cache_dir() -> Result<PathBuf> {
    if let Some(dir) = CACHE_OVERRIDE.get() {
        return Ok(dir.clone());
    }
    if let Some(dir) = std::env::var_os("FASTEMBED_CACHE_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME").context("neither FASTEMBED_CACHE_DIR nor HOME is set")?;
    Ok(PathBuf::from(home).join(".cache").join("repograph").join("fastembed"))
}

struct Files { model: PathBuf, tokenizer: PathBuf, pad_token: String, pad_id: u32 }

/// A graph-only `model.onnx` keeps its weights in `model.onnx_data` beside it and is a few MB;
/// the small model's 448 MB file holds them itself. Asking the hub for a data file a model never
/// had is a network round trip on every fused query — 240 ms measured, and a failure offline.
const EXTERNAL_DATA_STUB: u64 = 64 << 20;

fn keeps_weights_beside(model_len: u64) -> bool { model_len < EXTERNAL_DATA_STUB }

/// Cache hits never touch the network; the first run downloads with a progress bar.
fn fetch(model: &str) -> Result<Files> { fetch_from(&cache_dir()?, None, model) }

fn fetch_from(cache: &Path, endpoint: Option<&str>, model: &str) -> Result<Files> {
    let mut builder = hf_hub::api::sync::ApiBuilder::new().with_cache_dir(cache.to_path_buf()).with_progress(true);
    if let Some(e) = endpoint { builder = builder.with_endpoint(e.to_string()); }
    let api = builder.build()?;
    let name = model.to_string();
    let repo = api.model(name.clone());
    let get = |f: &str| repo.get(f).with_context(|| format!("fetch {name}/{f}"));
    let model = get("onnx/model.onnx")?;
    // The larger models keep their weights beside the graph; the session resolves the file by
    // its relative name, so it has to be fetched into the same snapshot. Only a stub-sized graph
    // can have one, so the small model never pays the lookup.
    let len = std::fs::metadata(&model).map(|m| m.len()).unwrap_or(0);
    if keeps_weights_beside(len) { let _ = repo.get("onnx/model.onnx_data"); }
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
    pub fn open(model: &str) -> Result<Embedder> {
        let files = fetch(model)?;
        let session = std::thread::scope(|s| {
            let tokenizer = s.spawn(|| load_tokenizer(&files));
            let session = load_session(&files.model).context("open embedding model")?;
            let tokenizer = tokenizer.join().map_err(|_| anyhow!("tokenizer thread panicked"))?.context("open tokenizer")?;
            Ok::<_, anyhow::Error>((session, tokenizer))
        });
        let (session, tokenizer) = session?;
        let wants_type_ids = session.inputs().iter().any(|i| i.name() == "token_type_ids");
        Ok(Embedder { session, tokenizer, wants_type_ids, name: model.to_string(), dim: None })
    }

    /// The hub id the vectors this embedder writes belong to; recorded in the store by `written_by`.
    pub fn name(&self) -> &str { &self.name }

    /// The width of the vectors this model gives. Only a forward pass knows it, so one short
    /// string is embedded and the answer kept: a caller comparing the model against a store's
    /// rows would otherwise pay that pass on every query.
    pub fn dim(&mut self) -> Result<usize> {
        match self.dim {
            Some(d) => Ok(d),
            None => {
                let d = self.query("probe")?.len();
                self.dim = Some(d);
                Ok(d)
            }
        }
    }

    /// Texts arrive already e5-prefixed (`dense::rows`): passages as `passage: `, generated
    /// questions as `query: `. A batch is padded to its longest member, so texts are batched
    /// by length — a ten-token label no longer rides in a 256-token batch.
    pub fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut out: Vec<Vec<f32>> = vec![Vec::new(); texts.len()];
        for chunk in length_batches(texts, BATCH) {
            let batch: Vec<String> = chunk.iter().map(|&i| texts[i].clone()).collect();
            for (i, v) in chunk.into_iter().zip(self.forward(&batch)?) {
                out[i] = v;
            }
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

/// Indices of `texts` grouped `batch` at a time in ascending byte length, so every batch pads
/// to a neighbour's length rather than to the corpus maximum.
fn length_batches(texts: &[String], batch: usize) -> Vec<Vec<usize>> {
    let mut order: Vec<usize> = (0..texts.len()).collect();
    order.sort_by_key(|&i| texts[i].len());
    order.chunks(batch).map(<[usize]>::to_vec).collect()
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
    fn the_override_outranks_the_store_which_outranks_the_config() {
        assert_eq!(resolve_from(Some(" x/y "), Some("a/b"), "c/d"), "x/y");
        assert_eq!(resolve_from(Some(""), Some("a/b"), "c/d"), "a/b");
        assert_eq!(resolve_from(None, None, "c/d"), "c/d");
        assert_eq!(resolve_from(None, Some("  "), "c/d"), "c/d");
    }

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
    fn length_batches_group_neighbours_and_cover_every_text_once() {
        let texts: Vec<String> = ["aaaa", "b", "cc", "ddddd", "eee"].iter().map(|s| s.to_string()).collect();
        let batches = length_batches(&texts, 2);
        assert_eq!(batches, vec![vec![1, 2], vec![4, 0], vec![3]]);
        assert!(length_batches(&[], 2).is_empty());
    }

    #[test]
    fn an_all_masked_row_is_a_zero_vector_not_a_nan() {
        let v = pool(&[1.0, 1.0], &[0], 1, 2);
        assert_eq!(v, vec![vec![0.0, 0.0]]);
    }

    #[test]
    fn normalise_leaves_a_zero_vector_untouched() {
        let mut v = [0.0f32, 0.0];
        normalise(&mut v);
        assert_eq!(v, [0.0, 0.0]);
    }

    #[test]
    fn normalise_scales_a_nonzero_vector_to_unit_length() {
        let mut v = [3.0f32, 4.0];
        normalise(&mut v);
        assert!((v[0] - 0.6).abs() < 1e-6 && (v[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn length_batches_keeps_original_order_among_equal_length_ties() {
        let texts: Vec<String> = ["aa", "bb", "cc", "dd"].iter().map(|s| s.to_string()).collect();
        // Every text is the same length: a stable sort must not reorder them.
        assert_eq!(length_batches(&texts, 10), vec![vec![0, 1, 2, 3]]);
    }

    #[test]
    fn length_batches_with_batch_larger_than_the_collection_yields_one_sorted_batch() {
        let texts: Vec<String> = ["mm", "z", "a"].iter().map(|s| s.to_string()).collect();
        assert_eq!(length_batches(&texts, 100), vec![vec![1, 2, 0]]);
    }

    #[test]
    fn pool_with_zero_length_rows_yields_no_vectors_not_a_panic() {
        assert!(pool(&[], &[], 0, 2).is_empty());
    }
}

#[cfg(test)]
mod fetch_tests {
    use super::*;

    /// A cache in hf-hub's layout for one model, with the model file sized as asked; the other
    /// three files are small and valid enough for `fetch` to read them.
    fn cache_with_model_of(dir: &std::path::Path, model_len: u64) {
        let root = dir.join("models--intfloat--multilingual-e5-small");
        std::fs::create_dir_all(root.join("refs")).unwrap();
        std::fs::write(root.join("refs/main"), "abc").unwrap();
        let snap = root.join("snapshots/abc");
        std::fs::create_dir_all(snap.join("onnx")).unwrap();
        let model = std::fs::File::create(snap.join("onnx/model.onnx")).unwrap();
        model.set_len(model_len).unwrap();
        std::fs::write(snap.join("tokenizer.json"), "{}").unwrap();
        std::fs::write(snap.join("config.json"), r#"{"pad_token_id": 1}"#).unwrap();
        std::fs::write(snap.join("tokenizer_config.json"), r#"{"pad_token": "<pad>"}"#).unwrap();
    }

    #[test]
    fn a_model_that_holds_its_weights_is_fetched_without_the_network() {
        let dir = tempfile::tempdir().unwrap();
        cache_with_model_of(dir.path(), EXTERNAL_DATA_STUB);
        // An endpoint nothing listens on: any lookup that leaves the cache fails at once.
        let files = fetch_from(dir.path(), Some("http://127.0.0.1:9"), "intfloat/multilingual-e5-small").unwrap();
        assert!(files.model.ends_with("onnx/model.onnx"));
        assert_eq!(files.pad_token, "<pad>");
        assert_eq!(files.pad_id, 1);
    }

    #[test]
    fn a_graph_only_stub_asks_for_the_weights_beside_it() {
        let dir = tempfile::tempdir().unwrap();
        cache_with_model_of(dir.path(), 1 << 20);
        // The stub's data file is not cached and the endpoint is dead, so the lookup fails; the
        // fetch still answers, as it did before, and the session open reports the missing file.
        let files = fetch_from(dir.path(), Some("http://127.0.0.1:9"), "intfloat/multilingual-e5-small").unwrap();
        assert!(files.model.ends_with("onnx/model.onnx"));
    }

    #[test]
    fn the_stub_threshold_separates_the_small_models_file_from_a_graph_only_one() {
        assert!(!keeps_weights_beside(448 << 20));
        assert!(!keeps_weights_beside(EXTERNAL_DATA_STUB));
        assert!(keeps_weights_beside(EXTERNAL_DATA_STUB - 1));
        assert!(keeps_weights_beside(2 << 20));
    }

    /// A listener that counts the connections a fetch opens. A dead port refuses every connection
    /// alike, so only a socket that accepts can tell a lookup that was skipped from one that was
    /// made and failed. Each accepted stream is dropped at once, so the client gives up instead of
    /// waiting for a reply, and the accept loop polls a flag so it can never outlive the test.
    fn counting_listener() -> (u16, impl FnOnce() -> usize) {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let seen = std::sync::Arc::new(AtomicUsize::new(0));
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let (counted, halt) = (seen.clone(), stop.clone());
        let accepting = std::thread::spawn(move || {
            while !halt.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok(_) => { counted.fetch_add(1, Ordering::SeqCst); }
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(1)),
                }
            }
        });
        (port, move || {
            stop.store(true, Ordering::SeqCst);
            accepting.join().unwrap();
            seen.load(Ordering::SeqCst)
        })
    }

    #[test]
    fn only_a_stub_sized_graph_opens_a_connection_for_the_weights_beside_it() {
        let connections_for = |model_len: u64| {
            let dir = tempfile::tempdir().unwrap();
            cache_with_model_of(dir.path(), model_len);
            let (port, connections) = counting_listener();
            let endpoint = format!("http://127.0.0.1:{port}");
            fetch_from(dir.path(), Some(&endpoint), "intfloat/multilingual-e5-small").unwrap();
            connections()
        };
        assert_eq!(connections_for(EXTERNAL_DATA_STUB), 0, "a model that holds its weights asks the hub for nothing");
        assert!(connections_for(1 << 20) >= 1, "a stub-sized graph asks the hub for the weights beside it");
    }
}
