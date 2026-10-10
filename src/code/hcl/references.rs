//! Terraform resolves a reference against its module, and a module is a directory: `var.region` in
//! `server.tf` names the variable `variables.tf` declares. `Modules` holds every directory's addresses,
//! read once per globbed `.tf` file before any extract, so the answer does not hang on walk order.

use super::{declarations, dir_of, inner};
use crate::code::lang::Lang;
use crate::code::prose::{self, Spans};
use crate::code::syntax;
use crate::model::{EdgeKind, Extraction};
use std::collections::{BTreeMap, BTreeSet};
use tree_sitter::Node;

#[derive(Default)]
pub(crate) struct Modules {
    /// directory → address → the `.tf` files declaring it
    by_dir: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    /// directory → its `.tf` files
    files: BTreeMap<String, BTreeSet<String>>,
}

impl Modules {
    pub(crate) fn add(&mut self, rel: &str, source: &str) {
        if !rel.ends_with(".tf") {
            return;
        }
        let dir = dir_of(rel).to_string();
        self.files.entry(dir.clone()).or_default().insert(rel.to_string());
        let Some(tree) = Lang::Hcl.parse(source.as_bytes()) else { return };
        let addresses = self.by_dir.entry(dir).or_default();
        for (address, _) in declarations(tree.root_node(), source.as_bytes(), true) {
            addresses.entry(address).or_default().push(rel.to_string());
        }
    }
}

pub(super) fn terraform(modules: &Modules, root: Node, own: &BTreeSet<String>, spans: &Spans, src: &[u8], rel: &str, ex: &mut Extraction) {
    let dir = dir_of(rel);
    let file = format!("file:{rel}");
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.kind() == "block" {
            if let Some(target_dir) = module_dir(n, src, rel) {
                let from = spans.owner(n.start_byte());
                for target in modules.files.get(target_dir.as_str()).into_iter().flatten() {
                    // `*`: a module call takes in every declaration of the directory.
                    ex.edge(&file, &format!("file:{target}"), EdgeKind::Imports, "*", rel);
                }
                // The walks follow a reference to a declaration and never an import, so the call reaches
                // the module through what the module declares.
                for (address, declaring) in modules.by_dir.get(target_dir.as_str()).into_iter().flatten() {
                    for target in declaring {
                        ex.edge(from, &format!("sym:{target}::{address}"), EdgeKind::References, "", rel);
                    }
                }
            }
        }
        if n.kind() != "variable_expr" {
            stack.extend(syntax::named(n));
            continue;
        }
        let Some(address) = address(n, src) else { continue };
        if bound_by_iteration(n, src) {
            continue;
        }
        // This file's own declaration first. A sibling's counts only when this file has none, since
        // Terraform refuses a module that declares one address twice.
        let declaring: Vec<&str> = if own.contains(&address) {
            vec![rel]
        } else {
            modules.by_dir.get(dir).and_then(|m| m.get(&address)).into_iter().flatten()
                .map(String::as_str).filter(|f| *f != rel).collect()
        };
        let from = spans.owner(n.start_byte());
        for declared_in in declaring {
            let target = format!("sym:{declared_in}::{address}");
            if target != from {
                ex.edge(from, &target, EdgeKind::References, "", rel);
            }
        }
    }
}

/// The address a traversal names, from its root and the attribute steps after it: `var.region` is
/// `var/region`, `data.aws_ami.u.id` is `data/aws_ami/u`, and `aws_instance.web[0].ip` is
/// `aws_instance/web`. `each`, `count`, `path`, `self` and `terraform` are values of the block, not
/// addresses. A traversal that ends before its address does names nothing.
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
fn trailing_steps<'t>(n: Node<'t>) -> Option<Node<'t>> {
    fn operation_of<'t>(n: Node<'t>) -> Option<Node<'t>> {
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
    let mut at = n.parent();
    while let Some(a) = at {
        let bound = match a.kind() {
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
        };
        if bound {
            return true;
        }
        at = a.parent();
    }
    false
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

/// `module "m" { source = "./dns" }`: the module's directory. Terraform reads only `./` and `../` as
/// local; `modules/dns` would be a registry address. `None` for a path that climbs above the root,
/// which names a directory outside the repository, and for the directory the call is in.
fn module_dir(block: Node, src: &[u8], rel: &str) -> Option<String> {
    let parts = inner(block);
    if parts.first().map(|k| syntax::text(*k, src)) != Some("module") {
        return None;
    }
    let body = parts.iter().find(|c| c.kind() == "body")?;
    let path = syntax::named(*body).into_iter()
        .filter(|a| a.kind() == "attribute")
        .find(|a| a.named_child(0).is_some_and(|k| syntax::text(k, src) == "source"))
        .and_then(|a| a.named_child(1))
        .and_then(|v| literal(v, src))
        .filter(|p| p.starts_with("./") || p.starts_with("../"))?;
    prose::join_under(dir_of(rel), path).filter(|d| d != dir_of(rel))
}

/// A string expression holding written-out text alone.
fn literal<'s>(expr: Node, src: &'s [u8]) -> Option<&'s str> {
    let mut n = expr;
    loop {
        match inner(n).as_slice() {
            [t] if t.kind() == "template_literal" => return Some(syntax::text(*t, src)),
            [one] if matches!(one.kind(), "expression" | "literal_value" | "string_lit" | "template_expr" | "quoted_template") => n = *one,
            _ => return None,
        }
    }
}

/// `inherits = ["base"]` and `targets = ["api"]` in a bake file name a target, or failing that a group,
/// of the same file.
pub(super) fn bake(root: Node, own: &BTreeSet<String>, spans: &Spans, src: &[u8], rel: &str, ex: &mut Extraction) {
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let key = (n.kind() == "attribute").then(|| n.named_child(0)).flatten().map(|k| syntax::text(k, src));
        if !matches!(key, Some("inherits" | "targets")) {
            stack.extend(syntax::named(n));
            continue;
        }
        let from = spans.owner(n.start_byte());
        for item in n.named_child(1).map(|v| syntax::find_all(v, &["tuple"])).unwrap_or_default().first().map(|t| inner(*t)).unwrap_or_default() {
            let Some(name) = literal(item, src) else { continue };
            let Some(address) = ["target", "group"].iter().map(|k| format!("{k}/{name}")).find(|a| own.contains(a)) else { continue };
            let target = format!("sym:{rel}::{address}");
            if target != from {
                ex.edge(from, &target, EdgeKind::References, "", rel);
            }
        }
    }
}
