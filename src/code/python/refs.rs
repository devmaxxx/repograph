//! What a Python file's code reaches: `Imports`, the names they bind, `Calls` through those
//! names and through typed attributes, `Extends` and `DecoratedBy`.

use super::defs::{owner, Defs};
use super::modules::Modules;
use crate::code::syntax::{descend, field_text, named, text};
use crate::model::{EdgeKind, Extraction};
use std::collections::{BTreeMap, BTreeSet};
use tree_sitter::Node;

/// What an imported name stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Bound {
    /// A module's file.
    Module(String),
    /// A name inside a module's file.
    Name(String, String),
}

/// An import's target file, its edge context, and the name it binds, if any.
type Import = (String, String, Option<(String, Bound)>);

struct Ctx<'a> {
    rel: &'a str,
    src: &'a [u8],
    modules: &'a Modules,
    defs: &'a Defs,
    /// Scope → bound name → what it stands for. "" is the module; a function's scope is its id suffix.
    bindings: BTreeMap<String, BTreeMap<String, Bound>>,
    /// Class id suffix → attribute → its type as a dotted path; empty when the file types it two ways.
    attrs: BTreeMap<String, BTreeMap<String, String>>,
    /// Function scope → names a parameter or local assignment makes its own for the whole function.
    shadows: BTreeMap<String, BTreeSet<String>>,
}

/// Where a lookup happens: the scope's id suffix, and the parameters of the lambdas around it.
#[derive(Clone, Copy)]
struct Sc<'a> {
    name: &'a str,
    lambda: &'a BTreeSet<String>,
}

pub(crate) fn read(modules: &Modules, rel: &str, src: &[u8], root: Node, defs: &Defs, ex: &mut Extraction) {
    let mut ctx = Ctx { rel, src, modules, defs, bindings: BTreeMap::new(), attrs: BTreeMap::new(), shadows: BTreeMap::new() };
    ctx.imports(root, ex);
    ctx.shadows(root);
    ctx.attribute_types(root);
    ctx.calls(root, ex);
    ctx.classes(root, ex);
}

/// A name or dotted attribute chain as written: `a`, `a.b.C`; anything else is not a path.
fn dotted(n: Node, src: &[u8]) -> Option<String> {
    match n.kind() {
        "identifier" => Some(text(n, src).to_string()),
        "attribute" => Some(format!("{}.{}", dotted(n.child_by_field_name("object")?, src)?, field_text(n, "attribute", src)?)),
        "type" => dotted(n.named_child(0)?, src),
        _ => None,
    }
}

/// `import a.b` or `from x import a.b as c`: the dotted name and the alias, if any.
fn name_and_alias(n: Node, src: &[u8]) -> Option<(String, Option<String>)> {
    match n.kind() {
        "aliased_import" => Some((text(n.child_by_field_name("name")?, src).to_string(), field_text(n, "alias", src).map(str::to_string))),
        "dotted_name" => Some((text(n, src).to_string(), None)),
        _ => None,
    }
}

/// The names an assignment-like target binds: `a`, `a, (b, *c)`; an attribute or subscript binds none.
fn target_names(n: Node, src: &[u8], out: &mut BTreeSet<String>) {
    match n.kind() {
        "identifier" => {
            out.insert(text(n, src).to_string());
        }
        "pattern_list" | "tuple_pattern" | "list_pattern" | "list_splat_pattern" | "parenthesized_expression" | "tuple" | "list"
        | "as_pattern_target" => {
            named(n).into_iter().for_each(|c| target_names(c, src, out));
        }
        _ => {}
    }
}

/// The name a parameter of any shape binds; `*args` and `**kw` wrap theirs.
fn param_names(p: Node, src: &[u8], out: &mut BTreeSet<String>) {
    match p.kind() {
        "identifier" => target_names(p, src, out),
        "list_splat_pattern" | "dictionary_splat_pattern" => named(p).into_iter().for_each(|c| target_names(c, src, out)),
        "typed_parameter" => named(p).into_iter().take(1).for_each(|c| param_names(c, src, out)),
        "default_parameter" | "typed_default_parameter" => p.child_by_field_name("name").into_iter().for_each(|c| target_names(c, src, out)),
        _ => {}
    }
}

fn parameters_of(f: Node, src: &[u8]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if let Some(ps) = f.child_by_field_name("parameters") {
        named(ps).into_iter().for_each(|p| param_names(p, src, &mut out));
    }
    out
}

/// Records `attr: ty`; a second, different type makes the attribute untyped, since which one a
/// call sees depends on control flow.
fn note_type(types: &mut BTreeMap<String, String>, attr: &str, ty: String) {
    match types.get(attr) {
        Some(old) if *old != ty => types.insert(attr.to_string(), String::new()),
        _ => types.insert(attr.to_string(), ty),
    };
}

impl Ctx<'_> {
    fn suffix<'b>(&self, id: &'b str) -> &'b str {
        id.strip_prefix("sym:").and_then(|s| s.strip_prefix(self.rel)).and_then(|s| s.strip_prefix("::")).unwrap_or("")
    }

    /// The scope a binding or a call at `n` belongs to: its owner's id suffix, "" for the file.
    fn scope(&self, n: Node) -> String {
        self.suffix(&owner(n, self.rel, self.src)).to_string()
    }

    /// The node an edge from `n` starts at: its owner when this file wrote it, else the file. A
    /// definition inside a redefinition can name a path no node was written for.
    fn source(&self, n: Node) -> String {
        let from = owner(n, self.rel, self.src);
        if self.defs.names.contains(self.suffix(&from)) { from } else { format!("file:{}", self.rel) }
    }

    /// A target in this file must be a node this file wrote; one in another file cannot be
    /// checked from here.
    fn written(&self, target: String) -> Option<String> {
        let own = target.strip_prefix("sym:").and_then(|s| s.strip_prefix(self.rel)).and_then(|s| s.strip_prefix("::"));
        own.is_none_or(|s| self.defs.names.contains(s)).then_some(target)
    }

    /// The names a function makes its own — parameters, assignment, `for`, `with`/`except … as`
    /// and walrus targets, nested definitions — and the names `global` and `nonlocal` hand back to
    /// the enclosing binding.
    fn locals(&self, f: Node) -> (BTreeSet<String>, BTreeSet<String>) {
        let mut set = parameters_of(f, self.src);
        let mut freed = BTreeSet::new();
        if let Some(body) = f.child_by_field_name("body") {
            descend(body, &mut |n| {
                match n.kind() {
                    "assignment" | "augmented_assignment" | "for_statement" | "for_in_clause" => {
                        n.child_by_field_name("left").into_iter().for_each(|l| target_names(l, self.src, &mut set));
                    }
                    "as_pattern" | "except_clause" => n.child_by_field_name("alias").into_iter().for_each(|a| target_names(a, self.src, &mut set)),
                    "named_expression" => n.child_by_field_name("name").into_iter().for_each(|a| target_names(a, self.src, &mut set)),
                    "function_definition" | "class_definition" => {
                        set.extend(field_text(n, "name", self.src).map(str::to_string));
                    }
                    "global_statement" | "nonlocal_statement" => named(n).into_iter().for_each(|c| target_names(c, self.src, &mut freed)),
                    _ => {}
                }
                true
            });
        }
        (set, freed)
    }

    /// What each function makes its own. A nested function's names widen its outer function's
    /// set, which only loses edges. `global` and `nonlocal` name the enclosing binding, so they
    /// cancel the shadow. A local import is a binding, not a shadow. A function no symbol stands
    /// for — one under an `if` or `try` — has no set: its names must not shadow the module's, and
    /// `calls` reads them off the function around the call instead.
    fn shadows(&mut self, root: Node) {
        let mut shadows: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut released: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        descend(root, &mut |f| {
            let Some(name) = (f.kind() == "function_definition").then(|| f.child_by_field_name("name")).flatten() else { return true };
            let key = self.scope(name);
            if key.is_empty() {
                return true;
            }
            let (set, freed) = self.locals(f);
            shadows.entry(key.clone()).or_default().extend(set);
            released.entry(key).or_default().extend(freed);
            true
        });
        for (key, mut set) in shadows {
            if let Some(freed) = released.get(&key) {
                set.retain(|n| !freed.contains(n));
            }
            self.shadows.insert(key, set);
        }
    }

    /// Whether the head of a path is a local of the scope or a parameter of a lambda around the
    /// call. `self` is a parameter too, but the receiver rules read it before any lookup.
    fn shadowed(&self, sc: Sc, head: &str) -> bool {
        sc.lambda.contains(head) || self.shadows.get(sc.name).is_some_and(|s| s.contains(head))
    }

    /// The parameters of every lambda that encloses `n`, and — when no symbol owns `n`, so no
    /// shadow set does either — the locals of every function around it.
    fn lambda_params(&self, n: Node, owned: bool) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for a in std::iter::successors(n.parent(), |p| p.parent()) {
            match a.kind() {
                "lambda" => {
                    if let Some(ps) = a.child_by_field_name("parameters") {
                        named(ps).into_iter().for_each(|p| param_names(p, self.src, &mut out));
                    }
                }
                "function_definition" if !owned => {
                    let (set, freed) = self.locals(a);
                    out.extend(set.into_iter().filter(|s| !freed.contains(s)));
                }
                _ => {}
            }
        }
        out
    }

    fn bound(&self, scope: &str, name: &str) -> Option<&Bound> {
        [scope, ""].iter().find_map(|s| self.bindings.get(*s).and_then(|m| m.get(name)))
    }

    fn imports(&mut self, root: Node, ex: &mut Extraction) {
        descend(root, &mut |n| {
            if matches!(n.kind(), "import_statement" | "import_from_statement") {
                self.import(n, ex);
                return false;
            }
            true
        });
    }

    fn import(&mut self, n: Node, ex: &mut Extraction) {
        let scope = self.scope(n);
        let mut c = n.walk();
        let names: Vec<(String, Option<String>)> = n.children_by_field_name("name", &mut c).filter_map(|x| name_and_alias(x, self.src)).collect();
        let mut found: Vec<Import> = Vec::new();
        if n.kind() == "import_statement" {
            for (name, alias) in names {
                let Some(file) = self.modules.absolute(self.rel, &name) else { continue };
                // `import a.b` binds the chain a call spells, `a.b.f()`.
                found.push((file.clone(), "*".into(), Some((alias.unwrap_or(name), Bound::Module(file)))));
            }
        } else {
            let Some(module) = n.child_by_field_name("module_name") else { return };
            let (level, from) = if module.kind() == "relative_import" {
                let kids = named(module);
                let level = kids.iter().find(|k| k.kind() == "import_prefix").map_or(0, |p| text(*p, self.src).matches('.').count());
                let from = kids.iter().find(|k| k.kind() == "dotted_name").map(|d| text(*d, self.src).to_string()).unwrap_or_default();
                (level, from)
            } else {
                (0, text(module, self.src).to_string())
            };
            let locate = |dotted: &str| {
                if level > 0 { self.modules.relative(self.rel, level, dotted) } else { self.modules.absolute(self.rel, dotted) }
            };
            let base = locate(&from);
            if named(n).iter().any(|x| x.kind() == "wildcard_import") {
                found.extend(base.clone().map(|b| (b, "*".to_string(), None)));
            }
            for (name, alias) in names {
                let bound_as = alias.unwrap_or_else(|| name.clone());
                let sub = locate(&if from.is_empty() { name.clone() } else { format!("{from}.{name}") });
                match (sub, &base) {
                    (Some(file), _) => found.push((file.clone(), "*".into(), Some((bound_as, Bound::Module(file))))),
                    (None, Some(b)) => {
                        // The edge names the package the statement imports; the binding follows a
                        // re-export in its `__init__.py` to the module that declares the name.
                        let (file, there) = self.modules.reexport(b, &name).unwrap_or_else(|| (b.clone(), name.clone()));
                        found.push((b.clone(), name, Some((bound_as, Bound::Name(file, there)))));
                    }
                    (None, None) => {}
                }
            }
        }
        let file_id = format!("file:{}", self.rel);
        for (file, context, binding) in found {
            ex.edge(&file_id, &format!("file:{file}"), EdgeKind::Imports, &context, self.rel);
            if let Some((name, bound)) = binding {
                self.bindings.entry(scope.clone()).or_default().insert(name, bound);
            }
        }
    }

    /// Each class's attribute types: a class-level annotation, and in `__init__` an annotated
    /// `self.x`, `self.x = T(…)` with a capitalised `T`, or `self.x = p` for an annotated parameter.
    fn attribute_types(&mut self, root: Node) {
        descend(root, &mut |n| {
            if n.kind() != "class_definition" {
                return true;
            }
            let (Some(name), Some(body)) = (n.child_by_field_name("name"), n.child_by_field_name("body")) else { return true };
            let class = self.suffix(&owner(name, self.rel, self.src)).to_string();
            if !self.defs.classes.contains(&class) {
                return true;
            }
            let mut types: BTreeMap<String, String> = BTreeMap::new();
            for stmt in named(body) {
                let def = if stmt.kind() == "decorated_definition" { stmt.child_by_field_name("definition") } else { Some(stmt) };
                let Some(def) = def else { continue };
                if def.kind() == "expression_statement" {
                    let Some(a) = def.named_child(0).filter(|a| a.kind() == "assignment") else { continue };
                    if let (Some(left), Some(ty)) = (a.child_by_field_name("left").filter(|l| l.kind() == "identifier"), a.child_by_field_name("type").and_then(|t| dotted(t, self.src))) {
                        note_type(&mut types, text(left, self.src), ty);
                    }
                } else if def.kind() == "function_definition" && field_text(def, "name", self.src) == Some("__init__") {
                    self.init_types(def, &mut types);
                }
            }
            // A class declared twice keeps its first definition's types, as its members do.
            self.attrs.entry(class).or_insert(types);
            true
        });
    }

    fn init_types(&self, init: Node, types: &mut BTreeMap<String, String>) {
        let mut params: BTreeMap<String, String> = BTreeMap::new();
        if let Some(ps) = init.child_by_field_name("parameters") {
            for p in named(ps).into_iter().filter(|p| matches!(p.kind(), "typed_parameter" | "typed_default_parameter")) {
                let name = p.child_by_field_name("name").or_else(|| p.named_child(0)).filter(|x| x.kind() == "identifier");
                if let (Some(name), Some(ty)) = (name, p.child_by_field_name("type").and_then(|t| dotted(t, self.src))) {
                    params.insert(text(name, self.src).to_string(), ty);
                }
            }
        }
        let Some(body) = init.child_by_field_name("body") else { return };
        descend(body, &mut |n| {
            if n.kind() != "assignment" {
                return true;
            }
            let Some(left) = n.child_by_field_name("left").filter(|l| l.kind() == "attribute") else { return true };
            if left.child_by_field_name("object").is_none_or(|o| text(o, self.src) != "self") {
                return true;
            }
            let Some(attr) = field_text(left, "attribute", self.src) else { return true };
            let right = n.child_by_field_name("right");
            let ty = n.child_by_field_name("type").and_then(|t| dotted(t, self.src)).or_else(|| match right.map(|r| (r.kind(), r)) {
                Some(("call", r)) => r
                    .child_by_field_name("function")
                    .and_then(|f| dotted(f, self.src))
                    .filter(|d| d.rsplit('.').next().is_some_and(|last| last.starts_with(|c: char| c.is_ascii_uppercase()))),
                Some(("identifier", r)) => params.get(text(r, self.src)).cloned(),
                _ => None,
            });
            if let Some(ty) = ty {
                note_type(types, attr, ty);
            }
            true
        });
    }

    /// `obj.attr` where `obj` is a dotted path: a bound module (`truth.read_jsonl`, `a.b.f`), a
    /// class bound by name or declared here (`Store.open`), or a bound prefix of the path (`mod.Cls.m`).
    fn qualified(&self, sc: Sc, obj: &str, attr: &str, local: bool) -> Option<String> {
        let scope = sc.name;
        if local && self.shadowed(sc, obj.split('.').next().unwrap_or(obj)) {
            return None;
        }
        let at = |b: &Bound, rest: &str| match b {
            Bound::Module(f) if rest.is_empty() => format!("sym:{f}::{attr}"),
            Bound::Module(f) => format!("sym:{f}::{rest}.{attr}"),
            Bound::Name(f, n) if rest.is_empty() => format!("sym:{f}::{n}.{attr}"),
            Bound::Name(f, n) => format!("sym:{f}::{n}.{rest}.{attr}"),
        };
        if let Some(b) = self.bound(scope, obj) {
            return Some(at(b, ""));
        }
        if self.defs.classes.contains(obj) {
            return Some(format!("sym:{}::{obj}.{attr}", self.rel));
        }
        let mut cut = obj.len();
        while let Some(i) = obj[..cut].rfind('.') {
            if let Some(b) = self.bound(scope, &obj[..i]) {
                return Some(at(b, &obj[i + 1..]));
            }
            cut = i;
        }
        None
    }

    fn callee(&self, sc: Sc, f: Node) -> Option<String> {
        let scope = sc.name;
        let target = match f.kind() {
            "identifier" => {
                let name = text(f, self.src);
                if self.shadowed(sc, name) {
                    return None;
                }
                match self.bound(scope, name) {
                    Some(Bound::Name(file, n)) => Some(format!("sym:{file}::{n}")),
                    Some(Bound::Module(_)) => None,
                    None => self.defs.top.contains(name).then(|| format!("sym:{}::{name}", self.rel)),
                }
            }
            "attribute" => self.member(sc, f),
            _ => None,
        };
        self.written(target?)
    }

    fn member(&self, sc: Sc, f: Node) -> Option<String> {
        let scope = sc.name;
        let attr = field_text(f, "attribute", self.src)?;
        let object = f.child_by_field_name("object")?;
        let class = scope.rsplit_once('.').map(|(c, _)| c).filter(|c| self.defs.classes.contains(*c));
        let is_self = |n: Node| n.kind() == "identifier" && text(n, self.src) == "self";
        if is_self(object) {
            return Some(format!("sym:{}::{}.{attr}", self.rel, class?));
        }
        if object.kind() == "attribute" && object.child_by_field_name("object").is_some_and(is_self) {
            let field = field_text(object, "attribute", self.src)?;
            let ty = self.attrs.get(class?)?.get(field).filter(|t| !t.is_empty())?;
            return self.qualified(sc, ty, attr, false);
        }
        self.qualified(sc, &dotted(object, self.src)?, attr, true)
    }

    fn calls(&self, root: Node, ex: &mut Extraction) {
        descend(root, &mut |n| {
            if n.kind() != "call" {
                return true;
            }
            let from = self.source(n);
            let scope = self.suffix(&from);
            let lambda = self.lambda_params(n, !scope.is_empty());
            let sc = Sc { name: scope, lambda: &lambda };
            let target = n.child_by_field_name("function").and_then(|f| self.callee(sc, f));
            if let Some(target) = target.filter(|t| *t != from) {
                ex.edge(&from, &target, EdgeKind::Calls, "", self.rel);
            }
            true
        });
    }

    /// A path as a class or decorator names it: a bound or top-level name, or a qualified one.
    fn path_target(&self, n: Node) -> Option<String> {
        self.callee(Sc { name: "", lambda: &BTreeSet::new() }, n)
    }

    fn classes(&self, root: Node, ex: &mut Extraction) {
        descend(root, &mut |n| {
            match n.kind() {
                "class_definition" => self.bases(n, ex),
                "decorated_definition" => self.decorators(n, ex),
                _ => {}
            }
            true
        });
    }

    fn bases(&self, class: Node, ex: &mut Extraction) {
        let (Some(name), Some(bases)) = (class.child_by_field_name("name"), class.child_by_field_name("superclasses")) else { return };
        let id = owner(name, self.rel, self.src);
        if !self.defs.classes.contains(self.suffix(&id)) {
            return;
        }
        for base in named(bases).into_iter().filter(|b| matches!(b.kind(), "identifier" | "attribute")) {
            if let Some(target) = self.path_target(base).filter(|t| *t != id) {
                ex.edge(&id, &target, EdgeKind::Extends, "", self.rel);
            }
        }
    }

    fn decorators(&self, decorated: Node, ex: &mut Extraction) {
        let Some(name) = decorated.child_by_field_name("definition").and_then(|d| d.child_by_field_name("name")) else { return };
        let from = owner(name, self.rel, self.src);
        // A definition inside a function is owned by that function, which it does not decorate.
        let declared_here = self.suffix(&from).rsplit('.').next() == Some(text(name, self.src));
        if from.starts_with("file:") || !declared_here || !self.defs.names.contains(self.suffix(&from)) {
            return;
        }
        for deco in named(decorated).into_iter().filter(|d| d.kind() == "decorator") {
            // `@x(…)` names `x`; the call's arguments configure it.
            let expr = deco.named_child(0).map(|e| if e.kind() == "call" { e.child_by_field_name("function").unwrap_or(e) } else { e });
            if let Some((expr, target)) = expr.and_then(|e| self.path_target(e).map(|t| (e, t))) {
                ex.edge(&from, &target, EdgeKind::DecoratedBy, &dotted(expr, self.src).unwrap_or_default(), self.rel);
            }
        }
    }
}
