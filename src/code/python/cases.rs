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
    assert_eq!(repo.resolver().python().absolute("lib/app/main.py", "shop").as_deref(), Some("lib/shop/__init__.py"));
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
