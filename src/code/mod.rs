pub(crate) mod blank;
pub(crate) mod dotnet;
pub mod engine;
pub(crate) mod exceptions;
pub(crate) mod front;
pub(crate) mod imports;
pub(crate) mod index;
pub(crate) mod jvm;
pub(crate) mod lang;
pub(crate) mod prose;
pub mod queries;
pub(crate) mod spec;
pub(crate) mod syntax;
pub(crate) mod typescript;

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
