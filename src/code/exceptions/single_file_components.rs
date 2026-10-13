//! A Vue single-file component is one symbol, its file's stem, that spans the file and that a
//! default import of the file names. Its `<script>` blocks are TypeScript, or TSX when one says
//! `lang="tsx"`, read at the file's own rows and columns with every other byte blanked, and read
//! as a `.ts` file is: what they declare is the file's, and what their bindings reach the
//! component reaches. A `<script setup>` block's top-level bindings are the component's instance
//! state, so the component references each.

use std::collections::BTreeSet;
use std::ops::Range;

use crate::code::engine::{self, Facts, File, Sink};
use crate::code::exceptions::{import_bindings, statement_prose};
use crate::code::imports::Resolver;
use crate::code::lang::Lang;
use crate::code::queries;
use crate::code::spec::{self, Embedded, Hooks};
use crate::model::{EdgeKind, NodeKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Script {
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
pub(crate) fn script_blocks(src: &str) -> Vec<Script> {
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
pub(crate) fn blanked(src: &str) -> Option<(String, Lang)> {
    let scripts = script_blocks(src);
    if scripts.is_empty() {
        return None;
    }
    let keep: Vec<Range<usize>> = scripts.iter().map(|s| s.content.clone()).collect();
    let lang = if scripts.iter().any(|s| s.tsx) { Lang::Tsx } else { Lang::TypeScript };
    Some((crate::code::blank::keep_ranges(src, &keep), lang))
}

/// The symbol a `.vue` file's component is: its file stem, `sym:<rel>::<FileStem>`. An SFC's one
/// default export is that component, so a default import of the file names it, whatever local
/// name the importer binds. None for any other path.
pub(crate) fn component_name(rel: &str) -> Option<&str> {
    let path = rel.strip_suffix(".vue")?;
    Some(path.rsplit('/').next().unwrap_or(path))
}

/// Every `.vue` file the walk globbed: a TypeScript import of one is an edge to a node only then.
#[derive(Default)]
pub(crate) struct Files(BTreeSet<String>);

impl Files {
    pub(crate) fn contains(&self, rel: &str) -> bool {
        self.0.contains(rel)
    }
}

/// The walk records a `.vue` path without opening the file: the extractor's read is its only one.
pub(crate) const HOOKS: Hooks = Hooks {
    register: Some(|r, rel| {
        r.state_mut::<Files>().0.insert(rel.to_string());
    }),
    state: Some(spec::state::<Files>),
    embed: Some(|source| blanked(source).map_or(Embedded::Empty, |(view, lang)| Embedded::View(view.into(), lang))),
    head: Some(statement_prose::head),
    resolve: Some(resolve),
    ..spec::NONE
};

/// The component first, so its whole-file span survives the dedup when the script's class has the
/// file's name.
fn resolve(resolver: &Resolver, file: &File, _: &Facts, sink: &mut Sink) {
    let (rel, source) = (file.rel, file.text);
    let stem = component_name(rel).unwrap_or(rel);
    let component = format!("sym:{rel}::{stem}");
    let last = source.lines().count().max(1) as u32;
    let signature = source.lines().map(str::trim).find(|l| l.starts_with("export default")).unwrap_or(stem);
    sink.node(NodeKind::Symbol, &component, stem, signature, rel, (1, last));
    let scripts = script_blocks(source);
    let view = if scripts.iter().any(|s| s.tsx) { Lang::Tsx } else { Lang::TypeScript };
    if let (Some(_), Some(decl)) = (file.tree, queries::decl(view)) {
        let script = File { lang: view, ..*file };
        let facts = engine::facts(spec::spec(view), decl, queries::refs(view), &script);
        sink.facts(&script, &facts);
        import_bindings::bind(resolver, &script, &facts, Some(&component), sink);
        let line_of = |byte: usize| source[..byte].matches('\n').count() as u32 + 1;
        let setup: Vec<(u32, u32)> = scripts.iter().filter(|s| s.setup).map(|s| (line_of(s.content.start), line_of(s.content.end))).collect();
        let bindings: BTreeSet<&str> = facts.defs.iter()
            .filter(|d| d.parent.is_none() && !d.qualified.contains('.') && d.id != component)
            .filter(|d| setup.iter().any(|(a, b)| *a <= d.line && d.line <= *b))
            .map(|d| d.id.as_str())
            .collect();
        for id in bindings {
            sink.edge(&component, id, EdgeKind::References, "", rel);
        }
    }
    sink.edge(&format!("file:{rel}"), &component, EdgeKind::Declares, "export", rel);
}
