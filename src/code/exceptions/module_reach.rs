//! A module deploys a whole file of this repository (Bicep) or a whole directory (Terraform). Its
//! import names those files by the default path rule, and the module references every top-level name
//! they declare: the walks follow a reference to a declaration and never an import, so a change there
//! reaches whoever deploys it through what it declares.

use crate::code::engine::{Facts, File, Sink};
use crate::code::imports::Resolver;
use crate::model::EdgeKind;

/// From the site's owner to each top-level declaration of the files the engine resolved it to.
pub(crate) fn reach(r: &Resolver, file: &File, facts: &Facts, sink: &mut Sink) {
    let rel = file.rel;
    for site in &facts.imports {
        let from = facts.source_id(rel, facts.owner(site.range.start));
        for target in site.targets(r, crate::code::spec::spec(file.lang), rel) {
            let Some(declared) = r.facts(target) else { continue };
            for d in declared.defs.iter().filter(|d| d.parent.is_none()) {
                sink.edge(&from, &d.id, EdgeKind::References, "", rel);
            }
        }
    }
}
