; A library's top-level declarations and the members of its named types. Dart has no visibility
; keyword: a leading underscore makes a name private to its library, which the spec reads off the
; name. A declaration spans its annotations, so its first line may be one.

[
  (class_declaration name: (identifier) @name)
  (mixin_declaration name: (identifier) @name)
  (extension_declaration name: (identifier) @name)
] @definition.class

; `class M = A with B;` keeps its name inside the mixin application.
(class_declaration (mixin_application_class . (identifier) @name)) @definition.class

(enum_declaration name: (identifier) @name) @definition.enum

(type_alias (type_identifier) @name) @definition.type

[
  (function_declaration signature: (_ name: (identifier) @name))
  (getter_declaration signature: (_ name: (identifier) @name))
  (setter_declaration signature: (_ name: (identifier) @name))
] @definition.function

; `const a = 1, b = 2;` declares two names over one statement.
(top_level_variable_declaration [
  (initialized_identifier_list (initialized_identifier name: (identifier) @name))
  (static_final_declaration_list (static_final_declaration name: (identifier) @name))
]) @definition.variable

; Members of a named type only: nothing a caller writes names an unnamed extension's. A named
; constructor or factory is `Type.name`, as a caller writes it; the unnamed constructor is its class,
; and an operator has no name a caller writes.
(_ name: (identifier) body: (_ (class_member [
  (method_declaration signature: (method_signature [
    (function_signature name: (identifier) @name)
    (getter_signature name: (identifier) @name)
    (setter_signature name: (identifier) @name)
    (constructor_signature name: (identifier) name: (identifier) @name)
    (constant_constructor_signature name: (identifier) name: (identifier) @name)
    (factory_constructor_signature name: (identifier) name: (identifier) @name)
    (redirecting_factory_constructor_signature name: (identifier) name: (identifier) @name)
  ]))
  (declaration [
    (function_signature name: (identifier) @name)
    (getter_signature name: (identifier) @name)
    (setter_signature name: (identifier) @name)
    (constructor_signature name: (identifier) name: (identifier) @name)
    (constant_constructor_signature name: (identifier) name: (identifier) @name)
    (factory_constructor_signature name: (identifier) name: (identifier) @name)
    (redirecting_factory_constructor_signature name: (identifier) name: (identifier) @name)
  ])
] @definition.method)))

(_ name: (identifier) body: (_ (class_member (declaration [
  (initialized_identifier_list (initialized_identifier name: (identifier) @name))
  (static_final_declaration_list (static_final_declaration name: (identifier) @name))
]) @definition.field)))
