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

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Hello { pub v: String, pub req: ask::Request }

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Reply { pub v: String, pub stdout: String, pub stderr: Vec<String> }

/// The resident answer, or None when this process has to answer: no socket, one nobody
/// listens on (unlinked here, so the next start binds cleanly), another version, a timeout,
/// or a reply that does not parse.
pub fn try_ask(repo: &Path, req: &ask::Request) -> Option<Reply> {
    let path = socket_path(repo);
    if !path.exists() { return None; }
    let Some(mut stream) = connect(&path) else {
        let _ = std::fs::remove_file(&path);
        return None;
    };
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok()?;
    let hello = serde_json::to_string(&Hello { v: VERSION.into(), req: req.clone() }).ok()?;
    writeln!(stream, "{hello}").ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    let reply: Reply = serde_json::from_str(&line).ok()?;
    if reply.v != VERSION { return None; }
    Some(reply)
}

/// A refused connect is a socket nobody listens on — or a listener whose backlog is full for
/// this instant, and the error does not tell the two apart. A dead socket refuses twice; a
/// burst of clients rarely does, and unlinking a live server's socket would send every later
/// question to a cold process for as long as that server ran.
fn connect(path: &Path) -> Option<UnixStream> {
    UnixStream::connect(path).or_else(|_| UnixStream::connect(path)).ok()
}

pub fn run(repo: &Path, cfg: &config::Config, every: u64, batch: usize, idle: u64, no_dense: bool) -> Result<()> {
    let path = socket_path(repo);
    if path.exists() && UnixStream::connect(&path).is_ok() { anyhow::bail!("another serve answers at {}", path.display()); }
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).with_context(|| format!("bind {}", path.display()))?;
    let _guard = Unlink(path.clone());
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
            Ok(stream) => {
                last_request = Instant::now();
                if let Err(e) = answer(stream, &mut watcher, &mut ctx) { eprintln!("serve: {e:#}"); }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => anyhow::bail!("the listener stopped accepting"),
        }
        if last_poll.elapsed() >= Duration::from_secs(every) {
            last_poll = Instant::now();
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

fn answer(mut stream: UnixStream, watcher: &mut crate::Watcher, ctx: &mut ask::Context) -> Result<()> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let hello: Hello = serde_json::from_str(&line).context("hello")?;
    if hello.v != VERSION {
        writeln!(stream, "{}", serde_json::to_string(&Reply { v: VERSION.into(), stdout: String::new(), stderr: vec![] })?)?;
        return Ok(());
    }
    // Anything still waiting predates this request — a poll's refresh, an answer that ended in
    // an error. A client is told what its own answer did and nothing else.
    log(ctx);
    if !hello.req.stale {
        // The same refresh a one-shot ask does before answering: batch 1 applies any change now.
        adopt_if_moved(watcher, ctx, 1)?;
    }
    let stdout = ctx.answer(&hello.req)?;
    let reply = Reply { v: VERSION.into(), stdout, stderr: ctx.notices() };
    writeln!(stream, "{}", serde_json::to_string(&reply)?)?;
    Ok(())
}

struct Unlink(PathBuf);
impl Drop for Unlink { fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); } }
