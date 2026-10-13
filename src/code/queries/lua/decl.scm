; A file declares the functions its own statements define, and those of a `do` block or a branch at
; its level, which a generated module wraps its optional parts in. A function defined in another
; function's body exists once that function has run, so it is not the file's. A `local function` is
; the file's own; any other is reachable once the file is required.

(chunk
  [
    (function_declaration "local" name: (identifier) @name) @definition.function
    (do_statement body: (block (function_declaration "local" name: (identifier) @name) @definition.function))
    (if_statement consequence: (block (function_declaration "local" name: (identifier) @name) @definition.function))
    (if_statement alternative: (elseif_statement consequence: (block (function_declaration "local" name: (identifier) @name) @definition.function)))
    (if_statement alternative: (else_statement body: (block (function_declaration "local" name: (identifier) @name) @definition.function)))
  ]
  (#set! export "never"))

; `function M.f()` and `function M:m()` name their table as the owner, so the id reads `M.f`.
(chunk
  [
    (function_declaration name: (identifier) @name) @definition.function
    (function_declaration name: (dot_index_expression table: (_) @owner field: (identifier) @name)) @definition.function
    (function_declaration name: (method_index_expression table: (_) @owner method: (identifier) @name)) @definition.method
    (assignment_statement
      (variable_list . name: (dot_index_expression table: (_) @owner field: (identifier) @name) .)
      (expression_list . value: (function_definition) .)) @definition.function
  ])

; `function M.f()` in a branch is `M.f` whichever branch runs.
(chunk
  [
    (do_statement body: (block [
        (function_declaration name: (dot_index_expression table: (_) @owner field: (identifier) @name)) @definition.function
        (function_declaration name: (method_index_expression table: (_) @owner method: (identifier) @name)) @definition.method
      ]))
    (if_statement consequence: (block [
        (function_declaration name: (dot_index_expression table: (_) @owner field: (identifier) @name)) @definition.function
        (function_declaration name: (method_index_expression table: (_) @owner method: (identifier) @name)) @definition.method
      ]))
    (if_statement alternative: (elseif_statement consequence: (block [
        (function_declaration name: (dot_index_expression table: (_) @owner field: (identifier) @name)) @definition.function
        (function_declaration name: (method_index_expression table: (_) @owner method: (identifier) @name)) @definition.method
      ])))
    (if_statement alternative: (else_statement body: (block [
        (function_declaration name: (dot_index_expression table: (_) @owner field: (identifier) @name)) @definition.function
        (function_declaration name: (method_index_expression table: (_) @owner method: (identifier) @name)) @definition.method
      ])))
  ])

; `return M` hands the table `M` to every file that requires this one, so a receiver naming the
; import reaches `M`'s members and no other function of the file.
(chunk (return_statement (expression_list . (identifier) @export.root .)))

; `return setmetatable(M, mt)` returns `M` itself.
((chunk (return_statement (expression_list
   . (function_call name: (identifier) @_setmetatable arguments: (arguments . (identifier) @export.root)) .)))
  (#eq? @_setmetatable "setmetatable"))

; `local m = require("a.b")` binds `m`, and so does `m = require("a.b")` on a local declared before.
; A bare `require "a.b"` loads the module for its effects, and `return require("a.b")` hands it on.
; Each shape sits under a different parent, so no site is read twice.
(variable_declaration
  (assignment_statement
    (variable_list . name: (identifier) @import.alias)
    (expression_list
      . value: (function_call
        name: (identifier) @_require
        arguments: (arguments . (string content: (string_content) @import.path) .)) @import))
  (#eq? @_require "require"))

([
  (chunk
    (assignment_statement
      (variable_list . name: (identifier) @import.alias)
      (expression_list
        . value: (function_call
          name: (identifier) @_require
          arguments: (arguments . (string content: (string_content) @import.path) .)) @import)))
  (block
    (assignment_statement
      (variable_list . name: (identifier) @import.alias)
      (expression_list
        . value: (function_call
          name: (identifier) @_require
          arguments: (arguments . (string content: (string_content) @import.path) .)) @import)))
 ]
  (#eq? @_require "require"))

([
  (chunk
    (function_call
      name: (identifier) @_require
      arguments: (arguments . (string content: (string_content) @import.path) .)) @import)
  (block
    (function_call
      name: (identifier) @_require
      arguments: (arguments . (string content: (string_content) @import.path) .)) @import)
  (block
    (return_statement
      (expression_list
        . (function_call
          name: (identifier) @_require
          arguments: (arguments . (string content: (string_content) @import.path) .)) @import .)))
 ]
  (#eq? @_require "require"))

; `return require("a.b")` at the file's level hands its importers `a.b`'s members as its own.
((chunk
   (return_statement
     (expression_list
       . (function_call
         name: (identifier) @_require
         arguments: (arguments . (string content: (string_content) @import.path) .)) @import .)))
  (#eq? @_require "require")
  (#set! reexport))
