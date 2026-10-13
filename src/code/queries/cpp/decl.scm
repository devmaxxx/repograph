; A file declares what C's queries declare, and its classes with their members. A namespace qualifies
; nothing: every file reopens it, and a call inside it names its functions bare. A method declared and
; defined apart is declared where its body is, under the class its `X::` names; a pure virtual one,
; which has no body, where it is declared. Any other prototype declares nothing.

; A method whose body sits in its class. A constructor is named by an identifier, a destructor and an
; operator by their own nodes; a static one is no less a member, so no modifier is read. A deleted or
; defaulted one has no body and declares nothing, and a class a function body declares is a local.
; Members sit at any depth in the class, since `#if` wraps some.
((function_definition
  declarator: [
    (function_declarator declarator: [(field_identifier) (identifier) (destructor_name) (operator_name)] @name)
    (pointer_declarator declarator: (function_declarator declarator: [(field_identifier) (operator_name)] @name))
    (pointer_declarator declarator: (pointer_declarator declarator: (function_declarator declarator: (field_identifier) @name)))
    (reference_declarator (function_declarator declarator: [(field_identifier) (operator_name)] @name))
  ]
  body: (_)) @definition.method
  (#set! within "field_declaration_list")
  (#set! outside "friend_declaration")
  (#set! outside "compound_statement"))

; A friend defined in its class is a function of the enclosing namespace, not a member.
((friend_declaration
  (function_definition
    declarator: [
      (function_declarator declarator: [(identifier) (operator_name)] @name)
      (pointer_declarator declarator: (function_declarator declarator: [(identifier) (operator_name)] @name))
      (reference_declarator (function_declarator declarator: [(identifier) (operator_name)] @name))
    ]
    body: (_)) @definition.function)
  (#set! parent "none")
  (#set! outside "compound_statement"))

; `virtual f() = 0` has no body anywhere: the declaration is the method.
((field_declaration
  declarator: [
    (function_declarator declarator: [(field_identifier) (destructor_name) (operator_name)] @name)
    (pointer_declarator declarator: (function_declarator declarator: [(field_identifier) (operator_name)] @name))
    (reference_declarator (function_declarator declarator: [(field_identifier) (operator_name)] @name))
  ]
  default_value: (number_literal)) @definition.method
  (#set! outside "compound_statement"))

; `macro T f(…) { … }` is a function: the parser recovers it as `T::f` with a `::` it inserts, so it
; comes before the out-of-line method, which would read `T` as the class.
((function_definition
  declarator: (function_declarator declarator: (qualified_identifier
    scope: (namespace_identifier)
    (MISSING "::")
    name: (identifier) @name))
  body: (_)) @definition.function
  (#set! outside "field_declaration_list")
  (#set! outside "compound_statement"))

; `R A::f() { … }`, `A::~A()`, `A<T>::f` and `A<T>::B::f`: the class is the scope nearest the name, and
; a template around it is a class too. A plain name around it may be a namespace, which qualifies
; nothing, so `ns::A::f` and `A::B::f` both read `A.f` and `B.f`.
(function_definition
  declarator: [
    (function_declarator declarator: [
      (qualified_identifier
        scope: [(namespace_identifier) @owner (template_type name: (type_identifier) @owner)]
        name: [(identifier) (destructor_name) (operator_name)] @name)
      (qualified_identifier
        scope: (namespace_identifier)
        name: (qualified_identifier
          scope: [(namespace_identifier) @owner (template_type name: (type_identifier) @owner)]
          name: [(identifier) (destructor_name) (operator_name)] @name))
      (qualified_identifier
        scope: (template_type name: (type_identifier) @owner)
        name: (qualified_identifier
          scope: [(namespace_identifier) @owner (template_type name: (type_identifier) @owner)]
          name: [(identifier) (destructor_name) (operator_name)] @name))
    ])
    (pointer_declarator declarator: (function_declarator declarator: [
      (qualified_identifier
        scope: [(namespace_identifier) @owner (template_type name: (type_identifier) @owner)]
        name: [(identifier) (operator_name)] @name)
      (qualified_identifier
        scope: (namespace_identifier)
        name: (qualified_identifier
          scope: [(namespace_identifier) @owner (template_type name: (type_identifier) @owner)]
          name: [(identifier) (operator_name)] @name))
      (qualified_identifier
        scope: (template_type name: (type_identifier) @owner)
        name: (qualified_identifier
          scope: [(namespace_identifier) @owner (template_type name: (type_identifier) @owner)]
          name: [(identifier) (operator_name)] @name))
    ]))
    (pointer_declarator declarator: (pointer_declarator declarator: (function_declarator declarator:
      (qualified_identifier
        scope: [(namespace_identifier) @owner (template_type name: (type_identifier) @owner)]
        name: (identifier) @name))))
    (reference_declarator (function_declarator declarator: [
      (qualified_identifier
        scope: [(namespace_identifier) @owner (template_type name: (type_identifier) @owner)]
        name: [(identifier) (operator_name)] @name)
      (qualified_identifier
        scope: (namespace_identifier)
        name: (qualified_identifier
          scope: [(namespace_identifier) @owner (template_type name: (type_identifier) @owner)]
          name: [(identifier) (operator_name)] @name))
      (qualified_identifier
        scope: (template_type name: (type_identifier) @owner)
        name: (qualified_identifier
          scope: [(namespace_identifier) @owner (template_type name: (type_identifier) @owner)]
          name: [(identifier) (operator_name)] @name))
    ]))
  ] body: (_)) @definition.method

; A function of an unnamed namespace is the file's own, as a `static` one is.
((namespace_definition
  !name
  body: (declaration_list
    (function_definition
      declarator: [
        (function_declarator declarator: [(identifier) (operator_name)] @name)
        (pointer_declarator declarator: (function_declarator declarator: [(identifier) (operator_name)] @name))
        (reference_declarator (function_declarator declarator: [(identifier) (operator_name)] @name))
      ] body: (_)) @definition.function))
  (#set! export "never"))

((function_definition
  (storage_class_specifier)* @modifier
  declarator: [
    (function_declarator declarator: [(identifier) (operator_name)] @name)
    (pointer_declarator declarator: (function_declarator declarator: [(identifier) (operator_name)] @name))
    (pointer_declarator declarator: (pointer_declarator declarator: (function_declarator declarator: (identifier) @name)))
    (reference_declarator (function_declarator declarator: [(identifier) (operator_name)] @name))
  ] body: (_)) @definition.function
  (#set! outside "field_declaration_list")
  (#set! outside "compound_statement"))

(preproc_function_def name: (identifier) @name) @definition.function

; `typedef struct x { … } y;` is one type, named `y`, as C reads it.
((type_definition
  type: [(struct_specifier body: (_)) (union_specifier body: (_))]
  declarator: (type_identifier) @name) @definition.struct
  (#set! outside "compound_statement"))

((type_definition
  type: (enum_specifier body: (_))
  declarator: (type_identifier) @name) @definition.enum
  (#set! outside "compound_statement"))

; `struct A::Impl { … }` defines the nested type its class declared.
([
  (class_specifier
    name: (qualified_identifier
      scope: [(namespace_identifier) @owner (template_type name: (type_identifier) @owner)]
      name: (type_identifier) @name)
    body: (_)) @definition.class
  (struct_specifier
    name: (qualified_identifier
      scope: [(namespace_identifier) @owner (template_type name: (type_identifier) @owner)]
      name: (type_identifier) @name)
    body: (_)) @definition.struct
]
  (#set! outside "compound_statement"))

; A class or struct with a body, nested in another or not. One written in a variable's declaration is
; the variable's shape, and one a function body declares is a local.
([
  (class_specifier name: (type_identifier) @name body: (_)) @definition.class
  (struct_specifier name: (type_identifier) @name body: (_)) @definition.struct
  (union_specifier name: (type_identifier) @name body: (_)) @definition.struct
]
  (#set! outside "type_definition")
  (#set! outside "declaration")
  (#set! outside "compound_statement"))

((enum_specifier name: (type_identifier) @name body: (_)) @definition.enum
  (#set! outside "type_definition")
  (#set! outside "declaration")
  (#set! outside "compound_statement"))

((alias_declaration name: (type_identifier) @name) @definition.type
  (#set! outside "compound_statement"))

; A typedef of anything else, as C reads it.
((type_definition
  declarator: [
    (type_identifier) @name
    (primitive_type) @name
    (pointer_declarator declarator: (type_identifier) @name)
    (function_declarator declarator: (type_identifier) @name)
    (function_declarator declarator: (parenthesized_declarator (type_identifier) @name))
    (function_declarator declarator: (parenthesized_declarator (pointer_declarator declarator: (type_identifier) @name)))
    (pointer_declarator declarator: (function_declarator declarator: (parenthesized_declarator (pointer_declarator declarator: (type_identifier) @name))))
  ]) @definition.type
  (#set! outside "compound_statement"))

; Data members. One declarator is the declaration's; with several, each is its own.
((field_declaration
  type: (_) .
  declarator: [
    (field_identifier) @name
    (pointer_declarator declarator: (field_identifier) @name)
    (pointer_declarator declarator: (pointer_declarator declarator: (field_identifier) @name))
    (reference_declarator (field_identifier) @name)
    (array_declarator declarator: (field_identifier) @name)
    (array_declarator declarator: (array_declarator declarator: (field_identifier) @name))
    (pointer_declarator declarator: (array_declarator declarator: (field_identifier) @name))
    (array_declarator declarator: (parenthesized_declarator (pointer_declarator declarator: (field_identifier) @name)))
    (function_declarator declarator: (parenthesized_declarator (pointer_declarator declarator: (field_identifier) @name)))
    (pointer_declarator declarator: (function_declarator declarator: (parenthesized_declarator (pointer_declarator declarator: (field_identifier) @name))))
  ] .) @definition.field
  (#set! outside "declaration")
  (#set! outside "compound_statement"))

((field_declaration
  declarator: [
    (field_identifier) @name
    (pointer_declarator declarator: (field_identifier) @name)
    (pointer_declarator declarator: (pointer_declarator declarator: (field_identifier) @name))
    (reference_declarator (field_identifier) @name)
    (array_declarator declarator: (field_identifier) @name)
    (array_declarator declarator: (array_declarator declarator: (field_identifier) @name))
    (pointer_declarator declarator: (array_declarator declarator: (field_identifier) @name))
  ] @definition.field)
  (#set! outside "declaration")
  (#set! outside "compound_statement"))

((enumerator_list (enumerator name: (identifier) @name) @definition.constant)
  (#set! outside "compound_statement"))

(preproc_include path: (string_literal) @import.path) @import
