mod ask;
mod bench;
mod changes;
mod code;
mod config;
mod doc;
mod dump;
mod enrich;
mod families;
mod rerank;
mod ids;
mod impact;
mod install_agent;
mod index;
mod legacy;
mod model;
mod prime;
mod query;
mod serve;
mod store;
mod walk;

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
        /// Answers in this process even when a `serve` is listening.
        #[arg(long)] no_serve: bool,
    },
    /// Answers `ask` from a resident process over `.repograph/serve.sock`: the model, the
    /// vectors and the indexes open once. Refreshes before every answer the way `ask` does,
    /// polls between them like `watch`, exits after `--idle` seconds without a question.
    Serve {
        #[arg(long, default_value_t = 30)] every: u64,
        #[arg(long, default_value_t = 1)] batch: usize,
        #[arg(long, default_value_t = 1800)] idle: u64,
        /// Seconds without a question after which the model is dropped and the process stays.
        #[arg(long, default_value_t = 300)] idle_model: u64,
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
    Explain {
        node: String,
        #[arg(long)] json: bool,
    },
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
        #[arg(long)] json: bool,
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
    Verify {
        #[arg(long)] json: bool,
    },
    /// Writes the agent-facing surface into this repository: one hook dispatched on four events, a
    /// skill, a subagent definition and a ten-line stanza in the instructions file. Idempotent —
    /// a second run rewrites the same bytes and says it changed nothing.
    InstallAgent {
        /// Claude Code's `.claude/`.
        #[arg(long)] claude: bool,
        /// Codex's `.codex/`.
        #[arg(long)] codex: bool,
        /// How this repository invokes the binary, in every file the install writes.
        #[arg(long, default_value = "repograph")] command: String,
    },
    /// What a coding agent should be told about this repository at the start of a session: node
    /// counts, whether the questions are written, which embedder the vectors belong to, how many
    /// families the documents define, and the five commands. Reads the store; never refreshes.
    Prime {
        #[arg(long)] json: bool,
    },
    /// Which families the documents define and where, how many nodes each holds, and which
    /// id-like prefixes were left as text because no line defines them. Reads the built store
    /// and the documents; writes nothing.
    Families {
        #[arg(long)] json: bool,
    },
    /// Writes reader questions for every requirement-like node through the configured
    /// command, then re-embeds. Costs model tokens once per passage; nothing per query.
    Enrich {
        #[arg(long, default_value_t = 12)] batch: usize,
        #[arg(long, default_value_t = 8)] parallel: usize,
        #[arg(long)] limit: Option<usize>,
        /// Also asks about code: symbols with a doc comment or a body, files with a head comment.
        #[arg(long)] code: bool,
        /// Reads the corpus's languages again instead of re-using the list the store was
        /// enriched under. A list that changes restales every entry, so it is taken once.
        #[arg(long)] detect_languages: bool,
        /// Leaves the prompt and the generator's reply of every batch that came back short in
        /// this directory, as `<first id>.in` and `.out`. `REPOGRAPH_ENRICH_KEEP` does the same.
        #[arg(long, value_name = "DIR")] keep_raw: Option<std::path::PathBuf>,
    },
    /// Embeds every row the dense index lacks, without re-reading the tree: a store copied
    /// without its vectors is re-embedded from its graph and questions alone, which is how a
    /// store is measured under another `REPOGRAPH_EMBED_MODEL`.
    Embed,
    /// Which embedder this store's vectors belong to, and what else has been measured. With a hub
    /// id, the switch: the model is downloaded and opened, `embed_model` is written into
    /// `repograph.toml`, and the index is rewritten under it — one command, and a working state
    /// at the end of it.
    Model {
        /// The model to switch to. Without one nothing is written and the table is printed.
        hub_id: Option<String>,
        /// Writes the key and stops, for someone who would rather re-embed at a time they choose.
        #[arg(long, requires = "hub_id")] no_embed: bool,
        /// The report as an object. A switch is not a reader, so it has no JSON form.
        #[arg(long, conflicts_with = "hub_id")] json: bool,
    },
    Bench {
        #[arg(long)] cases: Option<PathBuf>,
        #[arg(long)] rerank: bool,
        #[arg(long, conflicts_with = "rerank")] rerank_local: bool,
        #[arg(long, default_value_t = rerank::DEPTH)] depth: usize,
        /// Runs the suite this many times and prints the median beneath the runs. One run reads
        /// exactly as it always has; a bar judged against a single reading is measuring the
        /// machine as much as the change.
        #[arg(long, default_value_t = 1)] repeat: usize,
    },
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

pub struct UpdateReport {
    pub changed: usize,
    pub removed: usize,
    pub nodes: usize,
    pub edges: usize,
    /// Eligible nodes left without questions, when the store has questions for some others.
    /// Computed here because the graph is already in hand: a writer that re-loaded the store to
    /// say this would pay a whole graph read on the no-op update a commit hook fires.
    pub unenriched: Option<usize>,
    /// The stamp of the `graph.json` this update wrote, as written.
    pub graph_at: Option<walk::Stamp>,
}

/// Re-extracts what the diff names, drops what is gone, and writes the store back. Nothing else
/// is read: every citation a file holds was extracted when the file was read, whatever prefix it
/// names, and which of them a reader may follow is decided on the graph by `settle` — so a family
/// appearing or vanishing costs the file that declared it, and nothing more.
///
/// The one exception is a store an older grammar wrote: a file that has not moved can still hold
/// a citation the reader of the day never looked for, and no hash says so, so `manifest` is asked
/// and the whole tree is read once. Once, because the manifest saved below carries this build's
/// generation — after which an update is the incremental one again.
pub(crate) fn apply_diff(repo: &std::path::Path, store: &store::Store, graph: &mut model::Graph, entries: &[walk::Entry], diff: &walk::Diff, manifest: &walk::Manifest, ex: &Extractors) -> anyhow::Result<UpdateReport> {
    let named: std::collections::BTreeSet<&str> = diff.changed.iter().map(|e| e.rel.as_str()).collect();
    let code_changed = diff.changed.iter().filter(|e| e.kind == walk::FileKind::Code).map(|e| e.rel.as_str());
    if let Some(note) = code::lang::files_only_note(code_changed) {
        eprintln!("{note}");
    }
    let regrammar: Vec<&walk::Entry> = match manifest.stale_grammar() {
        true => entries.iter().filter(|e| !named.contains(e.rel.as_str())).collect(),
        false => Vec::new(),
    };
    if !regrammar.is_empty() {
        eprintln!("grammar: this store was read by generation {} and this build reads by {} — re-reading all {} files once, so citations the older grammar never looked for are found; the next update reads only what changed",
            manifest.grammar, walk::GRAMMAR, entries.len());
    }
    let stale: std::collections::BTreeSet<&str> = diff.removed.iter().map(String::as_str)
        .chain(diff.changed.iter().chain(regrammar.iter().copied()).map(|e| e.rel.as_str())).collect();
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
    // `Graph::apply` gives a shared id to whichever file declares it first, so the re-read runs
    // in the walk's own order: out of it, an update hands the id — its path, line, title and
    // body — to a different file than a build does.
    let mut work: Vec<&walk::Entry> = diff.changed.iter().chain(regrammar.iter().copied()).collect();
    work.sort_by(|a, b| a.rel.cmp(&b.rel));
    let reread = co_declared.iter().map(|rel| by_rel[rel]);
    let mut unread = false;
    for e in work.into_iter().chain(reread) {
        let text = match std::fs::read(repo.join(&e.rel)) {
            // NUL is legal inside a TypeScript string literal; only invalid UTF-8 marks a binary.
            Ok(b) => match String::from_utf8(b) {
                Ok(s) => s,
                Err(_) => { eprintln!("skipping {}: not UTF-8", e.rel); continue; }
            },
            // Not the arm above: a binary yields nothing however often it is read, where a file
            // that would not open is one this pass has no reading of at all.
            Err(err) => { eprintln!("read {}: {err}", e.rel); unread = true; continue; }
        };
        let extractor = match e.kind {
            walk::FileKind::Doc => &ex.doc,
            walk::FileKind::Code => &ex.code,
            walk::FileKind::Registry => &ex.registry,
        };
        graph.apply(extractor.extract(&e.rel, &text));
    }
    // Whether anything was re-extracted or dropped. Nothing else can move a family, so on a
    // no-op update — the one a commit hook fires — the settle below would read every node and
    // rebuild the whole edge set to arrive at what is already there.
    let moved = !diff.changed.is_empty() || !diff.removed.is_empty() || !regrammar.is_empty();
    if moved { graph.settle(); }
    let mut saved = walk::Manifest::from_entries(entries);
    // A file removed from the graph above and then not read is a hole, and the stamp is the only
    // thing that can recover it — on this path as much as on the grammar walk. The manifest below
    // records what the *walk* read, so a file the walk hashed and the extract then failed to open
    // is written down as current: the next diff finds it equal and the stamp cache never reopens
    // it. `0` and not the generation the store claimed, which on a machine holding two versions
    // can be newer than this build's: written back, that value would tell the build able to fill
    // the hole that there is nothing to do. `0` is stale against every generation there is.
    if unread { saved.grammar = 0; }
    let graph_at = store.save(graph, &saved)?;
    // Only when something was re-extracted: a tree that did not move cannot have grown a node
    // without questions, and the no-op update a commit hook fires should not read the questions
    // file to be told so.
    let unenriched = moved
        .then(|| enrich::Questions::load(store).ok().and_then(|q| enrich::unenriched_note(graph, &q)))
        .flatten();
    Ok(UpdateReport { changed: diff.changed.len(), removed: diff.removed.len(), nodes: graph.nodes.len(), edges: graph.edges.len(), unenriched, graph_at })
}

/// The store brought in line with the tree.
pub fn run_update(repo: &std::path::Path, cfg: &config::Config, wipe: bool) -> anyhow::Result<UpdateReport> {
    let store = store::Store::new(repo);
    if wipe { store.wipe()?; }
    let (mut graph, manifest) = store.load()?;
    let entries = walk::walk(repo, cfg, &manifest)?;
    let diff = manifest.diff(&entries);
    // A wipe has just emptied the graph, so this one test covers both fresh builds: `build`, and
    // an `update` on a store nobody has built yet.
    let bootstrap = graph.nodes.is_empty();
    // A tree that has not moved cannot have moved its families either, and the pass over every
    // node it takes to say so is what the no-op update a commit hook fires would pay for nothing.
    let quiet = !bootstrap && diff.changed.is_empty() && diff.removed.is_empty() && !manifest.stale_grammar();
    let before = (!quiet).then(|| families::of_graph(&graph));
    let r = apply_diff(repo, &store, &mut graph, &entries, &diff, &manifest, &extractors(repo)?)?;
    // `settle` has already moved the citations a new family admits or a lost one withdraws; what
    // is left is to say so, since the next `ask` answers over edges that were not there before.
    match (bootstrap, before) {
        (true, _) => eprintln!("{}", families::line(&families::of_graph(&graph))),
        (false, Some(before)) => {
            let moved = families::moved(&before, &families::of_graph(&graph));
            if !moved.is_empty() { eprintln!("families: {}", moved.join(", ")); }
        }
        (false, None) => {}
    }
    Ok(r)
}

/// Writes the manifest back when the walk saw stamps the stored one does not have — a store from
/// before the stat cache, or a file touched without being changed. Without this a tree that never
/// changes would be hashed in full on every question.
pub(crate) fn record_stamps(store: &store::Store, manifest: &walk::Manifest, entries: &[walk::Entry]) -> anyhow::Result<bool> {
    let mut now = walk::Manifest::from_entries(entries);
    if now.stamps == manifest.stamps { return Ok(false); }
    // Nothing was re-extracted here, so this save may not claim the graph beside it was read by
    // this build's grammar: that claim is `apply_diff`'s to make, after the walk that earns it.
    now.grammar = manifest.grammar;
    store.save_manifest(&now)?;
    Ok(true)
}

/// What a poll of the tree needs between rounds: the graph as this process last wrote it, and
/// the stamp of the manifest that says whether someone else has written since.
pub(crate) struct Watcher<'a> {
    repo: &'a std::path::Path,
    cfg: &'a config::Config,
    store: store::Store,
    ex: Extractors,
    graph: model::Graph,
    /// The `graph.json` stamp `graph` was read at or written to, handed to the reader that takes
    /// the graph over so it can hold the vectors' claim against it.
    graph_at: Option<walk::Stamp>,
    manifest: walk::Manifest,
    seen: Option<walk::Stamp>,
    deferred: u32,
    /// Whether the last poll found the store written by someone else and read it back. A poll
    /// that only reads reports `Quiet`, and a reader holding the graph from before it would
    /// then answer from a store older than the one a one-shot `ask` loads off disk.
    reloaded: bool,
}

/// A poll that changes nothing still writes a 10 MB graph, so a batch of one file per save is
/// worth waiting a poll or two for. The cap is what keeps a lone edit from waiting for a second
/// one that never comes: it lands within four polls whatever `--batch` says.
const MAX_DEFERRALS: u32 = 3;

fn refresh_now(pending: usize, batch: usize, deferred: u32) -> bool {
    pending > 0 && (pending >= batch || deferred >= MAX_DEFERRALS)
}

pub(crate) enum Polled { Quiet, Deferred { pending: usize }, Refreshed(UpdateReport) }

impl<'a> Watcher<'a> {
    fn open(repo: &'a std::path::Path, cfg: &'a config::Config) -> anyhow::Result<Watcher<'a>> {
        let store = store::Store::new(repo);
        let graph_at = store.stamp("graph.json");
        let (graph, manifest) = store.load()?;
        let seen = store.stamp("manifest.json");
        let ex = extractors(repo)?;
        Ok(Watcher { repo, cfg, ex, store, graph, graph_at, manifest, seen, deferred: 0, reloaded: false })
    }

    /// The store read back when another process has written it, without the walk a poll does —
    /// a `stat` and, only when it moved, a load. It is what a reader needs to be no older than
    /// the store on disk, which is all a `--stale` answer ever promised to be.
    fn reload_if_moved(&mut self) -> anyhow::Result<bool> {
        let on_disk = self.store.stamp("manifest.json");
        if on_disk == self.seen { return Ok(false); }
        let graph_at = self.store.stamp("graph.json");
        let (graph, manifest) = self.store.load()?;
        self.graph = graph;
        self.graph_at = graph_at;
        self.manifest = manifest;
        self.seen = on_disk;
        Ok(true)
    }

    /// Whatever the tree has moved since the last poll, applied and saved once `batch` files are
    /// waiting. An `ask` or an `update` writing the store meanwhile is picked up rather than
    /// overwritten, which is why the manifest stamp is checked before the graph in hand is used.
    fn poll(&mut self, batch: usize) -> anyhow::Result<Polled> {
        self.reloaded = self.reload_if_moved()?;
        let entries = walk::walk(self.repo, self.cfg, &self.manifest)?;
        let diff = self.manifest.diff(&entries);
        let pending = diff.changed.len() + diff.removed.len();
        // A store an older grammar wrote is behind the tree in a way no hash reports, so a quiet
        // poll is not the same as nothing to do: the refresh below is what repairs it, once.
        if !self.manifest.stale_grammar() && !refresh_now(pending, batch, self.deferred) {
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
        // This poll is an `update` in everything but name — it re-extracts and it writes — so it
        // says which families moved the way one does. A quiet poll pays none of it.
        let before = families::of_graph(&self.graph);
        let r = apply_diff(self.repo, &self.store, &mut self.graph, &entries, &diff, &self.manifest, &self.ex)?;
        let moved = families::moved(&before, &families::of_graph(&self.graph));
        if !moved.is_empty() { eprintln!("families: {}", moved.join(", ")); }
        self.manifest = walk::Manifest::from_entries(&entries);
        self.seen = self.store.stamp("manifest.json");
        self.graph_at = r.graph_at;
        Ok(Polled::Refreshed(r))
    }
}

/// Keeps the store in step with the tree for readers that do not refresh themselves.
fn run_watch(repo: &std::path::Path, cfg: &config::Config, every: u64, batch: usize, no_dense: bool) -> anyhow::Result<()> {
    let mut w = Watcher::open(repo, cfg)?;
    let model = index::embed::resolve(None, &cfg.embed_model);
    let threads = index::embed::threads(cfg.resources);
    let verbose = ask::timing_on();
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
                // Opened for this refresh and dropped with it. A watcher is idle between polls and
                // nobody is waiting on one, so the half-second the model takes to open is not a
                // cost anyone can see — while the 1.4 GB it holds is the whole of what `watch`
                // takes from a person on a 16 GB laptop, and it used to hold it until exit.
                if let Some(mut e) = ask::open_embedder(no_dense, &model, threads, index::embed::Weights::Mapped) {
                    let mut idx = index::dense::DenseIndex::load(&w.store)?;
                    let questions = enrich::Questions::load(&w.store)?;
                    idx.written_by(&model, e.dim()?);
                    embedded = idx.sync(&w.graph, &questions, &mut |texts| e.embed(texts))?;
                    idx.synced_against(w.graph_at);
                    // Saved with nothing embedded as well: the refresh moved the graph, and the
                    // claim is what tells the next reader it owes no rows.
                    idx.save(&w.store)?;
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

pub(crate) fn extractors(repo: &std::path::Path) -> anyhow::Result<Extractors> {
    let resolver = code::imports::Resolver::new(repo)?;
    Ok(Extractors {
        doc: Box::new(doc::DocExtractor::new()),
        code: Box::new(code::CodeExtractor::new(resolver)),
        registry: Box::new(doc::registry::RegistryExtractor),
    })
}

/// One number bounding every thread pool this process owns. The ONNX sessions take it through
/// `session_builder`; rayon takes it here, since `tokenizers::encode_batch` fans a batch out over
/// the global pool — twelve threads on this machine — behind the embedder's back. The error is
/// the pool having been built already, which is what a test binary sharing one process, or
/// `bench` reaching this after `main` did, look like: there is nothing to do about it and nothing
/// worth saying.
pub(crate) fn cap_pools(threads: usize) {
    let _ = rayon::ThreadPoolBuilder::new().num_threads(threads).build_global();
}

/// Rows embedded between one checkpoint and the next. A whole store on the default model is
/// about a minute a chunk here, which is both what an interrupted run loses and how often the
/// reader is told where it is; the checkpoint itself is an append of the chunk's bytes plus the
/// metadata rewrite, so paying it thirty-odd times over a rebuild is not measurable against the
/// forwards.
/// A checkpoint every ~600k characters, ramped in so the first line does not wait for the run's
/// fixed start-up as well: the whole-store rebuild used to print its first at 102.4 s against a
/// 60 s bar, and its last chunks 96.7 s apart, because 1,024 rows is a count and not an amount of
/// work.
const SYNC_CHUNK: index::dense::ChunkBudget = index::dense::ChunkBudget { chars: 600_000, max_rows: 1024, ramp: 16 };

fn embed_all(repo: &std::path::Path, no_dense: bool, cfg: &config::Config) -> anyhow::Result<()> {
    let model = index::embed::resolve(None, &cfg.embed_model);
    let open = std::time::Instant::now();
    let Some(mut emb) = ask::open_embedder(no_dense, &model, index::embed::threads(cfg.resources), index::embed::Weights::Mapped) else { return Ok(()) };
    // The probe belongs to opening: a session with its weights loaded still has to run one forward
    // before it can say how wide a vector is, and the line is printed after it so it reports the
    // wait it exists to explain rather than the part of it that came first.
    let dim = emb.dim()?;
    // The first chunk carries the run's fixed start-up as well as its own work — 2.24 GB of
    // weights paged in as the first forwards touch them — so without this line the first thing a
    // person sees is both, and the 60 s bar is missed before a row is embedded.
    eprintln!("dense: model open in {:.1}s, {dim}-d vectors", open.elapsed().as_secs_f32());
    embed_opened(repo, &model, &mut emb)
}

/// The same rows under a model the caller already has open. `model` switches into this rather
/// than through `embed_all`, because the model it just downloaded and probed is the model the
/// rows are owed to, and opening a second session of it would cost the reader a second wait for
/// nothing.
fn embed_opened(repo: &std::path::Path, model: &str, emb: &mut index::embed::Embedder) -> anyhow::Result<()> {
    let store = store::Store::new(repo);
    let graph_at = store.stamp("graph.json");
    let (graph, _) = store.load()?;
    let questions = enrich::Questions::load(&store)?;
    let mut dense = index::dense::DenseIndex::load(&store)?;
    let t = std::time::Instant::now();
    dense.written_by(model, emb.dim()?);
    let n = dense.sync_chunked(&graph, &questions, &mut |texts| emb.embed(texts), SYNC_CHUNK, &mut |idx, p| {
        idx.save(&store)?;
        let rate = p.done as f32 / t.elapsed().as_secs_f32().max(f32::EPSILON);
        eprintln!("dense: {}/{} rows, {rate:.1} rows/s, ~{:.0} min left", p.done, p.total, (p.total - p.done) as f32 / rate / 60.0);
        Ok(())
    })?;
    dense.synced_against(graph_at);
    dense.save(&store)?;
    println!("dense: embedded {n} rows in {:.1}s", t.elapsed().as_secs_f32());
    Ok(())
}

/// What the store is on, what the file asks for, and what else has been measured. The store's own
/// name is read from `vectors.json` rather than taken from the configuration: a store answers with
/// the model that wrote it, so the two disagreeing is the thing a reader most needs told, and it is
/// exactly what a configuration-shaped report would hide.
fn model_report(repo: &std::path::Path, cfg: &config::Config) -> anyhow::Result<String> {
    let store = store::Store::new(repo);
    let recorded = index::dense::DenseIndex::recorded_model(&store)?;
    let mut out = String::new();
    let mut say = |key: &str, value: String| out.push_str(&format!("{key:<12}{value}\n"));
    match &recorded {
        Some(m) => say("store", m.clone()),
        None => say("store", "no vectors yet — `repograph build` writes them".to_string()),
    }
    let from = match config::names_embed_model(repo) { true => config::PROJECT_FILE, false => "the built-in default" };
    say("configured", format!("{} ({from})", cfg.embed_model));
    if let Some(o) = std::env::var("REPOGRAPH_EMBED_MODEL").ok().filter(|v| !v.trim().is_empty()) {
        say("this run", format!("{o} (REPOGRAPH_EMBED_MODEL)"));
    }
    if recorded.as_deref().is_some_and(|m| m != cfg.embed_model) {
        out.push_str("\nThe two disagree. The store keeps answering with the model that wrote it; the\n\
            configured one takes effect at the next `build`, `update`, `enrich`, `embed` or\n\
            `watch`, which drops every row the other model wrote and rewrites the index whole.\n");
    }
    out.push('\n');
    out.push_str(&index::embed::catalogue(recorded.as_deref().or(Some(&cfg.embed_model))));
    out.push_str("\n`repograph model <hub id>` downloads one, writes the key and re-embeds.\n");
    Ok(out)
}

/// The same three facts for a program, and whether they agree. The catalogue is left out: those
/// figures are a document's, and a caller that wants them is better served by the document than by
/// a copy of it that ages inside a parser.
fn model_json(repo: &std::path::Path, cfg: &config::Config) -> anyhow::Result<String> {
    let store = store::Store::new(repo);
    let recorded = index::dense::DenseIndex::recorded_model(&store)?;
    let agrees = recorded.as_deref().is_none_or(|m| m == cfg.embed_model);
    Ok(serde_json::json!({
        "store": recorded,
        "configured": cfg.embed_model,
        "configured_from": match config::names_embed_model(repo) { true => config::PROJECT_FILE, false => "default" },
        "this_run": std::env::var("REPOGRAPH_EMBED_MODEL").ok().filter(|v| !v.trim().is_empty()),
        "agrees": agrees,
        "recommended": index::embed::RECOMMENDED,
    }).to_string() + "\n")
}

/// The spelling of a hub id to write down: the catalogue's where it holds a row, else the store's
/// own where the store already stands on this model. `written_by` compares model names byte for
/// byte, so `BAAI/BGE-M3` and `BAAI/bge-m3` are two models to every later `update` and the
/// difference costs a whole re-embed nobody asked for — a case the catalogue cannot canonicalise
/// for a model it does not list.
fn spelling<'a>(id: &'a str, recorded: Option<&'a str>) -> &'a str {
    let id = index::embed::measured(id).map_or(id, |m| m.model);
    match recorded {
        Some(m) if m.eq_ignore_ascii_case(id) => m,
        _ => id,
    }
}

/// The switch: the model is opened before anything at all is written.
///
/// That order is the whole contract. A typo, an id whose repository has no ONNX export, a machine
/// with no network — each has to leave the project exactly as it was, and the only thing that can
/// say a model is really there is opening it and asking it for a vector. So the fetch, the session
/// and one probe come first and the hub's own error is what a reader sees; only then is
/// `repograph.toml` touched, and only then are the rows rewritten. There is no rollback for a
/// configuration file, so nothing is written that might have to be taken back.
fn switch_model(repo: &std::path::Path, cfg: &config::Config, id: &str, no_embed: bool, no_dense: bool) -> anyhow::Result<()> {
    if no_dense {
        anyhow::bail!("--no-dense and `model {id}` ask for opposite things: the switch opens the model before it writes anything");
    }
    let store = store::Store::new(repo);
    let recorded = index::dense::DenseIndex::recorded_model(&store)?;
    let id = spelling(id, recorded.as_deref());
    let (was, now) = (recorded.as_deref().and_then(index::embed::measured), index::embed::measured(id));
    eprintln!("model: {} → {id}", recorded.as_deref().unwrap_or(&cfg.embed_model));
    match now {
        Some(m) => {
            eprintln!("model: {} of files in the hub cache, fetched once", m.cache);
            eprintln!("model: {:.1}× the default's embed — {} s for the bench fixture's 33,525 rows", m.times_the_default(), m.embed_s);
            match was.map(|w| w.vectors_mb) {
                Some(mb) if mb != m.vectors_mb => eprintln!("model: {} MB of vectors for that corpus, against the other model's {mb} MB on it", m.vectors_mb),
                _ => eprintln!("model: {} MB of vectors for that corpus", m.vectors_mb),
            }
        }
        None => {
            eprintln!("model: this catalogue holds no row for that id, so its size, its embed cost");
            eprintln!("       and its recall are unknown here — docs/bench/2026-09-22-embedders-results.md");
            eprintln!("       reads further models the catalogue does not list");
            if index::embed::reads_as_e5(id) {
                eprintln!("model: and nothing here recognises the id, so it is read with e5's pooling and");
                eprintln!("       prefixes — a wrong guess there costs recall rather than failing");
            }
        }
    }
    // Any change of model rewrites the index rather than extending it — rows another model wrote
    // are dropped on the name as much as on the width — and a change of width is the case where
    // that is most expensive and least visible. Said only where there are such rows: an empty
    // store and a switch back to the model that wrote it both lose nothing, and a warning about
    // work that will not happen is a reason to stop and check.
    match recorded.as_deref() {
        None => eprintln!("model: the store holds no vectors yet, so this is the model that writes its first"),
        Some(m) if m.eq_ignore_ascii_case(id) => eprintln!("model: the store's rows were written by this model already, so they are kept"),
        Some(_) => match (was.map(|m| m.dim), now.map(|m| m.dim)) {
            (Some(a), Some(b)) if a != b => eprintln!("model: {a}-d → {b}-d, so every row is re-embedded and the whole index rewritten, not extended"),
            _ => eprintln!("model: every row the other model wrote is dropped and the index rewritten, not extended"),
        },
    }
    let threads = index::embed::threads(cfg.resources);
    let open = std::time::Instant::now();
    let mut emb = index::embed::Embedder::open(id, threads, index::embed::Weights::Mapped)?;
    // A session that opens still has to run: the pooling, the prefixes and the empty cache inputs
    // a decoder export wants are this binary's guesses about the model, and one forward is what
    // turns them into a fact before the configuration is changed on the strength of them.
    let dim = emb.dim()?;
    eprintln!("model: opened in {:.1}s, {dim}-d vectors", open.elapsed().as_secs_f32());
    let path = config::set_embed_model(repo, id)?;
    println!("model: embed_model = \"{id}\" in {}", path.display());
    if no_embed {
        eprintln!("model: --no-embed, so the store still holds the old rows — `repograph embed` rewrites them");
        return Ok(());
    }
    // The same check `embed` makes, and for the same reason: a sync against an empty graph marks
    // every row dead and writes an index of nothing. The key is already set, which is the part
    // that had to survive, so this is a next step rather than a failure.
    if store.load()?.0.nodes.is_empty() {
        eprintln!("model: the graph is empty, so there is nothing to re-embed — run `repograph build`");
        return Ok(());
    }
    embed_opened(repo, id, &mut emb)
}

/// The graph an answer is read from: refreshed against the tree unless `--stale`, and, when
/// the store cannot be written, the stored one with a warning — the same contract as `ask`.
fn graph_for(repo: &std::path::Path, cfg: &config::Config, stale: bool) -> anyhow::Result<model::Graph> {
    let timing = ask::Timing::new();
    let store = store::Store::new(repo);
    match ask::graph_for_ask(repo, cfg, &store, stale, &timing) {
        Ok((graph, refreshed)) => {
            if let Some(r) = refreshed { eprintln!("refresh: {} changed, {} removed", r.changed, r.removed); }
            Ok(graph)
        }
        Err(err) => { eprintln!("refresh: skipped ({err:#})"); Ok(store.load()?.0) }
    }
}

/// The path as a user would write it. `canonicalize` on Windows answers in the `\\?\C:\…` form,
/// which std puts back itself wherever a call needs it; carried around instead it is four bytes
/// of the socket name's budget and a prefix in every message.
#[cfg(windows)]
fn plain(p: PathBuf) -> PathBuf {
    let Some(s) = p.to_str() else { return p };
    match s.strip_prefix(r"\\?\") {
        Some(rest) if rest.starts_with(r"UNC\") => PathBuf::from(format!(r"\\{}", &rest[4..])),
        Some(rest) => PathBuf::from(rest),
        None => p,
    }
}

#[cfg(unix)]
fn plain(p: PathBuf) -> PathBuf { p }

/// A determination the command was asked to make and made: no call path within the depth, a
/// bench suite that answered every case and missed a floor. Carried to `main` as an error because
/// it ends the command, and printed there as the sentence it is, on exit status 3 — where 1 stays
/// a failure to make a determination at all: no such symbol, no store to read.
///
/// 3 and not 2, which is the code the ledger's lever named first: 2 is already taken twice on the
/// way to this binary. `clap` exits 2 on a usage error, so `repograph bench --nosuchflag` — a
/// command that never ran — is indistinguishable from a suite that ran and missed a floor; and
/// `npm/repograph/bin/repograph.js` exits 2 when no platform binary is installed, which CI pins.
/// A verdict has to be a status no other layer writes, or a harness reading it learns nothing.
#[derive(Debug)]
struct Verdict(String);

impl std::fmt::Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) }
}

impl std::error::Error for Verdict {}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        // `{e:?}` is what `Termination for Result` printed before this function existed: an
        // anyhow report with its context chain, which several transcripts are read for.
        Err(e) => match e.downcast_ref::<Verdict>() {
            Some(v) => { eprintln!("{v}"); std::process::ExitCode::from(3) }
            None => { eprintln!("Error: {e:?}"); std::process::ExitCode::FAILURE }
        },
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let repo = plain(cli.repo.canonicalize()?);
    // Loaded per command: `bench` reads its own from `REPOGRAPH_BENCH_REPO`, and `explain`/`verify`
    // must not fail on a broken `repograph.toml` they never read. `embed` does read it — the model
    // the vectors are written with lives there — so it fails on a broken one like the other writers.
    let load_cfg = || config::Config::load(&repo);
    let wipe = matches!(cli.cmd, Cmd::Build);
    match cli.cmd {
        Cmd::Build | Cmd::Update => {
            let cfg = load_cfg()?;
            cap_pools(index::embed::threads(cfg.resources));
            let r = run_update(&repo, &cfg, wipe)?;
            println!("changed {} removed {} nodes {} edges {}", r.changed, r.removed, r.nodes, r.edges);
            if let Some(n) = r.unenriched {
                eprintln!("repograph: {n} requirement-like nodes have no questions — run `repograph enrich` to search them");
            }
            embed_all(&repo, cli.no_dense, &cfg)
        }
        Cmd::Enrich { batch, parallel, limit, code, detect_languages, keep_raw } => {
            let cfg = load_cfg()?;
            if let Some(why) = cfg.refusal("enrich_command") { anyhow::bail!(why); }
            cap_pools(index::embed::threads(cfg.resources));
            let store = store::Store::new(&repo);
            let (graph, _) = store.load()?;
            if graph.nodes.is_empty() { anyhow::bail!("graph is empty — run `repograph build`"); }
            let questions = enrich::Questions::load(&store)?;
            // Said before the run because it is the one thing about a 20-minute pass that cannot
            // be read back off the result: questions in the wrong language look like questions.
            let (languages, where_from) = match cfg.enrich_languages.is_empty() {
                false if cfg.enrich_languages_from_env => (cfg.enrich_languages.clone(), "REPOGRAPH_ENRICH_LANGUAGES"),
                false => (cfg.enrich_languages.clone(), "repograph.toml"),
                true => enrich::languages_for(&questions, &graph, detect_languages),
            };
            eprintln!("enrich: questions in {} ({where_from})", languages.join(", "));
            let keep = keep_raw.or_else(|| std::env::var_os("REPOGRAPH_ENRICH_KEEP").map(Into::into));
            let t = std::time::Instant::now();
            let r = enrich::run(&store, &graph, questions, &cfg.enrich_command, batch, parallel, enrich::Scope { limit, code, keep }, &languages)?;
            println!("enrich: {} nodes written, {} dropped, {} still without questions, {} batches ({} failed) in {:.0}s", r.generated, r.dropped, r.left, r.batches, r.failed, t.elapsed().as_secs_f32());
            // A run that was asked to write and wrote nothing has to exit like one, or a campaign
            // grades a store nobody enriched. `left > 0` is not the condition: a store legitimately
            // keeps nodes the model declines, and every honest run would then be red.
            if r.failed > 0 && r.generated == 0 {
                anyhow::bail!("enrich: {} of {} batches produced nothing — the generator did not answer", r.failed, r.batches);
            }
            embed_all(&repo, cli.no_dense, &cfg)
        }
        Cmd::Embed => {
            let cfg = load_cfg()?;
            cap_pools(index::embed::threads(cfg.resources));
            // The one command that reaches `embed_all` without having just written the graph
            // itself, so the check is here rather than in it: a sync against an empty graph
            // marks every row dead and saves an index of nothing, and run before the first
            // `build` it writes a `vectors.*` pair that makes `DenseIndex::present` true over
            // no rows. `build`, `update` and `enrich` over a tree that yields nothing keep
            // writing their empty graph and exiting 0.
            let (graph, _) = store::Store::new(&repo).load()?;
            if graph.nodes.is_empty() { anyhow::bail!("graph is empty — run `repograph build`"); }
            if cli.no_dense {
                // Otherwise this exits 0 having printed nothing at all, which reads exactly
                // like an embed that found every row already in place.
                println!("dense: nothing embedded, --no-dense is set");
                Ok(())
            } else {
                embed_all(&repo, cli.no_dense, &cfg)
            }
        }
        Cmd::Model { hub_id, no_embed, json } => {
            let cfg = load_cfg()?;
            cap_pools(index::embed::threads(cfg.resources));
            match hub_id {
                None if json => { print!("{}", model_json(&repo, &cfg)?); Ok(()) }
                None => { print!("{}", model_report(&repo, &cfg)?); Ok(()) }
                Some(id) => switch_model(&repo, &cfg, &id, no_embed, cli.no_dense),
            }
        }
        Cmd::Ask { words, json, seeds, bodies, rerank, rerank_local, depth, stale, no_serve } => {
            // Before the socket, not after it: a broken `repograph.toml` is the one thing a
            // resident process would hide, and a TOML parse is nothing against a process start.
            let cfg = load_cfg()?;
            cap_pools(index::embed::threads(cfg.resources));
            let req = ask::Request { words, json, seeds, bodies, rerank, rerank_local, depth, stale, no_dense: cli.no_dense };
            // `bench` and `dump` build their own contexts and never reach this; the environment
            // variable is for everything else that must be measured against a cold process.
            let resident = if no_serve || std::env::var_os("REPOGRAPH_NO_SERVE").is_some() { None } else { serve::try_ask(&repo, &req) };
            if let Some(reply) = resident {
                for n in reply.stderr { eprintln!("{n}"); }
                eprintln!("serve: answered by the resident process");
                print!("{}", reply.stdout);
                use std::io::Write;
                std::io::stdout().flush()?;
                std::process::exit(0)
            }
            let mut ctx = ask::Context::open(&repo, &cfg, req.stale, cli.no_dense)?;
            let text = ctx.answer(&req)?;
            // Before the answer: a refresh line reached the reader ahead of it back when it was
            // printed the moment it happened, and that is the order a human reads.
            for n in ctx.notices() { eprintln!("{n}"); }
            print!("{text}");
            // Nothing here is written back, and unwinding a 1.3 GB model session plus the graph
            // costs a fused answer a measurable share of its wall time: leave without it.
            use std::io::Write;
            std::io::stdout().flush()?;
            ctx.timing().stage("printed");
            std::process::exit(0)
        }
        Cmd::Serve { every, batch, idle, idle_model } => {
            let cfg = load_cfg()?;
            cap_pools(index::embed::threads(cfg.resources));
            serve::run(&repo, &cfg, every, batch, idle, idle_model, cli.no_dense)
        }
        Cmd::Watch { every, batch } => {
            let cfg = load_cfg()?;
            cap_pools(index::embed::threads(cfg.resources));
            run_watch(&repo, &cfg, every, batch, cli.no_dense)
        }
        Cmd::Explain { node, json } => {
            let (graph, _) = store::Store::new(&repo).load()?;
            let rendered = match json {
                true => query::explain_json(&graph, &node).map(|j| format!("{j}\n")),
                false => query::explain(&graph, &node),
            };
            match rendered {
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
        Cmd::Trace { from, to, depth, json, stale } => {
            let graph = graph_for(&repo, &load_cfg()?, stale)?;
            let Some(a) = query::resolve(&graph, &from) else { anyhow::bail!("no node matches {from}") };
            let Some(b) = query::resolve(&graph, &to) else { anyhow::bail!("no node matches {to}") };
            let found = impact::trace(&graph, &a.id, &b.id, depth);
            // No path within the depth is an answer to the question that was asked, so the JSON
            // form says so and exits 0 where the text form exits 3. A caller parsing JSON should
            // not have to read an exit code to learn what the object already says, and a `null`
            // path is easier to handle than a non-zero exit with no object.
            if json {
                println!("{}", impact::trace_json(&graph, &a.id, &b.id, depth, found.as_deref()));
                return Ok(());
            }
            match found {
                Some(path) => {
                    for (i, id) in path.iter().enumerate() {
                        let at = graph.nodes.get(id).map(|n| format!("{}:{}", n.file, n.line)).unwrap_or_default();
                        println!("{}{id}  {at}", if i == 0 { "" } else { "  → " });
                    }
                    Ok(())
                }
                None => Err(Verdict(format!("no call path from {} to {} within {depth} hops", a.id, b.id)).into()),
            }
        }
        Cmd::Changes { base, depth, json, stale } => {
            let graph = graph_for(&repo, &load_cfg()?, stale)?;
            let hunks = changes::hunks_from_git(&repo, &base)?;
            let r = changes::report(&graph, &hunks, depth);
            print!("{}", if json { changes::render_json(&graph, &r) } else { changes::render(&graph, &r) });
            Ok(())
        }
        Cmd::Verify { json } => {
            let (graph, _) = store::Store::new(&repo).load()?;
            match json {
                true => println!("{}", query::verify_json(&graph)),
                false => print!("{}", query::verify(&graph)),
            }
            if graph.nodes.is_empty() { anyhow::bail!("graph is empty — run `repograph build`"); }
            Ok(())
        }
        Cmd::InstallAgent { claude, codex, command } => {
            let targets: Vec<install_agent::Target> = match (claude, codex) {
                (false, false) => anyhow::bail!("name a harness: --claude, --codex, or both"),
                (c, x) => [(c, install_agent::Target::Claude), (x, install_agent::Target::Codex)]
                    .into_iter().filter(|(on, _)| *on).map(|(_, t)| t).collect(),
            };
            // The languages key is read out of `repograph.toml` and written back to it, so a file
            // this binary cannot read costs the line and not the install.
            match config::Config::load(&repo) {
                Ok(cfg) => if let Some(l) = install_agent::set_languages(&repo, &cfg)? {
                    println!("repograph.toml: enrich_languages = {l:?} (detected from the documents; edit the line to change)");
                },
                Err(err) => eprintln!("install-agent: repograph.toml was not read ({err:#}); enrich_languages left unset"),
            }
            for target in targets {
                let r = install_agent::install(&repo, target, &command)?;
                match r.written {
                    0 => println!("{target:?}: already installed, nothing written"),
                    n => println!("{target:?}: wrote {n} files\n  {}", r.paths.join("\n  ")),
                }
                for note in &r.notes { println!("{target:?}: {note}"); }
                // Codex reads its hooks from `~/.codex/hooks.json`, which is the machine's and not
                // this repository's. Printed for a person to paste; see `agent/codex.md`.
                if target == install_agent::Target::Codex {
                    let hook = repo.join(".codex/hooks/repograph-hook.mjs");
                    let abs = hook.canonicalize().unwrap_or(hook);
                    println!("\nTo run the hook on this machine, add to ~/.codex/hooks.json \
                              (merging with what is already there):\n{}\n\
                              Codex trusts a hook by hash: the first session after this is added \
                              asks once, and editing the script later asks again. The hook reads \
                              each session's own working directory and stays silent where there is \
                              no index, so one copy serves every repository — move it to \
                              ~/.codex/hooks/ and adjust the path if this checkout may go away.",
                             install_agent::codex_hooks_block(&abs.display().to_string()));
                }
            }
            Ok(())
        }
        Cmd::Prime { json } => {
            let store = store::Store::new(&repo);
            let (graph, _) = store.load()?;
            if graph.nodes.is_empty() { anyhow::bail!("graph is empty — run `repograph build`"); }
            let questions = enrich::Questions::load(&store)?;
            let families = families::of_graph(&graph).0.len();
            let model = index::dense::DenseIndex::recorded_model(&store)?;
            let b = prime::brief(&graph, &questions, families, model.as_deref());
            match json {
                true => println!("{}", b.json()),
                false => print!("{}", b.text()),
            }
            Ok(())
        }
        Cmd::Families { json } => families::run(&repo, &load_cfg()?, json),
        Cmd::Bench { cases, rerank, rerank_local, depth, repeat } => {
            let mut summaries = Vec::new();
            let mut met = true;
            for _ in 0..repeat.max(1) {
                let (ok, summary) = bench::run(&repo, cases.as_deref(), cli.no_dense, rerank, rerank_local, depth)?;
                met &= ok;
                summaries.push(summary);
            }
            if summaries.len() > 1 {
                let m = bench::median(&summaries);
                let counts = m.by_kind.iter().map(|(k, (h, t))| format!("{k} {h}/{t}")).collect::<Vec<_>>().join("  ");
                println!("\nmedian of {}  {counts}  p90 {} tok\n{}", summaries.len(), m.p90_tokens, bench::anchor_line(&m));
            }
            // Every run has to meet the floors, not the median of them: a suite that passes on
            // average is one whose exit code depends on which run a reader looked at.
            if met { Ok(()) } else { Err(Verdict("bench floors not met".to_string()).into()) }
        }
        Cmd::Dump { queries, out, depth } => dump::run(&repo, &queries, &out, depth, cli.no_dense),
        Cmd::ImportLegacy { graph_json } => {
            let store = store::Store::new(&repo);
            let (mut graph, manifest) = store.load()?;
            let text = std::fs::read_to_string(&graph_json)?;
            let r = legacy::import(&mut graph, &text)?;
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

    #[test]
    fn cap_pools_tolerates_a_pool_already_built() {
        cap_pools(2);
        cap_pools(2);
    }

    const ONE: &str = "# A\n\n**FR-PAY-22 · MUST · cancellation window**\n\nbody\n";
    const TWO: &str = "# A\n\n**FR-PAY-22 · MUST · cancellation window**\n\nbody\n\n**FR-PAY-23 · MUST · refund window**\n\nbody\n";

    fn built(repo: &std::path::Path, cfg: &config::Config) {
        run_update(repo, cfg, true).unwrap();
    }

    /// The report exists to say the one thing a configuration-shaped answer would hide: a store
    /// keeps answering with the model that wrote it, whatever the file says today. So where the
    /// store's name comes from, and what is printed when the two differ, are the report.
    #[test]
    fn the_report_reads_the_store_and_says_when_the_file_asks_for_another_model() {
        let dir = doc_repo(ONE);
        let repo = dir.path();
        let quiet = model_report(repo, &config::Config::default()).unwrap();
        assert!(quiet.contains("no vectors yet"), "{quiet}");
        assert!(quiet.contains("the built-in default"), "{quiet}");
        assert!(!quiet.contains("The two disagree"), "{quiet}");

        config::set_embed_model(repo, "BAAI/bge-m3").unwrap();
        let store = store::Store::new(repo);
        let mut dense = index::dense::DenseIndex::load(&store).unwrap();
        dense.written_by(index::embed::DEFAULT_MODEL, 384);
        dense.save(&store).unwrap();
        let cfg = config::Config { embed_model: "BAAI/bge-m3".to_string(), ..config::Config::default() };
        let split = model_report(repo, &cfg).unwrap();
        assert!(split.contains(index::embed::DEFAULT_MODEL), "the store's own model: {split}");
        assert!(split.contains(&format!("BAAI/bge-m3 ({})", config::PROJECT_FILE)), "{split}");
        assert!(split.contains("The two disagree"), "{split}");
    }

    #[test]
    fn an_ask_after_an_edit_answers_from_the_edited_file() {
        let dir = doc_repo(ONE);
        let (repo, cfg) = (dir.path(), config::Config::default());
        built(repo, &cfg);
        std::fs::write(repo.join("docs/a.md"), TWO).unwrap();
        let store = store::Store::new(repo);
        let (graph, refreshed) = ask::graph_for_ask(repo, &cfg, &store, false, &ask::Timing::new()).unwrap();
        let r = refreshed.expect("the edit is a refresh");
        assert_eq!((r.changed, r.removed), (1, 0));
        let opts = query::Options { seeds: 5, bodies: false, dense: false, json: false, depth: rerank::DEPTH };
        let words = ["refund".to_string(), "window".to_string()];
        let answer = query::ask(&graph, &index::lexical::Lexical::build(&graph, &enrich::Questions::default(), false), None, None, &words, &opts);
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
        let (graph, refreshed) = ask::graph_for_ask(repo, &cfg, &store::Store::new(repo), false, &ask::Timing::new()).unwrap();
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
        // The grammar is this build's: what is missing here is the stamps, and a store behind on
        // both would be re-read for the other reason.
        store.save_manifest(&walk::Manifest { files, stamps: Default::default(), grammar: walk::GRAMMAR }).unwrap();
        let graph_before = std::fs::read(repo.join(".repograph/graph.json")).unwrap();
        assert!(ask::graph_for_ask(repo, &cfg, &store, false, &ask::Timing::new()).unwrap().1.is_none());
        let manifest = store.load().unwrap().1;
        assert_eq!(manifest.stamps.len(), manifest.files.len());
        assert_eq!(std::fs::read(repo.join(".repograph/graph.json")).unwrap(), graph_before);
    }

    /// A store an earlier release wrote holds only what the grammar of the day could see, and no
    /// hash says so — the tree has not moved, so nothing here would ever be read again. The stamp
    /// on the manifest is what turns that into one walk, and the walk into the store this build
    /// would have written from the same tree.
    #[test]
    fn a_store_an_older_grammar_wrote_is_re_read_once_and_then_left_alone() {
        let dir = doc_repo(ONE);
        let (repo, cfg) = (dir.path(), config::Config::default());
        std::fs::write(repo.join("docs/b.md"), "# B\n\n**FR-PAY-24 · MUST · chargeback**\n\nbody\n").unwrap();
        built(repo, &cfg);
        let store = store::Store::new(repo);
        // Read as text, not bytes: a failure here is a diff a person has to read.
        let saved = |name: &str| std::fs::read_to_string(repo.join(".repograph").join(name)).unwrap();
        let fresh = (saved("graph.json"), saved("manifest.json"));

        // What such a release leaves: one file's nodes missing from the graph, beside a manifest
        // whose hashes and stamps all match the tree.
        let hole = |grammar: u32| {
            let (mut graph, manifest) = store.load().unwrap();
            graph.remove_file("docs/a.md");
            store.save(&graph, &walk::Manifest { grammar, ..manifest }).unwrap();
        };
        hole(0);
        assert!(!store.load().unwrap().0.nodes.contains_key("FR-PAY-22"));

        let r = run_update(repo, &cfg, false).unwrap();
        assert_eq!((r.changed, r.removed), (0, 0), "the re-read is the grammar's, not the diff's");
        assert_eq!((saved("graph.json"), saved("manifest.json")), fresh,
            "healed to the bytes a build of this version writes, manifest and stamp included");

        // Once: with the stamp current the same hole is left exactly as it is, because a file whose
        // hash has not moved is not read — which is the whole of what the walk above bought.
        hole(walk::GRAMMAR);
        let holed = saved("graph.json");
        assert_ne!(holed, fresh.0);
        run_update(repo, &cfg, false).unwrap();
        assert_eq!(saved("graph.json"), holed);
    }

    /// A file the walk cached and the re-read could not open is a hole in the graph, and the
    /// stamp is what would make it permanent — nothing looks twice at a file whose hash never
    /// moves. Unix only: Windows' read-only flag does not stop a read, so there is no portable
    /// way to shut a file against this process.
    #[cfg(unix)]
    #[test]
    fn a_file_the_grammar_walk_could_not_read_holds_the_stamp_back() {
        use std::os::unix::fs::PermissionsExt;
        let dir = doc_repo(ONE);
        let (repo, cfg) = (dir.path(), config::Config::default());
        std::fs::write(repo.join("docs/b.md"), "# B\n\n**FR-PAY-24 · MUST · chargeback**\n\nbody\n").unwrap();
        built(repo, &cfg);
        let store = store::Store::new(repo);
        let (graph, manifest) = store.load().unwrap();
        // A generation past this build's, so that the value held back below is visibly not the
        // one the store claimed: writing that back would hand the newer build a store it reads as
        // current and will not fill.
        store.save(&graph, &walk::Manifest { grammar: walk::GRAMMAR + 1, ..manifest }).unwrap();
        // Shut after the build, so the stamp still matches and the walk hands back the hash it
        // recorded instead of reading the file and leaving it out.
        let a = repo.join("docs/a.md");
        std::fs::set_permissions(&a, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&a).is_ok() { return; } // root, or a filesystem without modes

        run_update(repo, &cfg, false).unwrap();
        let (graph, manifest) = store.load().unwrap();
        assert!(!graph.nodes.contains_key("FR-PAY-22"), "unread, so its nodes are not in the graph");
        assert_eq!(manifest.grammar, 0, "and the store claims no generation read it whole");

        std::fs::set_permissions(&a, std::fs::Permissions::from_mode(0o644)).unwrap();
        run_update(repo, &cfg, false).unwrap();
        let (graph, manifest) = store.load().unwrap();
        assert!(graph.nodes.contains_key("FR-PAY-22"), "the next writer reads it again, unedited");
        assert_eq!(manifest.grammar, walk::GRAMMAR);
    }

    /// And the holdback is not the grammar walk's alone. A file the walk did not read — its stamp
    /// had not moved — is still re-read here when a co-declarer changes, and the manifest has by
    /// then recorded the changed file's new hash, so no later diff names anything: without the
    /// stamp held back this hole is the permanent one. Unix only, for the reason above.
    #[cfg(unix)]
    #[test]
    fn an_unread_co_declarer_holds_the_stamp_back_on_a_store_that_was_current() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (repo, cfg) = (dir.path(), config::Config::default());
        std::fs::create_dir_all(repo.join("docs")).unwrap();
        std::fs::write(repo.join("docs/a.md"), "# A\n\n**FR-PAY-22 · MUST · x in a**\n\nbody\n").unwrap();
        std::fs::write(repo.join("docs/b.md"), "# B\n\n**FR-PAY-22 · MUST · x in b**\n\n**FR-PAY-30 · MUST · only in b**\n\nbody\n").unwrap();
        built(repo, &cfg);
        let store = store::Store::new(repo);
        assert_eq!(store.load().unwrap().1.grammar, walk::GRAMMAR, "nothing here is grammar-stale");

        let b = repo.join("docs/b.md");
        std::fs::set_permissions(&b, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&b).is_ok() { return; } // root, or a filesystem without modes
        // `a.md` declares the shared id first and is its primary, so editing it re-reads `b.md`.
        std::fs::write(repo.join("docs/a.md"), "# A\n\n**FR-PAY-22 · MUST · x in a, edited**\n\nbody\n").unwrap();

        run_update(repo, &cfg, false).unwrap();
        let (graph, manifest) = store.load().unwrap();
        assert!(!graph.nodes.contains_key("FR-PAY-30"), "b.md was dropped and could not be read back");
        assert_eq!(manifest.grammar, 0, "so the store stops claiming any generation read it whole");

        std::fs::set_permissions(&b, std::fs::Permissions::from_mode(0o644)).unwrap();
        run_update(repo, &cfg, false).unwrap();
        let (graph, manifest) = store.load().unwrap();
        assert!(graph.nodes.contains_key("FR-PAY-30"), "and the held-back stamp is what recovers it");
        assert_eq!(manifest.grammar, walk::GRAMMAR);
    }

    #[test]
    fn stale_answers_from_the_store_and_leaves_it_untouched() {
        let dir = doc_repo(ONE);
        let (repo, cfg) = (dir.path(), config::Config::default());
        built(repo, &cfg);
        let before = std::fs::read(repo.join(".repograph/graph.json")).unwrap();
        std::fs::write(repo.join("docs/a.md"), TWO).unwrap();
        let (graph, refreshed) = ask::graph_for_ask(repo, &cfg, &store::Store::new(repo), true, &ask::Timing::new()).unwrap();
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
        let r = run_update(repo, &cfg, true).unwrap();
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
        run_update(repo, &cfg, true).unwrap();
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
        run_update(repo, &cfg, true).unwrap();
        std::fs::remove_file(repo.join("docs/a.md")).unwrap();
        run_update(repo, &cfg, false).unwrap();
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
        run_update(repo, &cfg, true).unwrap();
        std::fs::remove_file(repo.join("docs/a.md")).unwrap();
        run_update(repo, &cfg, false).unwrap();
        let (graph, _) = store::Store::new(repo).load().unwrap();
        let y = &graph.nodes["FR-PAY-20"];
        assert_eq!((y.file.as_str(), y.line, y.label.as_str()), ("docs/b.md", 5, "y in b"));
    }

    /// A user's path, and the four bytes the socket name gets back. std re-applies the prefix
    /// inside the calls that need it, which the metadata assertion is here to show.
    #[cfg(windows)]
    #[test]
    fn a_canonical_path_is_stated_without_its_verbatim_prefix() {
        use std::path::PathBuf;
        assert_eq!(super::plain(PathBuf::from(r"\\?\C:\a\b")), PathBuf::from(r"C:\a\b"));
        assert_eq!(super::plain(PathBuf::from(r"\\?\UNC\srv\share\x")), PathBuf::from(r"\\srv\share\x"));
        assert_eq!(super::plain(PathBuf::from(r"C:\a\b")), PathBuf::from(r"C:\a\b"));
        let temp = super::plain(std::env::temp_dir().canonicalize().unwrap());
        assert!(!temp.to_string_lossy().starts_with(r"\\?\"), "{}", temp.display());
        assert!(std::fs::metadata(&temp).unwrap().is_dir(), "the stripped path still names the directory");
    }
    /// The id that gets written down is the one the store's rows already carry, because a switch
    /// that reports the rows kept and then drops them for a difference of case is the expensive
    /// half of a re-embed nobody asked for.
    #[test]
    fn the_id_written_down_is_the_one_the_rows_already_carry() {
        let arctic = index::embed::RECOMMENDED;
        assert_eq!(spelling("snowflake/SNOWFLAKE-ARCTIC-EMBED-L-V2.0", None), arctic, "the catalogue's row canonicalises its own id");
        assert_eq!(spelling("baai/BGE-M3", Some("BAAI/bge-m3")), "BAAI/bge-m3", "a model the catalogue does not list keeps the store's spelling");
        assert_eq!(spelling("BAAI/bge-m3", None), "BAAI/bge-m3", "nothing to match, so the id stands as typed");
        assert_eq!(spelling("BAAI/bge-m3", Some(arctic)), "BAAI/bge-m3", "another model's rows do not lend their spelling");
    }

}
