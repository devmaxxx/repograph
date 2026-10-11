; A spread is a call, because codegen inlines the fragment where it is spread.
((fragment_spread (fragment_name (name) @name)) @reference.calls
  (#set! prefix "fragment/"))

; A directive anywhere in an operation decorates the operation, and one on a type or a field
; decorates that.
((directive (name) @name) @reference.decorated_by
  (#set! prefix "directive/"))

(implements_interfaces (named_type (name) @reference.extends))

((union_member_types (named_type (name) @reference.references))
  (#set! context "member"))

((type_condition (named_type (name) @reference.references))
  (#set! context "on"))

; `extend type T` changes `T` wherever it is defined, as an `ALTER TABLE` does.
((type_extension (_ (name) @reference.references))
  (#set! context "extend"))

; A type position names its type beneath list and non-null wrappers, and every node between the two is
; a wrapper, so the depth alone tells the wrappers apart; up to `[[T!]!]!`, seven deep.
((variable_definition
  (type [
    (named_type (name) @reference.references)
    (_ (named_type (name) @reference.references))
    (_ (_ (named_type (name) @reference.references)))
    (_ (_ (_ (named_type (name) @reference.references))))
    (_ (_ (_ (_ (named_type (name) @reference.references)))))
    (_ (_ (_ (_ (_ (named_type (name) @reference.references))))))
    (_ (_ (_ (_ (_ (_ (named_type (name) @reference.references)))))))
    (_ (_ (_ (_ (_ (_ (_ (named_type (name) @reference.references))))))))
  ]))
  (#set! context "variable"))

((field_definition
  (type [
    (named_type (name) @reference.references)
    (_ (named_type (name) @reference.references))
    (_ (_ (named_type (name) @reference.references)))
    (_ (_ (_ (named_type (name) @reference.references))))
    (_ (_ (_ (_ (named_type (name) @reference.references)))))
    (_ (_ (_ (_ (_ (named_type (name) @reference.references))))))
    (_ (_ (_ (_ (_ (_ (named_type (name) @reference.references)))))))
    (_ (_ (_ (_ (_ (_ (_ (named_type (name) @reference.references))))))))
  ]))
  (#set! context "type"))

((input_fields_definition (input_value_definition
  (type [
    (named_type (name) @reference.references)
    (_ (named_type (name) @reference.references))
    (_ (_ (named_type (name) @reference.references)))
    (_ (_ (_ (named_type (name) @reference.references))))
    (_ (_ (_ (_ (named_type (name) @reference.references)))))
    (_ (_ (_ (_ (_ (named_type (name) @reference.references))))))
    (_ (_ (_ (_ (_ (_ (named_type (name) @reference.references)))))))
    (_ (_ (_ (_ (_ (_ (_ (named_type (name) @reference.references))))))))
  ])))
  (#set! context "type"))

; A field's or a directive's arguments.
((arguments_definition (input_value_definition
  (type [
    (named_type (name) @reference.references)
    (_ (named_type (name) @reference.references))
    (_ (_ (named_type (name) @reference.references)))
    (_ (_ (_ (named_type (name) @reference.references))))
    (_ (_ (_ (_ (named_type (name) @reference.references)))))
    (_ (_ (_ (_ (_ (named_type (name) @reference.references))))))
    (_ (_ (_ (_ (_ (_ (named_type (name) @reference.references)))))))
    (_ (_ (_ (_ (_ (_ (_ (named_type (name) @reference.references))))))))
  ])))
  (#set! context "argument"))

(comment) @cite.comment

(string_value) @cite.string
