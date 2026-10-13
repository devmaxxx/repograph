; Locals adapted from the grammar's `locals.scm`:
; Copyright (c) 2021 Munif Tanjim, MIT License, https://github.com/tree-sitter-grammars/tree-sitter-lua

[
  (chunk)
  (do_statement)
  (while_statement)
  (repeat_statement)
  (if_statement)
  (for_statement)
  (function_declaration)
  (function_definition)
] @local.scope

; A `local` binds what its value is, which is no function this file declares. A `local` whose value
; is the call `require(...)` itself is the import's alias: its members resolve through the import.
; Any other value, `require("a").b` or `require("a") or {}` among them, is a value. A `local` with no
; value binds nothing yet, since a test assigns its modules to such locals in its setup.
(variable_declaration
  (assignment_statement
    (variable_list name: (identifier) @local.definition)
    (expression_list . value: [
      (nil) (false) (true) (number) (string) (vararg_expression) (function_definition)
      (table_constructor) (binary_expression) (unary_expression) (parenthesized_expression)
      (identifier) (dot_index_expression) (bracket_index_expression)
    ])))

(variable_declaration
  (assignment_statement
    (variable_list name: (identifier) @local.definition)
    (expression_list . value: (function_call name: (_) @_callee)))
  (#not-eq? @_callee "require"))

; A `local function` in a function's body binds in its whole block, which a call before it in the
; same block cannot see, but a call from a sibling function can. One in a block at the file's level
; is a declaration.
((block (function_declaration "local" name: (identifier) @local.definition)) @local.scope
  (#set! within "function_declaration"))

((block (function_declaration "local" name: (identifier) @local.definition)) @local.scope
  (#set! within "function_definition"))

(for_generic_clause (variable_list name: (identifier) @local.definition))

(for_numeric_clause name: (identifier) @local.definition)

(parameters name: (identifier) @local.definition)

(function_call name: (identifier) @reference.calls)

; `m.f()` and `m:f()`: `m` is a table the file declares members of, or a `require`'s alias.
((function_call name: (dot_index_expression table: (identifier) @receiver field: (identifier) @reference.calls))
  (#not-eq? @receiver "self"))

((function_call name: (method_index_expression table: (identifier) @receiver method: (identifier) @reference.calls))
  (#not-eq? @receiver "self"))

; `self` in `function M:g()` is `M`.
((function_call name: (dot_index_expression table: (identifier) @receiver field: (identifier) @reference.calls))
  (#eq? @receiver "self")
  (#set! receiver "owner"))

((function_call name: (method_index_expression table: (identifier) @receiver method: (identifier) @reference.calls))
  (#eq? @receiver "self")
  (#set! receiver "owner"))

(comment) @cite.comment

(string) @cite.string
