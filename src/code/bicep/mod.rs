//! Bicep. A declaration is a symbol by its symbolic name. An output is `output/<name>`, because outputs
//! have a namespace of their own and `output id` beside `param id` is legal. A child resource is
//! `<parent>.<child>`, the member a nested container keeps. References and module paths are read over
//! the same tree by `references.rs` and `modules.rs`.

mod modules;
mod references;
#[cfg(test)]
mod cases;

pub(crate) use modules::Files;

use crate::code::reader::{self, Collect, Extract, Reader};
use crate::code::imports::Resolver;
use crate::code::prose::{self, Spans};
use crate::code::syntax;
use crate::model::Extraction;
use tree_sitter::Node;

pub(crate) const DECLARATIONS: [&str; 7] = [
    "parameter_declaration", "variable_declaration", "resource_declaration", "module_declaration",
    "type_declaration", "user_defined_function", "output_declaration",
];

/// One declaration as the reference and module walks need it.
pub(crate) struct Decl<'t> {
    pub node: Node<'t>,
    pub id: String,
    /// What a reference in this file spells: `vnet.subnet` for `vnet::subnet`. An output's
    /// `output/id` holds a `/`, which no identifier does, so nothing refers to it.
    pub name: String,
}

/// The top-level declarations and the name each is filed under, shared by the extract and by the module
/// index so the two cannot disagree on what a file declares.
pub(crate) fn top_level<'t>(root: Node<'t>, src: &[u8]) -> Vec<(Node<'t>, String)> {
    syntax::named(root).into_iter()
        .filter(|n| DECLARATIONS.contains(&n.kind()))
        .filter_map(|n| Some((n, symbol_name(n, src)?)))
        .collect()
}

/// The name a declaration is filed under, before any parent is prefixed.
fn symbol_name(n: Node, src: &[u8]) -> Option<String> {
    // Only `user_defined_function` has a `name` field; the rest lead with the identifier after the keyword.
    let id = n.child_by_field_name("name").or_else(|| n.named_child(0))?;
    if id.kind() != "identifier" {
        return None;
    }
    let name = syntax::text(id, src);
    Some(if n.kind() == "output_declaration" { format!("output/{name}") } else { name.to_string() })
}

struct Walk<'t, 's> {
    rel: &'s str,
    src: &'s [u8],
    spans: Spans,
    decls: Vec<Decl<'t>>,
    ex: Extraction,
}

/// Bicep state: every globbed `.bicep` file and its top-level names, so a module path resolves to
/// a file that will have a node, and a module call to what that file declares.
pub(crate) const READER: Reader = Reader {
    extract: Extract::Tree(extract),
    collect: Collect::Source(|r, rel, source| r.state_mut::<Files>().add(rel, source)),
    state: Some(reader::state::<Files>),
    ..reader::NONE
};

fn extract(resolver: &Resolver, rel: &str, src: &[u8], root: Node, ex: &mut Extraction) {
    let mut w = Walk { rel, src, spans: Spans::new(rel), decls: Vec::new(), ex: std::mem::take(ex) };
    for (n, name) in top_level(root, src) {
        w.declare(n, name, None);
    }
    references::write(root, &w.decls, &w.spans, src, rel, &mut w.ex);
    modules::write(resolver.state::<Files>(), &w.decls, src, rel, &mut w.ex);
    prose::cite(root, src, rel, &["string"], &w.spans, &mut w.ex);
    *ex = w.ex;
}

impl<'t> Walk<'t, '_> {
    /// `parent` is the container's id and its name, for a resource nested in another.
    fn declare(&mut self, n: Node<'t>, name: String, parent: Option<(&str, &str)>) {
        let name = parent.map_or(name.clone(), |(_, container)| format!("{container}.{name}"));
        let id = format!("sym:{}::{name}", self.rel);
        let from = parent.map_or_else(|| format!("file:{}", self.rel), |(p, _)| p.to_string());
        let context = if exported(n, self.src) { "export" } else { "" };
        let body = prose::body(n, self.src, &["decorators"]);
        prose::declare(&mut self.ex, self.rel, &from, &id, n, &body, context);
        self.spans.push(n, &id);
        if n.kind() == "resource_declaration" {
            for child in syntax::find_all(n, &["resource_declaration"]) {
                if let Some(child_name) = symbol_name(child, self.src) {
                    self.declare(child, child_name, Some((&id, &name)));
                }
            }
        }
        self.decls.push(Decl { node: n, id, name });
    }
}

/// A parameter and an output are the module's interface to whoever deploys it; anything else is
/// exported only when it says so.
fn exported(n: Node, src: &[u8]) -> bool {
    if matches!(n.kind(), "parameter_declaration" | "output_declaration") {
        return true;
    }
    let mut at = prev_code(n);
    while let Some(d) = at.filter(|p| p.kind() == "decorators") {
        let named = syntax::named(d).into_iter().any(|dec| {
            let t = syntax::text(dec, src);
            t.starts_with("@export(") || t.starts_with("@sys.export(")
        });
        if named {
            return true;
        }
        at = prev_code(d);
    }
    false
}

/// A comment may sit between a decorator and the declaration it decorates, and it does not end the pair.
fn prev_code(n: Node) -> Option<Node> {
    std::iter::successors(n.prev_named_sibling(), |p| p.prev_named_sibling()).find(|p| p.kind() != "comment")
}

pub(super) fn next_code(n: Node) -> Option<Node> {
    std::iter::successors(n.next_named_sibling(), |p| p.next_named_sibling()).find(|p| p.kind() != "comment")
}
