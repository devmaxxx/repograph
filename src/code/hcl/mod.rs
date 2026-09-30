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
use crate::model::Extraction;
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
        prose::declare(&mut ex, rel, &file, &id, n, &prose::body(n, src, &[]), context);
        spans.push(n, &id);
        own.insert(address);
    }
    if terraform {
        references::terraform(resolver.hcl(), root, &own, &spans, src, rel, &mut ex);
    } else {
        references::bake(root, &own, &spans, src, rel, &mut ex);
    }
    prose::cite(root, src, rel, &["string_lit", "quoted_template", "heredoc_template"], &spans, &mut ex);
    ex
}

/// Every block this file declares, by address, with the node its span comes from. In Terraform,
/// `resource "T" "N"` is `T/N`, `data "T" "N"` is `data/T/N`, `variable` is `var/`, each `locals`
/// attribute is `local/`, and `output`, `module` and `provider` keep their keyword. In other HCL, the
/// address is the type and its labels.
pub(crate) fn declarations<'t>(root: Node<'t>, src: &[u8], terraform: bool) -> Vec<(String, Node<'t>)> {
    let Some(body) = prose::named(root).into_iter().find(|c| c.kind() == "body") else { return Vec::new() };
    let mut out = Vec::new();
    for block in prose::named(body).into_iter().filter(|c| c.kind() == "block") {
        let parts = inner(block);
        let Some(kind) = parts.first().filter(|c| c.kind() == "identifier").map(|c| prose::text(*c, src)) else { continue };
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
                let attributes = parts.iter().filter(|c| c.kind() == "body").flat_map(|b| prose::named(*b)).filter(|a| a.kind() == "attribute");
                for attribute in attributes {
                    if let Some(name) = attribute.named_child(0) {
                        out.push((format!("local/{}", prose::text(name, src)), attribute));
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
        "identifier" => prose::text(n, src),
        _ => match inner(n).as_slice() {
            [t] if t.kind() == "template_literal" => prose::text(*t, src),
            _ => return None,
        },
    };
    (!text.is_empty() && !text.contains('/')).then_some(text)
}

/// Named children less the delimiters the grammar names, such as `block_start`,
/// `quoted_template_end` and `tuple_start`.
pub(crate) fn inner<'t>(n: Node<'t>) -> Vec<Node<'t>> {
    prose::named(n).into_iter().filter(|c| !c.kind().ends_with("_start") && !c.kind().ends_with("_end")).collect()
}
