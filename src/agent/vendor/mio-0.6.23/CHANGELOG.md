# 0.6.23 (Dec 01, 2020)

### Changed
- **MSRV**: Increased the MSRV from 1.18.0 (Jun 8, 2017) to 1.31.0 (Dec 6,
  2018)
  (https://github.com/tokio-rs/mio/commit/4879e0d32ddfd98e762fc87240e594a3ad8fca30).

### Fixed
- Work around Linux kernel < 2.6.37 bug on 32-bits making timeouts longer then
  ~30 minutes effectively infinite
  (https://github.com/tokio-rs/mio/commit/e7cba59950e9c9fa6194e29b5b1e72029e3df455).
- Update miow and net2 depedencies to get rid of invalid memory layout assumption
  (https://github.com/tokio-rs/mio/commit/13f02ac0a86d7c0c0001e5ff8960a0b4340d075c).

# 0.6.22 (May 01, 2020)

### Added
- Add support for illumos target (#1294)

# 0.6.21 (November 27, 2019)

### Fixed
- remove `=` dependency on `cfg-if`.

# 0.6.20 (November 21, 2019)

### Fixed
- Use default IOCP concurrency value (#1161).
- setting FD_CLOEXEC in pipe (#1095).

# 0.6.19 (May 28, 2018)

### Fixed
- Do not trigger HUP events on kqueue platforms (#958).

# 0.6.18 (May 24, 2018)

### Fixed
- Fix compilation on kqueue platforms with 32bit C long (#948).

# 0.6.17 (May 15, 2018)

### Fixed
- Don't report `RDHUP` as `HUP` (#939)
- Fix lazycell related compilation issues.
- Fix EPOLLPRI conflicting with READABLE
- Abort process on ref count overflows

### Added
- Define PRI on all targets

# 0.6.16 (September 5, 2018)

* Add EPOLLPRI readiness to UnixReady on supported platforms (#867)
* Reduce spurious awaken calls (#875)

# 0.6.15 (July 3, 2018)

* Implement `Evented` for containers (#840).
* Fix android-aarch64 build (#850).

# 0.6.14 (March 8, 2018)

* Add `Poll::poll_interruptible` (#811)
* Add `Ready::all` and `usize` conversions (#825)

# 0.6.13 (February 5, 2018)

* Fix build on DragonFlyBSD.
* Add `TcpListener::from_std` that does not require the socket addr.
* Deprecate `TcpListener::from_listener` in favor of from_std.

# 0.6.12 (January 5, 2018)

* Add `TcpStream::peek` function (#773).
* Raise minimum Rust version to 1.18.0.
* `Poll`: retry select() when interrupted by a signal (#742).
* Deprecate `Events` index access (#713).
* Add `Events::clear` (#782).
* Add support for `lio_listio` (#780).

# 0.6.11 (October 25, 2017)

* Allow register to take empty interest (#640).
* Fix bug with TCP errors on windows (#725).
* Add TcpListener::accept_std (#733).
* Update IoVec to fix soundness bug -- includes behavior change. (#747).
* Minimum Rust version is now 1.14.0.
* Fix Android x86_64 build.
* Misc API & doc polish.

# 0.6.10 (July 27, 2017)

* Experimental support for Fuchsia
* Add `only_v6` option for UDP sockets
* Fix build on NetBSD
* Minimum Rust version is now 1.13.0
* Assignment operators (e.g. `|=`) are now implemented for `Ready`

# 0.6.9 (June 7, 2017)

* More socket options are exposed through the TCP types, brought in through the
  `net2` crate.

# 0.6.8 (May 26, 2017)

* Support Fuchia
* POSIX AIO support
* Fix memory leak caused by Register::new2
* Windows: fix handling failed TCP connections
* Fix build on aarch64-linux-android
* Fix usage of `O_CLOEXEC` with `SETFL`

# 0.6.7 (April 27, 2017)

* Ignore EPIPE coming out of `kevent`
* Timer thread should exit when timer is dropped.

# 0.6.6 (March 22, 2017)

* Add send(), recv() and connect() to UDPSocket.
* Fix bug in custom readiness queue
* Move net types into `net` module

# 0.6.5 (March 14, 2017)

* Misc improvements to kqueue bindings
* Add official support for iOS, Android, BSD
* Reimplement custom readiness queue
* `Poll` is now `Sync`
* Officially deprecate non-core functionality (timers, channel, etc...)
* `Registration` now implements `Evented`
* Fix bug around error conditions with `connect` on windows.
* Use iovec crate for scatter / gather operations
* Only support readable and writable readiness on all platforms
* Expose additional readiness in a platform specific capacity

# 0.6.4 (January 24, 2017)

* Fix compilation on musl
* Add `TcpStream::from_stream` which converts a std TCP stream to Mio.

# 0.6.3 (January 22, 2017)

* Implement readv/writev for `TcpStream`, allowing vectored reads/writes to
  work across platforms
* Remove `nix` dependency
* Implement `Display` and `Error` for some channel error types.
* Optimize TCP on Windows through `SetFileCompletionNotificationModes`

# 0.6.2 (December 18, 2016)

* Allow registration of custom handles on Windows (like `EventedFd` on Unix)
* Send only one byte for the awakener on Unix instead of four
* Fix a bug in the timer implementation which caused an infinite loop

# 0.6.1 (October 30, 2016)

* Update dependency of `libc` to 0.2.16
* Fix channel `dec` logic
* Fix a timer bug around timeout cancellation
* Don't allocate buffers for TCP reads on Windows
* Touched up documentation in a few places
* Fix an infinite looping timer thread on OSX
* Fix compile on 32-bit OSX
* Fix compile on FreeBSD

# 0.6.0 (September 2, 2016)

* Shift primary API towards `Poll`
* `EventLoop` and types to `deprecated` mod. All contents of the
  `deprecated` mod will be removed by Mio 1.0.
* Increase minimum supported Rust version to 1.9.0
* Deprecate unix domain socket implementation in favor of using a
  version external to Mio. For example: https://github.com/alexcrichton/mio-uds.
* Remove various types now included in `std`
* Updated TCP & UDP APIs to match the versions in `std`
* Enable implementing `Evented` for any type via `Registration`
* Rename `IoEvent` -> `Event`
* Access `Event` data via functions vs. public fields.
* Expose `Events` as a public type that is passed into `Poll`
* Use `std::time::Duration` for all APIs that require a time duration.
* Polled events are now retrieved via `Events` type.
* Implement `std::error::Error` for `TimerError`
* Relax `Send` bound on notify messages.
* Remove `Clone` impl for `Timeout` (future proof)
* Remove `mio::prelude`
* Remove `mio::util`
* Remove dependency on bytes

# 0.5.0 (December 3, 2015)

* Windows support (#239)
* NetBSD support (#306)
* Android support (#295)
* Don't re-export bytes types
* Renamed `EventLoop::register_opt` to `EventLoop::register` (#257)
* `EventLoopConfig` is now a builder instead of having public struct fields. It
  is also no longer `Copy`. (#259)
* `TcpSocket` is no longer exported in the public API (#262)
* Integrate with net2. (#262)
* `TcpListener` now returns the remote peer address from `accept` as well (#275)
* The `UdpSocket::{send_to, recv_from}` methods are no longer generic over `Buf`
  or `MutBuf` but instead take slices directly. The return types have also been
  updated to return the number of bytes transferred. (#260)
* Fix bug with kqueue where an error on registration prevented the
  changelist from getting flushed (#276)
* Support sending/receiving FDs over UNIX sockets (#291)
* Mio's socket types are permanently associated with an EventLoop (#308)
* Reduce unnecessary poll wakeups (#314)


# 0.4.1 (July 21, 2015)

* [BUGFIX] Fix notify channel concurrency bug (#216)

# 0.4.0 (July 16, 2015)

* [BUGFIX] EventLoop::register requests all events, not just readable.
* [BUGFIX] Attempting to send a message to a shutdown event loop fails correctly.
* [FEATURE] Expose TCP shutdown
* [IMPROVEMENT] Coalesce readable & writable into `ready` event (#184)
* [IMPROVEMENT] Rename TryRead & TryWrite function names to avoid conflict with std.
* [IMPROVEMENT] Provide TCP and UDP types in Mio (path to windows #155)
* [IMPROVEMENT] Use clock_ticks crate instead of time (path to windows #155)
* [IMPROVEMENT] Move unix specific features into mio::unix module
* [IMPROVEMENT] TcpListener sets SO_REUSEADDR by default
