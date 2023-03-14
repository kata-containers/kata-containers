//! Process-associated operations.

mod auxv;
#[cfg(not(target_os = "wasi"))]
mod chdir;
mod exit;
#[cfg(not(target_os = "wasi"))] // WASI doesn't have get[gpu]id.
mod id;
#[cfg(not(target_os = "wasi"))]
mod kill;
#[cfg(any(target_os = "android", target_os = "linux"))]
mod membarrier;
#[cfg(not(any(target_os = "fuchsia", target_os = "wasi")))] // WASI doesn't have [gs]etpriority.
mod priority;
#[cfg(not(any(target_os = "fuchsia", target_os = "redox", target_os = "wasi")))]
mod rlimit;
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "fuchsia",
    target_os = "dragonfly"
))]
mod sched;
mod sched_yield;
#[cfg(not(target_os = "wasi"))] // WASI doesn't have uname.
mod uname;
#[cfg(not(target_os = "wasi"))]
mod wait;

#[cfg(not(target_os = "wasi"))]
pub use auxv::clock_ticks_per_second;
#[cfg(target_vendor = "mustang")]
pub use auxv::init;
pub use auxv::page_size;
#[cfg(any(
    linux_raw,
    all(
        libc,
        any(
            all(target_os = "android", target_pointer_width = "64"),
            target_os = "linux"
        )
    )
))]
pub use auxv::{linux_execfn, linux_hwcap};
#[cfg(not(target_os = "wasi"))]
pub use chdir::chdir;
#[cfg(not(any(target_os = "wasi", target_os = "fuchsia")))]
pub use chdir::fchdir;
#[cfg(not(target_os = "wasi"))]
pub use chdir::getcwd;
#[cfg(not(target_os = "wasi"))]
pub use exit::EXIT_SIGNALED_SIGABRT;
pub use exit::{EXIT_FAILURE, EXIT_SUCCESS};
#[cfg(any(target_os = "android", target_os = "linux"))]
pub use id::Cpuid;
#[cfg(not(target_os = "wasi"))]
pub use id::{
    getegid, geteuid, getgid, getpid, getppid, getuid, setsid, Gid, Pid, RawGid, RawNonZeroPid,
    RawPid, RawUid, Uid,
};
#[cfg(not(target_os = "wasi"))]
pub use kill::{kill_current_process_group, kill_process, kill_process_group, Signal};
#[cfg(any(target_os = "android", target_os = "linux"))]
pub use membarrier::{
    membarrier, membarrier_cpu, membarrier_query, MembarrierCommand, MembarrierQuery,
};
#[cfg(not(any(target_os = "fuchsia", target_os = "wasi")))]
pub use priority::nice;
#[cfg(not(any(target_os = "fuchsia", target_os = "redox", target_os = "wasi")))]
pub use priority::{
    getpriority_pgrp, getpriority_process, getpriority_user, setpriority_pgrp, setpriority_process,
    setpriority_user,
};
#[cfg(any(target_os = "android", target_os = "linux"))]
pub use rlimit::prlimit;
#[cfg(not(any(target_os = "fuchsia", target_os = "redox", target_os = "wasi")))]
pub use rlimit::{getrlimit, setrlimit, Resource, Rlimit};
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "fuchsia",
    target_os = "dragonfly"
))]
pub use sched::{sched_getaffinity, sched_setaffinity, CpuSet};
pub use sched_yield::sched_yield;
#[cfg(not(target_os = "wasi"))]
pub use uname::{uname, Uname};
#[cfg(not(target_os = "wasi"))]
pub use wait::{wait, waitpid, WaitOptions, WaitStatus};
