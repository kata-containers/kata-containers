use nix::poll::{PollFlags, poll, PollFd};
use nix::unistd::{write, pipe};

#[test]
fn test_poll() {
    let (r, w) = pipe().unwrap();
    let mut fds = [PollFd::new(r, PollFlags::POLLIN)];

    // Poll an idle pipe.  Should timeout
    let nfds = poll(&mut fds, 100).unwrap();
    assert_eq!(nfds, 0);
    assert!(!fds[0].revents().unwrap().contains(PollFlags::POLLIN));

    write(w, b".").unwrap();

    // Poll a readable pipe.  Should return an event.
    let nfds = poll(&mut fds, 100).unwrap();
    assert_eq!(nfds, 1);
    assert!(fds[0].revents().unwrap().contains(PollFlags::POLLIN));
}

// ppoll(2) is the same as poll except for how it handles timeouts and signals.
// Repeating the test for poll(2) should be sufficient to check that our
// bindings are correct.
#[cfg(any(target_os = "android",
          target_os = "dragonfly",
          target_os = "freebsd",
          target_os = "linux"))]
#[test]
fn test_ppoll() {
    use nix::poll::ppoll;
    use nix::sys::signal::SigSet;
    use nix::sys::time::{TimeSpec, TimeValLike};

    let timeout = TimeSpec::milliseconds(1);
    let (r, w) = pipe().unwrap();
    let mut fds = [PollFd::new(r, PollFlags::POLLIN)];

    // Poll an idle pipe.  Should timeout
    let nfds = ppoll(&mut fds, timeout, SigSet::empty()).unwrap();
    assert_eq!(nfds, 0);
    assert!(!fds[0].revents().unwrap().contains(PollFlags::POLLIN));

    write(w, b".").unwrap();

    // Poll a readable pipe.  Should return an event.
    let nfds = ppoll(&mut fds, timeout, SigSet::empty()).unwrap();
    assert_eq!(nfds, 1);
    assert!(fds[0].revents().unwrap().contains(PollFlags::POLLIN));
}
