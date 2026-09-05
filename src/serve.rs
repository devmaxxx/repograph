//! `repograph serve`: the store, the vectors, the model and the indexes held by one process
//! that answers `ask` over a Unix socket. A fused answer costs a process ~480 ms to open the
//! model (measured on the bench corpus) and 5 ms to use it; a resident one pays the open once.
use crate::{ask, config};
use anyhow::{Context as _, Result};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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

/// The device and inode of a socket file, which is how a name is told from the thing that was
/// bound to it: the path can hold a replacement server's socket by the time this one exits.
fn socket_id(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let m = std::fs::metadata(path).ok()?;
    Some((m.dev(), m.ino()))
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Hello { pub v: String, pub req: ask::Request }

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Reply { pub v: String, pub stdout: String, pub stderr: Vec<String> }

/// The resident answer, or None when this process has to answer: no socket, one nobody
/// listens on, another version, a timeout, or a reply that does not parse.
///
/// Nothing here removes the socket file. A refused connect is a socket nobody listens on — or a
/// live listener whose backlog is full for this instant, and errno does not tell the two apart;
/// deleting the file on that guess would strand a running server holding 1.3 GB, and its own
/// exit would then take the replacement's socket with it. `serve` removes the file it bound
/// itself, and removes a dead one before it binds. All a client owes the question is an answer,
/// and it has one either way.
pub fn try_ask(repo: &Path, req: &ask::Request) -> Option<Reply> {
    let path = socket_path(repo);
    if !path.exists() { return None; }
    let mut stream = UnixStream::connect(&path).ok()?;
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok()?;
    let hello = serde_json::to_string(&Hello { v: VERSION.into(), req: req.clone() }).ok()?;
    writeln!(stream, "{hello}").ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    let reply: Reply = serde_json::from_str(&line).ok()?;
    if reply.v != VERSION {
        // Said out loud: a leftover server from another build answers nothing and every
        // question quietly costs a cold process instead, which looks like nothing at all.
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
    let path = socket_path(repo);
    if path.exists() && UnixStream::connect(&path).is_ok() { anyhow::bail!("another serve answers at {}", path.display()); }
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).with_context(|| format!("bind {}", path.display()))?;
    // Stamped the instant it exists, and re-read on the way out: the probe above cannot tell a
    // dead socket from a live server whose backlog is momentarily full, so a replacement may
    // have unlinked this file and bound its own at the same name while this one ran. Removing
    // the name rather than the socket would then strand the replacement — the same cascade the
    // client's unlink was deleted to avoid, one process further along.
    let _guard = Unlink(path.clone(), socket_id(&path));
    // The accept blocks on a thread of its own and hands each connection over, one at a time —
    // the answer still happens here, on the one thread that holds the context. A rendezvous
    // channel is what keeps it to one: the next connection is accepted but not delivered until
    // this loop is free, and the one after that waits in the kernel's backlog.
    let (tx, rx) = std::sync::mpsc::sync_channel::<UnixStream>(0);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(s) => if tx.send(s).is_err() { return; },
                Err(_) => return,
            }
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
            Ok(stream) => match answer(stream, &mut watcher, &mut ctx) {
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
fn hello_line(stream: &UnixStream) -> Option<String> {
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok()?;
    let mut line = String::new();
    let n = BufReader::new(stream.try_clone().ok()?).read_line(&mut line).ok()?;
    (n > 0).then_some(line)
}

/// Whether a question was asked, which is what `--idle` counts — a peer that vanished before
/// its hello asked nothing.
fn answer(mut stream: UnixStream, watcher: &mut crate::Watcher, ctx: &mut ask::Context) -> Result<bool> {
    let Some(line) = hello_line(&stream) else { return Ok(false) };
    let hello: Hello = serde_json::from_str(&line).context("hello")?;
    if hello.v != VERSION {
        writeln!(stream, "{}", serde_json::to_string(&Reply { v: VERSION.into(), stdout: String::new(), stderr: vec![] })?)?;
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
    let reply = Reply { v: VERSION.into(), stdout, stderr: ctx.notices() };
    writeln!(stream, "{}", serde_json::to_string(&reply)?)?;
    Ok(true)
}

struct Unlink(PathBuf, Option<(u64, u64)>);
impl Drop for Unlink {
    fn drop(&mut self) {
        if self.1.is_some() && socket_id(&self.0) == self.1 { let _ = std::fs::remove_file(&self.0); }
    }
}
