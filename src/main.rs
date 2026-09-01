mod code;
mod config;
mod doc;
mod ids;
mod model;
mod store;
mod walk;

use clap::{Parser, Subcommand};
use model::{Extractor, Noop};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "repograph", version)]
struct Cli {
    /// Repository root; defaults to the current directory.
    #[arg(long, global = true, default_value = ".")]
    repo: PathBuf,
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
        #[arg(long)] no_dense: bool,
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

fn extractors(cfg: &config::Config) -> Extractors {
    let ids = ids::IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
    Extractors {
        doc: Box::new(doc::DocExtractor::new(ids.clone())),
        code: Box::new(Noop),
        registry: Box::new(doc::registry::RegistryExtractor::new(ids)),
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let repo = cli.repo.canonicalize()?;
    let cfg = config::Config::load(&repo)?;
    let wipe = matches!(cli.cmd, Cmd::Build);
    match cli.cmd {
        Cmd::Build | Cmd::Update => {
            let r = run_update(&repo, &cfg, &extractors(&cfg), wipe)?;
            println!("changed {} removed {} nodes {} edges {}", r.changed, r.removed, r.nodes, r.edges);
            Ok(())
        }
        _ => anyhow::bail!("not implemented yet"),
    }
}
