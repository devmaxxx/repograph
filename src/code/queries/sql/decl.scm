; A migration declares what it creates and every table it attaches to: `ALTER TABLE t`, and an
; index, a policy or a trigger `ON t`. A table holds its columns and what is attached to it. A name
; is its schema and object as two parts, and a 3-part name is read as its first two.
;
; A table created here is a `type` and one only attached to a `record`: the first statement to name
; a table in a file decides which, as the first to declare an id keeps it.

; `PARTITION OF` names a second table, which the partition does not declare.
(toplevel_stmt (stmt [(CreateStmt [(kw_table) (kw_exists)] .
  (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))
  (CreateForeignTableStmt [(kw_table) (kw_exists)] .
  (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))])) @definition.type

(toplevel_stmt (stmt (CreateAsStmt (create_as_target
  (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))))) @definition.type

(toplevel_stmt (stmt (ViewStmt
  (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?)))) @definition.type

(toplevel_stmt (stmt (CreateMatViewStmt (create_mv_target
  (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))))) @definition.type

(toplevel_stmt (stmt (CreateFunctionStmt
  (func_name [(type_function_name) @name (ColId) @name] (indirection . (indirection_el (attr_name (ColLabel) @name)))?)))) @definition.function

; The grammar reads `CREATE TYPE` and `CREATE AGGREGATE` as one statement.
(toplevel_stmt (stmt (DefineStmt (kw_create) . (kw_type)
  (any_name (ColId) @name (attrs . (attr_name (ColLabel) @name))?)))) @definition.type

(toplevel_stmt (stmt (CreateSchemaStmt (ColId) @name))) @definition.module

(toplevel_stmt (stmt (CreateSeqStmt
  (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?)))) @definition.value

; A renamed table keeps its schema. The name it had is read as a reference, not declared here.
(toplevel_stmt (stmt (RenameStmt (kw_alter) . (kw_table)
  (relation_expr [
    (qualified_name (ColId) @name (indirection))
    (extended_relation_expr (qualified_name (ColId) @name (indirection)))
  ])
  (kw_rename) . (kw_to) . (name) @name))) @definition.type

; An unqualified table is renamed to an unqualified name.
(toplevel_stmt (stmt (RenameStmt (kw_alter) . (kw_table)
  (relation_expr [
    (qualified_name . (ColId) .)
    (extended_relation_expr (qualified_name . (ColId) .))
  ])
  (kw_rename) . (kw_to) . (name) @name))) @definition.type

; The grammar reads `ALTER TABLE` and `ALTER INDEX` as one statement.
(toplevel_stmt (stmt [
  (AlterTableStmt (kw_alter) . (kw_table) (relation_expr [
    (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?)
    (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))
  ]))
  (RenameStmt (kw_alter) . (kw_table) (relation_expr [
    (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?)
    (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))
  ]))
  (AlterObjectSchemaStmt (kw_alter) . (kw_table) (relation_expr [
    (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?)
    (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))
  ]))
  (IndexStmt (relation_expr [
    (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?)
    (extended_relation_expr (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))
  ]))
  (CreatePolicyStmt
    (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))
  (CreateTrigStmt
    (qualified_name (ColId) @name (indirection . (indirection_el (attr_name (ColLabel) @name)))?))
])) @definition.record

; What is attached to a table spans the statement attaching it, as the table it attaches to does.
(IndexStmt [(name) (opt_single_name)] @name) @definition.field

(CreatePolicyStmt (name) @name) @definition.field

(CreateTrigStmt (name) @name) @definition.field

(TableElement (columnDef (ColId) @name) @definition.field)

(alter_table_cmd (columnDef (ColId) @name) @definition.field)
