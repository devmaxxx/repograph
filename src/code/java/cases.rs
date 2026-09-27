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

#[test]
fn a_supertype_outside_the_repository_never_walks_into_a_type_star_or_a_package() {
    let repo = Repo::new(&[
        ("shop/billing/Invoice.java", INVOICE),
        ("shop/orders/Use.java", "package shop.orders;\n\nimport shop.billing.Invoice.*;\n\nclass Use extends Exception {}\n"),
        ("p.java", "public class p {}\n"),
        ("p/X.java", "package p;\n\nclass X extends Exception {}\n"),
    ]);
    for rel in ["shop/orders/Use.java", "p/X.java"] {
        let ex = repo.extract(rel);
        assert!(edges(&ex, EdgeKind::Extends).is_empty(), "{rel}: {:?}", ex.edges);
    }
}

#[test]
fn an_inherited_member_type_is_not_guessed_at_the_subclass_path() {
    let repo = Repo::new(&[
        ("shop/Base.java", "package shop;\n\npublic class Base { public static class Inner {} }\n"),
        ("shop/Sub.java", "package shop;\n\npublic class Sub extends Base {}\n"),
        ("shop/Use.java", "package shop;\n\nclass Use extends Sub.Inner {}\n"),
    ]);
    let ex = repo.extract("shop/Use.java");
    assert!(edges(&ex, EdgeKind::Extends).is_empty(), "{:?}", ex.edges);
}

#[test]
fn two_static_imports_of_one_name_each_name_their_file() {
    let repo = Repo::new(&[
        ("a/A.java", "package a;\n\npublic class A { public static int of(int n) { return n; } }\n"),
        ("b/B.java", "package b;\n\npublic class B { public static int of(int n) { return n; } }\n"),
        ("c/Use.java", "package c;\n\nimport static a.A.of;\nimport static b.B.of;\n\nclass Use {}\n"),
    ]);
    let imports = edges(&repo.extract("c/Use.java"), EdgeKind::Imports).into_iter().map(|(_, to, _)| to.to_string()).collect::<Vec<_>>();
    assert_eq!(imports, ["file:a/A.java", "file:b/B.java"]);
}

const CHECKOUT: &str = "package shop.orders;

import shop.billing.Invoice;
import shop.util.Money;
import static shop.util.Money.round;

public class Checkout {
    private final Invoice invoice;

    public Checkout(Invoice invoice) {
        this.invoice = invoice;
    }

    public void pay(Invoice given, int n) {
        Invoice local = given;
        var built = new Invoice();
        invoice.send();
        this.invoice.send();
        given.send();
        local.send();
        built.send();
        Money.round(n);
        round(n);
        total();
        Checkout.Lines.count();
        new Task() { public void run() { fromAnonymous(); } };
        pay(given, n);
    }

    int total() { return 0; }

    void fromAnonymous() {}

    static class Lines {
        static int count() { return 0; }
    }
}

interface Task {
    void run();
}
";

fn calls_from<'a>(ex: &'a crate::model::Extraction, from: &str) -> Vec<&'a str> {
    let mut to: Vec<&str> = edges(ex, EdgeKind::Calls).into_iter().filter(|(s, _, _)| *s == from).map(|(_, t, _)| t).collect();
    to.sort();
    to.dedup();
    to
}

#[test]
fn calls_through_fields_this_parameters_locals_statics_and_nested_types_are_edges() {
    let repo = Repo::new(&[
        ("shop/billing/Invoice.java", INVOICE),
        ("shop/util/Money.java", MONEY),
        ("shop/orders/Checkout.java", CHECKOUT),
    ]);
    let ex = repo.extract("shop/orders/Checkout.java");
    let from = "sym:shop/orders/Checkout.java::Checkout.pay";
    let got = calls_from(&ex, from);
    for to in [
        "sym:shop/billing/Invoice.java::Invoice.send",
        "sym:shop/util/Money.java::Money.round",
        "sym:shop/orders/Checkout.java::Checkout.total",
        "sym:shop/orders/Checkout.java::Checkout.Lines.count",
        // An anonymous class's method is not a symbol, so its call belongs to the method around it.
        "sym:shop/orders/Checkout.java::Checkout.fromAnonymous",
    ] {
        assert!(got.contains(&to), "{to} missing from {got:?}");
    }
    assert!(!got.contains(&from), "a recursive call is dropped: {got:?}");
    assert!(!edges(&ex, EdgeKind::Calls).iter().any(|(s, _, _)| s.ends_with(".run")), "{:?}", ex.edges);
}

#[test]
fn an_on_demand_static_import_resolves_only_when_it_is_the_only_one() {
    let repo = Repo::new(&[
        ("shop/util/Money.java", MONEY),
        ("shop/util/Tax.java", "package shop.util;\n\npublic final class Tax {\n    public static int round(int n) { return n; }\n}\n"),
        ("shop/One.java", "package shop;\n\nimport static shop.util.Money.*;\n\nclass One {\n    int go() { return round(1); }\n}\n"),
        ("shop/Two.java", "package shop;\n\nimport static shop.util.Money.*;\nimport static shop.util.Tax.*;\n\nclass Two {\n    int go() { return round(1); }\n}\n"),
    ]);
    let one = repo.extract("shop/One.java");
    assert!(calls_from(&one, "sym:shop/One.java::One.go").contains(&"sym:shop/util/Money.java::Money.round"), "{:?}", one.edges);
    let two = repo.extract("shop/Two.java");
    assert!(calls_from(&two, "sym:shop/Two.java::Two.go").is_empty(), "{:?}", two.edges);
}

#[test]
fn a_requirement_cited_in_a_java_comment_or_string_is_a_reference_from_its_declaration() {
    let src = "/* ADR-001 governs this file */\npackage shop;\n\npublic class Cart {\n    // FR-PAY-22: totals are rounded once\n    int total() {\n        String note = \"ADR-022\";\n        return 0;\n    }\n}\n";
    let ex = one("shop/Cart.java", src);
    let refs = edges(&ex, EdgeKind::References);
    assert!(refs.iter().any(|(s, t, c)| *s == "file:shop/Cart.java" && t.contains("ADR-001") && *c == "comment"), "{refs:?}");
    assert!(refs.iter().any(|(s, t, c)| *s == "sym:shop/Cart.java::Cart" && t.contains("FR-PAY-22") && *c == "comment"), "{refs:?}");
    assert!(refs.iter().any(|(s, t, c)| *s == "sym:shop/Cart.java::Cart.total" && t.contains("ADR-022") && *c == "string"), "{refs:?}");
}

const NAV: (&str, &str) = (
    "shop/util/Nav.java",
    "package shop.util;\n\npublic final class Nav {\n    public static void finish() {}\n    public static void helper() {}\n    public static void send() {}\n    public static void values() {}\n    public static void x() {}\n}\n",
);

#[test]
fn a_bare_call_from_a_type_whose_supertype_is_unread_is_no_edge() {
    let repo = Repo::new(&[
        NAV,
        ("shop/Base.java", "package shop;\n\npublic class Base {\n    public void helper() {}\n}\n"),
        ("shop/Mid.java", "package shop;\n\npublic class Mid extends Base {}\n"),
        ("shop/Screen.java", "package shop;\n\nimport static shop.util.Nav.finish;\n\npublic class Screen extends android.app.Activity {\n    void go() { finish(); }\n}\n"),
        ("shop/Sub.java", "package shop;\n\nimport static shop.util.Nav.helper;\n\nclass Sub extends Mid {\n    void go() { helper(); }\n}\n"),
        ("shop/Direct.java", "package shop;\n\nimport static shop.util.Nav.helper;\n\nclass Direct extends Base {\n    void go() { helper(); }\n}\n"),
        ("shop/Lost.java", "package shop;\n\npublic class Lost extends android.app.Activity {}\n"),
        ("shop/Far.java", "package shop;\n\nimport static shop.util.Nav.finish;\n\nclass Far extends Lost {\n    void go() { finish(); }\n}\n"),
    ]);
    let far = repo.extract("shop/Far.java");
    assert!(calls_from(&far, "sym:shop/Far.java::Far.go").is_empty(), "Lost's supertype is unread: {:?}", far.edges);
    let screen = repo.extract("shop/Screen.java");
    assert!(calls_from(&screen, "sym:shop/Screen.java::Screen.go").is_empty(), "the activity may declare finish: {:?}", screen.edges);
    let sub = repo.extract("shop/Sub.java");
    assert_eq!(calls_from(&sub, "sym:shop/Sub.java::Sub.go"), vec!["sym:shop/Base.java::Base.helper"], "Mid is walked to Base: {:?}", sub.edges);
    let direct = repo.extract("shop/Direct.java");
    assert_eq!(calls_from(&direct, "sym:shop/Direct.java::Direct.go"), vec!["sym:shop/Base.java::Base.helper"]);
}

#[test]
fn an_own_method_taking_the_arguments_wins_and_one_that_does_not_leaves_the_supertype_s() {
    let repo = Repo::new(&[
        ("shop/Base.java", "package shop;\n\npublic abstract class Base {\n    public abstract int size();\n    public void m(String s) {}\n}\n"),
        ("shop/Coll.java", "package shop;\n\nclass Coll extends Base {\n    public int size() { return 0; }\n    void m(int a, int b) {}\n    boolean isEmpty() { return size() == 0; }\n    void go() { m(\"x\"); }\n}\n"),
        ("shop/Two.java", "package shop;\n\ninterface Svc {\n    void run();\n}\n\nclass Impl implements Svc {\n    public void run() {}\n    void go() { run(); }\n}\n\nclass Up {\n    void m(String s) {}\n}\n\nclass Down extends Up {\n    void m(int a, int b) {}\n    void go() { m(\"x\"); }\n    void on(Down d) { d.m(\"x\"); }\n}\n"),
    ]);
    let coll = repo.extract("shop/Coll.java");
    assert_eq!(calls_from(&coll, "sym:shop/Coll.java::Coll.isEmpty"), vec!["sym:shop/Coll.java::Coll.size"], "an override binds the own method");
    assert_eq!(calls_from(&coll, "sym:shop/Coll.java::Coll.go"), vec!["sym:shop/Base.java::Base.m"]);
    let two = repo.extract("shop/Two.java");
    assert_eq!(calls_from(&two, "sym:shop/Two.java::Impl.go"), vec!["sym:shop/Two.java::Impl.run"]);
    assert_eq!(calls_from(&two, "sym:shop/Two.java::Down.go"), vec!["sym:shop/Two.java::Up.m"]);
    assert_eq!(calls_from(&two, "sym:shop/Two.java::Down.on"), vec!["sym:shop/Two.java::Up.m"]);
}

#[test]
fn an_own_method_that_cannot_take_the_arguments_leaves_the_call_to_an_unread_supertype() {
    let repo = Repo::new(&[
        ("shop/Screen.java", "package shop;\n\npublic class Screen extends android.app.Dialog {\n    void show(String msg) {}\n    void go() { show(); }\n    void say() { show(\"x\"); }\n}\n\nclass User {\n    Screen screen;\n    void go() { screen.show(); }\n}\n"),
        ("shop/Mixed.java", "package shop;\n\nclass Up {\n    void show(String m) {}\n}\n\nclass Mixed extends Up implements android.view.Shower {\n    void go() { show(); }\n}\n"),
    ]);
    let screen = repo.extract("shop/Screen.java");
    assert!(calls_from(&screen, "sym:shop/Screen.java::Screen.go").is_empty(), "{:?}", screen.edges);
    assert!(calls_from(&screen, "sym:shop/Screen.java::User.go").is_empty(), "{:?}", screen.edges);
    assert_eq!(calls_from(&screen, "sym:shop/Screen.java::Screen.say"), vec!["sym:shop/Screen.java::Screen.show"]);
    let mixed = repo.extract("shop/Mixed.java");
    assert!(calls_from(&mixed, "sym:shop/Mixed.java::Mixed.go").is_empty(), "{:?}", mixed.edges);
}

#[test]
fn a_receiver_in_another_file_binds_only_a_method_taking_the_arguments() {
    let repo = Repo::new(&[
        ("shop/Up.java", "package shop;\n\npublic class Up {\n    public void m(String s) {}\n}\n"),
        ("shop/Down.java", "package shop;\n\npublic class Down extends Up {\n    public void m() {}\n    public void v(String... xs) {}\n}\n"),
        ("shop/User.java", "package shop;\n\nclass User {\n    void on(Down d) { d.m(\"x\"); }\n    void off(Down d) { d.m(); }\n    void many(Down d) { d.v(\"a\", \"b\", \"c\"); }\n}\n"),
    ]);
    let user = repo.extract("shop/User.java");
    assert!(calls_from(&user, "sym:shop/User.java::User.on").is_empty(), "{:?}", user.edges);
    assert_eq!(calls_from(&user, "sym:shop/User.java::User.off"), vec!["sym:shop/Down.java::Down.m"]);
    assert_eq!(calls_from(&user, "sym:shop/User.java::User.many"), vec!["sym:shop/Down.java::Down.v"]);
}

#[test]
fn a_private_method_of_a_supertype_in_the_same_file_is_not_inherited() {
    let repo = Repo::new(&[
        NAV,
        ("shop/B.java", "package shop;\n\nimport static shop.util.Nav.helper;\n\nclass A {\n    private void helper() {}\n}\n\nclass B extends A {\n    void go() { helper(); }\n}\n"),
    ]);
    let ex = repo.extract("shop/B.java");
    assert_eq!(calls_from(&ex, "sym:shop/B.java::B.go"), vec!["sym:shop/util/Nav.java::Nav.helper"]);
}

#[test]
fn a_static_nested_class_calls_an_outer_static_method_but_not_one_an_instance_overload_shares() {
    let src = "package shop;\n\nclass Outer {\n    static int max2(int a) { return a; }\n    static void mixed() {}\n    void mixed(int n) {}\n    static class Builder {\n        void b() { max2(1); }\n        void c() { mixed(); }\n    }\n}\n";
    let ex = one("shop/Outer.java", src);
    assert_eq!(calls_from(&ex, "sym:shop/Outer.java::Outer.Builder.b"), vec!["sym:shop/Outer.java::Outer.max2"]);
    assert!(calls_from(&ex, "sym:shop/Outer.java::Outer.Builder.c").is_empty(), "{:?}", ex.edges);
}

#[test]
fn a_local_parameter_lambda_catch_for_resource_or_pattern_variable_hides_the_field_it_shadows() {
    let src = "package shop;

import java.util.List;

class Mail {
    void send() {}
    static void round() {}
}

class Other {
    void send() {}
    static void round() {}
}

class Post {
    Mail mail;
    List<Other> others;

    void param(Other mail) { mail.send(); }
    void lambda() { others.forEach(mail -> mail.send()); }
    void caught() { try { } catch (RuntimeException mail) { mail.send(); } }
    void loop() { for (Other mail = null; ; ) { mail.send(); } }
    void each() { for (var mail : others) { mail.send(); } }
    void pattern(Object o) { if (o instanceof Other mail) { mail.send(); } }
    void resource() throws Exception { try (AutoCloseable mail = null) { mail.send(); } }
    void local() { Other mail = null; mail.send(); }
    void cases(int k) { switch (k) { case 1: Other mail; break; default: mail = new Other(); mail.send(); } }
    void typeNamed() { Other Mail = null; Mail.round(); }
    void field() { mail.send(); }
}
";
    let ex = one("shop/Post.java", src);
    let wrong: Vec<_> = edges(&ex, EdgeKind::Calls).into_iter().filter(|(s, t, _)| t.ends_with("::Mail.send") && !s.ends_with(".field")).collect();
    assert!(wrong.is_empty(), "{wrong:?}");
    for m in ["param", "loop", "local", "cases"] {
        assert_eq!(calls_from(&ex, &format!("sym:shop/Post.java::Post.{m}")), vec!["sym:shop/Post.java::Other.send"], "{m}");
    }
    assert_eq!(calls_from(&ex, "sym:shop/Post.java::Post.typeNamed"), vec!["sym:shop/Post.java::Other.round"]);
    assert_eq!(calls_from(&ex, "sym:shop/Post.java::Post.field"), vec!["sym:shop/Post.java::Mail.send"]);
}

#[test]
fn an_anonymous_or_local_class_hides_what_it_or_its_unread_supertype_may_declare() {
    let src = "package shop;

class Mail {
    void send() {}
}

interface Task {
    void run();
}

class Job {
    void helper() {}
    void unread() { new Runnable() { public void run() { helper(); } }; }
    void own() { new Task() { public void run() { helper(); } void helper() {} }; }
    void local() {
        class Step { void helper() {} void run() { helper(); } }
        new Step().run();
    }
    void named() {
        class Mail { void other() {} }
        Mail m = new Mail();
        m.send();
    }
}
";
    let ex = one("shop/Job.java", src);
    for m in ["unread", "own", "local", "named"] {
        assert!(calls_from(&ex, &format!("sym:shop/Job.java::Job.{m}")).is_empty(), "{m}: {:?}", ex.edges);
    }
}

#[test]
fn a_type_s_static_member_is_called_only_when_no_value_or_static_import_may_hold_the_name() {
    let repo = Repo::new(&[
        ("shop/util/Money.java", MONEY),
        ("shop/Plain.java", "package shop;\n\nimport shop.util.Money;\n\nclass Plain {\n    void go() { Money.round(1); }\n}\n"),
        ("shop/Field.java", "package shop;\n\nimport shop.util.Money;\n\nclass Field {\n    java.util.logging.Logger Money;\n    void go() { Money.round(1); }\n}\n"),
        ("shop/Star.java", "package shop;\n\nimport shop.util.Money;\nimport static org.junit.Assert.*;\n\nclass Star {\n    void go() { Money.round(1); }\n}\n"),
        ("shop/Thrown.java", "package shop;\n\nimport shop.util.Money;\n\nclass Thrown extends Exception {\n    void go() { Money.round(1); }\n}\n"),
    ]);
    let plain = repo.extract("shop/Plain.java");
    assert_eq!(calls_from(&plain, "sym:shop/Plain.java::Plain.go"), vec!["sym:shop/util/Money.java::Money.round"]);
    for (rel, from) in [("shop/Field.java", "Field.go"), ("shop/Star.java", "Star.go"), ("shop/Thrown.java", "Thrown.go")] {
        let ex = repo.extract(rel);
        assert!(calls_from(&ex, &format!("sym:{rel}::{from}")).is_empty(), "{rel}: {:?}", ex.edges);
    }
}

#[test]
fn two_static_imports_of_one_name_bind_no_call() {
    let repo = Repo::new(&[
        ("a/A.java", "package a;\n\npublic class A { public static int of(int n) { return n; } }\n"),
        ("b/B.java", "package b;\n\npublic class B { public static int of(int n) { return n; } }\n"),
        ("c/Use.java", "package c;\n\nimport static a.A.of;\nimport static b.B.of;\n\nclass Use {\n    int go() { return of(1); }\n}\n"),
    ]);
    let ex = repo.extract("c/Use.java");
    assert!(calls_from(&ex, "sym:c/Use.java::Use.go").is_empty(), "{:?}", ex.edges);
}

#[test]
fn a_static_nested_class_reaches_no_outer_instance_method_and_an_inner_one_does() {
    let repo = Repo::new(&[
        NAV,
        ("shop/Outer.java", "package shop;\n\nimport static shop.util.Nav.helper;\n\nclass Outer {\n    void helper() {}\n    static class Nested {\n        void g() { helper(); }\n    }\n    class Inner {\n        void h() { helper(); }\n    }\n    interface Api {\n        default void i() { helper(); }\n    }\n}\n"),
    ]);
    let ex = repo.extract("shop/Outer.java");
    assert!(calls_from(&ex, "sym:shop/Outer.java::Outer.Nested.g").is_empty(), "{:?}", ex.edges);
    assert!(calls_from(&ex, "sym:shop/Outer.java::Outer.Api.i").is_empty(), "{:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:shop/Outer.java::Outer.Inner.h"), vec!["sym:shop/Outer.java::Outer.helper"]);
}

#[test]
fn a_type_parameter_is_not_the_repository_type_it_is_named_like() {
    let src = "package shop;\n\nclass State {\n    void reduce() {}\n}\n\nclass Store<State> {\n    State s;\n    void f() { s.reduce(); }\n}\n\nclass Box {\n    <State> void g(State x) { x.reduce(); }\n}\n";
    let ex = one("shop/Store.java", src);
    let wrong: Vec<_> = edges(&ex, EdgeKind::Calls).into_iter().filter(|(_, t, _)| t.ends_with("State.reduce")).collect();
    assert!(wrong.is_empty(), "{wrong:?}");
}

#[test]
fn a_field_never_stands_in_for_a_method_of_its_name() {
    let repo = Repo::new(&[
        NAV,
        ("shop/Base.java", "package shop;\n\npublic class Base {\n    public Object send;\n}\n"),
        ("shop/Holder.java", "package shop;\n\nimport static shop.util.Nav.send;\n\nclass Holder {\n    Object send;\n    void go() { send(); }\n}\n"),
        ("shop/Sub.java", "package shop;\n\nimport static shop.util.Nav.send;\n\nclass Sub extends Base {\n    void go() { send(); }\n}\n"),
    ]);
    let holder = repo.extract("shop/Holder.java");
    assert_eq!(calls_from(&holder, "sym:shop/Holder.java::Holder.go"), vec!["sym:shop/util/Nav.java::Nav.send"]);
    let sub = repo.extract("shop/Sub.java");
    assert!(!calls_from(&sub, "sym:shop/Sub.java::Sub.go").contains(&"sym:shop/Base.java::Base.send"), "{:?}", sub.edges);
}

#[test]
fn a_private_method_of_another_file_s_supertype_is_not_inherited() {
    let repo = Repo::new(&[
        NAV,
        ("shop/Base.java", "package shop;\n\npublic class Base {\n    private void helper() {}\n}\n"),
        ("shop/Sub.java", "package shop;\n\nimport static shop.util.Nav.helper;\n\nclass Sub extends Base {\n    void go() { helper(); }\n}\n"),
    ]);
    let sub = repo.extract("shop/Sub.java");
    assert!(!calls_from(&sub, "sym:shop/Sub.java::Sub.go").contains(&"sym:shop/Base.java::Base.helper"), "{:?}", sub.edges);
}

#[test]
fn object_record_and_enum_members_the_file_never_writes_hide_outer_and_imported_namesakes() {
    let repo = Repo::new(&[
        NAV,
        ("shop/Kinds.java", "package shop;\n\nimport static shop.util.Nav.values;\nimport static shop.util.Nav.x;\n\nclass Outer {\n    public Outer clone() { return this; }\n    class In {\n        void g() { clone(); }\n    }\n}\n\nenum Kind {\n    A;\n    void go() { values(); }\n}\n\nrecord P(int x, Mail mail) {\n    void go() { x(); }\n    void post() { mail.send(); }\n}\n\nclass Mail {\n    void send() {}\n}\n"),
    ]);
    let ex = repo.extract("shop/Kinds.java");
    for from in ["Outer.In.g", "Kind.go", "P.go"] {
        assert!(calls_from(&ex, &format!("sym:shop/Kinds.java::{from}")).is_empty(), "{from}: {:?}", ex.edges);
    }
    assert_eq!(calls_from(&ex, "sym:shop/Kinds.java::P.post"), vec!["sym:shop/Kinds.java::Mail.send"], "a record component is a typed field");
}

#[test]
fn an_own_varargs_method_never_beats_a_supertype_s_fixed_arity_one_taking_the_arguments() {
    let src = "package shop;\n\nclass Up {\n    void m(int a) {}\n}\n\nclass Down extends Up {\n    void m(int... xs) {}\n    void go() { m(1); }\n    void two() { m(1, 2); }\n}\n";
    let ex = one("shop/Down.java", src);
    assert!(calls_from(&ex, "sym:shop/Down.java::Down.go").is_empty(), "{:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:shop/Down.java::Down.two"), vec!["sym:shop/Down.java::Down.m"]);
}

#[test]
fn an_annotation_type_in_the_repository_decorates_and_a_platform_one_writes_nothing() {
    let repo = Repo::new(&[
        ("shop/meta/Audited.java", "package shop.meta;\n\npublic @interface Audited {\n    String value();\n}\n"),
        ("shop/Cart.java", "package shop;\n\nimport shop.meta.Audited;\n\n@Audited(\"cart\")\n@Deprecated\npublic class Cart {\n    @Audited(\"total\") int total;\n\n    @Override\n    @shop.meta.Audited(\"s\")\n    public String toString() { return \"\"; }\n}\n"),
    ]);
    let ex = repo.extract("shop/Cart.java");
    let deco = edges(&ex, EdgeKind::DecoratedBy);
    for from in ["sym:shop/Cart.java::Cart", "sym:shop/Cart.java::Cart.total", "sym:shop/Cart.java::Cart.toString"] {
        assert!(deco.contains(&(from, "sym:shop/meta/Audited.java::Audited", "")), "{from}: {deco:?}");
    }
    assert_eq!(deco.len(), 3, "`@Deprecated` and `@Override` resolve nowhere and write nothing: {deco:?}");
    assert!(!ids(&ex).iter().any(|i| i.starts_with("anno:") || i.starts_with("deco:")), "{:?}", ids(&ex));
}

// A type variable named like the annotation type shadows it; the compiler rejects the use, and the
// graph must not pretend it reached the type.
#[test]
fn an_annotation_named_like_a_type_variable_in_scope_decorates_nothing() {
    let repo = Repo::new(&[
        ("shop/Audited.java", "package shop;\n\npublic @interface Audited {}\n"),
        ("shop/Box.java", "package shop;\n\nclass Box<Audited> {\n    @Audited int size;\n}\n\nclass Other {\n    @Audited <Audited> void lift() {}\n    @Audited void drop() {}\n}\n"),
    ]);
    let ex = repo.extract("shop/Box.java");
    assert_eq!(edges(&ex, EdgeKind::DecoratedBy), vec![("sym:shop/Box.java::Other.drop", "sym:shop/Audited.java::Audited", "")], "{:?}", ex.edges);
}

#[test]
fn a_nested_annotation_type_decorates_a_member_and_a_constructor() {
    let src = "package shop;\n\npublic class Cart {\n    @interface Tracked {}\n\n    @Tracked\n    public Cart() {}\n}\n";
    let ex = one("shop/Cart.java", src);
    assert_eq!(edges(&ex, EdgeKind::DecoratedBy), vec![("sym:shop/Cart.java::Cart.Cart", "sym:shop/Cart.java::Cart.Tracked", "")], "{:?}", ex.edges);
}

// ---- signature types ----

const KEY: &str = "package shop;\n\npublic class Key {\n    public static class Inner {}\n}\n";

fn imports_to<'a>(ex: &'a crate::model::Extraction, to: &str) -> Vec<&'a str> {
    edges(ex, EdgeKind::Imports).into_iter().filter(|(_, t, _)| *t == to).map(|(_, _, c)| c).collect()
}

#[test]
fn a_type_named_only_in_a_signature_writes_the_type_edge_to_its_file() {
    let uses = [
        ("shop/Param.java", "package shop;\n\nclass Param {\n    void f(Key k) {}\n}\n"),
        ("shop/Return.java", "package shop;\n\ninterface Return {\n    Key get();\n}\n"),
        ("shop/Field.java", "package shop;\n\nclass Field {\n    private Key k;\n}\n"),
        ("shop/Ctor.java", "package shop;\n\nclass Ctor {\n    Ctor(Key k) {}\n}\n"),
        ("shop/Generic.java", "package shop;\n\nclass Generic {\n    java.util.Map<String, ? extends java.util.List<Key>> f() { return null; }\n}\n"),
        ("shop/Array.java", "package shop;\n\nclass Array {\n    void f(Key[] ks, Key... more) {}\n}\n"),
        ("shop/Nested.java", "package shop;\n\nclass Nested {\n    void f(Key.Inner i) {}\n}\n"),
        ("shop/Rec.java", "package shop;\n\nrecord Rec(Key k) {}\n"),
    ];
    let mut files = vec![("shop/Key.java", KEY)];
    files.extend(uses);
    let repo = Repo::new(&files);
    for (rel, _) in uses {
        let ex = repo.extract(rel);
        assert!(imports_to(&ex, "file:shop/Key.java").contains(&"Key"), "{rel}: {:?}", ex.edges);
    }
}

#[test]
fn a_signature_type_a_type_parameter_a_local_class_or_two_stars_bind_writes_nothing() {
    let repo = Repo::new(&[
        ("shop/Key.java", KEY),
        ("x/Dup.java", "package x;\n\npublic class Dup {}\n"),
        ("y/Dup.java", "package y;\n\npublic class Dup {}\n"),
        ("shop/Mask.java", "package shop;\n\nclass Mask<Key> {\n    Key k;\n    <Key> Key f(Key k) { return k; }\n}\n"),
        ("shop/Local.java", "package shop;\n\nclass Local {\n    void f() {\n        class Key {}\n        java.util.function.Consumer<Key> c = (Key k) -> {};\n    }\n}\n"),
        ("shop/Stars.java", "package shop;\n\nimport x.*;\nimport y.*;\n\nclass Stars {\n    void f(Dup d) {}\n}\n"),
    ]);
    for rel in ["shop/Mask.java", "shop/Local.java", "shop/Stars.java"] {
        let ex = repo.extract(rel);
        assert!(edges(&ex, EdgeKind::Imports).is_empty(), "{rel}: {:?}", ex.edges);
    }
}

const MAP: (&str, &str) = ("u/Map.java", "package u;\n\npublic interface Map {\n    boolean isEmpty();\n    default int size() { return 0; }\n}\n");
const ABSTRACT_MAP: (&str, &str) = (
    "u/AbstractMap.java",
    "package u;\n\npublic abstract class AbstractMap implements Map {\n    public boolean isEmpty() { return true; }\n    public int size() { return 1; }\n}\n",
);

#[test]
fn a_superclass_method_beats_the_interface_a_class_also_implements() {
    let repo = Repo::new(&[
        MAP,
        ABSTRACT_MAP,
        ("u/HashMap.java", "package u;\n\npublic class HashMap extends AbstractMap implements Map {\n    void go() { isEmpty(); }\n    void count() { size(); }\n}\n\nclass Near extends AbstractMap implements Map {\n    void go() { isEmpty(); }\n}\n"),
    ]);
    let ex = repo.extract("u/HashMap.java");
    assert_eq!(calls_from(&ex, "sym:u/HashMap.java::HashMap.go"), vec!["sym:u/AbstractMap.java::AbstractMap.isEmpty"], "over the interface's abstract method: {:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:u/HashMap.java::HashMap.count"), vec!["sym:u/AbstractMap.java::AbstractMap.size"], "over the interface's default: {:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:u/HashMap.java::Near.go"), vec!["sym:u/AbstractMap.java::AbstractMap.isEmpty"], "{:?}", ex.edges);
}

#[test]
fn an_interface_binds_only_when_no_superclass_level_declares_the_name_and_none_is_unread() {
    let repo = Repo::new(&[
        MAP,
        ("u/Base.java", "package u;\n\npublic class Base {}\n"),
        ("u/Lost.java", "package u;\n\npublic class Lost extends android.app.Activity {}\n"),
        ("u/Sized.java", "package u;\n\npublic interface Sized extends Map {\n    default int size() { return 2; }\n}\n"),
        ("u/Holder.java", "package u;\n\npublic class Holder implements Map {\n    public boolean isEmpty() { return true; }\n}\n"),
        (
            "u/Users.java",
            "package u;\n\nclass Plain extends Base implements Map {\n    void go() { size(); }\n}\n\nclass Screen extends android.app.Activity implements Map {\n    void go() { isEmpty(); }\n}\n\nclass Far extends Lost implements Map {\n    void go() { isEmpty(); }\n}\n\nclass Specific extends Holder implements Sized {\n    void go() { size(); }\n}\n",
        ),
    ]);
    let ex = repo.extract("u/Users.java");
    assert_eq!(calls_from(&ex, "sym:u/Users.java::Plain.go"), vec!["sym:u/Map.java::Map.size"], "Base declares no size: {:?}", ex.edges);
    assert!(calls_from(&ex, "sym:u/Users.java::Screen.go").is_empty(), "the activity may declare isEmpty: {:?}", ex.edges);
    assert!(calls_from(&ex, "sym:u/Users.java::Far.go").is_empty(), "Lost's superclass may declare isEmpty: {:?}", ex.edges);
    assert!(calls_from(&ex, "sym:u/Users.java::Specific.go").is_empty(), "Holder inherits Map's size and Sized overrides it: {:?}", ex.edges);
}

const NAMESAKE: (&str, &str) = ("a/u/U.java", "package a.u;\n\npublic class U {\n    public static void help() {}\n}\n");

#[test]
fn an_interface_s_static_method_is_not_inherited_by_its_implementors_or_subinterfaces() {
    let repo = Repo::new(&[
        NAMESAKE,
        ("a/I.java", "package a;\n\npublic interface I {\n    static void help() {}\n}\n"),
        ("a/J.java", "package a;\n\npublic interface J extends I {}\n"),
        (
            "a/D.java",
            "package a;\n\nimport static a.u.U.help;\n\nclass D implements J {\n    void go() { help(); }\n}\n\nclass C implements I {\n    void go() { help(); }\n}\n\ninterface K {\n    static void help() {}\n}\n\nclass E implements K {\n    void go() { help(); }\n}\n",
        ),
    ]);
    let ex = repo.extract("a/D.java");
    for from in ["D", "C", "E"] {
        assert_eq!(calls_from(&ex, &format!("sym:a/D.java::{from}.go")), vec!["sym:a/u/U.java::U.help"], "{from}: {:?}", ex.edges);
    }
}

#[test]
fn a_package_private_method_is_not_inherited_by_a_subclass_in_another_package() {
    let repo = Repo::new(&[
        ("a/Base.java", "package a;\n\npublic class Base {\n    void help() {}\n    protected void kept() {}\n}\n"),
        ("a/Mid.java", "package a;\n\npublic class Mid extends Base {}\n"),
        ("a/Near.java", "package a;\n\nclass Near extends Mid {\n    void go() { help(); }\n}\n"),
        (
            "b/Outer.java",
            "package b;\n\nclass Outer {\n    void help() {}\n    void kept() {}\n    class Deep extends a.Mid {\n        void go() { help(); }\n        void keep() { kept(); }\n    }\n    class Direct extends a.Base {\n        void go() { help(); }\n    }\n}\n",
        ),
    ]);
    let near = repo.extract("a/Near.java");
    assert_eq!(calls_from(&near, "sym:a/Near.java::Near.go"), vec!["sym:a/Base.java::Base.help"], "one package: {:?}", near.edges);
    let ex = repo.extract("b/Outer.java");
    assert_eq!(calls_from(&ex, "sym:b/Outer.java::Outer.Deep.go"), vec!["sym:b/Outer.java::Outer.help"], "{:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:b/Outer.java::Outer.Direct.go"), vec!["sym:b/Outer.java::Outer.help"], "{:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:b/Outer.java::Outer.Deep.keep"), vec!["sym:a/Base.java::Base.kept"], "a protected method crosses packages: {:?}", ex.edges);
}
