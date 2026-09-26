//! The pass after declarations: every name a C# file uses, resolved through `Scope` and written as
//! the edges TypeScript writes. Proof is a declaration the index holds; a name it does not hold —
//! a framework type, a lambda parameter — writes nothing, so a caller list never contains a guess.

use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use super::declarations::{join, Declared};
use super::index::Part;
use super::resolve::Scope;
use super::{dotted, head, named, text, type_names, Host};
use crate::code::imports::Resolver;
use crate::model::{EdgeKind, Extraction};

/// Where the walk is: the namespace, the innermost type's local name, the type parameters in
/// scope (the enclosing types' and the current member's own), and the id edges leave from.
#[derive(Clone)]
struct At {
    namespace: String,
    class: Option<String>,
    owner: String,
    type_params: BTreeSet<String>,
}

/// The names a `type_parameter_list` child declares. `<T>` on `class Repo<T>` or a generic
/// method reads as a type, but it names no declaration, so a repo-declared `T` must never shadow it.
fn declared_type_params(n: Node, src: &[u8]) -> BTreeSet<String> {
    named(n).into_iter()
        .find(|c| c.kind() == "type_parameter_list")
        .into_iter()
        .flat_map(named)
        .filter(|p| p.kind() == "type_parameter")
        .filter_map(|p| p.child_by_field_name("name"))
        .map(|nm| text(nm, src).to_string())
        .collect()
}

/// Locals and parameters of the member being read, to their declared type head — `None` for a
/// known local whose type this file cannot read (`var` from a call, an implicit lambda parameter,
/// `out var`, a pattern designation), which the extension rule may still stand in for.
type Locals = BTreeMap<String, Option<String>>;

const TYPES: [&str; 6] = ["class_declaration", "struct_declaration", "interface_declaration", "record_declaration", "enum_declaration", "delegate_declaration"];
const MEMBERS: [&str; 10] = [
    "method_declaration", "constructor_declaration", "destructor_declaration", "operator_declaration",
    "conversion_operator_declaration", "indexer_declaration", "property_declaration", "event_declaration",
    "field_declaration", "event_field_declaration",
];
/// Members with no name a caller writes: what they use belongs to their type.
const NAMELESS: [&str; 4] = ["destructor_declaration", "operator_declaration", "conversion_operator_declaration", "indexer_declaration"];

pub(crate) fn first_segment(local: &str) -> &str {
    local.split('.').next().unwrap_or(local)
}

pub fn scan(root: Node, rel: &str, src: &[u8], host: &Host, own: &Declared, resolver: &Resolver, ex: &mut Extraction) {
    let reader = Reader { rel, src, host, scope: Scope::new(rel, resolver.dotnet(), own, host), file: format!("file:{rel}") };
    let at = At { namespace: host.namespace.to_string(), class: None, owner: reader.file.clone(), type_params: BTreeSet::new() };
    reader.walk(root, &at, &mut Locals::new(), ex);
}

struct Reader<'a> {
    rel: &'a str,
    src: &'a [u8],
    host: &'a Host<'a>,
    scope: Scope<'a>,
    file: String,
}

impl Reader<'_> {
    fn walk(&self, n: Node, at: &At, locals: &mut Locals, ex: &mut Extraction) {
        match n.kind() {
            "compilation_unit" => {
                let mut at = at.clone();
                for c in named(n) {
                    // A file-scoped namespace covers its following siblings.
                    if c.kind() == "file_scoped_namespace_declaration" {
                        if let Some(name) = c.child_by_field_name("name") {
                            at.namespace = join(&at.namespace, &dotted(name, self.src));
                        }
                    } else {
                        self.walk(c, &at, locals, ex);
                    }
                }
                return;
            }
            "namespace_declaration" => {
                let mut at = at.clone();
                at.namespace = join(&at.namespace, &n.child_by_field_name("name").map(|x| dotted(x, self.src)).unwrap_or_default());
                if let Some(body) = n.child_by_field_name("body") {
                    for c in named(body) {
                        self.walk(c, &at, locals, ex);
                    }
                }
                return;
            }
            // A using names a namespace or its alias; its name is never a type use.
            "using_directive" => return,
            // `[assembly: X]` / `[module: X]`: a compilation-unit-level attribute, decorating the file.
            "global_attribute" => return self.attributes(n, at, &self.file, ex),
            k if TYPES.contains(&k) => return self.type_decl(n, at, ex),
            k if MEMBERS.contains(&k) && at.class.is_some() => return self.member(n, at, ex),
            "comment" | "string_literal" | "verbatim_string_literal" | "raw_string_literal" => return self.ids(n, at, ex),
            // An interpolated string's holes are code, so the walk goes on into them; a hole is
            // scanned by whatever it recurses into, and never as raw text here, so a literal
            // inside a hole is not read twice.
            "interpolated_string_expression" => self.literal_content_ids(n, at, ex),
            "parameter" => {
                if let Some(name) = n.child_by_field_name("name") {
                    locals.insert(text(name, self.src).to_string(), head(n.child_by_field_name("type"), self.src));
                }
            }
            "variable_declaration" => self.declare_locals(n, locals),
            "declaration_pattern" | "recursive_pattern" | "declaration_expression" | "var_pattern" | "catch_declaration" => {
                if let Some(name) = n.child_by_field_name("name") {
                    locals.insert(text(name, self.src).to_string(), head(n.child_by_field_name("type"), self.src));
                }
                for d in named(n) {
                    self.designated(d, locals);
                }
            }
            "foreach_statement" => {
                match n.child_by_field_name("left") {
                    Some(left) if left.kind() == "identifier" => {
                        locals.insert(text(left, self.src).to_string(), head(n.child_by_field_name("type"), self.src));
                    }
                    Some(left) => self.designated(left, locals),
                    None => {}
                }
            }
            // A catch variable and a query's range variables are in scope for their own clause or
            // query only, as a lambda's parameters are for its body.
            "catch_clause" | "query_expression" => {
                let mut inner = locals.clone();
                if n.kind() == "query_expression" {
                    self.range_variables(n, &mut inner);
                }
                self.type_uses(n, at, ex);
                for c in named(n) {
                    self.walk(c, at, &mut inner, ex);
                }
                return;
            }
            // A lambda's own parameters are in scope for its body only; cloning locals keeps a
            // same-named field or outer local from leaking in, and keeps the parameter itself from
            // leaking out.
            "lambda_expression" => return self.scoped_body(n, n.child_by_field_name("parameters"), n.child_by_field_name("body"), at, locals, ex),
            "anonymous_method_expression" => {
                let body = named(n).into_iter().find(|c| c.kind() == "block");
                return self.scoped_body(n, n.child_by_field_name("parameters"), body, at, locals, ex);
            }
            "local_function_statement" => {
                let mut type_params = at.type_params.clone();
                type_params.extend(declared_type_params(n, self.src));
                let at = At { type_params, ..at.clone() };
                return self.scoped_body(n, n.child_by_field_name("parameters"), n.child_by_field_name("body"), &at, locals, ex);
            }
            "invocation_expression" => {
                // The grammar reads `o is var (a, b)` as a call of `o is var` with `a` and `b` as
                // arguments; those are the pattern's designations, so they are locals.
                let var_pattern = n
                    .child_by_field_name("function")
                    .filter(|f| f.kind() == "is_expression")
                    .and_then(|f| f.child_by_field_name("right"))
                    .is_some_and(|r| r.kind() == "implicit_type");
                if var_pattern {
                    let mut names = Vec::new();
                    if let Some(args) = n.child_by_field_name("arguments") {
                        argument_names(args, &mut names);
                    }
                    for name in names {
                        locals.insert(text(name, self.src).to_string(), None);
                    }
                }
                if let Some(f) = n.child_by_field_name("function") {
                    self.type_args(f, at, ex);
                    for to in self.call_targets(f, at, locals) {
                        self.call(at, &to, ex);
                    }
                }
            }
            "member_access_expression" => {
                for f in ["expression", "name"] {
                    if let Some(g) = n.child_by_field_name(f) {
                        self.type_args(g, at, ex);
                    }
                }
                self.static_access(n, at, locals, ex);
            }
            "member_binding_expression" => {
                if let Some(g) = n.child_by_field_name("name") {
                    self.type_args(g, at, ex);
                }
            }
            // `new T()` calls the type, as TypeScript's `new` does.
            "object_creation_expression" => {
                if let Some(t) = head(n.child_by_field_name("type"), self.src) {
                    // A type parameter named like a repo type is not the type it names.
                    if !at.type_params.contains(&t) {
                        for p in self.scope.types(&t, &at.namespace, at.class.as_deref()) {
                            self.call(at, &format!("sym:{}::{}", p.rel, p.local), ex);
                        }
                    }
                }
            }
            _ => {}
        }
        self.type_uses(n, at, ex);
        for c in named(n) {
            self.walk(c, at, locals, ex);
        }
    }

    /// The type a node's `type`, `returns` or `right` field names, for `Imports`.
    fn type_uses(&self, n: Node, at: &At, ex: &mut Extraction) {
        let fields: &[&str] = match n.kind() {
            "is_expression" | "as_expression" => &["right"],
            _ => &["type", "returns"],
        };
        for f in fields {
            let Some(t) = n.child_by_field_name(f) else { continue };
            let mut names = Vec::new();
            type_names(t, self.src, &mut names);
            for name in names {
                self.use_type(&name, at, ex);
            }
        }
    }

    /// `AddScoped<IGateway, Gateway>()` and `Get<Order>()`: the grammar reads what sits between
    /// the angle brackets of a generic name as type arguments, so each is a type use wherever the
    /// generic name stands. A generic name in a type position is read by `type_uses` instead.
    fn type_args(&self, g: Node, at: &At, ex: &mut Extraction) {
        if g.kind() != "generic_name" {
            return;
        }
        for list in named(g).into_iter().filter(|c| c.kind() == "type_argument_list") {
            let mut names = Vec::new();
            for a in named(list) {
                type_names(a, self.src, &mut names);
            }
            for name in names {
                // A dotted argument found only through a using could be a package's namespace path.
                if name.contains('.') && self.scope.types_imported(&name, &at.namespace, at.class.as_deref()).1 {
                    continue;
                }
                self.use_type(&name, at, ex);
            }
        }
    }

    /// `Status.Open` and `Shop.Orders.Status.Open`: a chain of plain names used as a value names
    /// the longest prefix that resolves to a type. Only the outermost access of a chain is read,
    /// so an inner prefix is never taken for the chain's type.
    fn static_access(&self, n: Node, at: &At, locals: &Locals, ex: &mut Extraction) {
        let inner = n.parent().is_some_and(|p| p.kind() == "member_access_expression" && p.child_by_field_name("expression") == Some(n));
        if inner {
            return;
        }
        let Some(segments) = plain_chain(n, self.src) else { return };
        if !self.first_names_a_type(&segments[0], at, locals) {
            return;
        }
        let (ns, class) = (at.namespace.as_str(), at.class.as_deref());
        for k in (1..segments.len()).rev() {
            let prefix = segments[..k].join(".");
            let (parts, imported) = self.scope.types_imported(&prefix, ns, class);
            if parts.is_empty() {
                continue;
            }
            // Found only through a using, `A` could be a package's namespace, which the compiler
            // finds first. `A.M(…)` is the one shape that rules it out: a namespace followed by a
            // type is never invoked, while a pattern, a value or `nameof` takes `Ns.Type` as well.
            if !imported || (segments.len() == 2 && invoked(n)) {
                self.use_type(&prefix, at, ex);
            }
            return;
        }
    }

    /// Whether the compiler's simple-name lookup of `first` can only reach a type or a namespace:
    /// a local, a parameter or a type parameter comes first, and so does any member the enclosing
    /// types declare or inherit. A member whose declared type is spelled `first` still leaves the
    /// access naming the type, which is C#'s "Color Color" rule. A base the repository does not
    /// declare, a Razor component's implicit `ComponentBase`, and a `using static` of a type the
    /// repository cannot read could each hold a member of that name, so each is no proof.
    fn first_names_a_type(&self, first: &str, at: &At, locals: &Locals) -> bool {
        if locals.contains_key(first) || at.type_params.contains(first) || self.host.component.is_some() {
            return false;
        }
        let (ns, class) = (at.namespace.as_str(), at.class.as_deref());
        let named: BTreeSet<String> = self.scope.types(first, ns, class).into_iter().map(|p| p.full).collect();
        // A member `first` leaves the access naming the type only when its declared type is that
        // very type: one name, read where the member is declared, resolving to the same type.
        let passes = |p: &Part| match self.scope.members(p).and_then(|m| m.get(first)) {
            None => true,
            Some(t) => {
                let resolved: BTreeSet<String> = t.as_deref().map(|t| self.scope.types_around(t, p)).unwrap_or_default().into_iter().map(|q| q.full).collect();
                self.scope.plain_typed(p, first) && !named.is_empty() && resolved == named
            }
        };
        for level in self.scope.enclosing(ns, class) {
            if level.first().is_some_and(|p| self.scope.is_component(&p.full)) || !level.iter().all(passes) {
                return false;
            }
            let mut shadowed = false;
            let walked = self.scope.walk_bases(&level, |bases| {
                shadowed = bases.iter().any(|p| !passes(p) || !self.scope.full(&join(&p.full, first)).is_empty());
                shadowed
            });
            if shadowed || !walked.unresolved.is_empty() {
                return false;
            }
        }
        !self.scope.static_import_could_name(first)
    }

    fn use_type(&self, name: &str, at: &At, ex: &mut Extraction) -> Vec<Part> {
        // `T` in scope names a type parameter, not a declaration; resolving it would let a
        // same-named type elsewhere in the repository stand in for it.
        if at.type_params.contains(name) {
            return Vec::new();
        }
        let parts = self.scope.types(name, &at.namespace, at.class.as_deref());
        for p in parts.iter().filter(|p| p.rel != self.rel) {
            ex.edge(&self.file, &format!("file:{}", p.rel), EdgeKind::Imports, first_segment(&p.local), self.rel);
        }
        parts
    }

    fn declare_locals(&self, v: Node, locals: &mut Locals) {
        let declared = head(v.child_by_field_name("type"), self.src);
        for d in named(v).into_iter().filter(|d| d.kind() == "variable_declarator") {
            let Some(name) = d.child_by_field_name("name") else {
                for t in named(d) {
                    self.designated(t, locals);
                }
                continue;
            };
            // `var x = new T()` and `var x = (T)y` say the type on the right.
            let inferred = named(d).into_iter().find_map(|e| match e.kind() {
                "object_creation_expression" | "cast_expression" => head(e.child_by_field_name("type"), self.src),
                _ => None,
            });
            locals.insert(text(name, self.src).to_string(), declared.clone().or(inferred));
        }
    }

    /// The names a deconstruction declares — `var (a, (b, _))`, `foreach (var (a, b) in …)`,
    /// `is var (a, b)` — whose types this file does not read.
    fn designated(&self, d: Node, locals: &mut Locals) {
        if !matches!(d.kind(), "tuple_pattern" | "parenthesized_variable_designation") {
            return;
        }
        let mut cur = d.walk();
        for name in d.children_by_field_name("name", &mut cur) {
            locals.insert(text(name, self.src).to_string(), None);
        }
        for inner in named(d) {
            self.designated(inner, locals);
        }
    }

    /// `from x in`, `join x in`, `let x =`, and `into x`, whether after a `join` or continuing the
    /// query. The grammar names only `from`'s variable, so the others are read by the keyword they
    /// sit beside.
    fn range_variables(&self, q: Node, locals: &mut Locals) {
        let mut clauses = vec![q];
        for c in named(q) {
            if c.kind() == "join_clause" {
                clauses.extend(named(c).into_iter().filter(|j| j.kind() == "join_into_clause"));
            }
            clauses.push(c);
        }
        for c in clauses {
            let mut cur = c.walk();
            let children: Vec<Node> = c.children(&mut cur).collect();
            for (i, x) in children.iter().enumerate() {
                if x.kind() != "identifier" {
                    continue;
                }
                let before = i.checked_sub(1).map(|j| children[j].kind());
                let after = children.get(i + 1).map(|y| y.kind());
                let declares = match c.kind() {
                    "from_clause" | "join_clause" => after == Some("in"),
                    "let_clause" => before == Some("let"),
                    _ => before == Some("into"),
                };
                if declares {
                    locals.insert(text(*x, self.src).to_string(), None);
                }
            }
        }
    }

    /// A lambda's, anonymous method's or local function's own parameters, scoped to its body: locals
    /// are cloned before the parameters are read, so a parameter never leaks into the surrounding
    /// member's shared map, and never shadows a same-named field or outer local outside its own body.
    fn scoped_body(&self, n: Node, params: Option<Node>, body: Option<Node>, at: &At, locals: &mut Locals, ex: &mut Extraction) {
        let mut inner = locals.clone();
        match params {
            Some(p) if p.kind() == "implicit_parameter" => {
                inner.insert(text(p, self.src).to_string(), None);
            }
            Some(p) => self.walk(p, at, &mut inner, ex),
            None => {}
        }
        if let Some(b) = body {
            self.walk(b, at, &mut inner, ex);
        }
        self.type_uses(n, at, ex);
        for c in named(n) {
            if Some(c) == params || Some(c) == body {
                continue;
            }
            self.walk(c, at, locals, ex);
        }
    }

    fn type_decl(&self, n: Node, at: &At, ex: &mut Extraction) {
        let written = n.child_by_field_name("name").map(|x| text(x, self.src).to_string()).unwrap_or_default();
        let name = match (at.class.is_none(), self.host.component) {
            (true, Some(c)) => c.to_string(),
            _ => written,
        };
        let local = at.class.as_ref().map_or_else(|| name.clone(), |c| format!("{c}.{name}"));
        let id = format!("sym:{}::{local}", self.rel);
        // A nested type reads the enclosing type's own parameters as well as its own, as C# does.
        let mut type_params = at.type_params.clone();
        type_params.extend(declared_type_params(n, self.src));
        let scoped = At { type_params: type_params.clone(), ..at.clone() };
        let inner = At { namespace: at.namespace.clone(), class: Some(local.clone()), owner: id.clone(), type_params };
        let full = join(&at.namespace, &local);
        if self.scope.own().parts(&full).iter().any(|t| t.partial) {
            for p in self.scope.full(&full).into_iter().filter(|p| p.rel != self.rel) {
                ex.edge(&self.file, &format!("file:{}", p.rel), EdgeKind::Imports, first_segment(&p.local), self.rel);
            }
        }
        let mut locals = Locals::new();
        for c in named(n) {
            match c.kind() {
                // A base list is read in the scope around the type, as the compiler reads it.
                "base_list" => {
                    for b in named(c) {
                        let b = if b.kind() == "primary_constructor_base_type" { b.child_by_field_name("type") } else { Some(b) };
                        let Some(b) = b.filter(|b| b.kind() != "argument_list") else { continue };
                        let mut names = Vec::new();
                        type_names(b, self.src, &mut names);
                        for (i, base) in names.iter().enumerate() {
                            let parts = self.use_type(base, &scoped, ex);
                            if i == 0 {
                                for p in &parts {
                                    ex.edge(&id, &format!("sym:{}::{}", p.rel, p.local), EdgeKind::Extends, "", self.rel);
                                }
                            }
                        }
                    }
                }
                "attribute_list" => self.attributes(c, &scoped, &id, ex),
                "parameter_list" | "type_parameter_constraints_clause" => self.walk(c, &inner, &mut locals, ex),
                _ => {}
            }
        }
        if n.kind() == "delegate_declaration" {
            self.type_uses(n, &scoped, ex);
        }
        if let Some(body) = n.child_by_field_name("body") {
            for m in named(body) {
                self.walk(m, &inner, &mut locals, ex);
            }
        }
    }

    fn member(&self, n: Node, at: &At, ex: &mut Extraction) {
        let class = at.class.as_deref().unwrap_or_default();
        let name = match n.kind() {
            "field_declaration" | "event_field_declaration" => named(n).into_iter()
                .find(|c| c.kind() == "variable_declaration")
                .and_then(|v| named(v).into_iter().find(|d| d.kind() == "variable_declarator"))
                .and_then(|d| d.child_by_field_name("name")),
            k if NAMELESS.contains(&k) => None,
            _ => n.child_by_field_name("name"),
        };
        let owner = name.map_or_else(|| at.owner.clone(), |m| format!("sym:{}::{class}.{}", self.rel, text(m, self.src)));
        // A generic method's own `<U>` is in scope for its return type, parameters and body.
        let mut type_params = at.type_params.clone();
        type_params.extend(declared_type_params(n, self.src));
        let inner = At { owner: owner.clone(), type_params, ..at.clone() };
        let mut locals = Locals::new();
        self.type_uses(n, &inner, ex);
        for c in named(n) {
            if c.kind() == "attribute_list" {
                self.attributes(c, &inner, &owner, ex);
            } else {
                self.walk(c, &inner, &mut locals, ex);
            }
        }
    }

    /// L8: `[Tracked]` is `TrackedAttribute` or `Tracked`, suffix first, as the compiler looks it up. An
    /// attribute the repository does not declare writes nothing, and no shared node stands for it.
    fn attributes(&self, list: Node, at: &At, id: &str, ex: &mut Extraction) {
        for a in named(list).into_iter().filter(|a| a.kind() == "attribute") {
            let Some(name) = a.child_by_field_name("name").map(|x| dotted(x, self.src)) else { continue };
            let mut parts = Vec::new();
            if !name.ends_with("Attribute") {
                parts = self.use_type(&format!("{name}Attribute"), at, ex);
            }
            if parts.is_empty() {
                parts = self.use_type(&name, at, ex);
            }
            for p in &parts {
                ex.edge(id, &format!("sym:{}::{}", p.rel, p.local), EdgeKind::DecoratedBy, "", self.rel);
            }
            let inner = At { owner: id.to_string(), ..at.clone() };
            for arg in named(a).into_iter().filter(|x| x.kind() == "attribute_argument_list") {
                self.walk(arg, &inner, &mut Locals::new(), ex);
            }
        }
    }

    fn ids(&self, n: Node, at: &At, ex: &mut Extraction) {
        let ctx = if n.kind() == "comment" { "comment" } else { "string" };
        for hit in crate::ids::generic().find_all(text(n, self.src)) {
            ex.edge(&at.owner, &hit.id, EdgeKind::References, ctx, self.rel);
        }
    }

    /// An interpolated string's own text, its holes excluded: `string_content` and the escape
    /// sequences between them. A hole's own literals are found once, by the walk's recursion into it.
    fn literal_content_ids(&self, n: Node, at: &At, ex: &mut Extraction) {
        for c in named(n).into_iter().filter(|c| c.kind() == "string_content" || c.kind() == "escape_sequence") {
            for hit in crate::ids::generic().find_all(text(c, self.src)) {
                ex.edge(&at.owner, &hit.id, EdgeKind::References, "string", self.rel);
            }
        }
    }

    fn call(&self, at: &At, to: &str, ex: &mut Extraction) {
        // A recursive call is not a dependency.
        if at.owner != to {
            ex.edge(&at.owner, to, EdgeKind::Calls, "", self.rel);
        }
    }

    fn call_targets(&self, f: Node, at: &At, locals: &Locals) -> Vec<String> {
        let (ns, class) = (at.namespace.as_str(), at.class.as_deref());
        match f.kind() {
            "identifier" | "generic_name" => {
                let name = dotted(f, self.src);
                // A local or parameter named like a method shadows it: `Foo()` through a delegate
                // parameter `Foo` is a call through the parameter, not to the method, and this file
                // does not read a delegate's target, so it writes nothing rather than guess.
                if locals.contains_key(&name) {
                    return Vec::new();
                }
                let own = self.scope.enclosing_member(&name, ns, class);
                if own.is_empty() { self.scope.static_member(&name) } else { own }
            }
            "member_access_expression" => {
                let (Some(e), Some(m)) = (f.child_by_field_name("expression"), f.child_by_field_name("name")) else { return Vec::new() };
                self.through(e, &dotted(m, self.src), at, locals)
            }
            "conditional_access_expression" => {
                let Some(e) = f.child_by_field_name("condition") else { return Vec::new() };
                let binding = named(f).into_iter().find(|c| c.kind() == "member_binding_expression");
                let Some(m) = binding.and_then(|b| b.child_by_field_name("name")) else { return Vec::new() };
                self.through(e, &dotted(m, self.src), at, locals)
            }
            _ => Vec::new(),
        }
    }

    /// `method` called on the receiver `e`. The extension rule stands in only for a value whose
    /// type this file cannot read — never for `this`/`base` themselves, a predefined type, a type
    /// or namespace name, a qualified/generic/alias receiver, or an invocation or other expression
    /// receiver: none of those is a value of unknown type, so a miss there is a miss, not a guess.
    fn through(&self, e: Node, method: &str, at: &At, locals: &Locals) -> Vec<String> {
        let (ns, class) = (at.namespace.as_str(), at.class.as_deref());
        match e.kind() {
            "this" => self.scope.enclosing_member(method, ns, class),
            "base" => self.scope.base_member(method, ns, class),
            "member_access_expression" if e.child_by_field_name("expression").is_some_and(|x| x.kind() == "this") => {
                let field = e.child_by_field_name("name").map(|x| dotted(x, self.src)).unwrap_or_default();
                self.typed_or_extension(&field, method, at, locals, false, ns)
            }
            "identifier" => self.typed_or_extension(text(e, self.src), method, at, locals, true, ns),
            "member_access_expression" | "qualified_name" | "generic_name" | "alias_qualified_name" => {
                receiver_path(e, self.src).map(|t| self.scope.member_of(&t, method, ns, class)).unwrap_or_default()
            }
            // A predefined type, an invocation or any other expression receiver: this file does not
            // read the type such an expression produces, so no member lookup applies, and — because
            // it names no local, parameter or field — the extension rule does not either.
            _ => Vec::new(),
        }
    }

    fn typed_or_extension(&self, name: &str, method: &str, at: &At, locals: &Locals, bare: bool, ns: &str) -> Vec<String> {
        match self.typed(name, method, at, locals, bare) {
            Typed::Found(ids) => ids,
            Typed::Unknown => self.scope.extension(method, ns),
            Typed::None => Vec::new(),
        }
    }

    /// `method` on what `name` stands for: a local or parameter (bare names only — `this.name`
    /// skips straight to members, never consulting a same-named local), then a member of the
    /// enclosing types, then — for a bare name, not `this.name` — a type, which makes the call static.
    fn typed(&self, name: &str, method: &str, at: &At, locals: &Locals, bare: bool) -> Typed {
        let (ns, class) = (at.namespace.as_str(), at.class.as_deref());
        if bare {
            if let Some(t) = locals.get(name) {
                return match t {
                    Some(t) => self.on_type(t, method, ns, class),
                    None => self.unread(method),
                };
            }
        }
        if self.scope.is_enclosing_member(name, ns, class) {
            return match self.scope.enclosing_member_type(name, ns, class) {
                Some(t) => self.on_type(&t, method, ns, class),
                None => self.unread(method),
            };
        }
        // `name` naming a type parameter is not a static target: a same-named repo type must not
        // stand in for it, the same masking `use_type` applies to a type parameter written as a type.
        if bare && !at.type_params.contains(name) {
            return Typed::Found(self.scope.member_of(name, method, ns, class));
        }
        Typed::None
    }

    /// `method` on the repo type `t`: found there directly, or on its base chain, walked as far as
    /// the repo can prove (see `based`). `t` naming no repo type at all is `Unknown` rather than a
    /// proven miss: this file has no proof either way, so the extension rule may still stand in —
    /// that is what closes the gap `member_of` alone left, where a value of a real but
    /// unimplemented-here type silently blocked every extension.
    fn on_type(&self, t: &str, method: &str, ns: &str, class: Option<&str>) -> Typed {
        let parts = self.scope.types(t, ns, class);
        if parts.is_empty() {
            return Typed::Unknown;
        }
        let found = self.scope.member_ids(&parts, method);
        if !found.is_empty() {
            return Typed::Found(found);
        }
        self.based(&parts, method)
    }

    /// `method` walked up `parts`' base chain (`Scope::walk_bases`).
    ///
    /// A base name that resolves to no repo type at all — a framework base such as `object` or
    /// `List<T>` — blocks the extension rule outright: a type this file cannot read could easily
    /// declare `method` itself, so nothing here is proof either way. Only a chain that bottoms out
    /// entirely in repo types, none of which declare `method`, is a proven miss, and only then does
    /// `Unknown` let the extension rule stand in.
    fn based(&self, parts: &[Part], method: &str) -> Typed {
        let mut found = Vec::new();
        let walked = self.scope.walk_bases(parts, |level| {
            found = self.scope.member_ids(level, method);
            !found.is_empty()
        });
        if !found.is_empty() {
            Typed::Found(found)
        } else if !walked.unresolved.is_empty() {
            Typed::Found(Vec::new())
        } else {
            Typed::Unknown
        }
    }

    /// A known local, parameter or field whose declared type this file did not read. The extension
    /// rule may stand in for it only when no repo type declares `method` as a genuine instance
    /// member of its own — when one does, the unread value could be exactly that type, and an
    /// extension would be a guess this file cannot back up either way, so it writes nothing instead.
    fn unread(&self, method: &str) -> Typed {
        if self.scope.declares_instance_member(method) { Typed::Found(Vec::new()) } else { Typed::Unknown }
    }
}

/// What `typed` found `name` to stand for.
enum Typed {
    /// A resolved outcome the extension rule must not override: real hits, `method` on a value's
    /// resolved repo type or somewhere up its base chain; a value whose base chain runs into a
    /// non-repo type, empty, since that type might declare `method` itself and no extension can be
    /// proven over it either; or, for a bare name read as a type rather than a value, whatever
    /// `member_of` finds there, empty included, since a type name never triggers the extension rule
    /// regardless of what it resolves to.
    Found(Vec<String>),
    /// A local, parameter or field this file cannot prove either way: its declared type is unread,
    /// or its declared type names no repo type, or its declared type and its whole base chain are
    /// repo types and none of them declares `method`. The extension rule may stand in for any of these.
    Unknown,
    /// Neither a local, a parameter, a field or property, nor — for a bare name — a type: not a
    /// value, so never a member lookup and never an extension.
    None,
}

/// The identifiers a misparsed `var (a, (b, c))` designation holds, at any depth of nesting.
fn argument_names<'t>(n: Node<'t>, out: &mut Vec<Node<'t>>) {
    for c in named(n) {
        match c.kind() {
            "identifier" => out.push(c),
            "argument" | "tuple_expression" | "parenthesized_expression" => argument_names(c, out),
            _ => {}
        }
    }
}

/// Whether `n` is the function an invocation calls.
fn invoked(n: Node) -> bool {
    n.parent().is_some_and(|p| p.kind() == "invocation_expression" && p.child_by_field_name("function") == Some(n))
}

/// `["Shop", "Orders", "Status", "Open"]` from an access chain of plain identifiers, or none when
/// any link is something else — `this`, a call, a generic name, a predefined type.
fn plain_chain(e: Node, src: &[u8]) -> Option<Vec<String>> {
    match e.kind() {
        "identifier" => Some(vec![text(e, src).to_string()]),
        "member_access_expression" => {
            let name = e.child_by_field_name("name").filter(|x| x.kind() == "identifier")?;
            let mut out = plain_chain(e.child_by_field_name("expression")?, src)?;
            out.push(text(name, src).to_string());
            Some(out)
        }
        _ => None,
    }
}

/// `Shop.Checks.Guard` from a member-access chain whose every link is a plain name.
fn receiver_path(e: Node, src: &[u8]) -> Option<String> {
    match e.kind() {
        "identifier" => Some(text(e, src).to_string()),
        "member_access_expression" => Some(join(&receiver_path(e.child_by_field_name("expression")?, src)?, &dotted(e.child_by_field_name("name")?, src))),
        "qualified_name" | "generic_name" | "alias_qualified_name" => Some(dotted(e, src)),
        _ => None,
    }
}
