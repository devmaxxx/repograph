//! What Kotlin and Java share: node helpers, how a symbol's body is built, and the order a type
//! name is looked up in — this file first, then the one JVM index both languages fill.

use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use crate::code::index::QualifiedIndex;
use crate::model::{EdgeKind, Extraction};

pub(crate) fn text<'a>(n: Node, src: &'a [u8]) -> &'a str {
    n.utf8_text(src).unwrap_or("")
}

pub(crate) fn named<'t>(n: Node<'t>) -> Vec<Node<'t>> {
    let mut c = n.walk();
    n.named_children(&mut c).collect()
}

pub(crate) fn child<'t>(n: Node<'t>, kind: &str) -> Option<Node<'t>> {
    named(n).into_iter().find(|c| c.kind() == kind)
}

pub(crate) fn span(n: Node) -> (u32, u32) {
    (n.start_position().row as u32 + 1, n.end_position().row as u32 + 1)
}

/// `Outer` for `Outer.Inner`, `""` for a top-level path.
pub(crate) fn outer(path: &str) -> &str {
    path.rsplit_once('.').map_or("", |(o, _)| o)
}

/// `Outer.Inner` for `sym:a.kt::Outer.Inner`.
pub(crate) fn path_of(id: &str) -> &str {
    id.rsplit_once("::").map_or(id, |(_, p)| p)
}

/// A symbol's body as a TypeScript one is built: the comment block ending on the line above,
/// capped, then the line holding the name. The name's line and not the node's first, because a
/// Kotlin annotation sits inside the declaration node and `@Serializable` alone would be the one
/// line the retrievers index.
pub(crate) fn body(n: Node, name: Node, src: &[u8], comments: &[&str]) -> String {
    let mut parts = Vec::new();
    let mut next = n;
    while let Some(prev) = next.prev_named_sibling() {
        if !comments.contains(&prev.kind()) || prev.end_position().row + 1 < next.start_position().row {
            break;
        }
        parts.push(crate::code::symbols::comment_text(text(prev, src)));
        next = prev;
    }
    parts.reverse();
    let doc = crate::code::symbols::cap(parts.join("\n"), crate::code::symbols::DOC_CHARS);
    let at = name.start_byte();
    let start = src[..at].iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    let end = src[at..].iter().position(|&b| b == b'\n').map_or(src.len(), |i| at + i);
    let line = std::str::from_utf8(&src[start..end]).unwrap_or("").trim();
    if doc.is_empty() { line.to_string() } else { format!("{doc}\n{line}") }
}

/// A type name written inside the type at `at` (`""` at top level), resolved in this file: the
/// nearest enclosing type's nested name first, outward to the top level, as both compilers do.
/// `Outer.Inner` written in full resolves through its first segment.
pub(crate) fn in_file(types: &BTreeSet<String>, at: &str, written: &str) -> Option<String> {
    let (first, rest) = written.split_once('.').unwrap_or((written, ""));
    let mut scope = at;
    loop {
        let candidate = if scope.is_empty() { first.to_string() } else { format!("{scope}.{first}") };
        if types.contains(&candidate) {
            return Some(if rest.is_empty() { candidate } else { format!("{candidate}.{rest}") });
        }
        if scope.is_empty() {
            return None;
        }
        scope = outer(scope);
    }
}

/// The names a JVM file sees without qualifying them, read from its own header.
#[derive(Debug, Default, Clone)]
pub(crate) struct Scope {
    /// `""` for the default package.
    pub package: String,
    /// `import a.b.C`, and Kotlin's `import a.b.C as D`: the name the file uses -> `a.b.C`.
    pub singles: BTreeMap<String, String>,
    /// `import a.b.*`: `a.b`.
    pub stars: Vec<String>,
    /// `import static a.b.C.m`: `m` -> `a.b.C`.
    pub statics: BTreeMap<String, String>,
    /// `import static a.b.C.*`: `a.b.C`.
    pub static_stars: Vec<String>,
}

/// A declaration the index found: its file and its path under `sym:<rel>::`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    pub rel: String,
    pub path: String,
}

impl Target {
    pub(crate) fn id(&self) -> String {
        format!("sym:{}::{}", self.rel, self.path)
    }

    /// The top-level name, which is what an `Imports` context must spell for `impact` to list the importer.
    pub(crate) fn top(&self) -> &str {
        self.path.split('.').next().unwrap_or(&self.path)
    }
}

pub(crate) fn qualify(package: &str, name: &str) -> String {
    if package.is_empty() { name.to_string() } else { format!("{package}.{name}") }
}

/// Every declaration of a qualified name. The index holds `package.Top` only, so the longest prefix
/// it knows names the files and the rest is the nested path inside them: `a.b.Outer.Inner` is
/// `Outer.Inner` in each file declaring `a.b.Outer`.
pub(crate) fn declared(index: &QualifiedIndex, qualified: &str) -> Vec<Target> {
    let mut head = qualified;
    let mut tail: Vec<&str> = Vec::new();
    loop {
        let files = index.files(head);
        if !files.is_empty() {
            let top = head.rsplit('.').next().unwrap_or(head);
            let path = std::iter::once(top).chain(tail.iter().rev().copied()).collect::<Vec<_>>().join(".");
            return files.into_iter().map(|rel| Target { rel: rel.to_string(), path: path.clone() }).collect();
        }
        match head.rsplit_once('.') {
            Some((h, t)) => {
                tail.push(t);
                head = h;
            }
            None => return Vec::new(),
        }
    }
}

/// A type name as a file writes it — `C`, `C.Inner`, `a.b.C` — in the order both compilers look:
/// an explicit import, the file's own package, the on-demand imports, then the name read as fully
/// qualified. Every file declaring one qualified name is kept (`expect`/`actual`), but two on-demand
/// imports that each supply the name are the compiler's ambiguity error and resolve to nothing,
/// so a caller list never holds a guess.
pub(crate) fn resolve(index: &QualifiedIndex, scope: &Scope, written: &str) -> Vec<Target> {
    let (first, rest) = match written.split_once('.') {
        Some((f, r)) => (f, Some(r)),
        None => (written, None),
    };
    let join = |q: &str| rest.map_or_else(|| q.to_string(), |r| format!("{q}.{r}"));
    if let Some(q) = scope.singles.get(first) {
        return declared(index, &join(q));
    }
    let own = declared(index, &join(&qualify(&scope.package, first)));
    if !own.is_empty() {
        return own;
    }
    let mut starred: BTreeMap<String, Vec<Target>> = BTreeMap::new();
    for star in &scope.stars {
        let qualified = join(&qualify(star, first));
        let found = declared(index, &qualified);
        if !found.is_empty() {
            starred.insert(qualified, found);
        }
    }
    match starred.len() {
        0 if rest.is_some() => declared(index, written),
        1 => starred.into_values().next().unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// The ids a type name written inside the type at `at` stands for: this file's own types first,
/// because a name declared here shadows every import, then the index.
pub(crate) fn type_ids(types: &BTreeSet<String>, index: &QualifiedIndex, scope: &Scope, rel: &str, at: &str, written: &str) -> Vec<String> {
    if let Some(path) = in_file(types, at, written) {
        return vec![format!("sym:{rel}::{path}")];
    }
    resolve(index, scope, written).iter().map(Target::id).collect()
}

/// `Imports` for every explicit and static import that names a declaration in the repository, and
/// `Extends` for every supertype that resolves. A star import writes no `Imports`: which names it
/// brings in is known only at a use, and the use carries the edge.
pub(crate) fn link(types: &BTreeSet<String>, supers: &[(String, String)], index: &QualifiedIndex, scope: &Scope, rel: &str, ex: &mut Extraction) {
    let file_id = format!("file:{rel}");
    for qualified in scope.singles.values().chain(scope.statics.values()) {
        for t in declared(index, qualified) {
            if t.rel != rel {
                ex.edge(&file_id, &format!("file:{}", t.rel), EdgeKind::Imports, t.top(), rel);
            }
        }
    }
    for (from, written) in supers {
        // A type's header sees the types around it, not its own nested ones.
        for target in type_ids(types, index, scope, rel, outer(path_of(from)), written) {
            ex.edge(from, &target, EdgeKind::Extends, "", rel);
        }
    }
}

#[cfg(test)]
pub(crate) mod fixture {
    use crate::code::imports::Resolver;
    use crate::code::CodeExtractor;
    use crate::config::Config;
    use crate::model::{EdgeKind, Extraction, Extractor};

    /// A repository on disk, so the resolver indexes every file the way a build does.
    pub(crate) struct Repo {
        dir: tempfile::TempDir,
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

        /// The file at `rel` as `build` extracts it. The JVM globs are set because the index is
        /// filled only for the families `code_globs` reaches (L3), and the defaults do not yet.
        pub(crate) fn extract(&self, rel: &str) -> Extraction {
            let cfg = Config { code_globs: vec!["**/*.kt".into(), "**/*.java".into()], ..Config::default() };
            let text = std::fs::read_to_string(self.dir.path().join(rel)).unwrap();
            CodeExtractor::new(Resolver::new(self.dir.path(), &cfg).unwrap()).extract(rel, &text)
        }
    }

    pub(crate) fn one(rel: &str, src: &str) -> Extraction {
        Repo::new(&[(rel, src)]).extract(rel)
    }

    pub(crate) fn ids(ex: &Extraction) -> Vec<&str> {
        ex.nodes.iter().map(|n| n.id.as_str()).collect()
    }

    pub(crate) fn edges(ex: &Extraction, kind: EdgeKind) -> Vec<(&str, &str, &str)> {
        ex.edges.iter().filter(|e| e.kind == kind).map(|e| (e.source.as_str(), e.target.as_str(), e.context.as_str())).collect()
    }
}

#[cfg(test)]
mod cross {
    use super::fixture::{edges, Repo};
    use crate::model::EdgeKind;

    const INVOICE: &str = "package shop.billing;\n\npublic class Invoice {\n    public void send() {}\n}\n";

    #[test]
    fn a_kotlin_class_extends_a_java_class_it_imports() {
        let repo = Repo::new(&[
            ("shop/billing/Invoice.java", INVOICE),
            ("app/Draft.kt", "package app\n\nimport shop.billing.Invoice\n\nclass Draft : Invoice()\n"),
        ]);
        let ex = repo.extract("app/Draft.kt");
        assert!(edges(&ex, EdgeKind::Imports).contains(&("file:app/Draft.kt", "file:shop/billing/Invoice.java", "Invoice")), "{:?}", ex.edges);
        assert!(edges(&ex, EdgeKind::Extends).contains(&("sym:app/Draft.kt::Draft", "sym:shop/billing/Invoice.java::Invoice", "")), "{:?}", ex.edges);
    }

    #[test]
    fn a_java_class_implements_a_kotlin_interface_it_imports() {
        let repo = Repo::new(&[
            ("app/sync/Wipe.kt", "package app.sync\n\ninterface Wipe {\n    fun run()\n}\n"),
            ("shop/Job.java", "package shop;\n\nimport app.sync.Wipe;\n\npublic class Job implements Wipe {\n    public void run() {}\n}\n"),
        ]);
        let ex = repo.extract("shop/Job.java");
        assert!(edges(&ex, EdgeKind::Imports).contains(&("file:shop/Job.java", "file:app/sync/Wipe.kt", "Wipe")), "{:?}", ex.edges);
        assert!(edges(&ex, EdgeKind::Extends).contains(&("sym:shop/Job.java::Job", "sym:app/sync/Wipe.kt::Wipe", "")), "{:?}", ex.edges);
    }

    /// Pins the contract's empty-scope rule: a file with no package line is indexed by bare name.
    #[test]
    fn a_class_in_the_default_package_resolves_by_its_bare_name() {
        let repo = Repo::new(&[("Base.java", "public class Base {}\n"), ("Use.kt", "class Use : Base()\n")]);
        let ex = repo.extract("Use.kt");
        assert!(edges(&ex, EdgeKind::Extends).contains(&("sym:Use.kt::Use", "sym:Base.java::Base", "")), "{:?}", ex.edges);
    }
}
