; Locals adapted from the grammar's `locals.scm`:
; Copyright (c) 2016 Rob Rix, MIT License, https://github.com/tree-sitter/tree-sitter-ruby

[
  (method)
  (singleton_method)
  (lambda)
  (block)
  (do_block)
  (class)
  (module)
  (singleton_class)
] @local.scope

(block_parameter (identifier) @local.definition)
(block_parameters (identifier) @local.definition)
(destructured_parameter (identifier) @local.definition)
(hash_splat_parameter (identifier) @local.definition)
(lambda_parameters (identifier) @local.definition)
(method_parameters (identifier) @local.definition)
(splat_parameter (identifier) @local.definition)

(keyword_parameter name: (identifier) @local.definition)
(optional_parameter name: (identifier) @local.definition)

(assignment left: (identifier) @local.definition)
(operator_assignment left: (identifier) @local.definition)
(left_assignment_list (identifier) @local.definition)
(rest_assignment (identifier) @local.definition)
(destructured_left_assignment (identifier) @local.definition)

(exception_variable (identifier) @local.definition)
(for pattern: (identifier) @local.definition)

; A method called without a receiver is the caller's class's, or the file's: another file that
; reopens the class is no witness to which one runs. A bare word no local binds is such a call where
; a value stands; a method's own name, a parameter and the method of a call are none.
((call !receiver method: [(identifier) (constant)] @reference.calls)
  (#set! visibility "file"))

(([
  (program (identifier) @reference.calls)
  (body_statement (identifier) @reference.calls)
  (block_body (identifier) @reference.calls)
  (begin (identifier) @reference.calls)
  (then (identifier) @reference.calls)
  (else (identifier) @reference.calls)
  (ensure (identifier) @reference.calls)
  (do (identifier) @reference.calls)
  (parenthesized_statements (identifier) @reference.calls)
  (argument_list (identifier) @reference.calls)
  (array (identifier) @reference.calls)
  (interpolation (identifier) @reference.calls)
  (element_reference (identifier) @reference.calls)
  (binary (identifier) @reference.calls)
  (unary (identifier) @reference.calls)
  (range (identifier) @reference.calls)
  (conditional (identifier) @reference.calls)
  (pair (identifier) @reference.calls)
  (splat_argument (identifier) @reference.calls)
  (hash_splat_argument (identifier) @reference.calls)
  (block_argument (identifier) @reference.calls)
  (right_assignment_list (identifier) @reference.calls)
  (if_modifier (identifier) @reference.calls)
  (unless_modifier (identifier) @reference.calls)
  (while_modifier (identifier) @reference.calls)
  (until_modifier (identifier) @reference.calls)
  (rescue_modifier (identifier) @reference.calls)
  (assignment right: (identifier) @reference.calls)
  (operator_assignment right: (identifier) @reference.calls)
  (optional_parameter value: (identifier) @reference.calls)
  (keyword_parameter value: (identifier) @reference.calls)
  (if condition: (identifier) @reference.calls)
  (unless condition: (identifier) @reference.calls)
  (elsif condition: (identifier) @reference.calls)
  (while condition: (identifier) @reference.calls)
  (until condition: (identifier) @reference.calls)
  (case value: (identifier) @reference.calls)
  (case_match value: (identifier) @reference.calls)
  (pattern (identifier) @reference.calls)
  (in (identifier) @reference.calls)
  (method body: (identifier) @reference.calls)
  (singleton_method body: (identifier) @reference.calls)
  (call receiver: (identifier) @reference.calls)
])
  (#set! visibility "file"))

((call receiver: (self) @receiver method: [(identifier) (constant)] @reference.calls)
  (#set! receiver "owner"))

; `Utils.escape` is the path `Utils/escape`, read as a constant is: a module's function, or a class's
; member. A call on any other value names nothing, since its type is unread.
(call
  receiver: [
    (constant) @name
    (scope_resolution !scope name: (constant) @name)
    (scope_resolution scope: (constant) @name name: (constant) @name)
    (scope_resolution scope: (scope_resolution !scope name: (constant) @name) name: (constant) @name)
    (scope_resolution scope: (scope_resolution scope: (constant) @name name: (constant) @name) name: (constant) @name)
    (scope_resolution scope: (scope_resolution scope: (scope_resolution scope: (constant) @name name: (constant) @name) name: (constant) @name) name: (constant) @name)
  ]
  method: [(identifier) (constant)] @name) @reference.calls

; A constant where a value stands, and a path `A::B::C` as `A/B/C` wherever it stands. A path's
; inner part is none, and a superclass is read below. A path that is a constant's or a class's own
; name reaches the definition itself, which writes no edge.
([
  (program (constant) @reference.references)
  (body_statement (constant) @reference.references)
  (block_body (constant) @reference.references)
  (begin (constant) @reference.references)
  (then (constant) @reference.references)
  (else (constant) @reference.references)
  (ensure (constant) @reference.references)
  (do (constant) @reference.references)
  (parenthesized_statements (constant) @reference.references)
  (argument_list (constant) @reference.references)
  (array (constant) @reference.references)
  (interpolation (constant) @reference.references)
  (element_reference (constant) @reference.references)
  (binary (constant) @reference.references)
  (unary (constant) @reference.references)
  (range (constant) @reference.references)
  (conditional (constant) @reference.references)
  (pair (constant) @reference.references)
  (splat_argument (constant) @reference.references)
  (hash_splat_argument (constant) @reference.references)
  (block_argument (constant) @reference.references)
  (right_assignment_list (constant) @reference.references)
  (if_modifier (constant) @reference.references)
  (unless_modifier (constant) @reference.references)
  (while_modifier (constant) @reference.references)
  (until_modifier (constant) @reference.references)
  (rescue_modifier (constant) @reference.references)
  (exceptions (constant) @reference.references)
  (pattern (constant) @reference.references)
  (assignment right: (constant) @reference.references)
  (operator_assignment right: (constant) @reference.references)
  (optional_parameter value: (constant) @reference.references)
  (keyword_parameter value: (constant) @reference.references)
  (if condition: (constant) @reference.references)
  (unless condition: (constant) @reference.references)
  (elsif condition: (constant) @reference.references)
  (while condition: (constant) @reference.references)
  (until condition: (constant) @reference.references)
  (case value: (constant) @reference.references)
  (case_match value: (constant) @reference.references)
  (in (constant) @reference.references)
  (method body: (constant) @reference.references)
  (singleton_method body: (constant) @reference.references)
  (call receiver: (constant) @reference.references)
])

((_ [
    (scope_resolution !scope name: (constant) @name)
    (scope_resolution scope: (constant) @name name: (constant) @name)
    (scope_resolution scope: (scope_resolution !scope name: (constant) @name) name: (constant) @name)
    (scope_resolution scope: (scope_resolution scope: (constant) @name name: (constant) @name) name: (constant) @name)
    (scope_resolution scope: (scope_resolution scope: (scope_resolution scope: (constant) @name name: (constant) @name) name: (constant) @name) name: (constant) @name)
  ] @reference.references)
  (#set! outside "scope_resolution")
  (#set! outside "superclass"))

; A superclass is read where its class is declared, outside it.
(superclass [
    (constant) @name
    (scope_resolution !scope name: (constant) @name)
    (scope_resolution scope: (constant) @name name: (constant) @name)
    (scope_resolution scope: (scope_resolution !scope name: (constant) @name) name: (constant) @name)
    (scope_resolution scope: (scope_resolution scope: (constant) @name name: (constant) @name) name: (constant) @name)
  ] @reference.extends)

(comment) @cite.comment

[(string) (heredoc_body)] @cite.string
