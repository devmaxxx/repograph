//! The one read of a code file: which language reads it, the view an embedding language keeps of
//! it, one parse, the file node, and then the engine over that tree. Nothing after it parses the file
//! again or writes its file node.

use std::borrow::Cow;

use crate::code::engine::{self, File};
use crate::code::imports::Resolver;
use crate::code::lang::{detect, file_node};
use crate::code::spec::{spec, Embedded};
use crate::model::{Extraction, NodeKind};

/// What `rel` reads as, before `finish`.
pub(crate) fn read(resolver: &Resolver, rel: &str, text: &str) -> Extraction {
    let mut ex = Extraction::default();
    let Some(lang) = detect(rel, Some(text)) else {
        file_node(rel, &mut ex);
        return ex;
    };
    let spec = spec(lang);
    let (src, grammar) = match spec.hooks.embed.map(|embed| embed(text)) {
        None => (Cow::Borrowed(text), Some(lang)),
        Some(Embedded::Unread) => {
            file_node(rel, &mut ex);
            return ex;
        }
        Some(Embedded::Empty) => (Cow::Borrowed(text), None),
        Some(Embedded::View(view, by)) => (Cow::Owned(view), Some(by)),
    };
    let tree = grammar.and_then(|g| g.parse(src.as_bytes()));
    let file = File { lang, rel, text, src: &src, tree: tree.as_ref() };
    let head = spec.hooks.head.map(|head| head(&file)).unwrap_or_default();
    ex.node(NodeKind::File, &format!("file:{rel}"), rel, &head, rel, 1);
    engine::extract(resolver, spec, &file, &mut ex);
    ex
}
