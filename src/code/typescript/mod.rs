//! The TypeScript family: TypeScript and JavaScript, and Vue single-file components whose script
//! resolves names through the same resolver. A TypeScript file takes three passes over the front's
//! one parse.

pub mod calls;
pub mod idrefs;
pub mod symbols;
pub mod vue;

use crate::code::imports::Resolver;
use crate::code::engine::File;
use crate::code::spec::{self, Hooks};
use crate::model::{Extraction, NodeKind};

/// Its resolver state is the tsconfig and package.json `Resolver::new` reads, which no other
/// language shares; its sources are the extractor's alone.
pub(crate) const HOOKS: Hooks = Hooks { head: Some(head), ..spec::NONE };

/// The comments above a file's first statement, for Vue at the script's rows.
pub(crate) fn head(file: &File) -> String {
    file.tree.map(|tree| symbols::file_head(tree.root_node(), file.src.as_bytes())).unwrap_or_default()
}

/// Appends to the file node the front wrote, which an earlier version overwrote here.
pub(crate) fn extract(resolver: &Resolver, file: &File, ex: &mut Extraction) {
    let Some(tree) = file.tree else { return };
    let (rel, root, src) = (file.rel, tree.root_node(), file.src.as_bytes());
    let walked = symbols::Walk { resolver }.scan_tree(rel, root, src);
    // The walk's file node is the one the front wrote, from the same `head`.
    ex.nodes.extend(walked.nodes.into_iter().filter(|n| n.kind != NodeKind::File));
    ex.edges.extend(walked.edges);
    // Top-level names this file declares; a member id carries a dot and is not one.
    let prefix = format!("sym:{rel}::");
    let locals: std::collections::BTreeSet<String> = ex.nodes.iter()
        .filter_map(|n| n.id.strip_prefix(&prefix))
        .filter(|n| !n.contains('.'))
        .map(str::to_string)
        .collect();
    calls::scan_tree(resolver, rel, root, src, &locals, ex);
    idrefs::scan_tree(root, rel, src, ex);
}
