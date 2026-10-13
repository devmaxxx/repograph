//! What a Java file declares, as the lookup reads it: the engine's definitions, with the arity,
//! visibility, supertypes and annotations each declaration's node writes. The file's header for the
//! JVM index comes from the same read, so the index never names a symbol the graph lacks. `calls`
//! walks the file's calls and the types its signatures name.

mod calls;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use super::declared::{facts_of, scope, sites};
use crate::code::engine::{Def, Facts, File, Sink};
use crate::code::exceptions::package_scope::{self as jvm};
use crate::code::imports::Resolver;
use crate::code::index::{Arity, Header, Nested, QualifiedIndex, Reach};
use crate::code::lang::{Family, Lang};
use crate::code::syntax::{child, named, text};

pub(crate) const TYPES: &[&str] = &[
    "class_declaration",
    "interface_declaration",
    "enum_declaration",
    "record_declaration",
    "annotation_type_declaration",
];

const MEMBERS: &[&str] =
    &["method_declaration", "constructor_declaration", "annotation_type_element_declaration", "field_declaration", "constant_declaration"];

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
    /// Per member id some declaration of which a subtype may not inherit, which ones.
    pub reach: BTreeMap<String, Reach>,
    /// (annotated id, annotation name as written), resolved or dropped by `jvm::decorate`.
    pub annotations: Vec<(String, String)>,
}

/// The argument counts a method's `formal_parameters` take. A receiver parameter is no argument.
fn arity(params: Option<Node>) -> Arity {
    let params: Vec<Node> = params.map(named).unwrap_or_default().into_iter().filter(|p| matches!(p.kind(), "formal_parameter" | "spread_parameter")).collect();
    let varargs = params.last().is_some_and(|p| p.kind() == "spread_parameter");
    Arity { min: params.len() - usize::from(varargs), max: params.len(), varargs, inherits: false, private: false }
}

/// A type's or a method's own type parameters.
fn type_params(n: Node, src: &[u8]) -> BTreeSet<String> {
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
fn supertypes(n: Node, src: &[u8]) -> Vec<String> {
    let heads = [n.child_by_field_name("superclass"), n.child_by_field_name("interfaces"), child(n, "extends_interfaces")];
    heads.into_iter().flatten().flat_map(|head| named(child(head, "type_list").unwrap_or(head))).filter_map(|t| written_type(t, src)).collect()
}

/// Whether `modifiers` holds the keyword; the grammar gives keywords as unnamed children.
fn says(n: Node, word: &str) -> bool {
    child(n, "modifiers").is_some_and(|m| {
        let mut c = m.walk();
        let found = m.children(&mut c).any(|k| k.kind() == word);
        found
    })
}

/// Whether the body `n` sits in is an interface's or an annotation type's, whose members are
/// public without the word.
fn in_public_body(n: Node) -> bool {
    n.parent().is_some_and(|b| matches!(b.kind(), "interface_body" | "annotation_type_body"))
}

/// `@Name`, `@Name(…)` and `@a.b.Name(…)` on a declaration, as written. A name a type variable in
/// scope shadows is the compiler's error, not the annotation type, so it is left out.
fn annotate(n: Node, id: &str, masked: &BTreeSet<String>, src: &[u8], d: &mut Declared) {
    let Some(mods) = child(n, "modifiers") else { return };
    for a in named(mods).into_iter().filter(|c| matches!(c.kind(), "marker_annotation" | "annotation")) {
        let Some(name) = a.child_by_field_name("name") else { continue };
        let name: String = text(name, src).split_whitespace().collect();
        if !masked.contains(name.split('.').next().unwrap_or_default()) {
            d.annotations.push((id.to_string(), name));
        }
    }
}

/// The type parameters an annotation on `n`, declared inside the type at `owner`, sees.
fn around(n: Node, owner: Option<&str>, src: &[u8], d: &Declared) -> BTreeSet<String> {
    let mut masked = owner.map(|o| jvm::masked(&d.shapes, o)).unwrap_or_default();
    masked.extend(type_params(n, src));
    masked
}

/// A type as written with its generic arguments dropped; none for a primitive or an array, which
/// no method call on a declared type is made through.
fn written_type(t: Node, src: &[u8]) -> Option<String> {
    match t.kind() {
        "type_identifier" => Some(text(t, src).to_string()),
        "scoped_type_identifier" => Some(text(t, src).split_whitespace().collect()),
        "generic_type" => named(t).into_iter()
            .find(|c| matches!(c.kind(), "type_identifier" | "scoped_type_identifier"))
            .and_then(|c| written_type(c, src)),
        _ => None,
    }
}

/// What the lookup needs of every site the engine declared.
pub(crate) fn read(facts: &Facts, root: Node, src: &[u8]) -> Declared {
    let mut d = Declared::default();
    let kinds: Vec<&str> = TYPES.iter().chain(MEMBERS).copied().collect();
    for (n, i) in sites(facts, root, &kinds) {
        let def = &facts.defs[i];
        let owner = def.parent.map(|p| facts.defs[p].qualified.as_str());
        if TYPES.contains(&n.kind()) {
            declare_type(n, def, owner, src, &mut d);
        } else if let Some(owner) = owner {
            declare_member(n, def, owner, src, &mut d);
        }
    }
    // An overload set collapses to one id, so it is static only when every declaration is.
    d.statics.retain(|id| !d.instance.contains(id));
    d.reach.retain(|_, r| r.package || r.never);
    d
}

fn declare_type(n: Node, def: &Def, owner: Option<&str>, src: &[u8], d: &mut Declared) {
    let (id, path) = (&def.id, &def.qualified);
    annotate(n, id, &around(n, owner, src, d), src, d);
    let members_public = in_public_body(n);
    if owner.is_none() {
        d.top.insert(def.name.clone());
    } else {
        d.members.insert(id.clone());
        // A member type is inherited as a member is, so a subtype reads it by its simple name.
        let r = d.reach.entry(id.clone()).or_default();
        if says(n, "private") {
            r.never = true;
        } else if !members_public && !says(n, "public") && !says(n, "protected") {
            r.package = true;
        } else {
            r.always = true;
        }
    }
    d.types.insert(path.clone());
    for w in supertypes(n, src) {
        d.supers.push((id.clone(), w));
    }
    let components: Vec<Node> = match n.kind() {
        "record_declaration" => n.child_by_field_name("parameters").map(named).unwrap_or_default(),
        _ => Vec::new(),
    };
    let shape = jvm::Shape {
        // A member class not marked `static` holds its outer instance; a nested interface, enum or
        // record, and any type inside an interface, is static without the word.
        inner: owner.is_some() && n.kind() == "class_declaration" && !members_public && !says(n, "static"),
        // An enum's `values`, a record's accessors and an annotation's `annotationType` are
        // members the file never writes.
        implicit: matches!(n.kind(), "enum_declaration" | "record_declaration" | "annotation_type_declaration"),
        enumerated: n.kind() == "enum_declaration",
        components: components.iter().filter_map(|p| p.child_by_field_name("name")).map(|x| text(x, src).to_string()).collect(),
        type_params: type_params(n, src),
        superclass: n.child_by_field_name("superclass").and_then(|h| named(h).into_iter().find_map(|t| written_type(t, src))),
        ..Default::default()
    };
    d.shapes.insert(path.clone(), shape);
    for p in components {
        let Some(name) = p.child_by_field_name("name").map(|x| text(x, src)) else { continue };
        d.values.insert(format!("{id}.{name}"));
        if let Some(t) = p.child_by_field_name("type").and_then(|t| written_type(t, src)) {
            d.fields.entry(path.clone()).or_default().insert(name.to_string(), t);
        }
    }
}

/// One name a member declaration binds: `int a, b;` is two sites of one node.
fn declare_member(m: Node, def: &Def, owner: &str, src: &[u8], d: &mut Declared) {
    let id = &def.id;
    let members_public = in_public_body(m);
    let is_field = matches!(m.kind(), "field_declaration" | "constant_declaration");
    annotate(m, id, &around(m, Some(owner), src, d), src, d);
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
    if !says(m, "private") {
        let r = d.reach.entry(id.clone()).or_default();
        if members_public && !is_field && says(m, "static") {
            r.never = true;
        } else if !members_public && !says(m, "public") && !says(m, "protected") {
            r.package = true;
        } else {
            r.always = true;
        }
        reached.insert(id.clone());
    }
    if let (true, Some(t)) = (is_field, m.child_by_field_name("type").and_then(|t| written_type(t, src))) {
        d.fields.entry(owner.to_string()).or_default().insert(def.name.clone(), t);
    }
}

/// The package line, every top-level type and every nested path, for the JVM index and for the
/// headers `widen` stores.
pub(crate) fn header(rel: &str, source: &str) -> Header {
    let Some((facts, tree)) = facts_of(Lang::Java, rel, source) else { return Header::default() };
    let scope = scope(&facts);
    let d = read(&facts, tree.root_node(), source.as_bytes());
    let path = |id: &String| jvm::path_of(id).to_string();
    // A nested type another file may name keeps its path; a private member drops out, as no
    // other file reaches it, not even a subclass.
    let reached = |id: &String| d.types.contains(jvm::path_of(id)) || d.open_methods.contains(id) || d.open_values.contains(id);
    let values = d.open_values.difference(&d.open_methods).map(path).collect();
    let arities = d.arities.iter().filter(|(id, _)| d.open_methods.contains(*id)).map(|(id, a)| (path(id), a.clone())).collect();
    let supers = jvm::recorded(&d.types, &d.supers, &d.shapes, &scope);
    let reach = d.reach.iter().map(|(id, r)| (path(id), *r)).collect();
    let nested = Nested { members: d.members.iter().filter(|id| reached(id)).map(path).collect(), values, arities, types: d.types, supers, reach };
    let package = if scope.package.is_empty() { Vec::new() } else { vec![scope.package] };
    Header { scope: package, top: d.top, nested, ..Default::default() }
}

/// Calls, supertypes, annotations and the types signatures name, bound through receivers,
/// supertypes and overloads the engine's default does not read.
pub(crate) fn resolve(resolver: &Resolver, file: &File, facts: &Facts, sink: &mut Sink) {
    let Some(tree) = file.tree else { return };
    let (rel, src, root) = (file.rel, file.src.as_bytes(), tree.root_node());
    // A repository whose globs reach no JVM file has no index; every lookup then finds nothing.
    let empty = QualifiedIndex::default();
    let index = resolver.index(Family::Jvm).unwrap_or(&empty);
    let scope = scope(facts);
    let d = read(facts, root, src);
    let methods = jvm::Own {
        rel,
        types: &d.types,
        members: &d.methods,
        inheritable: &d.open_methods,
        statics: &d.statics,
        arities: &d.arities,
        private: &jvm::NONE,
        reach: &d.reach,
        at: "",
        supers: &d.supers,
        shapes: &d.shapes,
        index,
        scope: &scope,
        kind: jvm::Kind::Method,
        call: None,
        member_types: true,
        past_unread: false,
    };
    jvm::link(&d.supers, index, &scope, rel, |at, w| methods.type_at(at, w), sink);
    jvm::decorate(&d.annotations, rel, |at, w| methods.type_at(at, w), sink);
    let fields = jvm::Own { members: &d.values, inheritable: &d.open_values, statics: &jvm::NONE, kind: jvm::Kind::Field, ..methods };
    let member_types = RefCell::default();
    calls::scan(root, &calls::Ctx { methods, fields, src, d: &d, facts, member_types: &member_types }, sink);
}
