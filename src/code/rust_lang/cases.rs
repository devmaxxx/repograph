//! Rust extraction cases, each on an inline source so the grammar's shape for the construct is
//! pinned by the assertion rather than by a fixture.

use crate::code::CodeExtractor;
use crate::code::imports::Resolver;
use crate::code::lang::Lang;
use crate::config::Config;
use crate::model::{EdgeKind, Extraction, Extractor, NodeKind};
use super::crates::Target;

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
        let resolver = self.resolver();
        let src = std::fs::read_to_string(self.dir.path().join(rel)).unwrap();
        CodeExtractor::new(resolver).extract(rel, &src)
    }
}

pub(super) fn ids(ex: &Extraction) -> Vec<&str> {
    ex.nodes.iter().map(|n| n.id.as_str()).collect()
}

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

impl Repo {
    pub(super) fn resolver(&self) -> Resolver {
        let cfg = Config { code_globs: vec!["**/*.rs".into()], ..Config::default() };
        Resolver::new(self.dir.path(), &cfg).unwrap()
    }
}

#[test]
fn the_resolver_reads_every_cargo_manifest_its_walk_reaches() {
    let repo = Repo::new(&[
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        ("crates/shop-core/Cargo.toml", "[package]\nname = \"shop-core\"\nversion = \"0.1.0\"\n"),
        ("crates/shop-core/src/lib.rs", "pub mod hmac;\n"),
        ("crates/shop-core/src/hmac.rs", "pub fn sign() {}\n"),
        ("crates/shopd/Cargo.toml", "[package]\nname = \"shopd\"\nversion = \"0.1.0\"\n"),
        ("crates/shopd/src/main.rs", "fn main() {}\n"),
    ]);
    let path: Vec<String> = ["shop_core", "hmac", "sign"].map(String::from).to_vec();
    assert_eq!(
        repo.resolver().rust().resolve("crates/shopd/src/main.rs", &[], &path),
        Some(Target::Item { file: "crates/shop-core/src/hmac.rs".into(), name: "sign".into() })
    );
}

fn node<'a>(ex: &'a Extraction, id: &str) -> &'a crate::model::Node {
    ex.nodes.iter().find(|n| n.id == id).unwrap_or_else(|| panic!("{id} not extracted: {:?}", ids(ex)))
}

const SHOP: &str = r#"//! Orders.

/// An order line.
#[derive(Debug, Clone)]
pub struct Line { pub sku: String, qty: u32 }

pub enum State { Open, Paid }
pub union Bits { a: u32, b: f32 }
pub(crate) type Lines = Vec<Line>;
pub const LIMIT: usize = 10;
static mut COUNT: usize = 0;

macro_rules! money { ($x:expr) => { $x * 100 }; }

pub trait Priced {
    type Money;
    const ZERO: u32;
    fn price(&self) -> u32;
    fn doubled(&self) -> u32 { self.price() * 2 }
}

impl Line {
    pub fn new(sku: &str) -> Line { Line { sku: sku.into(), qty: 1 } }
    const MAX: u32 = 99;
}

fn helper() {}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Line { Line::new("a") }
    mod deeper { pub fn inner() {} }
}
"#;

#[test]
fn items_trait_members_and_impl_members_are_symbols() {
    let repo = Repo::new(&[("Cargo.toml", "[package]\nname = \"shop\"\n"), ("src/lib.rs", SHOP)]);
    let ex = repo.extract("src/lib.rs");
    let want = [
        "Line", "State", "Bits", "Lines", "LIMIT", "COUNT", "money", "Priced", "Priced.Money", "Priced.ZERO",
        "Priced.price", "Priced.doubled", "Line.new", "Line.MAX", "helper", "tests/fixture", "tests/deeper/inner",
    ];
    for s in want {
        assert!(ids(&ex).contains(&format!("sym:src/lib.rs::{s}").as_str()), "{s}: {:?}", ids(&ex));
    }
    assert_eq!(ex.nodes.iter().filter(|n| n.kind == NodeKind::Symbol).count(), want.len(), "an inline module is a scope, not a symbol: {:?}", ids(&ex));
    assert_eq!(node(&ex, "sym:src/lib.rs::tests/deeper/inner").label, "tests::deeper::inner");
}

#[test]
fn any_pub_is_an_export_and_members_hang_off_their_container() {
    let repo = Repo::new(&[("Cargo.toml", "[package]\nname = \"shop\"\n"), ("src/lib.rs", SHOP)]);
    let ex = repo.extract("src/lib.rs");
    let declares = edges(&ex, EdgeKind::Declares);
    for want in [
        ("file:src/lib.rs", "sym:src/lib.rs::Line", "export"),
        ("file:src/lib.rs", "sym:src/lib.rs::Lines", "export"),
        ("file:src/lib.rs", "sym:src/lib.rs::helper", ""),
        ("file:src/lib.rs", "sym:src/lib.rs::COUNT", ""),
        ("sym:src/lib.rs::Line", "sym:src/lib.rs::Line.new", ""),
        ("sym:src/lib.rs::Priced", "sym:src/lib.rs::Priced.price", ""),
        ("file:src/lib.rs", "sym:src/lib.rs::tests/fixture", ""),
    ] {
        assert!(declares.contains(&want), "{want:?} in {declares:?}");
    }
}

#[test]
fn a_body_is_the_doc_comment_and_the_signature_not_the_block() {
    let repo = Repo::new(&[("Cargo.toml", "[package]\nname = \"shop\"\n"), ("src/lib.rs", SHOP)]);
    let ex = repo.extract("src/lib.rs");
    let line = node(&ex, "sym:src/lib.rs::Line");
    assert_eq!(line.body, "An order line.\npub struct Line");
    assert_eq!((line.line, line.end), (5, 5));
    let new = node(&ex, "sym:src/lib.rs::Line.new");
    assert_eq!(new.body, "pub fn new(sku: &str) -> Line");
    assert_eq!((new.line, new.end), (23, 23));
    assert_eq!((node(&ex, "sym:src/lib.rs::Priced").line, node(&ex, "sym:src/lib.rs::Priced").end), (15, 20));
}

#[test]
fn a_foreign_impl_declares_its_members_from_the_file() {
    let repo = Repo::new(&[
        ("Cargo.toml", "[package]\nname = \"shop\"\n"),
        ("src/lib.rs", "pub mod store;\npub mod ops;\n"),
        ("src/store.rs", "pub struct Store;\n"),
        ("src/ops.rs", "use crate::store::Store;\nimpl Store {\n    pub fn open() -> Store { Store }\n    fn check(&self) {}\n}\n"),
    ]);
    let ex = repo.extract("src/ops.rs");
    let declares = edges(&ex, EdgeKind::Declares);
    assert!(declares.contains(&("file:src/ops.rs", "sym:src/ops.rs::Store.open", "export")), "{declares:?}");
    assert!(declares.contains(&("file:src/ops.rs", "sym:src/ops.rs::Store.check", "")), "{declares:?}");
    assert!(!ids(&ex).contains(&"sym:src/ops.rs::Store"), "the type is not this file's to declare");
}

#[test]
fn ids_in_comments_and_strings_belong_to_the_enclosing_item() {
    let src = "pub struct Line;\nimpl Line {\n    // FR-PAY-03 rounding\n    fn total(&self) -> &'static str { \"INV-11\" }\n}\n/// See FR-SEC-21.\npub fn f() {}\n";
    let repo = Repo::new(&[("src/lib.rs", src)]);
    let ex = repo.extract("src/lib.rs");
    let refs = edges(&ex, EdgeKind::References);
    assert!(refs.contains(&("sym:src/lib.rs::Line", "FR-PAY-03", "comment")), "{refs:?}");
    assert!(refs.contains(&("sym:src/lib.rs::Line.total", "INV-11", "string")), "{refs:?}");
    // A doc comment sits beside its item, not inside it, as TypeScript's does.
    assert!(refs.contains(&("file:src/lib.rs", "FR-SEC-21", "comment")), "{refs:?}");
}

#[test]
fn cfg_twins_are_one_symbol_whose_span_covers_both() {
    let src = "#[cfg(unix)]\nfn rename_over() {}\n\n#[cfg(windows)]\nfn rename_over() {\n    retry();\n}\n";
    let repo = Repo::new(&[("Cargo.toml", "[package]\nname = \"x\"\n"), ("src/lib.rs", src)]);
    let ex = repo.extract("src/lib.rs");
    let twins: Vec<_> = ex.nodes.iter().filter(|n| n.id == "sym:src/lib.rs::rename_over").collect();
    assert_eq!(twins.len(), 1, "{:?}", ids(&ex));
    assert_eq!((twins[0].line, twins[0].end), (2, 7), "a hunk in the windows twin is a change to rename_over");
    assert_eq!(edges(&ex, EdgeKind::Declares).len(), 1, "one Declares edge for the pair");
}

#[test]
fn same_named_members_of_two_trait_impls_keep_the_first_span() {
    let src = "struct X;\nimpl Display for X {\n    fn fmt(&self) {}\n}\n\nfn between() {}\n\nimpl Debug for X {\n    fn fmt(&self) {}\n}\n";
    let repo = Repo::new(&[("src/lib.rs", src)]);
    let ex = repo.extract("src/lib.rs");
    let fmts: Vec<_> = ex.nodes.iter().filter(|n| n.id == "sym:src/lib.rs::X.fmt").collect();
    assert_eq!(fmts.len(), 1, "{:?}", ids(&ex));
    assert_eq!((fmts[0].line, fmts[0].end), (3, 3), "the Debug impl's fmt must not stretch the Display one over `between`");
}

#[test]
fn an_id_cited_in_a_foreign_impl_starts_from_a_written_node() {
    let src = "impl Store {\n    // FR-PAY-03 rounding\n    fn m() {}\n}\n";
    let repo = Repo::new(&[("src/lib.rs", src)]);
    let ex = repo.extract("src/lib.rs");
    let refs = edges(&ex, EdgeKind::References);
    assert!(refs.contains(&("file:src/lib.rs", "FR-PAY-03", "comment")), "{refs:?}");
    for (from, _, _) in refs {
        assert!(ids(&ex).contains(&from), "{from} is not a node: {:?}", ids(&ex));
    }
}

fn shop(ops: &str) -> Repo {
    Repo::new(&[
        ("Cargo.toml", "[package]\nname = \"shop\"\nversion = \"0.1.0\"\n"),
        ("src/lib.rs", "pub mod store;\npub mod walk;\npub mod ops;\npub trait Open {\n    fn open() -> Self;\n}\n"),
        ("src/store.rs", "pub struct Store;\nimpl Store {\n    pub fn new() -> Store { Store }\n    pub fn reopen() -> Store { Self::new() }\n}\npub fn open() -> Store { Store::new() }\n"),
        ("src/walk.rs", "pub struct Manifest;\nimpl Manifest {\n    pub fn load() {}\n}\n"),
        ("src/ops.rs", ops),
    ])
}

#[test]
fn use_trees_flatten_into_imports_by_declared_name() {
    let repo = shop("use crate::{store::{self, Store as Db}, walk::*};\nuse serde::Serialize;\n");
    let ex = repo.extract("src/ops.rs");
    let mut imports = edges(&ex, EdgeKind::Imports);
    imports.sort();
    assert_eq!(
        imports,
        vec![
            ("file:src/ops.rs", "file:src/store.rs", "*"),
            // `impact` matches an import against the declared name, so an alias records `Store`.
            ("file:src/ops.rs", "file:src/store.rs", "Store"),
            ("file:src/ops.rs", "file:src/walk.rs", "*"),
        ]
    );
}

#[test]
fn pub_use_is_a_re_export_in_any_visibility() {
    let repo = Repo::new(&[
        ("Cargo.toml", "[package]\nname = \"shop\"\n"),
        ("src/lib.rs", "pub mod store;\npub use store::Store;\npub(crate) use store::*;\n"),
        ("src/store.rs", "pub struct Store;\n"),
    ]);
    let ex = repo.extract("src/lib.rs");
    let mut re = edges(&ex, EdgeKind::ReExports);
    re.sort();
    assert_eq!(re, vec![("file:src/lib.rs", "file:src/store.rs", "*"), ("file:src/lib.rs", "file:src/store.rs", "Store")]);
    assert!(edges(&ex, EdgeKind::Imports).is_empty());
}

#[test]
fn a_workspace_crate_name_resolves_to_its_library_and_a_same_file_use_writes_no_edge() {
    let repo = Repo::new(&[
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        ("crates/shop-core/Cargo.toml", "[package]\nname = \"shop-core\"\n"),
        ("crates/shop-core/src/lib.rs", "pub mod hmac;\n"),
        ("crates/shop-core/src/hmac.rs", "pub fn sign() {}\n#[cfg(test)]\nmod tests {\n    use super::*;\n}\n"),
        ("crates/shopd/Cargo.toml", "[package]\nname = \"shopd\"\n"),
        ("crates/shopd/src/main.rs", "use shop_core::hmac::sign;\nfn main() { sign(); }\n"),
    ]);
    let ex = repo.extract("crates/shopd/src/main.rs");
    assert_eq!(edges(&ex, EdgeKind::Imports), vec![("file:crates/shopd/src/main.rs", "file:crates/shop-core/src/hmac.rs", "sign")]);
    assert!(edges(&repo.extract("crates/shop-core/src/hmac.rs"), EdgeKind::Imports).is_empty(), "`use super::*` in a test module names its own file");
}

#[test]
fn a_foreign_impl_references_its_type_and_extends_its_trait() {
    let repo = shop("use crate::store::Store;\nuse crate::Open;\nimpl Open for Store {\n    fn open() -> Store { Store }\n}\n");
    let ex = repo.extract("src/ops.rs");
    assert_eq!(edges(&ex, EdgeKind::References), vec![("sym:src/ops.rs::Store.open", "sym:src/store.rs::Store", "impl")]);
    assert_eq!(edges(&ex, EdgeKind::Extends), vec![("sym:src/store.rs::Store", "sym:src/lib.rs::Open", "")]);
}

#[test]
fn a_local_impl_extends_without_a_reference_and_an_external_trait_writes_nothing() {
    let src = "pub trait Exec {\n    fn run(&self);\n}\npub struct Tokio;\nimpl Exec for Tokio {\n    fn run(&self) {}\n}\nimpl std::fmt::Display for Tokio {\n    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { Ok(()) }\n}\n";
    let repo = Repo::new(&[("Cargo.toml", "[package]\nname = \"x\"\n"), ("src/lib.rs", src)]);
    let ex = repo.extract("src/lib.rs");
    assert_eq!(edges(&ex, EdgeKind::Extends), vec![("sym:src/lib.rs::Tokio", "sym:src/lib.rs::Exec", "")]);
    assert!(edges(&ex, EdgeKind::References).is_empty());
    assert!(edges(&ex, EdgeKind::Declares).contains(&("sym:src/lib.rs::Tokio", "sym:src/lib.rs::Tokio.run", "")));
}
