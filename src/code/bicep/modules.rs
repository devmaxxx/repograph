//! `module m './x.bicep'`: the path is a file of this repository, so the declaration imports it. A
//! registry or template-spec module (`br:`, `br/`, `ts:`, `ts/`) lives somewhere else and resolves to
//! nothing, and so does a path that climbs out of the repository.

use super::{top_level, Decl};
use crate::code::lang::Lang;
use crate::code::prose;
use crate::model::{EdgeKind, Extraction};
use std::collections::BTreeMap;
use tree_sitter::Node;

/// Every globbed `.bicep` file and the top-level names it declares, read before any extract. A module
/// path resolves only to a file that will have a node, and the call reaches what that file declares.
#[derive(Default)]
pub(crate) struct Files(BTreeMap<String, Vec<String>>);

impl Files {
    pub(crate) fn add(&mut self, rel: &str, source: &str) {
        let src = source.as_bytes();
        let names = Lang::Bicep.parse(src)
            .map(|tree| top_level(tree.root_node(), src).into_iter().map(|(_, name)| name).collect())
            .unwrap_or_default();
        self.0.insert(rel.to_string(), names);
    }
}

pub(super) fn write(files: &Files, decls: &[Decl], src: &[u8], rel: &str, ex: &mut Extraction) {
    for d in decls.iter().filter(|d| d.node.kind() == "module_declaration") {
        let Some(path) = literal(d.node, src) else { continue };
        if ["br:", "br/", "ts:", "ts/"].iter().any(|p| path.starts_with(p)) {
            continue;
        }
        let Some(target) = within_repo(rel, path).filter(|t| t != rel) else { continue };
        let Some(declared) = files.0.get(&target) else { continue };
        // `*`: a module deploys the whole file, so `importers` counts it for every declaration there.
        ex.edge(&format!("file:{rel}"), &format!("file:{target}"), EdgeKind::Imports, "*", rel);
        // The walks follow a reference to a declaration and never an import (master L11), so the call
        // reaches the module through what the module declares.
        for short in declared {
            ex.edge(&d.id, &format!("sym:{target}::{short}"), EdgeKind::References, "", rel);
        }
    }
}

/// `path` relative to the importing file, as a repository path; `None` when it climbs above the root,
/// which names a file outside the repository, or is absolute, which Bicep does not resolve.
fn within_repo(rel: &str, path: &str) -> Option<String> {
    if path.starts_with('/') {
        return None;
    }
    let mut parts: Vec<&str> = rel.rsplit_once('/').map(|(dir, _)| dir.split('/').collect()).unwrap_or_default();
    for seg in path.split('/') {
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

/// The module's path when it is written out whole. An interpolated path names no one file.
fn literal<'s>(module: Node, src: &'s [u8]) -> Option<&'s str> {
    let string = prose::named(module).into_iter().find(|c| c.kind() == "string")?;
    match prose::named(string).as_slice() {
        [content] if content.kind() == "string_content" => Some(prose::text(*content, src)),
        _ => None,
    }
}
