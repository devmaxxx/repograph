//! A Python name is in scope through what its file's imports bind, and an import names a module by a
//! dotted path that only the interpreter's search path places: the importer's directory when it is a
//! script, each directory above it that holds a project manifest, that project's `src/`, and the
//! repository root. No query sees another file, so this hook places each import, writes its edge
//! with the name it binds, and resolves calls, base classes and decorators through those bindings,
//! the file's own classes, and the class a `self` attribute holds.
//!
//! An import's context is the name it binds from the module, as a TypeScript named import's is; a
//! whole module, imported or `*`, says `*`.

use std::collections::{BTreeSet, HashMap};

use crate::code::engine::{Facts, File, Import, Sink};
use crate::code::imports::{join, parent, Resolver};
use crate::code::queries::Kind;
use crate::model::EdgeKind;

/// The directories holding `pyproject.toml`, `setup.py` or `setup.cfg`. Nothing in the file is read,
/// so a `package-dir` naming another directory places nothing.
#[derive(Debug, Default)]
pub(crate) struct Roots(BTreeSet<String>);

pub(crate) fn manifest(r: &mut Resolver, rel: &str, _: &str) {
    r.state_mut::<Roots>().0.insert(parent(rel).to_string());
}

fn module_file(r: &Resolver, base: &str, dotted: &str) -> Option<String> {
    let path = join(base, &dotted.replace('.', "/"));
    [format!("{path}.py"), format!("{path}/__init__.py")].into_iter().find(|p| r.globbed(p))
}

/// The file `import dotted` names from `rel`, trying the roots in the order Python finds them. A
/// package's own directory is not on the path, so a module in it never finds its siblings by bare
/// name: `import types` beside a `types.py` is the standard library's.
pub(crate) fn absolute(r: &Resolver, rel: &str, dotted: &str) -> Option<String> {
    let roots = &r.state::<Roots>().0;
    let mut dir = parent(rel);
    let mut bases = if r.globbed(&join(dir, "__init__.py")) { Vec::new() } else { vec![dir.to_string()] };
    loop {
        if roots.contains(dir) && !bases.iter().any(|b| b == dir) {
            bases.push(dir.to_string());
        }
        if dir.is_empty() {
            break;
        }
        dir = parent(dir);
    }
    if !bases.iter().any(String::is_empty) {
        bases.push(String::new());
    }
    // A project that keeps its packages under `src/` is imported by the package's name, as an
    // installed one is, and only after the package was not found beside its manifest.
    let src_layout = |base: &str| roots.contains(base).then(|| module_file(r, &join(base, "src"), dotted)).flatten();
    bases.iter().find_map(|b| module_file(r, b, dotted).or_else(|| src_layout(b)))
}

/// The file `from <level dots><dotted> import …` names from `rel`: one dot is the package holding
/// `rel`, each further dot its parent, and an empty `dotted` the package itself.
pub(crate) fn relative(r: &Resolver, rel: &str, level: usize, dotted: &str) -> Option<String> {
    let mut dir = parent(rel).to_string();
    for _ in 1..level {
        if dir.is_empty() {
            return None;
        }
        dir = parent(&dir).to_string();
    }
    if dotted.is_empty() {
        let init = join(&dir, "__init__.py");
        return r.globbed(&init).then_some(init);
    }
    module_file(r, &dir, dotted)
}

/// `.x.y` as its dot count and the dotted rest.
fn split_level(path: &str) -> (usize, &str) {
    let rest = path.trim_start_matches('.');
    (path.len() - rest.len(), rest.trim())
}

/// The module that declares `name` when the package `init` re-exports it from a submodule, and the
/// name there. One hop: a chain of `__init__.py` re-exports is rare, and a cycle must not loop.
fn reexport(r: &Resolver, init: &str, name: &str) -> Option<(String, String)> {
    if init != "__init__.py" && !init.ends_with("/__init__.py") {
        return None;
    }
    let facts = r.facts(init)?;
    let site = facts.imports.iter().rev().find(|i| {
        let from = i.path.as_deref().map(split_level).filter(|(level, dotted)| *level > 0 && !dotted.is_empty());
        let there = i.names.first().filter(|n| *n != "*");
        from.is_some() && there.is_some_and(|n| i.alias.as_deref().unwrap_or(n) == name)
    })?;
    let (level, dotted) = split_level(site.path.as_deref()?);
    Some((relative(r, init, level, dotted)?, site.names[0].clone()))
}

/// What an imported name stands for.
#[derive(Debug)]
enum Bound {
    Module(String),
    /// A name inside a module's file.
    Name(String, String),
}

/// `a.b.c` with its whitespace dropped, when it is a dotted path of names; a call, a subscript or a
/// literal is not one.
fn dotted(text: &str) -> Option<String> {
    let path: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let name = |s: &str| s.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') && s.chars().all(|c| c.is_alphanumeric() || c == '_');
    path.split('.').all(name).then_some(path)
}

struct Scope<'a> {
    rel: &'a str,
    facts: &'a Facts,
    /// By the definition holding the import, `None` for the module.
    bindings: HashMap<Option<usize>, HashMap<String, Bound>>,
    /// By class and attribute, the attribute's class as a dotted path; empty where the file gives it
    /// two, since which one a call sees depends on control flow.
    attrs: HashMap<(usize, String), String>,
}

impl Scope<'_> {
    fn bound(&self, scope: Option<usize>, name: &str) -> Option<&Bound> {
        [scope, None].iter().find_map(|s| self.bindings.get(s).and_then(|m| m.get(name)))
    }

    /// The class `self` is inside a member of.
    fn class_of(&self, source: Option<usize>) -> Option<usize> {
        source.and_then(|i| self.facts.defs[i].parent).filter(|p| self.facts.defs[*p].kind.join() == Some('.'))
    }

    fn own_class(&self, qualified: &str) -> bool {
        self.facts.declares(qualified)
            && self.facts.defs.iter().any(|d| d.qualified == qualified && d.kind.join() == Some('.'))
    }

    /// `obj.attr` where `obj` is a path: a bound module, a class bound by name or declared here, or
    /// a bound prefix of the path (`mod.Cls.m`). A local the path starts with, at `at`, names nothing.
    fn qualified(&self, scope: Option<usize>, at: Option<usize>, obj: &str, attr: &str) -> Option<String> {
        let head = obj.split('.').next().unwrap_or(obj);
        if at.is_some_and(|byte| self.facts.locals.shadows(head, byte)) {
            return None;
        }
        let under = |b: &Bound, rest: &str| match (b, rest) {
            (Bound::Module(f), "") => format!("sym:{f}::{attr}"),
            (Bound::Module(f), rest) => format!("sym:{f}::{rest}.{attr}"),
            (Bound::Name(f, n), "") => format!("sym:{f}::{n}.{attr}"),
            (Bound::Name(f, n), rest) => format!("sym:{f}::{n}.{rest}.{attr}"),
        };
        if let Some(b) = self.bound(scope, obj) {
            return Some(under(b, ""));
        }
        if self.own_class(obj) {
            return Some(format!("sym:{}::{obj}.{attr}", self.rel));
        }
        let mut cut = obj.len();
        while let Some(i) = obj[..cut].rfind('.') {
            if let Some(b) = self.bound(scope, &obj[..i]) {
                return Some(under(b, &obj[i + 1..]));
            }
            cut = i;
        }
        None
    }

    /// What `receiver.name` names from `source`, a call's callee at byte `at` or, with `at` unset, a
    /// base class or a decorator, which no local can shadow and which read the module's bindings alone.
    fn target(&self, source: Option<usize>, at: Option<usize>, receiver: Option<&str>, name: &str) -> Option<String> {
        let scope = at.and(source);
        let target = match receiver {
            None => {
                if at.is_some_and(|byte| self.facts.locals.shadows(name, byte)) {
                    return None;
                }
                match self.bound(scope, name) {
                    Some(Bound::Name(f, n)) => Some(format!("sym:{f}::{n}")),
                    Some(Bound::Module(_)) => None,
                    None => self.facts.declares(name).then(|| format!("sym:{}::{name}", self.rel)),
                }
            }
            Some(obj) => {
                let class = at.and_then(|_| self.class_of(source));
                match obj.split_once('.') {
                    _ if obj == "self" => class.map(|c| format!("sym:{}::{}.{name}", self.rel, self.facts.defs[c].qualified)),
                    Some(("self", field)) if !field.contains('.') => {
                        let ty = self.attrs.get(&(class?, field.to_string())).filter(|t| !t.is_empty())?;
                        self.qualified(scope, None, ty, name)
                    }
                    _ => self.qualified(scope, at, obj, name),
                }
            }
        }?;
        // A target in this file must be a node it wrote; one in another file cannot be checked here.
        let own = target.strip_prefix("sym:").and_then(|s| s.strip_prefix(self.rel)).and_then(|s| s.strip_prefix("::"));
        own.is_none_or(|q| self.facts.declares(q)).then_some(target)
    }
}

/// The edge one import site writes, with the name it binds and what that name stands for.
struct Placed {
    target: String,
    context: String,
    binding: Option<(String, Bound)>,
}

fn import(r: &Resolver, rel: &str, site: &Import) -> Option<Placed> {
    let path = site.path.as_deref()?;
    let Some(name) = site.names.first() else {
        // `import a.b` binds the chain a call spells, `a.b.f()`.
        let file = absolute(r, rel, path)?;
        let name = site.alias.clone().unwrap_or_else(|| path.to_string());
        return Some(Placed { target: file.clone(), context: "*".into(), binding: Some((name, Bound::Module(file))) });
    };
    let (level, from) = split_level(path);
    let locate = |dotted: &str| if level > 0 { relative(r, rel, level, dotted) } else { absolute(r, rel, dotted) };
    if name == "*" {
        return Some(Placed { target: locate(from)?, context: "*".into(), binding: None });
    }
    let bound_as = site.alias.clone().unwrap_or_else(|| name.clone());
    if let Some(file) = locate(&if from.is_empty() { name.clone() } else { format!("{from}.{name}") }) {
        return Some(Placed { target: file.clone(), context: "*".into(), binding: Some((bound_as, Bound::Module(file))) });
    }
    // The edge names the package the statement imports; the binding follows a re-export in its
    // `__init__.py` to the module that declares the name.
    let base = locate(from)?;
    let (file, there) = reexport(r, &base, name).unwrap_or_else(|| (base.clone(), name.clone()));
    Some(Placed { target: base, context: name.clone(), binding: Some((bound_as, Bound::Name(file, there))) })
}

pub(crate) fn resolve(r: &Resolver, file: &File, facts: &Facts, sink: &mut Sink) {
    let rel = file.rel;
    let file_id = format!("file:{rel}");
    let mut sites: Vec<&Import> = facts.imports.iter().collect();
    sites.sort_by_key(|i| i.range.start);
    let mut bindings: HashMap<Option<usize>, HashMap<String, Bound>> = HashMap::new();
    for site in sites {
        let Some(Placed { target, context, binding }) = import(r, rel, site) else { continue };
        sink.edge(&file_id, &format!("file:{target}"), EdgeKind::Imports, &context, rel);
        if let Some((name, bound)) = binding {
            bindings.entry(facts.owner(site.range.start)).or_default().insert(name, bound);
        }
    }
    let mut scope = Scope { rel, facts, bindings, attrs: HashMap::new() };
    let params: HashMap<(usize, &str), &str> = facts
        .refs
        .iter()
        .filter(|p| p.edge == EdgeKind::References && p.context == "argument")
        .filter_map(|p| Some(((p.source?, p.receiver.as_deref()?), p.name.as_str())))
        .collect();
    for t in facts.refs.iter().filter(|t| t.edge == EdgeKind::References && matches!(t.context.as_str(), "type" | "variable")) {
        let (Some(source), Some(attr)) = (t.source, t.receiver.as_deref()) else { continue };
        let named = match t.context.as_str() {
            "variable" => match params.get(&(source, t.name.as_str())) {
                Some(ty) => *ty,
                None => continue,
            },
            _ => t.name.as_str(),
        };
        // A class-level annotation is a declaration of the class; a `self` attribute counts in
        // `__init__` alone, which every instance runs.
        let d = &facts.defs[source];
        if d.kind != Kind::Variable && d.name != "__init__" {
            continue;
        }
        let (Some(class), Some(ty)) = (scope.class_of(Some(source)), dotted(named)) else { continue };
        let typed = scope.attrs.entry((class, attr.to_string())).or_insert_with(|| ty.clone());
        if *typed != ty {
            typed.clear();
        }
    }
    let scope = scope;
    for call in facts.refs.iter().filter(|c| c.edge == EdgeKind::Calls) {
        let receiver = match call.receiver.as_deref() {
            Some(text) => match dotted(text) {
                Some(path) => Some(path),
                None => continue,
            },
            None => None,
        };
        let from = facts.source_id(rel, call.source);
        let target = scope.target(call.source, Some(call.range.start), receiver.as_deref(), &call.name);
        if let Some(to) = target.filter(|t| *t != from) {
            sink.edge(&from, &to, EdgeKind::Calls, "", rel);
        }
    }
    // A base and a decorator belong to a declared class or definition: one inside a function is
    // owned by that function, which it neither extends nor decorates.
    for site in facts.refs.iter().filter(|s| matches!(s.edge, EdgeKind::Extends | EdgeKind::DecoratedBy)) {
        let def = match site.edge {
            EdgeKind::Extends => site.source.filter(|i| facts.defs[*i].kind.join() == Some('.')),
            _ => facts.declared_at(&site.range),
        };
        let (Some(def), Some(path)) = (def, dotted(&site.name)) else { continue };
        let from = &facts.defs[def];
        let (receiver, name) = match path.rsplit_once('.') {
            Some((obj, name)) => (Some(obj), name),
            None => (None, path.as_str()),
        };
        let context = if site.edge == EdgeKind::DecoratedBy { path.as_str() } else { "" };
        if let Some(to) = scope.target(None, None, receiver, name).filter(|t| *t != from.id) {
            sink.edge(&from.id, &to, site.edge, context, rel);
        }
    }
}
