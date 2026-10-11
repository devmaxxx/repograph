//! Everything the pipeline asks of a language, as one record per language: its queries, the data the
//! engine reads them by, and the hooks for what the data cannot say. `spec` is the only place that
//! names every language: the front, the resolver, the header index, `changes` and the update's
//! widening read this table instead of matching on `Lang` each in its own way.
//!
//! Most of the vocabulary below describes ported languages, and only Shell, Bicep, HCL, Terraform and Swift are ported yet.
#![allow(dead_code)]

use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::collections::HashMap;

use crate::code::engine::{Def, Facts, File, Import, Sink};
use crate::code::imports::Resolver;
use crate::code::index::Header;
use crate::code::lang::Lang;
use crate::model::{EdgeKind, Extraction};

pub(crate) struct Spec {
    pub queries: Sources,
    /// Whose grammar and queries read an embedded view. Vue's script picks TypeScript or TSX per file,
    /// so its `embed` names the grammar and this is the default.
    pub view: Option<Lang>,
    /// What `changes` tells a comment-only hunk by; `None` where markup decides what a line is.
    pub comments: Option<CommentSyntax>,
    pub body: Body,
    pub export: Export,
    pub visibility: Visibility,
    pub ambiguity: Ambiguity,
    pub imports: ImportPaths,
    /// An edit of one file can change what another resolves to without moving a path or a header,
    /// so `serve` rebuilds the resolver when such a file changes.
    pub cross_file: bool,
    /// Whether files under a dotted directory are read. CI keeps its scripts under `.github/`,
    /// while a tsconfig or a manifest there stays out.
    pub dotted_dirs: bool,
    pub hooks: Hooks,
    /// The language's own reader, which the front calls after it has parsed the file, until the
    /// language is ported to queries.
    pub legacy: Option<Legacy>,
}

/// `.scm` sources, concatenated in order into the two compiled queries (`queries.rs`).
#[derive(Clone, Copy)]
pub(crate) struct Sources {
    /// Definitions, scopes, docs and imports: the collect pass and extract both run it.
    pub decl: &'static [&'static str],
    /// References, locals and citations: extract alone runs it.
    pub refs: &'static [&'static str],
}

#[derive(Clone, Copy)]
pub(crate) struct CommentSyntax {
    pub lines: &'static [&'static str],
    pub blocks: bool,
}

/// A body is doc lines and then one signature line; only the last line reaches the indexes.
#[derive(Clone, Copy)]
pub(crate) struct Body {
    pub doc: Doc,
    pub form: Form,
    pub cap: Cap,
    pub line: Line,
}

#[derive(Clone, Copy)]
pub(crate) enum Doc {
    /// Comment nodes of `kinds` directly above the definition; `between` are kinds that may stand
    /// between the two, as Bicep's decorators do. A shebang and a trailing comment are never doc.
    Adjacent { kinds: &'static [&'static str], between: &'static [&'static str] },
    /// The `@doc` capture: a Python docstring, a GraphQL description.
    Capture,
    None,
}

#[derive(Clone, Copy)]
pub(crate) enum Form {
    /// Markers kept, as Shell, Bicep and HCL write them.
    Raw,
    /// `//`, `///`, `/** */` and `#` removed.
    Stripped,
}

#[derive(Clone, Copy)]
pub(crate) enum Cap {
    /// The last `DOC_CHARS` bytes, the lines nearest the declaration.
    Tail,
    /// The first `DOC_CHARS` characters.
    Head,
}

#[derive(Clone, Copy)]
pub(crate) enum Line {
    /// The definition node's first line.
    First,
    /// The line holding `@name`, past any annotation lines.
    Name,
}

#[derive(Clone, Copy)]
pub(crate) enum Export {
    /// Exported only with one of these `@modifier` words.
    Show(&'static [&'static str]),
    /// Exported unless one of these `@modifier` words is present.
    Hide(&'static [&'static str]),
    /// A name starting with this character is hidden.
    Prefix(char),
    All,
}

#[derive(Clone, Copy)]
pub(crate) enum Visibility {
    /// The reference's own file.
    File,
    /// The own file and the files its resolved imports reach, transitively when asked.
    Imported { transitive: bool },
    /// The own file and the sibling files of the same language.
    Directory,
    /// The own file and the family's index by name, hidden names excluded.
    Family,
}

#[derive(Clone, Copy)]
pub(crate) enum Ambiguity {
    /// A match in the own file shadows the others; otherwise every candidate.
    OwnFirst,
    /// Exactly one candidate across the own file and the visible ones, otherwise nothing.
    Unique,
    All,
}

#[derive(Clone, Copy)]
pub(crate) struct ImportPaths {
    /// Tried after the importer's directory: the path as written relative to the repository root.
    pub root: bool,
    /// Appended to the path as written, in order; `""` tries it as written.
    pub suffixes: &'static [&'static str],
}

/// A language's own reader, over the tree the front parsed. It appends to an extraction that
/// already holds the file node.
pub(crate) type Legacy = fn(&Resolver, &File, &mut Extraction);

pub(crate) type Collect = fn(&mut Resolver, &str, &str) -> Option<Header>;

/// `(resolver, importer, site)`: the importer by path alone, because the transitive walk asks it of
/// files it has only the declarations of.
pub(crate) type ImportHook = fn(&Resolver, &str, &Import) -> Vec<String>;

/// What a blanking reader keeps of a file for its view's grammar.
pub(crate) enum Embedded {
    /// The markup could not be read; the file keeps its file node alone.
    Unread,
    /// No embedded code: the reader still runs, with no tree.
    Empty,
    /// The view, at the file's own rows and columns, and the grammar that reads it.
    View(String, Lang),
}

pub(crate) struct Hooks {
    /// The collect pass, for a file it does not read: Vue's bookkeeping needs the path alone.
    pub register: Option<fn(&mut Resolver, &str)>,
    /// What one globbed file adds to the resolver before any file is extracted, and the header it
    /// read, if any, which goes into the family's index. `None` reads `header` alone, and a language
    /// with neither never has its files opened.
    pub collect: Option<Collect>,
    /// What a name-indexed family records of a file, from `(rel, source)`. A file whose header moves
    /// re-reads every other file of its family.
    pub header: Option<fn(&str, &str) -> Header>,
    pub manifest: Option<Manifest>,
    /// The resolver's state for this language, created empty for every language that has one.
    pub state: Option<fn() -> Box<dyn Any + Send + Sync>>,
    pub embed: Option<fn(&str) -> Embedded>,
    /// The file node's body; every other file node's is empty.
    pub head: Option<fn(&File) -> String>,
    /// Definition and reference names as the language compares them.
    pub fold: Option<fn(&str) -> Cow<str>>,
    /// A body's last line that no `Line` gives.
    pub signature: Option<fn(&File, &Def) -> String>,
    /// The files an import site names, for the paths the default cannot compute.
    pub import: Option<ImportHook>,
    pub resolve: Option<fn(&Resolver, &File, &Facts, &mut Sink)>,
    /// Edge kinds the default resolution leaves to `resolve`.
    pub owns: &'static [EdgeKind],
}

impl Hooks {
    /// Whether the resolver's walk opens this language's files before extraction.
    pub fn collects(&self) -> bool {
        self.header.is_some() || self.collect.is_some()
    }
}

/// A build manifest the language reads, matched by file name; read only when the globs reach the
/// language's family.
pub(crate) struct Manifest {
    pub matches: fn(&str) -> bool,
    pub read: fn(&mut Resolver, &str, &str),
}

/// The hooks a language leaves at their defaults, for `..NONE`.
pub(crate) const NONE: Hooks = Hooks {
    register: None,
    collect: None,
    header: None,
    manifest: None,
    state: None,
    embed: None,
    head: None,
    fold: None,
    signature: None,
    import: None,
    resolve: None,
    owns: &[],
};

const C: Option<CommentSyntax> = Some(CommentSyntax { lines: &["//"], blocks: true });
const HASH: Option<CommentSyntax> = Some(CommentSyntax { lines: &["#"], blocks: false });
const HCL_COMMENTS: Option<CommentSyntax> = Some(CommentSyntax { lines: &["#", "//"], blocks: true });

/// The fields a language's port has yet to write down; nothing reads them while `legacy` is set.
const UNPORTED: Spec = Spec {
    queries: Sources { decl: &[], refs: &[] },
    view: None,
    comments: None,
    body: Body { doc: Doc::None, form: Form::Raw, cap: Cap::Tail, line: Line::First },
    export: Export::All,
    visibility: Visibility::File,
    ambiguity: Ambiguity::Unique,
    imports: ImportPaths { root: false, suffixes: &[""] },
    cross_file: false,
    dotted_dirs: false,
    hooks: NONE,
    legacy: None,
};

/// The comment lines directly above a declaration, markers kept, and its first line.
const PROSE: Body = Body { doc: Doc::Adjacent { kinds: &["comment"], between: &[] }, form: Form::Raw, cap: Cap::Tail, line: Line::First };

mod table {
    use super::*;
    use crate::code::exceptions::{
        attribute_lines, computed_paths, inheritance_clauses, module_reach, target_or_group, traversal_addresses, type_paths,
    };
    use crate::code::*;

    pub(super) const TYPESCRIPT: Spec = Spec { comments: C, hooks: typescript::HOOKS, legacy: Some(typescript::extract), ..UNPORTED };
    pub(super) const VUE: Spec = Spec { view: Some(Lang::TypeScript), hooks: typescript::vue::HOOKS, legacy: Some(typescript::vue::extract), ..UNPORTED };
    pub(super) const CSHARP: Spec = Spec { comments: C, hooks: dotnet::csharp::HOOKS, legacy: Some(dotnet::csharp::extract), ..UNPORTED };
    pub(super) const RAZOR: Spec = Spec { view: Some(Lang::CSharp), hooks: dotnet::razor::HOOKS, legacy: Some(dotnet::razor::extract), ..UNPORTED };
    pub(super) const KOTLIN: Spec = Spec { comments: C, hooks: jvm::kotlin::HOOKS, legacy: Some(jvm::kotlin::extract), ..UNPORTED };
    pub(super) const JAVA: Spec = Spec { comments: C, hooks: jvm::java::HOOKS, legacy: Some(jvm::java::extract), ..UNPORTED };
    pub(super) const RUST: Spec = Spec { comments: C, hooks: rust_lang::HOOKS, legacy: Some(rust_lang::extract), ..UNPORTED };
    pub(super) const PYTHON: Spec = Spec { comments: HASH, hooks: python::HOOKS, legacy: Some(python::extract), ..UNPORTED };
    pub(super) const DART: Spec = Spec { comments: C, hooks: dart::HOOKS, legacy: Some(dart::extract), ..UNPORTED };
    pub(super) const GRAPHQL: Spec = Spec { comments: HASH, hooks: graphql::HOOKS, legacy: Some(graphql::extract), ..UNPORTED };
    pub(super) const SQL: Spec = Spec {
        comments: Some(CommentSyntax { lines: &["--"], blocks: true }),
        hooks: sql::HOOKS,
        legacy: Some(sql::extract),
        ..UNPORTED
    };
    pub(super) const BICEP: Spec = Spec {
        queries: crate::code::queries::sources::BICEP,
        view: None,
        comments: C,
        body: Body { doc: Doc::Adjacent { kinds: &["comment"], between: &["decorators"] }, ..PROSE },
        // The query's `#set! export` decides: a parameter, an output, or what `@export()` decorates.
        export: Export::Show(&[]),
        visibility: Visibility::File,
        ambiguity: Ambiguity::OwnFirst,
        // A module reaches what its file declares, which an edit there moves.
        cross_file: true,
        imports: ImportPaths { root: false, suffixes: &[""] },
        dotted_dirs: false,
        hooks: Hooks { resolve: Some(module_reach::reach), ..NONE },
        legacy: None,
    };
    pub(super) const HCL: Spec = Spec {
        queries: crate::code::queries::sources::HCL,
        view: None,
        comments: HCL_COMMENTS,
        body: PROSE,
        // A bake file's every block is invoked by name from outside it.
        export: Export::All,
        visibility: Visibility::File,
        ambiguity: Ambiguity::OwnFirst,
        cross_file: false,
        imports: ImportPaths { root: false, suffixes: &[""] },
        dotted_dirs: false,
        hooks: Hooks { resolve: Some(target_or_group::resolve), owns: &[EdgeKind::References], ..NONE },
        legacy: None,
    };
    pub(super) const TERRAFORM: Spec = Spec {
        queries: crate::code::queries::sources::TERRAFORM,
        view: None,
        comments: HCL_COMMENTS,
        body: PROSE,
        // The query's `#set! export` decides: a variable and an output are the module's interface.
        export: Export::Show(&[]),
        // A module is a directory: what one file declares, every file beside it reads.
        visibility: Visibility::Directory,
        ambiguity: Ambiguity::OwnFirst,
        cross_file: true,
        imports: ImportPaths { root: false, suffixes: &[""] },
        dotted_dirs: false,
        hooks: Hooks {
            resolve: Some(|r, file, facts, sink| {
                traversal_addresses::resolve(r, file, facts, sink);
                module_reach::reach(r, file, facts, sink);
            }),
            owns: &[EdgeKind::References],
            ..NONE
        },
        legacy: None,
    };
    pub(super) const SWIFT: Spec = Spec {
        queries: crate::code::queries::sources::SWIFT,
        view: None,
        comments: C,
        // No doc, and the `signature` hook picks the line.
        body: Body { doc: Doc::None, form: Form::Raw, cap: Cap::Tail, line: Line::Name },
        export: Export::Hide(&["private", "fileprivate"]),
        // A module sees every file in it without an import.
        visibility: Visibility::Family,
        ambiguity: Ambiguity::OwnFirst,
        cross_file: false,
        imports: ImportPaths { root: false, suffixes: &[""] },
        dotted_dirs: false,
        hooks: Hooks {
            collect: Some(inheritance_clauses::collect),
            fold: Some(type_paths::fold),
            signature: Some(attribute_lines::signature),
            resolve: Some(inheritance_clauses::resolve),
            owns: &[EdgeKind::Extends],
            ..NONE
        },
        legacy: None,
    };
    pub(super) const SHELL: Spec = Spec {
        queries: crate::code::queries::sources::SHELL,
        view: None,
        comments: HASH,
        body: PROSE,
        // Any script that sources the file can call its functions.
        export: Export::All,
        visibility: Visibility::Imported { transitive: true },
        // Which of two definitions a call reaches is whichever `source` ran last, which the graph
        // does not model, so a name defined twice resolves to neither.
        ambiguity: Ambiguity::Unique,
        cross_file: true,
        // Unread: the `import` hook resolves every `source`, and a path it cannot place imports nothing.
        imports: ImportPaths { root: false, suffixes: &[] },
        dotted_dirs: true,
        hooks: Hooks {
            collect: Some(computed_paths::collect),
            state: Some(state::<computed_paths::Scripts>),
            import: Some(computed_paths::import),
            resolve: Some(computed_paths::runs),
            ..NONE
        },
        legacy: None,
    };
}

pub(crate) fn spec(lang: Lang) -> &'static Spec {
    match lang {
        Lang::TypeScript | Lang::Tsx => &table::TYPESCRIPT,
        Lang::Vue => &table::VUE,
        Lang::CSharp => &table::CSHARP,
        Lang::Razor => &table::RAZOR,
        Lang::Kotlin => &table::KOTLIN,
        Lang::Java => &table::JAVA,
        Lang::Rust => &table::RUST,
        Lang::Python => &table::PYTHON,
        Lang::Dart => &table::DART,
        Lang::Swift => &table::SWIFT,
        Lang::GraphQl => &table::GRAPHQL,
        Lang::Sql => &table::SQL,
        Lang::Bicep => &table::BICEP,
        Lang::Hcl => &table::HCL,
        Lang::Terraform => &table::TERRAFORM,
        Lang::Shell => &table::SHELL,
    }
}

/// The resolver's per-language state, one value per type.
#[derive(Default)]
pub(crate) struct States(HashMap<TypeId, Box<dyn Any + Send + Sync>>);

impl States {
    /// Every language's state, empty.
    pub fn new() -> States {
        let mut s = States::default();
        for lang in Lang::ALL {
            if let Some(make) = spec(lang).hooks.state {
                let v = make();
                let first = s.0.insert((*v).type_id(), v).is_none();
                assert!(first, "two languages register one state type; a state is keyed by its type, so give each its own");
            }
        }
        s
    }

    pub fn get<T: Any>(&self) -> &T {
        self.0.get(&TypeId::of::<T>()).and_then(|v| v.downcast_ref()).expect("a state no language registers")
    }

    pub fn get_mut<T: Any>(&mut self) -> &mut T {
        self.0.get_mut(&TypeId::of::<T>()).and_then(|v| v.downcast_mut()).expect("a state no language registers")
    }
}

/// `state` for a language whose state is `T`.
pub(crate) fn state<T: Any + Default + Send + Sync>() -> Box<dyn Any + Send + Sync> {
    Box::<T>::default()
}
