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
}
