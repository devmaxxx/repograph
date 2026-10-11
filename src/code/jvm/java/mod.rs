//! Java, read by a hand walk into the node and edge kinds TypeScript writes (spec "### Java").

mod calls;
mod declarations;

#[cfg(test)]
mod cases;

use tree_sitter::Node;

use crate::code::imports::Resolver;
use crate::code::index::{Header, Nested, QualifiedIndex};
use crate::code::jvm::{self, Scope};
use crate::code::lang::{Family, Lang};
use crate::code::reader::{self, Reader};
use crate::code::syntax::{child, named, text};
use crate::model::Extraction;

/// The comment kinds a Java doc block is read from.
pub(crate) const COMMENTS: &[&str] = &["line_comment", "block_comment"];

pub(crate) const READER: Reader = Reader { extract, header: Some(|_, source| header(source)), ..reader::NONE };

fn extract(resolver: &Resolver, rel: &str, source: &str, ex: &mut Extraction) {
    let Some(tree) = reader::open(Lang::Java, rel, source, ex) else { return };
    let (src, root) = (source.as_bytes(), tree.root_node());
    // A repository whose globs reach no JVM file has no index; every lookup then finds nothing.
    let empty = QualifiedIndex::default();
    let index = resolver.index(Family::Jvm).unwrap_or(&empty);
    let scope = scope(root, src);
    let declared = declarations::scan(root, rel, src, ex);
    let methods = jvm::Own {
        rel,
        types: &declared.types,
        members: &declared.methods,
        inheritable: &declared.open_methods,
        statics: &declared.statics,
        arities: &declared.arities,
        private: &jvm::NONE,
        reach: &declared.reach,
        at: "",
        supers: &declared.supers,
        shapes: &declared.shapes,
        index,
        scope: &scope,
        kind: jvm::Kind::Method,
        call: None,
        member_types: true,
        past_unread: false,
    };
    jvm::link(&declared.supers, index, &scope, rel, |at, w| methods.type_at(at, w), ex);
    jvm::decorate(&declared.annotations, rel, |at, w| methods.type_at(at, w), ex);
    let fields = jvm::Own { members: &declared.values, inheritable: &declared.open_values, statics: &jvm::NONE, kind: jvm::Kind::Field, ..methods };
    let member_types = std::cell::RefCell::default();
    calls::scan(root, &calls::Ctx { methods, fields, src, d: &declared, member_types: &member_types }, ex);
}

/// The package line, every top-level type and every nested path, for the JVM index and for the
/// headers `widen` stores, from one parse. All of it comes from the declarations walk, so the index
/// never names a symbol the graph lacks.
pub fn header(source: &str) -> Header {
    let src = source.as_bytes();
    let Some(tree) = Lang::Java.parse(src) else { return Header::default() };
    let root = tree.root_node();
    let package = package_of(root, src);
    let mut scratch = Extraction::default();
    let d = declarations::scan(root, "", src, &mut scratch);
    let path = |id: &String| jvm::path_of(id).to_string();
    // A nested type another file may name keeps its path; a private member drops out, as no
    // other file reaches it, not even a subclass.
    let reached = |id: &String| d.types.contains(jvm::path_of(id)) || d.open_methods.contains(id) || d.open_values.contains(id);
    let values = d.open_values.difference(&d.open_methods).map(path).collect();
    let arities = d.arities.iter().filter(|(id, _)| d.open_methods.contains(*id)).map(|(id, a)| (path(id), a.clone())).collect();
    let supers = jvm::recorded(&d.types, &d.supers, &d.shapes, &scope(root, src));
    let reach = d.reach.iter().map(|(id, r)| (path(id), *r)).collect();
    let nested = Nested { members: d.members.iter().filter(|id| reached(id)).map(path).collect(), values, arities, types: d.types, supers, reach };
    Header { scope: if package.is_empty() { Vec::new() } else { vec![package] }, top: d.top, nested, ..Default::default() }
}

pub(crate) fn package_of(root: Node, src: &[u8]) -> String {
    child(root, "package_declaration")
        .and_then(|p| named(p).into_iter().find(|c| matches!(c.kind(), "scoped_identifier" | "identifier")))
        .map(|i| text(i, src).split_whitespace().collect())
        .unwrap_or_default()
}

/// The package and the four import forms. `static` is an unnamed child of the declaration.
pub(crate) fn scope(root: Node, src: &[u8]) -> Scope {
    let mut s = Scope { package: package_of(root, src), ..Scope::default() };
    for imp in named(root).into_iter().filter(|n| n.kind() == "import_declaration") {
        let Some(id) = named(imp).into_iter().find(|c| matches!(c.kind(), "scoped_identifier" | "identifier")) else { continue };
        let qualified: String = text(id, src).split_whitespace().collect();
        let mut c = imp.walk();
        let is_static = imp.children(&mut c).any(|k| k.kind() == "static");
        match (is_static, child(imp, "asterisk").is_some()) {
            (false, false) => {
                let local = qualified.rsplit('.').next().unwrap_or(&qualified).to_string();
                s.singles.entry(local).or_default().insert(qualified);
            }
            (false, true) => s.stars.push(qualified),
            (true, false) => {
                if let Some((ty, member)) = qualified.rsplit_once('.') {
                    s.statics.entry(member.to_string()).or_default().insert(ty.to_string());
                }
            }
            (true, true) => s.static_stars.push(qualified),
        }
    }
    s
}
