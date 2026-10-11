//! Paths a script computes from its own location. A script runs from anywhere, so a path means
//! something in this repository only when it is written out, or built from the script's own location:
//! `$(dirname "$0")`, `$(dirname "${BASH_SOURCE[0]}")`, `${BASH_SOURCE%/*}`, `${0%/*}`, any of those
//! under `$(cd … && pwd)`, or a variable assigned one of these. `$HOME`, `$1` and a glob name
//! nothing.
//!
//! No query can evaluate a path, so this rule supplies what the default cannot: the files a
//! `source` line imports, and the scripts a command runs by path, written as References to the file.

use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use crate::code::engine::{Facts, File, Import, Sink};
use crate::code::imports::Resolver;
use crate::code::index::Header;
use crate::code::lang::Lang;
use crate::code::prose;
use crate::code::syntax;
use crate::model::EdgeKind;

/// Interpreters a script is run through: the first operand that is not a flag is the script.
const RUNNERS: [&str; 4] = ["bash", "sh", "zsh", "dash"];

/// Every globbed script, and the paths each `source` line names, read before any extract: a
/// `source` resolves through what the walk found, whatever order it reads files in, and the
/// transitive walk asks for the imports of a file it holds only the declarations of.
#[derive(Default)]
pub(crate) struct Scripts {
    globbed: BTreeSet<String>,
    /// script → `source` site start byte → candidate paths, in the order they are tried
    sources: BTreeMap<String, BTreeMap<usize, Vec<String>>>,
}

impl Scripts {
    /// The first candidate that is a globbed script.
    fn pick(&self, candidates: &[String]) -> Option<String> {
        candidates.iter().find(|c| self.globbed.contains(c.as_str())).cloned()
    }
}

/// The collect pass: a script's `source` lines, evaluated with the variables it assigns.
pub(crate) fn collect(r: &mut Resolver, rel: &str, source: &str) -> Option<Header> {
    let scripts = r.state_mut::<Scripts>();
    scripts.globbed.insert(rel.to_string());
    let src = source.as_bytes();
    let tree = Lang::Shell.parse(src)?;
    let root = tree.root_node();
    let vars = Vars::read(root, src);
    let sites = commands(root).into_iter()
        .filter_map(|c| {
            let (name, args) = words(c, src)?;
            if name != "source" && name != "." { return None }
            Some((c.start_byte(), candidates(rel, &vars.eval(*args.first()?, src)?)))
        })
        .filter(|(_, c)| !c.is_empty())
        .collect();
    scripts.sources.insert(rel.to_string(), sites);
    None
}

/// The script a `source` line imports.
pub(crate) fn import(r: &Resolver, rel: &str, site: &Import) -> Vec<String> {
    let scripts = r.state::<Scripts>();
    scripts.sources.get(rel).and_then(|sites| sites.get(&site.range.start)).and_then(|c| scripts.pick(c)).into_iter().collect()
}

/// A command that runs another script by its path references that script's file, from the
/// function or file that runs it.
pub(crate) fn runs(r: &Resolver, file: &File, facts: &Facts, sink: &mut Sink) {
    let Some(tree) = file.tree else { return };
    let (root, src, rel) = (tree.root_node(), file.src.as_bytes(), file.rel);
    let scripts = r.state::<Scripts>();
    let vars = Vars::read(root, src);
    // A path that is only text and holds no `/` is a PATH lookup, not a script of this repository.
    let is_path = |v: &Value| !matches!(v, Value::Literal(p) if !p.contains('/'));
    for c in commands(root) {
        let Some((name, args)) = words(c, src) else { continue };
        let run = match name {
            "source" | "." => None,
            "exec" => args.first().and_then(|a| vars.eval(*a, src)).filter(is_path),
            n if RUNNERS.contains(&n) => runner_script(&args, src).and_then(|a| vars.eval(a, src)),
            _ => c.child_by_field_name("name").and_then(|n| n.named_child(0)).and_then(|w| vars.eval(w, src)).filter(is_path),
        };
        if let Some(script) = run.and_then(|v| scripts.pick(&candidates(rel, &v))) {
            let from = facts.source_id(rel, facts.owner(c.start_byte()));
            sink.edge(&from, &format!("file:{script}"), EdgeKind::References, "", rel);
        }
    }
}

fn commands<'t>(root: Node<'t>) -> Vec<Node<'t>> {
    prose::all(root, "command")
}

/// The script an interpreter runs: the first operand that is not an option. `-o` and `+o` take the
/// option name after them, and `-c` runs a command string, so no file is run.
fn runner_script<'t>(args: &[Node<'t>], src: &[u8]) -> Option<Node<'t>> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let t = syntax::text(*a, src);
        if !t.starts_with(['-', '+']) { return Some(*a) }
        let flags = &t[1..];
        if !flags.starts_with('-') && flags.contains('c') { return None }
        if matches!(t, "-o" | "+o" | "-O" | "+O") { it.next(); }
    }
    None
}

#[derive(Clone, Debug, PartialEq)]
enum Value {
    /// The script's own path, `$0` or `${BASH_SOURCE[0]}`; only its directory names anything.
    Script,
    /// A path under the script's directory: the text after it, `/lib.sh`, or empty for the directory.
    Here(String),
    /// Text written out.
    Literal(String),
}

/// Variables assigned a path value. A name assigned two different values names neither, because which
/// one holds depends on how the script ran.
struct Vars(BTreeMap<String, Option<Value>>);

impl Vars {
    fn read(root: Node, src: &[u8]) -> Vars {
        // In source order, so `M="$HERE/measure.sh"` reads the `HERE` assigned above it.
        let assignments = prose::all(root, "variable_assignment");
        let mut vars = Vars(BTreeMap::new());
        for a in assignments {
            let Some(name) = a.child_by_field_name("name") else { continue };
            let value = a.child_by_field_name("value").and_then(|v| vars.eval(v, src));
            let name = syntax::text(name, src).to_string();
            match vars.0.get(&name) {
                Some(old) if *old != value => { vars.0.insert(name, None); }
                Some(_) => {}
                None => { vars.0.insert(name, value); }
            }
        }
        vars
    }

    /// What a word evaluates to, when the script's text fixes it.
    fn eval(&self, n: Node, src: &[u8]) -> Option<Value> {
        match n.kind() {
            "word" | "string_content" => literal(syntax::text(n, src)),
            "raw_string" => literal(syntax::text(n, src).trim_matches('\'')),
            // A string's quotes are anonymous, so its named children are exactly its pieces.
            "string" | "concatenation" => syntax::named(n).into_iter()
                .try_fold(None, |acc, piece| join(acc, self.eval(piece, src)?).map(Some))?
                .or_else(|| literal("")),
            "simple_expansion" | "expansion" => self.expansion(syntax::text(n, src)),
            "command_substitution" => self.substitution(n, src),
            _ => None,
        }
    }

    /// `$HERE` or `${HERE}`, `$0` and `${BASH_SOURCE[0]}`, and the two expansions that already are the
    /// script's directory.
    fn expansion(&self, text: &str) -> Option<Value> {
        let bare = text.chars().filter(|c| !matches!(c, '{' | '}' | '"')).collect::<String>().replace("[0]", "");
        match bare.as_str() {
            "$0" | "$BASH_SOURCE" => Some(Value::Script),
            "$BASH_SOURCE%/*" | "$0%/*" => Some(Value::Here(String::new())),
            _ => {
                let name = bare.strip_prefix('$')?;
                if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') { return None }
                self.0.get(name).cloned().flatten()
            }
        }
    }

    /// `$(dirname <script>)` is the script's directory. `$(cd <dir> && pwd)` is `<dir>` with its
    /// symlinks resolved, and a path inside a repository has none to resolve.
    fn substitution(&self, n: Node, src: &[u8]) -> Option<Value> {
        let kids = syntax::named(n);
        let [inner] = kids.as_slice() else { return None };
        match inner.kind() {
            "command" => match words(*inner, src)? {
                ("dirname", args) if args.len() == 1 => (self.eval(args[0], src)? == Value::Script).then(|| Value::Here(String::new())),
                _ => None,
            },
            "list" => {
                let steps = syntax::named(*inner);
                let [cd, pwd] = steps.as_slice() else { return None };
                let (cd_name, cd_args) = words(*cd, src)?;
                let (pwd_name, _) = words(*pwd, src)?;
                if cd_name != "cd" || pwd_name != "pwd" || !syntax::text(*inner, src).contains("&&") { return None }
                let dirs: Vec<Node> = cd_args.into_iter().filter(|a| !syntax::text(*a, src).starts_with('-')).collect();
                match dirs.as_slice() {
                    [dir] => match self.eval(*dir, src)? {
                        here @ Value::Here(_) => Some(here),
                        _ => None,
                    },
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// A command's name as written, and its arguments with `--` dropped.
fn words<'t, 's>(cmd: Node<'t>, src: &'s [u8]) -> Option<(&'s str, Vec<Node<'t>>)> {
    if cmd.kind() != "command" { return None }
    let name = cmd.child_by_field_name("name")?;
    let mut c = cmd.walk();
    let args = cmd.children_by_field_name("argument", &mut c).filter(|a| syntax::text(*a, src) != "--").collect();
    Some((syntax::text(name, src), args))
}

/// Where a path value may point in the repository, in the order tried: under the script's directory,
/// then, for text written relative, from the root, where scripts are most often run from. A path that
/// climbs above the root names something outside the repository, so it has no candidate.
fn candidates(rel: &str, value: &Value) -> Vec<String> {
    let dir = rel.rsplit_once('/').map_or("", |(d, _)| d);
    let under = prose::join_under;
    match value {
        Value::Here(rest) => under(dir, rest).into_iter().collect(),
        Value::Literal(p) if !p.starts_with('/') => [under(dir, p), under("", p)].into_iter().flatten().collect(),
        _ => Vec::new(),
    }
}

/// Written-out text, unless it holds what the shell would still expand: a glob, `~`, a backtick, or a
/// `$` the grammar left inside a word.
fn literal(text: &str) -> Option<Value> {
    (!text.contains(['*', '?', '[', '~', '`', '$'])).then(|| Value::Literal(text.to_string()))
}

/// One more piece of a word. Only text may follow a directory: `$HERE/lib.sh` is a path, and
/// `lib$HERE` is not one.
fn join(acc: Option<Value>, next: Value) -> Option<Value> {
    match (acc, next) {
        (None, v) => Some(v),
        (Some(Value::Here(a)), Value::Literal(b)) => Some(Value::Here(a + &b)),
        (Some(Value::Literal(a)), Value::Literal(b)) => Some(Value::Literal(a + &b)),
        _ => None,
    }
}
