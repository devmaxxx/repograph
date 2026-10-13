; A function's head is no call of itself: its name binds in the head alone, as the body's recursive
; call is still the function's.
(call
  target: (identifier) @_def
  (arguments . [
    (call target: (identifier) @local.definition)
    (binary_operator left: (call target: (identifier) @local.definition) operator: "when")
  ] @local.scope)
  (#any-of? @_def "def" "defp" "defmacro" "defmacrop" "defguard" "defguardp" "defdelegate"))

; A bare call is the caller's module's function first, then one an import in scope opens; a pipe's
; right side and a capture `&f/1` are calls of `f` too. A module attribute holds a typespec or a
; value, never a call, and a bare call in a `quote` is resolved in the module the macro writes into.
((call target: (identifier) @reference.calls)
  (#set! receiver "owner")
  (#set! outside "unary_operator @")
  (#set! outside "call quote"))

((binary_operator operator: "|>" right: (identifier) @reference.calls)
  (#set! receiver "owner")
  (#set! outside "unary_operator @")
  (#set! outside "call quote"))

((unary_operator operator: "&" operand: (binary_operator left: (identifier) @reference.calls operator: "/" right: (integer)))
  (#set! receiver "owner")
  (#set! outside "unary_operator @")
  (#set! outside "call quote"))

((call target: (dot left: (identifier) @_module right: (identifier) @reference.calls))
  (#eq? @_module "__MODULE__")
  (#set! receiver "owner")
  (#set! outside "unary_operator @")
  (#set! outside "call quote"))

; `Plug.Conn.f()` is the path `Plug.Conn/f`, through an alias when its first segment is one.
((call target: (dot left: (alias) @name right: (identifier) @name)) @reference.calls
  (#set! outside "unary_operator @"))

; A module named anywhere: as a value, a struct, a call's module, an `alias`, `import`, `require` or
; `use`, a `@behaviour`, a typespec. A module's own name reaches the module itself, which writes no edge.
(alias) @reference.references

(comment) @cite.comment

[(string) (charlist) (sigil)] @cite.string
