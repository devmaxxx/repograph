//! Rust extraction cases, each on an inline source so the grammar's shape for the construct is
//! pinned by the assertion rather than by a fixture.

use crate::code::CodeExtractor;
use crate::code::imports::Resolver;
use crate::code::lang::Lang;
use crate::config::Config;
use crate::model::{EdgeKind, Extraction, Extractor, NodeKind};

pub(super) struct Repo {
    dir: tempfile::TempDir,
}

impl Repo {
    pub(super) fn new(files: &[(&str, &str)]) -> Repo {
        let dir = tempfile::tempdir().unwrap();
        for (p, c) in files {
            let full = dir.path().join(p);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, c).unwrap();
        }
        Repo { dir }
    }

    pub(super) fn extract(&self, rel: &str) -> Extraction {
        // The language is not in the defaults until L7's PR, so the case names its glob, as a user would.
        let cfg = Config { code_globs: vec!["**/*.rs".into()], ..Config::default() };
        let resolver = Resolver::new(self.dir.path(), &cfg).unwrap();
        let src = std::fs::read_to_string(self.dir.path().join(rel)).unwrap();
        CodeExtractor::new(resolver).extract(rel, &src)
    }
}

// Shared by the extraction cases the later tasks add to this file.
#[allow(dead_code)]
pub(super) fn ids(ex: &Extraction) -> Vec<&str> {
    ex.nodes.iter().map(|n| n.id.as_str()).collect()
}

#[allow(dead_code)]
pub(super) fn edges(ex: &Extraction, kind: EdgeKind) -> Vec<(&str, &str, &str)> {
    ex.edges.iter().filter(|e| e.kind == kind).map(|e| (e.source.as_str(), e.target.as_str(), e.context.as_str())).collect()
}

#[test]
fn a_rust_file_is_one_file_node_and_parses_clean() {
    let src = "pub struct S<'a> { r: &'a str }\nfn f() -> char { '{' }\nconst RAW: &str = r#\"}\"#;\n";
    let repo = Repo::new(&[("src/lib.rs", src)]);
    let ex = repo.extract("src/lib.rs");
    assert!(ex.nodes.iter().any(|n| n.kind == NodeKind::File && n.id == "file:src/lib.rs"));
    assert_eq!(Lang::of("src/lib.rs"), Some(Lang::Rust));
    let tree = Lang::Rust.parse(src.as_bytes()).unwrap();
    assert!(!tree.root_node().has_error(), "{}", tree.root_node().to_sexp());
}

/// Every node kind and field name the walk matches on, checked against the pinned grammar, so a
/// grammar bump that renames one fails here rather than as a silently missing edge.
#[test]
fn every_kind_the_walk_matches_is_in_the_grammar() {
    let lang = Lang::Rust.grammar().unwrap();
    for kind in [
        "source_file", "mod_item", "declaration_list", "attribute_item", "attribute", "token_tree", "line_comment",
        "block_comment", "doc_comment", "use_declaration", "scoped_use_list", "use_list", "use_wildcard", "use_as_clause",
        "scoped_identifier", "identifier", "self", "crate", "super", "visibility_modifier", "struct_item",
        "field_declaration_list", "field_declaration", "field_identifier", "type_identifier", "enum_item", "union_item",
        "trait_item", "function_signature_item", "function_item", "associated_type", "type_item", "const_item",
        "static_item", "macro_definition", "impl_item", "generic_type", "type_arguments", "scoped_type_identifier",
        "reference_type", "dynamic_type", "abstract_type", "type_parameters", "type_parameter", "trait_bounds",
        "where_clause", "where_predicate", "call_expression", "field_expression", "generic_function",
        "macro_invocation", "string_literal", "raw_string_literal", "char_literal", "lifetime", "block",
    ] {
        assert_ne!(lang.id_for_node_kind(kind, true), 0, "kind {kind} missing");
    }
    for field in ["name", "body", "argument", "path", "list", "alias", "trait", "type", "function", "value", "field", "macro", "bounds", "left", "type_parameters", "arguments"] {
        assert!(lang.field_id_for_name(field).is_some(), "field {field} missing");
    }
}
