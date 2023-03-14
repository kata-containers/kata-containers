use crate::io::{self, OwnedFd};
use crate::{imp, path};

pub use imp::fs::MemfdFlags;

/// `memfd_create(path, flags)`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/memfd_create.2.html
#[inline]
pub fn memfd_create<P: path::Arg>(path: P, flags: MemfdFlags) -> io::Result<OwnedFd> {
    path.into_with_z_str(|path| imp::fs::syscalls::memfd_create(path, flags))
}
