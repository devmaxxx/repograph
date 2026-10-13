[
  (function_declaration)
  (method_declaration)
  (func_literal)
  (block)
  (if_statement)
  (for_statement)
  (expression_switch_statement)
  (type_switch_statement)
  (select_statement)
  (expression_case)
  (type_case)
  (default_case)
  (communication_case)
] @local.scope

; A parameter binds in the body alone: its type is read where the signature stands, so in
; `func mapping(setter setter)` the second `setter` is the package's type.
([
  (function_declaration [parameters: (parameter_list [
      (parameter_declaration name: (identifier) @local.definition)
      (variadic_parameter_declaration name: (identifier) @local.definition)])
    result: (parameter_list (parameter_declaration name: (identifier) @local.definition))]
    body: (block) @local.scope)
  (method_declaration [parameters: (parameter_list [
      (parameter_declaration name: (identifier) @local.definition)
      (variadic_parameter_declaration name: (identifier) @local.definition)])
    result: (parameter_list (parameter_declaration name: (identifier) @local.definition))]
    body: (block) @local.scope)
  (func_literal [parameters: (parameter_list [
      (parameter_declaration name: (identifier) @local.definition)
      (variadic_parameter_declaration name: (identifier) @local.definition)])
    result: (parameter_list (parameter_declaration name: (identifier) @local.definition))]
    body: (block) @local.scope)
])

; The receiver of `func (s *Server) Run()` is the method's type, so `s.handle()` is `Server.handle`.
((method_declaration
  receiver: (parameter_list (parameter_declaration name: (identifier) @local.definition))
  body: (block) @local.scope)
  (#set! receiver "owner"))

(short_var_declaration left: (expression_list (identifier) @local.definition))

(range_clause left: (expression_list (identifier) @local.definition))

(receive_statement left: (expression_list (identifier) @local.definition))

(type_switch_statement alias: (expression_list (identifier) @local.definition))

(statement_list (var_declaration [
  (var_spec name: (identifier) @local.definition)
  (var_spec_list (var_spec name: (identifier) @local.definition))
]))

(statement_list (const_declaration (const_spec name: (identifier) @local.definition)))

(statement_list (type_declaration [
  (type_spec name: (type_identifier) @local.definition)
  (type_alias name: (type_identifier) @local.definition)
]))

; A type parameter binds in its function, and a generic type's in the type alone.
(function_declaration type_parameters: (type_parameter_list (type_parameter_declaration name: (identifier) @local.definition)))

((type_spec type_parameters: (type_parameter_list (type_parameter_declaration name: (identifier) @local.definition))) @local.scope)

((type_alias type_parameters: (type_parameter_list (type_parameter_declaration name: (identifier) @local.definition))) @local.scope)

(call_expression function: (identifier) @reference.calls)

; `getTyped[bool](c, key)`: with more than one argument, the grammar reads an instantiated call as an
; index. With one, it reads a conversion of a generic type, and that stays a type use.
(call_expression function: (index_expression operand: (identifier) @reference.calls))

(call_expression function: (index_expression operand: (selector_expression operand: (identifier) @receiver field: (field_identifier) @reference.calls)))

; `pkg.F()` through an import, or `s.M()` through a method's receiver; on any other value it names
; nothing, since its type is unread.
(call_expression function: (selector_expression operand: (identifier) @receiver field: (field_identifier) @reference.calls))

; `var _ I = (*T)(nil)` asserts that `T` implements `I`. The grammar reads the parenthesised type as
; an expression, since only the call tells it apart.
((call_expression function: (parenthesized_expression (unary_expression operator: "*" operand: (identifier) @reference.references)))
  (#set! context "type"))

((call_expression function: (parenthesized_expression (unary_expression operator: "*" operand: (selector_expression operand: (identifier) @receiver field: (field_identifier) @reference.references))))
  (#set! context "type"))

(call_expression function: (parenthesized_expression (identifier) @reference.calls))

((qualified_type package: (package_identifier) @receiver name: (type_identifier) @reference.references)
  (#set! context "type"))

; A type named where a type is written. A declaration's own name and a qualified type's member are
; no use of a bare name.
([
  (argument_list (type_identifier) @reference.references)
  (array_type element: (type_identifier) @reference.references)
  (channel_type value: (type_identifier) @reference.references)
  (composite_literal type: (type_identifier) @reference.references)
  (const_spec type: (type_identifier) @reference.references)
  (field_declaration type: (type_identifier) @reference.references)
  (func_literal result: (type_identifier) @reference.references)
  (function_declaration result: (type_identifier) @reference.references)
  (function_type result: (type_identifier) @reference.references)
  (generic_type type: (type_identifier) @reference.references)
  (implicit_length_array_type element: (type_identifier) @reference.references)
  (map_type key: (type_identifier) @reference.references)
  (map_type value: (type_identifier) @reference.references)
  (method_declaration result: (type_identifier) @reference.references)
  (method_elem result: (type_identifier) @reference.references)
  (negated_type (type_identifier) @reference.references)
  (parameter_declaration type: (type_identifier) @reference.references)
  (parenthesized_type (type_identifier) @reference.references)
  (pointer_type (type_identifier) @reference.references)
  (slice_type element: (type_identifier) @reference.references)
  (type_alias type: (type_identifier) @reference.references)
  (type_assertion_expression type: (type_identifier) @reference.references)
  (type_case type: (type_identifier) @reference.references)
  (type_constraint (type_identifier) @reference.references)
  (type_conversion_expression type: (type_identifier) @reference.references)
  (type_elem (type_identifier) @reference.references)
  (type_instantiation_expression type: (type_identifier) @reference.references)
  (type_spec type: (type_identifier) @reference.references)
  (var_spec type: (type_identifier) @reference.references)
  (variadic_parameter_declaration type: (type_identifier) @reference.references)
 ]
  (#set! context "type"))

(comment) @cite.comment

[(interpreted_string_literal) (raw_string_literal)] @cite.string
