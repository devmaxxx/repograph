pub mod bicep;
pub mod blank;
pub mod calls;
pub mod csharp;
pub mod dart;
pub mod graphql;
pub mod hcl;
pub mod idrefs;
pub mod imports;
pub mod index;
pub mod java;
pub mod jvm;
pub mod kotlin;
pub mod lang;
pub(crate) mod prose;
pub(crate) mod python;
pub mod razor;
pub(crate) mod rust_lang;
pub mod shell;
pub mod sql;
pub mod swift;
pub mod symbols;
pub mod vue;

use crate::model::{Extraction, Extractor};

pub struct CodeExtractor {
    symbols: symbols::SymbolScanner,
}

impl CodeExtractor {
    pub fn new(resolver: imports::Resolver) -> CodeExtractor {
        CodeExtractor { symbols: symbols::SymbolScanner::new(resolver) }
    }
}

impl Extractor for CodeExtractor {
    fn extract(&self, rel: &str, text: &str) -> Extraction {
        let ex = match lang::Lang::of(rel) {
            Some(lang::Lang::TypeScript | lang::Lang::Tsx) => self.typescript(rel, text),
            Some(lang::Lang::CSharp) => csharp::extract(self.symbols.resolver(), rel, text),
            Some(lang::Lang::Razor) => razor::extract(self.symbols.resolver(), rel, text),
            Some(lang::Lang::Kotlin) => kotlin::extract(self.symbols.resolver(), rel, text),
            Some(lang::Lang::Java) => java::extract(self.symbols.resolver(), rel, text),
            Some(lang::Lang::Sql) => sql::extract(self.symbols.resolver(), rel, text),
            Some(lang::Lang::GraphQl) => graphql::extract(self.symbols.resolver(), rel, text),
            Some(lang::Lang::Rust) => rust_lang::extract(self.symbols.resolver(), rel, text),
            Some(lang::Lang::Python) => python::extract(self.symbols.resolver(), rel, text),
            Some(lang::Lang::Bicep) => bicep::extract(self.symbols.resolver(), rel, text),
            Some(lang::Lang::Hcl) => hcl::extract(self.symbols.resolver(), rel, text),
            Some(lang::Lang::Shell) => shell::extract(self.symbols.resolver(), rel, text),
            Some(lang::Lang::Dart) => dart::extract(self.symbols.resolver(), rel, text),
            Some(lang::Lang::Vue) => vue::extract(self.symbols.resolver(), rel, text),
            Some(lang::Lang::Swift) => swift::extract(self.symbols.resolver(), rel, text),
            None => {
                let mut ex = Extraction::default();
                lang::file_node(rel, &mut ex);
                ex
            }
            // Each family plan adds its arm above this one. Once every language has one, this arm
            // is unreachable, and harmless.
            #[allow(unreachable_patterns)]
            Some(other) => unreachable!("Lang::of returned {other:?}, which has no extractor arm"),
        };
        finish(ex)
    }
}

impl CodeExtractor {
    /// The shipped path, three passes and three parses. One parse for it is a refactor nothing
    /// measured, and not in this release.
    fn typescript(&self, rel: &str, text: &str) -> Extraction {
        let mut ex = self.symbols.scan(rel, text);
        // Top-level names this file declares; a member id carries a dot and is not one.
        let prefix = format!("sym:{rel}::");
        let locals: std::collections::BTreeSet<String> = ex.nodes.iter()
            .filter_map(|n| n.id.strip_prefix(&prefix))
            .filter(|n| !n.contains('.'))
            .map(str::to_string)
            .collect();
        calls::scan(self.symbols.resolver(), rel, text, &locals, &mut ex);
        idrefs::scan(rel, text, &mut ex);
        ex
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
