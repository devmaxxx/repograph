//! A Razor file's `@code` and `@functions` blocks as one C# compilation unit (L6). Every byte outside
//! the blocks is a space and every newline stays, so a row and column in the copy are the row and
//! column in the file; the blank bytes before the first block hold a wrapper class that the C# walk
//! reads as the component. The order the wrapper is placed in is documented on `view`.

use std::ops::Range;

use crate::code::blank::{keep_ranges, overwrite};

#[cfg_attr(not(test), expect(dead_code))]
// Read by the Razor extractor; `expect` flags this once it is.
#[derive(Debug, PartialEq, Eq)]
pub enum View {
    /// The file holds no block.
    None,
    /// A block does not close, or no bytes hold the wrapper: the blocks are not read.
    Unread(&'static str),
    Read(String),
}

/// Statements Razor lets a block open with `@`; C# would read `@if` as a verbatim identifier.
const TRANSITIONS: [&str; 9] = ["if", "foreach", "for", "while", "switch", "do", "try", "lock", "using"];

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn line_after(b: &[u8], from: usize) -> usize {
    b[from..].iter().position(|&c| c == b'\n').map_or(b.len(), |p| from + p + 1)
}

fn find(s: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    s.get(from..)?.windows(needle.len()).position(|w| w == needle).map(|p| from + p)
}

/// The byte after `@* ... *@`, Razor's own comment syntax; it does not nest and is not C#, so a
/// `{` or `}` inside one — or an `@code`/`@functions` a commented-out block opens with — is not read.
fn razor_comment_end(s: &[u8], i: usize) -> Option<usize> {
    find(s, b"*@", i + 2).map(|p| p + 2)
}

/// The nearest `@*` in `s[from..to]` that actually opens a Razor comment: `@@*` (Razor's own
/// escape for a literal `@`) and `a@*b` (an identifier byte right before the `@`, as in
/// email-style text) are text, not a transition into one, so a match preceded by either is
/// skipped in favor of the next.
fn razor_comment_open(s: &[u8], from: usize, to: usize) -> Option<usize> {
    let mut at = from;
    loop {
        let p = find(&s[..to], b"@*", at)?;
        if p > 0 && (s[p - 1] == b'@' || is_ident(s[p - 1])) {
            at = p + 1;
            continue;
        }
        return Some(p);
    }
}

/// Visual Studio writes it at the start of a `.razor` by default. It is not whitespace to the
/// scanner, so a first-line `@code` or directive would not start its line.
pub(crate) const BOM: char = '\u{FEFF}';

/// (byte of `@`, byte of the opening `{`, byte of the closing `}`) per block, in file order. A
/// directive opens a block only at the start of a line, as Razor requires; the first line starts
/// after a leading BOM, and every offset stays one into `src`.
#[cfg_attr(not(test), expect(dead_code))]
// Read by the Razor extractor; `expect` flags this once it is.
pub fn blocks(src: &str) -> Result<Vec<(usize, usize, usize)>, &'static str> {
    scan(src).map(|(found, _)| found)
}

/// A block's (byte of `@`, byte of `{`, byte of `}`), and a Razor comment's (byte of `@`, byte
/// after `*@`).
type Scanned = (Vec<(usize, usize, usize)>, Vec<(usize, usize)>);

/// `blocks`, plus every `@* ... *@` span crossed while balancing a block's braces (`close_of`
/// already steps over strings and comments to find them, so a naive text search does not have to).
fn scan(src: &str) -> Result<Scanned, &'static str> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut razor_comments = Vec::new();
    let mut line = if src.starts_with(BOM) { BOM.len_utf8() } else { 0 };
    while line < b.len() {
        let mut at = line;
        while at < b.len() && (b[at] == b' ' || b[at] == b'\t') {
            at += 1;
        }
        let word = ["@code", "@functions"].into_iter()
            .find(|w| b[at..].starts_with(w.as_bytes()) && !b.get(at + w.len()).copied().is_some_and(is_ident));
        let Some(word) = word else {
            // No directive on this line. A Razor comment may still open here — anywhere on the
            // line, not only at its start, since markup can precede it — and it may hold a
            // `@code`/`@functions` of its own on a later line; skip the whole comment, wherever
            // it closes, and resume scanning right there rather than at the next line, so a
            // directive right after it on the same line (`@* x *@ @code {`) is still found.
            let eol = b[line..].iter().position(|&c| c == b'\n').map_or(b.len(), |p| line + p);
            if let Some(p) = razor_comment_open(b, line, eol) {
                // Silently giving up here would read the rest of the file — including a real
                // block past this point — as `None`; surfacing the failure lets the census see
                // the file instead of losing it.
                line = razor_comment_end(b, p).ok_or("a razor comment that does not close")?;
                continue;
            }
            line = line_after(b, line);
            continue;
        };
        let mut open = at + word.len();
        while open < b.len() && b[open].is_ascii_whitespace() {
            open += 1;
        }
        if b.get(open) != Some(&b'{') {
            line = line_after(b, line);
            continue;
        }
        let close = close_of(b, open, &mut razor_comments).ok_or("a block that does not close")?;
        out.push((at, open, close));
        // Not `line_after(b, close)`: a comment can open right after the block's own `}` on the
        // same line, and jumping straight to the next line would step over it unseen.
        line = close + 1;
    }
    Ok((out, razor_comments))
}

/// The `}` balancing the `{` at `open`, stepping over comments, strings and character literals.
/// A `@* ... *@` crossed at the top level — never inside one of those, which are jumped over
/// whole — is Razor's own comment, not C#; its span is recorded in `razor_comments` so `view` can
/// blank exactly that text, rather than a second, string-blind search finding one inside a string
/// or a real C# comment and pairing it with an unrelated `*@` far away.
fn close_of(s: &[u8], open: usize, razor_comments: &mut Vec<(usize, usize)>) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = open;
    while i < s.len() {
        match s[i] {
            b'@' if s.get(i + 1) == Some(&b'*') => {
                let end = razor_comment_end(s, i)?;
                razor_comments.push((i, end));
                i = end;
                continue;
            }
            b'/' if s.get(i + 1) == Some(&b'/') => {
                i = line_after(s, i);
                continue;
            }
            b'/' if s.get(i + 1) == Some(&b'*') => {
                i = find(s, b"*/", i + 2)? + 2;
                continue;
            }
            b'$' | b'@' | b'"' => {
                if let Some(end) = string_end(s, i, razor_comments) {
                    i = end;
                    continue;
                }
                // A failed `$"` cannot have been a string past its own line; re-scanning its bytes
                // would let a quote inside a hole open a fresh string and hide a comment in it. A
                // bare `"` in markup is text — an inch mark — so it keeps the byte-by-byte retry.
                if interpolated_opener(s, i) {
                    i = line_after(s, i);
                    continue;
                }
            }
            b'\'' => {
                if let Some(len) = char_literal_len(s, i) {
                    i += len;
                    continue;
                }
            }
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Whether `s[i..]` opens a non-verbatim interpolated string: a `$` run, no `@`, then a quote.
fn interpolated_opener(s: &[u8], i: usize) -> bool {
    let mut j = i;
    let (mut interpolated, mut verbatim) = (false, false);
    while j < s.len() && (s[j] == b'$' || s[j] == b'@') {
        interpolated |= s[j] == b'$';
        verbatim |= s[j] == b'@';
        j += 1;
    }
    interpolated && !verbatim && s.get(j) == Some(&b'"')
}

fn string_end(s: &[u8], i: usize, razor_comments: &mut Vec<(usize, usize)>) -> Option<usize> {
    // An interpolation hole can cross a `@* ... *@` before the attempt as a whole turns out not
    // to be a string; abandoning it must not leave that span recorded, since the byte at `i` is
    // then read as ordinary text rather than as the string that would have held it.
    let before = razor_comments.len();
    let end = string_end_at(s, i, razor_comments);
    if end.is_none() {
        razor_comments.truncate(before);
    }
    end
}

/// The byte after the string literal starting at `i`, or None when none starts there. A regular or
/// interpolated literal that meets a newline is markup text, not a literal.
fn string_end_at(s: &[u8], i: usize, razor_comments: &mut Vec<(usize, usize)>) -> Option<usize> {
    let mut j = i;
    let (mut interpolated, mut verbatim) = (false, false);
    while j < s.len() && (s[j] == b'$' || s[j] == b'@') {
        interpolated |= s[j] == b'$';
        verbatim |= s[j] == b'@';
        j += 1;
    }
    if s.get(j) != Some(&b'"') {
        return None;
    }
    let quotes = s[j..].iter().take_while(|&&c| c == b'"').count();
    // A verbatim string escapes a quote by doubling it, so three in a row can be the start of a
    // verbatim literal (`@"""a}"` holds `"a}`), not a raw string's fence.
    if !verbatim && quotes >= 3 {
        let fence = &s[j..j + quotes];
        return find(s, fence, j + quotes).map(|k| k + quotes);
    }
    j += 1;
    while j < s.len() {
        match s[j] {
            b'\\' if !verbatim => j += 2,
            b'"' if verbatim && s.get(j + 1) == Some(&b'"') => j += 2,
            b'"' => return Some(j + 1),
            b'\n' if !verbatim => return None,
            b'{' if interpolated && s.get(j + 1) == Some(&b'{') => j += 2,
            b'{' if interpolated => j = close_of(s, j, razor_comments)? + 1,
            _ => j += 1,
        }
    }
    None
}

/// The length of the character literal at `i`: `'x'`, `'ä'`, `'\''`, `'A'`.
fn char_literal_len(s: &[u8], i: usize) -> Option<usize> {
    let first = *s.get(i + 1)?;
    let close = if first == b'\\' {
        (i + 3..(i + 12).min(s.len())).find(|&j| s[j] == b'\'')?
    } else {
        let width = match first {
            0..=0x7F => 1,
            0xC0..=0xDF => 2,
            0xE0..=0xEF => 3,
            _ => 4,
        };
        i + 1 + width
    };
    (s.get(close) == Some(&b'\'')).then_some(close + 1 - i)
}

fn transitions(src: &str, range: Range<usize>, out: &mut String) {
    let b = src.as_bytes();
    for i in range.clone() {
        if b[i] != b'@' || (i > 0 && is_ident(b[i - 1])) {
            continue;
        }
        let rest = &b[i + 1..range.end];
        if TRANSITIONS.iter().any(|k| rest.starts_with(k.as_bytes()) && !rest.get(k.len()).copied().is_some_and(is_ident)) {
            overwrite(out, i, " ");
        }
    }
}

/// The start of the nearest line above `at`'s line holding `need` bytes.
fn earlier_line(src: &str, at: usize, need: usize) -> Option<usize> {
    let b = src.as_bytes();
    let mut end = b[..at].iter().rposition(|&c| c == b'\n')?;
    loop {
        let start = b[..end].iter().rposition(|&c| c == b'\n').map_or(0, |p| p + 1);
        if end - start >= need {
            return Some(start);
        }
        if start == 0 {
            return None;
        }
        end = start - 1;
    }
}

/// The first line inside a block whose brace ends its own line, when that line starts with two
/// blank bytes and no line before it holds code.
fn indentation(src: &str, from: usize, to: usize) -> Option<usize> {
    let b = &src.as_bytes()[..to];
    let eol = b[from..].iter().position(|&c| c == b'\n')? + from;
    if !b[from..eol].iter().all(|c| c.is_ascii_whitespace()) {
        return None;
    }
    let mut line = eol + 1;
    while line < b.len() {
        let end = b[line..].iter().position(|&c| c == b'\n').map_or(b.len(), |p| line + p);
        // A CRLF blank line's content is `\r`, not empty; drop it so a blank line reads the same
        // as it would in an LF file, rather than looking like a line that opens with code.
        let content_end = if end > line && b[end - 1] == b'\r' { end - 1 } else { end };
        let blank = b[line..content_end].iter().take_while(|&&c| c == b' ' || c == b'\t').count();
        if blank >= 2 {
            return Some(line);
        }
        if content_end > line + blank {
            return None;
        }
        line = end + 1;
    }
    None
}

/// The file's blocks as one C# class `C`. `@code {` is 6 bytes before its brace and `class C` is 7,
/// so the wrapper goes into the directive only when it is long enough (`@functions {`); otherwise
/// `class` goes on the nearest line above with room and `C` over the `@`; otherwise, for a block on
/// the first line, into the block's own indentation.
#[cfg_attr(not(test), expect(dead_code))]
// Read by the Razor extractor; `expect` flags this once it is.
pub fn view(src: &str) -> View {
    let (found, razor_comments) = match scan(src) {
        Ok(fc) => fc,
        Err(why) => return View::Unread(why),
    };
    let (Some(&(at, open, _)), Some(&(_, _, close))) = (found.first(), found.last()) else { return View::None };
    let keep: Vec<Range<usize>> = found.iter().map(|&(_, o, c)| o + 1..c).collect();
    let mut out = keep_ranges(src, &keep);
    for r in &keep {
        transitions(src, r.clone(), &mut out);
    }
    // Every span close_of found is inside some block's own o+1..c, so it is already kept verbatim;
    // blank it here, in file order, the same way a transition is: spaces, with its newlines kept.
    let b = src.as_bytes();
    for (start, end) in razor_comments {
        let blanked: String = b[start..end].iter().map(|&c| if c == b'\n' { '\n' } else { ' ' }).collect();
        overwrite(&mut out, start, &blanked);
    }
    overwrite(&mut out, open, "{");
    overwrite(&mut out, close, "}");
    let directive = &src[at..open];
    if directive.len() >= 7 && !directive.contains('\n') {
        overwrite(&mut out, at, "class C");
    } else if let Some(line) = earlier_line(src, at, 5) {
        overwrite(&mut out, line, "class");
        overwrite(&mut out, at, "C");
    } else if let Some(indent) = indentation(src, open + 1, close) {
        overwrite(&mut out, at, "class");
        overwrite(&mut out, open, " ");
        overwrite(&mut out, indent, "C{");
    } else {
        return View::Unread("no blank bytes hold the wrapper");
    }
    View::Read(out)
}
