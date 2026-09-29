//! Rust: one parse per file, the module tree read from paths and `Cargo.toml` rather than from
//! other files' contents, so extracting a file never opens another.

use crate::code::imports::Resolver;
use crate::code::jvm::text;
use crate::code::lang::{file_node, Lang};
use crate::model::Extraction;
use tree_sitter::Node;

mod crates;
mod items;

pub use crates::Crates;

#[cfg(test)]
mod cases;

/// The file node, a symbol for every item at a declaring scope (file, inline `mod`, `trait` and
/// `impl` bodies) with its `Declares` edge, and a `References` edge for each requirement id cited
/// in a comment or string. Never opens another file.
pub fn extract(_resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let mut ex = Extraction::default();
    file_node(rel, &mut ex);
    let src = source.as_bytes();
    let Some(tree) = Lang::Rust.parse(src) else { return ex };
    let root = tree.root_node();
    items::read(rel, src, root, &mut ex);
    items::id_refs(rel, src, root, &mut ex);
    ex
}

pub(crate) fn field_text<'a>(n: Node, field: &str, src: &'a [u8]) -> Option<&'a str> {
    n.child_by_field_name(field).map(|c| text(c, src))
}

/// An item's id suffix inside inline modules: every segment but a member is joined with `/`.
pub(crate) fn scoped(inline: &[String], name: &str) -> String {
    if inline.is_empty() { name.to_string() } else { format!("{}/{name}", inline.join("/")) }
}

/// A type's path as written, generic arguments and references dropped: `&mut Tmux<E>` is `["Tmux"]`,
/// `crate::walk::Manifest` is `["crate", "walk", "Manifest"]`.
pub(crate) fn type_path(t: Node, src: &[u8]) -> Vec<String> {
    match t.kind() {
        "type_identifier" | "identifier" | "primitive_type" | "self" | "crate" | "super" => vec![text(t, src).to_string()],
        "generic_type" | "reference_type" | "pointer_type" => t.child_by_field_name("type").map_or_else(Vec::new, |x| type_path(x, src)),
        "scoped_type_identifier" | "scoped_identifier" => {
            let mut p = t.child_by_field_name("path").map_or_else(Vec::new, |x| type_path(x, src));
            p.extend(field_text(t, "name", src).map(str::to_string));
            p
        }
        _ => Vec::new(),
    }
}
