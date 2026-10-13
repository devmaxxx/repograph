pub(crate) mod blank;
pub mod engine;
pub(crate) mod exceptions;
pub(crate) mod front;
pub(crate) mod imports;
pub(crate) mod index;
pub(crate) mod lang;
pub(crate) mod prose;
pub(crate) mod queries;
pub(crate) mod spec;
pub(crate) mod syntax;

use crate::model::{Extraction, Extractor};

pub(crate) struct CodeExtractor {
    resolver: imports::Resolver,
}

impl CodeExtractor {
    pub fn new(resolver: imports::Resolver) -> CodeExtractor {
        CodeExtractor { resolver }
    }
}

impl Extractor for CodeExtractor {
    fn extract(&self, rel: &str, text: &str) -> Extraction {
        finish(front::read(&self.resolver, rel, text))
    }

    fn forget(&self, rels: &[String]) {
        self.resolver.forget(rels);
    }
}

/// Held for one update: what the resolver read ahead of extraction goes when it drops. An update
/// extracts only the files that moved, and a watcher or `serve` would otherwise keep every other
/// file's read-ahead for as long as the process runs.
pub(crate) struct Pass;

impl Drop for Pass {
    fn drop(&mut self) {
        forget_read_ahead();
    }
}

pub(crate) fn forget_read_ahead() {
    engine::forget_read_ahead();
    index::forget_read_ahead();
}

/// Overloads, a getter/setter pair and repeated decorators name one symbol each, in every language.
fn finish(mut ex: Extraction) -> Extraction {
    let mut seen = std::collections::HashSet::new();
    ex.nodes.retain(|n| seen.insert(n.id.clone()));
    ex.edges.sort();
    ex.edges.dedup();
    ex
}

#[cfg(test)]
mod cases;
