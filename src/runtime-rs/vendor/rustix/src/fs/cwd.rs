use crate::imp;
use imp::fd::{BorrowedFd, RawFd};

/// `AT_FDCWD`—Returns a handle representing the current working directory.
///
/// This returns a file descriptor which refers to the process current
/// directory which can be used as the directory argument in `*at`
/// functions such as [`openat`].
///
/// # References
///  - [POSIX]
///
/// [`openat`]: crate::fs::openat
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/fcntl.h.html
#[inline]
#[doc(alias = "AT_FDCWD")]
pub fn cwd() -> BorrowedFd<'static> {
    let at_fdcwd = imp::io::AT_FDCWD as RawFd;

    // # Safety
    //
    // `AT_FDCWD` is a reserved value that is never dynamically allocated, so
    // it'll remain valid for the duration of `'static`.
    #[allow(unsafe_code)]
    unsafe {
        BorrowedFd::<'static>::borrow_raw(at_fdcwd)
    }
}
