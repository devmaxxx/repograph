//! The two compiled queries of each language, and the capture contract they are held to. A capture
//! name maps to a `Role` and a pattern's `#set!` properties to a `Meta` once, at compile, so the
//! work per match is integer lookups.
//!
//! Only tree-sitter's built-in predicates (`#eq?`, `#match?`, `#any-of?` and their `#not-` forms)
//! and `#set!` are admitted. Their semantics are tree-sitter's and documented there; a predicate
//! dialect of our own would be a second language to learn and to get wrong.

use std::sync::OnceLock;

use tree_sitter::{CaptureQuantifier, Language, Query};

use crate::code::lang::Lang;
use crate::code::spec::spec;
use crate::model::EdgeKind;

/// The closed list of definition kinds. The kind decides how a container joins what it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Class, Struct, Interface, Enum, Trait, Protocol, Object, Record, Type, Resource,
    Module,
    Function, Method, Field, Property, Constant, Variable, Value, Output, Block,
}

impl Kind {
    fn of(name: &str) -> Option<Kind> {
        Some(match name {
            "class" => Kind::Class,
            "struct" => Kind::Struct,
            "interface" => Kind::Interface,
            "enum" => Kind::Enum,
            "trait" => Kind::Trait,
            "protocol" => Kind::Protocol,
            "object" => Kind::Object,
            "record" => Kind::Record,
            "type" => Kind::Type,
            "resource" => Kind::Resource,
            "module" => Kind::Module,
            "function" => Kind::Function,
            "method" => Kind::Method,
            "field" => Kind::Field,
            "property" => Kind::Property,
            "constant" => Kind::Constant,
            "variable" => Kind::Variable,
            "value" => Kind::Value,
            "output" => Kind::Output,
            "block" => Kind::Block,
            _ => return None,
        })
    }

    /// What a container puts between its name and a nested one; `None` for a leaf, which qualifies
    /// nothing. `.` joins members only, and modules join with `/` (L4).
    pub(crate) fn join(self) -> Option<char> {
        match self {
            Kind::Module => Some('/'),
            Kind::Class | Kind::Struct | Kind::Interface | Kind::Enum | Kind::Trait | Kind::Protocol | Kind::Object
            | Kind::Record | Kind::Type | Kind::Resource => Some('.'),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    Definition(Kind),
    Name,
    Owner,
    ScopeOwner,
    ScopeNamespace,
    Modifier,
    Doc,
    Reference(EdgeKind),
    Receiver,
    Import,
    ImportPath,
    ImportName,
    ImportAlias,
    LocalScope,
    LocalDefinition,
    CiteComment,
    CiteString,
    /// `@_…`: read by a predicate, never by the engine.
    Private,
}

impl Role {
    fn of(capture: &str) -> Option<Role> {
        if capture.starts_with('_') {
            return Some(Role::Private);
        }
        if let Some(kind) = capture.strip_prefix("definition.") {
            return Kind::of(kind).map(Role::Definition);
        }
        if let Some(edge) = capture.strip_prefix("reference.") {
            return Some(Role::Reference(match edge {
                "calls" => EdgeKind::Calls,
                "references" => EdgeKind::References,
                "extends" => EdgeKind::Extends,
                "implements" => EdgeKind::Implements,
                "decorated_by" => EdgeKind::DecoratedBy,
                _ => return None,
            }));
        }
        Some(match capture {
            "name" => Role::Name,
            "owner" => Role::Owner,
            "scope.owner" => Role::ScopeOwner,
            "scope.namespace" => Role::ScopeNamespace,
            "modifier" => Role::Modifier,
            "doc" => Role::Doc,
            "receiver" => Role::Receiver,
            "import" => Role::Import,
            "import.path" => Role::ImportPath,
            "import.name" => Role::ImportName,
            "import.alias" => Role::ImportAlias,
            "local.scope" => Role::LocalScope,
            "local.definition" => Role::LocalDefinition,
            "cite.comment" => Role::CiteComment,
            "cite.string" => Role::CiteString,
            _ => return None,
        })
    }
}

/// Edge contexts in use today; a new one is a change to the graph model, not to a query.
const CONTEXTS: &[&str] =
    &["", "export", "arg", "get", "*", "comment", "string", "alter", "impl", "extend", "on", "type", "variable", "argument", "member"];

/// A pattern's `#set!` properties.
#[derive(Debug, Default, Clone)]
pub(crate) struct Meta {
    pub prefix: String,
    pub context: String,
    /// `Some` overrides the spec's export rule for this pattern.
    pub export: Option<bool>,
    pub reexport: bool,
}

pub(crate) struct Compiled {
    pub query: Query,
    /// By capture index.
    pub roles: Vec<Role>,
    /// By pattern index.
    pub metas: Vec<Meta>,
}

impl std::fmt::Debug for Compiled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Compiled").field("patterns", &self.metas.len()).finish()
    }
}

/// `sources` concatenated in order and compiled against `grammar`, held to the capture contract.
pub(crate) fn compile(grammar: &Language, sources: &[&str]) -> Result<Compiled, String> {
    let text = sources.join("\n");
    let query = Query::new(grammar, &text).map_err(|e| format!("query does not compile: {e}"))?;
    let roles = query.capture_names().iter()
        .map(|name| Role::of(name).ok_or_else(|| format!("@{name} is not in the capture contract")))
        .collect::<Result<Vec<_>, _>>()?;
    let mut metas = Vec::with_capacity(query.pattern_count());
    for i in 0..query.pattern_count() {
        let at = query.start_byte_for_pattern(i);
        if !query.general_predicates(i).is_empty() || !query.property_predicates(i).is_empty() {
            return Err(format!("pattern at byte {at} uses a predicate the engine does not evaluate"));
        }
        let mut meta = Meta::default();
        for p in query.property_settings(i) {
            let value = p.value.as_deref().unwrap_or("");
            match &*p.key {
                "prefix" => meta.prefix = value.to_string(),
                "context" if CONTEXTS.contains(&value) => meta.context = value.to_string(),
                "export" if value == "always" => meta.export = Some(true),
                "export" if value == "never" => meta.export = Some(false),
                "reexport" => meta.reexport = true,
                key => return Err(format!("pattern at byte {at} sets {key} {value:?}, outside the closed set")),
            }
        }
        let present = |want: fn(Role) -> bool| query.capture_quantifiers(i).iter().zip(&roles)
            .any(|(q, r)| *q != CaptureQuantifier::Zero && want(*r));
        let declares = present(|r| matches!(r, Role::Definition(_) | Role::ScopeOwner | Role::ScopeNamespace));
        if declares && !present(|r| r == Role::Name) {
            return Err(format!("pattern at byte {at} declares without capturing @name"));
        }
        metas.push(meta);
    }
    Ok(Compiled { query, roles, metas })
}

/// Each ported language's `.scm` sources, embedded so the binary never reads a query from disk.
pub(crate) mod sources {
    use crate::code::spec::Sources;

    pub(crate) const BICEP: Sources =
        Sources { decl: &[include_str!("queries/bicep/decl.scm")], refs: &[include_str!("queries/bicep/refs.scm")] };

    pub(crate) const GRAPHQL: Sources =
        Sources { decl: &[include_str!("queries/graphql/decl.scm")], refs: &[include_str!("queries/graphql/refs.scm")] };

    pub(crate) const HCL: Sources =
        Sources { decl: &[include_str!("queries/hcl/decl.scm")], refs: &[include_str!("queries/hcl/refs.scm")] };

    pub(crate) const SHELL: Sources =
        Sources { decl: &[include_str!("queries/shell/decl.scm")], refs: &[include_str!("queries/shell/refs.scm")] };

    pub(crate) const SWIFT: Sources =
        Sources { decl: &[include_str!("queries/swift/decl.scm")], refs: &[include_str!("queries/swift/refs.scm")] };

    pub(crate) const TERRAFORM: Sources =
        Sources { decl: &[include_str!("queries/terraform/decl.scm")], refs: &[include_str!("queries/terraform/refs.scm")] };
}

const N: usize = Lang::ALL.len();

/// Compiled once per process, on first use; `None` for a language with no queries yet.
static DECL: [OnceLock<Option<Compiled>>; N] = [const { OnceLock::new() }; N];
static REFS: [OnceLock<Option<Compiled>>; N] = [const { OnceLock::new() }; N];

fn slot(lang: Lang) -> usize {
    Lang::ALL.iter().position(|l| *l == lang).expect("every language is in Lang::ALL")
}

fn cached(cache: &'static OnceLock<Option<Compiled>>, lang: Lang, sources: &[&str]) -> Option<&'static Compiled> {
    cache.get_or_init(|| {
        if sources.is_empty() {
            return None;
        }
        let grammar = lang.grammar().unwrap_or_else(|| panic!("{lang:?} has queries and no grammar"));
        // The contract test compiles every language's queries first, so this fails a test before a build.
        Some(compile(&grammar, sources).unwrap_or_else(|e| panic!("{lang:?}: {e}")))
    }).as_ref()
}

pub(crate) fn decl(lang: Lang) -> Option<&'static Compiled> {
    cached(&DECL[slot(lang)], lang, spec(lang).queries.decl)
}

pub(crate) fn refs(lang: Lang) -> Option<&'static Compiled> {
    cached(&REFS[slot(lang)], lang, spec(lang).queries.refs)
}

#[cfg(test)]
mod contract {
    use super::*;
    use streaming_iterator::StreamingIterator;

    /// Every language's queries compile against its grammar and keep to the contract.
    #[test]
    fn every_query_keeps_the_capture_contract() {
        for lang in Lang::ALL {
            let sources = spec(lang).queries;
            for part in [sources.decl, sources.refs].into_iter().filter(|s| !s.is_empty()) {
                let grammar = lang.grammar().unwrap_or_else(|| panic!("{lang:?} has queries and no grammar"));
                if let Err(e) = compile(&grammar, part) {
                    panic!("{lang:?}: {e}");
                }
            }
        }
    }

    /// One query per grammar, so the contract test above is not vacuous while few languages have
    /// queries: each grammar loads, compiles a query and matches its own root.
    #[test]
    fn every_grammar_compiles_and_runs_a_probe_query() {
        for lang in Lang::ALL {
            let Some(grammar) = lang.grammar() else { continue };
            let tree = lang.parse(b"").unwrap();
            let root = tree.root_node().kind();
            let c = compile(&grammar, &[&format!("({root}) @_probe")]).unwrap_or_else(|e| panic!("{lang:?}: {e}"));
            let mut cursor = tree_sitter::QueryCursor::new();
            assert_eq!(cursor.matches(&c.query, tree.root_node(), &b""[..]).count(), 1, "{lang:?}");
        }
    }

    fn rejects(source: &str) -> String {
        compile(&Lang::Python.grammar().unwrap(), &[source]).expect_err(source)
    }

    #[test]
    fn a_capture_outside_the_contract_is_refused() {
        assert!(rejects("(function_definition name: (identifier) @fn)").contains("@fn"));
        assert!(rejects("(function_definition) @definition.gadget").contains("@definition.gadget"));
        assert!(rejects("(call) @reference.invokes").contains("@reference.invokes"));
    }

    #[test]
    fn a_property_outside_the_closed_set_is_refused() {
        assert!(rejects(r#"((identifier) @reference.calls (#set! context "nearby"))"#).contains("context"));
        assert!(rejects(r#"((identifier) @reference.calls (#set! weight "2"))"#).contains("weight"));
        assert!(rejects(r#"((function_definition name: (identifier) @name) @definition.function (#set! export "maybe"))"#).contains("export"));
    }

    #[test]
    fn a_general_predicate_is_refused() {
        assert!(rejects(r#"((identifier) @reference.calls (#select-adjacent! @reference.calls))"#).contains("predicate"));
        assert!(rejects(r#"((identifier) @reference.calls (#is-not? local))"#).contains("predicate"));
    }

    #[test]
    fn a_definition_without_a_name_is_refused() {
        assert!(rejects("(function_definition) @definition.function").contains("@name"));
        assert!(rejects("(class_definition) @scope.owner").contains("@name"));
    }

    #[test]
    fn built_in_predicates_and_properties_compile_to_meta() {
        let c = compile(&Lang::Python.grammar().unwrap(), &[
            r#"((function_definition name: (identifier) @name) @definition.function (#not-match? @name "^_") (#set! prefix "fn/") (#set! export "never"))"#,
            r#"((import_from_statement module_name: (dotted_name) @import.path) @import (#set! reexport))"#,
            r#"((call function: (identifier) @name) @reference.calls (#any-of? @name "a" "b") (#set! context "arg"))"#,
        ]).unwrap();
        assert_eq!(c.metas[0].prefix, "fn/");
        assert_eq!(c.metas[0].export, Some(false));
        assert!(c.metas[1].reexport);
        assert_eq!(c.metas[2].context, "arg");
    }
}
