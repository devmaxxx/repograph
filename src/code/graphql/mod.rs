//! GraphQL. A definition walk over `tree-sitter-graphql`: operations and fragments under their own namespaces,
//! schema types with their fields, and every name a document uses. Names resolve through one repository-wide
//! index, because codegen treats a fragment's name as global.
mod document;

#[cfg(test)]
mod cases;

use std::collections::BTreeSet;

use crate::code::imports::Resolver;
use crate::code::index::Header;
use crate::code::lang::{file_node, Family, Lang};
use crate::code::reader::{self, Reader};
use crate::model::{EdgeKind, Extraction, NodeKind};

fn read(source: &str) -> Option<document::Read> {
    let tree = Lang::GraphQl.parse(source.as_bytes())?;
    let lines: Vec<&str> = source.lines().collect();
    Some(document::read(tree.root_node(), source.as_bytes(), &lines))
}

/// Every operation, fragment, directive and type the document defines or extends. A document that starts declaring
/// `fragment/X` changes what every spread of `X` resolves to, so the index keys on the whole id tail.
pub fn header(source: &str) -> Header {
    let top = read(source).map(|r| r.objects.into_iter().map(|o| o.name).collect()).unwrap_or_default();
    Header { top, ..Header::default() }
}

pub(crate) const READER: Reader = Reader { extract, header: Some(|_, source| header(source)), ..reader::NONE };

/// The file node, one `sym:<rel>::<tail>` per operation, fragment, directive and type the document defines or
/// extends (declared by the file, context `export`), and one `Type.field` per field of an object type, an
/// interface or an input (declared by its type).
///
/// A name the document uses resolves through `resolver`'s GraphQL index to every file declaring it, the document
/// itself included. A name no document declares, a built-in scalar or an undefined directive, is not linked. The
/// edges are `Calls` from the definition holding a spread (the file, in an anonymous operation) to the fragment,
/// `References` with context `on`, `variable`, `type`, `argument`, `member` or `extend`, `Extends` for
/// `implements`, `DecoratedBy` to a defined directive, and `References` with context `comment` or `string` to
/// each id a comment or a string cites. A source the grammar cannot parse yields the file node alone.
fn extract(resolver: &Resolver, rel: &str, source: &str, ex: &mut Extraction) {
    file_node(rel, ex);
    if let Some(read) = read(source) {
        declare(&read, rel, ex);
        link(resolver, &read, rel, ex);
    }
}

fn sym(rel: &str, tail: &str) -> String {
    format!("sym:{rel}::{tail}")
}

/// One node per name. A type defined and extended in one document spans its definition.
fn declare(read: &document::Read, rel: &str, ex: &mut Extraction) {
    let file = format!("file:{rel}");
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let defined = read.objects.iter().filter(|o| !o.extension);
    for o in defined.chain(read.objects.iter().filter(|o| o.extension)) {
        if seen.insert(o.name.clone()) {
            ex.node_span(NodeKind::Symbol, &sym(rel, &o.name), &o.label, &o.body, rel, o.span);
            ex.edge(&file, &sym(rel, &o.name), EdgeKind::Declares, "export", rel);
        }
    }
    for m in &read.members {
        let tail = format!("{}.{}", m.owner, m.name);
        if seen.insert(tail.clone()) {
            ex.node_span(NodeKind::Symbol, &sym(rel, &tail), &tail, &m.body, rel, m.span);
            ex.edge(&sym(rel, &m.owner), &sym(rel, &tail), EdgeKind::Declares, "export", rel);
        }
    }
}

/// Every file declaring `name`, as ids; none for a name no document declares, such as a built-in scalar.
fn resolve(resolver: &Resolver, own: &BTreeSet<&str>, rel: &str, name: &str) -> Vec<String> {
    let mut files: BTreeSet<&str> = resolver.index(Family::GraphQl).map(|i| i.files(name).into_iter().collect()).unwrap_or_default();
    // A document the walk has not indexed yet still resolves the names it declares itself.
    if own.contains(name) {
        files.insert(rel);
    }
    files.into_iter().map(|f| sym(f, name)).collect()
}

fn link(resolver: &Resolver, read: &document::Read, rel: &str, ex: &mut Extraction) {
    let own: BTreeSet<&str> = read.objects.iter().map(|o| o.name.as_str()).collect();
    let file = format!("file:{rel}");
    for l in &read.links {
        // An anonymous operation declares nothing, so what it spreads is the document's.
        let from = l.from.as_deref().map_or_else(|| file.clone(), |tail| sym(rel, tail));
        for to in resolve(resolver, &own, rel, &l.to) {
            // A recursive field (`children: [Node]` on `Node`) is not a dependency of the type on anything outside it.
            if to == from || from.starts_with(&format!("{to}.")) {
                continue;
            }
            ex.edge(&from, &to, l.kind, l.context, rel);
        }
    }
    // `extend type T` changes `T` wherever it is defined, so it depends on those files as an `ALTER TABLE` does.
    let defined: BTreeSet<&str> = read.objects.iter().filter(|o| !o.extension).map(|o| o.name.as_str()).collect();
    let extended: BTreeSet<&str> = read.objects.iter().filter(|o| o.extension && !defined.contains(o.name.as_str())).map(|o| o.name.as_str()).collect();
    for name in extended {
        let others = resolver.index(Family::GraphQl).map(|i| i.files(name)).unwrap_or_default();
        for other in others.into_iter().filter(|f| *f != rel) {
            ex.edge(&sym(rel, name), &sym(other, name), EdgeKind::References, "extend", rel);
        }
    }
    for c in &read.cites {
        let from = c.from.as_deref().map_or_else(|| file.clone(), |tail| sym(rel, tail));
        for hit in crate::ids::generic().find_all(&c.text) {
            ex.edge(&from, &hit.id, EdgeKind::References, c.context, rel);
        }
    }
}
