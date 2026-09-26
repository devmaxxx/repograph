//! Java, read by a hand walk into the node and edge kinds TypeScript writes (spec "### Java").

mod declarations;

#[cfg(test)]
mod cases;

use tree_sitter::Node;

use crate::code::imports::Resolver;
use crate::code::index::Header;
use crate::code::jvm::{child, named, text};
use crate::code::lang::Lang;
use crate::model::Extraction;

/// The comment kinds a Java doc block is read from.
pub(crate) const COMMENTS: &[&str] = &["line_comment", "block_comment"];

/// One parse per file; every pass shares the tree.
pub fn extract(_resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let mut ex = Extraction::default();
    crate::code::lang::file_node(rel, &mut ex);
    let src = source.as_bytes();
    let Some(tree) = Lang::Java.parse(src) else { return ex };
    let declared = declarations::scan(tree.root_node(), rel, src, &mut ex);
    declarations::link(&declared, rel, &mut ex);
    ex
}

/// The package line and every top-level type, for the JVM index and for the headers `widen` stores;
/// `top` comes from the declarations walk, so the index never names a symbol the graph lacks.
pub fn header(source: &str) -> Header {
    let src = source.as_bytes();
    let Some(tree) = Lang::Java.parse(src) else { return Header::default() };
    let root = tree.root_node();
    let package = package_of(root, src);
    let mut scratch = Extraction::default();
    let top = declarations::scan(root, "", src, &mut scratch).top;
    Header { scope: if package.is_empty() { Vec::new() } else { vec![package] }, top, ..Default::default() }
}

pub(crate) fn package_of(root: Node, src: &[u8]) -> String {
    child(root, "package_declaration")
        .and_then(|p| named(p).into_iter().find(|c| matches!(c.kind(), "scoped_identifier" | "identifier")))
        .map(|i| text(i, src).split_whitespace().collect())
        .unwrap_or_default()
}
