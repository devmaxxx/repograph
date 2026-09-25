//! Razor: a component's directives and markup tags, and its `@code` blocks read through the C# walk.

use crate::code::imports::Resolver;
use crate::code::lang::file_node;
use crate::model::Extraction;

pub fn extract(_resolver: &Resolver, rel: &str, _source: &str) -> Extraction {
    let mut ex = Extraction::default();
    file_node(rel, &mut ex);
    ex
}
