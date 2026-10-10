//! Dart extraction on inline sources, so the grammar's shape is pinned by the assertion.

use crate::code::CodeExtractor;
use crate::code::imports::Resolver;
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

fn resolver_in(files: &[(&str, &str)]) -> (tempfile::TempDir, Resolver) {
    let dir = tempfile::tempdir().unwrap();
    for (p, c) in files {
        let full = dir.path().join(p);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, c).unwrap();
    }
    let cfg = Config { code_globs: vec!["**/*.dart".into()], ..Config::default() };
    let r = Resolver::new(dir.path(), &cfg).unwrap();
    (dir, r)
}

const MONOREPO: &[(&str, &str)] = &[
    ("packages/core/pubspec.yaml", "name: core\nversion: 1.0.0\n"),
    ("packages/core/lib/orders.dart", "export 'src/orders_impl.dart' show Orders;\n"),
    ("packages/core/lib/src/orders_impl.dart", "class Orders {\n  void place() {}\n}\n\nclass Ledger {}\n"),
    ("app/pubspec.yaml", "name: shop\n"),
    ("app/lib/main.dart", "import 'dart:math';\nimport 'package:flutter/material.dart';\nimport 'package:core/orders.dart';\nimport 'state/cart.dart';\n\nclass Home {\n  late final Orders orders;\n  final cart = Cart();\n}\n"),
    ("app/lib/state/cart.dart", "class Cart {}\n\nclass Unused {}\n"),
];

#[test]
fn a_package_uri_names_lib_under_the_directory_whose_pubspec_says_that_name() {
    let (_d, r) = resolver_in(MONOREPO);
    assert_eq!(r.state::<crate::code::dart::library::Libraries>().target("app/lib/main.dart", "package:core/orders.dart").as_deref(), Some("packages/core/lib/orders.dart"));
    assert_eq!(r.state::<crate::code::dart::library::Libraries>().target("app/lib/main.dart", "package:shop/state/cart.dart").as_deref(), Some("app/lib/state/cart.dart"));
    assert_eq!(r.state::<crate::code::dart::library::Libraries>().target("app/lib/main.dart", "package:flutter/material.dart"), None);
    assert_eq!(r.state::<crate::code::dart::library::Libraries>().target("app/lib/main.dart", "dart:math"), None);
    assert_eq!(r.state::<crate::code::dart::library::Libraries>().target("app/lib/main.dart", "state/cart.dart").as_deref(), Some("app/lib/state/cart.dart"));
}

#[test]
fn an_import_writes_the_names_the_file_uses_and_an_external_one_writes_nothing() {
    let ex = extract_in(MONOREPO, "app/lib/main.dart");
    let imports = edges(&ex, EdgeKind::Imports);
    assert!(imports.contains(&("file:app/lib/main.dart", "file:packages/core/lib/orders.dart", "Orders")), "{imports:?}");
    assert!(imports.contains(&("file:app/lib/main.dart", "file:app/lib/state/cart.dart", "Cart")), "{imports:?}");
    assert_eq!(imports.len(), 2, "dart: and an external package name no file: {imports:?}");
}

#[test]
fn a_name_passed_on_by_an_export_resolves_to_the_file_that_declares_it() {
    let (_d, r) = resolver_in(MONOREPO);
    assert_eq!(r.state::<crate::code::dart::library::Libraries>().resolve("app/lib/main.dart", "Orders"), vec!["packages/core/lib/src/orders_impl.dart".to_string()]);
    // `show Orders` keeps `Ledger` out of the barrel.
    assert!(r.state::<crate::code::dart::library::Libraries>().resolve("app/lib/main.dart", "Ledger").is_empty());
}

#[test]
fn an_export_is_a_re_export_carrying_its_show_list_or_every_name() {
    let ex = extract_in(&[
        ("lib/shop.dart", "export 'src/cart.dart' show Cart;\nexport 'src/pay.dart';\n"),
        ("lib/src/cart.dart", "class Cart {}\n"),
        ("lib/src/pay.dart", "class Pay {}\n"),
    ], "lib/shop.dart");
    let re = edges(&ex, EdgeKind::ReExports);
    assert!(re.contains(&("file:lib/shop.dart", "file:lib/src/cart.dart", "Cart")), "{re:?}");
    assert!(re.contains(&("file:lib/shop.dart", "file:lib/src/pay.dart", "*")), "{re:?}");
}

#[test]
fn hide_keeps_a_name_out_and_a_prefix_is_read_through_its_prefix() {
    let files: &[(&str, &str)] = &[
        ("lib/b.dart", "class B {}\n\nclass C {}\n"),
        ("lib/a.dart", "import 'b.dart' hide B;\nimport 'b.dart' as bb;\n\nvoid f() { B(); C(); bb.B(); }\n"),
    ];
    let ex = extract_in(files, "lib/a.dart");
    let imports = edges(&ex, EdgeKind::Imports);
    assert!(imports.contains(&("file:lib/a.dart", "file:lib/b.dart", "C")), "{imports:?}");
    assert!(imports.contains(&("file:lib/a.dart", "file:lib/b.dart", "B")), "the prefixed import used as bb.B: {imports:?}");
    let (_d, r) = resolver_in(files);
    assert!(r.state::<crate::code::dart::library::Libraries>().resolve("lib/a.dart", "B").is_empty(), "hidden and not declared here");
    assert_eq!(r.state::<crate::code::dart::library::Libraries>().imported("lib/a.dart", Some("bb"), "B"), vec!["lib/b.dart".to_string()]);
}

#[test]
fn a_part_and_its_library_are_one_scope_and_the_part_sees_the_library_imports() {
    let files: &[(&str, &str)] = &[
        ("lib/util.dart", "class Util {}\n"),
        ("lib/shop.dart", "library shop;\n\nimport 'util.dart';\n\npart 'src/cart_part.dart';\npart 'src/pay_part.dart';\n\nclass Shop {}\n"),
        ("lib/src/cart_part.dart", "part of '../shop.dart';\n\nclass CartPart {\n  final u = Util();\n}\n"),
        ("lib/src/pay_part.dart", "part of shop;\n\nclass PayPart {}\n"),
    ];
    let (_d, r) = resolver_in(files);
    assert_eq!(r.state::<crate::code::dart::library::Libraries>().library_of("lib/src/cart_part.dart"), "lib/shop.dart");
    assert_eq!(r.state::<crate::code::dart::library::Libraries>().library_of("lib/src/pay_part.dart"), "lib/shop.dart");
    assert_eq!(r.state::<crate::code::dart::library::Libraries>().resolve("lib/shop.dart", "CartPart"), vec!["lib/src/cart_part.dart".to_string()]);
    assert_eq!(r.state::<crate::code::dart::library::Libraries>().resolve("lib/src/pay_part.dart", "Util"), vec!["lib/util.dart".to_string()]);
    let ex = extract_in(files, "lib/src/cart_part.dart");
    assert!(edges(&ex, EdgeKind::Imports).contains(&("file:lib/src/cart_part.dart", "file:lib/util.dart", "Util")), "{:?}", ex.edges);
}

#[test]
fn a_conditional_import_names_its_default_and_each_configured_file() {
    // Flutter's stub/io/web split: the default URI is what the analyzer reads, the configured one is what a device runs.
    let ex = extract_in(&[
        ("lib/storage_stub.dart", "class Storage {}\n"),
        ("lib/storage_io.dart", "class Storage {}\n"),
        ("lib/app.dart", "import 'storage_stub.dart' if (dart.library.io) 'storage_io.dart';\n\nclass App {\n  final s = Storage();\n}\n"),
    ], "lib/app.dart");
    let imports = edges(&ex, EdgeKind::Imports);
    assert!(imports.contains(&("file:lib/app.dart", "file:lib/storage_stub.dart", "Storage")), "{imports:?}");
    assert!(imports.contains(&("file:lib/app.dart", "file:lib/storage_io.dart", "Storage")), "the configured file is the one that runs on a device: {imports:?}");
}

const HOME: &[(&str, &str)] = &[
    ("lib/state/store.dart", "class Store {\n  void open(String id) {}\n}\n"),
    ("lib/net/client.dart", "class Client {\n  void connect() {}\n  static Client make() => Client();\n}\n\nClient connectNow() => Client();\n"),
    ("lib/base.dart", "abstract class Base {}\n\nmixin Logs {}\n"),
    ("lib/home.dart", "import 'package:flutter/widgets.dart';
import 'base.dart';
import 'state/store.dart';
import 'net/client.dart';
import 'net/client.dart' as net;

class Home extends Base with Logs implements StatelessWidget {
  final _store = Store();
  late final Client _client;

  void start() {
    _store.open('a');
    this._client.connect();
    _refresh();
    Client.make();
    net.connectNow();
    helper();
    start();
  }

  void _refresh() {
    start();
  }
}

void helper() {}
"),
];

#[test]
fn a_call_reaches_the_member_its_receiver_s_declared_or_constructed_type_declares() {
    let ex = extract_in(HOME, "lib/home.dart");
    let calls = edges(&ex, EdgeKind::Calls);
    let from = "sym:lib/home.dart::Home.start";
    for to in [
        "sym:lib/state/store.dart::Store.open",
        "sym:lib/net/client.dart::Client.connect",
        "sym:lib/home.dart::Home._refresh",
        "sym:lib/net/client.dart::Client.make",
        "sym:lib/net/client.dart::connectNow",
        "sym:lib/home.dart::helper",
    ] {
        assert!(calls.iter().any(|(s, t, _)| *s == from && *t == to), "{from} -> {to} missing from {calls:?}");
    }
    assert!(!calls.iter().any(|(s, t, _)| s == t), "a self-call is dropped: {calls:?}");
    assert!(calls.contains(&("sym:lib/home.dart::Home._refresh", "sym:lib/home.dart::Home.start", "")), "{calls:?}");
}

#[test]
fn a_constructor_call_in_a_field_initializer_is_a_call_from_that_field() {
    let ex = extract_in(HOME, "lib/home.dart");
    assert!(edges(&ex, EdgeKind::Calls).contains(&("sym:lib/home.dart::Home._store", "sym:lib/state/store.dart::Store", "")), "{:?}", ex.edges);
}

#[test]
fn a_supertype_the_repository_declares_is_extended_and_an_external_one_is_not() {
    let ex = extract_in(HOME, "lib/home.dart");
    let extends = edges(&ex, EdgeKind::Extends);
    assert!(extends.contains(&("sym:lib/home.dart::Home", "sym:lib/base.dart::Base", "")), "{extends:?}");
    assert!(extends.contains(&("sym:lib/home.dart::Home", "sym:lib/base.dart::Logs", "")), "{extends:?}");
    assert!(!extends.iter().any(|(_, t, _)| t.contains("StatelessWidget")), "{extends:?}");
}

#[test]
fn a_name_a_file_declares_shadows_the_same_name_an_import_brings() {
    let ex = extract_in(&[
        ("lib/other.dart", "void helper() {}\n"),
        ("lib/a.dart", "import 'other.dart';\n\nvoid helper() {}\n\nvoid run() { helper(); }\n"),
    ], "lib/a.dart");
    let calls = edges(&ex, EdgeKind::Calls);
    assert!(calls.contains(&("sym:lib/a.dart::run", "sym:lib/a.dart::helper", "")), "{calls:?}");
    assert!(!calls.iter().any(|(_, t, _)| t.starts_with("sym:lib/other.dart")), "{calls:?}");
}


#[test]
fn a_call_on_this_reaches_the_type_s_own_member() {
    // The grammar files `this` under `object` as a bare token, so it must read as no receiver at all.
    let ex = extract("lib/a.dart", "class A {\n  void a() { this.b(); }\n  void b() {}\n}\n");
    assert!(edges(&ex, EdgeKind::Calls).contains(&("sym:lib/a.dart::A.a", "sym:lib/a.dart::A.b", "")), "{:?}", ex.edges);
}

#[test]
fn a_library_two_exports_reach_passes_on_what_each_export_admits() {
    // `b.dart` reaches `e.dart` once filtered to `X` and once whole: `Y` still arrives through `d.dart`.
    let ex = extract_in(&[
        ("lib/a.dart", "import 'b.dart';\nvoid f() { X(); Y(); }\n"),
        ("lib/b.dart", "export 'c.dart' show X;\nexport 'd.dart';\n"),
        ("lib/c.dart", "export 'e.dart';\n"),
        ("lib/d.dart", "export 'e.dart';\n"),
        ("lib/e.dart", "class X {}\nclass Y {}\n"),
    ], "lib/a.dart");
    let calls = edges(&ex, EdgeKind::Calls);
    assert!(calls.contains(&("sym:lib/a.dart::f", "sym:lib/e.dart::X", "")), "{calls:?}");
    assert!(calls.contains(&("sym:lib/a.dart::f", "sym:lib/e.dart::Y", "")), "{calls:?}");
}

#[test]
fn a_local_in_a_method_does_not_own_the_call_in_its_initializer() {
    let ex = extract("lib/a.dart", "class C {\n  void m() {\n    final a = 1, b = helper();\n    this.inherited();\n  }\n}\nint helper() => 1;\n");
    let calls = edges(&ex, EdgeKind::Calls);
    assert_eq!(calls, vec![("sym:lib/a.dart::C.m", "sym:lib/a.dart::helper", "")]);
}

#[test]
fn a_prefixed_type_annotation_is_a_name_the_import_context_keeps() {
    let ex = extract_in(&[("lib/a.dart", "import 'b.dart' as p;\nclass C { p.Widget? w; }\n"), ("lib/b.dart", "class Widget {}\n")], "lib/a.dart");
    assert_eq!(edges(&ex, EdgeKind::Imports), vec![("file:lib/a.dart", "file:lib/b.dart", "Widget")]);
}

#[test]
fn an_export_with_hide_names_what_it_passes_on() {
    let ex = extract_in(&[("lib/a.dart", "export 'b.dart' hide Secret;\n"), ("lib/b.dart", "class Secret {}\nclass Open {}\n")], "lib/a.dart");
    assert_eq!(edges(&ex, EdgeKind::ReExports), vec![("file:lib/a.dart", "file:lib/b.dart", "Open")]);
}
