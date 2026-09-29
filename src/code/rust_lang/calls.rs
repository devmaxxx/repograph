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

/// Whether a file writes the symbol `t` names: another file's top-level names are all the line
/// scan knows, so anything below an inline module there is taken as written.
fn written(ctx: &Ctx, t: &Target) -> bool {
    match t {
        Target::Item { file, name } if file != ctx.rel && !name.contains('/') => ctx.crates.declares(file, name.split('.').next().unwrap_or(name)),
        _ => true,
    }
}

/// An `impl` ties its members to their type with `References` when the type is declared in
/// another file — an edge `impact::walks` follows, so `impact` on the type reaches them — and the
/// type to its trait with `Extends` when both resolve in the repository.
pub(crate) fn impls(ctx: &Ctx, ex: &mut Extraction) {
    let own = format!("sym:{}::", ctx.rel);
    for imp in &ctx.items.impls {
        let Some(ty) = ctx.resolve(&imp.inline, &imp.ty).filter(|t| written(ctx, t)).as_ref().and_then(id_of) else { continue };
        if !ty.starts_with(&own) {
            for m in &imp.members {
                ex.edge(&format!("{own}{m}"), &ty, EdgeKind::References, "impl", ctx.rel);
            }
        }
        if let Some(tr) = imp.trait_path.as_ref().and_then(|p| ctx.resolve(&imp.inline, p)).filter(|t| written(ctx, t)).as_ref().and_then(id_of) {
            ex.edge(&ty, &tr, EdgeKind::Extends, "", ctx.rel);
        }
    }
}
