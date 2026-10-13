//! Elixir's shapes, each on inline files, so the reading of a definition, a directive and a call is
//! pinned by the assertion rather than by a corpus. Elixir is a spec row and its two queries, with
//! no hook.

use crate::code::prose::testing::{ids, lines, repo};
use crate::model::{EdgeKind, Extraction};

const GLOBS: &[&str] = &["**/*.ex", "**/*.exs"];

fn extract(files: &[(&str, &str)], rel: &str) -> Extraction {
    let (_dir, resolver) = repo(GLOBS, files);
    let text = files.iter().find(|(f, _)| *f == rel).map(|(_, t)| *t).unwrap();
    crate::code::front::read(&resolver, rel, text)
}

const CONN: &str = r#"defmodule Plug.Conn do
  @moduledoc "A connection."

  defmodule Unfetched do
    defstruct [:aspect]
  end

  @doc "Sends it."
  @spec send(t) :: t
  def send(conn) when is_map(conn), do: conn
  def send(conn, opts), do: {conn, opts}

  defp check(conn), do: conn

  defmacro __using__(_) do
    quote do
      def injected, do: :ok
    end
  end

  defguard is_conn(term) when is_map(term)

  defimpl Inspect do
    def inspect(conn, _), do: conn
  end
end

defprotocol Plug.Exception do
  def status(exception)
end

defimpl Plug.Exception, for: Any do
  def status(_), do: 500
end
"#;

#[test]
fn a_module_nests_with_a_dot_a_hidden_function_is_no_export_and_an_impl_names_its_protocol_and_type() {
    let ex = extract(&[("lib/plug/conn.ex", CONN)], "lib/plug/conn.ex");
    assert_eq!(ids(&ex), [
        "file:lib/plug/conn.ex",
        "sym:lib/plug/conn.ex::Plug.Conn",
        "sym:lib/plug/conn.ex::Plug.Conn.Inspect",
        "sym:lib/plug/conn.ex::Plug.Conn.Inspect.inspect",
        "sym:lib/plug/conn.ex::Plug.Conn.Unfetched",
        "sym:lib/plug/conn.ex::Plug.Conn.__using__",
        "sym:lib/plug/conn.ex::Plug.Conn.check",
        "sym:lib/plug/conn.ex::Plug.Conn.is_conn",
        "sym:lib/plug/conn.ex::Plug.Conn.send",
        "sym:lib/plug/conn.ex::Plug.Exception",
        "sym:lib/plug/conn.ex::Plug.Exception.status",
        "sym:lib/plug/conn.ex::Plug.Exception/Any",
        "sym:lib/plug/conn.ex::Plug.Exception/Any.status",
    ], "a struct is its module, a `def` in a `quote` is the caller's, and every clause of `send` is one symbol");
    assert_eq!(lines(&ex, EdgeKind::Declares), [
        "file:lib/plug/conn.ex -> sym:lib/plug/conn.ex::Plug.Conn [export]",
        "file:lib/plug/conn.ex -> sym:lib/plug/conn.ex::Plug.Exception [export]",
        "file:lib/plug/conn.ex -> sym:lib/plug/conn.ex::Plug.Exception/Any [export]",
        "sym:lib/plug/conn.ex::Plug.Conn -> sym:lib/plug/conn.ex::Plug.Conn.Inspect [export]",
        "sym:lib/plug/conn.ex::Plug.Conn -> sym:lib/plug/conn.ex::Plug.Conn.Unfetched [export]",
        "sym:lib/plug/conn.ex::Plug.Conn -> sym:lib/plug/conn.ex::Plug.Conn.__using__ [export]",
        "sym:lib/plug/conn.ex::Plug.Conn -> sym:lib/plug/conn.ex::Plug.Conn.check []",
        "sym:lib/plug/conn.ex::Plug.Conn -> sym:lib/plug/conn.ex::Plug.Conn.is_conn [export]",
        "sym:lib/plug/conn.ex::Plug.Conn -> sym:lib/plug/conn.ex::Plug.Conn.send [export]",
        "sym:lib/plug/conn.ex::Plug.Conn.Inspect -> sym:lib/plug/conn.ex::Plug.Conn.Inspect.inspect [export]",
        "sym:lib/plug/conn.ex::Plug.Exception -> sym:lib/plug/conn.ex::Plug.Exception.status [export]",
        "sym:lib/plug/conn.ex::Plug.Exception/Any -> sym:lib/plug/conn.ex::Plug.Exception/Any.status [export]",
    ]);
    let node = |id: &str| ex.nodes.iter().find(|n| n.id == id).unwrap();
    let send = node("sym:lib/plug/conn.ex::Plug.Conn.send");
    assert_eq!((send.line, send.end), (10, 11), "a function's span runs from its first clause to its last");
    assert_eq!(send.body, "@doc \"Sends it.\"\n@spec send(t) :: t\ndef send(conn) when is_map(conn), do: conn");
    assert_eq!(node("sym:lib/plug/conn.ex::Plug.Exception/Any").label, "Plug.Exception.Any");
}

const ROUTER: &str = r#"defmodule Plug.Router do
  alias Plug.Conn
  alias Plug.Router.Utils, as: U
  import Plug.Helpers

  @spec call(Conn.t) :: Conn.t
  def call(conn) do
    conn
    |> match()
    |> dispatch
    |> Enum.map(&route/1)
    |> Conn.send()
    |> U.build()
    |> helper()
    |> __MODULE__.dispatch()
    |> inspect()
    |> Missing.call()
  end

  def call(conn, opts), do: call(conn)

  defp match(conn), do: conn
  defp dispatch(conn), do: conn
  defp route(conn), do: conn

  defmacro __before_compile__(_) do
    quote do
      def later, do: match(nil)
    end
  end
end
"#;

const HELPERS: &str = "defmodule Plug.Helpers do\n  def helper(x), do: x\nend\n";
const CONN_SEND: &str = "defmodule Plug.Conn do\n  def send(conn), do: conn\nend\n";
const UTILS: &str = "defmodule Plug.Router.Utils do\n  def build(x), do: x\nend\n";

#[test]
fn a_bare_call_is_the_own_module_s_then_an_import_s_and_an_alias_stands_for_its_module() {
    let files = [("lib/plug/router.ex", ROUTER), ("lib/plug/helpers.ex", HELPERS), ("lib/plug/conn.ex", CONN_SEND), ("lib/plug/router/utils.ex", UTILS)];
    let ex = extract(&files, "lib/plug/router.ex");
    assert_eq!(lines(&ex, EdgeKind::Calls), [
        "sym:lib/plug/router.ex::Plug.Router.call -> sym:lib/plug/conn.ex::Plug.Conn.send []",
        "sym:lib/plug/router.ex::Plug.Router.call -> sym:lib/plug/helpers.ex::Plug.Helpers.helper []",
        "sym:lib/plug/router.ex::Plug.Router.call -> sym:lib/plug/router.ex::Plug.Router.dispatch []",
        "sym:lib/plug/router.ex::Plug.Router.call -> sym:lib/plug/router.ex::Plug.Router.match []",
        "sym:lib/plug/router.ex::Plug.Router.call -> sym:lib/plug/router.ex::Plug.Router.route []",
        "sym:lib/plug/router.ex::Plug.Router.call -> sym:lib/plug/router/utils.ex::Plug.Router.Utils.build []",
    ], "a pipe's right side and a capture are calls, `call/1` from `call/2` is the one symbol, a typespec and a quote call nothing, and Kernel's `inspect` and an unknown module name nothing");
    assert_eq!(lines(&ex, EdgeKind::References), [
        "sym:lib/plug/router.ex::Plug.Router -> sym:lib/plug/conn.ex::Plug.Conn []",
        "sym:lib/plug/router.ex::Plug.Router -> sym:lib/plug/helpers.ex::Plug.Helpers []",
        "sym:lib/plug/router.ex::Plug.Router -> sym:lib/plug/router/utils.ex::Plug.Router.Utils []",
        "sym:lib/plug/router.ex::Plug.Router.call -> sym:lib/plug/conn.ex::Plug.Conn []",
        "sym:lib/plug/router.ex::Plug.Router.call -> sym:lib/plug/router/utils.ex::Plug.Router.Utils []",
    ], "a directive names its module, as a call's module does through the alias");
    assert!(lines(&ex, EdgeKind::Imports).is_empty(), "a directive names a module, never a file");
}

#[test]
fn an_alias_in_a_function_reaches_only_that_function() {
    let source = "defmodule A do\n  def one do\n    alias Plug.Conn, as: C\n    C.send(nil)\n  end\n\n  def two, do: C.send(nil)\nend\n";
    let files = [("lib/a.ex", source), ("lib/plug/conn.ex", CONN_SEND)];
    assert_eq!(lines(&extract(&files, "lib/a.ex"), EdgeKind::Calls), ["sym:lib/a.ex::A.one -> sym:lib/plug/conn.ex::Plug.Conn.send []"]);
}

#[test]
fn a_hidden_function_stays_out_of_another_module_s_reach() {
    let files = [
        ("lib/a.ex", "defmodule A do\n  defp hidden, do: 1\n  def open, do: 1\nend\n"),
        ("lib/b.ex", "defmodule B do\n  def run do\n    A.hidden()\n    A.open()\n  end\nend\n"),
    ];
    assert_eq!(lines(&extract(&files, "lib/b.ex"), EdgeKind::Calls), ["sym:lib/b.ex::B.run -> sym:lib/a.ex::A.open []"]);
}

#[test]
fn an_id_in_a_comment_or_string_is_cited_by_what_holds_it() {
    let ex = extract(&[("a.ex", "# ADR-004 explains the wait\ndefmodule W do\n  def wait, do: IO.puts(\"FR-APP-54\")\nend\n")], "a.ex");
    assert_eq!(lines(&ex, EdgeKind::References), ["file:a.ex -> ADR-004 [comment]", "sym:a.ex::W.wait -> FR-APP-54 [string]"]);
}

#[test]
fn an_elixir_file_is_read_by_its_extension_and_a_script_too() {
    use crate::code::lang::{Family, Lang};
    for rel in ["lib/plug.ex", "test/plug_test.exs", "mix.exs"] {
        assert_eq!(Lang::of(rel), Some(Lang::Elixir), "{rel}");
    }
    assert_eq!(Lang::Elixir.family(), Family::Elixir);
}

#[test]
fn an_alias_reaches_through_an_earlier_one_and_an_option_that_renames_nothing_keeps_it() {
    let sub = "defmodule Plug.Conn.Sub do\n  def x, do: 1\n  def y, do: 1\nend\n";
    let source = "defmodule A do\n  alias Plug.Conn, warn: false\n  alias Conn.Sub\n  import Sub\n  def f, do: Sub.x()\n  def g, do: y()\nend\n";
    let files = [("lib/a.ex", source), ("lib/plug/conn.ex", CONN_SEND), ("lib/plug/conn/sub.ex", sub)];
    assert_eq!(lines(&extract(&files, "lib/a.ex"), EdgeKind::Calls), [
        "sym:lib/a.ex::A.f -> sym:lib/plug/conn/sub.ex::Plug.Conn.Sub.x []",
        "sym:lib/a.ex::A.g -> sym:lib/plug/conn/sub.ex::Plug.Conn.Sub.y []",
    ]);
}

#[test]
fn an_impl_for_no_one_alias_keeps_its_functions_out_of_the_file_s_top_level() {
    let files = [
        ("lib/a.ex", "defimpl Inspect, for: [A, B] do\n  def inspect(a, _), do: a\nend\n"),
        ("lib/b.ex", "defmodule B do\n  def run(x), do: inspect(x)\nend\n"),
    ];
    assert!(lines(&extract(&files, "lib/b.ex"), EdgeKind::Calls).is_empty());
}

#[test]
fn a_module_call_in_a_quote_is_the_caller_s_like_a_bare_one() {
    let source = "defmodule A do\n  def f, do: 1\n  defmacro __using__(_) do\n    quote do\n      __MODULE__.f()\n    end\n  end\nend\n";
    assert!(lines(&extract(&[("lib/a.ex", source)], "lib/a.ex"), EdgeKind::Calls).is_empty());
}
