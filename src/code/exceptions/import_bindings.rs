//! A TypeScript name is in scope through what its file's imports bind, and an import names a file
//! by a specifier only `Resolver::resolve` places: tsconfig paths, package exports, the extension
//! and index rules. No query sees another file, so this hook places each import and re-export,
//! writes its edge with the names it binds, and resolves through those bindings and the file's own
//! top-level declarations: calls and `new`, components rendered in JSX, functions handed to a call,
//! getter reads, base classes and interfaces, and decorators.
//!
//! A call is proven or not written. Proof is an import, a top-level declaration of this file, the
//! declared type of the class field a call goes through, or the type a top-level function returns;
//! a name the file only assumes (a global, a parameter, `console`) yields no edge. What a function
//! handed to another call does with it is not proven — `rows.map(fn)` calls it, `register('T',
//! Cls)` may only keep it — and both count, because `impact` asks what breaks when the target
//! changes, and either caller does.
//!
//! A decorator is a shared `deco:<Name>` node, so every class one framework decorates is found by
//! it. A Vue component's decorator goes to the declaration the name resolves to instead: a shared
//! node would chain every decorated component through `apply_diff`'s co-declared closure.

use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use crate::code::engine::{Def, Facts, File, Sink};
use crate::code::exceptions::single_file_components::component_name;
use crate::code::imports::Resolver;
use crate::code::queries::Kind;
use crate::code::syntax::{decorated, name_of, named, spanning, text};
use crate::model::{Edge, EdgeKind, NodeKind};

pub(crate) fn resolve(resolver: &Resolver, file: &File, facts: &Facts, sink: &mut Sink) {
    bind(resolver, file, facts, None, sink);
}

/// `component` is the Vue component whose script `file` is: it owns what the file would, and its
/// decorators resolve to declarations.
pub(crate) fn bind(resolver: &Resolver, file: &File, facts: &Facts, component: Option<&str>, sink: &mut Sink) {
    let Some(tree) = file.tree else { return };
    let (rel, root, src) = (file.rel, tree.root_node(), file.src.as_bytes());
    let imported = imports(resolver, rel, root, src, sink);
    let own: BTreeSet<String> = facts.defs.iter()
        .filter(|d| d.parent.is_none() && !d.qualified.contains('.'))
        .map(|d| d.qualified.clone())
        .chain(component.and_then(|_| component_name(rel)).map(str::to_string))
        .collect();
    let decorated = Decorated { facts, rel, src, own: &own, imported: &imported, component: component.is_some() };
    classes(resolver, root, &decorated, sink);
    let scope = Scope::collect(root, rel, src, resolver, &own);
    let owners = Owners { facts, rel, root, src, classes: facts.defs.iter().filter(|d| d.parent.is_none() && d.kind == Kind::Class).map(|d| (d.range.clone(), d.qualified.as_str())).collect() };
    calls(resolver, &scope, &owners, root, src, component, sink);
}

/// A destructuring binds several names to one value, so what computes the value is the file's, not
/// the first name's. Its declarator starts with the pattern's bracket, as no other top-level
/// definition does.
fn destructures(src: &[u8], def: &Def) -> bool {
    def.parent.is_none() && matches!(src.get(def.range.start), Some(b'{' | b'['))
}

/// What a site inside the file belongs to: the innermost definition holding it, and for a decorator
/// in a class body, the member it decorates, which follows it there as a sibling.
struct Owners<'a, 't> {
    facts: &'a Facts,
    rel: &'a str,
    root: Node<'t>,
    src: &'a [u8],
    classes: Vec<(std::ops::Range<usize>, &'a str)>,
}

impl Owners<'_, '_> {
    fn of(&self, n: Node) -> String {
        let byte = match decorated(self.root, n.start_byte()) {
            Some(("class_body", member)) => member,
            _ => n.start_byte(),
        };
        let owner = self.facts.owner(byte).filter(|&i| !destructures(self.src, &self.facts.defs[i]));
        self.facts.source_id(self.rel, owner)
    }

    /// The class `this` names at `n`. Outside a class, an owner with a member is a method of a
    /// top-level object literal, and `this` there is the literal: `this.find()` is a call of its
    /// sibling `repo.find`.
    fn this_class(&self, n: Node, from: &str) -> Option<String> {
        self.class_of(n).map(str::to_string).or_else(|| {
            if !binds_this(n) {
                return None;
            }
            let name = from.strip_prefix("sym:")?.split_once("::")?.1;
            name.split_once('.').map(|(literal, _)| literal.to_string())
        })
    }

    fn class_of(&self, n: Node) -> Option<&str> {
        self.classes.iter().find(|(r, _)| r.contains(&n.start_byte())).map(|(_, c)| *c)
    }
}

fn unquote(s: &str) -> String {
    s.trim_matches(|c| c == '\'' || c == '"' || c == '`').to_string()
}

/// Each import's `Imports` edge, its context the names it binds as their file declares them, and
/// each re-export's `ReExports` edge, its context the names it passes on. The returned pairs are
/// the imported names with their files.
fn imports(resolver: &Resolver, rel: &str, root: Node, src: &[u8], sink: &mut Sink) -> Vec<(String, String)> {
    let file_id = format!("file:{rel}");
    let mut bound = Vec::new();
    let mut renames: Vec<String> = Vec::new();
    for stmt in named(root) {
        match stmt.kind() {
            "import_statement" => {
                // `import x = require('y')` keeps its source on the clause, not the statement.
                let require = named(stmt).into_iter().find(|c| c.kind() == "import_require_clause");
                let Some(source) = stmt.child_by_field_name("source").or_else(|| require?.child_by_field_name("source")) else { continue };
                let Some(target) = resolver.resolve(rel, &unquote(text(source, src))) else { continue };
                let mut names = Vec::new();
                for c in named(stmt) {
                    if c.kind() == "import_require_clause" {
                        names.extend(c.named_child(0).map(|n| text(n, src).to_string()));
                        continue;
                    }
                    if c.kind() != "import_clause" {
                        continue;
                    }
                    for part in named(c) {
                        match part.kind() {
                            // A `.vue` default import names the component, the stem `importers` matches.
                            "identifier" => names.push(component_name(&target).unwrap_or(text(part, src)).to_string()),
                            "named_imports" => names.extend(named(part).into_iter().filter(|s| s.kind() == "import_specifier").filter_map(|s| name_of(s, src))),
                            "namespace_import" => names.push("*".into()),
                            _ => {}
                        }
                    }
                }
                sink.edge(&file_id, &format!("file:{target}"), EdgeKind::Imports, &names.join(","), rel);
                bound.extend(names.into_iter().map(|n| (n, target.clone())));
            }
            "export_statement" => match stmt.child_by_field_name("source") {
                // `export * from` / `export { a, b } from`: a barrel edge, not a symbol.
                Some(source) => {
                    let Some(target) = resolver.resolve(rel, &unquote(text(source, src))) else { continue };
                    let mut names = Vec::new();
                    for c in named(stmt) {
                        match c.kind() {
                            "export_clause" => names.extend(named(c).into_iter().filter(|s| s.kind() == "export_specifier").filter_map(|s| specifier(s, src))),
                            "namespace_export" => names.push(c.named_child(0).map(|a| text(a, src)).unwrap_or("*").to_string()),
                            _ => {}
                        }
                    }
                    if names.is_empty() {
                        names.push("*".into());
                    }
                    sink.edge(&file_id, &format!("file:{target}"), EdgeKind::ReExports, &names.join(","), rel);
                }
                None if stmt.child_by_field_name("declaration").is_none() => {
                    for c in named(stmt).into_iter().filter(|c| c.kind() == "export_clause") {
                        renames.extend(named(c).into_iter().filter(|s| s.kind() == "export_specifier").filter_map(|s| specifier(s, src)).filter(|e| e.contains(" as ")));
                    }
                }
                None => {}
            },
            _ => {}
        }
    }
    // The file re-exports itself under the new name, so a caller importing the alias is walked
    // back to the declaration the way a barrel's is.
    if !renames.is_empty() {
        sink.edge(&file_id, &file_id, EdgeKind::ReExports, &renames.join(","), rel);
    }
    bound
}

/// An `export_specifier` as a re-export's context entry: `a`, or `a as b` when it renames.
fn specifier(n: Node, src: &[u8]) -> Option<String> {
    let name = name_of(n, src)?;
    Some(match n.child_by_field_name("alias").map(|a| text(a, src)) {
        Some(alias) if alias != name => format!("{name} as {alias}"),
        _ => name,
    })
}

struct Decorated<'a> {
    facts: &'a Facts,
    rel: &'a str,
    src: &'a [u8],
    own: &'a BTreeSet<String>,
    imported: &'a [(String, String)],
    component: bool,
}

impl Decorated<'_> {
    /// `id` decorated by each decorator among `decorators`, the node, if any, at `line`.
    fn write<'t>(&self, id: &str, decorators: impl IntoIterator<Item = Node<'t>>, line: u32, sink: &mut Sink) {
        for d in decorators {
            let (name, arg) = decorator_pair(d, self.src);
            if !self.component {
                sink.node(NodeKind::Symbol, &format!("deco:{name}"), &name, "", self.rel, (line, 0));
                sink.edge(id, &format!("deco:{name}"), EdgeKind::DecoratedBy, &arg, self.rel);
                continue;
            }
            let target = match self.own.contains(&name) {
                true => Some(format!("sym:{}::{name}", self.rel)),
                false => self.imported.iter().find(|(n, _)| *n == name).map(|(_, f)| format!("sym:{f}::{name}")),
            };
            if let Some(target) = target {
                sink.edge(id, &target, EdgeKind::DecoratedBy, &arg, self.rel);
            }
        }
    }
}

/// A decorator's callee and its first string argument, `""` for a bare decorator or one with none.
fn decorator_pair(deco: Node, src: &[u8]) -> (String, String) {
    let Some(inner) = deco.named_child(0) else { return (String::new(), String::new()) };
    if inner.kind() != "call_expression" {
        return (text(inner, src).to_string(), String::new());
    }
    let name = inner.child_by_field_name("function").map(|f| text(f, src)).unwrap_or("").to_string();
    let arg = inner.child_by_field_name("arguments")
        .and_then(|args| named(args).into_iter().find(|a| matches!(a.kind(), "string" | "template_string")))
        .map(|a| unquote(text(a, src)))
        .unwrap_or_default();
    (name, arg)
}

/// Each class's supertypes and decorators, and its members' decorators, in file order: the node a
/// shared decorator keeps is the first one written.
fn classes(resolver: &Resolver, root: Node, d: &Decorated, sink: &mut Sink) {
    let facts = d.facts;
    for def in facts.defs.iter().filter(|def| def.parent.is_none() && def.kind == Kind::Class) {
        let Some(class) = spanning(root, &def.range) else { continue };
        for h in named(class).into_iter().filter(|c| c.kind() == "class_heritage").flat_map(named) {
            let (kind, types) = match h.kind() {
                "extends_clause" => (EdgeKind::Extends, h.child_by_field_name("value").into_iter().collect()),
                "implements_clause" => (EdgeKind::Implements, named(h)),
                _ => continue,
            };
            for t in types {
                let t = if t.kind() == "generic_type" { t.child_by_field_name("name").unwrap_or(t) } else { t };
                let base = text(t, d.src).trim();
                let file = import_origin(resolver, root, base, d.rel, d.src).unwrap_or_else(|| d.rel.to_string());
                sink.edge(&def.id, &format!("sym:{file}::{base}"), kind, "", d.rel);
            }
        }
        // A decorator on a method sits in the class body before it: it is queued for the next
        // member, and anything else in between breaks the chain. A field holds its own.
        let mut pending = Vec::new();
        for m in class.child_by_field_name("body").map(named).unwrap_or_default() {
            if m.kind() == "decorator" {
                pending.push(m);
                continue;
            }
            let member = matches!(m.kind(), "method_definition" | "public_field_definition" | "abstract_method_signature")
                .then(|| facts.declared_at(&m.byte_range()))
                .flatten();
            let Some(member) = member else {
                pending.clear();
                continue;
            };
            let own = named(m).into_iter().filter(|c| c.kind() == "decorator");
            d.write(&facts.defs[member].id, own.chain(pending.drain(..)), m.start_position().row as u32 + 1, sink);
        }
        let named_row = class.child_by_field_name("name").unwrap_or(class).start_position().row as u32 + 1;
        d.write(&def.id, named(class).into_iter().filter(|c| c.kind() == "decorator"), named_row, sink);
        // A decorator before `export class` is the export statement's.
        if let Some(stmt) = class.parent().filter(|p| p.kind() == "export_statement") {
            d.write(&def.id, named(stmt).into_iter().filter(|c| c.kind() == "decorator"), stmt.start_position().row as u32 + 1, sink);
        }
    }
}

/// The file an identifier was imported from, if it was imported at all.
fn import_origin(resolver: &Resolver, root: Node, ident: &str, rel: &str, src: &[u8]) -> Option<String> {
    let stmt = named(root).into_iter()
        .filter(|s| s.kind() == "import_statement")
        .find(|s| text(*s, src).split(|c: char| !c.is_alphanumeric() && c != '_').any(|t| t == ident))?;
    resolver.resolve(rel, &unquote(text(stmt.child_by_field_name("source")?, src)))
}

/// What one file lets a call resolve to, gathered in one pass over its top-level statements.
#[derive(Default)]
struct Scope {
    /// local binding -> (file that declares it, the name it is declared under there); this file and
    /// the same name for its own top-level declarations, `import { a as b }` maps `b` to `a`
    names: BTreeMap<String, (String, String)>,
    /// `import * as ns` binding -> file
    namespaces: BTreeMap<String, String>,
    /// class name -> field name -> declared type name, from typed fields and constructor parameter
    /// properties — the NestJS injection shape `this.service.create()` goes through
    fields: BTreeMap<String, BTreeMap<String, String>>,
    /// top-level function -> the class it returns, from `(): T` or a body that is `new T(…)` — the
    /// test-helper shape `service().method()` goes through
    returns: BTreeMap<String, String>,
    /// `Class.member` for every `get`/`set` accessor of this file's top-level classes
    accessors: BTreeSet<String>,
}

/// `T` of `: T` or `: T<…>`; other annotations (unions, literals, arrays) name no class.
fn type_name(annotation: Node, src: &[u8]) -> Option<String> {
    let t = annotation.named_child(0)?;
    match t.kind() {
        "type_identifier" => Some(text(t, src).to_string()),
        "generic_type" => t.child_by_field_name("name").map(|n| text(n, src).to_string()),
        _ => None,
    }
}

/// Whether a function that binds its own `this` encloses `n`. An arrow does not: `find: () =>
/// this.b()` reads the module's `this`, not the literal the arrow is a property of.
fn binds_this(n: Node) -> bool {
    std::iter::successors(n.parent(), |p| p.parent())
        .any(|p| matches!(p.kind(), "method_definition" | "function_expression" | "generator_function" | "function_declaration"))
}

/// The `get`/`set` keyword is an unnamed child; a method named `get` has it as its name instead.
fn is_accessor(m: Node) -> bool {
    let mut c = m.walk();
    let found = m.children(&mut c).any(|k| matches!(k.kind(), "get" | "set"));
    found
}

/// A class member's name: `'quoted'()` and `['computed']()` lose their quotes, a computed name that
/// is not a string literal (`[Symbol.iterator]`) names nothing.
fn member_name(m: Node, src: &[u8]) -> Option<String> {
    let name = m.child_by_field_name("name")?;
    match name.kind() {
        "string" => Some(unquote(text(name, src))),
        "computed_property_name" => name.named_child(0).filter(|i| i.kind() == "string").map(|i| unquote(text(i, src))),
        _ => Some(text(name, src).to_string()),
    }
}

/// The identifiers a call argument hands over: the argument itself, and every identifier in a
/// property value or array element of it, however deep — `providers: [X]`, `useFactory: mk`,
/// `{ type: Dto }`, the shorthand `{ imports }`. A function or call inside is not looked into: what
/// it calls is its own call site.
fn handed_over<'t>(arg: Node<'t>, out: &mut Vec<Node<'t>>) {
    match arg.kind() {
        "identifier" | "shorthand_property_identifier" => out.push(arg),
        "pair" => {
            if let Some(v) = arg.child_by_field_name("value") {
                handed_over(v, out)
            }
        }
        "object" | "array" => {
            for x in named(arg) {
                handed_over(x, out);
            }
        }
        _ => {}
    }
}

/// A member read that is neither called nor assigned to: `this.secret`, `this.tokens.secret`.
fn is_read(n: Node) -> bool {
    let Some(p) = n.parent() else { return true };
    let callee = match p.kind() {
        "call_expression" => p.child_by_field_name("function"),
        "new_expression" => p.child_by_field_name("constructor"),
        "assignment_expression" => p.child_by_field_name("left"),
        "jsx_opening_element" | "jsx_self_closing_element" | "jsx_closing_element" => p.child_by_field_name("name"),
        _ => None,
    };
    callee != Some(n)
}

impl Scope {
    fn collect(root: Node, rel: &str, src: &[u8], resolver: &Resolver, locals: &BTreeSet<String>) -> Scope {
        let mut s = Scope::default();
        for name in locals {
            s.names.insert(name.clone(), (rel.to_string(), name.clone()));
        }
        for stmt in named(root) {
            match stmt.kind() {
                "import_statement" => s.import(stmt, rel, src, resolver),
                "export_statement" => {
                    if let Some(decl) = stmt.child_by_field_name("declaration") {
                        s.class(decl, src);
                        s.returns(decl, src);
                    }
                }
                _ => {
                    s.class(stmt, src);
                    s.returns(stmt, src);
                }
            }
        }
        s
    }

    fn import(&mut self, stmt: Node, rel: &str, src: &[u8], resolver: &Resolver) {
        let Some(source) = stmt.child_by_field_name("source") else { return };
        let Some(target) = resolver.resolve(rel, &unquote(text(source, src))) else { return };
        for part in named(stmt).into_iter().filter(|c| c.kind() == "import_clause").flat_map(named) {
            match part.kind() {
                // A default import binds whatever name the importer chose; the declaring file's
                // symbol is a best guess under that name, and a `.vue` one is the component.
                "identifier" => {
                    let n = text(part, src).to_string();
                    let declared = component_name(&target).map_or_else(|| n.clone(), str::to_string);
                    self.names.insert(n, (target.clone(), declared));
                }
                "namespace_import" => {
                    if let Some(id) = part.named_child(0) {
                        self.namespaces.insert(text(id, src).to_string(), target.clone());
                    }
                }
                "named_imports" => {
                    for spec in named(part).into_iter().filter(|s| s.kind() == "import_specifier") {
                        let Some(name) = spec.child_by_field_name("name") else { continue };
                        let local = spec.child_by_field_name("alias").unwrap_or(name);
                        self.names.insert(text(local, src).to_string(), (target.clone(), text(name, src).to_string()));
                    }
                }
                _ => {}
            }
        }
    }

    fn class(&mut self, decl: Node, src: &[u8]) {
        if !matches!(decl.kind(), "class_declaration" | "abstract_class_declaration") {
            return;
        }
        let Some(name) = decl.child_by_field_name("name").map(|n| text(n, src).to_string()) else { return };
        let Some(body) = decl.child_by_field_name("body") else { return };
        let fields = self.fields.entry(name.clone()).or_default();
        for m in named(body) {
            if m.kind() == "method_definition" && is_accessor(m) {
                if let Some(member) = member_name(m, src) {
                    self.accessors.insert(format!("{name}.{member}"));
                }
            }
            match m.kind() {
                "public_field_definition" => {
                    if let (Some(n), Some(t)) = (m.child_by_field_name("name"), m.child_by_field_name("type").and_then(|t| type_name(t, src))) {
                        fields.insert(text(n, src).to_string(), t);
                    }
                }
                "method_definition" if m.child_by_field_name("name").is_some_and(|n| text(n, src) == "constructor") => {
                    let Some(params) = m.child_by_field_name("parameters") else { continue };
                    for p in named(params) {
                        // Only a parameter with an accessibility modifier becomes a field.
                        if !named(p).iter().any(|k| k.kind() == "accessibility_modifier") {
                            continue;
                        }
                        let pattern = p.child_by_field_name("pattern").filter(|x| x.kind() == "identifier");
                        let ty = p.child_by_field_name("type").and_then(|t| type_name(t, src));
                        if let (Some(n), Some(t)) = (pattern, ty) {
                            fields.insert(text(n, src).to_string(), t);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn returns(&mut self, decl: Node, src: &[u8]) {
        let mut record = |name: Node, f: Node| {
            let annotated = f.child_by_field_name("return_type").and_then(|t| type_name(t, src));
            let built = || {
                let body = f.child_by_field_name("body").filter(|b| b.kind() == "new_expression")?;
                body.child_by_field_name("constructor").filter(|c| c.kind() == "identifier").map(|c| text(c, src).to_string())
            };
            if let Some(t) = annotated.or_else(built) {
                self.returns.insert(text(name, src).to_string(), t);
            }
        };
        match decl.kind() {
            "function_declaration" => {
                if let Some(n) = decl.child_by_field_name("name") {
                    record(n, decl)
                }
            }
            "lexical_declaration" | "variable_declaration" => {
                for d in named(decl).into_iter().filter(|d| d.kind() == "variable_declarator") {
                    let name = d.child_by_field_name("name").filter(|n| n.kind() == "identifier");
                    let value = d.child_by_field_name("value").filter(|v| matches!(v.kind(), "arrow_function" | "function_expression"));
                    if let (Some(n), Some(v)) = (name, value) {
                        record(n, v);
                    }
                }
            }
            _ => {}
        }
    }

    fn target(&self, callee: Node, class: Option<&str>, rel: &str, src: &[u8]) -> Option<String> {
        match callee.kind() {
            "identifier" | "shorthand_property_identifier" => {
                let (f, n) = self.names.get(text(callee, src))?;
                Some(format!("sym:{f}::{n}"))
            }
            "member_expression" => {
                let obj = callee.child_by_field_name("object")?;
                let prop = text(callee.child_by_field_name("property")?, src);
                match obj.kind() {
                    "this" => Some(format!("sym:{rel}::{}.{prop}", class?)),
                    "identifier" => {
                        let n = text(obj, src);
                        if let Some(f) = self.namespaces.get(n) {
                            return Some(format!("sym:{f}::{prop}"));
                        }
                        let (f, orig) = self.names.get(n)?;
                        Some(format!("sym:{f}::{orig}.{prop}"))
                    }
                    "call_expression" => {
                        let f = obj.child_by_field_name("function").filter(|f| f.kind() == "identifier")?;
                        let ty = self.returns.get(text(f, src))?;
                        let (file, t) = self.names.get(ty)?;
                        Some(format!("sym:{file}::{t}.{prop}"))
                    }
                    "member_expression" => {
                        let inner = obj.child_by_field_name("object")?;
                        if inner.kind() != "this" {
                            return None;
                        }
                        let field = text(obj.child_by_field_name("property")?, src);
                        let ty = self.fields.get(class?)?.get(field)?;
                        let (f, t) = self.names.get(ty)?;
                        Some(format!("sym:{f}::{t}.{prop}"))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// How a site reaches its target, strongest first: one edge per (owner, target) keeps `impact`'s
/// counts, and the strongest claim any site makes is the one it carries.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Reach {
    Call,
    Read,
    Pass,
}

impl Reach {
    fn context(self) -> &'static str {
        match self {
            Reach::Call => "",
            Reach::Read => Edge::READ,
            Reach::Pass => Edge::PASSED,
        }
    }
}

/// Every call, `new` and rendered component as an edge from its owner to what the scope proves it
/// reaches, a self-call dropped and repeats collapsed; and `import('./x')` and `require('./x')`
/// anywhere, from the symbol they sit in, so a lazy route or a test's fixture load still reaches
/// its file.
fn calls(resolver: &Resolver, scope: &Scope, owners: &Owners, root: Node, src: &[u8], component: Option<&str>, sink: &mut Sink) {
    let rel = owners.rel;
    let mut found: BTreeMap<(String, String), Reach> = BTreeMap::new();
    let mut loaded: BTreeSet<(String, String)> = BTreeSet::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        stack.extend(named(n));
        let callee = match n.kind() {
            "call_expression" => n.child_by_field_name("function"),
            "new_expression" => n.child_by_field_name("constructor"),
            // Rendering a component calls it. A lowercase tag is an intrinsic element, as React
            // reads it, even when a binding of that name is in scope.
            "jsx_opening_element" | "jsx_self_closing_element" => n.child_by_field_name("name")
                .filter(|c| c.kind() != "identifier" || text(*c, src).starts_with(|ch: char| ch.is_ascii_uppercase())),
            "member_expression" if is_read(n) => {
                read(scope, owners, n, src, &mut found);
                None
            }
            _ => None,
        };
        let Some(callee) = callee else { continue };
        if n.kind() == "call_expression" && (callee.kind() == "import" || text(callee, src) == "require") {
            let path = n.child_by_field_name("arguments").and_then(|a| a.named_child(0)).filter(|a| a.kind() == "string");
            if let Some(target) = path.and_then(|p| resolver.resolve(rel, &unquote(text(p, src)))) {
                loaded.insert((owners.of(n), format!("file:{target}")));
            }
        }
        let from = owners.of(n);
        let class = owners.this_class(n, &from);
        // A function handed to another — `rows.map(feedWire)`, `.filter(isIndexedType)` — is
        // called on the caller's behalf, and a change to it breaks the caller all the same; the
        // edge says `arg`, because a constant or a DI token handed over the same way is not.
        let mut passed = Vec::new();
        for a in n.child_by_field_name("arguments").map(named).unwrap_or_default() {
            handed_over(a, &mut passed);
        }
        let sites = std::iter::once((callee, Reach::Call)).chain(passed.into_iter().map(|x| (x, Reach::Pass)));
        for (x, how) in sites {
            let Some(target) = scope.target(x, class.as_deref(), rel, src) else { continue };
            note(&mut found, &from, target, how);
        }
    }
    for (from, target) in loaded {
        sink.edge(&from, &target, EdgeKind::Imports, "", rel);
    }
    let file_id = format!("file:{rel}");
    for ((from, target), how) in found {
        // The component spans its file, so what the script leaves on the file is the component's.
        let from = match component {
            Some(c) if from == file_id && target != c => c,
            _ => from.as_str(),
        };
        sink.edge(from, &target, EdgeKind::Calls, how.context(), rel);
    }
}

fn note(found: &mut BTreeMap<(String, String), Reach>, from: &str, target: String, how: Reach) {
    if from == target {
        return;
    }
    let slot = found.entry((from.to_string(), target)).or_insert(how);
    *slot = (*slot).min(how);
}

/// A member read that resolves to an accessor. One of this file's own classes is checked here; any
/// other member is a candidate for `settle`, while a bare export (`ns.x`) never is one. An object
/// literal's `this` is never one of the classes whose accessors are known.
fn read(scope: &Scope, owners: &Owners, n: Node, src: &[u8], found: &mut BTreeMap<(String, String), Reach>) {
    let rel = owners.rel;
    let Some(obj) = n.child_by_field_name("object") else { return };
    let through_this = obj.kind() == "this" || obj.child_by_field_name("object").is_some_and(|o| o.kind() == "this");
    let class = if through_this { owners.class_of(n) } else { None };
    let Some(target) = scope.target(n, class, rel, src) else { return };
    let Some((file, name)) = target.strip_prefix("sym:").and_then(|t| t.split_once("::")) else { return };
    if !name.contains('.') {
        return;
    }
    if file == rel && !scope.accessors.contains(name) {
        return;
    }
    note(found, &owners.of(n), target, Reach::Read);
}
