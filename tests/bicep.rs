//! Bicep through the binary: a module file is impacted by every file that deploys it, a trace crosses
//! module files, and `changes` names the declaration a hunk sits in.

mod common;

use common::{git, ok, path, write};

const DB: &str = "param sku string = 'B1'\nresource server 'Microsoft.DBforPostgreSQL/flexibleServers@2022-12-01' = {\n  name: 'pg'\n  sku: {\n    name: sku\n  }\n}\noutput id string = server.id\n";

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    // Bicep is read behind an explicit glob until its readings pass (spec §6). No documents, so the
    // graph holds only what these tests pin.
    write(r, "repograph.toml", "doc_globs = []\ncode_globs = [\"**/*.bicep\"]\n");
    write(r, ".gitignore", "/.repograph\n");
    write(r, "stack.bicep", "param env string\nmodule app './parts/web.bicep' = {\n  name: 'app-${env}'\n}\nmodule store 'parts/pg.bicep' = {\n  name: 'store'\n}\n");
    write(r, "parts/web.bicep", "module db './pg.bicep' = {\n  name: 'db'\n}\n");
    write(r, "parts/pg.bicep", DB);
    dir
}

#[test]
fn a_module_file_is_impacted_by_every_file_that_deploys_it() {
    let dir = repo();
    ok(dir.path(), &["build"]);
    let out = ok(dir.path(), &["impact", "parts/pg.bicep", "--depth", "3"]);
    assert!(out.contains("sym:stack.bicep::store"), "{out}");
    assert!(out.contains("sym:parts/web.bicep::db"), "{out}");
    assert!(out.contains("importers (2): parts/web.bicep, stack.bicep"), "{out}");
    // `store` deploys pg.bicep directly and never touches web.bicep, so it is not impacted by it.
    let out = ok(dir.path(), &["impact", "parts/web.bicep", "--depth", "3"]);
    assert!(out.contains("sym:stack.bicep::app"), "{out}");
    assert!(!out.contains("sym:stack.bicep::store"), "{out}");
}

#[test]
fn a_trace_crosses_a_module_into_the_module_it_deploys() {
    let dir = repo();
    ok(dir.path(), &["build"]);
    let out = ok(dir.path(), &["trace", "sym:stack.bicep::app", "parts/pg.bicep"]);
    let ids = path(&out);
    // A module is entered through what it declares, so the chain runs from declaration to declaration.
    assert_eq!(ids.len(), 3, "{out}");
    assert_eq!(ids[..2], ["sym:stack.bicep::app", "sym:parts/web.bicep::db"], "{out}");
    assert!(ids[2].starts_with("sym:parts/pg.bicep::"), "{out}");
}

#[test]
fn changes_names_the_declaration_a_hunk_sits_in() {
    let dir = repo();
    let r = dir.path();
    git(r, &["init", "-q"]);
    git(r, &["add", "-A"]);
    git(r, &["commit", "-q", "-m", "base"]);
    write(r, "parts/pg.bicep", &DB.replace("name: 'pg'", "name: 'pg-main'"));
    ok(r, &["build"]);
    let out = ok(r, &["changes", "--base", "HEAD"]);
    assert!(out.starts_with("changed: 1 symbol in 1 file"), "{out}");
    assert!(out.contains("sym:parts/pg.bicep::server"), "{out}");
    assert!(out.contains("sym:parts/pg.bicep::output/id"), "the output reading the server is affected: {out}");
}
