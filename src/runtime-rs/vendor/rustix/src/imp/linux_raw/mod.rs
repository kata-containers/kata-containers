//! The linux_raw backend.
//!
//! This makes Linux syscalls directly, without going through libc.

mod arch;
mod conv;
mod reg;
mod vdso;
mod vdso_wrappers;

pub(crate) mod elf;
pub(crate) mod fs;
pub(crate) mod io;
#[cfg(feature = "io_uring")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "io_uring")))]
pub(crate) mod io_uring;
pub(crate) mod net;
pub(crate) mod process;
pub(crate) mod rand;
pub(crate) mod syscalls;
pub(crate) mod thread;
pub(crate) mod time;

#[cfg(feature = "std")]
pub(crate) mod fd {
    pub use io_lifetimes::*;

    #[allow(unused_imports)]
    pub use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

    #[allow(unused_imports)]
    pub(crate) use std::os::unix::io::RawFd as LibcFd;
}

#[cfg(not(feature = "std"))]
pub(crate) use crate::io::fd;

// The linux_raw backend doesn't use actual libc, so we define selected
// libc-like definitions in a module called `c`.
pub(crate) mod c;
