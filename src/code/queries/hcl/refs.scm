; Each written-out item of a bake `inherits` or `targets` list: which block it names is the
; `target_or_group` rule's.
((attribute
  . (identifier) @_key
  . (expression
    (collection_value
      (tuple
        (expression
          . [
            (literal_value . (string_lit . (quoted_template_start) . (template_literal) @name . (quoted_template_end) .) .)
            (template_expr . (quoted_template . (quoted_template_start) . (template_literal) @name . (quoted_template_end) .) .)
          ] .))))) @reference.references
  (#any-of? @_key "inherits" "targets"))

(comment) @cite.comment

[(string_lit) (quoted_template) (heredoc_template)] @cite.string
