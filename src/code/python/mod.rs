//! Python: one parse per file; module names are read from paths and project manifests, so
//! extracting a file never opens another.

use crate::code::imports::Resolver;
use crate::code::lang::{file_node, Lang};
use crate::model::Extraction;

mod modules;

pub use modules::Modules;

#[cfg(test)]
mod cases;

pub fn extract(_resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let mut ex = Extraction::default();
    file_node(rel, &mut ex);
    let Some(_tree) = Lang::Python.parse(source.as_bytes()) else { return ex };
    ex
}
