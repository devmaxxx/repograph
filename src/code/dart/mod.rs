pub mod declarations;

#[cfg(test)]
mod cases;

use crate::code::imports::Resolver;
use crate::code::lang::{file_node, Lang};
use crate::model::Extraction;

/// One parse per file; every pass reads the same tree.
pub fn extract(_resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let mut ex = Extraction::default();
    file_node(rel, &mut ex);
    let src = source.as_bytes();
    let Some(tree) = Lang::Dart.parse(src) else { return ex };
    declarations::scan(tree.root_node(), rel, src, &mut ex);
    ex
}
