//! Functions which operate on file descriptors which might be terminals.

use crate::imp;
#[cfg(any(
    all(linux_raw, feature = "procfs"),
    all(libc, not(any(target_os = "fuchsia", target_os = "wasi")))
))]
#[cfg_attr(doc_cfg, doc(cfg(feature = "procfs")))]
use crate::io;
use imp::fd::AsFd;
#[cfg(any(
    all(linux_raw, feature = "procfs"),
    all(libc, not(any(target_os = "fuchsia", target_os = "wasi")))
))]
use {
    crate::ffi::ZString, crate::path::SMALL_PATH_BUFFER_SIZE, alloc::vec::Vec, imp::fd::BorrowedFd,
};

#[cfg(not(target_os = "wasi"))]
pub use imp::io::Tcflag;

/// `ICANON`
#[cfg(not(any(windows, target_os = "wasi")))]
pub const ICANON: Tcflag = imp::io::ICANON;

/// `isatty(fd)`—Tests whether a file descriptor refers to a terminal.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/isatty.html
/// [Linux]: https://man7.org/linux/man-pages/man3/isatty.3.html
#[inline]
pub fn isatty<Fd: AsFd>(fd: Fd) -> bool {
    imp::io::syscalls::isatty(fd.as_fd())
}

/// `ttyname_r(fd)`
///
/// If `reuse` is non-empty, reuse its buffer to store the result if possible.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/ttyname.html
/// [Linux]: https://man7.org/linux/man-pages/man3/ttyname.3.html
#[cfg(any(
    all(linux_raw, feature = "procfs"),
    all(libc, not(any(target_os = "fuchsia", target_os = "wasi")))
))]
#[cfg_attr(doc_cfg, doc(cfg(feature = "procfs")))]
#[inline]
pub fn ttyname<Fd: AsFd, B: Into<Vec<u8>>>(dirfd: Fd, reuse: B) -> io::Result<ZString> {
    _ttyname(dirfd.as_fd(), reuse.into())
}

#[cfg(any(
    all(linux_raw, feature = "procfs"),
    all(libc, not(any(target_os = "fuchsia", target_os = "wasi")))
))]
fn _ttyname(dirfd: BorrowedFd<'_>, mut buffer: Vec<u8>) -> io::Result<ZString> {
    // This code would benefit from having a better way to read into
    // uninitialized memory, but that requires `unsafe`.
    buffer.clear();
    buffer.reserve(SMALL_PATH_BUFFER_SIZE);
    buffer.resize(buffer.capacity(), 0_u8);

    loop {
        match imp::io::syscalls::ttyname(dirfd, &mut buffer) {
            Err(imp::io::Error::RANGE) => {
                buffer.reserve(1); // use `Vec` reallocation strategy to grow capacity exponentially
                buffer.resize(buffer.capacity(), 0_u8);
            }
            Ok(len) => {
                buffer.resize(len, 0_u8);
                return Ok(ZString::new(buffer).unwrap());
            }
            Err(errno) => return Err(errno),
        }
    }
}
