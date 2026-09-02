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
        #[arg(required = true)] words: Vec<String>,
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
    // Loaded per command: `bench` reads its own from `REPOGRAPH_BENCH_REPO`, and `explain`/`verify`
    // must not fail on a broken `repograph.toml` they never read.
    let load_cfg = || config::Config::load(&repo);
    let wipe = matches!(cli.cmd, Cmd::Build);
    match cli.cmd {
        Cmd::Build | Cmd::Update => {
            let cfg = load_cfg()?;
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
            let cfg = load_cfg()?;
            let store = store::Store::new(&repo);
            let (graph, _) = store.load()?;
            let ids = ids::IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
            let dense_idx = index::dense::DenseIndex::load(&store)?;
            // Opening the ONNX model costs ~0.6 s and 1.3 GB; an exact id or symbol match never
            // asks for it, so it is opened on the first fused query, not on every `ask`.
            let embedder: std::cell::RefCell<Option<Option<index::dense::Embedder>>> = std::cell::RefCell::new(None);
            let dense_fn = |q: &str, k: usize| -> Vec<String> {
                let mut slot = embedder.borrow_mut();
                let e = slot.get_or_insert_with(|| open_embedder(cli.no_dense));
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
            if bench::run(&repo, cases.as_deref(), cli.no_dense)? { Ok(()) } else { anyhow::bail!("bench floors not met") }
        }
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
