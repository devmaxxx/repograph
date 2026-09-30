//! What the Shell, Bicep and HCL walks share once a tree is parsed: the declaration that owns a byte,
//! the body the index reads for a declaration, and the ids cited in comments and strings. None of it
//! resolves a name; each family's module does that.

use crate::model::{EdgeKind, Extraction, NodeKind};
use tree_sitter::Node;

/// Longer doc comments are cut: the body feeds the lexical index, where a licence header pasted above
/// the first declaration would outweigh every name the file holds.
const DOC_CAP: usize = 600;

pub(crate) fn text<'s>(n: Node, src: &'s [u8]) -> &'s str {
    n.utf8_text(src).unwrap_or("")
}

pub(crate) fn named<'t>(n: Node<'t>) -> Vec<Node<'t>> {
    let mut c = n.walk();
    n.named_children(&mut c).collect()
}

/// Every descendant of one of `kinds`, in source order, not descending into a match: a grandchild belongs
/// to its own parent, so the caller reads it from there.
pub(crate) fn find<'t>(n: Node<'t>, kinds: &[&str]) -> Vec<Node<'t>> {
    let mut out = Vec::new();
    let mut stack = named(n);
    while let Some(x) = stack.pop() {
        if kinds.contains(&x.kind()) {
            out.push(x);
        } else {
            stack.extend(named(x));
        }
    }
    out.sort_by_key(|x| x.start_byte());
    out
}

/// The last row holding the node's text. A comment that swallows its newline ends at column 0 of the
/// next row, and counting that row would join the comment to a declaration a blank line below it.
pub(crate) fn last_row(n: Node) -> usize {
    let (start, end) = (n.start_position(), n.end_position());
    if end.column == 0 && end.row > start.row { end.row - 1 } else { end.row }
}

/// 1-based first and last line, the span `changes` compares a hunk against.
pub(crate) fn span(n: Node) -> (u32, u32) {
    (n.start_position().row as u32 + 1, last_row(n) as u32 + 1)
}

/// Declarations by byte range. The innermost one holding a byte owns it; outside them all, the file does.
pub(crate) struct Spans {
    file: String,
    spans: Vec<(usize, usize, String)>,
}

impl Spans {
    pub(crate) fn new(rel: &str) -> Spans {
        Spans { file: format!("file:{rel}"), spans: Vec::new() }
    }

    pub(crate) fn push(&mut self, n: Node, id: &str) {
        self.spans.push((n.start_byte(), n.end_byte(), id.to_string()));
    }

    pub(crate) fn owner(&self, byte: usize) -> &str {
        self.spans.iter()
            .filter(|(start, end, _)| *start <= byte && byte < *end)
            .min_by_key(|(start, end, _)| end - start)
            .map_or(self.file.as_str(), |(_, _, id)| id.as_str())
    }
}

/// The body the index reads: the comment lines directly above the declaration, then its first line.
/// `between` names sibling kinds that may stand between the two, as Bicep's decorators do.
pub(crate) fn body(n: Node, src: &[u8], between: &[&str]) -> String {
    let mut top = n;
    while let Some(p) = top.prev_named_sibling() {
        if !between.contains(&p.kind()) || last_row(p) + 1 != top.start_position().row { break }
        top = p;
    }
    let mut doc = Vec::new();
    let mut at = top;
    while let Some(p) = at.prev_named_sibling() {
        // A shebang is the interpreter line, not documentation of the function under it.
        if p.kind() != "comment" || text(p, src).starts_with("#!") || last_row(p) + 1 != at.start_position().row { break }
        doc.push(text(p, src).trim_end());
        at = p;
    }
    doc.reverse();
    let mut out = doc.join("\n");
    if out.len() > DOC_CAP {
        let mut cut = DOC_CAP;
        while !out.is_char_boundary(cut) { cut -= 1 }
        out.truncate(cut);
    }
    let first = text(n, src).lines().next().unwrap_or("");
    if out.is_empty() { first.to_string() } else { format!("{out}\n{first}") }
}

/// A symbol spanning `n`, declared by `parent`. The label is the id's name as `impact` splits it, at the first `::`, so what
/// `explain` prints is what `impact` was asked for.
pub(crate) fn declare(ex: &mut Extraction, rel: &str, parent: &str, id: &str, n: Node, body: &str, context: &str) {
    let label = id.split_once("::").map_or(id, |(_, name)| name);
    ex.node_span(NodeKind::Symbol, id, label, body, rel, span(n));
    ex.edge(parent, id, EdgeKind::Declares, context, rel);
}

/// Ids cited in comments and in the given string kinds, each from the declaration holding it: the edge
/// the TypeScript scan writes, so an ADR named above a Terraform resource reaches the resource.
pub(crate) fn cite(root: Node, src: &[u8], rel: &str, strings: &[&str], spans: &Spans, ex: &mut Extraction) {
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let context = match n.kind() {
            "comment" => "comment",
            k if strings.contains(&k) => "string",
            _ => {
                stack.extend(named(n));
                continue;
            }
        };
        for hit in crate::ids::generic().find_all(text(n, src)) {
            ex.edge(spans.owner(n.start_byte()), &hit.id, EdgeKind::References, context, rel);
        }
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use crate::code::imports::Resolver;
    use crate::config::Config;
    use crate::model::{EdgeKind, Extraction};

    /// `files` on disk and a resolver over them with `globs` as the only code globs, because none of
    /// these languages is read by default until its readings pass.
    pub(crate) fn repo(globs: &[&str], files: &[(&str, &str)]) -> (tempfile::TempDir, Resolver) {
        let dir = tempfile::tempdir().unwrap();
        for (rel, text) in files {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let cfg = Config { code_globs: globs.iter().map(|g| g.to_string()).collect(), ..Config::default() };
        let resolver = Resolver::new(dir.path(), &cfg).unwrap();
        (dir, resolver)
    }

    /// Edges of one kind as `source -> target [context]`, sorted and deduplicated as the dispatcher
    /// leaves them, so an assertion reads like the graph.
    pub(crate) fn lines(ex: &Extraction, kind: EdgeKind) -> Vec<String> {
        let mut out: Vec<String> = ex.edges.iter().filter(|e| e.kind == kind)
            .map(|e| format!("{} -> {} [{}]", e.source, e.target, e.context)).collect();
        out.sort();
        out.dedup();
        out
    }

    pub(crate) fn ids(ex: &Extraction) -> Vec<String> {
        let mut out: Vec<String> = ex.nodes.iter().map(|n| n.id.clone()).collect();
        out.sort();
        out.dedup();
        out
    }
}
