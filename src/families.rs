//! Which id families a repository has. A family is the prefix of any id the corpus *defines* —
//! a requirement line, its heading form, a milestone document, a registry row — so the documents
//! that already say it are the only place it is written down. `build` and `update` read the
//! definitions; every reader reads the graph those definitions became.
//!
//! Nothing configures this and nothing stores it. A default is the answer for a repository about
//! which nothing is known, and one corpus's list of families was never that answer for anybody
//! else's; a pin beside the graph would have been a second thing to keep in step with the
//! documents. The two losses are worth naming: a prefix a document only ever mentions —
//! `ISO-8601`, a ticket number, a year — stays plain text, and there is no way to take a family
//! away by hand. `repograph families` prints both halves so a prefix on the wrong side of that
//! line is something a reader can see.

use crate::ids::IdMatcher;
use crate::model::Graph;
use crate::store::Store;
use crate::walk::{Entry, FileKind};
use anyhow::Result;
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::OnceLock;

/// Which of the two kinds an id is written in, and the family it belongs to. They are counted
/// apart because the extractor matches them apart: `X-01` is an id, `X-M01` a milestone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family<'a> {
    Id(&'a str),
    Milestone(&'a str),
}

/// The id slot of the definition grammar, with any family in place of a known one. Bounded the
/// way `ids::bounded` is bounded, and one hyphen at most: `FR-PAY-22` is `FR-PAY`, `N-151` is `N`.
const FAMILY: &str = r"[A-Z][A-Z0-9]{0,5}(?:-[A-Z][A-Z0-9]{0,5})?";
/// The milestone slot, written as the extractor writes it in `kind_for` and `milestone_file`
/// rather than as the id slot above: what those two accept is what becomes a milestone node, and
/// a scan that admitted one letter more would derive a family no node is ever written in.
const MILESTONE: &str = r"[A-Z]+";

fn shapes() -> &'static (Regex, Regex) {
    static RE: OnceLock<(Regex, Regex)> = OnceLock::new();
    RE.get_or_init(|| {
        (
            Regex::new(&format!(r"^({FAMILY})-\d{{1,4}}$")).unwrap(),
            // A task is written in its milestone's family and hangs off it: `BE-M10/T05`.
            Regex::new(&format!(r"^({MILESTONE})-M\d{{2}}(?:/T\d{{1,4}})?$")).unwrap(),
        )
    })
}

/// The family an id is written in, or `None` for anything that is not an id at all — a file node,
/// an entity, a legacy concept, a symbol.
pub fn classify(id: &str) -> Option<Family<'_>> {
    let (ids, milestones) = shapes();
    if let Some(c) = milestones.captures(id) {
        return Some(Family::Milestone(c.get(1).unwrap().as_str()));
    }
    ids.captures(id).map(|c| Family::Id(c.get(1).unwrap().as_str()))
}

/// Where a family was first defined, or where a prefix nobody defines was first written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Site {
    pub file: String,
    pub line: u32,
    pub text: String,
}

/// A prefix the documents write and no line defines: a standard, a ticket number, a year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mention {
    pub prefix: String,
    pub mentions: usize,
    pub files: usize,
    pub example: Option<Site>,
}

/// What the documents say their families are: the two sets, each with the line that first
/// defined it, and everything left over as text.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Derived {
    pub ids: BTreeMap<String, Site>,
    pub milestones: BTreeMap<String, Site>,
    pub mention_only: Vec<Mention>,
}

fn names(m: &BTreeMap<String, Site>) -> Vec<String> {
    m.keys().cloned().collect()
}

impl Derived {
    /// The matcher every extractor reads ids through on a build.
    pub fn matcher(&self) -> IdMatcher {
        IdMatcher::new(&names(&self.ids), &names(&self.milestones))
    }

    /// What a build says it found. The whole list where it is short and its head with a count
    /// where it is not: a build's stderr is read at a glance, and a large corpus names dozens.
    pub fn line(&self) -> String {
        const SHOWN: usize = 20;
        let listed = |v: Vec<String>| match (v.len(), v.len() > SHOWN) {
            (0, _) => "(none)".to_string(),
            (n, true) => format!("{} … +{}", v[..SHOWN].join(", "), n - SHOWN),
            _ => v.join(", "),
        };
        format!(
            "families: {} · milestones: {}",
            listed(names(&self.ids)),
            listed(names(&self.milestones))
        )
    }

    /// Families the documents have gained or lost since the graph was extracted, each as the
    /// word an update prints. Empty is the ordinary case, and the one where the incremental path
    /// is enough: a family appearing or vanishing changes what every document extracts to.
    pub fn against(&self, graph: &Graph) -> Vec<String> {
        let (ids, milestones) = of_graph(graph);
        let mut out = Vec::new();
        for (derived, built, mark) in [
            (&self.ids, &ids, ""),
            (&self.milestones, &milestones, " milestone"),
        ] {
            for f in derived.keys().filter(|f| !built.contains_key(*f)) {
                out.push(format!("+{f}{mark}"));
            }
            for f in built.keys().filter(|f| !derived.contains_key(f.as_str())) {
                out.push(format!("-{f}{mark}"));
            }
        }
        out
    }
}

/// Every family the graph's own nodes are written in, with how many each holds. Code nodes are
/// excluded: a symbol named after an id is not one, and a file is not an id at all.
pub fn of_graph(graph: &Graph) -> (BTreeMap<String, usize>, BTreeMap<String, usize>) {
    let (mut ids, mut milestones) = (BTreeMap::new(), BTreeMap::new());
    for n in graph.nodes.values().filter(|n| !n.is_code()) {
        match classify(&n.id) {
            Some(Family::Id(f)) => *ids.entry(f.to_string()).or_default() += 1,
            Some(Family::Milestone(f)) => *milestones.entry(f.to_string()).or_default() += 1,
            None => {}
        }
    }
    (ids, milestones)
}

/// The matcher a reader reads ids through: the families the graph itself declares, read back off
/// the definitions they came from. Nothing a reader does may cost a pass over every document —
/// `serve` answers in 66 ms — so a family a document has only just grown reaches the read path
/// through the `build` or `update` that derives it.
pub fn from_graph(graph: &Graph) -> IdMatcher {
    let (ids, milestones) = of_graph(graph);
    let keys = |m: BTreeMap<String, usize>| m.into_keys().collect::<Vec<_>>();
    IdMatcher::new(&keys(ids), &keys(milestones))
}

/// Same shape as `IdMatcher`'s: a hit whose neighbour is alphanumeric or a hyphen belongs to a
/// longer token. `FR-PAY-22` is one id and not `PAY-22`, and `2026-09-05` is a date.
fn bounded(text: &str, start: usize, end: usize) -> bool {
    let tail = |b: u8| b.is_ascii_alphanumeric() || b == b'-';
    let left_ok = start == 0 || !tail(text.as_bytes()[start - 1]);
    let right_ok = end == text.len() || !tail(text.as_bytes()[end]);
    left_ok && right_ok
}

const EXAMPLE_CHARS: usize = 80;

fn site(rel: &str, line_no: u32, line: &str) -> Site {
    let text = line.trim();
    let cut = text.char_indices().nth(EXAMPLE_CHARS).map(|(i, _)| i);
    Site {
        file: rel.to_string(),
        line: line_no,
        text: match cut {
            Some(i) => format!("{}…", &text[..i]),
            None => text.to_string(),
        },
    }
}

#[derive(Default)]
struct Tally {
    mentions: usize,
    files: BTreeSet<String>,
    first: Option<Site>,
}

/// The definitions and the mentions in one pass over the documents. The definition shapes are
/// the extractor's own, with a generic family in the id slot; the mention tally is every id-like
/// token, so what is left once the definitions are taken out is what the report calls text.
struct Scan {
    definition: Regex,
    milestone_file: Regex,
    adr_file: Regex,
    id: Regex,
    milestone: Regex,
    /// One matcher per prefix, so a range (`FR-RPT-42…48`) and a slash list (`INV-11/12`) are
    /// counted the way the extractor would count them rather than as one mention each. Built
    /// lazily: a corpus names few prefixes and this runs per line.
    matchers: HashMap<String, IdMatcher>,
    ids: BTreeMap<String, Site>,
    milestones: BTreeMap<String, Site>,
    seen: BTreeMap<String, Tally>,
}

impl Scan {
    fn new() -> Scan {
        Scan {
            // `RequirementScanner`'s head, with a generic id in place of the matcher's: the bold
            // form, the list-item form and the heading form, each with the `·` that separates an
            // id from what it names. A line that merely opens with an id — `UTF-16 conversion` —
            // defines nothing, here or there.
            definition: Regex::new(&format!(
                r"^(?:#{{1,6}}\s+|\*\*|\s*[-*]\s+\*\*)?(?:({FAMILY})-\d{{1,4}}|({MILESTONE})-M\d{{2}})\s*·"
            )).unwrap(),
            milestone_file: Regex::new(&format!(r"(?:^|/)({MILESTONE})-M\d{{2}}[^/]*\.md$")).unwrap(),
            // The one family the extractor names in its own source: an `ADR-###.md` is that node
            // whatever any list says, so the file that carries one defines the family.
            adr_file: Regex::new(r"(?:^|/)ADR-\d{3,4}[^/]*\.md$").unwrap(),
            id: Regex::new(&format!(r"({FAMILY})-\d{{1,4}}")).unwrap(),
            milestone: Regex::new(&format!(r"({MILESTONE})-M\d{{2}}")).unwrap(),
            matchers: HashMap::new(),
            ids: BTreeMap::new(),
            milestones: BTreeMap::new(),
            seen: BTreeMap::new(),
        }
    }

    fn doc(&mut self, rel: &str, text: &str) {
        if let Some(c) = self.milestone_file.captures(rel) {
            let family = c[1].to_string();
            self.milestones.entry(family).or_insert_with(|| Site { file: rel.to_string(), line: 1, text: String::new() });
        }
        if self.adr_file.is_match(rel) {
            self.ids.entry("ADR".to_string()).or_insert_with(|| Site { file: rel.to_string(), line: 1, text: String::new() });
        }
        // An editor's byte-order mark would otherwise hide the first definition from `^`.
        let mut fenced = false;
        for (i, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
                continue;
            }
            // A head quoted inside a code fence is an example of the dialect and yields no node,
            // so it may not yield a family either — an update would otherwise find one family in
            // the documents and another in the graph on every run, for ever.
            if !fenced {
                self.define(rel, i as u32 + 1, line);
            }
            self.mentions(rel, i as u32 + 1, line);
        }
    }

    /// The ids a registry declares. Each becomes an invariant node whatever any family list
    /// says, so each defines its own family.
    fn registry(&mut self, rel: &str, text: &str) {
        for (i, id) in crate::doc::registry::declared_ids(text).iter().enumerate() {
            let at = || Site { file: rel.to_string(), line: i as u32 + 1, text: id.clone() };
            match classify(id) {
                Some(Family::Id(f)) => { self.ids.entry(f.to_string()).or_insert_with(at); }
                Some(Family::Milestone(f)) => { self.milestones.entry(f.to_string()).or_insert_with(at); }
                None => {}
            }
        }
    }

    fn define(&mut self, rel: &str, line_no: u32, line: &str) {
        let Some(c) = self.definition.captures(line) else { return };
        let at = || site(rel, line_no, line);
        if let Some(f) = c.get(1) {
            self.ids.entry(f.as_str().to_string()).or_insert_with(at);
        } else if let Some(f) = c.get(2) {
            self.milestones.entry(f.as_str().to_string()).or_insert_with(at);
        }
    }

    fn mentions(&mut self, rel: &str, line_no: u32, line: &str) {
        let mut prefixes: BTreeSet<String> = BTreeSet::new();
        let mut milestones: BTreeMap<String, usize> = BTreeMap::new();
        for c in self.milestone.captures_iter(line) {
            let whole = c.get(0).unwrap();
            if bounded(line, whole.start(), whole.end()) {
                *milestones.entry(c[1].to_string()).or_default() += 1;
            }
        }
        // A milestone id satisfies the id form too — `BE-M01` is `BE` and nothing more only
        // because the digits must follow the hyphen — so the two scans cannot both claim a hit.
        for c in self.id.captures_iter(line) {
            let whole = c.get(0).unwrap();
            if bounded(line, whole.start(), whole.end()) {
                prefixes.insert(c[1].to_string());
            }
        }
        for (prefix, n) in milestones {
            self.record(prefix, n, rel, line_no, line);
        }
        for prefix in prefixes {
            let n = self.hits(&prefix, line);
            if n > 0 {
                self.record(prefix, n, rel, line_no, line);
            }
        }
    }

    fn record(&mut self, prefix: String, n: usize, rel: &str, line_no: u32, line: &str) {
        let t = self.seen.entry(prefix).or_default();
        t.mentions += n;
        t.files.insert(rel.to_string());
        t.first.get_or_insert_with(|| site(rel, line_no, line));
    }

    /// How many ids of this prefix the line holds, ranges and slash lists expanded. No milestone
    /// family is given: the milestone form is counted above, and a matcher with no milestone
    /// family matches none.
    fn hits(&mut self, prefix: &str, line: &str) -> usize {
        let matcher = self.matchers.entry(prefix.to_string())
            .or_insert_with(|| IdMatcher::new(&[prefix.to_string()], &[]));
        matcher.find_all(line).len()
    }

    fn finish(self) -> Derived {
        let mut mention_only: Vec<Mention> = self.seen.into_iter()
            .filter(|(p, _)| !self.ids.contains_key(p) && !self.milestones.contains_key(p))
            .map(|(prefix, t)| Mention { prefix, mentions: t.mentions, files: t.files.len(), example: t.first })
            .collect();
        mention_only.sort_by(|a, b| b.mentions.cmp(&a.mentions).then(a.prefix.cmp(&b.prefix)));
        Derived { ids: self.ids, milestones: self.milestones, mention_only }
    }
}

/// The families the walked documents define. The walk has already decided which files are
/// documents and which are registries, so the globs are not read again here.
pub fn derive(repo: &Path, entries: &[Entry]) -> Result<Derived> {
    let mut scan = Scan::new();
    for e in entries {
        let read = || match std::fs::read_to_string(repo.join(&e.rel)) {
            Ok(text) => Some(text),
            Err(err) => { eprintln!("families: skipping {}: {err}", e.rel); None }
        };
        match e.kind {
            FileKind::Doc => if let Some(text) = read() { scan.doc(&e.rel, &text) },
            FileKind::Registry => if let Some(text) = read() { scan.registry(&e.rel, &text) },
            FileKind::Code => {}
        }
    }
    Ok(scan.finish())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    pub family: String,
    pub nodes: usize,
    pub defined: Site,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub families: Vec<Row>,
    pub milestones: Vec<Row>,
    pub mention_only: Vec<Mention>,
}

fn rows(defined: BTreeMap<String, Site>, counts: &BTreeMap<String, usize>) -> Vec<Row> {
    defined.into_iter()
        .map(|(family, defined)| Row { nodes: counts.get(&family).copied().unwrap_or(0), family, defined })
        .collect()
}

pub fn report(derived: Derived, graph: &Graph) -> Report {
    let (ids, milestones) = of_graph(graph);
    Report {
        families: rows(derived.ids, &ids),
        milestones: rows(derived.milestones, &milestones),
        mention_only: derived.mention_only,
    }
}

const NAME: usize = 36;

fn at(s: &Site) -> String {
    format!("{}:{}", s.file, s.line)
}

fn section(head: &str, rows: &[Row]) -> String {
    let mut out = format!("{:<NAME$}{:>7}  {}\n", head, "nodes", "defined");
    for r in rows {
        out.push_str(&format!("{:<NAME$}{:>7}  {}\n", format!("  {}", r.family), r.nodes, at(&r.defined)));
    }
    if rows.is_empty() { out.push_str("  (none)\n"); }
    out
}

pub fn render(r: &Report) -> String {
    let mut out = section("families", &r.families);
    out.push_str(&section("milestones", &r.milestones));
    out.push_str(&format!("{:<NAME$}{:>7}{:>7}  e.g.\n", "mention-only prefixes", "written", "files"));
    for m in &r.mention_only {
        let example = m.example.as_ref().map(|e| format!("  {}  {}", at(e), e.text)).unwrap_or_default();
        out.push_str(&format!("{:<NAME$}{:>7}{:>7}{example}\n", format!("  {}", m.prefix), m.mentions, m.files));
    }
    if r.mention_only.is_empty() { out.push_str("  (none)\n"); }
    out.push_str("\n# Not families: no line defines one of these. A prefix a document only writes —\n");
    out.push_str("# ISO-8601, RFC-7231, a ticket number, a year — is text, and stays text. To make one\n");
    out.push_str("# a family, write a line that defines it: `**PREFIX-1 · MUST · title**`.\n");
    out
}

pub fn run(repo: &Path, cfg: &crate::config::Config, json: bool) -> Result<()> {
    let (graph, manifest) = Store::new(repo).load()?;
    if graph.nodes.is_empty() { anyhow::bail!("graph is empty — run `repograph build`"); }
    let entries = crate::walk::walk(repo, cfg, &manifest)?;
    let r = report(derive(repo, &entries)?, &graph);
    match json {
        true => println!("{}", serde_json::to_string_pretty(&r)?),
        false => print!("{}", render(&r)),
    }
    Ok(())
}

/// The families the fixtures under `tests/` are written in — one corpus's, kept here because its
/// documents are what the extractor cases quote. Nothing outside a test reads a list of families.
#[cfg(test)]
pub(crate) fn test_matcher() -> IdMatcher {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    IdMatcher::new(
        &s(&[
            "FR-DM", "FR-CAL", "FR-VIS", "FR-PAY", "FR-PH", "FR-SEC", "FR-APP", "FR-MKT",
            "FR-AI", "FR-CRM", "FR-SHELL", "FR-TOOL", "FR-SVC", "FR-LIFE", "FR-WH", "FR-RPT",
            "FR-MIG", "FR-WEB", "FR-OPS", "FR-STAFF",
            "NFR-PH", "NFR-MKT", "NFR-MIG", "NFR-PAY", "NFR-DM", "NFR-RPT", "NFR-WEB", "NFR-SVC",
            "NFR-STAFF", "NFR",
            "AC-DM", "AC-VIS", "INV", "ADR", "OD", "OQ", "N", "R", "M", "W", "D", "G",
            "PREP", "CAL", "OR", "MON", "SEAM", "SG", "IDEA",
        ]),
        &s(&["BE", "FE", "PLAT", "SYNC", "OPS", "AI", "MOB"]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Extraction, NodeKind};

    fn scan(docs: &[(&str, &str)]) -> Derived {
        let mut s = Scan::new();
        for (rel, text) in docs { s.doc(rel, text); }
        s.finish()
    }

    fn one(text: &str) -> Derived {
        scan(&[("docs/a.md", text)])
    }

    fn families(d: &Derived) -> Vec<String> { names(&d.ids) }

    #[test]
    fn a_bold_head_and_a_heading_define_and_prose_does_not() {
        let d = one("**REQ-7 · MUST · первое**\n## AC-3 · критерий\n- **OQ-1 · вопрос**\nсм. FR-PAY-22 в другом файле\n");
        assert_eq!(families(&d), vec!["AC", "OQ", "REQ"]);
        assert_eq!(d.mention_only.iter().map(|m| m.prefix.as_str()).collect::<Vec<_>>(), vec!["FR-PAY"]);
    }

    #[test]
    fn a_line_that_only_opens_with_an_id_defines_nothing() {
        let d = one("UTF-16 conversion is lossy\nsee ISO-8601 and RFC-7231\n**REQ-1 · MUST · x**\n");
        assert_eq!(families(&d), vec!["REQ"]);
        assert_eq!(d.mention_only.iter().map(|m| m.prefix.as_str()).collect::<Vec<_>>(), vec!["ISO", "RFC", "UTF"]);
    }

    #[test]
    fn a_date_is_neither_a_definition_nor_a_mention() {
        let d = one("released 2026-09-05, revised 2026-09-06\n");
        assert!(d.ids.is_empty() && d.mention_only.is_empty());
    }

    #[test]
    fn a_two_part_family_is_read_whole_not_from_its_last_segment() {
        assert_eq!(families(&one("**FR-PAY-22 · MUST · окно отмены**\n")), vec!["FR-PAY"]);
    }

    #[test]
    fn a_head_inside_a_code_fence_defines_nothing() {
        // It yields no node either, so a family read out of one would be a family no update
        // could ever find in the graph.
        let d = one("```\n**REQ-7 · MUST · пример диалекта**\n```\n**AC-1 · MUST · настоящее**\n");
        assert_eq!(families(&d), vec!["AC"]);
    }

    #[test]
    fn a_milestone_document_defines_its_family_by_its_name_and_by_a_head() {
        let d = scan(&[
            ("docs/plans/BE-M01-foundation.md", "# BE-M01 — основание\n"),
            ("docs/plans/notes.md", "## QA-M02 · стабилизация\n"),
        ]);
        assert_eq!(names(&d.milestones), vec!["BE", "QA"]);
        assert!(d.ids.is_empty());
        assert_eq!(d.milestones["BE"].file, "docs/plans/BE-M01-foundation.md");
    }

    #[test]
    fn a_milestone_only_mentioned_is_text_and_not_a_family() {
        let d = one("см. QA-M01 и ещё раз QA-M01\n");
        assert!(d.milestones.is_empty());
        assert_eq!(d.mention_only.iter().map(|m| (m.prefix.as_str(), m.mentions)).collect::<Vec<_>>(), vec![("QA", 2)]);
    }

    #[test]
    fn an_adr_document_defines_the_adr_family_by_its_name() {
        let d = scan(&[("docs/adr/ADR-001-monorepo-and-tooling.md", "# ADR-001: Monorepo\n\nсм. INV-06.\n")]);
        assert_eq!(families(&d), vec!["ADR"]);
        assert_eq!(d.mention_only.iter().map(|m| m.prefix.as_str()).collect::<Vec<_>>(), vec!["INV"]);
    }

    #[test]
    fn a_registry_row_defines_its_prefix() {
        let mut s = Scan::new();
        s.registry("docs/constitution.yaml", "invariants:\n  - id: INV-01\n    statement: \"**A.**\"\n  - id: INV-02\n");
        let d = s.finish();
        assert_eq!(families(&d), vec!["INV"]);
        assert_eq!(d.ids["INV"], Site { file: "docs/constitution.yaml".into(), line: 1, text: "INV-01".into() });
    }

    #[test]
    fn a_range_and_a_slash_list_count_every_id_they_stand_for() {
        let d = one("см. INV-11…13 и OQ-1/2\n");
        assert_eq!(d.mention_only.iter().map(|m| (m.prefix.as_str(), m.mentions)).collect::<Vec<_>>(),
            vec![("INV", 3), ("OQ", 2)]);
    }

    #[test]
    fn an_example_line_is_cut_on_a_character_boundary() {
        let d = one(&format!("**REQ-1 · {}**\n", "ф".repeat(200)));
        let text = &d.ids["REQ"].text;
        assert_eq!(text.chars().count(), EXAMPLE_CHARS + 1);
        assert!(text.ends_with('…'));
    }

    fn graph_with(ids: &[&str]) -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        for id in ids { e.node(NodeKind::Requirement, id, "label", "тело", "docs/a.md", 1); }
        e.node(NodeKind::Symbol, "src/a.ts#REQ-9", "REQ-9", "", "src/a.ts", 1);
        g.apply(e);
        g
    }

    #[test]
    fn the_graph_reports_the_same_families_its_documents_defined() {
        let g = graph_with(&["REQ-7", "AC-3", "BE-M01", "BE-M01/T05", "entity:Foo"]);
        let (ids, milestones) = of_graph(&g);
        assert_eq!(ids.into_iter().collect::<Vec<_>>(), vec![("AC".to_string(), 1), ("REQ".to_string(), 1)]);
        assert_eq!(milestones.into_iter().collect::<Vec<_>>(), vec![("BE".to_string(), 2)]);
        // The symbol named after an id is not one, and neither is the entity.
        assert_eq!(from_graph(&g).find_all("см. REQ-7, BE-M01 и FR-PAY-22").into_iter().map(|h| h.id).collect::<Vec<_>>(),
            vec!["REQ-7", "BE-M01"]);
    }

    #[test]
    fn what_a_build_derives_is_what_its_graph_declares() {
        let d = one("**REQ-7 · MUST · x**\n## AC-3 · y\n");
        assert!(d.against(&graph_with(&["REQ-7", "AC-3"])).is_empty(), "no family moved, so no document is re-read");
    }

    #[test]
    fn a_family_gained_and_one_lost_are_both_named() {
        let d = one("**REQ-7 · MUST · x**\n# BE-M01 · веха\n");
        assert_eq!(d.against(&graph_with(&["AC-3", "REQ-7"])), vec!["-AC", "+BE milestone"]);
    }

    #[test]
    fn a_task_is_written_in_its_milestones_family() {
        assert_eq!(classify("BE-M10/T05"), Some(Family::Milestone("BE")));
        assert_eq!(classify("BE-M10"), Some(Family::Milestone("BE")));
        assert_eq!(classify("FR-PAY-22"), Some(Family::Id("FR-PAY")));
        assert_eq!(classify("N-151"), Some(Family::Id("N")));
        assert_eq!(classify("file:docs/a.md"), None);
        assert_eq!(classify("MOB-M01-T3"), None);
    }

    #[test]
    fn the_build_line_names_the_families_and_elides_a_long_list() {
        let d = one("**REQ-1 · MUST · x**\n# BE-M01 · веха\n");
        assert_eq!(d.line(), "families: REQ · milestones: BE");
        let text: String = (0..25).map(|i| format!("**F{i:02}-1 · MUST · x**\n")).collect();
        let line = one(&text).line();
        assert!(line.starts_with("families: F00, F01,"), "{line}");
        assert!(line.contains("… +5 · milestones: (none)"), "{line}");
    }
}
