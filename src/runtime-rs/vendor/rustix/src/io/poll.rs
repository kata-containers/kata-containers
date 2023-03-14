use crate::{imp, io};

pub use imp::io::{PollFd, PollFlags};

/// `poll(self.fds, timeout)`
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/poll.html
/// [Linux]: https://man7.org/linux/man-pages/man2/poll.2.html
#[inline]
pub fn poll(fds: &mut [PollFd<'_>], timeout: i32) -> io::Result<usize> {
    imp::io::syscalls::poll(fds, timeout)
}
