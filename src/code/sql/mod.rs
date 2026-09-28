//! PostgreSQL. Until the statement walk lands, a `.sql` file is its file node, as L1 already makes it.
use crate::code::imports::Resolver;
use crate::model::Extraction;

pub fn extract(_resolver: &Resolver, rel: &str, _source: &str) -> Extraction {
    let mut ex = Extraction::default();
    crate::code::lang::file_node(rel, &mut ex);
    ex
}
