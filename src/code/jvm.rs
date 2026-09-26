//! What Kotlin and Java share: node helpers, how a symbol's body is built, and the order a type
//! name is looked up in — this file first, then the one JVM index both languages fill.

use std::collections::BTreeSet;

use tree_sitter::Node;

pub(crate) fn text<'a>(n: Node, src: &'a [u8]) -> &'a str {
    n.utf8_text(src).unwrap_or("")
}

pub(crate) fn named<'t>(n: Node<'t>) -> Vec<Node<'t>> {
    let mut c = n.walk();
    n.named_children(&mut c).collect()
}

pub(crate) fn child<'t>(n: Node<'t>, kind: &str) -> Option<Node<'t>> {
    named(n).into_iter().find(|c| c.kind() == kind)
}

pub(crate) fn span(n: Node) -> (u32, u32) {
    (n.start_position().row as u32 + 1, n.end_position().row as u32 + 1)
}

/// `Outer` for `Outer.Inner`, `""` for a top-level path.
pub(crate) fn outer(path: &str) -> &str {
    path.rsplit_once('.').map_or("", |(o, _)| o)
}

/// `Outer.Inner` for `sym:a.kt::Outer.Inner`.
pub(crate) fn path_of(id: &str) -> &str {
    id.rsplit_once("::").map_or(id, |(_, p)| p)
}

/// A symbol's body as a TypeScript one is built: the comment block ending on the line above,
/// capped, then the line holding the name. The name's line and not the node's first, because a
/// Kotlin annotation sits inside the declaration node and `@Serializable` alone would be the one
/// line the retrievers index.
pub(crate) fn body(n: Node, name: Node, src: &[u8], comments: &[&str]) -> String {
    let mut parts = Vec::new();
    let mut next = n;
    while let Some(prev) = next.prev_named_sibling() {
        if !comments.contains(&prev.kind()) || prev.end_position().row + 1 < next.start_position().row {
            break;
        }
        parts.push(crate::code::symbols::comment_text(text(prev, src)));
        next = prev;
    }
    parts.reverse();
    let doc = crate::code::symbols::cap(parts.join("\n"), crate::code::symbols::DOC_CHARS);
    let at = name.start_byte();
    let start = src[..at].iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    let end = src[at..].iter().position(|&b| b == b'\n').map_or(src.len(), |i| at + i);
    let line = std::str::from_utf8(&src[start..end]).unwrap_or("").trim();
    if doc.is_empty() { line.to_string() } else { format!("{doc}\n{line}") }
}

/// A type name written inside the type at `at` (`""` at top level), resolved in this file: the
/// nearest enclosing type's nested name first, outward to the top level, as both compilers do.
/// `Outer.Inner` written in full resolves through its first segment.
pub(crate) fn in_file(types: &BTreeSet<String>, at: &str, written: &str) -> Option<String> {
    let (first, rest) = written.split_once('.').unwrap_or((written, ""));
    let mut scope = at;
    loop {
        let candidate = if scope.is_empty() { first.to_string() } else { format!("{scope}.{first}") };
        if types.contains(&candidate) {
            return Some(if rest.is_empty() { candidate } else { format!("{candidate}.{rest}") });
        }
        if scope.is_empty() {
            return None;
        }
        scope = outer(scope);
    }
}

#[cfg(test)]
pub(crate) mod fixture {
    use crate::code::imports::Resolver;
    use crate::code::CodeExtractor;
    use crate::config::Config;
    use crate::model::{EdgeKind, Extraction, Extractor};

    /// A repository on disk, so the resolver indexes every file the way a build does.
    pub(crate) struct Repo {
        dir: tempfile::TempDir,
    }

    impl Repo {
        pub(crate) fn new(files: &[(&str, &str)]) -> Repo {
            let dir = tempfile::tempdir().unwrap();
            for (p, c) in files {
                let full = dir.path().join(p);
                std::fs::create_dir_all(full.parent().unwrap()).unwrap();
                std::fs::write(full, c).unwrap();
            }
            Repo { dir }
        }

        /// The file at `rel` as `build` extracts it. The JVM globs are set because the index is
        /// filled only for the families `code_globs` reaches (L3), and the defaults do not yet.
        pub(crate) fn extract(&self, rel: &str) -> Extraction {
            let cfg = Config { code_globs: vec!["**/*.kt".into(), "**/*.java".into()], ..Config::default() };
            let text = std::fs::read_to_string(self.dir.path().join(rel)).unwrap();
            CodeExtractor::new(Resolver::new(self.dir.path(), &cfg).unwrap()).extract(rel, &text)
        }
    }

    pub(crate) fn one(rel: &str, src: &str) -> Extraction {
        Repo::new(&[(rel, src)]).extract(rel)
    }

    pub(crate) fn ids(ex: &Extraction) -> Vec<&str> {
        ex.nodes.iter().map(|n| n.id.as_str()).collect()
    }

    pub(crate) fn edges(ex: &Extraction, kind: EdgeKind) -> Vec<(&str, &str, &str)> {
        ex.edges.iter().filter(|e| e.kind == kind).map(|e| (e.source.as_str(), e.target.as_str(), e.context.as_str())).collect()
    }
}
