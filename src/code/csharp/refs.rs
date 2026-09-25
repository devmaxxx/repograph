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

fn first_segment(local: &str) -> &str {
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
            "declaration_pattern" | "recursive_pattern" | "declaration_expression" => {
                if let Some(name) = n.child_by_field_name("name") {
                    locals.insert(text(name, self.src).to_string(), head(n.child_by_field_name("type"), self.src));
                }
            }
            "foreach_statement" => {
                if let Some(left) = n.child_by_field_name("left").filter(|l| l.kind() == "identifier") {
                    locals.insert(text(left, self.src).to_string(), head(n.child_by_field_name("type"), self.src));
                }
            }
            // A lambda's own parameters are in scope for its body only; cloning locals keeps a
            // same-named field or outer local from leaking in, and keeps the parameter itself from
            // leaking out.
            "lambda_expression" => return self.scoped_body(n, n.child_by_field_name("parameters"), n.child_by_field_name("body"), at, locals, ex),
            "anonymous_method_expression" => {
                let body = named(n).into_iter().find(|c| c.kind() == "block");
                return self.scoped_body(n, n.child_by_field_name("parameters"), body, at, locals, ex);
            }
            "local_function_statement" => return self.scoped_body(n, n.child_by_field_name("parameters"), n.child_by_field_name("body"), at, locals, ex),
            "invocation_expression" => {
                if let Some(f) = n.child_by_field_name("function") {
                    for to in self.call_targets(f, at, locals) {
                        self.call(at, &to, ex);
                    }
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
            let Some(name) = d.child_by_field_name("name") else { continue };
            // `var x = new T()` and `var x = (T)y` say the type on the right.
            let inferred = named(d).into_iter().find_map(|e| match e.kind() {
                "object_creation_expression" | "cast_expression" => head(e.child_by_field_name("type"), self.src),
                _ => None,
            });
            locals.insert(text(name, self.src).to_string(), declared.clone().or(inferred));
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
        self.based(&parts, method, ns, class)
    }

    /// `method` walked up `parts`' base chain, however many files it crosses: `Scope::bases` holds
    /// every base-list name written for a type anywhere in the repo, not only in the file at hand,
    /// so a subclass and its base need not share a file with each other or with the caller. Each
    /// base name is resolved in the caller's own scope — a stand-in for the declaring file's own
    /// usings, which this pass never has, but sound whenever caller and base sit under the same
    /// imported namespace, as a subclass and its own base almost always do.
    ///
    /// A base name that resolves to no repo type at all — a framework base such as `object` or
    /// `List<T>` — blocks the extension rule outright: a type this file cannot read could easily
    /// declare `method` itself, so nothing here is proof either way. Only a chain that bottoms out
    /// entirely in repo types, none of which declare `method`, is a proven miss, and only then does
    /// `Unknown` let the extension rule stand in.
    fn based(&self, parts: &[Part], method: &str, ns: &str, class: Option<&str>) -> Typed {
        let mut seen: BTreeSet<String> = parts.iter().map(|p| p.full.clone()).collect();
        let mut frontier = parts.to_vec();
        let mut external = false;
        for _ in 0..32 {
            if frontier.is_empty() {
                break;
            }
            let mut next = Vec::new();
            let mut found = Vec::new();
            for p in &frontier {
                for b in self.scope.bases(&p.full) {
                    let bp = self.scope.types(&b, ns, class);
                    if bp.is_empty() {
                        external = true;
                        continue;
                    }
                    found.extend(self.scope.member_ids(&bp, method));
                    next.extend(bp.into_iter().filter(|q| seen.insert(q.full.clone())));
                }
            }
            if !found.is_empty() {
                return Typed::Found(found);
            }
            frontier = next;
        }
        if external { Typed::Found(Vec::new()) } else { Typed::Unknown }
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

/// `Shop.Checks.Guard` from a member-access chain whose every link is a plain name.
fn receiver_path(e: Node, src: &[u8]) -> Option<String> {
    match e.kind() {
        "identifier" => Some(text(e, src).to_string()),
        "member_access_expression" => Some(join(&receiver_path(e.child_by_field_name("expression")?, src)?, &dotted(e.child_by_field_name("name")?, src))),
        "qualified_name" | "generic_name" | "alias_qualified_name" => Some(dotted(e, src)),
        _ => None,
    }
}
