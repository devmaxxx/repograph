//! The work behind `repograph ask`, shaped so it can be done more than once against one open
//! store: `Context::open` reads the graph, the ids and the questions, and `Context::answer`
//! turns a `Request` into the text the command prints. A one-shot `ask` opens a context, answers
//! once and leaves; a resident process opens one and answers many times, over the same code.

use crate::{config, enrich, ids, index, model, query, rerank, store, walk};
use anyhow::Context as _;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

/// `REPOGRAPH_TIMING=1` prints where an `ask` spends its time, one line per stage on stderr.
pub(crate) struct Timing { on: bool, start: std::time::Instant, last: Cell<std::time::Instant> }

pub(crate) fn timing_on() -> bool { std::env::var_os("REPOGRAPH_TIMING").is_some() }

impl Timing {
    pub(crate) fn new() -> Timing {
        let now = std::time::Instant::now();
        Timing { on: timing_on(), start: now, last: Cell::new(now) }
    }

    pub(crate) fn stage(&self, what: &str) {
        if !self.on { return; }
        let now = std::time::Instant::now();
        eprintln!("timing: {:>7.1} ms  (+{:>6.1} ms)  {what}", (now - self.start).as_secs_f64() * 1e3, (now - self.last.get()).as_secs_f64() * 1e3);
        self.last.set(now);
    }
}

/// What a model open yields, named because it also crosses a thread boundary.
pub(crate) type Opened = Result<Option<index::embed::Embedder>, String>;

/// The embedder, or the line that says why there is none — returned rather than printed, so a
/// process whose stderr is a socket reply can hand that line to the client that asked. An arm
/// with no dense side is `Ok(None)`: nothing was wanted and nothing is missing.
pub(crate) fn embedder_or_notice(no_dense: bool, model: &str) -> Opened {
    if no_dense { return Ok(None); }
    index::embed::Embedder::open(model)
        .map(Some)
        .map_err(|err| format!("dense: model unavailable, continuing lexical-only ({err:#})"))
}

/// The same open for `watch` and `embed`, whose stderr is the reader's terminal.
pub(crate) fn open_embedder(no_dense: bool, model: &str) -> Option<index::embed::Embedder> {
    embedder_or_notice(no_dense, model).unwrap_or_else(|notice| { eprintln!("{notice}"); None })
}

/// The model opens on a thread this spawns, while `Context::answer` builds its BM25 indexes on
/// the caller's own thread — `Lexical::build` runs between this spawn and the `query::ask` call,
/// so the two overlap there rather than by anything ordered inside `ask` itself. Ids and the
/// questions store are read before the vectors that name the model, so they are not part of it.
/// A question that exact ids or symbols answer whole never opens the model, as before; with
/// `--no-dense` or no vectors nothing starts.
pub(crate) fn warm_model(dense: bool, whole: bool, model: &str) -> Option<std::thread::JoinHandle<Opened>> {
    if !dense || whole { return None; }
    let model = model.to_string();
    Some(std::thread::spawn(move || embedder_or_notice(false, &model)))
}

/// The stored graph brought in line with the working tree, plus what that cost when the tree had
/// moved. `ask` runs this before answering so an edit never has to be followed by an `update`;
/// the extractors are built only when there is something to re-read.
pub(crate) fn graph_for_ask(repo: &Path, cfg: &config::Config, store: &store::Store, stale: bool, timing: &Timing) -> anyhow::Result<(model::Graph, Option<crate::UpdateReport>)> {
    let (mut graph, manifest, source) = store.load_traced()?;
    timing.stage("graph loaded");
    if stale { return Ok((graph, None)); }
    let entries = walk::walk(repo, cfg, &manifest)?;
    let diff = manifest.diff(&entries);
    timing.stage("tree walked");
    if diff.changed.is_empty() && diff.removed.is_empty() {
        crate::record_stamps(store, &manifest, &entries)?;
        // A store another release or a bare `graph.json` left without a mirror pays the JSON
        // parse once; a refresh below writes the mirror on its own.
        if source == store::Source::Json {
            store.write_mirror("graph.json", &graph)?;
            timing.stage("mirror written");
        }
        return Ok((graph, None));
    }
    let r = crate::apply_diff(repo, store, &mut graph, &entries, &diff, &crate::extractors(repo, cfg)?)?;
    timing.stage("refreshed");
    Ok((graph, Some(r)))
}

/// One question and the flags it is asked under — everything `Cmd::Ask` carries that is not the
/// store itself, so a request can cross a socket without the context moving with it.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Request { pub words: Vec<String>, pub json: bool, pub seeds: usize, pub bodies: bool, pub rerank: bool, pub rerank_local: bool, pub depth: usize, pub stale: bool, pub no_dense: bool }

/// An open store ready to answer. Everything that costs more than a question to build — the
/// graph, the id matcher, the questions, the vectors, the embedding model, the cross-encoder —
/// is held here and kept between answers.
pub struct Context {
    cfg: config::Config,
    store: store::Store,
    graph: model::Graph,
    ids: ids::IdMatcher,
    questions: enrich::Questions,
    /// What `questions.json` looked like when the questions in hand were read, so a refresh
    /// that did not touch them does not pay to parse them again.
    questions_stamp: Option<walk::Stamp>,
    /// The BM25 indexes over `graph` and `questions`, built by the first answer that fuses and
    /// kept until either moves. Built there rather than in `open` so that on the first fused
    /// question the build still overlaps the model open, as it did when `query::ask` built it.
    lexical: RefCell<Option<index::lexical::Lexical>>,
    no_dense: bool,
    dense_idx: RefCell<Option<index::dense::DenseIndex>>,
    warm: RefCell<Option<std::thread::JoinHandle<Opened>>>,
    embedder: RefCell<Option<Option<index::embed::Embedder>>>,
    cross: RefCell<Option<index::cross::CrossEncoder>>,
    resync: Cell<bool>,
    notices: RefCell<Vec<String>>,
    timing: Timing,
}

impl Context {
    /// The store read the way `ask` reads it: refreshed against the tree unless `stale`, the
    /// stored graph with a warning when the store cannot be written.
    pub fn open(repo: &Path, cfg: &config::Config, stale: bool, no_dense: bool) -> anyhow::Result<Context> {
        let timing = Timing::new();
        let store = store::Store::new(repo);
        let mut notices = Vec::new();
        let (graph, refreshed) = match graph_for_ask(repo, cfg, &store, stale, &timing) {
            Ok(pair) => pair,
            // A store that cannot be written (read-only checkout, a walk that failed) still
            // holds an answer: say once that it may be behind, then give the stored one.
            Err(err) => { notices.push(format!("refresh: skipped ({err:#})")); (store.load()?.0, None) }
        };
        if let Some(r) = &refreshed { notices.push(format!("refresh: {} changed, {} removed", r.changed, r.removed)); }
        let ids = ids::IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
        timing.stage("ids ready");
        let (questions, source) = enrich::Questions::load_traced(&store)?;
        if !stale && source == store::Source::Json { questions.write_mirror(&store)?; }
        let questions_stamp = store.stamp(enrich::FILE);
        timing.stage("questions ready");
        Ok(Context {
            cfg: cfg.clone(), store, graph, ids, questions, questions_stamp, no_dense,
            lexical: RefCell::new(None),
            dense_idx: RefCell::new(None),
            warm: RefCell::new(None),
            embedder: RefCell::new(None),
            cross: RefCell::new(None),
            // The refresh above moved passages the vectors were built from, so the first fused
            // answer re-embeds the changed rows; an exact-id answer leaves them to the next one.
            resync: Cell::new(refreshed.is_some()),
            notices: RefCell::new(notices),
            timing,
        })
    }

    /// The arm this context was opened in. A resident process hands it to every client in the
    /// handshake: the `||` below cannot widen a lexical context back to a fused one, so a fused
    /// question has to be refused before it is asked rather than answered lexically.
    pub fn no_dense(&self) -> bool { self.no_dense }

    /// The text `ask` prints for this request — `render`'s output, byte for byte.
    pub fn answer(&mut self, req: &Request) -> anyhow::Result<String> {
        // A context opened without the dense arm has no vectors and no model to grow one from,
        // so it narrows a fused request and never the other way; `serve` refuses that pairing.
        let no_dense = self.no_dense || req.no_dense;
        let opts = query::Options { seeds: req.seeds, bodies: req.bodies, dense: !no_dense && index::dense::DenseIndex::present(&self.store), json: req.json, depth: req.depth };
        let Context { cfg, store, graph, questions, lexical, dense_idx, warm, embedder, cross, resync, notices, timing, .. } = &*self;
        // Opening the ONNX model costs ~220 ms and 1.3 GB, the vectors 50 MB; an exact id or
        // symbol match never asks for either, so on that path both still open lazily, on the
        // first fused query that never comes. A fused question starts both below, once the
        // last fallible step is behind them.
        let dense_fn = |q: &str, k: usize| -> (Vec<String>, Vec<String>) {
            // The vectors come first because they name the model: reading `vectors.json`
            // again through `recorded_model` would parse 3 MB a second time for what the
            // loaded index already holds.
            let mut idx = dense_idx.borrow_mut();
            let idx = idx.get_or_insert_with(|| {
                let i = index::dense::DenseIndex::load(store).unwrap_or_else(|err| { notices.borrow_mut().push(format!("dense: index unreadable, continuing lexical-only ({err:#})")); Default::default() });
                timing.stage("vectors loaded"); i
            });
            let mut slot = embedder.borrow_mut();
            let e = slot.get_or_insert_with(|| {
                // Collected, never printed: on the warm thread this line used to race the main
                // thread's own stderr, and over a socket it would land on the server's terminal
                // instead of reaching the client whose answer went lexical-only because of it.
                let opened = match warm.borrow_mut().take() {
                    Some(handle) => handle.join().unwrap_or_else(|_| Err("dense: model thread panicked, continuing lexical-only".to_string())),
                    None => {
                        let model = index::embed::resolve(idx.model_of_rows().as_deref(), &cfg.embed_model);
                        embedder_or_notice(no_dense, &model)
                    }
                };
                timing.stage("model opened");
                opened.unwrap_or_else(|notice| { notices.borrow_mut().push(notice); None })
            });
            let qvec = e.as_mut().and_then(|e| e.query(q).ok());
            // Decided before a single row is written: the resync below embeds and saves, so a
            // store whose rows are not this model's width has to be refused here — after it,
            // the notice would be an epitaph for the index the resync had already replaced.
            if let (Some(v), Some(emb)) = (&qvec, e.as_ref()) {
                if idx.dim > 0 && v.len() != idx.dim {
                    notices.borrow_mut().push(format!("dense: {}; continuing lexical-only", index::embed::width_mismatch(idx.dim, emb.name(), v.len())));
                    return (Vec::new(), Vec::new());
                }
            }
            // `--stale` asks for the store as it is and pays for no walk; embedding rows and
            // saving them is the most expensive thing this code does, and a one-shot `--stale`
            // never reaches it — the flag stays set for the next answer that did ask.
            if !req.stale && resync.replace(false) {
                if let Some(emb) = e.as_mut() {
                    // A reader appends to the store's own rows and never re-embeds them into
                    // another model's index: it claims the index for the model it opened, at
                    // the width this very query just measured.
                    let width = qvec.as_ref().map_or(idx.dim, |v| v.len());
                    idx.written_by(emb.name(), width);
                    match idx.sync(graph, questions, &mut |texts| emb.embed(texts)) {
                        Ok(0) => {}
                        Ok(n) => {
                            if let Err(err) = idx.save(store) { notices.borrow_mut().push(format!("refresh: vectors not saved ({err:#})")); }
                            notices.borrow_mut().push(format!("refresh: {n} vectors embedded"));
                        }
                        Err(err) => notices.borrow_mut().push(format!("refresh: vectors unchanged ({err:#})")),
                    }
                    timing.stage("vectors synced");
                }
            }
            let out = match qvec { Some(v) => idx.search(&v, k), None => (Vec::new(), Vec::new()) };
            timing.stage("query embedded and searched");
            out
        };
        // Collected like the local reranker's failure beside it, so `--rerank` and
        // `--rerank-local` tell a client the same thing when their model will not answer.
        let rerank_fn = |q: &str, c: &[(String, String)]| {
            let (picked, notice) = rerank::run_or_notice(&cfg.rerank_command, q, c);
            if let Some(n) = notice { notices.borrow_mut().push(n); }
            picked
        };
        if req.rerank_local && cross.borrow().is_none() {
            let dir = if cfg.reranker_dir.is_empty() { index::cross::default_dir()? } else { PathBuf::from(&cfg.reranker_dir) };
            *cross.borrow_mut() = Some(index::cross::CrossEncoder::open(&dir).context("--rerank-local")?);
        }
        // Below the last `?`: an error returned between the spawn and the join drops the
        // handle and leaves a thread mid-open of a 448 MB session. Nothing below this point
        // can fail, and the only work above it the open might have overlapped is
        // `--rerank-local`'s own session.
        if opts.dense {
            // The predicate `ask` fuses on, asked early so the model can start beside the
            // BM25 builds. `exact_seeds` is pure, so asking it twice costs one graph scan and
            // decides nothing differently — and only the dense arm can use the answer, so on
            // `--no-dense` the scan never runs.
            let (_, whole) = query::exact_seeds(graph, &self.ids, &req.words);
            if !whole {
                let mut slot = dense_idx.borrow_mut();
                let idx = slot.get_or_insert_with(|| {
                    let i = index::dense::DenseIndex::load(store).unwrap_or_else(|err| { notices.borrow_mut().push(format!("dense: index unreadable, continuing lexical-only ({err:#})")); Default::default() });
                    timing.stage("vectors loaded"); i
                });
                // The rows name the model, so nothing can start before they are read. A context
                // that has already opened the model keeps it rather than starting a second one.
                let mut handle = warm.borrow_mut();
                if handle.is_none() && embedder.borrow().is_none() {
                    let model = index::embed::resolve(idx.model_of_rows().as_deref(), &cfg.embed_model);
                    *handle = warm_model(true, whole, &model);
                }
            }
        }
        let local_fn = |q: &str, c: &[(String, String)]| -> Vec<String> {
            let mut m = cross.borrow_mut();
            let Some(m) = m.as_mut() else { return Vec::new() };
            let texts: Vec<String> = c.iter().map(|(_, t)| t.clone()).collect();
            match m.score(q, &texts) {
                Ok(s) => index::cross::pick(&s, c, index::cross::PICK),
                // Like a failing rerank command: say so and answer from the fused order.
                Err(e) => { notices.borrow_mut().push(format!("rerank-local: {e:#}; answering from the fused order")); Vec::new() }
            }
        };
        let rerank: Option<query::Rerank> = if req.rerank_local { Some(&local_fn) } else if req.rerank { Some(&rerank_fn) } else { None };
        // Only a reranked request ever reads the code list (`lexical_lists`), so a plain first
        // answer skips building it; a context that later takes a `--rerank` request adds it
        // without discarding what already answered the plain ones fine.
        let code_seat = rerank.is_some();
        let mut guard = lexical.borrow_mut();
        let lex = guard.get_or_insert_with(|| { let l = index::lexical::Lexical::build(graph, questions, code_seat); timing.stage("lexical built"); l });
        if code_seat { lex.ensure_code(graph, questions); }
        let answer = query::ask(graph, &self.ids, lex, Some(&dense_fn), rerank, &req.words, &opts);
        timing.stage("answered");
        Ok(query::render(&answer, graph, &opts))
    }

    /// The watcher's graph taken over after a poll moved it, which is how a resident context
    /// reaches the state a one-shot `ask` would have loaded from disk — without reading ten
    /// megabytes back to learn what the poll already holds. `refreshed` is the poll's own
    /// report when it applied a change and `None` when it only read a store someone else wrote.
    pub(crate) fn adopt(&mut self, w: &crate::Watcher, refreshed: Option<crate::UpdateReport>) -> anyhow::Result<()> {
        self.graph = w.graph.clone();
        // An adopted graph is a new population whether or not the questions moved with it, so
        // the indexes built over the old one cannot outlive this call.
        *self.lexical.borrow_mut() = None;
        let stamp = self.store.stamp(enrich::FILE);
        if stamp != self.questions_stamp {
            self.questions = enrich::Questions::load(&self.store)?;
            self.questions_stamp = stamp;
        }
        // The vectors another process rewrote, dropped so the next fused answer loads them —
        // reading what is on disk is what a one-shot does, and it is not the same as embedding
        // the rows again here, which would write a store nobody asked this process to write.
        let moved = { let idx = self.dense_idx.borrow(); idx.as_ref().is_some_and(|i| i.read_at() != self.store.stamp("vectors.f32")) };
        if moved { *self.dense_idx.borrow_mut() = None; }
        // A refresh this process applied leaves the store's vectors behind its graph by exactly
        // the rows that moved, so the next fused answer re-embeds them — the catch-up a one-shot
        // `ask` does after a refresh of its own. Someone else's store, read back whole, is the
        // other case and needs none of that: a one-shot loading it now would embed nothing
        // either, and the line above has already taken their vectors along with their graph.
        if let Some(r) = refreshed {
            self.notices.borrow_mut().push(format!("refresh: {} changed, {} removed", r.changed, r.removed));
            self.resync.set(true);
        }
        Ok(())
    }

    /// Notices `ask` used to print on stderr for this answer (refresh lines, dense fallbacks), drained.
    pub fn notices(&mut self) -> Vec<String> { std::mem::take(&mut self.notices.borrow_mut()) }

    pub(crate) fn timing(&self) -> &Timing { &self.timing }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_is_not_warmed_for_an_exact_answer_or_a_lexical_arm() {
        assert!(warm_model(false, false, "any").is_none());
        assert!(warm_model(true, true, "any").is_none());
    }

    // The `**ID · MUST · label**` form is what declares a requirement; an id in a heading
    // is only a reference to one, and leaves the graph with nothing to answer from.
    fn repo_with_two_docs() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("docs/pay.md"), "# Оплата\n\n**FR-PAY-1 · MUST · Штраф за отмену**\n\nШтраф списывается сам.\n").unwrap();
        std::fs::write(dir.path().join("docs/cal.md"), "# Календарь\n\n**FR-CAL-1 · MUST · Перенос визита**\n\nПеренос не считается отменой.\n").unwrap();
        std::fs::write(dir.path().join("repograph.toml"), "id_families = [\"FR-PAY\", \"FR-CAL\"]\n").unwrap();
        dir
    }

    #[test]
    fn a_context_answers_the_same_text_twice_and_for_an_exact_id() {
        let dir = repo_with_two_docs();
        let cfg = crate::config::Config::load(dir.path()).unwrap();
        let ex = crate::extractors(dir.path(), &cfg).unwrap();
        crate::run_update(dir.path(), &cfg, &ex, true).unwrap();
        let mut ctx = Context::open(dir.path(), &cfg, true, true).unwrap();
        let req = Request { words: vec!["штраф".into()], json: false, seeds: 5, bodies: false, rerank: false, rerank_local: false, depth: crate::rerank::DEPTH, stale: true, no_dense: true };
        let first = ctx.answer(&req).unwrap();
        let second = ctx.answer(&req).unwrap();
        assert_eq!(first, second);
        assert!(first.contains("FR-PAY-1"), "{first}");
        let exact = ctx.answer(&Request { words: vec!["FR-CAL-1".into()], ..req }).unwrap();
        assert!(exact.starts_with("FR-CAL-1"), "{exact}");
    }

    /// The dense arm as a machine without the model has it, prepared once for the whole binary:
    /// a cache in hf-hub's layout whose model file is 64 MB of nothing. The fetch is answered
    /// from disk — at that size the weights are taken to be inside the file, so nothing is looked
    /// up over the network — and the session then refuses to open it. Without this the two tests
    /// below would download 470 MB, or open the model already on the machine and embed with it.
    /// It is laid out under the name `resolve` returns rather than under the default one, so
    /// `REPOGRAPH_EMBED_MODEL` in the environment moves the fixture with it instead of missing
    /// it and sending `repo.get` to the network.
    fn an_unopenable_model() {
        static CACHE: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
        CACHE.get_or_init(|| {
            let dir = tempfile::tempdir().unwrap();
            let model = index::embed::resolve(None, index::embed::DEFAULT_MODEL);
            let root = dir.path().join(format!("models--{model}").replace('/', "--"));
            let snap = root.join("snapshots/abc");
            std::fs::create_dir_all(root.join("refs")).unwrap();
            std::fs::create_dir_all(snap.join("onnx")).unwrap();
            std::fs::write(root.join("refs/main"), "abc").unwrap();
            std::fs::File::create(snap.join("onnx/model.onnx")).unwrap().set_len(64 << 20).unwrap();
            std::fs::write(snap.join("tokenizer.json"), "{}").unwrap();
            std::fs::write(snap.join("config.json"), r#"{"pad_token_id": 1}"#).unwrap();
            std::fs::write(snap.join("tokenizer_config.json"), r#"{"pad_token": "<pad>"}"#).unwrap();
            // Process-wide, which is why it is set once behind the lock: no other test in this
            // binary opens an embedder — the rest are `--no-dense`, and the fetch tests are
            // handed their cache by argument. Set through the crate rather than through the
            // environment, which no test may write while the rest of the binary reads it.
            index::embed::set_cache_dir(dir.path().to_path_buf());
            dir
        });
    }

    /// A store with vectors, so `answer` takes the dense arm at all. The rows are invented:
    /// nothing here searches them, because the model will not open.
    ///
    /// The rows name their model rather than leaving it blank, and they name the same constant
    /// `an_unopenable_model` lays its cache out under. A blank one resolves through
    /// `UNNAMED_MODEL` instead, which is deliberately *not* the default — so a blank field would
    /// send this test looking for a model the fixture never cached, and out to the network to
    /// find it.
    fn vectors_beside_the_graph(repo: &Path) -> (PathBuf, PathBuf) {
        let (json, raw) = (repo.join(".repograph/vectors.json"), repo.join(".repograph/vectors.f32"));
        let model = index::embed::DEFAULT_MODEL;
        std::fs::write(&json, format!(r#"{{"ids":["FR-PAY-1"],"hashes":["h"],"kinds":[false],"dim":4,"model":"{model}"}}"#)).unwrap();
        std::fs::write(&raw, [0u8; 16]).unwrap();
        (json, raw)
    }

    fn fused(stale: bool) -> Request {
        Request { words: vec!["штраф".into()], json: false, seeds: 5, bodies: false, rerank: false, rerank_local: false, depth: crate::rerank::DEPTH, stale, no_dense: false }
    }

    #[test]
    fn a_stale_answer_leaves_the_resync_standing_for_the_answer_that_asked_for_a_refresh() {
        let dir = repo_with_two_docs();
        let cfg = crate::config::Config::load(dir.path()).unwrap();
        let ex = crate::extractors(dir.path(), &cfg).unwrap();
        crate::run_update(dir.path(), &cfg, &ex, true).unwrap();
        vectors_beside_the_graph(dir.path());
        an_unopenable_model();
        // A change on disk, so the open below refreshes and leaves rows for a fused answer.
        std::fs::write(dir.path().join("docs/new.md"), "**FR-PAY-2 · MUST · Возврат аванса**\n\nАванс возвращается при отмене салоном.\n").unwrap();
        let mut ctx = Context::open(dir.path(), &cfg, false, false).unwrap();
        assert!(ctx.resync.get(), "the refresh left rows the next fused answer has to embed");
        ctx.answer(&fused(true)).unwrap();
        assert!(ctx.resync.get(), "a --stale answer embeds nothing and leaves the flag where it was");
        ctx.answer(&fused(false)).unwrap();
        assert!(!ctx.resync.get(), "the next answer that did ask for a refresh takes it");
    }

    #[test]
    fn vectors_another_process_rewrote_are_dropped_rather_than_kept() {
        let dir = repo_with_two_docs();
        let cfg = crate::config::Config::load(dir.path()).unwrap();
        let ex = crate::extractors(dir.path(), &cfg).unwrap();
        crate::run_update(dir.path(), &cfg, &ex, true).unwrap();
        let (_, raw) = vectors_beside_the_graph(dir.path());
        an_unopenable_model();
        let mut ctx = Context::open(dir.path(), &cfg, true, false).unwrap();
        ctx.answer(&fused(true)).unwrap();
        assert!(ctx.dense_idx.borrow().is_some(), "the fused answer loaded the vectors");
        let w = crate::Watcher::open(dir.path(), &cfg).unwrap();
        ctx.adopt(&w, None).unwrap();
        assert!(ctx.dense_idx.borrow().is_some(), "vectors nobody rewrote are the ones already in hand");
        // What an `embed` in another process leaves behind: the same file, different rows.
        std::fs::write(&raw, [0u8; 32]).unwrap();
        ctx.adopt(&w, None).unwrap();
        assert!(ctx.dense_idx.borrow().is_none(), "the next fused answer reads them off disk instead");
    }

    // `serve`'s poll only ever calls `adopt` when it already decided something moved (a refresh
    // or a reload), so `adopt` itself invalidates unconditionally rather than diffing — this
    // pins that an answer built after one `adopt` is not the pair a stale answer would have
    // fused from.
    #[test]
    fn lexical_indexes_built_by_an_answer_are_dropped_once_the_context_adopts_a_watcher() {
        let dir = repo_with_two_docs();
        let cfg = crate::config::Config::load(dir.path()).unwrap();
        let ex = crate::extractors(dir.path(), &cfg).unwrap();
        crate::run_update(dir.path(), &cfg, &ex, true).unwrap();
        let mut ctx = Context::open(dir.path(), &cfg, true, true).unwrap();
        ctx.answer(&fused(true)).unwrap();
        assert!(ctx.lexical.borrow().is_some(), "the first fused answer built and kept the indexes");
        let w = crate::Watcher::open(dir.path(), &cfg).unwrap();
        ctx.adopt(&w, None).unwrap();
        assert!(ctx.lexical.borrow().is_none(), "an adopted graph invalidates the held indexes even when the questions did not move with it");
        ctx.answer(&fused(true)).unwrap();
        assert!(ctx.lexical.borrow().is_some(), "the next fused answer rebuilds them");
    }
}
