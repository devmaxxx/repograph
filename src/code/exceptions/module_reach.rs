//! A module deploys a whole file of this repository. Its import names that file, and the module
//! references every top-level name the file declares: the walks follow a reference to a declaration
//! and never an import, so a change to the file reaches whoever deploys it through what it declares.

use crate::code::engine::{self, Facts, File, Import, Sink};
use crate::code::imports::Resolver;
use crate::code::lang::Lang;
use crate::code::spec;
use crate::model::EdgeKind;

/// The default path, except the importing file itself: a module that deploys its own file reaches
/// nothing.
pub(crate) fn import(r: &Resolver, rel: &str, site: &Import) -> Vec<String> {
    engine::import_targets(r, spec::spec(Lang::Bicep), rel, site).into_iter().filter(|t| t != rel).collect()
}

/// From the site's owner to each top-level declaration of the files the engine resolved it to.
pub(crate) fn reach(r: &Resolver, file: &File, facts: &Facts, sink: &mut Sink) {
    let rel = file.rel;
    for site in &facts.imports {
        let from = facts.source_id(rel, facts.owner(site.range.start));
        for target in &site.targets {
            let Some(declared) = r.facts(target) else { continue };
            for d in declared.defs.iter().filter(|d| d.parent.is_none()) {
                sink.edge(&from, &d.id, EdgeKind::References, "", rel);
            }
        }
    }
}
