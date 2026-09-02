use crate::code::symbols::{is_top_level, member_name, parse};
use crate::ids::IdMatcher;
use crate::model::{EdgeKind, Extraction};
use tree_sitter::Node;

/// The symbol an id reference belongs to: the top-level function, class (with its method)
/// or `const` that lexically contains it, else the file. Only top-level declarations are
/// symbols, so a nested function or class is climbed through rather than named.
pub(crate) fn owner(mut n: Node, rel: &str, src: &[u8]) -> String {
    let mut method: Option<String> = None;
    while let Some(p) = n.parent() {
        match p.kind() {
            // A decorator sits beside the member it decorates in `class_body`; the member is
            // the next sibling that is not itself a decorator.
            "decorator" if p.parent().is_some_and(|b| b.kind() == "class_body") => {
                let mut s = p.next_named_sibling();
                while let Some(m) = s {
                    if m.kind() != "decorator" {
                        method = member_name(m, src);
                        break;
                    }
                    s = m.next_named_sibling();
                }
            }
            "method_definition" | "public_field_definition" => {
                if method.is_none() {
                    method = member_name(p, src);
                }
            }
            "function_declaration" | "function_signature" | "interface_declaration" | "type_alias_declaration"
            | "enum_declaration" | "internal_module" if is_top_level(p) => {
                if let Some(f) = name_of(p, src) {
                    return format!("sym:{rel}::{f}");
                }
            }
            "class_declaration" | "abstract_class_declaration" => {
                if is_top_level(p) {
                    if let Some(c) = name_of(p, src) {
                        return match method { Some(m) => format!("sym:{rel}::{c}.{m}"), None => format!("sym:{rel}::{c}") };
                    }
                }
                // A nested class owns the method seen so far; it is not a symbol.
                method = None;
            }
            "variable_declarator" if is_top_level(p) => {
                if let Some(v) = p.child_by_field_name("name").filter(|x| x.kind() == "identifier") {
                    return format!("sym:{rel}::{}", v.utf8_text(src).unwrap_or(""));
                }
            }
            _ => {}
        }
        n = p;
    }
    format!("file:{rel}")
}

fn name_of(n: Node, src: &[u8]) -> Option<String> {
    n.child_by_field_name("name").and_then(|c| c.utf8_text(src).ok()).map(str::to_string)
}

pub fn scan(ids: &IdMatcher, rel: &str, source: &str, ex: &mut Extraction) {
    let src = source.as_bytes();
    let Some(tree) = parse(rel, src) else { return };
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        let ctx = match n.kind() {
            "comment" => "comment",
            "string" | "template_string" => "string",
            _ => {
                let mut c = n.walk();
                stack.extend(n.named_children(&mut c));
                continue;
            }
        };
        let Ok(t) = n.utf8_text(src) else { continue };
        let hits = ids.find_all(t);
        if hits.is_empty() { continue; }
        let from = owner(n, rel, src);
        for h in hits {
            ex.edge(&from, &h.id, EdgeKind::References, ctx, rel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(rel: &str, src: &str) -> Vec<(String, String, String)> {
        let cfg = crate::config::Config::default();
        let ids = IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
        let mut ex = Extraction::default();
        scan(&ids, rel, src, &mut ex);
        ex.edges.iter().filter(|e| e.kind == EdgeKind::References)
            .map(|e| (e.source.clone(), e.target.clone(), e.context.clone())).collect()
    }

    #[test]
    fn comment_ids_attach_to_the_enclosing_symbol() {
        let r = refs("m.ts", "/** see FR-PAY-03 */\nexport function asGrosze() {}\nclass A {\n  // INV-11 here\n  run() { return 'FR-SEC-21'; }\n}\n");
        assert!(r.contains(&("file:m.ts".into(), "FR-PAY-03".into(), "comment".into())));
        assert!(r.contains(&("sym:m.ts::A".into(), "INV-11".into(), "comment".into())));
        assert!(r.contains(&("sym:m.ts::A.run".into(), "FR-SEC-21".into(), "string".into())));
    }

    #[test]
    fn test_titles_count() {
        let r = refs("t.spec.ts", "it('FR-VIS-35 owner sees templates', () => {});\n");
        assert_eq!(r, vec![("file:t.spec.ts".into(), "FR-VIS-35".into(), "string".into())]);
    }

    #[test]
    fn identifiers_are_not_scanned() {
        assert!(refs("x.ts", "const FR_PAY_22 = 1; const a = FRPAY22;\n").is_empty());
    }

    #[test]
    fn template_literal_ids_use_string_context_and_attach_to_method() {
        let r = refs("m.ts", "class A {\n  run() { return `see FR-SEC-21`; }\n}\n");
        assert!(r.contains(&("sym:m.ts::A.run".into(), "FR-SEC-21".into(), "string".into())));
    }

    #[test]
    fn staff_controller_fixture_yields_fr_vis_35() {
        let text = std::fs::read_to_string(format!("{}/tests/fixtures/staff.controller.ts", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let r = refs("c.ts", &text);
        assert!(r.contains(&("file:c.ts".into(), "FR-VIS-35".into(), "comment".into())));
        assert!(r.contains(&("sym:c.ts::helper".into(), "FR-SEC-21".into(), "string".into())));
    }
}
