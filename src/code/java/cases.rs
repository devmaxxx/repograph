//! Java extraction on inline sources, so the grammar's shape is pinned by the assertion.

use std::collections::BTreeSet;

use crate::code::jvm::fixture::{edges, ids, one};
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
