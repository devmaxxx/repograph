//! What a name-indexed family (L3) knows about each file before any file is extracted: the file's
//! header, and a map from qualified name to every file declaring it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::code::lang::{Family, Lang};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    /// package / namespace lines, in file order (`pl.x.core`, `Shop.Orders`); empty for families with none.
    pub scope: Vec<String>,
    /// top-level declared names, as they appear after `sym:<rel>::` with no `.`.
    pub top: BTreeSet<String>,
    /// lines that change what a file resolves and declare nothing: a C# `using` or `global using`,
    /// `@using` and `@namespace` in Razor's `_Imports.razor`, an import a family chooses to record.
    /// `widen` compares them with the rest; the index never holds them. A header recorded before a
    /// family wrote any reads back with none.
    #[serde(default)]
    pub directives: BTreeSet<String>,
    /// What a family whose names nest declares below the top level. Resolution confirms a nested
    /// name against it, so it sits in the header for `widen` to see it move; empty for every other
    /// family, and for a header recorded before it existed.
    #[serde(default, skip_serializing_if = "Nested::is_empty")]
    pub nested: Nested,
    /// Top-level names every declaration of which in this file is file-private, as Kotlin's
    /// `private` is: the index leaves them out, so no other file resolves to them. Empty for
    /// every family without file-private top-level names.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub private: BTreeSet<String>,
}

/// Every type path (`Outer`, `Outer.Inner`, top-level ones included) and every member path
/// (`Outer.run`) one file declares where another file can reach it.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nested {
    pub types: BTreeSet<String>,
    pub members: BTreeSet<String>,
    /// Member paths only a field declares, which no Java call binds. Empty for a family that reads
    /// its members as one namespace.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub values: BTreeSet<String>,
    /// Per method path, the argument counts each of its declarations takes: overloads share a
    /// path, and a call binds only a declaration that admits its count.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub arities: BTreeMap<String, Vec<Arity>>,
    /// How the file's types reach their supertypes; `None` for a family that records none, and
    /// for a file that declares no type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supers: Option<Supers>,
    /// Java member paths some declaration of which a subtype may not inherit; one absent here
    /// every subtype inherits.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub reach: BTreeMap<String, Reach>,
}

impl Nested {
    fn is_empty(&self) -> bool {
        self.types.is_empty() && self.members.is_empty() && self.values.is_empty() && self.arities.is_empty() && self.supers.is_none() && self.reach.is_empty()
    }
}

/// A file's supertypes as it writes them, with the names it resolves them through, so a subtype
/// in another file can read past a supertype that does not declare a name. In the header, a
/// supertype or an import that moves widens as any other declaration does.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Supers {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub package: String,
    /// Per type path, its supertypes as written; a type absent here writes none.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub written: BTreeMap<String, Vec<String>>,
    /// Types with a supertype the file never writes, such as an enum's, which may hold any name.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub implicit: BTreeSet<String>,
    /// Per type path, the supertype it extends as a class, when the file writes which one that is.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub classes: BTreeMap<String, String>,
    /// Enums, whose implicit `Enum` superclass declares names an interface's never beat.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub enums: BTreeSet<String>,
    /// Kotlin data classes, whose `copy` and `componentN` the file never writes.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub data: BTreeSet<String>,
    /// Per Java record path, its components, each with an accessor the file need not write.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub components: BTreeMap<String, BTreeSet<String>>,
    /// The file's explicit imports, local name to every qualified name bound to it. Recorded only
    /// when some type writes a supertype, since only resolving one reads them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub singles: BTreeMap<String, BTreeSet<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stars: Vec<String>,
}

/// Which subtypes inherit a member, across the declarations that share its id. Java inherits
/// neither an interface's `static` method nor, outside its package, a package-private member.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reach {
    /// Some declaration every subtype inherits.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub always: bool,
    /// Some package-private declaration, which only a subtype in its package inherits.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub package: bool,
    /// Some declaration no subtype inherits.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub never: bool,
}

/// How many arguments one method declaration takes: `min` without its defaults, `max` with every
/// parameter, and any number past `min` when its last parameter is varargs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Arity {
    pub min: usize,
    pub max: usize,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub varargs: bool,
    /// A Kotlin `override` or `actual`, which takes its defaults from the declaration it
    /// implements and may write none: fewer arguments than `min` may still bind it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub inherits: bool,
    /// A Kotlin `private` overload beside a public one, which takes no call from outside its type.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub private: bool,
}

/// The arguments a call passes. With a spread (`*arr`) their number is unknown, and only a
/// varargs parameter takes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Call {
    pub args: usize,
    pub spread: bool,
}

/// Whether a declaration takes a call's arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    Yes,
    No,
    /// It may, through defaults this file does not see: neither bind it nor look past it.
    Unsure,
}

impl Arity {
    pub fn admission(&self, call: Call) -> Admission {
        let under_max = self.varargs || call.args <= self.max;
        if call.spread {
            if self.varargs { Admission::Yes } else { Admission::No }
        } else if call.args >= self.min && under_max {
            Admission::Yes
        } else if self.inherits && under_max {
            Admission::Unsure
        } else {
            Admission::No
        }
    }
}

/// Whether a declaration among `arities` takes the call. A member with none recorded — a type, a
/// property — and a call whose count is unknown admit anything.
pub fn admission(arities: Option<&Vec<Arity>>, call: Option<Call>) -> Admission {
    let (Some(all), Some(call)) = (arities, call) else { return Admission::Yes };
    let each: Vec<Admission> = all.iter().map(|a| a.admission(call)).collect();
    [Admission::Yes, Admission::Unsure].into_iter().find(|a| each.contains(a)).unwrap_or(Admission::No)
}

/// Whether a declaration among `arities` without varargs takes the call, as Java's first phase
/// of overload resolution, which never expands varargs, reads it.
pub fn fixed(arities: Option<&Vec<Arity>>, call: Option<Call>) -> bool {
    match (arities, call) {
        (Some(all), Some(call)) => all.iter().any(|a| !a.varargs && a.admission(call) == Admission::Yes),
        _ => false,
    }
}

/// Qualified name -> every declaring file. A name several files declare — an `expect` and its
/// `actual`s, a `partial` type, a table many migrations alter — keeps all of them.
#[derive(Debug, Default)]
pub struct QualifiedIndex {
    by_name: BTreeMap<String, BTreeSet<String>>,
    /// Per file, the type paths and member paths it declares below the top level, for a family
    /// whose names nest. The qualified names stop at the top level, so without these a name like
    /// `a.b.Outer.Inner` could only be guessed to live in `Outer`'s file.
    nested: BTreeMap<String, Nested>,
}

impl QualifiedIndex {
    pub fn insert(&mut self, qualified: &str, rel: &str) {
        self.by_name.entry(qualified.to_string()).or_default().insert(rel.to_string());
    }

    pub fn insert_nested(&mut self, rel: &str, nested: Nested) {
        self.nested.insert(rel.to_string(), nested);
    }

    /// Whether `rel` declares a type at `path`.
    pub fn is_type(&self, rel: &str, path: &str) -> bool {
        self.nested.get(rel).is_some_and(|n| n.types.contains(path))
    }

    /// Whether `rel` declares a type or a member at `path`.
    pub fn declares(&self, rel: &str, path: &str) -> bool {
        self.nested.get(rel).is_some_and(|n| n.types.contains(path) || n.members.contains(path))
    }

    /// Whether `rel` declares a member at `path` that is no type.
    pub fn declares_member(&self, rel: &str, path: &str) -> bool {
        self.nested.get(rel).is_some_and(|n| n.members.contains(path) && !n.types.contains(path))
    }

    /// Whether `rel` declares a member at `path` a call can bind: no type, and no field alone.
    pub fn declares_method(&self, rel: &str, path: &str) -> bool {
        self.declares_member(rel, path) && self.nested.get(rel).is_some_and(|n| !n.values.contains(path))
    }

    /// The types `rel` declares, by path.
    pub fn types(&self, rel: &str) -> Option<&BTreeSet<String>> {
        self.nested.get(rel).map(|n| &n.types)
    }

    /// Which subtypes inherit the member `rel` declares at `path`; `None` when every one does.
    pub fn reach(&self, rel: &str, path: &str) -> Option<Reach> {
        self.nested.get(rel).and_then(|n| n.reach.get(path)).copied()
    }

    /// How `rel`'s types reach their supertypes, when its family records it.
    pub fn supers(&self, rel: &str) -> Option<&Supers> {
        self.nested.get(rel).and_then(|n| n.supers.as_ref())
    }

    /// The argument counts of the method `rel` declares at `path`.
    pub fn arities(&self, rel: &str, path: &str) -> Option<&Vec<Arity>> {
        self.nested.get(rel).and_then(|n| n.arities.get(path))
    }

    /// Every file declaring `qualified`, sorted.
    pub fn files(&self, qualified: &str) -> Vec<&str> {
        self.by_name.get(qualified).map(|f| f.iter().map(String::as_str).collect()).unwrap_or_default()
    }

    /// Names directly under `scope.` with their files — for `import a.b.*`, `using A.B;`.
    #[allow(dead_code)]
    pub fn under(&self, scope: &str) -> Vec<(&str, Vec<&str>)> {
        let prefix = format!("{scope}.");
        self.by_name
            .range(prefix.clone()..)
            .take_while(|(name, _)| name.starts_with(&prefix))
            .filter_map(|(name, files)| {
                let rest = &name[prefix.len()..];
                (!rest.contains('.')).then(|| (rest, files.iter().map(String::as_str).collect()))
            })
            .collect()
    }
}

/// Header for an indexed family's file; None for every other language. Family plans add arms.
pub fn header_for(lang: Lang, rel: &str, source: &str) -> Option<Header> {
    match lang.family() {
        // Path-resolved families (L3): the importer names a file, and no header is needed.
        Family::TypeScript | Family::Rust | Family::Python | Family::Dart | Family::Swift | Family::Bicep | Family::Hcl | Family::Shell => None,
        Family::DotNet => match lang {
            Lang::CSharp => Some(crate::code::csharp::index::facts(rel, source).header()),
            Lang::Razor => Some(crate::code::razor::header(rel, source)),
            // Reached only by a variant `of` cannot return yet for this family; see `Lang::family`.
            #[allow(unreachable_patterns)]
            _ => unreachable!("{lang:?} has no DotNet header arm: its plan has not landed"),
        },
        Family::Jvm => match lang {
            Lang::Kotlin => Some(crate::code::kotlin::header(source)),
            Lang::Java => Some(crate::code::java::header(source)),
            // Reached only by a variant `of` cannot return yet for this family; see `Lang::family`.
            #[allow(unreachable_patterns)]
            _ => unreachable!("{lang:?} has no JVM header arm: its plan has not landed"),
        },
        Family::Sql => Some(crate::code::sql::header(source)),
        Family::GraphQl => Some(crate::code::graphql::header(source)),
    }
}

/// Every name-indexed file's header as it was last read, kept in `.repograph/headers.json`.
///
/// Not the graph: a `Declares` edge names what a file declares but not the package or namespace it
/// declares it in. Not the manifest: every quiet `ask` rewrites that from the walk's entries.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Headers(pub BTreeMap<String, Header>);

const HEADERS: &str = "headers.json";

impl Headers {
    pub fn load(store: &crate::store::Store) -> anyhow::Result<Headers> {
        Ok(match store.read_bytes(HEADERS)? {
            // A record that does not parse costs one widening per family on the next update, not the
            // update: every file then reads as new. Said, because the cost is otherwise unexplained.
            Some(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|_| {
                eprintln!("headers.json unreadable; the next update re-reads every family");
                Headers::default()
            }),
            None => Headers::default(),
        })
    }

    /// An empty record leaves no file: a store with no name-indexed file never has one (L3).
    pub fn save(&self, store: &crate::store::Store) -> anyhow::Result<()> {
        if self.0.is_empty() {
            return store.remove(HEADERS);
        }
        store.write_atomic(HEADERS, &serde_json::to_vec(self)?)
    }
}

/// `apply_diff`'s L3 widening. `stale` holds the code files this update re-reads because their bytes
/// moved, `removed` the files gone from the walk, `all_rels` every code file the walk found, sorted.
/// Returns the other files to re-read, sorted, and brings `known` up to the tree.
///
/// When a name-indexed family's file appears, disappears, or re-reads to a header other than the
/// one recorded, every other file of that family joins the re-read: an unchanged file may now
/// resolve a name to another file. A body-only edit leaves the header as it was and widens nothing.
pub fn widen(repo: &Path, stale: &[String], removed: &[String], known: &mut Headers, all_rels: &[String]) -> Vec<String> {
    let family_of = |rel: &str| Lang::of(rel).map(Lang::family).filter(|f| *f != Family::TypeScript);
    widen_by(stale, removed, known, all_rels, &family_of, &|rel| read_header(repo, rel))
}

/// TypeScript resolves through tsconfig and package.json and has no header, so its sources are
/// not opened here: a TypeScript-only update pays nothing for L3.
fn read_header(repo: &Path, rel: &str) -> Option<Header> {
    let lang = Lang::of(rel).filter(|l| l.family() != Family::TypeScript)?;
    let source = std::fs::read_to_string(repo.join(rel)).ok()?;
    header_for(lang, rel, &source)
}

/// Whether any of `changed` now reads to a header other than the one recorded — the case where
/// `widen` re-reads a family, and where a resolver built before the edit would resolve those
/// re-reads against declarations the file no longer holds, or without ones it gained.
pub fn headers_move(repo: &Path, known: &Headers, changed: &[String]) -> bool {
    changed.iter().any(|rel| read_header(repo, rel).as_ref() != known.0.get(rel))
}

fn widen_by(
    stale: &[String],
    removed: &[String],
    known: &mut Headers,
    all_rels: &[String],
    family_of: &dyn Fn(&str) -> Option<Family>,
    header_of: &dyn Fn(&str) -> Option<Header>,
) -> Vec<String> {
    let mut moved: BTreeSet<Family> = BTreeSet::new();
    for rel in removed {
        if known.0.remove(rel).is_some() {
            moved.extend(family_of(rel));
        }
    }
    for rel in stale {
        let now = header_of(rel);
        let before = match &now {
            Some(h) => known.0.insert(rel.clone(), h.clone()),
            None => known.0.remove(rel),
        };
        // The whole header counts, `directives` included: a `global using` or an `_Imports.razor`
        // line changes what the family's other files resolve to without declaring a name.
        if before != now {
            moved.extend(family_of(rel));
        }
    }
    let rereading: BTreeSet<&str> = stale.iter().map(String::as_str).collect();
    let add: Vec<String> = all_rels
        .iter()
        .filter(|r| !rereading.contains(r.as_str()) && family_of(r).is_some_and(|f| moved.contains(&f)))
        .cloned()
        .collect();
    // A widened file's bytes did not move, so a header recorded for it stands. One never recorded —
    // a store written before its family's plan landed — is read now, or the next edit of that file
    // would widen the family again for nothing.
    for rel in &add {
        if !known.0.contains_key(rel) {
            if let Some(h) = header_of(rel) {
                known.0.insert(rel.clone(), h);
            }
        }
    }
    let walked: BTreeSet<&str> = all_rels.iter().map(String::as_str).collect();
    known.0.retain(|rel, _| walked.contains(rel.as_str()));
    add
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn h(scope: &[&str], top: &[&str]) -> Header {
        Header { scope: scope.iter().map(|s| s.to_string()).collect(), top: top.iter().map(|s| s.to_string()).collect(), ..Default::default() }
    }

    fn rels(r: &[&str]) -> Vec<String> {
        r.iter().map(|s| s.to_string()).collect()
    }

    /// `.jv` plays a name-indexed family and `.rs` a path family, which has no header.
    fn family_of(rel: &str) -> Option<Family> {
        match rel.rsplit_once('.').map(|(_, e)| e) {
            Some("jv") => Some(Family::Jvm),
            Some("sq") => Some(Family::Sql),
            Some("rs") => Some(Family::Rust),
            _ => None,
        }
    }

    fn run(stale: &[&str], removed: &[&str], known: &mut Headers, all: &[&str], now: &BTreeMap<&str, Header>) -> Vec<String> {
        let header_of = |rel: &str| family_of(rel).filter(|f| *f != Family::Rust).and_then(|_| now.get(rel).cloned());
        widen_by(&rels(stale), &rels(removed), known, &rels(all), &family_of, &header_of)
    }

    fn known(entries: &[(&str, Header)]) -> Headers {
        Headers(entries.iter().map(|(r, h)| (r.to_string(), h.clone())).collect())
    }

    #[test]
    fn a_body_only_edit_widens_nothing() {
        let mut k = known(&[("a.jv", h(&["p"], &["A"])), ("b.jv", h(&["p"], &["B"]))]);
        let before = k.clone();
        let now = BTreeMap::from([("a.jv", h(&["p"], &["A"])), ("b.jv", h(&["p"], &["B"]))]);
        assert!(run(&["a.jv"], &[], &mut k, &["a.jv", "b.jv"], &now).is_empty());
        assert_eq!(k, before);
    }

    #[test]
    fn a_new_top_level_name_re_reads_every_other_file_of_its_family_and_no_other() {
        let mut k = known(&[("a.jv", h(&["p"], &["A"])), ("b.jv", h(&["p"], &["B"])), ("c.jv", h(&["q"], &["C"])), ("x.sq", h(&[], &["t"]))]);
        let now = BTreeMap::from([("a.jv", h(&["p"], &["A", "Fresh"]))]);
        let add = run(&["a.jv"], &[], &mut k, &["a.jv", "b.jv", "c.jv", "x.sq", "y.rs"], &now);
        assert_eq!(add, ["b.jv", "c.jv"]);
        assert_eq!(k.0["a.jv"], h(&["p"], &["A", "Fresh"]));
    }

    #[test]
    fn a_package_that_moves_with_the_same_names_still_widens() {
        let mut k = known(&[("a.jv", h(&["p"], &["A"])), ("b.jv", h(&["p"], &["B"]))]);
        let now = BTreeMap::from([("a.jv", h(&["q"], &["A"]))]);
        assert_eq!(run(&["a.jv"], &[], &mut k, &["a.jv", "b.jv"], &now), ["b.jv"]);
    }

    #[test]
    fn a_changed_directive_widens_its_family_though_no_name_moved() {
        let with = Header { directives: ["Shop.Legacy".to_string()].into(), ..h(&["p"], &["A"]) };
        let mut k = known(&[("a.jv", h(&["p"], &["A"])), ("b.jv", h(&["p"], &["B"]))]);
        let now = BTreeMap::from([("a.jv", with.clone())]);
        assert_eq!(run(&["a.jv"], &[], &mut k, &["a.jv", "b.jv"], &now), ["b.jv"]);
        assert_eq!(k.0["a.jv"], with);
    }

    #[test]
    fn a_header_recorded_before_directives_existed_reads_back_with_none() {
        let old: Header = serde_json::from_str(r#"{"scope":["p"],"top":["A"]}"#).unwrap();
        assert_eq!(old, h(&["p"], &["A"]));
    }

    #[test]
    fn a_removed_file_widens_its_family_and_its_header_is_forgotten() {
        let mut k = known(&[("a.jv", h(&["p"], &["A"])), ("b.jv", h(&["p"], &["B"])), ("c.jv", h(&["p"], &["C"]))]);
        let add = run(&[], &["b.jv"], &mut k, &["a.jv", "c.jv"], &BTreeMap::new());
        assert_eq!(add, ["a.jv", "c.jv"]);
        assert!(!k.0.contains_key("b.jv"));
    }

    #[test]
    fn an_added_file_widens_its_family() {
        let mut k = known(&[("a.jv", h(&["p"], &["A"]))]);
        let now = BTreeMap::from([("n.jv", h(&["p"], &["N"]))]);
        assert_eq!(run(&["n.jv"], &[], &mut k, &["a.jv", "n.jv"], &now), ["a.jv"]);
        assert_eq!(k.0["n.jv"], h(&["p"], &["N"]));
    }

    #[test]
    fn a_path_resolved_family_never_widens_and_records_nothing() {
        let mut k = Headers::default();
        assert!(run(&["y.rs"], &["w.rs"], &mut k, &["y.rs", "z.rs"], &BTreeMap::new()).is_empty());
        assert!(k.0.is_empty());
    }

    #[test]
    fn a_widened_file_never_recorded_is_recorded_and_a_file_the_walk_lost_is_forgotten() {
        let mut k = known(&[("a.jv", h(&["p"], &["A"])), ("ghost.jv", h(&["p"], &["G"]))]);
        let now = BTreeMap::from([("a.jv", h(&["p"], &["A2"])), ("b.jv", h(&["p"], &["B"]))]);
        assert_eq!(run(&["a.jv"], &[], &mut k, &["a.jv", "b.jv"], &now), ["b.jv"]);
        assert_eq!(k.0.keys().map(String::as_str).collect::<Vec<_>>(), ["a.jv", "b.jv"]);
    }

    #[test]
    fn typescript_has_no_header() {
        assert_eq!(header_for(Lang::TypeScript, "a.ts", "export class A {}\n"), None);
        assert_eq!(header_for(Lang::Tsx, "a.tsx", "export const A = () => <p/>;\n"), None);
    }

    #[test]
    fn a_header_recorded_before_nested_paths_existed_reads_back_and_one_without_them_writes_none() {
        let old: Header = serde_json::from_str(r#"{"scope":["a"],"top":["B"]}"#).unwrap();
        assert_eq!(old, h(&["a"], &["B"]));
        assert_eq!(serde_json::to_string(&old).unwrap(), r#"{"scope":["a"],"top":["B"],"directives":[]}"#);
    }

    #[test]
    fn a_name_declared_by_several_files_resolves_to_all_of_them_sorted() {
        let mut idx = QualifiedIndex::default();
        idx.insert("Shop.Orders.Order", "src/b/Order.cs");
        idx.insert("Shop.Orders.Order", "src/a/Order.cs");
        assert_eq!(idx.files("Shop.Orders.Order"), ["src/a/Order.cs", "src/b/Order.cs"]);
        assert!(idx.files("Shop.Orders.Missing").is_empty());
    }

    #[test]
    fn under_lists_only_the_names_directly_below_a_scope() {
        let mut idx = QualifiedIndex::default();
        idx.insert("Shop.Orders.Order", "a.cs");
        idx.insert("Shop.Orders.Line", "b.cs");
        idx.insert("Shop.Orders.Internal.Audit", "c.cs");
        idx.insert("Shop.OrdersArchive.Old", "d.cs");
        assert_eq!(idx.under("Shop.Orders"), [("Line", vec!["b.cs"]), ("Order", vec!["a.cs"])]);
    }

    #[test]
    fn a_store_with_no_header_file_reads_as_empty_and_a_saved_one_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".repograph")).unwrap();
        let store = crate::store::Store::new(dir.path());
        assert_eq!(Headers::load(&store).unwrap(), Headers::default());
        let k = known(&[("a.jv", h(&["p"], &["A"]))]);
        k.save(&store).unwrap();
        assert_eq!(Headers::load(&store).unwrap(), k);
        std::fs::write(dir.path().join(".repograph/headers.json"), "{ not json").unwrap();
        assert_eq!(Headers::load(&store).unwrap(), Headers::default());
    }

    #[test]
    fn a_record_left_empty_removes_the_file_and_leftover_cleanup_keeps_a_full_one() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".repograph")).unwrap();
        let store = crate::store::Store::new(dir.path());
        let path = dir.path().join(".repograph").join("headers.json");
        known(&[("a.jv", h(&["p"], &["A"]))]).save(&store).unwrap();
        let mut k = Headers::load(&store).unwrap();
        assert!(run(&[], &["a.jv"], &mut k, &[], &BTreeMap::new()).is_empty());
        k.save(&store).unwrap();
        assert!(!path.exists(), "a store with no name-indexed file carries no header file");
        known(&[("a.jv", h(&["p"], &["A"]))]).save(&store).unwrap();
        store.drop_leftovers().unwrap();
        assert!(path.exists(), "a build killed before its save leaves the headers beside the graph that still answers");
    }
}
