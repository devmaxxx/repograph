//! What a command is to the graph. `source f` and `. f` import a script. A bare name that a function
//! of this file, or of a script it sources, has is a call. A path to another script runs it.

use super::paths::{candidates, words, Value, Vars};
use super::Scripts;
use crate::code::prose::{self, Spans};
use crate::model::{EdgeKind, Extraction};
use tree_sitter::Node;

/// Interpreters a script is run through: the first operand that is not a flag is the script.
const RUNNERS: [&str; 4] = ["bash", "sh", "zsh", "dash"];

fn commands<'t>(root: Node<'t>) -> Vec<Node<'t>> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.kind() == "command" { out.push(n) }
        stack.extend(prose::named(n));
    }
    out
}

/// Each `source` the script names, as candidate paths. `Scripts` picks among them once every globbed
/// script is known.
pub(super) fn sources(root: Node, src: &[u8], rel: &str) -> Vec<Vec<String>> {
    let vars = Vars::read(root, src);
    commands(root).into_iter()
        .filter_map(|c| {
            let (name, args) = words(c, src)?;
            if name != "source" && name != "." { return None }
            Some(candidates(rel, &vars.eval(*args.first()?, src)?))
        })
        .filter(|c| !c.is_empty())
        .collect()
}

pub(super) fn write(scripts: &Scripts, root: Node, spans: &Spans, src: &[u8], rel: &str, ex: &mut Extraction) {
    let vars = Vars::read(root, src);
    let file = format!("file:{rel}");
    for script in scripts.sourced(rel) {
        // `*`: sourcing runs the whole file, so `importers` counts it for every function there.
        ex.edge(&file, &format!("file:{script}"), EdgeKind::Imports, "*", rel);
    }
    let visible = scripts.visible(rel);
    // A path that is only text and holds no `/` is a PATH lookup, not a script of this repository.
    let is_path = |v: &Value| !matches!(v, Value::Literal(p) if !p.contains('/'));
    for c in commands(root) {
        let Some((name, args)) = words(c, src) else { continue };
        let from = spans.owner(c.start_byte());
        if !name.contains(['/', '$', '"', '\'']) {
            let defined = if scripts.defines(rel, name) { Some(rel) } else { visible.get(name).copied() };
            if let Some(script) = defined {
                let target = format!("sym:{script}::{name}");
                if target != from { ex.edge(from, &target, EdgeKind::Calls, "", rel) }
            }
        }
        let run = match name {
            "source" | "." => None,
            "exec" => args.first().and_then(|a| vars.eval(*a, src)).filter(is_path),
            n if RUNNERS.contains(&n) => args.iter()
                .find(|a| !prose::text(**a, src).starts_with('-'))
                .and_then(|a| vars.eval(*a, src)),
            _ => c.child_by_field_name("name").and_then(|n| n.named_child(0)).and_then(|w| vars.eval(w, src)).filter(is_path),
        };
        if let Some(script) = run.and_then(|v| scripts.pick(&candidates(rel, &v))) {
            ex.edge(from, &format!("file:{script}"), EdgeKind::References, "", rel);
        }
    }
}
