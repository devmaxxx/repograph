pub mod links;
pub mod registry;
pub mod requirements;

use crate::ids::IdMatcher;
use crate::model::{Extraction, Extractor};

pub struct DocExtractor { req: requirements::RequirementScanner }

impl DocExtractor {
    pub fn new(ids: IdMatcher) -> DocExtractor {
        DocExtractor { req: requirements::RequirementScanner::new(ids) }
    }
}

impl Extractor for DocExtractor {
    fn extract(&self, rel: &str, text: &str) -> Extraction {
        let mut ex = self.req.scan(rel, text);
        links::scan(rel, text, &mut ex);
        ex
    }
}
