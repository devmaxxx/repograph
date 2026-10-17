//! Background priority for the writes zerograph starts on its own: the detached `update` a reader,
//! a git hook or the MCP server hands a refresh to, `watch`, and `serve`'s poll between questions.
//! Nobody waits on any of them, and a build, a test run or an editor running beside them should
//! not wait because of them either. A command someone typed and is waiting on — `ask`, `build`,
//! `update` without `--detach`, `embed`, `model` — keeps the priority it was started with.
//!
//! This is not the band removed on 2026-09-09 (docs/history.md): that one took every rebuild,
//! foreground ones included, and four times the wall was paid by whoever was waiting on it.
//!
//! Failing to lower is never an error: the work is the same at normal priority, so the reason
//! goes to the run's stderr — `background.log` for a detached refresh — and the work goes on.

/// The whole process, for one that only writes. Called before it starts a thread: on Linux
/// priority belongs to a thread, and a thread takes its creator's when it is spawned, so the ONNX
/// and tokenizer pools started later run lowered too.
pub(crate) fn background_process() {
    if let Err(e) = lower(Scope::Process) { eprintln!("priority: not lowered ({e}); running at normal priority"); }
}

/// The calling thread alone, for a process whose other thread answers questions. The thread is
/// meant to end with its work: on Linux an unprivileged thread can lower its nice value but never
/// raise it back.
pub(crate) fn background_thread() {
    if let Err(e) = lower(Scope::Thread) { eprintln!("priority: not lowered ({e}); running at normal priority"); }
}

#[derive(Clone, Copy)]
enum Scope { Process, Thread }

/// Darwin's background band: the scheduler runs the work only on time nothing else wants, and its
/// disk and network I/O are throttled behind everyone else's.
#[cfg(target_os = "macos")]
fn lower(scope: Scope) -> std::io::Result<()> {
    let which = match scope { Scope::Process => libc::PRIO_DARWIN_PROCESS, Scope::Thread => libc::PRIO_DARWIN_THREAD };
    match unsafe { libc::setpriority(which, 0, libc::PRIO_DARWIN_BG) } {
        0 => Ok(()),
        _ => Err(std::io::Error::last_os_error()),
    }
}

/// Nice 19 and the idle I/O class. On Linux `who = 0` names the calling thread, not the process,
/// so the same two calls serve both scopes: made before any other thread exists, they are the
/// process's.
#[cfg(target_os = "linux")]
fn lower(_scope: Scope) -> std::io::Result<()> {
    // linux/ioprio.h; libc carries the syscall number but not these.
    const IOPRIO_WHO_PROCESS: libc::c_int = 1;
    const IOPRIO_CLASS_IDLE: libc::c_int = 3;
    const IOPRIO_CLASS_SHIFT: libc::c_int = 13;
    let nice = match unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, 19) } {
        0 => Ok(()),
        _ => Err(std::io::Error::last_os_error()),
    };
    let io = match unsafe { libc::syscall(libc::SYS_ioprio_set, IOPRIO_WHO_PROCESS, 0, IOPRIO_CLASS_IDLE << IOPRIO_CLASS_SHIFT) } {
        0 => Ok(()),
        _ => Err(std::io::Error::last_os_error()),
    };
    nice.and(io)
}

/// Background processing mode: low CPU, I/O and memory priority together. Either call works only
/// on the current process or thread, which is the one asking.
#[cfg(windows)]
fn lower(scope: Scope) -> std::io::Result<()> {
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentThread, SetPriorityClass, SetThreadPriority, PROCESS_MODE_BACKGROUND_BEGIN, THREAD_MODE_BACKGROUND_BEGIN,
    };
    let ok = match scope {
        Scope::Process => unsafe { SetPriorityClass(GetCurrentProcess(), PROCESS_MODE_BACKGROUND_BEGIN) },
        Scope::Thread => unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) },
    };
    match ok {
        0 => Err(std::io::Error::last_os_error()),
        _ => Ok(()),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn lower(_scope: Scope) -> std::io::Result<()> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "no background priority on this platform"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the calling thread runs at: Darwin's background flag, or Linux's nice value and I/O class.
    #[cfg(target_os = "macos")]
    fn current() -> (i32, i32) { (unsafe { libc::getpriority(libc::PRIO_DARWIN_THREAD, 0) }, 0) }

    #[cfg(target_os = "linux")]
    fn current() -> (i32, i32) {
        const IOPRIO_WHO_PROCESS: libc::c_int = 1;
        let nice = unsafe { libc::getpriority(libc::PRIO_PROCESS, 0) };
        let io = unsafe { libc::syscall(libc::SYS_ioprio_get, IOPRIO_WHO_PROCESS, 0) } as i32;
        (nice, io >> 13)
    }

    // On a thread of its own, so the test binary's other threads keep running at normal priority;
    // the thread that answers is the one this must leave alone.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn a_background_thread_is_lowered_and_the_thread_beside_it_is_not() {
        let before = current();
        let lowered = std::thread::spawn(|| { background_thread(); current() }).join().unwrap();
        // xnu answers 1 for a thread in the background band, not the flag it was set with.
        #[cfg(target_os = "macos")]
        assert_eq!((before.0, lowered.0), (0, 1));
        #[cfg(target_os = "linux")]
        assert_eq!(lowered, (19, 3), "nice 19 and the idle I/O class");
        assert_eq!(current(), before, "the calling thread kept its priority");
    }

    #[cfg(windows)]
    #[test]
    fn a_background_thread_is_lowered_without_an_error() {
        std::thread::spawn(|| lower(Scope::Thread).unwrap()).join().unwrap();
    }
}
