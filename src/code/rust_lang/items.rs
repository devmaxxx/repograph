//! What a Rust file declares: symbols and their `Declares` edges, and the table of names the
//! use, call and attribute passes resolve against.

use super::{field_text, scoped, type_path};
use crate::code::jvm::{cite, descend, named, text};
use crate::model::{EdgeKind, Extraction, NodeKind};
use std::collections::{BTreeMap, BTreeSet};
use tree_sitter::Node;

const ITEMS: [&str; 9] =
    ["function_item", "function_signature_item", "struct_item", "enum_item", "union_item", "type_item", "const_item", "static_item", "macro_definition"];
/// What a trait or `impl` body declares as `Container.member`.
const MEMBERS: [&str; 5] = ["function_item", "function_signature_item", "associated_type", "const_item", "type_item"];
/// As TypeScript's doc cap: BM25 documents are `id + label + body`, and a longer body drowns the name.
const BODY_CHARS: usize = 600;

#[derive(Debug, Default)]
pub(crate) struct Items {
    /// Every id suffix the file declares: `f`, `S`, `S.new`, `tests/helper`.
    pub names: BTreeSet<String>,
    /// Each scope's directly declared names, keyed by its inline modules joined with `/` ("" is the file).
    pub scopes: BTreeMap<String, BTreeSet<String>>,
    /// Inline module paths, joined with `/`.
    pub modules: BTreeSet<String>,
    /// `macro_rules!` name → its id suffix.
    pub macros: BTreeMap<String, String>,
    pub impls: Vec<Impl>,
}

// The use and call passes resolve against `ty` and `members`; nothing reads them until those passes exist.
#[expect(dead_code)]
#[derive(Debug)]
pub(crate) struct Impl {
    pub inline: Vec<String>,
    /// The implementing type's path as written: `["Store"]`, `["crate", "walk", "Manifest"]`.
    pub ty: Vec<String>,
    pub trait_path: Option<Vec<String>>,
    /// The id suffixes of the members the block declares.
    pub members: Vec<String>,
}

struct Walk<'a> {
    rel: &'a str,
    src: &'a [u8],
    items: Items,
    /// Suffix → the parent node of its `#[cfg]`-gated declaration, to recognise the gated twin beside it.
    gated: BTreeMap<String, usize>,
    ex: &'a mut Extraction,
}

pub(crate) fn read(rel: &str, src: &[u8], root: Node, ex: &mut Extraction) -> Items {
    let mut walk = Walk { rel, src, items: Items::default(), gated: BTreeMap::new(), ex };
    walk.scope(root, &[]);
    walk.items
}

fn exported(n: Node) -> bool {
    named(n).iter().any(|c| c.kind() == "visibility_modifier")
}

impl Walk<'_> {
    fn scope(&mut self, body: Node, inline: &[String]) {
        let file_id = format!("file:{}", self.rel);
        let key = inline.join("/");
        let children = named(body);
        // Everything but impls first: an `impl` may precede the type it implements, and whether
        // its members hang off a type of this scope depends on the whole scope.
        for &n in &children {
            let Some(name) = field_text(n, "name", self.src).map(str::to_string) else { continue };
            match n.kind() {
                "mod_item" => {
                    let Some(inner) = n.child_by_field_name("body") else { continue };
                    let nested = [inline, &[name]].concat();
                    self.items.modules.insert(nested.join("/"));
                    self.scope(inner, &nested);
                }
                "trait_item" => {
                    let suffix = scoped(inline, &name);
                    let id = self.declare(n, &file_id, &suffix, exported(n));
                    self.items.scopes.entry(key.clone()).or_default().insert(name);
                    if let Some(b) = n.child_by_field_name("body") {
                        self.members(b, &id, &suffix, false);
                    }
                }
                k if ITEMS.contains(&k) => {
                    let suffix = scoped(inline, &name);
                    self.declare(n, &file_id, &suffix, exported(n));
                    if k == "macro_definition" {
                        self.items.macros.insert(name.clone(), suffix);
                    }
                    self.items.scopes.entry(key.clone()).or_default().insert(name);
                }
                _ => {}
            }
        }
        for &n in children.iter().filter(|n| n.kind() == "impl_item") {
            let ty = n.child_by_field_name("type").map(|t| type_path(t, self.src)).unwrap_or_default();
            let Some(last) = ty.last().cloned() else { continue };
            let container = scoped(inline, &last);
            let local = ty.len() == 1 && self.items.scopes.get(&key).is_some_and(|s| s.contains(&last));
            // A type declared elsewhere has no node here to hang its members off; the file declares them.
            let from = if local { format!("sym:{}::{container}", self.rel) } else { file_id.clone() };
            let members = n.child_by_field_name("body").map(|b| self.members(b, &from, &container, !local)).unwrap_or_default();
            let trait_path = n.child_by_field_name("trait").map(|t| type_path(t, self.src)).filter(|p| !p.is_empty());
            self.items.impls.push(Impl { inline: inline.to_vec(), ty, trait_path, members });
        }
    }

    fn members(&mut self, body: Node, from: &str, container: &str, foreign: bool) -> Vec<String> {
        let mut out = Vec::new();
        for m in named(body).into_iter().filter(|m| MEMBERS.contains(&m.kind())) {
            let Some(name) = field_text(m, "name", self.src) else { continue };
            let suffix = format!("{container}.{name}");
            // A member under its type is reached through the type; one the file declares is the file's export.
            self.declare(m, from, &suffix, foreign && exported(m));
            out.push(suffix);
        }
        out
    }

    fn declare(&mut self, n: Node, from: &str, suffix: &str, export: bool) -> String {
        let id = format!("sym:{}::{suffix}", self.rel);
        let line = n.child_by_field_name("name").unwrap_or(n).start_position().row as u32 + 1;
        let end = n.end_position().row as u32 + 1;
        let gate = cfg_gated(n, self.src).then(|| n.parent().map_or(0, |p| p.id()));
        if !self.items.names.insert(suffix.to_string()) {
            // Only siblings both gated by `#[cfg]` are platform twins of one item, so the first's node
            // grows to cover the second and a hunk in either is a change to the one symbol. Any other
            // repeat (`fmt` in a Display and a Debug impl) is a different item under a shared id:
            // first wins, and its span must not swallow the code between them.
            if gate.is_some() && self.gated.get(suffix) == gate.as_ref() {
                if let Some(first) = self.ex.nodes.iter_mut().find(|x| x.id == id) {
                    first.end = first.end.max(end);
                }
            }
            return id;
        }
        if let Some(parent) = gate {
            self.gated.insert(suffix.to_string(), parent);
        }
        // The label reads as Rust writes the path; the id keeps `/` between inline modules.
        let label = suffix.replace('/', "::");
        self.ex.node_span(NodeKind::Symbol, &id, &label, &body(n, self.src), self.rel, (line, end));
        self.ex.edge(from, &id, EdgeKind::Declares, if export { "export" } else { "" }, self.rel);
        id
    }
}

/// What BM25 indexes for a symbol: its doc comment and signature, never the block, so a long body
/// cannot drown the name.
fn body(n: Node, src: &[u8]) -> String {
    let head_end = n.child_by_field_name("body").map_or(n.end_byte(), |b| b.start_byte());
    let head = String::from_utf8_lossy(&src[n.start_byte()..head_end]);
    let signature = head.split_whitespace().collect::<Vec<_>>().join(" ");
    let doc = doc(n, src);
    let full = if doc.is_empty() { signature } else { format!("{doc}\n{signature}") };
    full.chars().take(BODY_CHARS).collect()
}

/// Whether an attribute directly above the item is `#[cfg(...)]`.
fn cfg_gated(n: Node, src: &[u8]) -> bool {
    let mut at = n;
    while let Some(prev) = at.prev_named_sibling() {
        match prev.kind() {
            "attribute_item" if text(prev, src).replace(' ', "").starts_with("#[cfg(") => return true,
            "attribute_item" | "line_comment" => at = prev,
            _ => return false,
        }
    }
    false
}

/// The `///` lines directly above the item, read through any attributes between them and it.
fn doc(n: Node, src: &[u8]) -> String {
    let mut lines = Vec::new();
    let mut at = n;
    while let Some(prev) = at.prev_named_sibling() {
        match prev.kind() {
            "attribute_item" => {}
            "line_comment" if text(prev, src).starts_with("///") => lines.push(text(prev, src).trim_start_matches('/').trim().to_string()),
            _ => break,
        }
        at = prev;
    }
    lines.reverse();
    lines.join("\n")
}

/// The symbol a node sits in: the outermost item around it, with the member of a trait or `impl`
/// as `Container.m`. Only items at a declaring scope are symbols, so a `fn` inside a `fn` is
/// climbed through rather than named.
pub(crate) fn owner(n: Node, rel: &str, src: &[u8]) -> String {
    let mut chain = Vec::new();
    let mut at = n;
    while let Some(p) = at.parent() {
        chain.push(p);
        at = p;
    }
    chain.reverse();
    let mut inline: Vec<String> = Vec::new();
    for (i, p) in chain.iter().enumerate() {
        match p.kind() {
            "mod_item" => match field_text(*p, "name", src) {
                Some(m) => inline.push(m.to_string()),
                None => break,
            },
            "impl_item" | "trait_item" => {
                let container = if p.kind() == "trait_item" {
                    field_text(*p, "name", src).map(str::to_string)
                } else {
                    p.child_by_field_name("type").and_then(|t| type_path(t, src).pop())
                };
                let Some(container) = container.map(|c| scoped(&inline, &c)) else { break };
                // `chain[i + 1]` is the body; the member, when there is one, is the link after it.
                let member = chain.get(i + 2).filter(|m| MEMBERS.contains(&m.kind())).and_then(|m| field_text(*m, "name", src));
                return match member {
                    Some(m) => format!("sym:{rel}::{container}.{m}"),
                    None => format!("sym:{rel}::{container}"),
                };
            }
            k if ITEMS.contains(&k) => {
                if let Some(name) = field_text(*p, "name", src) {
                    return format!("sym:{rel}::{}", scoped(&inline, name));
                }
            }
            _ => {}
        }
    }
    format!("file:{rel}")
}

/// Requirement ids cited in comments and string literals, as `References` from their owner —
/// the same edge TypeScript's `idrefs::scan` writes, over Rust's comment and string kinds.
pub(crate) fn id_refs(rel: &str, src: &[u8], root: Node, ex: &mut Extraction) {
    let written: BTreeSet<String> = ex.nodes.iter().map(|n| n.id.clone()).collect();
    descend(root, &mut |n| {
        let context = match n.kind() {
            "line_comment" | "block_comment" => "comment",
            "string_literal" | "raw_string_literal" => "string",
            _ => return true,
        };
        if !crate::ids::generic().find_all(text(n, src)).is_empty() {
            let mut from = owner(n, rel, src);
            // An `impl` of a type declared in another file writes no node for it, and an edge must start from a written one.
            if !from.starts_with("file:") && !written.contains(&from) {
                from = format!("file:{rel}");
            }
            cite(n, &from, context, rel, src, ex);
        }
        false
    });
}
