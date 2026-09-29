//! Python module names, read from file paths and project manifests alone, so resolving an
//! import never opens the module it names.

use std::collections::BTreeSet;

#[derive(Debug, Default)]
pub struct Modules {
    files: BTreeSet<String>,
    /// Directories holding `pyproject.toml`, `setup.py` or `setup.cfg`.
    roots: BTreeSet<String>,
}

fn parent(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(d, _)| d)
}

fn join(dir: &str, rest: &str) -> String {
    if dir.is_empty() {
        rest.to_string()
    } else {
        format!("{dir}/{rest}")
    }
}

impl Modules {
    /// Every globbed `.py` file, before extraction starts.
    pub fn file(&mut self, rel: &str) {
        self.files.insert(rel.to_string());
    }

    /// Every `pyproject.toml`, `setup.py` and `setup.cfg`: its directory is a root. Nothing in
    /// the file is read, so a `package-dir` naming another directory resolves nothing.
    pub fn manifest(&mut self, rel: &str) {
        self.roots.insert(parent(rel).to_string());
    }

    /// The file `import dotted` names from `rel`. Roots are tried in the order Python would
    /// find them: the importing file's directory (a script's `sys.path[0]`), each manifest
    /// directory above it nearest first, then the repository root.
    #[cfg_attr(not(test), expect(dead_code, reason = "read by the Python reference pass, not yet written"))]
    pub fn absolute(&self, rel: &str, dotted: &str) -> Option<String> {
        let mut bases = vec![parent(rel).to_string()];
        let mut dir = parent(rel);
        loop {
            if self.roots.contains(dir) && !bases.iter().any(|b| b == dir) {
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
        bases.iter().find_map(|b| self.module_file(b, dotted).or_else(|| self.src_layout(b, dotted)))
    }

    /// A project that keeps its packages under `src/` is imported by the package's name, as an
    /// installed one is. Only a manifest's directory gets the second try, and only after the
    /// package was not found beside the manifest.
    fn src_layout(&self, base: &str, dotted: &str) -> Option<String> {
        self.roots.contains(base).then(|| self.module_file(&join(base, "src"), dotted)).flatten()
    }

    /// The file `from <level dots><dotted> import ...` names from `rel`: one dot is the package
    /// holding `rel`, each further dot its parent. An empty `dotted` is that package itself.
    #[cfg_attr(not(test), expect(dead_code, reason = "read by the Python reference pass, not yet written"))]
    pub fn relative(&self, rel: &str, level: usize, dotted: &str) -> Option<String> {
        let mut dir = parent(rel).to_string();
        for _ in 1..level {
            if dir.is_empty() {
                return None;
            }
            dir = parent(&dir).to_string();
        }
        if dotted.is_empty() {
            let init = join(&dir, "__init__.py");
            return self.files.contains(&init).then_some(init);
        }
        self.module_file(&dir, dotted)
    }

    fn module_file(&self, base: &str, dotted: &str) -> Option<String> {
        let path = join(base, &dotted.replace('.', "/"));
        [format!("{path}.py"), format!("{path}/__init__.py")].into_iter().find(|p| self.files.contains(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Modules {
        let mut m = Modules::default();
        for f in [
            "bench/compare/run.py",
            "bench/compare/truth.py",
            "lib/shop/__init__.py",
            "lib/shop/orders.py",
            "lib/shop/pay/__init__.py",
            "lib/shop/pay/card.py",
            "lib/app/main.py",
            "top.py",
        ] {
            m.file(f);
        }
        m.manifest("lib/pyproject.toml");
        m
    }

    #[test]
    fn an_import_names_a_module_or_a_package_from_the_nearest_root() {
        let m = table();
        assert_eq!(m.absolute("bench/compare/run.py", "truth").as_deref(), Some("bench/compare/truth.py"), "a sibling, as a script imports it");
        assert_eq!(m.absolute("lib/app/main.py", "shop.pay.card").as_deref(), Some("lib/shop/pay/card.py"), "from the project root");
        assert_eq!(m.absolute("lib/app/main.py", "shop.pay").as_deref(), Some("lib/shop/pay/__init__.py"));
        assert_eq!(m.absolute("lib/app/main.py", "top").as_deref(), Some("top.py"), "from the repository root");
        assert_eq!(m.absolute("bench/compare/run.py", "os"), None, "the standard library is not in the repository");
    }

    #[test]
    fn a_relative_import_climbs_packages() {
        let m = table();
        assert_eq!(m.relative("lib/shop/pay/card.py", 1, "").as_deref(), Some("lib/shop/pay/__init__.py"));
        assert_eq!(m.relative("lib/shop/pay/card.py", 2, "orders").as_deref(), Some("lib/shop/orders.py"));
        assert_eq!(m.relative("lib/shop/orders.py", 1, "pay.card").as_deref(), Some("lib/shop/pay/card.py"));
        assert_eq!(m.relative("top.py", 2, "x"), None, "no package above the repository root");
    }

    #[test]
    fn a_src_layout_project_is_imported_by_its_package_name() {
        let mut m = Modules::default();
        for f in [
            "svc/src/billing/__init__.py",
            "svc/src/billing/invoice.py",
            "svc/src/util.py",
            "svc/util.py",
            "svc/tests/test_invoice.py",
            "loose/src/orphan.py",
            "loose/main.py",
        ] {
            m.file(f);
        }
        m.manifest("svc/pyproject.toml");
        let test = "svc/tests/test_invoice.py";
        assert_eq!(m.absolute(test, "billing.invoice").as_deref(), Some("svc/src/billing/invoice.py"));
        assert_eq!(m.absolute(test, "billing").as_deref(), Some("svc/src/billing/__init__.py"));
        assert_eq!(m.absolute(test, "util").as_deref(), Some("svc/util.py"), "a module beside the manifest wins over src/");
        assert_eq!(m.absolute("loose/main.py", "orphan"), None, "src/ is tried only in a directory holding a manifest");
    }
}
