//! The two helpers the crate's own code cases read through, copied: `#[cfg(test)]` items in the
//! library are not compiled into the crate an integration test links.

use zerograph::code::engine::{EdgeKind, Extraction, Resolver};
use zerograph::Config;

/// `files` on disk and a resolver over them with `globs` as the only code globs, so a case reads
/// the one language it names and nothing the defaults would add.
pub fn repo(globs: &[&str], files: &[(&str, &str)]) -> (tempfile::TempDir, Resolver) {
    let dir = tempfile::tempdir().unwrap();
    for (rel, text) in files {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let cfg = Config { code_globs: globs.iter().map(|g| g.to_string()).collect(), ..Config::default() };
    let resolver = Resolver::new(dir.path(), &cfg).unwrap();
    (dir, resolver)
}

/// Edges of one kind as `source -> target [context]`, sorted and deduplicated as the dispatcher
/// leaves them, so an assertion reads like the graph.
pub fn lines(ex: &Extraction, kind: EdgeKind) -> Vec<String> {
    let mut out: Vec<String> = ex.edges.iter().filter(|e| e.kind == kind)
        .map(|e| format!("{} -> {} [{}]", e.source, e.target, e.context)).collect();
    out.sort();
    out.dedup();
    out
}
