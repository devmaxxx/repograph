; A foreign key depends on the table it references, from its column or, for a table constraint,
; from its table.
([
  (ColConstraintElem (kw_references) . (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) @reference.references)
  (ConstraintElem (kw_references) . (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) @reference.references)
 ]
  (#set! context "references"))

; `search_path` is session state no file records, so an unqualified name stands for `public`'s too,
; and a name in `public` for the unqualified one.
([
  (ColConstraintElem (kw_references) . (qualified_name . (ColId) @name .) @reference.references)
  (ConstraintElem (kw_references) . (qualified_name . (ColId) @name .) @reference.references)
 ]
  (#set! context "references")
  (#set! prefix "public/"))

([
  (ColConstraintElem (kw_references) . (qualified_name (ColId) @_schema (indirection . (indirection_el (attr_name (ColLabel) @name)) .)) @reference.references)
  (ConstraintElem (kw_references) . (qualified_name (ColId) @_schema (indirection . (indirection_el (attr_name (ColLabel) @name)) .)) @reference.references)
 ]
  (#match? @_schema "^([Pp][Uu][Bb][Ll][Ii][Cc]|\"public\")$")
  (#set! context "references"))

; A view depends on every relation its query reads, a join's, a subquery's and a CTE's included. The
; query nests a relation under its clauses and its joins, so the depth alone reaches it: five levels
; below the query for a plain `FROM t`, up to fourteen. A view's name reads as written, with no
; `public` twin.
((ViewStmt (SelectStmt [
    (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references)))))
    (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references))))))
    (_ (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references)))))))
    (_ (_ (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references))))))))
    (_ (_ (_ (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references)))))))))
    (_ (_ (_ (_ (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references))))))))))
    (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references)))))))))))
    (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references))))))))))))
    (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references)))))))))))))
    (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references))))))))))))))
  ]))
  (#set! context "from"))

((CreateMatViewStmt (SelectStmt [
    (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references)))))
    (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references))))))
    (_ (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references)))))))
    (_ (_ (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references))))))))
    (_ (_ (_ (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references)))))))))
    (_ (_ (_ (_ (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references))))))))))
    (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references)))))))))))
    (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references))))))))))))
    (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references)))))))))))))
    (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (_ (table_ref (relation_expr [(qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?) (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))]) @reference.references))))))))))))))
  ]))
  (#set! context "from"))

; A CTE binds its name for the statement it is part of.
(common_table_expr (name) @local.definition)

(toplevel_stmt) @local.scope

(CreateTrigStmt
  (func_name [(type_function_name) @name (ColId) @name] (indirection . (indirection_el (attr_name (ColLabel) @name)))?) @reference.calls)

((CreateTrigStmt (func_name (type_function_name) @name) @reference.calls)
  (#set! prefix "public/"))

((CreateTrigStmt (func_name (ColId) @_schema (indirection . (indirection_el (attr_name (ColLabel) @name)) .)) @reference.calls)
  (#match? @_schema "^([Pp][Uu][Bb][Ll][Ii][Cc]|\"public\")$"))

; A renamed table is the same table under a new name, so it depends on every file declaring the old.
((RenameStmt (kw_alter) . (kw_table)
  (relation_expr [
    (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?)
    (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))
  ]) @reference.references
  (kw_rename) . (kw_to))
  (#set! context "rename"))

(comment) @cite.comment

(string_literal) @cite.string

(dollar_quoted_string) @cite.string
