//! One walk over a GraphQL document: operations and fragments under their own namespaces, schema types and their
//! fields, and every name the document uses, unresolved. `mod.rs` resolves through the GraphQL index, so `header`
//! and `extract` read one list of declarations.
use tree_sitter::Node;

use crate::code::jvm::{child, find, named, text};
use crate::model::EdgeKind;

/// Capped as TypeScript's doc comments are, so a long description does not drown the declaring line.
const DOC_CHARS: usize = 600;

pub(super) struct Object {
    /// The id tail: `query/GetShelf`, `fragment/ShelfFields`, `Shelf`, `directive/auth`.
    pub name: String,
    pub label: String,
    pub span: (u32, u32),
    pub body: String,
    /// Declared by `extend type T` and not by `T`'s own definition.
    pub extension: bool,
}

pub(super) struct Member {
    pub owner: String,
    pub name: String,
    pub span: (u32, u32),
    pub body: String,
}

pub(super) struct Link {
    /// The id tail of the declaration the name is used in; `None` inside an anonymous operation.
    pub from: Option<String>,
    pub to: String,
    pub kind: EdgeKind,
    pub context: &'static str,
}

pub(super) struct Cite {
    pub from: Option<String>,
    pub context: &'static str,
    pub text: String,
}

#[derive(Default)]
pub(super) struct Read {
    pub objects: Vec<Object>,
    pub members: Vec<Member>,
    pub links: Vec<Link>,
    pub cites: Vec<Cite>,
}

pub(super) fn read(root: Node, src: &[u8], lines: &[&str]) -> Read {
    let mut r = Read::default();
    let mut definitions = Vec::new();
    find(root, &["definition", "comment"], &mut definitions);
    for d in definitions {
        if d.kind() == "comment" {
            r.cites.push(Cite { from: None, context: "comment", text: text(d, src).to_string() });
            continue;
        }
        let owner = definition(d, src, lines, &mut r);
        let mut texts = Vec::new();
        find(d, &["comment", "string_value"], &mut texts);
        for t in texts {
            let context = if t.kind() == "comment" { "comment" } else { "string" };
            r.cites.push(Cite { from: owner.clone(), context, text: text(t, src).to_string() });
        }
    }
    r
}

/// Through the grammar's wrappers to the node that is the definition itself.
fn inner(mut n: Node<'_>) -> Node<'_> {
    while matches!(n.kind(), "definition" | "executable_definition" | "type_system_definition" | "type_system_extension" | "type_definition" | "type_extension") {
        match n.named_child(0) {
            Some(c) => n = c,
            None => break,
        }
    }
    n
}

/// A description, or failing that the `#` lines directly above. The first definition's comments are the source
/// file's children, above the `document` node, so the walk steps out of the document to reach them.
fn doc(d: Node, n: Node, src: &[u8]) -> String {
    if let Some(desc) = child(n, "description") {
        return text(desc, src).trim_matches('"').trim().chars().take(DOC_CHARS).collect();
    }
    let mut parts = Vec::new();
    let mut next = d;
    loop {
        let Some(prev) = next.prev_named_sibling() else {
            match next.parent().filter(|p| p.kind() == "document") {
                Some(document) => {
                    next = document;
                    continue;
                }
                None => break,
            }
        };
        // A comment on the row the definition before it ends on is that definition's tail, not this one's doc.
        let trailing = prev.prev_named_sibling().is_some_and(|p| p.end_position().row == prev.start_position().row);
        if prev.kind() != "comment" || trailing || prev.end_position().row + 1 < next.start_position().row {
            break;
        }
        parts.push(text(prev, src).trim_start_matches('#').trim().to_string());
        next = prev;
    }
    parts.reverse();
    parts.join("\n").chars().take(DOC_CHARS).collect()
}

fn definition(d: Node, src: &[u8], lines: &[&str], r: &mut Read) -> Option<String> {
    let n = inner(d);
    // A description sits inside its definition, above the name; the name's row is the declaring line.
    let row = child(n, "name").or_else(|| child(n, "fragment_name")).map_or(n.start_position().row, |x| x.start_position().row);
    let at = (row as u32 + 1, n.end_position().row as u32 + 1);
    let doc = doc(d, n, src);
    let line = lines.get(row).map_or("", |l| l.trim());
    let body = if doc.is_empty() { line.to_string() } else { format!("{doc}\n{line}") };
    let declare = |r: &mut Read, name: String, label: &str, extension: bool| -> String {
        r.objects.push(Object { name: name.clone(), label: label.to_string(), span: at, body: body.clone(), extension });
        name
    };
    match n.kind() {
        "operation_definition" => {
            let operation = child(n, "operation_type").map_or("query", |t| text(t, src).trim());
            let owner = child(n, "name").map(|x| declare(r, format!("{operation}/{}", text(x, src)), text(x, src), false));
            uses(n, owner.as_deref(), src, r);
            owner
        }
        "fragment_definition" => {
            let name = text(child(child(n, "fragment_name")?, "name")?, src);
            let owner = declare(r, format!("fragment/{name}"), name, false);
            uses(n, Some(&owner), src, r);
            Some(owner)
        }
        "directive_definition" => {
            let name = text(child(n, "name")?, src);
            let owner = declare(r, format!("directive/{name}"), name, false);
            arguments(n, &owner, src, r);
            Some(owner)
        }
        kind if kind.ends_with("_type_definition") || kind.ends_with("_type_extension") => {
            let name = text(child(n, "name")?, src);
            let owner = declare(r, name.to_string(), name, kind.ends_with("_extension"));
            schema_type(n, &owner, src, lines, r);
            Some(owner)
        }
        _ => None,
    }
}

fn link(r: &mut Read, from: Option<&str>, to: Option<String>, kind: EdgeKind, context: &'static str) {
    if let Some(to) = to {
        r.links.push(Link { from: from.map(str::to_string), to, kind, context });
    }
}

/// The type a `type` node names beneath its list and non-null wrappers.
fn named_type(n: Node, src: &[u8]) -> Option<String> {
    if n.kind() == "named_type" {
        return child(n, "name").map(|x| text(x, src).to_string());
    }
    named(n).into_iter().find_map(|c| named_type(c, src))
}

/// What an operation or a fragment uses. A spread is a call, because codegen inlines the fragment where it is
/// spread. A type condition or a variable's type is a reference to a schema type.
fn uses(n: Node, owner: Option<&str>, src: &[u8], r: &mut Read) {
    let mut found = Vec::new();
    find(n, &["fragment_spread", "type_condition", "variable_definition", "directive"], &mut found);
    for u in found {
        match u.kind() {
            "fragment_spread" => {
                let name = child(u, "fragment_name").and_then(|f| child(f, "name")).map(|x| format!("fragment/{}", text(x, src)));
                link(r, owner, name, EdgeKind::Calls, "");
                spread_directives(u, owner, src, r);
            }
            "type_condition" => link(r, owner, named_type(u, src), EdgeKind::References, "on"),
            "variable_definition" => {
                link(r, owner, child(u, "type").and_then(|t| named_type(t, src)), EdgeKind::References, "variable");
                spread_directives(u, owner, src, r);
            }
            _ => link(r, owner, child(u, "name").map(|x| format!("directive/{}", text(x, src))), EdgeKind::DecoratedBy, ""),
        }
    }
}

/// The directives written on a spread or a variable, which `find` does not reach because it stops at the node.
fn spread_directives(n: Node, owner: Option<&str>, src: &[u8], r: &mut Read) {
    let mut found = Vec::new();
    find(n, &["directive"], &mut found);
    for d in found {
        link(r, owner, child(d, "name").map(|x| format!("directive/{}", text(x, src))), EdgeKind::DecoratedBy, "");
    }
}

/// A type's interfaces, union members, directives and fields. A field is `Type.field`; its type and its arguments'
/// types are references from the field, so `impact` on a type reaches every field that returns or takes it.
fn schema_type(n: Node, owner: &str, src: &[u8], lines: &[&str], r: &mut Read) {
    let mut interfaces = Vec::new();
    if let Some(list) = child(n, "implements_interfaces") {
        find(list, &["named_type"], &mut interfaces);
    }
    for i in interfaces {
        link(r, Some(owner), named_type(i, src), EdgeKind::Extends, "");
    }
    let mut members = Vec::new();
    if let Some(list) = child(n, "union_member_types") {
        find(list, &["named_type"], &mut members);
    }
    for m in members {
        link(r, Some(owner), named_type(m, src), EdgeKind::References, "member");
    }
    directives(n, owner, src, r);
    let Some(fields) = child(n, "fields_definition").or_else(|| child(n, "input_fields_definition")) else { return };
    for f in named(fields).into_iter().filter(|f| matches!(f.kind(), "field_definition" | "input_value_definition")) {
        let Some(name) = child(f, "name") else { continue };
        let row = name.start_position().row;
        let id = format!("{owner}.{}", text(name, src));
        r.members.push(Member {
            owner: owner.to_string(),
            name: text(name, src).to_string(),
            span: (row as u32 + 1, f.end_position().row as u32 + 1),
            body: lines.get(row).map_or("", |l| l.trim()).to_string(),
        });
        link(r, Some(&id), child(f, "type").and_then(|t| named_type(t, src)), EdgeKind::References, "type");
        arguments(f, &id, src, r);
        directives(f, &id, src, r);
    }
}

fn arguments(n: Node, from: &str, src: &[u8], r: &mut Read) {
    let Some(list) = child(n, "arguments_definition") else { return };
    for a in named(list).into_iter().filter(|a| a.kind() == "input_value_definition") {
        link(r, Some(from), child(a, "type").and_then(|t| named_type(t, src)), EdgeKind::References, "argument");
    }
}

/// A directive on a declaration decorates it, pointing at the directive's definition when the repository has one.
fn directives(n: Node, from: &str, src: &[u8], r: &mut Read) {
    let Some(list) = child(n, "directives") else { return };
    for d in named(list).into_iter().filter(|d| d.kind() == "directive") {
        link(r, Some(from), child(d, "name").map(|x| format!("directive/{}", text(x, src))), EdgeKind::DecoratedBy, "");
    }
}
