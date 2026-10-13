; Locals adapted from the grammar's `locals.scm`:
; Copyright (c) 2020 Max Brunsfeld and Pieter Goetschalckx, MIT License,
; https://github.com/tree-sitter/tree-sitter-ocaml

[
  (let_binding)
  (class_binding)
  (class_function)
  (method_definition)
  (fun_expression)
  (object_expression)
  (for_expression)
  (match_case)
] @local.scope

(value_pattern) @local.definition

; `{ idx; hash }` as a pattern binds `idx` and `hash` with no pattern node of their own.
(field_pattern (field_path (field_name) @local.definition) !pattern)

; A `let … in` binds in its whole expression, past the binding that holds the name.
(let_expression (value_definition (let_binding pattern: [
    (value_name) @local.definition
    (_ (value_name) @local.definition)
    (_ (_ (value_name) @local.definition))
  ]))) @local.scope

; A functor's parameter is a module no file names.
(module_binding (module_parameter (module_name) @local.definition)) @local.scope

; What is applied is called, and so is the right of `|>` and the left of `@@`. A path's modules are
; its receiver: `Foo.Bar.f` is `f` of `Foo.Bar`.
(application_expression function: [
    (value_path . (value_name) @reference.calls .)
    (value_path (module_path) @receiver (value_name) @reference.calls)
  ])

((infix_expression operator: (_) @_op right: [
    (value_path . (value_name) @reference.calls .)
    (value_path (module_path) @receiver (value_name) @reference.calls)
  ])
  (#eq? @_op "|>"))

((infix_expression left: [
    (value_path . (value_name) @reference.calls .)
    (value_path (module_path) @receiver (value_name) @reference.calls)
  ] operator: (_) @_op)
  (#eq? @_op "@@"))

; Any other value path is a reference. Each is found by what holds it, so that no path is read twice:
; under one of these fields wherever it stands, beside `::` or `#`, or as an element of a sequence.
(_ argument: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(_ body: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(_ condition: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(_ default: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(_ expression: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(_ from: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(_ in: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(_ index: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(_ initializer: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(_ object: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(_ record: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(_ sequence: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(_ to: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(_ value: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(cons_expression left: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(cons_expression right: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(hash_expression left: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(hash_expression right: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(array_expression [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(comprehension_guard [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(do_clause [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(expression_item [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(iarray_expression [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(list_expression [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(sequence_expression [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(toplevel_directive [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
(tuple_expression [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
((infix_expression left: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)] operator: (_) @_op)
  (#not-eq? @_op "@@"))
((infix_expression operator: (_) @_op right: [(value_path . (value_name) @reference.references .) (value_path (module_path) @receiver (value_name) @reference.references)])
  (#not-eq? @_op "|>"))

; A type's modules are its receiver too.
((type_constructor_path . (type_constructor) @reference.references .)
  (#set! context "type"))
((type_constructor_path (extended_module_path) @receiver (type_constructor) @reference.references)
  (#set! context "type"))

(comment) @cite.comment

[(string) (quoted_string)] @cite.string
