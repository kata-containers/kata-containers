//! The `msync` function.
//!
//! # Safety
//!
//! `msync` operates on a raw pointer. Some forms of `msync` may
//! mutate the memory or have other side effects.
#![allow(unsafe_code)]

use crate::{imp, io};
use core::ffi::c_void;

pub use imp::io::MsyncFlags;

/// `msync(addr, len, flags)`—Declares an expected access pattern
/// for a memory-mapped file.
///
/// # Safety
///
/// `addr` must be a valid pointer to memory that is appropriate to
/// call `msync` on. Some forms of `msync` may mutate the memory
/// or evoke a variety of side-effects on the mapping and/or the file.
///
/// # References
///  - [POSIX]
///  - [Linux `msync`]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/msync.html
/// [Linux `msync`]: https://man7.org/linux/man-pages/man2/msync.2.html
#[inline]
pub unsafe fn msync(addr: *mut c_void, len: usize, flags: MsyncFlags) -> io::Result<()> {
    imp::io::syscalls::msync(addr, len, flags)
}
