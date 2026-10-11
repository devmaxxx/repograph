//! `inherits = ["base"]` and `targets = ["api"]` in a bake file name a block of the same file by its
//! label alone: a target, or failing that a group. The query takes each written-out item; the
//! default would look the bare label up, which no `target/` or `group/` address is.

use crate::code::engine::{Facts, File, Sink};
use crate::code::imports::Resolver;
use crate::model::EdgeKind;

pub(crate) fn resolve(_: &Resolver, file: &File, facts: &Facts, sink: &mut Sink) {
    let rel = file.rel;
    for r in facts.refs.iter().filter(|r| r.edge == EdgeKind::References) {
        let Some(address) = ["target", "group"].iter().map(|k| format!("{k}/{}", r.name)).find(|a| facts.declares(a)) else { continue };
        let (from, target) = (facts.source_id(rel, r.source), format!("sym:{rel}::{address}"));
        if target != from {
            sink.edge(&from, &target, EdgeKind::References, "", rel);
        }
    }
}
