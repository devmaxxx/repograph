; A file declares its classes, interfaces, traits and enums with their methods, properties, constants
; and cases, and its functions. A class holds its members, so a method is `Route.run`; the namespace
; qualifies nothing, since code names a class by its short name once a `use` brings it in. What a
; function body or an anonymous class holds is no declaration of the file, but a class or function
; an `if` declares at the top level is.

(namespace_definition name: (namespace_name) @name) @scope.namespace

; A type names what a receiver reaches through its import: `Route::create()` through `use …\Route`.
([
  (class_declaration name: (name) @name @export.root) @definition.class
  (interface_declaration name: (name) @name @export.root) @definition.interface
  (trait_declaration name: (name) @name @export.root) @definition.trait
  (enum_declaration name: (name) @name @export.root) @definition.enum
  (function_definition name: (name) @name) @definition.function
]
  (#set! outside "method_declaration")
  (#set! outside "function_definition")
  (#set! outside "anonymous_function")
  (#set! outside "arrow_function"))

; A member's visibility is its modifier, and one written with none is public. A property's name is
; written without its `$`, as `$this->name` reads it. The last property or constant of a declaration
; is the declaration, so the doc comment above it is the member's; one before it is its own element.
([
  (method_declaration (visibility_modifier)? @modifier name: (name) @name) @definition.method
  (property_declaration (visibility_modifier)? @modifier (property_element name: (variable_name (name) @name)) .) @definition.property
  (property_declaration
    (visibility_modifier)? @modifier
    (property_element name: (variable_name (name) @name)) @definition.property
    .
    (property_element))
  (const_declaration (visibility_modifier)? @modifier (const_element (name) @name) .) @definition.constant
  (const_declaration (visibility_modifier)? @modifier (const_element (name) @name) @definition.constant . (const_element))
  (enum_case name: (name) @name) @definition.constant
]
  (#set! outside "method_declaration")
  (#set! outside "function_definition")
  (#set! outside "anonymous_function")
  (#set! outside "arrow_function")
  (#set! outside "anonymous_class"))

; A constructor's promoted parameter is a property of its class, written inside the constructor.
((property_promotion_parameter visibility: (visibility_modifier) @modifier name: (variable_name (name) @name)) @definition.property
  (#set! outside "anonymous_class"))

; `use A\B\C;` binds `C`, and `use A\B\C as D;` binds `D`. A `use function` or `use const` names no
; class file, and a group `use A\{B, C}` spells each path in two parts, which no capture joins.
(namespace_use_declaration !type !body
  (namespace_use_clause !type [(qualified_name (name) @import.alias) (name) @import.alias] @import.path !alias) @import)

(namespace_use_declaration !type !body
  (namespace_use_clause !type [(qualified_name) (name)] @import.path alias: (name) @import.alias) @import)
