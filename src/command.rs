//! Runs a configured model command: the prompt on stdin, the answer on stdout, under `sh` in an
//! empty directory of its own. `ask --rerank` is the one caller.
use anyhow::{Context, Result};
use std::io::Write;
use std::path::PathBuf;

/// The shell the configured command runs under: `sh`, on every platform. The commands in
/// `repograph.toml` are written in its syntax — a `VAR=value` prefix, `""` for an empty argument —
/// and a `cmd /C` or PowerShell rendering on Windows would make one key mean two things.
#[cfg(unix)]
fn shell() -> Result<std::process::Command> { Ok(std::process::Command::new("sh")) }

/// The same `sh`, which on Windows is Git for Windows'. Its default install puts `git` on PATH and
/// not `sh` — the Unix tools are an opt-in — so a `sh` PATH does not resolve is looked for beside
/// `git`, at the bash Claude Code names for its own Bash tool, and where the installer puts it;
/// found that way, Git's `usr\bin` goes on the child's PATH too, since that is where `awk` and the
/// rest of what a command may call live.
#[cfg(windows)]
fn shell() -> Result<std::process::Command> {
    use std::path::PathBuf;
    let path_dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if path_dirs.iter().any(|d| d.join("sh.exe").is_file()) { return Ok(std::process::Command::new("sh")); }
    let bash = std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH").map(PathBuf::from);
    let program_dirs: Vec<PathBuf> = [("ProgramFiles", ""), ("ProgramW6432", ""), ("LOCALAPPDATA", "Programs")].iter()
        .filter_map(|(k, sub)| std::env::var_os(k).map(|v| PathBuf::from(v).join(sub))).collect();
    let sh = git_sh(&path_dirs, bash.as_deref(), &program_dirs)
        .context("`sh` is not on PATH and no Git for Windows was found beside `git` or under Program Files: enrich and rerank run their command under sh, which Git for Windows provides")?;
    let mut cmd = std::process::Command::new(&sh);
    let root = sh.parent().and_then(std::path::Path::parent).map(std::path::Path::to_path_buf).unwrap_or_default();
    cmd.env("PATH", std::env::join_paths(std::iter::once(root.join("usr").join("bin")).chain(path_dirs))?);
    Ok(cmd)
}

/// `<Git>\bin\sh.exe` for the first `<Git>` that has one: the parent or grandparent of a directory
/// on `path_dirs` holding `git.exe` (`cmd\` and `mingw64\bin\` are both one install), the
/// grandparent of `bash_hint`, then `<dir>\Git` for each of `program_dirs`.
#[cfg(windows)]
fn git_sh(path_dirs: &[std::path::PathBuf], bash_hint: Option<&std::path::Path>, program_dirs: &[std::path::PathBuf]) -> Option<std::path::PathBuf> {
    let sh_in = |root: &std::path::Path| { let p = root.join("bin").join("sh.exe"); p.is_file().then_some(p) };
    let beside_git = path_dirs.iter().filter(|d| d.join("git.exe").is_file())
        .flat_map(|d| d.ancestors().skip(1).take(2).map(std::path::Path::to_path_buf).collect::<Vec<_>>());
    let named = bash_hint.and_then(|b| b.parent()?.parent()).map(std::path::Path::to_path_buf);
    let installed = program_dirs.iter().map(|d| d.join("Git"));
    beside_git.chain(named).chain(installed).find_map(|root| sh_in(&root))
}

/// `sh -c 'a | b'` returns b's status, so a generator that dies into a `tee` looks like success —
/// which is how one run reported 167 batches, 0 failed and nothing written. Shells that have
/// `pipefail` are asked for it; those that do not fall back to the empty-answer check in `run`,
/// which is the load-bearing half anyway: a pipeline's exit status is the operator's to get right,
/// an empty answer is nobody's to mistake for one.
fn pipefail_prefix() -> &'static str {
    static P: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let ok = *P.get_or_init(|| {
        shell()
            .and_then(|mut c| Ok(c.arg("-c").arg("set -o pipefail")
                .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status()?))
            .is_ok_and(|s| s.success())
    });
    if ok { "set -o pipefail; " } else { "" }
}

/// An empty directory of the command's own to run in. The generator is a tool like `claude -p`
/// that reads project files from its working directory — a `CLAUDE.md`, a settings file — and the
/// directory repograph was started in is a cloned repository's, whose files steer the tool
/// instead of the person who configured the command.
struct EmptyCwd(PathBuf);

impl EmptyCwd {
    /// The counter is what keeps two batches of one process apart: macOS reads the clock to the
    /// microsecond, so batches started together drew the same name and one of them failed. A name
    /// someone else made first is skipped rather than entered, and the directory is the owner's
    /// alone, so a shared temp directory lends the command nothing.
    fn new() -> Result<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = { let mut b = builder; std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o700); b };
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        for _ in 0..16 {
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("repograph-cwd-{}-{nanos}-{n}", std::process::id()));
            match builder.create(&dir) {
                Ok(()) => return Ok(Self(dir)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e).with_context(|| format!("create {}", dir.display())),
            }
        }
        anyhow::bail!("create an empty directory under {}: every name tried was taken", std::env::temp_dir().display())
    }
}

impl Drop for EmptyCwd {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

/// What of a command's stderr goes into an error. A generator that fails verbosely would
/// otherwise put its whole log, prompt echo included, into the run's output and `background.log`.
const STDERR_ECHO: usize = 2048;

fn clip_stderr(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let text = text.trim();
    if text.len() <= STDERR_ECHO { return text.to_string(); }
    let mut end = STDERR_ECHO;
    while !text.is_char_boundary(end) { end -= 1; }
    format!("{}… [{} more bytes]", &text[..end], text.len() - end)
}

pub fn run_command(command: &str, input: &str) -> Result<String> {
    let cwd = EmptyCwd::new()?;
    let mut child = shell()?.arg("-c").arg(format!("{}{command}", pipefail_prefix())).current_dir(&cwd.0)
        .stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped())
        .spawn().with_context(|| format!("spawn `{command}`"))?;
    // A command that answers without reading its whole prompt closes the pipe early — `claude -p`
    // never does, a wrapper or a stub may — and the write then fails with EPIPE while the answer is
    // already on stdout. The exit status and the output judge the run, not the write.
    let mut stdin = child.stdin.take().context("stdin")?;
    match stdin.write_all(input.as_bytes()) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
        Err(e) => return Err(e).with_context(|| format!("write the prompt to `{command}`")),
    }
    drop(stdin);
    let out = child.wait_with_output()?;
    if !out.status.success() {
        anyhow::bail!("`{command}` exited {}: {}", out.status, clip_stderr(&out.stderr));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_that_answers_without_reading_its_prompt_still_answers() {
        // Larger than any pipe buffer, so the write blocks until the child closes its end of the
        // pipe — the race the CI runner lost on the reranker's stub command, made deterministic.
        let prompt = "x".repeat(1 << 20);
        let out = run_command("exec 0<&-; printf 'answered\\n'", &prompt).unwrap();
        assert_eq!(out, "answered\n");
    }

    #[test]
    fn batches_started_together_each_get_their_own_empty_directory() {
        let dirs: Vec<EmptyCwd> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..8).map(|_| s.spawn(|| (0..32).map(|_| EmptyCwd::new().unwrap()).collect::<Vec<_>>())).collect();
            handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
        });
        let distinct: std::collections::BTreeSet<&PathBuf> = dirs.iter().map(|d| &d.0).collect();
        assert_eq!(distinct.len(), 256);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&dirs[0].0).unwrap().permissions().mode() & 0o777, 0o700);
        }
    }

    #[test]
    fn a_command_runs_in_an_empty_directory_that_is_not_the_callers() {
        let out = run_command("ls -A | wc -l; pwd", "").unwrap();
        let mut lines = out.lines();
        assert_eq!(lines.next().unwrap().trim(), "0");
        let pwd = lines.next().unwrap();
        assert_ne!(std::path::Path::new(pwd), std::env::current_dir().unwrap());
        assert!(!std::path::Path::new(pwd).exists(), "the directory goes with the run");
    }

    #[test]
    fn a_failing_commands_stderr_is_clipped_in_the_error() {
        let err = run_command("head -c 100000 /dev/zero | tr '\\0' 'e' >&2; exit 1", "").unwrap_err().to_string();
        assert!(err.len() < 4096, "{} bytes", err.len());
        assert!(err.contains("more bytes"), "{err}");
    }

    /// The shell a default Git for Windows install leaves findable: `git` on PATH under `cmd\`,
    /// `sh` two directories over. A synthetic tree and a synthetic PATH, so the test is about the
    /// lookup and not about what this machine has installed.
    #[cfg(windows)]
    #[test]
    fn a_shell_that_is_not_on_path_is_found_beside_git_and_then_under_program_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Git");
        for f in ["cmd\\git.exe", "bin\\sh.exe", "usr\\bin\\awk.exe"] {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"").unwrap();
        }
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        assert_eq!(git_sh(&[elsewhere.clone(), root.join("cmd")], None, &[]), Some(root.join("bin").join("sh.exe")), "beside git");
        assert_eq!(git_sh(std::slice::from_ref(&elsewhere), None, &[dir.path().to_path_buf()]), Some(root.join("bin").join("sh.exe")), "under a program directory");
        assert_eq!(git_sh(std::slice::from_ref(&elsewhere), Some(&root.join("bin").join("bash.exe")), &[]), Some(root.join("bin").join("sh.exe")), "from the bash Claude Code names");
        assert_eq!(git_sh(&[elsewhere], None, &[]), None, "nothing to find");
    }
}
