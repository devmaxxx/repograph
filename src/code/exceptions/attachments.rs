//! A file that attaches to a name it does not declare otherwise, as a migration altering a table
//! another one creates, depends on every other file declaring that name: without the edge, `impact`
//! on the creating migration never reaches the ones that alter it. A reference cannot say it, since
//! whether the file creates the table is nowhere in the statement that alters it; the query says it
//! by kind instead, declaring an attachment's table a `record` unless the file declared it before.

use crate::code::engine::{Facts, File, Sink};
use crate::code::imports::Resolver;
use crate::code::queries::Kind;
use crate::model::EdgeKind;

/// From each top-level `record` to the same name in every other file declaring it.
pub(crate) fn alters(r: &Resolver, file: &File, facts: &Facts, sink: &mut Sink) {
    let Some(index) = r.index(file.lang.family()) else { return };
    for d in facts.defs.iter().filter(|d| d.parent.is_none() && d.kind == Kind::Record) {
        for other in index.files(&d.qualified).into_iter().filter(|f| *f != file.rel) {
            sink.edge(&d.id, &format!("sym:{other}::{}", d.qualified), EdgeKind::References, "alter", file.rel);
        }
    }
}
