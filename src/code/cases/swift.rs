//! Swift extraction on inline sources.

use crate::code::imports::Resolver;
use crate::code::CodeExtractor;
use crate::config::Config;
use crate::model::{EdgeKind, Extraction, Extractor};

fn extract_in(files: &[(&str, &str)], rel: &str) -> Extraction {
    let dir = tempfile::tempdir().unwrap();
    for (p, c) in files {
        let full = dir.path().join(p);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, c).unwrap();
    }
    let cfg = Config { code_globs: vec!["**/*.swift".into()], ..Config::default() };
    let src = files.iter().find(|(p, _)| *p == rel).unwrap().1;
    CodeExtractor::new(Resolver::new(dir.path(), &cfg).unwrap()).extract(rel, src)
}

fn ids(ex: &Extraction) -> Vec<&str> {
    ex.nodes.iter().map(|n| n.id.as_str()).collect()
}

fn edges(ex: &Extraction, kind: EdgeKind) -> Vec<(&str, &str, &str)> {
    ex.edges.iter().filter(|e| e.kind == kind).map(|e| (e.source.as_str(), e.target.as_str(), e.context.as_str())).collect()
}

const RUNNER: &str = "import UIKit

let limit = 3
private let secret = 1
var counter = 0

func helper(_ n: Int) -> Int { n }
fileprivate func hidden() {}

protocol Store {
    func load() -> Int
    var size: Int { get }
}

@objc final class Cart: Base, Store {
    let id: Int = 1
    init(id: Int) {}
    func load() -> Int { 1 }
    private func drop() {}
    var size: Int { 1 }
    class Nested { func deep() {} }
    static func make() -> Cart { Cart(id: 1) }
}

struct Point { var x: Int; func moved() -> Point { self } }

enum Mode: String { case a, b; func flip() -> Mode { .a } }

extension Cart {
    func extra() {}
}
";

#[test]
fn types_functions_and_top_level_constants_are_symbols() {
    let ex = extract_in(&[("Sources/Cart.swift", RUNNER)], "Sources/Cart.swift");
    for id in [
        "sym:Sources/Cart.swift::limit",
        "sym:Sources/Cart.swift::secret",
        "sym:Sources/Cart.swift::helper",
        "sym:Sources/Cart.swift::hidden",
        "sym:Sources/Cart.swift::Store",
        "sym:Sources/Cart.swift::Store.load",
        "sym:Sources/Cart.swift::Cart",
        "sym:Sources/Cart.swift::Cart.load",
        "sym:Sources/Cart.swift::Cart.drop",
        "sym:Sources/Cart.swift::Cart.Nested",
        "sym:Sources/Cart.swift::Cart.Nested.deep",
        "sym:Sources/Cart.swift::Cart.make",
        "sym:Sources/Cart.swift::Point",
        "sym:Sources/Cart.swift::Point.moved",
        "sym:Sources/Cart.swift::Mode",
        "sym:Sources/Cart.swift::Mode.flip",
        "sym:Sources/Cart.swift::Cart.extra",
    ] {
        assert!(ids(&ex).contains(&id), "{id} missing from {:?}", ids(&ex));
    }
}

#[test]
fn properties_initializers_variables_and_enum_cases_are_not_symbols() {
    let ex = extract_in(&[("Sources/Cart.swift", RUNNER)], "Sources/Cart.swift");
    for id in [
        "sym:Sources/Cart.swift::counter",
        "sym:Sources/Cart.swift::Store.size",
        "sym:Sources/Cart.swift::Cart.id",
        "sym:Sources/Cart.swift::Cart.size",
        "sym:Sources/Cart.swift::Cart.init",
        "sym:Sources/Cart.swift::Point.x",
        "sym:Sources/Cart.swift::Mode.a",
    ] {
        assert!(!ids(&ex).contains(&id), "{id} is not a declaration the spec names: {:?}", ids(&ex));
    }
}

#[test]
fn private_and_fileprivate_are_not_exported_and_an_extension_member_is_declared_by_its_file() {
    let ex = extract_in(&[("Sources/Cart.swift", RUNNER)], "Sources/Cart.swift");
    let declares = edges(&ex, EdgeKind::Declares);
    assert!(declares.contains(&("file:Sources/Cart.swift", "sym:Sources/Cart.swift::limit", "export")), "{declares:?}");
    assert!(declares.contains(&("file:Sources/Cart.swift", "sym:Sources/Cart.swift::secret", "")), "{declares:?}");
    assert!(declares.contains(&("file:Sources/Cart.swift", "sym:Sources/Cart.swift::hidden", "")), "{declares:?}");
    assert!(declares.contains(&("sym:Sources/Cart.swift::Cart", "sym:Sources/Cart.swift::Cart.drop", "")), "{declares:?}");
    assert!(declares.contains(&("file:Sources/Cart.swift", "sym:Sources/Cart.swift::Cart.extra", "export")), "{declares:?}");
}

#[test]
fn an_inheritance_clause_is_an_edge_only_to_a_name_the_repository_declares() {
    let files: &[(&str, &str)] = &[
        ("Sources/Cart.swift", RUNNER),
        ("Sources/Base.swift", "class Base {}\n"),
        ("Sources/Tokens.swift", "struct Token {}\n\nextension Token: Store {}\n"),
    ];
    let ex = extract_in(files, "Sources/Cart.swift");
    let extends = edges(&ex, EdgeKind::Extends);
    assert!(extends.contains(&("sym:Sources/Cart.swift::Cart", "sym:Sources/Base.swift::Base", "")), "{extends:?}");
    assert!(extends.contains(&("sym:Sources/Cart.swift::Cart", "sym:Sources/Cart.swift::Store", "")), "{extends:?}");
    assert!(!extends.iter().any(|(_, t, _)| t.contains("String")), "an SDK type names nothing: {extends:?}");
    let ex = extract_in(files, "Sources/Tokens.swift");
    assert!(edges(&ex, EdgeKind::Extends).contains(&("sym:Sources/Tokens.swift::Token", "sym:Sources/Cart.swift::Store", "")), "{:?}", ex.edges);
}

#[test]
fn a_private_type_in_another_file_is_not_a_supertype_and_the_file_s_own_declaration_wins() {
    // An app and its extensions often both declare a `Theme` or a `Config`; a private one is its file's alone.
    let files: &[(&str, &str)] = &[
        ("Sources/A.swift", "private class Base {}\nfileprivate protocol Store {}\n"),
        ("Sources/B.swift", "class Base {}\nprotocol Store {}\nclass Sub: Base, Store {}\n"),
    ];
    let ex = extract_in(files, "Sources/B.swift");
    let extends = edges(&ex, EdgeKind::Extends);
    assert_eq!(extends, vec![
        ("sym:Sources/B.swift::Sub", "sym:Sources/B.swift::Base", ""),
        ("sym:Sources/B.swift::Sub", "sym:Sources/B.swift::Store", ""),
    ], "{extends:?}");
}

#[test]
fn an_extension_of_a_nested_type_inherits_from_the_file_declaring_its_outer_type() {
    let ex = extract_in(&[("a.swift", "extension Outer.Inner: Proto { func go() {} }\n"), ("b.swift", "class Outer { class Inner {} }\nprotocol Proto {}\n")], "a.swift");
    assert_eq!(edges(&ex, EdgeKind::Extends), vec![("sym:b.swift::Outer.Inner", "sym:b.swift::Proto", "")]);
}

#[test]
fn each_name_of_a_multi_binding_let_is_a_constant_over_the_whole_statement() {
    let ex = extract_in(&[("a.swift", "let a = 1, b = [\n    2,\n]\n")], "a.swift");
    let spans: Vec<(&str, u32, u32)> = ex.nodes.iter().filter(|n| n.id.starts_with("sym:")).map(|n| (n.id.as_str(), n.line, n.end)).collect();
    assert_eq!(spans, vec![("sym:a.swift::a", 1, 3), ("sym:a.swift::b", 1, 3)]);
}

#[test]
fn a_signature_is_the_line_holding_the_name_past_any_attribute_line() {
    let ex = extract_in(&[("Sources/Cart.swift", RUNNER)], "Sources/Cart.swift");
    let body = |id: &str| ex.nodes.iter().find(|n| n.id == id).map(|n| n.body.as_str());
    // The attribute shares its line with the declaration, and the line is the declaration's.
    assert_eq!(body("sym:Sources/Cart.swift::Cart"), Some("@objc final class Cart: Base, Store {"));
    // A member on its type's line reads as itself.
    assert_eq!(body("sym:Sources/Cart.swift::Point.moved"), Some("func moved() -> Point { self }"));
    let ex = extract_in(&[("a.swift", "@available(iOS 13, *)\n@MainActor\npublic struct Panel {}\n")], "a.swift");
    assert_eq!(ex.nodes.iter().find(|n| n.id == "sym:a.swift::Panel").map(|n| n.body.as_str()), Some("public struct Panel {}"));
}

#[test]
fn an_overload_declares_its_one_node_from_each_parent_and_with_each_export() {
    let src = "class Over {\n    func f(a: Int) {}\n    private func f(b: Int) {}\n}\nextension Over { func f(c: Int) {} }\n";
    let ex = extract_in(&[("a.swift", src)], "a.swift");
    let mut declares: Vec<_> = edges(&ex, EdgeKind::Declares).into_iter().filter(|(_, t, _)| *t == "sym:a.swift::Over.f").collect();
    declares.sort();
    assert_eq!(declares, vec![
        ("file:a.swift", "sym:a.swift::Over.f", "export"),
        ("sym:a.swift::Over", "sym:a.swift::Over.f", ""),
        ("sym:a.swift::Over", "sym:a.swift::Over.f", "export"),
    ]);
}

#[test]
fn an_extension_names_its_members_on_the_type_without_its_last_generic_arguments() {
    let src = "struct Box<T> { class Inner {} }\nextension Box<Int> { func boxed() {} }\nextension Box<Int>.Inner: P { func i() {} }\nprotocol P {}\n";
    let ex = extract_in(&[("a.swift", src)], "a.swift");
    assert!(ids(&ex).contains(&"sym:a.swift::Box.boxed"), "{:?}", ids(&ex));
    // Arguments mid-path stay in the name, which no type declares, so its clause reaches nothing.
    assert!(ids(&ex).contains(&"sym:a.swift::Box<Int>.Inner.i"), "{:?}", ids(&ex));
    assert_eq!(edges(&ex, EdgeKind::Extends), vec![]);
}

#[test]
fn an_operator_is_named_by_its_symbol_and_a_return_type_names_nothing() {
    let src = "struct P {\n    static func == (l: P, r: P) -> Bool { true }\n    static func <*> (l: P, r: P) -> P { l }\n    func size() -> Int { 1 }\n}\n";
    let ex = extract_in(&[("a.swift", src)], "a.swift");
    let syms: Vec<&str> = ids(&ex).into_iter().filter(|i| i.starts_with("sym:")).collect();
    assert_eq!(syms, vec!["sym:a.swift::P", "sym:a.swift::P.==", "sym:a.swift::P.<*>", "sym:a.swift::P.size"]);
}

#[test]
fn a_type_in_a_function_body_or_an_actor_declares_nothing_and_extends_nothing() {
    let src = "class Base {}\nfunc outer() {\n    class Local: Base { func lm() {} }\n}\nactor Act: Base { func a() {} }\nlet made: Int = {\n    class InClosure: Base {}\n    return 1\n}()\n";
    let ex = extract_in(&[("a.swift", src)], "a.swift");
    let syms: Vec<&str> = ids(&ex).into_iter().filter(|i| i.starts_with("sym:")).collect();
    assert_eq!(syms, vec!["sym:a.swift::Base", "sym:a.swift::outer", "sym:a.swift::made"]);
    assert_eq!(edges(&ex, EdgeKind::Extends), vec![]);
}

#[test]
fn an_extension_of_sugared_type_names_its_members_on_the_sugar() {
    let ex = extract_in(&[("a.swift", "extension [Int] { func total() -> Int { 0 } }\n")], "a.swift");
    assert!(ids(&ex).contains(&"sym:a.swift::[Int].total"), "{:?}", ids(&ex));
}

/// A query cannot recurse, so the chains of nested types it follows stop at a depth: a type nested
/// inside four others is declared, and nothing inside it is.
#[test]
fn limitation_a_type_nested_inside_four_others_declares_nothing_of_its_own() {
    let src = "class A { class B { class C { class D {\n    class E { func deep() {} }\n    func atD() {}\n} } } }\n";
    let ex = extract_in(&[("a.swift", src)], "a.swift");
    assert!(ids(&ex).contains(&"sym:a.swift::A.B.C.D.atD"), "{:?}", ids(&ex));
    assert!(ids(&ex).contains(&"sym:a.swift::A.B.C.D.E"), "{:?}", ids(&ex));
    assert!(!ids(&ex).contains(&"sym:a.swift::A.B.C.D.E.deep"), "{:?}", ids(&ex));
}
