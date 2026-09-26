//! Java's declarations: types, members as `Type.member`, visibility, and supertypes as written.

use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use super::COMMENTS;
use crate::code::jvm::{self, child, named, span, text};
use crate::model::{EdgeKind, Extraction, NodeKind};

pub(crate) const TYPES: &[&str] = &[
    "class_declaration",
    "interface_declaration",
    "enum_declaration",
    "record_declaration",
    "annotation_type_declaration",
];

/// What one Java file declares, kept for the passes that resolve names after this one.
#[derive(Debug, Default)]
pub struct Declared {
    /// Top-level types, package-private ones included: the index and `widen` both read exactly these.
    pub top: BTreeSet<String>,
    /// Every type path declared here (`Outer`, `Outer.Inner`), so a name resolves in-file first.
    pub types: BTreeSet<String>,
    /// Every member id declared here.
    pub members: BTreeSet<String>,
    /// Per type path, each field's declared type as written (`Invoice`, `shop.billing.Invoice`).
    pub fields: BTreeMap<String, BTreeMap<String, String>>,
    /// (declaring type's id, supertype as written).
    pub supers: Vec<(String, String)>,
}

/// Whether `modifiers` holds the keyword; the grammar gives keywords as unnamed children.
fn says(n: Node, word: &str) -> bool {
    child(n, "modifiers").is_some_and(|m| {
        let mut c = m.walk();
        let found = m.children(&mut c).any(|k| k.kind() == word);
        found
    })
}

/// Only `public` crosses a package, and a package is not a file. An interface's and an annotation
/// type's members are public without the word, so there the language's visibility is read, not the token.
fn exported(n: Node, members_public: bool) -> &'static str {
    if says(n, "public") || (members_public && !says(n, "private")) { "export" } else { "" }
}

/// A type as written with its generic arguments dropped; none for a primitive or an array, which
/// no method call on a declared type is made through.
pub(super) fn written_type(t: Node, src: &[u8]) -> Option<String> {
    match t.kind() {
        "type_identifier" => Some(text(t, src).to_string()),
        "scoped_type_identifier" => Some(text(t, src).split_whitespace().collect()),
        "generic_type" => named(t).into_iter()
            .find(|c| matches!(c.kind(), "type_identifier" | "scoped_type_identifier"))
            .and_then(|c| written_type(c, src)),
        _ => None,
    }
}

pub fn scan(root: Node, rel: &str, src: &[u8], ex: &mut Extraction) -> Declared {
    let file_id = format!("file:{rel}");
    let mut d = Declared::default();
    for n in named(root).into_iter().filter(|n| TYPES.contains(&n.kind())) {
        declare_type(n, rel, src, &file_id, None, false, ex, &mut d);
    }
    d
}

#[allow(clippy::too_many_arguments)]
fn declare_type(n: Node, rel: &str, src: &[u8], parent: &str, owner: Option<&str>, members_public: bool, ex: &mut Extraction, d: &mut Declared) {
    let Some(name_at) = n.child_by_field_name("name") else { return };
    let name = text(name_at, src).to_string();
    let path = owner.map_or_else(|| name.clone(), |o| format!("{o}.{name}"));
    let id = format!("sym:{rel}::{path}");
    ex.node_span(NodeKind::Symbol, &id, &path, &jvm::body(n, name_at, src, COMMENTS), rel, span(n));
    ex.edge(parent, &id, EdgeKind::Declares, exported(n, members_public), rel);
    if owner.is_none() {
        d.top.insert(name);
    } else {
        d.members.insert(id.clone());
    }
    d.types.insert(path.clone());
    // `extends` on a class, `implements` on a class, enum or record, `extends` on an interface.
    let heads = [n.child_by_field_name("superclass"), n.child_by_field_name("interfaces"), child(n, "extends_interfaces")];
    for head in heads.into_iter().flatten() {
        let list = child(head, "type_list").unwrap_or(head);
        for t in named(list) {
            if let Some(w) = written_type(t, src) {
                d.supers.push((id.clone(), w));
            }
        }
    }
    let Some(body) = n.child_by_field_name("body") else { return };
    let inner_public = matches!(n.kind(), "interface_declaration" | "annotation_type_declaration");
    let items: Vec<Node> = if body.kind() == "enum_body" {
        named(body).into_iter().filter(|c| c.kind() == "enum_body_declarations").flat_map(named).collect()
    } else {
        named(body)
    };
    for m in items {
        if TYPES.contains(&m.kind()) {
            declare_type(m, rel, src, &id, Some(&path), inner_public, ex, d);
        } else {
            declare_member(m, rel, src, &id, &path, inner_public, ex, d);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn declare_member(m: Node, rel: &str, src: &[u8], parent: &str, path: &str, members_public: bool, ex: &mut Extraction, d: &mut Declared) {
    let names: Vec<Node> = match m.kind() {
        // A constructor is named by its type, so it is `Type.Type`, and overloads collapse into one.
        "method_declaration" | "constructor_declaration" | "annotation_type_element_declaration" => {
            m.child_by_field_name("name").into_iter().collect()
        }
        // `int a, b;` declares two fields.
        "field_declaration" | "constant_declaration" => {
            let mut c = m.walk();
            let declarators: Vec<Node> = m.children_by_field_name("declarator", &mut c).collect();
            declarators.into_iter().filter_map(|v| v.child_by_field_name("name")).collect()
        }
        _ => return,
    };
    let is_field = matches!(m.kind(), "field_declaration" | "constant_declaration");
    let ty = m.child_by_field_name("type").and_then(|t| written_type(t, src));
    for name_at in names {
        let name = text(name_at, src);
        let id = format!("sym:{rel}::{path}.{name}");
        ex.node_span(NodeKind::Symbol, &id, &format!("{path}.{name}"), &jvm::body(m, name_at, src, COMMENTS), rel, span(m));
        ex.edge(parent, &id, EdgeKind::Declares, exported(m, members_public), rel);
        d.members.insert(id);
        if let (true, Some(t)) = (is_field, &ty) {
            d.fields.entry(path.to_string()).or_default().insert(name.to_string(), t.clone());
        }
    }
}
