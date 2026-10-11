; Every function definition, nested ones included: a function defined inside another is callable
; once the outer one has run, and a function qualifies nothing, so each is a name of the file.
(function_definition
  name: (_) @name) @definition.function

; `source f` and `. f` run a script in this shell. The script computes the path from its own
; location, so the `import` hook evaluates it rather than a capture.
(command
  name: (command_name) @_name
  (#any-of? @_name "source" ".")) @import
