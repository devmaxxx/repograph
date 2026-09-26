//! Java, read by a hand walk into the node and edge kinds TypeScript writes (spec "### Java").

use crate::code::imports::Resolver;
use crate::model::Extraction;

/// The file node alone until the declarations walk lands; a `.java` file is never read as TypeScript.
pub fn extract(_resolver: &Resolver, rel: &str, _source: &str) -> Extraction {
    let mut ex = Extraction::default();
    crate::code::lang::file_node(rel, &mut ex);
    ex
}
