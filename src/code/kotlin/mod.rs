//! Kotlin, read by a hand walk into the node and edge kinds TypeScript writes (spec "### Kotlin").

mod calls;
mod declarations;

#[cfg(test)]
mod cases;

use tree_sitter::Node;

use crate::code::imports::Resolver;
use crate::code::index::{Header, Nested, QualifiedIndex};
use crate::code::jvm::{self, child, named, text, Scope};
use crate::code::lang::{Family, Lang};
use crate::model::Extraction;

/// The comment kinds a Kotlin doc block is read from.
pub(crate) const COMMENTS: &[&str] = &["line_comment", "multiline_comment"];

/// One parse per file; every pass shares the tree.
pub fn extract(resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let mut ex = Extraction::default();
    crate::code::lang::file_node(rel, &mut ex);
    let src = source.as_bytes();
    let Some(tree) = Lang::Kotlin.parse(src) else { return ex };
    let root = tree.root_node();
    // A repository whose globs reach no JVM file has no index; every lookup then finds nothing.
    let empty = QualifiedIndex::default();
    let index = resolver.index(Family::Jvm).unwrap_or(&empty);
    let scope = scope(root, src);
    let declared = declarations::scan(root, rel, src, &mut ex);
    jvm::link(&declared.types, &declared.supers, index, &scope, rel, &mut ex);
    let own = jvm::Own {
        rel,
        types: &declared.types,
        members: &declared.members,
        inheritable: &declared.members,
        statics: &jvm::NONE,
        arities: &declared.arities,
        supers: &declared.supers,
        shapes: &declared.shapes,
        index,
        scope: &scope,
        kind: jvm::Kind::All,
        args: None,
    };
    calls::scan(root, &calls::Ctx { own, src, d: &declared }, &mut ex);
    ex
}

/// The package line, every top-level name and every nested path, for the JVM index and for the
/// headers `widen` stores, from one parse. All of it comes from the declarations walk itself, so the
/// index never names a symbol the graph lacks.
pub fn header(source: &str) -> Header {
    let src = source.as_bytes();
    let Some(tree) = Lang::Kotlin.parse(src) else { return Header::default() };
    let root = tree.root_node();
    let package = package_of(root, src);
    let mut scratch = Extraction::default();
    let d = declarations::scan(root, "", src, &mut scratch);
    let arities = d.arities.iter().map(|(id, a)| (jvm::path_of(id).to_string(), a.clone())).collect();
    let nested = Nested { types: d.types, members: d.members.iter().map(|id| jvm::path_of(id).to_string()).collect(), arities, ..Default::default() };
    Header { scope: if package.is_empty() { Vec::new() } else { vec![package] }, top: d.top, nested, private: d.private, ..Default::default() }
}

/// The dotted name on the `package` line; `""` for a file in the default package.
pub(crate) fn package_of(root: Node, src: &[u8]) -> String {
    child(root, "package_header").and_then(|p| child(p, "identifier")).map(|i| dotted(i, src)).unwrap_or_default()
}

/// `a.b.C` from an `identifier` node, whatever whitespace or comment the file put between segments.
pub(crate) fn dotted(n: Node, src: &[u8]) -> String {
    named(n).into_iter().filter(|c| c.kind() == "simple_identifier").map(|c| text(c, src)).collect::<Vec<_>>().join(".")
}

/// The package and the imports, read from the tree. Kotlin has no static import: `import a.b.C.m`
/// names a member, and `resolve` finds it through `C`'s file as a nested path.
pub(crate) fn scope(root: Node, src: &[u8]) -> Scope {
    let mut s = Scope { package: package_of(root, src), ..Scope::default() };
    let lists = named(root).into_iter().filter(|n| n.kind() == "import_list").flat_map(named);
    let headers = named(root).into_iter().filter(|n| n.kind() == "import_header").chain(lists.filter(|n| n.kind() == "import_header"));
    for h in headers.collect::<Vec<_>>() {
        let Some(id) = child(h, "identifier") else { continue };
        let qualified = dotted(id, src);
        if child(h, "wildcard_import").is_some() {
            s.stars.push(qualified);
            continue;
        }
        let local = child(h, "import_alias")
            .and_then(|a| child(a, "type_identifier"))
            .map(|t| text(t, src).to_string())
            .unwrap_or_else(|| qualified.rsplit('.').next().unwrap_or(&qualified).to_string());
        s.singles.entry(local).or_default().insert(qualified);
    }
    s
}
