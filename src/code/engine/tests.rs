//! The engine over synthetic Python queries: no language has queries yet, so these pin the
//! qualification, export, body, resolution and citation rules every port will lean on.

use super::*;
use crate::code::prose::testing::{lines, repo};
use crate::code::spec::{ImportPaths, Sources, NONE};

const DECL: &[&str] = &[
    r#"((function_definition name: (identifier) @name) @definition.function (#eq? @name "_kept") (#set! export "always"))"#,
    "(class_definition name: (identifier) @name) @definition.class",
    "(function_definition name: (identifier) @name) @definition.function",
    "(import_from_statement module_name: (_) @import.path name: (dotted_name) @import.name) @import",
];

const REFS: &[&str] = &[
    "(call function: (identifier) @name) @reference.calls",
    "(call function: (attribute object: (identifier) @receiver attribute: (identifier) @name)) @reference.calls",
    "(function_definition) @local.scope",
    "(parameters (identifier) @local.definition)",
    "(comment) @cite.comment",
];

const SOURCE: &str = r#"from .util import helper

# Pays the bill.
# Twice.
class Shop:
    def pay(self, amount):
        charge(amount)
        self.log()
        return helper()

def charge(x):
    charge(x)
    _hidden()  # see ADR-24

def _hidden():
    pass

def _kept():
    pass

def run(charge):
    charge()

def charge(y):
    pass
"#;

fn spec(body: Body, export: Export) -> Spec {
    Spec {
        queries: Sources { decl: &[], refs: &[] },
        view: None,
        comments: None,
        body,
        export,
        visibility: Visibility::File,
        ambiguity: Ambiguity::Unique,
        imports: ImportPaths { root: false, suffixes: &[""] },
        cross_file: false,
        dotted_dirs: false,
        hooks: NONE,
        legacy: None,
    }
}

const STRIPPED: Body = Body { doc: Doc::Adjacent { kinds: &["comment"], between: &[] }, form: Form::Stripped, cap: Cap::Head, line: Line::First };

fn compiled(sources: &[&str]) -> &'static Compiled {
    Box::leak(Box::new(queries::compile(&Lang::Python.grammar().unwrap(), sources).unwrap()))
}

fn read(spec: &Spec, rel: &str, source: &str) -> (Facts, Extraction) {
    let tree = Lang::Python.parse(source.as_bytes()).unwrap();
    let file = File { lang: Lang::Python, rel, text: source, src: source, tree: Some(&tree) };
    let facts = facts(spec, compiled(DECL), Some(compiled(REFS)), &file);
    let (_dir, resolver) = repo(&["**/*.py"], &[(rel, source)]);
    let mut ex = Extraction::default();
    write(&file, &facts, &mut ex);
    resolve(&resolver, spec, &file, &facts, &mut ex);
    (facts, ex)
}

#[test]
fn a_member_is_qualified_by_its_container_and_declared_by_it() {
    let (facts, ex) = read(&spec(STRIPPED, Export::Prefix('_')), "a.py", SOURCE);
    let ids: Vec<&str> = facts.defs.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, ["sym:a.py::Shop", "sym:a.py::Shop.pay", "sym:a.py::charge", "sym:a.py::_hidden", "sym:a.py::_kept", "sym:a.py::run"]);
    assert_eq!(lines(&ex, EdgeKind::Declares), [
        "file:a.py -> sym:a.py::Shop [export]",
        "file:a.py -> sym:a.py::_hidden []",
        "file:a.py -> sym:a.py::_kept [export]",
        "file:a.py -> sym:a.py::charge [export]",
        "file:a.py -> sym:a.py::run [export]",
        "sym:a.py::Shop -> sym:a.py::Shop.pay [export]",
    ]);
}

#[test]
fn a_repeated_id_keeps_its_first_declaration() {
    let (facts, _) = read(&spec(STRIPPED, Export::All), "a.py", SOURCE);
    let charge = facts.defs.iter().find(|d| d.qualified == "charge").unwrap();
    assert_eq!((charge.line, charge.body.as_str()), (11, "def charge(x):"));
}

#[test]
fn a_body_is_the_comments_above_and_the_first_line() {
    let (facts, _) = read(&spec(STRIPPED, Export::All), "a.py", SOURCE);
    assert_eq!(facts.defs[0].body, "Pays the bill.\nTwice.\nclass Shop:");
    let raw = Body { form: Form::Raw, ..STRIPPED };
    let (facts, _) = read(&spec(raw, Export::All), "a.py", SOURCE);
    assert_eq!(facts.defs[0].body, "# Pays the bill.\n# Twice.\nclass Shop:");
    let none = Body { doc: Doc::None, ..STRIPPED };
    let (facts, _) = read(&spec(none, Export::All), "a.py", SOURCE);
    assert_eq!(facts.defs[0].body, "class Shop:");
}

#[test]
fn a_reference_resolves_by_name_skipping_receivers_locals_and_itself() {
    let (_, ex) = read(&spec(STRIPPED, Export::All), "a.py", SOURCE);
    // `self.log()` has a receiver, `helper` is declared nowhere visible, `charge(x)` is the
    // definition calling itself, and `run`'s `charge()` calls its parameter.
    assert_eq!(lines(&ex, EdgeKind::Calls), [
        "sym:a.py::Shop.pay -> sym:a.py::charge []",
        "sym:a.py::charge -> sym:a.py::_hidden []",
    ]);
}

#[test]
fn a_comment_cites_from_its_owner() {
    let (_, ex) = read(&spec(STRIPPED, Export::All), "a.py", SOURCE);
    assert_eq!(lines(&ex, EdgeKind::References), ["sym:a.py::charge -> ADR-24 [comment]"]);
}

#[test]
fn an_import_site_keeps_its_path_and_names() {
    let (facts, _) = read(&spec(STRIPPED, Export::All), "a.py", SOURCE);
    let import = &facts.imports[0];
    assert_eq!((import.path.as_deref(), import.names.as_slice(), import.reexport), (Some(".util"), &["helper".to_string()][..], false));
}

#[test]
fn an_import_path_resolves_beside_the_importer_then_at_the_root_and_only_to_a_globbed_file() {
    let (_dir, resolver) = repo(&["**/*.py"], &[("pkg/a.py", ""), ("pkg/util.py", ""), ("lib/x.py", ""), ("pkg/notes.txt", "")]);
    let import = |path: &str| Import { path: Some(path.to_string()), names: Vec::new(), alias: None, reexport: false, range: 0..0 };
    let mut s = spec(STRIPPED, Export::All);
    s.imports = ImportPaths { root: false, suffixes: &["", ".py"] };
    assert_eq!(import_targets(&resolver, &s, "pkg/a.py", &import("util")), ["pkg/util.py"]);
    assert!(import_targets(&resolver, &s, "pkg/a.py", &import("lib/x")).is_empty());
    assert!(import_targets(&resolver, &s, "pkg/a.py", &import("notes.txt")).is_empty());
    s.imports.root = true;
    assert_eq!(import_targets(&resolver, &s, "pkg/a.py", &import("lib/x")), ["lib/x.py"]);
}

#[test]
fn the_default_header_lists_visible_top_level_names_and_what_containers_hold() {
    let (facts, _) = read(&spec(STRIPPED, Export::Prefix('_')), "a.py", SOURCE);
    let h = header(&facts);
    assert_eq!(h.top.iter().map(String::as_str).collect::<Vec<_>>(), ["Shop", "_kept", "charge", "run"]);
    assert_eq!(h.nested.types.iter().map(String::as_str).collect::<Vec<_>>(), ["Shop"]);
    assert_eq!(h.nested.members.iter().map(String::as_str).collect::<Vec<_>>(), ["Shop.pay"]);
}

#[test]
fn a_name_the_grammar_could_not_read_declares_nothing() {
    // The grammar wraps the whole file in an ERROR, so two well-formed functions sit inside it.
    let broken = "def ok():\n    pass\n\nclass (:\n    def inner(self): pass\n";
    let (facts, _) = read(&spec(STRIPPED, Export::All), "a.py", broken);
    assert!(facts.defs.is_empty(), "{:?}", facts.defs);
    let (facts, _) = read(&spec(STRIPPED, Export::All), "a.py", &broken.replace("class (:", "class C:"));
    assert_eq!(facts.defs.iter().map(|d| d.qualified.as_str()).collect::<Vec<_>>(), ["ok", "C", "C.inner"]);
}
