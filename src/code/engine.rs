//! The shared reader: a language's two queries run over the tree the front parsed, their captures
//! turned into `Facts`, and the facts written as nodes and edges by the language's `Spec`. Hooks read
//! the same facts instead of walking the tree a second time.
//!
//! No language reads through it yet: each keeps its `legacy` reader until its port, so until the
//! first one the engine is reached only by its tests and by a spec that names queries.
#![allow(dead_code)]

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::ops::Range;
use std::sync::Arc;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, QueryCursor, Tree};

use crate::code::imports::Resolver;
use crate::code::index::Header;
use crate::code::lang::Lang;
use crate::code::queries::{self, Compiled, Kind, Meta, Role};
use crate::code::spec::{Ambiguity, Body, Cap, Doc, Export, Form, Line, Spec, Visibility};
use crate::code::syntax::{self, comment_text, last_row, text_span, DOC_CHARS};
use crate::model::{EdgeKind, Extraction, Graph, NodeKind};

/// One file as the front read it.
pub(crate) struct File<'a> {
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
pub(crate) struct Def {
    pub id: String,
    pub qualified: String,
    pub name: String,
    pub kind: Kind,
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
pub(crate) struct Import {
    pub path: Option<String>,
    pub names: Vec<String>,
    pub alias: Option<String>,
    pub reexport: bool,
    pub range: Range<usize>,
    /// The files the site resolves to, by the spec's import rule, resolved once where the facts are
    /// read so that no reader resolves a site again.
    pub targets: Vec<String>,
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
pub(crate) struct Locals(HashMap<String, Vec<Range<usize>>>);

impl Locals {
    /// Whether a local binding of `name` encloses `byte`.
    pub(crate) fn shadows(&self, name: &str, byte: usize) -> bool {
        self.0.get(name).is_some_and(|scopes| scopes.iter().any(|scope| scope.contains(&byte)))
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Facts {
    pub defs: Vec<Def>,
    pub refs: Vec<Ref>,
    pub imports: Vec<Import>,
    pub cites: Vec<Cite>,
    pub locals: Locals,
    /// Package and namespace lines, in file order.
    pub namespaces: Vec<String>,
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

    /// The innermost definition whose node holds `byte`.
    pub(crate) fn owner(&self, byte: usize) -> Option<usize> {
        let defs = self.defs.iter().enumerate().map(|(i, d)| (&d.range, i));
        defs.chain(self.repeats.iter().map(|(range, i)| (range, *i)))
            .filter(|(range, _)| range.contains(&byte))
            .min_by_key(|(range, _)| range.len())
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
    Namespace,
}

fn text<'a>(n: Node, src: &'a str) -> &'a str {
    syntax::text(n, src.as_bytes())
}

fn matches_of<'t>(c: &'static Compiled, root: Node<'t>, src: &str, rel: &str, mut each: impl FnMut(usize, Vec<(Role, Node<'t>)>)) {
    let mut cursor = QueryCursor::new();
    cursor.set_match_limit(MATCH_LIMIT);
    let mut it = cursor.matches(&c.query, root, src.as_bytes());
    while let Some(m) = it.next() {
        each(m.pattern_index, m.captures().iter().map(|cap| (c.roles[cap.index as usize], cap.node)).collect());
    }
    if cursor.did_exceed_match_limit() {
        eprintln!("code: {rel} reached the query match limit; it keeps what matched before it");
    }
}

/// The facts `decl` and, when given, `refs` read from `file`, each import site with its targets.
pub(crate) fn facts(resolver: &Resolver, spec: &Spec, decl: &'static Compiled, refs: Option<&'static Compiled>, file: &File) -> Facts {
    let mut facts = Facts::default();
    let Some(tree) = file.tree else { return facts };
    let (root, src) = (tree.root_node(), file.src);
    let mut declared = Vec::new();
    matches_of(decl, root, src, file.rel, |pattern, caps| {
        read_declaration(&caps, pattern, src, &mut declared);
        if let Some(mut import) = read_import(&caps, &decl.metas[pattern], src) {
            import.targets = targets(resolver, spec, file.rel, &import);
            facts.imports.push(import);
        }
    });
    qualify(spec, decl, file, declared, &mut facts);
    facts.declared = facts.defs.iter().map(|d| d.qualified.clone()).collect();
    facts.exported = facts.defs.iter().filter(|d| d.export).map(|d| d.qualified.clone()).collect();
    let ranges = facts.defs.iter().enumerate().map(|(i, d)| (&d.range, i)).chain(facts.repeats.iter().map(|(r, i)| (r, *i)));
    for (r, i) in ranges {
        facts.at.entry((r.start, r.end)).or_insert(i);
    }
    if let Some(refs) = refs {
        let (mut scopes, mut binder_scopes, mut bound) = (Vec::new(), Vec::new(), Vec::new());
        matches_of(refs, root, src, file.rel, |pattern, caps| {
            // A definition whose own pattern captures a scope binds there and nowhere else: a loop
            // variable covers the loop's body, never the iterable it sits beside.
            let binds = caps.iter().any(|(r, _)| *r == Role::LocalDefinition);
            let own: Vec<Range<usize>> = caps.iter().filter(|(r, _)| *r == Role::LocalScope).map(|(_, n)| n.byte_range()).collect();
            for (role, node) in &caps {
                match role {
                    Role::Reference(edge) => facts.refs.push(read_reference(spec, &caps, *edge, *node, &refs.metas[pattern], src)),
                    Role::LocalScope if binds => binder_scopes.push(node.byte_range()),
                    Role::LocalScope => scopes.push(node.byte_range()),
                    Role::LocalDefinition => bound.push((node.start_byte(), text(*node, src).to_string(), own.clone())),
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
        // A plain definition binds in the innermost plain scope holding it. A scope that only a
        // binder's pattern captures serves it when no plain scope does: a comprehension's walrus
        // binds in the function around it, while a body local still stays in a function whose
        // pattern captures it together with the parameters, instead of shadowing the whole file.
        let innermost = |at: usize, among: &[Range<usize>]| among.iter().filter(|s| s.contains(&at)).min_by_key(|s| s.len()).cloned();
        for (at, name, own) in bound {
            let scopes = match own.is_empty() {
                true => vec![innermost(at, &scopes).or_else(|| innermost(at, &binder_scopes)).unwrap_or(0..src.len() + 1)],
                false => own,
            };
            facts.locals.0.entry(name).or_default().extend(scopes);
        }
        let owners: Vec<_> = facts.refs.iter().map(|r| facts.owner(r.range.start)).collect();
        facts.refs.iter_mut().zip(owners).for_each(|(r, o)| r.source = o);
        let owners: Vec<_> = facts.cites.iter().map(|c| facts.owner(c.range.start)).collect();
        facts.cites.iter_mut().zip(owners).for_each(|(c, o)| c.source = o);
    }
    facts
}

fn read_declaration<'t>(caps: &[(Role, Node<'t>)], pattern: usize, src: &str, out: &mut Vec<Declared<'t>>) {
    let declaring = caps.iter().find_map(|(role, node)| match role {
        Role::Definition(kind) => Some((Declaring::Def(*kind), *node)),
        Role::ScopeOwner => Some((Declaring::Owner, *node)),
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
        targets: Vec::new(),
    })
}

fn read_reference(spec: &Spec, caps: &[(Role, Node)], edge: EdgeKind, site: Node, meta: &Meta, src: &str) -> Ref {
    let name_node = caps.iter().find(|(r, _)| *r == Role::Name).map_or(site, |(_, n)| *n);
    let name = text(name_node, src);
    Ref {
        source: None,
        name: spec.hooks.fold.map_or_else(|| name.to_string(), |fold| fold(name).into_owned()),
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
}

/// Parents, qualified names, ids, export and bodies, in source order. The first pattern to match a
/// node in a role wins, so a query puts specific patterns before general ones.
fn qualify(spec: &Spec, decl: &Compiled, file: &File, mut declared: Vec<Declared>, facts: &mut Facts) {
    let src = file.src;
    declared.sort_by_key(|d| (d.node.start_byte(), std::cmp::Reverse(d.node.end_byte()), d.pattern));
    // One pattern matching one node once per name, as `let a = 1, b = 2` does, declares each name.
    declared.dedup_by(|b, a| {
        a.node.id() == b.node.id()
            && (a.role == b.role || matches!((a.role, b.role), (Declaring::Def(_), Declaring::Def(_))))
            && (a.pattern != b.pattern || a.names == b.names)
    });
    let fold = |s: &str| spec.hooks.fold.map_or_else(|| s.to_string(), |f| f(s).into_owned());
    let mut stack: Vec<Open> = Vec::new();
    let mut ids: HashMap<String, usize> = HashMap::new();
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
        let leaf = format!("{}{name}", meta.prefix);
        let qualified = base.map_or_else(|| leaf.clone(), |(b, j)| format!("{b}{j}{leaf}"));
        let Declaring::Def(kind) = d.role else {
            stack.push(Open { end: d.node.end_byte(), qualified, join: '.', def: None });
            continue;
        };
        let id = format!("sym:{}::{qualified}", file.rel);
        let export = meta.export.unwrap_or_else(|| exported(spec.export, &d.modifiers, &name));
        if let Some(&first) = ids.get(&id) {
            facts.repeats.push((d.node.byte_range(), first));
            facts.redeclared.push((parent.and_then(|p| p.def), first, export));
            if let Some(join) = kind.join() {
                stack.push(Open { end: d.node.end_byte(), qualified, join, def: Some(first) });
            }
            continue;
        }
        ids.insert(id.clone(), facts.defs.len());
        let (line, end) = text_span(d.node);
        let def = Def {
            id,
            qualified: qualified.clone(),
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
        let doc = doc_lines(spec.body, d.node, d.doc, src);
        let def = Def { body: if doc.is_empty() { signature } else { format!("{doc}\n{signature}") }, ..def };
        facts.defs.push(def);
        if let Some(join) = kind.join() {
            stack.push(Open { end: d.node.end_byte(), qualified, join, def: Some(facts.defs.len() - 1) });
        }
    }
}

fn exported(rule: Export, modifiers: &[String], name: &str) -> bool {
    match rule {
        Export::Show(words) => modifiers.iter().any(|m| words.contains(&m.as_str())),
        Export::Hide(words) => !modifiers.iter().any(|m| words.contains(&m.as_str())),
        Export::Prefix(c) => !name.starts_with(c),
        Export::All => true,
    }
}

fn signature_line(line: Line, def: Node, name: Node, src: &str) -> String {
    match line {
        Line::First => text(def, src).lines().next().unwrap_or("").to_string(),
        Line::Name => {
            let start = src[..name.start_byte()].rfind('\n').map_or(0, |i| i + 1);
            let end = src[name.start_byte()..].find('\n').map_or(src.len(), |i| name.start_byte() + i);
            src[start..end].trim().to_string()
        }
    }
}

/// The named sibling before `n`. HCL's grammar puts the comments above a file's first block outside its
/// `body`, so a first child of a `body` looks past it.
fn above(n: Node) -> Option<Node> {
    n.prev_named_sibling().or_else(|| n.parent().filter(|p| p.kind() == "body").and_then(|p| p.prev_named_sibling()))
}

/// `comment_text`, and `#` markers too: `comment_text` serves the C family alone.
fn stripped(raw: &str) -> String {
    comment_text(raw).lines().map(|l| l.trim_start_matches('#').trim()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

fn doc_lines(body: Body, def: Node, captured: Option<Node>, src: &str) -> String {
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
                if !kinds.contains(&p.kind()) || text(p, src).starts_with("#!") || last_row(p) + 1 != at.start_position().row || trails(p) {
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
        Form::Stripped => parts.iter().map(|p| stripped(p)).filter(|p| !p.is_empty()).collect::<Vec<_>>().join("\n"),
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
pub(crate) fn write(file: &File, facts: &Facts, ex: &mut Extraction) {
    let rel = file.rel;
    for d in &facts.defs {
        ex.node_span(NodeKind::Symbol, &d.id, &d.qualified, &d.body, rel, (d.line, d.end));
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
/// directory, then to the root when the spec says so, each with the spec's suffixes, kept only when
/// the walk globbed the result. A path naming a directory names every file of the importer's
/// language directly in it, as a Terraform module source does. The importer's own file and own
/// directory add nothing it does not already hold, so they name nothing.
pub(crate) fn import_targets(resolver: &Resolver, spec: &Spec, rel: &str, import: &Import) -> Vec<String> {
    let Some(path) = &import.path else { return Vec::new() };
    let dir = rel.rsplit_once('/').map_or("", |(d, _)| d);
    let bases = std::iter::once(dir).chain(spec.imports.root.then_some(""));
    for base in bases {
        for suffix in spec.imports.suffixes {
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

/// The files an import site names: the spec's `import` hook where it has one, else the default.
fn targets(resolver: &Resolver, spec: &Spec, rel: &str, import: &Import) -> Vec<String> {
    match spec.hooks.import {
        Some(hook) => hook(resolver, rel, import),
        None => import_targets(resolver, spec, rel, import),
    }
}

/// The files a reference may resolve into besides its own, by the spec's visibility, with what each
/// declares. Computed once per file: it does not depend on the name, and `Family` has none.
fn reach(resolver: &Resolver, spec: &Spec, file: &File, facts: &Facts) -> Vec<(String, Arc<Facts>)> {
    let rel = file.rel;
    let files: Vec<String> = match spec.visibility {
        Visibility::File | Visibility::Family => Vec::new(),
        Visibility::Imported { transitive } => {
            let mut seen = BTreeSet::from([rel.to_string()]);
            let mut queue: VecDeque<String> = facts.imports.iter().flat_map(|i| i.targets.iter().cloned()).collect();
            while let Some(next) = queue.pop_front() {
                if !seen.insert(next.clone()) {
                    continue;
                }
                if transitive {
                    if let Some(f) = resolver.facts(&next) {
                        queue.extend(f.imports.iter().flat_map(|i| i.targets.iter().cloned()));
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
    reach.iter().filter(|(_, facts)| facts.declares_visible(name)).map(|(f, _)| f.clone()).collect()
}

/// Every reference with no receiver and of a kind no hook owns, resolved by name.
pub(crate) fn resolve(resolver: &Resolver, spec: &Spec, file: &File, facts: &Facts, ex: &mut Extraction) {
    let rel = file.rel;
    let reach = reach(resolver, spec, file, facts);
    for r in &facts.refs {
        if r.receiver.is_some() || spec.hooks.owns.contains(&r.edge) || facts.locals.shadows(&r.name, r.range.start) {
            continue;
        }
        let own = facts.declares(&r.name);
        let others = visible(resolver, spec, file, &reach, &r.name);
        let targets: Vec<&str> = match spec.ambiguity {
            Ambiguity::OwnFirst if own => vec![rel],
            Ambiguity::OwnFirst => others.iter().map(String::as_str).collect(),
            Ambiguity::Unique => match (own, others.as_slice()) {
                (true, []) => vec![rel],
                (false, [one]) => vec![one.as_str()],
                _ => Vec::new(),
            },
            Ambiguity::All => own.then_some(rel).into_iter().chain(others.iter().map(String::as_str)).collect(),
        };
        let from = facts.source_id(rel, r.source);
        let container = r.source.and_then(|i| facts.defs[i].parent).map(|p| facts.defs[p].id.clone());
        for target in targets {
            let to = format!("sym:{target}::{}", r.name);
            if to != from && Some(&to) != container.as_ref() {
                ex.edge(&from, &to, r.edge, &r.context, rel);
            }
        }
    }
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
pub(crate) fn header(facts: &Facts) -> Header {
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

/// `decl` facts alone, for a file another file reads across through `Resolver::facts`.
pub(crate) fn declarations(resolver: &Resolver, lang: Lang, rel: &str, source: &str) -> Option<Facts> {
    let spec = crate::code::spec::spec(lang);
    let decl = queries::decl(lang)?;
    let tree = lang.parse(source.as_bytes())?;
    let file = File { lang, rel, text: source, src: source, tree: Some(&tree) };
    Some(facts(resolver, spec, decl, None, &file))
}

/// The engine's whole read of a file whose node the front has written.
pub(crate) fn extract(resolver: &Resolver, spec: &Spec, file: &File, ex: &mut Extraction) {
    let Some(decl) = queries::decl(file.lang) else { return };
    let facts = facts(resolver, spec, decl, queries::refs(file.lang), file);
    write(file, &facts, ex);
    let kind = |i: &Import| if i.reexport { EdgeKind::ReExports } else { EdgeKind::Imports };
    for i in &facts.imports {
        for t in &i.targets {
            ex.edge(&format!("file:{}", file.rel), &format!("file:{t}"), kind(i), "*", file.rel);
        }
    }
    resolve(resolver, spec, file, &facts, ex);
    if let Some(hook) = spec.hooks.resolve {
        hook(resolver, file, &facts, &mut Sink(ex));
    }
}

#[cfg(test)]
mod tests;
