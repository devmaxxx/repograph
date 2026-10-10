//! The TypeScript family: TypeScript and JavaScript, and Vue single-file components whose script
//! resolves names through the same resolver. A TypeScript file takes three passes and three parses;
//! one parse for all three is a refactor nothing measured.

pub mod calls;
pub mod idrefs;
pub mod symbols;
pub mod vue;

use crate::code::imports::Resolver;
use crate::code::reader::{self, Reader};
use crate::model::Extraction;

/// Its resolver state is the tsconfig and package.json `Resolver::new` reads, which no other
/// language shares; its sources are the extractor's alone.
pub(crate) const READER: Reader = Reader { extract, ..reader::NONE };

fn extract(resolver: &Resolver, rel: &str, text: &str, ex: &mut Extraction) {
    // The walk writes the file node itself, with the file head as its body.
    *ex = symbols::scan(resolver, rel, text);
    // Top-level names this file declares; a member id carries a dot and is not one.
    let prefix = format!("sym:{rel}::");
    let locals: std::collections::BTreeSet<String> = ex.nodes.iter()
        .filter_map(|n| n.id.strip_prefix(&prefix))
        .filter(|n| !n.contains('.'))
        .map(str::to_string)
        .collect();
    calls::scan(resolver, rel, text, &locals, ex);
    idrefs::scan(rel, text, ex);
}
