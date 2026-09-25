//! C#: one parse per file, a declarations pass and a references pass over the same tree.

#[cfg(test)]
mod cases;

use crate::code::imports::Resolver;
use crate::code::lang::file_node;
use crate::model::Extraction;

pub fn extract(_resolver: &Resolver, rel: &str, _source: &str) -> Extraction {
    let mut ex = Extraction::default();
    file_node(rel, &mut ex);
    ex
}
