//! Everything the pipeline asks of a language, as one record per language. Each language's module
//! declares its `READER`, and `reader` is the only place that names them all: the extractor, the
//! resolver, the header index and the update's widening read this table instead of matching on
//! `Lang` each in its own way.

use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::code::imports::Resolver;
use crate::code::index::Header;
use crate::code::lang::Lang;
use crate::model::{Extraction, Graph};

pub(crate) struct Reader {
    /// Reads one file into an empty extraction: the file node, through `open` for a language read by
    /// its grammar alone, then what the file declares and uses.
    pub extract: Extract,
    /// What a name-indexed family records of a file, from `(rel, source)`. A file whose header
    /// moves re-reads every other file of its family.
    pub header: Option<fn(&str, &str) -> Header>,
    /// What one globbed file adds to the resolver before any file is extracted, and the header it
    /// read, if any, which goes into the family's index. `None` reads `header` alone, and a reader
    /// with neither never has its files opened.
    pub collect: Option<Collect>,
    /// The resolver's state for this language, created empty for every reader that has one.
    pub state: Option<fn() -> Box<dyn Any + Send + Sync>>,
    pub manifest: Option<Manifest>,
    /// A widening rule for a language that resolves across files without a header to compare:
    /// `(stale, removed, graph, all_rels)` to the other files to re-read.
    pub widen: Option<Widen>,
    /// Whether files under a dotted directory are read. CI keeps its scripts under `.github/`,
    /// while a tsconfig or a manifest there stays out.
    pub dotted_dirs: bool,
}

pub(crate) type Extract = fn(&Resolver, &str, &str, &mut Extraction);

pub(crate) type Collect = fn(&mut Resolver, &str, &str) -> Option<Header>;

pub(crate) type Widen = fn(&[String], &[String], &Graph, &[String]) -> Vec<String>;

/// A build manifest the language reads, matched by file name; read only when the globs reach the
/// language's family.
pub(crate) struct Manifest {
    pub matches: fn(&str) -> bool,
    pub read: fn(&mut Resolver, &str, &str),
}

impl Reader {
    /// Whether the resolver's walk opens this language's files before extraction.
    pub fn collects(&self) -> bool {
        self.header.is_some() || self.collect.is_some()
    }
}

/// The fields a reader leaves at their defaults, for `..NONE`.
pub(crate) const NONE: Reader = Reader {
    extract: |_, rel, _, ex| crate::code::lang::file_node(rel, ex),
    header: None,
    collect: None,
    state: None,
    manifest: None,
    widen: None,
    dotted_dirs: false,
};

/// What `lang`'s reader makes of one file.
pub(crate) fn extract(lang: Lang, resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let mut ex = Extraction::default();
    (reader(lang).extract)(resolver, rel, source, &mut ex);
    ex
}

/// The file node, then the file parsed with `lang`'s grammar; `None` when the grammar cannot read
/// it, and the file keeps its file node alone.
pub(crate) fn open(lang: Lang, rel: &str, source: &str, ex: &mut Extraction) -> Option<tree_sitter::Tree> {
    crate::code::lang::file_node(rel, ex);
    lang.parse(source.as_bytes())
}

pub(crate) fn reader(lang: Lang) -> &'static Reader {
    use crate::code::*;
    match lang {
        Lang::TypeScript | Lang::Tsx => &typescript::READER,
        Lang::Vue => &typescript::vue::READER,
        Lang::CSharp => &dotnet::csharp::READER,
        Lang::Razor => &dotnet::razor::READER,
        Lang::Kotlin => &jvm::kotlin::READER,
        Lang::Java => &jvm::java::READER,
        Lang::Rust => &rust_lang::READER,
        Lang::Python => &python::READER,
        Lang::Dart => &dart::READER,
        Lang::Swift => &swift::READER,
        Lang::GraphQl => &graphql::READER,
        Lang::Sql => &sql::READER,
        Lang::Bicep => &bicep::READER,
        Lang::Hcl => &hcl::READER,
        Lang::Shell => &shell::READER,
    }
}

/// The resolver's per-language state, one value per type.
#[derive(Default)]
pub(crate) struct States(HashMap<TypeId, Box<dyn Any + Send + Sync>>);

impl States {
    /// Every reader's state, empty.
    pub fn new() -> States {
        let mut s = States::default();
        for lang in Lang::ALL {
            if let Some(make) = reader(lang).state {
                let v = make();
                let first = s.0.insert((*v).type_id(), v).is_none();
                assert!(first, "two readers register one state type; a state is keyed by its type, so give each its own");
            }
        }
        s
    }

    pub fn get<T: Any>(&self) -> &T {
        self.0.get(&TypeId::of::<T>()).and_then(|v| v.downcast_ref()).expect("a state no reader registers")
    }

    pub fn get_mut<T: Any>(&mut self) -> &mut T {
        self.0.get_mut(&TypeId::of::<T>()).and_then(|v| v.downcast_mut()).expect("a state no reader registers")
    }
}

/// `state` for a reader whose state is `T`.
pub(crate) fn state<T: Any + Default + Send + Sync>() -> Box<dyn Any + Send + Sync> {
    Box::<T>::default()
}
