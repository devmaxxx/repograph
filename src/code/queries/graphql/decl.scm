; A document declares its named operations, its fragments, its directives and every type it defines or
; extends, and a type declares the fields of its object, interface or input. An anonymous operation
; declares nothing, nor does an enum value or an argument.

; The operation type is part of the name, so a query and a mutation of one name stay apart.
(operation_definition (operation_type) @name (name) @name) @definition.function

((fragment_definition (fragment_name (name) @name)) @definition.function
  (#set! prefix "fragment/"))

((directive_definition (description)? @doc (name) @name) @definition.function
  (#set! prefix "directive/"))

(type_definition (_ (description)? @doc (name) @name) @definition.type)

(type_extension (_ (name) @name) @definition.type)

(fields_definition (field_definition (description)? @doc (name) @name) @definition.field)

(input_fields_definition (input_value_definition (description)? @doc (name) @name) @definition.field)
