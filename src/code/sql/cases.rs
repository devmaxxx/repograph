//! PostgreSQL extraction, each case on migrations written to a temporary repository. A name resolves through
//! the index the walk builds from the files on disk, so the file under test sits beside its neighbours
//! rather than being handed to the extractor alone.
use super::header;
use crate::code::imports::Resolver;
use crate::code::index::header_for;
use crate::code::lang::{Family, Lang};
use crate::code::CodeExtractor;
use crate::config::Config;
use crate::model::{EdgeKind, Extraction, Extractor, Node};

const SCHEMA: &str = r#"-- The client register.
CREATE SCHEMA app;

CREATE TABLE app.clients (
  id uuid PRIMARY KEY,
  "Status" text NOT NULL,
  Owner_Id uuid,
  CONSTRAINT clients_owner FOREIGN KEY (owner_id) REFERENCES app.owners (id)
);--> statement-breakpoint
CREATE VIEW app.active AS SELECT id FROM app.clients;
CREATE MATERIALIZED VIEW app.active_mv AS SELECT id FROM app.active;
CREATE OR REPLACE FUNCTION app.touch() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  INSERT INTO app.audit VALUES (1);
  RETURN NEW;
END;
$$;
CREATE PROCEDURE app.sweep() LANGUAGE sql AS $$ DELETE FROM app.clients $$;
CREATE TYPE app.status AS ENUM ('new', 'gone');
CREATE SEQUENCE app.ticket;
CREATE TABLE app.snapshot AS SELECT 1 AS x;
CREATE TRIGGER clients_touch BEFORE UPDATE ON app.clients FOR EACH ROW EXECUTE FUNCTION app.touch();
CREATE POLICY clients_tenant ON app.clients USING (true);
CREATE INDEX clients_owner_idx ON app.clients (owner_id);
CREATE INDEX ON app.clients (id);
"#;

const ATTACH: &str = r#"ALTER TABLE ONLY app.clients ADD COLUMN IF NOT EXISTS note text, ADD CONSTRAINT u UNIQUE (id);
CREATE TRIGGER clients_log AFTER INSERT ON app.clients FOR EACH ROW EXECUTE FUNCTION app.touch();
GRANT SELECT ON app.clients TO app_user;
COMMENT ON TABLE app.clients IS 'x';
UPDATE app.clients SET id = id;
ALTER INDEX app.clients_owner_idx RENAME TO idx2;
ALTER TABLE app.clients RENAME COLUMN note TO memo;
"#;

const REFS: &str = r#"CREATE TABLE "App"."Visits" ("When" date, Kind text);
CREATE TABLE App.Visits (Room text);
CREATE TABLE app.visits2 (
  client_id uuid REFERENCES app.clients (id),
  owner uuid,
  FOREIGN KEY (owner) REFERENCES app.owners
);
CREATE OR REPLACE VIEW app.owned WITH (security_invoker = true) AS
  SELECT c.id FROM app.clients c LEFT JOIN (SELECT id FROM app.owners) o ON true;
CREATE TABLE loans (user_id int REFERENCES users, desk_id int REFERENCES public.desks);
-- FR-SEC-20: the chain.
CREATE FUNCTION app.chain() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  -- INV-05 holds here
  RAISE EXCEPTION 'ADR-032';
END;
$$;
"#;

const AUDIT: &str = "CREATE TABLE app.audit (id int);\n";
const PUBLIC: &str = "CREATE TABLE public.users (id int);\nCREATE TABLE desks (id int);\n";
const BODY: &str = "CREATE FUNCTION app.make() RETURNS void LANGUAGE plpgsql AS $$ BEGIN CREATE TABLE app.hidden (x int); END $$;\n";

struct Repo {
    dir: tempfile::TempDir,
}

impl Repo {
    fn new() -> Repo {
        Repo::with(&[("db/001.sql", SCHEMA), ("db/002.sql", ATTACH), ("db/003.sql", REFS), ("db/005.sql", AUDIT), ("db/010.sql", PUBLIC), ("db/020.sql", BODY)])
    }

    /// A repository of just these files, for a case whose neighbours are not the shared fixture's.
    fn with(files: &[(&str, &str)]) -> Repo {
        let dir = tempfile::tempdir().unwrap();
        for (rel, text) in files {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        Repo { dir }
    }

    fn resolver(&self) -> Resolver {
        let cfg = Config { code_globs: vec!["**/*.sql".to_string()], ..Config::default() };
        Resolver::new(self.dir.path(), &cfg).unwrap()
    }

    fn extract(&self, rel: &str) -> Extraction {
        let text = std::fs::read_to_string(self.dir.path().join(rel)).unwrap();
        CodeExtractor::new(self.resolver()).extract(rel, &text)
    }
}

fn ids(ex: &Extraction) -> Vec<&str> {
    ex.nodes.iter().map(|n| n.id.as_str()).collect()
}

fn edges(ex: &Extraction, kind: EdgeKind) -> Vec<(&str, &str, &str)> {
    ex.edges.iter().filter(|e| e.kind == kind).map(|e| (e.source.as_str(), e.target.as_str(), e.context.as_str())).collect()
}

fn node<'a>(ex: &'a Extraction, id: &str) -> &'a Node {
    ex.nodes.iter().find(|n| n.id == id).unwrap_or_else(|| panic!("{id} missing from {:?}", ids(ex)))
}

#[test]
fn every_created_object_is_a_symbol_named_by_its_schema() {
    let ex = Repo::new().extract("db/001.sql");
    for tail in ["app", "app/clients", "app/active", "app/active_mv", "app/touch", "app/sweep", "app/status", "app/ticket", "app/snapshot"] {
        let id = format!("sym:db/001.sql::{tail}");
        assert!(ids(&ex).contains(&id.as_str()), "{id} missing from {:?}", ids(&ex));
    }
    assert_eq!(node(&ex, "sym:db/001.sql::app/clients").label, "clients");
    assert!(edges(&ex, EdgeKind::Declares).contains(&("file:db/001.sql", "sym:db/001.sql::app/clients", "export")));
}

#[test]
fn columns_triggers_policies_and_indexes_are_members_of_their_table() {
    let ex = Repo::new().extract("db/001.sql");
    let mut members: Vec<&str> = edges(&ex, EdgeKind::Declares).into_iter()
        .filter(|(source, _, _)| *source == "sym:db/001.sql::app/clients")
        .map(|(_, target, _)| target)
        .collect();
    members.sort_unstable();
    assert_eq!(members, vec![
        "sym:db/001.sql::app/clients.Status",
        "sym:db/001.sql::app/clients.clients_owner_idx",
        "sym:db/001.sql::app/clients.clients_tenant",
        "sym:db/001.sql::app/clients.clients_touch",
        "sym:db/001.sql::app/clients.id",
        "sym:db/001.sql::app/clients.owner_id",
    ]);
    assert_eq!(node(&ex, "sym:db/001.sql::app/clients.Status").label, "clients.Status");
}

#[test]
fn an_unquoted_name_folds_to_lower_case_and_a_quoted_one_keeps_its_case() {
    let ex = Repo::new().extract("db/003.sql");
    for id in ["sym:db/003.sql::App/Visits", "sym:db/003.sql::App/Visits.When", "sym:db/003.sql::App/Visits.kind", "sym:db/003.sql::app/visits", "sym:db/003.sql::app/visits.room"] {
        assert!(ids(&ex).contains(&id), "{id} missing from {:?}", ids(&ex));
    }
}

#[test]
fn a_statement_spans_its_lines_and_its_body_is_its_doc_and_opening_line() {
    let ex = Repo::new().extract("db/001.sql");
    let clients = node(&ex, "sym:db/001.sql::app/clients");
    assert_eq!((clients.line, clients.end, clients.body.as_str()), (4, 9, "CREATE TABLE app.clients ("));
    assert_eq!(node(&ex, "sym:db/001.sql::app").body, "The client register.\nCREATE SCHEMA app;");
    // drizzle's `--> statement-breakpoint` closes the table's line; it describes nothing below it.
    assert_eq!(node(&ex, "sym:db/001.sql::app/active").body, "CREATE VIEW app.active AS SELECT id FROM app.clients;");
    assert_eq!(ex.nodes.iter().filter(|n| n.id == "sym:db/001.sql::app/clients").count(), 1);
}

#[test]
fn an_attaching_statement_declares_the_table_in_its_own_file() {
    let ex = Repo::new().extract("db/002.sql");
    let mut got = ids(&ex);
    got.sort_unstable();
    assert_eq!(got, vec!["file:db/002.sql", "sym:db/002.sql::app/clients", "sym:db/002.sql::app/clients.clients_log", "sym:db/002.sql::app/clients.note"]);
    let clients = node(&ex, "sym:db/002.sql::app/clients");
    assert_eq!((clients.line, clients.end), (1, 1));
}

#[test]
fn a_dollar_quoted_body_declares_nothing() {
    let ex = Repo::new().extract("db/020.sql");
    assert!(ids(&ex).contains(&"sym:db/020.sql::app/make"), "{:?}", ids(&ex));
    assert!(!ids(&ex).iter().any(|id| id.contains("hidden")), "{:?}", ids(&ex));
}

#[test]
fn the_header_lists_what_a_file_creates_or_alters_and_no_member() {
    let attach = header(ATTACH);
    assert!(attach.scope.is_empty());
    assert_eq!(attach.top.iter().map(String::as_str).collect::<Vec<_>>(), vec!["app/clients"]);
    assert_eq!(
        header(SCHEMA).top.iter().map(String::as_str).collect::<Vec<_>>(),
        vec!["app", "app/active", "app/active_mv", "app/clients", "app/snapshot", "app/status", "app/sweep", "app/ticket", "app/touch"]
    );
    assert_eq!(header_for(Lang::Sql, "db/001.sql", SCHEMA), Some(header(SCHEMA)));
}

#[test]
fn the_sql_index_names_every_file_that_creates_or_alters_a_name() {
    let repo = Repo::new();
    let resolver = repo.resolver();
    let index = resolver.index(Family::Sql).expect("the globs reach .sql");
    assert_eq!(index.files("app/clients"), vec!["db/001.sql", "db/002.sql"]);
    assert_eq!(index.files("public/users"), vec!["db/010.sql"]);
    assert!(index.files("app/clients.note").is_empty());
}

#[test]
fn an_attaching_file_references_every_other_file_declaring_the_table() {
    let repo = Repo::new();
    let migration_002 = repo.extract("db/002.sql");
    let refs = edges(&migration_002, EdgeKind::References);
    assert!(refs.contains(&("sym:db/002.sql::app/clients", "sym:db/001.sql::app/clients", "alter")), "{refs:?}");
    assert!(!refs.iter().any(|(_, target, _)| *target == "sym:db/002.sql::app/clients"), "{refs:?}");
    // The creating migration names nothing it did not write.
    let migration_001 = repo.extract("db/001.sql");
    assert!(!edges(&migration_001, EdgeKind::References).iter().any(|(_, _, context)| *context == "alter"));
}

#[test]
fn a_trigger_calls_its_function_wherever_it_is_declared() {
    let repo = Repo::new();
    let migration_001 = repo.extract("db/001.sql");
    assert!(edges(&migration_001, EdgeKind::Calls).contains(&("sym:db/001.sql::app/clients.clients_touch", "sym:db/001.sql::app/touch", "")));
    let migration_002 = repo.extract("db/002.sql");
    assert_eq!(edges(&migration_002, EdgeKind::Calls), vec![("sym:db/002.sql::app/clients.clients_log", "sym:db/001.sql::app/touch", "")]);
}

#[test]
fn a_foreign_key_references_every_file_declaring_its_table_and_an_undeclared_table_is_not_linked() {
    let migration_003 = Repo::new().extract("db/003.sql");
    let refs = edges(&migration_003, EdgeKind::References);
    assert!(refs.contains(&("sym:db/003.sql::app/visits2.client_id", "sym:db/001.sql::app/clients", "references")), "{refs:?}");
    assert!(refs.contains(&("sym:db/003.sql::app/visits2.client_id", "sym:db/002.sql::app/clients", "references")), "{refs:?}");
    assert!(!refs.iter().any(|(_, target, _)| target.contains("owners")), "{refs:?}");
}

#[test]
fn a_view_references_the_relations_it_reads() {
    let repo = Repo::new();
    let migration_001 = repo.extract("db/001.sql");
    let own = edges(&migration_001, EdgeKind::References);
    assert!(own.contains(&("sym:db/001.sql::app/active", "sym:db/001.sql::app/clients", "from")), "{own:?}");
    assert!(own.contains(&("sym:db/001.sql::app/active_mv", "sym:db/001.sql::app/active", "from")), "{own:?}");
    let migration_003 = repo.extract("db/003.sql");
    let other = edges(&migration_003, EdgeKind::References);
    assert!(other.contains(&("sym:db/003.sql::app/owned", "sym:db/002.sql::app/clients", "from")), "{other:?}");
}

#[test]
fn an_unqualified_name_is_itself_or_public_and_a_public_name_is_also_unqualified() {
    let migration_003 = Repo::new().extract("db/003.sql");
    let refs = edges(&migration_003, EdgeKind::References);
    assert!(refs.contains(&("sym:db/003.sql::loans.user_id", "sym:db/010.sql::public/users", "references")), "{refs:?}");
    assert!(refs.contains(&("sym:db/003.sql::loans.desk_id", "sym:db/010.sql::desks", "references")), "{refs:?}");
}

#[test]
fn a_body_is_text_the_graph_cites_ids_from_and_reads_no_name_in() {
    let repo = Repo::new();
    let migration_001 = repo.extract("db/001.sql");
    let schema = edges(&migration_001, EdgeKind::References);
    assert!(!schema.iter().any(|(_, target, _)| target.contains("app/audit")), "{schema:?}");
    let migration_003 = repo.extract("db/003.sql");
    let refs = edges(&migration_003, EdgeKind::References);
    for want in [("sym:db/003.sql::app/chain", "INV-05", "string"), ("sym:db/003.sql::app/chain", "ADR-032", "string"), ("file:db/003.sql", "FR-SEC-20", "comment")] {
        assert!(refs.contains(&want), "{want:?} missing from {refs:?}");
    }
}

#[test]
fn a_quoted_mixed_case_table_and_its_unquoted_spelling_are_two_tables() {
    let repo = Repo::with(&[
        ("db/001.sql", "CREATE TABLE \"Clients\" (id int);\n"),
        ("db/002.sql", "CREATE TABLE clients (id int);\n"),
        ("db/003.sql", "CREATE TABLE visits (a int REFERENCES \"Clients\", b int REFERENCES Clients);\n"),
    ]);
    let migration_003 = repo.extract("db/003.sql");
    let refs = edges(&migration_003, EdgeKind::References);
    assert!(refs.contains(&("sym:db/003.sql::visits.a", "sym:db/001.sql::Clients", "references")), "{refs:?}");
    assert!(refs.contains(&("sym:db/003.sql::visits.b", "sym:db/002.sql::clients", "references")), "{refs:?}");
    assert!(!refs.contains(&("sym:db/003.sql::visits.a", "sym:db/002.sql::clients", "references")), "{refs:?}");
    assert!(!refs.contains(&("sym:db/003.sql::visits.b", "sym:db/001.sql::Clients", "references")), "{refs:?}");
}

#[test]
fn a_function_replaced_in_a_later_migration_is_called_in_every_file_that_declares_it() {
    let repo = Repo::with(&[
        ("db/001.sql", "CREATE FUNCTION app.touch() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END $$;\nCREATE TABLE app.clients (id int);\n"),
        ("db/004.sql", "CREATE OR REPLACE FUNCTION app.touch() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.at = now(); RETURN NEW; END $$;\n"),
        ("db/005.sql", "CREATE TRIGGER clients_touch BEFORE UPDATE ON app.clients FOR EACH ROW EXECUTE FUNCTION app.touch();\n"),
    ]);
    assert_eq!(repo.resolver().index(Family::Sql).unwrap().files("app/touch"), vec!["db/001.sql", "db/004.sql"]);
    let migration_005 = repo.extract("db/005.sql");
    let mut calls = edges(&migration_005, EdgeKind::Calls);
    calls.sort_unstable();
    assert_eq!(calls, vec![
        ("sym:db/005.sql::app/clients.clients_touch", "sym:db/001.sql::app/touch", ""),
        ("sym:db/005.sql::app/clients.clients_touch", "sym:db/004.sql::app/touch", ""),
    ]);
    // A replacement is a creation, not an attachment, so it names no other file.
    let migration_004 = repo.extract("db/004.sql");
    assert!(edges(&migration_004, EdgeKind::References).is_empty());
}

#[test]
fn a_renamed_table_is_declared_under_its_new_name_and_references_its_old_one() {
    let repo = Repo::with(&[
        ("db/001.sql", "CREATE TABLE app.clients (id int);\n"),
        ("db/002.sql", "ALTER TABLE app.clients RENAME TO customers;\nALTER TABLE app.customers SET SCHEMA crm;\n"),
        ("db/003.sql", "CREATE TABLE app.visits (c int REFERENCES app.customers, d int REFERENCES crm.customers);\n"),
    ]);
    let resolver = repo.resolver();
    let index = resolver.index(Family::Sql).expect("the globs reach .sql");
    assert_eq!(index.files("app/clients"), vec!["db/001.sql", "db/002.sql"]);
    assert_eq!(index.files("app/customers"), vec!["db/002.sql"]);
    assert_eq!(index.files("crm/customers"), vec!["db/002.sql"]);
    let migration_002 = repo.extract("db/002.sql");
    let renames = edges(&migration_002, EdgeKind::References);
    for want in [
        ("sym:db/002.sql::app/customers", "sym:db/001.sql::app/clients", "rename"),
        ("sym:db/002.sql::crm/customers", "sym:db/002.sql::app/customers", "rename"),
    ] {
        assert!(renames.contains(&want), "{want:?} missing from {renames:?}");
    }
    let migration_003 = repo.extract("db/003.sql");
    let fks = edges(&migration_003, EdgeKind::References);
    for want in [
        ("sym:db/003.sql::app/visits.c", "sym:db/002.sql::app/customers", "references"),
        ("sym:db/003.sql::app/visits.d", "sym:db/002.sql::crm/customers", "references"),
    ] {
        assert!(fks.contains(&want), "{want:?} missing from {fks:?}");
    }
    assert!(!fks.iter().any(|(_, target, _)| target.ends_with("app/clients")), "{fks:?}");
}
