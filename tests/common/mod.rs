//! The one place these tests spawn the binary.
//!
//! The child's environment is the parent's less the two variables the bench kit exports, because
//! each of them redirects what a command reads. `REPOGRAPH_BENCH_REPO` moves `bench` off the
//! `--repo` it was given — with it exported, the floors-missed case reads another corpus, finds no
//! graph and exits 1 where the suite is pinning a 3 — and `REPOGRAPH_EMBED_MODEL` outranks the
//! model a store records, so a shell left over from a measurement answers these tests under other
//! weights. Both are invisible in the failure they cause, which is why they are stripped here
//! rather than documented somewhere a reader would have to find.

use std::path::Path;
use std::process::{Command, Output};

pub fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_repograph"))
        .env_remove("REPOGRAPH_BENCH_REPO")
        .env_remove("REPOGRAPH_EMBED_MODEL")
        .arg("--no-dense")
        .arg("--repo")
        .arg(repo)
        .args(args)
        .output()
        .unwrap()
}
