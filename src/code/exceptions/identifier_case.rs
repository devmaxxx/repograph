//! An unquoted identifier folds to lower case and a quoted one keeps its case, `""` standing for a
//! quote inside it: `"Status"` is `Status` and `Status` is `status`. A query captures an identifier
//! as written, quotes included, and a quote is not something a query can take off.

use std::borrow::Cow;

pub(crate) fn fold(raw: &str) -> Cow<'_, str> {
    let raw = raw.trim();
    match raw.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        Some(inner) if inner.contains("\"\"") => Cow::Owned(inner.replace("\"\"", "\"")),
        Some(inner) => Cow::Borrowed(inner),
        None if raw.chars().any(char::is_uppercase) => Cow::Owned(raw.to_lowercase()),
        None => Cow::Borrowed(raw),
    }
}
