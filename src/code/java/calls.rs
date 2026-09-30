//! Java's calls and cited ids, one walk over the tree the declarations pass read.
//!
//! A call edge is written only where the file proves its target; a missing edge is acceptable, a
//! wrong one is not. So:
//! - Methods and fields are apart: a field never stands in for a method of its name, and a
//!   receiver is read through fields and locals only.
//! - A call binds only a declaration whose parameter count admits its arguments, the type's own
//!   before a supertype's, as an override binds. A bare call binds in the innermost enclosing type
//!   that has one, then in a static import. An `Object` method, or a level where no declaration
//!   takes the arguments and a supertype the repository does not declare may, is no edge; a
//!   supertype in another file is read through the supertypes its header records. A superclass's
//!   method beats an interface's, so the class chain is read first, and one it may hold unread
//!   refuses; an unread interface beside one that declares a method taking the arguments does not
//!   stop it. An interface's `static` method, and outside its package a package-private one, is
//!   not inherited, so the lookup reads past it; one overloaded with an inherited declaration
//!   refuses. A static nested type reaches outer static methods, never an instance one, so an
//!   instance namesake refuses rather than falls through to an import. A receiver typed in another
//!   file is read the same way from its header, its own declaration then its supertypes; an own
//!   declaration there that does not take the arguments refuses, as that level of a walk does.
//! - The residual: overloads of one arity told apart only by their argument types bind the first
//!   level's declaration, as an unread supertype's same-arity overload is not seen.
//! - A written type name binds a member type first: at each enclosing type, from the innermost, its
//!   own, then one its supertypes pass down, before imports and the package. A private member type
//!   is not passed down, nor a package-private one outside its package, and either still hides a
//!   namesake further up that path; one inherited along two paths binds nothing. A nested type's
//!   supertypes and a member's annotations are read the same way, from the type around them.
//! - A local, parameter, lambda, catch, `for`, resource or pattern variable hides the field it
//!   shadows; one whose type the file does not read makes a call through it no edge.
//! - An anonymous or local class hides what it declares and whatever its unread supertype may, and
//!   a local class's name hides the repository type it is named like, as a type parameter does:
//!   from its declaration on in a block, and in the whole body for a member class of such a type.
//! - `Type.member()` is a static call only when no local, field, inherited field or static import
//!   may hold the first name; a field typed as the type it is named like reads the same either way.
//! - The residual for a supertype the repository does not declare, such as `Activity` or
//!   `Exception`: it is taken to declare no capitalised field and no member type, as `new Util()`
//!   already reads it. So `Util.twice()` under it binds as it does in a plain class, and a type
//!   named under it binds the import or package type. A bare call there still refuses, since it
//!   may be inherited. The cost is such a supertype's capitalised field or nested type of that
//!   name, which the compiler would bind instead; refusing would drop every static call and type
//!   named in an Android, exception or framework class.
//! - A single static import shadows an on-demand one; two of a name, or two on-demand imports, bind
//!   nothing.
//! - A type named in a field's, a parameter's or a record component's type or a return type,
//!   generic arguments and array elements included, writes the file-to-file `Imports` an import of
//!   it writes, masked as any written type is, and none where the grammar failed around it.
//! - `new T(…)` calls the type it creates, read as any written type is, as a Kotlin constructor
//!   call and a C# `new` do; the type, not its constructor, since which overload runs is the
//!   same-arity residual again. An anonymous class's creation calls its supertype the same way,
//!   an interface included, where a Kotlin object expression writes nothing.
//!   `outer.new Inner()` binds only when the receiver's own type declares `Inner`, since an
//!   inherited one may come from a supertype the file never reads. `new T[n]` constructs no `T`,
//!   so it writes a signature's `Imports`, not a call.
//!
//! - A field's initializer is walked as a body is, from the field; an initializer block, static or
//!   not, from its class, since the block is no symbol.
//!
//! Edges it leaves out: calls through `super`, through a chain of calls, through another file's
//! fields, and from an enum constant's arguments and body.
//!
//! One wrong edge it can write: another file's supertypes resolve through that file's imports and
//! package, never its member types, so a nested class there extending an inherited member type
//! named like a top-level type of the package is read as extending the top-level one, and the walk
//! through it binds that type's members.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use super::declarations::{supertypes, type_params, written_type, Declared, TYPES};
use crate::code::index::Call;
use crate::code::jvm::{self, child, named, outer, split_id, text, Bound, Own};
use crate::model::{EdgeKind, Extraction};

pub(super) struct Ctx<'a> {
    /// The file's methods, looked up as a bare or a qualified call reads them.
    pub methods: Own<'a>,
    /// The file's fields and record components, looked up as a receiver reads them.
    pub fields: Own<'a>,
    pub src: &'a [u8],
    pub d: &'a Declared,
    /// `member_type` per (enclosing type, name): a type name is looked up at every signature,
    /// receiver and creation, and each lookup walks the supertypes through the headers.
    pub member_types: &'a RefCell<MemberTypes>,
}

pub(super) type MemberTypes = BTreeMap<(String, String), Bound>;

/// A local's type as ids of the declarations it resolves to, or `None` when the file does not
/// read it; either way the name hides the field it shadows.
type Locals = BTreeMap<String, Option<Vec<String>>>;

/// An anonymous class, a local class or an enum constant's body: a type with no symbol, whose
/// members and supertypes the walk still reads.
#[derive(Clone)]
struct Local {
    methods: BTreeSet<String>,
    fields: BTreeSet<String>,
    /// Its member types, which hide their names from the whole body, not only after them.
    types: BTreeSet<String>,
    /// `None` when a supertype is unread, so it may declare any name.
    supers: Option<Vec<String>>,
}

/// Where the walk is: the declared type around it, the declared method calls come from, the
/// locals and symbol-less types in scope, and the names a written type must not resolve.
#[derive(Clone, Default)]
struct At {
    class: String,
    method: Option<String>,
    locals: Locals,
    scopes: Vec<Local>,
    masked: BTreeSet<String>,
}

/// A declaration the declarations pass reached, so its id is a symbol.
fn is_declared(n: Node) -> bool {
    let Some(p) = n.parent() else { return false };
    match p.kind() {
        "program" => true,
        "class_body" | "interface_body" | "annotation_type_body" => p.parent().is_some_and(|t| TYPES.contains(&t.kind()) && is_declared(t)),
        "enum_body_declarations" => p.parent().and_then(|b| b.parent()).is_some_and(|t| t.kind() == "enum_declaration" && is_declared(t)),
        _ => false,
    }
}

pub(super) fn scan(root: Node, cx: &Ctx, ex: &mut Extraction) {
    walk(root, At::default(), cx, ex);
}

fn walk(n: Node, mut at: At, cx: &Ctx, ex: &mut Extraction) {
    match n.kind() {
        k if TYPES.contains(&k) && is_declared(n) => {
            if let Some(name) = n.child_by_field_name("name") {
                let name = text(name, cx.src);
                at.class = if at.class.is_empty() { name.to_string() } else { format!("{}.{name}", at.class) };
                at.method = None;
                at.locals.clear();
                at.scopes.clear();
                at.masked = jvm::masked(&cx.d.shapes, &at.class);
            }
        }
        k if TYPES.contains(&k) => {
            if let Some(name) = n.child_by_field_name("name") {
                at.masked.insert(text(name, cx.src).to_string());
            }
            at.masked.extend(type_params(n, cx.src));
            // An enum's, a record's or an annotation's supertype is one the file never writes.
            let implicit = matches!(k, "enum_declaration" | "record_declaration" | "annotation_type_declaration");
            let written = supertypes(n, cx.src);
            let supers = if implicit { None } else { resolved_all(&written, &at, cx) };
            let mut local = members_of(n.child_by_field_name("body"), supers, cx);
            if k == "record_declaration" {
                let components = n.child_by_field_name("parameters").map(named).unwrap_or_default();
                local.fields.extend(components.into_iter().filter_map(|p| p.child_by_field_name("name")).map(|x| text(x, cx.src).to_string()));
            }
            enter(local, &mut at, cx);
        }
        "method_declaration" | "constructor_declaration" | "compact_constructor_declaration" => {
            // A compact constructor is no symbol; its calls stay unattributed.
            if is_declared(n) && n.kind() != "compact_constructor_declaration" {
                at.method = n.child_by_field_name("name").map(|name| format!("sym:{}::{}.{}", cx.methods.rel, at.class, text(name, cx.src)));
                at.locals.clear();
            }
            at.masked.extend(type_params(n, cx.src));
            for p in n.child_by_field_name("parameters").map(named).unwrap_or_default() {
                parameter(p, &mut at, cx);
            }
            // A pattern variable's scope follows flow, which this walk does not track; hiding its
            // name for the whole method only costs edges.
            if let Some(body) = n.child_by_field_name("body") {
                patterns(body, &mut at, cx.src);
            }
        }
        "variable_declarator" if n.parent().is_some_and(|p| matches!(p.kind(), "field_declaration" | "constant_declaration") && is_declared(p)) => {
            at.method = n.child_by_field_name("name").map(|name| format!("sym:{}::{}.{}", cx.methods.rel, at.class, text(name, cx.src)));
            at.locals.clear();
        }
        // An initializer block is no symbol; what it calls, its class calls.
        "static_initializer" | "block" if is_declared(n) => {
            at.method = Some(format!("sym:{}::{}", cx.methods.rel, at.class));
            at.locals.clear();
        }
        "lambda_expression" => {
            let params = n.child_by_field_name("parameters");
            let untyped = match params {
                Some(p) if p.kind() == "identifier" => vec![p],
                Some(p) if p.kind() == "formal_parameters" => {
                    named(p).into_iter().for_each(|p| parameter(p, &mut at, cx));
                    Vec::new()
                }
                Some(p) => named(p),
                None => Vec::new(),
            };
            for p in untyped {
                at.locals.insert(text(p, cx.src).to_string(), None);
            }
        }
        "catch_clause" => {
            if let Some(p) = child(n, "catch_formal_parameter") {
                // `catch (A | B e)` has no one type a call could go through.
                let types = child(p, "catch_type").map(named).unwrap_or_default();
                let ty = match types.as_slice() {
                    [one] => written_type(*one, cx.src).and_then(|t| resolved(&t, &at, cx)),
                    _ => None,
                };
                if let Some(name) = p.child_by_field_name("name") {
                    at.locals.insert(text(name, cx.src).to_string(), ty);
                }
            }
        }
        "method_invocation" => {
            if let Some(from) = at.method.clone() {
                let args = n.child_by_field_name("arguments").map(|a| named(a).into_iter().filter(|x| !x.kind().ends_with("comment")).count());
                let call = args.map(|args| Call { args, spread: false });
                let called = Ctx { methods: Own { call, at: &at.class, ..cx.methods }, ..*cx };
                for to in targets(n, &at, &called) {
                    if to != from {
                        ex.edge(&from, &to, EdgeKind::Calls, "", cx.methods.rel);
                    }
                }
            }
        }
        // The type is read in the scope around the creation, before an anonymous body is entered.
        "object_creation_expression" => {
            if let Some(from) = &at.method {
                for to in created(n, &at, cx).into_iter().filter(|to| to != from) {
                    ex.edge(from, &to, EdgeKind::Calls, "", cx.methods.rel);
                }
            }
        }
        "line_comment" | "block_comment" => jvm::cite(n, &owner(&at, cx.methods.rel), "comment", cx.methods.rel, cx.src, ex),
        "string_literal" => jvm::cite(n, &owner(&at, cx.methods.rel), "string", cx.methods.rel, cx.src, ex),
        _ => {}
    }
    signature(n, &at, cx, ex);
    let body = n.child_by_field_name("body").map(|b| b.id());
    for c in named(n) {
        let mut inside = at.clone();
        if Some(c.id()) == body {
            match n.kind() {
                // The loop variable's scope is the body, not the iterated expression.
                "enhanced_for_statement" => {
                    let ty = n.child_by_field_name("type").and_then(|t| written_type(t, cx.src)).and_then(|t| resolved(&t, &at, cx));
                    if let Some(name) = n.child_by_field_name("name") {
                        inside.locals.insert(text(name, cx.src).to_string(), ty);
                    }
                }
                // Resources reach the body; the catch and finally clauses do not see them.
                "try_with_resources_statement" => {
                    for r in n.child_by_field_name("resources").map(named).unwrap_or_default() {
                        declare(r, &mut inside, cx);
                    }
                }
                _ => {}
            }
        }
        if c.kind() == "class_body" && n.kind() == "object_creation_expression" {
            let written = n.child_by_field_name("type").and_then(|t| written_type(t, cx.src));
            let supers = written.and_then(|w| resolved_all(&[w], &at, cx));
            enter(members_of(Some(c), supers, cx), &mut inside, cx);
        }
        if c.kind() == "class_body" && n.kind() == "enum_constant" {
            let id = format!("sym:{}::{}", cx.methods.rel, at.class);
            enter(members_of(Some(c), Some(vec![id]), cx), &mut inside, cx);
        }
        walk(c, inside, cx, ex);
        declare(c, &mut at, cx);
        if c.kind() == "switch_block_statement_group" {
            // A local declared under one label is in scope under the labels after it.
            for s in named(c) {
                declare(s, &mut at, cx);
            }
        }
    }
}

/// A type named in a field's, a parameter's or a record component's type or a return type, generic
/// arguments and array elements included, writes the edge an import of it would.
fn signature(n: Node, at: &At, cx: &Ctx, ex: &mut Extraction) {
    let t = match n.kind() {
        "field_declaration" | "constant_declaration" | "method_declaration" | "formal_parameter" | "array_creation_expression" => n.child_by_field_name("type"),
        "spread_parameter" => named(n).into_iter().find(|c| c.kind() != "modifiers" && c.kind() != "variable_declarator"),
        _ => None,
    };
    // An `ERROR` may hide the type parameter that masks a name.
    let t = t.filter(|_| !jvm::broken(n));
    let mut written = Vec::new();
    if let Some(t) = t {
        names(t, cx.src, &mut written);
    }
    for w in written {
        jvm::used(&resolved(&w, at, cx).unwrap_or_default(), cx.methods.rel, ex);
    }
}

/// Every type name a written type holds: `Map<K, List<V>[]>` holds `Map`, `K`, `List` and `V`.
fn names(t: Node, src: &[u8], out: &mut Vec<String>) {
    match t.kind() {
        "type_identifier" | "scoped_type_identifier" => out.extend(written_type(t, src)),
        "generic_type" => {
            out.extend(written_type(t, src));
            named(t).into_iter().filter(|c| c.kind() == "type_arguments").for_each(|a| names(a, src, out));
        }
        "array_type" => {
            if let Some(e) = t.child_by_field_name("element") {
                names(e, src, out);
            }
        }
        "type_arguments" | "wildcard" | "annotated_type" => named(t).into_iter().for_each(|c| names(c, src, out)),
        _ => {}
    }
}

fn owner(at: &At, rel: &str) -> String {
    match (&at.method, at.class.is_empty()) {
        (Some(m), _) => m.clone(),
        (None, false) => format!("sym:{rel}::{}", at.class),
        (None, true) => format!("file:{rel}"),
    }
}

/// A written type's ids in this file's view, or `None` when a type parameter or a local class is
/// named or nothing in the repository is.
fn resolved(written: &str, at: &At, cx: &Ctx) -> Option<Vec<String>> {
    if at.masked.contains(written.split('.').next().unwrap_or_default()) {
        return None;
    }
    let (first, rest) = written.split_once('.').map_or((written, None), |(f, r)| (f, Some(r)));
    for local in at.scopes.iter().rev() {
        match local.supers.as_deref().map(|ids| cx.methods.inherited_type(ids, first)) {
            Some(Bound::Found(ids)) => return nested(ids, rest, &cx.methods),
            Some(Bound::Refused) => return None,
            _ => {}
        }
    }
    type_named(&cx.methods, &at.class, written, cx.member_types)
}

/// A written type inside the type at `at`: a member type, declared or inherited, before what the
/// imports and the package bind.
fn type_named(own: &Own, at: &str, written: &str, cache: &RefCell<MemberTypes>) -> Option<Vec<String>> {
    let first = written.split('.').next().unwrap_or(written);
    let key = (at.to_string(), first.to_string());
    let cached = cache.borrow().get(&key).cloned();
    let bound = cached.unwrap_or_else(|| {
        let bound = own.member_type(at, first);
        cache.borrow_mut().insert(key, bound.clone());
        bound
    });
    let ids: Vec<String> = own.after_member_type(bound, at, written).into_iter().filter(|id| own.is_type(id)).collect();
    (!ids.is_empty()).then_some(ids)
}

/// `Inner.Deeper` under the member types `ids`, which hide every other type of their name, so
/// one that does not hold it binds nothing.
fn nested(ids: Vec<String>, rest: Option<&str>, own: &Own) -> Option<Vec<String>> {
    let Some(rest) = rest else { return Some(ids) };
    ids.iter().map(|id| Some(format!("{id}.{rest}")).filter(|c| own.is_type(c))).collect()
}

/// Every written supertype resolved, or `None` when one is unread.
fn resolved_all(written: &[String], at: &At, cx: &Ctx) -> Option<Vec<String>> {
    written.iter().map(|w| resolved(w, at, cx)).collect::<Option<Vec<_>>>().map(|ids| ids.concat())
}

fn members_of(body: Option<Node>, supers: Option<Vec<String>>, cx: &Ctx) -> Local {
    let mut local = Local { methods: BTreeSet::new(), fields: BTreeSet::new(), types: BTreeSet::new(), supers };
    for m in body.map(named).unwrap_or_default() {
        match m.kind() {
            "method_declaration" => local.methods.extend(m.child_by_field_name("name").map(|x| text(x, cx.src).to_string())),
            "field_declaration" => {
                let mut c = m.walk();
                let declarators: Vec<Node> = m.children_by_field_name("declarator", &mut c).collect();
                local.fields.extend(declarators.into_iter().filter_map(|v| v.child_by_field_name("name")).map(|x| text(x, cx.src).to_string()));
            }
            k if TYPES.contains(&k) => local.types.extend(m.child_by_field_name("name").map(|x| text(x, cx.src).to_string())),
            _ => {}
        }
    }
    local
}

/// Steps into a type with no symbol. A captured local its fields, or an unread supertype's, may
/// shadow no longer tells its type.
fn enter(local: Local, at: &mut At, cx: &Ctx) {
    for (name, ty) in at.locals.iter_mut() {
        if field_in(&local, name, cx) != Bound::Absent {
            *ty = None;
        }
    }
    at.masked.extend(local.types.iter().cloned());
    at.scopes.push(local);
}

/// A field `name` in a symbol-less type: its own, or one of its supertypes'.
fn field_in(local: &Local, name: &str, cx: &Ctx) -> Bound {
    if local.fields.contains(name) {
        return Bound::Refused;
    }
    match &local.supers {
        None if cx.fields.past_unread => Bound::Absent,
        None => Bound::Refused,
        Some(ids) if ids.is_empty() => Bound::Absent,
        // A supertype's declaration it may not inherit leaves a namesake further out unsettled.
        Some(ids) if !cx.fields.inherits_all(ids, name) => Bound::Refused,
        Some(ids) => cx.fields.on_receiver(ids, name),
    }
}

/// A method `name` called bare or on `this` in a symbol-less type. Its own is no symbol, so it
/// refuses as well as an unread supertype does.
fn method_in(local: &Local, name: &str, cx: &Ctx) -> Bound {
    if local.methods.contains(name) {
        return Bound::Refused;
    }
    match &local.supers {
        None => Bound::Refused,
        Some(ids) if ids.is_empty() => {
            if jvm::from_object(name) { Bound::Refused } else { Bound::Absent }
        }
        Some(ids) if !cx.methods.inherits_all(ids, name) => Bound::Refused,
        Some(ids) => cx.methods.on_receiver(ids, name),
    }
}

fn parameter(p: Node, at: &mut At, cx: &Ctx) {
    let name = p.child_by_field_name("name").or_else(|| child(p, "variable_declarator").and_then(|v| v.child_by_field_name("name")));
    let Some(name) = name else { return };
    // A varargs parameter is an array, which no call on a declared type goes through.
    let ty = match p.kind() {
        "formal_parameter" if p.child_by_field_name("dimensions").is_none() => {
            p.child_by_field_name("type").and_then(|t| written_type(t, cx.src)).and_then(|t| resolved(&t, at, cx))
        }
        _ => None,
    };
    at.locals.insert(text(name, cx.src).to_string(), ty);
}

/// Every pattern variable under `n`, bound with no type.
fn patterns(n: Node, at: &mut At, src: &[u8]) {
    let name = match n.kind() {
        "instanceof_expression" => n.child_by_field_name("name"),
        "type_pattern" | "record_pattern_component" => named(n).into_iter().rfind(|c| c.kind() == "identifier"),
        _ => None,
    };
    if let Some(name) = name {
        at.locals.insert(text(name, src).to_string(), None);
    }
    for c in named(n) {
        patterns(c, at, src);
    }
}

/// Binds the names a local variable declaration or a resource introduces to the siblings after it.
fn declare(n: Node, at: &mut At, cx: &Ctx) {
    match n.kind() {
        "local_variable_declaration" => {
            let written = n.child_by_field_name("type").and_then(|t| written_type(t, cx.src));
            let mut c = n.walk();
            let declarators: Vec<Node> = n.children_by_field_name("declarator", &mut c).collect();
            for v in declarators {
                let Some(name) = v.child_by_field_name("name") else { continue };
                let ty = if v.child_by_field_name("dimensions").is_some() { None } else { local_type(written.as_deref(), v.child_by_field_name("value"), at, cx) };
                at.locals.insert(text(name, cx.src).to_string(), ty);
            }
        }
        "resource" => {
            if let Some(name) = n.child_by_field_name("name") {
                let written = n.child_by_field_name("type").and_then(|t| written_type(t, cx.src));
                let ty = local_type(written.as_deref(), n.child_by_field_name("value"), at, cx);
                at.locals.insert(text(name, cx.src).to_string(), ty);
            }
        }
        "class_declaration" | "interface_declaration" | "enum_declaration" | "record_declaration" if !is_declared(n) => {
            if let Some(name) = n.child_by_field_name("name") {
                at.masked.insert(text(name, cx.src).to_string());
            }
        }
        _ => {}
    }
}

/// A local's type: as written, or for `var` the type a `new` initializer constructs.
fn local_type(written: Option<&str>, value: Option<Node>, at: &At, cx: &Ctx) -> Option<Vec<String>> {
    match written {
        Some("var") | None => {
            let created = value.filter(|x| x.kind() == "object_creation_expression" && child(*x, "class_body").is_none())?;
            created.child_by_field_name("type").and_then(|t| written_type(t, cx.src)).and_then(|t| resolved(&t, at, cx))
        }
        Some(t) => resolved(t, at, cx),
    }
}

fn targets(call: Node, at: &At, cx: &Ctx) -> Vec<String> {
    let Some(name) = call.child_by_field_name("name").map(|m| text(m, cx.src)) else { return Vec::new() };
    let Some(object) = call.child_by_field_name("object") else { return bare(name, at, cx) };
    if object.kind() == "this" {
        let bound = match at.scopes.last() {
            Some(local) => method_in(local, name, cx),
            None => cx.methods.level(&at.class, name),
        };
        return found(bound);
    }
    match receiver(object, at, cx) {
        Some(ids) => cx.methods.on_types(&ids, name),
        None => Vec::new(),
    }
}

/// The type `new T(…)` creates, read as any written type is. `outer.new Inner()` names a member
/// type of the receiver's type, which only a receiver whose own type declares `Inner` proves: an
/// inherited one may come from a supertype the lookup does not walk.
fn created(n: Node, at: &At, cx: &Ctx) -> Vec<String> {
    let Some(written) = n.child_by_field_name("type").and_then(|t| written_type(t, cx.src)) else { return Vec::new() };
    let mut before_new = n.walk();
    let qualifier = n.children(&mut before_new).take_while(|c| c.kind() != "new").find(|c| c.is_named() && !c.kind().ends_with("comment"));
    let Some(qualifier) = qualifier else { return resolved(&written, at, cx).unwrap_or_default() };
    let Some(outers) = receiver(qualifier, at, cx) else { return Vec::new() };
    let inner: Option<Vec<String>> = outers.iter().map(|id| Some(format!("{id}.{written}")).filter(|c| cx.methods.is_type(c))).collect();
    inner.unwrap_or_default()
}

fn found(b: Bound) -> Vec<String> {
    match b {
        Bound::Found(ids) => ids,
        _ => Vec::new(),
    }
}

fn bare(name: &str, at: &At, cx: &Ctx) -> Vec<String> {
    for local in at.scopes.iter().rev() {
        match method_in(local, name, cx) {
            Bound::Absent => {}
            b => return found(b),
        }
    }
    match cx.methods.member(&at.class, name) {
        Bound::Absent => {}
        b => return found(b),
    }
    let scope = cx.methods.scope;
    // A single static import shadows every on-demand one.
    let imported: Vec<&String> = match scope.statics.get(name) {
        Some(types) => types.iter().collect(),
        None => scope.static_stars.iter().collect(),
    };
    let [only] = imported.as_slice() else { return Vec::new() };
    let ids: Vec<String> = jvm::declared(cx.methods.index, only).iter().map(|t| t.id()).filter(|id| cx.methods.is_type(id)).collect();
    cx.methods.on_types(&ids, name)
}

/// The types a call's receiver has, when the file reads them.
fn receiver(object: Node, at: &At, cx: &Ctx) -> Option<Vec<String>> {
    match object.kind() {
        "identifier" => {
            let name = text(object, cx.src);
            match value(name, at, cx) {
                Some(ty) => ty,
                None => resolved(name, at, cx),
            }
        }
        "field_access" => {
            let base = object.child_by_field_name("object")?;
            let field = text(object.child_by_field_name("field")?, cx.src);
            if base.kind() == "this" {
                if !at.scopes.is_empty() {
                    return None;
                }
                return typed_field(cx.fields.level(&at.class, field), cx);
            }
            let chain = dotted(object, cx.src)?;
            let first = chain.split('.').next().unwrap_or_default();
            if value(first, at, cx).is_some() {
                return None;
            }
            resolved(&chain, at, cx)
        }
        "object_creation_expression" if child(object, "class_body").is_none() => {
            resolved(&written_type(object.child_by_field_name("type")?, cx.src)?, at, cx)
        }
        _ => None,
    }
}

/// `name` read as a value: `Some` with its types, or `Some(None)` when something that is not a
/// type may hold it, and `None` only when the name can be nothing but a type. A capitalised name
/// skips a level only an unread supertype may hold it at, as `new Name()` does.
fn value(name: &str, at: &At, cx: &Ctx) -> Option<Option<Vec<String>>> {
    if let Some(ty) = at.locals.get(name) {
        return Some(ty.clone());
    }
    let capital = name.starts_with(|c: char| c.is_ascii_uppercase());
    let cx = &Ctx { fields: Own { past_unread: capital, ..cx.fields }, ..*cx };
    for local in at.scopes.iter().rev() {
        if field_in(local, name, cx) != Bound::Absent {
            return Some(None);
        }
    }
    match cx.fields.member(&at.class, name) {
        Bound::Absent => {}
        b => return Some(typed_field(b, cx)),
    }
    static_import_could_name(name, cx).then_some(None)
}

/// A static import that names `name`, or an on-demand one whose type is unread or declares it.
fn static_import_could_name(name: &str, cx: &Ctx) -> bool {
    let scope = cx.fields.scope;
    scope.statics.contains_key(name)
        || scope.static_stars.iter().any(|star| {
            let targets = jvm::declared(cx.fields.index, star);
            targets.is_empty() || targets.iter().any(|t| cx.fields.index.declares(&t.rel, &format!("{}.{name}", t.path)))
        })
}

/// A field this file declares, read through its declared type.
fn typed_field(b: Bound, cx: &Ctx) -> Option<Vec<String>> {
    let Bound::Found(ids) = b else { return None };
    let [id] = ids.as_slice() else { return None };
    let (rel, path) = split_id(id)?;
    if rel != cx.fields.rel {
        return None;
    }
    let declared_in = outer(path);
    let name = path.rsplit('.').next()?;
    let t = cx.d.fields.get(declared_in)?.get(name)?;
    if jvm::masked(&cx.d.shapes, declared_in).contains(t.split('.').next().unwrap_or_default()) {
        return None;
    }
    type_named(&cx.fields, declared_in, t, cx.member_types)
}

fn dotted(n: Node, src: &[u8]) -> Option<String> {
    match n.kind() {
        "identifier" => Some(text(n, src).to_string()),
        "field_access" => Some(format!("{}.{}", dotted(n.child_by_field_name("object")?, src)?, text(n.child_by_field_name("field")?, src))),
        _ => None,
    }
}
