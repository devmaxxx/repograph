; One source for `.ml` and `.mli`: what an implementation declares with `let`, an interface declares
; with `val`. A `let` inside an expression is local, at any depth, so every definition sits outside
; `let_expression`. A module joins what it holds with `/`; a module type, a type and a class with `.`.

; A binding with parameters or a function for its body is a function; any other names a value. The
; first pattern to match a binding keeps it. An operator binding is no name a path reaches.
((let_binding pattern: (value_name) @name (parameter)) @definition.function
  (#set! outside "let_expression"))

((let_binding pattern: (value_name) @name body: [(fun_expression) (function_expression)]) @definition.function
  (#set! outside "let_expression"))

((let_binding pattern: (value_name) @name) @definition.value
  (#set! outside "let_expression"))

((let_binding pattern: [
    (tuple_pattern (value_name) @name)
    (parenthesized_pattern (tuple_pattern (value_name) @name))
  ]) @definition.value
  (#set! outside "let_expression"))

((external (value_name) @name) @definition.function
  (#set! outside "let_expression"))

; A functor parameter's signature says what an argument must hold, and declares none of it.
((value_specification (value_name) @name type: (function_type)) @definition.function
  (#set! outside "module_parameter"))

((value_specification (value_name) @name) @definition.value
  (#set! outside "module_parameter"))

; `module M = struct … end`, a functor, an alias and an interface's `module M : sig … end` alike.
((module_definition (module_binding (module_name) @name) @definition.module)
  (#set! outside "let_expression"))

(module_type_definition (module_type_name) @name) @definition.interface

((type_binding name: (type_constructor) @name) @definition.type
  (#set! outside "let_expression"))

(variant_declaration (constructor_declaration (constructor_name) @name) @definition.constant)

(record_declaration (field_declaration (field_name) @name) @definition.field)

(exception_definition (constructor_declaration (constructor_name) @name) @definition.constant)

(class_binding (class_name) @name) @definition.class

(method_definition (method_name) @name) @definition.method

; `open M`, `let open M in` and `M.(…)` show `M`'s names bare. `include M` hands them on as the
; file's own. An interface's `include` names a module type, which declares nothing a name reaches.
((open_module module: (module_path) @import.path) @import
  (#set! outside "let_expression"))

; A local open reaches its expression alone, where it shadows what the file opens.
((let_expression (open_module module: (module_path) @import.path)) @import
  (#set! visibility "node"))

((local_open_expression (module_path) @import.path) @import
  (#set! visibility "node"))

((include_module module: (module_path) @import.path) @import
  (#set! reexport))

; `module R = Core` binds `R` to `Core`, and a path through the file's `R` is read in `Core`.
((module_definition (module_binding (module_name) @import.alias body: (module_path) @import.path)) @import
  (#set! reexport))

; A path's first module is a file's, named by the file with no import: `Cset.union` uses `cset.ml`.
; It binds the name to that file, as `ocamldep` reads a dependency.
(module_path . (module_name) @import.path @import.alias) @import

(extended_module_path . (module_name) @import.path @import.alias) @import
