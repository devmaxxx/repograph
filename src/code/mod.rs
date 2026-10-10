pub mod bicep;
pub mod blank;
pub mod dart;
pub mod dotnet;
pub mod graphql;
pub mod hcl;
pub mod imports;
pub mod index;
pub mod jvm;
pub mod lang;
pub(crate) mod prose;
pub(crate) mod python;
pub(crate) mod reader;
pub(crate) mod rust_lang;
pub mod shell;
pub mod sql;
pub mod swift;
pub(crate) mod syntax;
pub mod typescript;

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
        let ex = match lang::Lang::of(rel) {
            Some(lang) => reader::extract(lang, &self.resolver, rel, text),
            None => {
                let mut ex = Extraction::default();
                lang::file_node(rel, &mut ex);
                ex
            }
        };
        finish(ex)
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
