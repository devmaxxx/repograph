//! Razor: a component `Foo.razor` is `sym:<rel>::Foo`, exported, in the namespace the Razor compiler
//! gives it. Its `@code` and `@functions` members are read by the C# walk over the blanked copy (L6);
//! `@inject T Name` is a member with a declared type; `@inherits` and `@implements` write `Extends`;
//! a markup tag naming a component writes `Calls` from the component, because rendering it is the
//! call. A view (`.cshtml`) and `_Imports.razor` are not components — the compiler names a view's
//! class after its path and nothing names it — so each writes its file node and the `Imports` its
//! directives prove. A file whose blocks cannot be read writes its file node alone.

pub(crate) mod blank;

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use crate::code::engine::{self, Facts, File, Sink};
use crate::code::exceptions::namespace_scope::declared::{self, join, Declared, TypeDecl, Using};
use crate::code::exceptions::namespace_scope::lookup::Scope;
use crate::code::exceptions::namespace_scope::project::{DotNet, Part};
use crate::code::exceptions::namespace_scope::walk::{self, first_segment};
use crate::code::exceptions::namespace_scope::Host;
use crate::code::exceptions::package_scope;
use crate::code::imports::Resolver;
use crate::code::index::Header;
use crate::code::lang::Lang;
use crate::code::queries;
use crate::code::spec::{self, Embedded, Hooks};
use crate::model::{EdgeKind, NodeKind};
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
    /// `@inherits` was written with type arguments, which `inherits` drops.
    pub inherits_generic: bool,
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
/// `global::` anchors a name at the global namespace, which is where the index already keys every
/// full name, so `@using global::Shop.Payments` is `@using Shop.Payments`.
fn using(text: &str) -> Option<Using> {
    let text = text.trim().trim_end_matches(';').trim();
    let target = |t: &str| t.trim().trim_start_matches("global::").to_string();
    let using = match text.strip_prefix("static ") {
        Some(t) => Using::Static(target(t)),
        None => match text.split_once('=') {
            Some((alias, t)) => Using::Alias(alias.trim().to_string(), target(t)),
            None => Using::Namespace(target(text)),
        },
    };
    let names = match &using {
        Using::Namespace(n) | Using::Static(n) => vec![n.as_str()],
        Using::Alias(a, t) => vec![a.as_str(), t.as_str()],
    };
    names.iter().all(|n| is_name(n)).then_some(using)
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
        let l = &without_comments(l);
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
            d.inherits = Some(base_name(t)).filter(|b| !b.is_empty());
            d.inherits_generic = last_segment_generic(t);
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
    header_of(rel, &directives(source))
}

/// `header`, from directives already read: the collector reads them again right after for
/// `add_razor`, and a second scan of the same file would cost what the first one did.
pub fn header_of(rel: &str, d: &Directives) -> Header {
    let mut h = Header::default();
    if d.unread {
        return h;
    }
    if let Some(name) = component_name(rel) {
        h.scope.extend(d.namespace.clone());
        h.top.insert(name);
    } else if is_imports(rel) {
        h.directives.extend(d.usings.iter().map(|u| format!("@{}", u.spelled())));
        h.directives.extend(d.namespace.clone().map(|n| format!("@namespace {n}")));
    }
    h
}

/// `Imports` for every name a directive's type writes.
fn imports(scope: &Scope, rel: &str, written: &str, namespace: &str, class: Option<&str>, ex: &mut Sink) {
    let file = format!("file:{rel}");
    for word in type_words(written) {
        for p in scope.types(&word, namespace, class).iter().filter(|p| p.rel != rel) {
            ex.edge(&file, &format!("file:{}", p.rel), EdgeKind::Imports, first_segment(&p.local), rel);
        }
    }
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
        if let Some((facts, tree)) = package_scope::declared::facts_of(Lang::CSharp, rel, &text) {
            let mut facts = Arc::unwrap_or_clone(facts);
            stand_for(&mut facts, rel, &name);
            let host = Host { component: Some(&name), injected: &injected, ..Host::default() };
            let own = declared::read(&facts, tree.root_node(), text.as_bytes(), &host);
            members.extend(own.types.into_iter().filter(|t| t.local == name).flat_map(|t| t.members));
        }
    }
    members
}

/// Razor's own directives feed both its header and `add_razor`, read once for both. Its state is
/// the C# reader's.
pub(crate) const HOOKS: Hooks = Hooks {
    header: Some(header),
    collect: Some(|r, rel, source| {
        let d = directives(source);
        r.state_mut::<DotNet>().add_razor(rel, &d, members(rel, source, &d));
        Some(header_of(rel, &d))
    }),
    embed: Some(|source| match blank::view(source) {
        View::Unread(_) => Embedded::Unread,
        View::Read(text) => Embedded::View(text, Lang::CSharp),
        View::None => Embedded::Empty,
    }),
    resolve: Some(resolve),
    ..spec::NONE
};

/// The blanking's wrapper class stands for the component: it, and every name under it, takes the
/// component's name.
fn stand_for(facts: &mut Facts, rel: &str, name: &str) {
    let Some(wrapper) = facts.defs.iter().find(|d| d.parent.is_none()).map(|d| d.qualified.clone()) else { return };
    for d in &mut facts.defs {
        let Some(rest) = d.qualified.strip_prefix(&wrapper).filter(|r| r.is_empty() || r.starts_with('.')) else { continue };
        d.qualified = format!("{name}{rest}");
        d.label = d.qualified.clone();
        d.id = format!("sym:{rel}::{}", d.qualified);
    }
}

/// The front parsed the blanked view, and has left an unread file at its file node; a file with no
/// block comes with no tree. A tree is `None` for no other reason, as the C# grammar always parses.
pub(crate) fn resolve(resolver: &Resolver, file: &File, _: &Facts, ex: &mut Sink) {
    let (rel, source, tree, blanked) = (file.rel, file.text, file.tree, file.src);
    let d = directives(source);
    let dotnet = resolver.state::<DotNet>();
    let mut usings = dotnet.razor_usings(rel);
    usings.extend(d.usings.iter().cloned());
    let namespace = dotnet.razor_namespace(rel, d.namespace.as_deref());
    let Some(name) = component_name(rel) else {
        view(resolver, rel, &d, &namespace, &usings, ex);
        return;
    };
    let file_id = format!("file:{rel}");
    let id = format!("sym:{rel}::{name}");
    let last = source.lines().count().max(1) as u32;
    let signature = source.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    // The component spans the file and reads as its first line, whatever its blocks hold.
    let view = File { lang: Lang::CSharp, rel, text: source, src: blanked, tree };
    let mut facts = tree.and_then(|_| Some(engine::facts(spec::spec(Lang::CSharp), queries::decl(Lang::CSharp)?, queries::refs(Lang::CSharp), &view)));
    let wrapper = facts.as_mut().and_then(|facts| {
        stand_for(facts, rel, &name);
        facts.defs.iter_mut().find(|d| d.parent.is_none())
    });
    match wrapper {
        Some(w) => (w.body, w.line, w.end, w.export) = (signature.to_string(), 1, last, true),
        None => {
            ex.node(NodeKind::Symbol, &id, &name, signature, rel, (1, last));
            ex.edge(&file_id, &id, EdgeKind::Declares, "export", rel);
        }
    }
    let injected = injected(&d);
    for (t, n, line) in &d.injects {
        let member = format!("{id}.{n}");
        ex.node(NodeKind::Symbol, &member, &format!("{name}.{n}"), &format!("@inject {t} {n}"), rel, (*line, *line));
        ex.edge(&id, &member, EdgeKind::Declares, "", rel);
    }
    let host = Host { component: Some(&name), namespace: &namespace, usings: &usings, injected: &injected };
    let mut own = match (tree, &facts) {
        (Some(tree), Some(facts)) => {
            ex.facts(&view, facts);
            let own = declared::read(facts, tree.root_node(), blanked.as_bytes(), &host);
            walk::scan(tree.root_node(), rel, blanked.as_bytes(), &host, &own, resolver, ex);
            own
        }
        // Without a block the component still has injected members to resolve through.
        _ => Declared {
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
            if d.inherits_generic {
                t.generic_bases.insert(base.clone());
            }
        }
    }
    let scope = Scope::new(rel, dotnet, &own, &host);
    for (t, _, _) in &d.injects {
        imports(&scope, rel, t, &namespace, Some(&name), ex);
    }
    for (t, _, base) in &d.types {
        imports(&scope, rel, t, &namespace, None, ex);
        // A base's name is every segment with its type arguments dropped: the first word of
        // `Outer<int>.InnerBase` is `Outer`, which is not the base.
        let name = base_name(t);
        if *base && !name.is_empty() {
            for p in &scope.types(&name, &namespace, None) {
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
            if inherited || walked.unresolved.iter().any(|(b, generic)| framework_parameters(b, *generic).is_none_or(|ps| ps.contains(&tag.as_str()))) {
                continue;
            }
        }
        for p in rendered(tag).into_iter().filter(|p| p.rel != rel) {
            ex.edge(&id, &format!("sym:{}::{}", p.rel, p.local), EdgeKind::Calls, "", rel);
            ex.edge(&file_id, &format!("file:{}", p.rel), EdgeKind::Imports, first_segment(&p.local), rel);
        }
    }
    // A code-behind `partial class` is part of the component, so each names the other.
    for p in scope.full(&join(&namespace, &name)).into_iter().filter(|p| p.rel != rel) {
        ex.edge(&file_id, &format!("file:{}", p.rel), EdgeKind::Imports, first_segment(&p.local), rel);
    }
}

/// The parameters a framework base declares, `None` for a base the repo cannot list. The interfaces
/// a code-behind lists beside its base declare none. A framework name under another namespace, or
/// with type arguments the framework's type does not take, is a library's type of the same name,
/// whose parameters no one can read.
fn framework_parameters(base: &str, generic: bool) -> Option<&'static [&'static str]> {
    let head = base_name(base);
    let (qualifier, name) = head.rsplit_once('.').map_or((None, head.as_str()), |(q, n)| (Some(q), n));
    let (namespace, parameters): (_, &'static [&'static str]) = match name {
        "ComponentBase" | "OwningComponentBase" => ("Microsoft.AspNetCore.Components", &[]),
        "LayoutComponentBase" => ("Microsoft.AspNetCore.Components", &["Body"]),
        "IDisposable" | "IAsyncDisposable" => ("System", &[]),
        _ => return None,
    };
    let arity = !generic || name == "OwningComponentBase";
    (qualifier.is_none_or(|q| q == namespace) && arity).then_some(parameters)
}

/// A written type as a C# base list keeps it: dots, without `global::` or any segment's type
/// arguments, so `Outer<int>.InnerBase` is `Outer.InnerBase`. Two words left side by side
/// (`A B`) name no type, and joining them would name one the text never wrote, so that is empty.
pub(crate) fn base_name(written: &str) -> String {
    let t = written.trim();
    let t = t.strip_prefix("global::").unwrap_or(t);
    let mut depth = 0usize;
    let mut out = String::new();
    for c in t.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let joined: Vec<&str> = out.split('.').map(str::trim).collect();
    if joined.iter().any(|s| s.contains(char::is_whitespace)) {
        return String::new();
    }
    joined.join(".")
}

/// Whether the type's own name, not an outer type it is nested in, carries type arguments.
fn last_segment_generic(written: &str) -> bool {
    let mut depth = 0usize;
    let mut generic = false;
    for c in written.chars() {
        match c {
            '<' => {
                generic |= depth == 0;
                depth += 1;
            }
            '>' => depth = depth.saturating_sub(1),
            '.' if depth == 0 => generic = false,
            _ => {}
        }
    }
    generic
}

/// A directive line without its `@* … *@` comments; one left open runs to the end of the line,
/// since the comment, not the argument, owns what follows.
fn without_comments(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(at) = rest.find("@*") {
        out.push_str(&rest[..at]);
        match rest[at + 2..].find("*@") {
            Some(end) => rest = &rest[at + 2 + end + 2..],
            None => rest = "",
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

/// A view or an imports file: the `Imports` its directives' types prove, read under its imports
/// files' usings and the namespace the Razor compiler gives its class.
fn view(resolver: &Resolver, rel: &str, d: &Directives, namespace: &str, usings: &[Using], ex: &mut Sink) {
    let own = Declared::default();
    let host = Host { namespace, usings, ..Host::default() };
    let scope = Scope::new(rel, resolver.state::<DotNet>(), &own, &host);
    for written in d.injects.iter().map(|(t, _, _)| t).chain(d.types.iter().map(|(t, _, _)| t)) {
        imports(&scope, rel, written, namespace, None, ex);
    }
}
