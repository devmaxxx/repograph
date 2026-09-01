use crate::model::{EdgeKind, Extraction};
use regex::Regex;
use std::sync::OnceLock;

fn link_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[[^\]]*\]\(([^)\s#]+)(?:#[^)]*)?\)").unwrap())
}

/// Resolve `target` against the directory of `rel`, collapsing `.` and `..`.
pub fn normalise(rel: &str, target: &str) -> String {
    let mut parts: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        rel.rsplit_once('/').map(|(d, _)| d.split('/').filter(|s| !s.is_empty()).collect()).unwrap_or_default()
    };
    for seg in target.trim_start_matches('/').split('/') {
        match seg {
            "" | "." => {}
            ".." => { parts.pop(); }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

pub fn scan(rel: &str, text: &str, ex: &mut Extraction) {
    for c in link_re().captures_iter(text) {
        let t = &c[1];
        if t.contains("://") || t.starts_with("mailto:") {
            continue;
        }
        ex.edge(&format!("file:{rel}"), &format!("file:{}", normalise(rel, t)), EdgeKind::Links, "", rel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn links(rel: &str, text: &str) -> Vec<(String, String)> {
        let mut ex = Extraction::default();
        scan(rel, text, &mut ex);
        ex.edges.iter().filter(|e| e.kind == EdgeKind::Links).map(|e| (e.source.clone(), e.target.clone())).collect()
    }

    #[test]
    fn relative_links_normalise_against_the_file_directory() {
        let l = links("docs/prd/03-calendar.md", "see [payments](06-payments.md#fr-pay-22) and [adr](../adr/ADR-002-hosting.md)");
        assert_eq!(l, vec![
            ("file:docs/prd/03-calendar.md".into(), "file:docs/prd/06-payments.md".into()),
            ("file:docs/prd/03-calendar.md".into(), "file:docs/adr/ADR-002-hosting.md".into()),
        ]);
    }

    #[test]
    fn urls_and_anchors_are_ignored() {
        assert!(links("a.md", "[x](https://example.com) [y](#local) [z](mailto:a@b.c)").is_empty());
    }

    #[test]
    fn root_relative_links_keep_their_path() {
        assert_eq!(links("docs/a.md", "[c](/docs/constitution.yaml)"), vec![("file:docs/a.md".into(), "file:docs/constitution.yaml".into())]);
    }
}
