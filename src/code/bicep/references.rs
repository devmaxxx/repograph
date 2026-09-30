//! In-file references. Bicep resolves a symbolic name against the file that spells it, so a name is a
//! reference exactly when a declaration of this file has it and nothing nearer binds it.

use super::Decl;
use crate::code::prose::{self, Spans};
use crate::model::{EdgeKind, Extraction};
use std::collections::BTreeMap;
use tree_sitter::Node;

pub(super) fn write(root: Node, decls: &[Decl], spans: &Spans, src: &[u8], rel: &str, ex: &mut Extraction) {
    let by_name: BTreeMap<&str, &str> = decls.iter().map(|d| (d.name.as_str(), d.id.as_str())).collect();
    let funcs: Vec<&str> = decls.iter().filter(|d| d.node.kind() == "user_defined_function").map(|d| d.name.as_str()).collect();
    // A decorator sits outside its declaration's span, so what it reads is owned from the declaration
    // after it: the second field is that owner's byte while the walk is inside one.
    let mut stack: Vec<(Node, Option<usize>)> = vec![(root, None)];
    while let Some((n, owner)) = stack.pop() {
        let spelled: Vec<String> = match n.kind() {
            "decorators" => {
                let owner = n.next_named_sibling().map(|d| d.start_byte());
                stack.extend(prose::named(n).into_iter().map(|c| (c, owner)));
                continue;
            }
            // `vnet::subnet` names the child; a parent whose child this file does not declare is still named.
            "resource_expression" => match (n.child_by_field_name("object"), n.child_by_field_name("resource")) {
                (Some(o), Some(r)) => {
                    let parent = prose::text(o, src);
                    vec![format!("{parent}.{}", prose::text(r, src)), parent.to_string()]
                }
                _ => Vec::new(),
            },
            // A call names a function, and only a `func` of this file is one: `range(0, 3)` beside
            // `param range` is the built-in.
            "identifier" if is_callee(n) => {
                let name = prose::text(n, src);
                if funcs.contains(&name) { vec![name.to_string()] } else { Vec::new() }
            }
            "identifier" if refers(n) => vec![prose::text(n, src).to_string()],
            "identifier" => Vec::new(),
            _ => {
                stack.extend(prose::named(n).into_iter().map(|c| (c, owner)));
                continue;
            }
        };
        let Some((name, target)) = spelled.iter().find_map(|s| by_name.get(s.as_str()).map(|t| (s, *t))) else { continue };
        let from = spans.owner(owner.unwrap_or_else(|| n.start_byte()));
        if from != target && !shadowed(n, name.split('.').next().unwrap_or(name), src) {
            ex.edge(from, target, EdgeKind::References, "", rel);
        }
    }
}

/// Whether an identifier is read rather than bound: a declaration's own name, an object key, a function
/// or lambda parameter, a loop variable and a decorator's name are not reads.
fn refers(n: Node) -> bool {
    let Some(p) = n.parent() else { return false };
    let first = p.named_child(0).is_some_and(|c| c.id() == n.id());
    match p.kind() {
        k if super::DECLARATIONS.contains(&k) => !first,
        "object_property" | "parameter" => !first,
        "for_statement" => p.child_by_field_name("initializer").is_none_or(|i| i.id() != n.id()),
        "lambda_expression" => is_body(p, n),
        "parenthesized_expression" => !p.parent().is_some_and(|l| l.kind() == "lambda_expression" && !is_body(l, p)),
        // `@description('…')` calls the built-in even in a file that declares `param description`.
        "call_expression" => !p.parent().is_some_and(|g| g.kind() == "decorator"),
        _ => true,
    }
}

fn is_callee(n: Node) -> bool {
    n.parent().is_some_and(|p| p.kind() == "call_expression" && p.child_by_field_name("function").is_some_and(|f| f.id() == n.id()))
}

/// A lambda's last named child is its body; every child before it is a parameter list.
fn is_body(lambda: Node, n: Node) -> bool {
    prose::named(lambda).last().is_some_and(|b| b.id() == n.id())
}

/// Whether a loop variable, lambda parameter or function parameter between the identifier and the file
/// binds `name` first. A binder covers what it scopes over: the loop's body, the lambda's body and the
/// function's body, never the iterable or the parameter list itself.
fn shadowed(n: Node, name: &str, src: &[u8]) -> bool {
    let mut child = n;
    let mut at = n.parent();
    while let Some(a) = at {
        let binders: Vec<Node> = match a.kind() {
            "for_statement" if a.child_by_field_name("body").is_some_and(|b| b.id() == child.id()) => {
                match a.child_by_field_name("initializer") {
                    Some(i) => vec![i],
                    None => a.named_child(0).map(prose::named).unwrap_or_default(),
                }
            }
            "lambda_expression" if is_body(a, child) => {
                let mut params = prose::named(a);
                params.pop();
                params.into_iter().flat_map(|p| if p.kind() == "identifier" { vec![p] } else { prose::named(p) }).collect()
            }
            "user_defined_function" if child.kind() != "parameters" => {
                // `parameters` is a child node here, not a field of the grammar.
                prose::named(a).into_iter().find(|c| c.kind() == "parameters")
                    .map(|ps| prose::named(ps).into_iter().filter_map(|p| p.named_child(0)).collect()).unwrap_or_default()
            }
            _ => Vec::new(),
        };
        if binders.iter().any(|b| prose::text(*b, src) == name) {
            return true;
        }
        child = a;
        at = a.parent();
    }
    false
}
