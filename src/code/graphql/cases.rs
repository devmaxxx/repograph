//! GraphQL extraction, each case on documents written to a temporary repository. A spread or a type resolves
//! through the index the walk builds from every document on disk, so the document under test sits beside
//! the ones it names.
use super::header;
use crate::code::imports::Resolver;
use crate::code::index::header_for;
use crate::code::lang::{Family, Lang};
use crate::code::CodeExtractor;
use crate::config::Config;
use crate::model::{EdgeKind, Extraction, Extractor, Node};

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

struct Repo {
    dir: tempfile::TempDir,
}

impl Repo {
    fn new() -> Repo {
        Repo::with(&[("app/shelf.gql", OPS), ("app/fragments.gql", FRAGMENTS), ("schema/schema.graphql", SCHEMA), ("schema/ext.graphql", EXT)])
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
        let cfg = Config { code_globs: vec!["**/*.gql".to_string(), "**/*.graphql".to_string()], ..Config::default() };
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

fn top(source: &str) -> Vec<String> {
    header(source).top.into_iter().collect()
}

#[test]
fn operations_and_fragments_live_under_their_own_namespaces() {
    let repo = Repo::new();
    let ops = repo.extract("app/shelf.gql");
    let mut symbols: Vec<&str> = ids(&ops).into_iter().filter(|id| id.starts_with("sym:")).collect();
    symbols.sort_unstable();
    assert_eq!(symbols, vec!["sym:app/shelf.gql::mutation/AddBook", "sym:app/shelf.gql::query/GetShelf", "sym:app/shelf.gql::subscription/BookAdded"]);
    assert_eq!(node(&ops, "sym:app/shelf.gql::query/GetShelf").label, "GetShelf");
    let fragments = repo.extract("app/fragments.gql");
    for id in ["sym:app/fragments.gql::fragment/ShelfFields", "sym:app/fragments.gql::fragment/BookFields"] {
        assert!(ids(&fragments).contains(&id), "{id} missing from {:?}", ids(&fragments));
    }
}

#[test]
fn schema_definitions_are_symbols_and_fields_are_members() {
    let ex = Repo::new().extract("schema/schema.graphql");
    for tail in ["Shelf", "Node", "Named", "BookInput", "Genre", "Item", "Date", "directive/auth", "Book", "Shelf.id", "Shelf.books", "Named.name", "BookInput.title", "BookInput.genre", "Book.title"] {
        let id = format!("sym:schema/schema.graphql::{tail}");
        assert!(ids(&ex).contains(&id.as_str()), "{id} missing from {:?}", ids(&ex));
    }
    assert!(!ids(&ex).iter().any(|id| id.ends_with("Genre.SCIFI") || id.contains("books.first")), "{:?}", ids(&ex));
    assert!(edges(&ex, EdgeKind::Declares).contains(&("sym:schema/schema.graphql::Shelf", "sym:schema/schema.graphql::Shelf.books", "export")));
    assert_eq!(node(&ex, "sym:schema/schema.graphql::Shelf.books").label, "Shelf.books");
}

#[test]
fn a_description_or_the_comments_above_are_the_body_and_the_name_line_starts_the_span() {
    let repo = Repo::new();
    let ops = repo.extract("app/shelf.gql");
    let query = node(&ops, "sym:app/shelf.gql::query/GetShelf");
    assert_eq!((query.line, query.end, query.body.as_str()), (2, 7, "The shelf screen.\nquery GetShelf($id: ID!, $filter: BookInput) {"));
    let schema = repo.extract("schema/schema.graphql");
    let shelf = node(&schema, "sym:schema/schema.graphql::Shelf");
    assert_eq!((shelf.line, shelf.end), (2, 5));
    assert_eq!(shelf.body, "A shelf of books, FR-WEB-02.\ntype Shelf implements Node & Named @key(fields: \"id\") {");
    let fragments = repo.extract("app/fragments.gql");
    assert_eq!(node(&fragments, "sym:app/fragments.gql::fragment/BookFields").body, "FR-WEB-01 names the fields a book card shows.\nfragment BookFields on Book {");
}

#[test]
fn an_extension_declares_the_type_and_its_fields_in_its_own_file() {
    let ex = Repo::new().extract("schema/ext.graphql");
    for id in ["sym:schema/ext.graphql::Shelf", "sym:schema/ext.graphql::Shelf.keeper", "sym:schema/ext.graphql::Keeper", "sym:schema/ext.graphql::Keeper.id"] {
        assert!(ids(&ex).contains(&id), "{id} missing from {:?}", ids(&ex));
    }
}

#[test]
fn the_header_lists_every_definition_and_extension_and_no_field() {
    assert_eq!(top(OPS), vec!["mutation/AddBook", "query/GetShelf", "subscription/BookAdded"]);
    assert_eq!(top(SCHEMA), vec!["Book", "BookInput", "Date", "Genre", "Item", "Named", "Node", "Shelf", "directive/auth"]);
    assert_eq!(top(EXT), vec!["Keeper", "Shelf"]);
    assert!(header(FRAGMENTS).scope.is_empty());
    assert_eq!(header_for(Lang::GraphQl, "app/shelf.gql", OPS), Some(header(OPS)));
}

#[test]
fn the_graphql_index_is_repository_wide() {
    let repo = Repo::new();
    let resolver = repo.resolver();
    let index = resolver.index(Family::GraphQl).expect("the globs reach .gql and .graphql");
    assert_eq!(index.files("Shelf"), vec!["schema/ext.graphql", "schema/schema.graphql"]);
    assert_eq!(index.files("fragment/BookFields"), vec!["app/fragments.gql"]);
}

#[test]
fn a_spread_calls_the_fragment_in_whichever_file_declares_it() {
    let repo = Repo::new();
    let ops = repo.extract("app/shelf.gql");
    assert_eq!(edges(&ops, EdgeKind::Calls), vec![
        ("file:app/shelf.gql", "sym:app/fragments.gql::fragment/BookFields", ""),
        ("sym:app/shelf.gql::query/GetShelf", "sym:app/fragments.gql::fragment/ShelfFields", ""),
        ("sym:app/shelf.gql::subscription/BookAdded", "sym:app/fragments.gql::fragment/BookFields", ""),
    ]);
    let fragments = repo.extract("app/fragments.gql");
    assert!(edges(&fragments, EdgeKind::Calls)
        .contains(&("sym:app/fragments.gql::fragment/ShelfFields", "sym:app/fragments.gql::fragment/BookFields", "")));
}

#[test]
fn type_conditions_variables_fields_and_arguments_reference_declared_schema_types() {
    let repo = Repo::new();
    let ops_ex = repo.extract("app/shelf.gql");
    let ops = edges(&ops_ex, EdgeKind::References);
    for want in [
        ("sym:app/shelf.gql::query/GetShelf", "sym:schema/schema.graphql::BookInput", "variable"),
        ("sym:app/shelf.gql::query/GetShelf", "sym:schema/schema.graphql::Book", "on"),
        ("sym:app/shelf.gql::mutation/AddBook", "sym:schema/schema.graphql::BookInput", "variable"),
    ] {
        assert!(ops.contains(&want), "{want:?} missing from {ops:?}");
    }
    // A built-in scalar is declared by no document, so nothing links to it.
    assert!(!ops.iter().any(|(_, target, _)| target.ends_with("::ID")), "{ops:?}");
    let fragments_ex = repo.extract("app/fragments.gql");
    let fragments = edges(&fragments_ex, EdgeKind::References);
    for target in ["sym:schema/schema.graphql::Shelf", "sym:schema/ext.graphql::Shelf"] {
        assert!(fragments.contains(&("sym:app/fragments.gql::fragment/ShelfFields", target, "on")), "{target} missing from {fragments:?}");
    }
    let schema_ex = repo.extract("schema/schema.graphql");
    let schema = edges(&schema_ex, EdgeKind::References);
    for want in [
        ("sym:schema/schema.graphql::Shelf.books", "sym:schema/schema.graphql::Book", "type"),
        ("sym:schema/schema.graphql::Shelf.books", "sym:schema/schema.graphql::Genre", "argument"),
        ("sym:schema/schema.graphql::BookInput.genre", "sym:schema/schema.graphql::Genre", "type"),
        ("sym:schema/schema.graphql::Item", "sym:schema/schema.graphql::Book", "member"),
        ("sym:schema/schema.graphql::Item", "sym:schema/ext.graphql::Shelf", "member"),
    ] {
        assert!(schema.contains(&want), "{want:?} missing from {schema:?}");
    }
}

#[test]
fn implements_extends_and_a_defined_directive_decorates() {
    let repo = Repo::new();
    let schema = repo.extract("schema/schema.graphql");
    assert_eq!(edges(&schema, EdgeKind::Extends), vec![
        ("sym:schema/schema.graphql::Named", "sym:schema/schema.graphql::Node", ""),
        ("sym:schema/schema.graphql::Shelf", "sym:schema/schema.graphql::Named", ""),
        ("sym:schema/schema.graphql::Shelf", "sym:schema/schema.graphql::Node", ""),
    ]);
    // `@key` is defined nowhere in the repository, so it decorates nothing.
    assert_eq!(edges(&schema, EdgeKind::DecoratedBy), vec![("sym:schema/schema.graphql::Shelf.books", "sym:schema/schema.graphql::directive/auth", "")]);
    let ops = repo.extract("app/shelf.gql");
    assert_eq!(edges(&ops, EdgeKind::DecoratedBy), vec![("sym:app/shelf.gql::mutation/AddBook", "sym:schema/schema.graphql::directive/auth", "")]);
}

#[test]
fn an_extension_references_the_definition_it_extends() {
    let ext = Repo::new().extract("schema/ext.graphql");
    let refs = edges(&ext, EdgeKind::References);
    assert!(refs.contains(&("sym:schema/ext.graphql::Shelf", "sym:schema/schema.graphql::Shelf", "extend")), "{refs:?}");
    assert!(refs.contains(&("sym:schema/ext.graphql::Shelf.keeper", "sym:schema/ext.graphql::Keeper", "type")), "{refs:?}");
}

#[test]
fn ids_in_comments_and_descriptions_are_cited() {
    let repo = Repo::new();
    let fragments = repo.extract("app/fragments.gql");
    assert!(edges(&fragments, EdgeKind::References).contains(&("file:app/fragments.gql", "FR-WEB-01", "comment")));
    let schema = repo.extract("schema/schema.graphql");
    assert!(edges(&schema, EdgeKind::References).contains(&("sym:schema/schema.graphql::Shelf", "FR-WEB-02", "string")));
}

#[test]
fn an_extension_of_a_type_no_document_defines_declares_it_and_extends_nothing() {
    let repo = Repo::with(&[
        ("schema/a.graphql", "extend type Query {\n  shelf: Shelf\n}\n\ntype Shelf {\n  id: ID!\n}\n"),
        ("schema/b.graphql", "extend input ShelfFilter {\n  genre: String\n}\n"),
    ]);
    let a = repo.extract("schema/a.graphql");
    for id in ["sym:schema/a.graphql::Query", "sym:schema/a.graphql::Query.shelf"] {
        assert!(ids(&a).contains(&id), "{id} missing from {:?}", ids(&a));
    }
    let refs = edges(&a, EdgeKind::References);
    assert!(!refs.iter().any(|(_, _, context)| *context == "extend"), "{refs:?}");
    assert!(refs.contains(&("sym:schema/a.graphql::Query.shelf", "sym:schema/a.graphql::Shelf", "type")), "{refs:?}");
    let b = repo.extract("schema/b.graphql");
    assert!(ids(&b).contains(&"sym:schema/b.graphql::ShelfFilter.genre"), "{:?}", ids(&b));
    assert_eq!(repo.resolver().index(Family::GraphQl).unwrap().files("Query"), vec!["schema/a.graphql"]);
}

#[test]
fn a_fragment_name_declared_in_two_documents_is_called_in_both() {
    let repo = Repo::with(&[
        ("web/fragments.gql", "fragment BookFields on Book {\n  id\n}\n"),
        ("admin/fragments.gql", "fragment BookFields on Book {\n  id\n  title\n}\n"),
        ("web/shelf.gql", "query GetShelf {\n  books { ...BookFields }\n}\n"),
    ]);
    let shelf = repo.extract("web/shelf.gql");
    let mut calls = edges(&shelf, EdgeKind::Calls);
    calls.sort_unstable();
    assert_eq!(calls, vec![
        ("sym:web/shelf.gql::query/GetShelf", "sym:admin/fragments.gql::fragment/BookFields", ""),
        ("sym:web/shelf.gql::query/GetShelf", "sym:web/fragments.gql::fragment/BookFields", ""),
    ]);
}

#[test]
fn a_definition_quoted_in_a_comment_or_a_description_declares_and_uses_nothing() {
    let quiet = "# query Stale { ...BookFields } then extend type Shelf { spare: Book }\n\
\"\"\"fragment Ghost on Book, type Phantom implements Node @auth, union Loose = Book | Shelf\"\"\"\n\
type Lamp {\n  \"directive @glow on FIELD, and every Lamp returns Book\"\n  id: ID!\n}\n";
    let repo = Repo::with(&[("schema/quiet.graphql", quiet), ("app/fragments.gql", FRAGMENTS), ("schema/schema.graphql", SCHEMA)]);
    let ex = repo.extract("schema/quiet.graphql");
    let mut symbols: Vec<&str> = ids(&ex).into_iter().filter(|id| id.starts_with("sym:")).collect();
    symbols.sort_unstable();
    assert_eq!(symbols, vec!["sym:schema/quiet.graphql::Lamp", "sym:schema/quiet.graphql::Lamp.id"]);
    assert_eq!(top(quiet), vec!["Lamp"]);
    for kind in [EdgeKind::Calls, EdgeKind::References, EdgeKind::Extends, EdgeKind::DecoratedBy] {
        assert!(edges(&ex, kind).is_empty(), "{kind:?}: {:?}", edges(&ex, kind));
    }
}

#[test]
fn a_directive_on_a_spread_or_a_variable_decorates_the_operation_holding_it() {
    let repo = Repo::with(&[(
        "app/q.gql",
        "directive @live on QUERY\ndirective @cached on FRAGMENT_SPREAD\ndirective @old on VARIABLE_DEFINITION\nfragment F on Book { id }\n\
query Q($a: Int @old) {\n  books { ...F @cached }\n}\n",
    )]);
    let ex = repo.extract("app/q.gql");
    let decorated = edges(&ex, EdgeKind::DecoratedBy);
    for directive in ["cached", "old"] {
        let target = format!("sym:app/q.gql::directive/{directive}");
        assert!(decorated.contains(&("sym:app/q.gql::query/Q", target.as_str(), "")), "{directive}: {decorated:?}");
    }
}

#[test]
fn a_comment_ending_the_line_of_the_definition_before_it_is_not_the_next_ones_description() {
    let repo = Repo::with(&[("schema/s.graphql", "type Old { id: ID } # retired soon\ntype Next { id: ID }\n")]);
    let ex = repo.extract("schema/s.graphql");
    assert_eq!(node(&ex, "sym:schema/s.graphql::Next").body, "type Next { id: ID }");
}
