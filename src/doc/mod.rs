#[cfg(test)]
mod cases;
pub mod links;
pub mod registry;
pub mod requirements;

use crate::model::{Extraction, Extractor};

pub struct DocExtractor { req: requirements::RequirementScanner }

impl Default for DocExtractor {
    fn default() -> Self { Self::new() }
}

impl DocExtractor {
    pub fn new() -> DocExtractor {
        DocExtractor { req: requirements::RequirementScanner::new() }
    }
}

impl Extractor for DocExtractor {
    fn extract(&self, rel: &str, text: &str) -> Extraction {
        let mut ex = self.req.scan(rel, text);
        links::scan(rel, text, &mut ex);
        ex
    }
}
