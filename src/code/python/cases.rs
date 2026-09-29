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
