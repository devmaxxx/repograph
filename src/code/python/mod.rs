//! Python: one parse per file; module names are read from paths and project manifests, so
//! extracting a file never opens another.

use crate::code::reader::{self, Collect, Extract, Manifest, Reader};
use crate::code::imports::Resolver;
use tree_sitter::Node;
use crate::model::Extraction;

mod defs;
mod modules;
mod refs;

pub use modules::Modules;

#[cfg(test)]
mod cases;

/// Python state: module names, every `.py` path and the directories holding a project manifest.
pub(crate) const READER: Reader = Reader {
    extract: Extract::Tree(extract),
    collect: Collect::Source(|r, rel, source| {
        let modules = r.state_mut::<Modules>();
        modules.file(rel);
        modules.init(rel, source);
    }),
    state: Some(reader::state::<Modules>),
    manifest: Some(Manifest {
        matches: |name| matches!(name, "pyproject.toml" | "setup.py" | "setup.cfg"),
        read: |r, rel, _| r.state_mut::<Modules>().manifest(rel),
    }),
    ..reader::NONE
};

/// One file's graph: its `file:` node, a symbol per module-level and class-level `def`, `class`
/// and plain-name assignment with `Declares` edges (`export` on the module's public names), and
/// `References` from the definition that holds each requirement id cited in a comment,
/// docstring or string, and `Imports`, `Calls`, `Extends` and `DecoratedBy` from what the code reaches.
fn extract(resolver: &Resolver, rel: &str, src: &[u8], root: Node, ex: &mut Extraction) {
    let defs = defs::read(rel, src, root, ex);
    defs::id_refs(rel, src, root, ex);
    refs::read(resolver.state::<Modules>(), rel, src, root, &defs, ex);
}
