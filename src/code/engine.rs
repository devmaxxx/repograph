//! The shared reader: a language's two queries run over the tree the front parsed, their captures
//! turned into `Facts`, and the facts written as nodes and edges by the language's `Spec`. Hooks read
//! the same facts instead of walking the tree a second time.

use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::ops::Range;
use std::sync::{Arc, Mutex, OnceLock};

use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, QueryCursor, Tree};

use crate::code::index::Header;
use crate::code::queries::{self, Kind, Meta, Role};
use crate::code::syntax::{self, comment_text, last_row, text_span, DOC_CHARS};
use crate::model::{Graph, NodeKind};

// What `tests/engine.rs` names besides the engine itself: re-exported here, so the modules that
// define it stay private to the crate.
pub use crate::code::imports::Resolver;
pub use crate::code::lang::Lang;
pub use crate::code::queries::{compile, Compiled};
pub use crate::code::spec::{Ambiguity, Body, Cap, Doc, Export, Form, ImportPaths, Label, Line, Receivers, Sources, Spec, Visibility, NONE};
pub use crate::model::{EdgeKind, Extraction};

/// One file as the front read it.
pub struct File<'a> {
    pub lang: Lang,
    pub rel: &'a str,
    /// The file as read.
    pub text: &'a str,
    /// What the grammar read: `text`, or an embedded language's view of it.
    pub src: &'a str,
    /// `None` only for an embedded language whose file holds no embedded code.
    pub tree: Option<&'a Tree>,
}

#[derive(Debug, Clone)]
pub struct Def {
    pub id: String,
    pub qualified: String,
    pub label: String,
    pub name: String,
    pub(crate) kind: Kind,
    pub range: Range<usize>,
    pub line: u32,
    pub end: u32,
    /// The declaring definition, by index; `None` when the file declares it.
    pub parent: Option<usize>,
    pub export: bool,
    pub body: String,
}

#[derive(Debug, Clone)]
pub(crate) struct Ref {
    /// The innermost definition holding the site, by index; `None` for the file.
    pub source: Option<usize>,
    pub name: String,
    pub receiver: Option<String>,
    pub edge: EdgeKind,
    pub context: String,
    pub range: Range<usize>,
}

#[derive(Debug, Clone)]
pub struct Import {
    pub path: Option<String>,
    pub names: Vec<String>,
    pub alias: Option<String>,
    pub reexport: bool,
    pub range: Range<usize>,
    /// The files the site resolves to, resolved on the first read: a file read across for what it
    /// declares never needs them.
    pub targets: OnceLock<Vec<String>>,
}

impl Import {
    /// The files the site resolves to by `spec`'s import rule, `spec` being the importing file's.
    pub(crate) fn targets(&self, resolver: &Resolver, spec: &Spec, rel: &str) -> &[String] {
        self.targets.get_or_init(|| match spec.hooks.import {
            Some(hook) => hook(resolver, rel, self),
            None => import_targets(resolver, spec, rel, self),
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Cite {
    pub source: Option<usize>,
    pub context: &'static str,
    pub range: Range<usize>,
}

/// Names bound locally, each with the scopes it binds in: the scope the definition's own pattern
/// captures, or else the innermost `@local.scope` holding it, as nvim's locals convention has it.
#[derive(Debug, Default, Clone)]
pub(crate) struct Locals(HashMap<String, Vec<Binding>>);

#[derive(Debug, Clone)]
pub(crate) struct Binding {
    scope: Range<usize>,
    at: usize,
    /// The name stands for the receiver of the definition it is bound in, as a Go method's receiver
    /// and Lua's `self` parameter do, so a call through it reaches the definition's owner.
    owner: bool,
}

impl Locals {
    /// Whether a local binding of `name`'s first segment encloses `byte`, in a scope that does not
    /// also hold `apart` when given: a loop variable `x` binds the path `x/name` read through it, and
    /// the file's table `M` binds around its own `M.f` declared at `apart`, while a parameter named
    /// `M` is some other value.
    pub(crate) fn shadows(&self, name: &str, byte: usize, apart: Option<usize>) -> bool {
        let root = name.split(['.', '/']).next().unwrap_or(name);
        let binds = |b: &Binding| b.scope.contains(&byte) && !apart.is_some_and(|d| b.scope.contains(&d));
        self.0.get(root).is_some_and(|bindings| bindings.iter().any(binds))
    }

    /// The binding of `name` a site at `byte` reads: of those written before it, the innermost
    /// scope's, and in one scope the last, as `local self = …` rebinds a parameter for what follows
    /// it and not for what precedes it.
    fn binding(&self, name: &str, byte: usize) -> Option<&Binding> {
        let bindings = self.0.get(name)?.iter().filter(|b| b.scope.contains(&byte) && b.at <= byte);
        bindings.min_by_key(|b| (b.scope.len(), std::cmp::Reverse(b.at), !b.owner))
    }
}

#[derive(Debug, Default, Clone)]
pub struct Facts {
    /// The language the facts were read in, whose spec resolves their import sites.
    pub lang: Option<Lang>,
    pub defs: Vec<Def>,
    pub(crate) refs: Vec<Ref>,
    pub imports: Vec<Import>,
    pub(crate) cites: Vec<Cite>,
    pub(crate) locals: Locals,
    /// Package and namespace lines, in file order.
    pub namespaces: Vec<String>,
    /// What the file hands its importers, by name: a receiver naming the import reaches the members
    /// of these and nothing else. Empty, it reaches the file's unowned top-level definitions.
    pub(crate) roots: Vec<String>,
    /// Qualified names of `defs`, and of the exported ones, so a reference looks a name up instead of
    /// scanning every definition.
    declared: HashSet<String>,
    exported: HashSet<String>,
    /// A definition whose id an earlier one took, by its range and the earlier one's index: a site
    /// inside it is still owned by that id, as `finish` keeps the first node of an id.
    repeats: Vec<(Range<usize>, usize)>,
    /// A repeated id's own parent and export, by the first's index. An overload in an extension or
    /// a private overload declares the one node from another parent or with another export.
    redeclared: Vec<(Option<usize>, usize, bool)>,
    /// A scope owner's range and the qualified name it stands for: `extension Cart: Store` holds a
    /// clause of `Cart`'s, though it writes no node of its own.
    stands_for: Vec<(Range<usize>, String)>,
    /// `defs` and `repeats` by their node's range, the first to hold a range kept.
    at: HashMap<(usize, usize), usize>,
}

impl Facts {
    pub(crate) fn source_id(&self, rel: &str, source: Option<usize>) -> String {
        source.map_or_else(|| format!("file:{rel}"), |i| self.defs[i].id.clone())
    }

    pub(crate) fn declares(&self, qualified: &str) -> bool {
        self.declared.contains(qualified)
    }

    fn declares_visible(&self, qualified: &str) -> bool {
        self.exported.contains(qualified)
    }

    /// The definition whose node spans exactly `range`, a repeated one included.
    pub(crate) fn declared_at(&self, range: &Range<usize>) -> Option<usize> {
        self.at.get(&(range.start, range.end)).copied()
    }

    /// Every declaration site with the definition it declares, a repeated one included: an overload
    /// adds its own arity to the one node.
    pub(crate) fn sites(&self) -> impl Iterator<Item = (&Range<usize>, usize)> {
        self.defs.iter().enumerate().map(|(i, d)| (&d.range, i)).chain(self.repeats.iter().map(|(range, i)| (range, *i)))
    }

    /// The innermost scope owner around `byte`, by the name it stands for.
    fn stands_for(&self, byte: usize) -> Option<&str> {
        self.stands_for.iter().filter(|(range, _)| range.contains(&byte)).min_by_key(|(range, _)| range.len()).map(|(_, q)| q.as_str())
    }

    /// The innermost definition whose node holds `byte`. Of nodes spanning one range, as a wrapper
    /// and the statement it wraps do, the one qualified under the other is the inner.
    pub(crate) fn owner(&self, byte: usize) -> Option<usize> {
        let depth = |mut i: usize| {
            let mut n = 0;
            while let Some(p) = self.defs[i].parent {
                (n, i) = (n + 1, p);
            }
            n
        };
        self.sites()
            .filter(|(range, _)| range.contains(&byte))
            .min_by_key(|(range, i)| (range.len(), std::cmp::Reverse(depth(*i))))
            .map(|(_, i)| i)
    }
}

/// What a hook may write: it appends, and never replaces what the engine wrote.
pub(crate) struct Sink<'a>(&'a mut Extraction);

impl Sink<'_> {
    pub(crate) fn node(&mut self, kind: NodeKind, id: &str, label: &str, body: &str, file: &str, span: (u32, u32)) {
        self.0.node_span(kind, id, label, body, file, span);
    }

    pub(crate) fn edge(&mut self, source: &str, target: &str, kind: EdgeKind, context: &str, file: &str) {
        self.0.edge(source, target, kind, context, file);
    }

    /// What `facts` define and cite, as `extract` writes its own: for a hook that reads a view by
    /// its view language's queries.
    pub(crate) fn facts(&mut self, file: &File, facts: &Facts) {
        write(file, facts, self.0);
    }
}

/// Bounds runaway matching; a file that reaches it keeps what it matched.
const MATCH_LIMIT: u32 = 64 * 1024;

/// A definition or scope match before qualification.
struct Declared<'t> {
    node: Node<'t>,
    pattern: usize,
    role: Declaring,
    names: Vec<Node<'t>>,
    owner: Option<Node<'t>>,
    modifiers: Vec<String>,
    doc: Option<Node<'t>>,
}

#[derive(Clone, Copy, PartialEq)]
enum Declaring {
    Def(Kind),
    Owner,
    Module,
    Namespace,
}

fn text<'a>(n: Node, src: &'a str) -> &'a str {
    syntax::text(n, src.as_bytes())
}

fn matches_of<'t>(c: &'static Compiled, root: Node<'t>, src: &str, rel: &str, mut each: impl FnMut(usize, Vec<(Role, Node<'t>)>)) {
    let mut cursor = QueryCursor::new();
    cursor.set_match_limit(MATCH_LIMIT);
    cursor.set_max_start_depth(c.depth);
    let mut it = cursor.matches(&c.query, root, src.as_bytes());
    while let Some(m) = it.next() {
        let meta = &c.metas[m.pattern_index];
        if meta.within.is_some() || !meta.outside.is_empty() {
            // The outermost capture stands for the match, in whatever order the pattern lists them: a
            // name inside the construct may sit under a node the construct itself is not under.
            let outer = m.captures().iter().map(|cap| cap.node).min_by_key(|n| (n.start_byte(), std::cmp::Reverse(n.end_byte())));
            if meta.within.is_some_and(|kind| !outer.is_some_and(|n| inside(root, n, kind))) {
                continue;
            }
            if meta.outside.iter().any(|kind| outer.is_some_and(|n| inside(root, n, *kind))) {
                continue;
            }
        }
        each(m.pattern_index, m.captures().iter().map(|cap| (c.roles[cap.index as usize], cap.node)).collect());
    }
    if cursor.did_exceed_match_limit() {
        eprintln!("code: {rel} reached the query match limit; it keeps what matched before it");
    }
}

/// Whether a node of `kind` encloses `node`. It walks down from the root, because `parent` walks
/// down from the root itself on every call and a generated grammar nests thousands of levels deep.
fn inside(root: Node, node: Node, kind: u16) -> bool {
    let mut at = root;
    while at.kind_id() != kind {
        match at.child_with_descendant(node) {
            Some(next) if next != node => at = next,
            _ => return false,
        }
    }
    true
}

/// The facts `decl` and, when given, `refs` read from `file`. An import site's targets wait for a reader.
pub fn facts(spec: &Spec, decl: &'static Compiled, refs: Option<&'static Compiled>, file: &File) -> Facts {
    let mut facts = declare(spec, decl, file);
    if let Some(refs) = refs {
        refer(spec, refs, file, &mut facts);
    }
    facts
}

/// What `decl` read of files not yet extracted, each under the fingerprint of what it read. A
/// name-indexed family reads every file's declarations for the index before extracting any, and the
/// extraction reading them again cost the query twice. It takes its file's entry, and the pass drops
/// the rest when it ends: an update reads ahead every file of the family but extracts only those that
/// moved.
static DECLARED: Mutex<BTreeMap<String, (u64, Arc<Facts>)>> = Mutex::new(BTreeMap::new());

fn fingerprint(spec: &Spec, file: &File) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (std::ptr::from_ref(spec) as usize, file.lang, file.src).hash(&mut h);
    // A view's text is the file as written and its source the copy read; otherwise they are one string.
    if !std::ptr::eq(file.text, file.src) {
        file.text.hash(&mut h);
    }
    h.finish()
}

/// `decl`'s facts of `file`, read once for every header and cross-file reader that asks before the
/// file's own extraction.
pub(crate) fn declared(spec: &Spec, decl: &'static Compiled, file: &File) -> Arc<Facts> {
    let key = fingerprint(spec, file);
    let known = DECLARED.lock().unwrap_or_else(|e| e.into_inner()).get(file.rel).filter(|(k, _)| *k == key).map(|(_, f)| f.clone());
    if let Some(facts) = known {
        return facts;
    }
    let facts = Arc::new(declare(spec, decl, file));
    DECLARED.lock().unwrap_or_else(|e| e.into_inner()).insert(file.rel.to_string(), (key, facts.clone()));
    facts
}

/// Drops every file's read-ahead, which only the pass that read it can use.
pub(crate) fn forget_read_ahead() {
    DECLARED.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

#[cfg(test)]
pub(crate) fn read_ahead(rel: &str) -> bool {
    DECLARED.lock().unwrap_or_else(|e| e.into_inner()).contains_key(rel)
}

/// `facts`, taking what a header already read of the file.
fn extracted(spec: &Spec, decl: &'static Compiled, refs: Option<&'static Compiled>, file: &File) -> Facts {
    let known = DECLARED.lock().unwrap_or_else(|e| e.into_inner()).remove(file.rel);
    let mut facts = match known {
        Some((key, facts)) if key == fingerprint(spec, file) => Arc::try_unwrap(facts).unwrap_or_else(|shared| (*shared).clone()),
        _ => declare(spec, decl, file),
    };
    if let Some(refs) = refs {
        refer(spec, refs, file, &mut facts);
    }
    facts
}

fn declare(spec: &Spec, decl: &'static Compiled, file: &File) -> Facts {
    let mut facts = Facts { lang: Some(file.lang), ..Facts::default() };
    let Some(tree) = file.tree else { return facts };
    let (root, src) = (tree.root_node(), file.src);
    let (mut declared, mut exports) = (Vec::new(), HashSet::new());
    matches_of(decl, root, src, file.rel, |pattern, caps| {
        read_declaration(&caps, pattern, src, &mut declared);
        if let Some(import) = read_import(&caps, &decl.metas[pattern], src) {
            facts.imports.push(import);
        }
        exports.extend(caps.iter().filter(|(r, _)| *r == Role::ExportName).map(|(_, n)| fold(spec, text(*n, src))));
        facts.roots.extend(caps.iter().filter(|(r, _)| *r == Role::ExportRoot).map(|(_, n)| fold(spec, text(*n, src))));
    });
    qualify(spec, decl, file, declared, &mut facts);
    // A later `export { a }` exports what the file declared plain, which the declaration's own
    // words never say.
    if !exports.is_empty() {
        for d in facts.defs.iter_mut().filter(|d| d.parent.is_none() && exports.contains(&d.qualified)) {
            d.export = true;
        }
        for r in facts.redeclared.iter_mut().filter(|r| r.0.is_none()) {
            r.2 |= exports.contains(&facts.defs[r.1].qualified);
        }
    }
    facts.declared = facts.defs.iter().map(|d| d.qualified.clone()).collect();
    facts.exported = facts.defs.iter().filter(|d| d.export).map(|d| d.qualified.clone()).collect();
    let ranges = facts.defs.iter().enumerate().map(|(i, d)| (&d.range, i)).chain(facts.repeats.iter().map(|(r, i)| (r, *i)));
    for (r, i) in ranges {
        facts.at.entry((r.start, r.end)).or_insert(i);
    }
    facts
}

fn refer(spec: &Spec, refs: &'static Compiled, file: &File, facts: &mut Facts) {
    let Some(tree) = file.tree else { return };
    let (root, src) = (tree.root_node(), file.src);
    let (mut scopes, mut bound, mut owned) = (Vec::new(), Vec::new(), Vec::new());
    matches_of(refs, root, src, file.rel, |pattern, caps| {
        // A definition whose own pattern captures a scope binds there and nowhere else: a loop
        // variable covers the loop's body, never the iterable it sits beside.
        let own: Vec<Range<usize>> = caps.iter().filter(|(r, _)| *r == Role::LocalScope).map(|(_, n)| n.byte_range()).collect();
        for (role, node) in &caps {
            match role {
                Role::Reference(edge) => {
                    if refs.metas[pattern].owner_receiver {
                        owned.push(facts.refs.len());
                    }
                    facts.refs.push(read_reference(spec, &caps, *edge, *node, &refs.metas[pattern], src));
                }
                Role::LocalScope => scopes.push(node.byte_range()),
                Role::LocalDefinition => bound.push((node.start_byte(), fold(spec, text(*node, src)), own.clone(), refs.metas[pattern].owner_receiver)),
                Role::CiteComment => facts.cites.push(Cite { source: None, context: "comment", range: node.byte_range() }),
                Role::CiteString => facts.cites.push(Cite { source: None, context: "string", range: node.byte_range() }),
                _ => {}
            }
        }
    });
    // A citation inside another is read as part of the outer one, as a comment or string
    // walk that stops at the first it meets reads it.
    facts.cites.sort_by_key(|c| (c.range.start, std::cmp::Reverse(c.range.end)));
    let mut end = 0;
    facts.cites.retain(|c| {
        let outer = c.range.start >= end;
        end = end.max(c.range.end);
        outer
    });
    // A plain definition binds in the innermost scope holding it, whichever pattern captured that
    // scope: a method's body local stays in the method whose pattern captures it with the
    // parameters, never in the class around it. A binding that outlives its construct, as
    // Python's walrus outlives a comprehension, is a pattern that captures its own scope.
    let innermost = |at: usize| scopes.iter().filter(|s: &&Range<usize>| s.contains(&at)).min_by_key(|s| s.len()).cloned();
    let mut binds_owner = false;
    for (at, name, own, owner) in bound {
        let scopes = match own.is_empty() {
            true => vec![innermost(at).unwrap_or(0..src.len() + 1)],
            false => own,
        };
        binds_owner |= owner;
        facts.locals.0.entry(name).or_default().extend(scopes.into_iter().map(|scope| Binding { scope, at, owner }));
    }
    let owners: Vec<_> = facts.refs.iter().map(|r| facts.owner(r.range.start)).collect();
    facts.refs.iter_mut().zip(owners).for_each(|(r, o)| r.source = o);
    if binds_owner || !owned.is_empty() {
        owner_receivers(facts, &owned);
    }
    if !facts.cites.is_empty() {
        let decorators = syntax::Decorators::of(root);
        let owners: Vec<_> = facts.cites.iter().map(|c| facts.owner(decorators.said_at(c.range.start))).collect();
        facts.cites.iter_mut().zip(owners).for_each(|(c, o)| c.source = o);
    }
}

/// Rewrites a receiver that names the definition's own owner to that owner. The binding the site
/// reads decides: one bound as the receiver names the owner, and any other names some other value. An
/// unbound word names the owner only in a method, where a pattern in `owned` says the word is the
/// implicit receiver, as Lua's `self` in `function M:g()` is. Outside a member, the word names no owner
/// and stays as written, which nothing declares.
fn owner_receivers(facts: &mut Facts, owned: &[usize]) {
    for i in 0..facts.refs.len() {
        let r = &facts.refs[i];
        let Some(receiver) = &r.receiver else { continue };
        let names_owner = match facts.locals.binding(receiver, r.range.start) {
            Some(b) => b.owner,
            None => owned.binary_search(&i).is_ok() && r.source.is_some_and(|s| facts.defs[s].kind == Kind::Method),
        };
        let source = r.source.map(|s| facts.defs[s].qualified.as_str());
        if let Some((owner, _)) = source.and_then(|q| q.rsplit_once(['.', '/'])).filter(|_| names_owner) {
            facts.refs[i].receiver = Some(owner.to_string());
        }
    }
}

fn read_declaration<'t>(caps: &[(Role, Node<'t>)], pattern: usize, src: &str, out: &mut Vec<Declared<'t>>) {
    let declaring = caps.iter().find_map(|(role, node)| match role {
        Role::Definition(kind) => Some((Declaring::Def(*kind), *node)),
        Role::ScopeOwner => Some((Declaring::Owner, *node)),
        Role::ScopeModule => Some((Declaring::Module, *node)),
        Role::ScopeNamespace => Some((Declaring::Namespace, *node)),
        _ => None,
    });
    let Some((role, node)) = declaring else { return };
    let of = |want: Role| caps.iter().filter(move |(r, _)| *r == want).map(|(_, n)| *n);
    let mut names: Vec<Node> = of(Role::Name).collect();
    names.sort_by_key(|n| n.start_byte());
    out.push(Declared {
        node,
        pattern,
        role,
        names,
        owner: of(Role::Owner).next(),
        modifiers: Vec::new(),
        doc: of(Role::Doc).next(),
    });
    let last = out.last_mut().expect("just pushed");
    last.modifiers = caps.iter().filter(|(r, _)| *r == Role::Modifier).map(|(_, n)| text(*n, src).trim().to_string()).collect();
}

/// An `@import.path` capture as a hook sees it: a quoted path without its quotes.
pub(crate) fn import_path(text: &str) -> &str {
    text.trim_matches(|c| c == '"' || c == '\'')
}

fn read_import(caps: &[(Role, Node)], meta: &Meta, src: &str) -> Option<Import> {
    let site = caps.iter().find(|(r, _)| *r == Role::Import)?.1;
    let first = |want: Role| caps.iter().find(|(r, _)| *r == want).map(|(_, n)| text(*n, src).to_string());
    Some(Import {
        path: first(Role::ImportPath).map(|p| import_path(&p).to_string()),
        names: caps.iter().filter(|(r, _)| *r == Role::ImportName).map(|(_, n)| text(*n, src).to_string()).collect(),
        alias: first(Role::ImportAlias),
        reexport: meta.reexport,
        range: site.byte_range(),
        targets: OnceLock::new(),
    })
}

/// `name` as the spec's `fold` compares it.
fn fold(spec: &Spec, name: &str) -> String {
    spec.hooks.fold.map_or_else(|| name.to_string(), |fold| fold(name).into_owned())
}

fn read_reference(spec: &Spec, caps: &[(Role, Node)], edge: EdgeKind, site: Node, meta: &Meta, src: &str) -> Ref {
    // Repeated names spell one path, as they do for a definition: `var.region` reads `var/region`.
    // Each is folded alone, as a quoted part keeps its case beside an unquoted one.
    let names: Vec<String> = caps.iter().filter(|(r, _)| *r == Role::Name).map(|(_, n)| fold(spec, text(*n, src))).collect();
    let path = if names.is_empty() { fold(spec, text(site, src)) } else { names.join("/") };
    Ref {
        source: None,
        // A prefix spells the address a bare name stands for, as it does for a definition.
        name: format!("{}{path}", meta.prefix),
        receiver: caps.iter().find(|(r, _)| *r == Role::Receiver).map(|(_, n)| text(*n, src).to_string()),
        edge,
        context: meta.context.clone(),
        range: site.byte_range(),
    }
}

/// A container on the qualification stack: a container-kind definition or a scope owner.
struct Open {
    end: usize,
    qualified: String,
    join: char,
    /// The definition's index, or `None` for a scope owner, which writes no node.
    def: Option<usize>,
    /// An adopting scope owner's index in `adopting`.
    adopt: Option<usize>,
}

/// Parents, qualified names, ids, export and bodies, in source order. The first pattern to match a
/// node in a role wins, so a query puts specific patterns before general ones.
fn qualify(spec: &Spec, decl: &Compiled, file: &File, mut declared: Vec<Declared>, facts: &mut Facts) {
    let src = file.src;
    declared.sort_by_key(|d| (d.node.start_byte(), std::cmp::Reverse(d.node.end_byte()), d.pattern));
    // The first pattern to match a node in a role keeps it, and one pattern matching one node once
    // per name, as `let a = 1, b = 2` does, declares each name. Kept by a set, since another node of
    // the same span may sort between two matches of one node.
    let mut seen = HashMap::new();
    declared.retain(|d| match seen.entry((d.node.id(), std::mem::discriminant(&d.role))) {
        std::collections::hash_map::Entry::Vacant(e) => {
            e.insert((d.pattern, vec![d.names.clone()]));
            true
        }
        std::collections::hash_map::Entry::Occupied(mut e) => {
            let (pattern, names) = e.get_mut();
            let another = *pattern == d.pattern && !names.contains(&d.names);
            if another {
                names.push(d.names.clone());
            }
            another
        }
    });
    let fold = |s: &str| fold(spec, s);
    let mut stack: Vec<Open> = Vec::new();
    let mut ids: HashMap<String, usize> = HashMap::new();
    // Adopting owners by range and the name they stand for, and the definitions and repeats
    // directly under each: whether the type is the file's is known only once the file is read,
    // since an `impl` may come before its type.
    let mut adopting: Vec<(Range<usize>, String)> = Vec::new();
    let (mut members, mut remembers) = (Vec::new(), Vec::new());
    // A variant's first definition, by its parent node.
    let mut variants: HashMap<usize, Option<usize>> = HashMap::new();
    for d in declared {
        while stack.last().is_some_and(|open| open.end < d.node.end_byte()) {
            stack.pop();
        }
        if d.names.is_empty() || d.names.iter().any(|n| syntax::broken(*n)) {
            continue;
        }
        let name = d.names.iter().map(|n| fold(text(*n, src))).collect::<Vec<_>>().join("/");
        if d.role == Declaring::Namespace {
            facts.namespaces.push(name);
            continue;
        }
        let meta = &decl.metas[d.pattern];
        let parent = stack.last();
        let base = match d.owner {
            Some(owner) => Some((text(owner, src).to_string(), '.')),
            None => parent.map(|p| (p.qualified.clone(), p.join)),
        };
        let under = |base: &Option<(String, char)>, tail: String| match base {
            Some((b, j)) => format!("{b}{j}{tail}"),
            None => tail,
        };
        let qualified = under(&base, format!("{}{name}", meta.prefix));
        let kind = match d.role {
            Declaring::Def(kind) => kind,
            Declaring::Module => {
                stack.push(Open { end: d.node.end_byte(), qualified, join: '/', def: None, adopt: None });
                continue;
            }
            _ => {
                facts.stands_for.push((d.node.byte_range(), qualified.clone()));
                let adopt = meta.adopt.then(|| {
                    adopting.push((d.node.byte_range(), qualified.clone()));
                    adopting.len() - 1
                });
                stack.push(Open { end: d.node.end_byte(), qualified, join: '.', def: None, adopt });
                continue;
            }
        };
        let id = format!("sym:{}::{qualified}", file.rel);
        let export = meta.export.unwrap_or_else(|| exported(spec.export, &d.modifiers, &name));
        let adopted = parent.and_then(|p| p.adopt);
        if let Some(&first) = ids.get(&id) {
            facts.repeats.push((d.node.byte_range(), first));
            if let Some(k) = adopted {
                remembers.push((facts.redeclared.len(), k));
            }
            facts.redeclared.push((parent.and_then(|p| p.def), first, export));
            let sibling = || d.node.parent().map(|p| p.id());
            if meta.variant && variants.get(&first).is_some_and(|at| *at == sibling()) {
                facts.defs[first].end = facts.defs[first].end.max(text_span(d.node).1);
            }
            if let Some(join) = kind.join() {
                stack.push(Open { end: d.node.end_byte(), qualified, join, def: Some(first), adopt: None });
            }
            continue;
        }
        if meta.variant {
            variants.insert(facts.defs.len(), d.node.parent().map(|p| p.id()));
        }
        if let Some(k) = adopted {
            members.push((facts.defs.len(), k));
        }
        ids.insert(id.clone(), facts.defs.len());
        let (line, end) = text_span(d.node);
        let label = match spec.label {
            Label::Qualified => qualified.clone(),
            Label::Spelled(separator) => qualified.replace('/', separator),
            Label::Name => {
                let container = match (d.owner, parent.and_then(|p| p.def)) {
                    (None, Some(p)) => Some((facts.defs[p].label.clone(), parent.map_or('.', |p| p.join))),
                    _ => base.clone(),
                };
                under(&container, fold(text(d.names[d.names.len() - 1], src)))
            }
        };
        let def = Def {
            id,
            qualified: qualified.clone(),
            label,
            name,
            kind,
            range: d.node.byte_range(),
            line,
            end,
            parent: parent.and_then(|p| p.def),
            export,
            body: String::new(),
        };
        let signature = match spec.hooks.signature {
            Some(signature) => signature(file, &def),
            None => signature_line(spec.body.line, d.node, d.names[0], src),
        };
        let doc = doc_lines(spec, d.node, d.doc, src);
        let body = if doc.is_empty() { signature } else { format!("{doc}\n{signature}") };
        // A whole `const` or `macro_rules!` reads as its signature, so the cap holds the two together.
        let body = if matches!(spec.body.line, Line::Signature) { syntax::cap(body, DOC_CHARS) } else { body };
        let def = Def { body, ..def };
        facts.defs.push(def);
        if let Some(join) = kind.join() {
            stack.push(Open { end: d.node.end_byte(), qualified, join, def: Some(facts.defs.len() - 1), adopt: None });
        }
    }
    let types: Vec<Option<usize>> = adopting.iter().map(|(_, q)| {
        ids.get(&format!("sym:{}::{q}", file.rel)).copied().filter(|t| facts.defs[*t].kind.join() == Some('.'))
    }).collect();
    for (k, (range, _)) in adopting.into_iter().enumerate() {
        if let Some(t) = types[k] {
            facts.repeats.push((range, t));
        }
    }
    for (i, k) in members {
        if let Some(t) = types[k] {
            (facts.defs[i].parent, facts.defs[i].export) = (Some(t), false);
        }
    }
    for (i, k) in remembers {
        if let Some(t) = types[k] {
            (facts.redeclared[i].0, facts.redeclared[i].2) = (Some(t), false);
        }
    }
}

fn exported(rule: Export, modifiers: &[String], name: &str) -> bool {
    match rule {
        Export::Show(words) => modifiers.iter().any(|m| words.contains(&m.as_str())),
        Export::Hide(words) => !modifiers.iter().any(|m| words.contains(&m.as_str())),
        Export::Prefix(c) => !name.starts_with(c),
        Export::Capitalized => name.starts_with(char::is_uppercase),
        Export::All => true,
    }
}

fn signature_line(line: Line, def: Node, name: Node, src: &str) -> String {
    match line {
        Line::First => text(def, src).lines().next().unwrap_or("").to_string(),
        // Held to the definition's own span: a member declared on its container's line reads as
        // itself, not as the container's line.
        Line::Name => {
            let start = src[..name.start_byte()].rfind('\n').map_or(0, |i| i + 1).max(def.start_byte());
            let end = src[name.start_byte()..].find('\n').map_or(src.len(), |i| name.start_byte() + i).min(def.end_byte());
            src[start..end].trim().to_string()
        }
        Line::Whole => {
            let start = src[..name.start_byte()].rfind('\n').map_or(0, |i| i + 1);
            let end = src[name.start_byte()..].find('\n').map_or(src.len(), |i| name.start_byte() + i);
            src[start..end].trim().to_string()
        }
        Line::Row => {
            let start = src[..def.start_byte()].rfind('\n').map_or(0, |i| i + 1);
            let end = src[def.start_byte()..].find('\n').map_or(src.len(), |i| def.start_byte() + i);
            src[start..end].trim().to_string()
        }
        Line::Header => match def.child_by_field_name("body") {
            Some(body) => src[def.start_byte()..body.start_byte()].split_whitespace().collect::<Vec<_>>().join(" "),
            None => text(def, src).lines().next().unwrap_or("").trim().to_string(),
        },
        Line::Signature => {
            let head = def.child_by_field_name("body").map_or(def.end_byte(), |body| body.start_byte());
            src[def.start_byte()..head].split_whitespace().collect::<Vec<_>>().join(" ")
        }
    }
}

/// Whether `n` is of `kind`, or of `kind.field` when it also has that field: Rust's `line_comment.outer`.
fn is_kind(n: Node, kind: &str) -> bool {
    match kind.split_once('.') {
        Some((kind, field)) => n.kind() == kind && n.child_by_field_name(field).is_some(),
        None => n.kind() == kind,
    }
}

/// The named sibling before `n`. HCL's grammar puts the comments above a file's first block outside its
/// `body`, so a first child of a `body` looks past it.
fn above(n: Node) -> Option<Node> {
    n.prev_named_sibling().or_else(|| n.parent().filter(|p| p.kind() == "body").and_then(|p| p.prev_named_sibling()))
}

/// `comment_text`, and `#` and the language's own line markers too: `comment_text` serves the C
/// family alone. A string is unquoted and nothing else: its lines are prose, where a leading `*`,
/// `#` or `//` is markdown.
fn stripped(raw: &str, markers: &[&str]) -> String {
    // `comment_text` reads `///` as `//` and a stray `/`.
    let raw = raw.trim();
    let raw = raw.strip_prefix("///").unwrap_or(raw);
    // A string prefix (`r"""`, `u'`, `f"`) is no part of the prose, and a comment never starts with one.
    let literal = raw.trim_start_matches(|c: char| "rRuUbBfF".contains(c));
    if let Some(quote) = literal.chars().next().filter(|c| matches!(c, '"' | '\'')) {
        return literal.trim_matches(quote).trim().to_string();
    }
    let unmark = |l: &str| markers.iter().fold(l.trim_start_matches('#'), |l, m| l.trim_start_matches(m)).trim().to_string();
    comment_text(raw).lines().map(unmark).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

fn doc_lines(spec: &Spec, def: Node, captured: Option<Node>, src: &str) -> String {
    let body = spec.body;
    let parts: Vec<&str> = match body.doc {
        Doc::None => Vec::new(),
        Doc::Capture => captured.map(|n| text(n, src)).into_iter().collect(),
        Doc::Adjacent { kinds, between } => {
            let mut top = def;
            while let Some(p) = top.prev_named_sibling() {
                if !between.contains(&p.kind()) || last_row(p) + 1 != top.start_position().row { break }
                top = p;
            }
            let trails = |c: Node| above(c).is_some_and(|p| last_row(p) == c.start_position().row);
            let mut doc = Vec::new();
            let mut at = top;
            while let Some(p) = above(at) {
                // A shebang is the interpreter line, not documentation of what follows it.
                if !kinds.iter().any(|k| is_kind(p, k)) || text(p, src).starts_with("#!") || last_row(p) + 1 != at.start_position().row || trails(p) {
                    break;
                }
                doc.push(text(p, src).trim_end());
                at = p;
            }
            doc.reverse();
            doc
        }
    };
    let joined = match body.form {
        Form::Raw => parts.join("\n"),
        Form::Stripped => {
            let markers = spec.comments.map_or(&[][..], |c| c.lines);
            parts.iter().map(|p| stripped(p, markers)).filter(|p| !p.is_empty()).collect::<Vec<_>>().join("\n")
        }
    };
    match body.cap {
        Cap::Head => syntax::cap(joined, DOC_CHARS),
        Cap::Tail => {
            let mut out = joined;
            if out.len() > DOC_CHARS {
                let mut cut = out.len() - DOC_CHARS;
                while !out.is_char_boundary(cut) { cut += 1 }
                out.drain(..cut);
            }
            out
        }
    }
}

/// Definitions, their `Declares` edges, and the ids cited in comments and strings.
pub fn write(file: &File, facts: &Facts, ex: &mut Extraction) {
    let rel = file.rel;
    for d in &facts.defs {
        ex.node_span(NodeKind::Symbol, &d.id, &d.label, &d.body, rel, (d.line, d.end));
        ex.edge(&facts.source_id(rel, d.parent), &d.id, EdgeKind::Declares, if d.export { "export" } else { "" }, rel);
    }
    for &(parent, first, export) in &facts.redeclared {
        ex.edge(&facts.source_id(rel, parent), &facts.defs[first].id, EdgeKind::Declares, if export { "export" } else { "" }, rel);
    }
    for c in &facts.cites {
        for hit in crate::ids::generic().find_all(&file.src[c.range.clone()]) {
            ex.edge(&facts.source_id(rel, c.source), &hit.id, EdgeKind::References, c.context, rel);
        }
    }
}

/// The repository files an import site names, by the default rule: relative to the importer's
/// directory, then under each of the spec's roots in order, each with its suffixes, kept only when
/// the walk globbed the result. A path naming a directory names every file of the importer's
/// language directly in it, as a Terraform module source does. The importer's own file and own
/// directory add nothing it does not already hold, so they name nothing. A `Dotted` spec reads none of
/// this: see `dotted`.
pub fn import_targets(resolver: &Resolver, spec: &Spec, rel: &str, import: &Import) -> Vec<String> {
    let Some(path) = &import.path else { return Vec::new() };
    let (roots, suffixes) = match spec.imports {
        ImportPaths::Relative { roots, suffixes } => (roots, suffixes),
        ImportPaths::Dotted { roots, suffixes } => return dotted(resolver, rel, &path.replace('.', "/"), roots, suffixes),
        ImportPaths::ModulePrefix { manifest, directive, skip } => return under_module(resolver, rel, path, manifest, directive, skip),
    };
    let dir = rel.rsplit_once('/').map_or("", |(d, _)| d);
    let bases = std::iter::once(dir).chain(roots.iter().copied());
    for base in bases {
        for suffix in suffixes {
            let Some(target) = crate::code::prose::join_under(base, &format!("{path}{suffix}")) else { continue };
            if target == rel || target == dir {
                continue;
            }
            if resolver.globbed(&target) {
                return vec![target];
            }
            let module = Lang::of(rel).map(|lang| resolver.in_dir(&target, lang)).unwrap_or_default();
            if !module.is_empty() {
                return module;
            }
        }
    }
    Vec::new()
}

/// The first globbed file `path` names under one of `roots`, by the first suffix that names one. A
/// module names a file, never a directory's every file.
fn dotted(resolver: &Resolver, rel: &str, path: &str, roots: &[&str], suffixes: &[&str]) -> Vec<String> {
    let candidates = roots.iter().flat_map(|root| suffixes.iter().map(move |suffix| (root, suffix)));
    candidates
        .filter_map(|(root, suffix)| crate::code::prose::join_under(root, &format!("{path}{suffix}")))
        .find(|target| target != rel && resolver.globbed(target))
        .into_iter()
        .collect()
}

/// The files of the directory `path` names under the importer's module, by `ImportPaths::ModulePrefix`.
fn under_module(resolver: &Resolver, rel: &str, path: &str, manifest: &'static str, directive: &str, skip: &[&str]) -> Vec<String> {
    let dir = rel.rsplit_once('/').map_or("", |(d, _)| d);
    let Some((base, module)) = resolver.module_of(dir, manifest, directive) else { return Vec::new() };
    // `import_path` unquotes the two quotes Go shares with other languages, not a raw string's backticks.
    let path = path.trim_matches('`');
    let Some(sub) = path.strip_prefix(module.as_str()).filter(|sub| sub.is_empty() || sub.starts_with('/')) else { return Vec::new() };
    let target = crate::code::prose::join_under(&base, sub.trim_start_matches('/'));
    let Some((target, lang)) = target.zip(Lang::of(rel)) else { return Vec::new() };
    let mut files = resolver.in_dir(&target, lang);
    files.retain(|f| f != rel && !skip.iter().any(|s| f.ends_with(s)));
    files
}

/// The files a reference may resolve into besides its own, by the spec's visibility, with what each
/// declares. Computed once per file: it does not depend on the name, and `Family` has none.
fn reach(resolver: &Resolver, spec: &Spec, file: &File, facts: &Facts) -> Vec<(String, Arc<Facts>)> {
    let rel = file.rel;
    let files: Vec<String> = match spec.visibility {
        Visibility::File | Visibility::Family => Vec::new(),
        Visibility::Imported { transitive } => {
            let mut seen = BTreeSet::from([rel.to_string()]);
            let mut queue: VecDeque<String> = facts.imports.iter().flat_map(|i| i.targets(resolver, spec, rel).iter().cloned()).collect();
            while let Some(next) = queue.pop_front() {
                if !seen.insert(next.clone()) {
                    continue;
                }
                if transitive {
                    if let Some(f) = resolver.facts(&next) {
                        let theirs = f.lang.map(crate::code::spec::spec).unwrap_or(spec);
                        queue.extend(f.imports.iter().flat_map(|i| i.targets(resolver, theirs, &next).iter().cloned()));
                    }
                }
            }
            seen.remove(rel);
            seen.into_iter().collect()
        }
        Visibility::Directory => resolver.siblings(rel, file.lang),
    };
    files.into_iter().filter_map(|f| resolver.facts(&f).map(|facts| (f, facts))).collect()
}

/// The files `reach` or the family's index holds a visible `name` in.
fn visible(resolver: &Resolver, spec: &Spec, file: &File, reach: &[(String, Arc<Facts>)], name: &str) -> Vec<String> {
    if let Visibility::Family = spec.visibility {
        return resolver.index(file.lang.family())
            .map(|index| index.files(name).into_iter().filter(|f| *f != file.rel).map(str::to_string).collect())
            .unwrap_or_default();
    }
    // A directory is one module, so a sibling's every declaration is in scope, exported or not.
    let seen = |facts: &Facts| match spec.visibility {
        Visibility::Directory => facts.declares(name),
        _ => facts.declares_visible(name),
    };
    reach.iter().filter(|(_, facts)| seen(facts)).map(|(f, _)| f.clone()).collect()
}

/// Every reference of a kind no hook owns, resolved by name; one with a receiver only where the spec
/// reads receivers itself (`Receivers::Imports`).
pub fn resolve(resolver: &Resolver, spec: &Spec, file: &File, facts: &Facts, ex: &mut Extraction) {
    let rel = file.rel;
    let reach = reach(resolver, spec, file, facts);
    // The files whose declaration of `name` a reference reaches, by the spec's ambiguity rule.
    let declaring = |name: &str| -> Vec<String> {
        let own = facts.declares(name);
        let others = visible(resolver, spec, file, &reach, name);
        match spec.ambiguity {
            Ambiguity::OwnFirst if own => vec![rel.to_string()],
            Ambiguity::OwnFirst => others,
            Ambiguity::Unique => match (own, others.as_slice()) {
                (true, []) => vec![rel.to_string()],
                (false, [_]) => others,
                _ => Vec::new(),
            },
            Ambiguity::OwnThenUnique if own => vec![rel.to_string()],
            Ambiguity::OwnThenUnique => match others.as_slice() {
                [_] => others,
                _ => Vec::new(),
            },
            Ambiguity::All => own.then(|| rel.to_string()).into_iter().chain(others).collect(),
        }
    };
    let mut receiving: Option<Receiving> = None;
    for r in &facts.refs {
        if spec.hooks.owns.contains(&r.edge) {
            continue;
        }
        if let Some(receiver) = &r.receiver {
            if let Receivers::Imports = spec.receivers {
                let receiving = receiving.get_or_insert_with(|| Receiving::of(facts));
                // A directory that is the module declares its members across its files.
                let module = |member: &str| match spec.visibility {
                    Visibility::Directory => declaring(member).into_iter().map(|f| format!("sym:{f}::{member}")).collect(),
                    _ => Vec::new(),
                };
                let all = matches!(spec.ambiguity, Ambiguity::All | Ambiguity::OwnFirst);
                let tos = receiving.target(facts, r, receiver, all, module, || Receiving::imports(resolver, spec, rel, facts));
                let from = facts.source_id(rel, r.source);
                let container = r.source.and_then(|i| facts.defs[i].parent).map(|p| facts.defs[p].id.as_str());
                for to in tos.iter().filter(|to| **to != from && Some(to.as_str()) != container) {
                    ex.edge(&from, to, r.edge, &r.context, rel);
                }
            }
            continue;
        }
        if facts.locals.shadows(&r.name, r.range.start, None) {
            continue;
        }
        let targets = declaring(&r.name);
        // Only a type inherits. A clause a scope owner holds is the type the owner stands for, found
        // by its outermost name as any other name is; one a file or a function holds names no subtype.
        let froms = if !matches!(r.edge, EdgeKind::Extends | EdgeKind::Implements)
            || r.source.is_some_and(|i| facts.defs[i].kind.join() == Some('.'))
        {
            vec![facts.source_id(rel, r.source)]
        } else if let Some(owner) = r.source.is_none().then(|| facts.stands_for(r.range.start)).flatten() {
            let outer = owner.split('.').next().unwrap_or(owner);
            declaring(outer).into_iter().map(|f| format!("sym:{f}::{owner}")).collect()
        } else {
            continue;
        };
        let container = r.source.and_then(|i| facts.defs[i].parent).map(|p| facts.defs[p].id.clone());
        for (from, target) in froms.iter().flat_map(|from| targets.iter().map(move |t| (from, t))) {
            let to = format!("sym:{target}::{}", r.name);
            if to != *from && Some(&to) != container.as_ref() {
                ex.edge(from, &to, r.edge, &r.context, rel);
            }
        }
    }
}

/// The name most of `files` declare themselves under. A directory can hold a file of another package,
/// as a `//go:build ignore` generator in `package main` is, and that file sorting first names nothing.
fn package_name(resolver: &Resolver, files: &[String]) -> Option<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for name in files.iter().filter_map(|f| resolver.facts(f)?.namespaces.first().cloned()) {
        match counts.iter_mut().find(|(n, _)| *n == name) {
            Some((_, count)) => *count += 1,
            None => counts.push((name, 1)),
        }
    }
    counts.into_iter().rev().max_by_key(|(_, count)| *count).map(|(name, _)| name)
}

/// The members each import alias exposes, with every definition of each.
type Imported = HashMap<String, HashMap<String, BTreeSet<String>>>;

/// What a receiver names from one file, built at its first reference with a receiver: the file's
/// own definitions by the receiver and member they spell, and, only once a receiver is no member of
/// the file's own, the members each import alias exposes. A file that calls through `self` alone
/// never reads the files it requires.
struct Receiving<'f> {
    own: HashMap<(&'f str, &'f str), usize>,
    imported: OnceCell<Imported>,
}

impl<'f> Receiving<'f> {
    fn of(facts: &'f Facts) -> Self {
        let own = facts.defs.iter().enumerate().filter_map(|(i, d)| d.qualified.rsplit_once(['.', '/']).map(|q| (q, i))).collect();
        Receiving { own, imported: OnceCell::new() }
    }

    fn imports(resolver: &Resolver, spec: &Spec, rel: &str, facts: &Facts) -> Imported {
        let mut files: HashMap<String, BTreeSet<&str>> = HashMap::new();
        for i in &facts.imports {
            let targets = i.targets(resolver, spec, rel);
            // An import with no alias is named by what its files call themselves: Go's
            // `import "a/render"` binds `render` by `package render`.
            let named = || package_name(resolver, targets);
            if let Some(alias) = i.alias.clone().or_else(named) {
                files.entry(alias).or_default().extend(targets.iter().map(String::as_str));
            }
        }
        files.into_iter().map(|(alias, files)| {
            let mut members: HashMap<String, BTreeSet<String>> = HashMap::new();
            // A file that re-exports a whole module, as Lua's `return require("a")` does, hands
            // that module's members on as its own.
            let mut queue: Vec<String> = files.into_iter().map(str::to_string).collect();
            let mut seen = BTreeSet::new();
            while let Some(f) = queue.pop() {
                let Some(theirs) = seen.insert(f.clone()).then(|| resolver.facts(&f)).flatten() else { continue };
                for (member, id) in exposed(&theirs) {
                    members.entry(member.to_string()).or_default().insert(id.to_string());
                }
                let their_spec = theirs.lang.map(crate::code::spec::spec).unwrap_or(spec);
                for i in theirs.imports.iter().filter(|i| i.reexport && i.names.is_empty() && i.alias.is_none()) {
                    queue.extend(i.targets(resolver, their_spec, &f).iter().cloned());
                }
            }
            (alias, members)
        }).collect()
    }

    /// The definitions `receiver.name` names. A table the file declares members of is the file's
    /// own, unless a binding around the site and not around the member names another value; next,
    /// `module` names what the rest of the module declares as `receiver.name`; last, an import's
    /// alias reaches the member of that name its files expose, the one member unless `all` takes
    /// every definition of it. A binding around the site shadows both of those.
    fn target(
        &self,
        facts: &Facts,
        r: &Ref,
        receiver: &str,
        all: bool,
        module: impl FnOnce(&str) -> Vec<String>,
        imports: impl FnOnce() -> Imported,
    ) -> Vec<String> {
        if let Some(&i) = self.own.get(&(receiver, r.name.as_str())) {
            let def = &facts.defs[i];
            let shadowed = facts.locals.shadows(receiver, r.range.start, Some(def.range.start));
            return if shadowed { Vec::new() } else { vec![def.id.clone()] };
        }
        if facts.locals.shadows(receiver, r.range.start, None) {
            return Vec::new();
        }
        let declared = module(&format!("{receiver}.{}", r.name));
        if !declared.is_empty() {
            return declared;
        }
        let Some(ids) = self.imported.get_or_init(imports).get(receiver).and_then(|m| m.get(r.name.as_str())) else { return Vec::new() };
        match (all, ids.len()) {
            (true, _) | (false, 1) => ids.iter().cloned().collect(),
            _ => Vec::new(),
        }
    }
}

/// What a module hands a receiver that names it, by member name: the members of its export roots
/// (`return M` hands `M.f` as `f`), or with no root its unowned top-level definitions, as a package
/// hands its package-level names. Hidden definitions and deeper members stay out.
fn exposed(facts: &Facts) -> impl Iterator<Item = (&str, &str)> {
    facts.defs.iter().filter(|d| d.export).filter_map(|d| {
        let member = match facts.roots.is_empty() {
            true => Some(d.qualified.as_str()),
            false => facts.roots.iter().find_map(|root| d.qualified.strip_prefix(root.as_str())?.strip_prefix('.')),
        }?;
        (!member.contains(['.', '/'])).then_some((member, d.id.as_str()))
    })
}

/// `apply_diff`'s widening for the languages that resolve across files with no header to compare.
/// Where names are seen through imports, a changed or removed file can add or drop a name a file
/// importing it calls, so every importer is read again, through the whole chain where the
/// visibility is transitive. A file whose import named a file that did not exist yet has no
/// `Imports` edge to follow, and waits for its own next edit. Where a directory is the module, a
/// changed or removed file can add, move or drop a name every file beside it reads, and a call of
/// the module reads what it declares: the files of that directory and their importers are read
/// again.
pub(crate) fn widen(stale: &[String], removed: &[String], graph: &Graph, all_rels: &[String]) -> Vec<String> {
    // A language with a header widens through `headers_move` instead, which re-reads only when a
    // declaration moved.
    let chain = |rel: &str| match Lang::of(rel).map(crate::code::spec::spec) {
        Some(Spec { visibility: Visibility::Imported { transitive }, hooks, .. }) if hooks.header.is_none() => Some(*transitive),
        _ => None,
    };
    fn module(rel: &str) -> Option<(&str, Lang)> {
        let lang = Lang::of(rel).filter(|l| matches!(crate::code::spec::spec(*l).visibility, Visibility::Directory))?;
        Some((rel.rsplit_once('/').map_or("", |(d, _)| d), lang))
    }
    let mut reached: BTreeSet<String> = BTreeSet::new();
    let mut frontier: Vec<(String, bool)> = Vec::new();
    let mut modules: BTreeSet<(&str, Lang)> = BTreeSet::new();
    for rel in stale.iter().chain(removed) {
        if let Some(transitive) = chain(rel) {
            let id = format!("file:{rel}");
            reached.insert(id.clone());
            frontier.push((id, transitive));
        }
        modules.extend(module(rel));
    }
    // Most refreshes touch no file of such a language, and then no edge needs grouping.
    if frontier.is_empty() && modules.is_empty() {
        return Vec::new();
    }
    // Grouped once: a chain of imports is as many hops as it is long, and each hop would otherwise
    // be a scan of every edge in the graph.
    let mut importers: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in graph.edges.iter().filter(|e| e.kind == EdgeKind::Imports) {
        importers.entry(e.target.as_str()).or_default().push(e.source.as_str());
    }
    while let Some((file, transitive)) = frontier.pop() {
        for &importer in importers.get(file.as_str()).into_iter().flatten() {
            if reached.insert(importer.to_string()) && transitive {
                frontier.push((importer.to_string(), transitive));
            }
        }
    }
    if !modules.is_empty() {
        // Once per file: every importer target and every file of the repository is looked up here,
        // and most are of no language a directory reads as one module. A removed file is still a
        // target of the edges its importers held.
        let module_of: HashMap<&str, (&str, Lang)> =
            all_rels.iter().chain(removed).filter_map(|r| module(r).map(|m| (r.as_str(), m))).collect();
        let in_module = |rel: &str| module_of.get(rel).filter(|m| modules.contains(m)).map(|&(_, lang)| lang);
        reached.extend(module_of.iter().filter(|(_, m)| modules.contains(m)).map(|(r, _)| format!("file:{r}")));
        for (target, sources) in &importers {
            let Some(lang) = target.strip_prefix("file:").and_then(in_module) else { continue };
            let same = sources.iter().filter(|s| s.strip_prefix("file:").is_some_and(|s| Lang::of(s) == Some(lang)));
            reached.extend(same.map(|s| s.to_string()));
        }
    }
    let stale: BTreeSet<&str> = stale.iter().map(String::as_str).collect();
    let present: BTreeSet<&str> = all_rels.iter().map(String::as_str).collect();
    reached.iter()
        .filter_map(|id| id.strip_prefix("file:"))
        .filter(|r| !stale.contains(r) && present.contains(r))
        .map(str::to_string)
        .collect()
}

/// The header a name-indexed family records when it has no `header` hook: its namespaces, its
/// visible top-level names, and what its containers hold.
pub fn header(facts: &Facts) -> Header {
    let mut h = Header { scope: facts.namespaces.clone(), ..Header::default() };
    for d in &facts.defs {
        match d.parent {
            None if d.export => {
                h.top.insert(d.qualified.clone());
            }
            None => {}
            Some(_) if d.kind.join().is_some() => {
                h.nested.types.insert(d.qualified.clone());
            }
            Some(_) => {
                h.nested.members.insert(d.qualified.clone());
            }
        }
        if d.parent.is_none() && d.kind.join().is_some() {
            h.nested.types.insert(d.qualified.clone());
        }
    }
    h
}

/// `header` of what `source` declares, for a family that keeps it in `headers.json`; empty when the
/// grammar reads nothing.
pub(crate) fn default_header(lang: Lang, rel: &str, source: &str) -> Header {
    declarations(lang, rel, source).map(|facts| header(&facts)).unwrap_or_default()
}

/// `decl` facts alone, for a file another file reads across through `Resolver::facts`.
pub(crate) fn declarations(lang: Lang, rel: &str, source: &str) -> Option<Arc<Facts>> {
    let spec = crate::code::spec::spec(lang);
    let decl = queries::decl(lang)?;
    let tree = lang.parse(source.as_bytes())?;
    let file = File { lang, rel, text: source, src: source, tree: Some(&tree) };
    Some(declared(spec, decl, &file))
}

/// The engine's whole read of a file whose node the front has written.
pub(crate) fn extract(resolver: &Resolver, spec: &Spec, file: &File, ex: &mut Extraction) {
    let Some(decl) = queries::decl(file.lang) else {
        // A language with no queries of its own reads its view through its hook alone.
        if let Some(hook) = spec.hooks.resolve {
            hook(resolver, file, &Facts::default(), &mut Sink(ex));
        }
        return;
    };
    let facts = extracted(spec, decl, queries::refs(file.lang), file);
    write(file, &facts, ex);
    let kind = |i: &Import| if i.reexport { EdgeKind::ReExports } else { EdgeKind::Imports };
    for i in &facts.imports {
        for t in i.targets(resolver, spec, file.rel) {
            ex.edge(&format!("file:{}", file.rel), &format!("file:{t}"), kind(i), "*", file.rel);
        }
    }
    resolve(resolver, spec, file, &facts, ex);
    if let Some(hook) = spec.hooks.resolve {
        hook(resolver, file, &facts, &mut Sink(ex));
    }
}
