//! Python: one parse per file; module names are read from paths and project manifests, so
//! extracting a file never opens another.

use crate::code::imports::Resolver;
use crate::code::lang::{file_node, Lang};
use crate::model::Extraction;

mod defs;
mod modules;
mod refs;

pub use modules::Modules;

#[cfg(test)]
mod cases;

/// One file's graph: its `file:` node, a symbol per module-level and class-level `def`, `class`
/// and plain-name assignment with `Declares` edges (`export` on the module's public names), and
/// `References` from the definition that holds each requirement id cited in a comment,
/// docstring or string, and `Imports`, `Calls`, `Extends` and `DecoratedBy` from what the code reaches.
pub fn extract(resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let mut ex = Extraction::default();
    file_node(rel, &mut ex);
    let src = source.as_bytes();
    let Some(tree) = Lang::Python.parse(src) else { return ex };
    let root = tree.root_node();
    let defs = defs::read(rel, src, root, &mut ex);
    defs::id_refs(rel, src, root, &mut ex);
    refs::read(resolver.python(), rel, src, root, &defs, &mut ex);
    ex
}
