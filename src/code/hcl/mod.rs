//! HCL. A `.tf` file is Terraform: a block is an address, written with `/` where Terraform writes `.`,
//! and a module is a directory, so a reference resolves against every `.tf` file beside the one
//! spelling it. Any other `.hcl` file, `docker-bake.hcl` among them, is read by block type and labels.

mod references;
#[cfg(test)]
mod cases;

pub(crate) use references::Modules;

use crate::code::imports::Resolver;
use crate::code::lang::{file_node, Lang};
use crate::code::prose::{self, Spans};
use crate::code::syntax;
use crate::model::{EdgeKind, Extraction, Graph};
use std::collections::BTreeSet;
use tree_sitter::Node;

pub fn extract(resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let mut ex = Extraction::default();
    file_node(rel, &mut ex);
    let src = source.as_bytes();
    let Some(tree) = Lang::Hcl.parse(src) else { return ex };
    let root = tree.root_node();
    let terraform = rel.ends_with(".tf");
    let file = format!("file:{rel}");
    let mut spans = Spans::new(rel);
    let mut own = BTreeSet::new();
    for (address, n) in declarations(root, src, terraform) {
        let id = format!("sym:{rel}::{address}");
        // A variable and an output are a Terraform module's interface; every bake block is invoked by
        // name from outside its file.
        let context = if !terraform || address.starts_with("var/") || address.starts_with("output/") { "export" } else { "" };
        spans.push(n, &id);
        // Aliased `provider "aws"` blocks share one address; the first block is the symbol.
        if own.insert(address) {
            prose::declare(&mut ex, rel, &file, &id, n, &prose::body(n, src, &[]), context);
        }
    }
    if terraform {
        references::terraform(resolver.hcl(), root, &own, &spans, src, rel, &mut ex);
    } else {
        references::bake(root, &own, &spans, src, rel, &mut ex);
    }
    prose::cite(root, src, rel, &["string_lit", "quoted_template", "heredoc_template"], &spans, &mut ex);
    ex
}

/// `apply_diff`'s widening for Terraform. A module is a directory, so a `.tf` file that changes or goes
/// can add, move or drop an address that every sibling reads, and a module call reads the outputs and
/// variables of the directory it names: both the siblings and the files calling the directory are read
/// again. Other `.hcl` files declare nothing a neighbour resolves against, so they widen nothing.
pub(crate) fn widen(stale: &[String], removed: &[String], graph: &Graph, all_rels: &[String]) -> Vec<String> {
    let dirs: BTreeSet<&str> = stale.iter().chain(removed)
        .filter(|r| r.ends_with(".tf"))
        .map(|r| dir_of(r))
        .collect();
    if dirs.is_empty() {
        return Vec::new();
    }
    let importers = graph.edges.iter()
        .filter(|e| e.kind == EdgeKind::Imports && e.source.ends_with(".tf"))
        .filter(|e| e.target.strip_prefix("file:").is_some_and(|t| t.ends_with(".tf") && dirs.contains(dir_of(t))))
        .filter_map(|e| e.source.strip_prefix("file:"));
    let siblings = all_rels.iter().map(String::as_str).filter(|r| r.ends_with(".tf") && dirs.contains(dir_of(r)));
    let reread: BTreeSet<&str> = importers.chain(siblings).collect();
    let stale: BTreeSet<&str> = stale.iter().map(String::as_str).collect();
    let present: BTreeSet<&str> = all_rels.iter().map(String::as_str).collect();
    reread.into_iter()
        .filter(|r| !stale.contains(r) && present.contains(r))
        .map(str::to_string)
        .collect()
}

pub(super) fn dir_of(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(d, _)| d)
}

/// Every block this file declares, by address, with the node its span comes from. In Terraform,
/// `resource "T" "N"` is `T/N`, `data "T" "N"` is `data/T/N`, `variable` is `var/`, each `locals`
/// attribute is `local/`, and `output`, `module` and `provider` keep their keyword. In other HCL, the
/// address is the type and its labels.
pub(crate) fn declarations<'t>(root: Node<'t>, src: &[u8], terraform: bool) -> Vec<(String, Node<'t>)> {
    let Some(body) = syntax::named(root).into_iter().find(|c| c.kind() == "body") else { return Vec::new() };
    let mut out = Vec::new();
    for block in syntax::named(body).into_iter().filter(|c| c.kind() == "block") {
        let parts = inner(block);
        let Some(kind) = parts.first().filter(|c| c.kind() == "identifier").map(|c| syntax::text(*c, src)) else { continue };
        // A label that is not plain text, or that holds a `/` as a lock file's registry paths do, names no
        // address this id scheme can hold.
        let labels: Option<Vec<&str>> = parts[1..].iter()
            .take_while(|c| matches!(c.kind(), "string_lit" | "identifier"))
            .map(|c| label(*c, src))
            .collect();
        let Some(labels) = labels else { continue };
        let address = match (terraform, kind, labels.as_slice()) {
            (true, "resource", [t, n]) => format!("{t}/{n}"),
            (true, "data", [t, n]) => format!("data/{t}/{n}"),
            (true, "variable", [n]) => format!("var/{n}"),
            (true, "output" | "module" | "provider", [n]) => format!("{kind}/{n}"),
            (true, "locals", []) => {
                let attributes = parts.iter().filter(|c| c.kind() == "body").flat_map(|b| syntax::named(*b)).filter(|a| a.kind() == "attribute");
                for attribute in attributes {
                    if let Some(name) = attribute.named_child(0) {
                        out.push((format!("local/{}", syntax::text(name, src)), attribute));
                    }
                }
                continue;
            }
            // `terraform {}`, `moved {}`, `import {}`: blocks no reference names.
            (true, _, _) | (false, _, []) => continue,
            (false, _, labels) => format!("{kind}/{}", labels.join("/")),
        };
        out.push((address, block));
    }
    out
}

/// A label's text: a bare identifier, or a string holding written-out text alone.
fn label<'s>(n: Node, src: &'s [u8]) -> Option<&'s str> {
    let text = match n.kind() {
        "identifier" => syntax::text(n, src),
        _ => match inner(n).as_slice() {
            [t] if t.kind() == "template_literal" => syntax::text(*t, src),
            _ => return None,
        },
    };
    (!text.is_empty() && !text.contains('/')).then_some(text)
}

/// Named children less the delimiters the grammar names, such as `block_start`,
/// `quoted_template_end` and `tuple_start`.
pub(crate) fn inner<'t>(n: Node<'t>) -> Vec<Node<'t>> {
    syntax::named(n).into_iter().filter(|c| !c.kind().ends_with("_start") && !c.kind().ends_with("_end")).collect()
}
