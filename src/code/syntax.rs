//! Tree-sitter node helpers every language's walk uses. Nothing here knows a grammar's node kinds
//! beyond the field name `name`, and nothing writes to the graph.

use tree_sitter::Node;

pub(crate) fn text<'a>(n: Node, src: &'a [u8]) -> &'a str {
    n.utf8_text(src).unwrap_or("")
}

pub(crate) fn named<'t>(n: Node<'t>) -> Vec<Node<'t>> {
    let mut c = n.walk();
    n.named_children(&mut c).collect()
}

pub(crate) fn child<'t>(n: Node<'t>, kind: &str) -> Option<Node<'t>> {
    named(n).into_iter().find(|c| c.kind() == kind)
}

pub(crate) fn field_text<'a>(n: Node, field: &str, src: &'a [u8]) -> Option<&'a str> {
    n.child_by_field_name(field).map(|c| text(c, src))
}

pub(crate) fn name_of(n: Node, src: &[u8]) -> Option<String> {
    field_text(n, "name", src).map(str::to_string)
}

/// The outermost node below the root spanning exactly `range`: a definition's node, found again from
/// the range a hook is handed.
pub(crate) fn spanning<'t>(root: Node<'t>, range: &std::ops::Range<usize>) -> Option<Node<'t>> {
    let mut n = root.descendant_for_byte_range(range.start, range.end)?;
    if n.byte_range() != *range {
        return None;
    }
    while let Some(p) = n.parent().filter(|p| p.byte_range() == *range && p.parent().is_some()) {
        n = p;
    }
    Some(n)
}

/// Whether `n` sits in a region the grammar could not read, or holds one: a name there may be
/// one the file declares in a shape the walk never sees, such as a type parameter.
pub(crate) fn broken(n: Node) -> bool {
    n.has_error() || std::iter::successors(n.parent(), |p| p.parent()).any(|p| p.is_error())
}

/// Visits every named descendant of `n` in source order, and goes below one only when `visit` returns true.
/// It walks with a cursor and no recursion: a generated grammar nests a left-recursive list as deep as it is
/// long, so a 50,000-row `INSERT … VALUES` is 50,000 levels and overflows the stack of a recursive walk.
pub(crate) fn descend<'t>(n: Node<'t>, visit: &mut impl FnMut(Node<'t>) -> bool) {
    let mut cursor = n.walk();
    if !cursor.goto_first_child() {
        return;
    }
    loop {
        let node = cursor.node();
        if node.is_named() && visit(node) && cursor.goto_first_child() {
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return;
            }
        }
    }
}

/// The last row holding the node's text. A comment that swallows its newline ends at column 0 of the
/// next row, and counting that row would join the comment to a declaration a blank line below it.
pub(crate) fn last_row(n: Node) -> usize {
    let (start, end) = (n.start_position(), n.end_position());
    if end.column == 0 && end.row > start.row { end.row - 1 } else { end.row }
}

/// Like `span`, but ending on `last_row`: the span the line-oriented grammars (Shell, Bicep, HCL)
/// give a declaration whose node swallows the newline after it.
pub(crate) fn text_span(n: Node) -> (u32, u32) {
    (n.start_position().row as u32 + 1, last_row(n) as u32 + 1)
}

/// A doc comment is capped so a class's essay does not drown the signature terms that make the
/// symbol findable by name: BM25 documents are `id + label + body`.
pub(crate) const DOC_CHARS: usize = 600;

/// Comment text without its markers: the `/** … */` fences, a leading `*` per line, `//`.
pub(crate) fn comment_text(raw: &str) -> String {
    let inner = raw.trim().trim_start_matches("/**").trim_start_matches("/*").trim_end_matches("*/");
    inner.lines()
        .map(|l| l.trim().trim_start_matches("//").trim_start_matches('*').trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>().join("\n")
}

pub(crate) fn cap(s: String, n: usize) -> String { s.chars().take(n).collect() }

/// Where the declaration beside the decorator holding byte `at` starts, and the kind of node the two
/// sit in: a decorator is said of that declaration, though it sits outside the declaration's node.
/// A cursor walks down to `at`, since a node's `parent` walks down from the root on every call.
pub(crate) fn decorated<'t>(root: Node<'t>, at: usize) -> Option<(&'t str, usize)> {
    let mut cur = root.walk();
    let mut container = root.kind();
    while cur.goto_first_child_for_byte(at).is_some() && cur.node().start_byte() <= at {
        if cur.node().kind() == "decorator" {
            while cur.goto_next_sibling() {
                let s = cur.node();
                if s.is_named() && !matches!(s.kind(), "decorator" | "comment") {
                    return Some((container, s.start_byte()));
                }
            }
            return None;
        }
        container = cur.node().kind();
    }
    None
}
