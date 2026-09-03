//! Edge cases of the code extractor, each on an inline source so the grammar's shape for the
//! construct is pinned by the assertion rather than by a shared fixture.

use super::CodeExtractor;
use crate::code::imports::Resolver;
use crate::ids::IdMatcher;
use crate::model::{EdgeKind, Extraction, Extractor, NodeKind};

struct Repo {
    dir: tempfile::TempDir,
}

impl Repo {
    fn new(files: &[(&str, &str)]) -> Repo {
        let dir = tempfile::tempdir().unwrap();
        for (p, c) in files {
            let full = dir.path().join(p);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, c).unwrap();
        }
        Repo { dir }
    }

    fn extract(&self, rel: &str, src: &str) -> Extraction {
        let cfg = crate::config::Config::default();
        let ids = IdMatcher::new(&cfg.id_families, &cfg.milestone_families);
        CodeExtractor::new(Resolver::new(self.dir.path()).unwrap(), ids).extract(rel, src)
    }
}

fn extract(rel: &str, src: &str) -> Extraction {
    Repo::new(&[]).extract(rel, src)
}

fn ids(ex: &Extraction) -> Vec<&str> {
    ex.nodes.iter().map(|n| n.id.as_str()).collect()
}

fn edges(ex: &Extraction, kind: EdgeKind) -> Vec<(&str, &str, &str)> {
    ex.edges.iter().filter(|e| e.kind == kind).map(|e| (e.source.as_str(), e.target.as_str(), e.context.as_str())).collect()
}

fn declares(ex: &Extraction, name: &str) -> Option<String> {
    ex.edges.iter().find(|e| e.kind == EdgeKind::Declares && e.target.ends_with(&format!("::{name}"))).map(|e| e.context.clone())
}

// ---- exports and declarations ----

#[test]
fn export_default_class_and_function_are_declared_as_exports() {
    let ex = extract("a.ts", "export default class Foo {\n  run() {}\n}\n");
    assert_eq!(declares(&ex, "Foo").as_deref(), Some("export"));
    assert!(ids(&ex).contains(&"sym:a.ts::Foo.run"));
    let ex = extract("b.ts", "export default function bar() { return 1; }\n");
    assert_eq!(declares(&ex, "bar").as_deref(), Some("export"));
}

#[test]
fn anonymous_default_exports_declare_nothing() {
    let ex = extract("a.ts", "export default class {}\nexport default () => 1;\n");
    assert!(ex.nodes.iter().all(|n| n.kind == NodeKind::File), "{:?}", ids(&ex));
}

#[test]
fn local_export_clause_marks_the_declarations_exported() {
    let ex = extract("a.ts", "const a = 1;\nfunction b() {}\nclass C {}\nexport { a, b as bee, C };\n");
    for s in ["a", "b", "C"] {
        assert_eq!(declares(&ex, s).as_deref(), Some("export"), "{s}");
    }
}

#[test]
fn several_declarators_in_one_statement_each_become_a_symbol() {
    let ex = extract("a.ts", "export const a = 1, b = 2;\nlet c = 3, d = 4;\n");
    for s in ["a", "b", "c", "d"] {
        assert!(ids(&ex).contains(&format!("sym:a.ts::{s}").as_str()), "{s}");
    }
    assert_eq!(declares(&ex, "b").as_deref(), Some("export"));
    assert_eq!(declares(&ex, "c").as_deref(), Some(""));
}

#[test]
fn destructured_declarations_name_each_binding() {
    let ex = extract("a.ts", "export const { x, y: why } = obj;\nconst [first, , third] = arr;\n");
    for s in ["x", "why", "first", "third"] {
        assert!(ids(&ex).contains(&format!("sym:a.ts::{s}").as_str()), "{s} in {:?}", ids(&ex));
    }
    assert!(!ids(&ex).iter().any(|i| i.contains('{') || i.contains('[')), "{:?}", ids(&ex));
}

#[test]
fn namespaces_enums_and_ambient_declarations_are_symbols() {
    let src = "export namespace Ns { export const inner = 1; }\nexport enum Color { Red }\nexport const enum Flag { On }\nexport declare const VERSION: string;\ndeclare function ambient(): void;\nexport declare class Amb {}\n";
    let ex = extract("a.ts", src);
    for s in ["Ns", "Color", "Flag", "VERSION", "ambient", "Amb"] {
        assert!(ids(&ex).contains(&format!("sym:a.ts::{s}").as_str()), "{s} in {:?}", ids(&ex));
    }
    assert_eq!(declares(&ex, "VERSION").as_deref(), Some("export"));
    assert_eq!(declares(&ex, "ambient").as_deref(), Some(""));
}

#[test]
fn function_overloads_collapse_into_one_symbol() {
    let ex = extract("a.ts", "export function f(a: string): string;\nexport function f(a: number): number;\nexport function f(a: any) { return a; }\n");
    assert_eq!(ex.nodes.iter().filter(|n| n.id == "sym:a.ts::f").count(), 1);
    assert_eq!(edges(&ex, EdgeKind::Declares), vec![("file:a.ts", "sym:a.ts::f", "export")]);
}

#[test]
fn type_only_and_satisfies_declarations_are_symbols() {
    let src = "export type Id = string;\nexport interface Shape { a: number }\nexport const cfg = { a: 1 } satisfies Shape;\nexport abstract class Base { abstract go(): void; }\n";
    let ex = extract("a.ts", src);
    for s in ["Id", "Shape", "cfg", "Base", "Base.go"] {
        assert!(ids(&ex).contains(&format!("sym:a.ts::{s}").as_str()), "{s} in {:?}", ids(&ex));
    }
}

#[test]
fn a_symbol_body_is_its_signature_line_except_for_functions() {
    let ex = extract("a.ts", "export class Big {\n  a = 1;\n  b = 2;\n}\nexport function f(\n  a: number,\n) {\n  return a;\n}\nexport const arrow = (x: number) =>\n  x + 1;\n");
    let body = |id: &str| ex.nodes.iter().find(|n| n.id == id).unwrap().body.clone();
    assert_eq!(body("sym:a.ts::Big"), "export class Big {");
    assert_eq!(body("sym:a.ts::f"), "export function f( a: number, ) { return a; }");
    assert_eq!(body("sym:a.ts::arrow"), "export const arrow = (x: number) =>");
}

#[test]
fn symbol_lines_point_at_the_declaration_not_its_decorator() {
    let ex = extract("a.ts", "\n@Injectable()\nexport class Svc {}\n\n@Injectable()\nclass Local {}\n");
    let line = |id: &str| ex.nodes.iter().find(|n| n.id == id).unwrap().line;
    assert_eq!(line("sym:a.ts::Svc"), 3);
    assert_eq!(line("sym:a.ts::Local"), 6);
}

// ---- class members ----

#[test]
fn every_member_kind_is_a_symbol_named_after_its_class() {
    let src = "export class A {\n  static count = 0;\n  private secret = 1;\n  readonly #hidden = 2;\n  constructor(private readonly dep: Dep) {}\n  get value() { return 1; }\n  set value(v: number) {}\n  async run() {}\n  static make() { return new A(); }\n  ['computed']() {}\n  'quoted-name'() {}\n  [Symbol.iterator]() {}\n  declare field: string;\n  optional?: number;\n}\n";
    let ex = extract("a.ts", src);
    let got = ids(&ex);
    for s in ["A.count", "A.secret", "A.#hidden", "A.constructor", "A.value", "A.run", "A.make", "A.computed", "A.quoted-name", "A.field", "A.optional"] {
        assert!(got.contains(&format!("sym:a.ts::{s}").as_str()), "{s} in {got:?}");
    }
    assert!(!got.iter().any(|i| i.contains('\'') || i.contains("[Symbol")), "{got:?}");
    assert_eq!(ex.nodes.iter().filter(|n| n.id == "sym:a.ts::A.value").count(), 1, "getter and setter share one symbol");
}

#[test]
fn class_expressions_and_nested_classes_are_not_scanned_for_members() {
    let ex = extract("a.ts", "export const A = class { run() {} };\nfunction f() { class Inner { go() {} } }\n");
    assert!(ids(&ex).contains(&"sym:a.ts::A"));
    assert!(!ids(&ex).iter().any(|i| i.contains("run") || i.contains("Inner")), "{:?}", ids(&ex));
}

#[test]
fn implements_is_not_extends() {
    let ex = extract("a.ts", "interface I {}\nexport class A implements I {}\nexport class B extends Object implements I {}\n");
    assert_eq!(edges(&ex, EdgeKind::Extends), vec![("sym:a.ts::B", "sym:a.ts::Object", "")]);
}

#[test]
fn extends_drops_type_arguments_and_follows_the_import() {
    let repo = Repo::new(&[("base.ts", "export class Base<T> {}\n")]);
    let ex = repo.extract("a.ts", "import { Base } from './base';\nexport class A extends Base<string> {}\nclass L {}\nclass M extends L {}\n");
    let ext = edges(&ex, EdgeKind::Extends);
    assert!(ext.contains(&("sym:a.ts::A", "sym:base.ts::Base", "")), "{ext:?}");
    assert!(ext.contains(&("sym:a.ts::M", "sym:a.ts::L", "")), "{ext:?}");
}

#[test]
fn extends_of_a_call_or_member_expression_keeps_the_expression_text() {
    let ex = extract("a.ts", "class A extends mixin(Base) {}\nclass B extends Outer.Inner {}\n");
    let ext = edges(&ex, EdgeKind::Extends);
    assert!(ext.contains(&("sym:a.ts::A", "sym:a.ts::mixin(Base)", "")), "{ext:?}");
    assert!(ext.contains(&("sym:a.ts::B", "sym:a.ts::Outer.Inner", "")), "{ext:?}");
}

// ---- decorators ----

#[test]
fn decorator_arguments_of_every_shape() {
    let src = "@Controller({ path: 'x' })\n@Ns.deco('ns')\n@plain\nexport class A {\n  @Tpl(`tpl`)\n  @Roles('first', 'second')\n  @Sym(Actions.X)\n  @Mixed(42, 'late')\n  run() {}\n}\n";
    let ex = extract("a.ts", src);
    let d = edges(&ex, EdgeKind::DecoratedBy);
    for want in [
        ("sym:a.ts::A", "deco:Controller", ""),
        ("sym:a.ts::A", "deco:Ns.deco", "ns"),
        ("sym:a.ts::A", "deco:plain", ""),
        ("sym:a.ts::A.run", "deco:Tpl", "tpl"),
        ("sym:a.ts::A.run", "deco:Roles", "first"),
        ("sym:a.ts::A.run", "deco:Sym", ""),
        ("sym:a.ts::A.run", "deco:Mixed", "late"),
    ] {
        assert!(d.contains(&want), "{want:?} in {d:?}");
    }
}

#[test]
fn a_decorator_queued_before_a_non_member_does_not_leak_onto_the_next_method() {
    let src = "class A {\n  @Leak()\n  [key: string]: unknown;\n  run() {}\n  @Own()\n  go() {}\n}\n";
    let ex = extract("a.ts", src);
    let d = edges(&ex, EdgeKind::DecoratedBy);
    assert!(!d.iter().any(|e| e.0 == "sym:a.ts::A.run"), "{d:?}");
    assert!(d.contains(&("sym:a.ts::A.go", "deco:Own", "")), "{d:?}");
}

#[test]
fn decorator_nodes_are_shared_across_files_by_name() {
    let ex = extract("a.ts", "@Injectable()\nexport class A {}\n@Injectable()\nexport class B {}\n");
    assert_eq!(ex.nodes.iter().filter(|n| n.id == "deco:Injectable").count(), 1);
    assert!(ex.nodes.iter().filter(|n| n.id == "deco:Injectable").all(|n| n.kind == NodeKind::Symbol && n.label == "Injectable"));
}

// ---- imports and re-exports ----

#[test]
fn import_forms_name_what_they_bring_in() {
    let repo = Repo::new(&[("lib.ts", "export {};\n"), ("side.ts", "export {};\n"), ("legacy.ts", "export {};\n")]);
    let src = "import def, { a, b as bee } from './lib';\nimport * as ns from './lib';\nimport type { T } from './lib';\nimport { type U, v } from './lib';\nimport './side';\nimport legacy = require('./legacy');\n";
    let ex = repo.extract("a.ts", src);
    let imp = edges(&ex, EdgeKind::Imports);
    let ctx: Vec<&str> = imp.iter().filter(|e| e.1 == "file:lib.ts").map(|e| e.2).collect();
    assert_eq!(ctx, vec!["*", "T", "U,v", "def,a,b"], "{imp:?}");
    assert!(imp.contains(&("file:a.ts", "file:side.ts", "")), "{imp:?}");
    assert!(imp.contains(&("file:a.ts", "file:legacy.ts", "legacy")), "{imp:?}");
}

#[test]
fn re_export_forms() {
    let repo = Repo::new(&[("lib.ts", "export {};\n")]);
    let src = "export * from './lib';\nexport * as ns from './lib';\nexport { a, b as bee } from './lib';\nexport type { T } from './lib';\n";
    let ex = repo.extract("a.ts", src);
    let mut re: Vec<&str> = edges(&ex, EdgeKind::ReExports).iter().map(|e| e.2).collect();
    re.sort();
    assert_eq!(re, vec!["*", "T", "a,b", "ns"]);
}

#[test]
fn unresolvable_imports_leave_no_edge() {
    let ex = extract("a.ts", "import x from 'react';\nimport y from './missing';\nimport z from 'node:fs';\nexport * from '@scope/pkg';\n");
    assert!(edges(&ex, EdgeKind::Imports).is_empty());
    assert!(edges(&ex, EdgeKind::ReExports).is_empty());
}

#[test]
fn dynamic_imports_are_edges_from_the_importing_symbol() {
    let repo = Repo::new(&[("lazy.ts", "export {};\n")]);
    let ex = repo.extract("a.ts", "export async function load() { return import('./lazy'); }\nconst m = require('./lazy');\n");
    let imp = edges(&ex, EdgeKind::Imports);
    assert!(imp.contains(&("sym:a.ts::load", "file:lazy.ts", "")), "{imp:?}");
    assert!(imp.contains(&("sym:a.ts::m", "file:lazy.ts", "")), "{imp:?}");
}

// ---- parsing ----

#[test]
fn tsx_parses_jsx_and_ts_does_not_need_to() {
    let ex = extract("App.tsx", "export function App() { return <div className=\"x\">hi</div>; }\nexport const Item = () => <li />;\n");
    assert!(ids(&ex).contains(&"sym:App.tsx::App"));
    assert!(ids(&ex).contains(&"sym:App.tsx::Item"));
    let ex = extract("g.ts", "export const id = <T,>(x: T): T => x;\nexport function g<T extends object>(x: T) { return x; }\n");
    assert!(ids(&ex).contains(&"sym:g.ts::id"));
    assert!(ids(&ex).contains(&"sym:g.ts::g"));
}

#[test]
fn a_syntax_error_keeps_the_declarations_around_it() {
    let ex = extract("a.ts", "export const before = 1;\nexport function broken( {\nexport const after = 2;\nexport class After {}\n");
    let got = ids(&ex);
    assert!(got.contains(&"sym:a.ts::before"), "{got:?}");
    assert!(got.contains(&"sym:a.ts::after") || got.contains(&"sym:a.ts::After"), "{got:?}");
}

#[test]
fn an_empty_or_comment_only_file_is_just_a_file_node() {
    for src in ["", "// nothing\n", "/* FR-PAY-03 */\n"] {
        let ex = extract("a.ts", src);
        assert_eq!(ex.nodes.len(), 1, "{src:?}");
        assert_eq!(ex.nodes[0].id, "file:a.ts");
        assert_eq!(ex.nodes[0].kind, NodeKind::File);
    }
}

#[test]
fn cyrillic_identifiers_and_strings_do_not_break_offsets() {
    let src = "export const цена = 'Сумма FR-PAY-22';\n// после INV-03\nexport function после() { return `см. FR-SEC-21 и ещё`; }\n";
    let ex = extract("a.ts", src);
    assert!(ids(&ex).contains(&"sym:a.ts::цена"), "{:?}", ids(&ex));
    assert!(ids(&ex).contains(&"sym:a.ts::после"));
    let refs = edges(&ex, EdgeKind::References);
    assert!(refs.contains(&("sym:a.ts::после", "FR-SEC-21", "string")), "{refs:?}");
    assert!(refs.contains(&("file:a.ts", "INV-03", "comment")), "{refs:?}");
}

// ---- id references ----

#[test]
fn ids_in_an_arrow_const_attach_to_the_const() {
    let ex = extract("a.ts", "export const handler = () => {\n  // FR-PAY-03\n  return 'INV-11';\n};\nfunction outer() { const inner = () => 'FR-SEC-21'; return inner; }\n");
    let refs = edges(&ex, EdgeKind::References);
    assert!(refs.contains(&("sym:a.ts::handler", "FR-PAY-03", "comment")), "{refs:?}");
    assert!(refs.contains(&("sym:a.ts::handler", "INV-11", "string")), "{refs:?}");
    assert!(refs.contains(&("sym:a.ts::outer", "FR-SEC-21", "string")), "a nested const is not a symbol: {refs:?}");
}

#[test]
fn ids_in_a_nested_function_inside_a_method_attach_to_the_method() {
    let ex = extract("a.ts", "class A {\n  run() {\n    const f = () => { /* FR-PAY-03 */ };\n    function g() { return 'INV-11'; }\n  }\n}\n");
    let refs = edges(&ex, EdgeKind::References);
    assert!(refs.contains(&("sym:a.ts::A.run", "FR-PAY-03", "comment")), "{refs:?}");
    assert!(refs.contains(&("sym:a.ts::A.run", "INV-11", "string")), "{refs:?}");
}

#[test]
fn ranges_and_slash_lists_in_comments_expand() {
    let ex = extract("a.ts", "// covers FR-RPT-42…44 and INV-11/12\n");
    let mut targets: Vec<&str> = edges(&ex, EdgeKind::References).iter().map(|e| e.1).collect();
    targets.sort();
    assert_eq!(targets, vec!["FR-RPT-42", "FR-RPT-43", "FR-RPT-44", "INV-11", "INV-12"]);
}

#[test]
fn the_same_id_twice_in_one_owner_is_one_edge() {
    let ex = extract("a.ts", "// FR-PAY-03 and again FR-PAY-03\nexport function f() { return 'FR-PAY-03'; }\n");
    let refs = edges(&ex, EdgeKind::References);
    assert_eq!(refs.iter().filter(|e| e.0 == "file:a.ts" && e.1 == "FR-PAY-03").count(), 1, "{refs:?}");
    assert_eq!(refs.iter().filter(|e| e.0 == "sym:a.ts::f").count(), 1, "{refs:?}");
}

#[test]
fn ids_in_regex_literals_jsx_text_and_identifiers_are_not_references() {
    let ex = extract("a.tsx", "const re = /FR-PAY-03/;\nconst FR_PAY_03 = 1;\nexport const V = () => <p>FR-SEC-21</p>;\n");
    assert!(edges(&ex, EdgeKind::References).is_empty(), "{:?}", edges(&ex, EdgeKind::References));
}

#[test]
fn ids_in_a_decorator_argument_attach_to_the_decorated_member() {
    let ex = extract("a.ts", "class A {\n  @RequireAction('FR-VIS-35')\n  run() {}\n}\n");
    let refs = edges(&ex, EdgeKind::References);
    assert!(refs.contains(&("sym:a.ts::A.run", "FR-VIS-35", "string")), "{refs:?}");
}

#[test]
fn ids_in_test_titles_and_describe_blocks_attach_to_the_file() {
    let src = "describe('FR-VIS-35 templates', () => {\n  it('INV-11 holds', () => {});\n  test.each([['FR-PAY-03']])('%s', () => {});\n});\n";
    let ex = extract("t.spec.ts", src);
    let mut refs = edges(&ex, EdgeKind::References);
    refs.sort();
    assert_eq!(refs, vec![("file:t.spec.ts", "FR-PAY-03", "string"), ("file:t.spec.ts", "FR-VIS-35", "string"), ("file:t.spec.ts", "INV-11", "string")]);
}

#[test]
fn ids_in_interfaces_enums_and_types_attach_to_that_symbol() {
    let src = "export interface I {\n  /** FR-PAY-03 */\n  a: number;\n}\nenum E { A = 'INV-11' }\ntype T = { k: 'FR-SEC-21' };\nexport namespace N { export const q = 'FR-VIS-35'; }\n";
    let ex = extract("a.ts", src);
    let refs = edges(&ex, EdgeKind::References);
    assert!(refs.contains(&("sym:a.ts::I", "FR-PAY-03", "comment")), "{refs:?}");
    assert!(refs.contains(&("sym:a.ts::E", "INV-11", "string")), "{refs:?}");
    assert!(refs.contains(&("sym:a.ts::T", "FR-SEC-21", "string")), "{refs:?}");
    assert!(refs.contains(&("sym:a.ts::N", "FR-VIS-35", "string")), "{refs:?}");
}

#[test]
fn ids_in_a_class_property_initialiser_attach_to_the_property() {
    let ex = extract("a.ts", "class A {\n  handler = () => 'FR-PAY-03';\n  static {\n    console.log('INV-11');\n  }\n}\n");
    let refs = edges(&ex, EdgeKind::References);
    assert!(refs.contains(&("sym:a.ts::A.handler", "FR-PAY-03", "string")), "{refs:?}");
    assert!(refs.contains(&("sym:a.ts::A", "INV-11", "string")), "{refs:?}");
}

#[test]
fn ids_in_jsx_attributes_are_strings_of_the_component() {
    let ex = extract("a.tsx", "export const View = () => <Route path=\"x\" title=\"FR-PAY-03\" />;\n");
    let refs = edges(&ex, EdgeKind::References);
    assert_eq!(refs, vec![("sym:a.tsx::View", "FR-PAY-03", "string")]);
}

#[test]
fn ambiguous_families_are_not_references() {
    let ex = extract("a.ts", "// step B1, table C11, size S3, item I-015\n");
    assert!(edges(&ex, EdgeKind::References).is_empty(), "{:?}", edges(&ex, EdgeKind::References));
}


// ---- the import resolver ----

fn resolver(files: &[(&str, &str)]) -> (Repo, Resolver) {
    let repo = Repo::new(files);
    let r = Resolver::new(repo.dir.path()).unwrap();
    (repo, r)
}

#[test]
fn package_exports_with_a_wildcard_subpath() {
    let (_repo, r) = resolver(&[
        ("packages/db/package.json", r#"{ "name": "@x/db", "exports": { ".": "./dist/index.js", "./*": { "types": "./dist/*.d.ts", "default": "./dist/*.js" } } }"#),
        ("packages/db/src/index.ts", ""),
        ("packages/db/src/seed/permissions.ts", ""),
        ("packages/ui/package.json", r#"{ "name": "@x/ui", "exports": { "./components/*": "./dist/components/ui/*.js" } }"#),
        ("packages/ui/src/components/ui/button.tsx", ""),
    ]);
    assert_eq!(r.resolve("apps/a.ts", "@x/db/seed/permissions").as_deref(), Some("packages/db/src/seed/permissions.ts"));
    assert_eq!(r.resolve("apps/a.ts", "@x/db").as_deref(), Some("packages/db/src/index.ts"));
    assert_eq!(r.resolve("apps/a.ts", "@x/ui/components/button").as_deref(), Some("packages/ui/src/components/ui/button.tsx"));
    assert_eq!(r.resolve("apps/a.ts", "@x/db/missing"), None);
}

#[test]
fn an_exact_export_wins_over_the_wildcard() {
    let (_repo, r) = resolver(&[
        ("packages/d/package.json", r#"{ "name": "@x/d", "exports": { "./time": "./dist/time/index.js", "./*": "./dist/*.js" } }"#),
        ("packages/d/src/time/index.ts", ""),
        ("packages/d/src/time.ts", ""),
    ]);
    assert_eq!(r.resolve("apps/a.ts", "@x/d/time").as_deref(), Some("packages/d/src/time/index.ts"));
}

#[test]
fn a_package_name_that_is_a_prefix_of_another_does_not_capture_it() {
    let (_repo, r) = resolver(&[
        ("packages/ui/package.json", r#"{ "name": "@x/ui", "exports": { ".": "./src/index.ts", "./*": "./src/*.ts" } }"#),
        ("packages/ui/src/index.ts", ""),
        ("packages/ui/src/kit.ts", ""),
        ("packages/ui-kit/package.json", r#"{ "name": "@x/ui-kit", "exports": { ".": "./src/index.ts" } }"#),
        ("packages/ui-kit/src/index.ts", ""),
    ]);
    assert_eq!(r.resolve("apps/a.ts", "@x/ui-kit").as_deref(), Some("packages/ui-kit/src/index.ts"));
    assert_eq!(r.resolve("apps/a.ts", "@x/ui/kit").as_deref(), Some("packages/ui/src/kit.ts"));
}

#[test]
fn jsonc_tsconfig_with_comments_urls_and_trailing_commas() {
    let (_repo, r) = resolver(&[
        ("tsconfig.json", "{\n  // comment\n  \"$schema\": \"https://json.schemastore.org/tsconfig\",\n  /* block\n  comment */\n  \"compilerOptions\": {\n    \"paths\": {\n      \"@/*\": [\"./src/*\",],\n    },\n  },\n}\n"),
        ("src/a.ts", ""),
    ]);
    assert_eq!(r.resolve("src/b.ts", "@/a").as_deref(), Some("src/a.ts"));
}

#[test]
fn tsconfig_paths_honour_base_url() {
    let (_repo, r) = resolver(&[
        ("tsconfig.json", r#"{ "compilerOptions": { "baseUrl": "./src", "paths": { "~/*": ["lib/*"] } } }"#),
        ("src/lib/util.ts", ""),
    ]);
    assert_eq!(r.resolve("src/app.ts", "~/util").as_deref(), Some("src/lib/util.ts"));
}

#[test]
fn a_file_beats_a_directory_index_of_the_same_name() {
    let (_repo, r) = resolver(&[("src/foo.ts", ""), ("src/foo/index.ts", ""), ("src/bar/index.tsx", "")]);
    assert_eq!(r.resolve("src/a.ts", "./foo").as_deref(), Some("src/foo.ts"));
    assert_eq!(r.resolve("src/a.ts", "./foo/index").as_deref(), Some("src/foo/index.ts"));
    assert_eq!(r.resolve("src/a.ts", "./bar").as_deref(), Some("src/bar/index.tsx"));
}

#[test]
fn non_source_specs_and_paths_outside_the_repo_are_none() {
    let (_repo, r) = resolver(&[("src/a.ts", ""), ("src/data.json", ""), ("src/s.css", ""), ("src/dist/x.ts", "")]);
    assert_eq!(r.resolve("src/a.ts", "./data.json"), None);
    assert_eq!(r.resolve("src/a.ts", "./s.css"), None);
    assert_eq!(r.resolve("src/a.ts", "../../../../etc/passwd"), None);
    assert_eq!(r.resolve("src/a.ts", "./dist/x"), None, "build output is not indexed");
    assert_eq!(r.resolve("src/a.ts", "./a.ts").as_deref(), Some("src/a.ts"), "an explicit extension still resolves");
}

#[test]
fn a_package_inside_node_modules_is_not_a_workspace_package() {
    let (_repo, r) = resolver(&[
        ("node_modules/zod/package.json", r#"{ "name": "zod", "exports": { ".": "./index.ts" } }"#),
        ("node_modules/zod/index.ts", ""),
        ("src/a.ts", ""),
    ]);
    assert_eq!(r.resolve("src/a.ts", "zod"), None);
}

#[test]
fn a_package_without_exports_falls_back_to_main_and_types() {
    let (_repo, r) = resolver(&[
        ("packages/m/package.json", r#"{ "name": "@x/m", "main": "./dist/index.js" }"#),
        ("packages/m/src/index.ts", ""),
        ("packages/t/package.json", r#"{ "name": "@x/t", "types": "./src/index.ts" }"#),
        ("packages/t/src/index.ts", ""),
    ]);
    assert_eq!(r.resolve("apps/a.ts", "@x/m").as_deref(), Some("packages/m/src/index.ts"));
    assert_eq!(r.resolve("apps/a.ts", "@x/t").as_deref(), Some("packages/t/src/index.ts"));
}

#[test]
fn a_malformed_package_json_or_tsconfig_does_not_abort_the_walk() {
    let (_repo, r) = resolver(&[
        ("packages/bad/package.json", "{ not json"),
        ("packages/bad/tsconfig.json", "{ also not json"),
        ("packages/ok/package.json", r#"{ "name": "@x/ok", "exports": { ".": "./src/index.ts" } }"#),
        ("packages/ok/src/index.ts", ""),
    ]);
    assert_eq!(r.resolve("apps/a.ts", "@x/ok").as_deref(), Some("packages/ok/src/index.ts"));
}

/// A development aid, not a test: prints the tree-sitter S-expression of `REPOGRAPH_DUMP`
/// so a new case can be written against the grammar's real shape.
/// `REPOGRAPH_DUMP='export * as ns from "./lib";' cargo test dump_tree -- --ignored --nocapture`
#[test]
#[ignore]
fn dump_tree() {
    let src = std::env::var("REPOGRAPH_DUMP").expect("set REPOGRAPH_DUMP to the source to parse");
    let rel = if std::env::var_os("REPOGRAPH_DUMP_TSX").is_some() { "dump.tsx" } else { "dump.ts" };
    let tree = crate::code::symbols::parse(rel, src.as_bytes()).unwrap();
    println!("{}", tree.root_node().to_sexp());
}

// ---- spans ----

fn node<'a>(ex: &'a Extraction, id: &str) -> &'a crate::model::Node {
    ex.nodes.iter().find(|n| n.id == id).unwrap_or_else(|| panic!("{id} not extracted"))
}

#[test]
fn a_function_declaration_spans_its_body() {
    let ex = extract("a.ts", "export function f() {\n  return 1;\n}\nconst x = 1;\n");
    let f = node(&ex, "sym:a.ts::f");
    assert_eq!((f.line, f.end), (1, 3));
    let x = node(&ex, "sym:a.ts::x");
    assert_eq!((x.line, x.end), (4, 4));
}

#[test]
fn a_class_and_each_member_carry_their_own_span() {
    let ex = extract("a.ts", "export class A {\n  one() {\n    return 1;\n  }\n\n  two() {}\n}\n");
    assert_eq!((node(&ex, "sym:a.ts::A").line, node(&ex, "sym:a.ts::A").end), (1, 7));
    assert_eq!((node(&ex, "sym:a.ts::A.one").line, node(&ex, "sym:a.ts::A.one").end), (2, 4));
    assert_eq!((node(&ex, "sym:a.ts::A.two").line, node(&ex, "sym:a.ts::A.two").end), (6, 6));
}

#[test]
fn a_decorated_export_class_still_starts_at_its_name_and_ends_at_its_brace() {
    let ex = extract("a.ts", "@Injectable()\nexport class S {\n  run() {}\n}\n");
    let s = node(&ex, "sym:a.ts::S");
    assert_eq!((s.line, s.end), (2, 4));
}

#[test]
fn a_decorator_node_has_no_span() {
    let ex = extract("a.ts", "@Injectable()\nexport class S {}\n");
    assert_eq!(node(&ex, "deco:Injectable").end, 0);
}
