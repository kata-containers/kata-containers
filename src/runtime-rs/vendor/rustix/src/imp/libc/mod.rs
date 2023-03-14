//! The libc backend.
//!
//! On most platforms, this uses the `libc` crate to make system calls. On
//! Windows, this uses the Winsock2 API in `winapi`, which can be adapted
//! to have a very `libc`-like interface.

#[cfg(not(any(windows, target_os = "wasi")))]
#[macro_use]
mod weak;

mod conv;
mod offset;

#[cfg(windows)]
mod io_lifetimes;
#[cfg(not(windows))]
#[cfg(not(feature = "std"))]
pub(crate) mod fd {
    pub(crate) use super::c::c_int as LibcFd;
    pub use crate::io::fd::*;
}
#[cfg(windows)]
pub(crate) mod fd {
    pub use super::io_lifetimes::*;
}
#[cfg(not(windows))]
#[cfg(feature = "std")]
pub(crate) mod fd {
    pub use io_lifetimes::*;

    #[allow(unused_imports)]
    #[cfg(target_os = "wasi")]
    pub(crate) use super::c::c_int as LibcFd;
    #[allow(unused_imports)]
    #[cfg(unix)]
    pub(crate) use std::os::unix::io::RawFd as LibcFd;
    #[allow(unused_imports)]
    #[cfg(unix)]
    pub use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
    #[allow(unused_imports)]
    #[cfg(target_os = "wasi")]
    pub use std::os::wasi::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
}

// On Windows we emulate selected libc-compatible interfaces. On non-Windows,
// we just use libc here, since this is the libc backend.
#[cfg(windows)]
pub(crate) mod c;
#[cfg(not(windows))]
pub(crate) use libc as c;

#[cfg(not(windows))]
pub(crate) mod fs;
pub(crate) mod io;
#[cfg(any(target_os = "android", target_os = "linux"))]
#[cfg(feature = "io_uring")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "io_uring")))]
pub(crate) mod io_uring;
#[cfg(not(any(target_os = "redox", target_os = "wasi")))] // WASI doesn't support `net` yet.
pub(crate) mod net;
#[cfg(not(windows))]
pub(crate) mod process;
#[cfg(not(windows))]
pub(crate) mod rand;
#[cfg(not(windows))]
pub(crate) mod thread;
#[cfg(not(windows))]
pub(crate) mod time;

/// If the host libc is glibc, return `true` if it is less than version 2.25.
///
/// To restate and clarify, this function returning true does not mean the libc
/// is glibc just that if it is glibc, it is less than version 2.25.
///
/// For now, this function is only available on Linux, but if it ends up being
/// used beyond that, this could be changed to e.g. `#[cfg(unix)]`.
#[cfg(all(unix, target_env = "gnu"))]
pub(crate) fn if_glibc_is_less_than_2_25() -> bool {
    // This is also defined inside `weak_or_syscall!` in
    // imp/libc/rand/syscalls.rs, but it's not convenient to re-export the weak
    // symbol from that macro, so we duplicate it at a small cost here.
    weak! { fn getrandom(*mut c::c_void, c::size_t, c::c_uint) -> c::ssize_t }

    // glibc 2.25 has `getrandom`, which is how we satisfy the API contract of
    // this function. But, there are likely other libc versions which have it.
    getrandom.get().is_none()
}
