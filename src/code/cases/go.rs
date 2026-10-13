//! Go's shapes, each on inline files, so the reading of a declaration, an import and a call is pinned
//! by the assertion rather than by a corpus. Go is a spec row and its two queries, with no hook.

use crate::code::prose::testing::{ids, lines, repo};
use crate::model::{EdgeKind, Extraction};

const GLOBS: &[&str] = &["**/*.go"];

const MOD: &str = "module example.com/shop\n\ngo 1.22\n";

fn extract(files: &[(&str, &str)], rel: &str) -> Extraction {
    let (_dir, resolver) = repo(GLOBS, files);
    let text = files.iter().find(|(f, _)| *f == rel).map(|(_, t)| *t).unwrap();
    crate::code::front::read(&resolver, rel, text)
}

const DECLS: &str = r#"package shop

import "fmt"

// Cart holds lines.
// It is not safe for concurrent use.
type Cart struct {
	lines, Total int
	fmt.Stringer
}

type (
	// Store keeps carts.
	Store interface {
		Get(id string) *Cart
	}
	ID   = string
	cents int
)

const Max = 10

const (
	minimum = iota
	_       = 2
)

var Default, other Cart

var (
	// Shared is the one cart.
	Shared = &Cart{}
)

var _ Store = (*memory)(nil)

// Add puts one line in.
func (c *Cart) Add(n int) { c.lines += n }

func (s *Tree[T]) Walk() {}

func New() *Cart {
	type local struct{ x int }
	var v = 1
	_ = v
	return &Cart{}
}
"#;

#[test]
fn the_file_declares_its_package_level_names_with_members_under_their_type() {
    let ex = extract(&[("go.mod", MOD), ("shop/cart.go", DECLS)], "shop/cart.go");
    assert_eq!(ids(&ex), [
        "file:shop/cart.go",
        "sym:shop/cart.go::Cart",
        "sym:shop/cart.go::Cart.Add",
        "sym:shop/cart.go::Cart.Total",
        "sym:shop/cart.go::Cart.lines",
        "sym:shop/cart.go::Default",
        "sym:shop/cart.go::ID",
        "sym:shop/cart.go::Max",
        "sym:shop/cart.go::New",
        "sym:shop/cart.go::Shared",
        "sym:shop/cart.go::Store",
        "sym:shop/cart.go::Store.Get",
        "sym:shop/cart.go::Tree.Walk",
        "sym:shop/cart.go::cents",
        "sym:shop/cart.go::minimum",
        "sym:shop/cart.go::other",
    ], "`_` declares nothing, an embedded field has no name, and a function's own type and var are locals");
    assert_eq!(lines(&ex, EdgeKind::Declares), [
        "file:shop/cart.go -> sym:shop/cart.go::Cart [export]",
        "file:shop/cart.go -> sym:shop/cart.go::Cart.Add [export]",
        "file:shop/cart.go -> sym:shop/cart.go::Default [export]",
        "file:shop/cart.go -> sym:shop/cart.go::ID [export]",
        "file:shop/cart.go -> sym:shop/cart.go::Max [export]",
        "file:shop/cart.go -> sym:shop/cart.go::New [export]",
        "file:shop/cart.go -> sym:shop/cart.go::Shared [export]",
        "file:shop/cart.go -> sym:shop/cart.go::Store [export]",
        "file:shop/cart.go -> sym:shop/cart.go::Tree.Walk [export]",
        "file:shop/cart.go -> sym:shop/cart.go::cents []",
        "file:shop/cart.go -> sym:shop/cart.go::minimum []",
        "file:shop/cart.go -> sym:shop/cart.go::other []",
        "sym:shop/cart.go::Cart -> sym:shop/cart.go::Cart.Total [export]",
        "sym:shop/cart.go::Cart -> sym:shop/cart.go::Cart.lines []",
        "sym:shop/cart.go::Store -> sym:shop/cart.go::Store.Get [export]",
    ], "a capitalised name is exported, and a method hangs off its file, wherever its type is");
    let node = |id: &str| ex.nodes.iter().find(|n| n.id == id).unwrap();
    let cart = node("sym:shop/cart.go::Cart");
    assert_eq!((cart.line, cart.end), (7, 10));
    assert_eq!(cart.body, "// Cart holds lines.\n// It is not safe for concurrent use.\ntype Cart struct {");
    assert_eq!(node("sym:shop/cart.go::Store").body, "// Store keeps carts.\nStore interface {", "a grouped spec reads its own comment");
    assert_eq!(node("sym:shop/cart.go::Shared").body, "// Shared is the one cart.\nShared = &Cart{}");
    assert_eq!(node("sym:shop/cart.go::Cart.Add").body, "// Add puts one line in.\nfunc (c *Cart) Add(n int) { c.lines += n }");
}

const ROUTER: &str = r#"package shop

import (
	"fmt"
	"example.com/shop/render"
	j "example.com/shop/codec/json"
	"example.com/other/thing"
	_ "example.com/shop/render"
)

func Serve(c *Cart) render.Render {
	fmt.Println(c)
	render.JSON(c)
	j.Marshal(c)
	thing.Do()
	return nil
}
"#;

const RENDER: &str = "package render\n\ntype Render interface{ Write() }\n\nfunc JSON(v any) {}\n\nfunc helper() {}\n";

#[test]
fn an_import_path_under_the_module_names_the_package_directory_and_its_name_binds_its_members() {
    let files = [
        ("go.mod", MOD),
        ("shop.go", ROUTER),
        ("cart.go", "package shop\n\ntype Cart struct{}\n"),
        ("render/render.go", RENDER),
        ("render/text.go", "package render\n\nfunc Text() {}\n"),
        ("render/render_test.go", "package render\n\nfunc TestJSON() {}\n"),
        ("render/inner/x.go", "package inner\n"),
        ("codec/json/std.go", "//go:build !jsoniter\n\npackage json\n\nvar Marshal = 1\n"),
        ("codec/json/iter.go", "//go:build jsoniter\n\npackage json\n\nvar Marshal = 2\n"),
    ];
    let ex = extract(&files, "shop.go");
    assert_eq!(lines(&ex, EdgeKind::Imports), [
        "file:shop.go -> file:codec/json/iter.go [*]",
        "file:shop.go -> file:codec/json/std.go [*]",
        "file:shop.go -> file:render/render.go [*]",
        "file:shop.go -> file:render/text.go [*]",
    ], "the standard library and another module name nothing, and a test file is no part of the package");
    assert_eq!(lines(&ex, EdgeKind::Calls), [
        "sym:shop.go::Serve -> sym:codec/json/iter.go::Marshal []",
        "sym:shop.go::Serve -> sym:codec/json/std.go::Marshal []",
        "sym:shop.go::Serve -> sym:render/render.go::JSON []",
    ], "each build variant's `Marshal` is reached, and `fmt` and `thing` are outside the repository");
    assert_eq!(lines(&ex, EdgeKind::References), [
        "sym:shop.go::Serve -> sym:cart.go::Cart [type]",
        "sym:shop.go::Serve -> sym:render/render.go::Render [type]",
    ]);
}

#[test]
fn a_local_named_like_an_import_hides_it_and_an_unexported_member_is_not_reached() {
    let files = [
        ("go.mod", MOD),
        ("main.go", "package main\n\nimport \"example.com/shop/render\"\n\nfunc main() {\n\trender.helper()\n\tgo func(render int) { render.JSON(1) }(1)\n}\n"),
        ("render/render.go", RENDER),
    ];
    assert!(lines(&extract(&files, "main.go"), EdgeKind::Calls).is_empty());
}

#[test]
fn a_bare_name_reaches_every_file_of_the_directory_that_declares_it() {
    let files = [
        ("go.mod", MOD),
        ("a.go", "package shop\n\nfunc run() {\n\tsetup()\n\tlater()\n\tfmtx := func() {}\n\tfmtx()\n}\n\nfunc fmtx() {}\n"),
        ("b_linux.go", "package shop\n\nfunc setup() {}\n"),
        ("b_windows.go", "//go:build windows\n\npackage shop\n\nfunc setup() {}\n\nfunc start() { setup() }\n"),
        ("sub/c.go", "package sub\n\nfunc later() {}\n"),
    ];
    assert_eq!(lines(&extract(&files, "a.go"), EdgeKind::Calls), [
        "sym:a.go::run -> sym:b_linux.go::setup []",
        "sym:a.go::run -> sym:b_windows.go::setup []",
    ], "a subdirectory is another package, and a local function value hides the package's");
    assert_eq!(lines(&extract(&files, "b_windows.go"), EdgeKind::Calls), [
        "sym:b_windows.go::start -> sym:b_windows.go::setup []",
    ], "a variant that declares the name builds with its own and never with another variant's");
}

#[test]
fn a_call_through_a_method_s_receiver_reaches_the_type_s_method_in_any_file_of_the_package() {
    let files = [
        ("go.mod", MOD),
        ("engine.go", r#"package shop

type Engine struct{ pool int }

func (e *Engine) Run() {
	e.handle()
	e.Use()
	e.missing()
	go func() { e.handle() }()
	other := &Engine{}
	other.handle()
	e := other
	e.serve()
}

func (Engine) Value() {}
"#),
        ("handle.go", "package shop\n\nfunc (engine *Engine) handle() { engine.Run() }\n\nfunc (e *Engine) serve() {}\n"),
        ("group.go", "package shop\n\ntype Group struct{}\n\nfunc (g *Group) Use() {}\n"),
    ];
    assert_eq!(lines(&extract(&files, "engine.go"), EdgeKind::Calls), [
        "sym:engine.go::Engine.Run -> sym:handle.go::Engine.handle []",
    ], "a promoted method needs the embedding read, a value's type is unread, and a rebound `e` is a value");
    assert_eq!(lines(&extract(&files, "handle.go"), EdgeKind::Calls), ["sym:handle.go::Engine.handle -> sym:engine.go::Engine.Run []"]);
}

#[test]
fn a_nested_module_reads_its_own_manifest_and_an_import_of_the_module_root_names_the_root() {
    let files = [
        ("go.mod", MOD),
        ("root.go", "package shop\n\nfunc Top() {}\n"),
        ("tools/go.mod", "// tools\nmodule \"example.com/tools\" // pinned\n"),
        ("tools/gen/gen.go", "package gen\n\nimport (\n\t\"example.com/shop\"\n\t\"example.com/tools/lib\"\n)\n\nfunc Gen() { shop.Top(); lib.Help() }\n"),
        ("tools/lib/lib.go", "package lib\n\nfunc Help() {}\n"),
    ];
    let ex = extract(&files, "tools/gen/gen.go");
    assert_eq!(lines(&ex, EdgeKind::Imports), ["file:tools/gen/gen.go -> file:tools/lib/lib.go [*]"], "another module is outside");
    assert_eq!(lines(&ex, EdgeKind::Calls), ["sym:tools/gen/gen.go::Gen -> sym:tools/lib/lib.go::Help []"]);
    let root = extract(&[("go.mod", MOD), ("cmd/main.go", "package main\n\nimport \"example.com/shop\"\n\nfunc main() { shop.Top() }\n"), ("root.go", "package shop\n\nfunc Top() {}\n")], "cmd/main.go");
    assert_eq!(lines(&root, EdgeKind::Calls), ["sym:cmd/main.go::main -> sym:root.go::Top []"]);
}

#[test]
fn a_type_parameter_or_local_type_is_no_package_type() {
    let files = [
        ("go.mod", MOD),
        ("a.go", "package shop\n\ntype T struct{}\n\ntype List[T any] struct{ items []T }\n\nfunc Map[T any](xs []T) T {\n\tvar x T\n\treturn x\n}\n\nfunc Use() T {\n\ttype T int\n\treturn nil\n}\n\nfunc Make() T { return T{} }\n"),
    ];
    assert_eq!(lines(&extract(&files, "a.go"), EdgeKind::References), [
        "sym:a.go::Make -> sym:a.go::T [type]",
        "sym:a.go::Use -> sym:a.go::T [type]",
    ], "a type parameter binds in its function or type, and a local type in the body, which `Use`'s result is outside");
}

#[test]
fn a_parameter_named_like_its_type_binds_in_the_body_and_an_interface_assertion_names_its_type() {
    let files = [
        ("go.mod", MOD),
        ("a.go", "package shop\n\nimport \"example.com/shop/render\"\n\ntype setter interface{ Set() }\n\ntype memory struct{}\n\nvar _ setter = (*memory)(nil)\n\nvar _ setter = (*render.JSON)(nil)\n\nfunc apply(setter setter) {}\n"),
        ("render/json.go", "package render\n\ntype JSON struct{}\n"),
    ];
    assert_eq!(lines(&extract(&files, "a.go"), EdgeKind::References), [
        "file:a.go -> sym:a.go::memory [type]",
        "file:a.go -> sym:a.go::setter [type]",
        "file:a.go -> sym:render/json.go::JSON [type]",
        "sym:a.go::apply -> sym:a.go::setter [type]",
    ], "the parameter's type is read where the signature stands, outside the parameter's scope");
}

#[test]
fn an_instantiated_generic_call_reaches_the_function_however_the_grammar_spells_it() {
    let files = [
        ("go.mod", MOD),
        ("a.go", "package shop\n\nimport \"example.com/shop/conv\"\n\nfunc get[T any](a, b any) T { var t T; return t }\n\nfunc use() {\n\tget[bool](1, 2)\n\tconv.To[[]int](1, 2)\n\tconv.To[int](1, 2)\n}\n"),
        ("conv/conv.go", "package conv\n\nfunc To[T any](a, b any) T { var t T; return t }\n"),
    ];
    assert_eq!(lines(&extract(&files, "a.go"), EdgeKind::Calls), [
        "sym:a.go::use -> sym:a.go::get []",
        "sym:a.go::use -> sym:conv/conv.go::To []",
    ], "`get[bool](1, 2)` parses as an index, `conv.To[[]int](1, 2)` as a call with type arguments");
}

#[test]
fn an_id_in_a_comment_or_string_is_cited_by_what_holds_it() {
    let ex = extract(&[("go.mod", MOD), ("a.go", "// ADR-004 explains the wait\npackage shop\n\nfunc wait() {\n\tprintln(\"FR-APP-54\")\n}\n")], "a.go");
    assert_eq!(lines(&ex, EdgeKind::References), ["file:a.go -> ADR-004 [comment]", "sym:a.go::wait -> FR-APP-54 [string]"]);
}

#[test]
fn a_go_file_is_read_by_its_extension() {
    use crate::code::lang::{Family, Lang};
    assert_eq!(Lang::of("cmd/main.go"), Some(Lang::Go));
    assert_eq!(Lang::Go.family(), Family::Go);
}

#[test]
fn a_generic_receiver_s_type_parameter_is_no_package_type() {
    let files = [
        ("go.mod", MOD),
        ("a.go", "package shop\n\ntype T struct{}\n\ntype Box[T any] struct{ v T }\n\nfunc (b *Box[T]) Get() T { var z T; return z }\n\nfunc (b Box[T]) Put(v T) {}\n"),
    ];
    assert_eq!(lines(&extract(&files, "a.go"), EdgeKind::References), [
        "sym:a.go::Box.Get -> sym:a.go::Box [type]",
        "sym:a.go::Box.Put -> sym:a.go::Box [type]",
    ]);
}

#[test]
fn an_import_without_a_name_is_named_by_the_package_most_of_its_files_declare_and_a_raw_path_is_read() {
    let files = [
        ("go.mod", MOD),
        ("a.go", "package shop\n\nimport (\n\t\"example.com/shop/render/v2\"\n\tr `example.com/shop/render`\n)\n\nfunc f() { render.JSON(1); r.JSON(2) }\n"),
        ("render/v2/a_gen.go", "//go:build ignore\n\npackage main\n\nfunc main() {}\n"),
        ("render/v2/render.go", RENDER),
        ("render/v2/text.go", "package render\n\nfunc Text() {}\n"),
        ("render/render.go", RENDER),
    ];
    assert_eq!(lines(&extract(&files, "a.go"), EdgeKind::Calls), [
        "sym:a.go::f -> sym:render/render.go::JSON []",
        "sym:a.go::f -> sym:render/v2/render.go::JSON []",
    ]);
}
