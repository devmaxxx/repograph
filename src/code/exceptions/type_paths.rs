//! `extension Outer<T>.Inner` extends `Outer.Inner`: a type path names a type without its generic
//! arguments, and without the space a writer may put around its dots. An operator's name is left
//! whole, since `<` and `>` are what it is spelled with.

use std::borrow::Cow;

pub(crate) fn fold(name: &str) -> Cow<'_, str> {
    let path = name.starts_with(|c: char| c.is_alphanumeric() || c == '_' || c == '`');
    if !path || !name.contains(|c: char| c == '<' || c.is_whitespace()) {
        return Cow::Borrowed(name);
    }
    let mut depth = 0usize;
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth > 0 || c.is_whitespace() => {}
            _ => out.push(c),
        }
    }
    Cow::Owned(out)
}
