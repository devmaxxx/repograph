//! A Terraform reference is a traversal, and what it names is an address: its root and one or two
//! attribute steps after it, `var.region` naming `var/region` and `data.aws_ami.u.id` naming
//! `data/aws_ami/u`. A module is a directory, so the address resolves against this file and then
//! against every other `.tf` file beside it.
//!
//! The query takes every traversal's root; the default cannot read the address the steps after it
//! spell, nor tell a `for` variable or a `dynamic` iterator from a resource type of the same name.

use tree_sitter::Node;

use crate::code::engine::{Facts, File, Sink};
use crate::code::imports::Resolver;
use crate::code::syntax;
use crate::model::EdgeKind;

pub(crate) fn resolve(r: &Resolver, file: &File, facts: &Facts, sink: &mut Sink) {
    let Some(tree) = file.tree else { return };
    let (root, src, rel) = (tree.root_node(), file.src.as_bytes(), file.rel);
    let mut siblings = None;
    for reference in facts.refs.iter().filter(|r| r.edge == EdgeKind::References) {
        let Some(n) = traversal(root, &reference.range) else { continue };
        let Some(address) = address(n, src) else { continue };
        if bound_by_iteration(n, src) {
            continue;
        }
        // This file's own declaration first. A sibling's counts only when this file has none, since
        // Terraform refuses a module that declares one address twice.
        let declaring: Vec<&str> = if facts.declares(&address) {
            vec![rel]
        } else {
            let siblings = siblings.get_or_insert_with(|| {
                r.siblings(rel, file.lang).into_iter().filter_map(|s| r.facts(&s).map(|f| (s, f))).collect::<Vec<_>>()
            });
            siblings.iter().filter(|(_, f)| f.declares(&address)).map(|(s, _)| s.as_str()).collect()
        };
        let from = facts.source_id(rel, reference.source);
        for declared_in in declaring {
            let target = format!("sym:{declared_in}::{address}");
            if target != from {
                sink.edge(&from, &target, EdgeKind::References, "", rel);
            }
        }
    }
}

/// The `variable_expr` the query captured, which spans exactly what its root identifier does.
fn traversal<'t>(root: Node<'t>, range: &std::ops::Range<usize>) -> Option<Node<'t>> {
    let mut n = root.descendant_for_byte_range(range.start, range.end)?;
    while n.kind() != "variable_expr" {
        n = n.parent().filter(|p| p.byte_range() == *range)?;
    }
    Some(n)
}

/// The address a traversal names, from its root and the attribute steps after it:
/// `aws_instance.web[0].ip` is `aws_instance/web`. `each`, `count`, `path`, `self` and `terraform`
/// are values of the block, not addresses, and a traversal that ends before its address does names
/// nothing.
fn address(n: Node, src: &[u8]) -> Option<String> {
    let root = syntax::text(n, src);
    let mut steps = Vec::new();
    let mut at = n.next_named_sibling().or_else(|| trailing_steps(n));
    while let Some(step) = at.filter(|s| s.kind() == "get_attr") {
        steps.push(syntax::text(step.named_child(0)?, src));
        at = step.next_named_sibling();
    }
    let take = |k: usize| (steps.len() >= k).then(|| steps[..k].join("/"));
    match root {
        "each" | "count" | "path" | "self" | "terraform" => None,
        "data" => Some(format!("data/{}", take(2)?)),
        root => Some(format!("{root}/{}", take(1)?)),
    }
}

/// The grammar hangs the attribute steps after an operation's last operand on the operation, not on
/// the operand: in `a == c.d` and in `!c.d` the `.d` follows the whole `a == c` or `!c`, and when that
/// operation is itself the last operand of another, on the outermost.
fn trailing_steps(n: Node) -> Option<Node> {
    fn operation_of(n: Node) -> Option<Node> {
        n.parent()
            .filter(|p| matches!(p.kind(), "binary_operation" | "unary_operation"))?
            .parent().filter(|p| p.kind() == "operation")
    }
    let mut operation = operation_of(n)?;
    loop {
        if let Some(next) = operation.next_named_sibling() {
            return Some(next);
        }
        operation = operation_of(operation)?;
    }
}

/// Whether a `for` expression's variable or a `dynamic` block's iterator binds the root first: `x.name`
/// inside `[for x in var.list : x.name]` reads the loop's `x`, never a resource type named `x`.
fn bound_by_iteration(n: Node, src: &[u8]) -> bool {
    let name = syntax::text(n, src);
    std::iter::successors(n.parent(), |a| a.parent()).any(|a| match a.kind() {
        // `for_intro` is a sibling of the body and condition, not their ancestor. Its own collection
        // is evaluated outside the loop, so only what stands beside it is bound.
        "for_tuple_expr" | "for_object_expr" => syntax::named(a).into_iter()
            .find(|c| c.kind() == "for_intro")
            .is_some_and(|intro| {
                let inside = intro.start_byte() <= n.start_byte() && n.end_byte() <= intro.end_byte();
                !inside && syntax::named(intro).into_iter().any(|c| c.kind() == "identifier" && syntax::text(c, src) == name)
            }),
        "block" => dynamic_iterator(a, src).is_some_and(|i| i == name),
        _ => false,
    })
}

/// A `dynamic "label"` block's iterator: `iterator = x` when written, else the label.
fn dynamic_iterator<'s>(block: Node, src: &'s [u8]) -> Option<&'s str> {
    let parts = inner(block);
    if parts.first().map(|k| syntax::text(*k, src)) != Some("dynamic") {
        return None;
    }
    let body = parts.iter().find(|c| c.kind() == "body");
    let explicit = body.into_iter().flat_map(|b| syntax::named(*b)).filter(|a| a.kind() == "attribute")
        .find(|a| a.named_child(0).is_some_and(|k| syntax::text(k, src) == "iterator"))
        .and_then(|a| a.named_child(1))
        .map(|v| syntax::text(v, src));
    explicit.or_else(|| parts.get(1).and_then(|l| inner(*l).first().map(|t| syntax::text(*t, src))))
}

/// Named children less the delimiters the grammar names, such as `block_start` and
/// `quoted_template_end`.
fn inner(n: Node) -> Vec<Node> {
    syntax::named(n).into_iter().filter(|c| !c.kind().ends_with("_start") && !c.kind().ends_with("_end")).collect()
}
