//! Razor: a component `Foo.razor` is `sym:<rel>::Foo`, exported, in the namespace the Razor compiler
//! gives it. Its `@code` and `@functions` members are read by the C# walk over the blanked copy (L6);
//! `@inject T Name` is a member with a declared type; `@inherits` and `@implements` write `Extends`;
//! a markup tag naming a component writes `Calls` from the component, because rendering it is the
//! call. A view (`.cshtml`) and `_Imports.razor` are not components — the compiler names a view's
//! class after its path and nothing names it — so each writes its file node and the `Imports` its
//! directives prove. A file whose blocks cannot be read writes its file node alone.

pub mod blank;

#[cfg(test)]
mod cases;

use std::sync::OnceLock;

use crate::code::csharp::declarations::{join, Declared, TypeDecl, Using};
use crate::code::csharp::index::Part;
use crate::code::csharp::refs::first_segment;
use crate::code::csharp::resolve::Scope;
use crate::code::csharp::{self, Host};
use crate::code::imports::Resolver;
use crate::code::index::Header;
use crate::code::lang::file_node;
use crate::model::{EdgeKind, Extraction, NodeKind};
use blank::View;

/// Where the Razor compiler puts a view's class when no `@namespace` names one; the repository
/// declares nothing there, so only the global namespace and the view's own usings reach it.
const VIEW_NAMESPACE: &str = "AspNetCoreGeneratedDocument";

/// Razor's directives in components and views: each is one line of C#, never markup.
const DIRECTIVES: [&str; 15] = [
    "page", "namespace", "using", "inject", "inherits", "implements", "model", "typeparam", "attribute",
    "layout", "rendermode", "preservewhitespace", "addTagHelper", "removeTagHelper", "tagHelperPrefix",
];

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Directives {
    pub namespace: Option<String>,
    pub usings: Vec<Using>,
    /// `@inject T Name`: (type as written, name, line).
    pub injects: Vec<(String, String, u32)>,
    /// `@inherits` and `@implements` (bases), and a view's `@model`: (type as written, line, is a base).
    pub types: Vec<(String, u32, bool)>,
    /// Upper-case markup tags outside the blocks and comments: (name, line).
    pub tags: Vec<(String, u32)>,
    /// The blocks cannot be read, so nothing the file writes past its node would be proven.
    pub unread: bool,
}

/// `Checkout` for `Pages/Checkout.razor`; None for `_Imports.razor` and for a view.
pub fn component_name(rel: &str) -> Option<String> {
    let file = rel.rsplit('/').next().unwrap_or(rel);
    file.strip_suffix(".razor").filter(|stem| *stem != "_Imports").map(str::to_string)
}

fn is_imports(rel: &str) -> bool {
    rel.rsplit('/').next() == Some("_Imports.razor")
}

fn is_name(s: &str) -> bool {
    !s.is_empty() && s.split('.').all(|w| w.starts_with(|c: char| c.is_alphabetic() || c == '_') && w.chars().all(|c| c.is_alphanumeric() || c == '_'))
}

/// `@using (Html.BeginForm())` in a view is a statement, not a directive, so a target that is not a
/// dotted name is none.
fn using(text: &str) -> Option<Using> {
    let text = text.trim().trim_end_matches(';').trim();
    let using = match text.strip_prefix("static ") {
        Some(t) => Using::Static(t.trim().to_string()),
        None => match text.split_once('=') {
            Some((alias, target)) => Using::Alias(alias.trim().to_string(), target.trim().to_string()),
            None => Using::Namespace(text.to_string()),
        },
    };
    let names = match &using {
        Using::Namespace(n) | Using::Static(n) => vec![n.as_str()],
        Using::Alias(a, t) => vec![a.as_str(), t.as_str()],
    };
    names.iter().all(|n| is_name(n.trim_start_matches("global::"))).then_some(using)
}

/// The type names a directive writes: `IStringLocalizer<Shop.Checkout>` gives both.
fn type_words(written: &str) -> Vec<String> {
    written.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
        .filter(|w| w.starts_with(|c: char| c.is_alphabetic() || c == '_'))
        .map(str::to_string)
        .collect()
}

/// `@* ... *@` and `<!-- ... -->` in markup: Razor reads neither a directive nor a component tag
/// inside one. One that never closes runs to the end of the file, as Razor reads it.
fn comments(src: &str, spans: &[(usize, usize, usize)]) -> Vec<(usize, usize)> {
    let b = src.as_bytes();
    let find = |needle: &[u8], from: usize| b[from.min(b.len())..].windows(needle.len()).position(|w| w == needle).map_or(b.len(), |p| from + p + needle.len());
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if let Some(&(_, _, close)) = spans.iter().find(|&&(at, _, close)| i >= at && i <= close) {
            i = close + 1;
            continue;
        }
        // `@@*` is an escaped `@`, and `a@*b` is text, as `blank` reads them.
        let razor = b[i..].starts_with(b"@*") && !(i > 0 && (b[i - 1] == b'@' || b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_'));
        let end = if razor {
            find(b"*@", i + 2)
        } else if b[i..].starts_with(b"<!--") {
            find(b"-->", i + 4)
        } else {
            i += 1;
            continue;
        };
        out.push((i, end));
        i = end;
    }
    out
}

pub fn directives(src: &str) -> Directives {
    let Ok(spans) = blank::blocks(src) else { return Directives { unread: true, ..Directives::default() } };
    let hidden = comments(src, &spans);
    let inside = |at: usize| spans.iter().any(|&(_, open, close)| at > open && at < close) || hidden.iter().any(|&(s, e)| at >= s && at < e);
    let mut d = Directives { unread: matches!(blank::view(src), View::Unread(_)), ..Directives::default() };
    // A directive's line is C#, not markup: `@inject IStringLocalizer<Checkout> L` renders nothing.
    let mut lines = Vec::new();
    let mut start = 0;
    for (i, raw) in src.split('\n').enumerate() {
        let (line, at) = (i as u32 + 1, start);
        start += raw.len() + 1;
        // `trim` keeps U+FEFF, so a BOM'd first-line directive would not start with its `@`.
        let l = if i == 0 { raw.strip_prefix(blank::BOM).unwrap_or(raw) } else { raw }.trim();
        if inside(at + raw.len() - raw.trim_start().len()) {
            continue;
        }
        let arg = |word: &str| l.strip_prefix(word).filter(|r| r.starts_with([' ', '\t'])).map(str::trim);
        let word = l.strip_prefix('@').map(|r| r.split(|c: char| !c.is_alphanumeric()).next().unwrap_or(""));
        if word.is_some_and(|w| DIRECTIVES.contains(&w)) {
            lines.push(at..start);
        }
        if let Some(ns) = arg("@namespace") {
            d.namespace = Some(ns.to_string());
        } else if let Some(u) = arg("@using") {
            match using(u) {
                Some(u) => d.usings.push(u),
                // A using statement's body is markup.
                None => {
                    lines.pop();
                }
            }
        } else if let Some(rest) = arg("@inject") {
            if let Some((t, name)) = rest.rsplit_once([' ', '\t']) {
                d.injects.push((t.trim().to_string(), name.trim().trim_start_matches('@').to_string(), line));
            }
        } else if let Some(t) = arg("@inherits").or_else(|| arg("@implements")) {
            d.types.push((t.to_string(), line, true));
        } else if let Some(t) = arg("@model") {
            d.types.push((t.to_string(), line, false));
        }
    }
    static TAG: OnceLock<regex::Regex> = OnceLock::new();
    let tag = TAG.get_or_init(|| regex::Regex::new(r"<([A-Z][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)[\s/>]").unwrap());
    for m in tag.captures_iter(src) {
        let at = m.get(0).map_or(0, |g| g.start());
        if !inside(at) && !lines.iter().any(|l| l.contains(&at)) {
            d.tags.push((m[1].to_string(), src[..at].matches('\n').count() as u32 + 1));
        }
    }
    d
}

/// A component's header: its name, and its own `@namespace` when it writes one. The computed
/// namespace needs the project, which `header_for` cannot see; `DotNet::component_parts` has it.
/// An `_Imports.razor` declares nothing, but its usings and namespace decide what every component
/// below it resolves, so its header's directives are those lines: `widen` re-reads the family when
/// they move.
pub fn header(rel: &str, source: &str) -> Header {
    let mut h = Header::default();
    let d = directives(source);
    if d.unread {
        return h;
    }
    if let Some(name) = component_name(rel) {
        h.scope.extend(d.namespace);
        h.top.insert(name);
    } else if is_imports(rel) {
        h.directives.extend(d.usings.iter().map(|u| format!("@{}", u.spelled())));
        h.directives.extend(d.namespace.map(|n| format!("@namespace {n}")));
    }
    h
}

/// `Imports` for every name a directive's type writes; the parts of its first name, for `Extends`.
fn imports(scope: &Scope, rel: &str, written: &str, namespace: &str, class: Option<&str>, ex: &mut Extraction) -> Vec<Part> {
    let file = format!("file:{rel}");
    let mut first = Vec::new();
    for (i, word) in type_words(written).into_iter().enumerate() {
        let parts = scope.types(&word, namespace, class);
        for p in parts.iter().filter(|p| p.rel != rel) {
            ex.edge(&file, &format!("file:{}", p.rel), EdgeKind::Imports, first_segment(&p.local), rel);
        }
        if i == 0 {
            first = parts;
        }
    }
    first
}

pub fn extract(resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let mut ex = Extraction::default();
    file_node(rel, &mut ex);
    let d = directives(source);
    if d.unread {
        return ex;
    }
    let dotnet = resolver.dotnet();
    let Some(name) = component_name(rel) else {
        view(resolver, rel, &d, &mut ex);
        return ex;
    };
    let mut usings = dotnet.razor_usings(rel);
    usings.extend(d.usings.iter().cloned());
    let namespace = dotnet.razor_namespace(rel, d.namespace.as_deref());
    let file = format!("file:{rel}");
    let id = format!("sym:{rel}::{name}");
    let last = source.lines().count().max(1) as u32;
    let signature = source.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    ex.node_span(NodeKind::Symbol, &id, &name, signature, rel, (1, last));
    ex.edge(&file, &id, EdgeKind::Declares, "export", rel);
    let injected: Vec<(String, String)> = d.injects.iter()
        .filter_map(|(t, n, _)| Some((n.clone(), type_words(t).into_iter().next()?)))
        .collect();
    for (t, n, line) in &d.injects {
        let member = format!("{id}.{n}");
        ex.node_span(NodeKind::Symbol, &member, &format!("{name}.{n}"), &format!("@inject {t} {n}"), rel, (*line, *line));
        ex.edge(&id, &member, EdgeKind::Declares, "", rel);
    }
    let host = Host { component: Some(&name), namespace: &namespace, usings: &usings, injected: &injected };
    let own = match blank::view(source) {
        View::Read(text) => csharp::read(resolver, rel, &text, &host, &mut ex),
        // Without a block the component still has injected members to resolve through. `Unread`
        // returned above, with `d.unread`.
        View::None | View::Unread(_) => Declared {
            types: vec![TypeDecl {
                namespace: namespace.clone(),
                local: name.clone(),
                partial: true,
                members: injected.iter().map(|(n, t)| (n.clone(), Some(t.clone()))).collect(),
                ..TypeDecl::default()
            }],
            ..Declared::default()
        },
    };
    let scope = Scope::new(rel, dotnet, &own, &host);
    for (t, _, _) in &d.injects {
        imports(&scope, rel, t, &namespace, Some(&name), &mut ex);
    }
    for (t, _, base) in &d.types {
        let parts = imports(&scope, rel, t, &namespace, None, &mut ex);
        if *base {
            for p in &parts {
                ex.edge(&id, &format!("sym:{}::{}", p.rel, p.local), EdgeKind::Extends, "", rel);
            }
        }
    }
    for (tag, _) in &d.tags {
        let parts = scope.types(tag, &namespace, Some(&name));
        // Razor renders a component and reads any other upper-case tag as an element; a component
        // written only in C# is a missing edge here, not a class guessed to be one.
        if !parts.iter().any(|p| component_name(&p.rel).is_some()) {
            continue;
        }
        for p in parts.iter().filter(|p| p.rel != rel) {
            ex.edge(&id, &format!("sym:{}::{}", p.rel, p.local), EdgeKind::Calls, "", rel);
            ex.edge(&file, &format!("file:{}", p.rel), EdgeKind::Imports, first_segment(&p.local), rel);
        }
    }
    // A code-behind `partial class` is part of the component, so each names the other.
    for p in scope.full(&join(&namespace, &name)).into_iter().filter(|p| p.rel != rel) {
        ex.edge(&file, &format!("file:{}", p.rel), EdgeKind::Imports, first_segment(&p.local), rel);
    }
    ex
}

/// A view or an imports file: the `Imports` its own directives' types prove. `_Imports.razor` is
/// for components, and a view's class sits in the namespace its own `@namespace` names or in
/// `VIEW_NAMESPACE`, never the project's.
fn view(resolver: &Resolver, rel: &str, d: &Directives, ex: &mut Extraction) {
    let namespace = d.namespace.clone().unwrap_or_else(|| VIEW_NAMESPACE.to_string());
    let own = Declared::default();
    let host = Host { namespace: &namespace, usings: &d.usings, ..Host::default() };
    let scope = Scope::new(rel, resolver.dotnet(), &own, &host);
    for written in d.injects.iter().map(|(t, _, _)| t).chain(d.types.iter().map(|(t, _, _)| t)) {
        imports(&scope, rel, written, &namespace, None, ex);
    }
}
