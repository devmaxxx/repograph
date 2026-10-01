//! Call edges: the symbol a call site sits in, to the symbol the file can prove it reaches.
//! Proof is an import, a top-level declaration of this file, or the declared type of the class
//! field the call goes through; a name the file only assumes (a global, a parameter, `console`)
//! yields no edge, so a caller list never contains a guess. The target is always proven; what a
//! function handed to another call does with it is not — `rows.map(fn)` calls it,
//! `register('T', Cls)` may only keep it — and both count, because `impact` asks what breaks
//! when the target changes, and either caller does.
use crate::code::idrefs::owner;
use crate::code::imports::Resolver;
use crate::code::symbols::{is_top_level, parse};
use crate::model::{EdgeKind, Extraction};
use std::collections::{BTreeMap, BTreeSet};
use tree_sitter::Node;

fn text<'a>(n: Node, src: &'a [u8]) -> &'a str { n.utf8_text(src).unwrap_or("") }

/// What one file lets a call resolve to, gathered in one pass over its top-level statements.
#[derive(Default)]
struct Scope {
    /// local binding -> (file that declares it, the name it is declared under there); this
    /// file and the same name for its own top-level declarations, `import { a as b }` maps
    /// `b` to `a`
    names: BTreeMap<String, (String, String)>,
    /// `import * as ns` binding -> file
    namespaces: BTreeMap<String, String>,
    /// class name -> field name -> declared type name, from typed fields and constructor
    /// parameter properties — the NestJS injection shape `this.service.create()` goes through
    fields: BTreeMap<String, BTreeMap<String, String>>,
    /// top-level function -> the class it returns, from `(): T` or a body that is `new T(…)` —
    /// the test-helper shape `service().method()` goes through
    returns: BTreeMap<String, String>,
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

fn class_of(n: Node, src: &[u8]) -> Option<String> {
    let mut cur = n;
    while let Some(p) = cur.parent() {
        if matches!(p.kind(), "class_declaration" | "abstract_class_declaration") && is_top_level(p) {
            return p.child_by_field_name("name").map(|c| text(c, src).to_string());
        }
        cur = p;
    }
    None
}

impl Scope {
    fn collect(root: Node, rel: &str, src: &[u8], resolver: &Resolver, locals: &BTreeSet<String>) -> Scope {
        let mut s = Scope::default();
        for name in locals { s.names.insert(name.clone(), (rel.to_string(), name.clone())); }
        let mut cur = root.walk();
        for stmt in root.named_children(&mut cur) {
            match stmt.kind() {
                "import_statement" => s.import(stmt, rel, src, resolver),
                "export_statement" => {
                    if let Some(decl) = stmt.child_by_field_name("declaration") { s.class(decl, src); s.returns(decl, src); }
                }
                _ => { s.class(stmt, src); s.returns(stmt, src); }
            }
        }
        s
    }

    fn import(&mut self, stmt: Node, rel: &str, src: &[u8], resolver: &Resolver) {
        let Some(source) = stmt.child_by_field_name("source") else { return };
        let spec = text(source, src).trim_matches(|c| c == '\'' || c == '"');
        let Some(target) = resolver.resolve(rel, spec) else { return };
        let mut cur = stmt.walk();
        for clause in stmt.named_children(&mut cur).filter(|c| c.kind() == "import_clause") {
            let mut cc = clause.walk();
            for part in clause.named_children(&mut cc) {
                match part.kind() {
                    // A default import binds whatever name the importer chose; the declaring
                    // file's symbol is a best guess under that name.
                    // A `.vue` default import is the component, declared under the file's stem.
                    "identifier" => {
                        let n = text(part, src).to_string();
                        let declared = crate::code::vue::component_name(&target).map_or_else(|| n.clone(), str::to_string);
                        self.names.insert(n, (target.clone(), declared));
                    }
                    "namespace_import" => {
                        if let Some(id) = part.named_child(0) { self.namespaces.insert(text(id, src).to_string(), target.clone()); }
                    }
                    "named_imports" => {
                        let mut nc = part.walk();
                        for spec in part.named_children(&mut nc).filter(|s| s.kind() == "import_specifier") {
                            let Some(name) = spec.child_by_field_name("name") else { continue };
                            let local = spec.child_by_field_name("alias").unwrap_or(name);
                            self.names.insert(text(local, src).to_string(), (target.clone(), text(name, src).to_string()));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn class(&mut self, decl: Node, src: &[u8]) {
        if !matches!(decl.kind(), "class_declaration" | "abstract_class_declaration") { return }
        let Some(name) = decl.child_by_field_name("name").map(|n| text(n, src).to_string()) else { return };
        let Some(body) = decl.child_by_field_name("body") else { return };
        let fields = self.fields.entry(name).or_default();
        let mut bc = body.walk();
        for m in body.named_children(&mut bc) {
            match m.kind() {
                "public_field_definition" => {
                    if let (Some(n), Some(t)) = (m.child_by_field_name("name"), m.child_by_field_name("type").and_then(|t| type_name(t, src))) {
                        fields.insert(text(n, src).to_string(), t);
                    }
                }
                "method_definition" if m.child_by_field_name("name").is_some_and(|n| text(n, src) == "constructor") => {
                    let Some(params) = m.child_by_field_name("parameters") else { continue };
                    let mut pc = params.walk();
                    for p in params.named_children(&mut pc) {
                        // Only a parameter with an accessibility modifier becomes a field.
                        let mut kids = p.walk();
                        if !p.named_children(&mut kids).any(|k| k.kind() == "accessibility_modifier") { continue }
                        let pattern = p.child_by_field_name("pattern").filter(|x| x.kind() == "identifier");
                        let ty = p.child_by_field_name("type").and_then(|t| type_name(t, src));
                        if let (Some(n), Some(t)) = (pattern, ty) { fields.insert(text(n, src).to_string(), t); }
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
            if let Some(t) = annotated.or_else(built) { self.returns.insert(text(name, src).to_string(), t); }
        };
        match decl.kind() {
            "function_declaration" => if let Some(n) = decl.child_by_field_name("name") { record(n, decl) },
            "lexical_declaration" | "variable_declaration" => {
                let mut dc = decl.walk();
                for d in decl.named_children(&mut dc).filter(|d| d.kind() == "variable_declarator") {
                    let name = d.child_by_field_name("name").filter(|n| n.kind() == "identifier");
                    let value = d.child_by_field_name("value").filter(|v| matches!(v.kind(), "arrow_function" | "function_expression"));
                    if let (Some(n), Some(v)) = (name, value) { record(n, v); }
                }
            }
            _ => {}
        }
    }

    fn target(&self, callee: Node, class: Option<&str>, rel: &str, src: &[u8]) -> Option<String> {
        match callee.kind() {
            "identifier" => {
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
                        if let Some(f) = self.namespaces.get(n) { return Some(format!("sym:{f}::{prop}")); }
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
                        if inner.kind() != "this" { return None }
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

/// Every call and `new` in the file, as an edge from its owner to what the scope proves it
/// reaches. `locals` are the names this file declares at top level (the scanner already knows
/// them); a self-call is dropped, and a repeated call collapses into one edge.
pub(crate) fn scan(resolver: &Resolver, rel: &str, source: &str, locals: &BTreeSet<String>, ex: &mut Extraction) {
    let src = source.as_bytes();
    let Some(tree) = parse(rel, src) else { return };
    scan_tree(resolver, rel, tree.root_node(), src, locals, ex);
}

/// `scan` over a tree someone else parsed: an embedded script is TypeScript at its host file's rows.
pub(crate) fn scan_tree(resolver: &Resolver, rel: &str, root: Node, src: &[u8], locals: &BTreeSet<String>, ex: &mut Extraction) {
    let scope = Scope::collect(root, rel, src, resolver, locals);
    // (owner, target) -> whether any site calls it rather than only passing it: one edge per
    // pair keeps `impact`'s counts, and a real call is the stronger claim, so it wins.
    let mut found: BTreeMap<(String, String), bool> = BTreeMap::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let mut c = n.walk();
        stack.extend(n.named_children(&mut c));
        let callee = match n.kind() {
            "call_expression" => n.child_by_field_name("function"),
            "new_expression" => n.child_by_field_name("constructor"),
            // Rendering a component calls it. A lowercase tag is an intrinsic element, as React
            // reads it, even when a binding of that name is in scope.
            "jsx_opening_element" | "jsx_self_closing_element" => n.child_by_field_name("name")
                .filter(|c| c.kind() != "identifier" || text(*c, src).starts_with(|ch: char| ch.is_ascii_uppercase())),
            _ => None,
        };
        let Some(callee) = callee else { continue };
        let class = class_of(n, src);
        // A function handed to another — `rows.map(feedWire)`, `.filter(isIndexedType)` — is
        // called on the caller's behalf, and a change to it breaks the caller all the same; the
        // edge says `arg`, because a constant or a DI token handed over the same way is not.
        let mut ac = n.walk();
        let passed: Vec<Node> = n.child_by_field_name("arguments")
            .map(|a| a.named_children(&mut ac).filter(|x| x.kind() == "identifier").collect())
            .unwrap_or_default();
        let from = owner(n, rel, src);
        for (i, x) in std::iter::once(callee).chain(passed).enumerate() {
            let Some(target) = scope.target(x, class.as_deref(), rel, src) else { continue };
            if from != target { *found.entry((from.clone(), target)).or_default() |= i == 0; }
        }
    }
    for ((from, target), called) in found {
        ex.edge(&from, &target, EdgeKind::Calls, if called { "" } else { "arg" }, rel);
    }
}
