//! Python extraction cases, each on an inline source.

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

    pub(super) fn resolver(&self) -> Resolver {
        let cfg = Config { code_globs: vec!["**/*.py".into()], ..Config::default() };
        Resolver::new(self.dir.path(), &cfg).unwrap()
    }

    pub(super) fn extract(&self, rel: &str) -> Extraction {
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
fn a_python_file_is_one_file_node_and_parses_clean() {
    let src = "import os\n\nclass A:\n    x: int = 1\n\n    async def run(self) -> None:\n        return None\n";
    let repo = Repo::new(&[("a.py", src)]);
    let ex = repo.extract("a.py");
    assert!(ex.nodes.iter().any(|n| n.kind == NodeKind::File && n.id == "file:a.py"));
    assert_eq!(Lang::of("pkg/a.py"), Some(Lang::Python));
    let tree = Lang::Python.parse(src.as_bytes()).unwrap();
    assert!(!tree.root_node().has_error(), "{}", tree.root_node().to_sexp());
}

#[test]
fn every_kind_the_walk_matches_is_in_the_grammar() {
    let lang = Lang::Python.grammar().unwrap();
    for kind in [
        "module", "expression_statement", "string", "string_content", "comment", "import_statement",
        "import_from_statement", "dotted_name", "aliased_import", "relative_import", "import_prefix",
        "wildcard_import", "assignment", "attribute", "identifier", "type", "decorated_definition", "decorator",
        "call", "argument_list", "class_definition", "function_definition", "block", "parameters",
        "typed_parameter", "typed_default_parameter", "if_statement", "comparison_operator", "try_statement",
        "list", "tuple", "keyword_argument", "pattern_list",
    ] {
        assert_ne!(lang.id_for_node_kind(kind, true), 0, "kind {kind} missing");
    }
    for field in ["name", "alias", "module_name", "left", "right", "type", "definition", "superclasses", "body", "function", "object", "attribute", "arguments", "parameters", "condition", "consequence"] {
        assert!(lang.field_id_for_name(field).is_some(), "field {field} missing");
    }
}

#[test]
fn the_resolver_reads_python_project_roots_from_its_walk() {
    let repo = Repo::new(&[
        ("lib/pyproject.toml", "[project]\nname = \"shop\"\n"),
        ("lib/shop/__init__.py", ""),
        ("lib/app/main.py", "import shop\n"),
    ]);
    assert_eq!(repo.resolver().state::<crate::code::python::Modules>().absolute("lib/app/main.py", "shop").as_deref(), Some("lib/shop/__init__.py"));
}

fn node<'a>(ex: &'a Extraction, id: &str) -> &'a crate::model::Node {
    ex.nodes.iter().find(|n| n.id == id).unwrap_or_else(|| panic!("{id} not extracted: {:?}", ids(ex)))
}

const SHOP: &str = r##""""Shop orders."""
import os

__all__ = ["Order", "total", "_hidden"]

LIMIT = 10
_cache: dict = {}
count: int


class Order:
    """An order. FR-PAY-03"""

    kind = "order"
    lines: list

    def __init__(self, store):
        self.store = store

    @property
    def total(self) -> int:
        return 0

    class Line:
        sku: str

        def price(self):
            # INV-11
            return "FR-SEC-21"


@decorate
def total(order: Order) -> int:
    """Sum."""
    return order.total


def _hidden():
    pass


async def helper():
    x = 1
    def inner():
        pass


if __name__ == "__main__":
    MAIN = 1
    def main():
        pass
"##;

#[test]
fn defs_classes_members_and_plain_assignments_are_symbols() {
    let repo = Repo::new(&[("shop.py", SHOP)]);
    let ex = repo.extract("shop.py");
    let want = [
        "__all__", "LIMIT", "_cache", "count", "Order", "Order.kind", "Order.lines", "Order.__init__", "Order.total",
        "Order.Line", "Order.Line.sku", "Order.Line.price", "total", "_hidden", "helper",
    ];
    for s in want {
        assert!(ids(&ex).contains(&format!("sym:shop.py::{s}").as_str()), "{s}: {:?}", ids(&ex));
    }
    assert_eq!(
        ex.nodes.iter().filter(|n| n.kind == NodeKind::Symbol).count(),
        want.len(),
        "no local, no attribute target, nothing under `if __name__`: {:?}",
        ids(&ex)
    );
}

#[test]
fn all_decides_exports_and_members_hang_off_their_class() {
    let repo = Repo::new(&[("shop.py", SHOP), ("plain.py", "def run():\n    pass\n\ndef _private():\n    pass\n")]);
    let ex = repo.extract("shop.py");
    let declares = edges(&ex, EdgeKind::Declares);
    for want in [
        ("file:shop.py", "sym:shop.py::Order", "export"),
        ("file:shop.py", "sym:shop.py::_hidden", "export"),
        ("file:shop.py", "sym:shop.py::LIMIT", ""),
        ("file:shop.py", "sym:shop.py::helper", ""),
        ("sym:shop.py::Order", "sym:shop.py::Order.total", ""),
        ("sym:shop.py::Order", "sym:shop.py::Order.Line", ""),
        ("sym:shop.py::Order.Line", "sym:shop.py::Order.Line.price", ""),
    ] {
        assert!(declares.contains(&want), "{want:?} in {declares:?}");
    }
    let ex = repo.extract("plain.py");
    let declares = edges(&ex, EdgeKind::Declares);
    assert!(declares.contains(&("file:plain.py", "sym:plain.py::run", "export")));
    assert!(declares.contains(&("file:plain.py", "sym:plain.py::_private", "")));
}

#[test]
fn a_body_is_the_docstring_and_the_header() {
    let repo = Repo::new(&[("shop.py", SHOP)]);
    let ex = repo.extract("shop.py");
    let order = node(&ex, "sym:shop.py::Order");
    assert_eq!(order.body, "An order. FR-PAY-03\nclass Order:");
    assert_eq!((order.line, order.end), (11, 29));
    let total = node(&ex, "sym:shop.py::total");
    assert_eq!(total.body, "Sum.\ndef total(order: Order) -> int:");
    assert_eq!((total.line, total.end), (33, 35), "the def's line, not the decorator's");
    assert_eq!(node(&ex, "sym:shop.py::LIMIT").body, "LIMIT = 10");
}

#[test]
fn ids_in_docstrings_comments_and_strings_belong_to_the_enclosing_definition() {
    let repo = Repo::new(&[("shop.py", SHOP)]);
    let ex = repo.extract("shop.py");
    let refs = edges(&ex, EdgeKind::References);
    assert!(refs.contains(&("sym:shop.py::Order", "FR-PAY-03", "comment")), "a docstring is documentation: {refs:?}");
    assert!(refs.contains(&("sym:shop.py::Order.Line.price", "INV-11", "comment")), "{refs:?}");
    assert!(refs.contains(&("sym:shop.py::Order.Line.price", "FR-SEC-21", "string")), "{refs:?}");
}

#[test]
fn a_redefined_name_keeps_the_first_span_and_declares_once() {
    let src = "class C:\n    @property\n    def v(self):\n        return 1\n\n    @v.setter\n    def v(self, x):\n        pass\n\n\ndef f():\n    pass\n\n\nx = 1\n\n\ndef f():\n    pass\n";
    let repo = Repo::new(&[("m.py", src)]);
    let ex = repo.extract("m.py");
    for id in ["sym:m.py::C.v", "sym:m.py::f"] {
        assert_eq!(ex.nodes.iter().filter(|n| n.id == id).count(), 1, "{id}: {:?}", ids(&ex));
    }
    assert_eq!((node(&ex, "sym:m.py::C.v").line, node(&ex, "sym:m.py::C.v").end), (3, 4));
    assert_eq!((node(&ex, "sym:m.py::f").line, node(&ex, "sym:m.py::f").end), (11, 12));
    assert_eq!(edges(&ex, EdgeKind::Declares).iter().filter(|e| e.1 == "sym:m.py::f").count(), 1);
}

#[test]
fn a_citation_below_a_redefinition_starts_from_a_written_node() {
    let src = "class C:\n    def a(self):\n        pass\n\n\nclass C:\n    def b(self):\n        return \"FR-PAY-03\"\n";
    let repo = Repo::new(&[("m.py", src)]);
    let ex = repo.extract("m.py");
    let written: Vec<&str> = ids(&ex);
    for e in &ex.edges {
        assert!(written.contains(&e.source.as_str()), "edge from unwritten {}: {:?}", e.source, written);
    }
    assert!(edges(&ex, EdgeKind::References).contains(&("sym:m.py::C.b", "FR-PAY-03", "string")));
}

fn set<'a>(v: Vec<(&'a str, &'a str, &'a str)>) -> std::collections::BTreeSet<(&'a str, &'a str, &'a str)> {
    v.into_iter().collect()
}

fn calls_from<'a>(ex: &'a Extraction, from: &str) -> std::collections::BTreeSet<&'a str> {
    ex.edges.iter().filter(|e| e.kind == EdgeKind::Calls && e.source == from).map(|e| e.target.as_str()).collect()
}

const ORDERS: &str = r##"import shop.store
from . import base
from .store import Store as Db, connect
from shop.base import *
from os import path


class Order(base.Base):
    ledger: Db

    def __init__(self, db: Db):
        self.db = db
        self.backup = Db()
        self.cache = make_cache()

    @base.traced
    def pay(self):
        self.db.open()
        self.backup.close()
        self.ledger.open()
        self.cache.clear()
        self.refund()
        shop.store.connect()
        Db.open(self)

    def refund(self):
        connect()
        path.join("a")


def make_cache():
    return {}


def main():
    Order(connect()).pay()
"##;

fn orders_repo() -> Repo {
    Repo::new(&[
        ("lib/pyproject.toml", "[project]\nname = \"shop\"\n"),
        ("lib/shop/__init__.py", ""),
        ("lib/shop/store.py", "class Store:\n    def open(self):\n        pass\n\n    def close(self):\n        pass\n\n\ndef connect():\n    return Store()\n"),
        ("lib/shop/base.py", "class Base:\n    pass\n\n\ndef traced(f):\n    return f\n"),
        ("lib/shop/orders.py", ORDERS),
    ])
}

#[test]
fn imports_name_a_module_or_a_name_and_an_outside_module_writes_nothing() {
    let ex = orders_repo().extract("lib/shop/orders.py");
    assert_eq!(
        set(edges(&ex, EdgeKind::Imports)),
        set(vec![
            ("file:lib/shop/orders.py", "file:lib/shop/base.py", "*"),
            ("file:lib/shop/orders.py", "file:lib/shop/store.py", "*"),
            ("file:lib/shop/orders.py", "file:lib/shop/store.py", "Store"),
            ("file:lib/shop/orders.py", "file:lib/shop/store.py", "connect"),
        ])
    );
}

#[test]
fn calls_through_imports_self_and_typed_attributes() {
    let ex = orders_repo().extract("lib/shop/orders.py");
    assert_eq!(
        calls_from(&ex, "sym:lib/shop/orders.py::Order.pay"),
        [
            "sym:lib/shop/orders.py::Order.refund",
            "sym:lib/shop/store.py::Store.close",
            "sym:lib/shop/store.py::Store.open",
            "sym:lib/shop/store.py::connect",
        ]
        .into_iter()
        .collect(),
        "`self.cache` has no class type, so `clear` is not resolved"
    );
    assert_eq!(calls_from(&ex, "sym:lib/shop/orders.py::Order.refund"), ["sym:lib/shop/store.py::connect"].into_iter().collect());
    assert_eq!(calls_from(&ex, "sym:lib/shop/orders.py::main"), ["sym:lib/shop/orders.py::Order", "sym:lib/shop/store.py::connect"].into_iter().collect());
    let ex = orders_repo().extract("lib/shop/store.py");
    assert_eq!(calls_from(&ex, "sym:lib/shop/store.py::connect"), ["sym:lib/shop/store.py::Store"].into_iter().collect());
}

#[test]
fn a_resolved_base_extends_and_a_resolved_decorator_decorates() {
    let ex = orders_repo().extract("lib/shop/orders.py");
    assert_eq!(edges(&ex, EdgeKind::Extends), vec![("sym:lib/shop/orders.py::Order", "sym:lib/shop/base.py::Base", "")]);
    assert_eq!(edges(&ex, EdgeKind::DecoratedBy), vec![("sym:lib/shop/orders.py::Order.pay", "sym:lib/shop/base.py::traced", "base.traced")]);
    let plain = Repo::new(&[("a.py", "import dataclasses\n\n\n@dataclasses.dataclass\nclass A:\n    @property\n    def x(self):\n        return 1\n")]).extract("a.py");
    assert!(edges(&plain, EdgeKind::DecoratedBy).is_empty(), "a decorator outside the repository names nothing (L8)");
}

#[test]
fn a_binding_inside_a_function_stays_in_that_function() {
    let repo = Repo::new(&[
        ("tools/util.py", "def helper():\n    pass\n"),
        ("tools/main.py", "def one():\n    from util import helper\n    helper()\n\n\ndef two():\n    helper()\n\n\nif __name__ == \"__main__\":\n    one()\n"),
    ]);
    let ex = repo.extract("tools/main.py");
    assert_eq!(calls_from(&ex, "sym:tools/main.py::one"), ["sym:tools/util.py::helper"].into_iter().collect());
    assert!(calls_from(&ex, "sym:tools/main.py::two").is_empty(), "`helper` is not bound in `two`");
    assert_eq!(calls_from(&ex, "file:tools/main.py"), ["sym:tools/main.py::one"].into_iter().collect(), "the main guard calls from the file");
}

#[test]
fn a_name_re_exported_by_a_package_init_resolves_to_its_declaring_module() {
    let repo = Repo::new(&[
        ("shop/__init__.py", "from .store import Store\n"),
        ("shop/store.py", "class Store:\n    def open(self):\n        pass\n"),
        ("app.py", "from shop import Store\n\n\ndef main():\n    Store()\n    Store.open(None)\n"),
    ]);
    let ex = repo.extract("app.py");
    let calls = calls_from(&ex, "sym:app.py::main");
    assert!(!calls.iter().any(|c| c.starts_with("sym:shop/__init__.py::")), "no symbol has that id: {calls:?}");
    assert_eq!(calls, ["sym:shop/store.py::Store", "sym:shop/store.py::Store.open"].into_iter().collect());
    assert_eq!(edges(&ex, EdgeKind::Imports), vec![("file:app.py", "file:shop/__init__.py", "Store")], "the edge names what the statement imports");
}

#[test]
fn a_decorator_called_with_arguments_decorates_with_its_callee() {
    let repo = Repo::new(&[
        ("tools/retry.py", "def retry(times):\n    def wrap(f):\n        return f\n    return wrap\n"),
        ("tools/job.py", "from retry import retry\n\n\n@retry(3)\ndef run():\n    pass\n\n\nclass Job:\n    @retry(times=2)\n    def go(self):\n        pass\n"),
    ]);
    let ex = repo.extract("tools/job.py");
    assert_eq!(
        set(edges(&ex, EdgeKind::DecoratedBy)),
        set(vec![
            ("sym:tools/job.py::Job.go", "sym:tools/retry.py::retry", "retry"),
            ("sym:tools/job.py::run", "sym:tools/retry.py::retry", "retry"),
        ])
    );
}

#[test]
fn an_own_file_target_no_node_backs_writes_no_edge() {
    let src = "class A:\n    def go(self):\n        self.missing()\n        A.absent()\n        self.t.run()\n\n    def __init__(self):\n        self.t = T()\n        self.t = U()\n\n\nclass T:\n    def run(self):\n        pass\n\n\nclass U:\n    def run(self):\n        pass\n";
    let ex = Repo::new(&[("m.py", src)]).extract("m.py");
    assert!(calls_from(&ex, "sym:m.py::A.go").is_empty(), "{:?}", calls_from(&ex, "sym:m.py::A.go"));
}

#[test]
fn a_decorated_definition_inside_a_function_does_not_decorate_the_function() {
    let src = "def deco(f):\n    return f\n\n\ndef outer():\n    @deco\n    def inner():\n        pass\n";
    let ex = Repo::new(&[("m.py", src)]).extract("m.py");
    assert!(edges(&ex, EdgeKind::DecoratedBy).is_empty(), "{:?}", edges(&ex, EdgeKind::DecoratedBy));
}

fn shadow_repo(main: &str) -> Repo {
    Repo::new(&[("tools/util.py", "def connect():\n    pass\n\n\nclass Store:\n    def open(self):\n        pass\n"), ("tools/store.py", "def open():\n    pass\n"), ("tools/main.py", main)])
}

fn no_calls(main: &str, from: &str) {
    let ex = shadow_repo(main).extract("tools/main.py");
    assert!(calls_from(&ex, from).is_empty(), "{:?}", calls_from(&ex, from));
}

#[test]
fn a_parameter_shadows_an_imported_name() {
    no_calls("from util import connect\n\n\ndef f(connect):\n    connect()\n", "sym:tools/main.py::f");
    no_calls("from util import connect\n\n\ndef f(*connect, **kw):\n    connect()\n", "sym:tools/main.py::f");
    no_calls("from util import connect\n\n\ndef f(a, *, connect=None):\n    connect()\n", "sym:tools/main.py::f");
}

#[test]
fn a_local_assignment_shadows_a_bound_module() {
    no_calls("import store\n\n\ndef f():\n    store = make()\n    store.open()\n", "sym:tools/main.py::f");
    no_calls("import store\n\n\ndef f():\n    a, (store, b) = make()\n    store.open()\n", "sym:tools/main.py::f");
}

#[test]
fn a_lambda_parameter_shadows_only_inside_the_lambda() {
    no_calls("from util import connect\n\n\nrun = lambda connect: connect()\n", "sym:tools/main.py::run");
    let ex = shadow_repo("from util import connect\n\n\ndef f():\n    g = lambda connect: connect()\n    connect()\n").extract("tools/main.py");
    assert_eq!(calls_from(&ex, "sym:tools/main.py::f"), ["sym:tools/util.py::connect"].into_iter().collect());
}

#[test]
fn a_loop_with_except_or_walrus_target_shadows() {
    no_calls("from util import Store\n\n\ndef f(xs):\n    for Store in xs:\n        Store.open()\n", "sym:tools/main.py::f");
    no_calls("from util import connect\n\n\ndef f(p):\n    with p as connect:\n        connect()\n", "sym:tools/main.py::f");
    no_calls("from util import connect\n\n\ndef f(p):\n    try:\n        p()\n    except OSError as connect:\n        connect()\n", "sym:tools/main.py::f");
    no_calls("from util import connect\n\n\ndef f(p):\n    if (connect := p):\n        connect()\n", "sym:tools/main.py::f");
}

#[test]
fn a_local_shadows_a_top_level_name_and_a_class() {
    no_calls("def helper():\n    pass\n\n\ndef f(helper):\n    helper()\n", "sym:tools/main.py::f");
    no_calls("class A:\n    def m(self):\n        pass\n\n\ndef f(A):\n    A.m(1)\n", "sym:tools/main.py::f");
}

#[test]
fn a_global_declaration_cancels_the_shadow_and_a_local_import_is_not_one() {
    let ex = shadow_repo("from util import connect\n\n\ndef f():\n    global connect\n    connect = 1\n    connect()\n").extract("tools/main.py");
    assert_eq!(calls_from(&ex, "sym:tools/main.py::f"), ["sym:tools/util.py::connect"].into_iter().collect());
    let ex = shadow_repo("def f():\n    from util import connect\n    connect()\n").extract("tools/main.py");
    assert_eq!(calls_from(&ex, "sym:tools/main.py::f"), ["sym:tools/util.py::connect"].into_iter().collect());
}

#[test]
fn a_function_under_an_if_shadows_only_its_own_calls() {
    let main = "from util import connect\n\n\ntry:\n    def helper(connect):\n        connect()\nexcept ImportError:\n    pass\n\nif __name__ == \"__main__\":\n    connect()\n";
    let ex = shadow_repo(main).extract("tools/main.py");
    assert_eq!(
        calls_from(&ex, "file:tools/main.py"),
        ["sym:tools/util.py::connect"].into_iter().collect(),
        "the module-level call keeps its edge, the parameter's call adds none"
    );
    no_calls("from util import connect\n\n\nif True:\n    def helper(connect):\n        connect()\n", "file:tools/main.py");
}

#[test]
fn all_extended_by_a_literal_exports_the_added_names() {
    let src = "__all__ = [\"a\"]\n__all__ += [\"go\"]\n\n\ndef a():\n    pass\n\n\ndef go():\n    pass\n\n\ndef _b():\n    pass\n";
    let ex = Repo::new(&[("m.py", src)]).extract("m.py");
    let declares = edges(&ex, EdgeKind::Declares);
    assert!(declares.contains(&("file:m.py", "sym:m.py::go", "export")), "{declares:?}");
    assert!(declares.contains(&("file:m.py", "sym:m.py::_b", "")), "{declares:?}");
}
