use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// (directory the tsconfig lives in, pattern -> targets), sorted nearest-first.
type PathsTier = Vec<(String, Vec<(String, Vec<String>)>)>;

pub struct Resolver {
    repo: PathBuf,
    paths: PathsTier,
    /// package name -> (package dir, exports subpath -> target)
    packages: BTreeMap<String, (String, BTreeMap<String, String>)>,
}

#[derive(Deserialize, Default)]
struct TsConfig {
    #[serde(default, rename = "compilerOptions")]
    compiler_options: CompilerOptions,
}

#[derive(Deserialize, Default)]
struct CompilerOptions {
    #[serde(default)]
    paths: BTreeMap<String, Vec<String>>,
}

#[derive(Deserialize, Default)]
struct PackageJson {
    name: Option<String>,
    #[serde(default)]
    exports: serde_json::Value,
}

fn trailing_comma_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r",(\s*[}\]])").unwrap())
}

fn strip_jsonc(text: &str) -> String {
    let no_comments: String = text
        .lines()
        .map(|l| {
            // A `//` inside a string literal would be cut too; tsconfig paths never contain one,
            // but `"https://…"` values do, so an odd quote count before `//` means "inside a string".
            match l.find("//") {
                Some(i) if !l[..i].contains('"') || l[..i].matches('"').count() % 2 == 0 => &l[..i],
                _ => l,
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    trailing_comma_re().replace_all(&no_comments, "$1").into_owned()
}

fn export_target(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(m) => {
            ["types", "import", "default", "require"].iter().find_map(|k| m.get(*k).and_then(export_target))
        }
        _ => None,
    }
}

impl Resolver {
    pub fn new(repo: &Path) -> Result<Resolver> {
        let mut paths: PathsTier = Vec::new();
        let mut packages = BTreeMap::new();
        for dent in ignore::WalkBuilder::new(repo).hidden(true).git_ignore(true).build().flatten() {
            let p = dent.path();
            let Some(name) = p.file_name().and_then(|n| n.to_str()) else { continue };
            let rel_dir = p
                .parent()
                .unwrap()
                .strip_prefix(repo)
                .unwrap_or(Path::new(""))
                .to_string_lossy()
                .replace('\\', "/");
            if name == "tsconfig.json" {
                let text = std::fs::read_to_string(p).with_context(|| p.display().to_string())?;
                let cfg: TsConfig = serde_json::from_str(&strip_jsonc(&text)).unwrap_or_default();
                if !cfg.compiler_options.paths.is_empty() {
                    paths.push((rel_dir, cfg.compiler_options.paths.into_iter().collect()));
                }
            } else if name == "package.json" && !rel_dir.contains("node_modules") {
                let text = std::fs::read_to_string(p)?;
                if let Ok(pkg) = serde_json::from_str::<PackageJson>(&text) {
                    if let Some(pkg_name) = pkg.name {
                        let mut map = BTreeMap::new();
                        match &pkg.exports {
                            serde_json::Value::Object(m) => {
                                for (k, v) in m {
                                    if let Some(t) = export_target(v) {
                                        map.insert(k.clone(), t);
                                    }
                                }
                            }
                            other => {
                                if let Some(t) = export_target(other) {
                                    map.insert(".".into(), t);
                                }
                            }
                        }
                        packages.insert(pkg_name, (rel_dir, map));
                    }
                }
            }
        }
        // Nearest tsconfig to the importing file wins: sort deepest directory first.
        paths.sort_by_key(|a| std::cmp::Reverse(a.0.len()));
        Ok(Resolver { repo: repo.to_path_buf(), paths, packages })
    }

    /// A resolved candidate is only useful if it is a node the walker actually indexes:
    /// `.ts`/`.tsx` source, never a `dist/` build artifact or `node_modules` — otherwise
    /// `resolve` would point an edge at a file with no corresponding graph node.
    fn is_indexed(&self, rel: &str) -> bool {
        let ext_ok = matches!(Path::new(rel).extension().and_then(|e| e.to_str()), Some("ts") | Some("tsx"));
        ext_ok
            && !Path::new(rel)
                .components()
                .any(|c| matches!(c.as_os_str().to_str(), Some("dist") | Some("node_modules")))
            && self.repo.join(rel).is_file()
    }

    fn exists(&self, rel: &str) -> Option<String> {
        let stem = rel.trim_end_matches(".js").trim_end_matches(".jsx").trim_end_matches(".mjs");
        let stem = stem.replace("/dist/", "/src/");
        let stem = stem.strip_suffix(".d.ts").unwrap_or(&stem).to_string();
        let candidates = [
            stem.clone(),
            format!("{stem}.ts"),
            format!("{stem}.tsx"),
            format!("{stem}/index.ts"),
            format!("{stem}/index.tsx"),
        ];
        candidates.into_iter().find(|c| self.is_indexed(c))
    }

    pub fn resolve(&self, from_rel: &str, spec: &str) -> Option<String> {
        if spec.starts_with('.') {
            return self.exists(&crate::doc::links::normalise(from_rel, spec));
        }
        if spec.starts_with("node:") {
            return None;
        }
        for (dir, patterns) in &self.paths {
            if !dir.is_empty() && !from_rel.starts_with(&format!("{dir}/")) {
                continue;
            }
            for (pat, targets) in patterns {
                let matched = match pat.strip_suffix('*') {
                    Some(prefix) => spec.strip_prefix(prefix).map(str::to_string),
                    None if pat == spec => Some(String::new()),
                    None => None,
                };
                let Some(rest) = matched else { continue };
                for t in targets {
                    let t = t.replace('*', &rest);
                    let joined = crate::doc::links::normalise(&format!("{dir}/x"), &t);
                    if let Some(hit) = self.exists(&joined) {
                        return Some(hit);
                    }
                }
            }
        }
        for (name, (dir, exports)) in &self.packages {
            let Some(rest) = spec.strip_prefix(name.as_str()) else { continue };
            let sub = if rest.is_empty() { ".".to_string() } else { format!(".{rest}") };
            let Some(t) = exports.get(&sub) else { continue };
            let joined = crate::doc::links::normalise(&format!("{dir}/x"), t);
            if let Some(hit) = self.exists(&joined) {
                return Some(hit);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let w = |p: &str, c: &str| {
            let full = d.path().join(p);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, c).unwrap();
        };
        w("tsconfig.json", r#"{ "compilerOptions": { "paths": {
            "@beauty-crm/contracts": ["./packages/contracts/src/index.ts"],
            "@beauty-crm/contracts/*": ["./packages/contracts/src/*"] } } }"#);
        w("packages/contracts/package.json", r#"{ "name": "@beauty-crm/contracts", "exports": {
            ".": { "types": "./dist/index.d.ts", "import": "./dist/index.js" },
            "./business": { "import": "./dist/business.js" } } }"#);
        w("packages/contracts/src/index.ts", "export * from './money.js';\n");
        w("packages/contracts/src/money.ts", "export const asGrosze = 1;\n");
        w("packages/contracts/src/business.ts", "export const b = 1;\n");
        w("packages/ui/tsconfig.json", r#"{ "compilerOptions": { "paths": { "@/*": ["./src/*"] } } }"#);
        w("packages/ui/src/button/index.tsx", "export const Button = 1;\n");
        w("packages/ui/src/app.tsx", "import { Button } from '@/button';\n");
        w("apps/api/src/modules/staff/staff.controller.ts", "");
        w("apps/api/src/shared/audit/index.ts", "");
        d
    }

    #[test]
    fn relative_with_js_suffix_and_index() {
        let d = repo();
        let r = Resolver::new(d.path()).unwrap();
        assert_eq!(
            r.resolve("apps/api/src/modules/staff/staff.controller.ts", "../../shared/audit/index.js").as_deref(),
            Some("apps/api/src/shared/audit/index.ts")
        );
        assert_eq!(
            r.resolve("apps/api/src/modules/staff/staff.controller.ts", "../../shared/audit").as_deref(),
            Some("apps/api/src/shared/audit/index.ts")
        );
        assert_eq!(r.resolve("packages/contracts/src/index.ts", "./money.js").as_deref(), Some("packages/contracts/src/money.ts"));
    }

    #[test]
    fn tsconfig_paths_root_and_nearest() {
        let d = repo();
        let r = Resolver::new(d.path()).unwrap();
        assert_eq!(r.resolve("apps/api/src/x.ts", "@beauty-crm/contracts").as_deref(), Some("packages/contracts/src/index.ts"));
        assert_eq!(r.resolve("apps/api/src/x.ts", "@beauty-crm/contracts/money").as_deref(), Some("packages/contracts/src/money.ts"));
        assert_eq!(r.resolve("packages/ui/src/app.tsx", "@/button").as_deref(), Some("packages/ui/src/button/index.tsx"));
    }

    #[test]
    fn package_exports_map_dist_to_src() {
        let d = repo();
        std::fs::remove_file(d.path().join("tsconfig.json")).unwrap();
        let r = Resolver::new(d.path()).unwrap();
        assert_eq!(r.resolve("apps/api/src/x.ts", "@beauty-crm/contracts/business").as_deref(), Some("packages/contracts/src/business.ts"));
        assert_eq!(r.resolve("apps/api/src/x.ts", "@beauty-crm/contracts").as_deref(), Some("packages/contracts/src/index.ts"));
    }

    #[test]
    fn external_packages_are_none() {
        let d = repo();
        let r = Resolver::new(d.path()).unwrap();
        assert_eq!(r.resolve("apps/api/src/x.ts", "@nestjs/common"), None);
        assert_eq!(r.resolve("apps/api/src/x.ts", "node:fs"), None);
    }
}
