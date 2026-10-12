; A file and each inline `mod` declare their items, a trait its members, and an `impl` the members of
; its type. Nothing in a function body is declared, nor in a `mod` nested more than three deep: a
; pattern that may start anywhere walks every function body to find out. An inline `mod` writes no
; symbol: it names what it holds, `tests/helper`, as a path does. Any `pub` exports an item,
; `pub(crate)` too, since what the crate sees is what a change reaches.

; `#[cfg(unix)] fn f` beside `#[cfg(windows)] fn f` is one item built two ways, so the first's span
; covers both, whatever attributes stand between the `cfg` and the item. Ahead of the items below,
; so it wins the node.
(source_file
  (attribute_item (attribute (identifier) @_cfg))
  .
  [(attribute_item) (line_comment)]*
  .
  [
    (function_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.function
    (struct_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.struct
    (enum_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.enum
    (type_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.type
    (const_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.constant
    (static_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.variable
  ]
  (#eq? @_cfg "cfg")
  (#set! variant))

(mod_item body: (declaration_list
  (attribute_item (attribute (identifier) @_cfg))
  .
  [(attribute_item) (line_comment)]*
  .
  [
    (function_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.function
    (struct_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.struct
    (enum_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.enum
    (type_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.type
    (const_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.constant
    (static_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.variable
  ]
  (#eq? @_cfg "cfg")
  (#set! variant))
  (#set! depth "5"))

(source_file [
  (function_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.function
  (function_signature_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.function
  (macro_definition name: (identifier) @name) @definition.function
  (struct_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.struct
  (union_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.struct
  (enum_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.enum
  (type_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.type
  (const_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.constant
  (static_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.variable
  (trait_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.trait
  (mod_item name: (identifier) @name body: (_)) @scope.module
])

(mod_item body: (declaration_list [
  (function_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.function
  (function_signature_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.function
  (macro_definition name: (identifier) @name) @definition.function
  (struct_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.struct
  (union_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.struct
  (enum_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.enum
  (type_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.type
  (const_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.constant
  (static_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.variable
  (trait_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.trait
  (mod_item name: (identifier) @name body: (_)) @scope.module
])
  (#set! depth "5"))

; A trait's members are reached through the trait, never exported by the file.
((source_file (trait_item body: (declaration_list [
  (function_item name: (identifier) @name) @definition.method
  (function_signature_item name: (identifier) @name) @definition.method
  (associated_type name: (type_identifier) @name) @definition.type
  (const_item name: (identifier) @name) @definition.constant
  (type_item name: (type_identifier) @name) @definition.type
])))
  (#set! export "never"))

((mod_item body: (declaration_list (trait_item body: (declaration_list [
  (function_item name: (identifier) @name) @definition.method
  (function_signature_item name: (identifier) @name) @definition.method
  (associated_type name: (type_identifier) @name) @definition.type
  (const_item name: (identifier) @name) @definition.constant
  (type_item name: (type_identifier) @name) @definition.type
]))))
  (#set! export "never")
  (#set! depth "5"))

; An `impl` names its members on its type's last segment, without generic arguments or a reference.
; A type this file declares adopts them; one declared elsewhere leaves them to the file, exported
; when `pub`, since this file is where they are written. An `impl` for a slice, a tuple or a
; function type has no name to hang members on, and declares nothing.
((source_file (impl_item type: [
    (type_identifier) @name
    (primitive_type) @name
    (scoped_type_identifier name: (type_identifier) @name)
    (generic_type type: [(type_identifier) @name (scoped_type_identifier name: (type_identifier) @name)])
    (reference_type type: [
      (type_identifier) @name
      (primitive_type) @name
      (scoped_type_identifier name: (type_identifier) @name)
      (generic_type type: [(type_identifier) @name (scoped_type_identifier name: (type_identifier) @name)])
    ])
  ]) @scope.owner)
  (#set! adopt))

((mod_item body: (declaration_list (impl_item type: [
    (type_identifier) @name
    (primitive_type) @name
    (scoped_type_identifier name: (type_identifier) @name)
    (generic_type type: [(type_identifier) @name (scoped_type_identifier name: (type_identifier) @name)])
    (reference_type type: [
      (type_identifier) @name
      (primitive_type) @name
      (scoped_type_identifier name: (type_identifier) @name)
      (generic_type type: [(type_identifier) @name (scoped_type_identifier name: (type_identifier) @name)])
    ])
  ]) @scope.owner))
  (#set! adopt)
  (#set! depth "5"))

(source_file (impl_item
  type: [
    (type_identifier)
    (primitive_type)
    (scoped_type_identifier)
    (generic_type)
    (reference_type type: [(type_identifier) (primitive_type) (scoped_type_identifier) (generic_type)])
  ]
  body: (declaration_list [
    (function_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.method
    (function_signature_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.method
    (associated_type name: (type_identifier) @name) @definition.type
    (const_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.constant
    (type_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.type
  ])))

(mod_item body: (declaration_list (impl_item
  type: [
    (type_identifier)
    (primitive_type)
    (scoped_type_identifier)
    (generic_type)
    (reference_type type: [(type_identifier) (primitive_type) (scoped_type_identifier) (generic_type)])
  ]
  body: (declaration_list [
    (function_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.method
    (function_signature_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.method
    (associated_type name: (type_identifier) @name) @definition.type
    (const_item (visibility_modifier "pub" @modifier)? name: (identifier) @name) @definition.constant
    (type_item (visibility_modifier "pub" @modifier)? name: (type_identifier) @name) @definition.type
  ])))
  (#set! depth "5"))