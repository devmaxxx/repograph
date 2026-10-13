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

; A `local` binds what its value is, which is no function this file declares. The alias of a
; `require` is no value: its members resolve through the import. A `local` with no value binds
; nothing yet, since a test assigns its modules to such locals in its setup.
(variable_declaration
  (assignment_statement
    (variable_list name: (identifier) @local.definition)
    (expression_list . value: (_) @_value))
  (#not-match? @_value "^require[ (\"'\\[]"))

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

; `m.f()` and `m:f()`; the spec resolves `m` when it is a `require`'s alias.
(function_call name: (dot_index_expression table: (identifier) @receiver field: (identifier) @reference.calls))

(function_call name: (method_index_expression table: (identifier) @receiver method: (identifier) @reference.calls))

(comment) @cite.comment

(string) @cite.string
