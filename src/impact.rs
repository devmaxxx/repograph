//! Walks over the code edges: who reaches a symbol, what a symbol reaches, and a path between
//! two. Callers import through barrels, so a caller's edge points at `sym:<barrel>::Name`,
//! never at the declaration; every walk therefore treats a symbol and its barrel aliases as one.
use crate::model::{Edge, EdgeKind, Graph};
use std::collections::{BTreeMap, BTreeSet};

/// The edge kinds along which a change to the target reaches the source.
const CODE: [EdgeKind; 2] = [EdgeKind::Calls, EdgeKind::Extends];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Dependent { pub id: String, pub depth: usize, pub kind: EdgeKind, pub via: String }

#[derive(Debug, Default)]
pub struct Impact { pub root: String, pub layers: Vec<Vec<Dependent>>, pub importers: Vec<String> }

fn name_of(id: &str) -> &str { id.rsplit("::").next().unwrap_or(id) }

/// `S.create` is exported as `S`; a barrel names the class, not the member.
fn bare(name: &str) -> &str { name.split('.').next().unwrap_or(name) }

fn exports(e: &Edge, bare: &str) -> bool { e.context == "*" || e.context.split(',').any(|c| c == bare) }

/// Every `sym:<barrel>::<Name>` a caller could have reached this symbol by.
pub fn aliases(graph: &Graph, id: &str) -> Vec<String> {
    let Some(n) = graph.nodes.get(id) else { return Vec::new() };
    let name = name_of(id);
    let mut files = vec![n.file.clone()];
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::new();
    let mut i = 0;
    while i < files.len() {
        let target = format!("file:{}", files[i]);
        for e in graph.edges.iter().filter(|e| e.kind == EdgeKind::ReExports && e.target == target && exports(e, bare(name))) {
            let barrel = e.source.trim_start_matches("file:").to_string();
            if seen.insert(barrel.clone()) {
                out.push(format!("sym:{barrel}::{name}"));
                files.push(barrel);
            }
        }
        i += 1;
    }
    out
}

/// The node a possibly-dangling `sym:<barrel>::<Name>` stands for, following re-exports forward.
pub fn canonical(graph: &Graph, id: &str) -> Option<String> {
    if graph.nodes.contains_key(id) { return Some(id.to_string()) }
    let (file, name) = id.strip_prefix("sym:")?.rsplit_once("::")?;
    let mut files = vec![file.to_string()];
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut i = 0;
    while i < files.len() {
        let source = format!("file:{}", files[i]);
        for e in graph.edges.iter().filter(|e| e.kind == EdgeKind::ReExports && e.source == source && exports(e, bare(name))) {
            let next = e.target.trim_start_matches("file:").to_string();
            let candidate = format!("sym:{next}::{name}");
            if graph.nodes.contains_key(&candidate) { return Some(candidate) }
            if seen.insert(next.clone()) { files.push(next) }
        }
        i += 1;
    }
    None
}

/// The symbols a class declares: its methods and fields.
fn members(graph: &Graph, id: &str) -> Vec<String> {
    graph.edges.iter().filter(|e| e.kind == EdgeKind::Declares && e.source == id && e.target.starts_with("sym:")).map(|e| e.target.clone()).collect()
}

/// `sym:f::C` for `sym:f::C.m`; none for a class or a file.
fn container(id: &str) -> Option<String> {
    let (file, name) = id.strip_prefix("sym:")?.rsplit_once("::")?;
    let (class, _) = name.split_once('.')?;
    Some(format!("sym:{file}::{class}"))
}

/// The root, its members (a class is changed through them) and every alias of each.
fn seeds(graph: &Graph, root: &str) -> Vec<String> {
    let mut out = vec![root.to_string()];
    out.extend(members(graph, root));
    let aliased: Vec<String> = out.iter().flat_map(|s| aliases(graph, s)).collect();
    out.extend(aliased);
    out
}

/// The edges to follow from `at`. Upstream, a member is also reached through its class by a
/// subclass — `extends C` inherits `C.m` — so the class's `Extends` edges count for the member
/// without the class itself being listed. Downstream, a class reaches what its members call.
fn step<'a>(graph: &Graph, by_key: &BTreeMap<&str, Vec<&'a Edge>>, at: &str, up: bool) -> Vec<&'a Edge> {
    let mut out: Vec<&Edge> = by_key.get(at).into_iter().flatten().copied().collect();
    if up {
        if let Some(c) = container(at) {
            out.extend(by_key.get(c.as_str()).into_iter().flatten().copied().filter(|e| e.kind == EdgeKind::Extends));
        }
    } else {
        for m in members(graph, at) { out.extend(by_key.get(m.as_str()).into_iter().flatten().copied()); }
    }
    out
}

fn index(graph: &Graph, up: bool) -> BTreeMap<&str, Vec<&Edge>> {
    let mut by_key: BTreeMap<&str, Vec<&Edge>> = BTreeMap::new();
    for e in graph.edges.iter().filter(|e| CODE.contains(&e.kind)) {
        by_key.entry(if up { e.target.as_str() } else { e.source.as_str() }).or_default().push(e);
    }
    by_key
}

fn walk(graph: &Graph, root: &str, depth: usize, up: bool) -> Vec<Vec<Dependent>> {
    let by_key = index(graph, up);
    let start = seeds(graph, root);
    let mut seen: BTreeSet<String> = start.iter().cloned().collect();
    let mut frontier = start;
    let mut layers = Vec::new();
    for d in 1..=depth {
        let mut next: Vec<Dependent> = Vec::new();
        for at in &frontier {
            for e in step(graph, &by_key, at, up) {
                let other = if up { e.source.clone() } else { canonical(graph, &e.target).unwrap_or_else(|| e.target.clone()) };
                if seen.insert(other.clone()) {
                    next.push(Dependent { id: other, depth: d, kind: e.kind, via: at.clone() });
                }
            }
        }
        if next.is_empty() { break }
        next.sort_by(|a, b| a.id.cmp(&b.id));
        frontier = next.iter().map(|x| x.id.clone()).collect();
        layers.push(next);
    }
    layers
}

/// Files that name the symbol without necessarily calling it: every importer of its name from
/// its file or any barrel, and the barrels themselves — a barrel that re-exports the symbol
/// names it as surely as an importer does, and `export { AuthService } from './auth.service.js'`
/// breaks before any caller when the class is renamed. It was the one file `impact AuthService`
/// left out on the bench corpus (9 of 10, 2026-09-03).
fn importers(graph: &Graph, root: &str) -> Vec<String> {
    let Some(n) = graph.nodes.get(root) else { return Vec::new() };
    let name = bare(name_of(root));
    let mut files: BTreeSet<String> = BTreeSet::from([format!("file:{}", n.file)]);
    let mut out: BTreeSet<String> = BTreeSet::new();
    for a in aliases(graph, root) {
        if let Some((f, _)) = a.trim_start_matches("sym:").rsplit_once("::") {
            files.insert(format!("file:{f}"));
            // A re-export cycle can walk back to the declaring file itself; it names the
            // symbol by declaring it, not by importing it.
            if f != n.file { out.insert(f.to_string()); }
        }
    }
    for e in graph.edges.iter().filter(|e| e.kind == EdgeKind::Imports && files.contains(&e.target) && exports(e, name)) {
        out.insert(e.source.trim_start_matches("file:").to_string());
    }
    out.into_iter().collect()
}

pub fn upstream(graph: &Graph, root: &str, depth: usize) -> Impact {
    Impact { root: root.to_string(), layers: walk(graph, root, depth, true), importers: importers(graph, root) }
}

pub fn downstream(graph: &Graph, root: &str, depth: usize) -> Impact {
    Impact { root: root.to_string(), layers: walk(graph, root, depth, false), importers: Vec::new() }
}

/// The shortest chain of code edges from `from` to `to` (or one of its aliases or members),
/// at most `depth` hops.
/// `trace` as an object: the two ends the ids resolved to, the depth asked for, and the chain as
/// `{id, at}` steps — `null` when there is none within that depth, which is an answer and not an
/// error.
pub fn trace_json(graph: &Graph, from: &str, to: &str, depth: usize, path: Option<&[String]>) -> String {
    #[derive(serde::Serialize)]
    struct Step<'a> { id: &'a str, at: String }
    #[derive(serde::Serialize)]
    struct Out<'a> { from: &'a str, to: &'a str, depth: usize, path: Option<Vec<Step<'a>>> }
    let steps = path.map(|p| p.iter().map(|id| Step {
        id,
        at: graph.nodes.get(id).map(|n| format!("{}:{}", n.file, n.line)).unwrap_or_default(),
    }).collect());
    serde_json::to_string(&Out { from, to, depth, path: steps }).unwrap_or_else(|_| "{}".to_string())
}

pub fn trace(graph: &Graph, from: &str, to: &str, depth: usize) -> Option<Vec<String>> {
    let goal: BTreeSet<String> = seeds(graph, to).into_iter().collect();
    let by_source = index(graph, false);
    let mut parent: BTreeMap<String, String> = BTreeMap::new();
    // The walk starts at `from` alone: `step` already reaches through its members, and the
    // printed path then names the class, not the member that happened to make the call.
    let mut frontier: Vec<String> = vec![from.to_string()];
    let mut seen: BTreeSet<String> = frontier.iter().cloned().collect();
    for _ in 0..depth {
        let mut next = Vec::new();
        for at in &frontier {
            for e in step(graph, &by_source, at, false) {
                let other = canonical(graph, &e.target).unwrap_or_else(|| e.target.clone());
                if !seen.insert(other.clone()) { continue }
                parent.insert(other.clone(), at.clone());
                if goal.contains(&other) {
                    let mut path = vec![other];
                    while let Some(p) = parent.get(path.last().unwrap()) { path.push(p.clone()); }
                    path.reverse();
                    return Some(path);
                }
                next.push(other);
            }
        }
        if next.is_empty() { break }
        frontier = next;
    }
    None
}

/// Fixed thresholds, printed beside the counts that produced them so the label can be argued
/// with and the list still used.
pub fn risk(direct: usize, _total: usize, files: usize) -> &'static str {
    if direct >= 30 || files >= 25 { "CRITICAL" }
    else if direct >= 15 || files >= 10 { "HIGH" }
    else if direct >= 5 || files >= 3 { "MEDIUM" }
    else { "LOW" }
}

pub fn files(graph: &Graph, imp: &Impact) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = imp.importers.iter().cloned().collect();
    for d in imp.layers.iter().flatten() {
        match graph.nodes.get(&d.id) {
            Some(n) => { out.insert(n.file.clone()); }
            None => { out.insert(d.id.trim_start_matches("file:").to_string()); }
        }
    }
    out
}

fn line_of(graph: &Graph, id: &str) -> String {
    match graph.nodes.get(id) { Some(n) => format!("{}:{}", n.file, n.line), None => "?".to_string() }
}

const LAYER_NAMES: [&str; 3] = ["will break", "likely affected", "may need testing"];

pub fn render(graph: &Graph, imp: &Impact, direction: &str) -> String {
    let up = direction == "upstream";
    let mut out = format!("{}  {}\n", imp.root, line_of(graph, &imp.root));
    for (i, layer) in imp.layers.iter().enumerate() {
        let name = if up { LAYER_NAMES.get(i).copied().unwrap_or("transitive") } else { "reaches" };
        out.push_str(&format!("d={}  {name} ({})\n", i + 1, layer.len()));
        for d in layer {
            let arrow = if up { "→" } else { "←" };
            out.push_str(&format!("  {}  {}  {:?} {arrow} {}\n", d.id, line_of(graph, &d.id), d.kind, d.via));
        }
    }
    if up {
        if !imp.importers.is_empty() { out.push_str(&format!("importers ({}): {}\n", imp.importers.len(), imp.importers.join(", "))); }
        let direct = imp.layers.first().map_or(0, Vec::len);
        let total: usize = imp.layers.iter().map(Vec::len).sum();
        let files = files(graph, imp).len();
        out.push_str(&format!("risk: {} — {direct} direct, {total} total, {files} files\n", risk(direct, total, files)));
    }
    out
}

pub fn render_json(graph: &Graph, imp: &Impact, direction: &str) -> String {
    let direct = imp.layers.first().map_or(0, Vec::len);
    let total: usize = imp.layers.iter().map(Vec::len).sum();
    let files = files(graph, imp);
    let layers: Vec<Vec<serde_json::Value>> = imp.layers.iter().map(|l| l.iter().map(|d| serde_json::json!({
        "id": d.id, "at": line_of(graph, &d.id), "depth": d.depth, "kind": format!("{:?}", d.kind), "via": d.via,
    })).collect()).collect();
    serde_json::json!({
        "root": imp.root, "at": line_of(graph, &imp.root), "direction": direction, "layers": layers,
        "importers": imp.importers, "files": files, "direct": direct, "total": total, "risk": risk(direct, total, files.len()),
    }).to_string() + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Extraction, NodeKind};

    /// controller.create → service.create; module constructs the service; a barrel re-exports
    /// the service and a worker calls it through the barrel; a job extends the worker.
    fn graph() -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::File, "file:s.ts", "s.ts", "", "s.ts", 1);
        e.node(NodeKind::Symbol, "sym:s.ts::S", "S", "", "s.ts", 3);
        e.node(NodeKind::Symbol, "sym:s.ts::S.create", "S.create", "", "s.ts", 5);
        e.edge("file:s.ts", "sym:s.ts::S", EdgeKind::Declares, "export", "s.ts");
        e.edge("sym:s.ts::S", "sym:s.ts::S.create", EdgeKind::Declares, "", "s.ts");
        e.node(NodeKind::File, "file:c.ts", "c.ts", "", "c.ts", 1);
        e.node(NodeKind::Symbol, "sym:c.ts::C.create", "C.create", "", "c.ts", 9);
        e.edge("file:c.ts", "file:s.ts", EdgeKind::Imports, "S", "c.ts");
        e.edge("sym:c.ts::C.create", "sym:s.ts::S.create", EdgeKind::Calls, "", "c.ts");
        e.node(NodeKind::File, "file:m.ts", "m.ts", "", "m.ts", 1);
        e.edge("file:m.ts", "file:s.ts", EdgeKind::Imports, "S", "m.ts");
        e.edge("file:m.ts", "sym:s.ts::S", EdgeKind::Calls, "", "m.ts");
        e.node(NodeKind::File, "file:index.ts", "index.ts", "", "index.ts", 1);
        e.edge("file:index.ts", "file:s.ts", EdgeKind::ReExports, "*", "index.ts");
        e.node(NodeKind::File, "file:w.ts", "w.ts", "", "w.ts", 1);
        e.node(NodeKind::Symbol, "sym:w.ts::W.run", "W.run", "", "w.ts", 4);
        e.edge("file:w.ts", "file:index.ts", EdgeKind::Imports, "S", "w.ts");
        e.edge("sym:w.ts::W.run", "sym:index.ts::S.create", EdgeKind::Calls, "", "w.ts");
        e.node(NodeKind::Symbol, "sym:w.ts::W", "W", "", "w.ts", 3);
        e.edge("sym:w.ts::W", "sym:w.ts::W.run", EdgeKind::Declares, "", "w.ts");
        e.node(NodeKind::Symbol, "sym:j.ts::J", "J", "", "j.ts", 2);
        e.edge("sym:j.ts::J", "sym:w.ts::W", EdgeKind::Extends, "", "j.ts");
        g.apply(e);
        g
    }

    #[test]
    fn aliases_follow_re_exports_back_to_every_barrel() {
        assert_eq!(aliases(&graph(), "sym:s.ts::S.create"), vec!["sym:index.ts::S.create"]);
        assert_eq!(aliases(&graph(), "sym:s.ts::S"), vec!["sym:index.ts::S"]);
    }

    #[test]
    fn canonical_walks_a_barrel_forward_to_the_declaration() {
        assert_eq!(canonical(&graph(), "sym:index.ts::S.create").as_deref(), Some("sym:s.ts::S.create"));
        assert_eq!(canonical(&graph(), "sym:s.ts::S.create").as_deref(), Some("sym:s.ts::S.create"));
        assert_eq!(canonical(&graph(), "sym:index.ts::Nope"), None);
    }

    #[test]
    fn upstream_of_a_class_reaches_callers_of_its_members_and_through_barrels() {
        let imp = upstream(&graph(), "sym:s.ts::S", 3);
        let d1: Vec<&str> = imp.layers[0].iter().map(|d| d.id.as_str()).collect();
        assert_eq!(d1, vec!["file:m.ts", "sym:c.ts::C.create", "sym:w.ts::W.run"]);
        // J extends W, and W.run is the caller: the subclass inherits the call, the class
        // itself is not listed as a dependent of its own member.
        let d2: Vec<(&str, EdgeKind)> = imp.layers[1].iter().map(|d| (d.id.as_str(), d.kind)).collect();
        assert_eq!(d2, vec![("sym:j.ts::J", EdgeKind::Extends)]);
        assert_eq!(imp.layers.len(), 2);
        assert_eq!(imp.importers, vec!["c.ts", "index.ts", "m.ts", "w.ts"]);
    }

    #[test]
    fn a_re_export_cycle_does_not_name_the_declaring_file_as_its_own_importer() {
        // b re-exports s, a re-exports b, and s re-exports a: aliases() walks the cycle all the
        // way back to s.ts itself, which must not then claim to import the symbol it declares.
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::File, "file:s.ts", "s.ts", "", "s.ts", 1);
        e.node(NodeKind::Symbol, "sym:s.ts::S", "S", "", "s.ts", 3);
        e.edge("file:s.ts", "sym:s.ts::S", EdgeKind::Declares, "export", "s.ts");
        e.node(NodeKind::File, "file:b.ts", "b.ts", "", "b.ts", 1);
        e.edge("file:b.ts", "file:s.ts", EdgeKind::ReExports, "*", "b.ts");
        e.node(NodeKind::File, "file:a.ts", "a.ts", "", "a.ts", 1);
        e.edge("file:a.ts", "file:b.ts", EdgeKind::ReExports, "*", "a.ts");
        e.edge("file:s.ts", "file:a.ts", EdgeKind::ReExports, "*", "s.ts");
        g.apply(e);
        let imp = upstream(&g, "sym:s.ts::S", 1).importers;
        assert!(!imp.contains(&"s.ts".to_string()), "{imp:?}");
        assert_eq!(imp, vec!["a.ts", "b.ts"]);
    }

    #[test]
    fn upstream_of_a_member_is_narrower_than_its_class() {
        let imp = upstream(&graph(), "sym:s.ts::S.create", 1);
        let d1: Vec<&str> = imp.layers[0].iter().map(|d| d.id.as_str()).collect();
        assert_eq!(d1, vec!["sym:c.ts::C.create", "sym:w.ts::W.run"]);
    }

    #[test]
    fn depth_caps_the_walk_and_a_dependent_appears_once_at_its_shallowest() {
        let imp = upstream(&graph(), "sym:s.ts::S", 1);
        assert_eq!(imp.layers.len(), 1);
        let deep = upstream(&graph(), "sym:s.ts::S", 9);
        let all: Vec<&str> = deep.layers.iter().flatten().map(|d| d.id.as_str()).collect();
        let set: BTreeSet<&str> = all.iter().copied().collect();
        assert_eq!(all.len(), set.len());
    }

    #[test]
    fn downstream_canonicalises_a_barrel_target() {
        let imp = downstream(&graph(), "sym:w.ts::W", 2);
        let d1: Vec<&str> = imp.layers[0].iter().map(|d| d.id.as_str()).collect();
        assert_eq!(d1, vec!["sym:s.ts::S.create"]);
    }

    #[test]
    fn trace_finds_the_shortest_call_path_and_reports_none_when_there_is_no_path() {
        // J → W by Extends, W → S.create through its member W.run and the barrel alias.
        assert_eq!(trace(&graph(), "sym:j.ts::J", "sym:s.ts::S", 6), Some(vec!["sym:j.ts::J".into(), "sym:w.ts::W".into(), "sym:s.ts::S.create".into()]));
        assert_eq!(trace(&graph(), "sym:s.ts::S", "sym:j.ts::J", 6), None);
        assert_eq!(trace(&graph(), "sym:j.ts::J", "sym:s.ts::S", 1), None);
    }

    #[test]
    fn risk_thresholds_are_the_documented_ones() {
        assert_eq!(risk(0, 0, 0), "LOW");
        assert_eq!(risk(4, 9, 2), "LOW");
        assert_eq!(risk(5, 5, 1), "MEDIUM");
        assert_eq!(risk(1, 1, 3), "MEDIUM");
        assert_eq!(risk(15, 15, 1), "HIGH");
        assert_eq!(risk(2, 40, 10), "HIGH");
        assert_eq!(risk(30, 30, 1), "CRITICAL");
        assert_eq!(risk(1, 60, 25), "CRITICAL");
    }

    #[test]
    fn render_lists_layers_with_path_line_and_ends_with_the_risk() {
        let g = graph();
        let out = render(&g, &upstream(&g, "sym:s.ts::S", 3), "upstream");
        assert!(out.starts_with("sym:s.ts::S  s.ts:3"));
        assert!(out.contains("d=1  will break (3)\n"));
        assert!(out.contains("  sym:c.ts::C.create  c.ts:9  Calls → sym:s.ts::S.create\n"));
        assert!(out.contains("importers (4): c.ts, index.ts, m.ts, w.ts\n"));
        assert!(out.ends_with("risk: MEDIUM — 3 direct, 4 total, 5 files\n"), "{out}");
    }

    #[test]
    fn render_json_is_valid_and_carries_the_same_counts() {
        let g = graph();
        let v: serde_json::Value = serde_json::from_str(&render_json(&g, &upstream(&g, "sym:s.ts::S", 3), "upstream")).unwrap();
        assert_eq!(v["root"], "sym:s.ts::S");
        assert_eq!(v["direction"], "upstream");
        assert_eq!(v["layers"][0].as_array().unwrap().len(), 3);
        assert_eq!(v["risk"], "MEDIUM");
    }
}
