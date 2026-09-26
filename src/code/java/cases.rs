//! Java extraction on inline sources, so the grammar's shape is pinned by the assertion.

use std::collections::BTreeSet;

use crate::code::jvm::fixture::{edges, ids, one, Repo};
use crate::model::EdgeKind;

const ORDERS: &str = "package shop.orders;

import shop.billing.Invoice;

public class OrderService extends Base implements Api {
    private final Invoice invoice;
    public static int COUNT = 0;
    int a, b;

    public OrderService(Invoice invoice) {
        this.invoice = invoice;
    }

    public void place(int n) {}

    static class Nested {
        public void deep() {}
    }
}

interface Api {
    void place(int n);
    int LIMIT = 3;
    private void hidden() {}
}

enum Status implements Api {
    OPEN, CLOSED;
    public void place(int n) {}
    void flip() {}
}

record Point(int x, int y) implements Api {
    public void place(int n) {}
    int sum() { return x + y; }
}

@interface Audited {
    String value();
}

interface Sub extends Api {}
";

#[test]
fn types_members_constructors_and_nested_types_are_declared() {
    let ex = one("shop/orders/OrderService.java", ORDERS);
    for id in [
        "file:shop/orders/OrderService.java",
        "sym:shop/orders/OrderService.java::OrderService",
        "sym:shop/orders/OrderService.java::OrderService.invoice",
        "sym:shop/orders/OrderService.java::OrderService.COUNT",
        "sym:shop/orders/OrderService.java::OrderService.a",
        "sym:shop/orders/OrderService.java::OrderService.b",
        "sym:shop/orders/OrderService.java::OrderService.OrderService",
        "sym:shop/orders/OrderService.java::OrderService.place",
        "sym:shop/orders/OrderService.java::OrderService.Nested",
        "sym:shop/orders/OrderService.java::OrderService.Nested.deep",
        "sym:shop/orders/OrderService.java::Api",
        "sym:shop/orders/OrderService.java::Api.place",
        "sym:shop/orders/OrderService.java::Api.LIMIT",
        "sym:shop/orders/OrderService.java::Status",
        "sym:shop/orders/OrderService.java::Status.flip",
        "sym:shop/orders/OrderService.java::Point",
        "sym:shop/orders/OrderService.java::Point.sum",
        "sym:shop/orders/OrderService.java::Audited",
        "sym:shop/orders/OrderService.java::Audited.value",
        "sym:shop/orders/OrderService.java::Sub",
    ] {
        assert!(ids(&ex).contains(&id), "{id} missing from {:?}", ids(&ex));
    }
    // Enum constants and record components are not symbols, in the extractor or the truth reader.
    for absent in ["Status.OPEN", "Point.x", "Point.y"] {
        assert!(!ids(&ex).iter().any(|i| i.ends_with(absent)), "{absent} in {:?}", ids(&ex));
    }
}

#[test]
fn only_public_is_exported_and_an_interface_member_is_public_without_the_word() {
    let ex = one("shop/orders/OrderService.java", ORDERS);
    let declares = edges(&ex, EdgeKind::Declares);
    let f = "sym:shop/orders/OrderService.java::";
    let ctx = |from: &str, to: &str| declares.iter().find(|(s, t, _)| *s == from && *t == to).map(|(_, _, c)| *c);
    assert_eq!(ctx("file:shop/orders/OrderService.java", &format!("{f}OrderService")), Some("export"));
    assert_eq!(ctx("file:shop/orders/OrderService.java", &format!("{f}Api")), Some(""));
    assert_eq!(ctx(&format!("{f}Api"), &format!("{f}Api.place")), Some("export"));
    assert_eq!(ctx(&format!("{f}Api"), &format!("{f}Api.LIMIT")), Some("export"));
    assert_eq!(ctx(&format!("{f}Api"), &format!("{f}Api.hidden")), Some(""));
    assert_eq!(ctx(&format!("{f}OrderService"), &format!("{f}OrderService.invoice")), Some(""));
    assert_eq!(ctx(&format!("{f}OrderService"), &format!("{f}OrderService.place")), Some("export"));
    assert_eq!(ctx(&format!("{f}OrderService"), &format!("{f}OrderService.Nested")), Some(""));
    assert_eq!(ctx(&format!("{f}OrderService.Nested"), &format!("{f}OrderService.Nested.deep")), Some("export"));
    assert_eq!(ctx(&format!("{f}Status"), &format!("{f}Status.flip")), Some(""));
    assert_eq!(ctx(&format!("{f}Audited"), &format!("{f}Audited.value")), Some("export"));
}

#[test]
fn extends_and_implements_declared_in_the_same_file_are_extended() {
    let ex = one("shop/orders/OrderService.java", ORDERS);
    let extends = edges(&ex, EdgeKind::Extends);
    let f = "sym:shop/orders/OrderService.java::";
    for (from, to) in [("OrderService", "Api"), ("Status", "Api"), ("Point", "Api"), ("Sub", "Api")] {
        assert!(extends.contains(&(format!("{f}{from}").as_str(), format!("{f}{to}").as_str(), "")), "{from} -> {to} missing from {extends:?}");
    }
    // `Base` is declared nowhere in this file; only the index (Task 4) may resolve it.
    assert!(!extends.iter().any(|(_, t, _)| t.ends_with("::Base")), "{extends:?}");
}

#[test]
fn a_declaration_spans_its_lines_and_its_body_is_its_doc_and_its_name_line() {
    let ex = one("shop/orders/OrderService.java", ORDERS);
    let n = ex.nodes.iter().find(|n| n.id == "sym:shop/orders/OrderService.java::OrderService").unwrap();
    assert_eq!((n.line, n.end), (5, 19));
    let src = "package shop;\n\npublic class Cart {\n    /** Totals the lines. FR-PAY-22 */\n    @Deprecated\n    public int total() { return 0; }\n}\n";
    let ex = one("shop/Cart.java", src);
    let n = ex.nodes.iter().find(|n| n.id == "sym:shop/Cart.java::Cart.total").unwrap();
    assert_eq!(n.body, "Totals the lines. FR-PAY-22\npublic int total() { return 0; }");
}

#[test]
fn the_header_names_exactly_the_top_level_types_the_file_declares() {
    let ex = one("shop/orders/OrderService.java", ORDERS);
    let declared: BTreeSet<String> = edges(&ex, EdgeKind::Declares).into_iter()
        .filter(|(s, _, _)| *s == "file:shop/orders/OrderService.java")
        .filter_map(|(_, t, _)| t.strip_prefix("sym:shop/orders/OrderService.java::"))
        .map(str::to_string)
        .collect();
    let h = super::header(ORDERS);
    assert_eq!(h.scope, vec!["shop.orders".to_string()]);
    assert_eq!(h.top, declared);
    assert_eq!(h.top.len(), 6, "{:?}", h.top);
    assert_eq!(super::header("class NoPackage {}\n").scope, Vec::<String>::new());
}

// Android and Windows-built Java often starts with a BOM and ends its lines in CRLF.
#[test]
fn a_bom_and_crlf_file_declares_as_a_plain_one() {
    let ex = one("shop/Crlf.java", "\u{feff}package shop;\r\n\r\npublic class Crlf {\r\n    public void go() {}\r\n}\r\n");
    assert!(ids(&ex).contains(&"sym:shop/Crlf.java::Crlf.go"), "{:?}", ids(&ex));
    assert_eq!(super::header("\u{feff}package shop;\r\nclass A {}\r\n").scope, vec!["shop".to_string()]);
    let n = ex.nodes.iter().find(|n| n.id == "sym:shop/Crlf.java::Crlf.go").unwrap();
    assert!(!n.body.contains('\r'), "{:?}", n.body);
}

#[test]
fn a_member_three_types_deep_keeps_every_segment() {
    let ex = one("shop/Deep.java", "package shop;\n\npublic class A {\n    static class B {\n        static class C {\n            void m() {}\n        }\n    }\n}\n");
    assert!(ids(&ex).contains(&"sym:shop/Deep.java::A.B.C.m"), "{:?}", ids(&ex));
    let declares = edges(&ex, EdgeKind::Declares);
    assert!(declares.contains(&("sym:shop/Deep.java::A.B", "sym:shop/Deep.java::A.B.C", "")), "{declares:?}");
    assert!(declares.contains(&("sym:shop/Deep.java::A.B.C", "sym:shop/Deep.java::A.B.C.m", "")), "{declares:?}");
}

const INVOICE: &str = "package shop.billing;\n\npublic class Invoice {\n    public void send() {}\n    public static class Line {}\n}\n";
const MONEY: &str = "package shop.util;\n\npublic final class Money {\n    public static int round(int n) { return n; }\n}\n";

#[test]
fn a_single_a_nested_and_a_static_import_each_name_their_file() {
    let repo = Repo::new(&[
        ("shop/billing/Invoice.java", INVOICE),
        ("shop/util/Money.java", MONEY),
        ("shop/orders/Use.java", "package shop.orders;\n\nimport shop.billing.Invoice;\nimport shop.billing.Invoice.Line;\nimport static shop.util.Money.round;\n\npublic class Use extends Invoice {\n    static class Row extends Line {}\n}\n"),
    ]);
    let ex = repo.extract("shop/orders/Use.java");
    let imports = edges(&ex, EdgeKind::Imports);
    assert!(imports.contains(&("file:shop/orders/Use.java", "file:shop/billing/Invoice.java", "Invoice")), "{imports:?}");
    assert!(imports.contains(&("file:shop/orders/Use.java", "file:shop/util/Money.java", "Money")), "{imports:?}");
    let extends = edges(&ex, EdgeKind::Extends);
    assert!(extends.contains(&("sym:shop/orders/Use.java::Use", "sym:shop/billing/Invoice.java::Invoice", "")), "{extends:?}");
    assert!(extends.contains(&("sym:shop/orders/Use.java::Use.Row", "sym:shop/billing/Invoice.java::Invoice.Line", "")), "{extends:?}");
}

#[test]
fn a_supertype_resolves_through_its_package_a_star_and_its_full_name() {
    let repo = Repo::new(&[
        ("shop/billing/Invoice.java", INVOICE),
        ("shop/billing/Draft.java", "package shop.billing;\n\nclass Draft extends Invoice {}\n"),
        ("shop/orders/Star.java", "package shop.orders;\n\nimport shop.billing.*;\n\nclass Star extends Invoice {}\n"),
        ("shop/orders/Full.java", "package shop.orders;\n\nclass Full extends shop.billing.Invoice {}\n"),
    ]);
    for (rel, class) in [("shop/billing/Draft.java", "Draft"), ("shop/orders/Star.java", "Star"), ("shop/orders/Full.java", "Full")] {
        let ex = repo.extract(rel);
        let from = format!("sym:{rel}::{class}");
        assert!(edges(&ex, EdgeKind::Extends).contains(&(from.as_str(), "sym:shop/billing/Invoice.java::Invoice", "")), "{rel}: {:?}", ex.edges);
    }
    assert!(edges(&repo.extract("shop/orders/Star.java"), EdgeKind::Imports).is_empty());
}

#[test]
fn a_generic_and_a_sealed_supertype_resolve_to_their_raw_type() {
    let repo = Repo::new(&[
        ("shop/Repo.java", "package shop;\n\npublic interface Repo<T> { T get(); }\n"),
        ("shop/Shape.java", "package shop;\n\npublic sealed interface Shape permits Circle {}\n"),
        ("shop/Circle.java", "package shop;\n\npublic record Circle(int r) implements Shape, Repo<Circle> { public Circle get() { return this; } }\n"),
    ]);
    let circle = repo.extract("shop/Circle.java");
    let ext = edges(&circle, EdgeKind::Extends);
    for to in ["sym:shop/Shape.java::Shape", "sym:shop/Repo.java::Repo"] {
        assert!(ext.contains(&("sym:shop/Circle.java::Circle", to, "")), "{to}: {ext:?}");
    }
    // `permits` names a subtype, not a supertype.
    assert!(edges(&repo.extract("shop/Shape.java"), EdgeKind::Extends).is_empty());
}

#[test]
fn a_name_two_star_imports_both_supply_resolves_to_nothing() {
    let repo = Repo::new(&[
        ("shop/billing/Invoice.java", INVOICE),
        ("shop/legacy/Invoice.java", "package shop.legacy;\n\npublic class Invoice {}\n"),
        ("shop/orders/Both.java", "package shop.orders;\n\nimport shop.billing.*;\nimport shop.legacy.*;\n\nclass Both extends Invoice {}\n"),
    ]);
    let ex = repo.extract("shop/orders/Both.java");
    assert!(edges(&ex, EdgeKind::Extends).is_empty(), "{:?}", ex.edges);
}
