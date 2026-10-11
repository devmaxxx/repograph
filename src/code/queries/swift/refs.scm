; A name in an inheritance clause, by its first segment: `Base<T>` and `Outer.Inner` name `Base` and
; `Outer`. Whose clause it is, a declared type's or an extension's, the resolve hook reads.
(inheritance_specifier inherits_from: (user_type . (type_identifier) @name)) @reference.extends
