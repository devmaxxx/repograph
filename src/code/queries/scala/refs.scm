; A parameter binds in its function's body, a class parameter in the class's, and a lambda's, a case's
; and a `for`'s bindings in what they hold, a case's down to a pattern nested in its pattern. A `val`,
; a `var` or a `def` a block declares is local.
; No field name on the lists: a curried `def f(a: A)(b: B)` writes `parameters` once per list, and a
; field pattern matches only the first.
(function_definition (parameters (parameter name: (identifier) @local.definition)) body: (_) @local.scope)

(class_definition (class_parameters (class_parameter name: (identifier) @local.definition)) body: (template_body) @local.scope)

; A type parameter binds across the whole definition, its signature included, and hides a type of its name.
(function_definition (type_parameters (identifier) @local.definition)) @local.scope

(class_definition type_parameters: (type_parameters (identifier) @local.definition)) @local.scope

(trait_definition type_parameters: (type_parameters (identifier) @local.definition)) @local.scope

(lambda_expression parameters: [
  (identifier) @local.definition
  (bindings (binding name: (identifier) @local.definition))
]) @local.scope

(case_clause pattern: [
  (identifier) @local.definition
  (tuple_pattern (identifier) @local.definition)
  (typed_pattern pattern: (identifier) @local.definition)
  (case_class_pattern pattern: (identifier) @local.definition)
  (capture_pattern name: (identifier) @local.definition)
  (tuple_pattern (tuple_pattern (identifier) @local.definition))
  (tuple_pattern (typed_pattern pattern: (identifier) @local.definition))
  (tuple_pattern (case_class_pattern pattern: (identifier) @local.definition))
  (case_class_pattern pattern: (case_class_pattern pattern: (identifier) @local.definition))
]) @local.scope

(for_expression enumerators: (enumerators (enumerator . (identifier) @local.definition))) @local.scope

(block) @local.scope

((block [
  (val_definition pattern: (identifier) @local.definition)
  (val_definition pattern: (tuple_pattern (identifier) @local.definition))
  (var_definition pattern: (identifier) @local.definition)
  (function_definition name: (identifier) @local.definition)
]))

; A bare call names a member of the type around it before anything else of its name. A member call on
; a value (`p.relativeTo(q)`) needs the value's type, which nothing here reads; one on a package or an
; object (`os.read(p)`, `os.remove.all(p)`, `TestUtil.prep()`) names what that path declares.
((call_expression function: [
  (identifier) @reference.calls
  (generic_function function: (identifier) @reference.calls)
])
  (#set! receiver "owner"))

((call_expression function: [
  (field_expression value: (identifier) @receiver field: (identifier) @reference.calls)
  (generic_function function: (field_expression value: (identifier) @receiver field: (identifier) @reference.calls))
])
  (#eq? @receiver "this")
  (#set! receiver "owner"))

((call_expression function: [
  (field_expression value: [(identifier) (field_expression)] @receiver field: (identifier) @reference.calls)
  (generic_function function: (field_expression value: [(identifier) (field_expression)] @receiver field: (identifier) @reference.calls))
])
  (#not-eq? @receiver "this"))

; A base clause names the types a definition extends, `with` included. Each is a child of the clause,
; not its field: a field pattern matches only the first of a repeated field.
(extends_clause [
  (type_identifier) @reference.extends
  (generic_type type: (type_identifier) @reference.extends)
  (stable_type_identifier (_) @receiver (type_identifier) @reference.extends)
  (generic_type type: (stable_type_identifier (_) @receiver (type_identifier) @reference.extends))
])

; A type written anywhere else: a parameter, a result, a value, a type argument, `new X`, a pattern.
; A type's own name in `type T = …` reaches the definition it names, which writes no edge.
((type_identifier) @reference.references
  (#set! context "type")
  (#set! outside "extends_clause")
  (#set! outside "stable_type_identifier")
  (#set! outside "import_declaration"))

((stable_type_identifier (_) @receiver (type_identifier) @reference.references)
  (#set! context "type")
  (#set! outside "extends_clause")
  (#set! outside "import_declaration"))

; A base's type arguments are types the clause is written with, not types the definition extends.
((type_arguments [
  (type_identifier) @reference.references
  (generic_type type: (type_identifier) @reference.references)
  (stable_type_identifier (_) @receiver (type_identifier) @reference.references)
  (generic_type type: (stable_type_identifier (_) @receiver (type_identifier) @reference.references))
])
  (#set! context "type")
  (#set! within "extends_clause"))

[(comment) (block_comment)] @cite.comment

[(string) (interpolated_string)] @cite.string
