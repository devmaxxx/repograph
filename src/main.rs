mod bench;
mod changes;
mod code;
mod config;
mod doc;
mod dump;
mod enrich;
mod rerank;
mod ids;
mod impact;
mod index;
mod legacy;
mod model;
mod query;
mod store;
mod walk;

use anyhow::Context;
use clap::{Parser, Subcommand};
use model::Extractor;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "repograph", version)]
struct Cli {
    /// Repository root; defaults to the current directory.
    #[arg(long, global = true, default_value = ".")]
    repo: PathBuf,
    #[arg(long, global = true)]
    no_dense: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Build,
    Update,
    Ask {
        #[arg(required = true)] words: Vec<String>,
        #[arg(long)] json: bool,
        #[arg(long, default_value_t = 5)] seeds: usize,
        #[arg(long)] bodies: bool,
        /// Lets the configured model command pick the seeds from the deep candidate list.
        /// Costs tokens per question; measured +4–5 paraphrase hits of 14 on the bench corpus.
        #[arg(long)] rerank: bool,
        /// Picks the seeds with a local cross-encoder instead of the model command — the same
        /// pool, zero tokens. Needs the exported model in `reranker_dir`.
        #[arg(long, conflicts_with = "rerank")] rerank_local: bool,
        /// Candidates the reranking model is shown; tokens per question grow with it.
        #[arg(long, default_value_t = rerank::DEPTH)] depth: usize,
        /// Answers from the store as it stands, without bringing it in line with the tree first.
        #[arg(long)] stale: bool,
    },
    /// Keeps the store in step with the tree for readers that do not refresh themselves —
    /// editors, MCP servers. Polls, applies the same incremental update `ask` does, and embeds
    /// what changed. Ctrl-C stops it; every write is a rename, so there is nothing to clean up.
    Watch {
        /// Seconds between polls.
        #[arg(long, default_value_t = 30)] every: u64,
        /// Files that have to be waiting before a poll refreshes; a smaller number of them is
        /// carried to the next poll instead, at most three times in a row.
        #[arg(long, default_value_t = 1)] batch: usize,
    },
    Explain { node: String },
    /// Who reaches a symbol (callers by depth, importing files, a risk line), or with `--down`
    /// what it reaches. A class is walked through its members; a caller that imported through a
    /// barrel is found all the same
    Impact {
        symbol: String,
        #[arg(long, default_value_t = 3)] depth: usize,
        #[arg(long)] down: bool,
        #[arg(long)] json: bool,
        /// Answers from the store as it stands, without bringing it in line with the tree first
        #[arg(long)] stale: bool,
    },
    /// The shortest chain of calls from one symbol to another, or that there is none within the depth
    Trace {
        from: String,
        to: String,
        #[arg(long, default_value_t = 6)] depth: usize,
        #[arg(long)] stale: bool,
    },
    /// What the working tree's diff touches and who reaches it: hunks against `--base` (staged,
    /// unstaged and untracked alike) mapped onto symbol spans, then the callers of each
    Changes {
        #[arg(long, default_value = "HEAD")] base: String,
        #[arg(long, default_value_t = 2)] depth: usize,
        #[arg(long)] json: bool,
        #[arg(long)] stale: bool,
    },
    Verify,
    /// Writes reader questions for every requirement-like node through the configured
    /// command, then re-embeds. Costs model tokens once per passage; nothing per query.
    Enrich {
        #[arg(long, default_value_t = 12)] batch: usize,
        #[arg(long, default_value_t = 8)] parallel: usize,
        #[arg(long)] limit: Option<usize>,
        /// Also asks about code: symbols with a doc comment or a body, files with a head comment.
        #[arg(long)] code: bool,
    },
    /// Embeds every row the dense index lacks, without re-reading the tree: a store copied
    /// without its vectors is re-embedded from its graph and questions alone, which is how a
    /// store is measured under another `REPOGRAPH_EMBED_MODEL`.
    Embed,
    Bench { #[arg(long)] cases: Option<PathBuf>, #[arg(long)] rerank: bool, #[arg(long, conflicts_with = "rerank")] rerank_local: bool, #[arg(long, default_value_t = rerank::DEPTH)] depth: usize },
    /// Writes every retriever's ranked list for each question in a JSONL file
    /// (`{"q","expect","kind"}` per line) so the mathematics can be done offline.
    Dump {
        #[arg(long)] queries: PathBuf,
        #[arg(long)] out: PathBuf,
        /// How deep each of the four lists is recorded.
        #[arg(long, default_value_t = 300)] depth: usize,
    },
    ImportLegacy { graph_json: PathBuf },
}

pub struct Extractors {
    pub doc: Box<dyn Extractor>,
    pub code: Box<dyn Extractor>,
    pub registry: Box<dyn Extractor>,
}

pub struct UpdateReport { pub changed: usize, pub removed: usize, pub nodes: usize, pub edges: usize }

/// Re-extracts what the diff names, drops what is gone, and writes the store back.
fn apply_diff(repo: &std::path::Path, store: &store::Store, graph: &mut model::Graph, entries: &[walk::Entry], diff: &walk::Diff, ex: &Extractors) -> anyhow::Result<UpdateReport> {
    let stale: std::collections::BTreeSet<&str> =
        diff.removed.iter().map(String::as_str).chain(diff.changed.iter().map(|e| e.rel.as_str())).collect();
    // A node's `path:line` comes from its primary declaring file. When that file goes, every
    // surviving declarer is re-read too, so line, label and body come from the file that is cited.
    // Re-reading a file removes it first, which orphans the primaries it held in turn — hence the
    // closure: shared decorator ids chain NestJS files together several hops deep.
    let by_rel: std::collections::BTreeMap<&str, &walk::Entry> = entries.iter().map(|e| (e.rel.as_str(), e)).collect();
    let mut co_declared: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    let mut frontier = stale.clone();
    while !frontier.is_empty() {
        frontier = graph.nodes.values()
            .filter(|n| frontier.contains(n.file.as_str()))
            .flat_map(|n| n.files.iter().map(String::as_str))
            .filter(|f| !stale.contains(f) && !co_declared.contains(f) && by_rel.contains_key(f))
            .map(|f| by_rel[f].rel.as_str())
            .collect();
        co_declared.extend(frontier.iter());
    }
    for rel in stale.iter().chain(co_declared.iter()) { graph.remove_file(rel); }
    let reread = co_declared.iter().map(|rel| by_rel[rel]);
    for e in diff.changed.iter().chain(reread) {
        let text = match std::fs::read(repo.join(&e.rel)) {
            // NUL is legal inside a TypeScript string literal; only invalid UTF-8 marks a binary.
            Ok(b) => match String::from_utf8(b) {
                Ok(s) => s,
                Err(_) => { eprintln!("skipping {}: not UTF-8", e.rel); continue; }
            },
            Err(err) => { eprintln!("read {}: {err}", e.rel); continue; }
        };
        let extractor = match e.kind {
            walk::FileKind::Doc => &ex.doc,
            walk::FileKind::Code => &ex.code,
            walk::FileKind::Registry => &ex.registry,
        };
        graph.apply(extractor.extract(&e.rel, &text));
    }
    store.save(graph, &walk::Manifest::from_entries(entries))?;
    Ok(UpdateReport { changed: diff.changed.len(), removed: diff.removed.len(), nodes: graph.nodes.len(), edges: graph.edges.len() })
}

pub fn run_update(repo: &std::path::Path, cfg: &config::Config, ex: &Extractors, wipe: bool) -> anyhow::Result<UpdateReport> {
    let store = store::Store::new(repo);
    if wipe { store.wipe()?; }
    let (mut graph, manifest) = store.load()?;
    let entries = walk::walk(repo, cfg, &manifest)?;
    let diff = manifest.diff(&entries);
    apply_diff(repo, &store, &mut graph, &entries, &diff, ex)
}

/// Writes the manifest back when the walk saw stamps the stored one does not have — a store from
/// before the stat cache, or a file touched without being changed. Without this a tree that never
/// changes would be hashed in full on every question.
fn record_stamps(store: &store::Store, manifest: &walk::Manifest, entries: &[walk::Entry]) -> anyhow::Result<bool> {
    let now = walk::Manifest::from_entries(entries);
    if now.stamps == manifest.stamps { return Ok(false); }
    store.save_manifest(&now)?;
    Ok(true)
}

/// The stored graph brought in line with the working tree, plus what that cost when the tree had
/// moved. `ask` runs this before answering so an edit never has to be followed by an `update`;
/// the extractors are built only when there is something to re-read.
fn graph_for_ask(repo: &std::path::Path, cfg: &config::Config, store: &store::Store, stale: bool, timing: &Timing) -> anyhow::Result<(model::Graph, Option<UpdateReport>)> {
    let (mut graph, manifest, source) = store.load_traced()?;
    timing.stage("graph loaded");
    if stale { return Ok((graph, None)); }
    let entries = walk::walk(repo, cfg, &manifest)?;
    let diff = manifest.diff(&entries);
    timing.stage("tree walked");
    if diff.changed.is_empty() && diff.removed.is_empty() {
        record_stamps(store, &manifest, &entries)?;
        // A store another release or a bare `graph.json` left without a mirror pays the JSON
        // parse once; a refresh below writes the mirror on its own.
        if source == store::Source::Json {
            store.write_mirror("graph.json", &graph)?;
            timing.stage("mirror written");
        }
        return Ok((graph, None));
    }
    let r = apply_diff(repo, store, &mut graph, &entries, &diff, &extractors(repo, cfg)?)?;
    timing.stage("refreshed");
    Ok((graph, Some(r)))
}

/// What a poll of the tree needs between rounds: the graph as this process last wrote it, and
/// the stamp of the manifest that says whether someone else has written since.
struct Watcher<'a> {
    repo: &'a std::path::Path,
    cfg: &'a config::Config,
    store: store::Store,
    ex: Extractors,
    graph: model::Graph,
    manifest: walk::Manifest,
    seen: Option<walk::Stamp>,
    deferred: u32,
}

/// A poll that changes nothing still writes a 10 MB graph, so a batch of one file per save is
/// worth waiting a poll or two for. The cap is what keeps a lone edit from waiting for a second
/// one that never comes: it lands within four polls whatever `--batch` says.
const MAX_DEFERRALS: u32 = 3;

fn refresh_now(pending: usize, batch: usize, deferred: u32) -> bool {
    pending > 0 && (pending >= batch || deferred >= MAX_DEFERRALS)
}

enum Polled { Quiet, Deferred { pending: usize }, Refreshed(UpdateReport) }

impl<'a> Watcher<'a> {
    fn open(repo: &'a std::path::Path, cfg: &'a config::Config) -> anyhow::Result<Watcher<'a>> {
        let store = store::Store::new(repo);
        let (graph, manifest) = store.load()?;
        let seen = store.stamp("manifest.json");
        Ok(Watcher { repo, cfg, ex: extractors(repo, cfg)?, store, graph, manifest, seen, deferred: 0 })
    }

    /// Whatever the tree has moved since the last poll, applied and saved once `batch` files are
    /// waiting. An `ask` or an `update` writing the store meanwhile is picked up rather than
    /// overwritten, which is why the manifest stamp is checked before the graph in hand is used.
    fn poll(&mut self, batch: usize) -> anyhow::Result<Polled> {
        let on_disk = self.store.stamp("manifest.json");
        if on_disk != self.seen {
            let (graph, manifest) = self.store.load()?;
            self.graph = graph;
            self.manifest = manifest;
            self.seen = on_disk;
        }
        let entries = walk::walk(self.repo, self.cfg, &self.manifest)?;
        let diff = self.manifest.diff(&entries);
        let pending = diff.changed.len() + diff.removed.len();
        if !refresh_now(pending, batch, self.deferred) {
            self.deferred = if pending > 0 { self.deferred + 1 } else { 0 };
            // Only a quiet tree may record stamps: the entries of a deferred poll carry the new
            // hashes, and storing those would retire the very changes still waiting to be read.
            if pending == 0 && record_stamps(&self.store, &self.manifest, &entries)? {
                self.manifest = walk::Manifest::from_entries(&entries);
                self.seen = self.store.stamp("manifest.json");
            }
            return Ok(if pending > 0 { Polled::Deferred { pending } } else { Polled::Quiet });
        }
        self.deferred = 0;
        let r = apply_diff(self.repo, &self.store, &mut self.graph, &entries, &diff, &self.ex)?;
        self.manifest = walk::Manifest::from_entries(&entries);
        self.seen = self.store.stamp("manifest.json");
        Ok(Polled::Refreshed(r))
    }
}

/// Keeps the store in step with the tree for readers that do not refresh themselves.
fn run_watch(repo: &std::path::Path, cfg: &config::Config, every: u64, batch: usize, no_dense: bool) -> anyhow::Result<()> {
    let mut w = Watcher::open(repo, cfg)?;
    let mut embedder: Option<Option<index::embed::Embedder>> = None;
    let mut dense: Option<index::dense::DenseIndex> = None;
    let verbose = timing_on();
    eprintln!("watch: {} every {every}s, batch {batch}; Ctrl-C to stop", repo.display());
    loop {
        let t = std::time::Instant::now();
        match w.poll(batch)? {
            Polled::Quiet => {}
            Polled::Deferred { pending } => {
                if verbose { eprintln!("watch: {pending} of {batch} pending, deferred {} of {MAX_DEFERRALS}", w.deferred); }
            }
            Polled::Refreshed(r) => {
                let mut embedded = 0;
                // The model costs ~0.6 s and 1.3 GB to open, so it waits for the first change; the
                // vectors then stay in memory, since every later refresh syncs them again.
                if let Some(e) = embedder.get_or_insert_with(|| open_embedder(no_dense)).as_mut() {
                    let idx = match dense {
                        Some(ref mut d) => d,
                        None => dense.insert(index::dense::DenseIndex::load(&w.store)?),
                    };
                    let questions = enrich::Questions::load(&w.store)?;
                    embedded = idx.sync(&w.graph, &questions, &mut |texts| e.embed(texts))?;
                    if embedded > 0 { idx.save(&w.store)?; }
                }
                println!("refresh: {} changed, {} removed, {} nodes, {} edges, {embedded} vectors in {:.1}s",
                    r.changed, r.removed, r.nodes, r.edges, t.elapsed().as_secs_f32());
                use std::io::Write;
                std::io::stdout().flush()?;
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(every));
    }
}

fn extractors(repo: &std::path::Path, cfg: &config::Config) -> anyhow::Result<Extractors> {
    let ids = ids::IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
    let resolver = code::imports::Resolver::new(repo)?;
    Ok(Extractors {
        doc: Box::new(doc::DocExtractor::new(ids.clone())),
        code: Box::new(code::CodeExtractor::new(resolver, ids.clone())),
        registry: Box::new(doc::registry::RegistryExtractor::new(ids)),
    })
}

fn embed_all(repo: &std::path::Path, no_dense: bool) -> anyhow::Result<()> {
    let Some(mut emb) = open_embedder(no_dense) else { return Ok(()) };
    let store = store::Store::new(repo);
    let (graph, _) = store.load()?;
    let questions = enrich::Questions::load(&store)?;
    let mut dense = index::dense::DenseIndex::load(&store)?;
    let t = std::time::Instant::now();
    let n = dense.sync(&graph, &questions, &mut |texts| emb.embed(texts))?;
    dense.save(&store)?;
    println!("dense: embedded {n} rows in {:.1}s", t.elapsed().as_secs_f32());
    Ok(())
}

/// `REPOGRAPH_TIMING=1` prints where an `ask` spends its time, one line per stage on stderr.
struct Timing { on: bool, start: std::time::Instant, last: std::cell::Cell<std::time::Instant> }

fn timing_on() -> bool { std::env::var_os("REPOGRAPH_TIMING").is_some() }

impl Timing {
    fn new() -> Timing {
        let now = std::time::Instant::now();
        Timing { on: timing_on(), start: now, last: std::cell::Cell::new(now) }
    }

    fn stage(&self, what: &str) {
        if !self.on { return; }
        let now = std::time::Instant::now();
        eprintln!("timing: {:>7.1} ms  (+{:>6.1} ms)  {what}", (now - self.start).as_secs_f64() * 1e3, (now - self.last.get()).as_secs_f64() * 1e3);
        self.last.set(now);
    }
}

fn open_embedder(no_dense: bool) -> Option<index::embed::Embedder> {
    if no_dense { return None; }
    match index::embed::Embedder::open() {
        Ok(e) => Some(e),
        Err(err) => { eprintln!("dense: model unavailable, continuing lexical-only ({err:#})"); None }
    }
}

/// The graph an answer is read from: refreshed against the tree unless `--stale`, and, when
/// the store cannot be written, the stored one with a warning — the same contract as `ask`.
fn graph_for(repo: &std::path::Path, cfg: &config::Config, stale: bool) -> anyhow::Result<model::Graph> {
    let timing = Timing::new();
    let store = store::Store::new(repo);
    match graph_for_ask(repo, cfg, &store, stale, &timing) {
        Ok((graph, refreshed)) => {
            if let Some(r) = refreshed { eprintln!("refresh: {} changed, {} removed", r.changed, r.removed); }
            Ok(graph)
        }
        Err(err) => { eprintln!("refresh: skipped ({err:#})"); Ok(store.load()?.0) }
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let repo = cli.repo.canonicalize()?;
    // Loaded per command: `bench` reads its own from `REPOGRAPH_BENCH_REPO`, and `explain`/`verify`
    // must not fail on a broken `repograph.toml` they never read.
    let load_cfg = || config::Config::load(&repo);
    let wipe = matches!(cli.cmd, Cmd::Build);
    match cli.cmd {
        Cmd::Build | Cmd::Update => {
            let cfg = load_cfg()?;
            let r = run_update(&repo, &cfg, &extractors(&repo, &cfg)?, wipe)?;
            println!("changed {} removed {} nodes {} edges {}", r.changed, r.removed, r.nodes, r.edges);
            embed_all(&repo, cli.no_dense)
        }
        Cmd::Enrich { batch, parallel, limit, code } => {
            let cfg = load_cfg()?;
            let store = store::Store::new(&repo);
            let (graph, _) = store.load()?;
            if graph.nodes.is_empty() { anyhow::bail!("graph is empty — run `repograph build`"); }
            let questions = enrich::Questions::load(&store)?;
            let t = std::time::Instant::now();
            let r = enrich::run(&store, &graph, questions, &cfg.enrich_command, batch, parallel, enrich::Scope { limit, code })?;
            println!("enrich: {} nodes written, {} dropped, {} still without questions, {} batches ({} failed) in {:.0}s", r.generated, r.dropped, r.left, r.batches, r.failed, t.elapsed().as_secs_f32());
            embed_all(&repo, cli.no_dense)
        }
        Cmd::Embed => embed_all(&repo, cli.no_dense),
        Cmd::Ask { words, json, seeds, bodies, rerank, rerank_local, depth, stale } => {
            let timing = Timing::new();
            let cfg = load_cfg()?;
            let store = store::Store::new(&repo);
            let (graph, refreshed) = match graph_for_ask(&repo, &cfg, &store, stale, &timing) {
                Ok(pair) => pair,
                // A store that cannot be written (read-only checkout, a walk that failed) still
                // holds an answer: say once that it may be behind, then give the stored one.
                Err(err) => { eprintln!("refresh: skipped ({err:#})"); (store.load()?.0, None) }
            };
            if let Some(r) = &refreshed { eprintln!("refresh: {} changed, {} removed", r.changed, r.removed); }
            let ids = ids::IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
            timing.stage("ids ready");
            let (questions, source) = enrich::Questions::load_traced(&store)?;
            if !stale && source == store::Source::Json { questions.write_mirror(&store)?; }
            timing.stage("questions ready");
            // Opening the ONNX model costs ~0.6 s and 1.3 GB, the vectors 50 MB; an exact id or
            // symbol match never asks for either, so both open on the first fused query.
            let embedder: std::cell::RefCell<Option<Option<index::embed::Embedder>>> = std::cell::RefCell::new(None);
            let dense_idx: std::cell::RefCell<Option<index::dense::DenseIndex>> = std::cell::RefCell::new(None);
            // The refresh above moved passages the vectors were built from. This query needs the
            // model open anyway, so the changed rows are re-embedded here — with `--no-dense` or
            // on the exact-id path nothing opens, and the vectors catch up on the next fused
            // query or `update`.
            let resync = std::cell::Cell::new(refreshed.is_some());
            let dense_fn = |q: &str, k: usize| -> (Vec<String>, Vec<String>) {
                let mut slot = embedder.borrow_mut();
                let e = slot.get_or_insert_with(|| { let e = open_embedder(cli.no_dense); timing.stage("model opened"); e });
                let mut idx = dense_idx.borrow_mut();
                let idx = idx.get_or_insert_with(|| {
                    let i = index::dense::DenseIndex::load(&store).unwrap_or_else(|err| { eprintln!("dense: index unreadable, continuing lexical-only ({err:#})"); Default::default() });
                    timing.stage("vectors loaded"); i
                });
                if resync.replace(false) {
                    if let Some(e) = e.as_mut() {
                        match idx.sync(&graph, &questions, &mut |texts| e.embed(texts)) {
                            Ok(0) => {}
                            Ok(n) => {
                                if let Err(err) = idx.save(&store) { eprintln!("refresh: vectors not saved ({err:#})"); }
                                eprintln!("refresh: {n} vectors embedded");
                            }
                            Err(err) => eprintln!("refresh: vectors unchanged ({err:#})"),
                        }
                        timing.stage("vectors synced");
                    }
                }
                let out = match e.as_mut().and_then(|e| e.query(q).ok()) { Some(v) => idx.search(&v, k), None => (Vec::new(), Vec::new()) };
                timing.stage("query embedded and searched");
                out
            };
            let opts = query::Options { seeds, bodies, dense: !cli.no_dense && index::dense::DenseIndex::present(&store), json, depth };
            let rerank_fn = |q: &str, c: &[(String, String)]| rerank::run(&cfg.rerank_command, q, c);
            let cross = std::cell::RefCell::new(if rerank_local {
                let dir = if cfg.reranker_dir.is_empty() { index::cross::default_dir()? } else { std::path::PathBuf::from(&cfg.reranker_dir) };
                Some(index::cross::CrossEncoder::open(&dir).context("--rerank-local")?)
            } else { None });
            let local_fn = |q: &str, c: &[(String, String)]| -> Vec<String> {
                let mut m = cross.borrow_mut();
                let Some(m) = m.as_mut() else { return Vec::new() };
                let texts: Vec<String> = c.iter().map(|(_, t)| t.clone()).collect();
                match m.score(q, &texts) {
                    Ok(s) => index::cross::pick(&s, c, index::cross::PICK),
                    // Like a failing rerank command: say so and answer from the fused order.
                    Err(e) => { eprintln!("rerank-local: {e:#}; answering from the fused order"); Vec::new() }
                }
            };
            let rerank: Option<query::Rerank> = if rerank_local { Some(&local_fn) } else if rerank { Some(&rerank_fn) } else { None };
            let answer = query::ask(&graph, &ids, &questions, Some(&dense_fn), rerank, &words, &opts);
            timing.stage("answered");
            print!("{}", query::render(&answer, &graph, &opts));
            // Nothing here is written back, and unwinding a 1.3 GB model session plus the graph
            // costs a fused answer a measurable share of its wall time: leave without it.
            use std::io::Write;
            std::io::stdout().flush()?;
            timing.stage("printed");
            std::process::exit(0)
        }
        Cmd::Watch { every, batch } => run_watch(&repo, &load_cfg()?, every, batch, cli.no_dense),
        Cmd::Explain { node } => {
            let (graph, _) = store::Store::new(&repo).load()?;
            match query::explain(&graph, &node) {
                Some(s) => { print!("{s}"); Ok(()) }
                None => anyhow::bail!("no node matches {node}"),
            }
        }
        Cmd::Impact { symbol, depth, down, json, stale } => {
            let graph = graph_for(&repo, &load_cfg()?, stale)?;
            let Some(root) = query::resolve(&graph, &symbol) else { anyhow::bail!("no node matches {symbol}") };
            let (imp, direction) = if down { (impact::downstream(&graph, &root.id, depth), "downstream") } else { (impact::upstream(&graph, &root.id, depth), "upstream") };
            print!("{}", if json { impact::render_json(&graph, &imp, direction) } else { impact::render(&graph, &imp, direction) });
            Ok(())
        }
        Cmd::Trace { from, to, depth, stale } => {
            let graph = graph_for(&repo, &load_cfg()?, stale)?;
            let Some(a) = query::resolve(&graph, &from) else { anyhow::bail!("no node matches {from}") };
            let Some(b) = query::resolve(&graph, &to) else { anyhow::bail!("no node matches {to}") };
            match impact::trace(&graph, &a.id, &b.id, depth) {
                Some(path) => {
                    for (i, id) in path.iter().enumerate() {
                        let at = graph.nodes.get(id).map(|n| format!("{}:{}", n.file, n.line)).unwrap_or_default();
                        println!("{}{id}  {at}", if i == 0 { "" } else { "  → " });
                    }
                    Ok(())
                }
                None => anyhow::bail!("no call path from {} to {} within {depth} hops", a.id, b.id),
            }
        }
        Cmd::Changes { base, depth, json, stale } => {
            let graph = graph_for(&repo, &load_cfg()?, stale)?;
            let hunks = changes::hunks_from_git(&repo, &base)?;
            let r = changes::report(&graph, &hunks, depth);
            print!("{}", if json { changes::render_json(&graph, &r) } else { changes::render(&graph, &r) });
            Ok(())
        }
        Cmd::Verify => {
            let (graph, _) = store::Store::new(&repo).load()?;
            print!("{}", query::verify(&graph));
            if graph.nodes.is_empty() { anyhow::bail!("graph is empty — run `repograph build`"); }
            Ok(())
        }
        Cmd::Bench { cases, rerank, rerank_local, depth } => {
            if bench::run(&repo, cases.as_deref(), cli.no_dense, rerank, rerank_local, depth)? { Ok(()) } else { anyhow::bail!("bench floors not met") }
        }
        Cmd::Dump { queries, out, depth } => dump::run(&repo, &queries, &out, depth, cli.no_dense),
        Cmd::ImportLegacy { graph_json } => {
            let cfg = load_cfg()?;
            let store = store::Store::new(&repo);
            let (mut graph, manifest) = store.load()?;
            let ids = ids::IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
            let text = std::fs::read_to_string(&graph_json)?;
            let r = legacy::import(&mut graph, &ids, &text)?;
            store.save(&graph, &manifest)?;
            println!(
                "legacy: {} edges, {} both endpoints resolved, {} one, {} concepts created, {} self-loops dropped",
                r.edges_seen, r.resolved_both, r.resolved_one, r.concepts_created, r.self_loops
            );
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_repo(body: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("docs/a.md"), body).unwrap();
        dir
    }

    const ONE: &str = "# A\n\n**FR-PAY-22 · MUST · cancellation window**\n\nbody\n";
    const TWO: &str = "# A\n\n**FR-PAY-22 · MUST · cancellation window**\n\nbody\n\n**FR-PAY-23 · MUST · refund window**\n\nbody\n";

    fn built(repo: &std::path::Path, cfg: &config::Config) {
        run_update(repo, cfg, &extractors(repo, cfg).unwrap(), true).unwrap();
    }

    #[test]
    fn an_ask_after_an_edit_answers_from_the_edited_file() {
        let dir = doc_repo(ONE);
        let (repo, cfg) = (dir.path(), config::Config::default());
        built(repo, &cfg);
        std::fs::write(repo.join("docs/a.md"), TWO).unwrap();
        let store = store::Store::new(repo);
        let (graph, refreshed) = graph_for_ask(repo, &cfg, &store, false, &Timing::new()).unwrap();
        let r = refreshed.expect("the edit is a refresh");
        assert_eq!((r.changed, r.removed), (1, 0));
        let ids = ids::IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
        let opts = query::Options { seeds: 5, bodies: false, dense: false, json: false, depth: rerank::DEPTH };
        let words = ["refund".to_string(), "window".to_string()];
        let answer = query::ask(&graph, &ids, &enrich::Questions::default(), None, None, &words, &opts);
        assert!(query::render(&answer, &graph, &opts).contains("FR-PAY-23"));
        // The store carries the edit too, so the next reader has nothing left to redo.
        assert!(store.load().unwrap().0.nodes.contains_key("FR-PAY-23"));
    }

    #[test]
    fn an_ask_on_an_unchanged_tree_writes_nothing() {
        let dir = doc_repo(ONE);
        let (repo, cfg) = (dir.path(), config::Config::default());
        built(repo, &cfg);
        let saved = |name: &str| std::fs::metadata(repo.join(".repograph").join(name)).unwrap().modified().unwrap();
        let (before_graph, before_manifest) = (saved("graph.json"), saved("manifest.json"));
        let (graph, refreshed) = graph_for_ask(repo, &cfg, &store::Store::new(repo), false, &Timing::new()).unwrap();
        assert!(refreshed.is_none());
        assert!(graph.nodes.contains_key("FR-PAY-22"));
        assert_eq!((saved("graph.json"), saved("manifest.json")), (before_graph, before_manifest));
    }

    // A store written before the stat cache carries no stamps. The first question records them,
    // so the walk behind the next one is stat-only, and leaves the graph alone doing it.
    #[test]
    fn a_first_ask_on_a_store_without_stamps_records_them() {
        let dir = doc_repo(ONE);
        let (repo, cfg) = (dir.path(), config::Config::default());
        built(repo, &cfg);
        let store = store::Store::new(repo);
        let files = store.load().unwrap().1.files;
        store.save_manifest(&walk::Manifest { files, stamps: Default::default() }).unwrap();
        let graph_before = std::fs::read(repo.join(".repograph/graph.json")).unwrap();
        assert!(graph_for_ask(repo, &cfg, &store, false, &Timing::new()).unwrap().1.is_none());
        let manifest = store.load().unwrap().1;
        assert_eq!(manifest.stamps.len(), manifest.files.len());
        assert_eq!(std::fs::read(repo.join(".repograph/graph.json")).unwrap(), graph_before);
    }

    #[test]
    fn stale_answers_from_the_store_and_leaves_it_untouched() {
        let dir = doc_repo(ONE);
        let (repo, cfg) = (dir.path(), config::Config::default());
        built(repo, &cfg);
        let before = std::fs::read(repo.join(".repograph/graph.json")).unwrap();
        std::fs::write(repo.join("docs/a.md"), TWO).unwrap();
        let (graph, refreshed) = graph_for_ask(repo, &cfg, &store::Store::new(repo), true, &Timing::new()).unwrap();
        assert!(refreshed.is_none());
        assert!(!graph.nodes.contains_key("FR-PAY-23"));
        assert_eq!(std::fs::read(repo.join(".repograph/graph.json")).unwrap(), before);
    }

    #[test]
    fn a_watch_poll_applies_an_edit_and_the_next_one_has_nothing_to_do() {
        let dir = doc_repo(ONE);
        let (repo, cfg) = (dir.path(), config::Config::default());
        built(repo, &cfg);
        let mut w = Watcher::open(repo, &cfg).unwrap();
        assert!(matches!(w.poll(1).unwrap(), Polled::Quiet));
        std::fs::write(repo.join("docs/a.md"), TWO).unwrap();
        let Polled::Refreshed(r) = w.poll(1).unwrap() else { panic!("the edit is a refresh") };
        assert_eq!((r.changed, r.removed), (1, 0));
        assert!(store::Store::new(repo).load().unwrap().0.nodes.contains_key("FR-PAY-23"));
        assert!(matches!(w.poll(1).unwrap(), Polled::Quiet));
    }

    // Under a batch a lone edit waits, but only for the three polls the cap allows — and the
    // deferred polls must not retire it by recording the stamps of the files still to be read.
    #[test]
    fn a_lone_edit_under_a_batch_is_applied_by_the_fourth_poll() {
        let dir = doc_repo(ONE);
        let (repo, cfg) = (dir.path(), config::Config::default());
        built(repo, &cfg);
        let mut w = Watcher::open(repo, &cfg).unwrap();
        std::fs::write(repo.join("docs/a.md"), TWO).unwrap();
        for _ in 0..MAX_DEFERRALS {
            assert!(matches!(w.poll(4).unwrap(), Polled::Deferred { pending: 1 }));
        }
        let Polled::Refreshed(r) = w.poll(4).unwrap() else { panic!("the cap releases the edit") };
        assert_eq!((r.changed, r.removed), (1, 0));
        assert!(store::Store::new(repo).load().unwrap().0.nodes.contains_key("FR-PAY-23"));
    }

    #[test]
    fn a_full_batch_refreshes_without_waiting() {
        let dir = doc_repo(ONE);
        let (repo, cfg) = (dir.path(), config::Config::default());
        built(repo, &cfg);
        let mut w = Watcher::open(repo, &cfg).unwrap();
        std::fs::write(repo.join("docs/a.md"), TWO).unwrap();
        std::fs::write(repo.join("docs/b.md"), "# B\n\n**FR-PAY-24 · MUST · chargeback**\n\nbody\n").unwrap();
        let Polled::Refreshed(r) = w.poll(2).unwrap() else { panic!("two files fill the batch") };
        assert_eq!((r.changed, r.removed), (2, 0));
    }

    #[test]
    fn a_batch_of_one_refreshes_on_the_first_pending_file() {
        assert!(refresh_now(1, 1, 0));
        assert!(refresh_now(9, 1, 0));
    }

    #[test]
    fn a_batch_waits_for_its_files_and_the_cap_ends_the_wait() {
        assert!(!refresh_now(2, 5, 0));
        assert!(!refresh_now(2, 5, MAX_DEFERRALS - 1));
        assert!(refresh_now(2, 5, MAX_DEFERRALS));
        assert!(refresh_now(5, 5, 0));
    }

    #[test]
    fn a_quiet_tree_never_refreshes_however_long_it_has_waited() {
        assert!(!refresh_now(0, 1, 0));
        assert!(!refresh_now(0, 1, MAX_DEFERRALS + 9));
    }

    // A watcher holds the graph between polls; another writer's `ask` or `update` must not be
    // undone by the next poll writing a graph that predates it.
    #[test]
    fn a_watch_poll_takes_up_a_store_another_writer_has_moved() {
        let dir = doc_repo(ONE);
        let (repo, cfg) = (dir.path(), config::Config::default());
        built(repo, &cfg);
        let mut w = Watcher::open(repo, &cfg).unwrap();
        std::fs::write(repo.join("docs/b.md"), TWO).unwrap();
        built(repo, &cfg);
        std::fs::write(repo.join("docs/c.md"), "# C\n\n**FR-PAY-25 · MUST · chargeback**\n\nbody\n").unwrap();
        assert!(matches!(w.poll(1).unwrap(), Polled::Refreshed(_)), "c.md is a refresh");
        let (stored, _) = store::Store::new(repo).load().unwrap();
        assert!(stored.nodes.contains_key("FR-PAY-25"));
        assert!(stored.nodes.contains_key("FR-PAY-23"), "the other writer's node survived the poll");
    }

    // A `.ts` that is not UTF-8 is a binary that landed under a code glob; it is reported and
    // skipped. A NUL byte inside a string literal is valid TypeScript and is parsed like any other.
    #[test]
    fn a_binary_under_a_code_glob_is_skipped_and_a_nul_in_a_string_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        std::fs::write(repo.join("bin.ts"), b"export const x = 1;\n\xff\xfe\x00").unwrap();
        std::fs::write(repo.join("nul.ts"), b"export const marker = 'a\x00b';\n").unwrap();
        let cfg = config::Config::default();
        let ex = extractors(repo, &cfg).unwrap();
        let r = run_update(repo, &cfg, &ex, true).unwrap();
        assert_eq!(r.changed, 2);
        let (graph, _) = store::Store::new(repo).load().unwrap();
        assert!(graph.nodes.contains_key("sym:nul.ts::marker"));
        assert!(!graph.nodes.keys().any(|k| k.starts_with("sym:bin.ts::")));
    }

    #[test]
    fn cyrillic_paths_are_walked_declared_and_cited() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        std::fs::create_dir_all(repo.join("docs/требования")).unwrap();
        std::fs::write(repo.join("docs/требования/оплата.md"), "# Оплата\n\n**FR-PAY-22 · MUST · Отмена**\n\nтело\n").unwrap();
        let cfg = config::Config::default();
        let ex = extractors(repo, &cfg).unwrap();
        run_update(repo, &cfg, &ex, true).unwrap();
        let (graph, manifest) = store::Store::new(repo).load().unwrap();
        assert!(manifest.files.contains_key("docs/требования/оплата.md"));
        let n = &graph.nodes["FR-PAY-22"];
        assert_eq!((n.file.as_str(), n.line), ("docs/требования/оплата.md", 3));
        assert!(graph.nodes.contains_key("file:docs/требования/оплата.md"));
    }

    // A requirement declared twice keeps a `path:line` that belongs to one file: when the primary
    // declarer goes, the survivor is re-read rather than relabelled with the primary's line.
    #[test]
    fn deleting_the_primary_declarer_rereads_the_survivor() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        std::fs::create_dir_all(repo.join("docs")).unwrap();
        std::fs::write(repo.join("docs/a.md"), "# A\n\n**FR-PAY-22 · MUST · first**\n\nbody a\n").unwrap();
        std::fs::write(repo.join("docs/b.md"), "# B\n\nintro\n\nmore\n\n**FR-PAY-22 · MUST · second**\n\nbody b\n").unwrap();
        let cfg = config::Config::default();
        let ex = extractors(repo, &cfg).unwrap();
        run_update(repo, &cfg, &ex, true).unwrap();
        std::fs::remove_file(repo.join("docs/a.md")).unwrap();
        run_update(repo, &cfg, &ex, false).unwrap();
        let (graph, _) = store::Store::new(repo).load().unwrap();
        let n = &graph.nodes["FR-PAY-22"];
        assert_eq!((n.file.as_str(), n.line, n.label.as_str()), ("docs/b.md", 7, "second"));
    }

    // Re-reading b.md for FR-PAY-22 removes b.md first, which was the primary of FR-PAY-20 as
    // well; that node must not end up citing c.md with b.md's line.
    #[test]
    fn rereading_a_declarer_does_not_strand_the_nodes_it_was_primary_for() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        std::fs::create_dir_all(repo.join("docs")).unwrap();
        std::fs::write(repo.join("docs/a.md"), "# A

**FR-PAY-22 · MUST · x in a**
").unwrap();
        std::fs::write(repo.join("docs/b.md"), "# B

**FR-PAY-22 · MUST · x in b**

**FR-PAY-20 · MUST · y in b**
").unwrap();
        std::fs::write(repo.join("docs/c.md"), "# C

intro

more

**FR-PAY-20 · MUST · y in c**
").unwrap();
        let cfg = config::Config::default();
        let ex = extractors(repo, &cfg).unwrap();
        run_update(repo, &cfg, &ex, true).unwrap();
        std::fs::remove_file(repo.join("docs/a.md")).unwrap();
        run_update(repo, &cfg, &ex, false).unwrap();
        let (graph, _) = store::Store::new(repo).load().unwrap();
        let y = &graph.nodes["FR-PAY-20"];
        assert_eq!((y.file.as_str(), y.line, y.label.as_str()), ("docs/b.md", 5, "y in b"));
    }
}
