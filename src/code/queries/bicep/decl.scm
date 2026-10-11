; A declaration is a symbol by its symbolic name, the identifier after its keyword. Only a file's
; own statements declare: a resource nested in another is the one exception, below.

; A parameter and an output are the module's interface to whoever deploys it. An output has a
; namespace of its own, `output id` beside `param id` is legal, so it is filed under `output/`.
(infrastructure
  (parameter_declaration . (identifier) @name) @definition.value
  (#set! export "always"))

(infrastructure
  (output_declaration . (identifier) @name) @definition.output
  (#set! prefix "output/")
  (#set! export "always"))

; Anything else is exported only when an `@export()` decorator says so. A comment may stand between
; the decorators and the declaration without ending the pair. These come before the plain patterns,
; because the first pattern to match a node decides its export.
(infrastructure
  (decorators (decorator) @_export (#match? @_export "^@(sys\\.)?export\\("))
  .
  (comment)*
  .
  [
    (variable_declaration . (identifier) @name) @definition.variable
    (type_declaration . (identifier) @name) @definition.type
    (user_defined_function name: (identifier) @name) @definition.function
    (module_declaration . (identifier) @name) @definition.value
    (resource_declaration . (identifier) @name) @definition.resource
  ]
  (#set! export "always"))

(infrastructure
  [
    (variable_declaration . (identifier) @name) @definition.variable
    (type_declaration . (identifier) @name) @definition.type
    (user_defined_function name: (identifier) @name) @definition.function
    (module_declaration . (identifier) @name) @definition.value
  ])

(infrastructure
  (resource_declaration . (identifier) @name) @definition.resource)

; A child resource is `<parent>.<child>`, the member its parent keeps. It stands in its parent's
; body, which a condition or a loop may wrap; nothing but a resource holds one.
(resource_declaration
  [
    (object (object_property (resource_declaration . (identifier) @name) @definition.resource))
    (if_statement (object (object_property (resource_declaration . (identifier) @name) @definition.resource)))
    (for_statement (object (object_property (resource_declaration . (identifier) @name) @definition.resource)))
    (for_statement (if_statement (object (object_property (resource_declaration . (identifier) @name) @definition.resource))))
  ])

; `module m './x.bicep'`: a path written out whole is a file of this repository. An interpolated path
; names no one file, a registry or template-spec module (`br:`, `ts:`) lives elsewhere, and Bicep
; resolves no absolute path.
(module_declaration
  .
  (identifier)
  .
  (string . (string_content) @import.path .)
  (#match? @import.path "\\.bicep$")
  (#not-match? @import.path "^(/|(br|ts)[:/])")) @import
