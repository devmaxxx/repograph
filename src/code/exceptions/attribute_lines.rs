//! A declaration's node starts at its first attribute, and Swift writes attributes on lines of their
//! own. The signature is the first line that does not open with one, a whole line skipped even when
//! the declaration goes on after the attribute, because that is the line the indexes have read for
//! these symbols all along.

use crate::code::engine::{Def, File};

pub(crate) fn signature(file: &File, def: &Def) -> String {
    file.src[def.range.clone()].lines().find(|l| !l.trim_start().starts_with('@')).unwrap_or("").trim().to_string()
}
