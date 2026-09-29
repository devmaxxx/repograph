//! What a Rust file's code reaches: an `impl`'s ties to its type and trait.

use super::crates::Target;
use super::uses::Ctx;
use crate::model::{EdgeKind, Extraction};

pub(crate) fn id_of(t: &Target) -> Option<String> {
    match t {
        Target::Item { file, name } => Some(format!("sym:{file}::{name}")),
        Target::Module { .. } => None,
    }
}

/// An `impl` ties its members to their type with `References` when the type is declared in
/// another file — an edge `impact::walks` follows, so `impact` on the type reaches them — and the
/// type to its trait with `Extends` when both resolve in the repository.
pub(crate) fn impls(ctx: &Ctx, ex: &mut Extraction) {
    let own = format!("sym:{}::", ctx.rel);
    for imp in &ctx.items.impls {
        let Some(ty) = ctx.resolve(&imp.inline, &imp.ty).as_ref().and_then(id_of) else { continue };
        if !ty.starts_with(&own) {
            for m in &imp.members {
                ex.edge(&format!("{own}{m}"), &ty, EdgeKind::References, "impl", ctx.rel);
            }
        }
        if let Some(tr) = imp.trait_path.as_ref().and_then(|p| ctx.resolve(&imp.inline, p)).as_ref().and_then(id_of) {
            ex.edge(&ty, &tr, EdgeKind::Extends, "", ctx.rel);
        }
    }
}
