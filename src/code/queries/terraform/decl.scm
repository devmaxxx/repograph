; A top-level block is a Terraform address, `/` standing for Terraform's `.`. A label is a bare
; identifier or a string of written-out text alone; any other label, or one holding the `/` a lock
; file's registry paths do, names no address, so the block declares nothing. The trailing anchor
; holds each kind to its label count: `resource "t" {}` is not an address.
(config_file
  (body
    (block
      . (identifier) @_kind
      . [(identifier) @name (string_lit . (quoted_template_start) . (template_literal) @name . (quoted_template_end) .)]
      . [(identifier) @name (string_lit . (quoted_template_start) . (template_literal) @name . (quoted_template_end) .)]
      . (block_start)) @definition.resource)
  (#eq? @_kind "resource")
  (#not-match? @name "/"))

(config_file
  (body
    (block
      . (identifier) @_kind
      . [(identifier) @name (string_lit . (quoted_template_start) . (template_literal) @name . (quoted_template_end) .)]
      . [(identifier) @name (string_lit . (quoted_template_start) . (template_literal) @name . (quoted_template_end) .)]
      . (block_start)) @definition.resource)
  (#eq? @_kind "data")
  (#not-match? @name "/")
  (#set! prefix "data/"))

; A variable and an output are the module's interface, read from the directory that calls it.
(config_file
  (body
    (block
      . (identifier) @_kind
      . [(identifier) @name (string_lit . (quoted_template_start) . (template_literal) @name . (quoted_template_end) .)]
      . (block_start)) @definition.variable)
  (#eq? @_kind "variable")
  (#not-match? @name "/")
  (#set! prefix "var/")
  (#set! export "always"))

(config_file
  (body
    (block
      . (identifier) @_kind
      . [(identifier) @name (string_lit . (quoted_template_start) . (template_literal) @name . (quoted_template_end) .)]
      . (block_start)) @definition.output)
  (#eq? @_kind "output")
  (#not-match? @name "/")
  (#set! prefix "output/")
  (#set! export "always"))

(config_file
  (body
    (block
      . (identifier) @_kind
      . [(identifier) @name (string_lit . (quoted_template_start) . (template_literal) @name . (quoted_template_end) .)]
      . (block_start)) @definition.value)
  (#eq? @_kind "module")
  (#not-match? @name "/")
  (#set! prefix "module/"))

; Aliased providers share one address, and the first block is the symbol.
(config_file
  (body
    (block
      . (identifier) @_kind
      . [(identifier) @name (string_lit . (quoted_template_start) . (template_literal) @name . (quoted_template_end) .)]
      . (block_start)) @definition.block)
  (#eq? @_kind "provider")
  (#not-match? @name "/")
  (#set! prefix "provider/"))

; Each attribute of an unlabelled `locals` block is its own address.
(config_file
  (body
    (block
      . (identifier) @_kind
      . (block_start)
      (body (attribute . (identifier) @name) @definition.variable)))
  (#eq? @_kind "locals")
  (#set! prefix "local/"))

; A module call at any depth whose source Terraform reads as local: `./` or `../`, never the
; registry address `modules/dns`.
((block
  . (identifier) @_kind
  (body
    (attribute
      . (identifier) @_source
      . (expression
        . [
          (literal_value . (string_lit . (quoted_template_start) . (template_literal) @import.path . (quoted_template_end) .) .)
          (template_expr . (quoted_template . (quoted_template_start) . (template_literal) @import.path . (quoted_template_end) .) .)
        ] .)))) @import
  (#eq? @_kind "module")
  (#eq? @_source "source")
  (#match? @import.path "^\\.\\.?/"))
