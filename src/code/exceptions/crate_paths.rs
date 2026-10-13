//! A Rust path names what the crate module tree says it does, and no query sees that tree: it is
//! read from file paths and `Cargo.toml` alone. This rule places each `use`, and resolves calls,
//! macro invocations, an `impl`'s type and trait, and attributes through what the scope binds.
//!
//! Resolution never opens a file. `mod x;` can only name `x.rs` or `x/mod.rs` below its parent
//! module's directory, so the set of `.rs` paths already says which modules exist; what a path
//! cannot say — a crate's name, a moved `[lib] path` — comes from the manifests.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use tree_sitter::Node;

use crate::code::engine::{Facts, File, Sink};
use crate::code::imports::{join, parent, Resolver};
use crate::code::syntax::{field_text, text};

mod calls;
mod uses;

static MACRO_RULES: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?m)^[ \t]*macro_rules![ \t]*([A-Za-z_]\w*)").expect("valid pattern"));

// rustfmt puts every top-level item at column 0 and every member below it indented, which is
// what lets a line scan tell a module's names from an impl's without a parse.
static TOP_ITEM: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r#"(?m)^(?:pub(?:\([^)\n]*\))?[ \t]+)?(?:(?:async|const|unsafe|extern[ \t]+"[^"\n]*")[ \t]+)*(?:fn|struct|enum|union|trait|type|const|static)[ \t]+(?:mut[ \t]+)?([A-Za-z_]\w*)"#,
    )
    .expect("valid pattern")
});
// An inline `mod x {` declares `x` in its file; a `mod x;` names a file, which `file_of` answers,
// and counting it here would let a path through a module whose file is absent land on a symbol nobody wrote.
static INLINE_MOD: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?m)^(?:pub(?:\([^)\n]*\))?[ \t]+)?mod[ \t]+([A-Za-z_]\w*)[ \t]*\{").expect("valid pattern")
});
static PUB_USE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?m)^pub(?:\([^)\n]*\))?[ \t]+use[ \t]+((?:[A-Za-z_]\w*::)*[A-Za-z_]\w*)(?:[ \t]+as[ \t]+([A-Za-z_]\w*))?[ \t]*;")
        .expect("valid pattern")
});

pub(crate) fn collect(r: &mut Resolver, rel: &str, source: &str) -> Option<crate::code::index::Header> {
    r.state_mut::<Crates>().file(rel, source);
    None
}

pub(crate) fn manifest(r: &mut Resolver, rel: &str, text: &str) {
    r.state_mut::<Crates>().manifest(rel, text);
}

pub(crate) fn resolve(r: &Resolver, file: &File, facts: &Facts, sink: &mut Sink) {
    let Some(tree) = file.tree else { return };
    let root = tree.root_node();
    let local = Local::of(file, facts);
    let mut ctx = uses::Ctx { rel: file.rel, src: file.src.as_bytes(), crates: r.state::<Crates>(), facts, local, bindings: uses::Bindings::default() };
    uses::read(&mut ctx, root, sink);
    calls::impls(&ctx, root, sink);
    calls::read(&ctx, root, sink);
    calls::attributes(&ctx, root, sink);
}

/// What the file declares, by id suffix, in the shapes a path is resolved against.
#[derive(Debug, Default)]
struct Local {
    /// Every id suffix: `f`, `S`, `S.new`, `tests/helper`.
    names: BTreeSet<String>,
    /// Each scope's directly declared names, keyed by its inline modules joined with `/` ("" is the file).
    scopes: BTreeMap<String, BTreeSet<String>>,
    /// Inline module paths, joined with `/`. A module that declares nothing is missing, and nothing can
    /// be reached through it.
    modules: BTreeSet<String>,
    /// `macro_rules!` name → its id suffix.
    macros: BTreeMap<String, String>,
}

impl Local {
    fn of(file: &File, facts: &Facts) -> Local {
        let own = format!("sym:{}::", file.rel);
        let mut local = Local::default();
        for d in &facts.defs {
            let Some(suffix) = d.id.strip_prefix(&own) else { continue };
            local.names.insert(suffix.to_string());
            let (scope, name) = suffix.rsplit_once('/').unwrap_or(("", suffix));
            if name.contains('.') {
                continue;
            }
            local.scopes.entry(scope.to_string()).or_default().insert(name.to_string());
            let mut module = scope;
            while !module.is_empty() {
                local.modules.insert(module.to_string());
                module = module.rsplit_once('/').map_or("", |(up, _)| up);
            }
            if file.src[d.range.clone()].starts_with("macro_rules!") {
                local.macros.insert(name.to_string(), suffix.to_string());
            }
        }
        local
    }
}

/// The inline modules around `n`, outermost first.
fn inline_of(n: Node, src: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = n;
    while let Some(p) = at.parent() {
        if p.kind() == "mod_item" {
            out.extend(field_text(p, "name", src).map(str::to_string));
        }
        at = p;
    }
    out.reverse();
    out
}

/// An item's id suffix inside inline modules: every segment but a member is joined with `/`.
fn scoped(inline: &[String], name: &str) -> String {
    if inline.is_empty() { name.to_string() } else { format!("{}/{name}", inline.join("/")) }
}

/// A type's path as written, generic arguments and references dropped: `&mut Tmux<E>` is `["Tmux"]`,
/// `crate::walk::Manifest` is `["crate", "walk", "Manifest"]`.
fn type_path(t: Node, src: &[u8]) -> Vec<String> {
    match t.kind() {
        "type_identifier" | "identifier" | "primitive_type" | "self" | "crate" | "super" => vec![text(t, src).to_string()],
        "generic_function" => t.child_by_field_name("function").map_or_else(Vec::new, |x| type_path(x, src)),
        "generic_type" | "reference_type" | "pointer_type" => t.child_by_field_name("type").map_or_else(Vec::new, |x| type_path(x, src)),
        "scoped_type_identifier" | "scoped_identifier" => {
            let mut p = t.child_by_field_name("path").map_or_else(Vec::new, |x| type_path(x, src));
            p.extend(field_text(t, "name", src).map(str::to_string));
            p
        }
        _ => Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Crate {
    /// As `use` spells it: `[lib] name`, else the package name, with `-` read as `_`.
    pub name: String,
    /// The manifest's directory; "" at the repository root.
    pub dir: String,
    /// The library root the manifest implies, whether or not that file exists.
    pub lib: String,
    /// `[[bin]] path` entries; the conventional roots are recognised by their shape.
    pub bins: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct Crates {
    crates: Vec<Crate>,
    files: BTreeSet<String>,
    macros: BTreeMap<String, BTreeSet<String>>,
    tops: BTreeMap<String, BTreeSet<String>>,
    /// File → name a column-0 `pub use` binds → the path it re-exports.
    reexports: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

/// A module: the directory its crate root sits in, that root, and the file-module segments below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Module {
    pub dir: String,
    pub root: String,
    pub segs: Vec<String>,
}

/// Where a path lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// A module: the file holding it and the inline modules inside that file, outermost first.
    Module { file: String, inline: Vec<String> },
    /// An item, named as it follows `sym:<file>::` — `Store`, `tests/helper`, `Store.new`.
    Item { file: String, name: String },
}

/// Rust's naming lints keep types upper-initial and modules snake_case, and that is the only way a
/// path says where its modules end without opening the file it names. A segment followed by
/// another cannot be a constant, so an acronym type (`IO`, `UUID`) is a type too.
pub(crate) fn is_type_name(seg: &str) -> bool {
    seg.starts_with(|c: char| c.is_ascii_uppercase())
}

/// The id suffix for `rest` inside inline modules `inline`: modules join with `/`, a type and its
/// associated item with `.` (spec L4). What follows the associated item — an enum variant's
/// field, say — is no symbol and is dropped.
pub(crate) fn item_name(inline: &[String], rest: &[String]) -> String {
    let mut parts: Vec<String> = inline.to_vec();
    for (i, seg) in rest.iter().enumerate() {
        if is_type_name(seg) && i + 1 < rest.len() {
            parts.push(format!("{seg}.{}", rest[i + 1]));
            break;
        }
        parts.push(seg.clone());
    }
    parts.join("/")
}

impl Crates {
    /// Every globbed `.rs` file, before extraction starts.
    pub fn file(&mut self, rel: &str, source: &str) {
        self.files.insert(rel.to_string());
        // A line scan, not a parse: `Resolver::new` reads every file, and a second parse of each
        // would double a build's parsing for one table.
        for c in MACRO_RULES.captures_iter(source) {
            self.macros.entry(c[1].to_string()).or_default().insert(rel.to_string());
        }
        let names: BTreeSet<String> = TOP_ITEM.captures_iter(source).chain(INLINE_MOD.captures_iter(source)).map(|c| c[1].to_string()).collect();
        if !names.is_empty() {
            self.tops.insert(rel.to_string(), names);
        }
        for c in PUB_USE.captures_iter(source) {
            let path: Vec<String> = c[1].split("::").map(str::to_string).collect();
            let bound = c.get(2).map_or_else(|| path.last().cloned().unwrap_or_default(), |a| a.as_str().to_string());
            if bound != "_" {
                self.reexports.entry(rel.to_string()).or_default().insert(bound, path);
            }
        }
    }

    /// Every `Cargo.toml`. A `[workspace]`-only manifest names no crate: its members have
    /// manifests of their own, and the walk reaches those too.
    pub fn manifest(&mut self, rel: &str, text: &str) {
        let Ok(table) = toml::from_str::<toml::Table>(text) else { return };
        let Some(package) = table.get("package").and_then(|p| p.as_table()) else { return };
        let Some(package_name) = package.get("name").and_then(|v| v.as_str()) else { return };
        let dir = parent(rel).to_string();
        let lib = table.get("lib").and_then(|l| l.as_table());
        let name = lib.and_then(|l| l.get("name")).and_then(|v| v.as_str()).unwrap_or(package_name);
        let lib_path = lib.and_then(|l| l.get("path")).and_then(|v| v.as_str()).map_or_else(|| join(&dir, "src/lib.rs"), |p| join(&dir, p));
        let bins = table
            .get("bin")
            .and_then(|b| b.as_array())
            .map(|bins| bins.iter().filter_map(|b| b.get("path").and_then(|p| p.as_str())).map(|p| join(&dir, p)).collect())
            .unwrap_or_default();
        self.crates.push(Crate { name: name.replace('-', "_"), dir, lib: lib_path, bins });
    }

    /// Files declaring `macro_rules! name` in the crate holding `rel`, sorted.
    pub fn macro_files(&self, rel: &str, name: &str) -> Vec<&str> {
        let here = self.crate_of(rel).map(|k| k.dir.as_str());
        self.macros
            .get(name)
            .into_iter()
            .flatten()
            .filter(|f| self.crate_of(f).map(|k| k.dir.as_str()) == here)
            .map(String::as_str)
            .collect()
    }

    fn crate_of(&self, rel: &str) -> Option<&Crate> {
        self.crates
            .iter()
            .filter(|k| k.dir.is_empty() || rel.strip_prefix(k.dir.as_str()).is_some_and(|r| r.starts_with('/')))
            .max_by_key(|k| k.dir.len())
    }

    fn is_root(&self, rel: &str) -> bool {
        let Some(k) = self.crate_of(rel) else { return false };
        if k.lib == rel || k.bins.iter().any(|b| b == rel) {
            return true;
        }
        let local = if k.dir.is_empty() { rel } else { &rel[k.dir.len() + 1..] };
        let segs: Vec<&str> = local.split('/').collect();
        match segs.as_slice() {
            ["src", "main.rs"] | ["build.rs"] => true,
            ["src", "bin", f] | ["tests", f] | ["benches", f] | ["examples", f] => f.ends_with(".rs"),
            ["src", "bin", _, "main.rs"] | ["tests", _, "main.rs"] | ["benches", _, "main.rs"] | ["examples", _, "main.rs"] => true,
            _ => false,
        }
    }

    /// The root whose directory is `dir`: the library first, because a module under `src/` is the
    /// library's when a crate has both, then `main.rs`, then the first by name — every test root
    /// in `tests/` shares that directory, so which one is named changes no path.
    fn root_in(&self, dir: &str) -> Option<String> {
        if let Some(k) = self.crates.iter().find(|k| parent(&k.lib) == dir && self.files.contains(&k.lib)) {
            return Some(k.lib.clone());
        }
        let main = join(dir, "main.rs");
        if self.files.contains(&main) && self.is_root(&main) {
            return Some(main);
        }
        let prefix = join(dir, "");
        self.files.range(prefix.clone()..).take_while(|f| f.starts_with(&prefix)).find(|f| parent(f) == dir && self.is_root(f)).cloned()
    }

    pub fn module_of(&self, rel: &str) -> Module {
        let own = || Module { dir: parent(rel).to_string(), root: rel.to_string(), segs: Vec::new() };
        if self.is_root(rel) {
            return own();
        }
        let floor = self.crate_of(rel).map(|k| k.dir.clone());
        let mut dir = parent(rel).to_string();
        loop {
            if let Some(root) = self.root_in(&dir) {
                let below = if dir.is_empty() { rel } else { &rel[dir.len() + 1..] };
                let mut segs: Vec<String> = below.trim_end_matches(".rs").split('/').map(str::to_string).collect();
                if segs.last().is_some_and(|s| s == "mod") {
                    segs.pop();
                }
                return Module { dir, root, segs };
            }
            // A module never belongs to a root above its own crate's manifest.
            if dir.is_empty() || floor.as_deref() == Some(dir.as_str()) {
                break;
            }
            dir = parent(&dir).to_string();
        }
        own()
    }

    pub fn file_of(&self, m: &Module) -> Option<String> {
        if m.segs.is_empty() {
            return self.files.contains(&m.root).then(|| m.root.clone());
        }
        let base = join(&m.dir, &m.segs.join("/"));
        [format!("{base}.rs"), format!("{base}/mod.rs")].into_iter().find(|p| self.files.contains(p))
    }

    /// `path` as written at a site in `rel` inside inline modules `inline`. Resolves `crate`,
    /// `self`, `super`, a workspace crate's name and a child file module, and follows one `pub use`
    /// to the declaring file; any other first segment — an external crate, a name the file itself
    /// binds — is the caller's to try first.
    pub fn resolve(&self, rel: &str, inline: &[String], path: &[String]) -> Option<Target> {
        self.resolve_from(rel, inline, path, true)
    }

    /// Whether `file` declares `name` at its top level, read for a glob import of another file,
    /// whose names nothing else here knows.
    pub fn declares(&self, file: &str, name: &str) -> bool {
        self.tops.get(file).is_some_and(|n| n.contains(name))
    }

    /// Whether `file` re-exports `name` with a column-0 `pub use`, so a glob of it binds a name it does not declare.
    pub fn reexports(&self, file: &str, name: &str) -> bool {
        self.reexports.get(file).is_some_and(|r| r.contains_key(name))
    }

    fn resolve_from(&self, rel: &str, inline: &[String], path: &[String], hop: bool) -> Option<Target> {
        let here = self.module_of(rel);
        let first = path.first()?;
        let (mut module, scope, mut rest): (Module, Vec<String>, &[String]) = match first.as_str() {
            "crate" => (Module { segs: Vec::new(), ..here }, Vec::new(), &path[1..]),
            "self" => (here, inline.to_vec(), &path[1..]),
            "super" => {
                let n = path.iter().take_while(|s| *s == "super").count();
                let (mut module, mut scope) = (here, inline.to_vec());
                for _ in 0..n {
                    // An inline module's parent is the enclosing scope of the same file.
                    if scope.pop().is_none() {
                        module.segs.pop()?;
                    }
                }
                (module, scope, &path[n..])
            }
            name => match self.crates.iter().find(|k| k.name == name && self.files.contains(&k.lib)) {
                Some(k) => (Module { dir: parent(&k.lib).to_string(), root: k.lib.clone(), segs: Vec::new() }, Vec::new(), &path[1..]),
                None if inline.is_empty() => {
                    let child = Module { segs: [here.segs.clone(), vec![name.to_string()]].concat(), ..here.clone() };
                    self.file_of(&child)?;
                    (here, Vec::new(), path)
                }
                None => return None,
            },
        };
        // Only until an inline module is entered can a segment still be a file.
        while scope.is_empty() {
            let Some(next) = rest.first() else { break };
            let child = Module { segs: [module.segs.clone(), vec![next.clone()]].concat(), ..module.clone() };
            if self.file_of(&child).is_none() {
                break;
            }
            module = child;
            rest = &rest[1..];
        }
        let file = self.file_of(&module)?;
        if rest.is_empty() {
            return Some(Target::Module { file, inline: scope });
        }
        // A file that re-exports a name holds no symbol for it: `crate::Store` through
        // `pub use store::Store;` is `src/store.rs`'s `Store`. One hop, so a cycle cannot loop.
        if hop && scope.is_empty() && !self.declares(&file, &rest[0]) {
            if let Some(via) = self.reexports.get(&file).and_then(|r| r.get(&rest[0])) {
                return self.resolve_from(&file, &[], &[via.clone(), rest[1..].to_vec()].concat(), false);
            }
        }
        // No symbol is written for a name the file does not declare — a braced or glob re-export,
        // a second hop — so an item id there would name a node no file has.
        if scope.is_empty() && !self.declares(&file, &rest[0]) {
            return None;
        }
        Some(Target::Item { file, name: item_name(&scope, rest) })
    }
}
