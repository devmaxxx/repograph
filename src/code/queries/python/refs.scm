; A call, a base class and a decorator, each by the dotted path it is written as. What a path names
; is the imports' to say, so `module_paths` resolves all three.
(call function: (identifier) @name) @reference.calls

(call function: (attribute object: (_) @receiver attribute: (identifier) @name)) @reference.calls

(class_definition superclasses: (argument_list [(identifier) (attribute)] @reference.extends))

; The site is the definition decorated, so the edge starts there; `@x(…)` names `x`.
(decorated_definition (decorator [(identifier) (attribute)] @name) definition: (_) @reference.decorated_by)

(decorated_definition (decorator (call function: [(identifier) (attribute)] @name)) definition: (_) @reference.decorated_by)

; The class an attribute holds, with the attribute as the receiver: a class-level annotation, or in
; `__init__` an annotated `self.x`, a `self.x` set to a capitalised call, or to an annotated
; parameter. A reference with a receiver writes no edge by default; the hook reads these to resolve
; `self.x.m()`.
((class_definition body: (block (expression_statement (assignment left: (identifier) @receiver type: (type [(identifier) (attribute)] @name)) @reference.references)))
  (#set! context "type"))

((assignment left: (attribute object: (identifier) @_self attribute: (identifier) @receiver) type: (type [(identifier) (attribute)] @name)) @reference.references
  (#eq? @_self "self")
  (#set! context "type"))

((assignment left: (attribute object: (identifier) @_self attribute: (identifier) @receiver) !type right: (call function: [(identifier) (attribute)] @name)) @reference.references
  (#eq? @_self "self")
  (#match? @name "(^|\\.)[A-Z][A-Za-z0-9_]*$")
  (#set! context "type"))

; A parameter's annotation, and a `self.x` set to a bare name at any depth of a function, which the
; hook joins by the parameter's name: a query cannot tie a body statement under an `if` to the
; parameter list.
([(typed_parameter . (identifier) @receiver type: (type [(identifier) (attribute)] @name))
  (typed_default_parameter name: (identifier) @receiver type: (type [(identifier) (attribute)] @name))] @reference.references
  (#set! context "argument"))

((assignment left: (attribute object: (identifier) @_self attribute: (identifier) @receiver) !type right: (identifier) @name) @reference.references
  (#eq? @_self "self")
  (#set! within "function_definition")
  (#set! context "variable"))

; What a function binds for its whole body: parameters, assignment, loop, `with`, `except` and walrus
; targets, and the definitions directly in it. A lambda binds its parameters and a comprehension its
; loop variables in themselves alone. Module and class level bind nothing: a name there is the
; module's own, and a call to it resolves to its declaration.
[(function_definition) (lambda) (list_comprehension) (set_comprehension) (dictionary_comprehension) (generator_expression)] @local.scope

[
  (parameters [(identifier) @local.definition (list_splat_pattern (identifier) @local.definition) (dictionary_splat_pattern (identifier) @local.definition)])
  (lambda_parameters [(identifier) @local.definition (list_splat_pattern (identifier) @local.definition) (dictionary_splat_pattern (identifier) @local.definition)])
  (typed_parameter . [(identifier) @local.definition (list_splat_pattern (identifier) @local.definition) (dictionary_splat_pattern (identifier) @local.definition)])
  (default_parameter name: (identifier) @local.definition)
  (typed_default_parameter name: (identifier) @local.definition)
]

([
  (assignment left: (identifier) @local.definition)
  (augmented_assignment left: (identifier) @local.definition)
  (for_statement left: (identifier) @local.definition)
  (named_expression name: (identifier) @local.definition)
  (as_pattern alias: (as_pattern_target (identifier) @local.definition))
  (as_pattern alias: (as_pattern_target [(tuple (identifier) @local.definition) (list (identifier) @local.definition)]))
  (pattern_list [(identifier) @local.definition (list_splat_pattern (identifier) @local.definition)])
  (tuple_pattern [(identifier) @local.definition (list_splat_pattern (identifier) @local.definition)])
  (list_pattern [(identifier) @local.definition (list_splat_pattern (identifier) @local.definition)])
]
  (#set! within "function_definition"))

(for_in_clause left: [
  (identifier) @local.definition
  (pattern_list [(identifier) @local.definition (list_splat_pattern (identifier) @local.definition)])
  (tuple_pattern [(identifier) @local.definition (list_splat_pattern (identifier) @local.definition)])
])

((function_definition body: (block [
  (function_definition name: (identifier) @local.definition)
  (class_definition name: (identifier) @local.definition)
  (decorated_definition definition: (_ name: (identifier) @local.definition))
])) @local.scope)

; A lone string statement is a docstring, documentation as a comment is.
(expression_statement . (string) @cite.comment .)

(string) @cite.string

(comment) @cite.comment
