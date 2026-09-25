//! C#: one parse per file, a declarations pass and a references pass over the same tree.

pub mod declarations;
pub mod index;
pub mod refs;
pub mod resolve;

#[cfg(test)]
mod cases;

use crate::code::imports::Resolver;
use crate::code::lang::{file_node, Lang};
use crate::model::Extraction;
use tree_sitter::Node;

/// What a walk reads under. A `.cs` file is its own host; a Razor component's blanked copy is read
/// under the component, because the Razor compiler — not the file — decides its class, namespace
/// and usings.
#[derive(Default, Clone, Copy)]
pub struct Host<'a> {
    /// The component the blanked copy's wrapper class stands for; `razor::extract` writes its node.
    pub component: Option<&'a str>,
    /// The namespace the host puts top-level declarations in before the file names one.
    pub namespace: &'a str,
    /// Usings the host brings: `@using` lines and every `_Imports.razor` above the component.
    pub usings: &'a [declarations::Using],
    /// `@inject T Name` as (name, type head): members of the component with a declared type.
    pub injected: &'a [(String, String)],
}

/// Parses `source` once and runs both passes over it. A file the grammar cannot parse contributes
/// only its file node — the extractor degrades to that rather than failing the file.
pub fn extract(resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let mut ex = Extraction::default();
    file_node(rel, &mut ex);
    read(resolver, rel, source, &Host::default(), &mut ex);
    ex
}

/// Both passes over one parse, under `host`. Razor calls this on its blanked copy.
pub(crate) fn read(resolver: &Resolver, rel: &str, source: &str, host: &Host, ex: &mut Extraction) -> declarations::Declared {
    let src = source.as_bytes();
    let Some(tree) = Lang::CSharp.parse(src) else { return declarations::Declared::default() };
    let own = declarations::scan(tree.root_node(), rel, src, host, ex);
    refs::scan(tree.root_node(), rel, src, host, &own, resolver, ex);
    own
}

pub(crate) fn text<'a>(n: Node, src: &'a [u8]) -> &'a str {
    n.utf8_text(src).unwrap_or("")
}

pub(crate) fn named<'t>(n: Node<'t>) -> Vec<Node<'t>> {
    let mut c = n.walk();
    n.named_children(&mut c).collect()
}

/// First and last line, 1-based. A declaration's extent includes its attribute lists, so a hunk
/// that edits only `[HttpGet("x")]` still lands inside the member it changes.
pub(crate) fn span(n: Node) -> (u32, u32) {
    (n.start_position().row as u32 + 1, n.end_position().row as u32 + 1)
}

/// The words of a node's `modifier` children: `public`, `static`, `partial`, and `this` on an
/// extension method's first parameter. The keyword is the modifier's only, anonymous, child.
pub(crate) fn modifiers(n: Node, src: &[u8]) -> Vec<String> {
    named(n).into_iter().filter(|c| c.kind() == "modifier").map(|c| text(c, src).to_string()).collect()
}

/// A type or namespace name as dots, without generic arguments or `global::`: `Shop.Bedrock.BaseRepo`.
pub(crate) fn dotted(t: Node, src: &[u8]) -> String {
    match t.kind() {
        "qualified_name" => {
            let q = t.child_by_field_name("qualifier").map(|q| dotted(q, src)).unwrap_or_default();
            let n = t.child_by_field_name("name").map(|n| dotted(n, src)).unwrap_or_default();
            declarations::join(&q, &n)
        }
        "generic_name" => named(t).into_iter().find(|c| c.kind() == "identifier").map(|c| text(c, src).to_string()).unwrap_or_default(),
        "alias_qualified_name" => t.child_by_field_name("name").map(|n| dotted(n, src)).unwrap_or_default(),
        _ => text(t, src).to_string(),
    }
}

/// Every class-like name a type expression uses: `Task<List<Order>>` gives `Task`, `List`, `Order`;
/// `Shop.Payments.IGateway?` gives `Shop.Payments.IGateway`. Predefined types (`int`) name nothing.
pub(crate) fn type_names(t: Node, src: &[u8], out: &mut Vec<String>) {
    match t.kind() {
        "identifier" => out.push(text(t, src).to_string()),
        "generic_name" | "qualified_name" | "alias_qualified_name" => {
            out.push(dotted(t, src));
            let mut stack = named(t);
            while let Some(c) = stack.pop() {
                match c.kind() {
                    "type_argument_list" => {
                        for a in named(c) {
                            type_names(a, src, out);
                        }
                    }
                    "generic_name" | "qualified_name" | "alias_qualified_name" => stack.extend(named(c)),
                    _ => {}
                }
            }
        }
        "nullable_type" | "array_type" | "pointer_type" | "ref_type" | "scoped_type" => {
            if let Some(inner) = t.child_by_field_name("type") {
                type_names(inner, src, out);
            }
        }
        _ => {}
    }
}

/// The name a receiver's declared type is looked up by: the outermost name, `List` for `List<Order>`.
pub(crate) fn head(t: Option<Node>, src: &[u8]) -> Option<String> {
    let mut v = Vec::new();
    type_names(t?, src, &mut v);
    v.into_iter().next()
}
