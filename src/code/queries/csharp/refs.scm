; Ids cited in comments and strings, from the declaration around them; an interpolated string's holes
; are code, so only its literal text cites. Calls, type uses, supertypes and attributes bind through
; the compiler's lookup order, which `namespace_scope` walks.
(comment) @cite.comment
(string_literal) @cite.string
(verbatim_string_literal) @cite.string
(raw_string_literal) @cite.string
(interpolated_string_expression (string_content) @cite.string)
