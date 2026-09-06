//! `repograph serve`: the store, the vectors, the model and the indexes held by one process
//! that answers `ask` over a Unix socket. A fused answer costs a process ~220 ms to open the
//! model (measured on the bench corpus) and 5 ms to use it; a resident one pays the open once.
use crate::{ask, config};
use anyhow::{Context as _, Result};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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
    ///
    /// `MetadataExt` carries both numbers and has carried them behind an unstable feature since
    /// 2019 (rust-lang/rust#63010), so the handle is asked instead — the open std's own
    /// `symlink_metadata` performs, for the same reason: no access rights, because this is a
    /// metadata question; every share right, because a server is listening on the file while it
    /// is asked; and the reparse point unfollowed, so the numbers are the socket file's own and
    /// never a target's.
    pub fn id(path: &Path) -> Option<(u64, u64)> {
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS,
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        let file = std::fs::OpenOptions::new()
            .access_mode(0)
            .share_mode(FILE_SHARE_DELETE | FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .ok()?;
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: the handle outlives the call, and the struct is only read back where the call
        // reports the kernel filled it in.
        let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) };
        if ok == 0 {
            return None;
        }
        Some((
            u64::from(info.dwVolumeSerialNumber),
            u64::from(info.nFileIndexHigh) << 32 | u64::from(info.nFileIndexLow),
        ))
    }
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A reranked answer takes about four seconds and the first fused one also opens a 1.3 GB
/// model, so the client waits far longer than an answer costs before it gives up and answers
/// the question in this process instead.
const IO_TIMEOUT: Duration = Duration::from_secs(30);

/// How often the loop wakes to look at its `--every` and `--idle` deadlines while no question
/// is waiting. No client waits on it — the accept has a thread of its own — so it only has to
/// be finer than the deadlines it guards. An accept polled on a timer instead of blocked would
/// cost every question half that timer: at 50 ms, measured, half a lexical answer.
const WAKE: Duration = Duration::from_millis(250);

pub fn socket_path(repo: &Path) -> PathBuf { repo.join(".repograph").join("serve.sock") }

/// What a `stat` says about `repograph.toml`, or None where there is none — the configuration a
/// one-shot `ask` would read, watched so a resident process cannot answer under an older one.
fn config_stamp(repo: &Path) -> Option<crate::walk::Stamp> {
    crate::walk::stamp_of(&std::fs::metadata(repo.join("repograph.toml")).ok()?)
}

/// What a `stat` says about the executable at this process's own path, or None where it cannot be
/// found. The version alone cannot tell two builds apart: the version moves only at a release,
/// so every development build in between answers the same string, and the README suggests `--idle
/// 86400` — rebuild, ask while yesterday's `serve` is up, and yesterday's code answers with
/// nothing to show for it. Read once per process and held, never per request: `current_exe` names
/// a path, and a rebuild replaces the file at it, so a server that re-read this would answer with
/// the stamp of the very binary the client is asking from and wave that pairing through. A
/// short-lived `ask` reads it at the one instant it runs, which is the same thing. Two copies of
/// one build stamp differently, which costs a fallback and never a wrong answer.
fn build_stamp() -> Option<crate::walk::Stamp> {
    crate::walk::stamp_of(&std::fs::metadata(std::env::current_exe().ok()?).ok()?)
}

/// The client's half of the handshake. `req.no_dense` is the arm the question was asked in;
/// the server's own arm comes back in the `Reply`, because only the server knows it.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Hello { pub v: String, #[serde(default)] pub build: Option<crate::walk::Stamp>, pub req: ask::Request }

/// The server's half, and its answer. `no_dense` is the arm this process was started in — a
/// server that opened no model can only answer lexically, and a client that asked a fused
/// question has to be told rather than handed a lexical answer under a fused question's name.
/// Both new fields default, so a reply in the older shape still parses. Between releases the
/// version it carries is this one, and what refuses it is the build it does not carry.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Reply {
    pub v: String,
    #[serde(default)] pub build: Option<crate::walk::Stamp>,
    #[serde(default)] pub no_dense: bool,
    pub stdout: String,
    pub stderr: Vec<String>,
}

/// The resident answer, or None when this process has to answer: no socket, one nobody
/// listens on, another version, another build of this version, a server that cannot answer in
/// the arm the question was asked in, a timeout, or a reply that does not parse.
///
/// Nothing here removes the socket file. A refused connect is a socket nobody listens on — or a
/// live listener whose backlog is full for this instant, and errno does not tell the two apart;
/// deleting the file on that guess would strand a running server holding 1.3 GB, and its own
/// exit would then take the replacement's socket with it. `serve` removes the file it bound
/// itself, and removes a dead one before it binds. All a client owes the question is an answer,
/// and it has one either way.
pub fn try_ask(repo: &Path, req: &ask::Request) -> Option<Reply> {
    let path = socket_path(repo);
    if !sys::present(&path) { return None; }
    let mut stream = sys::connect(&path).ok()?;
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok()?;
    let build = build_stamp();
    let hello = serde_json::to_string(&Hello { v: VERSION.into(), build, req: req.clone() }).ok()?;
    writeln!(stream, "{hello}").ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    let reply: Reply = serde_json::from_str(&line).ok()?;
    // Said out loud, all three: a question that quietly costs a cold process, or quietly gets a
    // lexical answer, looks like nothing at all. The build and the arm come first because a
    // refusal carries every field and only those two say which of them sent the question back;
    // the version is last because a refusal's own `v` is unmatchable by construction, and this
    // client should print the reason rather than that backstop.
    if reply.build != build {
        eprintln!("serve: the resident process is another build of {VERSION}; answering here");
        return None;
    }
    // This one matters most: a lexical answer to a fused question is an answer, so nothing about
    // it looks wrong. The other direction is not a mismatch — a server holding the model answers
    // a `--no-dense` question lexically, which is what was asked for.
    if reply.no_dense && !req.no_dense {
        eprintln!("serve: the resident process answers lexical-only and this question is fused; answering here");
        return None;
    }
    if reply.v != VERSION {
        eprintln!("serve: the resident process is version {}, this is {VERSION}; answering here", reply.v);
        return None;
    }
    Some(reply)
}

pub fn run(repo: &Path, cfg: &config::Config, every: u64, batch: usize, idle: u64, no_dense: bool) -> Result<()> {
    // Stamped first of all, because `cfg` was read before this call and the two opens below take
    // seconds on a large store: a file edited inside that window would be stamped as the baseline
    // and this process would then answer under the configuration from before the edit for as long
    // as it ran — the divergence the poll's check exists to prevent, narrowed to a start-up race.
    let cfg_stamp = config_stamp(repo);
    // Read here and carried, for the reason `build_stamp` gives: this process runs the build that
    // was at its path when it started, and nothing it stats later can still say so.
    let build = build_stamp();
    let path = socket_path(repo);
    if sys::present(&path) && sys::connect(&path).is_ok() { anyhow::bail!("another serve answers at {}", path.display()); }
    let _ = std::fs::remove_file(&path);
    let listener = sys::bind(&path).with_context(|| format!("bind {}", path.display()))?;
    // Stamped the instant it exists, and re-read on the way out: the probe above cannot tell a
    // dead socket from a live server whose backlog is momentarily full, so a replacement may
    // have unlinked this file and bound its own at the same name while this one ran. Removing
    // the name rather than the socket would then strand the replacement — the same cascade the
    // client's unlink was deleted to avoid, one process further along.
    let _guard = Unlink(path.clone(), sys::id(&path));
    // The accept blocks on a thread of its own and hands each connection over, one at a time —
    // the answer still happens here, on the one thread that holds the context. A rendezvous
    // channel is what keeps it to one: the next connection is accepted but not delivered until
    // this loop is free, and the one after that waits in the kernel's backlog.
    let (tx, rx) = std::sync::mpsc::sync_channel::<sys::Stream>(0);
    std::thread::spawn(move || loop {
        match sys::accept(&listener) {
            Ok(s) => if tx.send(s).is_err() { return; },
            Err(_) => return,
        }
    });
    let mut watcher = crate::Watcher::open(repo, cfg)?;
    // Opened stale: the watcher has just walked the tree and owns every later walk. Whatever
    // `open` has to say happened before any request existed, so it is written here rather than
    // handed to whichever client happens to connect first.
    let mut ctx = ask::Context::open(repo, cfg, true, no_dense)?;
    log(&mut ctx);
    eprintln!("serve: {} every {every}s, batch {batch}, idle {idle}s; Ctrl-C stops", path.display());
    let (mut last_poll, mut last_request) = (Instant::now(), Instant::now());
    loop {
        match rx.recv_timeout(WAKE) {
            // Only a question that arrived counts against `--idle`. A bare connect and close is
            // a liveness probe — another `serve` deciding whether to bind, a client that gave
            // up — and treating one as a request keeps this process resident, holding 1.3 GB,
            // for as long as anything at all polls the socket.
            Ok(stream) => match answer(stream, &mut watcher, &mut ctx, build) {
                Ok(asked) => if asked { last_request = Instant::now(); },
                Err(e) => { last_request = Instant::now(); eprintln!("serve: {e:#}"); }
            },
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => anyhow::bail!("the listener stopped accepting"),
        }
        if last_poll.elapsed() >= Duration::from_secs(every) {
            last_poll = Instant::now();
            // The configuration is read once, at start: an id family added, a model or a rerank
            // command changed would otherwise divide this process's answers from a one-shot's
            // for as long as it ran. Leaving is the whole of the fix — the next `ask` answers
            // in its own process under the new file, and the next `serve` starts under it too.
            if config_stamp(repo) != cfg_stamp {
                eprintln!("serve: repograph.toml changed, exiting — start serve again to answer under it");
                return Ok(());
            }
            adopt_if_moved(&mut watcher, &mut ctx, batch)?;
            // A poll between requests answers nobody: its notices are the server's own.
            log(&mut ctx);
        }
        if last_request.elapsed() >= Duration::from_secs(idle) { eprintln!("serve: idle for {idle}s, exiting"); return Ok(()); }
    }
}

/// Notices no client asked for, on the server's stderr.
fn log(ctx: &mut ask::Context) {
    for n in ctx.notices() { eprintln!("serve: {n}"); }
}

/// The poll `ask` does before it answers, and the one case the poll's own answer does not
/// name: a store some other process wrote, which the poll read back into the watcher. Either
/// way the watcher now holds a graph the context does not, and only an adopt leaves a reader
/// where a one-shot `ask` would have been after loading that store from disk.
fn adopt_if_moved(watcher: &mut crate::Watcher, ctx: &mut ask::Context, batch: usize) -> Result<()> {
    match watcher.poll(batch)? {
        crate::Polled::Refreshed(r) => ctx.adopt(watcher, Some(r))?,
        _ if watcher.reloaded => ctx.adopt(watcher, None)?,
        _ => {}
    }
    Ok(())
}

/// The client's hello, or None when the peer was gone before it said anything. A connect that
/// closes with nothing on it is a probe, not a question: `serve`'s own check for another server
/// makes exactly that shape, and so does a client that gave up. On macOS a socket with no other
/// end refuses the `setsockopt` below with EINVAL, which reached the server's stderr as an
/// unattributable `serve: Invalid argument (os error 22)`; elsewhere it is an empty read.
fn hello_line(stream: &sys::Stream) -> Option<String> {
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok()?;
    let mut line = String::new();
    let n = BufReader::new(stream.try_clone().ok()?).read_line(&mut line)
        .inspect_err(|e| if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) {
            eprintln!("serve: a client connected and said nothing for {}s; not counted as a question", IO_TIMEOUT.as_secs());
        }).ok()?;
    (n > 0).then_some(line)
}

/// Whether a question was asked, which is what `--idle` counts — a peer that vanished before
/// its hello asked nothing.
fn answer(mut stream: sys::Stream, watcher: &mut crate::Watcher, ctx: &mut ask::Context, build: Option<crate::walk::Stamp>) -> Result<bool> {
    let Some(line) = hello_line(&stream) else { return Ok(false) };
    let hello: Hello = serde_json::from_str(&line).context("hello")?;
    // The refusals, all three under one reply: the build and the arm on it name which of them
    // sent the question back, and the version on it matches no client, present or past, so none
    // can take a refusal for an answer. Answering first and refusing after would spend a fused
    // answer's work on a reply nobody reads.
    if hello.v != VERSION || hello.build != build || (ctx.no_dense() && !hello.req.no_dense) {
        writeln!(stream, "{}", serde_json::to_string(&refusal(ctx, build))?)?;
        return Ok(true);
    }
    // Anything still waiting predates this request — a poll's refresh, an answer that ended in
    // an error. A client is told what its own answer did and nothing else.
    log(ctx);
    if hello.req.stale {
        // `--stale` skips the walk, not the store. A one-shot answers from whatever is on disk
        // at this instant, so a resident one reads the store back when another process has
        // written it — the stat and a load, and none of the walk that was asked to be skipped.
        if watcher.reload_if_moved()? { ctx.adopt(watcher, None)?; }
    } else {
        // The same refresh a one-shot ask does before answering: batch 1 applies any change now.
        adopt_if_moved(watcher, ctx, 1)?;
    }
    let stdout = ctx.answer(&hello.req)?;
    let stderr = ctx.notices();
    let reply = Reply { stdout, stderr, ..header(ctx, build) };
    writeln!(stream, "{}", serde_json::to_string(&reply)?)?;
    Ok(true)
}

/// What this process is, with no answer in it: the three fields a client decides on.
fn header(ctx: &ask::Context, build: Option<crate::walk::Stamp>) -> Reply {
    Reply { v: VERSION.into(), build, no_dense: ctx.no_dense(), stdout: String::new(), stderr: vec![] }
}

/// The same header under a version no build carries. An empty `stdout` is a refusal here and a
/// perfectly good answer to a question nothing matched, and the two are told apart by fields a
/// client older than this path never reads: such a client checks `v`, matches, prints nothing and
/// exits 0 — the silent non-answer the refusal was written to stop, one binary generation on. A
/// version it cannot match sends it back to its own process instead, and the fields a current
/// client reads first still name the reason.
fn refusal(ctx: &ask::Context, build: Option<crate::walk::Stamp>) -> Reply {
    Reply { v: format!("{VERSION} (refused)"), ..header(ctx, build) }
}

struct Unlink(PathBuf, Option<(u64, u64)>);
impl Drop for Unlink {
    fn drop(&mut self) {
        if self.1.is_some() && sys::id(&self.0) == self.1 { let _ = std::fs::remove_file(&self.0); }
    }
}

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
