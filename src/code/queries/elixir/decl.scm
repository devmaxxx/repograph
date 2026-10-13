; Every definition is a call of a macro, told apart by the macro's name. A module's name is one
; token, `Plug.Conn`, and a module nested in another joins its name with `.`, as Elixir names it.

((call target: (identifier) @_def (arguments . (alias) @name)) @definition.class
  (#eq? @_def "defmodule"))

((call target: (identifier) @_def (arguments . (alias) @name)) @definition.protocol
  (#eq? @_def "defprotocol"))

; An implementation is a module of its own, named by its protocol and its type.
((call
  target: (identifier) @_def
  (arguments . (alias) @name (keywords (pair key: (keyword) @_for value: (alias) @name)))) @definition.class
  (#eq? @_def "defimpl")
  (#match? @_for "^for:"))

; A type that is no one alias, `for: [A, B]` or `for: __MODULE__`, is named as it is spelled, so that
; the functions inside are the implementation's and no top-level name of the file.
((call
  target: (identifier) @_def
  (arguments . (alias) @name (keywords (pair key: (keyword) @_for value: [(list) (identifier)] @name)))) @definition.class
  (#eq? @_def "defimpl")
  (#match? @_for "^for:"))

((call target: (identifier) @_def (arguments . (alias) @name .)) @definition.class
  (#eq? @_def "defimpl"))

; A function is named by its head, `f`, `f(a)`, `f(a) when g` or `f when g`. Every clause and arity
; of one name is the one symbol, whose span runs from its first clause to its last. A `def` in a
; `quote` is code a macro writes into its caller.
((call
  target: (identifier) @_def
  (arguments . [
    (identifier) @name
    (call target: (identifier) @name)
    (binary_operator left: (identifier) @name operator: "when")
    (binary_operator left: (call target: (identifier) @name) operator: "when")
  ])) @definition.method
  (#any-of? @_def "def" "defmacro" "defguard" "defdelegate")
  (#set! variant)
  (#set! outside "call quote"))

((call
  target: (identifier) @_def
  (arguments . [
    (identifier) @name
    (call target: (identifier) @name)
    (binary_operator left: (identifier) @name operator: "when")
    (binary_operator left: (call target: (identifier) @name) operator: "when")
  ])) @definition.method
  (#any-of? @_def "defp" "defmacrop" "defguardp")
  (#set! export "never")
  (#set! variant)
  (#set! outside "call quote"))

; `alias` and `import` name a module, never a file. An alias makes its last segment, or its `as:`,
; stand for the module; an import makes its functions callable bare.
((call target: (identifier) @_alias (arguments . (alias) @import.path .)) @import
  (#eq? @_alias "alias")
  (#set! binds "alias"))

((call
  target: (identifier) @_alias
  (arguments . (alias) @import.path . (keywords (pair key: (keyword) @_as value: (alias) @import.alias)))) @import
  (#eq? @_alias "alias")
  (#match? @_as "^as:")
  (#set! binds "alias"))

; Options that rename nothing, `warn: false`, leave the alias its last segment.
((call
  target: (identifier) @_alias
  (arguments . (alias) @import.path . (keywords) @_options)) @import
  (#eq? @_alias "alias")
  (#not-match? @_options "\\bas:")
  (#set! binds "alias"))

((call target: (identifier) @_import (arguments . (alias) @import.path)) @import
  (#eq? @_import "import")
  (#set! binds "open"))
