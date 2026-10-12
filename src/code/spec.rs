//! Everything the pipeline asks of a language, as one record per language: its queries, the data the
//! engine reads them by, and the hooks for what the data cannot say. `spec` is the only place that
//! names every language: the front, the resolver, the header index, `changes` and the update's
//! widening read this table instead of matching on `Lang` each in its own way.
//!
//! Most of the vocabulary below describes ported languages, and only Shell, Bicep, HCL, Terraform, Swift, GraphQL, SQL, Dart, Python, Rust and Kotlin are ported yet.
#![allow(dead_code)]

use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::collections::HashMap;

use crate::code::engine::{Def, Facts, File, Import, Sink};
use crate::code::imports::Resolver;
use crate::code::index::Header;
use crate::code::lang::Lang;
use crate::model::{EdgeKind, Extraction};

pub struct Spec {
    pub queries: Sources,
    /// Whose grammar and queries read an embedded view. Vue's script picks TypeScript or TSX per file,
    /// so its `embed` names the grammar and this is the default.
    pub view: Option<Lang>,
    /// What `changes` tells a comment-only hunk by; `None` where markup decides what a line is.
    pub comments: Option<CommentSyntax>,
    pub body: Body,
    pub label: Label,
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
pub struct Sources {
    /// Definitions, scopes, docs and imports: the collect pass and extract both run it.
    pub decl: &'static [&'static str],
    /// References, locals and citations: extract alone runs it.
    pub refs: &'static [&'static str],
}

#[derive(Clone, Copy)]
pub struct CommentSyntax {
    pub lines: &'static [&'static str],
    pub blocks: bool,
}

/// A body is doc lines and then one signature line; only the last line reaches the indexes.
#[derive(Clone, Copy)]
pub struct Body {
    pub doc: Doc,
    pub form: Form,
    pub cap: Cap,
    pub line: Line,
}

#[derive(Clone, Copy)]
pub enum Doc {
    /// Comment nodes of `kinds` directly above the definition; `between` are kinds that may stand
    /// between the two, as Bicep's decorators do. A shebang and a trailing comment are never doc.
    /// `line_comment.outer` is a kind with a field: Rust's `///`, where a plain `//` is no doc.
    Adjacent { kinds: &'static [&'static str], between: &'static [&'static str] },
    /// The `@doc` capture: a Python docstring, a GraphQL description.
    Capture,
    None,
}

#[derive(Clone, Copy)]
pub enum Form {
    /// Markers kept, as Shell, Bicep and HCL write them.
    Raw,
    /// `//`, `///`, `/** */`, `#` and the language's own line markers removed.
    Stripped,
}

#[derive(Clone, Copy)]
pub enum Cap {
    /// The last `DOC_CHARS` bytes, the lines nearest the declaration.
    Tail,
    /// The first `DOC_CHARS` characters.
    Head,
}

#[derive(Clone, Copy)]
pub enum Line {
    /// The definition node's first line.
    First,
    /// The line holding `@name`, past any annotation lines.
    Name,
    /// The whole line holding `@name`, past its definition's span: a member written on its
    /// container's line reads with that line, as a `val` parameter does on its `class` line.
    Whole,
    /// The whole row the definition starts on: a migration's statement reads with the marker a tool
    /// wrote after it, as the line a reader greps for.
    Row,
    /// Up to the definition's `body` field, whitespace collapsed: a signature over several lines
    /// reads as one. A definition with no body reads as its first line.
    Header,
    /// As `Header`, but a definition with no body reads whole, whitespace collapsed: a Rust `const`
    /// reads with its value and a `macro_rules!` with its rules, so doc and signature share one
    /// `DOC_CHARS` cap.
    Signature,
}

#[derive(Clone, Copy)]
pub enum Label {
    /// The qualified name, as the id ends.
    Qualified,
    /// The last `@name` part without its prefix, under its container's label: the id says
    /// `query/GetShelf` so a query and a mutation of one name stay apart, and a reader asks for
    /// `GetShelf`, and a column of `app/clients` reads `clients.id`.
    Name,
    /// The qualified name with each `/` between modules spelled as the language writes a path:
    /// Rust's `tests/helper` reads `tests::helper`.
    Spelled(&'static str),
}

#[derive(Clone, Copy)]
pub enum Export {
    /// Exported only with one of these `@modifier` words.
    Show(&'static [&'static str]),
    /// Exported unless one of these `@modifier` words is present.
    Hide(&'static [&'static str]),
    /// A name starting with this character is hidden.
    Prefix(char),
    All,
}

#[derive(Clone, Copy)]
pub enum Visibility {
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
pub enum Ambiguity {
    /// A match in the own file shadows the others; otherwise every candidate.
    OwnFirst,
    /// Exactly one candidate across the own file and the visible ones, otherwise nothing.
    Unique,
    All,
}

#[derive(Clone, Copy)]
pub struct ImportPaths {
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

pub struct Hooks {
    /// The collect pass, for a file it does not read: Vue's bookkeeping needs the path alone.
    pub(crate) register: Option<fn(&mut Resolver, &str)>,
    /// What one globbed file adds to the resolver before any file is extracted, and the header it
    /// read, if any, which goes into the family's index. `None` reads `header` alone, and a language
    /// with neither never has its files opened.
    pub(crate) collect: Option<Collect>,
    /// What a name-indexed family records of a file, from `(rel, source)`. A file whose header moves
    /// re-reads every other file of its family.
    pub(crate) header: Option<fn(&str, &str) -> Header>,
    pub(crate) manifest: Option<Manifest>,
    /// The resolver's state for this language, created empty for every language that has one.
    pub(crate) state: Option<fn() -> Box<dyn Any + Send + Sync>>,
    pub(crate) embed: Option<fn(&str) -> Embedded>,
    /// The file node's body; every other file node's is empty.
    pub(crate) head: Option<fn(&File) -> String>,
    /// Definition and reference names as the language compares them.
    pub(crate) fold: Option<fn(&str) -> Cow<str>>,
    /// A body's last line that no `Line` gives.
    pub(crate) signature: Option<fn(&File, &Def) -> String>,
    /// The files an import site names, for the paths the default cannot compute.
    pub(crate) import: Option<ImportHook>,
    pub(crate) resolve: Option<fn(&Resolver, &File, &Facts, &mut Sink)>,
    /// Edge kinds the default resolution leaves to `resolve`.
    pub(crate) owns: &'static [EdgeKind],
}

impl Spec {
    /// Whether the resolver's walk opens this language's files before extraction: a hook reads them,
    /// or a family that sees its files without an import is indexed by its queries.
    pub fn collects(&self) -> bool {
        self.hooks.header.is_some() || self.hooks.collect.is_some() || matches!(self.visibility, Visibility::Family)
    }
}

/// A build manifest the language reads, matched by file name; read only when the globs reach the
/// language's family.
pub(crate) struct Manifest {
    pub matches: fn(&str) -> bool,
    pub read: fn(&mut Resolver, &str, &str),
}

/// The hooks a language leaves at their defaults, for `..NONE`.
pub const NONE: Hooks = Hooks {
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
    label: Label::Qualified,
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
    use crate::code::exceptions::{attachments, computed_paths, crate_paths, identifier_case, library_scope, module_paths, module_reach, package_scope};
    use crate::code::*;

    pub(super) const TYPESCRIPT: Spec = Spec { comments: C, hooks: typescript::HOOKS, legacy: Some(typescript::extract), ..UNPORTED };
    pub(super) const VUE: Spec = Spec { view: Some(Lang::TypeScript), hooks: typescript::vue::HOOKS, legacy: Some(typescript::vue::extract), ..UNPORTED };
    pub(super) const CSHARP: Spec = Spec { comments: C, hooks: dotnet::csharp::HOOKS, legacy: Some(dotnet::csharp::extract), ..UNPORTED };
    pub(super) const RAZOR: Spec = Spec { view: Some(Lang::CSharp), hooks: dotnet::razor::HOOKS, legacy: Some(dotnet::razor::extract), ..UNPORTED };
    pub(super) const KOTLIN: Spec = Spec {
        queries: crate::code::queries::sources::KOTLIN,
        view: None,
        comments: C,
        body: Body {
            doc: Doc::Adjacent { kinds: &["line_comment", "multiline_comment"], between: &[] },
            form: Form::Stripped,
            cap: Cap::Head,
            line: Line::Whole,
        },
        label: Label::Qualified,
        // Kotlin is public by default, and `internal` crosses files inside a module.
        export: Export::Hide(&["private", "protected"]),
        // A package sees every file in it without an import.
        visibility: Visibility::Family,
        ambiguity: Ambiguity::OwnFirst,
        cross_file: false,
        imports: ImportPaths { root: false, suffixes: &[] },
        dotted_dirs: false,
        // The JVM index needs each file's arities and supertypes, and a call binds through
        // receivers, supertypes and overloads no query reads.
        hooks: Hooks { header: Some(package_scope::declared::header), resolve: Some(package_scope::receivers::resolve), ..NONE },
        legacy: None,
    };
    pub(super) const JAVA: Spec = Spec { comments: C, hooks: jvm::java::HOOKS, legacy: Some(jvm::java::extract), ..UNPORTED };
    pub(super) const RUST: Spec = Spec {
        queries: crate::code::queries::sources::RUST,
        view: None,
        comments: C,
        // The `///` lines, read through attributes, and the signature, or a whole `const`.
        body: Body {
            doc: Doc::Adjacent { kinds: &["line_comment.outer"], between: &["attribute_item"] },
            form: Form::Stripped,
            cap: Cap::Head,
            line: Line::Signature,
        },
        label: Label::Spelled("::"),
        export: Export::Show(&["pub"]),
        visibility: Visibility::File,
        ambiguity: Ambiguity::OwnFirst,
        cross_file: false,
        // Unread: `crate_paths` places every `use` and writes its edge with the name it binds.
        imports: ImportPaths { root: false, suffixes: &[] },
        dotted_dirs: false,
        hooks: Hooks {
            collect: Some(crate_paths::collect),
            state: Some(state::<crate_paths::Crates>),
            manifest: Some(Manifest { matches: |name| name == "Cargo.toml", read: crate_paths::manifest }),
            resolve: Some(crate_paths::resolve),
            ..NONE
        },
        legacy: None,
    };
    pub(super) const PYTHON: Spec = Spec {
        queries: crate::code::queries::sources::PYTHON,
        view: None,
        comments: HASH,
        // The docstring and the signature up to its colon.
        body: Body { doc: Doc::Capture, form: Form::Stripped, cap: Cap::Head, line: Line::Header },
        label: Label::Qualified,
        export: Export::Prefix('_'),
        visibility: Visibility::File,
        ambiguity: Ambiguity::OwnFirst,
        cross_file: false,
        // Unread: `module_paths` places every import and writes its edge with the names it binds.
        imports: ImportPaths { root: false, suffixes: &[] },
        dotted_dirs: false,
        hooks: Hooks {
            state: Some(state::<module_paths::Roots>),
            manifest: Some(Manifest {
                matches: |name| matches!(name, "pyproject.toml" | "setup.py" | "setup.cfg"),
                read: module_paths::manifest,
            }),
            resolve: Some(module_paths::resolve),
            owns: &[EdgeKind::Calls, EdgeKind::Extends, EdgeKind::DecoratedBy],
            ..NONE
        },
        legacy: None,
    };
    pub(super) const DART: Spec = Spec {
        queries: crate::code::queries::sources::DART,
        view: None,
        comments: C,
        // No doc. A declaration starts at its annotations, as the old reader's line did.
        body: Body { doc: Doc::None, form: Form::Raw, cap: Cap::Tail, line: Line::First },
        label: Label::Qualified,
        export: Export::Prefix('_'),
        visibility: Visibility::File,
        ambiguity: Ambiguity::OwnFirst,
        // What a file's names resolve to moves with the directives and declarations of every library
        // it imports.
        cross_file: true,
        imports: ImportPaths { root: false, suffixes: &[""] },
        dotted_dirs: false,
        hooks: Hooks {
            collect: Some(library_scope::collect),
            state: Some(state::<library_scope::Libraries>),
            manifest: Some(Manifest { matches: |name| name == "pubspec.yaml", read: library_scope::pubspec }),
            resolve: Some(library_scope::resolve),
            owns: &[EdgeKind::Calls, EdgeKind::Extends],
            ..NONE
        },
        legacy: None,
    };
    pub(super) const SQL: Spec = Spec {
        queries: crate::code::queries::sources::SQL,
        view: None,
        comments: Some(CommentSyntax { lines: &["--"], blocks: true }),
        body: Body { form: Form::Stripped, cap: Cap::Head, line: Line::Row, ..PROSE },
        label: Label::Name,
        export: Export::All,
        // `search_path` is session state no file records, so a name is looked up as written.
        visibility: Visibility::Family,
        // Every migration that creates or alters a table is one of its declarations.
        ambiguity: Ambiguity::All,
        cross_file: false,
        imports: ImportPaths { root: false, suffixes: &[""] },
        dotted_dirs: false,
        // The objects alone, in `headers.json`, so a migration that starts or stops creating or
        // altering a table re-reads the ones that name it.
        hooks: Hooks {
            header: Some(|rel, source| Header { top: crate::code::engine::default_header(Lang::Sql, rel, source).top, ..Header::default() }),
            fold: Some(identifier_case::fold),
            resolve: Some(attachments::alters),
            ..NONE
        },
        legacy: None,
    };
    pub(super) const BICEP: Spec = Spec {
        queries: crate::code::queries::sources::BICEP,
        view: None,
        comments: C,
        body: Body { doc: Doc::Adjacent { kinds: &["comment"], between: &["decorators"] }, ..PROSE },
        label: Label::Qualified,
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
        label: Label::Qualified,
        // A bake file's every block is invoked by name from outside it.
        export: Export::All,
        visibility: Visibility::File,
        ambiguity: Ambiguity::OwnFirst,
        cross_file: false,
        imports: ImportPaths { root: false, suffixes: &[""] },
        dotted_dirs: false,
        hooks: NONE,
        legacy: None,
    };
    pub(super) const TERRAFORM: Spec = Spec {
        queries: crate::code::queries::sources::TERRAFORM,
        view: None,
        comments: HCL_COMMENTS,
        body: PROSE,
        label: Label::Qualified,
        // The query's `#set! export` decides: a variable and an output are the module's interface.
        export: Export::Show(&[]),
        // A module is a directory: what one file declares, every file beside it reads.
        visibility: Visibility::Directory,
        ambiguity: Ambiguity::OwnFirst,
        cross_file: true,
        imports: ImportPaths { root: false, suffixes: &[""] },
        dotted_dirs: false,
        hooks: Hooks { resolve: Some(module_reach::reach), ..NONE },
        legacy: None,
    };
    pub(super) const SWIFT: Spec = Spec {
        queries: crate::code::queries::sources::SWIFT,
        view: None,
        comments: C,
        // No doc. A declaration starts at its attributes, so the line is the one holding its name.
        body: Body { doc: Doc::None, form: Form::Raw, cap: Cap::Tail, line: Line::Name },
        label: Label::Qualified,
        export: Export::Hide(&["private", "fileprivate"]),
        // A module sees every file in it without an import.
        visibility: Visibility::Family,
        ambiguity: Ambiguity::OwnFirst,
        cross_file: false,
        imports: ImportPaths { root: false, suffixes: &[""] },
        dotted_dirs: false,
        hooks: NONE,
        legacy: None,
    };
    pub(super) const GRAPHQL: Spec = Spec {
        queries: crate::code::queries::sources::GRAPHQL,
        view: None,
        comments: HASH,
        // A description sits inside its definition, above the name.
        body: Body { doc: Doc::Capture, form: Form::Stripped, cap: Cap::Head, line: Line::Name },
        label: Label::Name,
        export: Export::All,
        // Codegen treats a fragment's name as global, as the schema does a type's.
        visibility: Visibility::Family,
        // A fragment two documents declare is spread from both, and `extend type` adds to every
        // definition of its type.
        ambiguity: Ambiguity::All,
        cross_file: false,
        imports: ImportPaths { root: false, suffixes: &[""] },
        dotted_dirs: false,
        // The default header, kept in `headers.json` so a document that starts or stops declaring a
        // name re-reads the documents that use it.
        hooks: Hooks { header: Some(|rel, source| crate::code::engine::default_header(Lang::GraphQl, rel, source)), ..NONE },
        legacy: None,
    };
    pub(super) const SHELL: Spec = Spec {
        queries: crate::code::queries::sources::SHELL,
        view: None,
        comments: HASH,
        body: PROSE,
        label: Label::Qualified,
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
