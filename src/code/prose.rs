//! Path and tree helpers the resolution rules share, and the fixtures their cases build on.

use crate::code::syntax::named;
use tree_sitter::Node;

/// Every node of `kind` under `root`, nested ones included, in source order.
pub(crate) fn all<'t>(root: Node<'t>, kind: &str) -> Vec<Node<'t>> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.kind() == kind { out.push(n) }
        stack.extend(named(n));
    }
    out.sort_by_key(|n| n.start_byte());
    out
}

/// `text` resolved against directory `base`, both `/`-separated. `None` when `..` climbs above the root:
/// that names something outside the repository, and clamping it would name a wrong file inside.
pub(crate) fn join_under(base: &str, text: &str) -> Option<String> {
    let mut parts: Vec<&str> = base.split('/').filter(|s| !s.is_empty()).collect();
    for seg in text.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

#[cfg(test)]
pub(crate) mod testing {
    use crate::code::imports::Resolver;
    use crate::config::Config;
    use crate::model::{EdgeKind, Extraction};

    /// `files` on disk and a resolver over them with `globs` as the only code globs, so a case reads
    /// the one language it names and nothing the defaults would add.
    pub(crate) fn repo(globs: &[&str], files: &[(&str, &str)]) -> (tempfile::TempDir, Resolver) {
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
    pub(crate) fn lines(ex: &Extraction, kind: EdgeKind) -> Vec<String> {
        let mut out: Vec<String> = ex.edges.iter().filter(|e| e.kind == kind)
            .map(|e| format!("{} -> {} [{}]", e.source, e.target, e.context)).collect();
        out.sort();
        out.dedup();
        out
    }

    pub(crate) fn ids(ex: &Extraction) -> Vec<String> {
        let mut out: Vec<String> = ex.nodes.iter().map(|n| n.id.clone()).collect();
        out.sort();
        out.dedup();
        out
    }
}
