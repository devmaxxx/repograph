; Every function definition, nested ones included: a function defined inside another is callable
; once the outer one has run, and a function qualifies nothing, so each is a name of the file.
(function_definition
  name: (_) @name) @definition.function

; `source f` and `. f` run a script in this shell. The script computes the path from its own
; location, so the `import` hook evaluates the operand; the capture is what names the site to it,
; and its text survives an edit elsewhere in the file where a byte offset would not.
(command
  name: (command_name) @_name
  .
  argument: (_) @import.path
  (#any-of? @_name "source" ".")
  (#not-eq? @import.path "--")) @import

(command
  name: (command_name) @_name
  .
  argument: (word) @_end
  .
  argument: (_) @import.path
  (#any-of? @_name "source" ".")
  (#eq? @_end "--")) @import
