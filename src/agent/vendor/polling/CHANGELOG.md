# Version 2.5.2

- Update use of `libc::timespec` to prepare for future libc version (#55)
- Update use of `libc::kevent` to prepare for future libc version (#56)
- Add error message for Wepoll (#54)

# Version 2.5.1

- Fix the build error with MSRV on Windows

# Version 2.5.0

- Switch from `winapi` to `windows-sys` (#47)

# Version 2.4.0

- Fix the build error on illumos and Solaris (#43)
- Bump MSRV to 1.47 (#40)
- Optimize `Poller` internal representation (#40)

# Version 2.3.0

- Implement `AsRawFd` for `Poller` on most Unix systems (#39)
- Implement `AsRawHandle` for `Poller` on Windows (#39)
- Implement I/O safety traits on Rust 1.63+ (#39)

# Version 2.2.0

- Support VxWorks, Fuchsia and other Unix systems by using poll. (#26)

# Version 2.1.0

- Switch from `wepoll-sys` to `wepoll-ffi`.

# Version 2.0.3

- Update `cfg-if` dependency to 1.

# Version 2.0.2

- Replace manual pointer conversion with `as_ptr()` and `as_mut_ptr()`.

# Version 2.0.1

- Minor docs improvements.

# Version 2.0.0

- Add `Event` argument to `Poller::insert()`.
- Don't put fd/socket in non-blocking mode upon insertion.
- Rename `insert()`/`interest()`/`remove()` to `add()`/`modify()`/`delete()`.
- Replace `wepoll-sys-stjepang` with an `wepoll-sys`.

# Version 1.1.0

- Add "std" cargo feature.

# Version 1.0.3

- Remove `libc` dependency on Windows.

# Version 1.0.2

- Bump MSRV to 1.40.0
- Replace the `epoll_create1` hack with a cleaner solution.
- Pass timeout to `epoll_wait` to support systems without `timerfd`.

# Version 1.0.1

- Fix a typo in the readme.

# Version 1.0.0

- Stabilize.

# Version 0.1.9

- Fix compilation on x86_64-unknown-linux-gnux32

# Version 0.1.8

- Replace `log::debug!` with `log::trace!`.

# Version 0.1.7

- Specify oneshot mode in epoll/wepoll at insert.

# Version 0.1.6

- Add logging.

# Version 0.1.5

- Fix a bug where epoll would block when the timeout is set to zero.
- More tests.

# Version 0.1.4

- Optimize notifications.
- Fix a bug in timeouts on Windows where it would trigger too early.
- Support sub-nanosecond precision on Linux/Android.

# Version 0.1.3

- Improve error handling around event ports fcntl

# Version 0.1.2

- Add support for event ports (illumos and Solaris)

# Version 0.1.1

- Improve documentation
- Fix a bug in `Event::none()`.

# Version 0.1.0

- Initial version
