; A file declares its classes, objects, type aliases, functions and properties, and a type declares
; the same in its body, with a primary constructor's `val` and `var` parameters among them. A
; companion declares nothing of its own: a caller writes `Outer.make()`, so its members take the
; class's path. Nothing a function body, an initializer, an object literal or an enum entry holds
; is declared. `private` and `protected` keep a name to its file.

(source_file (package_header (identifier) @name) @scope.namespace)

; Unread by the engine: `package_scope` resolves an import through the family's index.
(source_file (import_list (import_header (identifier) @import.path [(import_alias (type_identifier) @import.alias) (wildcard_import) @import.name]?) @import))

(source_file
  [
    (class_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.class
    (object_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.object
    (type_alias (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.type
    (function_declaration (modifiers (visibility_modifier) @modifier)? (simple_identifier) @name) @definition.function
    (property_declaration (modifiers (visibility_modifier) @modifier)? (variable_declaration (simple_identifier) @name)) @definition.property
    (class_declaration (primary_constructor (class_parameter (modifiers (visibility_modifier) @modifier)? (binding_pattern_kind) (simple_identifier) @name) @definition.property))
    ; `object Keys { val TOKEN = 1 }` on one line reads as an infix call on an empty object literal.
    ((infix_expression . (object_literal) @_object . (simple_identifier) @name . (lambda_literal) .) @definition.object
      (#eq? @_object "object"))
  ])

; The one-line object's members, as a lambda's statements.
(source_file
  (infix_expression . (object_literal) @_object . (simple_identifier) . (lambda_literal (statements
    [
      (class_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.class
      (object_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.object
      (type_alias (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.type
      (function_declaration (modifiers (visibility_modifier) @modifier)? (simple_identifier) @name) @definition.method
      (property_declaration (modifiers (visibility_modifier) @modifier)? (variable_declaration (simple_identifier) @name)) @definition.property
      (class_declaration (primary_constructor (class_parameter (modifiers (visibility_modifier) @modifier)? (binding_pattern_kind) (simple_identifier) @name) @definition.property))
    ])) .)
  (#eq? @_object "object"))

; A type's members by how deep it nests: a query cannot recurse. A class or an object is a named node
; holding a body, which an enum entry and an object literal are not.
(source_file
  (_ (type_identifier) (_ 
    [
      (class_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.class
      (object_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.object
      (type_alias (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.type
      (function_declaration (modifiers (visibility_modifier) @modifier)? (simple_identifier) @name) @definition.method
      (property_declaration (modifiers (visibility_modifier) @modifier)? (variable_declaration (simple_identifier) @name)) @definition.property
      (class_declaration (primary_constructor (class_parameter (modifiers (visibility_modifier) @modifier)? (binding_pattern_kind) (simple_identifier) @name) @definition.property))
      (companion_object (class_body [
        (class_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.class
        (object_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.object
        (type_alias (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.type
        (function_declaration (modifiers (visibility_modifier) @modifier)? (simple_identifier) @name) @definition.method
        (property_declaration (modifiers (visibility_modifier) @modifier)? (variable_declaration (simple_identifier) @name)) @definition.property
        (class_declaration (primary_constructor (class_parameter (modifiers (visibility_modifier) @modifier)? (binding_pattern_kind) (simple_identifier) @name) @definition.property))
      ]))
    ])))

(source_file
  (_ (type_identifier) (_ (_ (type_identifier) (_ 
    [
      (class_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.class
      (object_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.object
      (type_alias (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.type
      (function_declaration (modifiers (visibility_modifier) @modifier)? (simple_identifier) @name) @definition.method
      (property_declaration (modifiers (visibility_modifier) @modifier)? (variable_declaration (simple_identifier) @name)) @definition.property
      (class_declaration (primary_constructor (class_parameter (modifiers (visibility_modifier) @modifier)? (binding_pattern_kind) (simple_identifier) @name) @definition.property))
      (companion_object (class_body [
        (class_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.class
        (object_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.object
        (type_alias (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.type
        (function_declaration (modifiers (visibility_modifier) @modifier)? (simple_identifier) @name) @definition.method
        (property_declaration (modifiers (visibility_modifier) @modifier)? (variable_declaration (simple_identifier) @name)) @definition.property
        (class_declaration (primary_constructor (class_parameter (modifiers (visibility_modifier) @modifier)? (binding_pattern_kind) (simple_identifier) @name) @definition.property))
      ]))
    ])))))

(source_file
  (_ (type_identifier) (_ (_ (type_identifier) (_ (_ (type_identifier) (_ 
    [
      (class_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.class
      (object_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.object
      (type_alias (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.type
      (function_declaration (modifiers (visibility_modifier) @modifier)? (simple_identifier) @name) @definition.method
      (property_declaration (modifiers (visibility_modifier) @modifier)? (variable_declaration (simple_identifier) @name)) @definition.property
      (class_declaration (primary_constructor (class_parameter (modifiers (visibility_modifier) @modifier)? (binding_pattern_kind) (simple_identifier) @name) @definition.property))
      (companion_object (class_body [
        (class_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.class
        (object_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.object
        (type_alias (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.type
        (function_declaration (modifiers (visibility_modifier) @modifier)? (simple_identifier) @name) @definition.method
        (property_declaration (modifiers (visibility_modifier) @modifier)? (variable_declaration (simple_identifier) @name)) @definition.property
        (class_declaration (primary_constructor (class_parameter (modifiers (visibility_modifier) @modifier)? (binding_pattern_kind) (simple_identifier) @name) @definition.property))
      ]))
    ])))))))

; A type declared in a companion is one level deeper than the class around it, and the walk above
; counts only bodies of types. Its members, as the class's own members are.
(source_file
  (_ (type_identifier) (_ (companion_object (class_body (_ (type_identifier) (_ 
    [
      (class_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.class
      (object_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.object
      (type_alias (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.type
      (function_declaration (modifiers (visibility_modifier) @modifier)? (simple_identifier) @name) @definition.method
      (property_declaration (modifiers (visibility_modifier) @modifier)? (variable_declaration (simple_identifier) @name)) @definition.property
      (class_declaration (primary_constructor (class_parameter (modifiers (visibility_modifier) @modifier)? (binding_pattern_kind) (simple_identifier) @name) @definition.property))
      (companion_object (class_body [
        (class_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.class
        (object_declaration (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.object
        (type_alias (modifiers (visibility_modifier) @modifier)? (type_identifier) @name) @definition.type
        (function_declaration (modifiers (visibility_modifier) @modifier)? (simple_identifier) @name) @definition.method
        (property_declaration (modifiers (visibility_modifier) @modifier)? (variable_declaration (simple_identifier) @name)) @definition.property
        (class_declaration (primary_constructor (class_parameter (modifiers (visibility_modifier) @modifier)? (binding_pattern_kind) (simple_identifier) @name) @definition.property))
      ]))
    ]
    )))))))
