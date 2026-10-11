pub mod bicep;
pub mod blank;
pub mod dart;
pub(crate) mod dotnet;
pub(crate) mod engine;
pub(crate) mod exceptions;
pub(crate) mod front;
pub mod graphql;
pub mod hcl;
pub mod imports;
pub mod index;
pub mod jvm;
pub mod lang;
pub(crate) mod prose;
pub(crate) mod python;
pub(crate) mod queries;
pub(crate) mod rust_lang;
pub(crate) mod spec;
pub mod sql;
pub mod swift;
pub(crate) mod syntax;
pub(crate) mod typescript;

use crate::model::{Extraction, Extractor};

pub struct CodeExtractor {
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
