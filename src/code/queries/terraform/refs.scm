; A traversal's root: which address it spells, from which steps, and whether a `for` or a
; `dynamic` block binds it first, is the `traversal_addresses` rule's.
(variable_expr) @reference.references

(comment) @cite.comment

[(string_lit) (quoted_template) (heredoc_template)] @cite.string
