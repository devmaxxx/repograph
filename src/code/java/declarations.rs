//! Java's declarations: types, members as `Type.member`, visibility, and supertypes as written.

use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use super::COMMENTS;
use crate::code::index::Arity;
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
    /// Per type path, each field's declared type as written (`Invoice`, `shop.billing.Invoice`),
    /// a record's components included.
    pub fields: BTreeMap<String, BTreeMap<String, String>>,
    /// (declaring type's id, supertype as written).
    pub supers: Vec<(String, String)>,
    /// Every method, constructor and annotation element id.
    pub methods: BTreeSet<String>,
    /// Every field id, and a record component's though it is no symbol: calls look fields up apart
    /// from methods.
    pub values: BTreeSet<String>,
    /// The method ids and the value ids some declaration not marked `private` reaches: a private
    /// member is not inherited, and no other file can call it.
    pub open_methods: BTreeSet<String>,
    pub open_values: BTreeSet<String>,
    /// Per type path, how it reaches names it does not declare.
    pub shapes: BTreeMap<String, jvm::Shape>,
    /// Per method id, the argument counts its declarations take.
    pub arities: BTreeMap<String, Vec<Arity>>,
    /// Method ids every declaration of which is `static`.
    pub statics: BTreeSet<String>,
    /// Method ids some declaration of which is not `static`.
    instance: BTreeSet<String>,
}

/// The argument counts a method's `formal_parameters` take. A receiver parameter is no argument.
pub(super) fn arity(params: Option<Node>) -> Arity {
    let params: Vec<Node> = params.map(named).unwrap_or_default().into_iter().filter(|p| matches!(p.kind(), "formal_parameter" | "spread_parameter")).collect();
    let varargs = params.last().is_some_and(|p| p.kind() == "spread_parameter");
    Arity { min: params.len() - usize::from(varargs), max: params.len(), varargs, inherits: false }
}

/// A type's or a method's own type parameters.
pub(super) fn type_params(n: Node, src: &[u8]) -> BTreeSet<String> {
    n.child_by_field_name("type_parameters")
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| child(p, "type_identifier"))
        .map(|t| text(t, src).to_string())
        .collect()
}

/// The supertypes a type declaration writes, as written: `extends` on a class, `implements` on a
/// class, enum or record, `extends` on an interface.
pub(super) fn supertypes(n: Node, src: &[u8]) -> Vec<String> {
    let heads = [n.child_by_field_name("superclass"), n.child_by_field_name("interfaces"), child(n, "extends_interfaces")];
    heads.into_iter().flatten().flat_map(|head| named(child(head, "type_list").unwrap_or(head))).filter_map(|t| written_type(t, src)).collect()
}

/// Whether `modifiers` holds the keyword; the grammar gives keywords as unnamed children.
pub(super) fn says(n: Node, word: &str) -> bool {
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
    // An overload set collapses to one id, so it is static only when every declaration is.
    d.statics.retain(|id| !d.instance.contains(id));
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
    for w in supertypes(n, src) {
        d.supers.push((id.clone(), w));
    }
    let shape = jvm::Shape {
        // A member class not marked `static` holds its outer instance; a nested interface, enum or
        // record, and any type inside an interface, is static without the word.
        inner: owner.is_some() && n.kind() == "class_declaration" && !members_public && !says(n, "static"),
        // An enum's `values`, a record's accessors and an annotation's `annotationType` are
        // members the file never writes.
        implicit: matches!(n.kind(), "enum_declaration" | "record_declaration" | "annotation_type_declaration"),
        type_params: type_params(n, src),
        ..Default::default()
    };
    d.shapes.insert(path.clone(), shape);
    if n.kind() == "record_declaration" {
        for p in n.child_by_field_name("parameters").map(named).unwrap_or_default() {
            let Some(pname) = p.child_by_field_name("name").map(|x| text(x, src)) else { continue };
            d.values.insert(format!("{id}.{pname}"));
            if let Some(t) = p.child_by_field_name("type").and_then(|t| written_type(t, src)) {
                d.fields.entry(path.clone()).or_default().insert(pname.to_string(), t);
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
    let open = !says(m, "private");
    let ty = m.child_by_field_name("type").and_then(|t| written_type(t, src));
    for name_at in names {
        let name = text(name_at, src);
        let id = format!("sym:{rel}::{path}.{name}");
        ex.node_span(NodeKind::Symbol, &id, &format!("{path}.{name}"), &jvm::body(m, name_at, src, COMMENTS), rel, span(m));
        ex.edge(parent, &id, EdgeKind::Declares, exported(m, members_public), rel);
        d.members.insert(id.clone());
        let (all, reached) = if is_field { (&mut d.values, &mut d.open_values) } else { (&mut d.methods, &mut d.open_methods) };
        all.insert(id.clone());
        if !is_field {
            let arities = d.arities.entry(id.clone()).or_default();
            arities.push(arity(m.child_by_field_name("parameters")));
            arities.sort();
            arities.dedup();
            if says(m, "static") { d.statics.insert(id.clone()) } else { d.instance.insert(id.clone()) };
        }
        if open {
            reached.insert(id);
        }
        if let (true, Some(t)) = (is_field, &ty) {
            d.fields.entry(path.to_string()).or_default().insert(name.to_string(), t.clone());
        }
    }
}
