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

/// Each outermost decorator of a tree, with where the declaration it stands beside starts and
/// whether the two sit in a class body: a decorator is said of that declaration, though it sits
/// outside the declaration's node. One walk reads them all, so a site asks a sorted list where a walk
/// down from the root scanned every level.
pub(crate) struct Decorators(Vec<(std::ops::Range<usize>, usize, bool)>);

impl Decorators {
    pub(crate) fn of(root: Node) -> Decorators {
        let mut found = Vec::new();
        // Most grammars have no decorator, and their files pay nothing for the walk.
        if root.language().id_for_node_kind("decorator", true) == 0 {
            return Decorators(found);
        }
        let mut cur = root.walk();
        if !cur.goto_first_child() {
            return Decorators(found);
        }
        loop {
            let n = cur.node();
            let decorator = n.kind() == "decorator";
            if decorator {
                let mut sibling = cur.clone();
                while sibling.goto_next_sibling() {
                    let s = sibling.node();
                    if s.is_named() && !matches!(s.kind(), "decorator" | "comment") {
                        let mut parent = cur.clone();
                        let member = parent.goto_parent() && parent.node().kind() == "class_body";
                        found.push((n.byte_range(), s.start_byte(), member));
                        break;
                    }
                }
            }
            if n.is_named() && !decorator && cur.goto_first_child() {
                continue;
            }
            while !cur.goto_next_sibling() {
                if !cur.goto_parent() {
                    return Decorators(found);
                }
            }
        }
    }

    /// Where a site at byte `at` is said: at the declaration when it sits in a decorator.
    pub(crate) fn said_at(&self, at: usize) -> usize {
        self.holding(at).map_or(at, |&(_, declaration, _)| declaration)
    }

    /// Like `said_at`, but only for a member's decorator. A class's own decorator sits outside the
    /// class, so what it hands over, such as a module's providers, the file hands over.
    pub(crate) fn member_at(&self, at: usize) -> usize {
        self.holding(at).filter(|(_, _, member)| *member).map_or(at, |&(_, declaration, _)| declaration)
    }

    fn holding(&self, at: usize) -> Option<&(std::ops::Range<usize>, usize, bool)> {
        let i = self.0.partition_point(|(range, _, _)| range.start <= at).checked_sub(1)?;
        Some(&self.0[i]).filter(|(range, _, _)| at < range.end)
    }
}

#[cfg(test)]
mod tests {
    use super::Decorators;
    use crate::code::lang::Lang;

    #[test]
    fn a_site_in_a_decorator_is_said_at_what_it_decorates_and_only_a_member_s_moves_for_member_at() {
        let src = "@Module({ a: f() })\nexport class M {\n  @Get({ b: g() })\n  // note\n  find() {}\n  plain() { h(); }\n}\n";
        let tree = Lang::TypeScript.parse(src.as_bytes()).unwrap();
        let d = Decorators::of(tree.root_node());
        let (f, g, h) = (src.find("f()").unwrap(), src.find("g()").unwrap(), src.find("h()").unwrap());
        let (class, method) = (src.find("class M").unwrap(), src.find("find()").unwrap());
        assert_eq!((d.said_at(f), d.member_at(f)), (class, f));
        assert_eq!((d.said_at(g), d.member_at(g)), (method, method), "the comment between is passed over");
        assert_eq!((d.said_at(h), d.member_at(h)), (h, h));
    }

    #[test]
    fn a_grammar_without_decorators_reads_none() {
        let src = "fn main() { f(); }\n";
        let tree = Lang::Rust.parse(src.as_bytes()).unwrap();
        assert_eq!(Decorators::of(tree.root_node()).0.len(), 0);
    }
}
