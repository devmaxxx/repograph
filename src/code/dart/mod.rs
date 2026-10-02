pub mod calls;
pub mod declarations;
pub mod library;

#[cfg(test)]
mod cases;

use std::collections::BTreeSet;

use tree_sitter::Node;

use crate::code::imports::Resolver;
use crate::code::lang::{file_node, Lang};
use crate::model::{EdgeKind, Extraction};
use declarations::{named, text};
use library::Libraries;

/// One parse per file; every pass reads the same tree.
pub fn extract(resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let mut ex = Extraction::default();
    file_node(rel, &mut ex);
    let src = source.as_bytes();
    let Some(tree) = Lang::Dart.parse(src) else { return ex };
    let root = tree.root_node();
    let declared = declarations::scan(root, rel, src, &mut ex);
    directives(resolver.dart(), rel, root, src, &mut ex);
    calls::scan(root, resolver.dart(), rel, src, &declared, &mut ex);
    // Only a supertype that resolves is an edge. Every Flutter widget extends a class declared
    // outside the repository, and a same-file guess would name a symbol no file declares.
    for (from, name) in &declared.supers {
        for f in resolver.dart().resolve(rel, name) {
            ex.edge(from, &format!("sym:{f}::{name}"), EdgeKind::Extends, "", rel);
        }
    }
    ex
}

#[derive(Default)]
struct Used {
    bare: BTreeSet<String>,
    /// (prefix, name) for every `p.Name` in the file
    prefixed: BTreeSet<(String, String)>,
}

fn used_names(root: Node, src: &[u8]) -> Used {
    let mut u = Used::default();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        match n.kind() {
            "identifier" | "type_identifier" => {
                u.bare.insert(text(n, src).to_string());
            }
            "member_expression" => {
                if let (Some(o), Some(p)) = (n.child_by_field_name("object"), n.child_by_field_name("property")) {
                    if o.kind() == "identifier" {
                        u.prefixed.insert((text(o, src).to_string(), text(p, src).to_string()));
                    }
                }
            }
            // `p.Widget` as a type is one `type` holding two `type_identifier`s around the `.`.
            "type" => {
                let parts = named(n);
                if let [p, name, ..] = parts.as_slice() {
                    let between = src.get(p.end_byte()..name.start_byte()).and_then(|b| std::str::from_utf8(b).ok());
                    if p.kind() == "type_identifier" && name.kind() == "type_identifier" && between.is_some_and(|b| b.trim() == ".") {
                        u.prefixed.insert((text(*p, src).to_string(), text(*name, src).to_string()));
                    }
                }
            }
            _ => {}
        }
        stack.extend(named(n));
    }
    u
}

/// An import's context is the names this file uses from it, as a TypeScript named import's is.
/// `impact` counts an importer only when the context names the symbol, and every Dart import brings
/// a whole namespace, so writing `*` would make every importer depend on every name.
fn directives(lib: &Libraries, rel: &str, root: Node, src: &[u8], ex: &mut Extraction) {
    let file_id = format!("file:{rel}");
    let used = used_names(root, src);
    for (target, prefix, names) in lib.import_targets(rel) {
        let kept: Vec<&str> = names.iter()
            .filter(|n| match &prefix {
                Some(p) => used.prefixed.contains(&(p.clone(), (*n).clone())),
                None => used.bare.contains(*n),
            })
            .map(String::as_str)
            .collect();
        ex.edge(&file_id, &format!("file:{target}"), EdgeKind::Imports, &kept.join(","), rel);
    }
    let Some(h) = lib.header(rel) else { return };
    for e in &h.exports {
        let (Some(t), Some(context)) = (lib.target(rel, &e.uri), lib.export_context(rel, e)) else { continue };
        ex.edge(&file_id, &format!("file:{t}"), EdgeKind::ReExports, &context, rel);
    }
}
