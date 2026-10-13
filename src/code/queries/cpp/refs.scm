[
  (compound_statement)
  (for_statement)
  (for_range_loop)
  (catch_clause)
] @local.scope

; A parameter binds in the body alone, which the grammar keeps beside the signature.
(function_definition
  declarator: [
    (function_declarator parameters: (parameter_list [
      (parameter_declaration declarator: [
        (identifier) @local.definition
        (pointer_declarator declarator: (identifier) @local.definition)
        (pointer_declarator declarator: (pointer_declarator declarator: (identifier) @local.definition))
        (reference_declarator (identifier) @local.definition)
        (array_declarator declarator: (identifier) @local.definition)
        (function_declarator declarator: (parenthesized_declarator (pointer_declarator declarator: (identifier) @local.definition)))
      ])
      (optional_parameter_declaration declarator: [
        (identifier) @local.definition
        (pointer_declarator declarator: (identifier) @local.definition)
        (reference_declarator (identifier) @local.definition)
      ])
    ]))
    (pointer_declarator declarator: (function_declarator parameters: (parameter_list (parameter_declaration declarator: [
      (identifier) @local.definition
      (pointer_declarator declarator: (identifier) @local.definition)
      (reference_declarator (identifier) @local.definition)
    ]))))
    (reference_declarator (function_declarator parameters: (parameter_list (parameter_declaration declarator: [
      (identifier) @local.definition
      (pointer_declarator declarator: (identifier) @local.definition)
      (reference_declarator (identifier) @local.definition)
    ]))))
  ]
  body: (compound_statement) @local.scope)

; A variable a block or a `for` header declares, bare, initialised, a pointer, a reference or a
; function pointer.
(declaration
  declarator: [
    (identifier) @local.definition
    (pointer_declarator declarator: (identifier) @local.definition)
    (pointer_declarator declarator: (pointer_declarator declarator: (identifier) @local.definition))
    (reference_declarator (identifier) @local.definition)
    (array_declarator declarator: (identifier) @local.definition)
    (array_declarator declarator: (array_declarator declarator: (identifier) @local.definition))
    (pointer_declarator declarator: (array_declarator declarator: (identifier) @local.definition))
    (function_declarator declarator: (parenthesized_declarator (pointer_declarator declarator: (identifier) @local.definition)))
    (init_declarator declarator: [
      (identifier) @local.definition
      (pointer_declarator declarator: (identifier) @local.definition)
      (pointer_declarator declarator: (pointer_declarator declarator: (identifier) @local.definition))
      (reference_declarator (identifier) @local.definition)
      (array_declarator declarator: (identifier) @local.definition)
      (array_declarator declarator: (array_declarator declarator: (identifier) @local.definition))
      (pointer_declarator declarator: (array_declarator declarator: (identifier) @local.definition))
      (function_declarator declarator: (parenthesized_declarator (pointer_declarator declarator: (identifier) @local.definition)))
    ])
  ]
  (#set! within "compound_statement"))

(for_range_loop declarator: [
  (identifier) @local.definition
  (reference_declarator (identifier) @local.definition)
  (pointer_declarator declarator: (identifier) @local.definition)
])

; A call by name, with or without template arguments. In a method, a member of its class is found
; before any function, as the implicit `this` finds it. A member call (`o.f()`, `p->f()`) needs the
; value's type, which nothing here reads.
((call_expression function: [
  (identifier) @reference.calls
  (template_function name: (identifier) @reference.calls)
])
  (#set! receiver "owner"))

; `ns::f()` and `::f()` name `f`, since a namespace qualifies nothing.
(call_expression function: (qualified_identifier name: [
  (identifier) @reference.calls
  (template_function name: (identifier) @reference.calls)
  (qualified_identifier name: (identifier) @reference.calls)
]))

(comment) @cite.comment

[(string_literal) (raw_string_literal)] @cite.string
