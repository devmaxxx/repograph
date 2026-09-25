//! C# extraction on inline sources, so the grammar's shape is pinned by the assertion.

use crate::code::imports::Resolver;
use crate::code::CodeExtractor;
use crate::config::Config;
use crate::model::{EdgeKind, Extraction, Extractor};

/// The defaults do not glob .NET until L7, and `Resolver::new` collects only globbed families.
pub(crate) fn dotnet_config() -> Config {
    Config { code_globs: vec!["**/*.cs".into(), "**/*.razor".into(), "**/*.cshtml".into()], ..Config::default() }
}

pub(crate) struct Repo {
    pub(crate) dir: tempfile::TempDir,
}

impl Repo {
    pub(crate) fn new(files: &[(&str, &str)]) -> Repo {
        let dir = tempfile::tempdir().unwrap();
        for (p, c) in files {
            let full = dir.path().join(p);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, c).unwrap();
        }
        Repo { dir }
    }

    pub(crate) fn resolver(&self) -> Resolver {
        Resolver::new(self.dir.path(), &dotnet_config()).unwrap()
    }

    /// Extracts `rel` as it is on disk, so the resolver and the extractor read the same bytes.
    pub(crate) fn extract(&self, rel: &str) -> Extraction {
        let src = std::fs::read_to_string(self.dir.path().join(rel)).unwrap();
        CodeExtractor::new(self.resolver()).extract(rel, &src)
    }
}

pub(crate) fn one(rel: &str, src: &str) -> Extraction {
    Repo::new(&[(rel, src)]).extract(rel)
}

pub(crate) fn ids(ex: &Extraction) -> Vec<&str> {
    ex.nodes.iter().map(|n| n.id.as_str()).collect()
}

// Read by the later cases that pin declarations and references, which this task does not add.
#[allow(dead_code)]
pub(crate) fn edges(ex: &Extraction, kind: EdgeKind) -> Vec<(&str, &str, &str)> {
    ex.edges.iter().filter(|e| e.kind == kind).map(|e| (e.source.as_str(), e.target.as_str(), e.context.as_str())).collect()
}

#[test]
fn a_cs_file_is_csharp_and_a_razor_view_is_razor() {
    use crate::code::lang::{Family, Lang};
    assert_eq!(Lang::of("Shop/OrderService.cs"), Some(Lang::CSharp));
    assert_eq!(Lang::of("Pages/Checkout.razor.cs"), Some(Lang::CSharp));
    assert_eq!(Lang::of("Pages/Checkout.razor"), Some(Lang::Razor));
    assert_eq!(Lang::of("Pages/Index.cshtml"), Some(Lang::Razor));
    assert_eq!(Lang::CSharp.family(), Family::DotNet);
    assert_eq!(Lang::Razor.family(), Family::DotNet);
    assert!(Lang::Razor.grammar().is_none());
    let ex = one("Shop/A.cs", "namespace Shop;\nclass A {}\n");
    assert!(ids(&ex).contains(&"file:Shop/A.cs"), "{:?}", ids(&ex));
}
