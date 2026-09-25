use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::code::index::{Header, QualifiedIndex};
use crate::code::lang::{Family, Lang};

/// (directory the tsconfig lives in, directory its targets are relative to — `baseUrl` —
/// and pattern -> targets), sorted nearest-first.
type PathsTier = Vec<(String, String, Vec<(String, Vec<String>)>)>;

pub struct Resolver {
    repo: PathBuf,
    paths: PathsTier,
    /// package name -> (package dir, exports subpath -> target)
    packages: BTreeMap<String, (String, BTreeMap<String, String>)>,
    /// Qualified name -> declaring files, one index per name-indexed family the globs reach.
    indexes: BTreeMap<Family, QualifiedIndex>,
    /// .NET: C# types with their members, extension methods, projects and their `global using`s.
    dotnet: crate::code::csharp::index::DotNet,
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
    #[serde(default, rename = "baseUrl")]
    base_url: Option<String>,
}

#[derive(Deserialize, Default)]
struct PackageJson {
    name: Option<String>,
    #[serde(default)]
    exports: serde_json::Value,
    types: Option<String>,
    typings: Option<String>,
    module: Option<String>,
    main: Option<String>,
}

fn trailing_comma_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r",(\s*[}\]])").unwrap())
}

fn block_comment_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?s)/\*.*?\*/").unwrap())
}

fn strip_jsonc(text: &str) -> String {
    let no_blocks = block_comment_re().replace_all(text, "");
    let no_comments: String = no_blocks
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

/// Every top-level name of a header under each scope it names. The header does not pair a name
/// with its scope, so a file holding two namespaces lists each name under both: the index names
/// every candidate file, a superset. `directives` stay out: they name what a file reads from, not
/// what it declares.
fn index_header(indexes: &mut BTreeMap<Family, QualifiedIndex>, family: Family, rel: &str, header: &Header) {
    let index = indexes.entry(family).or_default();
    for name in &header.top {
        if header.scope.is_empty() {
            index.insert(name, rel);
        }
        for scope in &header.scope {
            index.insert(&format!("{scope}.{name}"), rel);
        }
    }
}

/// The family a build manifest speaks for, by its file name.
fn manifest_family(name: &str) -> Option<Family> {
    match name {
        "Cargo.toml" => Some(Family::Rust),
        "pubspec.yaml" => Some(Family::Dart),
        "pyproject.toml" | "setup.cfg" | "setup.py" => Some(Family::Python),
        n if n.ends_with(".csproj") => Some(Family::DotNet),
        _ => None,
    }
}

impl Resolver {
    pub fn new(repo: &Path, cfg: &crate::config::Config) -> Result<Resolver> {
        let code = crate::walk::globs(&cfg.code_globs)?;
        let skip = crate::walk::globs(&cfg.skip)?;
        let mut paths: PathsTier = Vec::new();
        let mut packages = BTreeMap::new();
        let mut sources: Vec<(Lang, String, PathBuf)> = Vec::new();
        let mut manifests: Vec<(Family, String, PathBuf)> = Vec::new();
        let mut reached: BTreeSet<Family> = BTreeSet::new();
        for dent in ignore::WalkBuilder::new(repo).hidden(true).git_ignore(true).build().flatten() {
            let p = dent.path();
            let Some(name) = p.file_name().and_then(|n| n.to_str()) else { continue };
            let rel = p.strip_prefix(repo).unwrap_or(p).to_string_lossy().replace('\\', "/");
            if dent.file_type().is_some_and(|t| t.is_file()) && !skip.is_match(&rel) {
                if code.is_match(&rel) {
                    if let Some(lang) = Lang::of(&rel) {
                        reached.insert(lang.family());
                        // TypeScript's state is the tsconfig and package.json read below; its sources
                        // are the extractor's alone, so a TypeScript repository opens nothing more here.
                        if lang.family() != Family::TypeScript {
                            sources.push((lang, rel.clone(), p.to_path_buf()));
                        }
                    }
                }
                if let Some(family) = manifest_family(name) {
                    manifests.push((family, rel.clone(), p.to_path_buf()));
                }
            }
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
                    let base = match cfg.compiler_options.base_url {
                        Some(b) => crate::doc::links::normalise(&format!("{rel_dir}/x"), &b),
                        None => rel_dir.clone(),
                    };
                    paths.push((rel_dir, base, cfg.compiler_options.paths.into_iter().collect()));
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
                        // No `exports`: the entry point is whichever legacy field is set.
                        if map.is_empty() {
                            if let Some(t) = [pkg.types, pkg.typings, pkg.module, pkg.main].into_iter().flatten().next() {
                                map.insert(".".into(), t);
                            }
                        }
                        packages.insert(pkg_name, (rel_dir, map));
                    }
                }
            }
        }
        // Nearest tsconfig to the importing file wins: sort deepest directory first.
        paths.sort_by_key(|a| std::cmp::Reverse(a.0.len()));
        let mut resolver = Resolver { repo: repo.to_path_buf(), paths, packages, indexes: BTreeMap::new(), dotnet: Default::default() };
        // Manifests before sources: a path family's roots decide how its sources' paths read.
        for (_, rel, path) in manifests.iter().filter(|(f, _, _)| reached.contains(f)) {
            if let Ok(text) = std::fs::read_to_string(path) {
                resolver.collect_manifest(rel, &text);
            }
        }
        for (lang, rel, path) in &sources {
            // Not UTF-8 is a binary, which the extractor skips as well.
            if let Ok(text) = std::fs::read_to_string(path) {
                resolver.collect(*lang, rel, &text);
            }
        }
        Ok(resolver)
    }

    /// The index of a name-indexed family the globs reach; `None` for every other family.
    // Read by the name-indexed families' resolution, which their plans add.
    #[allow(dead_code)]
    pub fn index(&self, family: Family) -> Option<&QualifiedIndex> {
        self.indexes.get(&family)
    }

    pub fn dotnet(&self) -> &crate::code::csharp::index::DotNet {
        &self.dotnet
    }

    /// What one globbed source contributes before any file is extracted. A name-indexed family's
    /// header goes into its index; a path family's plan adds its arm below, for state of its own.
    fn collect(&mut self, lang: Lang, rel: &str, source: &str) {
        if let Some(header) = crate::code::index::header_for(lang, rel, source) {
            index_header(&mut self.indexes, lang.family(), rel, &header);
        }
        match lang {
            Lang::CSharp => self.dotnet.add_cs(rel, &crate::code::csharp::index::facts(rel, source)),
            Lang::Razor => self.dotnet.add_razor(rel, &crate::code::razor::directives(source)),
            _ => {}
        }
    }

    /// What a build manifest contributes; called only when the globs reach the manifest's family.
    /// Each path family's plan adds its arm; until one lands, no family reads a manifest.
    fn collect_manifest(&mut self, rel: &str, text: &str) {
        if rel.ends_with(".csproj") {
            self.dotnet.add_project(rel, text);
        }
    }

    /// A resolved candidate is only useful if it is a node the walker actually indexes:
    /// `.ts`/`.tsx` source, never a `dist/` build artifact or `node_modules` — otherwise
    /// `resolve` would point an edge at a file with no corresponding graph node.
    fn is_indexed(&self, rel: &str) -> bool {
        let ext_ok = matches!(
            Path::new(rel).extension().and_then(|e| e.to_str()),
            Some("ts") | Some("tsx") | Some("js") | Some("jsx") | Some("mjs") | Some("cjs")
        );
        ext_ok
            && !Path::new(rel)
                .components()
                .any(|c| matches!(c.as_os_str().to_str(), Some("dist") | Some("node_modules")))
            && self.repo.join(rel).is_file()
    }

    pub(crate) fn exists(&self, rel: &str) -> Option<String> {
        let stem = rel.trim_end_matches(".js").trim_end_matches(".jsx").trim_end_matches(".mjs");
        let stem = stem.replace("/dist/", "/src/");
        let stem = stem.strip_suffix(".d.ts").unwrap_or(&stem).to_string();
        // TypeScript first, and not for taste: `./money.js` written in a `.ts` file names
        // `money.ts`, and a repository that has both wants the source. The JavaScript spellings
        // answer the imports a `.mjs` hook or a config file writes, which resolved to nothing
        // while the corpus held no such file.
        let candidates = [
            stem.clone(),
            format!("{stem}.ts"),
            format!("{stem}.tsx"),
            format!("{stem}/index.ts"),
            format!("{stem}/index.tsx"),
            format!("{stem}.js"),
            format!("{stem}.jsx"),
            format!("{stem}.mjs"),
            format!("{stem}.cjs"),
            format!("{stem}/index.js"),
            format!("{stem}/index.mjs"),
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
        for (dir, base, patterns) in &self.paths {
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
                    let joined = crate::doc::links::normalise(&format!("{base}/x"), &t);
                    if let Some(hit) = self.exists(&joined) {
                        return Some(hit);
                    }
                }
            }
        }
        for (name, (dir, exports)) in &self.packages {
            let Some(rest) = spec.strip_prefix(name.as_str()) else { continue };
            if !rest.is_empty() && !rest.starts_with('/') {
                continue;
            }
            let sub = if rest.is_empty() { ".".to_string() } else { format!(".{rest}") };
            // An exact subpath first; otherwise the first `./*`-style pattern that fits it.
            let target = exports.get(&sub).cloned().or_else(|| {
                exports.iter().find_map(|(pat, t)| {
                    let (prefix, suffix) = pat.split_once('*')?;
                    let inner = sub.strip_prefix(prefix)?.strip_suffix(suffix)?;
                    Some(t.replace('*', inner))
                })
            });
            let Some(t) = target else { continue };
            let joined = crate::doc::links::normalise(&format!("{dir}/x"), &t);
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
        let r = Resolver::new(d.path(), &crate::config::Config::default()).unwrap();
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
        let r = Resolver::new(d.path(), &crate::config::Config::default()).unwrap();
        assert_eq!(r.resolve("apps/api/src/x.ts", "@beauty-crm/contracts").as_deref(), Some("packages/contracts/src/index.ts"));
        assert_eq!(r.resolve("apps/api/src/x.ts", "@beauty-crm/contracts/money").as_deref(), Some("packages/contracts/src/money.ts"));
        assert_eq!(r.resolve("packages/ui/src/app.tsx", "@/button").as_deref(), Some("packages/ui/src/button/index.tsx"));
    }

    #[test]
    fn package_exports_map_dist_to_src() {
        let d = repo();
        std::fs::remove_file(d.path().join("tsconfig.json")).unwrap();
        let r = Resolver::new(d.path(), &crate::config::Config::default()).unwrap();
        assert_eq!(r.resolve("apps/api/src/x.ts", "@beauty-crm/contracts/business").as_deref(), Some("packages/contracts/src/business.ts"));
        assert_eq!(r.resolve("apps/api/src/x.ts", "@beauty-crm/contracts").as_deref(), Some("packages/contracts/src/index.ts"));
    }

    #[test]
    fn external_packages_are_none() {
        let d = repo();
        let r = Resolver::new(d.path(), &crate::config::Config::default()).unwrap();
        assert_eq!(r.resolve("apps/api/src/x.ts", "@nestjs/common"), None);
        assert_eq!(r.resolve("apps/api/src/x.ts", "node:fs"), None);
    }

    #[test]
    fn block_comments_in_tsconfig_json_are_stripped_before_parsing() {
        let d = tempfile::tempdir().unwrap();
        let w = |p: &str, c: &str| {
            let full = d.path().join(p);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, c).unwrap();
        };
        w("tsconfig.json", "{\n  /* a block comment\n     spanning lines */\n  \"compilerOptions\": { \"paths\": { \"@x/*\": [\"./src/*\"] } }\n}\n");
        w("src/thing.ts", "export const x = 1;\n");
        let r = Resolver::new(d.path(), &crate::config::Config::default()).unwrap();
        assert_eq!(r.resolve("apps/y.ts", "@x/thing").as_deref(), Some("src/thing.ts"));
    }

    #[test]
    fn trailing_commas_in_tsconfig_json_are_tolerated() {
        let d = tempfile::tempdir().unwrap();
        let w = |p: &str, c: &str| {
            let full = d.path().join(p);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, c).unwrap();
        };
        w("tsconfig.json", r#"{ "compilerOptions": { "paths": { "@x/*": ["./src/*"], }, }, }"#);
        w("src/thing.ts", "export const x = 1;\n");
        let r = Resolver::new(d.path(), &crate::config::Config::default()).unwrap();
        assert_eq!(r.resolve("apps/y.ts", "@x/thing").as_deref(), Some("src/thing.ts"));
    }

    #[test]
    fn a_path_alias_scoped_to_its_own_tsconfig_does_not_resolve_outside_it() {
        let d = repo();
        let r = Resolver::new(d.path(), &crate::config::Config::default()).unwrap();
        // "@/*" is only declared under packages/ui/tsconfig.json; a file outside that
        // directory must not see it, even though "packages/ui/src/button/index.tsx" exists.
        assert_eq!(r.resolve("apps/api/src/x.ts", "@/button"), None);
        assert_eq!(r.resolve("packages/ui/src/app.tsx", "@/button").as_deref(), Some("packages/ui/src/button/index.tsx"));
    }

    #[test]
    fn a_wildcard_export_pattern_maps_through_its_prefix_and_suffix() {
        let d = tempfile::tempdir().unwrap();
        let w = |p: &str, c: &str| {
            let full = d.path().join(p);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, c).unwrap();
        };
        w("packages/pkg/package.json", r#"{ "name": "@x/pkg", "exports": { "./*": "./dist/*.js" } }"#);
        // "/dist/" is redirected to "/src/" by `exists`, so the built .js target maps to this file.
        w("packages/pkg/src/sub/thing.ts", "export const x = 1;\n");
        let r = Resolver::new(d.path(), &crate::config::Config::default()).unwrap();
        assert_eq!(r.resolve("apps/y.ts", "@x/pkg/sub/thing").as_deref(), Some("packages/pkg/src/sub/thing.ts"));
    }

    #[test]
    fn mjs_extension_relative_imports_resolve_like_js() {
        let d = repo();
        let r = Resolver::new(d.path(), &crate::config::Config::default()).unwrap();
        assert_eq!(r.resolve("packages/contracts/src/index.ts", "./money.mjs").as_deref(), Some("packages/contracts/src/money.ts"));
    }

    // A hook importing a hook: neither side has a TypeScript spelling to fall back on, and before
    // JavaScript entered the corpus this edge resolved to nothing.
    #[test]
    fn a_javascript_file_resolves_to_the_javascript_file() {
        let d = repo();
        let w = |p: &str, c: &str| {
            let full = d.path().join(p);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, c).unwrap();
        };
        w("tools/verify/verify.mjs", "import { jobs } from './jobs.mjs';\n");
        w("tools/verify/jobs.mjs", "export const jobs = [];\n");
        w("tools/lint/index.js", "export const lint = 1;\n");
        let r = Resolver::new(d.path(), &crate::config::Config::default()).unwrap();
        assert_eq!(r.resolve("tools/verify/verify.mjs", "./jobs.mjs").as_deref(), Some("tools/verify/jobs.mjs"));
        assert_eq!(r.resolve("tools/verify/verify.mjs", "../lint").as_deref(), Some("tools/lint/index.js"));
    }

    // The TypeScript spelling of `./money.js` still wins where both exist: that is what the import
    // means in a `.ts` file, and the source is what the reader asked about.
    #[test]
    fn typescript_wins_over_a_javascript_file_of_the_same_stem() {
        let d = repo();
        std::fs::write(d.path().join("packages/contracts/src/money.js"), "export const asGrosze = 1;\n").unwrap();
        let r = Resolver::new(d.path(), &crate::config::Config::default()).unwrap();
        assert_eq!(r.resolve("packages/contracts/src/index.ts", "./money.js").as_deref(), Some("packages/contracts/src/money.ts"));
    }

    #[test]
    fn a_header_puts_each_top_level_name_under_every_scope_it_names() {
        use crate::code::index::Header;
        let mut indexes = BTreeMap::new();
        let two = Header { scope: vec!["Shop.Orders".into(), "Shop.Legacy".into()], top: ["Order".to_string()].into(), ..Default::default() };
        let one = Header { scope: vec!["Shop.Orders".into()], top: ["Order".to_string()].into(), ..Default::default() };
        index_header(&mut indexes, Family::DotNet, "src/Order.cs", &two);
        index_header(&mut indexes, Family::DotNet, "src/Order.Partial.cs", &one);
        let idx = &indexes[&Family::DotNet];
        assert_eq!(idx.files("Shop.Orders.Order"), ["src/Order.Partial.cs", "src/Order.cs"]);
        assert_eq!(idx.files("Shop.Legacy.Order"), ["src/Order.cs"]);
        assert!(!indexes.contains_key(&Family::Jvm));
    }

    #[test]
    fn a_family_with_no_scope_indexes_its_bare_names() {
        use crate::code::index::Header;
        let mut indexes = BTreeMap::new();
        let header = Header { scope: Vec::new(), top: ["fragment/Card".to_string()].into(), ..Default::default() };
        index_header(&mut indexes, Family::GraphQl, "q/cards.gql", &header);
        assert_eq!(indexes[&Family::GraphQl].files("fragment/Card"), ["q/cards.gql"]);
    }

    #[test]
    fn a_directive_never_enters_the_index() {
        use crate::code::index::Header;
        let mut indexes = BTreeMap::new();
        let header = Header { scope: vec!["Shop.Orders".into()], top: ["Order".to_string()].into(), directives: ["Shop.Legacy".to_string()].into() };
        index_header(&mut indexes, Family::DotNet, "src/Order.cs", &header);
        let idx = &indexes[&Family::DotNet];
        assert_eq!(idx.files("Shop.Orders.Order"), ["src/Order.cs"]);
        assert!(idx.files("Shop.Legacy").is_empty() && idx.files("Shop.Legacy.Order").is_empty());
        assert!(idx.under("Shop.Legacy").is_empty());
    }

    #[test]
    fn a_typescript_repository_builds_no_index_and_a_file_no_grammar_reads_joins_none() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.ts"), "export class A {}\n").unwrap();
        // A log is no family's language, so no 0.6.0 grammar can turn this fixture into a real one.
        std::fs::write(d.path().join("boot.log"), "boot ok\n").unwrap();
        let mut cfg = crate::config::Config::default();
        cfg.code_globs.push("**/*.log".into());
        let r = Resolver::new(d.path(), &cfg).unwrap();
        assert!(r.index(Family::TypeScript).is_none());
        assert!(r.index(Family::DotNet).is_none());
    }
}
