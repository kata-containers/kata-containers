mod error;
#[cfg(not(windows))]
#[cfg(not(feature = "std"))]
mod io_slice;
mod poll_fd;
#[cfg(not(windows))]
mod types;

#[cfg_attr(windows, path = "windows_syscalls.rs")]
pub(crate) mod syscalls;

#[cfg(not(windows))]
#[cfg(not(feature = "std"))]
pub use io_slice::{IoSlice, IoSliceMut};
#[cfg(any(target_os = "android", target_os = "linux"))]
pub mod epoll;
pub use error::Error;
pub use poll_fd::{PollFd, PollFlags};
#[cfg(not(any(windows, target_os = "redox", target_os = "wasi")))]
pub use types::Advice;
#[cfg(target_os = "linux")]
pub use types::MremapFlags;
#[cfg(all(
    libc,
    not(any(windows, target_os = "ios", target_os = "macos", target_os = "wasi"))
))]
pub use types::PipeFlags;
#[cfg(not(any(
    windows,
    target_os = "illumos",
    target_os = "redox",
    target_os = "wasi"
)))]
pub use types::PIPE_BUF;
#[cfg(not(any(windows, target_os = "wasi")))]
pub use types::{
    DupFlags, MapFlags, MprotectFlags, MsyncFlags, ProtFlags, Tcflag, Termios, Winsize, ICANON,
};
#[cfg(any(target_os = "android", target_os = "linux"))]
pub use types::{EventfdFlags, MlockFlags, ReadWriteFlags, UserfaultfdFlags};

use super::c;

#[cfg(not(any(windows, target_os = "redox")))]
pub(crate) const AT_FDCWD: c::c_int = c::AT_FDCWD;
#[cfg(not(windows))]
pub(crate) const STDIN_FILENO: c::c_int = c::STDIN_FILENO;
#[cfg(not(windows))]
pub(crate) const STDOUT_FILENO: c::c_int = c::STDOUT_FILENO;
#[cfg(not(windows))]
pub(crate) const STDERR_FILENO: c::c_int = c::STDERR_FILENO;
