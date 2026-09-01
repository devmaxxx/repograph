mod bench;
mod code;
mod config;
mod doc;
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
        words: Vec<String>,
        #[arg(long)] json: bool,
        #[arg(long, default_value_t = 5)] seeds: usize,
        #[arg(long)] bodies: bool,
    },
    Explain { node: String },
    Verify,
    Bench { #[arg(long)] cases: Option<PathBuf> },
    ImportLegacy { graph_json: PathBuf },
}

pub struct Extractors {
    pub doc: Box<dyn Extractor>,
    pub code: Box<dyn Extractor>,
    pub registry: Box<dyn Extractor>,
}

pub struct UpdateReport { pub changed: usize, pub removed: usize, pub nodes: usize, pub edges: usize }

pub fn run_update(repo: &std::path::Path, cfg: &config::Config, ex: &Extractors, wipe: bool) -> anyhow::Result<UpdateReport> {
    let store = store::Store::new(repo);
    if wipe { store.wipe()?; }
    let (mut graph, manifest) = store.load()?;
    let entries = walk::walk(repo, cfg)?;
    let diff = manifest.diff(&entries);
    for rel in &diff.removed { graph.remove_file(rel); }
    for e in &diff.changed {
        graph.remove_file(&e.rel);
        let text = match std::fs::read(repo.join(&e.rel)) {
            Ok(b) if !b.contains(&0) => String::from_utf8_lossy(&b).into_owned(),
            // A NUL byte means a binary that matched a source glob; skip it, do not fail the run.
            Ok(_) => continue,
            Err(err) => { eprintln!("read {}: {err}", e.rel); continue; }
        };
        let extractor = match e.kind {
            walk::FileKind::Doc => &ex.doc,
            walk::FileKind::Code => &ex.code,
            walk::FileKind::Registry => &ex.registry,
        };
        graph.apply(extractor.extract(&e.rel, &text));
    }
    store.save(&graph, &walk::Manifest::from_entries(&entries))?;
    Ok(UpdateReport { changed: diff.changed.len(), removed: diff.removed.len(), nodes: graph.nodes.len(), edges: graph.edges.len() })
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

fn open_embedder(no_dense: bool) -> Option<index::dense::Embedder> {
    if no_dense { return None; }
    match index::dense::Embedder::open() {
        Ok(e) => Some(e),
        Err(err) => { eprintln!("dense: model unavailable, continuing lexical-only ({err:#})"); None }
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let repo = cli.repo.canonicalize()?;
    let cfg = config::Config::load(&repo)?;
    let wipe = matches!(cli.cmd, Cmd::Build);
    match cli.cmd {
        Cmd::Build | Cmd::Update => {
            let r = run_update(&repo, &cfg, &extractors(&repo, &cfg)?, wipe)?;
            println!("changed {} removed {} nodes {} edges {}", r.changed, r.removed, r.nodes, r.edges);
            if let Some(mut emb) = open_embedder(cli.no_dense) {
                let store = store::Store::new(&repo);
                let (graph, _) = store.load()?;
                let mut dense = index::dense::DenseIndex::load(&store)?;
                let t = std::time::Instant::now();
                let n = dense.sync(&graph, &mut |texts| emb.passages(texts))?;
                dense.save(&store)?;
                println!("dense: embedded {n} nodes in {:.1}s", t.elapsed().as_secs_f32());
            }
            Ok(())
        }
        Cmd::Ask { words, json, seeds, bodies } => {
            let store = store::Store::new(&repo);
            let (graph, _) = store.load()?;
            let ids = ids::IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
            let dense_idx = index::dense::DenseIndex::load(&store)?;
            let embedder = std::cell::RefCell::new(open_embedder(cli.no_dense));
            let dense_fn = |q: &str, k: usize| -> Vec<String> {
                let mut e = embedder.borrow_mut();
                match e.as_mut().and_then(|e| e.query(q).ok()) { Some(v) => dense_idx.search(&v, k), None => Vec::new() }
            };
            let opts = query::Options { seeds, bodies, dense: !cli.no_dense && !dense_idx.ids.is_empty(), json };
            let answer = query::ask(&graph, &ids, Some(&dense_fn), &words, &opts);
            print!("{}", query::render(&answer, &graph, &opts));
            Ok(())
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
        Cmd::Bench { cases } => {
            let cases = cases.unwrap_or_else(|| std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/bench/cases.jsonl")));
            if bench::run(&repo, &cases, cli.no_dense)? { Ok(()) } else { anyhow::bail!("bench floors not met") }
        }
        Cmd::ImportLegacy { graph_json } => {
            let store = store::Store::new(&repo);
            let (mut graph, manifest) = store.load()?;
            let ids = ids::IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
            let text = std::fs::read_to_string(&graph_json)?;
            let r = legacy::import(&mut graph, &ids, &text)?;
            store.save(&graph, &manifest)?;
            println!(
                "legacy: {} edges, {} both endpoints resolved, {} one, {} concepts created",
                r.edges_seen, r.resolved_both, r.resolved_one, r.concepts_created
            );
            Ok(())
        }
    }
}
