//! PostgreSQL through the binary, on synthetic migrations: `impact`, `trace` and `changes`, and the widening that
//! re-reads an unchanged migration when a table it references gains or loses a declaring file.
mod common;

use std::path::Path;
use std::process::Command;

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

const ATTACH: &str = "ALTER TABLE ONLY app.clients ADD COLUMN IF NOT EXISTS note text;\nCREATE TRIGGER clients_log AFTER INSERT ON app.clients FOR EACH ROW EXECUTE FUNCTION app.touch();\n";

const VISITS: &str = "CREATE TABLE app.visits (\n  client_id uuid REFERENCES app.clients (id)\n);\nCREATE VIEW app.owned AS SELECT c.id FROM app.clients c;\n";

fn write(repo: &Path, rel: &str, text: &str) {
    let path = repo.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// The repository's own config names `.sql`: the defaults do not read SQL yet. The store is
/// ignored so that `changes` diffs the migrations and nothing else.
fn repo(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "repograph.toml", "code_globs = [\"**/*.sql\"]\n");
    write(dir.path(), ".gitignore", ".repograph/\n");
    for (rel, text) in files {
        write(dir.path(), rel, text);
    }
    dir
}

fn stdout(repo: &Path, args: &[&str]) -> String {
    let out = common::run(repo, args);
    assert!(out.status.success(), "{args:?} exited {:?}: {}", out.status.code(), String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

/// A hook exports its own repository into what it runs, and those variables outrank `-C`.
fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(repo)
        .env_remove("GIT_DIR").env_remove("GIT_WORK_TREE").env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR").env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        .args(["-c", "user.name=repograph tests", "-c", "user.email=tests@example.invalid", "-c", "commit.gpgsign=false"])
        .args(args).output().unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn impact_on_a_table_names_the_migrations_that_alter_reference_or_read_it() {
    let dir = repo(&[("db/001.sql", SCHEMA), ("db/002.sql", ATTACH), ("db/003.sql", VISITS)]);
    stdout(dir.path(), &["build"]);
    // `alter`, `references` and `from` are `References` edges; `walks` is what lets `impact` follow them.
    let out = stdout(dir.path(), &["impact", "sym:db/001.sql::app/clients"]);
    assert!(out.starts_with("sym:db/001.sql::app/clients  "), "{out}");
    for id in ["sym:db/002.sql::app/clients", "sym:db/003.sql::app/visits.client_id", "sym:db/003.sql::app/owned", "sym:db/001.sql::app/active"] {
        assert!(out.contains(id), "{id} missing:\n{out}");
    }
}

#[test]
fn a_table_reaches_through_its_members_and_a_schema_contains_none_of_its_tables() {
    let dir = repo(&[("db/001.sql", SCHEMA), ("db/002.sql", ATTACH)]);
    stdout(dir.path(), &["build"]);
    let down = stdout(dir.path(), &["impact", "--down", "sym:db/001.sql::app/clients"]);
    assert!(down.contains("sym:db/001.sql::app/touch"), "{down}");
    let up = stdout(dir.path(), &["impact", "sym:db/001.sql::app/touch"]);
    assert!(up.contains("sym:db/001.sql::app/clients.clients_touch"), "{up}");
    assert!(up.contains("sym:db/002.sql::app/clients.clients_log"), "{up}");
    let schema = stdout(dir.path(), &["impact", "sym:db/001.sql::app"]);
    assert!(schema.starts_with("sym:db/001.sql::app  "), "{schema}");
    assert!(!schema.contains("app/clients"), "a schema is a prefix of a name, not a container: {schema}");
}

#[test]
fn trace_runs_from_a_table_through_its_trigger_to_the_function_and_not_back() {
    let dir = repo(&[("db/001.sql", SCHEMA)]);
    stdout(dir.path(), &["build"]);
    let there = stdout(dir.path(), &["trace", "sym:db/001.sql::app/clients", "sym:db/001.sql::app/touch"]);
    assert!(there.contains("sym:db/001.sql::app/clients") && there.contains("sym:db/001.sql::app/touch"), "{there}");
    let back = common::run(dir.path(), &["trace", "sym:db/001.sql::app/touch", "sym:db/001.sql::app/clients"]);
    assert_eq!(back.status.code(), Some(3), "{}", String::from_utf8_lossy(&back.stdout));
}

#[test]
fn changes_names_a_column_in_its_tables_place_and_a_schema_beside_it() {
    let dir = repo(&[("db/001.sql", SCHEMA)]);
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "base"]);
    stdout(dir.path(), &["build"]);
    let edited = SCHEMA.replace("CREATE SCHEMA app;", "CREATE SCHEMA IF NOT EXISTS app;").replace("\"Status\" text NOT NULL,", "\"Status\" text,");
    write(dir.path(), "db/001.sql", &edited);
    stdout(dir.path(), &["update"]);
    let out = stdout(dir.path(), &["changes", "--base", "HEAD"]);
    let touched: Vec<&str> = out.lines().filter_map(|l| l.strip_prefix("  ")).filter_map(|l| l.split("  ").next()).filter(|id| id.starts_with("sym:")).collect();
    assert!(touched.contains(&"sym:db/001.sql::app/clients.Status"), "{out}");
    assert!(touched.contains(&"sym:db/001.sql::app"), "{out}");
    assert!(!touched.contains(&"sym:db/001.sql::app/clients"), "{out}");
}

#[test]
fn a_new_migration_altering_a_table_re_reads_the_migrations_that_reference_it() {
    let dir = repo(&[("db/001.sql", SCHEMA), ("db/003.sql", VISITS)]);
    stdout(dir.path(), &["build"]);
    write(dir.path(), "db/004.sql", "ALTER TABLE app.clients ADD COLUMN note text;\n");
    stdout(dir.path(), &["update"]);
    let added = stdout(dir.path(), &["explain", "sym:db/004.sql::app/clients"]);
    assert!(added.contains("References → sym:db/001.sql::app/clients  [alter]"), "{added}");
    // Only a re-read of the unchanged 003 writes these: its names now resolve to 004 as well.
    assert!(added.contains("References ← sym:db/003.sql::app/visits.client_id  [references]"), "{added}");
    assert!(added.contains("References ← sym:db/003.sql::app/owned  [from]"), "{added}");
    std::fs::remove_file(dir.path().join("db/004.sql")).unwrap();
    stdout(dir.path(), &["update"]);
    let visits = stdout(dir.path(), &["explain", "sym:db/003.sql::app/visits.client_id"]);
    assert!(visits.contains("References → sym:db/001.sql::app/clients  [references]"), "{visits}");
    assert!(!visits.contains("db/004.sql"), "{visits}");
}
