//! OCaml's shapes, each on inline files, so the reading of a binding, a module path and an `open`
//! is pinned by the assertion rather than by a corpus. OCaml is a spec row and its two queries,
//! with no hook.

use crate::code::prose::testing::{ids, lines, repo};
use crate::model::{EdgeKind, Extraction};

const GLOBS: &[&str] = &["**/*.ml", "**/*.mli"];

fn extract(files: &[(&str, &str)], rel: &str) -> Extraction {
    let (_dir, resolver) = repo(GLOBS, files);
    let text = files.iter().find(|(f, _)| *f == rel).map(|(_, t)| *t).unwrap();
    crate::code::front::read(&resolver, rel, text)
}

const DECLS: &str = r#"open Import

(** Runs both. *)
let rec run x = step x and step y = run y

let a, b = 1, 2
let ( +! ) x y = x + y
let () = run 0
let apply = fun x -> x
let config = { size = 1 }
let%test probe = true

let outer x =
  let inner = x in
  inner

module M = struct
  type t = A | B of int
  type r = { fld : int; mutable other : string }
  external raw : int -> int = "c_raw"
  exception Oops of string
end

module type S = sig
  val v : int
  val f : int -> int
end

module F (X : sig val hidden : int end) = struct let w = X.hidden end

class counter = object method count = 1 end
"#;

#[test]
fn bindings_modules_types_and_classes_are_declared_and_a_local_let_is_not() {
    let ex = extract(&[("lib/decls.ml", DECLS)], "lib/decls.ml");
    assert_eq!(ids(&ex), [
        "file:lib/decls.ml",
        "sym:lib/decls.ml::F",
        "sym:lib/decls.ml::F/w",
        "sym:lib/decls.ml::M",
        "sym:lib/decls.ml::M/Oops",
        "sym:lib/decls.ml::M/r",
        "sym:lib/decls.ml::M/r.fld",
        "sym:lib/decls.ml::M/r.other",
        "sym:lib/decls.ml::M/raw",
        "sym:lib/decls.ml::M/t",
        "sym:lib/decls.ml::M/t.A",
        "sym:lib/decls.ml::M/t.B",
        "sym:lib/decls.ml::S",
        "sym:lib/decls.ml::S.f",
        "sym:lib/decls.ml::S.v",
        "sym:lib/decls.ml::a",
        "sym:lib/decls.ml::apply",
        "sym:lib/decls.ml::b",
        "sym:lib/decls.ml::config",
        "sym:lib/decls.ml::counter",
        "sym:lib/decls.ml::counter.count",
        "sym:lib/decls.ml::outer",
        "sym:lib/decls.ml::probe",
        "sym:lib/decls.ml::run",
        "sym:lib/decls.ml::step",
    ], "`let rec … and …` declares both, a tuple each name, a `let … in` and an operator nothing, and a functor parameter's signature nothing");
    assert!(lines(&ex, EdgeKind::Declares).iter().all(|l| l.ends_with("[export]")), "with no interface read, everything is exported");
    let node = |id: &str| ex.nodes.iter().find(|n| n.id == id).unwrap();
    assert_eq!(node("sym:lib/decls.ml::run").body, "(** Runs both. *)\nlet rec run x = step x and step y = run y", "the comment above `let` documents the first binding");
    assert_eq!(node("sym:lib/decls.ml::step").body, "let rec run x = step x and step y = run y", "a binding after `and` reads its row and no doc");
    let m = node("sym:lib/decls.ml::M");
    assert_eq!((m.line, m.end, m.body.as_str()), (17, 22, "module M = struct"));
    assert_eq!(lines(&ex, EdgeKind::Calls), [
        "file:lib/decls.ml -> sym:lib/decls.ml::run []",
        "sym:lib/decls.ml::run -> sym:lib/decls.ml::step []",
        "sym:lib/decls.ml::step -> sym:lib/decls.ml::run []",
    ], "`let () = run 0` declares nothing, so its call is the file's");
}

const CSET: &str = "(** Sets of characters. *)\nlet union a b = a\nlet empty = []\ntype t = int list\nmodule Inner = struct let deep = 1 end\n";

const USER: &str = r#"open Import
let a = helper
let b = R.exec 1
let c = Cset.union 1 2
let d = Core.union 1 2
let e = Cset.Inner.deep
let f (x : Cset.t) = x
module Local = struct let g = 1 end
let h = Local.g
module F (Cset : sig val union : int end) = struct let k = Cset.union end
let i = Missing.x
let j = let helper = 2 in helper
let l = Cset.(union empty empty)
let m = Util.v
let n = 1 |> Core.exec
let o = Core.exec @@ empty
"#;

fn modules() -> [(&'static str, &'static str); 9] {
    [
        ("lib/cset.ml", CSET),
        ("lib/cset.mli", "(** The union. *)\nval union : int -> int -> int\nval empty : int list\nmodule Inner : sig val deep : int end\n"),
        ("lib/core.ml", "include Cset\nlet exec x = x\n"),
        ("lib/import.ml", "module R = Core\nlet helper = 1\n"),
        ("lib/user.ml", USER),
        ("test/import.ml", "let helper = 2\n"),
        ("test/t.ml", "open Import\nlet x = helper\nlet y = Util.v\nlet z = Shape.area\n"),
        ("a/util.ml", "let v = 1\n"),
        ("b/shape.mli", "val area : int\n"),
    ]
}

#[test]
fn a_module_is_its_file_beside_the_user_first_then_the_one_file_of_its_name() {
    let mut files = modules().to_vec();
    files.push(("b/util.ml", "let v = 1\n"));
    assert_eq!(lines(&extract(&files, "lib/user.ml"), EdgeKind::Imports), [
        "file:lib/user.ml -> file:lib/core.ml [*]",
        "file:lib/user.ml -> file:lib/cset.ml [*]",
        "file:lib/user.ml -> file:lib/import.ml [*]",
    ], "every module a path names is an import, as `ocamldep` reads one; a name two files share names neither, and one no file has nothing");
    assert_eq!(lines(&extract(&files, "test/t.ml"), EdgeKind::Imports), [
        "file:test/t.ml -> file:b/shape.mli [*]",
        "file:test/t.ml -> file:test/import.ml [*]",
    ], "the `import.ml` beside the user wins, and a module with an interface alone is that interface");
    let core = extract(&files, "lib/core.ml");
    assert_eq!(lines(&core, EdgeKind::ReExports), ["file:lib/core.ml -> file:lib/cset.ml [*]"], "`include` hands a module on");
    assert_eq!(lines(&extract(&files, "lib/import.ml"), EdgeKind::ReExports), ["file:lib/import.ml -> file:lib/core.ml [*]"], "so does an alias");
    assert!(lines(&extract(&files, "lib/cset.ml"), EdgeKind::Imports).is_empty(), "`cset.mli` is the module's own interface, not an import");
}

#[test]
fn a_bare_name_is_the_own_file_s_then_an_opened_module_s_and_a_path_is_read_module_by_module() {
    let files = modules();
    let ex = extract(&files, "lib/user.ml");
    assert_eq!(lines(&ex, EdgeKind::Calls), [
        "sym:lib/user.ml::b -> sym:lib/core.ml::exec []",
        "sym:lib/user.ml::c -> sym:lib/cset.ml::union []",
        "sym:lib/user.ml::d -> sym:lib/cset.ml::union []",
        "sym:lib/user.ml::l -> sym:lib/cset.ml::union []",
        "sym:lib/user.ml::n -> sym:lib/core.ml::exec []",
        "sym:lib/user.ml::o -> sym:lib/core.ml::exec []",
    ], "`R` is the opened `Import`'s alias of `Core`, `Core.union` is `Cset`'s through `include`, `Cset.(…)` opens `Cset`, and `|>` and `@@` apply; a functor's parameter shadows the file `Cset`");
    assert_eq!(lines(&ex, EdgeKind::References), [
        "sym:lib/user.ml::a -> sym:lib/import.ml::helper []",
        "sym:lib/user.ml::e -> sym:lib/cset.ml::Inner/deep []",
        "sym:lib/user.ml::f -> sym:lib/cset.ml::t [type]",
        "sym:lib/user.ml::h -> sym:lib/user.ml::Local/g []",
        "sym:lib/user.ml::l -> sym:lib/cset.ml::empty []",
        "sym:lib/user.ml::m -> sym:a/util.ml::v []",
    ], "a `let … in` hides the opened `helper`, `Missing.x` names nothing, `Util` is the one `util.ml` anywhere, and `empty` outside `Cset.(…)` is no `Cset`'s");
    let mut twice = files.to_vec();
    twice.push(("b/util.ml", "let v = 1\n"));
    assert!(!lines(&extract(&twice, "lib/user.ml"), EdgeKind::References).iter().any(|l| l.contains("util.ml")), "a name two files share names neither");
    assert_eq!(lines(&extract(&files, "test/t.ml"), EdgeKind::References), [
        "sym:test/t.ml::x -> sym:test/import.ml::helper []",
        "sym:test/t.ml::y -> sym:a/util.ml::v []",
        "sym:test/t.ml::z -> sym:b/shape.mli::area []",
    ]);
}

#[test]
fn an_interface_declares_with_val_and_documents_what_it_declares() {
    let ex = extract(&modules(), "lib/cset.mli");
    assert_eq!(ids(&ex), ["file:lib/cset.mli", "sym:lib/cset.mli::Inner", "sym:lib/cset.mli::Inner/deep", "sym:lib/cset.mli::empty", "sym:lib/cset.mli::union"]);
    let node = |id: &str| ex.nodes.iter().find(|n| n.id == id).unwrap();
    assert_eq!(node("sym:lib/cset.mli::union").body, "(** The union. *)\nval union : int -> int -> int");
    let source = "include module type of Cset\ninclude S with type t := t\nval x : t\n";
    assert!(lines(&extract(&[("lib/more.mli", source), ("lib/cset.ml", CSET)], "lib/more.mli"), EdgeKind::ReExports).is_empty(), "an interface's `include` names a module type, which hands no module on");
}

#[test]
fn a_changed_module_widens_to_its_users_and_through_what_hands_it_on() {
    let mut g = crate::model::Graph::default();
    let mut edge = |source: &str, target: &str, kind: EdgeKind| {
        g.edges.insert(crate::model::Edge {
            source: format!("file:{source}"),
            target: format!("file:{target}"),
            kind,
            context: "*".into(),
            file: source.into(),
        });
    };
    edge("lib/core.ml", "lib/cset.ml", EdgeKind::ReExports);
    edge("lib/import.ml", "lib/core.ml", EdgeKind::ReExports);
    edge("lib/user.ml", "lib/import.ml", EdgeKind::Imports);
    edge("lib/user.ml", "lib/other.ml", EdgeKind::Imports);
    edge("lib/far.ml", "lib/user.ml", EdgeKind::Imports);
    let all = ["lib/cset.ml", "lib/core.ml", "lib/import.ml", "lib/user.ml", "lib/other.ml", "lib/far.ml"];
    let owned = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let mut w = crate::code::engine::widen(&owned(&["lib/cset.ml"]), &[], &g, &owned(&all));
    w.sort();
    assert_eq!(w, ["lib/core.ml", "lib/import.ml", "lib/user.ml"], "a user of a user reads nothing it passes on");
}

#[test]
fn an_id_in_a_comment_or_string_is_cited_by_what_holds_it() {
    let ex = extract(&[("a.ml", "(* ADR-004 explains the wait *)\nlet wait () = print_string \"FR-APP-54\"\n")], "a.ml");
    assert_eq!(lines(&ex, EdgeKind::References), ["file:a.ml -> ADR-004 [comment]", "sym:a.ml::wait -> FR-APP-54 [string]"]);
}

#[test]
fn an_ocaml_file_is_read_by_its_extension_and_an_interface_by_its_own_grammar() {
    use crate::code::lang::{detect, Family, Lang};
    assert_eq!(Lang::of("lib/re.ml"), Some(Lang::OCaml));
    assert_eq!(Lang::of("lib/re.mli"), Some(Lang::OCamlInterface));
    assert_eq!(detect("bin/tool", Some("#!/usr/bin/env ocaml\n")), Some(Lang::OCaml));
    assert_eq!((Lang::OCaml.family(), Lang::OCamlInterface.family()), (Family::OCaml, Family::OCaml));
}

#[test]
fn an_open_reads_the_module_its_path_names_a_local_open_its_expression_alone_and_a_type_is_no_value() {
    let files = [
        ("lib/shapes.ml", "type t = int\nlet t = 0\nmodule Inner = struct let deep = 1 end\nmodule Own = struct let mine = 1 end\ninclude Own\n"),
        ("lib/fmt.ml", "let list = 1\nlet pair = 2\n"),
        ("lib/dyn.ml", "let list = 3\n"),
        ("lib/use.ml", concat!(
            "open Shapes.Inner\n",
            "type list = int\n",
            "let a = deep\n",
            "let b = Shapes.mine\n",
            "let c (t : Shapes.t) = Shapes.t\n",
            "let d { pair; _ } = pair\n",
            "open Fmt\n",
            "let e = let open Dyn in list\n",
            "let f = list\n",
            "let g = Dyn.(list)\n",
            "module M = struct let x = 1 end\n",
            "open M\n",
            "let h = x\n",
            "let i (x : list) = x\n",
        )),
    ];
    assert_eq!(lines(&extract(&files, "lib/use.ml"), EdgeKind::References), [
        "sym:lib/use.ml::a -> sym:lib/shapes.ml::Inner/deep []",
        "sym:lib/use.ml::b -> sym:lib/shapes.ml::Own/mine []",
        "sym:lib/use.ml::c -> sym:lib/shapes.ml::t []",
        "sym:lib/use.ml::c -> sym:lib/shapes.ml::t [type]",
        "sym:lib/use.ml::e -> sym:lib/dyn.ml::list []",
        "sym:lib/use.ml::f -> sym:lib/fmt.ml::list []",
        "sym:lib/use.ml::g -> sym:lib/dyn.ml::list []",
        "sym:lib/use.ml::h -> sym:lib/use.ml::M/x []",
        "sym:lib/use.ml::i -> sym:lib/use.ml::list [type]",
    ], "`open Shapes.Inner` opens `Inner`, `include Own` hands on a module of the file's own, `t` is a type and a value under one id, a field pattern binds `pair`, `let open Dyn` and `Dyn.(…)` reach their expression alone and shadow `open Fmt` there, `open M` opens the file's own `M`, and the type `list` is never the value");
}

#[test]
fn a_name_the_file_binds_to_a_module_is_no_mention_of_the_file_it_spells() {
    let files = [
        ("lib/re.ml", "let exec = 0\n"),
        ("lib/core.ml", "let exec = 1\n"),
        ("lib/ext.ml", "let v = 2\n"),
        ("lib/own.ml", "let w = 3\n"),
        ("lib/use.ml", "module Re = Core\nlet a = Re.exec\nmodule Ext = Lib.Ext\nlet b = Ext.v\nmodule Own = struct let w = 4 end\nlet c = Own.w\n"),
    ];
    let ex = extract(&files, "lib/use.ml");
    assert_eq!(lines(&ex, EdgeKind::Imports), [
        "file:lib/use.ml -> file:lib/core.ml [*]",
        "file:lib/use.ml -> file:lib/ext.ml [*]",
    ], "`Re` is `Core` and `Own` the file's own, so neither reads its file; `Ext` is bound to a module no file holds, so its own name is the guess");
    assert_eq!(lines(&ex, EdgeKind::ReExports), ["file:lib/use.ml -> file:lib/core.ml [*]"]);
    assert_eq!(lines(&ex, EdgeKind::References), [
        "sym:lib/use.ml::a -> sym:lib/core.ml::exec []",
        "sym:lib/use.ml::b -> sym:lib/ext.ml::v []",
        "sym:lib/use.ml::c -> sym:lib/use.ml::Own/w []",
    ]);
}

#[test]
fn an_open_or_include_shows_its_names_from_where_it_is_written_to_the_end_of_its_module() {
    let files = [
        ("lib/bar.ml", "let early = 1\nlet inside = 2\nlet late = 3\n"),
        ("lib/wrap.ml", "module M = struct\n  include Bar\nend\n"),
        ("lib/use.ml", concat!(
            "let a = early\n",
            "module M = struct\n  open Bar\n  let b = inside\nend\n",
            "let c = inside\n",
            "open Bar\n",
            "let d = late\n",
            "let e = Wrap.M.late\n",
            "let f = Wrap.late\n",
        )),
    ];
    assert_eq!(lines(&extract(&files, "lib/use.ml"), EdgeKind::References), [
        "sym:lib/use.ml::M/b -> sym:lib/bar.ml::inside []",
        "sym:lib/use.ml::d -> sym:lib/bar.ml::late []",
        "sym:lib/use.ml::e -> sym:lib/bar.ml::late []",
    ], "`early` before the open and `inside` outside the module opening it name nothing, and an include in `M` is `M`'s alone");
}

#[test]
fn a_module_bound_to_one_the_file_defines_is_read_in_that_module_and_not_in_a_file_of_its_name() {
    let files = [
        ("lib/foo.ml", "let x = 5\n"),
        ("lib/a.ml", "module Foo = struct let x = 1 end\nmodule Bar = Foo\nlet y = Bar.x\n"),
        ("lib/b.ml", "let z = A.Bar.x\n"),
    ];
    assert_eq!(lines(&extract(&files, "lib/a.ml"), EdgeKind::References), ["sym:lib/a.ml::y -> sym:lib/a.ml::Foo/x []"]);
    assert_eq!(lines(&extract(&files, "lib/b.ml"), EdgeKind::References), ["sym:lib/b.ml::z -> sym:lib/a.ml::Foo/x []"]);
}

#[test]
fn a_comment_above_a_type_an_exception_or_a_class_documents_it() {
    let ex = extract(&[("a.ml", "(** T. *)\ntype t = int\n(** E. *)\nexception Oops\n(** C. *)\nclass cls = object end\n")], "a.ml");
    let body = |id: &str| ex.nodes.iter().find(|n| n.id == id).unwrap().body.clone();
    assert_eq!(body("sym:a.ml::t"), "(** T. *)\ntype t = int");
    assert_eq!(body("sym:a.ml::Oops"), "(** E. *)\nexception Oops");
    assert_eq!(body("sym:a.ml::cls"), "(** C. *)\nclass cls = object end");
}
