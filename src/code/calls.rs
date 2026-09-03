//! Call edges: the symbol a call site sits in, to the symbol the file can prove it reaches.
//! Proof is an import, a top-level declaration of this file, or the declared type of the class
//! field the call goes through; a name the file only assumes (a global, a parameter, `console`)
//! yields no edge, so a caller list never contains a guess.
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
                    if let Some(decl) = stmt.child_by_field_name("declaration") { s.class(decl, src); }
                }
                _ => s.class(stmt, src),
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
                    "identifier" => { let n = text(part, src).to_string(); self.names.insert(n.clone(), (target.clone(), n)); }
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
/// them); a self-call is dropped, a repeated call collapses in the extractor's dedup.
pub(crate) fn scan(resolver: &Resolver, rel: &str, source: &str, locals: &BTreeSet<String>, ex: &mut Extraction) {
    let src = source.as_bytes();
    let Some(tree) = parse(rel, src) else { return };
    let root = tree.root_node();
    let scope = Scope::collect(root, rel, src, resolver, locals);
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let mut c = n.walk();
        stack.extend(n.named_children(&mut c));
        let callee = match n.kind() {
            "call_expression" => n.child_by_field_name("function"),
            "new_expression" => n.child_by_field_name("constructor"),
            _ => None,
        };
        let Some(callee) = callee else { continue };
        let class = class_of(n, src);
        let Some(target) = scope.target(callee, class.as_deref(), rel, src) else { continue };
        let from = owner(n, rel, src);
        if from == target { continue }
        ex.edge(&from, &target, EdgeKind::Calls, "", rel);
    }
}
