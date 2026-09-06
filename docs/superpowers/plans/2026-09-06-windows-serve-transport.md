# Windows Serve Transport Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `repograph ask` on Windows (win32-x64) is answered by a resident `repograph serve` the way it is on unix — same socket file, same handshake, same speed — and a Windows user gets there honestly: the crate compiles on `x86_64-pc-windows-msvc`, the release carries a `repograph.exe`, the npm launcher resolves a `win32-x64` package, and the model cache lands somewhere on a machine that never sets `HOME`. Today the crate does not compile on Windows at all (`src/serve.rs:7` imports `std::os::unix::net` unconditionally), the npm launcher has no `win32-x64` entry and points at a `cargo install` that cannot succeed, and a cfg-gate that would drop `serve` on Windows was refused by Max: "must be fast".

**Architecture:** repograph is a Rust CLI (`src/`) over a `.repograph/` store. `serve` (`src/serve.rs`) binds `.repograph/serve.sock`, accepts on a thread of its own, hands each connection to the one thread holding the model and the indexes through a rendezvous channel, and answers one JSON line with one JSON line; `ask` (`src/main.rs:403`) tries the socket first unless `--no-serve` / `REPOGRAPH_NO_SERVE`. Staleness is decided by three stamps — the executable's (`build_stamp`), `repograph.toml`'s (`config_stamp`), and the socket file's own identity (`socket_id`, device + inode) which is what lets a server exiting remove only the socket it bound. This plan puts the transport behind a two-function-family seam (`Listener`/`Stream`, `bind`/`connect`/`accept`/`present`/`id`) with a unix half that is std's `UnixListener`/`UnixStream` verbatim and a Windows half that is the same `AF_UNIX` socket through `socket2`. Nothing above the seam changes.

**Tech Stack:** Rust 2021 pinned at 1.98.0 (`rust-toolchain.toml`), every dependency `=`-pinned; GitHub Actions (`ci.yml` on push and pull_request, `release.yml` on `v*` tags); bash 3.2 for `scripts/npm-pack.sh`; Node ≥ 18 for the launcher. There is no Windows toolchain on the Mac (`rustup target list --installed` → `aarch64-apple-darwin` only) and none is to be installed: Windows is judged by CI and only by CI.

**Spec:** this document's *Decision* section. No separate spec was written; the decision and its consequences are argued here because they are the plan.

## Global Constraints

- The worktree is `/Users/max/Documents/projects/repograph/.worktrees/windows-serve`, branch `feat/windows-serve`, cut from `origin/main` at `32577e2` (`feat(config): the model is a setting, in the project or on the machine (#13)`). Local `main` is at `d87c607` (#6) and is seven PRs behind; nothing is read from it. Never `cd` into the main checkout; use `git -C` or run from the worktree.
- **No step builds for Windows on this machine.** Not `cargo check --target x86_64-pc-windows-msvc`, not a mingw toolchain, not a VM. Every Windows-affecting task ends with a push and a watched CI run (`gh auth switch --user devmaxxx && gh run watch`), and the run's verdict is the step's verdict. `cargo tree --target x86_64-pc-windows-msvc` is allowed — it resolves the lock without compiling, and it ran during planning.
- The unix path keeps today's behaviour byte for byte: the code compiled on unix after Task 2 differs from today's by two type names, one function moved into a module, and an `accept` loop spelled out where `incoming()` was. The wire format (`Hello`/`Reply`, one line each) does not change on any platform. The twelve `tests/serve.rs` tests and a timing spot check on a self-index (Task 3 Step 6) are the judge; the bench floors are untouched because no retrieval code is.
- Cargo dependencies stay `=`-pinned. The one dependency added (`socket2 =0.6.5`, Windows only) is already in `Cargo.lock` at that version through `hf-hub → reqwest → hyper-util`; on unix the set of compiled crates does not change.
- Versions are not bumped: `Cargo.toml` and the npm packages already say `0.5.0` on `origin/main`.
- The fixture `/Users/max/bench/beauty-crm-502e8a6d` is not touched. The timing spot check indexes the worktree's own docs.
- Any `gh` call runs `gh auth switch --user devmaxxx && gh …` in the same shell command.
- The commit hook rejects AI attribution trailers and session links, and rejects any single shell command that contains both a heredoc and `git commit` unless the heredoc's first line is a Conventional Commits subject: edits and commits go in separate commands; `git commit -F - <<'MSG'` works when the first heredoc line is the subject. Subjects are Conventional Commits (`feat(serve): …`, `ci: …`, `build(npm): …`, `docs: …`).
- Comments say why, never what; no ticket ids in comments; tool directives stay.
- Every claim of "works on Windows" in a commit, a README sentence or the PR body cites a CI run URL. Nothing about Windows is asserted from memory.
- Implementers never dispatch subagents.

---

## Decision: the Windows transport

Three candidates were on the table. The requirement that decides between them is not "does it connect" — all three do — but what each one does to the four things `serve` already gets right on unix: the socket is private to the repository's owner, a dead socket costs a client nothing, a live server's identity can be told from its name, and the code above the transport is written once.

### (a) TCP on 127.0.0.1, ephemeral port in `.repograph/serve.port` — rejected

std-only, no new dependency. Everything else is worse.

*Access.* Any process of any user on the machine can connect to a loopback port; Windows has no `SO_PEERCRED`. A `serve` answers questions about the repository (its contents, effectively) and, on a `--rerank` question, runs the *server's* configured `rerank_command` — the user's Claude tokens, spent by whoever connects. So (a) needs an authentication token. It would live in a file beside the port file, and the file's protection is whatever NTFS ACL `.repograph/` inherited: under `C:\Users\<me>\…` that is the owner, SYSTEM and Administrators; on a data drive (`D:\src\…`) the default for a newly created tree is often `Authenticated Users: Modify`, i.e. readable by every local account. Fixing that means writing a DACL from Rust (`SetNamedSecurityInfoW` through `windows-sys`'s `Win32_Security_Authorization`), which is more Windows code than the whole transport, none of it testable here. The residual after all of that is the same as on unix — any process running *as the user* holds the token — but the road there is long and unverifiable.

*Staleness, the one that kills it.* On unix a dead server leaves a socket file nobody listens on, and a connect is refused at once. On TCP a dead server leaves a port number, and Windows hands ephemeral ports back out: the next process to listen there — a dev server, a database, a debugger — accepts the client's connect, receives a `Hello` line it does not understand, and says nothing. The client then waits out `IO_TIMEOUT`, thirty seconds, before answering in its own process. The token does not help (the stranger never replies); only a protocol change (a server banner before the client speaks) or a PID-liveness check would, and the first changes the unix wire format this plan is forbidden to touch while the second is a guess. A `serve` exists to make `ask` faster; a transport that can make it thirty seconds slower is disqualified.

*Also unverified:* whether Windows Defender Firewall prompts on a loopback-only listener (it should not; it sometimes does).

### (b) Named pipe `\\.\pipe\repograph-…` — rejected

Needs `windows-sys` features `Win32_System_Pipes`, `Win32_Security`, `Win32_Storage_FileSystem` (the crate is in the lock, the features are not compiled today). The pipe namespace is machine-global, so the name must encode the repository path *and* the user, or two users with the same checkout path collide. The default DACL of a pipe created with a null security descriptor grants Everyone read — a client needs write to send a `Hello`, so other users cannot ask, but a name is a name: a process that creates `\\.\pipe\repograph-<hash>` *first* (any user, any time) is the server every client of that repository will talk to, and its `Reply` is taken as the answer. Guarding against that means `FILE_FLAG_FIRST_PIPE_INSTANCE` on the server, an explicit owner-only DACL, and `GetNamedPipeServerProcessId` plus an owner check on the client — the classic named-pipe squatting defences — again more security code than transport, and again untestable from here. Structurally it is also a different loop: a pipe instance is created per connection (`CreateNamedPipeW` + `ConnectNamedPipe`), there is no `incoming()`, and `serve`'s accept thread would need rewriting rather than re-typing. The one genuine advantage — no stale file, a pipe name disappears with its last handle — does not pay for the rest.

### (c) `AF_UNIX` on Windows through `socket2` — **chosen**

Windows 10 1803 (build 17063) added `AF_UNIX` stream sockets to winsock; Rust 1.98 itself requires Windows 10 1809 or later, so every machine that can run this binary has them. std does not expose them (`std::os::unix::net` is unix-only), but `socket2` does, and `socket2 0.6.5` is already in `Cargo.lock`. Verified in this session from the crate's source: `Domain::UNIX` and `SockAddr::unix` are compiled on all non-WASI targets (`src/lib.rs:229`, `src/sockaddr.rs:219`); the Windows address builder writes `sun_family = AF_UNIX`, requires the path to be UTF-8, and refuses one of 108 bytes or more (`src/sys/windows.rs:1035–1060`; `SOCKADDR_UN.sun_path` is `[i8; 108]` in `windows-sys 0.61.2`); `Socket::new` runs `WSAStartup` once through a `Once` (`sys/windows.rs:280`); `bind`, `connect`, `listen`, `accept`, `try_clone`, `set_read_timeout`/`set_write_timeout`, and `Read`/`Write` for both `Socket` and `&Socket` are all ungated (`socket.rs:168, 192, 240, 259, 356, 1104, 2400–2449`).

What it keeps, and why that is the whole argument:

- **The path.** `socket_path` stays `.repograph/serve.sock` on every platform. The socket file is an NTFS reparse point created by `bind`; Microsoft's own write-up states that connecting needs write permission on that file and binding needs write permission on its directory — the same security model as unix, enforced by the same thing (the repository directory's ACL), with no token and no DACL code.
- **The refusal.** A dead server leaves a file nobody listens on; a connect to it fails, and the client falls back at once. Nothing can reuse the name except another `serve` in that directory. The client still never deletes the file, for the reason `try_ask`'s comment gives.
- **The code.** `run`, `answer`, `hello_line`, `try_ask`, the accept thread and the rendezvous channel are written once against `Listener` and `Stream`. The Windows half of the seam is forty lines.
- **Speed.** A loopback `AF_UNIX` connect on Windows costs what a TCP loopback connect costs — well under a millisecond — and the rest of an answer is the same code. The unix half is std's types, so unix pays nothing at all.

What it changes, and how each change is handled:

- **`socket_id` has no inode to read.** Its replacement identity on Windows is NTFS's file reference number (`MetadataExt::file_index`, 64 bits *including* the MFT sequence number, so a record reused for a new file is a new identity) together with `volume_serial_number`. Both come back `Some` from `symlink_metadata`: verified in std 1.98's source, `lstat` opens a handle and calls `GetFileInformationByHandle` (`sys/fs/windows.rs:526, 1484–1500`); only `DirEntry::metadata` and the sharing-violation fallback yield `None`. Creation time was considered and rejected: NTFS *tunnels* it — a file recreated under a deleted name within fifteen seconds inherits the old creation time — and "deleted and recreated under the same name a moment later" is exactly what a replacement `serve` does.
- **`Path::exists` on a reparse point.** `exists` follows reparse points and an `AF_UNIX` socket file leads nowhere. std 1.98's `stat` and `try_exists` both treat `ERROR_CANT_ACCESS_FILE` as "the reparse point itself is there" (`sys/fs/windows.rs:1470–1482, 1733–1738`), so `exists` very likely works; the seam does not bet on it and asks `symlink_metadata` on Windows, which opens the reparse point itself and cannot be wrong about it.
- **The 108-byte limit.** `sun_path` is 108 bytes on Windows as on Linux (104 on macOS); a repository at a very long path cannot bind, on any platform, today. Windows paths run longer, so this will be met sooner there; the failure is `serve` refusing to start with the path in the message, and `ask` answering in its own process because no file exists. Documented, not fixed: the relative-path trick (bind after `set_current_dir`) would change unix behaviour.
- **Nothing runs `Drop` on Ctrl-C** on Windows either — the socket file stays, as it does after `SIGINT` on unix, and the next `serve` removes it before binding as it already does.

Unverified until the Windows CI job runs — each has a test or a step that decides it, listed under *Verification ledger*: that a connect to a stale socket file is refused promptly (`WSAECONNREFUSED`) rather than accepted by nothing (Task 3's timed assertion); that `remove_file` succeeds on the reparse point while the process's own listener is still bound (the consequence if not is a stale file removed by the next `serve`, never a wrong answer); that `DeleteFileW` from `std::fs::remove_file` is what a bound socket's file needs (Microsoft's write-up says "DeleteFile … should be used to delete the socket file prior to calling bind").

The seam is a private `mod sys` inside `src/serve.rs` under `#[cfg(unix)]` / `#[cfg(windows)]`, not a new file: the two halves together are shorter than the comments that would justify a module of their own, and the reader of `serve.rs` sees both platforms in one screen.

## Verification ledger

What was verified during planning, from sources on this machine, and what only the Windows CI run can settle.

| Claim | Verified how | Or judged by |
|---|---|---|
| `socket2 0.6.5` builds `AF_UNIX` addresses on Windows; every method the seam calls is ungated | read `~/.cargo/registry/src/*/socket2-0.6.5/src/{lib,sockaddr,socket,sys/windows}.rs` | — |
| `socket2 0.6.5` is already in `Cargo.lock`; `cargo tree --target x86_64-pc-windows-msvc -i socket2` resolves it from the lock on the Mac | ran it | Task 2 Step 4 re-runs it after the `Cargo.toml` edit |
| `symlink_metadata` gives `Some` for `file_index`/`volume_serial_number`; `stat`/`try_exists` accept `ERROR_CANT_ACCESS_FILE` | read `~/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/src/rust/library/std/src/sys/fs/windows.rs` (1.98.0) | Task 2's unit test on Windows CI |
| `std::env::home_dir()` is un-deprecated on 1.98.0 | compiled a one-liner with `rustc -D warnings` (1.98.0); printed `Some("/Users/max")` | — |
| `ort-sys 2.0.0-rc.13` has `x86_64-pc-windows-msvc` prebuilt rows; a no-EP build resolves to the `directml` row and links `onnxruntime` statically plus `dxguid DXCORE DXGI D3D12 DirectML` | read `build/download/{dist.tsv,resolve.rs}` and `build/static_link/mod.rs` | — |
| `repograph.exe` loads on a machine with only the inbox `DirectML.dll` (no DLL shipped beside it) | *not verifiable here*; ort's docs say the DirectML build "require[s] helper dylibs" and repograph does not enable `copy-dylibs` | Task 1's `cargo test` (spawns the exe a dozen times) and Task 5's release smoke step `repograph.exe --version`; contingency in Task 5 Step 5 |
| `onig_sys 69.9.3` builds on MSVC without bindgen (`config.h.win64`, `generate` feature off) | read its `build.rs:89` | Task 1 |
| `tree-sitter 0.27`, `tokenizers 0.22` (`onig`), `blake3` (MASM), `hf-hub 0.5` (`schannel`, symlink-or-rename cache) compile on `x86_64-pc-windows-msvc` | *not verified here*; all are routinely built there upstream | Task 1 |
| Windows `AF_UNIX`: 10 1803+, reparse-point file, not unlinked on close, ACL-secured like unix | Microsoft's announcement (devblogs, fetched) | Task 3's tests on Windows CI |
| A connect to a stale socket file is refused at once on Windows | *not stated by Microsoft's write-up* | Task 3 Step 3's `< 5 s` assertion |
| `sh` (Git for Windows) is on `PATH` on `windows-latest`, so the `awk`/`printf` generators in `src/enrich.rs` and `src/rerank.rs` unit tests run | *not verified* | Task 1; contingency: `$GITHUB_PATH` step |
| The runner's git does not convert the checkout to CRLF | *not verified* | Task 1 sets `core.autocrlf false` before checkout regardless |

---

## File Structure

| File | Responsibility | Tasks |
|---|---|---|
| `.github/workflows/ci.yml` | the Windows job: clippy + test on `windows-latest` | 1 |
| `Cargo.toml`, `Cargo.lock` | `socket2` under `[target.'cfg(windows)'.dependencies]` | 2 |
| `src/serve.rs` | the transport seam `mod sys`; call sites re-typed; a unit test on the socket file's identity | 2 |
| `tests/serve.rs` | the test-side seam; `symlink_metadata` for presence; the timed fallback; the replaced-binary test unix-only | 3 |
| `src/index/mod.rs`, `src/index/embed.rs`, `src/index/cross.rs`, `src/config.rs` | `cache_root()`; home through `std::env::home_dir` at the three `HOME` sites | 4 |
| `.github/workflows/release.yml` | Windows in the matrix, `.zip`, the load smoke, `workflow_dispatch`, the npm job's third platform | 5 |
| `scripts/npm-pack.sh` | `win32-x64=<repograph.exe>` | 5 |
| `npm/repograph-win32-x64/package.json` (new), `npm/repograph/package.json`, `npm/repograph/bin/repograph.js`, `.gitignore` | the platform package, the optional dependency, the `.exe` resolution | 5 |
| `README.md`, `npm/repograph/README.md`, `src/config.rs` doc comment | the Windows story in words | 6 |

---

### Task 0: Worktree and the unix baseline

**Files:**
- Create (outside the repo): `$M/`, `$M/task0.txt`
- Nothing in the repo changes; no commit.

**Interfaces:**
- Produces: the base binary `$M/repograph-base` for Task 3's timing spot check; the unix test counts every later task is held to.

- [ ] **Step 1: The worktree, off `origin/main`**

```bash
R=/Users/max/Documents/projects/repograph
git -C "$R" fetch origin
git -C "$R" worktree add "$R/.worktrees/windows-serve" -b feat/windows-serve origin/main
W="$R/.worktrees/windows-serve"; git -C "$W" log --oneline -1
```

Expected: `32577e2 feat(config): the model is a setting, in the project or on the machine (#13)`. Anything else: `origin/main` moved since this plan was written — stop and report the new tip; `src/serve.rs`, `tests/serve.rs`, `src/config.rs` and the npm files were read at `32577e2` and a newer tip has to be diffed against that reading first.

- [ ] **Step 2: The unix baseline**

```bash
M=/Users/max/bench/windows-serve-2026-09-06; mkdir -p "$M"
cd "$W" && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1
cargo test --release 2>&1 | grep 'test result'
cp target/release/repograph "$M/repograph-base"
cargo tree --target x86_64-pc-windows-msvc --edges normal -p repograph --depth 1 | tee "$M/win-deps-before.txt"
```

Expected: clippy clean; `444 passed; 0 failed; 2 ignored` for the unit tests and `12 passed` for `tests/serve.rs` (the counts the 2026-09-06 residue plan recorded at the same tip — a different count is a tip that moved, see Step 1); the Windows dependency list has no `socket2` line at depth 1. `cargo tree` with a target the host cannot build for resolves without compiling — that is the only Windows-facing command this plan runs locally.

- [ ] **Step 3: The task record**

`$M/task0.txt`: the worktree's commit, the three counts, the sha of `$M/repograph-base`. Report DONE with that path.

---

### Task 1: The Windows CI job, red first

**Files:**
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Produces: the judge every later Windows step is held to. It must fail now, at `src/serve.rs:7`, so that a later green is evidence and not a job that never compiled the crate.

Background the brief cannot know: `ci.yml` has two jobs, `test` (ubuntu: clippy `-D warnings`, then `cargo test`) and `bench` (ubuntu, gated on a secret). The Windows job mirrors `test`, nothing more; the bench stays unix. The build on `windows-latest` downloads ONNX Runtime through `ort-sys` (`download-binaries`) and compiles oniguruma and tree-sitter with MSVC's `cl`, which the runner has; expect ten to fifteen minutes.

- [ ] **Step 1: Add the job**

Append to `.github/workflows/ci.yml`, after the `bench` job:

```yaml
  windows:
    runs-on: windows-latest
    steps:
      # The repository is LF throughout and the tests compare bytes; a checkout the runner's
      # git had converted to CRLF would be a different corpus.
      - run: git config --global core.autocrlf false
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.98.0
        with: { components: clippy }
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo test
```

- [ ] **Step 2: Commit and push; watch it fail where expected**

```bash
git -C "$W" add .github/workflows/ci.yml
git -C "$W" commit -F - <<'MSG'
ci: clippy and test on windows-latest

Red on purpose: src/serve.rs imports std::os::unix::net unconditionally, so the job fails at the
first compile. The seam that turns it green is the next commit; a job added green would prove
nothing about it.
MSG
git -C "$W" push -u origin feat/windows-serve
gh auth switch --user devmaxxx && gh run watch --repo devmaxxx/repograph --exit-status $(gh run list --repo devmaxxx/repograph --branch feat/windows-serve --workflow ci --limit 1 --json databaseId --jq '.[0].databaseId'); true
gh auth switch --user devmaxxx && gh run view --repo devmaxxx/repograph --log-failed $(gh run list --repo devmaxxx/repograph --branch feat/windows-serve --workflow ci --limit 1 --json databaseId --jq '.[0].databaseId') | grep -n "error\[E\|error:" | head -20 | tee "$M/task1-red.txt"
```

Expected: `test` green, `windows` red, and the first error names `src/serve.rs` and `std::os::unix` (`E0433` unresolved path or the like). **Any error before that one** — a dependency that does not build on MSVC, the toolchain action failing — is a finding this plan could not make from the Mac: record it in `$M/task1-red.txt`, and if it is in a dependency, stop and report; the transport is moot until the crate's dependencies build there.

---

### Task 2: The transport seam in `src/serve.rs`

**Files:**
- Modify: `Cargo.toml` (and `Cargo.lock` through it), `src/serve.rs`

**Interfaces:**
- Consumes: `socket2::{Socket, Domain, Type, SockAddr}` (Windows only); `std::os::unix::net` (unix only); `std::os::{unix,windows}::fs::MetadataExt`.
- Produces: `sys::{Listener, Stream, bind, connect, accept, present, id}` — private to `serve.rs`; the rest of the file is re-typed against them and otherwise untouched.

- [ ] **Step 1: The dependency**

Append to `Cargo.toml` after `[dev-dependencies]`:

```toml
# std has no AF_UNIX on Windows; socket2 has, and is in the lock already through hf-hub's client.
[target.'cfg(windows)'.dependencies]
socket2 = "=0.6.5"
```

- [ ] **Step 2: Write the failing unit test**

`src/serve.rs` has no `mod tests` today. Add at the end of the file:

```rust
#[cfg(test)]
mod tests {
    use super::sys;

    /// The two facts `try_ask` and `Unlink` read off the socket file, through the platform's own
    /// metadata: on Windows the file is a reparse point, and `exists` would ask what it points at.
    #[test]
    fn a_bound_socket_file_is_present_and_carries_an_identity_until_it_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("serve.sock");
        assert!(!sys::present(&path) && sys::id(&path).is_none());
        let first = sys::bind(&path).unwrap();
        assert!(sys::present(&path));
        let id = sys::id(&path).expect("a bound socket file has an identity");
        drop(first);
        std::fs::remove_file(&path).unwrap();
        assert!(!sys::present(&path), "the name goes with the file");
        let _second = sys::bind(&path).unwrap();
        // NTFS bumps a reused record's sequence number, so the new file is a new identity by
        // construction. A unix filesystem may hand the inode straight back, so there this is not
        // asserted — and the guard's dev+ino check has always lived with that.
        if cfg!(windows) { assert_ne!(sys::id(&path), Some(id), "bound again under the name, another file"); }
    }
}
```

Run: `cd "$W" && cargo test --release a_bound_socket_file 2>&1 | tail -5`
Expected: compile error — `sys` does not exist.

- [ ] **Step 3: The seam, and the call sites**

In `src/serve.rs`, delete the line `use std::os::unix::net::{UnixListener, UnixStream};` and the whole of `fn socket_id` with its doc comment (the comment moves into `sys::id`). Insert after the `use` block:

```rust
/// The socket, one implementation per platform under one name. Both are AF_UNIX at
/// `socket_path`: Windows has spoken it since 10 1803, only through winsock, which std does not
/// wrap, so that side goes through socket2 while unix keeps std's own types. Everything below
/// this module is written once, against `Listener` and `Stream`.
#[cfg(unix)]
mod sys {
    use std::io;
    use std::path::Path;
    pub use std::os::unix::net::{UnixListener as Listener, UnixStream as Stream};
    pub fn bind(path: &Path) -> io::Result<Listener> { Listener::bind(path) }
    pub fn connect(path: &Path) -> io::Result<Stream> { Stream::connect(path) }
    pub fn accept(listener: &Listener) -> io::Result<Stream> { listener.accept().map(|(s, _)| s) }
    /// Whether the name is there at all — a file, not a listener.
    pub fn present(path: &Path) -> bool { path.exists() }
    /// The device and inode of a socket file, which is how a name is told from the thing that was
    /// bound to it: the path can hold a replacement server's socket by the time this one exits.
    pub fn id(path: &Path) -> Option<(u64, u64)> {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::metadata(path).ok()?;
        Some((m.dev(), m.ino()))
    }
}

#[cfg(windows)]
mod sys {
    use socket2::{Domain, SockAddr, Socket, Type};
    use std::io;
    use std::path::Path;
    pub type Listener = Socket;
    pub type Stream = Socket;
    pub fn bind(path: &Path) -> io::Result<Listener> {
        let s = Socket::new(Domain::UNIX, Type::STREAM, None)?;
        s.bind(&SockAddr::unix(path)?)?;
        // The backlog std's `UnixListener::bind` asks for on unix.
        s.listen(128)?;
        Ok(s)
    }
    pub fn connect(path: &Path) -> io::Result<Stream> {
        let s = Socket::new(Domain::UNIX, Type::STREAM, None)?;
        s.connect(&SockAddr::unix(path)?)?;
        Ok(s)
    }
    pub fn accept(listener: &Listener) -> io::Result<Stream> { listener.accept().map(|(s, _)| s) }
    /// The socket file is a reparse point with nothing behind it, so the metadata that says it
    /// is there has to be its own, never a target's.
    pub fn present(path: &Path) -> bool { std::fs::symlink_metadata(path).is_ok() }
    /// NTFS's file reference number, sequence included — a record reused for a new file carries
    /// a new sequence, so a name unlinked and bound again is another identity, the way a fresh
    /// inode is on unix. Creation time would not do: NTFS tunnels it, and a file recreated under
    /// a name within fifteen seconds inherits the old one, which is a replacement server's timing.
    pub fn id(path: &Path) -> Option<(u64, u64)> {
        use std::os::windows::fs::MetadataExt;
        let m = std::fs::symlink_metadata(path).ok()?;
        Some((u64::from(m.volume_serial_number()?), m.file_index()?))
    }
}
```

Then, each call site, in file order:

- `try_ask`: `if !path.exists() { return None; }` → `if !sys::present(&path) { return None; }`; `UnixStream::connect(&path).ok()?` → `sys::connect(&path).ok()?`.
- `run`: `if path.exists() && UnixStream::connect(&path).is_ok()` → `if sys::present(&path) && sys::connect(&path).is_ok()`; `UnixListener::bind(&path)` → `sys::bind(&path)`; `Unlink(path.clone(), socket_id(&path))` → `Unlink(path.clone(), sys::id(&path))`; `sync_channel::<UnixStream>(0)` → `sync_channel::<sys::Stream>(0)`; the accept thread becomes

```rust
    std::thread::spawn(move || loop {
        match sys::accept(&listener) {
            Ok(s) => if tx.send(s).is_err() { return; },
            Err(_) => return,
        }
    });
```

  which is what `incoming()` expands to on unix — socket2 has no `incoming`, and one loop spelled out beats two.
- `fn hello_line(stream: &UnixStream)` → `&sys::Stream`; `fn answer(mut stream: UnixStream, …)` → `mut stream: sys::Stream`.
- `impl Drop for Unlink`: `socket_id(&self.0) == self.1` → `sys::id(&self.0) == self.1`.

The comment above `run`'s `_guard` and the comment on `hello_line` (the macOS `EINVAL` note) stay as they are; both are still true.

- [ ] **Step 4: Unix green, the lock, the diff read**

```bash
cd "$W" && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1
cargo test --release 2>&1 | grep 'test result'
cargo tree --target x86_64-pc-windows-msvc --edges normal -p repograph --depth 1 | grep socket2
git diff --stat Cargo.lock; git diff Cargo.lock | grep '^[+-] '
git diff src/serve.rs | grep '^[+-]' | grep -v '^[+-]\s*//' | wc -l
```

Expected: clippy clean; `445 passed` (the new unit test) and `12 passed`; `socket2 v0.6.5` listed under `repograph` for the Windows target; the lock diff is exactly one added line, `+ "socket2",` in the `repograph` package's dependency list; the non-comment diff of `serve.rs` is the seam plus the re-typed lines named in Step 3 and nothing else — read it and confirm no line of `run`'s loop, `answer`, or the handshake moved.

- [ ] **Step 5: Commit**

```bash
git -C "$W" add Cargo.toml Cargo.lock src/serve.rs
git -C "$W" commit -F - <<'MSG'
feat(serve): the socket behind one seam, AF_UNIX through socket2 on Windows

The same socket file at the same path on every platform; std's types where std has them, socket2's
Socket on Windows, which has spoken AF_UNIX since 10 1803. The socket file's identity — what lets
a server remove only the socket it bound — is dev+ino on unix and NTFS's file reference number
on Windows; creation time was rejected because NTFS tunnels it across a delete-and-recreate.
Unix compiles the same code it did: two type names and an accept loop spelled out.
MSG
```

No push yet: `tests/serve.rs` still imports `std::os::unix::net` and `cargo clippy --all-targets` on Windows compiles it. Task 3 pushes both.

---

### Task 3: The test-side seam, and the unix speed check

**Files:**
- Modify: `tests/serve.rs`

**Interfaces:**
- Consumes: the same `socket2` API (an integration test links the crate's regular dependencies, so no dev-dependency is needed).
- Produces: twelve tests that compile and pass on all three platforms, one of them now timed; one test and one helper unix-only for a reason stated in the file.

Background the brief cannot know: four places in `tests/serve.rs` speak to the socket directly — `reply_to` (connect), `a_socket_nobody_listens_on_is_answered_here_and_left_for_the_next_serve` (bind and drop), `a_reply_from_another_version_is_ignored` and `a_reply_from_another_build_of_this_version_is_ignored` (bind, accept, fake reply). Two more read the file's presence with `exists`. And `a_binary_replaced_under_a_live_server_is_another_build_and_the_client_answers_here` renames a copy of the binary over one that a spawned server is running from: Windows will not replace a running image (`MoveFileEx` with `MOVEFILE_REPLACE_EXISTING` on a mapped executable fails), and `cargo build` there fails the same way while a `serve` is up, so the case that test guards cannot arise on Windows.

- [ ] **Step 1: The seam in the test file**

Add after the `use` lines at the top of `tests/serve.rs`:

```rust
/// The crate is a binary with no library, so the test carries its own copy of the transport seam
/// `src/serve.rs` keeps: std's Unix socket where std has one, socket2's AF_UNIX on Windows.
#[cfg(unix)]
mod transport {
    use std::path::Path;
    pub use std::os::unix::net::{UnixListener as Listener, UnixStream as Stream};
    pub fn bind(path: &Path) -> Listener { Listener::bind(path).unwrap() }
    pub fn connect(path: &Path) -> std::io::Result<Stream> { Stream::connect(path) }
    pub fn accept(listener: &Listener) -> Stream { listener.accept().unwrap().0 }
}

#[cfg(windows)]
mod transport {
    use socket2::{Domain, SockAddr, Socket, Type};
    use std::path::Path;
    pub type Listener = Socket;
    pub type Stream = Socket;
    pub fn bind(path: &Path) -> Listener {
        let s = Socket::new(Domain::UNIX, Type::STREAM, None).unwrap();
        s.bind(&SockAddr::unix(path).unwrap()).unwrap();
        s.listen(1).unwrap();
        s
    }
    pub fn connect(path: &Path) -> std::io::Result<Stream> {
        let s = Socket::new(Domain::UNIX, Type::STREAM, None)?;
        s.connect(&SockAddr::unix(path)?)?;
        Ok(s)
    }
    pub fn accept(listener: &Listener) -> Stream { listener.accept().unwrap().0 }
}
```

Then:
- `reply_to`: `std::os::unix::net::UnixStream::connect(&sock)` → `transport::connect(&sock)`.
- the three `std::os::unix::net::UnixListener::bind(&sock).unwrap()` → `transport::bind(&sock)`; the two `listener.accept().unwrap()` → `transport::accept(&listener)` (the variable binding stays `let mut s`).
- `wait_for_socket`: `while !sock.exists()` → `while std::fs::symlink_metadata(&sock).is_err()`, with the comment `// A socket file is a reparse point on Windows, and \`exists\` would follow it to nowhere; its own metadata is what says it is there, on every platform.` Same substitution for `assert!(sock.exists(), "the client leaves the socket file alone")`.
- `fn ask_from` and `fn a_binary_replaced_under_a_live_server_is_another_build_and_the_client_answers_here`: add `#[cfg(unix)]` above each, and above the test's doc comment add the reason: `// Unix only: Windows will not replace a running image, and a rebuild there fails until the server stops, so the pairing this test refuses cannot be made.` (`stamp_of` stays unconditional; the version test uses it too.)

- [ ] **Step 2: The timed fallback**

In `a_socket_nobody_listens_on_is_answered_here_and_left_for_the_next_serve`, wrap the first `ask`:

```rust
    let started = Instant::now();
    let (out, err) = ask(dir.path(), &[], &["штраф"]);
    // A connect nobody accepts is refused at once on every platform this runs on. One that were
    // accepted by nothing would sit out the client's whole read timeout, and a resident answer
    // exists to be faster than a process, not thirty seconds slower than one.
    assert!(started.elapsed() < Duration::from_secs(5), "the fallback took {:?}", started.elapsed());
```

- [ ] **Step 3: Unix green, and the diff read**

```bash
cd "$W" && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1
cargo test --release --test serve 2>&1 | grep 'test result'
grep -n "std::os::unix" tests/serve.rs
```

Expected: clippy clean; `12 passed`; the only `std::os::unix` left is inside `#[cfg(unix)] mod transport`.

- [ ] **Step 4: Push; the Windows job is the judge**

```bash
git -C "$W" add tests/serve.rs
git -C "$W" commit -F - <<'MSG'
test(serve): the socket tests speak the platform's transport

A test-side copy of the seam, the socket file's presence read from its own metadata, the dead-socket
fallback timed, and the replaced-binary test unix-only: Windows will not replace a running image.
MSG
git -C "$W" push
gh auth switch --user devmaxxx && gh run watch --repo devmaxxx/repograph --exit-status $(gh run list --repo devmaxxx/repograph --branch feat/windows-serve --workflow ci --limit 1 --json databaseId --jq '.[0].databaseId') | tail -5 | tee "$M/task3-ci.txt"
```

Expected: `test` and `windows` both green; the Windows log shows `12 passed` for `tests/serve.rs` minus one (`11 passed`, the replaced-binary test compiled out) and the unit count one higher than Task 0's. Record the run URL in `$M/task3-ci.txt`.

Triage, if the Windows job is red — in this order, each a separate commit, each pushed and watched:
1. A compile error in `src/serve.rs` or `tests/serve.rs`: a type or a trait the seam got wrong. Fix from socket2's source (`~/.cargo/registry/src/*/socket2-0.6.5/src/socket.rs`), not from memory.
2. `a_socket_nobody_listens_on…` failing its 5 s assertion: Windows accepted a connect nobody listens on and the client waited out `IO_TIMEOUT`. This is the one finding that would reopen the *Decision*: record the timing from the log, stop and report — the mitigations (a server banner, a liveness check) change the wire format and are not this plan's to choose.
3. `a_bound_socket_file…` failing on `id`: `file_index` came back `None` from `symlink_metadata`. Fall back to `std::fs::File::open` with `FILE_FLAG_OPEN_REPARSE_POINT` via `OpenOptionsExt::custom_flags` and `File::metadata`, which is the handle path std's docs promise `Some` for; same commit, same test.
4. Unit tests in `src/enrich.rs` / `src/rerank.rs` failing to spawn `sh`: add to the Windows job, before `cargo test`, `- run: echo "C:\Program Files\Git\usr\bin" | Out-File -FilePath $env:GITHUB_PATH -Append` with the comment `# enrich and rerank run their command under sh -c; on Windows that is Git for Windows' sh.`
5. Anything else: it is outside the transport. Record it in `$M/task3-ci.txt` with the failing test's name and stop and report; a `#[cfg(not(windows))]` on a test is a decision for the controller, with the reason written in the file.

- [ ] **Step 5: The unix speed spot check**

Not a bench: a check that the unix binary after Task 2 answers over the socket in the time the one before did. On the worktree's own docs, `--no-dense`, median of eleven, before and after interleaved:

```bash
cd "$W" && cargo build --release 2>&1 | tail -1
B1="$W/target/release/repograph"; B0="$M/repograph-base"
"$B1" --repo "$W" --no-dense build >/dev/null 2>&1
for b in "$B0" "$B1"; do
  "$b" --repo "$W" --no-dense serve --every 3600 --idle 120 >/dev/null 2>"$M/serve-$(basename $b).log" & pid=$!
  until [ -S "$W/.repograph/serve.sock" ]; do sleep 0.05; done
  "$b" --repo "$W" --no-dense ask paraphrase >/dev/null 2>&1   # the first answer pays the open
  for i in $(seq 11); do /usr/bin/time -p "$b" --repo "$W" --no-dense ask paraphrase levers 2>&1 >/dev/null | awk '/^real/{print $2}'; done | sort -n | sed -n 6p | tee -a "$M/task3-timing.txt"
  kill $pid; wait $pid 2>/dev/null
done
cat "$M/task3-timing.txt"
```

Expected: two medians within the noise of each other (tens of milliseconds on this repository; the README's figure for the lexical arm on the bench corpus is 6.8 ms plus a process start). A second median clearly above the first is a regression the diff read in Task 2 Step 4 should have made impossible — stop and report with both numbers.

---

### Task 4: A home directory Windows has

**Files:**
- Modify: `src/index/mod.rs`, `src/index/embed.rs`, `src/index/cross.rs`, `src/config.rs`

**Interfaces:**
- Produces: `crate::index::cache_root() -> Result<PathBuf>` = the home directory's `.cache/repograph`; `embed::cache_dir` and `cross::default_dir` join `fastembed` / `reranker` onto it; `config::machine_path` reads home the same way.

Background the brief cannot know: three sites read `HOME` from the environment — `src/index/embed.rs:76` (the model cache, an error without it), `src/index/cross.rs:24` (the reranker), `src/config.rs:111` (the machine config, silently `None` without it). A Windows console sets `USERPROFILE`, never `HOME`, so on Windows the first fused `ask` would fail with "neither FASTEMBED_CACHE_DIR nor HOME is set" and the machine config would never be read. `std::env::home_dir()` — un-deprecated, verified to compile under `-D warnings` on 1.98.0 — reads `$HOME` on unix (the same lookup as today, falling back to the passwd entry only when it is unset, where today errors) and `USERPROFILE` then the profile API on Windows. The config tests set `REPOGRAPH_CONFIG`, never `HOME`, so they are unaffected; `embed.rs` tests set the cache through `set_cache_dir`, likewise.

- [ ] **Step 1: Write the failing test**

Append to `src/index/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    /// The cache is under whatever the platform calls home, which on a Windows console is not
    /// `$HOME`; on unix the two lookups agree, so this asserts the shape and, on Windows CI where
    /// `HOME` may be unset, that there is an answer at all.
    #[test]
    fn the_cache_root_is_under_the_home_the_platform_names() {
        let home = std::env::home_dir().expect("a home directory");
        assert_eq!(super::cache_root().unwrap(), home.join(".cache").join("repograph"));
    }
}
```

Run: `cd "$W" && cargo test --release the_cache_root 2>&1 | tail -3` — expected: compile error, no `cache_root`.

- [ ] **Step 2: The helper and the three sites**

`src/index/mod.rs`, after the `pub mod` lines:

```rust
use anyhow::Context as _;

/// `~/.cache/repograph`, the root of every cache this process keeps: the embedder's files and the
/// exported reranker. Read through the platform's idea of home rather than `$HOME`, which a
/// Windows console never sets; there it is the profile directory.
pub fn cache_root() -> anyhow::Result<std::path::PathBuf> {
    let home = std::env::home_dir().context("no home directory to keep the model cache under")?;
    Ok(home.join(".cache").join("repograph"))
}
```

`src/index/embed.rs` `cache_dir`: the last two lines become `Ok(crate::index::cache_root()?.join("fastembed"))`, the `HOME` line and its message deleted.
`src/index/cross.rs` `default_dir`: body becomes `Ok(crate::index::cache_root()?.join("reranker"))`; its doc comment's `$HOME/.cache/repograph/reranker` becomes `~/.cache/repograph/reranker`.
`src/config.rs` `machine_path`: `None => Path::new(&named("HOME")?).join(".config"),` → `None => std::env::home_dir()?.join(".config"),`; the doc comment's `$HOME/.config/repograph/config.toml` becomes `~/.config/repograph/config.toml`, and the sentence "`None` when the environment names no home at all" stays true. The `reranker_dir` field's doc comment (`$HOME/.cache/repograph/reranker`) becomes `~/.cache/repograph/reranker`.

- [ ] **Step 3: Green on unix, pushed, green on Windows**

```bash
cd "$W" && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && cargo test --release 2>&1 | grep 'test result'
grep -rn '"HOME"' src/ ; echo "(expected: nothing)"
git -C "$W" add src/index/mod.rs src/index/embed.rs src/index/cross.rs src/config.rs
git -C "$W" commit -F - <<'MSG'
fix(cache): the caches and the machine config live under the platform's home

Three sites read $HOME, which a Windows console never sets: the model cache errored, the reranker
errored, and the machine config was silently absent. One helper reads home through std, which is
$HOME on unix as before and the profile directory on Windows.
MSG
git -C "$W" push
gh auth switch --user devmaxxx && gh run watch --repo devmaxxx/repograph --exit-status $(gh run list --repo devmaxxx/repograph --branch feat/windows-serve --workflow ci --limit 1 --json databaseId --jq '.[0].databaseId') | tail -3 | tee "$M/task4-ci.txt"
```

Expected: `446 passed` on unix; both jobs green; no literal `"HOME"` left in `src/`.

---

### Task 5: A Windows binary in the release, and a launcher that finds it

**Files:**
- Modify: `.github/workflows/release.yml`, `scripts/npm-pack.sh`, `npm/repograph/package.json`, `npm/repograph/bin/repograph.js`, `.gitignore`
- Create: `npm/repograph-win32-x64/package.json`

**Interfaces:**
- Produces: release asset `repograph-x86_64-pc-windows-msvc.zip` holding `repograph.exe` at its root; npm package `@devmaxxx/repograph-win32-x64` with `bin/repograph.exe`; the launcher resolving `bin/repograph.exe` on `win32`.

Background the brief cannot know: `release.yml` builds a two-target matrix, tars `repograph` from `target/<target>/release`, uploads with `softprops/action-gh-release@v2`, then an `npm` job on ubuntu downloads `repograph-*.tar.gz`, untars into `bin/<platform>/`, calls `scripts/npm-pack.sh` and publishes platform packages first, launcher last. `npm-pack.sh` requires `-x` on the binary, copies it to `npm/repograph-<platform>/bin/repograph` and `chmod 755`s it — a downloaded `.exe` on Linux has no execute bit and must be named `repograph.exe`. The launcher's `require.resolve(\`${pkg}/bin/repograph\`)` tries no extension of its own for a file that is not `.js`/`.json`/`.node`. On Windows, `run:` steps execute in PowerShell and a native command's non-zero exit fails the step. The release workflow runs only on tags and a tag is Max's to push, so this task adds `workflow_dispatch` with the publish steps gated on a tag ref: the build, the smoke and the zip can be exercised on the branch.

- [ ] **Step 1: `release.yml`**

The trigger and the matrix:

```yaml
on:
  push:
    tags: ['v*']
  # A run from the branch builds, smokes and packs every target and publishes nothing: the
  # Windows build can be proven on a runner before a tag is spent.
  workflow_dispatch:
jobs:
  build:
    strategy:
      matrix:
        include:
          - { os: macos-14, target: aarch64-apple-darwin, ext: tar.gz }
          - { os: ubuntu-latest, target: x86_64-unknown-linux-gnu, ext: tar.gz }
          - { os: windows-latest, target: x86_64-pc-windows-msvc, ext: zip }
```

The steps after `cargo build --release --target ${{ matrix.target }}`:

```yaml
      - if: runner.os != 'Windows'
        run: tar -C target/${{ matrix.target }}/release -czf repograph-${{ matrix.target }}.tar.gz repograph
      # Run before it is packed: the pyke ONNX Runtime build for Windows is compiled with the
      # DirectML provider and imports its DLL, and a binary that cannot load says so here, on a
      # runner with nothing but the inbox copy, rather than on a user's machine.
      - if: runner.os == 'Windows'
        run: |
          .\target\${{ matrix.target }}\release\repograph.exe --version
          Compress-Archive -Path .\target\${{ matrix.target }}\release\repograph.exe -DestinationPath repograph-${{ matrix.target }}.zip
      - if: startsWith(github.ref, 'refs/tags/')
        uses: softprops/action-gh-release@v2
        with: { files: "repograph-${{ matrix.target }}.${{ matrix.ext }}" }
```

The `npm` job gains `if: startsWith(github.ref, 'refs/tags/')` beside `needs: build`, and its two `run` steps become:

```yaml
      - run: gh release download "$GITHUB_REF_NAME" --pattern 'repograph-*.tar.gz' --pattern 'repograph-*.zip'
        env: { GH_TOKEN: "${{ github.token }}" }
      - run: |
          mkdir -p bin/darwin-arm64 bin/linux-x64 bin/win32-x64
          tar -C bin/darwin-arm64 -xzf repograph-aarch64-apple-darwin.tar.gz
          tar -C bin/linux-x64 -xzf repograph-x86_64-unknown-linux-gnu.tar.gz
          unzip -q repograph-x86_64-pc-windows-msvc.zip -d bin/win32-x64
          scripts/npm-pack.sh "${GITHUB_REF_NAME#v}" darwin-arm64=bin/darwin-arm64/repograph linux-x64=bin/linux-x64/repograph win32-x64=bin/win32-x64/repograph.exe
```

The publish loop's globs (`*-arm64-*.tgz`, `*-x64-*.tgz`) already match `repograph-win32-x64-<v>.tgz`; the job's comment "its two platform packages" becomes "its three platform packages".

- [ ] **Step 2: `scripts/npm-pack.sh`**

- Usage line: `scripts/npm-pack.sh [version] [darwin-arm64=<binary>] [linux-x64=<binary>] [win32-x64=<binary.exe>]`.
- A third variable `win32_x64=""` and a third `case` arm `win32-x64=*) win32_x64=${arg#*=} ;;`.
- `pack_platform` takes the binary's name as a third argument, and checks presence rather than the execute bit — the Windows binary arrives from a zip on Linux with no bit to check:

```bash
pack_platform() {
  local platform=$1 src=$2 name=$3 dir
  [ -f "$src" ] || { echo "$platform: $src is not a file" >&2; exit 2; }
  dir=npm/repograph-$platform
  mkdir -p "$dir/bin"
  cp "$src" "$dir/bin/$name" && chmod 755 "$dir/bin/$name"
  (cd "$dir" && npm pkg set version="$version" && npm pack --silent --pack-destination "$root/$out")
}

[ -n "$darwin_arm64" ] && pack_platform darwin-arm64 "$darwin_arm64" repograph
[ -n "$linux_x64" ] && pack_platform linux-x64 "$linux_x64" repograph
[ -n "$win32_x64" ] && pack_platform win32-x64 "$win32_x64" repograph.exe
```

- The launcher loop: `for platform in darwin-arm64 linux-x64 win32-x64; do`.

- [ ] **Step 3: The packages and the launcher**

`npm/repograph-win32-x64/package.json`, the linux one with these fields changed:

```json
  "name": "@devmaxxx/repograph-win32-x64",
  "description": "repograph binary for Windows on x86-64 (MSVC); installed through @devmaxxx/repograph",
  "os": ["win32"],
  "cpu": ["x64"],
  "files": ["bin/repograph.exe"],
```

`npm/repograph/package.json` `optionalDependencies`: add `"@devmaxxx/repograph-win32-x64": "0.5.0"`.

`npm/repograph/bin/repograph.js`:

```js
const PACKAGES = {
  "darwin-arm64": "@devmaxxx/repograph-darwin-arm64",
  "linux-x64": "@devmaxxx/repograph-linux-x64",
  "win32-x64": "@devmaxxx/repograph-win32-x64",
};

// Cargo names the Windows binary with its extension, and `require.resolve` tries none of its
// own for a file that is not a module.
const BINARY = process.platform === "win32" ? "bin/repograph.exe" : "bin/repograph";
```

and `require.resolve(\`${pkg}/bin/repograph\`)` → `require.resolve(\`${pkg}/${BINARY}\`)`.

`.gitignore`: `/npm/*/bin/repograph` → add the line `/npm/*/bin/repograph.exe`.

- [ ] **Step 4: Judged on the Mac — the script and the launcher**

```bash
cd "$W" && bash -n scripts/npm-pack.sh && echo "syntax ok"
cp target/release/repograph "$M/fake.exe"     # the script never runs the binary; any file will do
scripts/npm-pack.sh 0.5.0 darwin-arm64=target/release/repograph win32-x64="$M/fake.exe" | tail -8
tar tzf dist/npm/devmaxxx-repograph-win32-x64-0.5.0.tgz
tar xzf dist/npm/devmaxxx-repograph-0.5.0.tgz -O package/package.json | grep -A3 optionalDependencies
git status --porcelain npm/   # the copied binaries must be ignored
# The launcher: the map, the error path, and the .exe resolution — process.platform stubbed.
node -e 'Object.defineProperty(process,"platform",{value:"win32"});Object.defineProperty(process,"arch",{value:"x64"});require("./npm/repograph/bin/repograph.js")'; echo "exit $?"
mkdir -p "$M/nm/node_modules/@devmaxxx/repograph-win32-x64/bin" && printf '#!/bin/sh\necho fake-exe "$@"\n' > "$M/nm/node_modules/@devmaxxx/repograph-win32-x64/bin/repograph.exe" && chmod 755 "$M/nm/node_modules/@devmaxxx/repograph-win32-x64/bin/repograph.exe"
cp npm/repograph/bin/repograph.js "$M/nm/launcher.js"
(cd "$M/nm" && node -e 'Object.defineProperty(process,"platform",{value:"win32"});Object.defineProperty(process,"arch",{value:"x64"});process.argv=[process.argv[0],"x","ask","hi"];require("./launcher.js")'); echo "exit $?"
rm -rf dist/npm
```

Expected, in order: `syntax ok`; the pack lists `devmaxxx-repograph-win32-x64-0.5.0.tgz` and the launcher's tarball; the tar listing shows `package/bin/repograph.exe` and `package/package.json`; the launcher's `optionalDependencies` names all three at `0.5.0`; `git status` shows only `npm/repograph-win32-x64/package.json` as new (the binaries ignored); the first `node` run prints `repograph: @devmaxxx/repograph-win32-x64 is not installed …` and `exit 2`; the second prints `fake-exe ask hi` and `exit 0` — the `.exe` path resolved and was spawned. (`npm pkg set` rewrote the three `package.json` versions to `0.5.0`, which is what they already say; `git diff npm/` must be the win32 entries only.)

- [ ] **Step 5: Judged on a runner — the release workflow from the branch**

```bash
git -C "$W" add .github/workflows/release.yml scripts/npm-pack.sh npm/ .gitignore
git -C "$W" commit -F - <<'MSG'
build(release): a Windows binary, zipped and smoked, and the win32-x64 npm package

x86_64-pc-windows-msvc joins the matrix; the exe is run once on the runner before it is packed,
because the ONNX Runtime build it links is compiled with DirectML and a load failure belongs in
the log, not on a user's machine. The npm launcher resolves bin/repograph.exe on win32.
workflow_dispatch builds and smokes without publishing, so a tag is not spent to find out.
MSG
git -C "$W" push
gh auth switch --user devmaxxx && gh workflow run release.yml --repo devmaxxx/repograph --ref feat/windows-serve
sleep 20; gh auth switch --user devmaxxx && gh run watch --repo devmaxxx/repograph --exit-status $(gh run list --repo devmaxxx/repograph --branch feat/windows-serve --workflow release --limit 1 --json databaseId --jq '.[0].databaseId') | tail -8 | tee "$M/task5-release.txt"
```

Expected: three `build` jobs green, the Windows one's log showing `repograph 0.5.0` from the smoke step and `Compress-Archive` completing; the upload and `npm` steps skipped (not a tag). Record the run URL in `$M/task5-release.txt`.

**Contingency, decided by that log and nothing else.** If the smoke step fails with `STATUS_DLL_NOT_FOUND` (exit `0xC0000135`) or "entry point not found": the exe's import table is not satisfied by the runner's inbox `DirectML.dll`. Then, in one further commit: add `ort = { version = "=2.0.0-rc.13", features = ["copy-dylibs"] }` under `[target.'cfg(windows)'.dependencies]` (a feature added for one target; unix compiles what it did), which makes `ort-sys` place the pyke archive's DLLs beside the built exe; change the Windows archive step to `Compress-Archive -Path .\target\…\release\repograph.exe, .\target\…\release\*.dll`; extend the win32 package's `files` to `["bin/repograph.exe", "bin/*.dll"]`; make `npm-pack.sh`'s win32 arm copy `*.dll` from the source's directory; and say in the README that the Windows binary ships the DirectML DLL its ONNX Runtime build imports. Re-run Step 5. If the smoke step passes, none of this exists and the README says nothing about it.

---

### Task 6: The story in words

**Files:**
- Modify: `README.md`, `npm/repograph/README.md`

**Interfaces:**
- Produces: every sentence that named two platforms names three; the serve section says what the Windows socket is; the cache and config paths have their Windows spelling; the one thing that still needs a POSIX shell on Windows says so.

Anchors are at `origin/main` `32577e2`; re-find by text, not by line.

- [ ] **Step 1: `README.md`**

- Install block comment (line 71): `# npm: prebuilt binary for macOS arm64 and Linux x64` → `# npm: prebuilt binary for macOS arm64, Linux x64 and Windows x64`.
- The launcher paragraph (line 75): `… comes from \`@devmaxxx/repograph-darwin-arm64\` or \`@devmaxxx/repograph-linux-x64\`, …` → the three names; `Both are under the same scope` → `All three are under the same scope`; `build both binaries as GitHub release assets` → `build all three binaries as GitHub release assets (a tarball for the unix ones, a zip holding \`repograph.exe\` for Windows)`.
- Serve section (line 184): `\`serve\` opens them once and answers over a Unix socket:` → `\`serve\` opens them once and answers over a Unix socket — on Windows too, where the same socket file has existed since Windows 10 1803 and is protected the way the repository directory is:`. After the fallback paragraph, one sentence: `The socket path is limited to 108 bytes on Linux and Windows and 104 on macOS; a repository deep enough to exceed it cannot start \`serve\`, and \`ask\` answers in its own process as it would with no server at all.`
- Config table (line 485): `else \`~/.config/repograph/config.toml\`` → `else \`~/.config/repograph/config.toml\` (\`%USERPROFILE%\.config\repograph\config.toml\` on Windows)`.
- Embeddings (line 526): `else \`~/.cache/repograph/fastembed\`` → `else \`~/.cache/repograph/fastembed\` (\`%USERPROFILE%\.cache\repograph\fastembed\` on Windows)`.
- Spending tokens, the paragraph beginning `\`enrich_command\` is any` (line 569): append `Both commands run under \`sh -c\`; on Windows that is Git for Windows' \`sh\`, which must be on \`PATH\` — nothing else in repograph needs a shell.`
- The `serve` row of the status table (line 47) is unchanged: the numbers there are the unix measurement and nothing in this plan re-measures.

- [ ] **Step 2: `npm/repograph/README.md`**

Line 23: the three package names; line 26: the Windows cache path in parentheses as above.

- [ ] **Step 3: Read back, commit, push**

```bash
cd "$W" && grep -n "both binaries\|Both are\|and Linux x64\b" README.md npm/repograph/README.md; echo "(expected: nothing)"
git -C "$W" add README.md npm/repograph/README.md
git -C "$W" commit -F - <<'MSG'
docs: the Windows story — the socket, the paths, the shell, the packages
MSG
git -C "$W" push
```

CI runs again (docs only; it should be green as before).

---

### Task 7: The pull request

**Files:** none.

- [ ] **Step 1: Open it**

```bash
gh auth switch --user devmaxxx && gh pr create --repo devmaxxx/repograph --base main --head feat/windows-serve \
  --title "feat(serve): the resident answer on Windows, and a Windows binary in the release" \
  --body-file - <<'BODY'
## What

`repograph ask` on Windows is answered by `repograph serve` the way it is on unix: the same
socket file, the same handshake, AF_UNIX through socket2 where std has no Unix socket. The crate
compiles on x86_64-pc-windows-msvc, CI runs clippy and the tests there, the release carries
`repograph.exe` in a zip, the npm launcher resolves `@devmaxxx/repograph-win32-x64`, and the
model cache and the machine config are found under the profile directory.

## Why AF_UNIX and not TCP or a named pipe

Argued in `docs/superpowers/plans/2026-09-06-windows-serve-transport.md` (Decision). In one
line: TCP turns a dead server's port into a thirty-second stall the moment another program takes
the port, and a named pipe needs a DACL and an anti-squatting check that nobody here can test;
the socket file keeps the directory's ACL, refuses a dead connect at once, and leaves the serve
loop untouched.

## Evidence

- Windows CI: <run URL from $M/task3-ci.txt> (serve tests), <run URL from $M/task4-ci.txt> (home).
- Release dry run from the branch, three targets, the Windows exe smoked on the runner: <run URL from $M/task5-release.txt>.
- Unix: the twelve serve tests unchanged in outcome; the socket round trip measured before/after on a self-index: <two medians from $M/task3-timing.txt>.

## Not done here

The release itself: the next `v*` tag exercises the upload and the npm publish for the third
package; `workflow_dispatch` proved everything before that step.
BODY
```

Expected: a PR URL. Report DONE with it and the paths under `$M`.

---

## Placeholder check, done when the plan was written

Every `$M/...` path is a file a step creates before another reads it; the run URLs the PR body cites are read from those files. The two expected unix test counts (`444`/`12`) are copied from the 2026-09-06 residue plan's Task 0 at the same tip and are confirmed or refuted by Task 0 Step 2 before anything depends on them. The Rust in Tasks 2, 3 and 4 is written against `src/serve.rs`, `tests/serve.rs`, `src/index/embed.rs`, `src/index/cross.rs`, `src/config.rs` and `src/index/mod.rs` as they stand at `32577e2` (read through `git show origin/main:`); the socket2 calls against `socket2-0.6.5`'s source in the local registry; the std claims against the 1.98.0 `rust-src` under `~/.rustup/toolchains/stable-aarch64-apple-darwin`. No step says "similar to" another. Nothing in this plan builds for Windows on the Mac; every Windows verdict is a CI run whose URL the plan writes down.
