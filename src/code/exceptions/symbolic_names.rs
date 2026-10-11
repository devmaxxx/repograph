//! A name read in a file means a declaration of that same file, looked up the way Bicep's compiler
//! does: a child resource answers to its short name inside its parent, nearest parent first, and to
//! `parent::child` anywhere; a call names a function and nothing else, so `range(0, 3)` beside
//! `param range` is the built-in; and a decorator reads for the declaration it decorates, which it
//! stands outside of.
//!
//! The query takes every read and the locals that bind one; what a read names, and from which
//! declaration, is this rule's.

use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use crate::code::engine::{Facts, File, Sink};
use crate::code::imports::Resolver;
use crate::code::queries::Kind;
use crate::code::syntax;
use crate::model::EdgeKind;

pub(crate) fn resolve(_: &Resolver, file: &File, facts: &Facts, sink: &mut Sink) {
    let Some(tree) = file.tree else { return };
    let (root, src, rel) = (tree.root_node(), file.src.as_bytes(), file.rel);
    let ids: HashMap<&str, &str> = facts.defs.iter().map(|d| (d.qualified.as_str(), d.id.as_str())).collect();
    let functions: HashSet<&str> = facts.defs.iter().filter(|d| d.kind == Kind::Function).map(|d| d.qualified.as_str()).collect();
    for r in facts.refs.iter().filter(|r| r.edge == EdgeKind::References) {
        let Some(n) = root.descendant_for_byte_range(r.range.start, r.range.end) else { continue };
        let text = syntax::text(n, src);
        let spelled: Vec<String> = match n.kind() {
            // The outer accessor alone is read: `a::b::c` never also names `a::b`.
            "resource_expression" if n.parent().is_some_and(|p| p.kind() == "resource_expression") => continue,
            "resource_expression" => match (n.child_by_field_name("object"), n.child_by_field_name("resource")) {
                (Some(o), Some(c)) => {
                    let parent = syntax::text(o, src);
                    vec![format!("{parent}.{}", syntax::text(c, src)), parent.to_string()]
                }
                _ => continue,
            },
            _ if is_callee(n) => if functions.contains(text) { vec![text.to_string()] } else { continue },
            _ => scoped(n, text, facts),
        };
        let Some((name, target)) = spelled.iter().find_map(|s| ids.get(s.as_str()).map(|t| (s, *t))) else { continue };
        // The name a binder would have to spell: the parent of `vnet::subnet`, or the bare name.
        let spelling = if n.kind() == "identifier" { text } else { name.split('.').next().unwrap_or(name) };
        let from = facts.source_id(rel, facts.owner(decorated(n).unwrap_or(n.start_byte())));
        if from != target && !facts.locals.shadows(spelling, n.start_byte()) {
            sink.edge(&from, target, EdgeKind::References, "", rel);
        }
    }
}

/// The qualified names a bare name may mean at `n`, nearest first.
fn scoped(n: Node, name: &str, facts: &Facts) -> Vec<String> {
    let mut out: Vec<String> = std::iter::successors(n.parent(), |p| p.parent())
        .filter(|p| p.kind() == "resource_declaration")
        .filter_map(|p| facts.declared_at(&p.byte_range()))
        .map(|parent| format!("{}.{name}", facts.defs[parent].qualified))
        .collect();
    out.push(name.to_string());
    out
}

fn is_callee(n: Node) -> bool {
    n.parent().is_some_and(|p| p.kind() == "call_expression" && p.child_by_field_name("function").is_some_and(|f| f.id() == n.id()))
}

/// Where the declaration a decorator decorates starts, for a read inside the decorator. A comment
/// may stand between the two.
fn decorated(n: Node) -> Option<usize> {
    let decorators = std::iter::successors(n.parent(), |p| p.parent()).find(|p| p.kind() == "decorators")?;
    std::iter::successors(decorators.next_named_sibling(), |s| s.next_named_sibling()).find(|s| s.kind() != "comment").map(|s| s.start_byte())
}
