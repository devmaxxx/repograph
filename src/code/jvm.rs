//! What Kotlin and Java share: node helpers, how a symbol's body is built, and the order a type
//! name is looked up in — this file first, then the one JVM index both languages fill.

use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use crate::code::index::{self, Admission, Arity, Call, QualifiedIndex, Supers};
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

/// The `Imports` edge a type named in a signature writes to each other file declaring it, spelled
/// as an import of that type is, so `impact` lists the file whose signatures name the type.
pub(crate) fn used(ids: &[String], rel: &str, ex: &mut Extraction) {
    for (to, path) in ids.iter().filter_map(|id| split_id(id)) {
        if to != rel {
            let top = path.split('.').next().unwrap_or(path);
            ex.edge(&format!("file:{rel}"), &format!("file:{to}"), EdgeKind::Imports, top, rel);
        }
    }
}

/// `DecoratedBy` from each annotated declaration to the annotation's own declaration when the name
/// resolves in the repository, exactly as a type use would, and nothing otherwise: a node every
/// annotated file shared would pull all of them into each update's co-declared closure, and an
/// annotation declared outside the repository names nothing `impact` can walk to.
pub(crate) fn decorate(types: &BTreeSet<String>, annotations: &[(String, String)], index: &QualifiedIndex, scope: &Scope, rel: &str, ex: &mut Extraction) {
    for (from, written) in annotations {
        // An annotation stands outside a type's body, so it sees the types around it, not its own.
        for target in type_ids(types, index, scope, rel, outer(path_of(from)), written) {
            ex.edge(from, &target, EdgeKind::DecoratedBy, "", rel);
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

/// What a file's header records of its supertypes, from the declarations pass run with an empty
/// `rel`; `None` when it declares no type.
pub(crate) fn recorded(types: &BTreeSet<String>, supers: &[(String, String)], shapes: &BTreeMap<String, Shape>, scope: &Scope) -> Option<Supers> {
    if types.is_empty() {
        return None;
    }
    let mut out = Supers { package: scope.package.clone(), ..Supers::default() };
    for (from, written) in supers {
        out.written.entry(path_of(from).to_string()).or_default().push(written.clone());
    }
    out.implicit = shapes.iter().filter(|(_, s)| s.implicit).map(|(p, _)| p.clone()).collect();
    out.classes = shapes.iter().filter_map(|(p, s)| s.superclass.clone().map(|c| (p.clone(), c))).collect();
    if !out.written.is_empty() {
        out.singles = scope.singles.clone();
        out.stars = scope.stars.clone();
    }
    Some(out)
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
    /// The supertype written as the class it extends: Java's `extends` on a class, Kotlin's
    /// supertype written with a constructor call. A member a class level declares beats an
    /// interface's, abstract or default, so that chain is walked first. `None` reads every
    /// supertype as one level, which is also how a Kotlin class whose secondary constructors call
    /// `super(…)` is read, since it writes its class supertype without a call.
    pub superclass: Option<String>,
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

/// A supertype as one level of the walk up from a subtype.
enum Hop {
    /// It declares the name, taking the arguments.
    Declares(String),
    /// Another file declares the name there without provably taking the arguments.
    Unread,
    /// The call is no edge whatever the other levels hold.
    Refused,
    /// It does not declare the name: what its own supertypes hold.
    Above(Walked),
}

/// What the supertypes of one type hold of a name, class levels apart from interface ones.
#[derive(Default)]
struct Walked {
    /// Declarations a class level binds, which beat every interface's.
    class: Vec<String>,
    /// A class level may hold the name unread, so no interface's declaration is proof.
    class_unread: bool,
    /// Declarations interface levels bind.
    found: Vec<String>,
    /// An interface level may hold the name unread.
    unread: bool,
    /// The call is no edge whatever the other levels hold.
    refused: bool,
}

impl Walked {
    fn refused() -> Walked {
        Walked { refused: true, ..Walked::default() }
    }

    /// Anything a level above may hold unread.
    fn doubtful(&self) -> bool {
        self.refused || self.class_unread || self.unread
    }

    fn bound(self) -> Bound {
        if self.refused {
            return Bound::Refused;
        }
        if !self.class.is_empty() {
            return settle(self.class, false);
        }
        if self.class_unread {
            return Bound::Refused;
        }
        settle(self.found, self.unread)
    }
}

/// A type's written supertypes, each as the ids it resolves to, empty when it resolves to none.
struct Chain {
    /// The class it extends, when the file tells which one that is.
    class: Option<Vec<String>>,
    interfaces: Vec<Vec<String>>,
    /// A supertype it never writes, such as an enum's.
    implicit: bool,
}

impl Chain {
    fn of<'w>(written: impl Iterator<Item = &'w String>, superclass: Option<&String>, implicit: bool, resolve: impl Fn(&str) -> Vec<String>) -> Chain {
        let mut chain = Chain { class: None, interfaces: Vec::new(), implicit };
        for w in written {
            if chain.class.is_none() && superclass == Some(w) {
                chain.class = Some(resolve(w));
            } else {
                chain.interfaces.push(resolve(w));
            }
        }
        chain
    }
}

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
    ///
    /// From outside the member's type only its non-private overloads count. A call that only a
    /// private overload takes is refused: the compiler rejects it or binds a namesake further
    /// out, and which of the two this lookup cannot tell.
    fn admission(&self, id: &str, inside: bool) -> Admission {
        let arities = self.arities_of(id);
        let all = index::admission(arities, self.call);
        if inside || !arities.is_some_and(|a| a.iter().any(|a| a.private)) {
            return all;
        }
        let open: Vec<Arity> = arities.into_iter().flatten().filter(|a| !a.private).copied().collect();
        match index::admission(Some(&open), self.call) {
            Admission::No if all != Admission::No => Admission::Unsure,
            admission => admission,
        }
    }

    /// Whether the call is made inside the type at `path`, where its private members are visible.
    fn inside(&self, path: &str) -> bool {
        self.at == path || self.at.strip_prefix(path).is_some_and(|rest| rest.starts_with('.'))
    }

    /// Whether the member `id` of the type at `path` is visible where the call is made.
    fn visible(&self, id: &str, path: &str) -> bool {
        !self.private.contains(id) || self.inside(path)
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
    /// takes the call's arguments, as C# binds, then those of every supertype, walked into other
    /// files through the supertypes their headers record. A supertype the repository does not
    /// declare, or one in another file that declares `name` without provably taking the
    /// arguments, may hold the name, so the lookup refuses rather than let a top-level or imported
    /// namesake stand in. Overloads of one arity told apart only by their argument types are the
    /// residual: the first level's is written.
    pub(crate) fn level(&self, path: &str, name: &str) -> Bound {
        let id = format!("sym:{}::{path}.{name}", self.rel);
        if self.members.contains(&id) && self.visible(&id, path) {
            match self.admission(&id, self.inside(path)) {
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
            } else if self.elsewhere(rel, &format!("{path}.{name}")) && self.admission(&format!("{id}.{name}"), false) == Admission::Yes {
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
        self.above(self.rel, path, name, seen).bound()
    }

    /// `name` among the supertypes of the type at `path` in `rel`, this file or another whose
    /// header records its supertypes. The class chain is walked first, as both compilers bind a
    /// class's member over an interface's: when a class level declares the name, the interfaces
    /// are never read, and when one may hold it unread, the call is refused. A supertype in
    /// another file that declares the name without provably taking the arguments counts as
    /// unread: its overloads are all the index knows of it.
    fn above(&self, rel: &str, path: &str, name: &str, seen: &mut BTreeSet<String>) -> Walked {
        if !seen.insert(format!("sym:{rel}::{path}")) {
            return Walked::default();
        }
        let Some(chain) = self.supertypes(rel, path) else { return Walked::refused() };
        let mut w = Walked { unread: chain.implicit, ..Walked::default() };
        if let Some(class) = chain.class {
            w.class_unread = class.is_empty();
            for t in class {
                match self.hop(&t, name, seen) {
                    Hop::Declares(id) => w.class.push(id),
                    Hop::Unread | Hop::Refused => w.class_unread = true,
                    Hop::Above(up) => {
                        w.class.extend(up.class);
                        w.found.extend(up.found);
                        w.class_unread |= up.class_unread || up.refused;
                        w.unread |= up.unread;
                    }
                }
            }
            if !w.class.is_empty() {
                return Walked { class: w.class, ..Walked::default() };
            }
            if w.class_unread {
                return Walked { class_unread: true, ..Walked::default() };
            }
        }
        for targets in chain.interfaces {
            w.unread |= targets.is_empty();
            for t in targets {
                match self.hop(&t, name, seen) {
                    Hop::Declares(id) => w.found.push(id),
                    Hop::Unread => w.unread = true,
                    Hop::Refused => return Walked::refused(),
                    Hop::Above(up) => {
                        w.unread |= up.doubtful();
                        w.found.extend(up.class.into_iter().chain(up.found));
                    }
                }
            }
        }
        w
    }

    /// The supertype `t` as a level of the walk up from a subtype.
    fn hop(&self, t: &str, name: &str, seen: &mut BTreeSet<String>) -> Hop {
        let Some((trel, tpath)) = split_id(t) else { return Hop::Above(Walked::default()) };
        let member = format!("{t}.{name}");
        let elsewhere = trel != self.rel;
        let declared = if elsewhere { self.elsewhere(trel, &format!("{tpath}.{name}")) } else { self.inheritable.contains(&member) };
        let admission = if declared { self.admission(&member, !elsewhere && self.inside(tpath)) } else { Admission::No };
        match admission {
            Admission::Unsure => Hop::Refused,
            Admission::Yes => Hop::Declares(member),
            Admission::No if elsewhere && declared => Hop::Unread,
            Admission::No => Hop::Above(self.above(trel, tpath, name, seen)),
        }
    }

    /// The written supertypes of the type at `path` in `rel`, resolved there. `None` when the
    /// index records nothing of `rel`'s supertypes.
    fn supertypes(&self, rel: &str, path: &str) -> Option<Chain> {
        // A type's header sees the types around it, not its own nested ones.
        if rel == self.rel {
            let from = format!("sym:{rel}::{path}");
            let written = self.supers.iter().filter(|(f, _)| *f == from).map(|(_, w)| w);
            let shape = self.shape(path);
            return Some(Chain::of(written, shape.superclass.as_ref(), shape.implicit, |w| type_ids(self.types, self.index, self.scope, rel, outer(path), w)));
        }
        let (recorded, types) = (self.index.supers(rel)?, self.index.types(rel)?);
        let scope = Scope { package: recorded.package.clone(), singles: recorded.singles.clone(), stars: recorded.stars.clone(), ..Scope::default() };
        let written = recorded.written.get(path).into_iter().flatten();
        let (class, implicit) = (recorded.classes.get(path), recorded.implicit.contains(path));
        Some(Chain::of(written, class, implicit, |w| type_ids(types, self.index, &scope, rel, outer(path), w)))
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
    fn a_kotlin_class_is_decorated_by_a_java_annotation_type_it_imports() {
        let repo = Repo::new(&[
            ("shop/meta/Audited.java", "package shop.meta;\n\npublic @interface Audited {}\n"),
            ("app/Job.kt", "package app\n\nimport shop.meta.Audited\n\n@Audited\nclass Job\n"),
        ]);
        let ex = repo.extract("app/Job.kt");
        assert!(edges(&ex, EdgeKind::DecoratedBy).contains(&("sym:app/Job.kt::Job", "sym:shop/meta/Audited.java::Audited", "")), "{:?}", ex.edges);
    }

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
