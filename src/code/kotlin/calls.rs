//! Kotlin's calls and cited ids, one walk over the tree the declarations pass read.
//!
//! A call edge is written only where the file proves its target, as C#'s refs pass does: a name
//! bound by something whose type this file does not read — `it`, an untyped lambda parameter, a
//! destructured or `for` variable, a `when` subject, a member of an anonymous object — hides the
//! property, type or function it shadows, and a call through it writes nothing.

use std::collections::BTreeMap;

use tree_sitter::Node;

use super::declarations::{holder, is_type, members_of, one_line_object, written_type, Declared};
use crate::code::jvm::{self, child, named, outer, path_of, qualify, split_id, text, type_ids, Own};
use crate::model::{EdgeKind, Extraction};

pub(super) struct Ctx<'a> {
    pub own: Own<'a>,
    pub src: &'a [u8],
    pub d: &'a Declared,
}

/// A local's type as ids of the declarations it resolves to, or `None` when the file does not
/// read it; either way the name hides whatever it shadows.
type Locals = BTreeMap<String, Option<Vec<String>>>;

/// Where the walk is: the type path around it, the declared function calls come from, the locals
/// the statements seen so far bind, and whether `this` has left the declared type — inside an
/// object literal or a local class it names that anonymous type.
#[derive(Clone, Default)]
struct At {
    class: String,
    function: Option<String>,
    locals: Locals,
    opaque: bool,
}

/// A declaration the declarations pass reached: top level, or in the body of a declared type. A
/// local `fun`, or one inside an `object :` literal or a lambda, is not a symbol, so its calls
/// belong to the declared function around it and no `Calls` edge starts at an id no node has.
fn is_declared(n: Node) -> bool {
    holder(n).is_some_and(|h| h.kind() == "source_file" || ((is_type(h) || h.kind() == "companion_object") && is_declared(h)))
}

pub(super) fn scan(root: Node, cx: &Ctx, ex: &mut Extraction) {
    walk(root, At::default(), cx, ex);
}

fn walk(n: Node, mut at: At, cx: &Ctx, ex: &mut Extraction) {
    match n.kind() {
        _ if is_type(n) && is_declared(n) => {
            let name = one_line_object(n).map(|(name, _)| name).or_else(|| child(n, "type_identifier"));
            if let Some(name) = name {
                let name = text(name, cx.src);
                at.class = if at.class.is_empty() { name.to_string() } else { format!("{}.{name}", at.class) };
                at.function = None;
                at.locals.clear();
                at.opaque = false;
            }
        }
        "class_declaration" | "object_declaration" | "object_literal" => {
            at.opaque = true;
            for m in child(n, "class_body").map(members_of).unwrap_or_default() {
                if let Some(name) = declared_name(m, cx.src) {
                    at.locals.insert(name, None);
                }
            }
            for p in child(n, "primary_constructor").map(named).unwrap_or_default() {
                if let Some(name) = child(p, "simple_identifier") {
                    at.locals.insert(text(name, cx.src).to_string(), None);
                }
            }
        }
        "function_declaration" if is_declared(n) => {
            if let Some(s) = child(n, "simple_identifier") {
                let name = text(s, cx.src);
                let path = if at.class.is_empty() { name.to_string() } else { format!("{}.{name}", at.class) };
                at.function = Some(format!("sym:{}::{path}", cx.own.rel));
                at.locals.clear();
                parameters(n, &mut at, cx);
            }
        }
        "function_declaration" | "anonymous_function" => parameters(n, &mut at, cx),
        "lambda_literal" if !n.parent().is_some_and(|p| one_line_object(p).is_some()) => match child(n, "lambda_parameters") {
            Some(ps) => {
                for p in named(ps) {
                    bind(p, &mut at, cx);
                }
            }
            None => {
                at.locals.insert("it".to_string(), None);
            }
        },
        "for_statement" => {
            for v in named(n).into_iter().filter(|c| matches!(c.kind(), "variable_declaration" | "multi_variable_declaration")) {
                bind(v, &mut at, cx);
            }
        }
        "when_expression" => {
            if let Some(v) = child(n, "when_subject").and_then(|s| child(s, "variable_declaration")) {
                bind(v, &mut at, cx);
            }
        }
        "catch_block" => {
            if let Some(name) = child(n, "simple_identifier") {
                let ty = written_type(n, cx.src).map(|t| resolved(&t, &at, cx));
                at.locals.insert(text(name, cx.src).to_string(), ty);
            }
        }
        "call_expression" => {
            if let (Some(from), Some(callee)) = (at.function.clone(), n.named_child(0)) {
                for to in targets(callee, &at, cx) {
                    // A recursive call says nothing about what the function depends on.
                    if to != from {
                        ex.edge(&from, &to, EdgeKind::Calls, "", cx.own.rel);
                    }
                }
            }
        }
        "line_comment" | "multiline_comment" => jvm::cite(n, &owner(&at, cx.own.rel), "comment", cx.own.rel, cx.src, ex),
        "string_literal" => jvm::cite(n, &owner(&at, cx.own.rel), "string", cx.own.rel, cx.src, ex),
        _ => {}
    }
    for c in named(n) {
        // A local is seen by the statements after it, so it joins the `At` its later siblings get.
        if at.function.is_some() && !is_declared(c) {
            remember(c, &mut at, cx);
        }
        walk(c, at.clone(), cx, ex);
    }
}

fn owner(at: &At, rel: &str) -> String {
    match (&at.function, at.class.is_empty()) {
        (Some(f), _) => f.clone(),
        (None, false) => format!("sym:{rel}::{}", at.class),
        (None, true) => format!("file:{rel}"),
    }
}

/// The name a member of an anonymous or local type declares.
fn declared_name(m: Node, src: &[u8]) -> Option<String> {
    let name = match m.kind() {
        "function_declaration" => child(m, "simple_identifier"),
        "property_declaration" => child(m, "variable_declaration").and_then(|v| child(v, "simple_identifier")),
        "class_declaration" | "object_declaration" => child(m, "type_identifier"),
        _ => None,
    }?;
    Some(text(name, src).to_string())
}

/// A type as written, resolved where the walk is.
fn resolved(written: &str, at: &At, cx: &Ctx) -> Vec<String> {
    type_ids(cx.own.types, cx.own.index, cx.own.scope, cx.own.rel, &at.class, written)
}

fn parameters(f: Node, at: &mut At, cx: &Ctx) {
    for p in child(f, "function_value_parameters").map(named).unwrap_or_default() {
        if let Some(name) = child(p, "simple_identifier") {
            // A function-typed parameter reads as no type, so `onDone()` through it is a call through a value.
            let ty = written_type(p, cx.src).map(|t| resolved(&t, at, cx));
            at.locals.insert(text(name, cx.src).to_string(), ty);
        }
    }
}

/// A `variable_declaration` binds its name, typed when written; a destructuring binds each name untyped.
fn bind(v: Node, at: &mut At, cx: &Ctx) {
    match v.kind() {
        "variable_declaration" => {
            if let Some(name) = child(v, "simple_identifier") {
                let ty = written_type(v, cx.src).map(|t| resolved(&t, at, cx));
                at.locals.insert(text(name, cx.src).to_string(), ty);
            }
        }
        "multi_variable_declaration" => {
            for inner in named(v) {
                bind(inner, at, cx);
            }
        }
        _ => {}
    }
}

/// `val s: Store = …` takes its annotation; `val s = Store()` the class it constructs; any other
/// local is known and untyped. A local `fun` or class hides the member or top-level name it shares.
fn remember(n: Node, at: &mut At, cx: &Ctx) {
    match n.kind() {
        "property_declaration" => {
            if let Some(v) = child(n, "multi_variable_declaration") {
                return bind(v, at, cx);
            }
            let Some(v) = child(n, "variable_declaration") else { return };
            let Some(name) = child(v, "simple_identifier").map(|s| text(s, cx.src).to_string()) else { return };
            let ty = match written_type(v, cx.src) {
                Some(t) => Some(resolved(&t, at, cx)),
                None => constructed(n, at, cx),
            };
            at.locals.insert(name, ty);
        }
        "function_declaration" | "class_declaration" | "object_declaration" => {
            if let Some(name) = declared_name(n, cx.src) {
                at.locals.insert(name, None);
            }
        }
        _ => {}
    }
}

/// The class `val s = Store()` constructs: the callee's targets, when every one is a type.
fn constructed(n: Node, at: &At, cx: &Ctx) -> Option<Vec<String>> {
    let callee = child(n, "call_expression")?.named_child(0).filter(|c| c.kind() == "simple_identifier")?;
    let ids = targets(callee, at, cx);
    (!ids.is_empty() && ids.iter().all(|id| is_type_id(id, cx))).then_some(ids)
}

fn is_type_id(id: &str, cx: &Ctx) -> bool {
    split_id(id).is_some_and(|(rel, path)| if rel == cx.own.rel { cx.own.types.contains(path) } else { cx.own.index.is_type(rel, path) })
}

/// The ids a callee reaches: `f()` and `Type()`; `x.m()` and `this.x.m()` through a parameter, a
/// local or a property with a declared type; `Obj.m()` and `Type.m()` on an object or a companion.
fn targets(callee: Node, at: &At, cx: &Ctx) -> Vec<String> {
    match callee.kind() {
        "simple_identifier" => {
            let name = text(callee, cx.src);
            if at.locals.contains_key(name) {
                return Vec::new();
            }
            if let Some(ids) = cx.own.member(&at.class, name) {
                return ids;
            }
            top_level(name, cx)
        }
        "navigation_expression" => {
            let parts = named(callee);
            let (Some(receiver), Some(suffix)) = (parts.first().copied(), parts.last().copied()) else { return Vec::new() };
            let Some(method) = child(suffix, "simple_identifier").map(|m| text(m, cx.src)) else { return Vec::new() };
            if is_this(receiver, cx) {
                return if at.opaque { Vec::new() } else { cx.own.member(&at.class, method).unwrap_or_default() };
            }
            receiver_types(receiver, at, cx).map(|ids| cx.own.on_types(&ids, method)).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// Plain `this`: a labelled `this@Outer` or `super` is some other receiver.
fn is_this(n: Node, cx: &Ctx) -> bool {
    n.kind() == "this_expression" && text(n, cx.src) == "this"
}

/// A name no enclosing type holds, read as Kotlin does past the members: this file's own top level
/// or an explicit import, the file's package, then the star imports. A name both this file and an
/// import bind, or two stars bind, is an overload the file cannot settle, and writes nothing.
fn top_level(name: &str, cx: &Ctx) -> Vec<String> {
    let (own, scope, index) = (&cx.own, cx.own.scope, cx.own.index);
    let imported = scope.singles.get(name);
    if cx.d.top.contains(name) {
        return if imported.is_some() { Vec::new() } else { vec![format!("sym:{}::{name}", own.rel)] };
    }
    if let Some(bound) = imported {
        let mut each = bound.iter();
        return match (each.next(), each.next()) {
            (Some(q), None) => jvm::declared(index, q).iter().map(jvm::Target::id).collect(),
            _ => Vec::new(),
        };
    }
    let at = |base: &str| -> Vec<String> { index.files(&qualify(base, name)).into_iter().map(|rel| format!("sym:{rel}::{name}")).collect() };
    let package = at(&scope.package);
    if !package.is_empty() {
        return package;
    }
    let mut starred: Vec<Vec<String>> = scope.stars.iter().map(|s| at(s)).filter(|ids| !ids.is_empty()).collect();
    if starred.len() == 1 { starred.remove(0) } else { Vec::new() }
}

/// The types a receiver stands for: a local, then a property of the enclosing types, then a
/// top-level property or type of this file, then a capitalised name read as a type. A value of
/// unknown type, or a lowercase name none of those binds, claims no call.
fn receiver_types(receiver: Node, at: &At, cx: &Ctx) -> Option<Vec<String>> {
    match receiver.kind() {
        "simple_identifier" => {
            let name = text(receiver, cx.src);
            if let Some(local) = at.locals.get(name) {
                return local.clone();
            }
            if let Some(ids) = cx.own.member(&at.class, name) {
                return typed_member(&ids, cx);
            }
            if cx.own.types.contains(name) {
                return Some(vec![format!("sym:{}::{name}", cx.own.rel)]);
            }
            if cx.d.top.contains(name) {
                let t = cx.d.fields.get("").and_then(|f| f.get(name))?;
                return Some(type_ids(cx.own.types, cx.own.index, cx.own.scope, cx.own.rel, "", t));
            }
            name.starts_with(|c: char| c.is_ascii_uppercase()).then(|| resolved(name, at, cx))
        }
        // `this.x` is the property `x`, whatever local shadows it.
        "navigation_expression" => {
            let parts = named(receiver);
            let [this, suffix] = parts.as_slice() else { return None };
            if !is_this(*this, cx) || at.opaque {
                return None;
            }
            let prop = child(*suffix, "simple_identifier").map(|s| text(s, cx.src))?;
            typed_member(&cx.own.member(&at.class, prop)?, cx)
        }
        _ => None,
    }
}

/// What a member found by `Own::member` stands for as a receiver: a nested type is itself, a
/// property of this file its declared type read where it is declared. A member of another file
/// has no type the index holds.
fn typed_member(ids: &[String], cx: &Ctx) -> Option<Vec<String>> {
    let [id] = ids else { return None };
    let (rel, path) = split_id(id)?;
    if rel != cx.own.rel {
        return None;
    }
    if cx.own.types.contains(path) {
        return Some(vec![id.clone()]);
    }
    let declared_in = outer(path);
    let name = path_of(id).rsplit('.').next()?;
    let t = cx.d.fields.get(declared_in)?.get(name)?;
    Some(type_ids(cx.own.types, cx.own.index, cx.own.scope, cx.own.rel, declared_in, t))
}
