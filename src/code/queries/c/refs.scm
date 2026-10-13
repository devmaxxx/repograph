[
  (compound_statement)
  (for_statement)
] @local.scope

; A parameter binds in the body alone, which the grammar keeps beside the signature: a function
; pointer parameter `cb` hides a global `cb` from the calls the body makes through it.
(function_definition
  declarator: [
    (function_declarator parameters: (parameter_list (parameter_declaration declarator: [
      (identifier) @local.definition
      (pointer_declarator declarator: (identifier) @local.definition)
      (pointer_declarator declarator: (pointer_declarator declarator: (identifier) @local.definition))
      (array_declarator declarator: (identifier) @local.definition)
      (function_declarator declarator: (parenthesized_declarator (pointer_declarator declarator: (identifier) @local.definition)))
    ])))
    (pointer_declarator declarator: (function_declarator parameters: (parameter_list (parameter_declaration declarator: [
      (identifier) @local.definition
      (pointer_declarator declarator: (identifier) @local.definition)
      (pointer_declarator declarator: (pointer_declarator declarator: (identifier) @local.definition))
      (array_declarator declarator: (identifier) @local.definition)
      (function_declarator declarator: (parenthesized_declarator (pointer_declarator declarator: (identifier) @local.definition)))
    ]))))
  ]
  body: (compound_statement) @local.scope)

; A variable a block or a `for` header declares, bare, initialised, a pointer or a function pointer.
(declaration
  declarator: [
    (identifier) @local.definition
    (pointer_declarator declarator: (identifier) @local.definition)
    (array_declarator declarator: (identifier) @local.definition)
    (function_declarator declarator: (parenthesized_declarator (pointer_declarator declarator: (identifier) @local.definition)))
    (init_declarator declarator: [
      (identifier) @local.definition
      (pointer_declarator declarator: (identifier) @local.definition)
      (pointer_declarator declarator: (pointer_declarator declarator: (identifier) @local.definition))
      (array_declarator declarator: (identifier) @local.definition)
      (function_declarator declarator: (parenthesized_declarator (pointer_declarator declarator: (identifier) @local.definition)))
    ])
  ]
  (#set! within "compound_statement"))

(call_expression function: (identifier) @reference.calls)

(comment) @cite.comment

(string_literal) @cite.string
