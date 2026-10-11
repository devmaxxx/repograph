; A name read anywhere an expression is names this file's declaration of that name. A declaration's
; own name, an object key, a parameter and a loop variable bind rather than read, so no pattern takes
; them. A call needs no pattern of its own to tell a function from a parameter: Bicep refuses a call
; to a name a parameter or variable shadows, so a shadowed built-in is spelled `sys.range(…)`.
[
  (arguments (identifier) @reference.references)
  (array (identifier) @reference.references)
  (assignment_expression (identifier) @reference.references)
  (binary_expression (identifier) @reference.references)
  (unary_expression (identifier) @reference.references)
  (ternary_expression (identifier) @reference.references)
  (member_expression object: (identifier) @reference.references)
  (subscript_expression (identifier) @reference.references)
  (interpolation (identifier) @reference.references)
  (parenthesized_expression (identifier) @reference.references)
  (type (identifier) @reference.references)
  (nullable_type (identifier) @reference.references)
  (union_type (identifier) @reference.references)
  (parameterized_type (identifier) @reference.references)
  (compatible_identifier (identifier) @reference.references)
  (metadata_declaration (identifier) @reference.references)
  (assert_statement (identifier) @reference.references)
  (test_block (identifier) @reference.references)
  (import_statement (identifier) @reference.references)
  (import_with_statement (identifier) @reference.references)
  (import_functionality (identifier) @reference.references)
  (object_property (_) . (identifier) @reference.references)
  (lambda_expression (identifier) @reference.references .)
  (for_statement . (_) (identifier) @reference.references)
  (call_expression function: (identifier) @reference.references)
]

[
  (parameter_declaration . (_) (identifier) @reference.references)
  (variable_declaration . (_) (identifier) @reference.references)
  (resource_declaration . (_) (identifier) @reference.references)
  (module_declaration . (_) (identifier) @reference.references)
  (type_declaration . (_) (identifier) @reference.references)
  (user_defined_function . (_) (identifier) @reference.references)
  (output_declaration . (_) (identifier) @reference.references)
]

; A binder covers what it scopes over and nothing beside it: a loop's variables its body, never the
; iterable; a function's parameters its result type and body, never the parameter types. A lambda's
; parameters are reads only in its body, so the whole lambda is their scope.
(for_statement
  initializer: (identifier) @local.definition
  body: (_) @local.scope)

(for_statement
  (for_loop_parameters (loop_variable) @local.definition)
  body: (_) @local.scope)

(for_statement
  (for_loop_parameters (loop_enumerator) @local.definition)
  body: (_) @local.scope)

(lambda_expression
  .
  [
    (identifier) @local.definition
    (parenthesized_expression (identifier) @local.definition)
  ]) @local.scope

(user_defined_function
  (parameters (parameter . (identifier) @local.definition))
  returns: (_) @local.scope)

(user_defined_function
  (parameters (parameter . (identifier) @local.definition))
  (_) @local.scope
  .)

(comment) @cite.comment

(string) @cite.string
