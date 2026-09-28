//! PostgreSQL names as ids. An unquoted identifier folds to lower case and a quoted one keeps its case, and
//! a schema-qualified name joins its parts with `/`, which leaves `.` to mean a member.
use tree_sitter::Node;

/// One identifier as PostgreSQL resolves it: `"Status"` is `Status`, `Status` is `status`.
pub(super) fn fold(raw: &str) -> String {
    let raw = raw.trim();
    match raw.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        Some(inner) => inner.replace("\"\"", "\""),
        None => raw.to_lowercase(),
    }
}

/// A name node's parts joined with `/`. `qualified_name`, `any_name`, `func_name`, `name` and `ColId` all spell
/// a part as a `ColId`, a `ColLabel` or, for an unqualified function, a `type_function_name`.
pub(super) fn object(n: Node, src: &[u8]) -> Option<String> {
    let mut parts = Vec::new();
    collect(n, src, &mut parts);
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn collect(n: Node, src: &[u8], out: &mut Vec<String>) {
    if matches!(n.kind(), "ColId" | "ColLabel" | "type_function_name") {
        out.push(fold(n.utf8_text(src).unwrap_or("")));
        return;
    }
    let mut c = n.walk();
    for k in n.named_children(&mut c) {
        collect(k, src, out);
    }
}

/// The last part, the name a person asks for: `app/clients` is `clients`.
pub(super) fn bare(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

/// The names a written reference may stand for, in order. `search_path` is session state no file records, so an
/// unqualified name tries itself and then `public`, a `public/` name tries its unqualified twin, and nothing
/// else is guessed.
pub(super) fn candidates(name: &str) -> Vec<String> {
    match name.strip_prefix("public/") {
        Some(rest) => vec![name.to_string(), rest.to_string()],
        None if !name.contains('/') => vec![name.to_string(), format!("public/{name}")],
        None => vec![name.to_string()],
    }
}
