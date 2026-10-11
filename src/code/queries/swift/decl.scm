; A file declares its own statements' types, functions and `let` constants, and a type declares its
; methods and the types nested in it. Nothing in a function body declares, nor does an actor, a
; property, an initializer or an enum case. A `private` or `fileprivate` word hides a declaration;
; `private(set)` restricts its setter alone.

; `extension Box<Int>` names its members on `Box`. A pattern of its own, ahead of the one below, so it
; wins the node; a path with arguments mid-way, `Box<Int>.Inner`, keeps them.
(source_file
  (class_declaration declaration_kind: "extension" name: (user_type . (type_identifier) @name . (type_arguments) .)) @scope.owner)

(source_file
  [
    (class_declaration (modifiers (visibility_modifier)* @modifier)? declaration_kind: "class" name: (type_identifier) @name) @definition.class
    (class_declaration (modifiers (visibility_modifier)* @modifier)? declaration_kind: "struct" name: (type_identifier) @name) @definition.struct
    (class_declaration (modifiers (visibility_modifier)* @modifier)? declaration_kind: "enum" name: (type_identifier) @name) @definition.enum
    (protocol_declaration (modifiers (visibility_modifier)* @modifier)? name: (type_identifier) @name) @definition.protocol
    (function_declaration (modifiers (visibility_modifier)* @modifier)? "func" . name: _ @name) @definition.function
    ; One match per bound name, so `let a = 1, b = 2` declares both, each over the whole statement.
    (property_declaration
      (modifiers (visibility_modifier)* @modifier)?
      (value_binding_pattern) @_let
      name: (pattern bound_identifier: (simple_identifier) @name)
      (#eq? @_let "let")) @definition.constant
    ; An extension declares no type: its members are named on the type it extends, as a caller writes
    ; them, and the file declares them. `extension [Int]` names its type by the sugar it is written in.
    (class_declaration declaration_kind: "extension" name: (_) @name) @scope.owner
  ])

; A protocol's requirements, and the members of a type by how deep it nests. A query cannot recurse,
; so a type nested inside four others declares nothing of its own.
(source_file
  (protocol_declaration body: (protocol_body (protocol_function_declaration (modifiers (visibility_modifier)* @modifier)? "func" . name: _ @name) @definition.method)))

(source_file
  (class_declaration declaration_kind: ["class" "struct" "enum" "extension"] body: (_
    [
      (function_declaration (modifiers (visibility_modifier)* @modifier)? "func" . name: _ @name) @definition.method
      (class_declaration (modifiers (visibility_modifier)* @modifier)? declaration_kind: "class" name: (type_identifier) @name) @definition.class
      (class_declaration (modifiers (visibility_modifier)* @modifier)? declaration_kind: "struct" name: (type_identifier) @name) @definition.struct
      (class_declaration (modifiers (visibility_modifier)* @modifier)? declaration_kind: "enum" name: (type_identifier) @name) @definition.enum
      (protocol_declaration (modifiers (visibility_modifier)* @modifier)? name: (type_identifier) @name) @definition.protocol
      (protocol_declaration body: (protocol_body (protocol_function_declaration (modifiers (visibility_modifier)* @modifier)? "func" . name: _ @name) @definition.method))
    ])))

(source_file
  (class_declaration declaration_kind: ["class" "struct" "enum" "extension"] body: (_
    (class_declaration declaration_kind: ["class" "struct" "enum"] body: (_
      [
        (function_declaration (modifiers (visibility_modifier)* @modifier)? "func" . name: _ @name) @definition.method
        (class_declaration (modifiers (visibility_modifier)* @modifier)? declaration_kind: "class" name: (type_identifier) @name) @definition.class
        (class_declaration (modifiers (visibility_modifier)* @modifier)? declaration_kind: "struct" name: (type_identifier) @name) @definition.struct
        (class_declaration (modifiers (visibility_modifier)* @modifier)? declaration_kind: "enum" name: (type_identifier) @name) @definition.enum
        (protocol_declaration (modifiers (visibility_modifier)* @modifier)? name: (type_identifier) @name) @definition.protocol
        (protocol_declaration body: (protocol_body (protocol_function_declaration (modifiers (visibility_modifier)* @modifier)? "func" . name: _ @name) @definition.method))
      ])))))

(source_file
  (class_declaration declaration_kind: ["class" "struct" "enum" "extension"] body: (_
    (class_declaration declaration_kind: ["class" "struct" "enum"] body: (_
      (class_declaration declaration_kind: ["class" "struct" "enum"] body: (_
        [
          (function_declaration (modifiers (visibility_modifier)* @modifier)? "func" . name: _ @name) @definition.method
          (class_declaration (modifiers (visibility_modifier)* @modifier)? declaration_kind: "class" name: (type_identifier) @name) @definition.class
          (class_declaration (modifiers (visibility_modifier)* @modifier)? declaration_kind: "struct" name: (type_identifier) @name) @definition.struct
          (class_declaration (modifiers (visibility_modifier)* @modifier)? declaration_kind: "enum" name: (type_identifier) @name) @definition.enum
          (protocol_declaration (modifiers (visibility_modifier)* @modifier)? name: (type_identifier) @name) @definition.protocol
          (protocol_declaration body: (protocol_body (protocol_function_declaration (modifiers (visibility_modifier)* @modifier)? "func" . name: _ @name) @definition.method))
        ])))))))

(source_file
  (class_declaration declaration_kind: ["class" "struct" "enum" "extension"] body: (_
    (class_declaration declaration_kind: ["class" "struct" "enum"] body: (_
      (class_declaration declaration_kind: ["class" "struct" "enum"] body: (_
        (class_declaration declaration_kind: ["class" "struct" "enum"] body: (_
          [
            (function_declaration (modifiers (visibility_modifier)* @modifier)? "func" . name: _ @name) @definition.method
            (class_declaration (modifiers (visibility_modifier)* @modifier)? declaration_kind: "class" name: (type_identifier) @name) @definition.class
            (class_declaration (modifiers (visibility_modifier)* @modifier)? declaration_kind: "struct" name: (type_identifier) @name) @definition.struct
            (class_declaration (modifiers (visibility_modifier)* @modifier)? declaration_kind: "enum" name: (type_identifier) @name) @definition.enum
            (protocol_declaration (modifiers (visibility_modifier)* @modifier)? name: (type_identifier) @name) @definition.protocol
            (protocol_declaration body: (protocol_body (protocol_function_declaration (modifiers (visibility_modifier)* @modifier)? "func" . name: _ @name) @definition.method))
          ])))))))))
