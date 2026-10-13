//! Lua's shapes, each on inline files, so the reading of a declaration, a `require` and a call is
//! pinned by the assertion rather than by a corpus. Lua arrives as data alone: a spec row and its two
//! queries.

use crate::code::prose::testing::{ids, lines, repo};
use crate::model::{EdgeKind, Extraction};

const GLOBS: &[&str] = &["**/*.lua"];

fn extract(files: &[(&str, &str)], rel: &str) -> Extraction {
    let (_dir, resolver) = repo(GLOBS, files);
    let text = files.iter().find(|(f, _)| *f == rel).map(|(_, t)| *t).unwrap();
    crate::code::front::read(&resolver, rel, text)
}

const MODULE: &str = r#"local M = {}

--- Joins two paths.
-- The separator is the platform's.
local function join(a, b)
  local function sep() return "/" end
  return a .. sep() .. b
end

function M.path(a, b) return join(a, b) end

function M:new() return setmetatable({}, self) end

function M.sub.deep() end

M.alias = function() end

function global_helper() end

do
  function M.in_do() end
  local function hidden() end
end

if jit then
  function M.branch() end
else
  local function fallback()
    function M.late() end
  end
end

return M
"#;

#[test]
fn the_file_declares_its_functions_with_their_table_as_owner_and_none_inside_another_function() {
    let ex = extract(&[("src/m.lua", MODULE)], "src/m.lua");
    assert_eq!(ids(&ex), [
        "file:src/m.lua",
        "sym:src/m.lua::M.alias",
        "sym:src/m.lua::M.branch",
        "sym:src/m.lua::M.in_do",
        "sym:src/m.lua::M.new",
        "sym:src/m.lua::M.path",
        "sym:src/m.lua::M.sub.deep",
        "sym:src/m.lua::fallback",
        "sym:src/m.lua::global_helper",
        "sym:src/m.lua::hidden",
        "sym:src/m.lua::join",
    ], "a function in a block at the file's level is the file's, and one in a function's body is not");
    assert_eq!(lines(&ex, EdgeKind::Declares), [
        "file:src/m.lua -> sym:src/m.lua::M.alias [export]",
        "file:src/m.lua -> sym:src/m.lua::M.branch [export]",
        "file:src/m.lua -> sym:src/m.lua::M.in_do [export]",
        "file:src/m.lua -> sym:src/m.lua::M.new [export]",
        "file:src/m.lua -> sym:src/m.lua::M.path [export]",
        "file:src/m.lua -> sym:src/m.lua::M.sub.deep [export]",
        "file:src/m.lua -> sym:src/m.lua::fallback []",
        "file:src/m.lua -> sym:src/m.lua::global_helper [export]",
        "file:src/m.lua -> sym:src/m.lua::hidden []",
        "file:src/m.lua -> sym:src/m.lua::join []",
    ], "a `local function` is the file's own");
    let node = |id: &str| ex.nodes.iter().find(|n| n.id == id).unwrap();
    let join = node("sym:src/m.lua::join");
    assert_eq!((join.label.as_str(), join.line, join.end), ("join", 5, 8));
    assert_eq!(join.body, "--- Joins two paths.\n-- The separator is the platform's.\nlocal function join(a, b)");
    let new = node("sym:src/m.lua::M.new");
    assert_eq!((new.label.as_str(), new.line, new.end, new.body.as_str()), ("M.new", 12, 12, "function M:new() return setmetatable({}, self) end"));
    assert_eq!(node("sym:src/m.lua::M.alias").line, 16, "an assigned function starts at its table");
}

#[test]
fn a_call_reaches_the_own_function_and_a_local_binding_shadows_it() {
    let src = r#"local function join(a, b) return a end
local function sep() return join end
function run(join) return join() end
function go()
  local function sep() end
  return sep() .. join()
end
do
  local function quote(s) return s end
  function M.q(s) return quote(s) end
end
local cb = function()
  local function sep() end
  return sep()
end
"#;
    let ex = extract(&[("a.lua", src)], "a.lua");
    assert_eq!(lines(&ex, EdgeKind::Calls), ["sym:a.lua::M.q -> sym:a.lua::quote []", "sym:a.lua::go -> sym:a.lua::join []"],
        "a parameter shadows `join`, and a `sep` inside a function or a function value shadows the file's");
}

const REQUIRES: &str = r#"local fs = require("luarocks.fs")
local cfg <const> = require "luarocks.core.cfg"
local util = require("util")
local missing = require("lfs")
local dir
require("luarocks.loader")

local function setup()
  dir = require("luarocks.dir")
end

return require("luarocks.loader")
"#;

#[test]
fn each_require_shape_imports_the_file_its_dotted_name_names_under_a_root() {
    let files = [
        ("src/luarocks/main.lua", REQUIRES),
        ("src/luarocks/fs.lua", "local fs = {}\nreturn fs\n"),
        ("src/luarocks/core/cfg/init.lua", "return {}\n"),
        ("lua/util.lua", "return {}\n"),
        ("src/luarocks/loader.lua", "return {}\n"),
        ("src/luarocks/dir.lua", "return {}\n"),
    ];
    assert_eq!(lines(&extract(&files, "src/luarocks/main.lua"), EdgeKind::Imports), [
        "file:src/luarocks/main.lua -> file:lua/util.lua [*]",
        "file:src/luarocks/main.lua -> file:src/luarocks/core/cfg/init.lua [*]",
        "file:src/luarocks/main.lua -> file:src/luarocks/dir.lua [*]",
        "file:src/luarocks/main.lua -> file:src/luarocks/fs.lua [*]",
        "file:src/luarocks/main.lua -> file:src/luarocks/loader.lua [*]",
    ], "a module outside the repository, `lfs`, imports nothing");
}

#[test]
fn a_dotted_name_is_never_read_from_the_importer_s_directory() {
    let files = [("src/a/main.lua", "local b = require(\"b\")\n"), ("src/a/b.lua", "return {}\n")];
    assert!(lines(&extract(&files, "src/a/main.lua"), EdgeKind::Imports).is_empty());
}

const CALLER: &str = r#"local fs = require("luarocks.fs")
local tools = require("luarocks.tools")
require("luarocks.globals")
require("luarocks.other")

local function copy(src, dst)
  fs.make_dir(dst)
  local obj = fs.new()
  obj:close()
  tools:run()
  shared()
  only_here()
  return fs.missing()
end

function M.check(fs)
  return fs.make_dir("x")
end
"#;

#[test]
fn a_member_resolves_through_the_require_its_receiver_names() {
    let files = [
        ("src/luarocks/caller.lua", CALLER),
        ("src/luarocks/fs.lua", "local fs = {}\nfunction fs.make_dir(d) end\nfunction fs.new() end\nlocal function make_dir() end\nreturn fs\n"),
        ("src/luarocks/tools.lua", "local T = {}\nfunction T:run() end\nreturn T\n"),
        ("src/luarocks/globals.lua", "function shared() end\nfunction only_here() end\n"),
        ("src/luarocks/other.lua", "function shared() end\n"),
    ];
    let ex = extract(&files, "src/luarocks/caller.lua");
    assert_eq!(lines(&ex, EdgeKind::Calls), [
        "sym:src/luarocks/caller.lua::copy -> sym:src/luarocks/fs.lua::fs.make_dir []",
        "sym:src/luarocks/caller.lua::copy -> sym:src/luarocks/fs.lua::fs.new []",
        "sym:src/luarocks/caller.lua::copy -> sym:src/luarocks/globals.lua::only_here []",
        "sym:src/luarocks/caller.lua::copy -> sym:src/luarocks/tools.lua::T.run []",
    ], "`obj:close()` is a method on a value, `shared` is defined twice, `fs.missing` is defined nowhere, \
        and a parameter named `fs` is no module");
}

#[test]
fn a_member_two_required_files_declare_resolves_to_neither() {
    let files = [
        ("a.lua", "local m = require(\"x\")\nm = require(\"y\")\nlocal function go() m.f() end\n"),
        ("x.lua", "local X = {}\nfunction X.f() end\nreturn X\n"),
        ("y.lua", "local Y = {}\nfunction Y.f() end\nreturn Y\n"),
    ];
    assert!(lines(&extract(&files, "a.lua"), EdgeKind::Calls).is_empty());
}

#[test]
fn an_id_in_a_comment_or_string_is_cited_by_what_holds_it() {
    let ex = extract(&[("a.lua", "-- ADR-004 explains the wait\nlocal function wait()\n  print(\"FR-APP-54\")\nend\n")], "a.lua");
    assert_eq!(lines(&ex, EdgeKind::References), ["file:a.lua -> ADR-004 [comment]", "sym:a.lua::wait -> FR-APP-54 [string]"]);
}

#[test]
fn a_lua_file_is_read_by_its_extension_or_its_shebang() {
    use crate::code::lang::{detect, Family, Lang};
    assert_eq!(Lang::of("src/luarocks/fs.lua"), Some(Lang::Lua));
    assert_eq!(Lang::Lua.family(), Family::Lua);
    for head in ["#!/usr/bin/env lua\n", "#!/usr/bin/lua5.4\n", "#!/usr/local/bin/lua5.1\n"] {
        assert_eq!(detect("bin/luarocks", Some(head)), Some(Lang::Lua), "{head:?}");
    }
    assert_eq!(detect("bin/luarocks", Some("#!/usr/bin/env luajit\n")), None, "LuaJIT is another interpreter's name");
}

#[test]
fn a_receiver_reaches_only_what_the_required_file_returns_or_else_its_top_level_functions() {
    let files = [
        ("a.lua", "local m = require(\"x\")\nlocal n = require(\"y\")\nlocal function go()\n  m.deep()\n  m.parse()\n  m.helper()\n  m.run()\n  n.top()\n  n.inner()\nend\n"),
        ("x.lua", "local X = {}\nlocal H = {}\nfunction H.parse() end\nfunction helper() end\nfunction X.sub.deep() end\nfunction X.run() end\nreturn X\n"),
        ("y.lua", "local T = {}\nfunction T.inner() end\nfunction top() end\n"),
    ];
    assert_eq!(lines(&extract(&files, "a.lua"), EdgeKind::Calls), [
        "sym:a.lua::go -> sym:x.lua::X.run []",
        "sym:a.lua::go -> sym:y.lua::top []",
    ], "`X.sub.deep` is no member of `X`, `H` is not returned, a global `helper` is not `X`'s, and a file \
        that returns no table hands on its top-level functions alone");
}

#[test]
fn a_call_through_the_file_s_own_table_or_self_reaches_its_member() {
    let src = r#"local M = {}
function M.h() end
function M.g() return M.h() end
function M:m() return self:g(), self.h() end
function M.k(M) return M.h() end
local function helper() return M.h(), self.h() end
return M
"#;
    assert_eq!(lines(&extract(&[("m.lua", src)], "m.lua"), EdgeKind::Calls), [
        "sym:m.lua::M.g -> sym:m.lua::M.h []",
        "sym:m.lua::M.m -> sym:m.lua::M.g []",
        "sym:m.lua::M.m -> sym:m.lua::M.h []",
        "sym:m.lua::helper -> sym:m.lua::M.h []",
    ], "a parameter named `M` is another value, and `self` outside a member names no table");
}

#[test]
fn a_local_holding_more_than_the_require_call_is_a_value_and_shadows_a_global() {
    let files = [
        ("a.lua", "require(\"g\")\nlocal cfg = require(\"x\").settings\nlocal m = require(\"x\") or {}\nlocal function go()\n  cfg()\n  m()\n  m.run()\nend\n"),
        ("g.lua", "function cfg() end\nfunction m() end\n"),
        ("x.lua", "local X = {}\nfunction X.run() end\nreturn X\n"),
    ];
    assert!(lines(&extract(&files, "a.lua"), EdgeKind::Calls).is_empty(), "`cfg` and `m` are locals, not the globals `g.lua` defines");
}
