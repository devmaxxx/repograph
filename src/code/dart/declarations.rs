use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use crate::code::syntax::{child, named, span, text};
use crate::model::{EdgeKind, Extraction, NodeKind};

#[derive(Debug, Default)]
pub struct Declared {
    /// Top-level names this file declares, private ones included: a part of the same library sees them.
    pub top: BTreeSet<String>,
    /// Every member id this file declares, for a bare name called inside its type (implicit `this`).
    pub members: BTreeSet<String>,
    /// Type name -> field name -> the class the field holds, from its declared type or from the
    /// constructor its initializer calls.
    pub fields: BTreeMap<String, BTreeMap<String, String>>,
    /// (declaring id, supertype name) from `extends`, `with` and `implements`, resolved by the caller.
    pub supers: Vec<(String, String)>,
}

/// Dart has no visibility keyword: a leading underscore makes a name private to its library.
fn context(name: &str) -> &'static str {
    if name.starts_with('_') { "" } else { "export" }
}

/// The class a `type` node names: `State` for `State<HomeScreen>`, nothing for a function type or
/// `void`. In a supertype list a generic argument is a sibling `type` whose first child is another
/// `type`, so it names nothing here.
pub(crate) fn type_name(t: Node, src: &[u8]) -> Option<String> {
    let first = if t.kind() == "type" { t.named_child(0)? } else { t };
    (first.kind() == "type_identifier").then(|| text(first, src).to_string())
}

fn declare(ex: &mut Extraction, rel: &str, parent: &str, qualified: &str, name: &str, n: Node, src: &[u8]) -> String {
    let id = format!("sym:{rel}::{qualified}");
    let signature = text(n, src).lines().next().unwrap_or("").trim().to_string();
    ex.node_span(NodeKind::Symbol, &id, qualified, &signature, rel, span(n));
    ex.edge(parent, &id, EdgeKind::Declares, context(name), rel);
    id
}

/// A type declaration's name; `class M = A with B;` keeps it inside `mixin_application_class`, and
/// an unnamed extension has none, because nothing a caller writes names it.
fn type_name_of(n: Node, src: &[u8]) -> Option<String> {
    if let Some(name) = n.child_by_field_name("name") {
        return Some(text(name, src).to_string());
    }
    child(n, "mixin_application_class").and_then(|m| child(m, "identifier")).map(|i| text(i, src).to_string())
}

fn supertypes(n: Node, src: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack: Vec<Node> = named(n).into_iter()
        .filter(|c| matches!(c.kind(), "superclass" | "interfaces" | "mixin_application_class"))
        .collect();
    while let Some(part) = stack.pop() {
        for c in named(part) {
            match c.kind() {
                "type" => out.extend(type_name(c, src)),
                "mixins" | "mixin_application" => stack.push(c),
                _ => {}
            }
        }
    }
    out
}

/// Each name a variable declaration introduces, with its `initialized_identifier`: `const a = 1, b = 2;` is two.
fn variables<'t>(n: Node<'t>, src: &[u8]) -> Vec<(String, Node<'t>)> {
    named(n).into_iter()
        .filter(|l| matches!(l.kind(), "initialized_identifier_list" | "static_final_declaration_list"))
        .flat_map(named)
        .filter_map(|i| i.child_by_field_name("name").map(|x| (text(x, src).to_string(), i)))
        .collect()
}

/// The names a class member declares, each with the node whose lines it spans. The unnamed
/// constructor is its class and declares nothing; a named constructor or factory is `Type.name`,
/// which is how callers write it; an operator has no name a caller writes.
pub(crate) fn member_names<'t>(m: Node<'t>, src: &[u8]) -> Vec<(String, Node<'t>)> {
    let mut out = Vec::new();
    for part in named(m) {
        let signatures = match part.kind() {
            "method_declaration" => part.child_by_field_name("signature").map(named).unwrap_or_default(),
            "declaration" => named(part),
            _ => continue,
        };
        for s in signatures {
            match s.kind() {
                "function_signature" | "getter_signature" | "setter_signature" => {
                    out.extend(s.child_by_field_name("name").map(|x| (text(x, src).to_string(), part)));
                }
                "constructor_signature" | "constant_constructor_signature" | "factory_constructor_signature"
                | "redirecting_factory_constructor_signature" => {
                    let mut c = s.walk();
                    // The grammar files the `.` between the two names under `name` too.
                    let names: Vec<Node> = s.children_by_field_name("name", &mut c).filter(|n| n.is_named()).collect();
                    if let Some(second) = names.get(1) {
                        out.push((text(*second, src).to_string(), part));
                    }
                }
                "initialized_identifier_list" | "static_final_declaration_list" => {
                    for i in named(s) {
                        out.extend(i.child_by_field_name("name").map(|x| (text(x, src).to_string(), part)));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// The class a field holds. `final _state = OfficeState();` states it through the constructor, the
/// way a Flutter `State` holds what it owns; a capitalised callee is a class by Dart's naming
/// convention, and a lower-case one is a function whose return type this walk cannot see.
fn field_type(part: Node, init: Node, src: &[u8]) -> Option<String> {
    if let Some(t) = child(part, "type") {
        return type_name(t, src);
    }
    let value = init.child_by_field_name("value").filter(|v| v.kind() == "call_expression")?;
    let function = value.child_by_field_name("function")?;
    let callee = match function.kind() {
        "identifier" => function,
        "member_expression" => function.child_by_field_name("object").filter(|o| o.kind() == "identifier")?,
        _ => return None,
    };
    let name = text(callee, src);
    name.starts_with(|c: char| c.is_ascii_uppercase()).then(|| name.to_string())
}

fn member(m: Node, rel: &str, src: &[u8], owner_id: &str, owner: &str, ex: &mut Extraction, d: &mut Declared) {
    for (name, part) in member_names(m, src) {
        let id = declare(ex, rel, owner_id, &format!("{owner}.{name}"), &name, part, src);
        d.members.insert(id);
    }
    for part in named(m).into_iter().filter(|p| p.kind() == "declaration") {
        for (name, init) in variables(part, src) {
            if let Some(t) = field_type(part, init, src) {
                d.fields.entry(owner.to_string()).or_default().insert(name, t);
            }
        }
    }
}

pub fn scan(root: Node, rel: &str, src: &[u8], ex: &mut Extraction) -> Declared {
    let file_id = format!("file:{rel}");
    let mut d = Declared::default();
    for n in named(root) {
        match n.kind() {
            "class_declaration" | "mixin_declaration" | "extension_declaration" | "enum_declaration" => {
                let Some(name) = type_name_of(n, src) else { continue };
                let id = declare(ex, rel, &file_id, &name, &name, n, src);
                d.top.insert(name.clone());
                for s in supertypes(n, src) {
                    d.supers.push((id.clone(), s));
                }
                let Some(body) = n.child_by_field_name("body") else { continue };
                for m in named(body).into_iter().filter(|m| m.kind() == "class_member") {
                    member(m, rel, src, &id, &name, ex, &mut d);
                }
            }
            "type_alias" => {
                let Some(t) = child(n, "type_identifier") else { continue };
                let name = text(t, src).to_string();
                declare(ex, rel, &file_id, &name, &name, n, src);
                d.top.insert(name);
            }
            "function_declaration" | "getter_declaration" | "setter_declaration" => {
                let Some(x) = n.child_by_field_name("signature").and_then(|s| s.child_by_field_name("name")) else { continue };
                let name = text(x, src).to_string();
                declare(ex, rel, &file_id, &name, &name, n, src);
                d.top.insert(name);
            }
            "top_level_variable_declaration" => {
                for (name, _) in variables(n, src) {
                    declare(ex, rel, &file_id, &name, &name, n, src);
                    d.top.insert(name);
                }
            }
            _ => {}
        }
    }
    d
}
