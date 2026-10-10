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
    pub extract: fn(&Resolver, &str, &str) -> Extraction,
    /// What a name-indexed family records of a file, from `(rel, source)`. A file whose header
    /// moves re-reads every other file of its family.
    pub header: Option<fn(&str, &str) -> Header>,
    /// What one globbed file adds to the resolver before any file is extracted.
    pub collect: Collect,
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

pub(crate) type Widen = fn(&[String], &[String], &Graph, &[String]) -> Vec<String>;

pub(crate) enum Collect {
    /// The header, when there is one, goes into the family's index and nothing else is kept.
    Header,
    /// Only the path is kept; the file is not opened.
    Path(fn(&mut Resolver, &str)),
    /// The function owns the file's whole contribution, its header included.
    Source(fn(&mut Resolver, &str, &str)),
}

/// A build manifest the language reads, matched by file name; read only when the globs reach the
/// language's family.
pub(crate) struct Manifest {
    pub matches: fn(&str) -> bool,
    pub read: fn(&mut Resolver, &str, &str),
}

/// The fields a reader leaves at their defaults, for `..NONE`.
pub(crate) const NONE: Reader = Reader {
    extract: files_only,
    header: None,
    collect: Collect::Header,
    state: None,
    manifest: None,
    widen: None,
    dotted_dirs: false,
};

fn files_only(_: &Resolver, rel: &str, _: &str) -> Extraction {
    let mut ex = Extraction::default();
    crate::code::lang::file_node(rel, &mut ex);
    ex
}

pub(crate) fn reader(lang: Lang) -> &'static Reader {
    use crate::code::*;
    match lang {
        Lang::TypeScript | Lang::Tsx => &typescript::READER,
        Lang::Vue => &vue::READER,
        Lang::CSharp => &csharp::READER,
        Lang::Razor => &razor::READER,
        Lang::Kotlin => &kotlin::READER,
        Lang::Java => &java::READER,
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
                s.0.entry((*v).type_id()).or_insert(v);
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
