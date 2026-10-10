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

/// Whether `n` sits in a region the grammar could not read, or holds one: a name there may be
/// one the file declares in a shape the walk never sees, such as a type parameter.
pub(crate) fn broken(n: Node) -> bool {
    n.has_error() || std::iter::successors(n.parent(), |p| p.parent()).any(|p| p.is_error())
}

/// Every descendant of one of `kinds`, in source order, not descending into a match: a grandchild
/// belongs to its own parent, so the caller reads it from there.
pub(crate) fn find<'t>(n: Node<'t>, kinds: &[&str], out: &mut Vec<Node<'t>>) {
    descend(n, &mut |c| {
        let hit = kinds.contains(&c.kind());
        if hit {
            out.push(c);
        }
        !hit
    });
}

pub(crate) fn find_all<'t>(n: Node<'t>, kinds: &[&str]) -> Vec<Node<'t>> {
    let mut out = Vec::new();
    find(n, kinds, &mut out);
    out
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

/// 1-based first and last line of the node's extent. A declaration's extent includes its
/// annotations and attribute lists, so a hunk that edits only `[HttpGet("x")]` still lands inside
/// the member it changes.
pub(crate) fn span(n: Node) -> (u32, u32) {
    (n.start_position().row as u32 + 1, n.end_position().row as u32 + 1)
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
