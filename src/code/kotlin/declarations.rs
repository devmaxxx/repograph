//! Kotlin's declarations: symbols, `Declares`, visibility, and supertypes as written.

use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use super::COMMENTS;
use crate::code::jvm::{self, child, named, span, text};
use crate::model::{EdgeKind, Extraction, NodeKind};

/// What one Kotlin file declares, kept for the passes that resolve names after this one.
#[derive(Debug, Default)]
pub struct Declared {
    /// Top-level names, private ones included: the index and `widen` both read exactly these.
    pub top: BTreeSet<String>,
    /// Top-level names only `private` declarations here bind.
    pub private: BTreeSet<String>,
    /// Every type path declared here (`Outer`, `Outer.Inner`), so a name resolves in-file first.
    pub types: BTreeSet<String>,
    /// Every member id declared here, constructor properties included.
    pub members: BTreeSet<String>,
    /// Per type path, each property's declared type as written (`Store`, `a.b.Store`); `""` holds
    /// the top-level properties.
    pub fields: BTreeMap<String, BTreeMap<String, String>>,
    /// (declaring type's id, supertype as written).
    pub supers: Vec<(String, String)>,
    /// Per type path, how it reaches names it does not declare.
    pub shapes: BTreeMap<String, jvm::Shape>,
    /// Per function id, each declaration's parameters as a lambda passed to it reads them; two
    /// entries are overloads.
    pub lambdas: BTreeMap<String, Vec<Vec<Param>>>,
}

/// A parameter as a lambda argument meets it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Param {
    Value,
    /// A function type with no receiver: the lambda sees the names around it.
    Plain,
    /// `T.() -> R`: the lambda's `this` is a `T`, as written. Empty when `T` is a type parameter.
    Receiver(String),
}

/// A type's or a function's own type parameters.
pub(super) fn type_params(n: Node, src: &[u8]) -> BTreeSet<String> {
    child(n, "type_parameters")
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| child(p, "type_identifier"))
        .map(|t| text(t, src).to_string())
        .collect()
}

fn has_modifier(n: Node, word: &str, src: &[u8]) -> bool {
    child(n, "modifiers").is_some_and(|m| named(m).into_iter().any(|c| c.kind() == "class_modifier" && text(c, src) == word))
}

/// The receiver an extension function or a `T.() -> R` type is written with. `Some("")` when it
/// is written but no plain type: a function type, or one of `masked`.
pub(super) fn receiver(n: Node, src: &[u8], masked: &BTreeSet<String>) -> Option<String> {
    let r = n.child_by_field_name("receiver")?;
    let t = written_type(r, src).unwrap_or_default();
    let first = t.split('.').next().unwrap_or_default();
    Some(if masked.contains(first) { String::new() } else { t })
}

fn params(f: Node, src: &[u8], masked: &BTreeSet<String>) -> Vec<Param> {
    child(f, "function_value_parameters")
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.kind() == "parameter")
        .map(|p| match child(p, "function_type") {
            Some(ft) => receiver(ft, src, masked).map_or(Param::Plain, Param::Receiver),
            None => Param::Value,
        })
        .collect()
}

impl Declared {
    /// Type parameters in scope at the type `path`: its own and every enclosing type's. An outer
    /// one reaches only an `inner` type, but masking more only costs an edge.
    pub(super) fn masked(&self, path: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut p = path;
        while !p.is_empty() {
            out.extend(self.shapes.get(p).map(|s| s.type_params.iter().cloned()).into_iter().flatten());
            p = jvm::outer(p);
        }
        out
    }
}

/// Kotlin is public by default, and `internal` crosses files inside a module, so only these two
/// keep a name to its file.
fn hidden(n: Node, src: &[u8]) -> bool {
    child(n, "modifiers").is_some_and(|m| {
        named(m).into_iter().any(|c| c.kind() == "visibility_modifier" && matches!(text(c, src), "private" | "protected"))
    })
}

/// The type a property, parameter or supertype is written with. `Store?` is `Store`, `a.b.Store<T>`
/// is `a.b.Store`; a function type names no receiver a call could go through, so it is none.
pub(super) fn written_type(n: Node, src: &[u8]) -> Option<String> {
    let t = named(n).into_iter().find(|c| matches!(c.kind(), "user_type" | "nullable_type"))?;
    let user = if t.kind() == "nullable_type" { child(t, "user_type")? } else { t };
    let parts: Vec<&str> = named(user).into_iter().filter(|c| c.kind() == "type_identifier").map(|c| text(c, src)).collect();
    (!parts.is_empty()).then(|| parts.join("."))
}

/// `object Keys { val TOKEN = 1 }` on one top-level line: the grammar reads an empty object
/// literal, an infix call named `Keys` and a lambda holding the members. Returns the name and the
/// node the members sit in.
pub(super) fn one_line_object(n: Node) -> Option<(Node, Node)> {
    if n.kind() != "infix_expression" {
        return None;
    }
    let parts = named(n);
    let [literal, name, lambda] = parts.as_slice() else { return None };
    let shaped = literal.kind() == "object_literal" && literal.named_child_count() == 0 && name.kind() == "simple_identifier" && lambda.kind() == "lambda_literal";
    shaped.then(|| (*name, child(*lambda, "statements").unwrap_or(*lambda)))
}

/// A class, an object, or an object the grammar split as `one_line_object` reads it.
pub(super) fn is_type(n: Node) -> bool {
    matches!(n.kind(), "class_declaration" | "object_declaration") || one_line_object(n).is_some()
}

/// The declarations a type's body holds. A nested one-line object's member comes back wrapped in
/// an `ERROR`, and is still the member the file wrote.
pub(super) fn members_of(body: Node) -> Vec<Node> {
    named(body).into_iter().flat_map(|m| if m.kind() == "ERROR" { named(m) } else { vec![m] }).collect()
}

/// The node declaring `n` as a member: a type's declaration, a companion, or the file. `None` for
/// a statement inside a function, a lambda or an initializer.
pub(super) fn holder(n: Node) -> Option<Node> {
    let mut p = n.parent()?;
    if p.kind() == "ERROR" && p.parent().is_some_and(|b| matches!(b.kind(), "class_body" | "enum_class_body")) {
        p = p.parent()?;
    }
    match p.kind() {
        "source_file" => Some(p),
        "class_body" | "enum_class_body" => p.parent(),
        "statements" | "lambda_literal" => {
            let lambda = if p.kind() == "statements" { p.parent()? } else { p };
            lambda.parent().filter(|o| one_line_object(*o).is_some())
        }
        _ => None,
    }
}

fn name_node(n: Node) -> Option<Node> {
    if let Some((name, _)) = one_line_object(n) {
        return Some(name);
    }
    match n.kind() {
        "class_declaration" | "object_declaration" | "type_alias" => child(n, "type_identifier"),
        // An extension's receiver sits before the name and is not part of it.
        "function_declaration" => child(n, "simple_identifier"),
        "property_declaration" => child(n, "variable_declaration").and_then(|v| child(v, "simple_identifier")),
        _ => None,
    }
}

/// Writes every symbol `root` declares into `ex`, and returns what the passes after it resolve against.
pub fn scan(root: Node, rel: &str, src: &[u8], ex: &mut Extraction) -> Declared {
    let file_id = format!("file:{rel}");
    let mut d = Declared::default();
    let mut public = BTreeSet::new();
    for n in named(root) {
        declare(n, rel, src, &file_id, None, ex, &mut d);
        if let Some(name) = name_node(n) {
            let name = text(name, src).to_string();
            if hidden(n, src) {
                d.private.insert(name);
            } else {
                public.insert(name);
            }
        }
    }
    // An overload with no modifier makes the name reachable from the package.
    d.private.retain(|name| !public.contains(name));
    d
}

fn declare(n: Node, rel: &str, src: &[u8], parent: &str, owner: Option<&str>, ex: &mut Extraction, d: &mut Declared) {
    if n.kind() == "companion_object" {
        // A caller writes `Outer.make()`, never `Outer.Companion.make()`, so a companion's members
        // take the class's own path and the id a call resolves to is the id declared.
        if let (Some(body), Some(_)) = (child(n, "class_body"), owner) {
            for m in members_of(body) {
                declare(m, rel, src, parent, owner, ex, d);
            }
        }
        return;
    }
    let Some(name_at) = name_node(n) else { return };
    let name = text(name_at, src).to_string();
    let path = owner.map_or_else(|| name.clone(), |o| format!("{o}.{name}"));
    let id = format!("sym:{rel}::{path}");
    ex.node_span(NodeKind::Symbol, &id, &path, &jvm::body(n, name_at, src, COMMENTS), rel, span(n));
    ex.edge(parent, &id, EdgeKind::Declares, if hidden(n, src) { "" } else { "export" }, rel);
    match owner {
        None => {
            d.top.insert(name.clone());
        }
        Some(_) => {
            d.members.insert(id.clone());
        }
    }
    if let Some(t) = child(n, "variable_declaration").and_then(|v| written_type(v, src)) {
        d.fields.entry(owner.unwrap_or_default().to_string()).or_default().insert(name.clone(), t);
    }
    if n.kind() == "function_declaration" {
        let mut masked = type_params(n, src);
        masked.extend(owner.map(|o| d.masked(o)).unwrap_or_default());
        d.lambdas.entry(id.clone()).or_default().push(params(n, src, &masked));
    }
    if !is_type(n) {
        return;
    }
    d.types.insert(path.clone());
    let enumerated = child(n, "enum_class_body").is_some() || has_modifier(n, "enum", src);
    let shape = jvm::Shape {
        data: has_modifier(n, "data", src),
        inner: has_modifier(n, "inner", src),
        object: n.kind() != "class_declaration",
        implicit: enumerated,
        type_params: type_params(n, src),
    };
    d.shapes.insert(path.clone(), shape);
    for spec in named(n).into_iter().filter(|c| c.kind() == "delegation_specifier") {
        let target = child(spec, "constructor_invocation").unwrap_or(spec);
        if let Some(t) = written_type(target, src) {
            d.supers.push((id.clone(), t));
        }
    }
    if let Some(ctor) = child(n, "primary_constructor") {
        // Only `val` and `var` parameters are properties; a plain parameter lives in the constructor.
        for p in named(ctor).into_iter().filter(|p| p.kind() == "class_parameter" && child(*p, "binding_pattern_kind").is_some()) {
            let Some(pname_at) = child(p, "simple_identifier") else { continue };
            let pname = text(pname_at, src).to_string();
            let pid = format!("{id}.{pname}");
            ex.node_span(NodeKind::Symbol, &pid, &format!("{path}.{pname}"), &jvm::body(p, pname_at, src, COMMENTS), rel, span(p));
            ex.edge(&id, &pid, EdgeKind::Declares, if hidden(p, src) { "" } else { "export" }, rel);
            d.members.insert(pid);
            if let Some(t) = written_type(p, src) {
                d.fields.entry(path.clone()).or_default().insert(pname, t);
            }
        }
    }
    let body = one_line_object(n).map(|(_, members)| members).or_else(|| child(n, "class_body")).or_else(|| child(n, "enum_class_body"));
    if let Some(body) = body {
        for m in members_of(body) {
            declare(m, rel, src, &id, Some(&path), ex, d);
        }
    }
}
