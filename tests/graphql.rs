//! GraphQL through the binary, on synthetic documents: `impact` on a fragment and on a schema type, a trace through
//! a spread chain, `changes` on a field, and the widening that re-reads a document when a fragment it spreads first
//! appears.
mod common;

use std::path::Path;
use std::process::Command;

const OPS: &str = r##"# The shelf screen.
query GetShelf($id: ID!, $filter: BookInput) {
  shelf(id: $id) {
    ...ShelfFields
    books { ... on Book { title } }
  }
}

mutation AddBook($input: BookInput!) @auth(role: "writer") {
  addBook(input: $input) { id }
}

subscription BookAdded {
  bookAdded { ...BookFields }
}

{ viewer { ...BookFields } }
"##;

const FRAGMENTS: &str = r##"fragment ShelfFields on Shelf {
  id
  books { ...BookFields }
}

# FR-WEB-01 names the fields a book card shows.
fragment BookFields on Book {
  id
  title
}
"##;

const SCHEMA: &str = r##""""A shelf of books, FR-WEB-02."""
type Shelf implements Node & Named @key(fields: "id") {
  id: ID!
  books(first: Int, genre: Genre): [Book!]! @auth(role: "reader")
}

interface Node {
  id: ID!
}

interface Named implements Node {
  id: ID!
  name: String
}

input BookInput {
  title: String!
  genre: Genre
}

enum Genre {
  SCIFI
  POETRY
}

union Item = Book | Shelf

scalar Date

directive @auth(role: String) on FIELD_DEFINITION | MUTATION

type Book {
  id: ID!
  title: String
}
"##;

const EXT: &str = "extend type Shelf {\n  keeper: Keeper\n}\n\ntype Keeper {\n  id: ID!\n}\n";

fn write(repo: &Path, rel: &str, text: &str) {
    let path = repo.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// The repository's own config names GraphQL: the defaults do not read GraphQL yet. The store is ignored so that
/// `changes` diffs the documents and nothing else.
fn repo(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "repograph.toml", "code_globs = [\"**/*.gql\", \"**/*.graphql\"]\n");
    write(dir.path(), ".gitignore", ".repograph/\n");
    for (rel, text) in files {
        write(dir.path(), rel, text);
    }
    dir
}

fn all() -> tempfile::TempDir {
    repo(&[("app/shelf.gql", OPS), ("app/fragments.gql", FRAGMENTS), ("schema/schema.graphql", SCHEMA), ("schema/ext.graphql", EXT)])
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
fn impact_on_a_fragment_names_what_spreads_it_directly_and_through_another_fragment() {
    let dir = all();
    stdout(dir.path(), &["build"]);
    let out = stdout(dir.path(), &["impact", "fragment/BookFields"]);
    assert!(out.starts_with("sym:app/fragments.gql::fragment/BookFields  "), "{out}");
    for id in ["sym:app/fragments.gql::fragment/ShelfFields", "sym:app/shelf.gql::subscription/BookAdded", "file:app/shelf.gql", "sym:app/shelf.gql::query/GetShelf"] {
        assert!(out.contains(id), "{id} missing:\n{out}");
    }
    // A person asks by the name they spread, not by its namespace.
    let by_label = stdout(dir.path(), &["impact", "BookFields"]);
    assert!(by_label.starts_with("sym:app/fragments.gql::fragment/BookFields  "), "{by_label}");
}

#[test]
fn trace_runs_from_an_operation_through_a_fragment_to_the_fragment_it_spreads_and_not_back() {
    let dir = all();
    stdout(dir.path(), &["build"]);
    let there = stdout(dir.path(), &["trace", "query/GetShelf", "fragment/BookFields"]);
    for id in ["sym:app/shelf.gql::query/GetShelf", "sym:app/fragments.gql::fragment/ShelfFields", "sym:app/fragments.gql::fragment/BookFields"] {
        assert!(there.contains(id), "{id} missing:\n{there}");
    }
    let back = common::run(dir.path(), &["trace", "fragment/BookFields", "query/GetShelf"]);
    assert_eq!(back.status.code(), Some(3), "{}", String::from_utf8_lossy(&back.stdout));
}

#[test]
fn impact_on_a_schema_type_names_the_fields_and_documents_that_use_it() {
    let dir = all();
    stdout(dir.path(), &["build"]);
    let out = stdout(dir.path(), &["impact", "Book"]);
    assert!(out.starts_with("sym:schema/schema.graphql::Book  "), "{out}");
    for id in ["sym:schema/schema.graphql::Shelf.books", "sym:schema/schema.graphql::Item", "sym:app/fragments.gql::fragment/BookFields", "sym:app/shelf.gql::query/GetShelf"] {
        assert!(out.contains(id), "{id} missing:\n{out}");
    }
}

#[test]
fn changes_names_the_field_a_hunk_touches_in_place_of_its_type() {
    let dir = all();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "base"]);
    stdout(dir.path(), &["build"]);
    write(dir.path(), "schema/schema.graphql", &SCHEMA.replace("  title: String\n}", "  title: String!\n}"));
    stdout(dir.path(), &["update"]);
    let out = stdout(dir.path(), &["changes", "--base", "HEAD"]);
    let touched: Vec<&str> = out.lines().filter_map(|l| l.strip_prefix("  ")).filter_map(|l| l.split("  ").next()).filter(|id| id.starts_with("sym:")).collect();
    assert_eq!(touched, vec!["sym:schema/schema.graphql::Book.title"], "{out}");
}

#[test]
fn a_new_fragment_re_reads_the_documents_that_spread_it() {
    let dir = repo(&[("app/shelf.gql", OPS), ("schema/schema.graphql", SCHEMA)]);
    stdout(dir.path(), &["build"]);
    let before = stdout(dir.path(), &["explain", "sym:app/shelf.gql::subscription/BookAdded"]);
    assert!(!before.contains("Calls →"), "{before}");
    write(dir.path(), "app/book.gql", "fragment BookFields on Book {\n  id\n}\n");
    stdout(dir.path(), &["update"]);
    let out = stdout(dir.path(), &["explain", "sym:app/book.gql::fragment/BookFields"]);
    // Only a re-read of the unchanged shelf.gql writes these: its spreads resolve to nothing until book.gql exists.
    assert!(out.contains("Calls ← sym:app/shelf.gql::subscription/BookAdded"), "{out}");
    assert!(out.contains("Calls ← file:app/shelf.gql"), "{out}");
}
