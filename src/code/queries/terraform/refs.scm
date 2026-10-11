; A traversal names an address: its root and the attribute steps Terraform's address takes after it,
; `var.region` naming `var/region` and `data.aws_ami.u.id` naming `data/aws_ami/u`. A traversal that
; stops before its address names nothing, and `each`, `count`, `path`, `self` and `terraform` are
; values of the block, never addresses.
(_
  (variable_expr (identifier) @name @_root) @reference.references
  . (get_attr (identifier) @name)
  (#not-any-of? @_root "data" "each" "count" "path" "self" "terraform"))

(_
  (variable_expr (identifier) @name @_root) @reference.references
  . (get_attr (identifier) @name)
  . (get_attr (identifier) @name)
  (#eq? @_root "data"))

; The grammar hangs the steps after an operation's last operand on the operation: in `a == c.d` and
; in `!c.d` the `.d` follows the whole `a == c` or `!c`, and on the outermost operation when one nests
; in another. Two levels of nesting are read.
(_
  (operation (_ (variable_expr (identifier) @name @_root) @reference.references .))
  . (get_attr (identifier) @name)
  (#not-any-of? @_root "data" "each" "count" "path" "self" "terraform"))

(_
  (operation (_ (operation (_ (variable_expr (identifier) @name @_root) @reference.references .)) .))
  . (get_attr (identifier) @name)
  (#not-any-of? @_root "data" "each" "count" "path" "self" "terraform"))

(_
  (operation (_ (variable_expr (identifier) @name @_root) @reference.references .))
  . (get_attr (identifier) @name)
  . (get_attr (identifier) @name)
  (#eq? @_root "data"))

; A `for` variable binds in the body and the condition, never in the collection beside it.
(for_tuple_expr (for_intro (identifier) @local.definition) (expression) @local.scope)
(for_tuple_expr (for_intro (identifier) @local.definition) (for_cond) @local.scope)
(for_object_expr (for_intro (identifier) @local.definition) (expression) @local.scope)
(for_object_expr (for_intro (identifier) @local.definition) (for_cond) @local.scope)

; A `dynamic "label"` block's iterator binds in its body: the label, unless an `iterator = x` written
; there renames it to `x`. A repetition from the body's first item captures every item, and the
; negated match must hold for each, so the label binds only when no item is an `iterator` attribute.
(block
  . (identifier) @_kind
  . (string_lit (template_literal) @local.definition)
  (body . (_)+ @_item) @local.scope
  (#eq? @_kind "dynamic")
  (#not-match? @_item "^iterator[ \t]*="))
(block
  . (identifier) @_kind
  (body
    (attribute
      . (identifier) @_iterator
      . (expression (variable_expr (identifier) @local.definition)))) @local.scope
  (#eq? @_kind "dynamic")
  (#eq? @_iterator "iterator"))

(comment) @cite.comment

[(string_lit) (quoted_template) (heredoc_template)] @cite.string
