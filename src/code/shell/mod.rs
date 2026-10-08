//! Shell. Every function definition is a symbol, and all are exported, because any script that
//! sources the file can call them. `paths.rs` evaluates the paths a script names through its own
//! directory; `commands.rs` turns each command into a source, a call or a run.

mod commands;
mod paths;
#[cfg(test)]
mod cases;

use crate::code::imports::Resolver;
use crate::code::lang::{file_node, Lang};
use crate::code::prose::{self, Spans};
use crate::model::{EdgeKind, Extraction, Graph};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use tree_sitter::Node;

pub fn extract(resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let mut ex = Extraction::default();
    file_node(rel, &mut ex);
    let src = source.as_bytes();
    let Some(tree) = Lang::Shell.parse(src) else { return ex };
    let root = tree.root_node();
    let file = format!("file:{rel}");
    let mut spans = Spans::new(rel);
    for f in functions(root) {
        let Some(name) = f.child_by_field_name("name") else { continue };
        let id = format!("sym:{rel}::{}", prose::text(name, src));
        prose::declare(&mut ex, rel, &file, &id, f, &prose::body(f, src, &[]), "export");
        spans.push(f, &id);
    }
    commands::write(resolver.shell(), root, &spans, src, rel, &mut ex);
    prose::cite(root, src, rel, &["string", "raw_string", "heredoc_body"], &spans, &mut ex);
    ex
}

/// `apply_diff`'s widening for scripts. Bash resolves a called name across the files a script sources
/// with no header to compare, so a changed or removed script can add or drop a function that a script
/// sourcing it calls: every script sourcing it, directly or through another, is read again. A script
/// whose `source` named a file that did not exist yet has no `Imports` edge to follow, and waits for
/// its own next edit.
pub(crate) fn widen(stale: &[String], removed: &[String], graph: &Graph, all_rels: &[String]) -> Vec<String> {
    let mut reached: BTreeSet<String> = stale.iter().chain(removed)
        .filter(|r| Lang::of(r) == Some(Lang::Shell))
        .map(|r| format!("file:{r}"))
        .collect();
    // Grouped once: a chain of `source` lines is as many hops as it is long, and each hop would
    // otherwise be a scan of every edge in the graph.
    let mut sourced_by: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for e in graph.edges.iter().filter(|e| e.kind == EdgeKind::Imports) {
        sourced_by.entry(e.target.as_str()).or_default().push(e.source.as_str());
    }
    let mut frontier: Vec<String> = reached.iter().cloned().collect();
    while let Some(file) = frontier.pop() {
        for &importer in sourced_by.get(file.as_str()).into_iter().flatten() {
            if reached.insert(importer.to_string()) { frontier.push(importer.to_string()); }
        }
    }
    let stale: BTreeSet<&str> = stale.iter().map(String::as_str).collect();
    let present: BTreeSet<&str> = all_rels.iter().map(String::as_str).collect();
    reached.iter()
        .filter_map(|id| id.strip_prefix("file:"))
        .filter(|r| !stale.contains(r) && present.contains(r))
        .map(str::to_string)
        .collect()
}

/// Every function definition, nested ones included: a function defined inside another is callable
/// once the outer one has run.
fn functions<'t>(root: Node<'t>) -> Vec<Node<'t>> {
    prose::all(root, "function_definition")
}

/// What every globbed script offers the others, read before any extract, so a call resolves whatever
/// order the walk reads files in.
#[derive(Default)]
pub(crate) struct Scripts {
    /// script → the functions it defines; every globbed script has an entry, functions or not
    functions: BTreeMap<String, BTreeSet<String>>,
    /// script → each `source` it names, as candidate paths in the order they are tried
    sources: BTreeMap<String, Vec<Vec<String>>>,
}

impl Scripts {
    pub(crate) fn add(&mut self, rel: &str, source: &str) {
        let src = source.as_bytes();
        let Some(tree) = Lang::Shell.parse(src) else {
            self.functions.entry(rel.to_string()).or_default();
            return;
        };
        let root = tree.root_node();
        let names = functions(root).into_iter()
            .filter_map(|f| f.child_by_field_name("name"))
            .map(|n| prose::text(n, src).to_string())
            .collect();
        self.functions.insert(rel.to_string(), names);
        self.sources.insert(rel.to_string(), commands::sources(root, src, rel));
    }

    fn defines(&self, rel: &str, name: &str) -> bool {
        self.functions.get(rel).is_some_and(|f| f.contains(name))
    }

    /// The first candidate that is a globbed script. The key is returned, not the candidate, so the
    /// answer outlives candidates computed on the spot.
    fn pick<'a>(&'a self, candidates: &[String]) -> Option<&'a str> {
        candidates.iter().find_map(|c| self.functions.get_key_value(c.as_str()).map(|(k, _)| k.as_str()))
    }

    fn sourced<'a>(&'a self, rel: &str) -> Vec<&'a str> {
        self.sources.get(rel).into_iter().flatten().filter_map(|c| self.pick(c)).collect()
    }

    /// Functions `rel` can call through what it sources, directly or not: name → every script that
    /// defines it. Which definition a call reaches is whichever `source` ran last, and the graph does
    /// not model run order, so a name with more than one definition is left for the caller to drop.
    fn visible<'a>(&'a self, rel: &'a str) -> BTreeMap<&'a str, Vec<&'a str>> {
        let mut out: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let mut seen = BTreeSet::from([rel]);
        let mut queue: VecDeque<&str> = self.sourced(rel).into();
        while let Some(script) = queue.pop_front() {
            if !seen.insert(script) { continue }
            for f in self.functions.get(script).into_iter().flatten() {
                out.entry(f.as_str()).or_default().push(script);
            }
            queue.extend(self.sourced(script));
        }
        out
    }
}
