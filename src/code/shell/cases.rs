//! Shell's shapes, each on an inline script, so the grammar's reading of a source line, a call or a
//! run is pinned by the assertion rather than by a corpus.

use crate::code::prose::testing::{ids, lines, repo};
use crate::model::{EdgeKind, Extraction};

const GLOBS: &[&str] = &["**/*.sh", "**/*.bash"];

fn extract(files: &[(&str, &str)], rel: &str) -> Extraction {
    let (_dir, resolver) = repo(GLOBS, files);
    let text = files.iter().find(|(f, _)| *f == rel).map(|(_, t)| *t).unwrap();
    super::extract(&resolver, rel, text)
}

const FUNCTIONS: &str = r#"#!/usr/bin/env bash
# Prints a greeting.
greet() {
  echo "hi"
}
function bye {
  greet
}
refuse() { echo "$*" >&2; exit 2; }
recurse() { recurse; }
"#;

#[test]
fn both_function_forms_are_exported_symbols_with_their_spans_and_a_call_between_them() {
    let ex = extract(&[("a.sh", FUNCTIONS)], "a.sh");
    assert_eq!(ids(&ex), ["file:a.sh", "sym:a.sh::bye", "sym:a.sh::greet", "sym:a.sh::recurse", "sym:a.sh::refuse"]);
    assert!(lines(&ex, EdgeKind::Declares).iter().all(|l| l.ends_with("[export]")), "{:?}", lines(&ex, EdgeKind::Declares));
    let node = |id: &str| ex.nodes.iter().find(|n| n.id == id).unwrap();
    assert_eq!(node("sym:a.sh::greet").body, "# Prints a greeting.\ngreet() {", "the shebang is not documentation");
    assert_eq!((node("sym:a.sh::greet").line, node("sym:a.sh::greet").end), (3, 5));
    assert_eq!((node("sym:a.sh::bye").line, node("sym:a.sh::bye").end), (6, 8));
    assert_eq!((node("sym:a.sh::refuse").line, node("sym:a.sh::refuse").end), (9, 9));
    assert_eq!(lines(&ex, EdgeKind::Calls), ["sym:a.sh::bye -> sym:a.sh::greet []"], "a function calling itself adds no edge");
}

const SOURCES: &str = r#"HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
. "$HERE/lib.sh" || exit 2
source "${BASH_SOURCE%/*}/two.sh"
source "$(dirname -- "$0")/three.sh"
. "$ROOT/shared/four.sh"
source ${0%/*}/five.sh
LIB=lib/six.sh
source "$LIB"
. ./seven.sh
source "$HOME/.bashrc"
source "$1"
for f in "$HERE"/*.sh; do source "$f"; done
X=a.sh
X=b.sh
source "$X"
helper
"#;

#[test]
fn a_source_path_built_from_the_scripts_own_directory_is_imported_and_nothing_else_is() {
    let files = [
        ("tools/run.sh", SOURCES), ("tools/lib.sh", "helper() { :; }\n"), ("tools/two.sh", ""), ("tools/three.sh", ""),
        ("shared/four.sh", ""), ("tools/five.sh", ""), ("lib/six.sh", ""), ("tools/seven.sh", ""), ("a.sh", ""), ("b.sh", ""),
    ];
    let ex = extract(&files, "tools/run.sh");
    assert_eq!(lines(&ex, EdgeKind::Imports), [
        "file:tools/run.sh -> file:lib/six.sh [*]",
        "file:tools/run.sh -> file:shared/four.sh [*]",
        "file:tools/run.sh -> file:tools/five.sh [*]",
        "file:tools/run.sh -> file:tools/lib.sh [*]",
        "file:tools/run.sh -> file:tools/seven.sh [*]",
        "file:tools/run.sh -> file:tools/three.sh [*]",
        "file:tools/run.sh -> file:tools/two.sh [*]",
    ], "$HOME, $1, a loop variable and a variable assigned twice name nothing");
    assert_eq!(lines(&ex, EdgeKind::Calls), ["file:tools/run.sh -> sym:tools/lib.sh::helper []"]);
}

const RUNS: &str = r#"HERE="$(cd "$(dirname "$0")" && pwd)"
check() {
  "$HERE/quiet.sh" --wait 600
}
bash tools/build.sh
./tools/deploy.sh
exec "$HERE/final.sh"
M="$HERE/measure.sh"
"$M" --n 5
sh missing.sh
node script.js
"#;

#[test]
fn running_a_script_references_its_file_from_the_function_or_file_that_runs_it() {
    let files = [
        ("tools/ci.sh", RUNS), ("tools/quiet.sh", ""), ("tools/build.sh", ""), ("tools/deploy.sh", ""),
        ("tools/final.sh", ""), ("tools/measure.sh", ""),
    ];
    let ex = extract(&files, "tools/ci.sh");
    assert_eq!(lines(&ex, EdgeKind::References), [
        "file:tools/ci.sh -> file:tools/build.sh []",
        "file:tools/ci.sh -> file:tools/deploy.sh []",
        "file:tools/ci.sh -> file:tools/final.sh []",
        "file:tools/ci.sh -> file:tools/measure.sh []",
        "sym:tools/ci.sh::check -> file:tools/quiet.sh []",
    ]);
}

#[test]
fn a_call_resolves_to_this_file_or_through_what_it_sources_transitively() {
    let files = [
        ("a.sh", ". ./b.sh\nlog() { log_impl \"$@\"; }\nlog start\n"),
        ("b.sh", ". ./c.sh\nlog_impl() { stamp; }\n"),
        ("c.sh", "stamp() { date; }\n"),
    ];
    let a = extract(&files, "a.sh");
    assert_eq!(lines(&a, EdgeKind::Calls), ["file:a.sh -> sym:a.sh::log []", "sym:a.sh::log -> sym:b.sh::log_impl []"]);
    assert_eq!(lines(&a, EdgeKind::Imports), ["file:a.sh -> file:b.sh [*]"]);
    assert_eq!(lines(&extract(&files, "b.sh"), EdgeKind::Calls), ["sym:b.sh::log_impl -> sym:c.sh::stamp []"]);
}

#[test]
fn an_id_in_a_comment_or_string_is_cited_by_what_holds_it() {
    let ex = extract(&[("a.sh", "# ADR-004 explains the wait\nwait_quiet() {\n  echo \"FR-APP-54\"\n}\n")], "a.sh");
    assert_eq!(lines(&ex, EdgeKind::References), ["file:a.sh -> ADR-004 [comment]", "sym:a.sh::wait_quiet -> FR-APP-54 [string]"]);
}

#[test]
fn a_script_under_a_dotted_directory_sources_and_calls_like_any_other() {
    let files = [
        (".github/scripts/deploy.sh", "HERE=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\n. \"$HERE/lib.sh\"\nlib_line start\n"),
        (".github/scripts/lib.sh", "lib_line() { :; }\n"),
        // `.git` is the one dotted directory the walk never enters.
        (".git/hooks/lib.sh", "lib_line() { :; }\n"),
    ];
    let ex = extract(&files, ".github/scripts/deploy.sh");
    assert_eq!(lines(&ex, EdgeKind::Imports), ["file:.github/scripts/deploy.sh -> file:.github/scripts/lib.sh [*]"]);
    assert_eq!(lines(&ex, EdgeKind::Calls), ["file:.github/scripts/deploy.sh -> sym:.github/scripts/lib.sh::lib_line []"]);
}

#[test]
fn a_path_climbing_above_the_repository_root_names_no_script() {
    let main = "source ../../x.sh\nbash ../../x.sh\nsource \"$(dirname \"$0\")/../../x.sh\"\n";
    let ex = extract(&[("a/main.sh", main), ("x.sh", "")], "a/main.sh");
    assert!(lines(&ex, EdgeKind::Imports).is_empty(), "{:?}", lines(&ex, EdgeKind::Imports));
    assert!(lines(&ex, EdgeKind::References).is_empty(), "{:?}", lines(&ex, EdgeKind::References));
    let ok = extract(&[("a/main.sh", "source ../x.sh\n"), ("x.sh", "")], "a/main.sh");
    assert_eq!(lines(&ok, EdgeKind::Imports), ["file:a/main.sh -> file:x.sh [*]"]);
}

#[test]
fn a_name_defined_in_more_than_one_reachable_script_is_no_call() {
    let files = [
        ("main.sh", ". ./c.sh\n. ./e.sh\nrun() { f; g; }\nown() { h; }\n. ./h.sh\nh() { :; }\n"),
        ("c.sh", "f() { :; }\n"),
        ("e.sh", ". ./d.sh\n"),
        ("d.sh", "f() { :; }\ng() { :; }\n"),
        ("h.sh", "h() { :; }\n"),
    ];
    assert_eq!(lines(&extract(&files, "main.sh"), EdgeKind::Calls), ["sym:main.sh::run -> sym:d.sh::g []"]);
}

#[test]
fn an_interpreter_run_skips_options_and_a_command_string() {
    let files = [("a.sh", "bash -o pipefail x.sh\nbash -e -x y.sh\nbash -c x.sh\nsh -ec y.sh\n"), ("x.sh", ""), ("y.sh", "")];
    let ex = extract(&files, "a.sh");
    assert_eq!(lines(&ex, EdgeKind::References), ["file:a.sh -> file:x.sh []", "file:a.sh -> file:y.sh []"]);
    let none = extract(&[("a.sh", "bash -o pipefail\nbash -c x.sh\n"), ("pipefail", ""), ("x.sh", "")], "a.sh");
    assert!(lines(&none, EdgeKind::References).is_empty());
}
