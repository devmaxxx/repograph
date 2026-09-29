//! `use` declarations: each tree flattened into the names it binds, written as `Imports` — or
//! `ReExports` for any `pub use` — from this file to the file each path lands in.

use super::crates::{item_name, Crates, Target};
use super::items::Items;
use super::{field_text, inline_of, scoped, type_path};
use crate::code::jvm::{descend, named};
use crate::model::{EdgeKind, Extraction};
use std::collections::BTreeMap;
use tree_sitter::Node;

/// What the file's `use` declarations bind, per inline scope, keyed as `Items::scopes` is.
#[derive(Debug, Default)]
pub(crate) struct Bindings {
    pub names: BTreeMap<String, BTreeMap<String, Target>>,
    pub globs: BTreeMap<String, Vec<Target>>,
}

/// Everything a pass needs to resolve a path written in this file.
pub(crate) struct Ctx<'a> {
    pub rel: &'a str,
    pub src: &'a [u8],
    pub crates: &'a Crates,
    pub items: &'a Items,
    pub bindings: Bindings,
}

struct Flat {
    path: Vec<String>,
    bound: Option<String>,
    glob: bool,
}

/// `use a::{b::{self, C as D}, e::*}` is `a::b` bound as `b`, `a::b::C` bound as `D`, and a glob of `a::e`.
fn flatten(n: Node, src: &[u8], prefix: &[String], out: &mut Vec<Flat>) {
    let under = |x: Option<Node>| -> Vec<String> { [prefix, &x.map_or_else(Vec::new, |x| type_path(x, src))].concat() };
    match n.kind() {
        "scoped_use_list" => {
            let base = under(n.child_by_field_name("path"));
            if let Some(list) = n.child_by_field_name("list") {
                flatten(list, src, &base, out);
            }
        }
        "use_list" => {
            for item in named(n) {
                flatten(item, src, prefix, out);
            }
        }
        "use_wildcard" => out.push(Flat { path: under(n.named_child(0)), bound: None, glob: true }),
        "use_as_clause" => {
            // `as _` imports a trait for its methods and binds no name.
            let bound = field_text(n, "alias", src).filter(|a| *a != "_").map(str::to_string);
            out.push(Flat { path: under(n.child_by_field_name("path")), bound, glob: false });
        }
        _ => {
            let mut path = under(Some(n));
            if path.len() > 1 && path.last().is_some_and(|s| s == "self") {
                path.pop();
            }
            let bound = path.last().cloned();
            if !path.is_empty() {
                out.push(Flat { path, bound, glob: false });
            }
        }
    }
}

pub(crate) fn read(ctx: &mut Ctx, root: Node, ex: &mut Extraction) {
    let file_id = format!("file:{}", ctx.rel);
    let mut decls = Vec::new();
    descend(root, &mut |n| {
        if n.kind() == "use_declaration" {
            decls.push(n);
            return false;
        }
        true
    });
    for decl in decls {
        let Some(arg) = decl.child_by_field_name("argument") else { continue };
        let public = decl.named_child(0).is_some_and(|c| c.kind() == "visibility_modifier");
        // A `use` inside a function body is bound for its module scope: over-wide by one body,
        // and the only way the call pass, which reads scopes, sees it at all.
        let inline = inline_of(decl, ctx.src);
        let key = inline.join("/");
        let mut flat = Vec::new();
        flatten(arg, ctx.src, &[], &mut flat);
        for Flat { path, bound, glob } in flat {
            let Some(target) = ctx.resolve(&inline, &path) else { continue };
            let (file, name) = match &target {
                Target::Module { file, .. } => (file.clone(), None),
                Target::Item { file, name } => (file.clone(), Some(name.clone())),
            };
            if glob {
                ctx.bindings.globs.entry(key.clone()).or_default().push(target);
            } else if let Some(b) = bound {
                ctx.bindings.names.entry(key.clone()).or_default().insert(b, target);
            }
            if file == ctx.rel {
                continue;
            }
            // `impact` matches an import's context against the declared name, never an alias.
            let context = match name {
                Some(n) if !glob => n.split('.').next().unwrap_or(&n).to_string(),
                _ => "*".to_string(),
            };
            let kind = if public { EdgeKind::ReExports } else { EdgeKind::Imports };
            ex.edge(&file_id, &format!("file:{file}"), kind, &context, ctx.rel);
        }
    }
}

/// `target` followed by `rest`: into a module's file, or from a type to its associated item.
fn extend(crates: &Crates, target: &Target, rest: &[String]) -> Option<Target> {
    if rest.is_empty() {
        return Some(target.clone());
    }
    match target {
        Target::Module { file, inline } => crates.resolve(file, inline, &[vec!["self".to_string()], rest.to_vec()].concat()),
        Target::Item { file, name } => {
            let (scope, last) = match name.rsplit_once('/') {
                Some((s, l)) => (s.split('/').map(str::to_string).collect::<Vec<_>>(), l),
                None => (Vec::new(), name.as_str()),
            };
            if last.contains('.') {
                return None;
            }
            Some(Target::Item { file: file.clone(), name: item_name(&scope, &[vec![last.to_string()], rest.to_vec()].concat()) })
        }
    }
}

impl Ctx<'_> {
    /// A path written in this file at `inline`, read as rustc's uniform paths read it: what the
    /// scope itself binds — a `use`, an item, an inline module, a glob's name — before the module tree.
    pub(crate) fn resolve(&self, inline: &[String], path: &[String]) -> Option<Target> {
        let first = path.first()?;
        if !matches!(first.as_str(), "crate" | "self" | "super") {
            let key = inline.join("/");
            if let Some(t) = self.bindings.names.get(&key).and_then(|m| m.get(first)) {
                return extend(self.crates, t, &path[1..]);
            }
            if self.in_scope(self.rel, inline, first) {
                return Some(Target::Item { file: self.rel.to_string(), name: item_name(inline, path) });
            }
            for glob in self.bindings.globs.get(&key).into_iter().flatten() {
                if let Target::Module { file, inline: scope } = glob {
                    if self.in_scope(file, scope, first) {
                        return Some(Target::Item { file: file.clone(), name: item_name(scope, path) });
                    }
                }
            }
        }
        self.crates.resolve(self.rel, inline, path)
    }

    /// Whether a scope declares `name`: this file's scopes are known item by item; another
    /// file's top level is known from the line scan, and its inline modules not at all.
    fn in_scope(&self, file: &str, inline: &[String], name: &str) -> bool {
        if file == self.rel {
            self.items.scopes.get(&inline.join("/")).is_some_and(|s| s.contains(name)) || self.items.modules.contains(&scoped(inline, name))
        } else {
            inline.is_empty() && self.crates.declares(file, name)
        }
    }
}
