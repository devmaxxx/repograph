; A file declares its types, in any namespace, and a type declares its types and its methods,
; constructors, properties, events and fields; a record its positional parameters, and a class or a
; struct its primary constructor's. An enum's members, a delegate's parameters, and destructors,
; operators, conversions and indexers declare nothing: no caller writes their names. Both branches of
; an `#if` declare, since which one compiles is the build's choice.
;
; What crosses a file: `public` and `internal` always, `protected internal` with them; a top-level type
; with no word, being `internal`; an interface's members unless `private` or `protected`, and its
; types unless `private`. A member or a nested type with no word is `private`. Patterns are ordered
; so the first to match a node decides.
;
; Each pattern that reads a default from the body around a declaration costs the compile 2 to 15 ms,
; a tenth of a second for the lot when an `#if` inside a body had patterns of its own. So under an
; `#if` in an interface or a type body the word alone decides, as it does for an interface's
; `protected` type: no input writes either. A type in the first branch of an `#if` gets the two
; patterns it needs, 7 ms, since the catch-all exported a nested `private` one; under `#else` or
; `#elif` it still falls to the catch-all.

(namespace_declaration name: (_) @name) @scope.namespace
(file_scoped_namespace_declaration name: (_) @name) @scope.namespace

; A type written `private` or `file`, at any depth.
([
    (class_declaration (modifier ["private" "file"]) name: (identifier) @name) @definition.class
    (struct_declaration (modifier ["private" "file"]) name: (identifier) @name) @definition.struct
    (record_declaration (modifier ["private" "file"]) name: (identifier) @name) @definition.record
    (interface_declaration (modifier ["private" "file"]) name: (identifier) @name) @definition.interface
    (enum_declaration (modifier ["private" "file"]) name: (identifier) @name) @definition.enum
    (delegate_declaration (modifier ["private" "file"]) name: (identifier) @name) @definition.type
  ] (#set! export "never"))

; An interface's members and nested types.
((interface_declaration body: (declaration_list [
    (method_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.method
    (constructor_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.method
    (property_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.property
    (event_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.property
    (field_declaration (modifier ["private" "protected"]) (variable_declaration (variable_declarator name: (identifier) @name))) @definition.field
    (event_field_declaration (modifier ["private" "protected"]) (variable_declaration (variable_declarator name: (identifier) @name))) @definition.field
  ])) (#set! export "never"))
((interface_declaration body: (declaration_list [
    (method_declaration name: (identifier) @name) @definition.method
    (constructor_declaration name: (identifier) @name) @definition.method
    (property_declaration name: (identifier) @name) @definition.property
    (event_declaration name: (identifier) @name) @definition.property
    (field_declaration (variable_declaration (variable_declarator name: (identifier) @name))) @definition.field
    (event_field_declaration (variable_declaration (variable_declarator name: (identifier) @name))) @definition.field
    (class_declaration name: (identifier) @name) @definition.class
    (struct_declaration name: (identifier) @name) @definition.struct
    (record_declaration name: (identifier) @name) @definition.record
    (interface_declaration name: (identifier) @name) @definition.interface
    (enum_declaration name: (identifier) @name) @definition.enum
    (delegate_declaration name: (identifier) @name) @definition.type
  ])) (#set! export "always"))

; A block namespace's own types, which are top-level; then a type in any other body, which is nested.
((namespace_declaration body: (declaration_list [
    (class_declaration name: (identifier) @name) @definition.class
    (struct_declaration name: (identifier) @name) @definition.struct
    (record_declaration name: (identifier) @name) @definition.record
    (interface_declaration name: (identifier) @name) @definition.interface
    (enum_declaration name: (identifier) @name) @definition.enum
    (delegate_declaration name: (identifier) @name) @definition.type
  ])) (#set! export "always"))
((namespace_declaration body: (declaration_list (preproc_if [
    (class_declaration name: (identifier) @name) @definition.class
    (struct_declaration name: (identifier) @name) @definition.struct
    (record_declaration name: (identifier) @name) @definition.record
    (interface_declaration name: (identifier) @name) @definition.interface
    (enum_declaration name: (identifier) @name) @definition.enum
    (delegate_declaration name: (identifier) @name) @definition.type
  ]))) (#set! export "always"))
(declaration_list [
    (class_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.class
    (struct_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.struct
    (record_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.record
    (interface_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.interface
    (enum_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.enum
    (delegate_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.type
  ])
(declaration_list (preproc_if [
    (class_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.class
    (struct_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.struct
    (record_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.record
    (interface_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.interface
    (enum_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.enum
    (delegate_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.type
  ]))

; Positional parameters: a record's are public properties, a class's or a struct's private captures.
(record_declaration (parameter_list (parameter name: (identifier) @name) @definition.property) (#set! export "always"))
([(class_declaration (parameter_list (parameter name: (identifier) @name) @definition.property)) (struct_declaration (parameter_list (parameter name: (identifier) @name) @definition.property))] (#set! export "never"))

; Every other member, and every type left, which is at the file or in a file-scoped namespace.
([
    (method_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.method
    (constructor_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.method
    (property_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.property
    (event_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.property
    (field_declaration (modifier ["public" "internal"] @modifier)? (variable_declaration (variable_declarator name: (identifier) @name))) @definition.field
    (event_field_declaration (modifier ["public" "internal"] @modifier)? (variable_declaration (variable_declarator name: (identifier) @name))) @definition.field
  ])
([
    (class_declaration name: (identifier) @name) @definition.class
    (struct_declaration name: (identifier) @name) @definition.struct
    (record_declaration name: (identifier) @name) @definition.record
    (interface_declaration name: (identifier) @name) @definition.interface
    (enum_declaration name: (identifier) @name) @definition.enum
    (delegate_declaration name: (identifier) @name) @definition.type
  ] (#set! export "always"))
