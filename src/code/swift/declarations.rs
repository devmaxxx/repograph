use std::collections::BTreeSet;

use tree_sitter::Node;

use crate::code::syntax::{child, named, span, text};
use crate::model::{EdgeKind, Extraction, NodeKind};

#[derive(Debug, Default)]
pub struct Declared {
    /// Top-level type names this file declares and does not hide: what an inheritance clause anywhere in
    /// the module can name.
    pub types: BTreeSet<String>,
    /// Every top-level type name this file declares, `private` and `fileprivate` included: what its own
    /// clauses see first.
    pub own: BTreeSet<String>,
    /// (the type's qualified name, a name in its inheritance clause, whether this file declares the type).
    /// An extension's clause belongs to a type declared wherever the index says.
    pub supers: Vec<(String, String, bool)>,
}

const TYPE_KEYWORDS: [&str; 3] = ["class", "struct", "enum"];

/// The grammar writes class, struct, enum, actor and extension as one node kind; the keyword is its
/// anonymous token.
fn keyword(n: Node) -> Option<&'static str> {
    let mut c = n.walk();
    let found = n.children(&mut c)
        .filter(|k| !k.is_named())
        .find_map(|k| ["class", "struct", "enum", "extension", "actor"].into_iter().find(|w| *w == k.kind()));
    found
}

/// `private(set)` restricts a setter and keeps the declaration visible, so only the two bare words hide one.
fn hidden(n: Node, src: &[u8]) -> bool {
    child(n, "modifiers").is_some_and(|m| named(m).into_iter()
        .any(|c| c.kind() == "visibility_modifier" && matches!(text(c, src), "private" | "fileprivate")))
}

fn declare(ex: &mut Extraction, rel: &str, parent: &str, qualified: &str, n: Node, src: &[u8]) -> String {
    let id = format!("sym:{rel}::{qualified}");
    let signature = text(n, src).lines().find(|l| !l.trim_start().starts_with('@')).unwrap_or("").trim().to_string();
    ex.node_span(NodeKind::Symbol, &id, qualified, &signature, rel, span(n));
    ex.edge(parent, &id, EdgeKind::Declares, if hidden(n, src) { "" } else { "export" }, rel);
    id
}

fn inherits(n: Node, src: &[u8]) -> Vec<String> {
    named(n).into_iter()
        .filter(|s| s.kind() == "inheritance_specifier")
        .filter_map(|s| s.child_by_field_name("inherits_from"))
        .filter_map(|u| child(u, "type_identifier"))
        .map(|t| text(t, src).to_string())
        .collect()
}

fn type_decl(n: Node, rel: &str, src: &[u8], parent: &str, owner: Option<&str>, ex: &mut Extraction, d: &mut Declared) {
    let Some(name) = n.child_by_field_name("name").map(|t| text(t, src).to_string()) else { return };
    let qualified = owner.map_or_else(|| name.clone(), |o| format!("{o}.{name}"));
    let id = declare(ex, rel, parent, &qualified, n, src);
    if owner.is_none() {
        if !hidden(n, src) {
            d.types.insert(name.clone());
        }
        d.own.insert(name);
    }
    for s in inherits(n, src) {
        d.supers.push((qualified.clone(), s, true));
    }
    body(n, rel, src, &id, &qualified, ex, d);
}

fn body(n: Node, rel: &str, src: &[u8], parent: &str, owner: &str, ex: &mut Extraction, d: &mut Declared) {
    let Some(b) = n.child_by_field_name("body") else { return };
    for m in named(b) {
        match m.kind() {
            "function_declaration" | "protocol_function_declaration" => {
                if let Some(f) = m.child_by_field_name("name") {
                    declare(ex, rel, parent, &format!("{owner}.{}", text(f, src)), m, src);
                }
            }
            "class_declaration" if keyword(m).is_some_and(|k| TYPE_KEYWORDS.contains(&k)) => {
                type_decl(m, rel, src, parent, Some(owner), ex, d);
            }
            "protocol_declaration" => type_decl(m, rel, src, parent, Some(owner), ex, d),
            _ => {}
        }
    }
}

pub fn scan(root: Node, rel: &str, src: &[u8], ex: &mut Extraction) -> Declared {
    let file_id = format!("file:{rel}");
    let mut d = Declared::default();
    for n in named(root) {
        match n.kind() {
            "class_declaration" => match keyword(n) {
                Some("extension") => {
                    // An extension declares no type: its members are named on the type it extends, as a
                    // caller writes them, and the file declares them.
                    let Some(user) = n.child_by_field_name("name") else { continue };
                    let owner: Vec<&str> = named(user).into_iter().filter(|t| t.kind() == "type_identifier").map(|t| text(t, src)).collect();
                    let owner = owner.join(".");
                    body(n, rel, src, &file_id, &owner, ex, &mut d);
                    for s in inherits(n, src) {
                        d.supers.push((owner.clone(), s, false));
                    }
                }
                Some(k) if TYPE_KEYWORDS.contains(&k) => type_decl(n, rel, src, &file_id, None, ex, &mut d),
                _ => {}
            },
            "protocol_declaration" => type_decl(n, rel, src, &file_id, None, ex, &mut d),
            "function_declaration" => {
                let Some(f) = n.child_by_field_name("name") else { continue };
                declare(ex, rel, &file_id, text(f, src), n, src);
            }
            "property_declaration" if child(n, "value_binding_pattern").is_some_and(|v| text(v, src) == "let") => {
                let mut c = n.walk();
                let patterns: Vec<Node> = n.children_by_field_name("name", &mut c).collect();
                for p in patterns {
                    let Some(b) = p.child_by_field_name("bound_identifier") else { continue };
                    declare(ex, rel, &file_id, text(b, src), n, src);
                }
            }
            _ => {}
        }
    }
    d
}
