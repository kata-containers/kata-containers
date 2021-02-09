# 0.1.16

* Fix possible blocking in signal handler registered by `Signals`.

# 0.1.15

* Make `Signals` work in edge-triggered mode in mio too, by always draining
  everything from the socket. Needed, because mio 0.7 doesn't have
  level-triggered any more.

# 0.1.14

* `mio-0_7-support` feature for use with mio 0.7.0+.
* Bump minimal rustc version to 1.31.0 (signal-hook-registry can still build
  with 1.26.0).

# 0.1.13

* Some doc clarifications.

# 0.1.12

* `cleanup` module to register resetting signals to default.

# registry-1.2.0

* `unregister_signal`, to remove all hooks of one signal.

# 0.1.11

* Docs improvements.
* Fix registering pipes as well as sockets into the pipe module (#27).

# registry-1.1.1

* Update deps.

# registry-1.1.0

* Adding Windows support (thanks to @qnighy).

# 0.1.10

* Fix busy loop in Iterator::forever when the mio-support feature is enabled
  (#16).

# registry-1.0.1

* Include the registry files in the crates.io tarball.

# 0.1.9
# registry-1.0.0

* Split into backend signal-hook-registry and the frontend. The backend is much
  less likely to have breaking changes so it contains the things that can be in
  the application just once.

# 0.1.8

* The `Signals` iterator can now be closed (from another instance or thread),
  which can be used to shut down the thread handling signals from the main
  thread.

# 0.1.7

* The `Signals` iterator allows adding signals after creation.
* Fixed a bug where `Signals` registrations could be unregirestered too soon if
  the `Signals` was cloned previously.

# 0.1.6

* The internally used ArcSwap thing doesn't block other ArcSwaps now (has
  independent generation lock).

# 0.1.5

* Re-exported signal constants, so users no longer need libc.

# 0.1.4

* Compilation fix for android-aarch64

# 0.1.3

* Tokio support.
* Mio support.
* Dependency updates.

# 0.1.2

* Dependency updates.

# 0.1.1

* Get rid of `catch_unwind` inside the signal handler.
* Link to the nix crate.

# 0.1.0

* Initial basic implementation.
* Flag helpers.
* Pipe helpers.
* High-level iterator helper.
