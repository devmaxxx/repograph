//! Which scheduling band a writer's threads run in.
//!
//! A rebuild is not something anyone waits for, and it is something everyone feels: on this
//! machine the shipped foreground run cost the person's own six-thread compile 15.7% of its speed
//! and stretched a 1 ms wake from 269 µs to 2.5 ms at p99, on performance cores at 3.49 GHz. One
//! call moves the whole process to the background band — +1.7% on the same compile, 603 µs at
//! p99, 1.70 GHz — for 4.1× the wall time (docs/bench/2026-09-07-unnoticeable-results.md).
//! Writers take that trade by default; readers answer a person and never call this.

use anyhow::{bail, Result};
use serde::Deserialize;

/// `background` is the default because a rebuild has nobody waiting on it. `normal` is the whole
/// of the other direction, for a build server or a person who would rather have the wall time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    #[default]
    Background,
    Normal,
}

/// The configured value unless `REPOGRAPH_PRIORITY` names another, in the shape the other run
/// variables use: unset or empty keeps what the files said.
pub fn from_env(configured: Priority, var: Option<&str>) -> Result<Priority> {
    let Some(raw) = var else { return Ok(configured) };
    let word = raw.trim();
    if word.is_empty() {
        return Ok(configured);
    }
    match word.to_ascii_lowercase().as_str() {
        "background" => Ok(Priority::Background),
        "normal" => Ok(Priority::Normal),
        other => bail!("REPOGRAPH_PRIORITY is {other:?}, which is neither \"background\" nor \"normal\""),
    }
}

/// Lowers this process for the rest of its life. One-way on purpose: every caller either exits
/// after its embed or, in `watch`'s case, answers nobody for as long as it runs — and Linux
/// cannot raise a nice value back without `CAP_SYS_NICE`, so a restore would promise on one
/// platform what the other could not keep.
///
/// Call it before `cap_pools` and before any model session is opened. On macOS the ordering is
/// free, but on Linux the nice value, the scheduling policy and the I/O context are per-thread
/// and copied at `clone`, so a thread that already exists keeps the band it was born in.
pub fn apply(p: Priority) {
    if p == Priority::Normal {
        return;
    }
    lower();
}

/// One call rather than one per thread: `PRIO_DARWIN_PROCESS` is a task policy, so ORT's intra-op
/// workers and rayon's — created later, inside libraries that offer no hook to reach them — land
/// in the same band as the main thread. The per-thread `QOS_CLASS_BACKGROUND` is not inherited
/// and would need a thread factory per pool to reach what this reaches. It also throttles the
/// process's disk and its later-opened sockets, which is why there is no second call here.
#[cfg(target_os = "macos")]
fn lower() {
    if unsafe { libc::setpriority(libc::PRIO_DARWIN_PROCESS, 0, libc::PRIO_DARWIN_BG) } != 0 {
        warn("setpriority(PRIO_DARWIN_PROCESS, PRIO_DARWIN_BG)");
    }
}

/// Three calls where macOS needs one, because Linux hands out the three properties separately.
/// `SCHED_IDLE` runs only when nothing in `SCHED_OTHER` is runnable, which is what protects the
/// person; the nice value is what a scheduler that ignores the policy still honours; and the I/O
/// class is the disk, which a nice value does not reach at all under `none` or `mq-deadline` and
/// only shades a weight under BFQ.
#[cfg(target_os = "linux")]
fn lower() {
    // uapi/linux/sched.h. `libc` 0.2.189 carries it for android and l4re only.
    const SCHED_IDLE: libc::c_int = 5;
    // uapi/linux/ioprio.h; `libc` carries none of the three, only the syscall number.
    const IOPRIO_WHO_PROCESS: libc::c_long = 1;
    const IOPRIO_CLASS_IDLE: libc::c_long = 3;
    const IOPRIO_CLASS_SHIFT: libc::c_long = 13;

    if unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, 19) } != 0 {
        warn("setpriority(PRIO_PROCESS, 19)");
    }
    // Zeroed rather than named: `SCHED_IDLE` requires a priority of 0 on every libc, and the
    // struct has fields beyond `sched_priority` on some of them.
    let idle: libc::sched_param = unsafe { std::mem::zeroed() };
    if unsafe { libc::sched_setscheduler(0, SCHED_IDLE, &idle) } != 0 {
        warn("sched_setscheduler(SCHED_IDLE)");
    }
    let class = IOPRIO_CLASS_IDLE << IOPRIO_CLASS_SHIFT;
    let set = unsafe { libc::syscall(libc::SYS_ioprio_set as libc::c_long, IOPRIO_WHO_PROCESS, 0 as libc::c_long, class) };
    if set != 0 {
        warn("ioprio_set(IOPRIO_CLASS_IDLE)");
    }
}

/// Said out loud rather than passed over in silence: someone who wrote `priority = "background"`
/// should read that it did nothing here instead of inferring it from the fan. Windows has
/// `SetPriorityClass(PROCESS_MODE_BACKGROUND_BEGIN)`, which is the same idea and is neither built
/// nor measured in this repository, so it is named in the docs and not claimed in the code.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn lower() {
    eprintln!("priority: background is not implemented on this platform, so this run keeps normal priority");
}

/// A band that could not be entered is a louder rebuild, never a failed one.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn warn(call: &str) {
    eprintln!("priority: {call} failed ({}), so this run keeps normal priority", std::io::Error::last_os_error());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both tests below read and write the one scheduling priority the whole test binary shares,
    /// and cargo runs tests on threads of a single process: without this they race and the one
    /// asserting nothing moved reads the other's lowering.
    static PROCESS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn the_run_variable_outranks_the_file_and_an_empty_one_keeps_it() {
        assert_eq!(from_env(Priority::Background, None).unwrap(), Priority::Background);
        assert_eq!(from_env(Priority::Background, Some("normal")).unwrap(), Priority::Normal);
        assert_eq!(from_env(Priority::Normal, Some("")).unwrap(), Priority::Normal);
        assert_eq!(from_env(Priority::Normal, Some(" BACKGROUND ")).unwrap(), Priority::Background);
    }

    #[test]
    fn a_priority_the_run_variable_misspells_is_an_error_naming_it() {
        let err = from_env(Priority::Background, Some("fast")).unwrap_err().to_string();
        assert!(err.contains("REPOGRAPH_PRIORITY"), "{err}");
        assert!(err.contains("fast"), "{err}");
    }

    #[test]
    fn applying_normal_touches_nothing() {
        let _g = PROCESS.lock().unwrap_or_else(|e| e.into_inner());
        let before = darwin_background();
        apply(Priority::Normal);
        assert_eq!(darwin_background(), before);
    }

    /// The only automated proof the call reaches the kernel; everything else about the band is a
    /// measurement (docs/plans/2026-09-07-unnoticeable.md §5). It restores the process because
    /// every other test in this binary would otherwise finish on the efficiency cluster.
    #[test]
    #[cfg(target_os = "macos")]
    fn lowering_the_process_is_visible_to_the_kernel_and_undone_after_the_test() {
        struct Restore;
        impl Drop for Restore {
            fn drop(&mut self) {
                unsafe { libc::setpriority(libc::PRIO_DARWIN_PROCESS, 0, 0) };
            }
        }
        let _g = PROCESS.lock().unwrap_or_else(|e| e.into_inner());
        let _restore = Restore;
        assert_eq!(darwin_background(), Some(0));
        apply(Priority::Background);
        assert_eq!(darwin_background(), Some(1));
    }

    /// `getpriority` on the Darwin background policy, or `None` where there is no such policy.
    fn darwin_background() -> Option<i32> {
        #[cfg(target_os = "macos")]
        {
            Some(unsafe { libc::getpriority(libc::PRIO_DARWIN_PROCESS, 0) })
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }
}
