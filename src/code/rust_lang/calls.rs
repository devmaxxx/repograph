//! What a Rust file's code reaches: an `impl`'s ties to its type and trait, its calls and macro
//! invocations, and the attributes and derives on its items.

use super::crates::Target;
use super::items::owner;
use super::uses::Ctx;
use super::{field_text, inline_of, scoped, text, type_path};
use crate::code::jvm::descend;
use crate::model::{EdgeKind, Extraction};
use std::collections::{BTreeMap, BTreeSet};
use tree_sitter::Node;

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
pub(crate) fn impls(ctx: &Ctx, ex: &mut Extraction) {
    let own = format!("sym:{}::", ctx.rel);
    for imp in &ctx.items.impls {
        let Some(ty) = ctx.resolve(&imp.inline, &imp.ty).filter(|t| written(ctx, t)).as_ref().and_then(id_of) else { continue };
        if !ty.starts_with(&own) {
            for m in &imp.members {
                ex.edge(&format!("{own}{m}"), &ty, EdgeKind::References, "impl", ctx.rel);
            }
        }
        if let Some(tr) = imp.trait_path.as_ref().and_then(|p| ctx.resolve(&imp.inline, p)).filter(|t| written(ctx, t)).as_ref().and_then(id_of) {
            ex.edge(&ty, &tr, EdgeKind::Extends, "", ctx.rel);
        }
    }
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
    if ctx.items.names.contains(&here) {
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
                if ctx.items.names.contains(&here) {
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
    if let Some(suffix) = ctx.items.macros.get(name) {
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

/// Node ids this file writes, so an edge starts and ends only at ids that exist: another file's
/// ids are vouched for by the resolver, this file's by what it declared.
struct Written {
    own: String,
    ids: BTreeSet<String>,
}

impl Written {
    fn new(ctx: &Ctx, ex: &Extraction) -> Written {
        Written { own: format!("sym:{}::", ctx.rel), ids: ex.nodes.iter().map(|n| n.id.clone()).collect() }
    }

    fn target(&self, id: String) -> Option<String> {
        (!id.starts_with(&self.own) || self.ids.contains(&id)).then_some(id)
    }

    /// An `impl` of a type declared elsewhere writes no node for a bare container, so its calls start at the file.
    fn source(&self, rel: &str, id: String) -> String {
        if id.starts_with("file:") || self.ids.contains(&id) { id } else { format!("file:{rel}") }
    }
}

/// Every call and macro invocation in the file, as an edge from its owner to what the scope
/// proves it reaches. A self-call is dropped; a repeated call collapses in the extractor's dedup.
pub(crate) fn read(ctx: &Ctx, root: Node, ex: &mut Extraction) {
    let fields = Fields::read(ctx.src, root);
    let written = Written::new(ctx, ex);
    let mut edges = Vec::new();
    descend(root, &mut |n| {
        let target = match n.kind() {
            "call_expression" => n.child_by_field_name("function").and_then(|f| callee(ctx, &fields, n, f)),
            "macro_invocation" => macro_target(ctx, n),
            _ => None,
        };
        if let Some(target) = target.and_then(|t| written.target(t)) {
            let from = written.source(ctx.rel, owner(n, ctx.rel, ctx.src));
            if from != target {
                edges.push((from, target));
            }
        }
        true
    });
    for (from, to) in edges {
        ex.edge(&from, &to, EdgeKind::Calls, "", ctx.rel);
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

pub(crate) fn attributes(ctx: &Ctx, root: Node, ex: &mut Extraction) {
    let written = Written::new(ctx, ex);
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
        let from = owner(name, ctx.rel, ctx.src);
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
            if let Some(id) = target.as_ref().and_then(id_of).and_then(|t| written.target(t)) {
                found.push((from.clone(), id, path.join("::")));
            }
        }
        false
    });
    for (from, to, context) in found {
        // Only a symbol this file writes is decorated; a bare container has none.
        if written.ids.contains(&from) {
            ex.edge(&from, &to, EdgeKind::DecoratedBy, &context, ctx.rel);
        }
    }
}
