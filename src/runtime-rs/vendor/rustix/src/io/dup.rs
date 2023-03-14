//! Functions which duplicate file descriptors.

use crate::imp;
use crate::io::{self, OwnedFd};
use imp::fd::AsFd;

#[cfg(not(target_os = "wasi"))]
pub use imp::io::DupFlags;

/// `dup(fd)`—Creates a new `OwnedFd` instance that shares the same
/// underlying [file description] as `fd`.
///
/// Note that this function does not set the `O_CLOEXEC` flag. To do a `dup`
/// that does set `O_CLOEXEC`, use [`fcntl_dupfd_cloexec`].
///
/// POSIX guarantees that `dup` will use the lowest unused file descriptor,
/// however it is not safe in general to rely on this, as file descriptors may
/// be unexpectedly allocated on other threads or in libraries.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [file description]: https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap03.html#tag_03_258
/// [`fcntl_dupfd_cloexec`]: crate::fs::fcntl_dupfd_cloexec
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/dup.html
/// [Linux]: https://man7.org/linux/man-pages/man2/dup.2.html
#[cfg(not(target_os = "wasi"))]
#[inline]
pub fn dup<Fd: AsFd>(fd: Fd) -> io::Result<OwnedFd> {
    imp::io::syscalls::dup(fd.as_fd())
}

/// `dup2(fd, new)`—Creates a new `OwnedFd` instance that shares the
/// same underlying [file description] as the existing `OwnedFd` instance,
/// closing `new` and reusing its file descriptor.
///
/// Note that this function does not set the `O_CLOEXEC` flag. To do a `dup2`
/// that does set `O_CLOEXEC`, use [`dup3`] with [`DupFlags::CLOEXEC`] on
/// platforms which support it.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [file description]: https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap03.html#tag_03_258
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/dup2.html
/// [Linux]: https://man7.org/linux/man-pages/man2/dup2.2.html
#[cfg(not(target_os = "wasi"))]
#[inline]
pub fn dup2<Fd: AsFd>(fd: Fd, new: &OwnedFd) -> io::Result<()> {
    imp::io::syscalls::dup2(fd.as_fd(), new)
}

/// `dup3(fd, new, flags)`—Creates a new `OwnedFd` instance that shares the
/// same underlying [file description] as the existing `OwnedFd` instance,
/// closing `new` and reusing its file descriptor, with flags.
///
/// `dup3` is the same as [`dup2`] but adds an additional flags operand,
/// and it fails in the case that `fd` and `new` have the same file descriptor
/// value. This additional difference is the reason this function isn't named
/// `dup2_with`.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [file description]: https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap03.html#tag_03_258
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/dup2.html
/// [Linux]: https://man7.org/linux/man-pages/man2/dup2.2.html
#[cfg(not(target_os = "wasi"))]
#[inline]
pub fn dup3<Fd: AsFd>(fd: Fd, new: &OwnedFd, flags: DupFlags) -> io::Result<()> {
    imp::io::syscalls::dup3(fd.as_fd(), new, flags)
}
