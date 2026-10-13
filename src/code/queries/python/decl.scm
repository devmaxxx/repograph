; A module declares its top-level `def`s, `class`es and plain-name assignments, and a class the same
; in its own body, through classes nested two deep. Nothing under an `if`, a `try`, a `with` or a
; `def` is declared: which branch runs, or what a function binds, is not the module's interface.
; Each depth is spelled out, since a query has no recursion and a class inside a function must not
; lend its members a place. A definition starts at its `def` or `class` line, so its decorators sit
; above it, and its first statement is its docstring when that is a lone string.

(module [
  (function_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.function
  (class_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.class
  (decorated_definition definition: [
    (function_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.function
    (class_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.class
  ])
  (expression_statement . (assignment left: (identifier) @name)) @definition.variable
])

; A class's members hang off it and are never the module's exports, whatever their name.
((module [
  (class_definition body: (block [
    (function_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.function
    (class_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.class
    (decorated_definition definition: [
      (function_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.function
      (class_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.class
    ])
    (expression_statement . (assignment left: (identifier) @name)) @definition.variable
  ]))
  (decorated_definition definition: (class_definition body: (block [
    (function_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.function
    (class_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.class
    (decorated_definition definition: [
      (function_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.function
      (class_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.class
    ])
    (expression_statement . (assignment left: (identifier) @name)) @definition.variable
  ])))
])
  (#set! export "never"))

; The members of a class inside a class, as a model's `Meta` holds its options.
((module [
  (class_definition body: (block [
    (class_definition body: (block [
      (function_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.function
      (decorated_definition definition: (function_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.function)
      (expression_statement . (assignment left: (identifier) @name)) @definition.variable
    ]))
    (decorated_definition definition: (class_definition body: (block [
      (function_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.function
      (decorated_definition definition: (function_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.function)
      (expression_statement . (assignment left: (identifier) @name)) @definition.variable
    ])))
  ]))
  (decorated_definition definition: (class_definition body: (block [
    (class_definition body: (block [
      (function_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.function
      (decorated_definition definition: (function_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.function)
      (expression_statement . (assignment left: (identifier) @name)) @definition.variable
    ]))
    (decorated_definition definition: (class_definition body: (block [
      (function_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.function
      (decorated_definition definition: (function_definition name: (identifier) @name body: (block . (expression_statement . (string) @doc .)?)) @definition.function)
      (expression_statement . (assignment left: (identifier) @name)) @definition.variable
    ])))
  ])))
])
  (#set! export "never"))

; Each imported module or name is a site of its own, so a name keeps its own alias. The path is the
; module as written, a relative one with its leading dots.
(import_statement name: (dotted_name) @import.path @import)

(import_statement name: (aliased_import name: (dotted_name) @import.path alias: (identifier) @import.alias) @import)

(import_from_statement module_name: (_) @import.path name: (dotted_name) @import.name @import)

(import_from_statement module_name: (_) @import.path name: (aliased_import name: (dotted_name) @import.name alias: (identifier) @import.alias) @import)

(import_from_statement module_name: (_) @import.path (wildcard_import) @import.name @import)
