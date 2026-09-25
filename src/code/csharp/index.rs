//! The .NET index: every C# type's declaring files and members, extension methods by name, each
//! project's root namespace and `global using`s — what `Scope` resolves a name against.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use super::declarations::{self, join, Declared, Using};
use super::Host;
use crate::code::lang::Lang;
use crate::model::Extraction;

/// One declaration of a type: its file, the name after `sym:<rel>::`, and its qualified name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Part {
    pub rel: String,
    pub local: String,
    pub full: String,
}

type Members = BTreeMap<String, Option<String>>;

/// One part as the index keeps it: what it declares, and its base list as written, read later in
/// the part's own scope rather than any caller's.
#[derive(Debug)]
struct Entry {
    part: Part,
    members: Members,
    bases: Vec<String>,
    generic_bases: BTreeSet<String>,
}

thread_local! {
    /// `Resolver::new` asks `header_for` and then `collect` about the same file back to back; a parse
    /// of the corpus's 1,176 C# files costs about 0.4 s, and one parse serves both calls.
    static LAST: RefCell<Option<(String, u64, Declared)>> = const { RefCell::new(None) };
}

fn fingerprint(source: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut h);
    h.finish()
}

/// What a `.cs` file declares, read for the index; the graph nodes the walk writes are discarded.
pub fn facts(rel: &str, source: &str) -> Declared {
    let key = fingerprint(source);
    let hit = LAST.with(|l| l.borrow().as_ref().filter(|(r, k, _)| r == rel && *k == key).map(|(_, _, d)| d.clone()));
    if let Some(d) = hit {
        return d;
    }
    let mut scratch = Extraction::default();
    let d = Lang::CSharp.parse(source.as_bytes())
        .map(|t| declarations::scan(t.root_node(), rel, source.as_bytes(), &Host::default(), &mut scratch))
        .unwrap_or_default();
    LAST.with(|l| *l.borrow_mut() = Some((rel.to_string(), key, d.clone())));
    d
}

fn parent(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(d, _)| d)
}

/// One namespace segment as the Razor compiler writes it (`CSharpIdentifier.AppendSanitized`):
/// every character an identifier cannot hold becomes `_`, and a leading digit gets a `_` before it.
fn identifier(s: &str) -> String {
    let lead = if s.starts_with(char::is_numeric) { "_" } else { "" };
    lead.chars().chain(s.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })).collect()
}

/// Razor sanitises every segment of the namespace it builds — the `@namespace` and the project's
/// root namespace included, which the SDK only rids of spaces.
fn sanitised(namespace: &str) -> String {
    namespace.split('.').filter(|s| !s.is_empty()).map(identifier).collect::<Vec<_>>().join(".")
}

/// Where the Razor compiler puts a view's class when no `@namespace` names one
/// (`MvcViewDocumentClassifierPass`); the repository declares nothing there.
const VIEW_NAMESPACE: &str = "AspNetCoreGeneratedDocument";

fn is_view(rel: &str) -> bool {
    rel.ends_with(".cshtml")
}

/// The folders of `dir` below `base` as namespace segments: `Pages/Admin` gives `Pages.Admin`.
fn folders(base: &str, dir: &str) -> String {
    let below = if base.is_empty() { dir } else { dir.strip_prefix(base).unwrap_or(dir).trim_start_matches('/') };
    below.split('/').filter(|s| !s.is_empty()).map(identifier).collect::<Vec<_>>().join(".")
}

/// A `.razor` component as the index keeps it.
#[derive(Debug)]
struct Component {
    /// The `@namespace` the file names, if any.
    namespace: Option<String>,
    /// Its `@code` and `@inject` members.
    members: Members,
    /// Its `@inherits`, as written.
    bases: Vec<String>,
    /// Its `@inherits` when written with type arguments.
    generic_bases: BTreeSet<String>,
    /// Its own `@using`s.
    usings: Vec<Using>,
}

#[derive(Debug, Default)]
pub struct DotNet {
    /// `Namespace.Outer.Inner` → every part with its members and base list.
    types: BTreeMap<String, Vec<Entry>>,
    /// Extension method name → (declaring namespace, declaring type's full name).
    extensions: BTreeMap<String, BTreeSet<(String, String)>>,
    /// Names of every field, property or method some type declares as its own — an extension of
    /// another type excluded. Cheap enough to ask "could a real repo type be the receiver here"
    /// without resolving one, for a value whose own declared type this file did not read.
    instance_members: BTreeSet<String>,
    /// `.cs` rel → its own file usings, so a base list another file wrote reads in that file's scope.
    usings: BTreeMap<String, Vec<Using>>,
    /// `.cs` rel → its `global using`s.
    global: BTreeMap<String, Vec<Using>>,
    /// Directory holding a `.csproj` (`""` at the root) → the project's root namespace.
    projects: BTreeMap<String, String>,
    /// Built on first use. `Resolver::new` adds every file before any extraction asks, so it is never stale.
    global_by_project: OnceLock<BTreeMap<Option<String>, Vec<Using>>>,
    /// `_Imports.razor` directory → its usings and its `@namespace`.
    razor_imports: BTreeMap<String, (Vec<Using>, Option<String>)>,
    /// `_ViewImports.cshtml` directory → its usings and its `@namespace`: the views' `_Imports.razor`.
    view_imports: BTreeMap<String, (Vec<Using>, Option<String>)>,
    /// Component rel → what the index keeps of it.
    components: BTreeMap<String, Component>,
    /// Built on first use, like `global_by_project`: component full name → its parts.
    components_by_full: OnceLock<BTreeMap<String, Vec<Part>>>,
}

impl DotNet {
    pub(crate) fn add_cs(&mut self, rel: &str, d: &Declared) {
        for t in &d.types {
            let part = Part { rel: rel.to_string(), local: t.local.clone(), full: t.full() };
            self.types.entry(t.full()).or_default().push(Entry { part, members: t.members.clone(), bases: t.bases.clone(), generic_bases: t.generic_bases.clone() });
            for m in &t.extensions {
                self.extensions.entry(m.clone()).or_default().insert((t.namespace.clone(), t.full()));
            }
            self.instance_members.extend(t.members.keys().filter(|m| !t.extensions.contains(*m)).cloned());
        }
        if !d.usings.is_empty() {
            self.usings.insert(rel.to_string(), d.usings.clone());
        }
        if !d.global_usings.is_empty() {
            self.global.insert(rel.to_string(), d.global_usings.clone());
        }
    }

    pub(crate) fn add_project(&mut self, rel: &str, text: &str) {
        static ROOT: OnceLock<regex::Regex> = OnceLock::new();
        let re = ROOT.get_or_init(|| regex::Regex::new(r"<RootNamespace>\s*([^<\s]+)\s*</RootNamespace>").unwrap());
        let stem = rel.rsplit('/').next().unwrap_or(rel).trim_end_matches(".csproj");
        // The SDK's default is `$(MSBuildProjectName.Replace(" ", "_"))`: spaces only.
        let root = re.captures(text).map_or_else(|| stem.replace(' ', "_"), |c| c[1].to_string());
        self.projects.insert(parent(rel).to_string(), root);
    }

    /// Every part of `full` outside `except`: the file being extracted reads its own parts from its
    /// tree, which is the one the extractor holds.
    pub fn parts(&self, full: &str, except: &str) -> Vec<&Part> {
        self.types.get(full).into_iter().flatten().map(|e| &e.part).filter(|p| p.rel != except).collect()
    }

    fn entry(&self, full: &str, rel: &str, local: &str) -> Option<&Entry> {
        self.types.get(full)?.iter().find(|e| e.part.rel == rel && e.part.local == local)
    }

    pub fn members(&self, full: &str, rel: &str, local: &str) -> Option<&Members> {
        match self.entry(full, rel, local) {
            Some(e) => Some(&e.members),
            None => self.component(full, rel, local).map(|c| &c.members),
        }
    }

    /// The component `rel` when it is the part `local` of `full`.
    fn component(&self, full: &str, rel: &str, local: &str) -> Option<&Component> {
        self.components.get(rel).filter(|_| self.component_parts(full).iter().any(|p| p.rel == rel && p.local == local))
    }

    /// The base-list names one part writes, unresolved: a component's `@inherits` among them.
    pub fn bases(&self, full: &str, rel: &str, local: &str) -> &[String] {
        match self.entry(full, rel, local) {
            Some(e) => &e.bases,
            None => self.component(full, rel, local).map_or(&[], |c| c.bases.as_slice()),
        }
    }

    /// Whether one part writes `base` with type arguments.
    pub fn generic_base(&self, full: &str, rel: &str, local: &str, base: &str) -> bool {
        match self.entry(full, rel, local) {
            Some(e) => e.generic_bases.contains(base),
            None => self.component(full, rel, local).is_some_and(|c| c.generic_bases.contains(base)),
        }
    }

    /// The file usings `rel` writes, its `global using`s excluded.
    pub fn usings(&self, rel: &str) -> &[Using] {
        self.usings.get(rel).map_or(&[], Vec::as_slice)
    }

    /// The usings a file's declarations read: a component's are its imports files' and its own `@using`s.
    pub fn file_usings(&self, rel: &str) -> Vec<Using> {
        match self.components.get(rel) {
            Some(c) => {
                let mut usings = self.razor_usings(rel);
                usings.extend(c.usings.iter().cloned());
                usings
            }
            None => self.usings(rel).to_vec(),
        }
    }

    /// Whether any type in the repository declares `member` as a field, property or method of its
    /// own — an extension of another type does not count.
    pub fn declares_instance_member(&self, member: &str) -> bool {
        self.instance_members.contains(member)
    }

    pub fn extensions(&self, method: &str) -> Vec<(String, String)> {
        self.extensions.get(method).map(|s| s.iter().cloned().collect()).unwrap_or_default()
    }

    /// The directory of the `.csproj` nearest above `rel`.
    pub fn project_of(&self, rel: &str) -> Option<&str> {
        let mut dir = parent(rel);
        loop {
            if let Some((d, _)) = self.projects.get_key_value(dir) {
                return Some(d.as_str());
            }
            if dir.is_empty() {
                return None;
            }
            dir = parent(dir);
        }
    }

    pub fn root_namespace(&self, rel: &str) -> Option<(&str, &str)> {
        let dir = self.project_of(rel)?;
        Some((dir, self.projects[dir].as_str()))
    }

    /// The `global using`s of every `.cs` file under the same nearest project as `rel`.
    pub fn global_usings(&self, rel: &str) -> Vec<Using> {
        let by = self.global_by_project.get_or_init(|| {
            let mut m: BTreeMap<Option<String>, Vec<Using>> = BTreeMap::new();
            for (r, usings) in &self.global {
                m.entry(self.project_of(r).map(str::to_string)).or_default().extend(usings.iter().cloned());
            }
            m
        });
        by.get(&self.project_of(rel).map(str::to_string)).cloned().unwrap_or_default()
    }

    /// `members` are what `razor::members` reads from the component's blocks and injects.
    pub(crate) fn add_razor(&mut self, rel: &str, d: &crate::code::razor::Directives, members: Members) {
        if d.unread {
            return;
        }
        if crate::code::razor::is_imports(rel) {
            let map = if is_view(rel) { &mut self.view_imports } else { &mut self.razor_imports };
            map.insert(parent(rel).to_string(), (d.usings.clone(), d.namespace.clone()));
        } else if crate::code::razor::component_name(rel).is_some() {
            self.instance_members.extend(members.keys().cloned());
            let generic_bases = d.inherits.iter().filter(|_| d.inherits_generic).cloned().collect();
            let component = Component { namespace: d.namespace.clone(), members, bases: d.inherits.iter().cloned().collect(), generic_bases, usings: d.usings.clone() };
            self.components.insert(rel.to_string(), component);
        }
    }

    /// The imports files a Razor file reads: `_ViewImports.cshtml` for a view, `_Imports.razor` for a component.
    fn imports_of(&self, rel: &str) -> &BTreeMap<String, (Vec<Using>, Option<String>)> {
        if is_view(rel) { &self.view_imports } else { &self.razor_imports }
    }

    /// `rel`'s folder and each above it, nearest first, up to its project's: the Razor compiler looks
    /// for imports files from the project root down, never above it.
    fn razor_dirs<'r>(&self, rel: &'r str) -> Vec<&'r str> {
        let top = self.project_of(rel).unwrap_or("");
        let mut dirs = Vec::new();
        let mut dir = parent(rel);
        loop {
            dirs.push(dir);
            if dir == top || dir.is_empty() {
                return dirs;
            }
            dir = parent(dir);
        }
    }

    /// The usings of every imports file over a Razor file, outermost first.
    pub fn razor_usings(&self, rel: &str) -> Vec<Using> {
        let imports = self.imports_of(rel);
        self.razor_dirs(rel).into_iter().rev().filter_map(|d| imports.get(d)).flat_map(|(u, _)| u.iter().cloned()).collect()
    }

    /// The namespace the Razor compiler gives a file's class: its own `@namespace`; else the nearest
    /// imports file's `@namespace` plus the folders below it; else, for a component, the project's
    /// root namespace plus the folders below the project, and for a view `VIEW_NAMESPACE`.
    pub fn razor_namespace(&self, rel: &str, own: Option<&str>) -> String {
        if let Some(ns) = own {
            return sanitised(ns);
        }
        let here = parent(rel);
        let imports = self.imports_of(rel);
        for dir in self.razor_dirs(rel) {
            if let Some((_, Some(ns))) = imports.get(dir) {
                return sanitised(&join(ns, &folders(dir, here)));
            }
        }
        if is_view(rel) {
            return VIEW_NAMESPACE.to_string();
        }
        match self.root_namespace(rel) {
            Some((project, root)) => sanitised(&join(root, &folders(project, here))),
            None => folders("", here),
        }
    }

    /// Every component whose computed full name is `full`.
    pub fn component_parts(&self, full: &str) -> Vec<Part> {
        let by = self.components_by_full.get_or_init(|| {
            let mut m: BTreeMap<String, Vec<Part>> = BTreeMap::new();
            for (rel, Component { namespace: own, .. }) in &self.components {
                let Some(stem) = crate::code::razor::component_name(rel) else { continue };
                let full = join(&self.razor_namespace(rel, own.as_deref()), &stem);
                m.entry(full.clone()).or_default().push(Part { rel: rel.clone(), local: stem, full });
            }
            m
        });
        by.get(full).cloned().unwrap_or_default()
    }
}
