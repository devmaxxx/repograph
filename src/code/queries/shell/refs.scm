; A command is a call when this file, or a script it sources, defines its name. A name holding a
; path, an expansion or a quote runs a file or is computed, and is no function's name.
(command
  name: (command_name) @name
  (#not-match? @name "[/$\"']")) @reference.calls

(comment) @cite.comment

[
  (string)
  (raw_string)
  (heredoc_body)
] @cite.string
