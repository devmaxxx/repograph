; A file declares its classes, traits, objects and enums with their members, and the definitions a
; package object or the top level holds. A type holds its members, so a method is `Path.root`; the
; package qualifies nothing, since the index keys a name by it apart from the id. What a function
; body, a value's initialiser, an anonymous class or a given holds is local, not the file's.

(package_clause name: (package_identifier) @name) @scope.namespace

; A package object's members are its package's own, as `os.pwd` names the one in `package object os`.
(package_object name: (identifier) @name) @scope.namespace

; An access modifier hides a member from other files; one with a qualifier, as `private[os]`, keeps it
; visible across its package.
([
  (class_definition (modifiers (access_modifier) @modifier)? name: (_) @name) @definition.class
  (trait_definition (modifiers (access_modifier) @modifier)? name: (_) @name) @definition.trait
  (object_definition (modifiers (access_modifier) @modifier)? name: (_) @name) @definition.object
  (enum_definition (modifiers (access_modifier) @modifier)? name: (_) @name) @definition.enum
]
  (#set! outside "function_definition")
  (#set! outside "block")
  (#set! outside "lambda_expression")
  (#set! outside "instance_expression")
  (#set! outside "val_definition")
  (#set! outside "var_definition")
  (#set! outside "given_definition")
  (#set! outside "extension_definition"))

; What a package object or the top level holds is its package's: a function, not a method.
(compilation_unit [
  (function_definition (modifiers (access_modifier) @modifier)? name: (_) @name) @definition.function
  (function_declaration (modifiers (access_modifier) @modifier)? name: (_) @name) @definition.function
  (val_definition (modifiers (access_modifier) @modifier)? pattern: (identifier) @name) @definition.value
  (var_definition (modifiers (access_modifier) @modifier)? pattern: (identifier) @name) @definition.variable
  (type_definition (modifiers (access_modifier) @modifier)? name: (type_identifier) @name) @definition.type
])

(package_object body: (template_body [
  (function_definition (modifiers (access_modifier) @modifier)? name: (_) @name) @definition.function
  (function_declaration (modifiers (access_modifier) @modifier)? name: (_) @name) @definition.function
  (val_definition (modifiers (access_modifier) @modifier)? pattern: (identifier) @name) @definition.value
  (val_declaration (modifiers (access_modifier) @modifier)? name: (identifier) @name) @definition.value
  (var_definition (modifiers (access_modifier) @modifier)? pattern: (identifier) @name) @definition.variable
  (type_definition (modifiers (access_modifier) @modifier)? name: (type_identifier) @name) @definition.type
]))

; A member is what a type's body holds directly; a `val` is a property, as a caller reads it alike. An
; auxiliary constructor, `def this(...)`, is the class's, which `new` names.
([
  (class_definition body: (template_body [
    (function_definition (modifiers (access_modifier) @modifier)? name: (_) @name) @definition.method
    (function_declaration (modifiers (access_modifier) @modifier)? name: (_) @name) @definition.method
    (val_definition (modifiers (access_modifier) @modifier)? pattern: (identifier) @name) @definition.property
    (val_declaration (modifiers (access_modifier) @modifier)? name: (identifier) @name) @definition.property
    (var_definition (modifiers (access_modifier) @modifier)? pattern: (identifier) @name) @definition.property
    (var_declaration (modifiers (access_modifier) @modifier)? name: (identifier) @name) @definition.property
    (type_definition (modifiers (access_modifier) @modifier)? name: (type_identifier) @name) @definition.type
  ]))
  (object_definition body: (template_body [
    (function_definition (modifiers (access_modifier) @modifier)? name: (_) @name) @definition.method
    (function_declaration (modifiers (access_modifier) @modifier)? name: (_) @name) @definition.method
    (val_definition (modifiers (access_modifier) @modifier)? pattern: (identifier) @name) @definition.property
    (val_declaration (modifiers (access_modifier) @modifier)? name: (identifier) @name) @definition.property
    (var_definition (modifiers (access_modifier) @modifier)? pattern: (identifier) @name) @definition.property
    (var_declaration (modifiers (access_modifier) @modifier)? name: (identifier) @name) @definition.property
    (type_definition (modifiers (access_modifier) @modifier)? name: (type_identifier) @name) @definition.type
  ]))
  (trait_definition body: (template_body [
    (function_definition (modifiers (access_modifier) @modifier)? name: (_) @name) @definition.method
    (function_declaration (modifiers (access_modifier) @modifier)? name: (_) @name) @definition.method
    (val_definition (modifiers (access_modifier) @modifier)? pattern: (identifier) @name) @definition.property
    (val_declaration (modifiers (access_modifier) @modifier)? name: (identifier) @name) @definition.property
    (var_definition (modifiers (access_modifier) @modifier)? pattern: (identifier) @name) @definition.property
    (var_declaration (modifiers (access_modifier) @modifier)? name: (identifier) @name) @definition.property
    (type_definition (modifiers (access_modifier) @modifier)? name: (type_identifier) @name) @definition.type
  ]))
  (enum_definition body: (enum_body [
    (function_definition (modifiers (access_modifier) @modifier)? name: (_) @name) @definition.method
    (val_definition (modifiers (access_modifier) @modifier)? pattern: (identifier) @name) @definition.property
  ]))
]
  (#not-eq? @name "this")
  (#set! outside "function_definition")
  (#set! outside "block")
  (#set! outside "lambda_expression")
  (#set! outside "instance_expression")
  (#set! outside "val_definition")
  (#set! outside "var_definition")
  (#set! outside "given_definition")
  (#set! outside "extension_definition"))

((enum_body (enum_case_definitions [
  (simple_enum_case name: (identifier) @name) @definition.constant
  (full_enum_case name: (identifier) @name) @definition.constant
]))
  (#set! outside "function_definition")
  (#set! outside "block"))

; `import a.b.C` binds `C`; `import a.b.{C, D}` binds each selector, and `import a.b._` opens `a.b`.
; A renamed selector (`D => E`) binds a name no declaration has, and `D => _` hides one, so neither is
; read. The path is the run of its parts, its first and its last captured, which the engine reads as
; the text they span.
(import_declaration . (identifier) @import.path [(identifier) (operator_identifier)] @import.path .) @import

[
  (import_declaration . (identifier) @import.path . (namespace_wildcard) @import.name @import)
  (import_declaration . (identifier) @import.path (identifier) @import.path . (namespace_wildcard) @import.name @import)
]

[
  (import_declaration . (identifier) @import.path .
    (namespace_selectors [(identifier) (operator_identifier) (namespace_wildcard)] @import.name @import))
  (import_declaration . (identifier) @import.path (identifier) @import.path .
    (namespace_selectors [(identifier) (operator_identifier) (namespace_wildcard)] @import.name @import))
]
