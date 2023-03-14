mod auxv;
mod types;

use super::c;

#[cfg(not(windows))]
pub(crate) mod syscalls;
#[cfg(not(target_os = "wasi"))]
pub(crate) use auxv::clock_ticks_per_second;
pub(crate) use auxv::page_size;
#[cfg(any(
    all(target_os = "android", target_pointer_width = "64"),
    target_os = "linux"
))]
pub(crate) use auxv::{linux_execfn, linux_hwcap};
#[cfg(not(target_os = "wasi"))]
pub(crate) use c::{
    WCONTINUED, WEXITSTATUS, WIFCONTINUED, WIFEXITED, WIFSIGNALED, WIFSTOPPED, WNOHANG, WSTOPSIG,
    WTERMSIG, WUNTRACED,
};
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "fuchsia",
    target_os = "dragonfly"
))]
pub(crate) mod cpu_set;
#[cfg(not(target_os = "wasi"))]
pub(crate) use types::RawUname;
#[cfg(not(any(target_os = "fuchsia", target_os = "redox", target_os = "wasi")))]
pub use types::Resource;
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "fuchsia",
    target_os = "dragonfly"
))]
pub(crate) use types::{raw_cpu_set_new, RawCpuSet, CPU_SETSIZE};
#[cfg(any(target_os = "android", target_os = "linux"))]
pub use types::{MembarrierCommand, RawCpuid};
#[cfg(not(target_os = "wasi"))]
pub use types::{RawGid, RawNonZeroPid, RawPid, RawUid, Signal, EXIT_SIGNALED_SIGABRT};
pub use types::{EXIT_FAILURE, EXIT_SUCCESS};
