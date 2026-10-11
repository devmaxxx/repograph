//! A Swift module sees every type its files declare without an import, so an inheritance clause names
//! a top-level type of its own file, or failing that of any file that does not hide it. An extension's
//! clause belongs to the type it extends, wherever that type's outermost name is declared: the edge
//! starts at that type, which the default could not do, since it starts an edge at the site's owner
//! and an extension owns nothing.

use std::collections::BTreeSet;

use tree_sitter::Node;

use crate::code::engine::{self, Def, Facts, File, Sink};
use crate::code::imports::Resolver;
use crate::code::index::Header;
use crate::code::lang::{Family, Lang};
use crate::code::syntax::{named, text};
use crate::model::EdgeKind;

/// The types a file declares at its top level; an extension's nested type is the extended type's.
fn top_types(facts: &Facts) -> impl Iterator<Item = &Def> {
    facts.defs.iter().filter(|d| d.parent.is_none() && d.kind.join().is_some() && d.qualified == d.name)
}

/// The family's index holds what other files may name: the top-level types a file does not hide.
pub(crate) fn collect(r: &mut Resolver, rel: &str, source: &str) -> Option<Header> {
    let facts = engine::declarations(r, Lang::Swift, rel, source)?;
    Some(Header { top: top_types(&facts).filter(|d| d.export).map(|d| d.qualified.clone()).collect(), ..Header::default() })
}

/// `extension Outer<T>.Inner` extends `Outer.Inner`.
fn extended(extension: Node, src: &str) -> String {
    let Some(path) = extension.child_by_field_name("name") else { return String::new() };
    named(path).into_iter().filter(|t| t.kind() == "type_identifier").map(|t| text(t, src.as_bytes())).collect::<Vec<_>>().join(".")
}

fn top_level_extension(n: Node, src: &str) -> bool {
    n.parent().is_some_and(|p| p.kind() == "source_file")
        && n.child_by_field_name("declaration_kind").is_some_and(|k| text(k, src.as_bytes()) == "extension")
}

pub(crate) fn resolve(r: &Resolver, file: &File, facts: &Facts, sink: &mut Sink) {
    let Some(tree) = file.tree else { return };
    let (rel, src) = (file.rel, file.src);
    // A private type still serves its own file's clauses.
    let own: BTreeSet<&str> = top_types(facts).map(|d| d.name.as_str()).collect();
    let files_of = |name: &str| -> Vec<String> {
        if own.contains(name) {
            return vec![rel.to_string()];
        }
        r.index(Family::Swift).map(|i| i.files(name).into_iter().map(str::to_string).collect()).unwrap_or_default()
    };
    for site in facts.refs.iter().filter(|s| s.edge == EdgeKind::Extends) {
        let mut clause = tree.root_node().descendant_for_byte_range(site.range.start, site.range.end);
        while let Some(n) = clause.filter(|n| n.kind() != "inheritance_specifier") {
            clause = n.parent();
        }
        let Some(declaration) = clause.and_then(|c| c.parent()) else { continue };
        // Only a declared type's clause and a top-level extension's are read: a type in a function
        // body, or nested deeper than the query declares, writes nothing.
        let from: Vec<String> = match facts.declared_at(&declaration.byte_range()) {
            Some(i) => vec![facts.defs[i].id.clone()],
            None if top_level_extension(declaration, src) => {
                let owner = extended(declaration, src);
                let outer = owner.split('.').next().unwrap_or(&owner);
                files_of(outer).into_iter().map(|f| format!("sym:{f}::{owner}")).collect()
            }
            None => continue,
        };
        // A name no file declares, an SDK type such as `UIResponder`, writes nothing.
        for f in files_of(&site.name) {
            for from in &from {
                sink.edge(from, &format!("sym:{f}::{}", site.name), EdgeKind::Extends, "", rel);
            }
        }
    }
}
