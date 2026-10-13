; A file declares its types, in any namespace, and a type declares its types and its methods,
; constructors, properties, events and fields; a record its positional parameters, and a class or a
; struct its primary constructor's. An enum's members, a delegate's parameters, and destructors,
; operators, conversions and indexers declare nothing: no caller writes their names. Both branches of
; an `#if` declare, since which one compiles is the build's choice.
;
; What crosses a file: `public` and `internal` always, `protected internal` with them; a top-level type
; with no word, being `internal`; an interface's members unless `private` or `protected`. A member or a
; nested type with no word is `private`. Patterns are ordered so the first to match a node decides.

(namespace_declaration name: (_) @name) @scope.namespace
(file_scoped_namespace_declaration name: (_) @name) @scope.namespace

; An interface's members and nested types, directly or in an `#if`.
((_ "interface" body: (declaration_list [
    (method_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.method
    (constructor_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.method
    (property_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.property
    (event_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.property
    (field_declaration (modifier ["public" "internal"]) (variable_declaration (variable_declarator name: (identifier) @name))) @definition.field
    (event_field_declaration (modifier ["public" "internal"]) (variable_declaration (variable_declarator name: (identifier) @name))) @definition.field
    (class_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.class
    (struct_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.struct
    (record_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.record
    (interface_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.interface
    (enum_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.enum
    (delegate_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.type
  ])) (#set! export "always"))
((_ "interface" body: (declaration_list [
    (method_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.method
    (constructor_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.method
    (property_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.property
    (event_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.property
    (field_declaration (modifier ["private" "protected"]) (variable_declaration (variable_declarator name: (identifier) @name))) @definition.field
    (event_field_declaration (modifier ["private" "protected"]) (variable_declaration (variable_declarator name: (identifier) @name))) @definition.field
    (class_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.class
    (struct_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.struct
    (record_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.record
    (interface_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.interface
    (enum_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.enum
    (delegate_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.type
  ])) (#set! export "never"))
((_ "interface" body: (declaration_list [
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
((_ "interface" body: (declaration_list (preproc_if [
    (method_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.method
    (constructor_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.method
    (property_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.property
    (event_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.property
    (field_declaration (modifier ["public" "internal"]) (variable_declaration (variable_declarator name: (identifier) @name))) @definition.field
    (event_field_declaration (modifier ["public" "internal"]) (variable_declaration (variable_declarator name: (identifier) @name))) @definition.field
    (class_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.class
    (struct_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.struct
    (record_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.record
    (interface_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.interface
    (enum_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.enum
    (delegate_declaration (modifier ["public" "internal"]) name: (identifier) @name) @definition.type
  ]))) (#set! export "always"))
((_ "interface" body: (declaration_list (preproc_if [
    (method_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.method
    (constructor_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.method
    (property_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.property
    (event_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.property
    (field_declaration (modifier ["private" "protected"]) (variable_declaration (variable_declarator name: (identifier) @name))) @definition.field
    (event_field_declaration (modifier ["private" "protected"]) (variable_declaration (variable_declarator name: (identifier) @name))) @definition.field
    (class_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.class
    (struct_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.struct
    (record_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.record
    (interface_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.interface
    (enum_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.enum
    (delegate_declaration (modifier ["private" "protected"]) name: (identifier) @name) @definition.type
  ]))) (#set! export "never"))
((_ "interface" body: (declaration_list (preproc_if [
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
  ]))) (#set! export "always"))

; A class's, a struct's and a record's nested types, directly or in an `#if`.
((_ ["class" "struct" "record"] body: (declaration_list [
    (class_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.class
    (struct_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.struct
    (record_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.record
    (interface_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.interface
    (enum_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.enum
    (delegate_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.type
  ])))
((_ ["class" "struct" "record"] body: (declaration_list (preproc_if [
    (class_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.class
    (struct_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.struct
    (record_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.record
    (interface_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.interface
    (enum_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.enum
    (delegate_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.type
  ]))))

; Positional parameters: a record's are public properties, a class's or a struct's private captures.
(record_declaration (parameter_list (parameter name: (identifier) @name) @definition.property) (#set! export "always"))
(_ ["class" "struct"] (parameter_list (parameter name: (identifier) @name) @definition.property) (#set! export "never"))

; Every other member, and every type left, which is a top-level one.
([
    (method_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.method
    (constructor_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.method
    (property_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.property
    (event_declaration (modifier ["public" "internal"] @modifier)? name: (identifier) @name) @definition.property
    (field_declaration (modifier ["public" "internal"] @modifier)? (variable_declaration (variable_declarator name: (identifier) @name))) @definition.field
    (event_field_declaration (modifier ["public" "internal"] @modifier)? (variable_declaration (variable_declarator name: (identifier) @name))) @definition.field
  ])
([
    (class_declaration (modifier ["private" "file"]) name: (identifier) @name) @definition.class
    (struct_declaration (modifier ["private" "file"]) name: (identifier) @name) @definition.struct
    (record_declaration (modifier ["private" "file"]) name: (identifier) @name) @definition.record
    (interface_declaration (modifier ["private" "file"]) name: (identifier) @name) @definition.interface
    (enum_declaration (modifier ["private" "file"]) name: (identifier) @name) @definition.enum
    (delegate_declaration (modifier ["private" "file"]) name: (identifier) @name) @definition.type
  ] (#set! export "never"))
([
    (class_declaration name: (identifier) @name) @definition.class
    (struct_declaration name: (identifier) @name) @definition.struct
    (record_declaration name: (identifier) @name) @definition.record
    (interface_declaration name: (identifier) @name) @definition.interface
    (enum_declaration name: (identifier) @name) @definition.enum
    (delegate_declaration name: (identifier) @name) @definition.type
  ] (#set! export "always"))
