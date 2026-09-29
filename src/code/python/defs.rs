//! What a Python file declares: symbols and their `Declares` edges, and the table of names the
//! reference pass resolves against.

use super::{field_text, text};
use crate::code::jvm::{cite, descend, named};
use crate::model::{EdgeKind, Extraction, NodeKind};
use std::collections::BTreeSet;
use tree_sitter::Node;

/// As TypeScript's doc cap: BM25 documents are `id + label + body`, and a longer body drowns the name.
const BODY_CHARS: usize = 600;

/// The names the file declares, for the reference pass to resolve against.
#[derive(Debug, Default)]
pub(crate) struct Defs {
    /// Every id suffix the file declares: `f`, `C`, `C.m`, `C.Inner.m`.
    pub names: BTreeSet<String>,
    /// The id suffixes that are classes.
    pub classes: BTreeSet<String>,
    /// The module's own top-level names.
    pub top: BTreeSet<String>,
}

struct Walk<'a> {
    rel: &'a str,
    src: &'a [u8],
    all: Option<BTreeSet<String>>,
    defs: Defs,
    ex: &'a mut Extraction,
}

pub(crate) fn read(rel: &str, src: &[u8], root: Node, ex: &mut Extraction) -> Defs {
    let mut walk = Walk { rel, src, all: dunder_all(root, src), defs: Defs::default(), ex };
    walk.scope(root, None);
    walk.defs
}

/// `__all__ = [...]` or `(...)` of string literals at module level; `None` when there is none,
/// or when it is computed, since then only the underscore rule says anything.
fn dunder_all(root: Node, src: &[u8]) -> Option<BTreeSet<String>> {
    named(root).into_iter().rev().find_map(|s| {
        let a = s.named_child(0).filter(|a| s.kind() == "expression_statement" && a.kind() == "assignment")?;
        a.child_by_field_name("left").filter(|l| text(*l, src) == "__all__")?;
        let right = a.child_by_field_name("right").filter(|r| matches!(r.kind(), "list" | "tuple"))?;
        named(right).into_iter().map(|i| (i.kind() == "string").then(|| string_content(i, src))).collect()
    })
}

fn string_content(s: Node, src: &[u8]) -> String {
    named(s).into_iter().filter(|p| p.kind() == "string_content").map(|p| text(p, src)).collect()
}

/// The first statement of a body when it is a bare string: the docstring.
fn docstring(body: Node, src: &[u8]) -> Option<String> {
    let first = body.named_child(0).filter(|s| s.kind() == "expression_statement")?;
    let s = first.named_child(0).filter(|s| s.kind() == "string")?;
    Some(string_content(s, src).trim().to_string())
}

impl Walk<'_> {
    fn scope(&mut self, body: Node, container: Option<&str>) {
        for outer in named(body) {
            let decl = if outer.kind() == "decorated_definition" { outer.child_by_field_name("definition") } else { Some(outer) };
            let Some(decl) = decl else { continue };
            match decl.kind() {
                "function_definition" | "class_definition" => {
                    let Some(name) = field_text(decl, "name", self.src) else { continue };
                    let suffix = container.map_or_else(|| name.to_string(), |c| format!("{c}.{name}"));
                    let block = decl.child_by_field_name("body");
                    let doc = block.and_then(|b| docstring(b, self.src));
                    let head_end = block.map_or(decl.end_byte(), |b| b.start_byte());
                    let header = String::from_utf8_lossy(&self.src[decl.start_byte()..head_end]).split_whitespace().collect::<Vec<_>>().join(" ");
                    let body = match doc {
                        Some(d) if !d.is_empty() => format!("{d}\n{header}"),
                        _ => header,
                    };
                    self.declare(decl, outer, container, &suffix, name, &body);
                    if decl.kind() == "class_definition" {
                        self.defs.classes.insert(suffix.clone());
                        if let Some(b) = block {
                            self.scope(b, Some(&suffix));
                        }
                    }
                }
                "expression_statement" => {
                    let Some(a) = decl.named_child(0).filter(|a| a.kind() == "assignment") else { continue };
                    let Some(name) = a.child_by_field_name("left").filter(|l| l.kind() == "identifier").map(|l| text(l, self.src)) else { continue };
                    let suffix = container.map_or_else(|| name.to_string(), |c| format!("{c}.{name}"));
                    let body = text(decl, self.src).lines().next().unwrap_or("").trim().to_string();
                    self.declare(a, decl, container, &suffix, name, &body);
                }
                _ => {}
            }
        }
    }

    /// `decl` carries the name; `outer` — a decorated definition, a statement — carries the extent.
    /// A name declared a second time (a property setter, a second `def`) shares
    /// the first one's id, so the first keeps its span and edge rather than stretching over the
    /// code between the two.
    fn declare(&mut self, decl: Node, outer: Node, container: Option<&str>, suffix: &str, name: &str, body: &str) {
        if !self.defs.names.insert(suffix.to_string()) {
            return;
        }
        let id = format!("sym:{}::{suffix}", self.rel);
        let name_node = decl.child_by_field_name("name").or_else(|| decl.child_by_field_name("left")).unwrap_or(decl);
        let line = name_node.start_position().row as u32 + 1;
        let end = outer.end_position().row as u32 + 1;
        let body: String = body.chars().take(BODY_CHARS).collect();
        self.ex.node_span(NodeKind::Symbol, &id, suffix, &body, self.rel, (line, end));
        let (from, context) = match container {
            Some(c) => (format!("sym:{}::{c}", self.rel), ""),
            None => {
                let exported = self.all.as_ref().map_or(!name.starts_with('_'), |all| all.contains(name));
                self.defs.top.insert(name.to_string());
                (format!("file:{}", self.rel), if exported { "export" } else { "" })
            }
        };
        self.ex.edge(&from, &id, EdgeKind::Declares, context, self.rel);
    }
}

/// The symbol a node sits in: the definition at a declaring scope around it — a module-level
/// or class-level `def`, `class` or plain assignment — else the file.
pub(crate) fn owner(n: Node, rel: &str, src: &[u8]) -> String {
    let mut chain: Vec<Node> = std::iter::successors(n.parent(), |p| p.parent()).collect();
    chain.reverse();
    let mut path: Vec<String> = Vec::new();
    for p in &chain {
        match p.kind() {
            "module" | "block" | "decorated_definition" | "expression_statement" => {}
            "class_definition" => match field_text(*p, "name", src) {
                Some(c) => path.push(c.to_string()),
                None => break,
            },
            "function_definition" => {
                path.extend(field_text(*p, "name", src).map(str::to_string));
                break;
            }
            "assignment" => {
                path.extend(p.child_by_field_name("left").filter(|l| l.kind() == "identifier").map(|l| text(l, src).to_string()));
                break;
            }
            // Anything else — an `if`, a `try`, a call — encloses what is not a symbol.
            _ => break,
        }
    }
    if path.is_empty() { format!("file:{rel}") } else { format!("sym:{rel}::{}", path.join(".")) }
}

/// Requirement ids cited in comments and strings, as `References` from their owner. A docstring
/// is documentation and says `comment`, as a TypeScript doc comment does; any other string says `string`.
pub(crate) fn id_refs(rel: &str, src: &[u8], root: Node, ex: &mut Extraction) {
    let written: BTreeSet<String> = ex.nodes.iter().map(|n| n.id.clone()).collect();
    descend(root, &mut |n| {
        let context = match n.kind() {
            "comment" => "comment",
            "string" if n.parent().is_some_and(|p| p.kind() == "expression_statement" && p.named_child_count() == 1) => "comment",
            "string" => "string",
            _ => return true,
        };
        if !crate::ids::generic().find_all(text(n, src)).is_empty() {
            let mut from = owner(n, rel, src);
            // A definition inside a redefinition can name a path no node was written for, and an edge must start from a written one.
            if !from.starts_with("file:") && !written.contains(&from) {
                from = format!("file:{rel}");
            }
            cite(n, &from, context, rel, src, ex);
        }
        false
    });
}
