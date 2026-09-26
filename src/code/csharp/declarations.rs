//! Symbols and `Declares` for a C# file, and the facts the .NET index keeps about it.

use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use super::{dotted, generic, head, modifiers, named, span, text, Host};
use crate::code::index::Header;
use crate::model::{EdgeKind, Extraction, NodeKind};

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

fn is_type(kind: &str) -> bool {
    matches!(kind, "class_declaration" | "struct_declaration" | "interface_declaration" | "record_declaration" | "enum_declaration" | "delegate_declaration")
}

/// `Status` or `Shop.Orders.Status`: a name, dotted or not, with no type arguments anywhere.
fn plain_type(t: Option<Node>) -> bool {
    t.is_some_and(|t| match t.kind() {
        "identifier" => true,
        "qualified_name" => ["qualifier", "name"].iter().all(|f| plain_type(t.child_by_field_name(f))),
        _ => false,
    })
}

fn name_of(n: Node, src: &[u8]) -> Option<String> {
    n.child_by_field_name("name").map(|x| text(x, src).to_string())
}

/// C#'s defaults as the spec reads them: `public`, `internal` and `protected internal` cross files;
/// a top-level type with no modifier is `internal`; a member with no modifier is `private`, except in
/// an interface, whose members are public. `private protected` and `file` stay in their file.
fn exported(mods: &[String], top_level: bool, in_interface: bool) -> bool {
    if mods.iter().any(|m| m == "private" || m == "file") {
        return false;
    }
    if mods.iter().any(|m| m == "public" || m == "internal") {
        return true;
    }
    if mods.iter().any(|m| m == "protected") {
        return false;
    }
    top_level || in_interface
}

/// A declaration's body: the comment lines directly above it, then its first line that is not an
/// attribute list — the signature a reader recognises it by.
fn signature(n: Node, src: &[u8]) -> String {
    let line = text(n, src).lines().map(str::trim).find(|l| !l.starts_with('[')).unwrap_or("").to_string();
    let mut doc = Vec::new();
    let mut below = n.start_position().row;
    let mut cur = n.prev_named_sibling();
    while let Some(c) = cur.filter(|c| c.kind() == "comment" && c.end_position().row + 1 >= below) {
        doc.push(text(c, src).trim_start_matches('/').trim_start_matches('*').trim().to_string());
        below = c.start_position().row;
        cur = c.prev_named_sibling();
    }
    doc.reverse();
    let doc: String = doc.join("\n").chars().take(600).collect();
    if doc.is_empty() { line } else { format!("{doc}\n{line}") }
}

pub fn scan(root: Node, rel: &str, src: &[u8], host: &Host, ex: &mut Extraction) -> Declared {
    let mut w = Walk { rel, src, host, d: Declared::default() };
    w.container(root, &format!("file:{rel}"), host.namespace.to_string(), ex);
    w.d
}

struct Walk<'a> {
    rel: &'a str,
    src: &'a [u8],
    host: &'a Host<'a>,
    d: Declared,
}

impl Walk<'_> {
    fn container(&mut self, n: Node, parent: &str, namespace: String, ex: &mut Extraction) {
        let mut namespace = namespace;
        for c in unbranched(n) {
            match c.kind() {
                "using_directive" => self.using(c),
                // A file-scoped namespace covers the declarations after it, which are its siblings.
                "file_scoped_namespace_declaration" => {
                    if let Some(name) = c.child_by_field_name("name") {
                        namespace = join(&namespace, &dotted(name, self.src));
                    }
                }
                "namespace_declaration" => {
                    let name = c.child_by_field_name("name").map(|x| dotted(x, self.src)).unwrap_or_default();
                    if let Some(body) = c.child_by_field_name("body") {
                        self.container(body, parent, join(&namespace, &name), ex);
                    }
                }
                k if is_type(k) => self.declare_type(c, parent, &namespace, None, ex),
                _ => {}
            }
        }
    }

    fn using(&mut self, u: Node) {
        let mut cur = u.walk();
        let words: Vec<&str> = u.children(&mut cur).filter(|k| !k.is_named()).map(|k| k.kind()).collect();
        let alias = u.child_by_field_name("name");
        let Some(target) = named(u).into_iter().rfind(|k| Some(*k) != alias) else { return };
        let target = dotted(target, self.src);
        let using = match alias {
            Some(a) => Using::Alias(text(a, self.src).to_string(), target),
            None if words.contains(&"static") => Using::Static(target),
            None => Using::Namespace(target),
        };
        if words.contains(&"global") {
            self.d.global_usings.push(using);
        } else {
            self.d.usings.push(using);
        }
    }

    /// `owner` is the containing type's local name and whether it is an interface.
    fn declare_type(&mut self, n: Node, parent: &str, namespace: &str, owner: Option<(&str, bool)>, ex: &mut Extraction) {
        let Some(written) = name_of(n, self.src) else { return };
        // The Razor wrapper is the blanked copy's only top-level type, and it is the component.
        let wrapper = owner.is_none() && self.host.component.is_some();
        let name = match (wrapper, self.host.component) {
            (true, Some(c)) => c.to_string(),
            _ => written,
        };
        let local = owner.map_or_else(|| name.clone(), |(o, _)| format!("{o}.{name}"));
        let id = format!("sym:{}::{local}", self.rel);
        let mods = modifiers(n, self.src);
        if !wrapper {
            let ctx = if exported(&mods, owner.is_none(), owner.is_some_and(|(_, i)| i)) { "export" } else { "" };
            ex.node_span(NodeKind::Symbol, &id, &local, &signature(n, self.src), self.rel, span(n));
            ex.edge(parent, &id, EdgeKind::Declares, ctx, self.rel);
        }
        // Razor compiles every component to a partial class, so a `.razor.cs` part always joins it.
        let mut t = TypeDecl { namespace: namespace.to_string(), local: local.clone(), partial: wrapper || mods.iter().any(|m| m == "partial"), ..TypeDecl::default() };
        if wrapper {
            for (member, ty) in self.host.injected {
                t.members.insert(member.clone(), Some(ty.clone()));
            }
        }
        for c in named(n) {
            match c.kind() {
                "base_list" => {
                    for b in named(c) {
                        let b = if b.kind() == "primary_constructor_base_type" { b.child_by_field_name("type") } else { Some(b) };
                        if let Some(b) = b.filter(|b| b.kind() != "argument_list") {
                            if generic(b) {
                                t.generic_bases.insert(dotted(b, self.src));
                            }
                            t.bases.push(dotted(b, self.src));
                        }
                    }
                }
                // A record's positional parameters are public properties; a class's are private captures.
                // A delegate's parameters are its signature, not members.
                "parameter_list" if n.kind() != "delegate_declaration" => {
                    let ctx = if n.kind() == "record_declaration" { "export" } else { "" };
                    for p in named(c).into_iter().filter(|p| p.kind() == "parameter") {
                        let Some(pname) = name_of(p, self.src) else { continue };
                        self.write_member(p, &id, &local, &pname, ctx, ex);
                        if plain_type(p.child_by_field_name("type")) {
                            t.plain.insert(pname.clone());
                        }
                        t.members.insert(pname, head(p.child_by_field_name("type"), self.src));
                    }
                }
                _ => {}
            }
        }
        let interface = n.kind() == "interface_declaration";
        if let Some(body) = n.child_by_field_name("body").filter(|b| b.kind() == "declaration_list") {
            for m in unbranched(body) {
                if is_type(m.kind()) {
                    self.declare_type(m, &id, namespace, Some((&local, interface)), ex);
                } else {
                    self.member(m, &id, &local, interface, &mut t, ex);
                }
            }
        }
        self.d.types.push(t);
    }

    fn member(&mut self, m: Node, id: &str, local: &str, interface: bool, t: &mut TypeDecl, ex: &mut Extraction) {
        let mods = modifiers(m, self.src);
        let ctx = if exported(&mods, false, interface) { "export" } else { "" };
        match m.kind() {
            "method_declaration" | "constructor_declaration" => {
                let Some(name) = name_of(m, self.src) else { return };
                let first = m.child_by_field_name("parameters").and_then(|ps| named(ps).into_iter().find(|p| p.kind() == "parameter"));
                if first.is_some_and(|p| modifiers(p, self.src).iter().any(|w| w == "this")) {
                    t.extensions.insert(name.clone());
                }
                self.write_member(m, id, local, &name, ctx, ex);
                t.members.entry(name).or_insert(None);
            }
            "property_declaration" | "event_declaration" => {
                let Some(name) = name_of(m, self.src) else { return };
                self.write_member(m, id, local, &name, ctx, ex);
                if plain_type(m.child_by_field_name("type")) {
                    t.plain.insert(name.clone());
                }
                t.members.insert(name, head(m.child_by_field_name("type"), self.src));
            }
            "field_declaration" | "event_field_declaration" => {
                let Some(var) = named(m).into_iter().find(|c| c.kind() == "variable_declaration") else { return };
                let ty = head(var.child_by_field_name("type"), self.src);
                let plain = plain_type(var.child_by_field_name("type"));
                for d in named(var).into_iter().filter(|c| c.kind() == "variable_declarator") {
                    if let Some(name) = name_of(d, self.src) {
                        self.write_member(m, id, local, &name, ctx, ex);
                        if plain {
                            t.plain.insert(name.clone());
                        }
                        t.members.insert(name, ty.clone());
                    }
                }
            }
            // Destructors, operators, conversions and indexers have no name a caller writes.
            _ => {}
        }
    }

    fn write_member(&mut self, m: Node, parent: &str, local: &str, name: &str, ctx: &str, ex: &mut Extraction) {
        let id = format!("{parent}.{name}");
        ex.node_span(NodeKind::Symbol, &id, &format!("{local}.{name}"), &signature(m, self.src), self.rel, span(m));
        ex.edge(parent, &id, EdgeKind::Declares, ctx, self.rel);
    }
}

/// `n`'s named children with every `#if`/`#elif`/`#else` branch opened in place. Which branch a
/// build compiles is the build's choice, not the file's, and the truth counts every line, so both
/// branches declare; a name both declare is one id, kept once by the dispatch's dedup.
fn unbranched<'t>(n: Node<'t>) -> Vec<Node<'t>> {
    named(n)
        .into_iter()
        .flat_map(|c| match c.kind() {
            "preproc_if" | "preproc_elif" | "preproc_else" => unbranched(c),
            _ => vec![c],
        })
        .collect()
}
