//! Dart extraction on inline sources, so the grammar's shape is pinned by the assertion.

use crate::code::imports::Resolver;
use crate::code::CodeExtractor;
use crate::config::Config;
use crate::model::{EdgeKind, Extraction, Extractor, NodeKind};

/// A repository on disk with these files, Dart globbed, and `rel` extracted from it. The resolver
/// reads headers from disk, so the file under test is written like the rest.
pub(super) fn extract_in(files: &[(&str, &str)], rel: &str) -> Extraction {
    let dir = tempfile::tempdir().unwrap();
    for (p, c) in files {
        let full = dir.path().join(p);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, c).unwrap();
    }
    let cfg = Config { code_globs: vec!["**/*.dart".into()], ..Config::default() };
    let src = files.iter().find(|(p, _)| *p == rel).expect("rel is one of the files").1;
    CodeExtractor::new(Resolver::new(dir.path(), &cfg).unwrap()).extract(rel, src)
}

pub(super) fn extract(rel: &str, src: &str) -> Extraction {
    extract_in(&[(rel, src)], rel)
}

pub(super) fn ids(ex: &Extraction) -> Vec<&str> {
    ex.nodes.iter().map(|n| n.id.as_str()).collect()
}

pub(super) fn edges(ex: &Extraction, kind: EdgeKind) -> Vec<(&str, &str, &str)> {
    ex.edges.iter().filter(|e| e.kind == kind).map(|e| (e.source.as_str(), e.target.as_str(), e.context.as_str())).collect()
}

#[test]
fn a_dart_file_is_read_by_the_dart_grammar_and_not_by_typescript() {
    // `class A {}` parses as TypeScript too; `import 'x.dart' show A;` does not declare `show` in Dart.
    let ex = extract("lib/a.dart", "import 'x.dart' show A;\n\nclass A {}\n");
    assert!(ids(&ex).contains(&"file:lib/a.dart"), "{:?}", ids(&ex));
    assert!(!ids(&ex).iter().any(|i| i.ends_with("::show")), "{:?}", ids(&ex));
    assert_eq!(ex.nodes.iter().filter(|n| n.kind == NodeKind::File).count(), 1);
}

const DECLS: &str = "typedef Pricer = int Function(int);

const limit = 3, _hidden = 4;

int total(int a) => a;

int get version => 1;

abstract class Base {
  void must();
}

mixin Audited on Base {
  void audit() {}
}

class Cart extends Base with Audited implements Comparable<Cart> {
  final Order order;
  final _store = Store();
  static const int max = 2;

  Cart(this.order);
  Cart.empty() : order = Order();
  factory Cart.of(Order o) => Cart(o);

  int get size => 1;
  void add(int n) {}
  void _drop() {}
  int operator +(int other) => other;
}

extension CartX on Cart {
  int twice() => size * 2;
}

extension on Base {
  void hidden() {}
}

enum Status {
  open,
  closed;

  bool get done => this == closed;
}

class _Private {}
";

#[test]
fn every_declaration_kind_the_spec_names_is_a_symbol() {
    let ex = extract("lib/cart.dart", DECLS);
    for id in [
        "sym:lib/cart.dart::Pricer",
        "sym:lib/cart.dart::limit",
        "sym:lib/cart.dart::_hidden",
        "sym:lib/cart.dart::total",
        "sym:lib/cart.dart::version",
        "sym:lib/cart.dart::Base",
        "sym:lib/cart.dart::Base.must",
        "sym:lib/cart.dart::Audited",
        "sym:lib/cart.dart::Audited.audit",
        "sym:lib/cart.dart::Cart",
        "sym:lib/cart.dart::Cart.order",
        "sym:lib/cart.dart::Cart._store",
        "sym:lib/cart.dart::Cart.max",
        "sym:lib/cart.dart::Cart.empty",
        "sym:lib/cart.dart::Cart.of",
        "sym:lib/cart.dart::Cart.size",
        "sym:lib/cart.dart::Cart.add",
        "sym:lib/cart.dart::Cart._drop",
        "sym:lib/cart.dart::CartX",
        "sym:lib/cart.dart::CartX.twice",
        "sym:lib/cart.dart::Status",
        "sym:lib/cart.dart::Status.done",
        "sym:lib/cart.dart::_Private",
    ] {
        assert!(ids(&ex).contains(&id), "{id} missing from {:?}", ids(&ex));
    }
}

#[test]
fn a_constructor_an_operator_an_enum_constant_and_an_unnamed_extension_name_nothing() {
    let ex = extract("lib/cart.dart", DECLS);
    let ids = ids(&ex);
    // The unnamed constructor is the class itself, which is already a symbol.
    assert!(!ids.contains(&"sym:lib/cart.dart::Cart.Cart"), "{ids:?}");
    assert!(!ids.iter().any(|i| i.contains('+') || i.contains("operator")), "{ids:?}");
    assert!(!ids.contains(&"sym:lib/cart.dart::Status.open"), "{ids:?}");
    assert!(!ids.iter().any(|i| i.ends_with("hidden") && !i.ends_with("_hidden")), "{ids:?}");
}

#[test]
fn a_leading_underscore_is_library_private_and_everything_else_is_exported() {
    let ex = extract("lib/cart.dart", DECLS);
    let declares = edges(&ex, EdgeKind::Declares);
    assert!(declares.contains(&("file:lib/cart.dart", "sym:lib/cart.dart::Cart", "export")), "{declares:?}");
    assert!(declares.contains(&("file:lib/cart.dart", "sym:lib/cart.dart::_Private", "")), "{declares:?}");
    assert!(declares.contains(&("file:lib/cart.dart", "sym:lib/cart.dart::_hidden", "")), "{declares:?}");
    assert!(declares.contains(&("sym:lib/cart.dart::Cart", "sym:lib/cart.dart::Cart.add", "export")), "{declares:?}");
    assert!(declares.contains(&("sym:lib/cart.dart::Cart", "sym:lib/cart.dart::Cart._drop", "")), "{declares:?}");
}

#[test]
fn a_class_spans_its_lines() {
    let ex = extract("lib/cart.dart", DECLS);
    let n = ex.nodes.iter().find(|n| n.id == "sym:lib/cart.dart::Cart").unwrap();
    assert_eq!((n.line, n.end), (17, 30));
}

fn declared(src: &str) -> super::declarations::Declared {
    let tree = crate::code::lang::Lang::Dart.parse(src.as_bytes()).unwrap();
    let mut ex = Extraction::default();
    super::declarations::scan(tree.root_node(), "lib/cart.dart", src.as_bytes(), &mut ex)
}

#[test]
fn a_field_has_the_type_it_declares_or_the_class_its_initializer_constructs() {
    let d = declared(DECLS);
    let fields = &d.fields["Cart"];
    assert_eq!(fields.get("order").map(String::as_str), Some("Order"));
    assert_eq!(fields.get("_store").map(String::as_str), Some("Store"));
}

#[test]
fn extends_with_and_implements_are_supertypes_and_a_generic_argument_is_not() {
    let d = declared(DECLS);
    let supers: Vec<&str> = d.supers.iter().filter(|(from, _)| from == "sym:lib/cart.dart::Cart").map(|(_, s)| s.as_str()).collect();
    for s in ["Base", "Audited", "Comparable"] {
        assert!(supers.contains(&s), "{s} missing from {supers:?}");
    }
    assert!(!supers.contains(&"Cart"), "Comparable<Cart> does not extend Cart: {supers:?}");
}
