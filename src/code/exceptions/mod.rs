//! What a language cannot say in data: one file per resolution rule, named for the rule and never
//! for a language. A rule two languages share is one file. A rule joins only when the default
//! resolution, measured on the language's files, writes wrong edges or loses too many right ones.

pub(crate) mod attribute_lines;
pub(crate) mod computed_paths;
pub(crate) mod inheritance_clauses;
pub(crate) mod module_reach;
pub(crate) mod type_paths;
