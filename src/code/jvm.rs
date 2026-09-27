//! What Kotlin and Java share: node helpers, how a symbol's body is built, and the order a type
//! name is looked up in — this file first, then the one JVM index both languages fill.

use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use crate::code::index::{self, Admission, Arity, Call, QualifiedIndex};
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
    /// `import a.b.C`, and Kotlin's `import a.b.C as D`: the name the file uses -> every `a.b.C`
    /// bound to it. Kotlin lets two imports share a name (overloaded functions), so it is a set,
    /// and a lookup that meets more than one treats the name as ambiguous.
    pub singles: BTreeMap<String, BTreeSet<String>>,
    /// `import a.b.*`: `a.b`.
    pub stars: Vec<String>,
    /// `import static a.b.C.m`: `m` -> every `a.b.C`; `import static a.A.of` and `b.B.of` are both legal.
    pub statics: BTreeMap<String, BTreeSet<String>>,
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

fn last(qualified: &str) -> &str {
    qualified.rsplit('.').next().unwrap_or(qualified)
}

/// The files that declare `head`'s last segment as a type at the top level. A Kotlin top-level
/// `val network` sits in the index under `app.network` too, and cannot own `app.network.X`.
fn type_files<'i>(index: &'i QualifiedIndex, head: &str) -> Vec<&'i str> {
    index.files(head).into_iter().filter(|rel| index.is_type(rel, last(head))).collect()
}

/// Every file an import of `qualified` names. A top-level name of any kind matches whole, since
/// Kotlin imports a top-level function by name; otherwise the longest prefix some file declares
/// as a type names the files, and only those that declare the rest as a nested type or member count.
pub(crate) fn declared(index: &QualifiedIndex, qualified: &str) -> Vec<Target> {
    let exact = index.files(qualified);
    if !exact.is_empty() {
        let path = last(qualified).to_string();
        return exact.into_iter().map(|rel| Target { rel: rel.to_string(), path: path.clone() }).collect();
    }
    let mut head = qualified;
    while let Some((h, _)) = head.rsplit_once('.') {
        head = h;
        let files = type_files(index, head);
        if !files.is_empty() {
            let path = format!("{}{}", last(head), &qualified[head.len()..]);
            return files.into_iter().filter(|rel| index.declares(rel, &path)).map(|rel| Target { rel: rel.to_string(), path: path.clone() }).collect();
        }
    }
    Vec::new()
}

/// `qualified` read as a type: the longest prefix, no shorter than `floor`, that some file declares
/// as a type names the files, and each must declare the whole nested path as a type as well —
/// `Sub.Inner` inherited from `Base`, or a class inside a companion, is a path the graph lacks.
/// `None` when nothing down to the floor is a type, so the next lookup step may look.
fn as_type(index: &QualifiedIndex, qualified: &str, floor: &str) -> Option<Vec<Target>> {
    let mut head = qualified;
    loop {
        let files = type_files(index, head);
        if !files.is_empty() {
            let path = format!("{}{}", last(head), &qualified[head.len()..]);
            return Some(files.into_iter().filter(|rel| index.is_type(rel, &path)).map(|rel| Target { rel: rel.to_string(), path: path.clone() }).collect());
        }
        if head.len() <= floor.len() {
            return None;
        }
        head = head.rsplit_once('.')?.0;
    }
}

/// A type name as a file writes it — `C`, `C.Inner`, `a.b.C` — in the order both compilers look:
/// an explicit import, the file's own package, the on-demand imports, then the name read as fully
/// qualified. The first step that binds the name decides, even when what it binds is not in the
/// graph. The own-package and on-demand steps never strip past the name the file wrote: a JDK
/// `Exception` must not land in whatever `shop.Color.*` or a package-named value happens to be.
/// Every file declaring one qualified name is kept (`expect`/`actual`); two imports that bind the
/// name differently are the compiler's ambiguity error and resolve to nothing.
pub(crate) fn resolve(index: &QualifiedIndex, scope: &Scope, written: &str) -> Vec<Target> {
    let (first, rest) = match written.split_once('.') {
        Some((f, r)) => (f, Some(r)),
        None => (written, None),
    };
    let join = |q: &str| rest.map_or_else(|| q.to_string(), |r| format!("{q}.{r}"));
    if let Some(bound) = scope.singles.get(first) {
        let mut each = bound.iter();
        return match (each.next(), each.next()) {
            (Some(q), None) => as_type(index, &join(q), "").unwrap_or_default(),
            _ => Vec::new(),
        };
    }
    let own = qualify(&scope.package, first);
    if let Some(found) = as_type(index, &join(&own), &own) {
        return found;
    }
    let mut starred: BTreeMap<String, Vec<Target>> = BTreeMap::new();
    for star in &scope.stars {
        let base = qualify(star, first);
        if let Some(found) = as_type(index, &join(&base), &base) {
            starred.insert(base, found);
        }
    }
    match starred.len() {
        0 if rest.is_some() => as_type(index, written, "").unwrap_or_default(),
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
    for qualified in scope.singles.values().chain(scope.statics.values()).flatten() {
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

/// Requirement and ADR ids a comment or a string cites, from the declaration holding it, with the
/// contexts TypeScript's pass writes, so `ask` reads a citation the same in every language.
pub(crate) fn cite(n: Node, owner: &str, context: &str, rel: &str, src: &[u8], ex: &mut Extraction) {
    for hit in crate::ids::generic().find_all(text(n, src)) {
        ex.edge(owner, &hit.id, EdgeKind::References, context, rel);
    }
}

/// `sym:<rel>::<path>` split into its file and its path.
pub(crate) fn split_id(id: &str) -> Option<(&str, &str)> {
    id.strip_prefix("sym:")?.split_once("::")
}

/// How a type reaches names it does not declare itself.
#[derive(Debug, Default, Clone)]
pub(crate) struct Shape {
    /// Holds its outer instance: Kotlin's `inner`, Java's non-static member class. Any other
    /// nested type reaches no outer instance member.
    pub inner: bool,
    /// A Kotlin `object`: a nested type reaches its members without an instance.
    pub object: bool,
    /// A Kotlin `data class`, whose generated `copy` and `componentN` the file never writes.
    pub data: bool,
    /// A supertype the file never writes, such as an enum's `Enum`, which may hold any name.
    pub implicit: bool,
    /// The type's own type parameters, which a same-named repository type must never stand in for.
    pub type_params: BTreeSet<String>,
}

/// What a name looked up among a type's members binds to.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Bound {
    Found(Vec<String>),
    /// Something the file cannot read may hold the name, or two declarations do: the call is no edge.
    Refused,
    /// Provably nothing here holds it, so the next scope out may.
    Absent,
}

/// Every class inherits these from `Any` / `Object`, which the graph never declares.
const IMPLICIT_MEMBERS: [&str; 3] = ["toString", "equals", "hashCode"];
/// And a Java class these from `Object` as well.
const OBJECT_MEMBERS: [&str; 6] = ["getClass", "clone", "finalize", "notify", "notifyAll", "wait"];

/// Whether every Java class holds a method of this name without declaring it.
pub(crate) fn from_object(name: &str) -> bool {
    IMPLICIT_MEMBERS.contains(&name) || OBJECT_MEMBERS.contains(&name)
}

/// Which of a type's members a lookup reads. Kotlin reads them as one namespace, since a call may
/// go through a property of function type; Java keeps methods and fields apart, so a field never
/// stands in for the method of its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    All,
    /// Java methods, which every class also inherits from `Object`.
    Method,
    /// Java fields. Another file's member of the name counts though it may be a method: a lookup
    /// that meets one reads no type for it and so claims nothing.
    Field,
}

/// Type parameters in scope at the type `path`: its own and every enclosing type's. An outer one
/// reaches only an inner type, but masking more only costs an edge.
pub(crate) fn masked(shapes: &BTreeMap<String, Shape>, path: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut p = path;
    while !p.is_empty() {
        out.extend(shapes.get(p).map(|s| s.type_params.iter().cloned()).into_iter().flatten());
        p = outer(p);
    }
    out
}

/// A data class's generated members: `copy` and `component1`, `component2`, ….
fn generated(name: &str) -> bool {
    name == "copy" || name.strip_prefix("component").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The empty set, for a family that records no such members.
pub(crate) static NONE: BTreeSet<String> = BTreeSet::new();

/// One JVM file as the passes after declarations read it: what it declares, and what it sees.
#[derive(Clone, Copy)]
pub(crate) struct Own<'a> {
    pub rel: &'a str,
    pub types: &'a BTreeSet<String>,
    pub members: &'a BTreeSet<String>,
    /// The members a subtype inherits: all of them but Java's `private` ones.
    pub inheritable: &'a BTreeSet<String>,
    /// Methods every declaration of which is `static`, which a static nested type reaches
    /// without an outer instance.
    pub statics: &'a BTreeSet<String>,
    /// Per member id, the argument counts its declarations take.
    pub arities: &'a BTreeMap<String, Vec<Arity>>,
    /// Kotlin's `private` members, which only their own type and the types inside it reach.
    pub private: &'a BTreeSet<String>,
    /// The type path the call is made from.
    pub at: &'a str,
    pub supers: &'a [(String, String)],
    pub shapes: &'a BTreeMap<String, Shape>,
    pub index: &'a QualifiedIndex,
    pub scope: &'a Scope,
    pub kind: Kind,
    /// The arguments of the call being resolved; `None` admits any declaration.
    pub call: Option<Call>,
}

impl Own<'_> {
    fn arities_of(&self, id: &str) -> Option<&Vec<Arity>> {
        match split_id(id) {
            Some((rel, path)) if rel != self.rel => self.index.arities(rel, path),
            _ => self.arities.get(id),
        }
    }

    /// Whether a declaration of the member `id` takes this call's arguments. One that does not is
    /// read as absent, so the lookup goes on to the supertypes or the next scope out.
    fn admission(&self, id: &str) -> Admission {
        index::admission(self.arities_of(id), self.call)
    }

    /// Whether the member `id` of the type at `path` is visible where the call is made.
    fn visible(&self, id: &str, path: &str) -> bool {
        !self.private.contains(id) || self.at == path || self.at.strip_prefix(path).is_some_and(|rest| rest.starts_with('.'))
    }

    /// Java binds a supertype's method that takes the arguments without varargs before an own
    /// one that takes them only through varargs.
    fn beaten_by_fixed(&self, id: &str, path: &str, name: &str) -> bool {
        self.kind == Kind::Method
            && self.call.is_some()
            && !index::fixed(self.arities_of(id), self.call)
            && matches!(self.inherited(path, name, &mut BTreeSet::new()), Bound::Found(ids) if ids.iter().any(|i| index::fixed(self.arities_of(i), self.call)))
    }

    fn shape(&self, path: &str) -> Shape {
        self.shapes.get(path).cloned().unwrap_or_default()
    }

    /// Whether another file declares `path` as a member of the kind this lookup reads.
    fn elsewhere(&self, rel: &str, path: &str) -> bool {
        match self.kind {
            Kind::All => self.index.declares(rel, path),
            Kind::Method => self.index.declares_method(rel, path),
            Kind::Field => self.index.declares_member(rel, path),
        }
    }

    /// An unqualified `name` inside the type at `at`, walked outward as the compilers walk it: a
    /// type's own and inherited members, then the enclosing type's, but past a type that is not
    /// `inner` only into an `object`. A level hidden by that boundary that declares the name is
    /// refused rather than skipped, since a companion's members share the class's path and may be
    /// the ones meant — unless the name there is a nested type or a static method, which need no
    /// instance.
    pub(crate) fn member(&self, at: &str, name: &str) -> Bound {
        let mut path = at;
        let mut instance = true;
        while !path.is_empty() {
            let reachable = instance || self.shape(path).object;
            match self.level(path, name) {
                // A nested type needs no instance to be named.
                Bound::Found(ids) if reachable || ids.iter().all(|id| self.is_type(id) || self.statics.contains(id)) => return Bound::Found(ids),
                Bound::Found(_) => return Bound::Refused,
                Bound::Refused if reachable => return Bound::Refused,
                _ => {}
            }
            instance &= self.shape(path).inner;
            path = outer(path);
        }
        Bound::Absent
    }

    pub(crate) fn is_type(&self, id: &str) -> bool {
        split_id(id).is_some_and(|(rel, path)| if rel == self.rel { self.types.contains(path) } else { self.index.is_type(rel, path) })
    }

    /// `name` among the members of this file's type at `path`: its own declaration first when it
    /// takes the call's arguments, as C# binds, then those of every supertype. A supertype the
    /// repository does not declare, or one in another file that declares no `name` taking the
    /// arguments — the index holds no supertypes to walk on — may hold the name, so the lookup
    /// refuses rather than let a top-level or imported namesake stand in. Overloads of one arity
    /// told apart only by their argument types are the residual: the first level's is written.
    pub(crate) fn level(&self, path: &str, name: &str) -> Bound {
        let id = format!("sym:{}::{path}.{name}", self.rel);
        if self.members.contains(&id) && self.visible(&id, path) {
            match self.admission(&id) {
                Admission::Yes if self.beaten_by_fixed(&id, path, name) => return Bound::Refused,
                Admission::Yes => return Bound::Found(vec![id]),
                Admission::Unsure => return Bound::Refused,
                Admission::No => {}
            }
        }
        let object = self.kind == Kind::Method && OBJECT_MEMBERS.contains(&name);
        if IMPLICIT_MEMBERS.contains(&name) || object || (self.shape(path).data && generated(name)) {
            return Bound::Refused;
        }
        self.inherited(path, name, &mut BTreeSet::new())
    }

    /// `name` on the types `ids`, as an implicit receiver reads it: an unread type refuses.
    pub(crate) fn on_receiver(&self, ids: &[String], name: &str) -> Bound {
        if ids.is_empty() {
            return Bound::Refused;
        }
        let mut found = Vec::new();
        for id in ids {
            let Some((rel, path)) = split_id(id) else { return Bound::Refused };
            let here = if rel == self.rel {
                self.level(path, name)
            } else if self.elsewhere(rel, &format!("{path}.{name}")) && self.admission(&format!("{id}.{name}")) == Admission::Yes {
                Bound::Found(vec![format!("{id}.{name}")])
            } else {
                Bound::Refused
            };
            match here {
                Bound::Found(ids) => found.extend(ids),
                Bound::Refused => return Bound::Refused,
                Bound::Absent => {}
            }
        }
        settle(found, false)
    }

    /// `name` called on a value of the types `ids`: only a member some type provably declares.
    pub(crate) fn on_types(&self, ids: &[String], name: &str) -> Vec<String> {
        ids.iter()
            .flat_map(|id| match self.on_receiver(std::slice::from_ref(id), name) {
                Bound::Found(ids) => ids,
                _ => Vec::new(),
            })
            .collect()
    }

    fn inherited(&self, path: &str, name: &str, seen: &mut BTreeSet<String>) -> Bound {
        if !seen.insert(path.to_string()) {
            return Bound::Absent;
        }
        let from = format!("sym:{}::{path}", self.rel);
        let mut found = Vec::new();
        let mut unread = self.shape(path).implicit;
        for (_, written) in self.supers.iter().filter(|(f, _)| *f == from) {
            // A type's header sees the types around it, not its own nested ones.
            let targets = type_ids(self.types, self.index, self.scope, self.rel, outer(path), written);
            unread |= targets.is_empty();
            for t in targets {
                let Some((rel, tpath)) = split_id(&t) else { continue };
                let member = format!("{t}.{name}");
                let declared = if rel != self.rel { self.elsewhere(rel, &format!("{tpath}.{name}")) } else { self.inheritable.contains(&member) };
                let admission = if declared { self.admission(&member) } else { Admission::No };
                if admission == Admission::Unsure {
                    return Bound::Refused;
                }
                if admission == Admission::Yes {
                    found.push(member);
                } else if rel != self.rel {
                    unread = true;
                } else {
                    match self.inherited(tpath, name, seen) {
                        Bound::Found(ids) => found.extend(ids),
                        Bound::Refused => unread = true,
                        Bound::Absent => {}
                    }
                }
            }
        }
        settle(found, unread)
    }
}

/// Found ids when they share one path — an `expect` and its `actual`s — and a refusal when two
/// paths declare the name, which only an overload resolution this file does not run could settle.
fn settle(mut ids: Vec<String>, unread: bool) -> Bound {
    ids.sort();
    ids.dedup();
    let paths: BTreeSet<&str> = ids.iter().filter_map(|id| split_id(id).map(|(_, p)| p)).collect();
    match paths.len() {
        0 if unread => Bound::Refused,
        0 => Bound::Absent,
        1 => Bound::Found(ids),
        _ => Bound::Refused,
    }
}

/// The source set a file sits in: `androidMain` for `shared/src/androidMain/kotlin/…`.
fn source_set(rel: &str) -> Option<&str> {
    let mut parts = rel.split('/');
    parts.by_ref().find(|p| *p == "src")?;
    parts.next()
}

/// A platform's own main source set: `androidMain`, not `commonMain` or `androidHostTest`.
fn platform_main(rel: &str) -> bool {
    source_set(rel).and_then(|s| s.strip_suffix("Main")).is_some_and(|p| !p.is_empty() && p != "common")
}

/// Whether several files declaring one qualified name are an `expect` and its `actual`s: one file
/// outside every platform's main set, the rest inside one. Any other set is overloads or
/// namesakes, which only overload resolution could tell apart.
pub(crate) fn expect_family(files: &[&str]) -> bool {
    files.len() < 2 || files.iter().filter(|f| !platform_main(f)).count() == 1
}

/// The ids of one declaration a caller in `rel` links against. An `actual` in a platform's main
/// source set is linked only from common code, which runs on every platform, or from a source set
/// of that same platform (`androidHostTest` for `androidMain`); an Android module never links the
/// iOS actual. A declaration outside any platform set, the `expect` among them, is always kept.
pub(crate) fn same_platform(rel: &str, ids: Vec<String>) -> Vec<String> {
    if ids.len() < 2 {
        return ids;
    }
    let caller = source_set(rel);
    let keep = |id: &String| {
        let platform = split_id(id).and_then(|(r, _)| source_set(r)).and_then(|s| s.strip_suffix("Main")).filter(|p| !p.is_empty() && *p != "common");
        match (platform, caller) {
            (None, _) => true,
            (Some(p), Some(c)) => c.starts_with("common") || c.starts_with(p),
            (Some(_), None) => false,
        }
    };
    ids.into_iter().filter(keep).collect()
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

    #[test]
    fn a_kotlin_call_reaches_a_java_method_through_a_typed_property() {
        let repo = Repo::new(&[
            ("shop/orders/OrderService.java", "package shop.orders;\n\npublic class OrderService {\n    public void place(int n) {}\n}\n"),
            ("app/Checkout.kt", "package app\n\nimport shop.orders.OrderService\n\nclass Checkout(private val orders: OrderService) {\n    fun pay() { orders.place(1) }\n}\n"),
        ]);
        let ex = repo.extract("app/Checkout.kt");
        assert!(edges(&ex, EdgeKind::Calls).contains(&("sym:app/Checkout.kt::Checkout.pay", "sym:shop/orders/OrderService.java::OrderService.place", "")), "{:?}", ex.edges);
    }

    #[test]
    fn a_java_call_reaches_a_kotlin_method_through_a_typed_field() {
        let repo = Repo::new(&[
            ("app/network/Tokens.kt", "package app.network\n\nclass SessionTokens {\n    fun read(): String = \"\"\n}\n"),
            ("shop/Login.java", "package shop;\n\nimport app.network.SessionTokens;\n\npublic class Login {\n    private final SessionTokens tokens = null;\n    public void go() { tokens.read(); }\n}\n"),
        ]);
        let ex = repo.extract("shop/Login.java");
        assert!(edges(&ex, EdgeKind::Calls).contains(&("sym:shop/Login.java::Login.go", "sym:app/network/Tokens.kt::SessionTokens.read", "")), "{:?}", ex.edges);
    }

    /// Pins the contract's empty-scope rule: a file with no package line is indexed by bare name.
    #[test]
    fn a_class_in_the_default_package_resolves_by_its_bare_name() {
        let repo = Repo::new(&[("Base.java", "public class Base {}\n"), ("Use.kt", "class Use : Base()\n")]);
        let ex = repo.extract("Use.kt");
        assert!(edges(&ex, EdgeKind::Extends).contains(&("sym:Use.kt::Use", "sym:Base.java::Base", "")), "{:?}", ex.edges);
    }
}
