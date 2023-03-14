//! The unsafe `close` for raw file descriptors.
//!
//! # Safety
//!
//! Operating on raw file descriptors is unsafe.
#![allow(unsafe_code)]

use crate::imp;
use imp::fd::RawFd;

/// `close(raw_fd)`—Closes a `RawFd` directly.
///
/// Most users won't need to use this, as `OwnedFd` automatically closes its
/// file descriptor on `Drop`.
///
/// This function does not return a `Result`, as it is the [responsibility]
/// of filesystem designers to not return errors from `close`. Users who chose
/// to use NFS or similar filesystems should take care to monitor for problems
/// externally.
///
/// [responsibility]: https://lwn.net/Articles/576518/
///
/// # References
///
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/close.html
/// [Linux]: https://man7.org/linux/man-pages/man2/close.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/close.2.html#//apple_ref/doc/man/2/close
///
/// # Safety
///
/// This function takes a `RawFd`, which must be valid before the call, and is
/// not valid after the call.
#[inline]
pub unsafe fn close(raw_fd: RawFd) {
    imp::io::syscalls::close(raw_fd)
}
