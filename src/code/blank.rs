//! Embedded languages read by blanking (L6): a copy of the file in which only the embedded code
//! keeps its bytes, so every row and column a grammar reports in the copy is the file's own, and
//! no offset table exists anywhere.

// Every caller is a family plan's (Razor, Vue); until one lands, the tests are the only reader.
#![allow(dead_code)]

use std::ops::Range;

/// Same length as `src`; every byte outside `keep` becomes b' ' except b'\n'.
///
/// Panics when a range splits a character: the copy must stay UTF-8, and a range cut mid-character
/// comes from a caller measuring in something other than bytes.
pub fn keep_ranges(src: &str, keep: &[Range<usize>]) -> String {
    let mut kept = vec![false; src.len()];
    for r in keep {
        assert!(
            r.start <= r.end && r.end <= src.len() && src.is_char_boundary(r.start) && src.is_char_boundary(r.end),
            "keep range {r:?} is not a character range of a {}-byte source",
            src.len()
        );
        kept[r.clone()].iter_mut().for_each(|k| *k = true);
    }
    let bytes: Vec<u8> = src.bytes().zip(kept).map(|(b, k)| if k || b == b'\n' { b } else { b' ' }).collect();
    // A multi-byte character is wholly inside a kept range or wholly outside, and one outside
    // becomes that many ASCII spaces, so the bytes are still UTF-8.
    String::from_utf8(bytes).expect("blanking whole characters keeps UTF-8")
}

/// Overwrites `buf[at..at + text.len()]` in place; panics if that would split a char or change length.
///
/// It also panics when the bytes replaced and `text` hold line breaks in different places: a
/// wrapper that moves a line break moves every row after it.
pub fn overwrite(buf: &mut String, at: usize, text: &str) {
    let end = at + text.len();
    assert!(
        end <= buf.len() && buf.is_char_boundary(at) && buf.is_char_boundary(end),
        "overwriting {at}..{end} of a {}-byte buffer would split a character or grow it",
        buf.len()
    );
    let breaks = |s: &str| s.bytes().enumerate().filter(|(_, b)| *b == b'\n').map(|(i, _)| i).collect::<Vec<_>>();
    assert!(breaks(&buf[at..end]) == breaks(text), "overwriting {at}..{end} would move a line break");
    buf.replace_range(at..end, text);
}

#[cfg(test)]
// A one-range slice is the exact shape a single embedded block passes; not a mistaken `vec!`.
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use super::*;

    fn position(s: &str, needle: &str) -> (usize, usize) {
        let at = s.find(needle).unwrap();
        (s[..at].matches('\n').count(), at - s[..at].rfind('\n').map_or(0, |i| i + 1))
    }

    #[test]
    fn a_kept_block_keeps_its_rows_and_columns_and_nothing_else_survives() {
        let src = "<h1>Заказы</h1>\n@code {\n    private int count;\n}\n<p>@count</p>\n";
        let start = src.find("private").unwrap();
        let end = src.find(';').unwrap() + 1;
        let out = keep_ranges(src, &[start..end]);
        assert_eq!(out.len(), src.len());
        assert_eq!(out.matches('\n').count(), src.matches('\n').count());
        assert_eq!(position(&out, "private int count;"), position(src, "private int count;"));
        assert_eq!(out.trim(), "private int count;");
    }

    #[test]
    fn a_blanked_multibyte_character_becomes_as_many_spaces_as_it_had_bytes() {
        assert_eq!(keep_ranges("é\nx", &[3..4]), "  \nx");
    }

    #[test]
    #[should_panic(expected = "is not a character range")]
    fn a_range_that_splits_a_character_panics() {
        keep_ranges("é", &[0..1]);
    }

    #[test]
    fn a_directive_is_overwritten_inside_its_own_bytes() {
        let mut buf = String::from("@functions {\n    void Go() {}\n}\n");
        overwrite(&mut buf, 0, "class F    {");
        assert_eq!(buf, "class F    {\n    void Go() {}\n}\n");
    }

    #[test]
    #[should_panic(expected = "would split a character or grow it")]
    fn an_overwrite_past_the_end_panics() {
        let mut buf = String::from("@code {");
        overwrite(&mut buf, 3, "class X {");
    }

    #[test]
    #[should_panic(expected = "would move a line break")]
    fn an_overwrite_that_moves_a_line_break_panics() {
        let mut buf = String::from("@code\n{");
        overwrite(&mut buf, 0, "class X");
    }
}
