//! PostgreSQL. A statement walk over `tree-sitter-postgres`: what a migration creates or attaches to a table,
//! keyed by the name PostgreSQL resolves. Every file that declares or alters a name is one of its declaring
//! files, and a reference resolves to all of them.
mod names;
mod statements;

#[cfg(test)]
mod cases;

use std::collections::BTreeSet;

use crate::code::reader::{self, Extract, Reader};
use crate::code::imports::Resolver;
use crate::code::index::Header;
use crate::code::lang::{Family, Lang};
use crate::model::{EdgeKind, Extraction, NodeKind};

fn read(source: &str) -> Option<statements::Read> {
    let tree = Lang::Sql.parse(source.as_bytes())?;
    let lines: Vec<&str> = source.lines().collect();
    Some(statements::read(tree.root_node(), source.as_bytes(), &lines))
}

/// Every object the file creates or attaches to, `ALTER TABLE t` and `CREATE TRIGGER … ON t` included.
///
/// The index must list a migration that alters `t` among `t`'s files, and a migration starting to alter `t`
/// is exactly the change that moves what a foreign key elsewhere resolves to, so the update widens on it. A schema
/// is part of each name, so there is no scope.
pub fn header(source: &str) -> Header {
    let top = read(source).map(|r| r.objects.into_iter().map(|o| o.name).collect()).unwrap_or_default();
    Header { top, ..Header::default() }
}

pub(crate) const READER: Reader = Reader { extract: Extract::Source(extract), header: Some(|_, source| header(source)), ..reader::NONE };

/// The file node, one `sym:<rel>::<schema>/<object>` per object the file creates or attaches to (declared by the
/// file, context `export`), and one `<table>.<member>` per column, trigger, policy and named index (declared by
/// its table).
///
/// A name the file uses resolves through `resolver`'s SQL index to every file declaring it, the file itself
/// included. A name no file declares is not linked. The edges are `References` with context `alter`, `rename`,
/// `references` or `from`, `Calls` from a trigger to its function, and `References` from the enclosing statement,
/// or the file between statements, to each id a comment or a string cites. A source the grammar cannot parse
/// yields the file node alone.
fn extract(resolver: &Resolver, rel: &str, source: &str, ex: &mut Extraction) {
    if let Some(read) = read(source) {
        declare(&read, rel, ex);
        link(resolver, &read, rel, ex);
    }
}

fn sym(rel: &str, tail: &str) -> String {
    format!("sym:{rel}::{tail}")
}

/// One node per name. A table created and attached to in one migration is one declaration spanning the statement
/// that created it, whichever of the two the file wrote first.
fn declare(read: &statements::Read, rel: &str, ex: &mut Extraction) {
    let file = format!("file:{rel}");
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let created = read.objects.iter().filter(|o| !o.attached);
    for o in created.chain(read.objects.iter().filter(|o| o.attached)) {
        if seen.insert(o.name.clone()) {
            ex.node_span(NodeKind::Symbol, &sym(rel, &o.name), names::bare(&o.name), &o.body, rel, o.span);
            ex.edge(&file, &sym(rel, &o.name), EdgeKind::Declares, "export", rel);
        }
    }
    for m in &read.members {
        let tail = format!("{}.{}", m.table, m.name);
        if seen.insert(tail.clone()) {
            let label = format!("{}.{}", names::bare(&m.table), m.name);
            ex.node_span(NodeKind::Symbol, &sym(rel, &tail), &label, &m.body, rel, m.span);
            ex.edge(&sym(rel, &m.table), &sym(rel, &tail), EdgeKind::Declares, "export", rel);
        }
    }
}

/// Every declaration `name` can stand for. The first of `names::candidates` that some file declares wins, because
/// `search_path` is not in any file.
fn resolve(resolver: &Resolver, own: &BTreeSet<&str>, rel: &str, name: &str) -> Vec<String> {
    for candidate in names::candidates(name) {
        let mut files: BTreeSet<&str> = resolver.index(Family::Sql).map(|i| i.files(&candidate).into_iter().collect()).unwrap_or_default();
        // A file the walk has not indexed yet still resolves the names it declares itself.
        if own.contains(candidate.as_str()) {
            files.insert(rel);
        }
        if !files.is_empty() {
            return files.into_iter().map(|f| sym(f, &candidate)).collect();
        }
    }
    Vec::new()
}

fn link(resolver: &Resolver, read: &statements::Read, rel: &str, ex: &mut Extraction) {
    let own: BTreeSet<&str> = read.objects.iter().map(|o| o.name.as_str()).collect();
    for l in &read.links {
        let from = sym(rel, &l.from);
        for to in resolve(resolver, &own, rel, &l.to) {
            // A foreign key from a column to its own table is not a dependency of the column on anything outside it.
            if to == from || from.starts_with(&format!("{to}.")) {
                continue;
            }
            ex.edge(&from, &to, l.kind, l.context, rel);
        }
    }
    // A migration that alters `t` without creating it depends on every other file declaring `t`. Without this
    // edge, `impact` on the creating migration would never reach the ones that alter it.
    let created: BTreeSet<&str> = read.objects.iter().filter(|o| !o.attached).map(|o| o.name.as_str()).collect();
    let attached: BTreeSet<&str> = read.objects.iter().filter(|o| o.attached && !created.contains(o.name.as_str())).map(|o| o.name.as_str()).collect();
    for name in attached {
        let others = resolver.index(Family::Sql).map(|i| i.files(name)).unwrap_or_default();
        for other in others.into_iter().filter(|f| *f != rel) {
            ex.edge(&sym(rel, name), &sym(other, name), EdgeKind::References, "alter", rel);
        }
    }
    let file = format!("file:{rel}");
    for c in &read.cites {
        let from = c.from.as_deref().map_or_else(|| file.clone(), |tail| sym(rel, tail));
        for hit in crate::ids::generic().find_all(&c.text) {
            ex.edge(&from, &hit.id, EdgeKind::References, c.context, rel);
        }
    }
}
