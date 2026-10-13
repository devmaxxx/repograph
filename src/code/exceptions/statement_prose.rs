//! A declaration's body is its statement as written: the comments directly above the statement,
//! `export` wrapper included, then the line that declares it, and a whole function flattened to one
//! line, since a function's body is the prose a question about it matches. The comment block is the
//! one ending on the line before; one left standing a blank line above is a section heading. A
//! `declare` statement reads as what it declares, and an exported one says `export`. A member reads
//! its own comments and its first line past its decorators. The file's head is the comments before
//! its first statement.
//!
//! No `Doc` reads a comment above a wrapper the definition sits in, and no `Line` reads a whole
//! function, so this hook writes the body whole.

use tree_sitter::Node;

use crate::code::engine::{Def, File};
use crate::code::syntax::{cap, comment_text, spanning, text, DOC_CHARS};

/// A file head gets twice a doc comment's cap, being the module's own account.
const HEAD_CHARS: usize = 2 * DOC_CHARS;

pub(crate) fn head(file: &File) -> String {
    let Some(tree) = file.tree else { return String::new() };
    let (root, src) = (tree.root_node(), file.src.as_bytes());
    let mut cur = root.walk();
    let parts: Vec<String> = root.named_children(&mut cur).take_while(|c| c.kind() == "comment").map(|c| comment_text(text(c, src))).collect();
    cap(parts.join("\n"), HEAD_CHARS)
}

pub(crate) fn body(file: &File, def: &Def) -> String {
    let Some(node) = file.tree.and_then(|t| spanning(t.root_node(), &def.range)) else { return String::new() };
    let src = file.src.as_bytes();
    if def.parent.is_some() {
        let signature = text(node, src).lines().find(|l| !l.trim_start().starts_with('@')).unwrap_or("").trim();
        return with_doc(doc_comment(node, src), signature.to_string());
    }
    let decl = match node.kind() {
        "variable_declarator" => node.parent().unwrap_or(node),
        _ => node,
    };
    let wrapped = decl.parent().filter(|p| p.kind() == "ambient_declaration").unwrap_or(decl);
    let outer = wrapped.parent().filter(|p| p.kind() == "export_statement");
    let written = text(decl, src);
    let signature = match decl.kind() {
        "function_declaration" => written.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" "),
        _ => written.lines().next().unwrap_or("").trim().to_string(),
    };
    let signature = match outer {
        Some(_) if !signature.starts_with("export") => format!("export {signature}"),
        _ => signature,
    };
    with_doc(doc_comment(outer.unwrap_or(wrapped), src), signature)
}

fn with_doc(doc: String, signature: String) -> String {
    if doc.is_empty() { signature } else { format!("{doc}\n{signature}") }
}

fn doc_comment(n: Node, src: &[u8]) -> String {
    let mut parts = Vec::new();
    let mut next = n;
    while let Some(prev) = next.prev_sibling() {
        if prev.kind() != "comment" || prev.end_position().row + 1 < next.start_position().row {
            break;
        }
        parts.push(comment_text(text(prev, src)));
        next = prev;
    }
    parts.reverse();
    cap(parts.join("\n"), DOC_CHARS)
}
