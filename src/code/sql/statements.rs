//! One walk over a PostgreSQL file's statements: what each creates, what it attaches to a table, and which
//! names it uses. Nothing is resolved here; `mod.rs` resolves through the SQL index, so `header` and
//! `extract` read one list and cannot disagree about what a file declares.
use tree_sitter::Node;

use super::names;
use crate::model::EdgeKind;

/// Capped as TypeScript's doc comments are, so a migration's essay does not drown its statement line.
const DOC_CHARS: usize = 600;

pub(super) struct Object {
    /// The id tail: `app/clients`.
    pub name: String,
    pub span: (u32, u32),
    pub body: String,
    /// Declared by `ALTER TABLE t` or `CREATE TRIGGER|POLICY|INDEX … ON t`, not by the statement creating `t`.
    pub attached: bool,
}

pub(super) struct Member {
    pub table: String,
    pub name: String,
    pub span: (u32, u32),
    pub body: String,
}

pub(super) struct Link {
    /// The id tail of the declaration the name is used in.
    pub from: String,
    /// The object named, folded and joined, not yet resolved to a file.
    pub to: String,
    pub kind: EdgeKind,
    pub context: &'static str,
}

pub(super) struct Cite {
    /// The id tail of the statement the text sits in; `None` between statements.
    pub from: Option<String>,
    pub context: &'static str,
    pub text: String,
}

#[derive(Default)]
pub(super) struct Read {
    pub objects: Vec<Object>,
    pub members: Vec<Member>,
    pub links: Vec<Link>,
    pub cites: Vec<Cite>,
}

fn text<'a>(n: Node, src: &'a [u8]) -> &'a str {
    n.utf8_text(src).unwrap_or("")
}

fn named(n: Node<'_>) -> Vec<Node<'_>> {
    let mut c = n.walk();
    n.named_children(&mut c).collect()
}

fn child<'t>(n: Node<'t>, kind: &str) -> Option<Node<'t>> {
    named(n).into_iter().find(|c| c.kind() == kind)
}

fn span(n: Node) -> (u32, u32) {
    (n.start_position().row as u32 + 1, n.end_position().row as u32 + 1)
}

/// Every descendant of one of `kinds`, not descending into a match.
fn find<'t>(n: Node<'t>, kinds: &[&str], out: &mut Vec<Node<'t>>) {
    for c in named(n) {
        if kinds.contains(&c.kind()) {
            out.push(c);
        } else {
            find(c, kinds, out);
        }
    }
}

fn line_at<'a>(lines: &[&'a str], row: u32) -> &'a str {
    lines.get(row as usize - 1).map_or("", |l| l.trim())
}

/// The grammar gives `ALTER TABLE` and `ALTER INDEX` one node kind, and `CREATE TYPE` and `CREATE AGGREGATE`
/// another, so a statement's own first words decide which it is.
fn opens_with(stmt: Node, src: &[u8], words: &[&str]) -> bool {
    text(stmt, src).split_whitespace().take(words.len()).map(str::to_lowercase).eq(words.iter().map(|w| w.to_string()))
}

fn comment_text(raw: &str) -> String {
    raw.trim().trim_start_matches("--").trim_start_matches("/*").trim_end_matches("*/").trim().to_string()
}

/// The comment lines directly above a statement. A comment starting on the row the statement before it ends on
/// is that statement's tail (drizzle's `--> statement-breakpoint`) and describes nothing below it.
fn doc(top: Node, src: &[u8]) -> String {
    let mut parts = Vec::new();
    let mut next = top;
    while let Some(prev) = next.prev_named_sibling() {
        let trailing = prev.prev_named_sibling().is_some_and(|p| p.end_position().row == prev.start_position().row);
        if prev.kind() != "comment" || trailing || prev.end_position().row + 1 < next.start_position().row {
            break;
        }
        parts.push(comment_text(text(prev, src)));
        next = prev;
    }
    parts.reverse();
    parts.join("\n").chars().take(DOC_CHARS).collect()
}

pub(super) fn read(root: Node, src: &[u8], lines: &[&str]) -> Read {
    let mut r = Read::default();
    for top in named(root) {
        match top.kind() {
            "comment" => r.cites.push(Cite { from: None, context: "comment", text: text(top, src).to_string() }),
            "toplevel_stmt" => {
                let Some(stmt) = child(top, "stmt").and_then(|s| s.named_child(0)) else { continue };
                let at = span(stmt);
                let doc = doc(top, src);
                let line = line_at(lines, at.0);
                let body = if doc.is_empty() { line.to_string() } else { format!("{doc}\n{line}") };
                let owner = statement(stmt, src, at, &body, lines, &mut r);
                cite(stmt, src, owner, &mut r);
            }
            _ => {}
        }
    }
    r
}

/// Reads one statement into `r` and returns the id tail its comments and strings belong to.
fn statement(stmt: Node, src: &[u8], at: (u32, u32), body: &str, lines: &[&str], r: &mut Read) -> Option<String> {
    let create = |r: &mut Read, name: String, attached: bool| -> String {
        r.objects.push(Object { name: name.clone(), span: at, body: body.to_string(), attached });
        name
    };
    let member = |r: &mut Read, table: &str, name: String| -> String {
        r.members.push(Member { table: table.to_string(), name: name.clone(), span: at, body: line_at(lines, at.0).to_string() });
        format!("{table}.{name}")
    };
    match stmt.kind() {
        "CreateStmt" => {
            let table = create(r, names::object(child(stmt, "qualified_name")?, src)?, false);
            if let Some(list) = child(stmt, "OptTableElementList") {
                elements(list, &table, src, lines, r);
            }
            Some(table)
        }
        "CreateAsStmt" => Some(create(r, names::object(child(child(stmt, "create_as_target")?, "qualified_name")?, src)?, false)),
        "ViewStmt" => {
            let view = create(r, names::object(child(stmt, "qualified_name")?, src)?, false);
            reads(stmt, &view, src, r);
            Some(view)
        }
        "CreateMatViewStmt" => {
            let view = create(r, names::object(child(child(stmt, "create_mv_target")?, "qualified_name")?, src)?, false);
            reads(stmt, &view, src, r);
            Some(view)
        }
        "CreateFunctionStmt" => Some(create(r, names::object(child(stmt, "func_name")?, src)?, false)),
        "DefineStmt" if opens_with(stmt, src, &["create", "type"]) => Some(create(r, names::object(child(stmt, "any_name")?, src)?, false)),
        "CreateSchemaStmt" => Some(create(r, names::object(child(stmt, "ColId")?, src)?, false)),
        "CreateSeqStmt" => Some(create(r, names::object(child(stmt, "qualified_name")?, src)?, false)),
        "IndexStmt" => {
            let table = create(r, relation(stmt, src)?, true);
            match child(stmt, "name").or_else(|| child(stmt, "opt_single_name")).and_then(|n| names::object(n, src)) {
                Some(index) => Some(member(r, &table, index)),
                None => Some(table),
            }
        }
        "CreatePolicyStmt" => {
            let table = create(r, names::object(child(stmt, "qualified_name")?, src)?, true);
            Some(member(r, &table, names::object(child(stmt, "name")?, src)?))
        }
        "CreateTrigStmt" => {
            let table = create(r, names::object(child(stmt, "qualified_name")?, src)?, true);
            let trigger = member(r, &table, names::object(child(stmt, "name")?, src)?);
            if let Some(function) = child(stmt, "func_name").and_then(|f| names::object(f, src)) {
                r.links.push(Link { from: trigger.clone(), to: function, kind: EdgeKind::Calls, context: "" });
            }
            Some(trigger)
        }
        // `AlterTableStmt`, and the `RENAME` and `SET SCHEMA` forms the grammar gives statements of their own.
        _ if opens_with(stmt, src, &["alter", "table"]) => {
            let table = create(r, relation(stmt, src)?, true);
            if let Some(list) = child(stmt, "alter_table_cmds") {
                elements(list, &table, src, lines, r);
            }
            // The renamed table is the same table under a new name: the new name is declared here, as a creation,
            // and depends on every file declaring the old one, so an FK written against it still reaches them.
            if let Some(renamed) = new_name(stmt, src, &table) {
                create(r, renamed.clone(), false);
                r.links.push(Link { from: renamed, to: table.clone(), kind: EdgeKind::References, context: "rename" });
            }
            Some(table)
        }
        _ => None,
    }
}

/// The name `ALTER TABLE t RENAME TO n` or `ALTER TABLE t SET SCHEMA s` gives `t`. PostgreSQL keeps a renamed
/// table in its schema. `RENAME COLUMN` and `RENAME CONSTRAINT` rename a member and do not match. The clause is
/// read from the statement's own keyword nodes, so the same words in a string or a comment rename nothing.
fn new_name(stmt: Node, src: &[u8], table: &str) -> Option<String> {
    if let Some(name) = clause(stmt, &["kw_rename", "kw_to"]).and_then(|n| names::object(n, src)) {
        return Some(match table.rsplit_once('/') {
            Some((schema, _)) => format!("{schema}/{name}"),
            None => name,
        });
    }
    clause(stmt, &["kw_set", "kw_schema"]).and_then(|n| names::object(n, src)).map(|schema| format!("{schema}/{}", names::bare(table)))
}

/// The `name` directly after `keywords`, among the statement's own children with comments set aside.
fn clause<'t>(stmt: Node<'t>, keywords: &[&str]) -> Option<Node<'t>> {
    let kids: Vec<Node> = named(stmt).into_iter().filter(|k| k.kind() != "comment").collect();
    kids.windows(keywords.len() + 1).find_map(|w| {
        let (words, last) = w.split_at(keywords.len());
        (words.iter().map(Node::kind).eq(keywords.iter().copied()) && last[0].kind() == "name").then_some(last[0])
    })
}

/// The table an `ALTER TABLE` or a `CREATE INDEX … ON` names, behind `ONLY` or `IF EXISTS` when written.
fn relation(stmt: Node, src: &[u8]) -> Option<String> {
    relation_name(child(stmt, "relation_expr")?, src)
}

/// A `relation_expr`'s name, written bare or behind `ONLY`.
fn relation_name(rel: Node, src: &[u8]) -> Option<String> {
    let q = child(rel, "qualified_name").or_else(|| child(rel, "extended_relation_expr").and_then(|x| child(x, "qualified_name")))?;
    names::object(q, src)
}

/// Columns and table constraints, in a `CREATE TABLE` element list or an `ALTER TABLE` command list.
fn elements(list: Node, table: &str, src: &[u8], lines: &[&str], r: &mut Read) {
    let mut found = Vec::new();
    find(list, &["columnDef", "TableConstraint"], &mut found);
    for e in found {
        if e.kind() == "TableConstraint" {
            let mut constraints = Vec::new();
            find(e, &["ConstraintElem"], &mut constraints);
            for c in constraints {
                references(c, table, src, r);
            }
            continue;
        }
        let Some(name) = child(e, "ColId").map(|c| names::fold(text(c, src))) else { continue };
        let at = span(e);
        let id = format!("{table}.{name}");
        r.members.push(Member { table: table.to_string(), name, span: at, body: line_at(lines, at.0).to_string() });
        let mut constraints = Vec::new();
        find(e, &["ColConstraintElem"], &mut constraints);
        for c in constraints {
            references(c, &id, src, r);
        }
    }
}

/// `REFERENCES t` in a column or a table constraint; a `CHECK`, a `DEFAULT` or a `UNIQUE` names no table.
fn references(elem: Node, from: &str, src: &[u8], r: &mut Read) {
    if !text(elem, src).to_lowercase().contains("references") {
        return;
    }
    if let Some(to) = child(elem, "qualified_name").and_then(|q| names::object(q, src)) {
        r.links.push(Link { from: from.to_string(), to, kind: EdgeKind::References, context: "references" });
    }
}

/// Every relation a view's query reads or joins, subqueries included. An unqualified name a `WITH` in the query
/// binds is that CTE, not a table, wherever in the query the `WITH` sits: a CTE cannot be schema-qualified, so a
/// qualified name is always a relation.
fn reads(stmt: Node, view: &str, src: &[u8], r: &mut Read) {
    let mut bound = Vec::new();
    ctes(stmt, src, &mut bound);
    let mut relations = Vec::new();
    find(stmt, &["relation_expr"], &mut relations);
    for to in relations.into_iter().filter_map(|rel| relation_name(rel, src)) {
        if !bound.contains(&to) {
            r.links.push(Link { from: view.to_string(), to, kind: EdgeKind::References, context: "from" });
        }
    }
}

/// The name of every CTE under `n`, a `WITH` inside another CTE's body included.
fn ctes(n: Node, src: &[u8], out: &mut Vec<String>) {
    for c in named(n) {
        if c.kind() == "common_table_expr" {
            out.extend(child(c, "name").and_then(|name| names::object(name, src)));
        }
        ctes(c, src, out);
    }
}

/// Comments, string literals and dollar-quoted bodies. PL/pgSQL is not parsed, so the ids a body cites are the one
/// thing in it the graph can follow, and they belong to the statement they sit in.
fn cite(stmt: Node, src: &[u8], owner: Option<String>, r: &mut Read) {
    let mut texts = Vec::new();
    find(stmt, &["comment", "string_literal", "dollar_quoted_string"], &mut texts);
    for t in texts {
        let context = if t.kind() == "comment" { "comment" } else { "string" };
        r.cites.push(Cite { from: owner.clone(), context, text: text(t, src).to_string() });
    }
}
