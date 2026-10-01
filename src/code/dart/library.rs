use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use super::declarations::{child, named, text};
use crate::code::lang::Lang;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Directive {
    pub uri: String,
    pub prefix: Option<String>,
    pub show: Vec<String>,
    pub hide: Vec<String>,
}

impl Directive {
    fn admits(&self, name: &str) -> bool {
        (self.show.is_empty() || self.show.iter().any(|s| s == name)) && !self.hide.iter().any(|h| h == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartOf {
    Uri(String),
    Name(String),
}

#[derive(Debug, Default, Clone)]
pub struct Header {
    pub library: Option<String>,
    pub imports: Vec<Directive>,
    pub exports: Vec<Directive>,
    /// The URIs of `part` directives, as written.
    pub parts: Vec<String>,
    pub part_of: Option<PartOf>,
    pub top: BTreeSet<String>,
}

fn unquote(u: Node, src: &[u8]) -> String {
    text(u, src).trim_matches(|c| c == '\'' || c == '"').to_string()
}

/// A directive's URI, unquoted. For a conditional import this is the default URI; `uris_of` adds the rest.
fn uri_of(n: Node, src: &[u8]) -> Option<String> {
    let u = n.child_by_field_name("uri").or_else(|| child(n, "uri"))?;
    let u = if u.kind() == "configurable_uri" { child(u, "uri")? } else { u };
    Some(unquote(u, src))
}

/// Every URI a directive may load: the default, then each `if (…) 'x.dart'` in order. Flutter splits a
/// platform API into a stub and an io or web file this way, and the configured file is the one a device
/// runs, so each is imported with the same prefix and combinators.
fn uris_of(n: Node, src: &[u8]) -> Vec<String> {
    let Some(default) = uri_of(n, src) else { return Vec::new() };
    let configured = n.child_by_field_name("uri").or_else(|| child(n, "configurable_uri"))
        .filter(|u| u.kind() == "configurable_uri")
        .map(|u| named(u).into_iter()
            .filter(|c| c.kind() == "configuration_uri")
            .filter_map(|c| child(c, "uri"))
            .map(|c| unquote(c, src))
            .collect::<Vec<_>>())
        .unwrap_or_default();
    std::iter::once(default).chain(configured).collect()
}

/// `show` and `hide` are the combinator's first token; the grammar gives neither a node of its own.
fn combinators(n: Node, src: &[u8], d: &mut Directive) {
    for c in named(n).into_iter().filter(|c| c.kind() == "combinator") {
        let names = named(c).into_iter().map(|i| text(i, src).to_string());
        if text(c, src).trim_start().starts_with("show") { d.show.extend(names) } else { d.hide.extend(names) }
    }
}

pub fn header_of(source: &str) -> Header {
    let src = source.as_bytes();
    let Some(tree) = Lang::Dart.parse(src) else { return Header::default() };
    let root = tree.root_node();
    let mut h = Header::default();
    for n in named(root) {
        match n.kind() {
            "library_name" => h.library = child(n, "dotted_identifier_list").map(|l| text(l, src).to_string()),
            "import_or_export" => {
                for inner in named(n) {
                    let (spec, exporting) = match inner.kind() {
                        "library_import" => match child(inner, "import_specification") { Some(s) => (s, false), None => continue },
                        "library_export" => (inner, true),
                        _ => continue,
                    };
                    let prefix = spec.child_by_field_name("alias").map(|a| text(a, src).to_string());
                    for uri in uris_of(spec, src) {
                        let mut d = Directive { uri, prefix: prefix.clone(), ..Directive::default() };
                        combinators(spec, src, &mut d);
                        if exporting { h.exports.push(d) } else { h.imports.push(d) }
                    }
                }
            }
            "part_directive" => h.parts.extend(uri_of(n, src)),
            "part_of_directive" => {
                h.part_of = match child(n, "uri") {
                    Some(_) => uri_of(n, src).map(PartOf::Uri),
                    None => child(n, "dotted_identifier_list").map(|l| PartOf::Name(text(l, src).to_string())),
                };
            }
            _ => {}
        }
    }
    let mut scratch = crate::model::Extraction::default();
    h.top = super::declarations::scan(root, "", src, &mut scratch).top;
    h
}

#[derive(serde::Deserialize)]
struct Pubspec {
    name: Option<String>,
}

#[derive(Debug, Default)]
pub struct Libraries {
    /// package name -> the repository-relative directory holding its `pubspec.yaml` ("" at the root)
    packages: BTreeMap<String, String>,
    /// every globbed `.dart` file's header
    files: BTreeMap<String, Header>,
    /// `library a.b;` -> the file that says it, for `part of a.b;`
    named: BTreeMap<String, String>,
}

impl Libraries {
    pub fn collect_manifest(&mut self, rel: &str, text: &str) {
        let Ok(Pubspec { name: Some(name) }) = serde_yaml::from_str::<Pubspec>(text) else { return };
        let dir = rel.rsplit_once('/').map_or("", |(d, _)| d).to_string();
        self.packages.insert(name, dir);
    }

    pub fn collect(&mut self, rel: &str, source: &str) {
        let h = header_of(source);
        if let Some(lib) = &h.library {
            self.named.insert(lib.clone(), rel.to_string());
        }
        self.files.insert(rel.to_string(), h);
    }

    pub fn header(&self, rel: &str) -> Option<&Header> {
        self.files.get(rel)
    }

    /// The globbed file a URI names: `package:` through the pubspec that declares the package, a
    /// relative URI beside `from`. `dart:` and a package this repository does not hold name nothing,
    /// and so does a file the walk did not glob, because an edge to it would have no node.
    pub fn target(&self, from: &str, uri: &str) -> Option<String> {
        let rel = if let Some(rest) = uri.strip_prefix("package:") {
            let (package, path) = rest.split_once('/')?;
            let dir = self.packages.get(package)?;
            if dir.is_empty() { format!("lib/{path}") } else { format!("{dir}/lib/{path}") }
        } else if uri.contains(':') {
            return None;
        } else {
            crate::doc::links::normalise(from, uri)
        };
        self.files.contains_key(&rel).then_some(rel)
    }

    /// The file that names `rel`'s library: itself, or the library its `part of` points at.
    pub fn library_of(&self, rel: &str) -> String {
        match self.files.get(rel).and_then(|h| h.part_of.as_ref()) {
            Some(PartOf::Uri(u)) => self.target(rel, u).unwrap_or_else(|| rel.to_string()),
            Some(PartOf::Name(n)) => self.named.get(n).cloned().unwrap_or_else(|| rel.to_string()),
            None => rel.to_string(),
        }
    }

    /// A library's files: the file itself and every part it names.
    pub fn units(&self, lib: &str) -> Vec<String> {
        let mut out = vec![lib.to_string()];
        if let Some(h) = self.files.get(lib) {
            out.extend(h.parts.iter().filter_map(|p| self.target(lib, p)));
        }
        out
    }

    /// The public names a library exports, each with its declaring files: its own units' names, then
    /// whatever its `export`s pass on. `seen` holds the libraries on the current export chain, which
    /// stops a cycle; a library two exports reach is read through each, since each filters it apart.
    fn namespace(&self, lib: &str, seen: &mut BTreeSet<String>) -> BTreeMap<String, BTreeSet<String>> {
        let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        if !seen.insert(lib.to_string()) {
            return out;
        }
        for unit in self.units(lib) {
            let Some(h) = self.files.get(&unit) else { continue };
            for name in h.top.iter().filter(|n| !n.starts_with('_')) {
                out.entry(name.clone()).or_default().insert(unit.clone());
            }
        }
        let exports = self.files.get(lib).map(|h| h.exports.clone()).unwrap_or_default();
        for e in exports {
            let Some(t) = self.target(lib, &e.uri) else { continue };
            for (name, files) in self.namespace(&t, seen) {
                if e.admits(&name) {
                    out.entry(name).or_default().extend(files);
                }
            }
        }
        seen.remove(lib);
        out
    }

    /// A re-export's context: `*` when it passes on everything, else the names it admits.
    pub fn export_context(&self, from: &str, e: &Directive) -> Option<String> {
        let t = self.target(from, &e.uri)?;
        if e.show.is_empty() && e.hide.is_empty() {
            return Some("*".to_string());
        }
        let names: Vec<String> = self.namespace(&t, &mut BTreeSet::new()).into_keys().filter(|n| e.admits(n)).collect();
        Some(names.join(","))
    }

    /// Files declaring `name` as `from` sees it. Its own library comes first and shadows every import,
    /// as Dart's scoping does; then each unprefixed import. Two imports giving the same name resolve
    /// to both, each keeping its own id.
    pub fn resolve(&self, from: &str, name: &str) -> Vec<String> {
        let lib = self.library_of(from);
        let own: Vec<String> = self.units(&lib).into_iter()
            .filter(|u| self.files.get(u).is_some_and(|h| h.top.contains(name)))
            .collect();
        if !own.is_empty() {
            return own;
        }
        self.imported(&lib, None, name)
    }

    /// Files declaring `name` through `lib`'s imports carrying `prefix` (None: the unprefixed ones).
    pub fn imported(&self, lib: &str, prefix: Option<&str>, name: &str) -> Vec<String> {
        let lib = self.library_of(lib);
        let mut out = BTreeSet::new();
        let imports = self.files.get(&lib).map(|h| h.imports.clone()).unwrap_or_default();
        for d in imports.iter().filter(|d| d.prefix.as_deref() == prefix && d.admits(name)) {
            let Some(t) = self.target(&lib, &d.uri) else { continue };
            if let Some(files) = self.namespace(&t, &mut BTreeSet::new()).remove(name) {
                out.extend(files);
            }
        }
        out.into_iter().collect()
    }

    /// Each import of `from`'s library that names a file: the file, the prefix, and the names the
    /// import admits. A part reads its library's imports, so it gets the same list.
    pub fn import_targets(&self, from: &str) -> Vec<(String, Option<String>, BTreeSet<String>)> {
        let lib = self.library_of(from);
        let imports = self.files.get(&lib).map(|h| h.imports.clone()).unwrap_or_default();
        imports.into_iter().filter_map(|d| {
            let t = self.target(&lib, &d.uri)?;
            let names = self.namespace(&t, &mut BTreeSet::new()).into_keys().filter(|n| d.admits(n)).collect();
            Some((t, d.prefix, names))
        }).collect()
    }
}
