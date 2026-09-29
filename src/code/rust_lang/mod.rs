//! Rust: one parse per file, the module tree read from paths and `Cargo.toml` rather than from
//! other files' contents, so extracting a file never opens another.

use crate::code::imports::Resolver;
use crate::code::lang::{file_node, Lang};
use crate::model::Extraction;

#[cfg(test)]
mod cases;

pub fn extract(_resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let mut ex = Extraction::default();
    file_node(rel, &mut ex);
    let Some(_tree) = Lang::Rust.parse(source.as_bytes()) else { return ex };
    ex
}
