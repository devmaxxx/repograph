; A file declares the functions it defines, its tagged and typedef'd types with their members, and its
; function-like macros. A prototype declares nothing: the definition is the symbol, wherever its
; prototype stands. Definitions sit at any depth, since `#ifdef` branches and `extern "C" { … }` hold
; most of a header, but a type a function body declares is a local, and so are its members.

((function_definition
  (storage_class_specifier)* @modifier
  declarator: [
    (function_declarator declarator: (identifier) @name)
    (pointer_declarator declarator: (function_declarator declarator: (identifier) @name))
    (pointer_declarator declarator: (pointer_declarator declarator: (function_declarator declarator: (identifier) @name)))
  ]) @definition.function)

(preproc_function_def name: (identifier) @name) @definition.function

; `typedef struct x { … } y;` is one type, named `y` as its users write it; a tag that differs from
; its typedef name is not declared apart. The typedef is the node, so the comment above it is the doc.
((type_definition
  type: [(struct_specifier body: (_)) (union_specifier body: (_))]
  declarator: (type_identifier) @name) @definition.struct
  (#set! outside "compound_statement"))

((type_definition
  type: (enum_specifier body: (_))
  declarator: (type_identifier) @name) @definition.enum
  (#set! outside "compound_statement"))

; A tagged type standing alone. In a typedef the type is the typedef's, and one nested in its members
; is no type apart: those members are the holder's, as an unnamed one's are. One written in a
; variable's declaration is the variable's shape, not a type the file offers, and has no members.
([
  (struct_specifier name: (type_identifier) @name body: (_))
  (union_specifier name: (type_identifier) @name body: (_))
] @definition.struct
  (#set! outside "type_definition")
  (#set! outside "declaration")
  (#set! outside "compound_statement"))

((enum_specifier name: (type_identifier) @name body: (_)) @definition.enum
  (#set! outside "type_definition")
  (#set! outside "declaration")
  (#set! outside "compound_statement"))

; A typedef of anything else: a scalar, a pointer, a function or a function pointer. A typedef the
; patterns above declare keeps their kind. The grammar reads a standard name as a primitive type, which
; a portability header still declares: `typedef intptr_t ssize_t;`.
((type_definition
  declarator: [
    (type_identifier) @name
    (primitive_type) @name
    (pointer_declarator declarator: (type_identifier) @name)
    (function_declarator declarator: (type_identifier) @name)
    (function_declarator declarator: (parenthesized_declarator (type_identifier) @name))
    (function_declarator declarator: (parenthesized_declarator (pointer_declarator declarator: (type_identifier) @name)))
    (pointer_declarator declarator: (function_declarator declarator: (parenthesized_declarator (pointer_declarator declarator: (type_identifier) @name))))
  ]) @definition.type
  (#set! outside "compound_statement"))

; A struct's and a union's members. One declarator is the declaration's; with several, each is its own.
((field_declaration_list
  (field_declaration
    type: (_) .
    declarator: [
      (field_identifier) @name
      (pointer_declarator declarator: (field_identifier) @name)
      (pointer_declarator declarator: (pointer_declarator declarator: (field_identifier) @name))
      (array_declarator declarator: (field_identifier) @name)
      (function_declarator declarator: (parenthesized_declarator (pointer_declarator declarator: (field_identifier) @name)))
      (pointer_declarator declarator: (function_declarator declarator: (parenthesized_declarator (pointer_declarator declarator: (field_identifier) @name))))
    ] .) @definition.field)
  (#set! outside "declaration")
  (#set! outside "compound_statement"))

((field_declaration_list
  (field_declaration
    declarator: [
      (field_identifier) @name
      (pointer_declarator declarator: (field_identifier) @name)
      (pointer_declarator declarator: (pointer_declarator declarator: (field_identifier) @name))
      (array_declarator declarator: (field_identifier) @name)
    ] @definition.field))
  (#set! outside "declaration")
  (#set! outside "compound_statement"))

; An enumerator is a constant of the file wherever its enum is written, outside a function.
((enumerator_list (enumerator name: (identifier) @name) @definition.constant)
  (#set! outside "compound_statement"))

; `#include "x.h"` names a file of the repository; `<x.h>` names one on the compiler's path.
(preproc_include path: (string_literal) @import.path) @import
