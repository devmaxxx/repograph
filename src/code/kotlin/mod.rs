//! Kotlin, read by a hand walk into the node and edge kinds TypeScript writes (spec "### Kotlin").

mod declarations;

#[cfg(test)]
mod cases;

use tree_sitter::Node;

use crate::code::imports::Resolver;
use crate::code::index::Header;
use crate::code::jvm::{child, named, text};
use crate::code::lang::Lang;
use crate::model::Extraction;

/// The comment kinds a Kotlin doc block is read from.
pub(crate) const COMMENTS: &[&str] = &["line_comment", "multiline_comment"];

/// One parse per file; every pass shares the tree.
pub fn extract(_resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let mut ex = Extraction::default();
    crate::code::lang::file_node(rel, &mut ex);
    let src = source.as_bytes();
    let Some(tree) = Lang::Kotlin.parse(src) else { return ex };
    let declared = declarations::scan(tree.root_node(), rel, src, &mut ex);
    declarations::link(&declared, rel, &mut ex);
    ex
}

/// The package line and every top-level name, for the JVM index and for the headers `widen` stores.
/// `top` comes from the declarations walk itself, so the index never names a symbol the graph lacks.
pub fn header(source: &str) -> Header {
    let src = source.as_bytes();
    let Some(tree) = Lang::Kotlin.parse(src) else { return Header::default() };
    let root = tree.root_node();
    let package = package_of(root, src);
    let mut scratch = Extraction::default();
    let top = declarations::scan(root, "", src, &mut scratch).top;
    Header { scope: if package.is_empty() { Vec::new() } else { vec![package] }, top, ..Default::default() }
}

/// The dotted name on the `package` line; `""` for a file in the default package.
pub(crate) fn package_of(root: Node, src: &[u8]) -> String {
    child(root, "package_header").and_then(|p| child(p, "identifier")).map(|i| dotted(i, src)).unwrap_or_default()
}

/// `a.b.C` from an `identifier` node, whatever whitespace or comment the file put between segments.
pub(crate) fn dotted(n: Node, src: &[u8]) -> String {
    named(n).into_iter().filter(|c| c.kind() == "simple_identifier").map(|c| text(c, src)).collect::<Vec<_>>().join(".")
}
