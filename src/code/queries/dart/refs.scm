; A call, by its callee's shape: a bare name, or a member of a name, of `this`, or of a field of
; `this`. Which declaration each reaches is the library's to say, so `library_scope` resolves them.
(call_expression function: (identifier) @name) @reference.calls

(call_expression function: (member_expression
  object: [
    (identifier)
    "this"
    (member_expression object: "this" property: (identifier))
  ] @receiver
  property: (identifier) @name)) @reference.calls

; A supertype by its outermost name: `State<Home>` extends `State`, and its argument names nothing.
; `with` and `implements` are read as `extends` is.
(superclass type: (type . (type_identifier) @name) @reference.extends)

(superclass (mixins (type . (type_identifier) @name) @reference.extends))

(interfaces (type . (type_identifier) @name) @reference.extends)

(mixin_application_class (mixin_application (type . (type_identifier) @name) @reference.extends))

(mixin_application_class (mixin_application (mixins (type . (type_identifier) @name) @reference.extends)))

; A field's type with the field as the receiver: the class a call through that field reaches. It
; is the type the field declares, or else the class its initializer constructs, a capitalised callee
; being a class by Dart's convention. A reference with a receiver writes no edge by default.
((class_member (declaration (type . (type_identifier) @name) @reference.references [
  (initialized_identifier_list (initialized_identifier name: (identifier) @receiver))
  (static_final_declaration_list (static_final_declaration name: (identifier) @receiver))
]))
  (#set! context "type"))

((class_member (declaration . [
  (initialized_identifier_list (initialized_identifier name: (identifier) @receiver
    value: (call_expression function: [(identifier) @name (member_expression object: (identifier) @name)]) @reference.references))
  (static_final_declaration_list (static_final_declaration name: (identifier) @receiver
    value: (call_expression function: [(identifier) @name (member_expression object: (identifier) @name)]) @reference.references))
]))
  (#match? @name "^[A-Z]")
  (#set! context "type"))
