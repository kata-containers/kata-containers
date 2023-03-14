<p align="center">

  <a href="https://github.com/slog-rs/slog">
  <img src="https://cdn.rawgit.com/slog-rs/misc/master/media/slog.svg" alt="slog-rs logo">
  </a>
  <br>

  <a href="https://travis-ci.org/slog-rs/scope">
      <img src="https://img.shields.io/travis/slog-rs/scope/master.svg" alt="Travis CI Build Status">
  </a>

  <a href="https://crates.io/crates/slog-scope">
      <img src="https://img.shields.io/crates/d/slog-scope.svg" alt="slog-scope on crates.io">
  </a>

  <a href="https://gitter.im/slog-rs/slog">
      <img src="https://img.shields.io/gitter/room/slog-rs/slog.svg" alt="slog-rs Gitter Chat">
  </a>
</p>

# slog-scope - Logging scopes for [slog-rs]

`slog-scope` allows logging without manually handling `Logger` objects.

It is generally advised **NOT** to use `slog_scope` in libraries. Read more in
[slog-rs
FAQ](https://github.com/slog-rs/slog/wiki/FAQ#do-i-have-to-pass-logger-around)

For more information, help, to report issues etc. see [slog-rs][slog-rs].


[slog-rs]: //github.com/slog-rs/slog



## Verification Recommendation

To help with the maintaince, the ownership of this crate is potentially shared between multiple developers.
It is recommended to always use [cargo-crev](https://github.com/crev-dev/cargo-crev)
to verify the trustworthiness of each of your dependencies, including this one.
