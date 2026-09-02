mod bench;
mod code;
mod config;
mod doc;
mod dump;
mod enrich;
mod rerank;
mod ids;
mod index;
mod legacy;
mod model;
mod query;
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
        /// Candidates the reranking model is shown; tokens per question grow with it.
        #[arg(long, default_value_t = rerank::DEPTH)] depth: usize,
        /// Answers from the store as it stands, without bringing it in line with the tree first.
        #[arg(long)] stale: bool,
    },
    Explain { node: String },
    Verify,
    /// Writes reader questions for every requirement-like node through the configured
    /// command, then re-embeds. Costs model tokens once per passage; nothing per query.
    Enrich {
        #[arg(long, default_value_t = 12)] batch: usize,
        #[arg(long, default_value_t = 8)] parallel: usize,
        #[arg(long)] limit: Option<usize>,
    },
    Bench { #[arg(long)] cases: Option<PathBuf>, #[arg(long)] rerank: bool, #[arg(long, default_value_t = rerank::DEPTH)] depth: usize },
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
    let (mut graph, manifest) = store.load()?;
    timing.stage("graph loaded");
    if stale { return Ok((graph, None)); }
    let entries = walk::walk(repo, cfg, &manifest)?;
    let diff = manifest.diff(&entries);
    timing.stage("tree walked");
    if diff.changed.is_empty() && diff.removed.is_empty() {
        record_stamps(store, &manifest, &entries)?;
        return Ok((graph, None));
    }
    let r = apply_diff(repo, store, &mut graph, &entries, &diff, &extractors(repo, cfg)?)?;
    timing.stage("refreshed");
    Ok((graph, Some(r)))
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

impl Timing {
    fn new() -> Timing {
        let now = std::time::Instant::now();
        Timing { on: std::env::var_os("REPOGRAPH_TIMING").is_some(), start: now, last: std::cell::Cell::new(now) }
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
        Cmd::Enrich { batch, parallel, limit } => {
            let cfg = load_cfg()?;
            let store = store::Store::new(&repo);
            let (graph, _) = store.load()?;
            if graph.nodes.is_empty() { anyhow::bail!("graph is empty — run `repograph build`"); }
            let questions = enrich::Questions::load(&store)?;
            let t = std::time::Instant::now();
            let r = enrich::run(&store, &graph, questions, &cfg.enrich_command, batch, parallel, limit)?;
            println!("enrich: {} nodes written, {} dropped, {} still without questions, {} batches ({} failed) in {:.0}s", r.generated, r.dropped, r.left, r.batches, r.failed, t.elapsed().as_secs_f32());
            embed_all(&repo, cli.no_dense)
        }
        Cmd::Ask { words, json, seeds, bodies, rerank, depth, stale } => {
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
            let questions = enrich::Questions::load(&store)?;
            timing.stage("ids and questions ready");
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
            let rerank: Option<query::Rerank> = if rerank { Some(&rerank_fn) } else { None };
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
        Cmd::Explain { node } => {
            let (graph, _) = store::Store::new(&repo).load()?;
            match query::explain(&graph, &node) {
                Some(s) => { print!("{s}"); Ok(()) }
                None => anyhow::bail!("no node matches {node}"),
            }
        }
        Cmd::Verify => {
            let (graph, _) = store::Store::new(&repo).load()?;
            print!("{}", query::verify(&graph));
            if graph.nodes.is_empty() { anyhow::bail!("graph is empty — run `repograph build`"); }
            Ok(())
        }
        Cmd::Bench { cases, rerank, depth } => {
            if bench::run(&repo, cases.as_deref(), cli.no_dense, rerank, depth)? { Ok(()) } else { anyhow::bail!("bench floors not met") }
        }
        Cmd::Dump { queries, out, depth } => dump::run(&repo, &queries, &out, depth),
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
