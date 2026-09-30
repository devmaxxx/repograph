//! Shell through the binary: a sourced function is impacted by the scripts calling it, a call chain is
//! traced, a script run by another is an edge the walks do not follow, and a function added to a
//! sourced file is called after an `update` that re-read only that file.

mod common;

use common::{ok, path, write};

const QUIET: &str = "pids() {\n  ps -o pid= -p \"$$\"\n}\nbusy() {\n  echo \" $(pids) \"\n}\nmain() {\n  busy\n}\nwait_quiet() {\n  until main; do sleep 1; done\n}\nwait_quiet\n";
const RUN: &str = "HERE=\"$(cd \"$(dirname \"${BASH_SOURCE[0]}\")\" && pwd)\"\n. \"$HERE/lib.sh\" || exit 2\nlib_line start\nlib_extra\n\"$HERE/quiet.sh\" --wait 600\n";

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    // Shell is read only behind an explicit glob until its readings pass.
    write(r, "repograph.toml", "doc_globs = []\ncode_globs = [\"**/*.sh\"]\n");
    write(r, "probe/lib.sh", "lib_line() {\n  printf '%s\\n' \"$1\"\n}\n");
    write(r, "probe/quiet.sh", QUIET);
    write(r, "probe/run.sh", RUN);
    dir
}

#[test]
fn a_sourced_function_is_impacted_by_the_script_calling_it() {
    let dir = repo();
    ok(dir.path(), &["build"]);
    let out = ok(dir.path(), &["impact", "lib_line", "--depth", "3"]);
    assert!(out.contains("file:probe/run.sh"), "{out}");
    assert!(out.contains("importers (1): probe/run.sh"), "{out}");
}

#[test]
fn a_call_chain_is_traced_and_a_script_run_by_another_is_not_walked() {
    let dir = repo();
    let r = dir.path();
    ok(r, &["build"]);
    let out = ok(r, &["trace", "wait_quiet", "pids"]);
    assert_eq!(path(&out), ["sym:probe/quiet.sh::wait_quiet", "sym:probe/quiet.sh::main", "sym:probe/quiet.sh::busy", "sym:probe/quiet.sh::pids"], "{out}");
    // The run is in the graph, but a `References` edge to a file is not one the walks follow.
    let out = ok(r, &["explain", "probe/quiet.sh"]);
    assert!(out.contains("References \u{2190} file:probe/run.sh"), "{out}");
    let out = common::run(r, &["trace", "probe/run.sh", "pids"]);
    assert_eq!(out.status.code(), Some(3), "no path: {}", String::from_utf8_lossy(&out.stdout));
    let out = ok(r, &["impact", "probe/quiet.sh", "--depth", "3"]);
    assert!(!out.contains("probe/run.sh"), "{out}");
}

#[test]
fn a_function_added_to_a_sourced_script_is_called_after_an_update_of_that_script_alone() {
    let dir = repo();
    let r = dir.path();
    ok(r, &["build"]);
    // `run.sh` has called `lib_extra` since the build; only `lib.sh` changes.
    write(r, "probe/lib.sh", "lib_line() {\n  printf '%s\\n' \"$1\"\n}\nlib_extra() {\n  :\n}\n");
    ok(r, &["update"]);
    let out = ok(r, &["impact", "lib_extra", "--depth", "3"]);
    assert!(out.contains("file:probe/run.sh"), "the script sourcing lib.sh was read again: {out}");
}

#[test]
fn a_package_style_function_is_impacted_without_a_phantom_importer() {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    write(r, "repograph.toml", "doc_globs = []\ncode_globs = [\"**/*.sh\"]\n");
    // Google's shell style names a library's functions `package::function`, one `word` to bash.
    write(r, "lib.sh", "log::info() {\n  :\n}\n");
    write(r, "run.sh", ". ./lib.sh\nlog::info start\n");
    ok(r, &["build"]);
    let out = ok(r, &["impact", "sym:lib.sh::log::info", "--depth", "3"]);
    assert!(out.contains("file:run.sh"), "{out}");
    assert!(out.contains("importers (1): run.sh"), "{out}");
    assert!(!out.contains("lib.sh::log,") && !out.contains("lib.sh::log\n"), "the id split on its last `::`: {out}");
}
