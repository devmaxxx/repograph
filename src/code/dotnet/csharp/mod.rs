//! C#: one parse per file, a declarations pass and a references pass over the same tree.

pub mod declarations;
pub mod index;
pub mod refs;
pub mod resolve;

#[cfg(test)]
pub(crate) mod cases;

use crate::code::reader::{self, Collect, Extract, Manifest, Reader};
use crate::code::imports::Resolver;
use crate::code::lang::{Family, Lang};
use crate::code::syntax::{named, text};
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

/// .NET state: C# types with their members, extension methods, projects and their `global using`s.
pub(crate) const READER: Reader = Reader {
    extract: Extract::Source(extract),
    header: Some(|rel, source| index::facts(rel, source).header()),
    collect: Collect::Source(|r, rel, source| {
        let facts = index::facts(rel, source);
        r.add_header(Family::DotNet, rel, &facts.header());
        r.state_mut::<index::DotNet>().add_cs(rel, &facts);
    }),
    state: Some(reader::state::<index::DotNet>),
    manifest: Some(Manifest { matches: |name| name.ends_with(".csproj"), read: |r, rel, text| r.state_mut::<index::DotNet>().add_project(rel, text) }),
    ..reader::NONE
};

/// Parses `source` once and runs both passes over it. A file the grammar cannot parse contributes
/// only its file node — the extractor degrades to that rather than failing the file.
fn extract(resolver: &Resolver, rel: &str, source: &str, ex: &mut Extraction) {
    read(resolver, rel, source, &Host::default(), ex);
}

/// Both passes over one parse, under `host`. Razor calls this on its blanked copy.
pub(crate) fn read(resolver: &Resolver, rel: &str, source: &str, host: &Host, ex: &mut Extraction) -> declarations::Declared {
    let src = source.as_bytes();
    let Some(tree) = Lang::CSharp.parse(src) else { return declarations::Declared::default() };
    let own = declarations::scan(tree.root_node(), rel, src, host, ex);
    refs::scan(tree.root_node(), rel, src, host, &own, resolver, ex);
    own
}

/// The Razor wrapper is the blanked copy's only top-level type, and it stands for the component
/// rather than for what it's written as. Returns the name both passes use, and whether this is
/// that wrapper — which gets no `Symbol` node of its own (`declarations::scan`, `refs::scan`
/// write it once, from `razor::extract`).
pub(crate) fn wrapper_name(top_level: bool, host: &Host, written: String) -> (bool, String) {
    match (top_level, host.component) {
        (true, Some(c)) => (true, c.to_string()),
        _ => (false, written),
    }
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

/// Whether a type ends in type arguments, which `dotted` drops: `A.B<T>` and `global::B<T>`.
pub(crate) fn generic(t: Node) -> bool {
    match t.kind() {
        "qualified_name" | "alias_qualified_name" => t.child_by_field_name("name").is_some_and(generic),
        kind => kind == "generic_name",
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
        "tuple_type" => {
            for el in named(t).into_iter().filter(|c| c.kind() == "tuple_element") {
                if let Some(inner) = el.child_by_field_name("type") {
                    type_names(inner, src, out);
                }
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
