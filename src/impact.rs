//! Walks over the code edges: who reaches a symbol, what a symbol reaches, and a path between
//! two. Callers import through barrels, so a caller's edge points at `sym:<barrel>::Name`,
//! never at the declaration; every walk therefore treats a symbol and its barrel aliases as one.
use crate::model::{Edge, EdgeKind, Graph};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Dependent { pub id: String, pub depth: usize, pub kind: EdgeKind, pub via: String, pub passed: bool }

/// How an edge reads to a person: an argument edge is `Passes`, since nothing proves it is called.
pub(crate) fn label(kind: EdgeKind, passed: bool) -> String { if passed { "Passes".to_string() } else { format!("{kind:?}") } }

/// True when `candidate` is the row to keep for a dependent over `existing`: a shallower depth
/// always wins; at the same depth, a call beats an argument-only edge. Shared by the walk's own
/// per-layer merge, where both are always at the same depth, and `changes::report`'s merge across
/// every root's walk, where they can differ.
pub(crate) fn beats(candidate: &Dependent, existing: &Dependent) -> bool {
    candidate.depth < existing.depth || (candidate.depth == existing.depth && existing.passed && !candidate.passed)
}

#[derive(Debug, Default)]
pub struct Impact { pub root: String, pub layers: Vec<Vec<Dependent>>, pub importers: Vec<String> }

/// A file path never holds `::`, but a name can (a Shell `log::info`), so the file ends at the first.
fn name_of(id: &str) -> &str { id.split_once("::").map_or(id, |(_, name)| name) }

/// `S.create` is exported as `S`; a barrel names the class, not the member.
fn bare(name: &str) -> &str { name.split('.').next().unwrap_or(name) }

/// The `(original, exported)` pairs an import or re-export names: `export { a as b }` is written
/// `a as b`, every other entry names one thing under its own name.
fn entries(e: &Edge) -> impl Iterator<Item = (&str, &str)> {
    e.context.split(',').map(|c| c.split_once(" as ").unwrap_or((c, c)))
}

fn exports(e: &Edge, bare: &str) -> bool { e.context == "*" || entries(e).any(|(_, exported)| exported == bare) }

/// The names a re-export gives `bare` in the file that re-exports it: itself through `*`, its
/// alias through a rename, none when the edge does not carry it.
fn renamed_to(e: &Edge, bare: &str) -> Vec<String> {
    if e.context == "*" { return vec![bare.to_string()] }
    entries(e).filter(|(orig, _)| *orig == bare).map(|(_, exported)| exported.to_string()).collect()
}

/// `renamed_to` read backwards: the names in the re-exported file that a barrel's `bare` stands for.
fn renamed_from(e: &Edge, bare: &str) -> Vec<String> {
    if e.context == "*" { return vec![bare.to_string()] }
    entries(e).filter(|(_, exported)| *exported == bare).map(|(orig, _)| orig.to_string()).collect()
}

/// `name` with its first segment replaced: `S.create` under `T` is `T.create`.
fn rebase(name: &str, head: &str) -> String { format!("{head}{}", &name[bare(name).len()..]) }

/// The declarations a renamed re-export stands for, for a name no node carries:
/// `export { contrast as contrastRatio }` makes `contrastRatio` name `contrast`. An alias typed
/// as its id, `sym:<barrel>::contrastRatio`, is followed from that barrel alone.
pub(crate) fn renamed(graph: &Graph, name: &str) -> Vec<String> {
    if name.starts_with("sym:") { return exact(graph, name).into_iter().collect() }
    let mut out: BTreeSet<String> = BTreeSet::new();
    for e in graph.edges.iter().filter(|e| e.kind == EdgeKind::ReExports) {
        if entries(e).any(|(orig, exported)| orig != exported && exported == bare(name)) {
            let file = e.source.trim_start_matches("file:");
            out.extend(exact(graph, &format!("sym:{file}::{name}")));
        }
    }
    out.into_iter().collect()
}

/// The node a possibly-dangling `sym:<barrel>::<Name>` stands for, following re-exports forward.
/// A member no node declares — `parse` on a zod schema, a literal's shorthand property —
/// stands for its container, which has a `path:line` where the member has none.
pub fn canonical(graph: &Graph, id: &str) -> Option<String> {
    exact(graph, id).or_else(|| container(id).and_then(|c| canonical(graph, &c)))
}

/// `canonical` without the member fold: the node itself or the declaration behind a barrel.
fn exact(graph: &Graph, id: &str) -> Option<String> {
    exact_via(graph, id, |source| graph.edges.iter().filter(|e| e.kind == EdgeKind::ReExports && e.source == source).collect())
}

/// `exact` with the re-exports of a file supplied by `from`: a scan of every edge for a caller
/// that asks once, an `Index`'s grouped map for a walk that asks at every hop.
fn exact_via<'g>(graph: &Graph, id: &str, from: impl Fn(&str) -> Vec<&'g Edge>) -> Option<String> {
    if graph.nodes.contains_key(id) { return Some(id.to_string()) }
    let (file, name) = id.strip_prefix("sym:")?.split_once("::")?;
    // A barrel that renames hands the walk on under the original name, so a step is a file and
    // the name the symbol has there.
    let mut at = vec![(file.to_string(), name.to_string())];
    let mut seen: BTreeSet<(String, String)> = at.iter().cloned().collect();
    let mut i = 0;
    while i < at.len() {
        let (source, name) = (format!("file:{}", at[i].0), at[i].1.clone());
        for e in from(&source) {
            for orig in renamed_from(e, bare(&name)) {
                let next = (e.target.trim_start_matches("file:").to_string(), rebase(&name, &orig));
                let candidate = format!("sym:{}::{}", next.0, next.1);
                if graph.nodes.contains_key(&candidate) { return Some(candidate) }
                if seen.insert(next.clone()) { at.push(next) }
            }
        }
        i += 1;
    }
    None
}

/// `sym:f::C` for `sym:f::C.m`, and `sym:f::O.I` for `sym:f::O.I.m`; none for a class or a file.
/// A `/` inside a name joins one language's own address (`app/clients`), so only `.` splits.
fn container(id: &str) -> Option<String> {
    let (file, name) = id.strip_prefix("sym:")?.split_once("::")?;
    let (class, _) = name.rsplit_once('.')?;
    Some(format!("sym:{file}::{class}"))
}

/// Whether a change to an edge's target reaches its source along it. A `References` edge counts
/// only when it points at a symbol: until 0.6.0 every one pointed at a requirement id, an entity or
/// a file, which `ask` expands and no blast walk follows, and the new languages write them between
/// declarations — a migration altering a table, a field naming a type (spec L11). An `Implements`
/// edge from a document — a task, an invariant's test — points at a requirement or a file, and
/// only a class's points at a symbol.
pub(crate) fn walks(e: &Edge) -> bool {
    match e.kind {
        EdgeKind::Calls | EdgeKind::Extends => true,
        EdgeKind::References | EdgeKind::Implements => e.target.starts_with("sym:"),
        _ => false,
    }
}

/// The edges every walk looks up, grouped once by the end they are looked up from: code edges by
/// target (upstream) or source (downstream), members by class, re-exports and imports by the file
/// they name. `changes` walks one root per changed symbol, and scanning the whole edge set again
/// for each root took a minute on a diff of 1,140 symbols.
pub struct Index<'a> {
    graph: &'a Graph,
    up: bool,
    code: BTreeMap<&'a str, Vec<&'a Edge>>,
    members: BTreeMap<&'a str, Vec<&'a str>>,
    re_exports: BTreeMap<&'a str, Vec<&'a Edge>>,
    /// The same re-exports by the barrel that makes them, which is the end `exact` follows.
    re_exports_from: BTreeMap<&'a str, Vec<&'a Edge>>,
    imports: BTreeMap<&'a str, Vec<&'a Edge>>,
    /// Class to the interfaces it implements, and back, by the interface's declaration: an
    /// `implements` clause names the interface by the file it was imported from, often a barrel.
    interfaces: BTreeMap<&'a str, Vec<String>>,
    implementers: BTreeMap<String, Vec<&'a str>>,
}

impl<'a> Index<'a> {
    pub fn new(graph: &'a Graph, up: bool) -> Index<'a> { Self::build(graph, up, true) }

    /// Only what `seeds` reads, for a caller that looks a name up once and
    /// never walks: `explain` builds one per call.
    pub(crate) fn names(graph: &'a Graph) -> Index<'a> { Self::build(graph, true, false) }

    fn build(graph: &'a Graph, up: bool, full: bool) -> Index<'a> {
        let mut ix = Index {
            graph, up, code: BTreeMap::new(), members: BTreeMap::new(), re_exports: BTreeMap::new(), re_exports_from: BTreeMap::new(), imports: BTreeMap::new(),
            interfaces: BTreeMap::new(), implementers: BTreeMap::new(),
        };
        let mut implements: Vec<&Edge> = Vec::new();
        for e in &graph.edges {
            if e.kind == EdgeKind::Implements && e.target.starts_with("sym:") { implements.push(e); }
            match e.kind {
                _ if walks(e) => ix.code.entry(if up { e.target.as_str() } else { e.source.as_str() }).or_default().push(e),
                EdgeKind::Declares if e.target.starts_with("sym:") => ix.members.entry(e.source.as_str()).or_default().push(e.target.as_str()),
                EdgeKind::ReExports => {
                    ix.re_exports.entry(e.target.as_str()).or_default().push(e);
                    ix.re_exports_from.entry(e.source.as_str()).or_default().push(e);
                }
                EdgeKind::Imports if full => ix.imports.entry(e.target.as_str()).or_default().push(e),
                _ => {}
            }
        }
        // Only a barrel can stand for an interface no node is: an `implements OnModuleInit` from
        // a package names a file that re-exports nothing, and `exact` would scan every edge to
        // learn that, once per class.
        let barrels: BTreeSet<&str> = ix.re_exports.values().flatten().map(|e| e.source.trim_start_matches("file:")).collect();
        for e in implements {
            let declared = graph.nodes.contains_key(&e.target);
            let in_barrel = || e.target.strip_prefix("sym:").and_then(|t| t.split_once("::")).is_some_and(|(f, _)| barrels.contains(f));
            let Some(iface) = (declared || in_barrel()).then(|| ix.exact(&e.target)).flatten() else { continue };
            ix.interfaces.entry(e.source.as_str()).or_default().push(iface.clone());
            ix.implementers.entry(iface).or_default().push(e.source.as_str());
        }
        ix
    }

    /// `exact` over the grouped re-exports, so a hop reads one barrel's edges and not the graph's.
    fn exact(&self, id: &str) -> Option<String> {
        exact_via(self.graph, id, |source| self.re_exports_from.get(source).cloned().unwrap_or_default())
    }

    /// Where a downward step lands, and whether the walk may go on from there. A target folded into
    /// its container is shown as it, but the container's own calls are its other methods' calls,
    /// which this one does not make — walking on would report paths that only a sibling has.
    fn landing(&self, target: &str) -> (String, bool) {
        match self.exact(target) {
            Some(id) => (id, true),
            None => (canonical(self.graph, target).unwrap_or_else(|| target.to_string()), false),
        }
    }

    /// Every `sym:<barrel>::<Name>` a caller could have reached this symbol by.
    pub(crate) fn aliases(&self, id: &str) -> Vec<String> {
        let Some(n) = self.graph.nodes.get(id) else { return Vec::new() };
        // `exact` in reverse. A local `export { a as b }` is the file re-exporting itself, so
        // that alias comes out in the declaring file.
        let mut at = vec![(n.file.clone(), name_of(id).to_string())];
        let mut seen: BTreeSet<(String, String)> = at.iter().cloned().collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < at.len() {
            let (target, name) = (format!("file:{}", at[i].0), at[i].1.clone());
            for e in self.re_exports.get(target.as_str()).into_iter().flatten() {
                for exported in renamed_to(e, bare(&name)) {
                    let next = (e.source.trim_start_matches("file:").to_string(), rebase(&name, &exported));
                    if seen.insert(next.clone()) {
                        out.push(format!("sym:{}::{}", next.0, next.1));
                        at.push(next);
                    }
                }
            }
            i += 1;
        }
        out
    }

    /// Every symbol a type declares, at any depth: a nested type's members are the outer type's
    /// too, so a change to `Outer` reaches a caller of `Outer.Inner.go` (spec L4). Breadth-first,
    /// so a type's own members come first, in the order a one-level walk listed them.
    fn members(&self, id: &str) -> Vec<&'a str> {
        let mut out: Vec<&'a str> = Vec::new();
        let mut seen: BTreeSet<&str> = BTreeSet::from([id]);
        let mut owner = id;
        let mut at = 0;
        loop {
            for &m in self.members.get(owner).into_iter().flatten() {
                if seen.insert(m) { out.push(m); }
            }
            let Some(&next) = out.get(at) else { break };
            owner = next;
            at += 1;
        }
        out
    }

    /// The root, its members (a class is changed through them) and every alias of each.
    pub(crate) fn seeds(&self, root: &str) -> Vec<String> {
        let mut out = vec![root.to_string()];
        out.extend(self.members(root).iter().map(|m| m.to_string()));
        let aliased: Vec<String> = out.iter().flat_map(|s| self.aliases(s)).collect();
        out.extend(aliased);
        out
    }

    /// The interface members `C.m` stands behind: `I.m` for every `I` that `C` implements and
    /// that declares `m`.
    fn contracts(&self, at: &str) -> Vec<String> {
        let Some(class) = container(at) else { return Vec::new() };
        let name = &at[class.len() + 1..];
        self.interfaces.get(class.as_str()).into_iter().flatten()
            .map(|i| format!("{i}.{name}"))
            .filter(|m| self.graph.nodes.contains_key(m))
            .collect()
    }

    /// The methods a call on `I.m` runs: `C.m` for every class `C` that implements `I` and
    /// declares `m` itself.
    fn implementations(&self, id: &str) -> Vec<String> {
        let Some(iface) = container(id) else { return Vec::new() };
        let name = &id[iface.len() + 1..];
        self.implementers.get(&iface).into_iter().flatten()
            .map(|c| format!("{c}.{name}"))
            .filter(|m| self.graph.nodes.contains_key(m))
            .collect()
    }

    /// Every id a caller of the root reaches it by through an interface: the members of the
    /// interfaces the root's class implements, and each one's barrel aliases.
    pub(crate) fn dispatched(&self, root: &str) -> Vec<String> {
        self.seeds(root).iter().flat_map(|s| self.contracts(s))
            .flat_map(|i| { let aliases = self.aliases(&i); std::iter::once(i).chain(aliases) })
            .collect()
    }

    /// The edges to follow from `at`. Upstream, a member is also reached through its class by a
    /// subclass — `extends C` inherits `C.m` — and by an implemented interface's change, so the
    /// class's `Extends` and `Implements` edges count for the member without the class itself
    /// being listed; and a call through an interface `C` implements is a call on `C.m`.
    /// Downstream, a class reaches what its members call.
    fn step(&self, at: &str) -> Vec<&'a Edge> {
        let mut out: Vec<&Edge> = self.code.get(at).into_iter().flatten().copied().collect();
        if self.up {
            if let Some(c) = container(at) {
                out.extend(self.code.get(c.as_str()).into_iter().flatten().copied().filter(|e| matches!(e.kind, EdgeKind::Extends | EdgeKind::Implements)));
            }
            for i in self.contracts(at) {
                let aliases = self.aliases(&i);
                for via in std::iter::once(i).chain(aliases) {
                    out.extend(self.code.get(via.as_str()).into_iter().flatten().copied());
                }
            }
        } else {
            for m in self.members(at) { out.extend(self.code.get(m).into_iter().flatten().copied()); }
        }
        // A call before an argument edge, so a node reached both ways is reached by the call.
        out.sort_by_key(|e| e.passes());
        out
    }

    /// Upstream only: each target `X.m` that no node declares, for a seed `X` — `KEYS.filter`,
    /// `loginSchema.parse` — mapped to the seed it belongs to; a barrel's alias of a member is a
    /// seed of its own, not one of these. `--down` and `trace` land such a
    /// target on `X` and stop there, so a caller of it is a caller of `X` at the first layer and
    /// at no other; a longer seed (`S.m` over `S`) claims `S.m.bind`.
    pub(crate) fn undeclared(&self, seeds: &[String]) -> BTreeMap<String, String> {
        let mut out: BTreeMap<String, String> = BTreeMap::new();
        for seed in seeds {
            let prefix = format!("{seed}.");
            let range = self.code.range::<str, _>((std::ops::Bound::Included(prefix.as_str()), std::ops::Bound::Unbounded));
            for (target, _) in range.take_while(|(t, _)| t.starts_with(&prefix)).filter(|(t, _)| !self.graph.nodes.contains_key(**t) && !seeds.iter().any(|s| s == *t)) {
                let owner = out.entry(target.to_string()).or_insert_with(|| seed.clone());
                if seed.len() > owner.len() { *owner = seed.clone(); }
            }
        }
        out
    }

    /// Where an edge followed from its walked end arrives, and whether the walk may go on from
    /// there. Upstream that is the edge's source. Downstream, a call on an interface member lands
    /// on it and on each implementation, since the graph cannot tell which one runs.
    fn lands(&self, e: &Edge) -> Vec<(String, bool)> {
        if self.up { return vec![(e.source.clone(), true)] }
        let (id, goes_on) = self.landing(&e.target);
        let dispatch = if goes_on { self.implementations(&id) } else { Vec::new() };
        std::iter::once((id, goes_on)).chain(dispatch.into_iter().map(|m| (m, true))).collect()
    }

    fn walk(&self, root: &str, depth: usize) -> Vec<Vec<Dependent>> {
        let up = self.up;
        let mut start = self.seeds(root);
        // A row reached through an undeclared member names the symbol it belongs to as `via`, as
        // `canonical` names it in a row.
        let undeclared = if up { self.undeclared(&start) } else { BTreeMap::new() };
        start.extend(undeclared.keys().cloned());
        let mut seen: BTreeSet<String> = start.iter().cloned().collect();
        let mut frontier = start;
        let mut layers = Vec::new();
        for d in 1..=depth {
            let mut next: BTreeMap<String, Dependent> = BTreeMap::new();
            let mut open: BTreeSet<String> = BTreeSet::new();
            for at in &frontier {
                let via = undeclared.get(at).unwrap_or(at);
                for (e, (other, goes_on)) in self.step(at).into_iter().flat_map(|e| self.lands(e).into_iter().map(move |l| (e, l))) {
                    if seen.contains(&other) { continue }
                    if goes_on { open.insert(other.clone()); }
                    let candidate = Dependent { id: other.clone(), depth: d, kind: e.kind, via: via.clone(), passed: e.passes() };
                    // A call beats an argument edge whichever owner in the layer came first.
                    if next.get(&other).is_some_and(|x| !beats(&candidate, x)) { continue }
                    next.insert(other, candidate);
                }
            }
            if next.is_empty() { break }
            seen.extend(next.keys().cloned());
            let next: Vec<Dependent> = next.into_values().collect();
            frontier = next.iter().map(|x| x.id.clone()).filter(|id| open.contains(id)).collect();
            layers.push(next);
        }
        layers
    }

    /// Files that name the symbol without necessarily calling it: every importer of its name from
    /// its file or any barrel, and the barrels themselves — a barrel that re-exports the symbol
    /// names it as surely as an importer does, and `export { AuthService } from './auth.service.js'`
    /// breaks before any caller when the class is renamed. It was the one file `impact AuthService`
    /// left out on the bench corpus (9 of 10, 2026-09-03).
    fn importers(&self, root: &str) -> Vec<String> {
        let Some(n) = self.graph.nodes.get(root) else { return Vec::new() };
        // Each file with the name the symbol is imported by from it: a rename exports it under
        // another.
        let mut names: BTreeSet<(String, String)> = BTreeSet::from([(format!("file:{}", n.file), bare(name_of(root)).to_string())]);
        let mut out: BTreeSet<String> = BTreeSet::new();
        for a in self.aliases(root) {
            if let Some((f, name)) = a.trim_start_matches("sym:").split_once("::") {
                names.insert((format!("file:{f}"), bare(name).to_string()));
                // A re-export cycle, or a local rename, leads back to the declaring file itself;
                // it names the symbol by declaring it, not by importing it.
                if f != n.file { out.insert(f.to_string()); }
            }
        }
        for (f, name) in &names {
            for e in self.imports.get(f.as_str()).into_iter().flatten().filter(|e| exports(e, name)) {
                out.insert(e.source.trim_start_matches("file:").to_string());
            }
        }
        out.into_iter().collect()
    }

    /// Callers by depth and importing files; the index must have been built upstream.
    pub fn upstream(&self, root: &str, depth: usize) -> Impact {
        Impact { root: root.to_string(), layers: self.walk(root, depth), importers: self.importers(root) }
    }
}

pub fn upstream(graph: &Graph, root: &str, depth: usize) -> Impact { Index::new(graph, true).upstream(root, depth) }

pub fn downstream(graph: &Graph, root: &str, depth: usize) -> Impact {
    Impact { root: root.to_string(), layers: Index::new(graph, false).walk(root, depth), importers: Vec::new() }
}

/// `trace` as an object: the two ends the ids resolved to, the depth asked for, and the chain as
/// `{id, at}` steps — `null` when there is none within that depth, which is an answer and not an
/// error.
pub fn trace_json(graph: &Graph, from: &str, to: &str, depth: usize, path: Option<&[(String, bool)]>) -> String {
    #[derive(serde::Serialize)]
    struct Step<'a> { id: &'a str, at: String, passes: bool }
    #[derive(serde::Serialize)]
    struct Out<'a> { from: &'a str, to: &'a str, depth: usize, path: Option<Vec<Step<'a>>> }
    let steps = path.map(|p| p.iter().map(|(id, passes)| Step {
        id,
        at: graph.nodes.get(id).map(|n| format!("{}:{}", n.file, n.line)).unwrap_or_default(),
        passes: *passes,
    }).collect());
    serde_json::to_string(&Out { from, to, depth, path: steps }).unwrap_or_else(|_| "{}".to_string())
}

/// The shortest chain of code edges from `from` to `to` (or one of its aliases or members),
/// at most `depth` hops, each step with whether it was reached by being passed rather than called.
pub fn trace(graph: &Graph, from: &str, to: &str, depth: usize) -> Option<Vec<(String, bool)>> {
    // The walk marks `from` seen before it looks, so it would never arrive where it started.
    if from == to { return Some(vec![(from.to_string(), false)]) }
    let by_source = Index::new(graph, false);
    let goal: BTreeSet<String> = by_source.seeds(to).into_iter().collect();
    let mut parent: BTreeMap<String, (String, bool)> = BTreeMap::new();
    // The walk starts at `from` alone: `step` already reaches through its members, and the
    // printed path then names the class, not the member that happened to make the call.
    let mut frontier: Vec<String> = vec![from.to_string()];
    let mut seen: BTreeSet<String> = frontier.iter().cloned().collect();
    for _ in 0..depth {
        let mut next = Vec::new();
        for at in &frontier {
            for (e, (other, goes_on)) in by_source.step(at).into_iter().flat_map(|e| by_source.lands(e).into_iter().map(move |l| (e, l))) {
                if !seen.insert(other.clone()) { continue }
                parent.insert(other.clone(), (at.clone(), e.passes()));
                if goal.contains(&other) {
                    let mut path = vec![other];
                    while let Some((p, _)) = parent.get(path.last().unwrap()) { path.push(p.clone()); }
                    path.reverse();
                    return Some(path.into_iter().map(|id| { let passed = parent.get(&id).is_some_and(|p| p.1); (id, passed) }).collect());
                }
                if goes_on { next.push(other); }
            }
        }
        if next.is_empty() { break }
        frontier = next;
    }
    None
}

/// Fixed thresholds, printed beside the counts that produced them so the label can be argued
/// with and the list still used.
pub fn risk(direct: usize, _total: usize, files: usize) -> &'static str {
    if direct >= 30 || files >= 25 { "CRITICAL" }
    else if direct >= 15 || files >= 10 { "HIGH" }
    else if direct >= 5 || files >= 3 { "MEDIUM" }
    else { "LOW" }
}

pub fn files(graph: &Graph, imp: &Impact) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = imp.importers.iter().cloned().collect();
    for d in imp.layers.iter().flatten() {
        match graph.nodes.get(&d.id) {
            Some(n) => { out.insert(n.file.clone()); }
            None => { out.insert(d.id.trim_start_matches("file:").to_string()); }
        }
    }
    out
}

fn line_of(graph: &Graph, id: &str) -> String {
    match graph.nodes.get(id) { Some(n) => format!("{}:{}", n.file, n.line), None => "?".to_string() }
}

const LAYER_NAMES: [&str; 3] = ["will break", "likely affected", "may need testing"];

pub fn render(graph: &Graph, imp: &Impact, direction: &str) -> String {
    let up = direction == "upstream";
    let mut out = format!("{}  {}\n", imp.root, line_of(graph, &imp.root));
    for (i, layer) in imp.layers.iter().enumerate() {
        let name = if up { LAYER_NAMES.get(i).copied().unwrap_or("transitive") } else { "reaches" };
        out.push_str(&format!("d={}  {name} ({})\n", i + 1, layer.len()));
        for d in layer {
            let arrow = if up { "→" } else { "←" };
            out.push_str(&format!("  {}  {}  {} {arrow} {}\n", d.id, line_of(graph, &d.id), label(d.kind, d.passed), d.via));
        }
    }
    if up {
        if !imp.importers.is_empty() { out.push_str(&format!("importers ({}): {}\n", imp.importers.len(), imp.importers.join(", "))); }
        let direct = imp.layers.first().map_or(0, Vec::len);
        let total: usize = imp.layers.iter().map(Vec::len).sum();
        let files = files(graph, imp).len();
        out.push_str(&format!("risk: {} — {direct} direct, {total} total, {files} files\n", risk(direct, total, files)));
    }
    out
}

pub fn render_json(graph: &Graph, imp: &Impact, direction: &str) -> String {
    let direct = imp.layers.first().map_or(0, Vec::len);
    let total: usize = imp.layers.iter().map(Vec::len).sum();
    let files = files(graph, imp);
    let layers: Vec<Vec<serde_json::Value>> = imp.layers.iter().map(|l| l.iter().map(|d| serde_json::json!({
        "id": d.id, "at": line_of(graph, &d.id), "depth": d.depth, "kind": format!("{:?}", d.kind), "passes": d.passed, "via": d.via,
    })).collect()).collect();
    serde_json::json!({
        "root": imp.root, "at": line_of(graph, &imp.root), "direction": direction, "layers": layers,
        "importers": imp.importers, "files": files, "direct": direct, "total": total, "risk": risk(direct, total, files.len()),
    }).to_string() + "\n"
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::{Extraction, NodeKind};

    /// controller.create → service.create; module constructs the service; a barrel re-exports
    /// the service and a worker calls it through the barrel; a job extends the worker.
    fn graph() -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::File, "file:s.ts", "s.ts", "", "s.ts", 1);
        e.node(NodeKind::Symbol, "sym:s.ts::S", "S", "", "s.ts", 3);
        e.node(NodeKind::Symbol, "sym:s.ts::S.create", "S.create", "", "s.ts", 5);
        e.edge("file:s.ts", "sym:s.ts::S", EdgeKind::Declares, "export", "s.ts");
        e.edge("sym:s.ts::S", "sym:s.ts::S.create", EdgeKind::Declares, "", "s.ts");
        e.node(NodeKind::File, "file:c.ts", "c.ts", "", "c.ts", 1);
        e.node(NodeKind::Symbol, "sym:c.ts::C.create", "C.create", "", "c.ts", 9);
        e.edge("file:c.ts", "file:s.ts", EdgeKind::Imports, "S", "c.ts");
        e.edge("sym:c.ts::C.create", "sym:s.ts::S.create", EdgeKind::Calls, "", "c.ts");
        e.node(NodeKind::File, "file:m.ts", "m.ts", "", "m.ts", 1);
        e.edge("file:m.ts", "file:s.ts", EdgeKind::Imports, "S", "m.ts");
        e.edge("file:m.ts", "sym:s.ts::S", EdgeKind::Calls, "", "m.ts");
        e.node(NodeKind::File, "file:index.ts", "index.ts", "", "index.ts", 1);
        e.edge("file:index.ts", "file:s.ts", EdgeKind::ReExports, "*", "index.ts");
        e.node(NodeKind::File, "file:w.ts", "w.ts", "", "w.ts", 1);
        e.node(NodeKind::Symbol, "sym:w.ts::W.run", "W.run", "", "w.ts", 4);
        e.edge("file:w.ts", "file:index.ts", EdgeKind::Imports, "S", "w.ts");
        e.edge("sym:w.ts::W.run", "sym:index.ts::S.create", EdgeKind::Calls, "", "w.ts");
        e.node(NodeKind::Symbol, "sym:w.ts::W", "W", "", "w.ts", 3);
        e.edge("sym:w.ts::W", "sym:w.ts::W.run", EdgeKind::Declares, "", "w.ts");
        e.node(NodeKind::Symbol, "sym:j.ts::J", "J", "", "j.ts", 2);
        e.edge("sym:j.ts::J", "sym:w.ts::W", EdgeKind::Extends, "", "j.ts");
        g.apply(e);
        g
    }

    #[test]
    fn aliases_follow_re_exports_back_to_every_barrel() {
        let g = graph();
        assert_eq!(Index::new(&g, true).aliases("sym:s.ts::S.create"), vec!["sym:index.ts::S.create"]);
        assert_eq!(Index::new(&g, true).aliases("sym:s.ts::S"), vec!["sym:index.ts::S"]);
    }

    #[test]
    fn canonical_walks_a_barrel_forward_to_the_declaration() {
        assert_eq!(canonical(&graph(), "sym:index.ts::S.create").as_deref(), Some("sym:s.ts::S.create"));
        assert_eq!(canonical(&graph(), "sym:s.ts::S.create").as_deref(), Some("sym:s.ts::S.create"));
        assert_eq!(canonical(&graph(), "sym:index.ts::Nope"), None);
    }

    #[test]
    fn upstream_of_a_class_reaches_callers_of_its_members_and_through_barrels() {
        let imp = upstream(&graph(), "sym:s.ts::S", 3);
        let d1: Vec<&str> = imp.layers[0].iter().map(|d| d.id.as_str()).collect();
        assert_eq!(d1, vec!["file:m.ts", "sym:c.ts::C.create", "sym:w.ts::W.run"]);
        // J extends W, and W.run is the caller: the subclass inherits the call, the class
        // itself is not listed as a dependent of its own member.
        let d2: Vec<(&str, EdgeKind)> = imp.layers[1].iter().map(|d| (d.id.as_str(), d.kind)).collect();
        assert_eq!(d2, vec![("sym:j.ts::J", EdgeKind::Extends)]);
        assert_eq!(imp.layers.len(), 2);
        assert_eq!(imp.importers, vec!["c.ts", "index.ts", "m.ts", "w.ts"]);
    }

    /// A `repo` whose members no node declares — a schema's `parse`, a literal's shorthand
    /// property — so every call targets a `repo.*` id that is only a name, directly or through
    /// the barrel.
    pub(crate) fn undeclared_members() -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::File, "file:r.ts", "r.ts", "", "r.ts", 1);
        e.node_span(NodeKind::Symbol, "sym:r.ts::repo", "repo", "", "r.ts", (3, 20));
        e.edge("file:r.ts", "sym:r.ts::repo", EdgeKind::Declares, "export", "r.ts");
        e.node(NodeKind::File, "file:index.ts", "index.ts", "", "index.ts", 1);
        e.edge("file:index.ts", "file:r.ts", EdgeKind::ReExports, "*", "index.ts");
        e.node(NodeKind::Symbol, "sym:a.ts::A.run", "A.run", "", "a.ts", 4);
        e.edge("sym:a.ts::A.run", "sym:r.ts::repo.find", EdgeKind::Calls, "", "a.ts");
        e.node(NodeKind::Symbol, "sym:b.ts::go", "go", "", "b.ts", 2);
        e.edge("sym:b.ts::go", "sym:index.ts::repo.save", EdgeKind::Calls, "", "b.ts");
        e.node(NodeKind::Symbol, "sym:r.ts::repository", "repository", "", "r.ts", 30);
        e.node(NodeKind::Symbol, "sym:b.ts::other", "other", "", "b.ts", 9);
        e.edge("sym:b.ts::other", "sym:r.ts::repository.find", EdgeKind::Calls, "", "b.ts");
        g.apply(e);
        g
    }

    #[test]
    fn a_re_export_cycle_does_not_name_the_declaring_file_as_its_own_importer() {
        // b re-exports s, a re-exports b, and s re-exports a: aliases() walks the cycle all the
        // way back to s.ts itself, which must not then claim to import the symbol it declares.
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::File, "file:s.ts", "s.ts", "", "s.ts", 1);
        e.node(NodeKind::Symbol, "sym:s.ts::S", "S", "", "s.ts", 3);
        e.edge("file:s.ts", "sym:s.ts::S", EdgeKind::Declares, "export", "s.ts");
        e.node(NodeKind::File, "file:b.ts", "b.ts", "", "b.ts", 1);
        e.edge("file:b.ts", "file:s.ts", EdgeKind::ReExports, "*", "b.ts");
        e.node(NodeKind::File, "file:a.ts", "a.ts", "", "a.ts", 1);
        e.edge("file:a.ts", "file:b.ts", EdgeKind::ReExports, "*", "a.ts");
        e.edge("file:s.ts", "file:a.ts", EdgeKind::ReExports, "*", "s.ts");
        g.apply(e);
        let imp = upstream(&g, "sym:s.ts::S", 1).importers;
        assert!(!imp.contains(&"s.ts".to_string()), "{imp:?}");
        assert_eq!(imp, vec!["a.ts", "b.ts"]);
    }

    #[test]
    fn upstream_of_a_member_is_narrower_than_its_class() {
        let imp = upstream(&graph(), "sym:s.ts::S.create", 1);
        let d1: Vec<&str> = imp.layers[0].iter().map(|d| d.id.as_str()).collect();
        assert_eq!(d1, vec!["sym:c.ts::C.create", "sym:w.ts::W.run"]);
    }

    #[test]
    fn depth_caps_the_walk_and_a_dependent_appears_once_at_its_shallowest() {
        let imp = upstream(&graph(), "sym:s.ts::S", 1);
        assert_eq!(imp.layers.len(), 1);
        let deep = upstream(&graph(), "sym:s.ts::S", 9);
        let all: Vec<&str> = deep.layers.iter().flatten().map(|d| d.id.as_str()).collect();
        let set: BTreeSet<&str> = all.iter().copied().collect();
        assert_eq!(all.len(), set.len());
    }

    #[test]
    fn an_undeclared_member_target_folds_into_its_container() {
        let mut g = undeclared_members();
        let mut e = Extraction::default();
        e.edge("sym:r.ts::repo", "sym:r.ts::repo.find", EdgeKind::Calls, "", "r.ts");
        g.apply(e);
        assert_eq!(canonical(&g, "sym:index.ts::repo.save").as_deref(), Some("sym:r.ts::repo"));
        assert_eq!(canonical(&g, "sym:x.ts::nobody.m"), None);
        let imp = downstream(&g, "sym:b.ts::go", 1);
        let d1: Vec<&str> = imp.layers[0].iter().map(|d| d.id.as_str()).collect();
        assert_eq!(d1, vec!["sym:r.ts::repo"]);
        // A member calling a sibling is its container reaching itself: no row at all.
        assert!(downstream(&g, "sym:r.ts::repo", 1).layers.is_empty());
    }

    #[test]
    fn a_walk_down_stops_at_a_container_it_reached_through_one_undeclared_method() {
        // `go` calls `repo.save`; the container as a whole calls `lock`, through some other member.
        let mut g = undeclared_members();
        let mut e = Extraction::default();
        e.node(NodeKind::Symbol, "sym:l.ts::lock", "lock", "", "l.ts", 1);
        e.edge("sym:r.ts::repo", "sym:l.ts::lock", EdgeKind::Calls, "", "r.ts");
        g.apply(e);
        let imp = downstream(&g, "sym:b.ts::go", 3);
        let ids: Vec<&str> = imp.layers.iter().flatten().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["sym:r.ts::repo"]);
        assert_eq!(trace(&g, "sym:b.ts::go", "sym:l.ts::lock", 6), None);
        assert!(trace(&g, "sym:b.ts::go", "sym:r.ts::repo", 6).is_some());
        assert!(trace(&g, "sym:r.ts::repo", "sym:l.ts::lock", 6).is_some());
    }

    #[test]
    fn downstream_canonicalises_a_barrel_target() {
        let imp = downstream(&graph(), "sym:w.ts::W", 2);
        let d1: Vec<&str> = imp.layers[0].iter().map(|d| d.id.as_str()).collect();
        assert_eq!(d1, vec!["sym:s.ts::S.create"]);
    }

    #[test]
    fn trace_finds_the_shortest_call_path_and_reports_none_when_there_is_no_path() {
        // J → W by Extends, W → S.create through its member W.run and the barrel alias.
        assert_eq!(trace(&graph(), "sym:j.ts::J", "sym:s.ts::S", 6), Some(vec![("sym:j.ts::J".into(), false), ("sym:w.ts::W".into(), false), ("sym:s.ts::S.create".into(), false)]));
        assert_eq!(trace(&graph(), "sym:s.ts::S", "sym:j.ts::J", 6), None);
        assert_eq!(trace(&graph(), "sym:j.ts::J", "sym:s.ts::S", 1), None);
    }

    #[test]
    fn a_trace_from_a_symbol_to_itself_is_the_one_node_path() {
        assert_eq!(trace(&graph(), "sym:w.ts::W", "sym:w.ts::W", 6), Some(vec![("sym:w.ts::W".into(), false)]));
    }

    #[test]
    fn a_value_passed_as_an_argument_reads_as_passed_not_called_and_still_counts_upstream() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Symbol, "sym:a.ts::A", "A", "", "a.ts", 1);
        e.node(NodeKind::Symbol, "sym:t.ts::TOKEN", "TOKEN", "", "t.ts", 1);
        e.node(NodeKind::Symbol, "sym:t.ts::run", "run", "", "t.ts", 2);
        e.edge("sym:a.ts::A", "sym:t.ts::TOKEN", EdgeKind::Calls, "arg", "a.ts");
        e.edge("sym:a.ts::A", "sym:t.ts::run", EdgeKind::Calls, "", "a.ts");
        g.apply(e);
        let down = render(&g, &downstream(&g, "sym:a.ts::A", 1), "downstream");
        assert!(down.contains("  sym:t.ts::TOKEN  t.ts:1  Passes ← sym:a.ts::A\n"), "{down}");
        assert!(down.contains("  sym:t.ts::run  t.ts:2  Calls ← sym:a.ts::A\n"), "{down}");
        assert_eq!(trace(&g, "sym:a.ts::A", "sym:t.ts::TOKEN", 2), Some(vec![("sym:a.ts::A".into(), false), ("sym:t.ts::TOKEN".into(), true)]));
        assert_eq!(upstream(&g, "sym:t.ts::TOKEN", 1).layers[0].len(), 1);
    }

    #[test]
    fn a_call_from_one_owner_beats_a_passed_edge_from_another_at_the_same_depth() {
        // R calls A and Z; A only passes `repo.save`, Z calls `repo.find`, and both fold to `repo`.
        let mut g = undeclared_members();
        let mut e = Extraction::default();
        e.node(NodeKind::Symbol, "sym:x.ts::R", "R", "", "x.ts", 1);
        e.node(NodeKind::Symbol, "sym:x.ts::A", "A", "", "x.ts", 2);
        e.node(NodeKind::Symbol, "sym:x.ts::Z", "Z", "", "x.ts", 3);
        e.edge("sym:x.ts::R", "sym:x.ts::A", EdgeKind::Calls, "", "x.ts");
        e.edge("sym:x.ts::R", "sym:x.ts::Z", EdgeKind::Calls, "", "x.ts");
        e.edge("sym:x.ts::A", "sym:r.ts::repo.save", EdgeKind::Calls, "arg", "x.ts");
        e.edge("sym:x.ts::Z", "sym:r.ts::repo.find", EdgeKind::Calls, "", "x.ts");
        g.apply(e);
        let imp = downstream(&g, "sym:x.ts::R", 2);
        let d2: Vec<(&str, &str, bool)> = imp.layers[1].iter().map(|d| (d.id.as_str(), d.via.as_str(), d.passed)).collect();
        assert_eq!(d2, vec![("sym:r.ts::repo", "sym:x.ts::Z", false)]);
    }

    #[test]
    fn json_marks_an_argument_edge_as_passes_in_impact_and_trace() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Symbol, "sym:a.ts::A", "A", "", "a.ts", 1);
        e.node(NodeKind::Symbol, "sym:t.ts::TOKEN", "TOKEN", "", "t.ts", 1);
        e.node(NodeKind::Symbol, "sym:t.ts::run", "run", "", "t.ts", 2);
        e.edge("sym:a.ts::A", "sym:t.ts::TOKEN", EdgeKind::Calls, "arg", "a.ts");
        e.edge("sym:a.ts::A", "sym:t.ts::run", EdgeKind::Calls, "", "a.ts");
        g.apply(e);
        let v: serde_json::Value = serde_json::from_str(&render_json(&g, &downstream(&g, "sym:a.ts::A", 1), "downstream")).unwrap();
        let rows: Vec<(&str, &serde_json::Value)> = v["layers"][0].as_array().unwrap().iter().map(|d| (d["id"].as_str().unwrap(), &d["passes"])).collect();
        assert_eq!(rows, vec![("sym:t.ts::TOKEN", &serde_json::json!(true)), ("sym:t.ts::run", &serde_json::json!(false))]);
        let path = trace(&g, "sym:a.ts::A", "sym:t.ts::TOKEN", 2);
        let v: serde_json::Value = serde_json::from_str(&trace_json(&g, "sym:a.ts::A", "sym:t.ts::TOKEN", 2, path.as_deref())).unwrap();
        assert_eq!((&v["path"][0]["passes"], &v["path"][1]["passes"]), (&serde_json::json!(false), &serde_json::json!(true)));
    }

    #[test]
    fn risk_thresholds_are_the_documented_ones() {
        assert_eq!(risk(0, 0, 0), "LOW");
        assert_eq!(risk(4, 9, 2), "LOW");
        assert_eq!(risk(5, 5, 1), "MEDIUM");
        assert_eq!(risk(1, 1, 3), "MEDIUM");
        assert_eq!(risk(15, 15, 1), "HIGH");
        assert_eq!(risk(2, 40, 10), "HIGH");
        assert_eq!(risk(30, 30, 1), "CRITICAL");
        assert_eq!(risk(1, 60, 25), "CRITICAL");
    }

    #[test]
    fn render_lists_layers_with_path_line_and_ends_with_the_risk() {
        let g = graph();
        let out = render(&g, &upstream(&g, "sym:s.ts::S", 3), "upstream");
        assert!(out.starts_with("sym:s.ts::S  s.ts:3"));
        assert!(out.contains("d=1  will break (3)\n"));
        assert!(out.contains("  sym:c.ts::C.create  c.ts:9  Calls → sym:s.ts::S.create\n"));
        assert!(out.contains("importers (4): c.ts, index.ts, m.ts, w.ts\n"));
        assert!(out.ends_with("risk: MEDIUM — 3 direct, 4 total, 5 files\n"), "{out}");
    }

    #[test]
    fn render_json_is_valid_and_carries_the_same_counts() {
        let g = graph();
        let v: serde_json::Value = serde_json::from_str(&render_json(&g, &upstream(&g, "sym:s.ts::S", 3), "upstream")).unwrap();
        assert_eq!(v["root"], "sym:s.ts::S");
        assert_eq!(v["direction"], "upstream");
        assert_eq!(v["layers"][0].as_array().unwrap().len(), 3);
        assert_eq!(v["risk"], "MEDIUM");
    }
}
#[cfg(test)]
mod container_cases {
    use super::{bare, container};

    #[test]
    fn a_member_is_changed_through_its_class() {
        assert_eq!(container("sym:a.ts::S.create").as_deref(), Some("sym:a.ts::S"));
    }

    #[test]
    fn a_nested_member_is_changed_through_the_nearest_class() {
        assert_eq!(container("sym:a.cs::Outer.Inner.Deep").as_deref(), Some("sym:a.cs::Outer.Inner"));
    }

    #[test]
    fn a_class_and_a_file_have_no_container() {
        assert_eq!(container("sym:a.ts::S"), None);
        assert_eq!(container("file:a.ts"), None);
    }

    #[test]
    fn a_slash_joined_address_is_one_name_and_a_dot_after_it_is_its_member() {
        assert_eq!(container("sym:db/0002_rls.sql::app/clients.status").as_deref(), Some("sym:db/0002_rls.sql::app/clients"));
        assert_eq!(bare("app/clients.status"), "app/clients");
        assert_eq!(container("sym:infra/staging/main.tf::aws_instance/web"), None);
        assert_eq!(bare("aws_instance/web"), "aws_instance/web");
        // A dot in the file's path is not a member: the name starts after the first `::`.
        assert_eq!(container("sym:db/v1.2/x.sql::app/clients.status").as_deref(), Some("sym:db/v1.2/x.sql::app/clients"));
    }

    /// A package-style Shell function is one name holding `::`. Split at the last one, its file
    /// reads `lib.sh::log`, its name `info`, and an importer spelling `log::info` is lost.
    #[test]
    fn a_name_holding_a_double_colon_keeps_its_file_and_its_importers() {
        use crate::model::{EdgeKind, Extraction, Graph, NodeKind};
        assert_eq!(container("sym:lib.sh::log::info"), None);
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Symbol, "sym:lib.sh::log::info", "log::info", "", "lib.sh", 1);
        e.edge("file:all.sh", "file:lib.sh", EdgeKind::ReExports, "*", "all.sh");
        e.edge("file:run.sh", "file:all.sh", EdgeKind::Imports, "log::info", "run.sh");
        g.apply(e);
        assert_eq!(super::Index::new(&g, true).aliases("sym:lib.sh::log::info"), ["sym:all.sh::log::info"]);
        assert_eq!(super::upstream(&g, "sym:lib.sh::log::info", 3).importers, ["all.sh", "run.sh"]);
    }
}

#[cfg(test)]
mod walk_cases {
    use super::{downstream, trace, upstream, walks};
    use crate::model::{Edge, EdgeKind, Extraction, Graph, NodeKind};

    fn edge(target: &str, kind: EdgeKind) -> Edge {
        Edge { source: "sym:db/0002.sql::app/clients.status".into(), target: target.into(), kind, context: String::new(), file: "db/0002.sql".into() }
    }

    #[test]
    fn a_call_an_extension_and_a_reference_to_a_symbol_are_walked() {
        assert!(walks(&edge("sym:a.ts::S", EdgeKind::Calls)));
        assert!(walks(&edge("sym:a.ts::S", EdgeKind::Extends)));
        assert!(walks(&edge("sym:db/0001.sql::app/clients", EdgeKind::References)));
        assert!(walks(&edge("sym:a.ts::Port", EdgeKind::Implements)));
    }

    #[test]
    fn a_document_s_implements_edge_to_a_requirement_or_a_file_is_not_walked() {
        for target in ["FR-DM-05", "gate:ranking_input_whitelist_test", "file:packages/db/test/contours.spec.ts"] {
            assert!(!walks(&edge(target, EdgeKind::Implements)), "{target}");
        }
    }

    #[test]
    fn a_reference_to_a_document_id_an_entity_or_a_file_is_not_walked_and_nor_is_an_import() {
        for target in ["FR-PAY-22", "entity:CancellationPolicy", "file:docs/x.md"] {
            assert!(!walks(&edge(target, EdgeKind::References)), "{target}");
        }
        for kind in [EdgeKind::Imports, EdgeKind::ReExports, EdgeKind::Declares, EdgeKind::DecoratedBy] {
            assert!(!walks(&edge("sym:a.ts::S", kind)), "{kind:?}");
        }
    }

    /// A migration's column names the table another migration created; a TypeScript function cites
    /// a requirement. The first is a dependency a blast walk must see, the second is `ask`'s alone.
    fn graph() -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Symbol, "sym:db/0001.sql::app/clients", "app/clients", "", "db/0001.sql", 1);
        e.node(NodeKind::Symbol, "sym:db/0002.sql::app/clients.status", "app/clients.status", "", "db/0002.sql", 1);
        e.edge("sym:db/0002.sql::app/clients.status", "sym:db/0001.sql::app/clients", EdgeKind::References, "", "db/0002.sql");
        e.node(NodeKind::Requirement, "FR-PAY-22", "FR-PAY-22", "", "docs/06.md", 1);
        e.node(NodeKind::Symbol, "sym:svc/pay.ts::pay", "pay", "", "svc/pay.ts", 1);
        e.edge("sym:svc/pay.ts::pay", "FR-PAY-22", EdgeKind::References, "comment", "svc/pay.ts");
        g.apply(e);
        g
    }

    #[test]
    fn impact_and_trace_follow_a_reference_between_symbols() {
        let g = graph();
        let up = upstream(&g, "sym:db/0001.sql::app/clients", 3);
        let d1: Vec<(&str, EdgeKind)> = up.layers[0].iter().map(|d| (d.id.as_str(), d.kind)).collect();
        assert_eq!(d1, vec![("sym:db/0002.sql::app/clients.status", EdgeKind::References)]);
        assert_eq!(up.layers.len(), 1);
        assert_eq!(
            trace(&g, "sym:db/0002.sql::app/clients.status", "sym:db/0001.sql::app/clients", 3),
            Some(vec![("sym:db/0002.sql::app/clients.status".to_string(), false), ("sym:db/0001.sql::app/clients".to_string(), false)])
        );
    }

    #[test]
    fn a_reference_to_a_requirement_is_walked_by_neither() {
        let g = graph();
        assert!(upstream(&g, "FR-PAY-22", 3).layers.is_empty());
        assert!(trace(&g, "sym:svc/pay.ts::pay", "FR-PAY-22", 3).is_none());
    }

    /// `changes` is the third reader of `walks`, through `impact::upstream`, and the SQL blast
    /// suite's headline row: a table's hunk names the migration whose column depends on it.
    #[test]
    fn changes_to_a_table_name_the_migration_whose_column_references_it() {
        use crate::changes::{report, Hunk};
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node_span(NodeKind::Symbol, "sym:db/0001.sql::app/clients", "app/clients", "", "db/0001.sql", (1, 1));
        e.node_span(NodeKind::Symbol, "sym:db/0002.sql::app/clients.status", "app/clients.status", "", "db/0002.sql", (1, 1));
        e.edge("sym:db/0002.sql::app/clients.status", "sym:db/0001.sql::app/clients", EdgeKind::References, "", "db/0002.sql");
        g.apply(e);
        let r = report(&g, &[Hunk { file: "db/0001.sql".into(), start: 1, end: 1 }], 3);
        let affected: Vec<(&str, EdgeKind)> = r.affected.iter().map(|d| (d.id.as_str(), d.kind)).collect();
        assert_eq!(affected, [("sym:db/0002.sql::app/clients.status", EdgeKind::References)]);
        assert!(r.files.contains("db/0002.sql") && !r.files.contains("db/0001.sql"), "{:?}", r.files);
    }

    /// The commonest SQL shape: a table whose column references the table itself. Its own column
    /// must not inflate "will break" or the risk, and a swapped index would walk the edge backwards.
    #[test]
    fn a_self_referencing_table_is_not_its_own_dependent_and_a_reference_is_walked_one_way() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Symbol, "sym:db/1.sql::app/staff", "app/staff", "", "db/1.sql", 1);
        e.node(NodeKind::Symbol, "sym:db/1.sql::app/staff.manager_id", "app/staff.manager_id", "", "db/1.sql", 2);
        e.edge("sym:db/1.sql::app/staff", "sym:db/1.sql::app/staff.manager_id", EdgeKind::Declares, "", "db/1.sql");
        e.edge("sym:db/1.sql::app/staff.manager_id", "sym:db/1.sql::app/staff", EdgeKind::References, "", "db/1.sql");
        e.node(NodeKind::Symbol, "sym:db/2.sql::app/shifts.staff_id", "app/shifts.staff_id", "", "db/2.sql", 1);
        e.edge("sym:db/2.sql::app/shifts.staff_id", "sym:db/1.sql::app/staff", EdgeKind::References, "", "db/2.sql");
        g.apply(e);
        let up: Vec<String> = upstream(&g, "sym:db/1.sql::app/staff", 5).layers.into_iter().flatten().map(|d| d.id).collect();
        assert_eq!(up, ["sym:db/2.sql::app/shifts.staff_id"]);
        let down: Vec<String> = downstream(&g, "sym:db/2.sql::app/shifts.staff_id", 5).layers.into_iter().flatten().map(|d| d.id).collect();
        assert_eq!(down, ["sym:db/1.sql::app/staff"]);
        assert!(trace(&g, "sym:db/1.sql::app/staff", "sym:db/2.sql::app/shifts.staff_id", 5).is_none());
    }

    #[test]
    fn a_change_to_an_outer_type_reaches_callers_of_its_nested_type_s_members() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        for (id, line) in [("sym:a.cs::Outer", 1), ("sym:a.cs::Outer.Inner", 2), ("sym:a.cs::Outer.Inner.Go", 3)] {
            e.node(NodeKind::Symbol, id, id.rsplit("::").next().unwrap(), "", "a.cs", line);
        }
        e.edge("sym:a.cs::Outer", "sym:a.cs::Outer.Inner", EdgeKind::Declares, "", "a.cs");
        e.edge("sym:a.cs::Outer.Inner", "sym:a.cs::Outer.Inner.Go", EdgeKind::Declares, "", "a.cs");
        e.node(NodeKind::Symbol, "sym:b.cs::Caller.Run", "Caller.Run", "", "b.cs", 1);
        e.edge("sym:b.cs::Caller.Run", "sym:a.cs::Outer.Inner.Go", EdgeKind::Calls, "", "b.cs");
        g.apply(e);
        let up = upstream(&g, "sym:a.cs::Outer", 3);
        let d1: Vec<&str> = up.layers.first().map(|l| l.iter().map(|d| d.id.as_str()).collect()).unwrap_or_default();
        assert_eq!(d1, ["sym:b.cs::Caller.Run"]);
    }

    /// A repository written as an object literal, extracted from source rather than drawn: one
    /// method reaches `lockOverlapGroup`, a sibling reaches nothing, a third reaches
    /// `recomputeOverlapFlags`, and each has its own caller in another file.
    fn literal_repository() -> Graph {
        use crate::model::Extractor;
        let files = [
            ("l.ts", "export function lockOverlapGroup() {}\nexport function recomputeOverlapFlags() {}\n"),
            ("r.ts", "import { lockOverlapGroup, recomputeOverlapFlags } from './l';\nexport const repo = {\n  lock() {\n    lockOverlapGroup();\n  },\n  applyEdit() {\n    return 1;\n  },\n  recompute: () => recomputeOverlapFlags(),\n};\n"),
            ("brief.ts", "import { repo } from './r';\nexport class BriefService {\n  get() { return repo.applyEdit(); }\n}\n"),
            ("guard.ts", "import { repo } from './r';\nexport function guard() { repo.lock(); }\n"),
            ("move.ts", "import { repo } from './r';\nexport function applyMove() { repo.recompute(); }\n"),
        ];
        let dir = tempfile::tempdir().unwrap();
        for (p, c) in files { std::fs::write(dir.path().join(p), c).unwrap(); }
        let resolver = crate::code::imports::Resolver::new(dir.path(), &crate::config::Config::default()).unwrap();
        let extractor = crate::code::CodeExtractor::new(resolver);
        let mut g = Graph::default();
        for (p, c) in files { g.apply(extractor.extract(p, c)); }
        g
    }

    fn ids(layers: Vec<Vec<crate::impact::Dependent>>) -> Vec<String> { layers.into_iter().flatten().map(|d| d.id).collect() }

    #[test]
    fn a_path_through_a_literal_s_other_method_is_walked_by_no_command() {
        let g = literal_repository();
        let up = ids(upstream(&g, "sym:l.ts::lockOverlapGroup", 3).layers);
        assert_eq!(up, ["sym:r.ts::repo.lock", "sym:guard.ts::guard"], "the callers of the sibling methods do not reach it");
        assert_eq!(ids(downstream(&g, "sym:brief.ts::BriefService.get", 3).layers), ["sym:r.ts::repo.applyEdit"]);
        assert_eq!(trace(&g, "sym:brief.ts::BriefService.get", "sym:l.ts::lockOverlapGroup", 6), None);
        assert_eq!(ids(downstream(&g, "sym:move.ts::applyMove", 3).layers), ["sym:r.ts::repo.recompute", "sym:l.ts::recomputeOverlapFlags"]);
        let changed = crate::changes::report(&g, &[crate::changes::Hunk { file: "r.ts".into(), start: 4, end: 4 }], 3);
        assert_eq!(changed.touched, ["sym:r.ts::repo.lock"]);
        assert_eq!(changed.affected.into_iter().map(|d| d.id).collect::<Vec<_>>(), ["sym:guard.ts::guard"]);
    }

    #[test]
    fn impact_up_and_down_trace_and_changes_agree_on_every_pair_across_a_literal() {
        let g = literal_repository();
        let symbols = [
            "sym:brief.ts::BriefService.get", "sym:guard.ts::guard", "sym:move.ts::applyMove", "sym:r.ts::repo.lock",
            "sym:r.ts::repo.applyEdit", "sym:r.ts::repo.recompute", "sym:l.ts::lockOverlapGroup", "sym:l.ts::recomputeOverlapFlags",
        ];
        for s in symbols { assert!(g.nodes.contains_key(s), "{s} is a node"); }
        for from in symbols {
            let down = ids(downstream(&g, from, 3).layers);
            for to in symbols.iter().filter(|&&t| t != from) {
                let up = ids(upstream(&g, to, 3).layers);
                let line = g.nodes[*to].line;
                let changed = crate::changes::report(&g, &[crate::changes::Hunk { file: g.nodes[*to].file.clone(), start: line, end: line }], 3);
                let traced = trace(&g, from, to, 3).is_some();
                let reached = [down.iter().any(|d| d == to), up.iter().any(|u| u == from), changed.affected.iter().any(|d| d.id == from)];
                assert_eq!(reached, [traced; 3], "{from} → {to}: down, up, changes against trace");
            }
        }
    }

    #[test]
    fn a_literal_is_changed_through_its_methods_and_explain_lists_the_callers_impact_does() {
        let g = literal_repository();
        let d1 = ids(upstream(&g, "sym:r.ts::repo", 1).layers);
        assert_eq!(d1, ["sym:brief.ts::BriefService.get", "sym:guard.ts::guard", "sym:move.ts::applyMove"]);
        let v: serde_json::Value = serde_json::from_str(&crate::query::explain_json(&g, "sym:r.ts::repo").unwrap()).unwrap();
        let mut callers: Vec<&str> = v["edges"].as_array().unwrap().iter()
            .filter(|e| e["dir"] == "in" && e["kind"] == "Calls").map(|e| e["other"].as_str().unwrap()).collect();
        callers.sort();
        callers.dedup();
        assert_eq!(callers, d1);
    }

    /// Every file extracted from source, imports resolved against the others.
    fn extracted(files: &[(&str, &str)]) -> Graph {
        use crate::model::Extractor;
        let dir = tempfile::tempdir().unwrap();
        for (p, c) in files {
            let path = dir.path().join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, c).unwrap();
        }
        let resolver = crate::code::imports::Resolver::new(dir.path(), &crate::config::Config::default()).unwrap();
        let extractor = crate::code::CodeExtractor::new(resolver);
        let mut g = Graph::default();
        for (p, c) in files { g.apply(extractor.extract(p, c)); }
        g
    }

    /// A port and two adapters, extracted from source: `Receipts` holds the port and calls it
    /// through a barrel, `NextVisitAdapter` implements it and calls `score`, `Stub` implements it
    /// through the barrel and calls nothing.
    fn port_and_adapters() -> Graph {
        extracted(&[
            ("port.ts", "export interface NextVisitPort {\n  suggest(id: string): void;\n}\n"),
            ("index.ts", "export * from './port';\n"),
            ("score.ts", "export function score() {}\n"),
            ("adapter.ts", "import { NextVisitPort } from './port';\nimport { score } from './score';\nexport class NextVisitAdapter implements NextVisitPort {\n  suggest(id: string) {\n    score();\n  }\n}\n"),
            ("stub.ts", "import { NextVisitPort } from './index';\nexport class Stub implements NextVisitPort {\n  suggest(id: string) {}\n}\n"),
            ("receipts.ts", "import { NextVisitPort } from './index';\nexport class Receipts {\n  constructor(private nextVisit: NextVisitPort) {}\n  close() {\n    this.nextVisit.suggest('x');\n  }\n}\n"),
        ])
    }

    /// `contrast` renamed twice: in its own file (`export { contrast as contrastRatio }`) and by a
    /// barrel (`export { contrast as ratio } from`), with a caller importing each of the three names.
    fn renamed_contrast() -> Graph {
        extracted(&[
            ("theme/contrast.ts", "export function contrast(a: number, b: number) { return a / b; }\nexport { contrast as contrastRatio };\n"),
            ("theme/index.ts", "export { contrast as ratio } from './contrast';\n"),
            ("one.ts", "import { contrastRatio } from './theme/contrast';\nexport function one() { return contrastRatio(1, 2); }\n"),
            ("two.ts", "import { contrast } from './theme/contrast';\nexport function two() { return contrast(1, 2); }\n"),
            ("three.ts", "import { ratio } from './theme';\nexport function three() { return ratio(1, 2); }\n"),
        ])
    }

    #[test]
    fn a_renamed_re_export_counts_the_callers_of_every_name_it_goes_by() {
        let g = renamed_contrast();
        let root = "sym:theme/contrast.ts::contrast";
        let imp = upstream(&g, root, 3);
        assert_eq!(ids(imp.layers), ["sym:one.ts::one", "sym:three.ts::three", "sym:two.ts::two"]);
        assert_eq!(imp.importers, ["one.ts", "theme/index.ts", "three.ts", "two.ts"]);
        for from in ["sym:one.ts::one", "sym:three.ts::three"] {
            assert_eq!(ids(downstream(&g, from, 1).layers), [root], "{from} reaches the original");
            assert!(trace(&g, from, root, 3).is_some(), "{from} traces to the original");
        }
    }

    #[test]
    fn a_call_through_a_port_is_a_caller_of_every_adapter() {
        let g = port_and_adapters();
        for adapter in ["sym:adapter.ts::NextVisitAdapter.suggest", "sym:stub.ts::Stub.suggest", "sym:adapter.ts::NextVisitAdapter"] {
            let d1: Vec<String> = upstream(&g, adapter, 1).layers.into_iter().flatten().map(|d| d.id).collect();
            assert_eq!(d1, ["sym:receipts.ts::Receipts.close"], "{adapter}");
        }
        assert_eq!(ids(upstream(&g, "sym:score.ts::score", 3).layers), ["sym:adapter.ts::NextVisitAdapter.suggest", "sym:receipts.ts::Receipts.close"]);
    }

    #[test]
    fn a_change_to_a_port_s_member_breaks_its_callers_and_every_class_that_implements_it() {
        let d1 = ids(upstream(&port_and_adapters(), "sym:port.ts::NextVisitPort.suggest", 1).layers);
        assert_eq!(d1, ["sym:adapter.ts::NextVisitAdapter", "sym:receipts.ts::Receipts.close", "sym:stub.ts::Stub"]);
    }

    #[test]
    fn a_walk_down_through_a_port_lands_on_the_port_and_every_adapter() {
        let g = port_and_adapters();
        let down = downstream(&g, "sym:receipts.ts::Receipts.close", 3);
        let layers: Vec<Vec<String>> = down.layers.into_iter().map(|l| l.into_iter().map(|d| d.id).collect()).collect();
        assert_eq!(layers, [
            vec!["sym:adapter.ts::NextVisitAdapter.suggest", "sym:port.ts::NextVisitPort.suggest", "sym:stub.ts::Stub.suggest"],
            vec!["sym:score.ts::score"],
        ]);
        let path: Vec<String> = trace(&g, "sym:receipts.ts::Receipts.close", "sym:score.ts::score", 3).unwrap().into_iter().map(|(id, _)| id).collect();
        assert_eq!(path, ["sym:receipts.ts::Receipts.close", "sym:adapter.ts::NextVisitAdapter.suggest", "sym:score.ts::score"]);
    }

    #[test]
    fn impact_up_and_down_trace_and_changes_agree_on_every_pair_across_a_port() {
        let g = port_and_adapters();
        let symbols = [
            "sym:receipts.ts::Receipts.close", "sym:port.ts::NextVisitPort.suggest", "sym:adapter.ts::NextVisitAdapter.suggest",
            "sym:stub.ts::Stub.suggest", "sym:score.ts::score",
        ];
        for s in symbols { assert!(g.nodes.contains_key(s), "{s} is a node"); }
        for from in symbols {
            let down = ids(downstream(&g, from, 3).layers);
            for to in symbols.iter().filter(|&&t| t != from) {
                let up = ids(upstream(&g, to, 3).layers);
                let line = g.nodes[*to].line;
                let changed = crate::changes::report(&g, &[crate::changes::Hunk { file: g.nodes[*to].file.clone(), start: line, end: line }], 3);
                let traced = trace(&g, from, to, 3).is_some();
                let reached = [down.iter().any(|d| d == to), up.iter().any(|u| u == from), changed.affected.iter().any(|d| d.id == from)];
                assert_eq!(reached, [traced; 3], "{from} → {to}: down, up, changes against trace");
            }
        }
    }

    #[test]
    fn an_alias_typed_by_name_or_id_resolves_to_the_original() {
        let g = renamed_contrast();
        let root = "sym:theme/contrast.ts::contrast";
        for alias in ["contrastRatio", "ratio", "sym:theme/contrast.ts::contrastRatio", "sym:theme/index.ts::ratio"] {
            assert_eq!(crate::query::resolve_code(&g, alias).unwrap().0.id, root, "{alias}");
        }
        assert!(crate::query::resolve_code(&g, "nothing").is_err());
    }

    /// `KEYS.filter` is an array method no node declares; the call depends on `KEYS` all the same.
    /// `far` reaches it through a barrel.
    fn keys() -> Graph {
        extracted(&[
            ("keys.ts", "export const KEYS = ['a', 'b'];\nexport function pick() {\n  return KEYS.filter((k) => k === 'a');\n}\n"),
            ("index.ts", "export * from './keys';\n"),
            ("far.ts", "import { KEYS } from './index';\nexport function far() { return KEYS.map((k) => k); }\n"),
        ])
    }

    #[test]
    fn a_caller_of_an_undeclared_member_is_a_caller_of_its_symbol_and_names_it_as_via() {
        let g = keys();
        let root = "sym:keys.ts::KEYS";
        let imp = upstream(&g, root, 3);
        let rows: Vec<(&str, &str)> = imp.layers.iter().flatten().map(|d| (d.id.as_str(), d.via.as_str())).collect();
        assert_eq!(rows, [("sym:far.ts::far", "sym:index.ts::KEYS"), ("sym:keys.ts::pick", root)]);
        let text = super::render(&g, &imp, "upstream");
        assert!(text.contains("  sym:keys.ts::pick  keys.ts:2  Calls → sym:keys.ts::KEYS\n"), "{text}");
        let v: serde_json::Value = serde_json::from_str(&super::render_json(&g, &imp, "upstream")).unwrap();
        assert_eq!(v["layers"][0][1]["via"], root);
        // The commands agree: `--down` and `trace` land `KEYS.filter` on `KEYS` too.
        for from in ["sym:keys.ts::pick", "sym:far.ts::far"] {
            assert_eq!(ids(downstream(&g, from, 3).layers), [root], "{from}");
            assert!(trace(&g, from, root, 3).is_some(), "{from}");
        }
    }

    #[test]
    fn changes_to_a_symbol_name_it_as_the_via_of_a_caller_of_its_undeclared_member() {
        let g = keys();
        let r = crate::changes::report(&g, &[crate::changes::Hunk { file: "keys.ts".into(), start: 1, end: 1 }], 3);
        assert_eq!(r.touched, ["sym:keys.ts::KEYS"]);
        let text = crate::changes::render(&g, &r);
        assert!(text.contains("  d=1  sym:keys.ts::pick  keys.ts:2  ← sym:keys.ts::KEYS\n"), "{text}");
        let v: serde_json::Value = serde_json::from_str(&crate::changes::render_json(&g, &r)).unwrap();
        let vias: Vec<(&str, &str)> = v["affected"].as_array().unwrap().iter().map(|d| (d["id"].as_str().unwrap(), d["via"].as_str().unwrap())).collect();
        assert_eq!(vias, [("sym:far.ts::far", "sym:index.ts::KEYS"), ("sym:keys.ts::pick", "sym:keys.ts::KEYS")]);
    }

    #[test]
    fn explain_lists_a_call_through_the_port_as_a_caller_of_the_adapter_as_impact_does() {
        let g = port_and_adapters();
        let id = "sym:adapter.ts::NextVisitAdapter.suggest";
        let d1 = ids(upstream(&g, id, 1).layers);
        let v: serde_json::Value = serde_json::from_str(&crate::query::explain_json(&g, id).unwrap()).unwrap();
        let mut callers: Vec<&str> = v["edges"].as_array().unwrap().iter()
            .filter(|e| e["dir"] == "in" && e["kind"] == "Calls").map(|e| e["other"].as_str().unwrap()).collect();
        callers.sort();
        callers.dedup();
        assert_eq!(callers, d1);
    }
}
