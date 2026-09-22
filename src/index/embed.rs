//! The e5 embedder over its own ONNX session, opened by hub id.
//!
//! fastembed opened the same files behind a private session builder: tokenizer first, then a
//! session forced to the heaviest graph optimisation. Measured on the cached model, the
//! tokenizer takes ~0.35 s to deserialise (a 16 MB unigram vocabulary) and the session ~0.25 s
//! at Level1 — Level3 costs up to 0.4 s more to open and buys nothing on a 256-token query. Opening
//! the two concurrently caps model open at the slower of them, which halves a fused `ask`.

use anyhow::{anyhow, bail, Context, Result};
use ort::session::{builder::{GraphOptimizationLevel, SessionBuilder}, Session};
use ort::value::Tensor;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

/// What a writer embeds with when the repository names no model: the small one, priced for a
/// first build nobody has tuned yet, and the only model `bench` holds floors for
/// (docs/adr/ADR-002-two-defaults-multiplied.md).
pub const DEFAULT_MODEL: &str = "intfloat/multilingual-e5-small";

/// What a store that records no model at all was written with. Pinned to the name rather than to
/// `DEFAULT_MODEL` — which holds the same string today and has already held the other one: every
/// store written before the field existed holds small-model rows, and that stays true however the
/// default moves afterwards. Tying the two together would tell a reader that yesterday's 384-d
/// store is today's default, and re-embed it whole to find out otherwise.
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
/// Texts per forward. Kept beside the token budget rather than replaced by it: sixty-four
/// twenty-token questions are 1,280 padded tokens, so on the short rows — most of the store —
/// the count is what closes a batch.
const BATCH: usize = 64;
/// Padded tokens per forward. A batch is padded to its longest member, so a count alone bounds
/// nothing: the sixty-four longest passages of any corpus form one 64 × 256 forward whose
/// attention scores are 268 MB a layer, and the arena the runtime grows for that shape is never
/// given back. The same texts through 8 × 256 forwards ran in the same wall time for 1.7 GB less
/// (docs/bench/2026-09-07-resource-usage-results.md), so the budget costs throughput nothing and
/// bounds the peak on a machine and a corpus this code has never seen.
const TOKEN_BUDGET: usize = 2048;
/// Texts per tokenizer pass when only their lengths are wanted. Big enough that the pass is a
/// handful of calls over a whole store, small enough that the padding it allocates is not.
const LENGTH_CHUNK: usize = 1024;

pub struct Embedder {
    session: Session,
    tokenizer: Tokenizer,
    wants_type_ids: bool,
    wants_position_ids: bool,
    /// A decoder exported with its generation cache still in the signature. An embedding is one
    /// pass over the whole text, so each of these is fed empty: name, heads, head width.
    cache_inputs: Vec<(String, usize, usize)>,
    profile: Profile,
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
    Ok(crate::index::cache_root()?.join("fastembed"))
}

/// How a model turns a padded batch's hidden states into one vector per text.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Pooling {
    Mean,
    /// The first position, which the BERT-family retrievers train as the sentence vector.
    Cls,
    /// The last attended position: a causal model has seen the whole text only there.
    LastToken,
}

/// What a model's card asks of its caller and its graph does not say: which file holds the
/// graph, how to pool, and what to put in front of a question and a passage. The store keeps
/// the e5 tags on every row whatever wrote it — `dense` tells a question row from a passage by
/// them, and row hashes are taken over them — so the words a model actually reads are swapped
/// in here, at the last step before the tokenizer.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Profile {
    onnx: &'static str,
    /// Files the graph resolves by relative name beyond `<onnx>_data`.
    beside: &'static [&'static str],
    pooling: Pooling,
    query: &'static str,
    passage: &'static str,
}

const E5: Profile = Profile { onnx: "onnx/model.onnx", beside: &[], pooling: Pooling::Mean, query: QUERY_TAG, passage: PASSAGE_TAG };
const QUERY_TAG: &str = "query: ";
const PASSAGE_TAG: &str = "passage: ";

/// An unknown model reads as e5: that is what every store written so far was embedded with, and
/// a wrong guess about pooling or prefixes costs recall rather than a crash, which `bench` exists
/// to show. A guess about `beside` is the one that does crash: the id is matched by substring, so
/// a look-alike repo without the named file fails to open rather than falling back.
fn profile(model: &str) -> Profile {
    let m = model.to_ascii_lowercase();
    let cls = Profile { pooling: Pooling::Cls, query: "", passage: "", ..E5 };
    if m.contains("snowflake-arctic-embed") {
        Profile { query: QUERY_TAG, ..cls }
    } else if m.contains("bge-m3") {
        Profile { beside: &["onnx/Constant_7_attr__value"], ..cls }
    } else if m.contains("granite-embedding") {
        // IBM publishes no ONNX graph; Teradata's export names its fp32 file differently.
        Profile { onnx: if m.starts_with("teradata/") { "onnx/model-fp32.onnx" } else { E5.onnx }, ..cls }
    } else if m.contains("embeddinggemma") {
        Profile { query: "task: search result | query: ", passage: "title: none | text: ", ..E5 }
    } else if m.contains("qwen3-embedding") {
        Profile {
            // Its graph file is 307 MB of embedding-free graph, past the size under which a
            // data file is looked for, and the 2 GB of weights still sit beside it.
            beside: &["onnx/model.onnx_data"],
            pooling: Pooling::LastToken,
            query: "Instruct: Given a web search query, retrieve relevant passages that answer the query\nQuery:",
            passage: "",
            ..E5
        }
    } else {
        E5
    }
}

/// Whether a model falls through to e5's recipe — its pooling, its `query: `/`passage: ` tags —
/// because nothing in `profile` recognised the id. True for the e5 models themselves, and for
/// every id this binary has never heard of: the guess costs recall rather than a crash, and a
/// reader about to spend a download on one is owed the sentence.
pub fn reads_as_e5(model: &str) -> bool { profile(model) == E5 }

/// Cases in the paraphrase arm of the 82-case suite. The arm an embedder moves: keyword and code
/// read 40/40 and 12/12 for every model measured but the smallest.
pub const PARAPHRASE_CASES: u32 = 30;

/// A model this project has measured end to end — one run each, on the pinned beauty-crm fixture's
/// 33,525 rows, enriched store, no rerank, fp32 ONNX on CPU
/// (docs/bench/2026-09-22-embedders-results.md). One list, so that `repograph model`, the README's
/// table and the prose quote the same figures; every id in it resolves to a profile `profile`
/// recognises, which a test holds.
pub struct Measured {
    pub model: &'static str,
    /// The width of the vectors, and so what a switch rewrites the whole index for.
    pub dim: u32,
    /// Hits of `PARAPHRASE_CASES`.
    pub paraphrase: u32,
    /// Seconds to embed the fixture's rows. Quoted as a ratio against the control's, which is
    /// the part of it that survives a different machine.
    pub embed_s: u32,
    /// What those rows take in `vectors.f32`.
    pub vectors_mb: u32,
    /// What the model's files take in the hub cache once fetched — the download a switch starts.
    pub cache: &'static str,
    pub licence: &'static str,
}

/// The control first, then the models that tie at the top of the paraphrase arm by what they cost
/// to embed, then the rest by recall. Four of them read 21/30 against the control's 15/30 and the
/// suite was run once per model, so the order inside that tie is cost and not rank: nothing here
/// separates them on recall.
pub const MEASURED: &[Measured] = &[
    Measured { model: DEFAULT_MODEL, dim: 384, paraphrase: 15, embed_s: 144, vectors_mb: 51, cache: "578M", licence: "MIT" },
    Measured { model: RECOMMENDED, dim: 768, paraphrase: 21, embed_s: 733, vectors_mb: 103, cache: "1.2G", licence: "Gemma" },
    Measured { model: "Snowflake/snowflake-arctic-embed-l-v2.0", dim: 1024, paraphrase: 21, embed_s: 1944, vectors_mb: 137, cache: "2.1G", licence: "Apache-2.0" },
    Measured { model: "BAAI/bge-m3", dim: 1024, paraphrase: 21, embed_s: 2059, vectors_mb: 137, cache: "2.1G", licence: "MIT" },
    Measured { model: "onnx-community/Qwen3-Embedding-0.6B-ONNX", dim: 1024, paraphrase: 21, embed_s: 4153, vectors_mb: 137, cache: "2.2G", licence: "Apache-2.0" },
    Measured { model: "intfloat/multilingual-e5-base", dim: 768, paraphrase: 17, embed_s: 608, vectors_mb: 103, cache: "1.0G", licence: "MIT" },
    Measured { model: "Teradata/granite-embedding-278m-multilingual", dim: 768, paraphrase: 16, embed_s: 548, vectors_mb: 103, cache: "1.0G", licence: "Apache-2.0" },
    Measured { model: "Teradata/granite-embedding-107m-multilingual", dim: 384, paraphrase: 14, embed_s: 97, vectors_mb: 51, cache: "424M", licence: "Apache-2.0" },
    Measured { model: "Snowflake/snowflake-arctic-embed-m-v2.0", dim: 768, paraphrase: 14, embed_s: 663, vectors_mb: 103, cache: "1.2G", licence: "Apache-2.0" },
];

/// The model the readings point at for a project that wants more recall than the default's. Not
/// the default itself: a first build pays for the model before anyone knows whether they need it,
/// and this one's licence is a decision somebody has to make rather than inherit.
pub const RECOMMENDED: &str = "onnx-community/embeddinggemma-300m-ONNX";

impl Measured {
    /// This model's whole-store embed against the default's — the number someone deciding whether
    /// to switch is actually weighing, and the one that carries to another machine.
    pub fn times_the_default(&self) -> f32 { self.embed_s as f32 / control().embed_s as f32 }
}

/// The catalogue's row for a hub id, or `None` for a model nobody here has measured. Not knowing
/// one is allowed: `embed_model` takes any id the hub serves, and this list says only which of
/// them have numbers behind them.
pub fn measured(model: &str) -> Option<&'static Measured> {
    MEASURED.iter().find(|m| m.model.eq_ignore_ascii_case(model))
}

/// The row every other row is quoted against. The default is in the list — a test holds it there —
/// so the fallback is unreachable and is written only because an index is not a proof.
fn control() -> &'static Measured { measured(DEFAULT_MODEL).unwrap_or(&MEASURED[0]) }

/// The catalogue as a terminal prints it, 78 columns at the widest so that an 80-column window
/// folds nothing: a column is dropped rather than wrapped, and the memory, the cold-ask and the
/// flip counts stay in the results document. `on` is the model a store is standing on, marked.
pub fn catalogue(on: Option<&str>) -> String {
    let row = |mark: &str, id: &str, dim: &str, para: &str, embed: &str, licence: &str| {
        format!("{mark} {id:<44}  {dim:>4}  {para:>5}  {embed:>5}  {licence}\n")
    };
    let mut s = row(" ", "hub id", "dim", "para", "embed", "licence");
    for m in MEASURED {
        let mark = if on.is_some_and(|o| o.eq_ignore_ascii_case(m.model)) { "→" } else { " " };
        let cost = format!("{:.1}×", m.times_the_default());
        s.push_str(&row(mark, m.model, &m.dim.to_string(), &format!("{}/{PARAPHRASE_CASES}", m.paraphrase), &cost, m.licence));
    }
    s.push_str(&format!("\npara is the paraphrase arm of the 82-case suite; embed is the whole store\n\
        against the default's {} s on the bench fixture. One run each, so anything\n\
        within four hits of the control is noise and the tie at the top is not a\n\
        ranking (docs/bench/2026-09-22-embedders-results.md).\n\n\
        {RECOMMENDED} is what the readings point at: the\n\
        recall of the two 568M models for a third of their embed, and the least memory\n\
        of any of them. Its licence is Gemma's rather than an OSI one, which is the one\n\
        reason to refuse it; the MIT row at the same recall is BAAI/bge-m3, at the cost\n\
        the table gives.\n", control().embed_s));
    s
}

/// A stored row's text as this model's card wants it worded.
fn reword<'a>(text: &'a str, p: &Profile) -> std::borrow::Cow<'a, str> {
    use std::borrow::Cow;
    let swap = |tag: &str, with: &str| (tag != with).then(|| text.strip_prefix(tag).map(|rest| format!("{with}{rest}"))).flatten();
    swap(QUERY_TAG, p.query).or_else(|| swap(PASSAGE_TAG, p.passage)).map_or(Cow::Borrowed(text), Cow::Owned)
}

struct Files { model: PathBuf, tokenizer: PathBuf, pad_token: String, pad_id: u32 }

/// A graph-only `model.onnx` keeps its weights in `model.onnx_data` beside it and is a few MB;
/// the small model's 448 MB file holds them itself. Asking the hub for a data file a model never
/// had is a network round trip on every fused query — 240 ms measured, and a failure offline.
/// Only a margin, and Qwen3 is the counterexample: 307 MB of graph with 2 GB of weights beside
/// it, well over the threshold. A model that size names its data file in `beside` instead.
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
    let profile = profile(&name);
    let model = get(profile.onnx)?;
    // The larger models keep their weights beside the graph; the session resolves the file by
    // its relative name, so it has to be fetched into the same snapshot. External data is legal
    // at any graph size and the model is the reader's to choose, so 64 MB is a margin rather
    // than a proof: a file that small cannot be holding 448 MB of weights itself, and the small
    // model's does, so it clears the margin by a factor of seven and never pays the lookup.
    let len = std::fs::metadata(&model).map(|m| m.len()).unwrap_or(0);
    if keeps_weights_beside(len) { let _ = repo.get(&format!("{}_data", profile.onnx)); }
    for f in profile.beside { get(f)?; }
    let tokenizer = get("tokenizer.json")?;
    let config: serde_json::Value = serde_json::from_slice(&std::fs::read(get("config.json")?)?)?;
    let tok_config: serde_json::Value = serde_json::from_slice(&std::fs::read(get("tokenizer_config.json")?)?)?;
    // Some tokenizers write the pad token as an added-token object rather than a bare string.
    let pad = &tok_config["pad_token"];
    let pad_token = pad.as_str().or_else(|| pad["content"].as_str()).context("tokenizer_config.json: pad_token")?.to_string();
    let pad_id = config["pad_token_id"].as_u64().unwrap_or(0) as u32;
    Ok(Files { model, tokenizer, pad_token, pad_id })
}

fn load_tokenizer(files: &Files) -> Result<Tokenizer> {
    let mut tk = Tokenizer::from_file(&files.tokenizer).map_err(|e| anyhow!("{e}"))?;
    tk.with_truncation(Some(TruncationParams { max_length: MAX_TOKENS, ..Default::default() })).map_err(|e| anyhow!("{e}"))?;
    // Qwen3's config.json carries no pad_token_id at all, and its pad token is a real entry in
    // the vocabulary — ask there first so the two never name different tokens.
    let pad_id = tk.token_to_id(&files.pad_token).unwrap_or(files.pad_id);
    tk.with_padding(Some(PaddingParams {
        strategy: PaddingStrategy::BatchLongest,
        pad_id,
        pad_token: files.pad_token.clone(),
        ..Default::default()
    }));
    Ok(tk)
}

/// How much of the machine a run may take. A word rather than a count, because the number that
/// matters is a fraction of whatever box this is and a count written for one box is wrong on the
/// next; the three names are amounts of the machine, which is what the key is called.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Resources {
    Low,
    #[default]
    Balanced,
    Full,
}

/// The configured level unless `REPOGRAPH_RESOURCES` names another, in the shape the other run
/// variables use: unset or empty keeps what the files said.
pub fn resources_from_env(configured: Resources, var: Option<&str>) -> Result<Resources> {
    let Some(raw) = var else { return Ok(configured) };
    let word = raw.trim();
    if word.is_empty() {
        return Ok(configured);
    }
    match word.to_ascii_lowercase().as_str() {
        "low" => Ok(Resources::Low),
        "balanced" => Ok(Resources::Balanced),
        "full" => Ok(Resources::Full),
        other => bail!("REPOGRAPH_RESOURCES is {other:?}, which is none of \"low\", \"balanced\" or \"full\""),
    }
}

/// How many threads a model session and the tokenizer pool may take: the level's rule over the
/// cores this machine reports.
pub fn threads(level: Resources) -> usize {
    threads_from(level, std::thread::available_parallelism().map_or(0, |n| n.get()))
}

/// Halves, sixths and thirds of the logical cores, with one thread as the floor. `balanced` is a
/// third rather than half because ORT sizes its own intra-op pool to the performance cores alone,
/// which on a 6+6 machine is already half — half here would be today's pool under another name
/// and give nothing back. `low` is half of `balanced` again, for a laptop someone is working on.
///
/// `full` is a half and not "no cap at all", which is the other thing it could have meant and was
/// measured against: leaving both pools to size themselves gives ORT its six performance cores
/// and rayon all twelve, 18 threads with 6 running, and that loses to capping rayon at the same
/// six — 178.1 s against 168.8 s on a whole-store embed of the fixture, for the same 814 user
/// seconds either way (docs/bench/2026-09-09-normal-band-only-results.md). Twelve tokenizer
/// threads contending for six cores cost more than they add, so `full` takes the shape that is
/// both faster and cheaper in threads.
///
/// On four logical cores or fewer `balanced` and `low` meet at one thread and only `full` still
/// names a different amount; on two, all three do.
fn threads_from(level: Resources, cores: usize) -> usize {
    match level {
        Resources::Full => (cores / 2).max(1),
        Resources::Balanced => (cores / 3).max(1),
        Resources::Low => (cores / 6).max(1),
    }
}

/// Where the GEMMs read their weights from. `Packed` is the runtime's default: at session open
/// MLAS makes its own copy of every GEMM weight in the layout its kernels want, which is 1.13 GB
/// of anonymous memory for the 24 layers of the large model and is what memory pressure counts.
/// `Mapped` keeps the weights where they already are — the memory-mapped `model.onnx_data`, whose
/// pages are clean, reclaimable and shared between processes: the anonymous footprint of a full
/// embed falls from 1.74 GB to 0.61 GB for 12% more wall
/// (docs/bench/2026-09-07-unnoticeable-results.md). A writer takes that trade; a reader keeps
/// `Packed`, because it runs a handful of GEMMs for one query and would pay the layout on every
/// one of them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Weights {
    Packed,
    Mapped,
}

/// Every ONNX session this binary opens, the embedder's and the reranker's alike. Left to
/// itself ORT sizes its intra-op pool to the machine's performance cores and holds them for the
/// whole run — 444% of a core for the 43 minutes a full re-embed took, measured in
/// docs/bench/2026-09-07-resource-usage-results.md. Nothing else is set: spin control off and
/// memory-pattern off each cost more wall time than they gave back.
pub(crate) fn session_builder(threads: usize, weights: Weights) -> Result<SessionBuilder> {
    let builder = Session::builder().map_err(|e| anyhow!("{e}"))?
        .with_optimization_level(GraphOptimizationLevel::Level1).map_err(|e| anyhow!("{e}"))?
        .with_intra_threads(threads).map_err(|e| anyhow!("{e}"))?;
    match weights {
        Weights::Packed => Ok(builder),
        Weights::Mapped => builder.with_prepacking(false).map_err(|e| anyhow!("{e}")),
    }
}

fn load_session(model: &Path, threads: usize, weights: Weights) -> Result<Session> {
    let mut builder = session_builder(threads, weights)?;
    builder.commit_from_file(model).map_err(|e| anyhow!("{e}"))
}

fn normalise(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 { for x in v { *x /= norm; } }
}

impl Embedder {
    pub fn open(model: &str, threads: usize, weights: Weights) -> Result<Embedder> {
        let files = fetch(model)?;
        let session = std::thread::scope(|s| {
            let tokenizer = s.spawn(|| load_tokenizer(&files));
            let session = load_session(&files.model, threads, weights).context("open embedding model")?;
            let tokenizer = tokenizer.join().map_err(|_| anyhow!("tokenizer thread panicked"))?.context("open tokenizer")?;
            Ok::<_, anyhow::Error>((session, tokenizer))
        });
        let (session, tokenizer) = session?;
        let wants = |input: &str| session.inputs().iter().any(|i| i.name() == input);
        let cache_inputs = session.inputs().iter()
            .filter(|i| i.name().starts_with("past_key_values"))
            .map(|i| match i.dtype() {
                ort::value::ValueType::Tensor { ty: ort::value::TensorElementType::Float32, shape, .. }
                    if shape.len() == 4 && shape[1] > 0 && shape[3] > 0 =>
                    Ok((i.name().to_string(), shape[1] as usize, shape[3] as usize)),
                other => Err(anyhow!("{model}: cache input {} is {other:?}, not an fp32 [batch, heads, past, width]", i.name())),
            })
            .collect::<Result<Vec<_>>>()?;
        let (wants_type_ids, wants_position_ids) = (wants("token_type_ids"), wants("position_ids"));
        Ok(Embedder { session, tokenizer, wants_type_ids, wants_position_ids, cache_inputs, profile: profile(model), name: model.to_string(), dim: None })
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
    /// questions as `query: `. A batch is padded to its longest member, so what a forward costs
    /// is its count times that longest — and both are bounded here, by `BATCH` and by
    /// `TOKEN_BUDGET`. The lengths are the tokenizer's own, counted in a pass of its own: bytes
    /// put a 300-token Cyrillic passage in the same batch as a 60-token Latin one and pad both
    /// to the longer, which is the padding this budget exists to stop paying for.
    pub fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let texts: Vec<std::borrow::Cow<str>> = texts.iter().map(|t| reword(t, &self.profile)).collect();
        let lens = self.token_lengths(&texts)?;
        let mut out: Vec<Vec<f32>> = vec![Vec::new(); texts.len()];
        for chunk in token_batches(&lens, BATCH, TOKEN_BUDGET) {
            let batch: Vec<&str> = chunk.iter().map(|&i| texts[i].as_ref()).collect();
            for (i, v) in chunk.into_iter().zip(self.forward(&batch)?) {
                out[i] = v;
            }
        }
        Ok(out)
    }

    pub fn query(&mut self, text: &str) -> Result<Vec<f32>> {
        let text = format!("{}{text}", self.profile.query);
        Ok(self.forward(&[&text])?.remove(0))
    }

    /// How many real tokens each text is, as the model will see it. The tokenizer pads a batch
    /// to its longest member, so the count is the attention mask's, not the encoding's length.
    fn token_lengths(&self, texts: &[impl AsRef<str>]) -> Result<Vec<usize>> {
        let mut lens = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(LENGTH_CHUNK) {
            let encodings = self.tokenizer
                .encode_batch(chunk.iter().map(|t| t.as_ref()).collect(), true)
                .map_err(|e| anyhow!("{e}"))?;
            lens.extend(encodings.iter().map(|e| e.get_attention_mask().iter().filter(|&&m| m == 1).count()));
        }
        Ok(lens)
    }

    fn forward(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let encodings = self.tokenizer
            .encode_batch(texts.to_vec(), true)
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
        if self.wants_position_ids {
            let positions: Vec<i64> = (0..batch).flat_map(|_| 0..len as i64).collect();
            inputs.push(("position_ids".into(), Tensor::from_array(([batch, len], positions))?.into()));
        }
        for (name, heads, width) in &self.cache_inputs {
            inputs.push((name.clone().into(), Tensor::from_array(([batch, *heads, 0, *width], Vec::<f32>::new()))?.into()));
        }
        let outputs = self.session.run(inputs)?;
        // A graph that pools for itself also carries whatever comes after the pooling — a dense
        // head, in one case — which no pooling of the hidden states done here would reproduce.
        if let Some(pooled) = outputs.get("sentence_embedding") {
            let (shape, data) = pooled.try_extract_tensor::<f32>()?;
            let dim = *shape.get(1).context("sentence_embedding is not [batch, dim]")? as usize;
            // `chunks(0)` panics, and a graph whose second dimension is zero or negative is the
            // one case that reaches it: an export this binary guessed wrong about, which is owed
            // the hub's own id in a message rather than a backtrace.
            anyhow::ensure!(dim > 0, "sentence_embedding is {dim}-d, so the model returned no vector");
            return Ok(data.chunks(dim).map(|row| { let mut v = row.to_vec(); normalise(&mut v); v }).collect());
        }
        let hidden = outputs.get("last_hidden_state")
            .or_else(|| (outputs.len() == 1).then(|| &outputs[0]))
            .context("model has no last_hidden_state output")?;
        let (shape, data) = hidden.try_extract_tensor::<f32>()?;
        let dim = *shape.get(2).context("last_hidden_state is not [batch, tokens, dim]")? as usize;
        Ok(pool(data, &mask, len, dim, self.profile.pooling))
    }
}

/// Indices of the texts whose token counts are `lens`, grouped into forwards of at most
/// `max_batch` texts and, once padded to the batch's longest member, at most `budget` tokens.
/// Ascending by length, so a batch pads to a neighbour rather than to the corpus maximum. A text
/// longer than the budget on its own is still a forward of one — refusing it would leave a
/// passage unembedded — and `budget == 0` is the count-only rule this replaced.
fn token_batches(lens: &[usize], max_batch: usize, budget: usize) -> Vec<Vec<usize>> {
    let mut order: Vec<usize> = (0..lens.len()).collect();
    order.sort_by_key(|&i| lens[i]);
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut batch: Vec<usize> = Vec::new();
    for i in order {
        // Ascending order makes `lens[i]` the batch's longest member the moment it joins.
        let over_budget = budget > 0 && lens[i] * (batch.len() + 1) > budget;
        if !batch.is_empty() && (batch.len() >= max_batch || over_budget) {
            out.push(std::mem::take(&mut batch));
        }
        batch.push(i);
    }
    if !batch.is_empty() { out.push(batch); }
    out
}

/// One L2-normalised vector per text. Padding is on the right, so the first position is always
/// a real token and the last real one is wherever the attention mask ends. A row with no real
/// token at all is the zero vector under every pooling: reading a padded position instead would
/// hand back a unit vector of whatever the graph computed for padding, which matches queries.
fn pool(hidden: &[f32], mask: &[i64], len: usize, dim: usize, how: Pooling) -> Vec<Vec<f32>> {
    let batch = mask.len().checked_div(len).unwrap_or(0);
    (0..batch).map(|b| {
        let row = &mask[b * len..][..len];
        let at = |t: usize| &hidden[(b * len + t) * dim..][..dim];
        let last = row.iter().rposition(|&m| m != 0);
        let mut v = match (how, last) {
            (_, None) => vec![0f32; dim],
            (Pooling::Cls, _) => at(0).to_vec(),
            (Pooling::LastToken, Some(t)) => at(t).to_vec(),
            (Pooling::Mean, _) => {
                let mut v = vec![0f32; dim];
                let mut n = 0f32;
                for t in (0..len).filter(|&t| row[t] != 0) {
                    n += 1.0;
                    for (x, y) in v.iter_mut().zip(at(t)) { *x += y; }
                }
                for x in &mut v { *x /= n.max(1.0); }
                v
            }
        };
        normalise(&mut v);
        v
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cls_takes_the_first_position_and_last_token_the_last_attended_one() {
        // One text of three positions, the third padded.
        let hidden = [1.0, 0.0,  0.0, 2.0,  9.0, 9.0];
        let mask = [1, 1, 0];
        assert_eq!(pool(&hidden, &mask, 3, 2, Pooling::Cls), vec![vec![1.0, 0.0]]);
        assert_eq!(pool(&hidden, &mask, 3, 2, Pooling::LastToken), vec![vec![0.0, 1.0]]);
    }

    #[test]
    fn a_row_keeps_its_e5_tag_in_the_store_and_loses_it_on_the_way_to_another_model() {
        let arctic = profile("Snowflake/snowflake-arctic-embed-m-v2.0");
        assert_eq!(reword("passage: FR-PAY-03\nтело", &arctic), "FR-PAY-03\nтело");
        assert_eq!(reword("query: где список", &arctic), "query: где список");
        let gemma = profile("onnx-community/embeddinggemma-300m-ONNX");
        assert_eq!(reword("query: где список", &gemma), "task: search result | query: где список");
        assert_eq!(reword("passage: a", &gemma), "title: none | text: a");
        assert_eq!(reword("passage: a", &E5), "passage: a");
    }

    /// A catalogue entry is an invitation to download two gigabytes, so it may not name a model
    /// whose recipe this binary does not have: a fall-through to e5's pooling and prefixes is a
    /// silent loss of recall, which is precisely what the table claims to have measured. The e5
    /// models are the exception, being what that profile is.
    #[test]
    fn every_measured_model_resolves_to_a_profile_and_the_default_is_among_them() {
        for m in MEASURED {
            let is_e5 = m.model.to_ascii_lowercase().contains("multilingual-e5");
            assert!(!reads_as_e5(m.model) || is_e5, "{} falls through to the e5 profile", m.model);
        }
        assert!(measured(DEFAULT_MODEL).is_some(), "the control is the row every other is quoted against");
        assert!(measured(RECOMMENDED).is_some(), "the recommendation is one of the measured rows");
        assert_eq!(control().embed_s, 144);
    }

    /// The table is printed into whatever terminal ran the command, and a folded row is unreadable
    /// in the one place it has to be read: where a person is choosing what to download.
    #[test]
    fn the_catalogue_fits_an_eighty_column_terminal_and_marks_the_row_it_was_given() {
        let text = catalogue(Some("BAAI/BGE-M3"));
        for line in text.lines() {
            assert!(line.chars().count() <= 80, "{} columns: {line}", line.chars().count());
        }
        let marked: Vec<&str> = text.lines().filter(|l| l.starts_with('→')).collect();
        assert_eq!(marked.len(), 1, "one row marked, whatever case the store recorded: {marked:?}");
        assert!(marked[0].contains("BAAI/bge-m3"), "{}", marked[0]);
        assert!(catalogue(None).lines().all(|l| !l.starts_with('→')));
        assert!(catalogue(Some("someone/unmeasured")).contains(DEFAULT_MODEL));
    }

    /// One table, three copies: this list, the README's and the readings'. A figure corrected in
    /// the code and left standing in a document contradicts the binary the reader is holding, and
    /// nothing else here would catch it — the documents are checked against the list instead of
    /// trusted to follow it.
    #[test]
    fn the_documents_quote_the_figures_this_list_holds() {
        const README: &str = include_str!("../../README.md");
        const READINGS: &str = include_str!("../../docs/bench/2026-09-22-embedders-results.md");
        let row = |doc: &'static str, model: &str| {
            // The paraphrase column is what picks the measurement table out of the documents'
            // other tables, which name these ids too.
            doc.lines()
                .find(|l| l.starts_with("| `") && l.contains(model) && l.contains(&format!("/{PARAPHRASE_CASES}")))
                .unwrap_or_else(|| panic!("no table row for {model}"))
        };
        for m in MEASURED {
            let readme = row(README, m.model);
            let readings = row(READINGS, m.model);
            for (doc, line) in [("README", readme), ("readings", readings)] {
                for want in [format!("| {} |", m.dim), format!("{}/{PARAPHRASE_CASES}", m.paraphrase), m.licence.to_string()] {
                    assert!(line.contains(&want), "{doc} row for {} does not say {want}: {line}", m.model);
                }
            }
            assert!(readme.contains(&format!("{:.1}×", m.times_the_default())), "README row for {}: {readme}", m.model);
            assert!(readme.contains(&format!("{} MB", m.vectors_mb)), "README row for {}: {readme}", m.model);
            for want in [format!("| {} |", m.embed_s), format!("| {} |", m.vectors_mb), format!("| {} |", m.cache)] {
                assert!(readings.contains(&want), "readings row for {} does not say {want}: {readings}", m.model);
            }
        }
    }

    /// The cost a switch quotes is a ratio, so the control's own row has to read as 1.
    #[test]
    fn the_embed_cost_is_quoted_against_the_default() {
        assert_eq!(measured(DEFAULT_MODEL).unwrap().times_the_default(), 1.0);
        let gemma = measured(RECOMMENDED).unwrap();
        assert!((gemma.times_the_default() - 5.09).abs() < 0.01, "{}", gemma.times_the_default());
    }

    #[test]
    fn an_unknown_model_is_read_as_e5() {
        assert_eq!(profile("someone/some-model"), E5);
        assert_eq!(profile("intfloat/multilingual-e5-base"), E5);
        assert_eq!(profile("Teradata/granite-embedding-107m-multilingual").onnx, "onnx/model-fp32.onnx");
    }

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
        let v = pool(&hidden, &mask, 2, 2, Pooling::Mean);
        let r = 0.5f32.sqrt();
        assert!((v[0][0] - r).abs() < 1e-6 && (v[0][1] - r).abs() < 1e-6);
        assert!((v[1][0] - 0.6).abs() < 1e-6 && (v[1][1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn token_batches_close_on_the_padded_token_budget_and_cover_every_text_once() {
        // 4 × 256 = 1,024 and 2 × 300 = 600 both exceed the budget; 3 × 20 = 60 does not.
        let batches = token_batches(&[10, 300, 20, 256, 5], 64, 512);
        assert_eq!(batches, vec![vec![4, 0, 2], vec![3], vec![1]]);
        assert!(token_batches(&[], 64, 512).is_empty());
    }

    #[test]
    fn token_batches_keep_a_text_longer_than_the_budget_as_a_batch_of_one() {
        assert_eq!(token_batches(&[10, 1000, 20], 64, 512), vec![vec![0, 2], vec![1]]);
    }

    #[test]
    fn token_batches_never_exceed_the_count_cap_however_small_the_texts() {
        let batches = token_batches(&vec![1; 200], 64, 100_000);
        assert_eq!(batches.iter().map(Vec::len).collect::<Vec<_>>(), vec![64, 64, 64, 8]);
    }

    #[test]
    fn token_batches_with_no_budget_are_the_old_count_batches() {
        assert_eq!(token_batches(&[4, 1, 2, 5, 3], 2, 0), vec![vec![1, 2], vec![4, 0], vec![3]]);
    }

    #[test]
    fn an_all_masked_row_is_a_zero_vector_not_a_nan() {
        for how in [Pooling::Mean, Pooling::Cls, Pooling::LastToken] {
            assert_eq!(pool(&[1.0, 1.0], &[0], 1, 2, how), vec![vec![0.0, 0.0]], "{how:?}");
        }
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
    fn token_batches_keep_original_order_among_equal_lengths() {
        // Every text is the same length: a stable sort must not reorder them.
        assert_eq!(token_batches(&[2, 2, 2, 2], 10, 512), vec![vec![0, 1, 2, 3]]);
    }

    #[test]
    fn token_batches_with_cap_larger_than_the_collection_yield_one_sorted_batch() {
        assert_eq!(token_batches(&[2, 1, 1], 100, 512), vec![vec![1, 2, 0]]);
    }

    #[test]
    fn each_level_takes_its_fraction_of_the_cores_and_never_reaches_zero() {
        assert_eq!(threads_from(Resources::Full, 12), 6);
        assert_eq!(threads_from(Resources::Balanced, 12), 4);
        assert_eq!(threads_from(Resources::Low, 12), 2);
        assert_eq!(threads_from(Resources::Balanced, 8), 2);
        assert_eq!(threads_from(Resources::Low, 8), 1);
        assert_eq!(threads_from(Resources::Full, 4), 2);
        assert_eq!(threads_from(Resources::Balanced, 4), 1, "the two named-down levels meet on a small box");
        assert_eq!(threads_from(Resources::Low, 4), 1);
        assert_eq!(threads_from(Resources::Full, 0), 1, "a machine that reports no cores still gets one thread");
        assert_eq!(threads_from(Resources::Balanced, 0), 1);
    }

    #[test]
    fn an_unknown_level_in_the_environment_is_an_error_naming_the_variable() {
        assert_eq!(resources_from_env(Resources::Balanced, None).unwrap(), Resources::Balanced);
        assert_eq!(resources_from_env(Resources::Balanced, Some("")).unwrap(), Resources::Balanced);
        assert_eq!(resources_from_env(Resources::Balanced, Some(" LOW ")).unwrap(), Resources::Low);
        let err = resources_from_env(Resources::Balanced, Some("half")).unwrap_err().to_string();
        assert!(err.contains("REPOGRAPH_RESOURCES"), "{err}");
    }

    #[test]
    fn pool_with_zero_length_rows_yields_no_vectors_not_a_panic() {
        assert!(pool(&[], &[], 0, 2, Pooling::Mean).is_empty());
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
