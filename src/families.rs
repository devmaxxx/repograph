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

use crate::model::Graph;
use crate::store::Store;
use crate::walk::{Entry, FileKind};
use anyhow::{Context, Result};
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

/// Which of the two kinds an id is written in, and the family it belongs to. They are counted
/// apart because the extractor matches them apart: `X-01` is an id, `X-M01` a milestone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family<'a> {
    Id(&'a str),
    Milestone(&'a str),
}

/// The id slot of the definition grammar, with any family in place of a known one: `FR-PAY-22`
/// is `FR-PAY`, `N-151` is `N`, `SECURITY-12` is `SECURITY`, `FR-PAY-EU-1` is `FR-PAY-EU`. The
/// slot is wide because a corpus's prefixes are its own and nothing else writes them down; the
/// hyphen before the digits is what keeps `B1` and `C11` out. What holds it together is the fixed
/// point: a family this reads out of a definition head goes into the matcher the extractor writes
/// nodes with, and `classify` must read the same family back off those nodes or an update would
/// find one set in the documents and another in the graph on every run, for ever.
pub(crate) const FAMILY: &str = r"[A-Z][A-Z0-9]{0,11}(?:-[A-Z][A-Z0-9]{0,11}){0,3}";
/// The milestone slot, shared with the extractor's own `milestone_file` and written as `kind_for`
/// writes it: what those accept is what becomes a milestone node, and a scan that admitted one
/// letter more would derive a family no node is ever written in.
pub(crate) const MILESTONE: &str = r"[A-Z]+";

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

fn site(rel: &str, line_no: u32, line: &str) -> Site {
    Site { file: rel.to_string(), line: line_no, text: crate::query::headline(line.trim()) }
}

#[derive(Default)]
struct Tally {
    mentions: usize,
    files: BTreeSet<String>,
    first: Option<Site>,
}

/// Whether the mention tally is wanted. `build`, `update` and a resident poll read the
/// definitions and nothing else; counting the prefixes nobody defines costs a matcher per
/// distinct prefix and a pass over every source file, and only `repograph families` prints it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mentions { Skip, Count }

/// The definitions and the mentions in one pass over the documents. The definition shapes are
/// the extractor's own, with a generic family in the id slot; the mention tally is every id-like
/// token, so what is left once the definitions are taken out is what the report calls text.
struct Scan {
    definition: Regex,
    tally: Mentions,
    ids: BTreeMap<String, Site>,
    milestones: BTreeMap<String, Site>,
    seen: BTreeMap<String, Tally>,
}

impl Scan {
    fn new(tally: Mentions) -> Scan {
        Scan {
            tally,
            // `RequirementScanner`'s head, with a generic id in place of the matcher's: the bold
            // form, the list-item form and the heading form, each with the `·` that separates an
            // id from what it names. A line that merely opens with an id — `UTF-16 conversion` —
            // defines nothing, here or there.
            definition: Regex::new(&format!(
                r"^(?:#{{1,6}}\s+|\*\*|\s*[-*]\s+\*\*)?(?:({FAMILY})-\d{{1,4}}|({MILESTONE})-M\d{{2}})\s*·"
            )).unwrap(),
            ids: BTreeMap::new(),
            milestones: BTreeMap::new(),
            seen: BTreeMap::new(),
        }
    }

    fn doc(&mut self, rel: &str, text: &str) {
        let named = || Site { file: rel.to_string(), line: 1, text: String::new() };
        if let Some(c) = crate::doc::requirements::milestone_file().captures(rel) {
            self.milestones.entry(c[2].to_string()).or_insert_with(named);
        }
        // The one family the extractor names in its own source: an `ADR-###.md` is that node
        // whatever any list says, so the file that carries one defines the family.
        if crate::doc::requirements::adr_file().is_match(rel) {
            self.ids.entry("ADR".to_string()).or_insert_with(named);
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
            // the documents and another in the graph on every run, for ever. The tally is behind
            // the same guard: this repository's own README quotes the dialect in a fence, and
            // listing those prefixes as mention-only points a reader at an example line under a
            // footer telling them to write exactly that line to make it a family.
            if !fenced {
                self.define(rel, i as u32 + 1, line);
                self.mentions(rel, i as u32 + 1, line);
            }
        }
    }

    /// A source file defines nothing — an id in a comment is a citation — so only the tally
    /// reads one. A prefix cited only from code and defined nowhere is exactly the case the
    /// mention half of the report exists to surface.
    fn code(&mut self, rel: &str, text: &str) {
        self.tally_lines(rel, text);
    }

    fn tally_lines(&mut self, rel: &str, text: &str) {
        for (i, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
            self.mentions(rel, i as u32 + 1, line);
        }
    }

    /// The ids a registry declares. Each becomes an invariant node whatever any family list
    /// says, so each defines its own family.
    fn registry(&mut self, rel: &str, text: &str) {
        // A registry's prose cites ids too — a `basis:` row naming `N-039` — and the extractor
        // resolves those into edges, so a prefix written only there belongs in the tally.
        self.tally_lines(rel, text);
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
        if self.tally == Mentions::Skip { return; }
        // The extractor's own reading of the line — ranges and slash lists expanded, boundaries
        // applied — so a mention is counted the way the graph would have cited it.
        let mut per_prefix: BTreeMap<String, usize> = BTreeMap::new();
        for hit in crate::ids::generic().find_all(line) {
            if let Some(Family::Id(f) | Family::Milestone(f)) = classify(&hit.id) {
                *per_prefix.entry(f.to_string()).or_default() += 1;
            }
        }
        for (prefix, n) in per_prefix {
            self.record(prefix, n, rel, line_no, line);
        }
    }

    fn record(&mut self, prefix: String, n: usize, rel: &str, line_no: u32, line: &str) {
        let t = self.seen.entry(prefix).or_default();
        t.mentions += n;
        t.files.insert(rel.to_string());
        t.first.get_or_insert_with(|| site(rel, line_no, line));
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

fn scan_tree(repo: &Path, entries: &[Entry], tally: Mentions) -> Result<Derived> {
    let mut scan = Scan::new(tally);
    for e in entries {
        // Nothing in a source file defines a family, so with no tally to fill there is nothing
        // to open one for.
        if e.kind == FileKind::Code && tally == Mentions::Skip { continue; }
        let bytes = match (std::fs::read(repo.join(&e.rel)), e.kind) {
            (Ok(b), _) => b,
            // A document and a registry are where a family is written down. Reading a failure as
            // "this file defines nothing" hands the caller a family that has vanished, and an
            // update answers that by re-extracting the tree without it and deleting every node
            // in it — so the derivation fails instead, naming the file.
            (Err(err), FileKind::Doc | FileKind::Registry) =>
                return Err(err).with_context(|| format!("families: read {}", e.rel)),
            (Err(err), FileKind::Code) => { eprintln!("families: skipping {}: {err}", e.rel); continue; }
        };
        // A file the extractor will refuse for the same reason declares no node either, so
        // leaving this one out cannot cost a family.
        let Ok(text) = String::from_utf8(bytes) else {
            eprintln!("families: skipping {}: not UTF-8", e.rel);
            continue;
        };
        match e.kind {
            FileKind::Doc => scan.doc(&e.rel, &text),
            FileKind::Registry => scan.registry(&e.rel, &text),
            FileKind::Code => scan.code(&e.rel, &text),
        }
    }
    Ok(scan.finish())
}

/// The families the walked documents define. The walk has already decided which files are
/// documents and which are registries, so the globs are not read again here.
pub fn derive(repo: &Path, entries: &[Entry]) -> Result<Derived> {
    scan_tree(repo, entries, Mentions::Skip)
}

/// The same, plus the id-like prefixes no line defines — the whole of what `repograph families`
/// reports, and the only caller that pays for the tally.
pub fn survey(repo: &Path, entries: &[Entry]) -> Result<Derived> {
    scan_tree(repo, entries, Mentions::Count)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    pub family: String,
    pub nodes: usize,
    /// `None` where the graph holds nodes of this family and no document defines one any more —
    /// a definition edited away with no update since. The documents and the store being out of
    /// step is the one state this command exists to expose, so it is a row, not an omission.
    pub defined: Option<Site>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub families: Vec<Row>,
    pub milestones: Vec<Row>,
    pub mention_only: Vec<Mention>,
}

/// A row per family the documents define or the graph holds, which are the same set only when
/// the store is in step with the tree.
fn rows(defined: BTreeMap<String, Site>, counts: &BTreeMap<String, usize>) -> Vec<Row> {
    let mut out: Vec<Row> = counts.iter()
        .filter(|(family, _)| !defined.contains_key(*family))
        .map(|(family, nodes)| Row { family: family.clone(), nodes: *nodes, defined: None })
        .collect();
    out.extend(defined.into_iter().map(|(family, site)| Row {
        nodes: counts.get(&family).copied().unwrap_or(0), family, defined: Some(site),
    }));
    out.sort_by(|a, b| a.family.cmp(&b.family));
    out
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

fn defined_at(s: &Option<Site>) -> String {
    match s {
        Some(s) => at(s),
        None => "(nothing defines it any more — run `repograph update`)".to_string(),
    }
}

fn section(head: &str, rows: &[Row]) -> String {
    let mut out = format!("{:<NAME$}{:>7}  {}\n", head, "nodes", "defined");
    for r in rows {
        out.push_str(&format!("{:<NAME$}{:>7}  {}\n", format!("  {}", r.family), r.nodes, defined_at(&r.defined)));
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
    let r = report(survey(repo, &entries)?, &graph);
    match json {
        true => println!("{}", serde_json::to_string_pretty(&r)?),
        false => print!("{}", render(&r)),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Extraction, NodeKind};

    fn scan(docs: &[(&str, &str)]) -> Derived {
        let mut s = Scan::new(Mentions::Count);
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
        // Nor a mention: listing REQ as mention-only would point a reader at the fenced example
        // under a footer telling them to write exactly that line to make it a family.
        assert!(d.mention_only.is_empty(), "{:?}", d.mention_only);
    }

    #[test]
    fn a_prefix_written_only_in_code_or_only_in_a_registry_is_still_counted() {
        let mut s = Scan::new(Mentions::Count);
        s.code("src/pay.ts", "// implements NEW-1 and cites TCK-42\n");
        s.registry("docs/constitution.yaml", "invariants:\n  - id: INV-01\n    basis: \"решение 1, `N-039`\"\n");
        let d = s.finish();
        assert_eq!(families(&d), vec!["INV"]);
        assert_eq!(d.mention_only.iter().map(|m| m.prefix.as_str()).collect::<Vec<_>>(), vec!["N", "NEW", "TCK"]);
    }

    #[test]
    fn a_derivation_for_a_build_counts_no_mentions_at_all() {
        let mut s = Scan::new(Mentions::Skip);
        s.doc("docs/a.md", "**REQ-1 · MUST · x**\nсм. ISO-8601\n");
        let d = s.finish();
        assert_eq!(families(&d), vec!["REQ"]);
        assert!(d.mention_only.is_empty());
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
        let mut s = Scan::new(Mentions::Count);
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
        assert_eq!(text.chars().count(), crate::query::HEADLINE + 1);
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
    }

    #[test]
    fn a_family_the_graph_holds_and_no_document_defines_is_a_row_of_its_own() {
        let r = report(one("**REQ-7 · MUST · x**\n"), &graph_with(&["REQ-7", "AC-3"]));
        assert_eq!(r.families.iter().map(|f| (f.family.as_str(), f.nodes, f.defined.is_some())).collect::<Vec<_>>(),
            vec![("AC", 1, false), ("REQ", 1, true)]);
        assert!(render(&r).contains("nothing defines it any more"), "{}", render(&r));
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
        // A task written with a hyphen where the dialect writes a slash is neither shape.
        assert_eq!(classify("MOB-M01-T3"), None);
        // The hyphen before the digits is what keeps a bare label out of the id slot.
        assert_eq!(classify("B1"), None);
        assert_eq!(classify("C11"), None);
        assert_eq!(classify("SECURITY-12"), Some(Family::Id("SECURITY")));
        assert_eq!(classify("FR-PAY-EU-1"), Some(Family::Id("FR-PAY-EU")));
    }

    /// The invariant the whole derivation rests on: what a definition head yields as a family is
    /// what the extractor writes ids in, and `classify` reads that same family back off the node.
    /// Broken, an update finds one set in the documents and another in the graph for ever.
    #[test]
    fn a_long_prefix_and_a_three_part_one_survive_the_round_trip() {
        use crate::model::Extractor;
        let text = "**SECURITY-12 · MUST · доступ**\n\nтело\n\n## FR-PAY-EU-1 · возврат в ЕС\n\nтело\n";
        let d = one(text);
        assert_eq!(families(&d), vec!["FR-PAY-EU", "SECURITY"]);

        let ex = crate::doc::DocExtractor::new().extract("docs/a.md", text);
        let mut written: Vec<&str> = ex.nodes.iter().map(|n| n.id.as_str()).filter(|id| !id.contains(':')).collect();
        written.sort_unstable();
        assert_eq!(written, vec!["FR-PAY-EU-1", "SECURITY-12"]);
        assert_eq!(classify("SECURITY-12"), Some(Family::Id("SECURITY")));
        assert_eq!(classify("FR-PAY-EU-1"), Some(Family::Id("FR-PAY-EU")));

        let mut g = Graph::default();
        g.apply(ex);
        assert!(d.against(&g).is_empty(), "{:?}", d.against(&g));
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
