use tree_sitter::Node;

use super::declarations::{member_names, named, text, Declared};
use super::library::Libraries;
use crate::model::{EdgeKind, Extraction};

const TYPES: [&str; 4] = ["class_declaration", "mixin_declaration", "extension_declaration", "enum_declaration"];

/// The symbol a call sits in: the member, field or top-level declaration around it, with closures
/// climbed through. A call inside the unnamed constructor belongs to the class, which is what that
/// constructor is.
fn owner(n: Node, rel: &str, src: &[u8]) -> String {
    let mut var: Option<String> = None;
    let mut member: Option<String> = None;
    let mut cur = n;
    while let Some(p) = cur.parent() {
        match p.kind() {
            "initialized_identifier" | "static_final_declaration" if var.is_none() => {
                var = p.child_by_field_name("name").map(|x| text(x, src).to_string());
            }
            "class_member" if member.is_none() => {
                member = var.clone().or_else(|| member_names(p, src).into_iter().next().map(|(m, _)| m));
            }
            k if TYPES.contains(&k) => {
                let Some(t) = p.child_by_field_name("name") else { return format!("file:{rel}") };
                let t = text(t, src);
                return match &member {
                    Some(m) => format!("sym:{rel}::{t}.{m}"),
                    None => format!("sym:{rel}::{t}"),
                };
            }
            "function_declaration" | "getter_declaration" | "setter_declaration" => {
                if let Some(x) = p.child_by_field_name("signature").and_then(|s| s.child_by_field_name("name")) {
                    return format!("sym:{rel}::{}", text(x, src));
                }
            }
            "top_level_variable_declaration" => {
                if let Some(v) = &var {
                    return format!("sym:{rel}::{v}");
                }
            }
            _ => {}
        }
        cur = p;
    }
    format!("file:{rel}")
}

fn class_of(n: Node, src: &[u8]) -> Option<String> {
    let mut cur = n;
    while let Some(p) = cur.parent() {
        if TYPES.contains(&p.kind()) {
            return p.child_by_field_name("name").map(|t| text(t, src).to_string());
        }
        cur = p;
    }
    None
}

fn ids(lib: &Libraries, rel: &str, name: &str, member: Option<&str>) -> Vec<String> {
    lib.resolve(rel, name).into_iter()
        .map(|f| match member {
            Some(m) => format!("sym:{f}::{name}.{m}"),
            None => format!("sym:{f}::{name}"),
        })
        .collect()
}

/// A member access's receiver; `None` for `this`, which the grammar files under `object` as a bare
/// token, so `this.x()` and an implicit-`this` access read the same.
fn receiver(m: Node) -> Option<Node> {
    m.child_by_field_name("object").filter(|o| o.kind() != "this")
}

fn targets(callee: Node, class: Option<&str>, lib: &Libraries, rel: &str, src: &[u8], d: &Declared) -> Vec<String> {
    let field = |name: &str| class.and_then(|c| d.fields.get(c)).and_then(|f| f.get(name)).cloned();
    match callee.kind() {
        "identifier" => {
            let name = text(callee, src);
            // Dart's implicit `this`: inside a type body a bare name is that type's own member
            // before it is anything the library or an import declares.
            if let Some(c) = class {
                let member = format!("sym:{rel}::{c}.{name}");
                if d.members.contains(&member) {
                    return vec![member];
                }
            }
            ids(lib, rel, name, None)
        }
        "member_expression" => {
            let Some(prop) = callee.child_by_field_name("property").map(|p| text(p, src).to_string()) else { return Vec::new() };
            match receiver(callee) {
                // No `object` field: the receiver is the `this` keyword, which has no node of its own.
                None => class.map(|c| vec![format!("sym:{rel}::{c}.{prop}")]).unwrap_or_default(),
                Some(obj) if obj.kind() == "identifier" => {
                    let receiver = text(obj, src);
                    if let Some(t) = field(receiver) {
                        return ids(lib, rel, &t, Some(&prop));
                    }
                    let prefixed = lib.imported(rel, Some(receiver), &prop);
                    if !prefixed.is_empty() {
                        return prefixed.into_iter().map(|f| format!("sym:{f}::{prop}")).collect();
                    }
                    // `Type.member()`: a static member or a named constructor.
                    ids(lib, rel, receiver, Some(&prop))
                }
                // `this.field.m()`
                Some(obj) if obj.kind() == "member_expression" && receiver(obj).is_none() => {
                    let Some(name) = obj.child_by_field_name("property").map(|p| text(p, src).to_string()) else { return Vec::new() };
                    field(&name).map(|t| ids(lib, rel, &t, Some(&prop))).unwrap_or_default()
                }
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

/// Every call in the file, as an edge from its owner to each id the file can prove it reaches. A
/// name nothing declares (a Flutter API, a parameter, a local) yields no edge.
pub fn scan(root: Node, lib: &Libraries, rel: &str, src: &[u8], d: &Declared, ex: &mut Extraction) {
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        stack.extend(named(n));
        if n.kind() != "call_expression" {
            continue;
        }
        let Some(callee) = n.child_by_field_name("function") else { continue };
        let from = owner(n, rel, src);
        let class = class_of(n, src);
        for to in targets(callee, class.as_deref(), lib, rel, src, d) {
            if to != from {
                ex.edge(&from, &to, EdgeKind::Calls, "", rel);
            }
        }
    }
}
