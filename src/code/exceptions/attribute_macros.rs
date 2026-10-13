//! A macro written where C++ takes an attribute — between a class key and the class's name, or after
//! a declarator — expands to an attribute or to nothing: an export (`class LIB_EXPORT Status`) or a
//! thread-safety annotation (`int n_ GUARDED_BY(mu_);`, `void f() LOCKS_EXCLUDED(mu_) {`). The
//! grammar cannot expand it and reads the class as a function named by the macro, or the member as a
//! prototype of the macro, so the file is read with those macros blanked, every row and column kept.
//! A macro that stands alone as a member before an access label or a friend (`Q_OBJECT`) goes too:
//! the grammar reads it as the type of a member the next keyword names.
//!
//! A macro is a word of capitals, digits and underscores. After a declarator name it is blanked only
//! with an argument list that holds no two adjacent words, and only before `;`, `=` or `,`: a
//! parameter list always holds a type beside a name, so `Status MAKE_IT(int n);` stays a prototype.

use std::ops::Range;

use crate::code::blank::keep_ranges;
use crate::code::lang::Lang;
use crate::code::spec::{self, Embedded, Hooks};

pub(crate) const HOOKS: Hooks = Hooks { embed: Some(view), ..spec::NONE };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tok {
    Word,
    Punct(u8),
}

/// The file's code tokens: words and punctuation, never inside a comment, a literal or a
/// preprocessor line, where a capitalised word is no annotation.
fn tokens(src: &str) -> Vec<(Tok, Range<usize>)> {
    let b = src.as_bytes();
    let (mut out, mut i, mut line_start) = (Vec::new(), 0, true);
    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            line_start = true;
            i += 1;
            continue;
        }
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if line_start && c == b'#' {
            // A directive runs to the first line end no backslash continues.
            while i < b.len() && !(b[i] == b'\n' && b[i - 1] != b'\\') {
                i += 1;
            }
            continue;
        }
        line_start = false;
        if b[i..].starts_with(b"//") {
            i = src[i..].find('\n').map_or(b.len(), |n| i + n);
        } else if b[i..].starts_with(b"/*") {
            i = src[i + 2..].find("*/").map_or(b.len(), |n| i + 2 + n + 2);
        } else if c == b'R' && b.get(i + 1) == Some(&b'"') {
            let open = src[i + 2..].find('(').map_or(b.len(), |n| i + 2 + n);
            let close = format!("){}\"", src.get(i + 2..open).unwrap_or(""));
            i = src.get(open..).and_then(|rest| rest.find(&close)).map_or(b.len(), |n| open + n + close.len());
        } else if c == b'"' || c == b'\'' {
            let mut j = i + 1;
            while j < b.len() && b[j] != c && b[j] != b'\n' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            i = j + 1;
        } else if c.is_ascii_alphanumeric() || c == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            // A number is a word too: it never reads as a macro, and it stops a run as one does.
            out.push((Tok::Word, start..i));
        } else {
            out.push((Tok::Punct(c), i..i + 1));
            i += 1;
        }
    }
    out
}

fn is_macro(word: &str) -> bool {
    word.len() > 1
        && word.starts_with(|c: char| c.is_ascii_uppercase())
        && word.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
}

const KEYWORDS: &[&str] = &[
    "return", "case", "else", "throw", "new", "delete", "sizeof", "goto", "co_return", "co_yield", "co_await", "typedef",
    "using", "namespace", "operator", "class", "struct", "union", "enum", "public", "private", "protected", "virtual",
    "static", "inline", "explicit", "friend", "template", "typename", "const", "constexpr", "volatile", "mutable",
    "extern", "override", "final", "noexcept", "decltype", "default",
];

const CONTROL: &[&str] = &["if", "while", "for", "switch", "catch", "return", "sizeof", "alignof", "decltype", "noexcept"];

/// The view, or the file as it is when no macro sits where an attribute goes.
fn view(src: &str) -> Embedded {
    let toks = tokens(src);
    let word = |k: usize| toks.get(k).filter(|(t, _)| *t == Tok::Word).map(|(_, r)| &src[r.clone()]);
    let punct = |k: usize| toks.get(k).and_then(|(t, _)| match t { Tok::Punct(c) => Some(*c), Tok::Word => None });
    // The token past a balanced argument list opening at `k`, and whether the list holds two
    // adjacent words, as a parameter list does.
    let past_args = |k: usize| -> Option<(usize, bool)> {
        let (mut depth, mut paired, mut j) = (0usize, false, k);
        while j < toks.len() {
            match toks[j].0 {
                Tok::Punct(b'(') => depth += 1,
                Tok::Punct(b')') => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((j + 1, paired));
                    }
                }
                Tok::Word if j > k && toks[j - 1].0 == Tok::Word => paired = true,
                Tok::Punct(b';' | b'{' | b'}') => return None,
                _ => {}
            }
            j += 1;
        }
        None
    };
    // A macro at `k` with its arguments, if any: (the token after it, the bytes it spans, paired).
    let annotation = |k: usize| -> Option<(usize, Range<usize>, bool)> {
        word(k).filter(|w| is_macro(w))?;
        let start = toks[k].1.start;
        match punct(k + 1) {
            Some(b'(') => {
                let (next, paired) = past_args(k + 1)?;
                Some((next, start..toks[next - 1].1.end, paired))
            }
            _ => Some((k + 1, toks[k].1.clone(), false)),
        }
    };
    let mut opener: Vec<Option<usize>> = vec![None; toks.len()];
    let mut open = Vec::new();
    for (k, (t, _)) in toks.iter().enumerate() {
        match t {
            Tok::Punct(b'(') => open.push(k),
            Tok::Punct(b')') => opener[k] = open.pop(),
            _ => {}
        }
    }
    let mut blank: Vec<Range<usize>> = Vec::new();
    for k in 0..toks.len() {
        // `{ MACRO public:` or `; MACRO friend`: a member that is a macro alone.
        let member_start = k == 0 || matches!(punct(k - 1), Some(b'{' | b';' | b'}' | b':'));
        if let Some((next, span, _)) = annotation(k).filter(|_| member_start) {
            if matches!(word(next), Some("public" | "private" | "protected" | "friend")) {
                blank.push(span);
                continue;
            }
        }
        // `class MACRO… Name`: the run of macros ends at the class's name.
        if matches!(word(k), Some("class" | "struct" | "union")) {
            let (mut at, mut run) = (k + 1, Vec::new());
            while let Some((next, span, _)) = annotation(at) {
                if word(next).is_none() {
                    break;
                }
                run.push(span);
                at = next;
            }
            if !run.is_empty() && word(at).is_some_and(|w| !KEYWORDS.contains(&w)) {
                blank.extend(run);
            }
            continue;
        }
        // After a parameter list or its qualifiers: `) MACRO(…) {`, `) const MACRO(…);`.
        // A parameter list's `(` follows the declarator's name; a condition's follows `if` or `while`.
        let params = |close: usize| opener[close].is_some_and(|open| open > 0 && word(open - 1).is_some_and(|w| !CONTROL.contains(&w)));
        let after_params = (punct(k) == Some(b')') && params(k)) || matches!(word(k), Some("const" | "override" | "final" | "noexcept"));
        // After a declarator name that follows a type: `int n_ MACRO(mu_);`.
        let after_name = word(k).is_some_and(|w| !KEYWORDS.contains(&w) && !is_macro(w))
            && k > 0
            && (word(k - 1).is_some() || matches!(punct(k - 1), Some(b'*' | b'&' | b'>' | b']')));
        if !after_params && !after_name {
            continue;
        }
        let (mut at, mut run) = (k + 1, Vec::new());
        while let Some((next, span, paired)) = annotation(at) {
            // Arguments with something inside them, none of it a type beside a name.
            let args = next > at + 3;
            if after_name && !after_params && (!args || paired) {
                break;
            }
            run.push(span);
            at = next;
        }
        let ends = match punct(at) {
            Some(b';' | b'=' | b',') => true,
            Some(b'{' | b':') => after_params,
            _ => after_params && matches!(word(at), Some("const" | "override" | "final" | "noexcept")),
        };
        if ends && !run.is_empty() {
            blank.extend(run);
        }
    }
    if blank.is_empty() {
        return Embedded::View(src.to_string(), Lang::Cpp);
    }
    blank.sort_by_key(|r| r.start);
    blank.dedup();
    let mut keep = Vec::with_capacity(blank.len() + 1);
    let mut at = 0;
    for r in &blank {
        if r.start >= at {
            keep.push(at..r.start);
            at = r.end;
        }
    }
    keep.push(at..src.len());
    Embedded::View(keep_ranges(src, &keep), Lang::Cpp)
}

#[cfg(test)]
mod tests {
    use super::view;
    use crate::code::spec::Embedded;

    fn read(src: &str) -> String {
        match view(src) {
            Embedded::View(v, _) => v,
            _ => unreachable!("the view is always the file's"),
        }
    }

    /// `src` with each of `parts` turned to spaces.
    fn blanked(src: &str, parts: &[&str]) -> String {
        parts.iter().fold(src.to_string(), |s, p| s.replacen(p, &" ".repeat(p.len()), 1))
    }

    #[test]
    fn an_export_macro_before_a_class_name_is_blanked() {
        for (src, parts) in [
            ("class LIB_EXPORT Status {\n};", &["LIB_EXPORT"][..]),
            ("struct LIB_EXPORT ALIGNED(8) Range : Base {};", &["LIB_EXPORT", "ALIGNED(8)"]),
            ("class SCOPED_LOCKABLE MutexLock final {};", &["SCOPED_LOCKABLE"]),
        ] {
            assert_eq!(read(src), blanked(src, parts));
        }
    }

    #[test]
    fn a_macro_alone_as_a_member_before_a_label_or_a_friend_is_blanked() {
        for src in ["class A : public QObject {\n    Q_OBJECT\n\n public:\n  A();\n};", "class A {\n  Q_OBJECT\n  friend void f(A*) {}\n};"] {
            assert_eq!(read(src), blanked(src, &["Q_OBJECT"]));
        }
    }

    #[test]
    fn a_capitalised_class_name_is_not_a_macro() {
        for src in ["class FOO : public Bar {};", "class FOO;", "class FOO {};", "template <class T> class X {};", "template <class KEY> X {};"] {
            assert_eq!(read(src), src);
        }
    }

    #[test]
    fn an_annotation_after_a_declarator_is_blanked() {
        for (src, part) in [
            ("  int n_ GUARDED_BY(mu_);", "GUARDED_BY(mu_)"),
            ("  Foo* p_ PT_GUARDED_BY(mu_) = nullptr;", "PT_GUARDED_BY(mu_)"),
            ("void f() LOCKS_EXCLUDED(mu_) {}", "LOCKS_EXCLUDED(mu_)"),
            ("void g()\n    EXCLUSIVE_LOCKS_REQUIRED(mu_);", "EXCLUSIVE_LOCKS_REQUIRED(mu_)"),
            ("void h() const NO_ANALYSIS {}", "NO_ANALYSIS"),
            ("Lock() LOCK_FUNCTION() : held_(true) {}", "LOCK_FUNCTION()"),
        ] {
            assert_eq!(read(src), blanked(src, &[part]));
        }
    }

    #[test]
    fn a_capitalised_function_a_call_or_a_condition_stays() {
        for src in [
            "static Status OK() { return Status(); }",
            "static Status OK();",
            "Status MAKE_IT(int n);",
            "virtual Status OK_TOO(const Slice& s) = 0;",
            "int main() { return RUN_ALL_TESTS(); }",
            "void t() { if (ready) ASSERT_OK(s); while (n) STEP(n); }",
            "x = CHECK(y);",
            "// class LIB_EXPORT Status {",
            "const char* s = \"class LIB_EXPORT Status {\";",
            "#define DECL class LIB_EXPORT Status {",
        ] {
            assert_eq!(read(src), src);
        }
    }
}
