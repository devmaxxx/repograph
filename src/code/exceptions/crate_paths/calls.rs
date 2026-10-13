//! What a Rust file's code reaches: an `impl`'s ties to its type and trait, its calls and macro
//! invocations, and the attributes and derives on its items.

use std::collections::BTreeMap;

use tree_sitter::Node;

use super::uses::Ctx;
use super::{inline_of, scoped, type_path, Target};
use crate::code::engine::Sink;
use crate::code::syntax::{descend, field_text, named, text};
use crate::model::EdgeKind;

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
pub(crate) fn impls(ctx: &Ctx, root: Node, sink: &mut Sink) {
    let own = format!("sym:{}::", ctx.rel);
    descend(root, &mut |n| {
        if n.kind() != "impl_item" {
            return matches!(n.kind(), "source_file" | "mod_item" | "declaration_list");
        }
        let inline = inline_of(n, ctx.src);
        let ty_path = n.child_by_field_name("type").map(|t| type_path(t, ctx.src)).unwrap_or_default();
        if ty_path.is_empty() {
            return false;
        }
        let Some(ty) = ctx.resolve(&inline, &ty_path).filter(|t| written(ctx, t)).as_ref().and_then(id_of) else { return false };
        if !ty.starts_with(&own) {
            let members = n.child_by_field_name("body").map(named).unwrap_or_default();
            for m in members.into_iter().filter_map(|m| ctx.facts.declared_at(&m.byte_range())) {
                sink.edge(&ctx.facts.defs[m].id, &ty, EdgeKind::References, "impl", ctx.rel);
            }
        }
        let trait_path = n.child_by_field_name("trait").map(|t| type_path(t, ctx.src)).filter(|p| !p.is_empty());
        if let Some(tr) = trait_path.and_then(|p| ctx.resolve(&inline, &p)).filter(|t| written(ctx, t)).as_ref().and_then(id_of) {
            sink.edge(&ty, &tr, EdgeKind::Extends, "", ctx.rel);
        }
        false
    });
}

/// Struct name → field → the type the field's methods are called on.
struct Fields(BTreeMap<String, BTreeMap<String, Vec<String>>>);

/// Wrappers a method call goes through: `self.store.get()` on an `Arc<Store>` calls `Store::get`.
const TRANSPARENT: [&str; 3] = ["Box", "Arc", "Rc"];

fn first_named<'t>(n: Node<'t>) -> Option<Node<'t>> {
    let mut c = n.walk();
    let first = n.named_children(&mut c).find(|x| x.kind() != "lifetime");
    first
}

fn peel(t: Node, src: &[u8]) -> Vec<String> {
    match t.kind() {
        "reference_type" | "pointer_type" => t.child_by_field_name("type").map_or_else(Vec::new, |x| peel(x, src)),
        "dynamic_type" | "abstract_type" => t.child_by_field_name("trait").map_or_else(Vec::new, |x| peel(x, src)),
        "generic_type" => {
            let base = type_path(t, src);
            if base.last().is_some_and(|b| TRANSPARENT.contains(&b.as_str())) {
                if let Some(inner) = t.child_by_field_name("type_arguments").and_then(first_named) {
                    return peel(inner, src);
                }
            }
            base
        }
        _ => type_path(t, src),
    }
}

/// A struct's type parameters → the path of each one's first bound, from `<E: Exec>` or `where E: Exec`.
fn bounds_of(item: Node, src: &[u8]) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    let mut add = |name: &str, bounds: Option<Node>| {
        let Some(b) = bounds else { return };
        if let Some(path) = first_named(b).map(|f| peel(f, src)).filter(|p| !p.is_empty()) {
            out.entry(name.to_string()).or_insert(path);
        }
    };
    if let Some(params) = item.child_by_field_name("type_parameters") {
        let mut c = params.walk();
        for p in params.named_children(&mut c).filter(|p| p.kind() == "type_parameter") {
            if let Some(name) = field_text(p, "name", src) {
                add(name, p.child_by_field_name("bounds"));
            }
        }
    }
    let mut c = item.walk();
    for w in item.named_children(&mut c).filter(|w| w.kind() == "where_clause") {
        let mut wc = w.walk();
        for pred in w.named_children(&mut wc).filter(|p| p.kind() == "where_predicate") {
            if let Some(left) = pred.child_by_field_name("left") {
                add(text(left, src), pred.child_by_field_name("bounds"));
            }
        }
    }
    out
}

impl Fields {
    fn read(src: &[u8], root: Node) -> Fields {
        let mut out = BTreeMap::new();
        descend(root, &mut |n| {
            if n.kind() != "struct_item" {
                return true;
            }
            let Some(name) = field_text(n, "name", src) else { return true };
            let Some(body) = n.child_by_field_name("body").filter(|b| b.kind() == "field_declaration_list") else { return true };
            let bounds = bounds_of(n, src);
            let mut fields = BTreeMap::new();
            let mut fc = body.walk();
            for f in body.named_children(&mut fc).filter(|f| f.kind() == "field_declaration") {
                let (Some(field), Some(ty)) = (field_text(f, "name", src), f.child_by_field_name("type")) else { continue };
                let mut path = peel(ty, src);
                // A field typed by the struct's own parameter is called through the parameter's bound.
                if let [param] = path.as_slice() {
                    if let Some(bound) = bounds.get(param) {
                        path = bound.clone();
                    }
                }
                if !path.is_empty() {
                    fields.insert(field.to_string(), path);
                }
            }
            out.insert(name.to_string(), fields);
            true
        });
        Fields(out)
    }

    fn type_of(&self, owner: &str, field: &str) -> Option<&Vec<String>> {
        self.0.get(owner)?.get(field)
    }
}

/// The type `self` and `Self` stand for at `n`: the enclosing `impl`'s type, or the trait.
fn self_type(n: Node, src: &[u8]) -> Option<Vec<String>> {
    let mut at = n;
    while let Some(p) = at.parent() {
        match p.kind() {
            "impl_item" => return p.child_by_field_name("type").map(|t| type_path(t, src)),
            "trait_item" => return field_text(p, "name", src).map(|s| vec![s.to_string()]),
            _ => at = p,
        }
    }
    None
}

/// `T.m` for a method called on a value of type `ty`: this scope's own member when the file
/// declares it, else `m` under wherever `T` resolves. A type that resolves nowhere in the
/// repository (`String`, a dependency's) is an unknown receiver.
fn member(ctx: &Ctx, inline: &[String], ty: &[String], method: &str) -> Option<String> {
    let last = ty.last()?;
    let here = scoped(inline, &format!("{last}.{method}"));
    if ctx.local.names.contains(&here) {
        return Some(format!("sym:{}::{here}", ctx.rel));
    }
    let Target::Item { file, name } = ctx.resolve(inline, ty)? else { return None };
    if name.contains('.') {
        return None;
    }
    Some(format!("sym:{file}::{name}.{method}"))
}

fn callee(ctx: &Ctx, fields: &Fields, call: Node, f: Node) -> Option<String> {
    let inline = inline_of(call, ctx.src);
    match f.kind() {
        "identifier" | "scoped_identifier" | "generic_function" => {
            let mut path = type_path(f, ctx.src);
            // A module prefix may name another `Store`, so only `T::m` and `Self::m` take the local
            // node; a longer path is the resolver's to answer.
            let bare_member = path.len() == 2;
            if path.first().is_some_and(|s| s == "Self") {
                path.splice(0..1, self_type(call, ctx.src)?);
            }
            // A foreign `impl T` writes `T.m` under this file, so `T::m` is that node; the type's
            // declaring file holds no `m` for a resolver to land on.
            if let ([.., ty, method], true) = (path.as_slice(), bare_member) {
                let here = scoped(&inline, &format!("{ty}.{method}"));
                if ctx.local.names.contains(&here) {
                    return Some(format!("sym:{}::{here}", ctx.rel));
                }
            }
            id_of(&ctx.resolve(&inline, &path)?)
        }
        "field_expression" => {
            let method = text(f.child_by_field_name("field")?, ctx.src);
            let value = f.child_by_field_name("value")?;
            let ty = match value.kind() {
                "self" => self_type(call, ctx.src)?,
                "field_expression" if value.child_by_field_name("value").is_some_and(|v| v.kind() == "self") => {
                    let field = text(value.child_by_field_name("field")?, ctx.src);
                    let owner_type = self_type(call, ctx.src)?;
                    fields.type_of(owner_type.last()?, field)?.clone()
                }
                _ => return None,
            };
            member(ctx, &inline, &ty, method)
        }
        _ => None,
    }
}

fn macro_target(ctx: &Ctx, n: Node) -> Option<String> {
    let m = n.child_by_field_name("macro")?;
    if m.kind() != "identifier" {
        let path = type_path(m, ctx.src);
        if let Some(id) = ctx.resolve(&inline_of(n, ctx.src), &path).as_ref().and_then(id_of) {
            return Some(id);
        }
        // `#[macro_export]` roots a macro at its crate whichever file writes it, so `crate::m!`
        // names a file the path cannot.
        return match path.as_slice() {
            [first, name] if first == "crate" => crate_macro(ctx, name),
            _ => None,
        };
    }
    let name = text(m, ctx.src);
    // Textual scope: this file's own definition first, then one the crate holds unambiguously.
    if let Some(suffix) = ctx.local.macros.get(name) {
        return Some(format!("sym:{}::{suffix}", ctx.rel));
    }
    crate_macro(ctx, name)
}

fn crate_macro(ctx: &Ctx, name: &str) -> Option<String> {
    match ctx.crates.macro_files(ctx.rel, name).as_slice() {
        [one] => Some(format!("sym:{one}::{name}")),
        _ => None,
    }
}

/// `id` when a node is written for it: another file's ids are vouched for by the resolver, this
/// file's by what it declared.
fn landed(ctx: &Ctx, id: String) -> Option<String> {
    match id.strip_prefix(&format!("sym:{}::", ctx.rel)) {
        Some(suffix) if !ctx.local.names.contains(suffix) => None,
        _ => Some(id),
    }
}

/// Every call and macro invocation in the file, as an edge from its owner to what the scope
/// proves it reaches. A self-call is dropped; a repeated call collapses in the extractor's dedup.
pub(crate) fn read(ctx: &Ctx, root: Node, sink: &mut Sink) {
    let fields = Fields::read(ctx.src, root);
    let mut edges = Vec::new();
    descend(root, &mut |n| {
        let target = match n.kind() {
            "call_expression" => n.child_by_field_name("function").and_then(|f| callee(ctx, &fields, n, f)),
            "macro_invocation" => macro_target(ctx, n),
            _ => None,
        };
        if let Some(target) = target.and_then(|t| landed(ctx, t)) {
            let from = ctx.facts.source_id(ctx.rel, ctx.facts.owner(n.start_byte()));
            if from != target {
                edges.push((from, target));
            }
        }
        true
    });
    for (from, to) in edges {
        sink.edge(&from, &to, EdgeKind::Calls, "", ctx.rel);
    }
}

/// The paths a `derive(...)` lists. Its arguments are a flat token tree, so `a::B, C` is read by
/// collecting identifiers and splitting at the commas.
fn derive_paths(args: Node, src: &[u8]) -> Vec<Vec<String>> {
    let mut out = vec![Vec::new()];
    let mut c = args.walk();
    for t in args.children(&mut c) {
        match t.kind() {
            "identifier" => out.last_mut().expect("starts with one path").push(text(t, src).to_string()),
            "," => out.push(Vec::new()),
            _ => {}
        }
    }
    out.into_iter().filter(|p| !p.is_empty()).collect()
}

/// Items an attribute can decorate as a symbol; a field or variant has none of its own.
const DECORATED: [&str; 9] =
    ["function_item", "function_signature_item", "struct_item", "enum_item", "union_item", "trait_item", "type_item", "const_item", "static_item"];

pub(crate) fn attributes(ctx: &Ctx, root: Node, sink: &mut Sink) {
    let mut found = Vec::new();
    descend(root, &mut |n| {
        if n.kind() != "attribute_item" {
            return true;
        }
        // An outer attribute decorates the next sibling that is neither an attribute nor a comment.
        let mut next = n.next_named_sibling();
        while let Some(s) = next.filter(|s| matches!(s.kind(), "attribute_item" | "line_comment" | "block_comment")) {
            next = s.next_named_sibling();
        }
        let Some(name) = next.filter(|i| DECORATED.contains(&i.kind())).and_then(|item| item.child_by_field_name("name")) else { return false };
        // Only a symbol is decorated: an item in an `extern` block or a function body has none.
        let Some(from) = ctx.facts.owner(name.start_byte()).map(|i| ctx.facts.defs[i].id.clone()) else { return false };
        let Some(attr) = n.named_child(0).filter(|a| a.kind() == "attribute") else { return false };
        let Some(head) = attr.named_child(0).map(|p| type_path(p, ctx.src)) else { return false };
        let paths = match (head.as_slice(), attr.child_by_field_name("arguments")) {
            ([d], Some(args)) if d == "derive" => derive_paths(args, ctx.src),
            _ => vec![head],
        };
        let inline = inline_of(n, ctx.src);
        for path in paths {
            // The macro namespace: a single name is only what a `use` binds, never a local item.
            let target = match path.as_slice() {
                [one] => ctx.bindings.names.get(&inline.join("/")).and_then(|m| m.get(one)).cloned(),
                _ => ctx.resolve(&inline, &path),
            };
            if let Some(id) = target.as_ref().and_then(id_of).and_then(|t| landed(ctx, t)) {
                found.push((from.clone(), id, path.join("::")));
            }
        }
        false
    });
    for (from, to, context) in found {
        sink.edge(&from, &to, EdgeKind::DecoratedBy, &context, ctx.rel);
    }
}
