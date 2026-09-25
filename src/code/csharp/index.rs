//! The .NET index: every C# type's declaring files and members, extension methods by name, each
//! project's root namespace and `global using`s — what `Scope` resolves a name against.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use super::declarations::{self, Declared, Using};
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

/// MSBuild's default root namespace keeps the project file's name, with every character an
/// identifier cannot hold replaced by `_`; the Razor compiler does the same to folder names.
pub(crate) fn identifier(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == '_' || c == '.' { c } else { '_' }).collect()
}

#[derive(Debug, Default)]
pub struct DotNet {
    /// `Namespace.Outer.Inner` → every part with its members.
    types: BTreeMap<String, Vec<(Part, Members)>>,
    /// Extension method name → (declaring namespace, declaring type's full name).
    extensions: BTreeMap<String, BTreeSet<(String, String)>>,
    /// `.cs` rel → its `global using`s.
    global: BTreeMap<String, Vec<Using>>,
    /// Directory holding a `.csproj` (`""` at the root) → the project's root namespace.
    projects: BTreeMap<String, String>,
    /// Built on first use. `Resolver::new` adds every file before any extraction asks, so it is never stale.
    global_by_project: OnceLock<BTreeMap<Option<String>, Vec<Using>>>,
}

impl DotNet {
    pub(crate) fn add_cs(&mut self, rel: &str, d: &Declared) {
        for t in &d.types {
            let part = Part { rel: rel.to_string(), local: t.local.clone(), full: t.full() };
            self.types.entry(t.full()).or_default().push((part, t.members.clone()));
            for m in &t.extensions {
                self.extensions.entry(m.clone()).or_default().insert((t.namespace.clone(), t.full()));
            }
        }
        if !d.global_usings.is_empty() {
            self.global.insert(rel.to_string(), d.global_usings.clone());
        }
    }

    pub(crate) fn add_project(&mut self, rel: &str, text: &str) {
        static ROOT: OnceLock<regex::Regex> = OnceLock::new();
        let re = ROOT.get_or_init(|| regex::Regex::new(r"<RootNamespace>\s*([^<\s]+)\s*</RootNamespace>").unwrap());
        let stem = rel.rsplit('/').next().unwrap_or(rel).trim_end_matches(".csproj");
        let root = re.captures(text).map_or_else(|| identifier(stem), |c| c[1].to_string());
        self.projects.insert(parent(rel).to_string(), root);
    }

    /// Every part of `full` outside `except`: the file being extracted reads its own parts from its
    /// tree, which is the one the extractor holds.
    pub fn parts(&self, full: &str, except: &str) -> Vec<&Part> {
        self.types.get(full).into_iter().flatten().filter(|(p, _)| p.rel != except).map(|(p, _)| p).collect()
    }

    pub fn members(&self, full: &str, rel: &str, local: &str) -> Option<&Members> {
        self.types.get(full)?.iter().find(|(p, _)| p.rel == rel && p.local == local).map(|(_, m)| m)
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

    // Read by Razor's namespace derivation; `expect` flags this once it is in the non-test build —
    // a case test already exercises it, so the same `expect` on the test-cfg build would never fire.
    #[cfg_attr(not(test), expect(dead_code))]
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
}
