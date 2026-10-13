; A variable's name is written with its `$`, a function's and a class's without it, so no local binding
; ever hides a function or a class, and the file declares no locals.

(function_call_expression function: (name) @reference.calls)

; `$this->m()`, `self::m()` and `static::m()` call the method's own class, inherited members aside.
; `parent::m()` calls the class it extends, which the owner would misname.
((member_call_expression object: (variable_name (name) @receiver) name: (name) @reference.calls)
  (#eq? @receiver "this")
  (#set! receiver "owner"))

((nullsafe_member_call_expression object: (variable_name (name) @receiver) name: (name) @reference.calls)
  (#eq? @receiver "this")
  (#set! receiver "owner"))

((scoped_call_expression scope: (relative_scope) @receiver name: (name) @reference.calls)
  (#any-of? @receiver "self" "static")
  (#set! receiver "owner"))

; `Route::create()` through the class's import, or its namespace, or the own file. On any other value
; a call names nothing, since its type is unread.
(scoped_call_expression scope: (name) @receiver name: (name) @reference.calls)

; `Route::NAME`, `self::NAME`; `Route::class` names the class itself.
((class_constant_access_expression . (name) @receiver . (name) @reference.references)
  (#not-eq? @reference.references "class"))

((class_constant_access_expression . (relative_scope) @receiver . (name) @reference.references)
  (#any-of? @receiver "self" "static")
  (#not-eq? @reference.references "class")
  (#set! receiver "owner"))

((class_constant_access_expression . (name) @reference.references . (name) @_class)
  (#eq? @_class "class")
  (#set! context "type"))

(class_declaration (base_clause (name) @reference.extends))
(interface_declaration (base_clause (name) @reference.extends))
(class_declaration (class_interface_clause (name) @reference.implements))
(enum_declaration (class_interface_clause (name) @reference.implements))

; A trait's methods are the class's own, as a mixin's are.
((use_declaration (name) @reference.references)
  (#set! context "impl"))

; A class named where a type is written: a parameter, a return, a property, a `catch`. A qualified
; name names a class of another namespace, which only its full path tells apart, so it is left.
([
  (named_type (name) @reference.references)
  (object_creation_expression . (name) @reference.references)
  (binary_expression operator: "instanceof" right: (name) @reference.references)
 ]
  (#set! context "type"))

(comment) @cite.comment

[(string) (encapsed_string) (heredoc) (nowdoc)] @cite.string
