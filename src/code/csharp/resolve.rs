//! C#'s name lookup, in the order the compiler applies it: the enclosing type and the types around
//! it, each enclosing namespace outward to the global one, then aliases, then `using` and
//! `using static` from the file, its host and its project's `global using`s. The first step that
//! finds a declaration wins; inside a step every declaring file is kept, so a partial type
//! resolves to all its parts, as `expect`/`actual` does in Kotlin.

use std::collections::BTreeMap;

use super::declarations::{join, Declared, Using};
use super::index::{DotNet, Part};
use super::Host;

pub struct Scope<'a> {
    rel: &'a str,
    dotnet: &'a DotNet,
    own: &'a Declared,
    usings: Vec<Using>,
}

impl<'a> Scope<'a> {
    pub fn new(rel: &'a str, dotnet: &'a DotNet, own: &'a Declared, host: &Host) -> Scope<'a> {
        let mut usings = own.usings.clone();
        usings.extend(own.global_usings.iter().cloned());
        usings.extend(host.usings.iter().cloned());
        usings.extend(dotnet.global_usings(rel));
        Scope { rel, dotnet, own, usings }
    }

    pub(crate) fn usings(&self) -> &[Using] {
        &self.usings
    }

    pub(crate) fn dotnet(&self) -> &'a DotNet {
        self.dotnet
    }

    pub(crate) fn own(&self) -> &'a Declared {
        self.own
    }

    /// Every part of the fully qualified type `full`: this file's from its tree, others' from the index.
    pub fn full(&self, full: &str) -> Vec<Part> {
        let mut out: Vec<Part> = self.own.parts(full).into_iter()
            .map(|t| Part { rel: self.rel.to_string(), local: t.local.clone(), full: full.to_string() })
            .collect();
        out.extend(self.dotnet.parts(full, self.rel).into_iter().cloned());
        out.sort();
        out.dedup();
        out
    }

    pub fn types(&self, name: &str, namespace: &str, class: Option<&str>) -> Vec<Part> {
        if let Some(c) = class {
            let mut chain: Vec<&str> = c.split('.').collect();
            while !chain.is_empty() {
                let hits = self.full(&join(namespace, &format!("{}.{name}", chain.join("."))));
                if !hits.is_empty() {
                    return hits;
                }
                chain.pop();
            }
        }
        let mut ns: Vec<&str> = namespace.split('.').filter(|s| !s.is_empty()).collect();
        loop {
            let hits = self.full(&join(&ns.join("."), name));
            if !hits.is_empty() {
                return hits;
            }
            if ns.pop().is_none() {
                break;
            }
        }
        let (first, rest) = name.split_once('.').map_or((name, None), |(f, r)| (f, Some(r)));
        for u in &self.usings {
            if let Using::Alias(alias, target) = u {
                if alias == first {
                    return self.full(&rest.map_or_else(|| target.clone(), |r| join(target, r)));
                }
            }
        }
        let mut hits: Vec<Part> = self.usings.iter()
            .filter_map(|u| match u {
                Using::Namespace(n) | Using::Static(n) => Some(self.full(&join(n, name))),
                Using::Alias(..) => None,
            })
            .flatten()
            .collect();
        hits.sort();
        hits.dedup();
        hits
    }

    /// The members one part declares, each with its declared type head.
    pub fn members(&self, p: &Part) -> Option<&'a BTreeMap<String, Option<String>>> {
        if p.rel == self.rel {
            return self.own.types.iter().find(|t| t.local == p.local && t.full() == p.full).map(|t| &t.members);
        }
        self.dotnet.members(&p.full, &p.rel, &p.local)
    }

    /// `sym:` ids of `member` on each part that declares it.
    pub fn member_ids(&self, parts: &[Part], member: &str) -> Vec<String> {
        parts.iter()
            .filter(|p| self.members(p).is_some_and(|m| m.contains_key(member)))
            .map(|p| format!("sym:{}::{}.{member}", p.rel, p.local))
            .collect()
    }

    /// `member` through a type name: `Receipt.Create()`, or a receiver declared as `type_name`.
    pub fn member_of(&self, type_name: &str, member: &str, namespace: &str, class: Option<&str>) -> Vec<String> {
        self.member_ids(&self.types(type_name, namespace, class), member)
    }

    /// The enclosing type and each type around it, innermost first.
    fn enclosing(&self, namespace: &str, class: Option<&str>) -> Vec<Vec<Part>> {
        let Some(c) = class else { return Vec::new() };
        let mut chain: Vec<&str> = c.split('.').collect();
        let mut out = Vec::new();
        while !chain.is_empty() {
            out.push(self.full(&join(namespace, &chain.join("."))));
            chain.pop();
        }
        out
    }

    /// An unqualified call or `this.M()`: `member` on the enclosing type, then the types around it.
    pub fn enclosing_member(&self, member: &str, namespace: &str, class: Option<&str>) -> Vec<String> {
        self.enclosing(namespace, class).iter().map(|parts| self.member_ids(parts, member)).find(|ids| !ids.is_empty()).unwrap_or_default()
    }

    /// The declared type of a field, property, event, primary-constructor parameter or `@inject` of the enclosing types.
    pub fn enclosing_member_type(&self, member: &str, namespace: &str, class: Option<&str>) -> Option<String> {
        self.enclosing(namespace, class).iter().flatten().find_map(|p| self.members(p)?.get(member)?.clone())
    }

    /// `M()` imported by `using static T;`.
    pub fn static_member(&self, member: &str) -> Vec<String> {
        self.usings.iter()
            .filter_map(|u| match u {
                Using::Static(t) => Some(self.member_ids(&self.full(t), member)),
                _ => None,
            })
            .flatten()
            .collect()
    }

    /// `base.M()`: `member` on the bases this file writes for the enclosing type, read in the scope around it.
    pub fn base_member(&self, member: &str, namespace: &str, class: Option<&str>) -> Vec<String> {
        let Some(c) = class else { return Vec::new() };
        let outer = c.rsplit_once('.').map(|(o, _)| o);
        self.own.parts(&join(namespace, c)).iter()
            .flat_map(|t| t.bases.iter())
            .flat_map(|b| self.member_of(b, member, namespace, outer))
            .collect()
    }

    /// An extension method on a receiver whose type the repository does not declare: resolved only
    /// when exactly one type declaring an extension of that name sits in a namespace the file
    /// imports — a using, or an enclosing namespace. Two candidates and the file proves neither.
    pub fn extension(&self, method: &str, namespace: &str) -> Vec<String> {
        let mut imported: Vec<String> = self.usings.iter()
            .filter_map(|u| match u {
                Using::Namespace(n) => Some(n.clone()),
                _ => None,
            })
            .collect();
        let mut ns = namespace.to_string();
        loop {
            imported.push(ns.clone());
            match ns.rsplit_once('.') {
                Some((outer, _)) => ns = outer.to_string(),
                None => break,
            }
        }
        imported.push(String::new());
        let own = self.own.types.iter().filter(|t| t.extensions.contains(method)).map(|t| (t.namespace.clone(), t.full()));
        let mut owners: Vec<String> = self.dotnet.extensions(method).into_iter().chain(own)
            .filter(|(n, _)| imported.contains(n))
            .map(|(_, full)| full)
            .collect();
        owners.sort();
        owners.dedup();
        match owners.as_slice() {
            [only] => self.member_ids(&self.full(only), method),
            _ => Vec::new(),
        }
    }
}
