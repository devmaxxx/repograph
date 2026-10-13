; A name in an inheritance clause, by its first segment: `Base<T>` and `Outer.Inner` name `Base` and
; `Outer`. The clause is the declared type's that holds it; an extension declares no type, so its
; clause names nothing.
(inheritance_specifier inherits_from: (user_type . (type_identifier) @name)) @reference.extends
