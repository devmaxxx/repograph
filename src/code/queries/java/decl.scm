; A file declares its types, and a type declares its types, methods, constructors, annotation
; elements and fields in its body, an enum's after its constants. A constructor is named by its type,
; so it is `Type.Type`. Nothing an enum constant's body, an anonymous class or a method body holds is
; declared, nor a record's components. Only `public` crosses a package; an interface's and an annotation
; type's members are public without the word, so the container's keyword is their modifier, and a
; private interface method is the one exception.

(program (package_declaration [(scoped_identifier) (identifier)] @name) @scope.namespace)

; Unread by the engine: `package_scope` resolves an import through the family's index. `static`
; is kept among the names, since a static import names a member and not a type.
(program (import_declaration "static"? @import.name [(scoped_identifier) (identifier)] @import.path (asterisk)? @import.name) @import)

; Ahead of the levels below, so a private interface method keeps its first match.
(program (_ "interface" body: (_ (method_declaration (modifiers "private") name: (identifier) @name) @definition.method)) (#set! export "never"))
(program (_ ["class" "interface" "enum" "record" "@interface"] body: (_ (_ "interface" body: (_ (method_declaration (modifiers "private") name: (identifier) @name) @definition.method)))) (#set! export "never"))
(program (_ ["class" "interface" "enum" "record" "@interface"] body: (_ (_ ["class" "interface" "enum" "record" "@interface"] body: (_ (_ "interface" body: (_ (method_declaration (modifiers "private") name: (identifier) @name) @definition.method)))))) (#set! export "never"))

(program (_ (modifiers "public" @modifier)? ["class" "interface" "enum" "record" "@interface"] name: (identifier) @name) @definition.class)

; A type's members by how deep it nests: a query cannot recurse. Each step names a type by its keyword,
; so no step passes through a method, an enum constant or an anonymous class. A type four deep declares
; itself and not its members: each further level costs compile time on every build, and no input holds
; one. A step into an enum's declarations is taken at the two shallow levels only, below.
(program
  (_ ["class" "enum" "record" "interface" @modifier "@interface" @modifier] body: (_ [
    (_ (modifiers "public" @modifier)? ["class" "interface" "enum" "record" "@interface"] name: (identifier) @name) @definition.class
    (method_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
    (constructor_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
    (annotation_type_element_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
    (field_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
    (constant_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
    (enum_body_declarations [
      (_ (modifiers "public" @modifier)? ["class" "interface" "enum" "record" "@interface"] name: (identifier) @name) @definition.class
      (method_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
      (constructor_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
      (annotation_type_element_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
      (field_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
      (constant_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
    ])
  ])))

(program
  (_ ["class" "interface" "enum" "record" "@interface"] body: (_ (_ ["class" "enum" "record" "interface" @modifier "@interface" @modifier] body: (_ [
    (_ (modifiers "public" @modifier)? ["class" "interface" "enum" "record" "@interface"] name: (identifier) @name) @definition.class
    (method_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
    (constructor_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
    (annotation_type_element_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
    (field_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
    (constant_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
    (enum_body_declarations [
      (_ (modifiers "public" @modifier)? ["class" "interface" "enum" "record" "@interface"] name: (identifier) @name) @definition.class
      (method_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
      (constructor_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
      (annotation_type_element_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
      (field_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
      (constant_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
    ])
  ])))))

; A type nested in an enum reaches its own members through the enum's declarations, a step the levels
; above do not take: once from a top-level enum, once from an enum nested in a type.
(program
  (_ body: (enum_body (enum_body_declarations (_ ["class" "enum" "record" "interface" @modifier "@interface" @modifier] body: (_ [
    (_ (modifiers "public" @modifier)? ["class" "interface" "enum" "record" "@interface"] name: (identifier) @name) @definition.class
    (method_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
    (constructor_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
    (annotation_type_element_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
    (field_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
    (constant_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
    (enum_body_declarations [
      (_ (modifiers "public" @modifier)? ["class" "interface" "enum" "record" "@interface"] name: (identifier) @name) @definition.class
      (method_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
      (constructor_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
      (annotation_type_element_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
      (field_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
      (constant_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
    ])
  ]))))))

(program
  (_ ["class" "interface" "enum" "record" "@interface"] body: (_ (_ body: (enum_body (enum_body_declarations (_ ["class" "enum" "record" "interface" @modifier "@interface" @modifier] body: (_ [
    (_ (modifiers "public" @modifier)? ["class" "interface" "enum" "record" "@interface"] name: (identifier) @name) @definition.class
    (method_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
    (constructor_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
    (annotation_type_element_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
    (field_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
    (constant_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
    (enum_body_declarations [
      (_ (modifiers "public" @modifier)? ["class" "interface" "enum" "record" "@interface"] name: (identifier) @name) @definition.class
      (method_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
      (constructor_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
      (annotation_type_element_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
      (field_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
      (constant_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
    ])
  ]))))))))

(program
  (_ ["class" "interface" "enum" "record" "@interface"] body: (_ (_ ["class" "interface" "enum" "record" "@interface"] body: (_ (_ ["class" "enum" "record" "interface" @modifier "@interface" @modifier] body: (_ [
    (_ (modifiers "public" @modifier)? ["class" "interface" "enum" "record" "@interface"] name: (identifier) @name) @definition.class
    (method_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
    (constructor_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
    (annotation_type_element_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
    (field_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
    (constant_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
    (enum_body_declarations [
      (_ (modifiers "public" @modifier)? ["class" "interface" "enum" "record" "@interface"] name: (identifier) @name) @definition.class
      (method_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
      (constructor_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
      (annotation_type_element_declaration (modifiers "public" @modifier)? name: (identifier) @name) @definition.method
      (field_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
      (constant_declaration (modifiers "public" @modifier)? declarator: (variable_declarator name: (identifier) @name)) @definition.field
    ])
  ])))))))
