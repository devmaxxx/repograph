//! What a language cannot say in data: one file per resolution rule, named for the rule and never
//! for a language. A rule two languages share is one file. A rule stays only while real code needs
//! it: without it, the default loses edges the corpora or the fixture hold.

pub(crate) mod attachments;
pub(crate) mod computed_paths;
pub(crate) mod identifier_case;
pub(crate) mod library_scope;
pub(crate) mod module_paths;
pub(crate) mod module_reach;
