extern crate nix;

use nix::fcntl::OFlag;
use nix::pty::*;
use nix::unistd::close;
use std::os::unix::io::AsRawFd;

/// Regression test for Issue #659
/// `PtyMaster` should panic rather than double close the file descriptor
/// This must run in its own test process because it deliberately creates a race
/// condition.
#[test]
#[should_panic(expected = "Closing an invalid file descriptor!")]
// In Travis on i686-unknown-linux-musl, this test gets SIGABRT.  I don't know
// why.  It doesn't happen on any other target, and it doesn't happen on my PC.
#[cfg_attr(all(target_env = "musl", target_arch = "x86"), ignore)]
fn test_double_close() {
    let m = posix_openpt(OFlag::O_RDWR).unwrap();
    close(m.as_raw_fd()).unwrap();
    drop(m);            // should panic here
}
