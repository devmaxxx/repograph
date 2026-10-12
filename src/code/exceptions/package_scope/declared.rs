//! What a Kotlin file declares, as the lookup reads it: the engine's definitions, with the arity,
//! visibility, supertypes and annotations each declaration's node writes. The file's header for the
//! JVM index comes from the same read, so the index never names a symbol the graph lacks.

use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use crate::code::engine::{self, Facts, File};
use crate::code::exceptions::package_scope::{self as jvm, Scope};
use crate::code::index::{Arity, Header, Nested};
use crate::code::lang::Lang;
use crate::code::queries;
use crate::code::spec::spec;
use crate::code::syntax::{child, named, text};

/// What one Kotlin file declares, kept for the passes that resolve names after this one.
#[derive(Debug, Default)]
pub struct Declared {
    /// Top-level names, private ones included. The header keeps them beside `private`, and the index
    /// reads only those `private` leaves out, since no other file reaches a private name.
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
    /// Per member function id, the argument counts its declarations take.
    pub arities: BTreeMap<String, Vec<Arity>>,
    /// Member ids every declaration of which is `private`: no subtype inherits them, and no other
    /// type reaches them. A private overload beside a public one is told apart by its arity.
    pub private_members: BTreeSet<String>,
    /// Member ids some declaration of which is not `private`.
    pub open_members: BTreeSet<String>,
    /// (annotated id, annotation name as written), resolved or dropped by `jvm::decorate`.
    pub annotations: Vec<(String, String)>,
}

/// The argument counts a function takes: a parameter with a default may be left out, and a
/// `vararg` one takes any number. The grammar writes a default as the expression after its
/// parameter and `vararg` as a modifier before it.
fn arity(f: Node, src: &[u8]) -> Arity {
    let inherits = says(f, "override", src) || says(f, "actual", src);
    let mut a = Arity { min: 0, max: 0, varargs: false, inherits, private: says(f, "private", src) };
    let mut vararg = false;
    let mut last_required = false;
    for c in child(f, "function_value_parameters").map(named).unwrap_or_default() {
        match c.kind() {
            "parameter_modifiers" => {
                vararg = named(c).into_iter().any(|m| m.kind() == "parameter_modifier" && text(m, src) == "vararg");
            }
            "parameter" => {
                a.max += 1;
                a.varargs |= vararg;
                last_required = !vararg;
                a.min += usize::from(!vararg);
                vararg = false;
            }
            "line_comment" | "multiline_comment" => {}
            _ if last_required => {
                a.min -= 1;
                last_required = false;
            }
            _ => {}
        }
    }
    a
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

/// Whether any modifier of `n` is `word`, whichever kind the grammar gives it.
fn says(n: Node, word: &str, src: &[u8]) -> bool {
    child(n, "modifiers")
        .is_some_and(|m| named(m).into_iter().any(|c| c.kind() != "annotation" && text(c, src).split_whitespace().any(|w| w == word)))
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
    /// Type parameters in scope at the type `path`.
    pub(super) fn masked(&self, path: &str) -> BTreeSet<String> {
        jvm::masked(&self.shapes, path)
    }
}

/// Kotlin is public by default, and `internal` crosses files inside a module, so only these two
/// keep a name to its file.
fn hidden(n: Node, src: &[u8]) -> bool {
    child(n, "modifiers").is_some_and(|m| {
        named(m).into_iter().any(|c| c.kind() == "visibility_modifier" && matches!(text(c, src), "private" | "protected"))
    })
}

/// `@Name`, `@Name(…)` and `@get:Name` on a declaration, as written. A name a type parameter in
/// scope shadows is the compiler's error, not the class, so it is left out.
fn annotate(n: Node, id: &str, masked: &BTreeSet<String>, src: &[u8], d: &mut Declared) {
    let Some(mods) = child(n, "modifiers") else { return };
    for a in named(mods).into_iter().filter(|c| c.kind() == "annotation") {
        let target = child(a, "constructor_invocation").unwrap_or(a);
        let Some(name) = written_type(target, src) else { continue };
        if !masked.contains(name.split('.').next().unwrap_or_default()) {
            d.annotations.push((id.to_string(), name));
        }
    }
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

/// The declaration node a site's range spans: a node with a single child spans its range too.
fn node_at<'t>(root: Node<'t>, range: &std::ops::Range<usize>) -> Option<Node<'t>> {
    let mut n = root.descendant_for_byte_range(range.start, range.end)?;
    while !DECLARATIONS.contains(&n.kind()) {
        n = n.parent().filter(|p| p.byte_range() == *range)?;
    }
    Some(n)
}

const DECLARATIONS: [&str; 7] =
    ["class_declaration", "object_declaration", "type_alias", "function_declaration", "property_declaration", "class_parameter", "infix_expression"];

/// What the lookup needs of every site the engine declared, read in source order so a type's
/// shape is known before its members are.
pub(crate) fn read(facts: &Facts, root: Node, src: &[u8]) -> Declared {
    let mut d = Declared::default();
    let mut public = BTreeSet::new();
    let mut sites: Vec<_> = facts.sites().collect();
    sites.sort_by_key(|(range, _)| (range.start, std::cmp::Reverse(range.end)));
    for (range, i) in sites {
        let Some(n) = node_at(root, range) else { continue };
        let def = &facts.defs[i];
        let (id, path, name) = (&def.id, &def.qualified, &def.name);
        let owner = def.parent.map(|p| facts.defs[p].qualified.as_str());
        let mut around = owner.map(|o| d.masked(o)).unwrap_or_default();
        if n.kind() != "class_parameter" {
            around.extend(type_params(n, src));
        }
        annotate(n, id, &around, src, &mut d);
        match owner {
            None => {
                d.top.insert(name.clone());
                if hidden(n, src) { d.private.insert(name.clone()) } else { public.insert(name.clone()) };
            }
            Some(_) => {
                d.members.insert(id.clone());
                let visibility = if says(n, "private", src) { &mut d.private_members } else { &mut d.open_members };
                visibility.insert(id.clone());
            }
        }
        let written = if n.kind() == "class_parameter" { written_type(n, src) } else { child(n, "variable_declaration").and_then(|v| written_type(v, src)) };
        if let Some(t) = written {
            d.fields.entry(owner.unwrap_or_default().to_string()).or_default().insert(name.clone(), t);
        }
        if n.kind() == "function_declaration" {
            let mut masked = type_params(n, src);
            masked.extend(owner.map(|o| d.masked(o)).unwrap_or_default());
            d.lambdas.entry(id.clone()).or_default().push(params(n, src, &masked));
            if owner.is_some() {
                let arities = d.arities.entry(id.clone()).or_default();
                arities.push(arity(n, src));
                arities.sort();
                arities.dedup();
            }
        }
        if is_type(n) {
            shape(n, id, path, src, &mut d);
        }
    }
    // An overload with no modifier makes the name reachable from the package.
    d.private.retain(|name| !public.contains(name));
    // Overloads share an id, so one not marked `private` keeps it reachable.
    let open = std::mem::take(&mut d.open_members);
    d.private_members.retain(|id| !open.contains(id));
    d.open_members = open;
    d
}

/// A type's shape and its supertypes as written.
fn shape(n: Node, id: &str, path: &str, src: &[u8], d: &mut Declared) {
    d.types.insert(path.to_string());
    let enumerated = child(n, "enum_class_body").is_some() || has_modifier(n, "enum", src);
    let shape = jvm::Shape {
        data: has_modifier(n, "data", src),
        inner: has_modifier(n, "inner", src),
        object: n.kind() != "class_declaration",
        implicit: enumerated,
        enumerated,
        type_params: type_params(n, src),
        superclass: None,
        ..Default::default()
    };
    d.shapes.insert(path.to_string(), shape);
    for spec in named(n).into_iter().filter(|c| c.kind() == "delegation_specifier") {
        let invoked = child(spec, "constructor_invocation");
        if let Some(t) = written_type(invoked.unwrap_or(spec), src) {
            if invoked.is_some() {
                d.shapes.entry(path.to_string()).or_default().superclass = Some(t.clone());
            }
            d.supers.push((id.to_string(), t));
        }
    }
}

/// The package and the imports. Kotlin has no static import: `import a.b.C.m` names a member, and
/// the lookup finds it through `C`'s file as a nested path.
pub(crate) fn scope(facts: &Facts) -> Scope {
    let mut s = Scope { package: facts.namespaces.first().cloned().unwrap_or_default(), ..Scope::default() };
    for import in &facts.imports {
        let Some(qualified) = import.path.clone() else { continue };
        if import.names.iter().any(|n| n == "*") {
            s.stars.push(qualified);
            continue;
        }
        let local = import.alias.clone().unwrap_or_else(|| qualified.rsplit('.').next().unwrap_or(&qualified).to_string());
        s.singles.entry(local).or_default().insert(qualified);
    }
    s
}

/// The package line, every top-level name and every nested path, for the JVM index and for the
/// headers `widen` stores.
pub(crate) fn header(rel: &str, source: &str) -> Header {
    let (Some(decl), Some(tree)) = (queries::decl(Lang::Kotlin), Lang::Kotlin.parse(source.as_bytes())) else { return Header::default() };
    let file = File { lang: Lang::Kotlin, rel, text: source, src: source, tree: Some(&tree) };
    let facts = engine::facts(spec(Lang::Kotlin), decl, None, &file);
    let scope = scope(&facts);
    let d = read(&facts, tree.root_node(), source.as_bytes());
    // A private member reaches no other file, not even a subclass's.
    let reached = |id: &&String| !d.private_members.contains(*id);
    let arities = d.arities.iter().filter(|(id, _)| reached(id)).map(|(id, a)| (jvm::path_of(id).to_string(), a.clone())).collect();
    let members = d.members.iter().filter(reached).map(|id| jvm::path_of(id).to_string()).collect();
    let supers = jvm::recorded(&d.types, &d.supers, &d.shapes, &scope);
    let nested = Nested { types: d.types, members, arities, supers, ..Default::default() };
    let package = if scope.package.is_empty() { Vec::new() } else { vec![scope.package] };
    Header { scope: package, top: d.top, nested, private: d.private, ..Default::default() }
}
