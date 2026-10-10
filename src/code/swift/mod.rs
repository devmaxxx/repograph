pub mod declarations;

#[cfg(test)]
mod cases;

use std::collections::{BTreeMap, BTreeSet};

use crate::code::reader::{self, Collect, Reader};
use crate::code::imports::Resolver;
use crate::code::lang::{file_node, Lang};
use crate::model::{EdgeKind, Extraction};

/// The top-level type names one Swift file declares. `Resolver::collect` gathers them from every globbed
/// Swift file before any file is read, because a module sees every file in it without an import.
pub fn types(source: &str) -> BTreeSet<String> {
    let src = source.as_bytes();
    let mut scratch = Extraction::default();
    Lang::Swift.parse(src)
        .map(|t| declarations::scan(t.root_node(), "", src, &mut scratch).types)
        .unwrap_or_default()
}

/// Every globbed Swift file's top-level type names, by name: where an inheritance clause resolves.
#[derive(Default)]
pub(crate) struct Types(BTreeMap<String, BTreeSet<String>>);

impl Types {
    /// The files declaring the top-level type `name`, in path order.
    pub(crate) fn files(&self, name: &str) -> Vec<String> {
        self.0.get(name).map(|files| files.iter().cloned().collect()).unwrap_or_default()
    }
}

pub(crate) const READER: Reader = Reader {
    extract,
    collect: Collect::Source(|r, rel, source| {
        for name in types(source) {
            r.state_mut::<Types>().0.entry(name).or_default().insert(rel.to_string());
        }
    }),
    state: Some(reader::state::<Types>),
    ..reader::NONE
};

pub fn extract(resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let mut ex = Extraction::default();
    file_node(rel, &mut ex);
    let src = source.as_bytes();
    let Some(tree) = Lang::Swift.parse(src) else { return ex };
    let d = declarations::scan(tree.root_node(), rel, src, &mut ex);
    // A file's own declaration shadows the module's, and another file's `private` type is never seen,
    // because `types` leaves it out of the resolver.
    let files_of = |name: &str| if d.own.contains(name) { vec![rel.to_string()] } else { resolver.state::<Types>().files(name) };
    for (ty, sup, here) in &d.supers {
        // An extension's clause belongs to the extended type, in whichever file declares it.
        let from: Vec<String> = if *here {
            vec![format!("sym:{rel}::{ty}")]
        } else {
            // `extension Outer.Inner` is declared wherever the top-level `Outer` is.
            files_of(ty.split('.').next().unwrap_or(ty)).into_iter().map(|f| format!("sym:{f}::{ty}")).collect()
        };
        // An SDK name (`UIResponder`, `String`) is declared by no file in the repository, so it writes nothing.
        for f in files_of(sup) {
            for from_id in &from {
                ex.edge(from_id, &format!("sym:{f}::{sup}"), EdgeKind::Extends, "", rel);
            }
        }
    }
    ex
}
