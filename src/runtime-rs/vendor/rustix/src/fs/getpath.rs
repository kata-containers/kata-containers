use crate::ffi::ZString;
use crate::{imp, io};
use imp::fd::AsFd;

/// `fcntl(fd, F_GETPATH)`
///
/// # References
///  - [Apple]
///
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fcntl.2.html
#[inline]
pub fn getpath<Fd: AsFd>(fd: Fd) -> io::Result<ZString> {
    imp::fs::syscalls::getpath(fd.as_fd())
}
