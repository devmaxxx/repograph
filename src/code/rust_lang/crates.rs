//! The Rust module tree, read from file paths and `Cargo.toml` alone.
//!
//! Resolution never opens a file. `mod x;` can only name `x.rs` or `x/mod.rs` below its parent
//! module's directory, so the set of `.rs` paths already says which modules exist; what a path
//! cannot say — a crate's name, a moved `[lib] path` — comes from the manifests.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

static MACRO_RULES: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?m)^[ \t]*macro_rules![ \t]*([A-Za-z_]\w*)").expect("valid pattern"));

// rustfmt puts every top-level item at column 0 and every member below it indented, which is
// what lets a line scan tell a module's names from an impl's without a parse.
static TOP_ITEM: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r#"(?m)^(?:pub(?:\([^)\n]*\))?[ \t]+)?(?:(?:async|const|unsafe|extern[ \t]+"[^"\n]*")[ \t]+)*(?:fn|struct|enum|union|trait|type|const|static|mod)[ \t]+(?:mut[ \t]+)?([A-Za-z_]\w*)"#,
    )
    .expect("valid pattern")
});
static PUB_USE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?m)^pub(?:\([^)\n]*\))?[ \t]+use[ \t]+((?:[A-Za-z_]\w*::)*[A-Za-z_]\w*)(?:[ \t]+as[ \t]+([A-Za-z_]\w*))?[ \t]*;")
        .expect("valid pattern")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crate {
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
pub struct Crates {
    crates: Vec<Crate>,
    files: BTreeSet<String>,
    macros: BTreeMap<String, BTreeSet<String>>,
    tops: BTreeMap<String, BTreeSet<String>>,
    /// File → name a column-0 `pub use` binds → the path it re-exports.
    reexports: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

/// A module: the directory its crate root sits in, that root, and the file-module segments below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub dir: String,
    pub root: String,
    pub segs: Vec<String>,
}

/// Where a path lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A module: the file holding it and the inline modules inside that file, outermost first.
    Module { file: String, inline: Vec<String> },
    /// An item, named as it follows `sym:<file>::` — `Store`, `tests/helper`, `Store.new`.
    Item { file: String, name: String },
}

pub(crate) fn parent(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(d, _)| d)
}

fn join(dir: &str, rest: &str) -> String {
    let rest = rest.trim_start_matches("./");
    if dir.is_empty() { rest.to_string() } else { format!("{dir}/{rest}") }
}

/// Rust's naming lints keep types CamelCase and modules snake_case, and that is the only way a
/// path says where its modules end without opening the file it names.
pub(crate) fn is_type_name(seg: &str) -> bool {
    seg.starts_with(|c: char| c.is_ascii_uppercase()) && seg.chars().any(|c| c.is_ascii_lowercase())
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
        let names: BTreeSet<String> = TOP_ITEM.captures_iter(source).map(|c| c[1].to_string()).collect();
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

    // Only the unit test reads this until the call pass exists; that pass removes the attribute.
    #[cfg_attr(not(test), expect(dead_code))]
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
        self.crates.iter().filter(|k| k.dir.is_empty() || rel.starts_with(&format!("{}/", k.dir))).max_by_key(|k| k.dir.len())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn table(manifests: &[(&str, &str)], files: &[&str]) -> Crates {
        let mut c = Crates::default();
        for (rel, text) in manifests {
            c.manifest(rel, text);
        }
        // A path only lands on a name its file declares, so each fixture file declares the names the cases use.
        for f in files {
            c.file(f, "pub fn charge() {}\npub fn total() {}\npub fn add() {}\npub fn f() {}\npub struct Order;\npub struct Card;\npub struct Line;\nmod fixtures {}\n");
        }
        c
    }

    fn path(p: &str) -> Vec<String> {
        p.split("::").map(str::to_string).collect()
    }

    fn item(file: &str, name: &str) -> Option<Target> {
        Some(Target::Item { file: file.into(), name: name.into() })
    }

    fn shop() -> Crates {
        table(
            &[
                ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
                ("crates/shop-core/Cargo.toml", "[package]\nname = \"shop-core\"\nversion = \"0.1.0\"\n"),
                ("crates/shopd/Cargo.toml", "[package]\nname = \"shopd\"\nversion = \"0.1.0\"\n\n[[bin]]\nname = \"admin\"\npath = \"tools/admin.rs\"\n"),
            ],
            &[
                "crates/shop-core/src/lib.rs",
                "crates/shop-core/src/orders.rs",
                "crates/shop-core/src/orders/line.rs",
                "crates/shop-core/src/pay/mod.rs",
                "crates/shop-core/src/pay/card.rs",
                "crates/shop-core/tests/checkout.rs",
                "crates/shop-core/tests/common/mod.rs",
                "crates/shopd/src/lib.rs",
                "crates/shopd/src/main.rs",
                "crates/shopd/src/routes/mod.rs",
                "crates/shopd/src/routes/cart.rs",
                "crates/shopd/src/bin/seed.rs",
                "crates/shopd/tools/admin.rs",
            ],
        )
    }

    #[test]
    fn a_module_is_named_by_its_path_below_the_nearest_root() {
        let c = shop();
        assert_eq!(
            c.module_of("crates/shop-core/src/orders/line.rs"),
            Module { dir: "crates/shop-core/src".into(), root: "crates/shop-core/src/lib.rs".into(), segs: vec!["orders".into(), "line".into()] }
        );
        assert_eq!(c.module_of("crates/shop-core/src/pay/mod.rs").segs, vec!["pay".to_string()]);
        let common = c.module_of("crates/shop-core/tests/common/mod.rs");
        assert_eq!((common.dir.as_str(), common.segs.clone()), ("crates/shop-core/tests", vec!["common".to_string()]));
        // A crate with a library and a binary: a module under `src/` is the library's.
        assert_eq!(c.module_of("crates/shopd/src/routes/cart.rs").root, "crates/shopd/src/lib.rs");
        for root in ["crates/shopd/src/bin/seed.rs", "crates/shopd/tools/admin.rs", "crates/shop-core/tests/checkout.rs"] {
            assert_eq!(c.module_of(root), Module { dir: parent(root).into(), root: root.into(), segs: vec![] }, "{root}");
        }
    }

    #[test]
    fn crate_self_super_and_a_workspace_crate_resolve_through_the_tree() {
        let c = shop();
        let line = "crates/shop-core/src/orders/line.rs";
        assert_eq!(c.resolve(line, &[], &path("crate::pay::card::charge")), item("crates/shop-core/src/pay/card.rs", "charge"));
        assert_eq!(c.resolve(line, &[], &path("super::Order")), item("crates/shop-core/src/orders.rs", "Order"));
        assert_eq!(c.resolve("crates/shop-core/src/pay/card.rs", &[], &path("self::Card::new")), item("crates/shop-core/src/pay/card.rs", "Card.new"));
        assert_eq!(c.resolve("crates/shopd/src/routes/cart.rs", &[], &path("shop_core::orders::line::Line")), item("crates/shop-core/src/orders/line.rs", "Line"));
        // Uniform paths: a child module is named without `self::`.
        assert_eq!(c.resolve("crates/shopd/src/lib.rs", &[], &path("routes::cart::add")), item("crates/shopd/src/routes/cart.rs", "add"));
        assert_eq!(c.resolve("crates/shopd/src/lib.rs", &[], &path("crate::routes")), Some(Target::Module { file: "crates/shopd/src/routes/mod.rs".into(), inline: vec![] }));
        assert_eq!(c.resolve(line, &[], &path("serde::Serialize")), None, "an external crate resolves to nothing");
    }

    #[test]
    fn an_inline_module_nests_inside_its_file() {
        let c = shop();
        let orders = "crates/shop-core/src/orders.rs";
        let tests = vec!["tests".to_string()];
        assert_eq!(c.resolve(orders, &tests, &path("super::total")), item(orders, "total"));
        assert_eq!(c.resolve(orders, &tests, &path("super")), Some(Target::Module { file: orders.into(), inline: vec![] }));
        assert_eq!(c.resolve(orders, &[], &path("self::fixtures::order")), item(orders, "fixtures/order"));
        assert_eq!(c.resolve(orders, &[], &path("self::line::Line")), item("crates/shop-core/src/orders/line.rs", "Line"));
    }

    #[test]
    fn a_lib_table_renames_and_moves_the_library() {
        let c = table(&[("x/Cargo.toml", "[package]\nname = \"a-b\"\n\n[lib]\nname = \"ab_core\"\npath = \"./lib/core.rs\"\n")], &["x/lib/core.rs", "x/lib/util.rs", "y/main.rs"]);
        assert_eq!(c.module_of("x/lib/util.rs").segs, vec!["util".to_string()]);
        assert_eq!(c.resolve("x/lib/util.rs", &[], &path("ab_core::util::f")), item("x/lib/util.rs", "f"));
        assert_eq!(c.resolve("x/lib/util.rs", &[], &path("a_b::util::f")), None, "the package name is not the library's when [lib] names it");
        // A file no manifest reaches is a crate of its own: `mod x;` beside it still resolves.
        assert_eq!(c.module_of("y/main.rs").root, "y/main.rs");
    }

    #[test]
    fn a_macro_is_found_by_a_line_scan_within_its_crate() {
        let mut c = shop();
        c.file("crates/shop-core/src/orders.rs", "#[macro_export]\nmacro_rules! money {\n    () => {};\n}\n");
        assert_eq!(c.macro_files("crates/shop-core/src/pay/card.rs", "money"), vec!["crates/shop-core/src/orders.rs"]);
        assert!(c.macro_files("crates/shopd/src/main.rs", "money").is_empty(), "another crate needs a path to it");
    }

    #[test]
    fn item_names_join_modules_with_a_slash_and_members_with_a_dot() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(item_name(&[], &s(&["Store", "new"])), "Store.new");
        assert_eq!(item_name(&s(&["tests"]), &s(&["Helper", "build", "x"])), "tests/Helper.build");
        assert_eq!(item_name(&[], &s(&["fixtures", "order"])), "fixtures/order");
        assert_eq!(item_name(&[], &s(&["MAX_LEN"])), "MAX_LEN");
    }

    #[test]
    fn a_file_s_top_level_names_are_read_by_the_line_scan() {
        let mut c = Crates::default();
        c.file("src/walk.rs", "pub struct Manifest;\npub(crate) const fn limit() -> u32 { 1 }\nstatic mut N: u32 = 0;\nimpl Manifest {\n    pub fn load() {}\n}\n");
        for name in ["Manifest", "limit", "N"] {
            assert!(c.declares("src/walk.rs", name), "{name}");
        }
        assert!(!c.declares("src/walk.rs", "load"), "an indented member is not a top-level name");
    }

    #[test]
    fn a_path_through_a_crate_root_re_export_lands_on_the_declaring_file() {
        let mut c = Crates::default();
        c.manifest("Cargo.toml", "[package]\nname = \"shop\"\n");
        c.file("src/lib.rs", "mod store;\npub use store::Store;\npub mod ops;\n");
        c.file("src/store.rs", "pub struct Store;\nimpl Store {\n    pub fn new() -> Store { Store }\n}\n");
        c.file("src/ops.rs", "");
        assert_eq!(c.resolve("src/ops.rs", &[], &path("crate::Store")), item("src/store.rs", "Store"));
        assert_eq!(c.resolve("src/ops.rs", &[], &path("crate::Store::new")), item("src/store.rs", "Store.new"));
    }
}
