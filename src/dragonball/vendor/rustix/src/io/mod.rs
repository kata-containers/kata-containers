//! I/O operations.

mod close;
#[cfg(not(windows))]
mod dup;
mod error;
#[cfg(any(target_os = "android", target_os = "linux"))]
mod eventfd;
#[cfg(not(feature = "std"))]
pub(crate) mod fd;
mod ioctl;
#[cfg(not(any(windows, target_os = "redox")))]
mod is_read_write;
#[cfg(not(any(windows, target_os = "redox", target_os = "wasi")))]
mod madvise;
#[cfg(not(any(windows, target_os = "wasi")))]
mod mmap;
#[cfg(not(any(windows, target_os = "wasi")))]
mod msync;
mod owned_fd;
#[cfg(not(any(windows, target_os = "wasi")))]
mod pipe;
mod poll;
#[cfg(all(feature = "procfs", any(target_os = "android", target_os = "linux")))]
mod procfs;
#[cfg(not(windows))]
mod read_write;
#[cfg(not(feature = "std"))]
mod seek_from;
#[cfg(not(windows))]
mod stdio;
#[cfg(not(windows))]
mod tty;
#[cfg(any(target_os = "android", target_os = "linux"))]
mod userfaultfd;

#[cfg(any(target_os = "android", target_os = "linux"))]
pub use crate::imp::io::epoll;
pub use close::close;
#[cfg(not(any(windows, target_os = "wasi")))]
pub use dup::{dup, dup2, dup3, DupFlags};
pub use error::{with_retrying, Error, Result};
#[cfg(any(target_os = "android", target_os = "linux"))]
pub use eventfd::{eventfd, EventfdFlags};
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use ioctl::ioctl_fioclex;
pub use ioctl::ioctl_fionbio;
#[cfg(not(any(windows, target_os = "redox")))]
pub use ioctl::ioctl_fionread;
#[cfg(any(target_os = "android", target_os = "linux"))]
pub use ioctl::{ioctl_blkpbszget, ioctl_blksszget};
#[cfg(not(any(windows, target_os = "wasi")))]
pub use ioctl::{ioctl_tcgets, ioctl_tiocgwinsz, Termios, Winsize};
#[cfg(not(any(windows, target_os = "redox", target_os = "wasi")))]
pub use ioctl::{ioctl_tiocexcl, ioctl_tiocnxcl};
#[cfg(not(any(windows, target_os = "redox")))]
pub use is_read_write::is_read_write;
#[cfg(not(any(windows, target_os = "redox", target_os = "wasi")))]
pub use madvise::{madvise, Advice};
#[cfg(not(any(windows, target_os = "wasi")))]
pub use mmap::{
    mlock, mmap, mmap_anonymous, mprotect, munlock, munmap, MapFlags, MprotectFlags, ProtFlags,
};
#[cfg(any(target_os = "android", target_os = "linux"))]
pub use mmap::{mlock_with, MlockFlags};
#[cfg(any(linux_raw, all(libc, target_os = "linux")))]
pub use mmap::{mremap, mremap_fixed, MremapFlags};
#[cfg(not(any(windows, target_os = "wasi")))]
pub use msync::{msync, MsyncFlags};
pub use owned_fd::OwnedFd;
#[cfg(not(any(windows, target_os = "wasi")))]
pub use pipe::pipe;
#[cfg(not(any(
    windows,
    target_os = "illumos",
    target_os = "redox",
    target_os = "wasi"
)))]
pub use pipe::PIPE_BUF;
#[cfg(not(any(windows, target_os = "ios", target_os = "macos", target_os = "wasi")))]
pub use pipe::{pipe_with, PipeFlags};
pub use poll::{poll, PollFd, PollFlags};
#[cfg(all(feature = "procfs", any(target_os = "android", target_os = "linux")))]
pub use procfs::{proc_self_fd, proc_self_fdinfo_fd, proc_self_maps, proc_self_pagemap};
#[cfg(not(windows))]
pub use read_write::{pread, pwrite, read, readv, write, writev, IoSlice, IoSliceMut};
#[cfg(not(any(windows, target_os = "redox")))]
pub use read_write::{preadv, pwritev};
#[cfg(any(target_os = "android", target_os = "linux"))]
pub use read_write::{preadv2, pwritev2, ReadWriteFlags};
#[cfg(not(windows))]
pub use stdio::{stderr, stdin, stdout, take_stderr, take_stdin, take_stdout};
#[cfg(not(windows))]
pub use tty::isatty;
#[cfg(any(
    all(linux_raw, feature = "procfs"),
    all(libc, not(any(windows, target_os = "fuchsia", target_os = "wasi")))
))]
pub use tty::ttyname;
#[cfg(not(windows))]
#[cfg(not(target_os = "wasi"))]
pub use tty::{Tcflag, ICANON};
#[cfg(any(target_os = "android", target_os = "linux"))]
pub use userfaultfd::{userfaultfd, UserfaultfdFlags};

// Declare `SeekFrom`.
#[cfg(not(feature = "std"))]
pub use seek_from::SeekFrom;
#[cfg(feature = "std")]
pub use std::io::SeekFrom;
