; A top-level block with at least one label is its type and labels: `target/api` in a bake file. A
; label is a bare identifier or a string of written-out text alone; any other label, or one holding
; a `/`, names nothing this id scheme can hold, so the block declares nothing.
(config_file
  (body
    (block
      . (identifier) @name
      . ([(identifier) @name (string_lit . (quoted_template_start) . (template_literal) @name . (quoted_template_end) .)])+
      . (block_start)) @definition.block)
  (#not-match? @name "/"))
