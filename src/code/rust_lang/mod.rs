//! Rust: one parse per file, the module tree read from paths and `Cargo.toml` rather than from
//! other files' contents, so extracting a file never opens another.

use crate::code::imports::Resolver;
use crate::code::reader::{self, Collect, Extract, Manifest, Reader};
use crate::code::syntax::{field_text, text};
use crate::model::Extraction;
use tree_sitter::Node;

mod calls;
mod crates;
mod items;
mod uses;

pub use crates::Crates;

#[cfg(test)]
mod cases;

/// Rust state: the module tree, every `.rs` path and `Cargo.toml` the globs reach.
pub(crate) const READER: Reader = Reader {
    extract: Extract::Tree(extract),
    collect: Collect::Source(|r, rel, source| r.state_mut::<Crates>().file(rel, source)),
    state: Some(reader::state::<Crates>),
    manifest: Some(Manifest { matches: |name| name == "Cargo.toml", read: |r, rel, text| r.state_mut::<Crates>().manifest(rel, text) }),
    ..reader::NONE
};

/// The file node, a symbol for every item at a declaring scope (file, inline `mod`, `trait` and
/// `impl` bodies) with its `Declares` edge, and a `References` edge for each requirement id cited
/// in a comment or string. Never opens another file.
fn extract(resolver: &Resolver, rel: &str, src: &[u8], root: Node, ex: &mut Extraction) {
    let items = items::read(rel, src, root, ex);
    items::id_refs(rel, src, root, ex);
    let mut ctx = uses::Ctx { rel, src, crates: resolver.state::<Crates>(), items: &items, bindings: uses::Bindings::default() };
    uses::read(&mut ctx, root, ex);
    calls::impls(&ctx, ex);
    calls::read(&ctx, root, ex);
    calls::attributes(&ctx, root, ex);
}

/// The inline modules around `n`, outermost first.
pub(crate) fn inline_of(n: Node, src: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = n;
    while let Some(p) = at.parent() {
        if p.kind() == "mod_item" {
            out.extend(field_text(p, "name", src).map(str::to_string));
        }
        at = p;
    }
    out.reverse();
    out
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
        "generic_function" => t.child_by_field_name("function").map_or_else(Vec::new, |x| type_path(x, src)),
        "generic_type" | "reference_type" | "pointer_type" => t.child_by_field_name("type").map_or_else(Vec::new, |x| type_path(x, src)),
        "scoped_type_identifier" | "scoped_identifier" => {
            let mut p = t.child_by_field_name("path").map_or_else(Vec::new, |x| type_path(x, src));
            p.extend(field_text(t, "name", src).map(str::to_string));
            p
        }
        _ => Vec::new(),
    }
}
