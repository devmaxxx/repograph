//! HCL. Until its declarations are read, a globbed `.tf` or `.hcl` file is its file node, so the census and
//! the dispatch can run through `Lang` before any walk exists.

use crate::code::imports::Resolver;
use crate::model::Extraction;

pub fn extract(_resolver: &Resolver, rel: &str, _source: &str) -> Extraction {
    let mut ex = Extraction::default();
    crate::code::lang::file_node(rel, &mut ex);
    ex
}
