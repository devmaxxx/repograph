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

/// What a client waits for an answer that is not reranked: a fused one is about 220 ms, and the
/// first of them also opens a 1.3 GB model, so thirty seconds is far longer than an answer costs
/// and still short enough that a wedged server hands the question straight back.
const IO_TIMEOUT: Duration = Duration::from_secs(30);

/// What a client waits for a reranked one instead. `--rerank` is about four seconds of model
/// round-trip; `--rerank-local` is 70-80 s cold on CPU for a 200-candidate pool, most of it the
/// 448 MB cross-encoder session the first such question opens. Under `IO_TIMEOUT` the client gave
/// up on a question the resident process went on to finish and throw away, then scored the same
/// pool itself behind its own cold open — the whole cost a second time, and no `serve:` line to
/// say so. Waiting is strictly cheaper than falling back for anything short of a server that will
/// never answer at all.
const RERANK_TIMEOUT: Duration = Duration::from_secs(300);

/// How long this request's reply is worth waiting for once the server has said it is working on
/// it. Only the read moves: the hello is one line, so the write stays on `IO_TIMEOUT` whatever
/// the question costs to answer.
fn reply_timeout(req: &ask::Request) -> Duration {
    if req.rerank || req.rerank_local { RERANK_TIMEOUT } else { IO_TIMEOUT }
}

/// Whether this request's answer is one the server acknowledges before it starts. The long wait
/// is right for a server that is reranking and wrong for one that will never answer, and the two
/// are indistinguishable from the client until the server says something.
fn acknowledged(req: &ask::Request) -> bool { reply_timeout(req) > IO_TIMEOUT }

/// How often the loop wakes to look at its `--every` and `--idle` deadlines while no question
/// is waiting. No client waits on it — the accept has a thread of its own — so it only has to
/// be finer than the deadlines it guards. An accept polled on a timer instead of blocked would
/// cost every question half that timer: at 50 ms, measured, half a lexical answer.
const WAKE: Duration = Duration::from_millis(250);

/// The socket a repository's server binds and its clients connect to: `.repograph/serve.sock`,
/// which is where a person looks for it — unless that name will not fit in `sun_path`, 104 bytes
/// on macOS including the NUL, in which case a name derived from the canonical repository path
/// goes in the temporary directory. 100 rather than 104 is one margin for every platform, and
/// four bytes is not worth two numbers. Server and client both come here, so the fallback is
/// never half-taken. The hash names a file; it defends nothing, which is why it is four lines of
/// FNV rather than a dependency.
pub fn socket_path(repo: &Path) -> PathBuf {
    let in_repo = repo.join(".repograph").join("serve.sock");
    if in_repo.as_os_str().len() <= 100 { return in_repo; }
    let canonical = std::fs::canonicalize(repo).unwrap_or_else(|_| repo.to_path_buf());
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in canonical.as_os_str().as_encoded_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    std::env::temp_dir().join(format!("repograph-{h:016x}.sock"))
}

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

/// What a server sends before a reranked answer, as soon as it has read a request it means to
/// answer: the line that separates a server which is working from one that never will. A client
/// waits `IO_TIMEOUT` for it and `RERANK_TIMEOUT` only after it. Its shape shares no required
/// field with `Reply`, so neither can be read as the other, and a server that does not send one
/// — an older build, or any request that is not reranked — answers on the first line as before.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Ack { pub ack: String }

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
    try_ask_within(repo, req, IO_TIMEOUT)
}

/// `try_ask` with the wait for the server's first line named, which is the only thing a test can
/// shorten without the server's cooperation.
fn try_ask_within(repo: &Path, req: &ask::Request, first_line: Duration) -> Option<Reply> {
    let path = socket_path(repo);
    if !sys::present(&path) { return None; }
    let mut stream = sys::connect(&path).ok()?;
    // Whatever the question costs the server, its first line is owed straight away.
    stream.set_read_timeout(Some(first_line)).ok()?;
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok()?;
    let build = build_stamp();
    let hello = serde_json::to_string(&Hello { v: VERSION.into(), build, req: req.clone() }).ok()?;
    writeln!(stream, "{hello}").ok()?;
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    // The acknowledgement buys the long wait; without one the reply had better be on this line.
    if acknowledged(req) && serde_json::from_str::<Ack>(&line).is_ok() {
        // Best-effort: on macOS, a peer that has already written the reply and closed makes this
        // setsockopt fail with EINVAL (the same quirk `hello_line` documents), even though the
        // reply is already sitting in this socket's receive buffer waiting to be read. Losing the
        // ability to extend the wait is not a reason to throw away an answer that already arrived.
        let _ = stream.set_read_timeout(Some(reply_timeout(req)));
        line.clear();
        reader.read_line(&mut line).ok()?;
    }
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

pub fn run(repo: &Path, cfg: &config::Config, every: u64, batch: usize, idle: u64, idle_model: u64, no_dense: bool) -> Result<()> {
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
    // After the guard exists, so a signal arriving between the two still finds something to run.
    catch_termination();
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
    eprintln!("serve: {} every {every}s, batch {batch}, idle {idle}s, model idle {idle_model}s; Ctrl-C stops", path.display());
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
        if terminated() { eprintln!("serve: terminated, exiting"); return Ok(()); }
        if drop_model_now(idle_model, last_request.elapsed(), ctx.model_open()) {
            ctx.drop_model();
            eprintln!("serve: no question for {idle_model}s, dropped the model — the next fused question pays the open");
        }
        if last_request.elapsed() >= Duration::from_secs(idle) { eprintln!("serve: idle for {idle}s, exiting"); return Ok(()); }
    }
}

/// Whether the weights should go now. Two thresholds read one clock: `--idle` ends the process,
/// `--idle-model` ends only the 1.3 GB behind the dense arm, because a server left up overnight is
/// worth its resident lexical answer and is not worth that. Zero turns the drop off, and a context
/// holding nothing is left alone so the line is printed once per drop rather than every wake.
fn drop_model_now(idle_model: u64, since: Duration, holding: bool) -> bool {
    idle_model > 0 && holding && since >= Duration::from_secs(idle_model)
}

/// A `SIGTERM`ed `serve` used to leave its socket file behind, because the unlink is a `Drop` and
/// a default-handled signal runs none. The handler sets a flag rather than unlinking: the loop
/// wakes every `WAKE` anyway, so a quarter second later it leaves through the same guard as every
/// other exit — the one that checks the socket's identity first, so a replacement server that has
/// already bound this name keeps its socket.
static TERMINATED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn terminated() -> bool { TERMINATED.load(std::sync::atomic::Ordering::Relaxed) }

#[cfg(unix)]
extern "C" fn note_term(_sig: libc::c_int) {
    // The one thing a handler may do here: a relaxed store to an atomic is async-signal-safe,
    // where an unlink of a name another process may now own is merely fast.
    TERMINATED.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Asks for `SIGTERM` and `SIGINT` to reach `terminated()` instead of killing the process where
/// its `Drop`s cannot run. On Windows there is no equivalent and the socket file is left for the
/// next `serve` to remove, which the README says.
#[cfg(unix)]
fn catch_termination() {
    for sig in [libc::SIGTERM, libc::SIGINT] {
        unsafe { libc::signal(sig, note_term as *const () as libc::sighandler_t) };
    }
}

#[cfg(not(unix))]
fn catch_termination() {}

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
    // Before any of the work: what the client is waiting on is the proof that someone is doing
    // it. A server wedged behind this line hands the question back in `IO_TIMEOUT`, as every
    // other request already does.
    if acknowledged(&hello.req) {
        writeln!(stream, "{}", serde_json::to_string(&Ack { ack: VERSION.into() })?)?;
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
    use super::{acknowledged, build_stamp, drop_model_now, reply_timeout, socket_path, sys, try_ask, try_ask_within, Ack, Reply, IO_TIMEOUT, RERANK_TIMEOUT, VERSION};
    use crate::ask::Request;
    use std::io::{BufRead, BufReader, Write};
    use std::time::Duration;

    /// The bug behind a `--rerank-local` question that never printed a `serve:` line: the client
    /// timed out at thirty seconds on an answer that takes seventy, threw away the resident one
    /// and paid a second cold open to answer it again. `tests/serve.rs` cannot reach this — there
    /// is no reranker model on the runner — so the choice is pinned here.
    #[test]
    fn a_reranked_question_is_waited_out_and_a_plain_one_is_not() {
        let plain = Request { words: vec!["чаевые".into()], json: false, seeds: 5, bodies: false, rerank: false, rerank_local: false, depth: crate::rerank::DEPTH, stale: false, no_dense: false };
        assert_eq!(reply_timeout(&plain), IO_TIMEOUT);
        assert_eq!(reply_timeout(&Request { rerank_local: true, ..plain.clone() }), RERANK_TIMEOUT);
        assert_eq!(reply_timeout(&Request { rerank: true, ..plain.clone() }), RERANK_TIMEOUT);
    }

    /// `--idle` exits, `--idle-model` forgets — two thresholds off the one clock, so a server
    /// left up overnight keeps the 6.8 ms lexical answer and not the 1.3 GB behind the dense one.
    #[test]
    fn the_weights_go_on_their_own_idle_and_only_while_something_is_held() {
        assert!(!drop_model_now(0, Duration::from_secs(9_999), true), "zero is off, at any age");
        assert!(!drop_model_now(300, Duration::from_secs(299), true));
        assert!(drop_model_now(300, Duration::from_secs(300), true), "the threshold is reached, not passed");
        assert!(!drop_model_now(300, Duration::from_secs(301), false),
                "nothing held: the line would otherwise print on every wake for the rest of the day");
    }


    /// `sun_path` is 104 bytes on macOS, including the NUL, so a repository under a deep enough
    /// path cannot bind a socket inside itself at all — which was every store copy the resource
    /// rounds worked on. The name moves; the way both sides compute it does not.
    #[test]
    fn a_deep_repository_gets_a_socket_that_fits() {
        let deep = std::path::PathBuf::from("/private/tmp").join("a".repeat(120));
        let p = socket_path(&deep);
        assert!(p.as_os_str().len() <= 100, "{}", p.display());
        assert!(p.file_name().unwrap().to_string_lossy().starts_with("repograph-"), "{}", p.display());
        assert_eq!(socket_path(&deep), p, "a client that computes it again finds the same name");
        assert_ne!(socket_path(&deep.join("x")), p, "another repository is another socket");
    }

    #[test]
    fn a_shallow_repository_keeps_the_socket_it_has_always_had() {
        let p = socket_path(std::path::Path::new("/tmp/r"));
        assert!(p.ends_with(".repograph/serve.sock"), "{}", p.display());
    }

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

    /// The long wait is for a server that is reranking, and a server that accepted the connection
    /// and will never answer looks exactly like one until it says otherwise. `serve` is an
    /// accelerator: a client may not do worse with one than without, and five silent minutes is
    /// as much worse as this protocol can get.
    #[test]
    fn a_server_that_never_answers_hands_a_reranked_question_back_on_the_first_line() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".repograph")).unwrap();
        let listener = sys::bind(&socket_path(dir.path())).unwrap();
        // Accepts, reads nothing, replies nothing -- a wedged worker, a process under a debugger,
        // a reranker child that hung.
        let held = std::thread::spawn(move || {
            let accepted = sys::accept(&listener);
            std::thread::sleep(Duration::from_secs(2));
            drop(accepted);
        });
        let req = Request { words: vec!["чаевые".into()], json: false, seeds: 5, bodies: false, rerank: true, rerank_local: false, depth: crate::rerank::DEPTH, stale: false, no_dense: false };
        assert!(acknowledged(&req), "this is the request that buys the long wait");
        let started = std::time::Instant::now();
        assert!(try_ask_within(dir.path(), &req, Duration::from_millis(300)).is_none(), "nothing came back to parse");
        assert!(started.elapsed() < Duration::from_secs(2), "fell back after {:?}", started.elapsed());
        let _ = held.join();
    }

    /// The extension only matters if a reply that was not yet on the wire when the ack was read
    /// is still picked up afterwards: the ack buys the wait, and this is what the wait is for.
    #[test]
    fn a_reply_that_arrives_after_the_ack_still_extends_the_wait_to_find_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".repograph")).unwrap();
        let listener = sys::bind(&socket_path(dir.path())).unwrap();
        let build = build_stamp();
        let held = std::thread::spawn(move || {
            let mut s = sys::accept(&listener).unwrap();
            let mut line = String::new();
            BufReader::new(s.try_clone().unwrap()).read_line(&mut line).unwrap();
            writeln!(s, "{}", serde_json::to_string(&Ack { ack: VERSION.into() }).unwrap()).unwrap();
            // Later than the shortened first-line wait below, so the read that finds it only
            // succeeds if the ack actually moved the deadline out to `reply_timeout`.
            std::thread::sleep(Duration::from_millis(700));
            let reply = Reply { v: VERSION.into(), build, no_dense: false, stdout: "hi".into(), stderr: vec![] };
            writeln!(s, "{}", serde_json::to_string(&reply).unwrap()).unwrap();
        });
        let req = Request { words: vec!["чаевые".into()], json: false, seeds: 5, bodies: false, rerank: true, rerank_local: false, depth: crate::rerank::DEPTH, stale: false, no_dense: false };
        let got = try_ask_within(dir.path(), &req, Duration::from_millis(300));
        held.join().unwrap();
        assert_eq!(got.map(|r| r.stdout), Some("hi".into()), "the late reply should still have been read after the ack extended the timeout");
    }

    /// On macOS, `setsockopt` for a read timeout fails with EINVAL once the peer has already
    /// written everything and closed — the same quirk `hello_line` documents, met here from the
    /// other side of the same socket. A server that answers fast enough to write its reply and
    /// return (dropping the connection) before the client gets to extend its own timeout must
    /// not cost the client an answer that already fully arrived.
    #[test]
    fn a_reply_already_behind_a_closed_peer_is_read_even_when_extending_the_timeout_fails() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".repograph")).unwrap();
        let listener = sys::bind(&socket_path(dir.path())).unwrap();
        let build = build_stamp();
        let held = std::thread::spawn(move || {
            let mut s = sys::accept(&listener).unwrap();
            let mut line = String::new();
            BufReader::new(s.try_clone().unwrap()).read_line(&mut line).unwrap();
            writeln!(s, "{}", serde_json::to_string(&Ack { ack: VERSION.into() }).unwrap()).unwrap();
            // No delay: the reply and the drop of `s` below race the client's own extension of
            // its read timeout, and on macOS this side wins often enough to pin the bug on.
            let reply = Reply { v: VERSION.into(), build, no_dense: false, stdout: "hi2".into(), stderr: vec![] };
            writeln!(s, "{}", serde_json::to_string(&reply).unwrap()).unwrap();
        });
        let req = Request { words: vec!["чаевые".into()], json: false, seeds: 5, bodies: false, rerank: true, rerank_local: false, depth: crate::rerank::DEPTH, stale: false, no_dense: false };
        let got = try_ask(dir.path(), &req);
        held.join().unwrap();
        assert_eq!(got.map(|r| r.stdout), Some("hi2".into()), "a reply that already fully arrived must not be thrown away over a timeout that could not be extended");
    }

    /// The two lines a client reads share no required field, so neither can be taken for the
    /// other: a reply on the first line is an older server's answer and is read as one.
    #[test]
    fn an_acknowledgement_and_a_reply_are_never_read_as_each_other() {
        let ack = serde_json::to_string(&Ack { ack: "0.5.3".into() }).unwrap();
        let reply = serde_json::to_string(&Reply {
            v: "0.5.3".into(), build: None, no_dense: false, stdout: String::new(), stderr: vec![],
        }).unwrap();
        assert!(serde_json::from_str::<Reply>(&ack).is_err(), "{ack}");
        assert!(serde_json::from_str::<Ack>(&reply).is_err(), "{reply}");
    }

    /// What `run` does on the way out: the listener is still alive on its thread when the guard
    /// removes the file. If the platform refused this, the cost would be a stale file for the
    /// next `serve` to sweep — worth knowing rather than guessing.
    #[test]
    fn the_socket_file_is_removed_while_its_listener_is_still_bound() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("serve.sock");
        let _listener = sys::bind(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(!sys::present(&path));
    }

    /// The whole reason for a socket file over a port: a name nobody listens on is settled at
    /// once, not accepted by a stranger and waited out. Windows and Linux refuse the connect;
    /// macOS sometimes completes it against the file a dropped listener left and hands back a
    /// stream with nothing behind it, which reads as end of file — what `try_ask` does with a
    /// reply it cannot parse is answer in its own process, so the outcome is the same either way.
    #[test]
    fn a_connect_to_a_socket_nobody_listens_on_is_settled_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("serve.sock");
        drop(sys::bind(&path).unwrap());
        let started = std::time::Instant::now();
        match sys::connect(&path) {
            Err(err) => assert_eq!(err.kind(), std::io::ErrorKind::ConnectionRefused, "{err}"),
            Ok(stream) => {
                if cfg!(windows) { panic!("a socket nobody listens on is refused on Windows, and this connect was taken"); }
                stream.set_read_timeout(Some(std::time::Duration::from_secs(1))).unwrap();
                let mut line = String::new();
                let read = BufReader::new(stream).read_line(&mut line);
                assert!(matches!(read, Ok(0) | Err(_)), "nothing is behind it: {read:?} {line:?}");
            }
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(2), "settled after {:?}", started.elapsed());
    }

    /// A user's name is in the socket's path, and on Windows the path crosses into `sun_path` as
    /// UTF-8: two bytes a Cyrillic letter, and a conversion the runner's own ASCII path never makes.
    #[test]
    fn a_socket_binds_and_connects_under_a_directory_that_is_not_ascii() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("Максим");
        std::fs::create_dir(&home).unwrap();
        let path = home.join("serve.sock");
        let _listener = sys::bind(&path).unwrap();
        assert!(sys::present(&path));
        sys::connect(&path).unwrap();
    }
}
