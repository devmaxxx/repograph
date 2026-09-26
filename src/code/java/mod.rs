//! Java, read by a hand walk into the node and edge kinds TypeScript writes (spec "### Java").

mod declarations;

#[cfg(test)]
mod cases;

use tree_sitter::Node;

use crate::code::imports::Resolver;
use crate::code::index::{Header, Nested, QualifiedIndex};
use crate::code::jvm::{self, child, named, text, Scope};
use crate::code::lang::{Family, Lang};
use crate::model::Extraction;

/// The comment kinds a Java doc block is read from.
pub(crate) const COMMENTS: &[&str] = &["line_comment", "block_comment"];

/// One parse per file; every pass shares the tree.
pub fn extract(resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let mut ex = Extraction::default();
    crate::code::lang::file_node(rel, &mut ex);
    let src = source.as_bytes();
    let Some(tree) = Lang::Java.parse(src) else { return ex };
    let root = tree.root_node();
    // A repository whose globs reach no JVM file has no index; every lookup then finds nothing.
    let empty = QualifiedIndex::default();
    let index = resolver.index(Family::Jvm).unwrap_or(&empty);
    let scope = scope(root, src);
    let declared = declarations::scan(root, rel, src, &mut ex);
    jvm::link(&declared.types, &declared.supers, index, &scope, rel, &mut ex);
    ex
}

/// The package line, every top-level type and every nested path, for the JVM index and for the
/// headers `widen` stores, from one parse. All of it comes from the declarations walk, so the index
/// never names a symbol the graph lacks.
pub fn header(source: &str) -> Header {
    let src = source.as_bytes();
    let Some(tree) = Lang::Java.parse(src) else { return Header::default() };
    let root = tree.root_node();
    let package = package_of(root, src);
    let mut scratch = Extraction::default();
    let d = declarations::scan(root, "", src, &mut scratch);
    let nested = Nested { types: d.types, members: d.members.iter().map(|id| jvm::path_of(id).to_string()).collect() };
    Header { scope: if package.is_empty() { Vec::new() } else { vec![package] }, top: d.top, nested, ..Default::default() }
}

pub(crate) fn package_of(root: Node, src: &[u8]) -> String {
    child(root, "package_declaration")
        .and_then(|p| named(p).into_iter().find(|c| matches!(c.kind(), "scoped_identifier" | "identifier")))
        .map(|i| text(i, src).split_whitespace().collect())
        .unwrap_or_default()
}

/// The package and the four import forms. `static` is an unnamed child of the declaration.
pub(crate) fn scope(root: Node, src: &[u8]) -> Scope {
    let mut s = Scope { package: package_of(root, src), ..Scope::default() };
    for imp in named(root).into_iter().filter(|n| n.kind() == "import_declaration") {
        let Some(id) = named(imp).into_iter().find(|c| matches!(c.kind(), "scoped_identifier" | "identifier")) else { continue };
        let qualified: String = text(id, src).split_whitespace().collect();
        let mut c = imp.walk();
        let is_static = imp.children(&mut c).any(|k| k.kind() == "static");
        match (is_static, child(imp, "asterisk").is_some()) {
            (false, false) => {
                let local = qualified.rsplit('.').next().unwrap_or(&qualified).to_string();
                s.singles.entry(local).or_default().insert(qualified);
            }
            (false, true) => s.stars.push(qualified),
            (true, false) => {
                if let Some((ty, member)) = qualified.rsplit_once('.') {
                    s.statics.entry(member.to_string()).or_default().insert(ty.to_string());
                }
            }
            (true, true) => s.static_stars.push(qualified),
        }
    }
    s
}
