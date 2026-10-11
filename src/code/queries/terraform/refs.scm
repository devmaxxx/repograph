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

; A `dynamic "label"` block's iterator binds in its body: the label, and an `iterator = x` written
; there. Binding the label beside an explicit iterator costs only a resource type spelled like a
; nested block's type.
(block
  . (identifier) @_kind
  . (string_lit (template_literal) @local.definition)
  (body) @local.scope
  (#eq? @_kind "dynamic"))
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
