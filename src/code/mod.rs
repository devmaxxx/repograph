pub mod idrefs;
pub mod imports;
pub mod symbols;

use crate::ids::IdMatcher;
use crate::model::{Extraction, Extractor};

pub struct CodeExtractor {
    symbols: symbols::SymbolScanner,
    ids: IdMatcher,
}

impl CodeExtractor {
    pub fn new(resolver: imports::Resolver, ids: IdMatcher) -> CodeExtractor {
        CodeExtractor { symbols: symbols::SymbolScanner::new(resolver), ids }
    }
}

impl Extractor for CodeExtractor {
    fn extract(&self, rel: &str, text: &str) -> Extraction {
        let mut ex = self.symbols.scan(rel, text);
        idrefs::scan(&self.ids, rel, text, &mut ex);
        ex.edges.sort();
        ex.edges.dedup();
        ex
    }
}
