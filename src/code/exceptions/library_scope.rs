//! A Dart name is in scope through its library, never its file alone: a library is a file and the
//! parts it names, it sees what each import's `show`, `hide` and prefix admit, and an import reaches
//! whatever the imported library `export`s on, through any chain of exports. A file's own queries
//! cannot see the other files of the chain, so each globbed file's directives and top-level names are
//! collected first, and calls and the import edges are resolved through them.
//!
//! An import's context is the names the file uses from it, as a TypeScript named import's is: every
//! Dart import brings a whole namespace, and `impact` counts an importer only when the context names
//! the symbol, so `*` would make every importer depend on every name.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use tree_sitter::Node;

use crate::code::engine::{Facts, File, Sink};
use crate::code::imports::Resolver;
use crate::code::lang::Lang;
use crate::code::syntax::{child, named, text};
use crate::model::EdgeKind;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Directive {
    uri: String,
    prefix: Option<String>,
    show: Vec<String>,
    hide: Vec<String>,
}

impl Directive {
    fn admits(&self, name: &str) -> bool {
        (self.show.is_empty() || self.show.iter().any(|s| s == name)) && !self.hide.iter().any(|h| h == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PartOf {
    Uri(String),
    Name(String),
}

/// What a file adds to its library's scope.
#[derive(Debug, Default, Clone)]
struct Unit {
    library: Option<String>,
    imports: Vec<Directive>,
    exports: Vec<Directive>,
    /// The URIs of `part` directives, as written.
    parts: Vec<String>,
    part_of: Option<PartOf>,
    /// Private names included: a part declares them for its whole library.
    top: BTreeSet<String>,
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

fn unit_of(rel: &str, source: &str) -> Unit {
    let src = source.as_bytes();
    let Some(tree) = Lang::Dart.parse(src) else { return Unit::default() };
    let root = tree.root_node();
    let mut u = Unit::default();
    for n in named(root) {
        match n.kind() {
            "library_name" => u.library = child(n, "dotted_identifier_list").map(|l| text(l, src).to_string()),
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
                        if exporting { u.exports.push(d) } else { u.imports.push(d) }
                    }
                }
            }
            "part_directive" => u.parts.extend(uri_of(n, src)),
            "part_of_directive" => {
                u.part_of = match child(n, "uri") {
                    Some(_) => uri_of(n, src).map(PartOf::Uri),
                    None => child(n, "dotted_identifier_list").map(|l| PartOf::Name(text(l, src).to_string())),
                };
            }
            _ => {}
        }
    }
    // The decl query reads the tree parsed here: read later through the resolver, every file a
    // lookup reaches would be parsed a second time.
    if let Some(decl) = crate::code::queries::decl(Lang::Dart) {
        let file = File { lang: Lang::Dart, rel, text: source, src: source, tree: Some(&tree) };
        let facts = crate::code::engine::facts(crate::code::spec::spec(Lang::Dart), decl, None, &file);
        u.top = facts.defs.into_iter().filter(|d| d.parent.is_none()).map(|d| d.qualified).collect();
    }
    u
}

#[derive(serde::Deserialize)]
struct Pubspec {
    name: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct Libraries {
    /// package name -> the repository-relative directory holding its `pubspec.yaml` ("" at the root)
    packages: BTreeMap<String, String>,
    files: BTreeMap<String, Unit>,
    /// `library a.b;` -> the file that says it, for `part of a.b;`
    named: BTreeMap<String, String>,
}

pub(crate) fn collect(r: &mut Resolver, rel: &str, source: &str) -> Option<crate::code::index::Header> {
    let u = unit_of(rel, source);
    let lib = r.state_mut::<Libraries>();
    if let Some(name) = &u.library {
        lib.named.insert(name.clone(), rel.to_string());
    }
    lib.files.insert(rel.to_string(), u);
    None
}

pub(crate) fn pubspec(r: &mut Resolver, rel: &str, text: &str) {
    let Ok(Pubspec { name: Some(name) }) = serde_yaml::from_str::<Pubspec>(text) else { return };
    let dir = rel.rsplit_once('/').map_or("", |(d, _)| d).to_string();
    r.state_mut::<Libraries>().packages.insert(name, dir);
}

impl Libraries {
    /// The globbed file a URI names: `package:` through the pubspec that declares the package, a
    /// relative URI beside `from`. `dart:` and a package this repository does not hold name nothing,
    /// and so does a file the walk did not glob, because an edge to it would have no node.
    pub(crate) fn target(&self, from: &str, uri: &str) -> Option<String> {
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
    pub(crate) fn library_of(&self, rel: &str) -> String {
        match self.files.get(rel).and_then(|u| u.part_of.as_ref()) {
            Some(PartOf::Uri(u)) => self.target(rel, u).unwrap_or_else(|| rel.to_string()),
            Some(PartOf::Name(n)) => self.named.get(n).cloned().unwrap_or_else(|| rel.to_string()),
            None => rel.to_string(),
        }
    }

    /// A library's files: the file itself and every part it names.
    fn units(&self, lib: &str) -> Vec<String> {
        let mut out = vec![lib.to_string()];
        if let Some(u) = self.files.get(lib) {
            out.extend(u.parts.iter().filter_map(|p| self.target(lib, p)));
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
            let Some(u) = self.files.get(&unit) else { continue };
            for name in u.top.iter().filter(|n| !n.starts_with('_')) {
                out.entry(name.clone()).or_default().insert(unit.clone());
            }
        }
        let exports = self.files.get(lib).map(|u| u.exports.clone()).unwrap_or_default();
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
    fn export_context(&self, from: &str, e: &Directive) -> Option<String> {
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
    pub(crate) fn resolve(&self, from: &str, name: &str) -> Vec<String> {
        let lib = self.library_of(from);
        let own: Vec<String> = self.units(&lib).into_iter().filter(|u| self.files.get(u).is_some_and(|f| f.top.contains(name))).collect();
        if !own.is_empty() {
            return own;
        }
        self.imported(&lib, None, name)
    }

    /// Files declaring `name` through `lib`'s imports carrying `prefix` (None: the unprefixed ones).
    pub(crate) fn imported(&self, lib: &str, prefix: Option<&str>, name: &str) -> Vec<String> {
        let lib = self.library_of(lib);
        let mut out = BTreeSet::new();
        let imports = self.files.get(&lib).map(|u| u.imports.clone()).unwrap_or_default();
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
    fn import_targets(&self, from: &str) -> Vec<(String, Option<String>, BTreeSet<String>)> {
        let lib = self.library_of(from);
        let imports = self.files.get(&lib).map(|u| u.imports.clone()).unwrap_or_default();
        imports.into_iter().filter_map(|d| {
            let t = self.target(&lib, &d.uri)?;
            let names = self.namespace(&t, &mut BTreeSet::new()).into_keys().filter(|n| d.admits(n)).collect();
            Some((t, d.prefix, names))
        }).collect()
    }
}

#[derive(Default)]
struct Used {
    bare: BTreeSet<String>,
    /// (prefix, name) for every `p.Name` in the file
    prefixed: BTreeSet<(String, String)>,
}

fn used_names(root: Node, src: &[u8]) -> Used {
    let mut u = Used::default();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        match n.kind() {
            "identifier" | "type_identifier" => {
                u.bare.insert(text(n, src).to_string());
            }
            "member_expression" => {
                if let (Some(o), Some(p)) = (n.child_by_field_name("object"), n.child_by_field_name("property")) {
                    if o.kind() == "identifier" {
                        u.prefixed.insert((text(o, src).to_string(), text(p, src).to_string()));
                    }
                }
            }
            // `p.Widget` as a type is one `type` holding two `type_identifier`s around the `.`.
            "type" => {
                if let [p, name, ..] = named(n).as_slice() {
                    let between = src.get(p.end_byte()..name.start_byte()).and_then(|b| std::str::from_utf8(b).ok());
                    if p.kind() == "type_identifier" && name.kind() == "type_identifier" && between.is_some_and(|b| b.trim() == ".") {
                        u.prefixed.insert((text(*p, src).to_string(), text(*name, src).to_string()));
                    }
                }
            }
            _ => {}
        }
        stack.extend(named(n));
    }
    u
}

fn directives(lib: &Libraries, rel: &str, root: Node, src: &[u8], sink: &mut Sink) {
    let file_id = format!("file:{rel}");
    let used = used_names(root, src);
    for (target, prefix, names) in lib.import_targets(rel) {
        let kept: Vec<&str> = names.iter()
            .filter(|n| match &prefix {
                Some(p) => used.prefixed.contains(&(p.clone(), (*n).clone())),
                None => used.bare.contains(*n),
            })
            .map(String::as_str)
            .collect();
        sink.edge(&file_id, &format!("file:{target}"), EdgeKind::Imports, &kept.join(","), rel);
    }
    let Some(u) = lib.files.get(rel) else { return };
    for e in &u.exports {
        let (Some(t), Some(context)) = (lib.target(rel, &e.uri), lib.export_context(rel, e)) else { continue };
        sink.edge(&file_id, &format!("file:{t}"), EdgeKind::ReExports, &context, rel);
    }
}

/// The import edges, then each supertype and each call resolved through the library's scope. The
/// default reads a file alone, which misses a supertype a part or an import declares.
pub(crate) fn resolve(r: &Resolver, file: &File, facts: &Facts, sink: &mut Sink) {
    let (lib, rel) = (r.state::<Libraries>(), file.rel);
    if let Some(tree) = file.tree {
        directives(lib, rel, tree.root_node(), file.src.as_bytes(), sink);
    }
    for r in facts.refs.iter().filter(|r| r.edge == EdgeKind::Extends) {
        // Only a type inherits; a supertype that resolves nowhere is a Flutter class this repository
        // does not hold, and a same-file guess would name a symbol no file declares.
        let Some(sub) = r.source.map(|i| &facts.defs[i]).filter(|d| d.kind.join().is_some()) else { continue };
        for f in lib.resolve(rel, &r.name) {
            let to = format!("sym:{f}::{}", r.name);
            if to != sub.id {
                sink.edge(&sub.id, &to, EdgeKind::Extends, &r.context, rel);
            }
        }
    }
    let class_of = |mut at: Option<usize>| {
        while let Some(i) = at {
            let d = &facts.defs[i];
            if d.kind.join().is_some() {
                return Some(d.qualified.as_str());
            }
            at = d.parent;
        }
        None
    };
    // The class each field holds, which the query reads as a reference with the field as its receiver.
    let mut fields: HashMap<(&str, &str), &str> = HashMap::new();
    for f in facts.refs.iter().filter(|f| f.edge == EdgeKind::References) {
        if let (Some(class), Some(field)) = (class_of(f.source), f.receiver.as_deref()) {
            fields.insert((class, field), f.name.as_str());
        }
    }
    let ids = |name: &str, member: Option<&str>| -> Vec<String> {
        lib.resolve(rel, name).into_iter()
            .map(|f| match member {
                Some(m) => format!("sym:{f}::{name}.{m}"),
                None => format!("sym:{f}::{name}"),
            })
            .collect()
    };
    for call in facts.refs.iter().filter(|c| c.edge == EdgeKind::Calls) {
        let class = class_of(call.source);
        let name = call.name.as_str();
        // Only a member this type declares: an inherited one is declared on another type.
        let own = || class.map(|c| format!("{c}.{name}")).filter(|q| facts.declares(q)).map(|q| format!("sym:{rel}::{q}"));
        let field = |f: &str| class.and_then(|c| fields.get(&(c, f)).copied());
        let targets = match call.receiver.as_deref() {
            // Dart's implicit `this`: inside a type a bare name is its own member before it is
            // anything the library or an import declares.
            None => own().map_or_else(|| ids(name, None), |m| vec![m]),
            Some("this") => own().into_iter().collect(),
            // `this.field.m()`, the one receiver the query captures with a dot in it.
            Some(r) if r.contains('.') => {
                let f = r.rsplit('.').next().unwrap_or(r).trim();
                field(f).map(|t| ids(t, Some(name))).unwrap_or_default()
            }
            Some(r) => match field(r) {
                Some(t) => ids(t, Some(name)),
                None => {
                    let prefixed = lib.imported(rel, Some(r), name);
                    if prefixed.is_empty() {
                        // `Type.member()`: a static member or a named constructor.
                        ids(r, Some(name))
                    } else {
                        prefixed.into_iter().map(|f| format!("sym:{f}::{name}")).collect()
                    }
                }
            },
        };
        let from = facts.source_id(rel, call.source);
        for to in targets.into_iter().filter(|t| *t != from) {
            sink.edge(&from, &to, EdgeKind::Calls, "", rel);
        }
    }
}
