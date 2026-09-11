pub mod calls;
pub mod idrefs;
pub mod imports;
pub mod symbols;

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
        // Overloads, a getter/setter pair and repeated decorators name one symbol each.
        let mut seen = std::collections::HashSet::new();
        ex.nodes.retain(|n| seen.insert(n.id.clone()));
        ex.edges.sort();
        ex.edges.dedup();
        ex
    }
}

#[cfg(test)]
mod cases;
