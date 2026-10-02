#[cfg(test)]
mod cases;

use std::collections::BTreeSet;
use std::ops::Range;

use crate::code::imports::Resolver;
use crate::code::lang::{file_node, Lang};
use crate::model::{EdgeKind, Extraction, NodeKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    /// The bytes between `<script …>` and `</script>`.
    pub content: Range<usize>,
    pub tsx: bool,
    pub setup: bool,
}

/// The `>` closing the tag opened at `at`, stepping over quoted attribute values.
fn tag_end(src: &str, at: usize) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (k, b) in src.as_bytes()[at..].iter().enumerate() {
        match (quote, *b) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, b'"' | b'\'') => quote = Some(*b),
            (None, b'>') => return Some(at + k),
            _ => {}
        }
    }
    None
}

/// The `</template` that closes a top-level template, counting the `<template v-if>` inside it and
/// stepping over HTML comments, where a `<template>` opens nothing.
fn template_end(src: &str, from: usize) -> Option<usize> {
    let (mut depth, mut i) = (1, from);
    loop {
        let close = src[i..].find("</template").map(|c| i + c)?;
        let comment = src[i..].find("<!--").map(|c| i + c).filter(|c| *c < close);
        let open = src[i..].find("<template").map(|o| i + o).filter(|o| *o < close);
        match (comment, open) {
            (Some(c), o) if o.is_none_or(|o| c < o) => {
                i = src[c..].find("-->").map(|e| c + e + 3)?;
            }
            (_, Some(open)) => {
                depth += 1;
                i = open + "<template".len();
            }
            _ => {
                depth -= 1;
                if depth == 0 {
                    return Some(close);
                }
                i = close + "</template".len();
            }
        }
    }
}

fn attr(attrs: &str, key: &str) -> Option<String> {
    let re = regex::Regex::new(&format!(r#"(?:^|\s){key}\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#)).ok()?;
    let c = re.captures(attrs)?;
    c.get(1).or_else(|| c.get(2)).or_else(|| c.get(3)).map(|m| m.as_str().to_string())
}

fn has_attr(attrs: &str, key: &str) -> bool {
    attrs.split_whitespace().any(|a| a == key || a.starts_with(&format!("{key}=")))
}

/// The `<script>` blocks among a single-file component's top-level elements. Each top-level element
/// ends at its own closing tag, so a `<script>` in template text or in a comment is not a block.
pub fn script_blocks(src: &str) -> Vec<Script> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(off) = src[i..].find('<') {
        let at = i + off;
        if src[at..].starts_with("<!--") {
            i = src[at..].find("-->").map_or(src.len(), |e| at + e + 3);
            continue;
        }
        let name: String = src[at + 1..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        if name.is_empty() {
            i = at + 1;
            continue;
        }
        let Some(open_end) = tag_end(src, at) else { break };
        let attrs = &src[at + 1 + name.len()..open_end];
        if attrs.trim_end().ends_with('/') {
            i = open_end + 1;
            continue;
        }
        let body = open_end + 1;
        let end = if name == "template" {
            template_end(src, body)
        } else {
            src[body..].find(&format!("</{name}")).map(|e| body + e)
        };
        let Some(end) = end else { break };
        if name == "script" {
            out.push(Script { content: body..end, tsx: attr(attrs, "lang").as_deref() == Some("tsx"), setup: has_attr(attrs, "setup") });
        }
        i = src[end..].find('>').map_or(src.len(), |e| end + e + 1);
    }
    out
}

/// The file with only its script blocks kept, each at its own rows and columns (L6), and the grammar
/// that reads them. None when the component has no script.
pub fn blanked(src: &str) -> Option<(String, Lang)> {
    let scripts = script_blocks(src);
    if scripts.is_empty() {
        return None;
    }
    let keep: Vec<Range<usize>> = scripts.iter().map(|s| s.content.clone()).collect();
    let lang = if scripts.iter().any(|s| s.tsx) { Lang::Tsx } else { Lang::TypeScript };
    Some((crate::code::blank::keep_ranges(src, &keep), lang))
}

/// L8: a `deco:<Name>` shared by every decorated component would chain them all through `apply_diff`'s
/// co-declared closure, so the edge goes to the declaration the name resolves to, or nowhere.
fn decorators_to_declarations(rel: &str, ex: &mut Extraction) {
    let file_id = format!("file:{rel}");
    let prefix = format!("sym:{rel}::");
    let own: BTreeSet<String> = ex.nodes.iter()
        .filter_map(|n| n.id.strip_prefix(&prefix))
        .filter(|n| !n.contains('.'))
        .map(str::to_string)
        .collect();
    let imported: Vec<(String, String)> = ex.edges.iter()
        .filter(|e| e.kind == EdgeKind::Imports && e.source == file_id)
        .flat_map(|e| {
            let target = e.target.trim_start_matches("file:").to_string();
            e.context.split(',').filter(|n| !n.is_empty()).map(move |n| (n.to_string(), target.clone())).collect::<Vec<_>>()
        })
        .collect();
    ex.nodes.retain(|n| !n.id.starts_with("deco:"));
    for mut e in std::mem::take(&mut ex.edges) {
        if let Some(name) = e.target.strip_prefix("deco:").map(str::to_string) {
            let target = if own.contains(&name) {
                Some(format!("sym:{rel}::{name}"))
            } else {
                imported.iter().find(|(n, _)| *n == name).map(|(_, f)| format!("sym:{f}::{name}"))
            };
            let Some(target) = target else { continue };
            e.target = target;
        }
        ex.edges.push(e);
    }
}

/// The symbol a `.vue` file's component is: its file stem, `sym:<rel>::<FileStem>`. An SFC's one
/// default export is that component, so a default import of the file names it, whatever local
/// name the importer binds. None for any other path.
pub(crate) fn component_name(rel: &str) -> Option<&str> {
    let path = rel.strip_suffix(".vue")?;
    Some(path.rsplit('/').next().unwrap_or(path))
}

pub fn extract(resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let stem = component_name(rel).unwrap_or(rel);
    let component = format!("sym:{rel}::{stem}");
    let last = source.lines().count().max(1) as u32;
    let signature = source.lines().map(str::trim).find(|l| l.starts_with("export default")).unwrap_or(stem).to_string();
    let mut ex = Extraction::default();
    // First, so the whole-file span survives the dedup when the script's class has the file's name.
    ex.node_span(NodeKind::Symbol, &component, stem, &signature, rel, (1, last));
    let tree = blanked(source).and_then(|(blank, lang)| lang.parse(blank.as_bytes()).map(|t| (blank, t)));
    match tree {
        None => file_node(rel, &mut ex),
        Some((blank, tree)) => {
            let src = blank.as_bytes();
            let root = tree.root_node();
            let walked = crate::code::symbols::Walk { resolver }.scan_tree(rel, root, src);
            ex.nodes.extend(walked.nodes);
            ex.edges.extend(walked.edges);
            let prefix = format!("sym:{rel}::");
            let locals: BTreeSet<String> = ex.nodes.iter()
                .filter_map(|n| n.id.strip_prefix(&prefix))
                .filter(|n| !n.contains('.'))
                .map(str::to_string)
                .collect();
            crate::code::calls::scan_tree(resolver, rel, root, src, &locals, &mut ex);
            crate::code::idrefs::scan_tree(root, rel, src, &mut ex);
            decorators_to_declarations(rel, &mut ex);
            owned_by_the_component(source, rel, &component, &mut ex);
        }
    }
    ex.edge(&format!("file:{rel}"), &component, EdgeKind::Declares, "export", rel);
    ex
}

/// The component spans the file, so what the walk leaves on `file:<rel>` is the component's: an
/// Options-API object (`export default defineComponent({…})`) declares nothing the walk names, and its
/// calls land on the file. A `<script setup>` block's top-level bindings are the component's instance
/// state, so the component references each; L11 then walks from the component to what they call.
fn owned_by_the_component(source: &str, rel: &str, component: &str, ex: &mut Extraction) {
    let file_id = format!("file:{rel}");
    for e in ex.edges.iter_mut() {
        let walked = e.kind == EdgeKind::Calls || (e.kind == EdgeKind::References && e.target.starts_with("sym:"));
        if walked && e.source == file_id && e.target != component {
            e.source = component.to_string();
        }
    }
    let line_of = |byte: usize| source[..byte].matches('\n').count() as u32 + 1;
    let setup: Vec<(u32, u32)> = script_blocks(source).into_iter()
        .filter(|s| s.setup)
        .map(|s| (line_of(s.content.start), line_of(s.content.end)))
        .collect();
    let prefix = format!("sym:{rel}::");
    let bindings: Vec<String> = ex.nodes.iter()
        .filter(|n| n.id != component && n.id.strip_prefix(&prefix).is_some_and(|name| !name.contains('.')))
        .filter(|n| setup.iter().any(|(a, b)| *a <= n.line && n.line <= *b))
        .map(|n| n.id.clone())
        .collect();
    for id in bindings {
        ex.edge(component, &id, EdgeKind::References, "", rel);
    }
}
