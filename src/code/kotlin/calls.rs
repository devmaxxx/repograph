//! Kotlin's calls and cited ids, one walk over the tree the declarations pass read.
//!
//! A call edge is written only where the file proves its target; a missing edge is acceptable, a
//! wrong one is not. So:
//! - A name bound by something whose type this file does not read — `it`, an untyped lambda
//!   parameter, a destructured or `for` variable, a `when` subject, a member of an anonymous
//!   object — hides the property, type or function it shadows, and a call through it writes nothing.
//! - Kotlin resolves a bare name level by level — locals, implicit receivers innermost first, the
//!   enclosing types' members, then the top level — and moves outward when no candidate at a level
//!   takes the arguments. A member binds only a function whose parameter count, read with its
//!   defaults and `vararg`, admits the call's arguments, the type's own before a supertype's. A name two
//!   levels bind is no edge, and so is one a level where no function takes the arguments may bind
//!   through a supertype or a receiver the file cannot read. The residual: overloads of one arity
//!   told apart only by their argument types bind the first level's function.
//! - A lambda passed to anything but a closed list of stdlib functions whose lambda has no
//!   receiver may run with a receiver this file never sees, so it refuses lowercase bare names and
//!   `this.` calls. A capitalised callee is still resolved there: receiver scopes do not declare
//!   capitalised members, and refusing constructors and Composables would drop nearly every Compose
//!   call. That is the residual this walk accepts.
//!
//! - A property's initializer, delegate and accessors are walked as a function body is, from the
//!   property. A name read as a value — an object passed as an argument, a property read bare —
//!   writes `References` from the declaration around it to what the same lookup binds, never to a
//!   function, since Kotlin reads a value among properties, objects and types only.
//! - A type named in a signature — a parameter's, a property's or a variable's type, a return type,
//!   an extension receiver, and their generic arguments — writes the file-to-file `Imports` an
//!   import of it writes, resolved as a supertype is, with type parameters and local classes masked.
//!
//! Edges it leaves out: calls on a value whose type is inferred from anything but a constructor
//! call, receivers typed by another file's properties, a lambda passed to another file's function
//! taking `T.() -> R` (read as an unknown receiver), a companion's members from a nested type, and
//! any bare call from a type whose supertype chain reaches a type the repository does not declare
//! before a level that declares the name.

use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use super::declarations::{holder, is_type, members_of, one_line_object, receiver, type_params, written_type, Declared, Param};
use crate::code::index::Call;
use crate::code::jvm::{self, child, named, outer, path_of, qualify, split_id, text, type_ids, Bound, Own};
use crate::model::{EdgeKind, Extraction};

pub(super) struct Ctx<'a> {
    pub own: Own<'a>,
    pub src: &'a [u8],
    pub d: &'a Declared,
}

/// A local's type as ids of the declarations it resolves to, or `None` when the file does not
/// read it; either way the name hides whatever it shadows, an implicit receiver's member included.
type Locals = BTreeMap<String, Option<Vec<String>>>;

/// An implicit receiver a function or lambda brings into scope.
#[derive(Clone)]
enum Receiver {
    /// A lambda whose receiver this file does not see.
    Unknown,
    /// The receiver's types; empty when it is written but unread, so it may hold any name.
    Types(Vec<String>),
}

/// Where the walk is: the type path around it, the declared function calls come from, the locals
/// and receivers in scope, the type parameters a written type may name, and whether `this` has
/// left the declared type — inside an object literal or a local class it names that anonymous type.
#[derive(Clone, Default)]
struct At {
    class: String,
    function: Option<String>,
    locals: Locals,
    receivers: Vec<Receiver>,
    type_params: BTreeSet<String>,
    opaque: bool,
}

impl At {
    fn bind(&mut self, name: &str, ty: Option<Vec<String>>) {
        self.locals.insert(name.to_string(), ty);
    }
}

/// Stdlib and Compose functions whose lambda has no receiver, so it sees the names around it.
/// `runCatching` is here only called bare: `x.runCatching {}` takes `x` as its receiver.
const PLAIN: [&str; 25] = [
    "let", "also", "any", "all", "none", "takeIf", "takeUnless", "flatMap", "zip", "fold", "reduce", "groupBy", "sumOf", "first", "firstOrNull",
    "last", "lastOrNull", "count", "repeat", "lazy", "remember", "withLock", "synchronized", "use", "onEach",
];
const PLAIN_PREFIXES: [&str; 6] = ["map", "filter", "forEach", "associate", "sortedBy", "assertFails"];
/// Called bare, these run their lambda on the `this` already in scope, or on none.
const PLAIN_BARE: [&str; 3] = ["runCatching", "run", "apply"];

fn plain(name: &str) -> bool {
    PLAIN.contains(&name) || PLAIN_PREFIXES.iter().any(|p| name.starts_with(p))
}

fn capitalised(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
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
                at.receivers.clear();
                at.type_params = cx.d.masked(&at.class);
                at.opaque = false;
            }
        }
        "class_declaration" | "object_declaration" | "object_literal" => {
            at.opaque = true;
            at.type_params.extend(type_params(n, cx.src));
            let written: Vec<Vec<String>> = named(n)
                .into_iter()
                .filter(|c| c.kind() == "delegation_specifier")
                .map(|spec| written_type(child(spec, "constructor_invocation").unwrap_or(spec), cx.src).map(|t| resolved(&t, &at, cx)).unwrap_or_default())
                .collect();
            if !written.is_empty() {
                // One unread supertype may hold any name, so the anonymous type then holds them all.
                let ids = if written.iter().any(Vec::is_empty) { Vec::new() } else { written.concat() };
                at.receivers.push(Receiver::Types(ids));
            }
            // Every type inherits these from `Any`, which the graph never declares.
            for name in ["toString", "equals", "hashCode"] {
                at.bind(name, None);
            }
            for m in child(n, "class_body").map(members_of).unwrap_or_default() {
                if let Some(name) = declared_name(m, cx.src) {
                    at.bind(&name, None);
                }
            }
            for p in child(n, "primary_constructor").map(named).unwrap_or_default() {
                if let Some(name) = child(p, "simple_identifier") {
                    at.bind(text(name, cx.src), None);
                }
            }
        }
        "function_declaration" if is_declared(n) => {
            if let Some(s) = child(n, "simple_identifier") {
                let name = text(s, cx.src);
                let path = if at.class.is_empty() { name.to_string() } else { format!("{}.{name}", at.class) };
                at.function = Some(format!("sym:{}::{path}", cx.own.rel));
                at.locals.clear();
                at.receivers.clear();
                function(n, &mut at, cx);
            }
        }
        "function_declaration" | "anonymous_function" => function(n, &mut at, cx),
        "property_declaration" if is_declared(n) => enter_property(n, &mut at, cx),
        "getter" | "setter" => {
            if let Some(p) = accessed(n).filter(|p| is_declared(*p)) {
                enter_property(p, &mut at, cx);
            }
            for p in named(n).into_iter().filter(|p| p.kind() == "parameter_with_optional_type") {
                if let Some(name) = child(p, "simple_identifier") {
                    at.bind(text(name, cx.src), None);
                }
            }
        }
        "lambda_literal" if !n.parent().is_some_and(|p| one_line_object(p).is_some()) => {
            if let Some(r) = lambda_receiver(n, &at, cx) {
                at.receivers.push(r);
            }
            match child(n, "lambda_parameters") {
                Some(ps) => {
                    for p in named(ps) {
                        bind(p, &mut at, cx);
                    }
                }
                None => at.bind("it", None),
            }
        }
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
                at.bind(text(name, cx.src), ty);
            }
        }
        "call_expression" => {
            if let (Some(from), Some(callee)) = (at.function.clone(), n.named_child(0)) {
                let called = Ctx { own: Own { call: arguments(n), ..cx.own }, src: cx.src, d: cx.d };
                for to in jvm::same_platform(cx.own.rel, targets(callee, &at, &called)) {
                    // A recursive call says nothing about what the function depends on.
                    if to != from {
                        ex.edge(&from, &to, EdgeKind::Calls, "", cx.own.rel);
                    }
                }
            }
        }
        "simple_identifier" if is_value(n) => {
            if let Some(from) = at.function.clone() {
                for to in jvm::same_platform(cx.own.rel, referenced(text(n, cx.src), &at, cx)) {
                    if to != from {
                        ex.edge(&from, &to, EdgeKind::References, "", cx.own.rel);
                    }
                }
            }
        }
        "line_comment" | "multiline_comment" => jvm::cite(n, &owner(&at, cx.own.rel), "comment", cx.own.rel, cx.src, ex),
        "string_literal" => jvm::cite(n, &owner(&at, cx.own.rel), "string", cx.own.rel, cx.src, ex),
        _ => {}
    }
    signature(n, &at, cx, ex);
    for c in named(n) {
        // A local is seen by the statements after it, so it joins the `At` its later siblings get.
        if at.function.is_some() && !is_declared(c) {
            remember(c, &mut at, cx);
        }
        walk(c, at.clone(), cx, ex);
    }
}

/// The kinds a type written in a declaration's own signature takes.
const TYPE_KINDS: [&str; 6] = ["user_type", "nullable_type", "non_nullable_type", "parenthesized_type", "function_type", "receiver_type"];

/// A type named in a parameter's, a property's or a variable's type, a return type or an extension
/// receiver, generic arguments included, writes the edge an import of it would. A type parameter
/// or a local class of that name is no repository type.
fn signature(n: Node, at: &At, cx: &Ctx, ex: &mut Extraction) {
    if !matches!(
        n.kind(),
        "parameter" | "class_parameter" | "parameter_with_optional_type" | "variable_declaration" | "function_declaration" | "anonymous_function" | "property_declaration"
    ) {
        return;
    }
    let mut written = Vec::new();
    for t in named(n).into_iter().filter(|t| TYPE_KINDS.contains(&t.kind())) {
        names(t, cx.src, &mut written);
    }
    for w in written {
        if at.locals.contains_key(w.split('.').next().unwrap_or_default()) {
            continue;
        }
        jvm::used(&jvm::same_platform(cx.own.rel, resolved(&w, at, cx)), cx.own.rel, ex);
    }
}

/// Every type name a written type holds: `Map<K, List<V>>` holds `Map`, `K`, `List` and `V`.
fn names(t: Node, src: &[u8], out: &mut Vec<String>) {
    if t.kind() == "user_type" {
        let segments: Vec<&str> = named(t).into_iter().filter(|c| c.kind() == "type_identifier").map(|c| text(c, src)).collect();
        if !segments.is_empty() {
            out.push(segments.join("."));
        }
    }
    for c in named(t) {
        names(c, src, out);
    }
}

/// The context as seen from inside `at`'s class, where that class's private members are visible.
fn within<'a>(at: &'a At, cx: &Ctx<'a>) -> Ctx<'a> {
    Ctx { own: Own { at: &at.class, ..cx.own }, src: cx.src, d: cx.d }
}

/// How many arguments a call passes: the parenthesised ones and a trailing lambda.
fn arguments(call: Node) -> Option<Call> {
    let suffix = child(call, "call_suffix")?;
    let listed: Vec<Node> = child(suffix, "value_arguments").map(named).unwrap_or_default().into_iter().filter(|v| v.kind() == "value_argument").collect();
    let spread = listed.iter().any(|v| child(*v, "spread_expression").is_some());
    Some(Call { args: listed.len() + usize::from(child(suffix, "annotated_lambda").is_some()), spread })
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

/// A type as written, resolved where the walk is. A type parameter is no repository type, whatever
/// it shares a name with, so it resolves to nothing.
fn resolved(written: &str, at: &At, cx: &Ctx) -> Vec<String> {
    if at.type_params.contains(written.split('.').next().unwrap_or_default()) {
        return Vec::new();
    }
    type_ids(cx.own.types, cx.own.index, cx.own.scope, cx.own.rel, &at.class, written)
}

/// A function's type parameters, its extension receiver, then its parameters, which are bound
/// inside the receiver and so hide its members.
fn function(f: Node, at: &mut At, cx: &Ctx) {
    at.type_params.extend(type_params(f, cx.src));
    if let Some(written) = receiver(f, cx.src, &at.type_params) {
        let ids = if written.is_empty() { Vec::new() } else { resolved(&written, at, cx) };
        at.receivers.push(Receiver::Types(ids));
    }
    for p in child(f, "function_value_parameters").map(named).unwrap_or_default() {
        if let Some(name) = child(p, "simple_identifier") {
            // A function-typed parameter reads as no type, so `onDone()` through it is a call through a value.
            let ty = written_type(p, cx.src).map(|t| resolved(&t, at, cx));
            at.bind(text(name, cx.src), ty);
        }
    }
}

/// The walk enters a declared property the way it enters a declared function, the property as the caller.
fn enter_property(p: Node, at: &mut At, cx: &Ctx) {
    let Some(s) = child(p, "variable_declaration").and_then(|v| child(v, "simple_identifier")) else { return };
    let name = text(s, cx.src);
    let path = if at.class.is_empty() { name.to_string() } else { format!("{}.{name}", at.class) };
    at.function = Some(format!("sym:{}::{path}", cx.own.rel));
    at.locals.clear();
    at.receivers.clear();
    property(p, at, cx);
}

/// The property an accessor on a line of its own belongs to: the grammar parses it beside the
/// property, after any other accessor or comment, rather than inside it.
fn accessed(accessor: Node) -> Option<Node> {
    let mut p = accessor.prev_named_sibling();
    while let Some(n) = p.filter(|n| matches!(n.kind(), "getter" | "setter" | "line_comment" | "multiline_comment")) {
        p = n.prev_named_sibling();
    }
    p.filter(|n| n.kind() == "property_declaration")
}

/// A property as its initializer, delegate and accessors see it: its own type parameters and
/// extension receiver, the backing `field`, and the primary constructor's parameters, which an
/// initializer reads before any property of the same name. An accessor never sees those
/// parameters, but reading one there as a local only costs an edge.
fn property(p: Node, at: &mut At, cx: &Ctx) {
    at.type_params.extend(type_params(p, cx.src));
    if let Some(written) = receiver(p, cx.src, &at.type_params) {
        let ids = if written.is_empty() { Vec::new() } else { resolved(&written, at, cx) };
        at.receivers.push(Receiver::Types(ids));
    }
    at.bind("field", None);
    let ctor = holder(p).filter(|h| h.kind() == "class_declaration").and_then(|h| child(h, "primary_constructor"));
    for c in ctor.map(named).unwrap_or_default().into_iter().filter(|c| c.kind() == "class_parameter") {
        if let Some(name) = child(c, "simple_identifier") {
            let ty = written_type(c, cx.src).map(|t| resolved(&t, at, cx));
            at.bind(text(name, cx.src), ty);
        }
    }
}

/// Whether a `simple_identifier` is a name read as a value: not a declaration's name, a named
/// argument's, a member after `.`, a callee, an infix function, a receiver or an assignment target.
/// The parents are listed rather than excluded, so a grammar shape not read here writes nothing.
fn is_value(n: Node) -> bool {
    let Some(p) = n.parent() else { return false };
    let first = p.named_child(0) == Some(n);
    match p.kind() {
        "value_argument" => !(first && p.named_child_count() > 1),
        "infix_expression" => p.named_child(1) != Some(n),
        "indexing_expression" | "property_declaration" | "property_delegate" | "function_body" | "function_value_parameters" | "statements"
        | "control_structure_body" | "jump_expression" | "assignment" | "parenthesized_expression" | "collection_literal" | "if_expression"
        | "when_subject" | "when_condition" | "range_test" | "guard_condition" | "for_statement" | "while_statement" | "do_while_statement"
        | "indexing_suffix" | "interpolated_expression" | "spread_expression" | "prefix_expression" | "postfix_expression" | "as_expression"
        | "check_expression" | "additive_expression" | "multiplicative_expression" | "comparison_expression" | "equality_expression"
        | "conjunction_expression" | "disjunction_expression" | "elvis_expression" | "range_expression" => true,
        _ => false,
    }
}

/// The declarations a bare name read as a value reaches. Kotlin reads a value among properties,
/// objects and types only, and the lookup reads functions too, so a name whose first binding may be
/// a function is no proof of which value is meant.
fn referenced(name: &str, at: &At, cx: &Ctx) -> Vec<String> {
    match lookup(name, at, cx) {
        Named::Ids(ids) if !ids.iter().any(|id| may_be_function(id, cx)) => ids,
        _ => Vec::new(),
    }
}

/// This file knows each of its functions; another file's member function carries its arities in
/// the index, and another file's top-level name is known as a function or not only when it is a type.
fn may_be_function(id: &str, cx: &Ctx) -> bool {
    let Some((rel, path)) = split_id(id) else { return true };
    if rel == cx.own.rel {
        return cx.d.lambdas.contains_key(id);
    }
    if path.contains('.') {
        cx.own.index.arities(rel, path).is_some()
    } else {
        !cx.own.index.is_type(rel, path)
    }
}

/// A `variable_declaration` binds its name, typed when written; a destructuring binds each name untyped.
fn bind(v: Node, at: &mut At, cx: &Ctx) {
    match v.kind() {
        "variable_declaration" => {
            if let Some(name) = child(v, "simple_identifier") {
                let ty = written_type(v, cx.src).map(|t| resolved(&t, at, cx));
                at.bind(text(name, cx.src), ty);
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
            at.bind(&name, ty);
        }
        "function_declaration" | "class_declaration" | "object_declaration" => {
            if let Some(name) = declared_name(n, cx.src) {
                at.bind(&name, None);
            }
        }
        _ => {}
    }
}

/// The class `val s = Store()` constructs: the callee's targets, when every one is a type.
fn constructed(n: Node, at: &At, cx: &Ctx) -> Option<Vec<String>> {
    let callee = child(n, "call_expression")?.named_child(0).filter(|c| c.kind() == "simple_identifier")?;
    let ids = targets(callee, at, cx);
    (!ids.is_empty() && ids.iter().all(|id| cx.own.is_type(id))).then_some(ids)
}

/// The receiver a lambda runs with, `None` when it has none. It is read where it is written:
/// `x.apply {}`, `x.run {}` and `with(x) {}` take `x`'s type, and this file's function taking
/// `T.() -> R` takes `T`. A lambda passed anywhere else may have a receiver the file never sees.
fn lambda_receiver(lambda: Node, at: &At, cx: &Ctx) -> Option<Receiver> {
    let parent = lambda.parent()?;
    let trailing = match parent.kind() {
        "annotated_lambda" => true,
        "value_argument" => false,
        // Not passed to a call: a value, whose receiver would be written in its type.
        _ => return None,
    };
    let suffix = if trailing { parent.parent() } else { parent.parent().and_then(|args| args.parent()) }.filter(|s| s.kind() == "call_suffix")?;
    let call = suffix.parent().filter(|c| c.kind() == "call_expression")?;
    let callee = call.named_child(0)?;
    let of = |x: Node| match receiver_types(x, at, cx) {
        Some(ids) if !ids.is_empty() => Receiver::Types(ids),
        _ => Receiver::Unknown,
    };
    match callee.kind() {
        "simple_identifier" => {
            let name = text(callee, cx.src);
            if name == "with" {
                let first = child(suffix, "value_arguments").and_then(|a| child(a, "value_argument")).and_then(|a| a.named_child(0));
                return Some(first.map_or(Receiver::Unknown, of));
            }
            if plain(name) || PLAIN_BARE.contains(&name) {
                return None;
            }
        }
        "navigation_expression" => {
            let parts = named(callee);
            let method = parts.last().and_then(|s| child(*s, "simple_identifier")).map(|m| text(m, cx.src)).unwrap_or_default();
            if matches!(method, "apply" | "run") {
                return Some(parts.first().map_or(Receiver::Unknown, |x| of(*x)));
            }
            if plain(method) {
                return None;
            }
        }
        _ => return Some(Receiver::Unknown),
    }
    declared_receiver(&targets(callee, at, cx), trailing, cx)
}

/// What this file's function called with a lambda gives it as a receiver: every declaration of the
/// callee must agree, and a lambda not in last place could be any function-typed parameter.
fn declared_receiver(ids: &[String], trailing: bool, cx: &Ctx) -> Option<Receiver> {
    let mut seen: BTreeSet<&Param> = BTreeSet::new();
    for id in ids {
        let Some(signatures) = cx.d.lambdas.get(id).filter(|_| split_id(id).is_some_and(|(rel, _)| rel == cx.own.rel)) else {
            return Some(Receiver::Unknown);
        };
        for ps in signatures {
            if trailing {
                seen.insert(ps.last().unwrap_or(&Param::Value));
            } else {
                seen.extend(ps.iter().filter(|p| **p != Param::Value));
            }
        }
    }
    let mut each = seen.into_iter();
    match (each.next(), each.next()) {
        (Some(Param::Plain), None) => None,
        (Some(Param::Receiver(t)), None) if !t.is_empty() => {
            let (rel, path) = split_id(&ids[0])?;
            let ids = type_ids(cx.own.types, cx.own.index, cx.own.scope, rel, outer(path), t);
            Some(if ids.is_empty() { Receiver::Unknown } else { Receiver::Types(ids) })
        }
        _ => Some(Receiver::Unknown),
    }
}

/// Whether a receiver may hold `name`. A lambda's unseen receiver is taken to hold no capitalised
/// name: that is the residual the module doc names.
fn holds(r: &Receiver, name: &str, cx: &Ctx) -> Bound {
    match r {
        Receiver::Unknown if capitalised(name) => Bound::Absent,
        Receiver::Unknown => Bound::Refused,
        Receiver::Types(ids) => cx.own.on_receiver(ids, name),
    }
}

/// What a bare name binds.
enum Named {
    Local(Option<Vec<String>>),
    Ids(Vec<String>),
    /// Something binds it, but not provably one declaration.
    Refused,
    Nothing,
}

/// A bare name read level by level: locals in every enclosing scope, the implicit receivers innermost first, the enclosing
/// types, then the top level. The first level that binds it wins only when no level after it
/// binds it too.
fn lookup(name: &str, at: &At, cx: &Ctx) -> Named {
    let cx = &within(at, cx);
    if let Some(ty) = at.locals.get(name) {
        return Named::Local(ty.clone());
    }
    let receivers = at.receivers.iter().rev().map(|r| holds(r, name, cx));
    let class = std::iter::once_with(|| cx.own.member(&at.class, name));
    // A top-level function's arity is not recorded, so a spread may reach one it cannot take.
    let spread = cx.own.call.is_some_and(|c| c.spread);
    let top = std::iter::once_with(|| match top_level(name, cx) {
        Some(ids) if ids.is_empty() => Bound::Absent,
        Some(_) if spread => Bound::Refused,
        Some(ids) => Bound::Found(ids),
        None => Bound::Refused,
    });
    let mut levels = receivers.chain(class).chain(top).filter(|b| *b != Bound::Absent);
    match levels.next() {
        None => Named::Nothing,
        Some(Bound::Found(ids)) if levels.next().is_none() => Named::Ids(ids),
        Some(_) => Named::Refused,
    }
}

/// The ids a callee reaches: `f()` and `Type()`; `x.m()` and `this.x.m()` through a parameter, a
/// local or a property with a declared type; `Obj.m()` and `Type.m()` on an object or a companion.
fn targets(callee: Node, at: &At, cx: &Ctx) -> Vec<String> {
    let cx = &within(at, cx);
    match callee.kind() {
        "simple_identifier" => match lookup(text(callee, cx.src), at, cx) {
            Named::Ids(ids) => ids,
            _ => Vec::new(),
        },
        "navigation_expression" => {
            let parts = named(callee);
            let (Some(receiver), Some(suffix)) = (parts.first().copied(), parts.last().copied()) else { return Vec::new() };
            let Some(method) = child(suffix, "simple_identifier").map(|m| text(m, cx.src)) else { return Vec::new() };
            if is_this(receiver, cx) {
                return match this(method, at, cx) {
                    Bound::Found(ids) => ids,
                    _ => Vec::new(),
                };
            }
            receiver_types(receiver, at, cx).map(|ids| cx.own.on_types(&ids, method)).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// `this.name`: the innermost receiver's member, or the enclosing type's own or inherited one,
/// never an outer type's.
fn this(name: &str, at: &At, cx: &Ctx) -> Bound {
    let cx = &within(at, cx);
    match at.receivers.last() {
        Some(Receiver::Unknown) => Bound::Refused,
        Some(Receiver::Types(ids)) => cx.own.on_receiver(ids, name),
        None if at.opaque || at.class.is_empty() => Bound::Refused,
        None => cx.own.level(&at.class, name),
    }
}

/// Plain `this`: a labelled `this@Outer` or `super` is some other receiver.
fn is_this(n: Node, cx: &Ctx) -> bool {
    n.kind() == "this_expression" && text(n, cx.src) == "this"
}

/// A name no enclosing scope holds, read as Kotlin does past the members: this file's own top level
/// or an explicit import, the file's package, then the star imports. `None` when two of those bind
/// it, or two stars do: an overload the file cannot settle.
fn top_level(name: &str, cx: &Ctx) -> Option<Vec<String>> {
    let (own, scope, index) = (&cx.own, cx.own.scope, cx.own.index);
    let imported = scope.singles.get(name);
    let files = |base: &str| index.files(&qualify(base, name));
    // A set of files is kept only as an expect/actual family: anything else is overloads.
    let at = |base: &str| -> Option<Vec<String>> {
        let files = files(base);
        jvm::expect_family(&files).then(|| files.into_iter().map(|rel| format!("sym:{rel}::{name}")).collect())
    };
    if cx.d.top.contains(name) {
        // The rest of the package sits at the same level as this file: a namesake there is an overload.
        let elsewhere = files(&scope.package).into_iter().any(|rel| rel != own.rel);
        return (imported.is_none() && !elsewhere).then(|| vec![format!("sym:{}::{name}", own.rel)]);
    }
    let package = at(&scope.package)?;
    if let Some(bound) = imported {
        let mut each = bound.iter();
        return match (each.next(), each.next()) {
            (Some(q), None) if package.is_empty() => Some(jvm::declared(index, q).iter().map(jvm::Target::id).collect()),
            _ => None,
        };
    }
    if !package.is_empty() {
        return Some(package);
    }
    let mut starred = Vec::new();
    for star in &scope.stars {
        let ids = at(star)?;
        if !ids.is_empty() {
            starred.push(ids);
        }
    }
    match starred.len() {
        0 => Some(Vec::new()),
        1 => Some(starred.remove(0)),
        _ => None,
    }
}

/// The types a receiver stands for: whatever the name binds — a local, a receiver's or an
/// enclosing type's property, a top-level property or type — then a capitalised name read as a
/// type. A value of unknown type, or a lowercase name nothing binds, claims no call.
fn receiver_types(receiver: Node, at: &At, cx: &Ctx) -> Option<Vec<String>> {
    let cx = &within(at, cx);
    match receiver.kind() {
        "simple_identifier" => {
            let name = text(receiver, cx.src);
            match lookup(name, at, cx) {
                Named::Local(ty) => ty,
                Named::Ids(ids) => typed_member(&ids, cx),
                Named::Refused => None,
                Named::Nothing => capitalised(name).then(|| resolved(name, at, cx)),
            }
        }
        // `this.x` is the property `x`, whatever local shadows it.
        "navigation_expression" => {
            let parts = named(receiver);
            let [this_, suffix] = parts.as_slice() else { return None };
            if !is_this(*this_, cx) {
                return None;
            }
            let prop = child(*suffix, "simple_identifier").map(|s| text(s, cx.src))?;
            match this(prop, at, cx) {
                Bound::Found(ids) => typed_member(&ids, cx),
                _ => None,
            }
        }
        _ => None,
    }
}

/// What a declaration a name binds stands for as a receiver: a type is itself, a property of this
/// file its declared type read where it is declared, with the type parameters there masked. A
/// property of another file has no type the index holds.
fn typed_member(ids: &[String], cx: &Ctx) -> Option<Vec<String>> {
    if !ids.is_empty() && ids.iter().all(|id| cx.own.is_type(id)) {
        return Some(ids.to_vec());
    }
    let [id] = ids else { return None };
    let (rel, path) = split_id(id)?;
    if rel != cx.own.rel {
        return None;
    }
    let declared_in = outer(path);
    let name = path_of(id).rsplit('.').next()?;
    let t = cx.d.fields.get(declared_in)?.get(name)?;
    if cx.d.masked(declared_in).contains(t.split('.').next().unwrap_or_default()) {
        return None;
    }
    Some(type_ids(cx.own.types, cx.own.index, cx.own.scope, cx.own.rel, declared_in, t))
}
