//! The working tree's diff, read onto the graph: which symbols the hunks sit in, and who reaches
//! those. Line numbers come from the new side of `git diff -U0`, so the graph must have been
//! refreshed against the working tree first — `main` does that before calling in.
use crate::impact::{self, Dependent};
use crate::model::{Graph, NodeKind};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk { pub file: String, pub start: u32, pub end: u32 }

/// New-side ranges of a zero-context unified diff. A pure deletion (`+c,0`) has no new lines;
/// it is recorded as the line the cut lands on, so the symbol around it still counts as changed.
pub fn parse(diff: &str) -> Vec<Hunk> {
    let mut out = Vec::new();
    let mut file: Option<String> = None;
    for line in diff.lines() {
        if let Some(p) = line.strip_prefix("+++ ") {
            file = p.strip_prefix("b/").map(str::to_string);
            continue;
        }
        let Some(rest) = line.strip_prefix("@@ ") else { continue };
        let Some(f) = &file else { continue };
        let Some(plus) = rest.split_whitespace().find(|w| w.starts_with('+')) else { continue };
        let (c, d) = match plus[1..].split_once(',') {
            Some((c, d)) => (c.parse::<u32>().unwrap_or(0), d.parse::<u32>().unwrap_or(1)),
            None => (plus[1..].parse::<u32>().unwrap_or(0), 1),
        };
        let (start, end) = if d == 0 { (c.max(1), c.max(1)) } else { (c, c + d - 1) };
        out.push(Hunk { file: f.clone(), start, end });
    }
    out
}

/// Symbols whose span meets a hunk; a hunk outside every symbol falls to its file node. A class
/// whose member matched is dropped — the member is the change, the class only contains it.
pub fn touched(graph: &Graph, hunks: &[Hunk]) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for h in hunks {
        let mut any = false;
        for n in graph.nodes.values().filter(|n| n.kind == NodeKind::Symbol && n.file == h.file && n.line > 0) {
            let end = n.end.max(n.line);
            if n.line <= h.end && end >= h.start { out.insert(n.id.clone()); any = true; }
        }
        // A file the graph never indexed — a `.kt`, a `.sql`, a lockfile — is still a file the
        // diff changed. Reported as its file id, so the answer says "this changed, I cannot say
        // which symbol" instead of saying nothing: on the bench corpus the silence was a third of
        // a large diff (2026-09-03, 11 of 18 files named). A deleted file never gets here — its
        // `+++ /dev/null` yields no hunk — so every id emitted is a file on the new side.
        if !any { out.insert(format!("file:{}", h.file)); }
    }
    let members: Vec<String> = out.iter().cloned().collect();
    out.retain(|id| !members.iter().any(|m| m.len() > id.len() && m.starts_with(id.as_str()) && m[id.len()..].starts_with('.')));
    out.into_iter().collect()
}

pub struct Report { pub touched: Vec<String>, pub depth: usize, pub affected: Vec<Dependent>, pub files: BTreeSet<String>, pub risk: &'static str }

/// A hunk outside every symbol — an import line, a trailing comment — changes the whole file,
/// so every symbol declared in it is walked; a symbol hunk walks that symbol alone.
fn roots(graph: &Graph, touched: &[String]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for id in touched {
        match id.strip_prefix("file:") {
            Some(rel) => out.extend(graph.nodes.values().filter(|n| n.kind == NodeKind::Symbol && n.file == rel).map(|n| n.id.clone())),
            None => { out.insert(id.clone()); }
        }
    }
    out
}

pub fn report(graph: &Graph, hunks: &[Hunk], depth: usize) -> Report {
    let touched = touched(graph, hunks);
    let roots = roots(graph, &touched);
    let root_files: BTreeSet<String> = roots.iter().filter_map(|r| graph.nodes.get(r).map(|n| n.file.clone())).collect();
    let mut affected: Vec<Dependent> = Vec::new();
    let mut files: BTreeSet<String> = BTreeSet::new();
    for id in &roots {
        let imp = impact::upstream(graph, id, depth);
        files.extend(imp.importers.iter().cloned());
        // A dependent that is itself being changed is not affected, it is the change.
        for d in imp.layers.into_iter().flatten().filter(|d| !roots.contains(&d.id)) {
            // One row per dependent, at the shallowest depth any touched symbol reaches it.
            match affected.iter_mut().find(|a| a.id == d.id) {
                Some(a) if d.depth < a.depth => *a = d,
                Some(_) => {}
                None => affected.push(d),
            }
        }
    }
    affected.sort_by(|a, b| (a.depth, &a.id).cmp(&(b.depth, &b.id)));
    files.extend(affected.iter().map(|d| file_of(graph, &d.id)));
    files.retain(|f| !root_files.contains(f));
    let direct = affected.iter().filter(|d| d.depth == 1).count();
    let risk = impact::risk(direct, affected.len(), files.len());
    Report { touched, depth, affected, files, risk }
}

fn file_of(graph: &Graph, id: &str) -> String {
    match graph.nodes.get(id) { Some(n) => n.file.clone(), None => id.trim_start_matches("file:").to_string() }
}

fn plural(n: usize, one: &str) -> String { format!("{n} {one}{}", if n == 1 { "" } else { "s" }) }

fn span_of(graph: &Graph, id: &str) -> String {
    match graph.nodes.get(id) {
        Some(n) if n.end > n.line => format!("{}:{}-{}", n.file, n.line, n.end),
        Some(n) => format!("{}:{}", n.file, n.line),
        None if id.starts_with("file:") => "not indexed".into(),
        None => "?".into(),
    }
}

pub fn render(graph: &Graph, r: &Report) -> String {
    if r.touched.is_empty() { return "changed: 0 symbols\n".into() }
    let symbols = r.touched.iter().filter(|id| !id.starts_with("file:")).count();
    let changed_files: BTreeSet<String> = r.touched.iter().map(|id| file_of(graph, id)).collect();
    let mut out = format!("changed: {} in {}\n", plural(symbols, "symbol"), plural(changed_files.len(), "file"));
    for id in &r.touched { out.push_str(&format!("  {id}  {}\n", span_of(graph, id))); }
    out.push_str(&format!("affected (depth {}): {} in {}\n", r.depth, plural(r.affected.len(), "symbol"), plural(r.files.len(), "file")));
    for d in &r.affected {
        let at = graph.nodes.get(&d.id).map(|n| format!("{}:{}", n.file, n.line)).unwrap_or_else(|| d.id.trim_start_matches("file:").to_string());
        out.push_str(&format!("  d={}  {}  {at}  ← {}\n", d.depth, d.id, d.via));
    }
    let direct = r.affected.iter().filter(|d| d.depth == 1).count();
    out.push_str(&format!("risk: {} — {direct} direct, {} total, {}\n", r.risk, r.affected.len(), plural(r.files.len(), "file")));
    out
}

pub fn render_json(graph: &Graph, r: &Report) -> String {
    let touched: Vec<serde_json::Value> = r.touched.iter()
        .map(|id| serde_json::json!({ "id": id, "at": span_of(graph, id), "indexed": graph.nodes.contains_key(id) })).collect();
    let affected: Vec<serde_json::Value> = r.affected.iter().map(|d| serde_json::json!({
        "id": d.id, "at": graph.nodes.get(&d.id).map(|n| format!("{}:{}", n.file, n.line)), "depth": d.depth, "kind": format!("{:?}", d.kind), "via": d.via,
    })).collect();
    serde_json::json!({ "touched": touched, "affected": affected, "files": r.files, "risk": r.risk }).to_string() + "\n"
}

fn git(repo: &Path, args: &[&str]) -> anyhow::Result<String> {
    let out = std::process::Command::new("git").arg("-C").arg(repo).args(args).output()?;
    anyhow::ensure!(out.status.success(), "git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Hunks of the working tree against `base` — staged and unstaged alike, plus every untracked
/// file as one hunk over its whole length, so a new file's symbols count as changed too.
pub fn hunks_from_git(repo: &Path, base: &str) -> anyhow::Result<Vec<Hunk>> {
    let mut hunks = parse(&git(repo, &["diff", "-U0", "--no-color", "--no-ext-diff", base, "--", "."])?);
    for f in git(repo, &["ls-files", "--others", "--exclude-standard"])?.lines().filter(|l| !l.is_empty()) {
        hunks.push(Hunk { file: f.to_string(), start: 1, end: u32::MAX });
    }
    Ok(hunks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EdgeKind, Extraction};

    const DIFF: &str = "diff --git a/s.ts b/s.ts\n--- a/s.ts\n+++ b/s.ts\n@@ -6,2 +6,3 @@ export class S {\n+  // more\n@@ -20 +21,0 @@\n-old\ndiff --git a/new.ts b/new.ts\nnew file mode 100644\n--- /dev/null\n+++ b/new.ts\n@@ -0,0 +1,2 @@\n+a\n+b\ndiff --git a/gone.ts b/gone.ts\n--- a/gone.ts\n+++ /dev/null\n@@ -1,3 +0,0 @@\n-x\n";

    #[test]
    fn parse_takes_new_side_ranges_and_records_a_pure_deletion_as_one_line() {
        assert_eq!(parse(DIFF), vec![
            Hunk { file: "s.ts".into(), start: 6, end: 8 },
            Hunk { file: "s.ts".into(), start: 21, end: 21 },
            Hunk { file: "new.ts".into(), start: 1, end: 2 },
        ]);
    }

    fn graph() -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::File, "file:s.ts", "s.ts", "", "s.ts", 1);
        e.node_span(NodeKind::Symbol, "sym:s.ts::S", "S", "", "s.ts", (3, 12));
        e.node_span(NodeKind::Symbol, "sym:s.ts::S.create", "S.create", "", "s.ts", (5, 8));
        e.node_span(NodeKind::Symbol, "sym:s.ts::S.list", "S.list", "", "s.ts", (9, 11));
        e.node_span(NodeKind::Symbol, "sym:s.ts::helper", "helper", "", "s.ts", (14, 14));
        e.edge("file:s.ts", "sym:s.ts::S", EdgeKind::Declares, "export", "s.ts");
        e.edge("sym:s.ts::S", "sym:s.ts::S.create", EdgeKind::Declares, "", "s.ts");
        e.edge("sym:s.ts::S", "sym:s.ts::S.list", EdgeKind::Declares, "", "s.ts");
        e.node(NodeKind::Symbol, "sym:c.ts::C.create", "C.create", "", "c.ts", 9);
        e.edge("sym:c.ts::C.create", "sym:s.ts::S.create", EdgeKind::Calls, "", "c.ts");
        g.apply(e);
        g
    }

    #[test]
    fn a_hunk_inside_a_member_names_the_member_not_the_class() {
        assert_eq!(touched(&graph(), &[Hunk { file: "s.ts".into(), start: 6, end: 7 }]), vec!["sym:s.ts::S.create"]);
    }

    #[test]
    fn a_hunk_spanning_two_members_names_both() {
        assert_eq!(touched(&graph(), &[Hunk { file: "s.ts".into(), start: 8, end: 9 }]), vec!["sym:s.ts::S.create", "sym:s.ts::S.list"]);
    }

    #[test]
    fn a_hunk_in_the_class_but_outside_every_member_names_the_class() {
        assert_eq!(touched(&graph(), &[Hunk { file: "s.ts".into(), start: 4, end: 4 }]), vec!["sym:s.ts::S"]);
    }

    #[test]
    fn a_hunk_outside_every_symbol_falls_to_the_file() {
        assert_eq!(touched(&graph(), &[Hunk { file: "s.ts".into(), start: 1, end: 1 }]), vec!["file:s.ts"]);
    }

    #[test]
    fn a_whole_file_hunk_names_every_symbol_of_the_file_but_no_containing_class() {
        assert_eq!(touched(&graph(), &[Hunk { file: "s.ts".into(), start: 1, end: u32::MAX }]), vec!["sym:s.ts::S.create", "sym:s.ts::S.list", "sym:s.ts::helper"]);
    }

    #[test]
    fn a_hunk_in_a_file_the_graph_never_indexed_is_reported_as_that_file() {
        assert_eq!(touched(&graph(), &[Hunk { file: "Foo.kt".into(), start: 1, end: 9 }]), vec!["file:Foo.kt"]);
    }

    #[test]
    fn an_unindexed_file_renders_as_changed_with_no_span_and_reaches_nothing() {
        let g = graph();
        let r = report(&g, &[Hunk { file: "Foo.kt".into(), start: 1, end: 9 }], 2);
        assert_eq!(r.touched, vec!["file:Foo.kt"]);
        assert!(r.affected.is_empty());
        assert_eq!(
            render(&g, &r),
            "changed: 0 symbols in 1 file\n  file:Foo.kt  not indexed\naffected (depth 2): 0 symbols in 0 files\nrisk: LOW — 0 direct, 0 total, 0 files\n"
        );
        let v: serde_json::Value = serde_json::from_str(&render_json(&g, &r)).unwrap();
        assert_eq!(v["touched"][0]["indexed"], false);
    }

    #[test]
    fn report_unions_the_callers_of_every_touched_symbol() {
        let g = graph();
        let r = report(&g, &[Hunk { file: "s.ts".into(), start: 6, end: 7 }], 2);
        assert_eq!(r.touched, vec!["sym:s.ts::S.create"]);
        assert_eq!(r.affected.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(), vec!["sym:c.ts::C.create"]);
        assert_eq!(r.files, BTreeSet::from(["c.ts".to_string()]));
        assert_eq!(r.risk, "LOW");
    }

    #[test]
    fn a_file_level_change_walks_every_symbol_of_the_file_and_lists_none_of_them_as_affected() {
        let g = graph();
        let r = report(&g, &[Hunk { file: "s.ts".into(), start: 1, end: 1 }], 2);
        assert_eq!(r.touched, vec!["file:s.ts"]);
        assert_eq!(r.affected.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(), vec!["sym:c.ts::C.create"]);
        assert_eq!(r.files, BTreeSet::from(["c.ts".to_string()]));
    }

    #[test]
    fn a_file_level_change_in_an_indexed_file_renders_zero_symbols_but_names_the_file() {
        let g = graph();
        let r = report(&g, &[Hunk { file: "s.ts".into(), start: 1, end: 1 }], 2);
        assert_eq!(
            render(&g, &r),
            "changed: 0 symbols in 1 file\n  file:s.ts  s.ts:1\naffected (depth 2): 1 symbol in 1 file\n  d=1  sym:c.ts::C.create  c.ts:9  ← sym:s.ts::S.create\nrisk: LOW — 1 direct, 1 total, 1 file\n"
        );
    }

    #[test]
    fn render_says_what_changed_and_what_it_reaches() {
        let g = graph();
        let out = render(&g, &report(&g, &[Hunk { file: "s.ts".into(), start: 6, end: 7 }], 2));
        assert_eq!(out, "changed: 1 symbol in 1 file\n  sym:s.ts::S.create  s.ts:5-8\naffected (depth 2): 1 symbol in 1 file\n  d=1  sym:c.ts::C.create  c.ts:9  ← sym:s.ts::S.create\nrisk: LOW — 1 direct, 1 total, 1 file\n");
    }

    #[test]
    fn an_empty_diff_renders_a_clean_report() {
        let g = graph();
        assert_eq!(render(&g, &report(&g, &[], 2)), "changed: 0 symbols\n");
    }
}
