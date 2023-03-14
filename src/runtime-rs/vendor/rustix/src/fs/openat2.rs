use crate::io::{self, OwnedFd};
use crate::{imp, path};
use imp::fd::AsFd;
use imp::fs::{Mode, OFlags, ResolveFlags};

/// `openat2(dirfd, path, OpenHow { oflags, mode, resolve }, sizeof(OpenHow))`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/openat2.2.html
#[inline]
pub fn openat2<Fd: AsFd, P: path::Arg>(
    dirfd: Fd,
    path: P,
    oflags: OFlags,
    mode: Mode,
    resolve: ResolveFlags,
) -> io::Result<OwnedFd> {
    path.into_with_z_str(|path| {
        imp::fs::syscalls::openat2(dirfd.as_fd(), path, oflags, mode, resolve)
    })
}
