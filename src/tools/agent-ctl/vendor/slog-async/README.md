# slog-async - Asynchronous drain for [slog-rs][slog-rs]

<p align="center">
  <a href="https://travis-ci.org/slog-rs/async">
      <img src="https://img.shields.io/travis/slog-rs/async/master.svg" alt="Travis CI Build Status">
  </a>

  <a href="https://crates.io/crates/slog-async">
      <img src="https://img.shields.io/crates/d/slog-async.svg" alt="slog-async on crates.io">
  </a>

  <a href="https://gitter.im/dpc/slog-async">
      <img src="https://img.shields.io/gitter/room/dpc/slog-rs.svg" alt="slog-rs Gitter Chat">
  </a>

  <a href="https://deps.rs/repo/github/slog-rs/async">
        <img src="https://deps.rs/repo/github/slog-rs/async/status.svg" alt="slog-rs Dependency Status">
  </a>
</p>

For more information, help, to report issues etc. see [slog-rs][slog-rs].

Note: Unlike other logging solutions `slog-rs` does not have a hardcoded async
logging implementation. This crate is just a reasonable reference
implementation. It should have good performance and work well in most use
cases. See documentation and implementation for more details.

It's relatively easy to implement custom `slog-rs` async logging. Feel free to
use this one as a starting point.

[slog-rs]: //github.com/slog-rs/slog
