//! A day with the binary, on the path a person keeps a project under: a repository directory that
//! is Cyrillic and holds a space, a `repograph.toml` written the way an editor on Windows writes
//! one, a build, a resident `serve` and a question answered through its socket, `enrich` through
//! the configured shell against a stand-in for `claude`, an edit, `update`, `changes`, and finally
//! the server killed the way a person kills one — leaving a socket file for the next `serve` to
//! sweep. Every one of those is exercised somewhere on its own; none of them together, and none
//! under a path that is not ASCII.
//!
//! Written as a test rather than as a shell script on purpose: the three platforms this runs on
//! then run the same code, instead of three scripts that have to be kept saying the same thing.
//! What genuinely differs is the README's PowerShell recipe for starting a background server,
//! which is a claim about PowerShell and is checked on Windows, in the job, where it means
//! something.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The bytes a socket path may take, its terminator included: 104 on macOS (`sys/un.h`'s
/// `sun_path[104]`), 108 on Linux and in Windows' `SOCKADDR_UN`. socket2 and std both refuse a
/// path that needs the whole array, so a usable one is shorter than this.
#[cfg(target_os = "macos")]
const SUN_LEN: usize = 104;
#[cfg(not(target_os = "macos"))]
const SUN_LEN: usize = 108;

/// Cyrillic and a space, and ASCII file names underneath it: git's default `core.quotepath`
/// escapes a non-ASCII *file* name in its diff headers, which is a finding of its own and not
/// this test's subject. The directory is what the socket, the walk and `git -C` are judged on.
const REPO_DIR: &str = "Проект beauty crm";

/// Where else to look when the platform's own temp directory leaves no room. `/tmp` is the
/// shortest root every unix has, and on macOS a symlink to `/private/tmp`, which is what a bind
/// sees — so the length is measured after canonicalizing. Windows' temp directory is short
/// enough on its own, and there is no equivalent short root to fall back to.
#[cfg(unix)]
const SHORTER_ROOTS: &[&str] = &["/tmp"];
#[cfg(not(unix))]
const SHORTER_ROOTS: &[&str] = &[];

fn repograph() -> Command { Command::new(env!("CARGO_BIN_EXE_repograph")) }

/// A temp directory whose repository's socket path still fits `sun_path`. macOS hands out
/// `/private/var/folders/…/T/` — around 57 bytes before anything of ours — and the name above
/// plus `.repograph/serve.sock` wants 46 more, which is over that platform's 104. So the root is
/// chosen by the length it leaves rather than taken as given, and if none is short enough the
/// test says the number instead of failing inside a bind.
fn short_root() -> tempfile::TempDir {
    let roots = std::iter::once(std::env::temp_dir()).chain(SHORTER_ROOTS.iter().map(PathBuf::from));
    let mut longest = String::new();
    for root in roots {
        let Ok(dir) = tempfile::Builder::new().prefix("rg").tempdir_in(&root) else { continue };
        let real = dir.path().canonicalize().unwrap_or_else(|_| dir.path().to_path_buf());
        let sock = real.join(REPO_DIR).join(".repograph").join("serve.sock");
        let len = sock.to_string_lossy().len();
        if len < SUN_LEN { return dir; }
        longest = format!("{len} bytes at {}", sock.display());
    }
    panic!("no temp root leaves a socket path under {SUN_LEN} bytes: {longest}");
}

/// `git` with an identity of its own and the machine's configuration kept out of it: a runner has
/// no `user.name`, a developer has a signing key and hooks, and all this repository has to do is
/// hold one commit for `changes --base HEAD` to diff against.
fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(repo)
        .env_remove("GIT_DIR").env_remove("GIT_WORK_TREE").env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR").env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        .args(["-c", "user.name=repograph tests", "-c", "user.email=tests@example.invalid", "-c", "commit.gpgsign=false"])
        .args(args).output().unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

struct Run { out: String, err: String }

fn run(repo: &Path, args: &[&str], shell_path: Option<&Path>) -> Run {
    let mut cmd = repograph();
    cmd.arg("--no-dense").arg("--repo").arg(repo).args(args);
    // On the child, never on this process: tests share one environment and run at once.
    if let Some(dir) = shell_path { cmd.env("PATH", enrich_path(dir)); }
    let out = cmd.output().unwrap();
    let r = Run {
        out: String::from_utf8_lossy(&out.stdout).into_owned(),
        err: String::from_utf8_lossy(&out.stderr).into_owned(),
    };
    assert!(out.status.success(), "repograph {args:?} exited {}: {}{}", out.status, r.out, r.err);
    r
}

/// The PATH the `enrich` child gets: the stand-in `claude` first, and on unix only the two
/// directories a shell and `awk` live in after it. A developer machine has a real `claude` on its
/// PATH, and an inherited one would let it answer — slowly, with that person's tokens, and green.
/// Windows keeps the rest of the PATH: nothing there is a `claude`, and the shell resolver reads
/// it to find Git's `sh`. What makes a real answer red on either is the assertion on the text.
fn enrich_path(bin: &Path) -> std::ffi::OsString {
    let mut dirs = vec![bin.to_path_buf()];
    #[cfg(unix)]
    dirs.extend([PathBuf::from("/bin"), PathBuf::from("/usr/bin")]);
    #[cfg(windows)]
    {
        let inherited = std::env::var_os("PATH").unwrap_or_default();
        dirs.extend(std::env::split_paths(&inherited));
    }
    std::env::join_paths(dirs).unwrap()
}

/// A shell script named the way the default `enrich_command` names it — no extension, a `#!`
/// line — because that is the shape npm's own shim for a node CLI has, and the reason the
/// configured command runs under `sh` on every platform rather than under `cmd`.
fn fake_claude(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let claude = dir.join("claude");
    std::fs::write(&claude, "#!/bin/sh\nawk '/^### /{printf \"%s\\tq for %s\\n\", $2, $2}'\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&claude, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn a_repository_under_a_cyrillic_directory_with_a_space_is_built_served_asked_enriched_and_diffed() {
    let root = short_root();
    let repo = root.path().join(REPO_DIR);
    std::fs::create_dir_all(repo.join("docs")).unwrap();
    std::fs::write(repo.join("docs/pay.md"), "**FR-PAY-1 · MUST · Штраф за отмену**\n\nШтраф списывается сам (INV-1).\n\n**INV-1 · MUST · Деньги не сгорают**\n\nОтмена не сжигает деньги.\n").unwrap();
    std::fs::write(repo.join("docs/cal.md"), "**FR-CAL-1 · MUST · Перенос визита**\n\nПеренос не считается отменой.\n").unwrap();
    std::fs::write(repo.join("README.md"), "# Проект\n\nЗаписи и оплаты.\n").unwrap();
    // What the README tells a user to write, so the store this test makes is not also a file the
    // diff below has to explain.
    std::fs::write(repo.join(".gitignore"), ".repograph/\n").unwrap();
    // A byte-order mark and CRLF endings: the bytes Notepad and Visual Studio write, and the
    // bytes a file authored on Windows still has when a colleague reads it on Linux. toml_parser
    // drops the mark in its lexer, which is why this is expected to parse rather than to fail.
    std::fs::write(repo.join("repograph.toml"), "\u{feff}id_families = [\"FR-PAY\", \"FR-CAL\", \"INV\"]\r\n").unwrap();
    git(&repo, &["-c", "init.defaultBranch=main", "init", "-q"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "--no-verify", "-m", "the project as it stands"]);

    let built = run(&repo, &["build"], None);
    let nodes = built.out.split_whitespace().skip_while(|w| *w != "nodes").nth(1).unwrap_or("0");
    assert!(nodes.parse::<u32>().unwrap_or(0) > 0, "the walk found nothing under this path: {}{}", built.out, built.err);

    let mut server = repograph().arg("--no-dense").arg("--repo").arg(&repo)
        .args(["serve", "--every", "3600", "--idle", "120"])
        .stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    let direct = run(&repo, &["ask", "--no-serve", "штраф"], None).out;
    assert!(direct.contains("FR-PAY-1"), "{direct}");
    let start = Instant::now();
    let resident = loop {
        let r = run(&repo, &["ask", "штраф"], None);
        if r.err.contains("serve: answered by the resident process") { break r; }
        assert!(start.elapsed() < Duration::from_secs(30), "serve never answered on its socket under this path: {}", r.err);
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(resident.out, direct, "the resident answer is the process's answer");

    // The line a user's `enrich` really runs: the configured default, `claude` on PATH, through
    // whichever `sh` this platform resolves — Git for Windows' where std has no shell of its own.
    // Nothing else spawns the default command; the other tests hand `run_command` an awk line.
    let bin = root.path().join("bin");
    fake_claude(&bin);
    let enriched = run(&repo, &["enrich", "--batch", "1", "--parallel", "1"], Some(&bin));
    let questions = std::fs::read_to_string(repo.join(".repograph/questions.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&questions).unwrap();
    assert!(parsed["entries"].as_object().is_some_and(|e| !e.is_empty()),
        "the default enrich command wrote no questions: {}{}", enriched.out, questions);
    // The stand-in's own words, not merely some words: anything else answering — a real `claude`
    // that outlived the PATH above — would otherwise pass this for the wrong reason.
    assert!(questions.contains("q for FR-PAY-1"), "something other than the stand-in answered: {questions}");

    // An edit under a live server, then the two commands that read git from this path.
    let mut readme = std::fs::read_to_string(repo.join("README.md")).unwrap();
    readme.push_str("\nОтмена визита оплачивается.\n");
    std::fs::write(repo.join("README.md"), readme).unwrap();
    let updated = run(&repo, &["update"], None);
    assert!(updated.out.starts_with("changed 1 "), "one file changed: {}", updated.out);
    let changes = run(&repo, &["changes", "--base", "HEAD"], None);
    assert!(changes.out.contains("README.md"), "`git -C` under this path found no diff: {}{}", changes.out, changes.err);

    // Killed, which is what `Stop-Process` and a closed terminal both are: no exit runs, so the
    // socket file stays. The README says so, and the next `serve` is what sweeps it.
    let sock = repo.join(".repograph/serve.sock");
    server.kill().unwrap();
    let _ = server.wait();
    assert!(std::fs::symlink_metadata(&sock).is_ok(), "a killed server left no socket to sweep");
    let swept = repograph().arg("--no-dense").arg("--repo").arg(&repo).args(["serve", "--idle", "1"]).output().unwrap();
    let err = String::from_utf8_lossy(&swept.stderr).into_owned();
    assert!(swept.status.success(), "the next serve would not bind over the dead socket: {err}");
    assert!(err.contains("idle for 1s"), "{err}");
    assert!(std::fs::symlink_metadata(&sock).is_err(), "and took its own socket with it: {err}");
}
