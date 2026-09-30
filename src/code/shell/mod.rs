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
use crate::model::Extraction;
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

/// Every function definition, nested ones included: a function defined inside another is callable
/// once the outer one has run.
fn functions<'t>(root: Node<'t>) -> Vec<Node<'t>> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.kind() == "function_definition" { out.push(n) }
        stack.extend(prose::named(n));
    }
    out.sort_by_key(|n| n.start_byte());
    out
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

    /// Functions `rel` can call through what it sources, directly or not: name → the defining script.
    /// Breadth-first, and the nearest definition wins; a name two sourced scripts both define is rare
    /// enough that reading which `source` line runs last is not worth it.
    fn visible<'a>(&'a self, rel: &'a str) -> BTreeMap<&'a str, &'a str> {
        let mut out = BTreeMap::new();
        let mut seen = BTreeSet::from([rel]);
        let mut queue: VecDeque<&str> = self.sourced(rel).into();
        while let Some(script) = queue.pop_front() {
            if !seen.insert(script) { continue }
            for f in self.functions.get(script).into_iter().flatten() {
                out.entry(f.as_str()).or_insert(script);
            }
            queue.extend(self.sourced(script));
        }
        out
    }
}
