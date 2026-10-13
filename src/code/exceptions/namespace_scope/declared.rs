//! What the .NET index keeps of a C# file's declarations, read from the query's facts and the tree.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use tree_sitter::Node;

use super::{dotted, generic, head, modifiers, Host};
use crate::code::engine::Facts;
use crate::code::exceptions::package_scope;
use crate::code::index::Header;
use crate::code::syntax::{named, text};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Using {
    /// `using Shop.Payments;`: the types directly in the namespace.
    Namespace(String),
    /// `using static Shop.Checks.Guard;`: the members and nested types of one type.
    Static(String),
    /// `using Pay = Shop.Payments.IPaymentGateway;`
    Alias(String, String),
}

impl Using {
    /// The directive as written, less `global` and the semicolon: a `Header.directives` entry.
    pub fn spelled(&self) -> String {
        match self {
            Using::Namespace(n) => format!("using {n}"),
            Using::Static(t) => format!("using static {t}"),
            Using::Alias(a, t) => format!("using {a} = {t}"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TypeDecl {
    /// `Shop.Orders`; empty for the global namespace.
    pub namespace: String,
    /// What follows `sym:<rel>::`: `OrderService`, `OrderService.Line`.
    pub local: String,
    pub partial: bool,
    /// Member name → the head name of its declared type; `None` for methods and constructors.
    pub members: BTreeMap<String, Option<String>>,
    /// Members whose declared type is one name and nothing more — no `?`, `[]` or type arguments —
    /// the only shape C#'s "Color Color" rule reads as the type itself.
    pub plain: BTreeSet<String>,
    /// Base-list names as written, resolved by the references pass.
    pub bases: Vec<String>,
    /// The `bases` written with type arguments. C# resolves a base by its name alone, but Razor
    /// needs the arity to tell the framework's `ComponentBase` from a library's `ComponentBase<T>`.
    pub generic_bases: BTreeSet<String>,
    /// Methods whose first parameter carries `this`.
    pub extensions: BTreeSet<String>,
}

impl TypeDecl {
    pub fn full(&self) -> String {
        join(&self.namespace, &self.local)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Declared {
    pub types: Vec<TypeDecl>,
    /// File usings, block-namespace usings included: a using inside `namespace A { … }` is read as
    /// the file's, which can only widen what resolves and never names a wrong declaration in practice.
    pub usings: Vec<Using>,
    /// `global using`: every file of the same project reads these (Task 3).
    pub global_usings: Vec<Using>,
}

impl Declared {
    /// Every part of `Namespace.Outer.Inner` this file declares.
    pub fn parts(&self, full: &str) -> Vec<&TypeDecl> {
        self.types.iter().filter(|t| t.full() == full).collect()
    }

    /// The namespaces that directly declare a top-level type, in file order, and those types' names;
    /// every using, spelled, as the header's directives. `widen` compares whole headers, and a
    /// `global using` changes what every file of its project resolves, so it has to move the header;
    /// `index_header` skips directives, so none becomes a qualified name.
    pub fn header(&self) -> Header {
        let mut h = Header::default();
        for t in self.types.iter().filter(|t| !t.local.contains('.')) {
            if !t.namespace.is_empty() && !h.scope.contains(&t.namespace) {
                h.scope.push(t.namespace.clone());
            }
            h.top.insert(t.local.clone());
        }
        h.directives.extend(self.usings.iter().map(Using::spelled));
        h.directives.extend(self.global_usings.iter().map(|u| format!("global {}", u.spelled())));
        h
    }
}

pub fn join(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_string(),
        (_, true) => a.to_string(),
        _ => format!("{a}.{b}"),
    }
}

/// `Status` or `Shop.Orders.Status`: a name, dotted or not, with no type arguments anywhere.
fn plain_type(t: Option<Node>) -> bool {
    t.is_some_and(|t| match t.kind() {
        "identifier" => true,
        "qualified_name" => ["qualifier", "name"].iter().all(|f| plain_type(t.child_by_field_name(f))),
        _ => false,
    })
}

const DECLARATIONS: [&str; 13] = [
    "class_declaration", "struct_declaration", "interface_declaration", "record_declaration", "enum_declaration",
    "delegate_declaration", "method_declaration", "constructor_declaration", "property_declaration",
    "event_declaration", "field_declaration", "event_field_declaration", "parameter",
];

/// What the lookup needs of every declaration the query read: each type's namespace, parts, members
/// with their declared types, bases and extension methods, and the file's usings. Under a Razor host
/// the top-level type is the wrapper, which is the component and always a part of it.
pub(crate) fn read(facts: &Facts, root: Node, src: &[u8], host: &Host) -> Declared {
    let mut d = Declared::default();
    usings(root, src, &mut d);
    let scoped = file_scoped(root, src);
    let mut types: HashMap<usize, usize> = HashMap::new();
    // Definitions come first and then the repeats of their ids, so a part is known before the
    // members and the second part that join it.
    for (range, i) in facts.sites() {
        let def = &facts.defs[i];
        let Some(n) = package_scope::declared::node_at(root, range, &DECLARATIONS) else { continue };
        if def.kind.join().is_none() {
            if let Some(t) = def.parent.and_then(|o| types.get(&o)) {
                member(n, &def.name, src, &mut d.types[*t]);
            }
            continue;
        }
        let namespace = match def.parent {
            None => namespace(n, &scoped, host.namespace, src),
            Some(owner) => types.get(&owner).map(|t| d.types[*t].namespace.clone()).unwrap_or_default(),
        };
        let part = declare_type(n, &def.qualified, namespace, def.parent.is_none() && host.component.is_some(), host, src);
        match types.get(&i).map(|t| &mut d.types[*t]) {
            // A second part in the file and namespace of the first shares its id, and so its entry.
            Some(first) if first.namespace == part.namespace => {
                first.partial |= part.partial;
                first.bases.extend(part.bases);
                first.generic_bases.extend(part.generic_bases);
            }
            Some(_) => d.types.push(part),
            None => {
                types.insert(i, d.types.len());
                d.types.push(part);
            }
        }
    }
    d
}

fn declare_type(n: Node, local: &str, namespace: String, wrapper: bool, host: &Host, src: &[u8]) -> TypeDecl {
    // Razor compiles every component to a partial class, so a `.razor.cs` part always joins it.
    let partial = wrapper || modifiers(n, src).iter().any(|m| m == "partial");
    let mut t = TypeDecl { namespace, local: local.to_string(), partial, ..TypeDecl::default() };
    if wrapper {
        for (member, ty) in host.injected {
            t.members.insert(member.clone(), Some(ty.clone()));
        }
    }
    for b in named(n).into_iter().filter(|c| c.kind() == "base_list").flat_map(named) {
        let b = if b.kind() == "primary_constructor_base_type" { b.child_by_field_name("type") } else { Some(b) };
        if let Some(b) = b.filter(|b| b.kind() != "argument_list") {
            if generic(b) {
                t.generic_bases.insert(dotted(b, src));
            }
            t.bases.push(dotted(b, src));
        }
    }
    t
}

fn member(m: Node, name: &str, src: &[u8], t: &mut TypeDecl) {
    let written = match m.kind() {
        "method_declaration" | "constructor_declaration" => {
            let first = m.child_by_field_name("parameters").and_then(|ps| named(ps).into_iter().find(|p| p.kind() == "parameter"));
            if first.is_some_and(|p| modifiers(p, src).iter().any(|w| w == "this")) {
                t.extensions.insert(name.to_string());
            }
            t.members.entry(name.to_string()).or_insert(None);
            return;
        }
        "field_declaration" | "event_field_declaration" => named(m).into_iter().find(|c| c.kind() == "variable_declaration").and_then(|v| v.child_by_field_name("type")),
        _ => m.child_by_field_name("type"),
    };
    if plain_type(written) {
        t.plain.insert(name.to_string());
    }
    t.members.insert(name.to_string(), head(written, src));
}

/// Usings at the top of the file and of every block namespace: a using inside `namespace A { … }` is
/// read as the file's, which only widens what resolves.
fn usings(n: Node, src: &[u8], d: &mut Declared) {
    for c in unbranched(n) {
        match c.kind() {
            "using_directive" => using(c, src, d),
            "namespace_declaration" => {
                if let Some(body) = c.child_by_field_name("body") {
                    usings(body, src, d);
                }
            }
            _ => {}
        }
    }
}

fn using(u: Node, src: &[u8], d: &mut Declared) {
    let mut cur = u.walk();
    let words: Vec<&str> = u.children(&mut cur).filter(|k| !k.is_named()).map(|k| k.kind()).collect();
    let alias = u.child_by_field_name("name");
    let Some(target) = named(u).into_iter().rfind(|k| Some(*k) != alias) else { return };
    let target = dotted(target, src);
    let using = match alias {
        Some(a) => Using::Alias(text(a, src).to_string(), target),
        None if words.contains(&"static") => Using::Static(target),
        None => Using::Namespace(target),
    };
    if words.contains(&"global") {
        d.global_usings.push(using);
    } else {
        d.usings.push(using);
    }
}

/// Each file-scoped namespace with where it starts: it covers the declarations after it.
fn file_scoped(root: Node, src: &[u8]) -> Vec<(usize, String)> {
    unbranched(root)
        .into_iter()
        .filter(|c| c.kind() == "file_scoped_namespace_declaration")
        .filter_map(|c| c.child_by_field_name("name").map(|name| (c.start_byte(), dotted(name, src))))
        .collect()
}

/// The namespace a top-level type is in: the host's, then the file-scoped one before it, then the
/// block namespaces around it, outermost first.
fn namespace(n: Node, scoped: &[(usize, String)], host: &str, src: &[u8]) -> String {
    let mut blocks = Vec::new();
    let mut at = n.parent();
    while let Some(p) = at {
        if p.kind() == "namespace_declaration" {
            blocks.extend(p.child_by_field_name("name").map(|name| dotted(name, src)));
        }
        at = p.parent();
    }
    let file = scoped.iter().rfind(|(start, _)| *start < n.start_byte()).map_or("", |(_, ns)| ns.as_str());
    blocks.into_iter().rev().fold(join(host, file), |ns, b| join(&ns, &b))
}

/// `n`'s named children with every `#if`/`#elif`/`#else` branch opened in place. Which branch a
/// build compiles is the build's choice, not the file's, and the truth counts every line, so both
/// branches declare; a name both declare is one id, kept once by the dispatch's dedup.
pub(crate) fn unbranched<'t>(n: Node<'t>) -> Vec<Node<'t>> {
    named(n)
        .into_iter()
        .flat_map(|c| match c.kind() {
            "preproc_if" | "preproc_elif" | "preproc_else" => unbranched(c),
            _ => vec![c],
        })
        .collect()
}
