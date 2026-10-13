; A file declares what its own top-level statements declare. A declaration with one spec is the
; whole statement, so its doc comment above `type`, `const` or `var` is the spec's; in a group, each
; spec is its own definition with the comment above it.

(source_file (package_clause (package_identifier) @name) @scope.namespace)

(source_file (function_declaration name: (identifier) @name) @definition.function)

; A method is its receiver type's: `func (s *Server) Run()` is `Server.Run`.
(source_file
  (method_declaration
    receiver: (parameter_list
      (parameter_declaration type: [
        (type_identifier) @owner
        (pointer_type (type_identifier) @owner)
        (generic_type type: (type_identifier) @owner)
        (pointer_type (generic_type type: (type_identifier) @owner))
      ]))
    name: (field_identifier) @name) @definition.method)

(source_file
  [
    (type_declaration . (type_spec name: (type_identifier) @name type: (struct_type)) .) @definition.struct
    (type_declaration "(" (type_spec name: (type_identifier) @name type: (struct_type)) @definition.struct)
  ])

(source_file
  [
    (type_declaration . (type_spec name: (type_identifier) @name type: (interface_type)) .) @definition.interface
    (type_declaration "(" (type_spec name: (type_identifier) @name type: (interface_type)) @definition.interface)
  ])

(source_file
  [
    (type_declaration . [(type_spec name: (type_identifier) @name) (type_alias name: (type_identifier) @name)] .) @definition.type
    (type_declaration "(" [(type_spec name: (type_identifier) @name) (type_alias name: (type_identifier) @name)] @definition.type)
  ])

; A struct's fields and an interface's methods are its members. An embedded field has no name and
; declares nothing.
(source_file
  (type_declaration
    (type_spec
      type: (struct_type (field_declaration_list (field_declaration name: (field_identifier) @name) @definition.field)))))

(source_file
  (type_declaration
    (type_spec
      type: (interface_type (method_elem name: (field_identifier) @name) @definition.method))))

; `_` declares nothing: `var _ I = (*T)(nil)` asserts that `T` implements `I`.
((source_file
  [
    (const_declaration . (const_spec name: (identifier) @name) .) @definition.constant
    (const_declaration "(" (const_spec name: (identifier) @name) @definition.constant)
  ])
  (#not-eq? @name "_"))

((source_file
  [
    (var_declaration . (var_spec name: (identifier) @name) .) @definition.variable
    (var_declaration (var_spec_list (var_spec name: (identifier) @name) @definition.variable))
  ])
  (#not-eq? @name "_"))

; `import r "a/render"` binds `r`; with no name, the package's own clause names the import. `.` and
; `_` are kept as written, so no receiver names them.
(import_spec name: (_)? @import.alias path: (_) @import.path) @import
