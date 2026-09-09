use crate::ids::IdMatcher;
use crate::model::{EdgeKind, Extraction, NodeKind};
use regex::Regex;
use std::sync::OnceLock;

const BODY_CAP: usize = 40;

/// The two file-name conventions that declare a node whatever families the documents define: a
/// milestone plan is named for its milestone, an ADR for its number. Group 1 is the id the file
/// declares, group 2 the family it is written in — the derivation reads the family off the same
/// pattern the extractor reads the id off, so the two cannot drift apart.
pub(crate) fn milestone_file() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(&format!(r"(?:^|/)(({})-M\d{{2}})[^/]*\.md$", crate::families::MILESTONE)).unwrap())
}

pub(crate) fn adr_file() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:^|/)(ADR-\d{3,4})[^/]*\.md$").unwrap())
}

fn kind_for(id: &str) -> NodeKind {
    static MILESTONE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let ms = MILESTONE.get_or_init(|| Regex::new(r"^[A-Z]+-M\d{2}$").unwrap());
    if id.starts_with("INV-") { NodeKind::Invariant }
    else if id.starts_with("ADR-") { NodeKind::Adr }
    else if ms.is_match(id) { NodeKind::Milestone }
    else { NodeKind::Requirement }
}

pub struct RequirementScanner {
    ids: IdMatcher,
    head: Regex,
    entity: Regex,
    task: Regex,
}

impl RequirementScanner {
    pub fn new(ids: IdMatcher) -> RequirementScanner {
        let id = ids.single_pattern();
        RequirementScanner {
            ids,
            // The title stops where a bold head's `**` closes (136 heads on the bench corpus
            // carry references after it — that tail opens the body, so no word is lost to
            // retrieval) and before a modality hung off a dash (71 heads write
            // `· title — MUST` instead of `· MUST · title`).
            head: Regex::new(&format!(
                r"^(?:#{{1,6}}\s+|\*\*|\s*[-*]\s+\*\*)?({id})\s*·\s*(?:(MUST|SHOULD|LATER)\s*·\s*)?(.*?)\s*(?:[—–-]\s*(?:MUST|SHOULD|LATER))?\s*(?:\*\*(.*))?\s*\**\s*$"
            )).unwrap(),
            entity: Regex::new(r"`([A-Za-z][A-Za-z0-9_.]*)`").unwrap(),
            task: Regex::new(r"^\s*-\s+\[[ xX]\]\s+\*\*(T\d{2,3})\*\*\s*(.*)$").unwrap(),
        }
    }

    pub fn scan(&self, rel: &str, text: &str) -> Extraction {
        let mut ex = Extraction::default();
        let file_id = format!("file:{rel}");
        ex.node(NodeKind::File, &file_id, rel, "", rel, 1);

        // An editor's byte-order mark would otherwise hide the first head from `^`.
        let lines: Vec<&str> = text.trim_start_matches('\u{feff}').lines().collect();
        let mut heads: Vec<(usize, String, String, String)> = Vec::new();
        let mut fenced = false;
        for (i, line) in lines.iter().enumerate() {
            // A head quoted inside a code fence is an example of the dialect, not a requirement.
            if line.trim_start().starts_with("```") { fenced = !fenced; continue; }
            if fenced { continue; }
            if let Some(c) = self.head.captures(line) {
                let tail = c.get(4).map(|m| m.as_str().trim().trim_end_matches('*').trim()).unwrap_or("");
                heads.push((i, c[1].to_string(), c[3].trim().to_string(), tail.to_string()));
            }
        }

        let mut in_block = vec![false; lines.len()];
        for (n, (start, id, title, tail)) in heads.iter().enumerate() {
            let mut end = heads.get(n + 1).map(|h| h.0).unwrap_or(lines.len());
            if let Some(j) = lines[start + 1..end].iter().position(|l| l.starts_with('#')) {
                end = start + 1 + j;
            }
            let end = end.min(start + 1 + BODY_CAP);
            let mut body = lines[start + 1..end].join("\n");
            if !tail.is_empty() { body = format!("{tail}\n{body}"); }
            in_block[*start..end].fill(true);
            ex.node(kind_for(id), id, title, body.trim(), rel, *start as u32 + 1);
            ex.edge(&file_id, id, EdgeKind::Declares, "", rel);
            // Backticked names after a bold head's closing `**` are entities as much as those
            // inside it, so the whole head line is scanned, not the title alone.
            for cap in self.entity.captures_iter(lines[*start]) {
                let name = &cap[1];
                if self.ids.is_id(name) { continue; }
                let eid = format!("entity:{name}");
                ex.node(NodeKind::Entity, &eid, name, "", rel, *start as u32 + 1);
                ex.edge(id, &eid, EdgeKind::References, "title", rel);
            }
            let scope = lines[*start..end].join("\n");
            for hit in self.ids.find_all(&scope) {
                if hit.id != *id {
                    ex.edge(id, &hit.id, EdgeKind::References, "body", rel);
                }
            }
        }

        // Ids in prose outside any requirement block still tie the document to
        // the graph; the File node is their source.
        let owner = self.file_owner(rel, &lines, &mut ex);
        for (i, line) in lines.iter().enumerate() {
            if in_block[i] { continue; }
            if let Some(c) = self.task.captures(line) {
                if let Some(ms) = &owner {
                    let tid = format!("{ms}/{}", &c[1]);
                    ex.node(NodeKind::Task, &tid, c[2].trim(), "", rel, i as u32 + 1);
                    ex.edge(ms, &tid, EdgeKind::Declares, "", rel);
                    for hit in self.ids.find_all(&c[2]) {
                        ex.edge(&tid, &hit.id, EdgeKind::Implements, "task", rel);
                    }
                    continue;
                }
            }
            let source = owner.as_deref().unwrap_or(&file_id);
            for hit in self.ids.find_all(line) {
                if Some(hit.id.as_str()) != owner.as_deref() {
                    ex.edge(source, &hit.id, EdgeKind::References, "prose", rel);
                }
            }
        }

        ex.edges.sort();
        ex.edges.dedup();
        ex
    }

    /// A milestone or ADR file owns its prose: `BE-M01-….md` is the node BE-M01.
    fn file_owner(&self, rel: &str, lines: &[&str], ex: &mut Extraction) -> Option<String> {
        let (id, kind) = if let Some(c) = milestone_file().captures(rel) {
            (c[1].to_string(), NodeKind::Milestone)
        } else {
            let c = adr_file().captures(rel)?;
            (c[1].to_string(), NodeKind::Adr)
        };
        let title = lines.iter().find(|l| l.starts_with("# ")).map(|l| l[2..].trim()).unwrap_or(&id).to_string();
        ex.node(kind, &id, &title, "", rel, 1);
        ex.edge(&format!("file:{rel}"), &id, EdgeKind::Declares, "", rel);
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(rel: &str, text: &str) -> Extraction {
        RequirementScanner::new(crate::families::test_matcher()).scan(rel, text)
    }

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn has(ex: &Extraction, s: &str, t: &str, k: EdgeKind) -> bool {
        ex.edges.iter().any(|e| e.source == s && e.target == t && e.kind == k)
    }

    #[test]
    fn bold_dialect_head_body_and_entity() {
        let ex = scan("docs/06.md", &fixture("06-payments.md"));
        let n = ex.nodes.iter().find(|n| n.id == "FR-PAY-22").unwrap();
        assert_eq!(n.kind, NodeKind::Requirement);
        assert_eq!(n.line, 5);
        assert!(n.label.starts_with("`CancellationPolicy` — правило"));
        assert!(n.body.contains("freeUntilMinutes"));
        assert!(n.body.contains("штраф по FR-PAY-26"));
        assert!(!n.body.contains("FR-PAY-26 · SHOULD"));
        assert!(ex.nodes.iter().any(|n| n.id == "entity:CancellationPolicy" && n.kind == NodeKind::Entity));
        assert!(has(&ex, "FR-PAY-22", "entity:CancellationPolicy", EdgeKind::References));
        assert!(has(&ex, "FR-PAY-22", "N-151", EdgeKind::References));
        assert!(has(&ex, "FR-PAY-22", "FR-PAY-26", EdgeKind::References));
    }

    #[test]
    fn ids_outside_blocks_hang_off_the_file_node() {
        let ex = scan("docs/06.md", &fixture("06-payments.md"));
        assert!(ex.nodes.iter().any(|n| n.id == "file:docs/06.md" && n.kind == NodeKind::File));
        assert!(has(&ex, "file:docs/06.md", "FR-PAY-20", EdgeKind::References));
        assert!(has(&ex, "file:docs/06.md", "INV-12", EdgeKind::References));
        assert!(has(&ex, "file:docs/06.md", "FR-CAL-40", EdgeKind::References));
        assert!(has(&ex, "file:docs/06.md", "FR-PAY-22", EdgeKind::Declares));
    }

    #[test]
    fn heading_dialect_with_ranges() {
        let ex = scan("docs/03.md", &fixture("03-calendar.md"));
        let n = ex.nodes.iter().find(|n| n.id == "FR-CAL-40").unwrap();
        assert_eq!(n.label, "Единый словарь машинных кодов конфликтов");
        for t in ["FR-CAL-41", "FR-CAL-125", "FR-PAY-143", "OQ-25", "FR-CAL-155", "FR-CAL-156", "FR-CAL-157", "FR-CAL-50"] {
            assert!(has(&ex, "FR-CAL-40", t, EdgeKind::References), "{t}");
        }
        assert!(!has(&ex, "FR-CAL-40", "FR-CAL-40", EdgeKind::References));
        assert!(ex.nodes.iter().any(|n| n.id == "FR-CAL-41"));
    }

    #[test]
    fn milestone_file_yields_milestone_and_tasks() {
        let ex = scan("docs/prd/plans/milestones/backend/BE-M01-foundation.md", &fixture("BE-M01-foundation.md"));
        assert!(ex.nodes.iter().any(|n| n.id == "BE-M01" && n.kind == NodeKind::Milestone));
        let t = ex.nodes.iter().find(|n| n.id == "BE-M01/T03").unwrap();
        assert_eq!(t.kind, NodeKind::Task);
        assert_eq!(t.line, 6);
        assert!(has(&ex, "BE-M01", "BE-M01/T03", EdgeKind::Declares));
        assert!(has(&ex, "BE-M01/T03", "FR-DM-05", EdgeKind::Implements));
        assert!(has(&ex, "BE-M01/T03", "FR-DM-04", EdgeKind::Implements));
    }

    #[test]
    fn adr_file_yields_adr_node() {
        let ex = scan("docs/adr/ADR-001-monorepo-and-tooling.md", "# ADR-001: Monorepo\n\nSee INV-06.\n");
        let n = ex.nodes.iter().find(|n| n.id == "ADR-001").unwrap();
        assert_eq!(n.kind, NodeKind::Adr);
        assert_eq!(n.label, "ADR-001: Monorepo");
        assert!(has(&ex, "ADR-001", "INV-06", EdgeKind::References));
    }

    #[test]
    fn invariant_family_gets_invariant_kind() {
        let ex = scan("docs/01.md", "**INV-05 · MUST · Каждая запись аудируется.**\n");
        assert_eq!(ex.nodes.iter().find(|n| n.id == "INV-05").unwrap().kind, NodeKind::Invariant);
    }

    #[test]
    fn head_without_modality_is_still_scanned() {
        let ex = scan("docs/06.md", &fixture("06-payments.md"));
        let n = ex.nodes.iter().find(|n| n.id == "FR-PAY-31").unwrap();
        assert_eq!((n.label.as_str(), n.body.as_str()), ("Без штрафа возврат в течение суток.", ""));
    }

    #[test]
    fn edges_are_unique_per_key() {
        let ex = scan("docs/x.md", "**FR-PAY-22 · MUST · a**\n\nFR-PAY-26 и снова FR-PAY-26.\n");
        let n = ex.edges.iter().filter(|e| e.target == "FR-PAY-26").count();
        assert_eq!(n, 1);
    }
}
