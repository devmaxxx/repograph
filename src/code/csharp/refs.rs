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

/// Locals and parameters of the member being read, to their declared type heads.
type Locals = BTreeMap<String, String>;

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
                if let (Some(name), Some(t)) = (n.child_by_field_name("name"), head(n.child_by_field_name("type"), self.src)) {
                    locals.insert(text(name, self.src).to_string(), t);
                }
            }
            "variable_declaration" => self.declare_locals(n, locals),
            "declaration_pattern" | "recursive_pattern" => {
                if let (Some(name), Some(t)) = (n.child_by_field_name("name"), head(n.child_by_field_name("type"), self.src)) {
                    locals.insert(text(name, self.src).to_string(), t);
                }
            }
            "foreach_statement" => {
                if let (Some(left), Some(t)) = (n.child_by_field_name("left").filter(|l| l.kind() == "identifier"), head(n.child_by_field_name("type"), self.src)) {
                    locals.insert(text(left, self.src).to_string(), t);
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
            if let Some(t) = declared.clone().or(inferred) {
                locals.insert(text(name, self.src).to_string(), t);
            }
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
}
