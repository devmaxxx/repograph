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

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::code::csharp::declarations::{self, join, Declared, TypeDecl, Using};
use crate::code::csharp::index::Part;
use crate::code::csharp::refs::first_segment;
use crate::code::csharp::resolve::Scope;
use crate::code::csharp::{self, Host};
use crate::code::imports::Resolver;
use crate::code::index::Header;
use crate::code::lang::{file_node, Lang};
use crate::model::{EdgeKind, Extraction, NodeKind};
use blank::View;

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
    /// Upper-case markup tags outside the blocks and comments: (name, line, the upper-case tag
    /// open around it). A tag nested in a component can be that component's parameter, not a render.
    pub tags: Vec<(String, u32, Option<String>)>,
    /// `@inherits`, as written: the component's base class, where its inherited parameters live.
    pub inherits: Option<String>,
    /// The blocks cannot be read, so nothing the file writes past its node would be proven.
    pub unread: bool,
}

/// `Checkout` for `Pages/Checkout.razor`; None for `_Imports.razor` and for a view.
pub fn component_name(rel: &str) -> Option<String> {
    let file = rel.rsplit('/').next().unwrap_or(rel);
    file.strip_suffix(".razor").filter(|stem| *stem != "_Imports").map(str::to_string)
}

/// `_Imports.razor` for components, `_ViewImports.cshtml` for views.
pub(crate) fn is_imports(rel: &str) -> bool {
    matches!(rel.rsplit('/').next(), Some("_Imports.razor" | "_ViewImports.cshtml"))
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
        // A using statement (`@using (Html.BeginForm()) { … }`) is no directive, and its body is markup.
        let statement = word == Some("using") && arg("@using").and_then(using).is_none();
        if word.is_some_and(|w| DIRECTIVES.contains(&w)) && !statement {
            lines.push(at..start);
        }
        if let Some(ns) = arg("@namespace") {
            d.namespace = Some(ns.to_string());
        } else if let Some(u) = arg("@using") {
            d.usings.extend(using(u));
        } else if let Some(rest) = arg("@inject") {
            if let Some((t, name)) = rest.rsplit_once([' ', '\t']) {
                d.injects.push((t.trim().to_string(), name.trim().trim_start_matches('@').to_string(), line));
            }
        } else if let Some(t) = arg("@inherits") {
            d.inherits = Some(base_name(t));
            d.types.push((t.to_string(), line, true));
        } else if let Some(t) = arg("@implements") {
            d.types.push((t.to_string(), line, true));
        } else if let Some(t) = arg("@model") {
            d.types.push((t.to_string(), line, false));
        }
    }
    static TAG: OnceLock<regex::Regex> = OnceLock::new();
    let tag = TAG.get_or_init(|| regex::Regex::new(r"<(/?)([A-Z][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)[\s/>]").unwrap());
    let b = src.as_bytes();
    // The upper-case tags open around the current one, innermost last.
    let mut open: Vec<String> = Vec::new();
    for m in tag.captures_iter(src) {
        let whole = m.get(0).expect("group 0 always matches");
        let at = whole.start();
        // A markup tag never follows a name: `List<Badge>` and `OfType<Badge>()` are C# generics. A
        // closing tag may follow text (`<Ext>Save</Ext>`), so only an opening one is tested.
        let generic = m[1].is_empty() && at > 0 && (b[at - 1].is_ascii_alphanumeric() || b[at - 1] == b'_' || b[at - 1] == b'.');
        if generic || inside(at) || lines.iter().any(|l| l.contains(&at)) {
            continue;
        }
        let name = m[2].to_string();
        if !m[1].is_empty() {
            if let Some(i) = open.iter().rposition(|o| *o == name) {
                open.truncate(i);
            }
            continue;
        }
        d.tags.push((name.clone(), src[..at].matches('\n').count() as u32 + 1, open.last().cloned()));
        if !self_closing(b, whole.end() - 1) {
            open.push(name);
        }
    }
    d
}

/// Whether the tag whose name ends before `from` closes itself: its `>`, past quoted attribute
/// values, follows a `/`.
fn self_closing(b: &[u8], from: usize) -> bool {
    let mut quote = None;
    for i in from..b.len() {
        match (quote, b[i]) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, b'"' | b'\'') => quote = Some(b[i]),
            (None, b'>') => return i > 0 && b[i - 1] == b'/',
            _ => {}
        }
    }
    false
}

/// A component's header: its name, and its own `@namespace` when it writes one. The computed
/// namespace needs the project, which `header_for` cannot see; `DotNet::component_parts` has it.
/// An imports file declares nothing, but its usings and namespace decide what every component
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

/// `@inject T Name` as (name, type head).
fn injected(d: &Directives) -> Vec<(String, String)> {
    d.injects.iter().filter_map(|(t, n, _)| Some((n.clone(), type_words(t).into_iter().next()?))).collect()
}

/// A component's members as the index keeps them: its `@code`/`@functions` members and its
/// `@inject`s, each with its declared type head. Another file needs them to tell a parameter tag
/// from a rendered one, and to resolve a code-behind's call into a block.
pub fn members(rel: &str, source: &str, d: &Directives) -> BTreeMap<String, Option<String>> {
    let Some(name) = component_name(rel) else { return BTreeMap::new() };
    let injected = injected(d);
    let mut members: BTreeMap<String, Option<String>> = injected.iter().map(|(n, t)| (n.clone(), Some(t.clone()))).collect();
    if let View::Read(text) = blank::view(source) {
        if let Some(tree) = Lang::CSharp.parse(text.as_bytes()) {
            let host = Host { component: Some(&name), injected: &injected, ..Host::default() };
            let own = declarations::scan(tree.root_node(), rel, text.as_bytes(), &host, &mut Extraction::default());
            members.extend(own.types.into_iter().filter(|t| t.local == name).flat_map(|t| t.members));
        }
    }
    members
}

pub fn extract(resolver: &Resolver, rel: &str, source: &str) -> Extraction {
    let mut ex = Extraction::default();
    file_node(rel, &mut ex);
    let text = match blank::view(source) {
        View::Unread(_) => return ex,
        View::Read(text) => Some(text),
        View::None => None,
    };
    let d = directives(source);
    let dotnet = resolver.dotnet();
    let mut usings = dotnet.razor_usings(rel);
    usings.extend(d.usings.iter().cloned());
    let namespace = dotnet.razor_namespace(rel, d.namespace.as_deref());
    let Some(name) = component_name(rel) else {
        view(resolver, rel, &d, &namespace, &usings, &mut ex);
        return ex;
    };
    let file = format!("file:{rel}");
    let id = format!("sym:{rel}::{name}");
    let last = source.lines().count().max(1) as u32;
    let signature = source.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    ex.node_span(NodeKind::Symbol, &id, &name, signature, rel, (1, last));
    ex.edge(&file, &id, EdgeKind::Declares, "export", rel);
    let injected = injected(&d);
    for (t, n, line) in &d.injects {
        let member = format!("{id}.{n}");
        ex.node_span(NodeKind::Symbol, &member, &format!("{name}.{n}"), &format!("@inject {t} {n}"), rel, (*line, *line));
        ex.edge(&id, &member, EdgeKind::Declares, "", rel);
    }
    let host = Host { component: Some(&name), namespace: &namespace, usings: &usings, injected: &injected };
    let mut own = match text {
        Some(text) => csharp::read(resolver, rel, &text, &host, &mut ex),
        // Without a block the component still has injected members to resolve through.
        None => Declared {
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
    // The wrapper class is blanked from `@inherits`; its base is written back so the tag loop can
    // walk the chain for inherited parameters.
    if let Some(base) = &d.inherits {
        for t in own.types.iter_mut().filter(|t| t.local == name) {
            t.bases.push(base.clone());
        }
    }
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
    // Razor renders a component and reads any other upper-case tag as an element, so only the parts
    // of a component count; a component written only in C# is a missing edge, not a guess.
    let rendered = |tag: &str| -> Vec<Part> {
        let mut parts = scope.component_types(tag, &namespace);
        parts.retain(|p| !dotnet.component_parts(&p.full).is_empty());
        parts
    };
    for (tag, _, parent) in &d.tags {
        // Inside a component, a tag naming one of its members, or a member of a base it inherits, is a
        // parameter (`<Card><Header>`). Inside one whose parameters the repo cannot list — not declared
        // here, or on a base from a library — any child could be one.
        if let Some(parent) = parent {
            let owner = rendered(parent);
            if owner.is_empty() || owner.iter().any(|p| scope.members(p).is_some_and(|m| m.contains_key(tag))) {
                continue;
            }
            let mut inherited = false;
            let walked = scope.walk_bases(&owner, |level| {
                inherited = level.iter().any(|p| scope.members(p).is_some_and(|m| m.contains_key(tag)));
                inherited
            });
            if inherited || walked.unresolved.iter().any(|b| framework_parameters(b).is_none_or(|ps| ps.contains(&tag.as_str()))) {
                continue;
            }
        }
        for p in rendered(tag).into_iter().filter(|p| p.rel != rel) {
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

/// The parameters a framework base declares, `None` for a base the repo cannot list. The interfaces
/// a code-behind lists beside its base declare none.
fn framework_parameters(base: &str) -> Option<&'static [&'static str]> {
    let head = base_name(base);
    match head.rsplit('.').next().unwrap_or(&head) {
        "ComponentBase" | "OwningComponentBase" | "IDisposable" | "IAsyncDisposable" => Some(&[]),
        "LayoutComponentBase" => Some(&["Body"]),
        _ => None,
    }
}

/// A written type as a C# base list keeps it: dots, without generic arguments or `global::`.
fn base_name(written: &str) -> String {
    let t = written.trim();
    let t = t.strip_prefix("global::").unwrap_or(t);
    t.split('<').next().unwrap_or(t).trim().to_string()
}

/// A view or an imports file: the `Imports` its directives' types prove, read under its imports
/// files' usings and the namespace the Razor compiler gives its class.
fn view(resolver: &Resolver, rel: &str, d: &Directives, namespace: &str, usings: &[Using], ex: &mut Extraction) {
    let own = Declared::default();
    let host = Host { namespace, usings, ..Host::default() };
    let scope = Scope::new(rel, resolver.dotnet(), &own, &host);
    for written in d.injects.iter().map(|(t, _, _)| t).chain(d.types.iter().map(|(t, _, _)| t)) {
        imports(&scope, rel, written, namespace, None, ex);
    }
}
