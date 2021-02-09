<p align="center">

  <a href="https://github.com/slog-rs/slog">
  <img src="https://cdn.rawgit.com/slog-rs/misc/master/media/slog.svg" alt="slog-rs logo">
  </a>
  <br>

  <a href="https://travis-ci.org/slog-rs/slog">
      <img src="https://img.shields.io/travis/slog-rs/slog/master.svg" alt="Travis CI Build Status">
  </a>

  <a href="https://crates.io/crates/slog">
      <img src="https://img.shields.io/crates/d/slog.svg" alt="slog-rs on crates.io">
  </a>

  <a href="https://gitter.im/slog-rs/slog">
      <img src="https://img.shields.io/gitter/room/slog-rs/slog.svg" alt="slog-rs Gitter Chat">
  </a>

  <a href="https://docs.rs/releases/search?query=slog-">
      <img src="https://docs.rs/slog/badge.svg" alt="docs-rs: release versions documentation">
  </a>
  <br>
    <strong><a href="https://github.com/slog-rs/slog/wiki/Getting-started">Getting started</a></strong>
  <a href="//github.com/slog-rs/slog/wiki/Introduction-to-structured-logging-with-slog">Introduction</a>
  <a href="//github.com/slog-rs/slog/wiki/FAQ">FAQ</a>
  <br>
  <a href="https://crates.io/search?q=slog">Crate list</a>
</p>

# slog-rs - The Logging for [Rust][rust]

### Introduction (please read)

`slog` is an ecosystem of reusable components for structured, extensible,
composable and contextual logging for [Rust][rust].

The ambition is to be The Logging Library for Rust. `slog` should accommodate a
variety of logging features and requirements. If there is a feature that you
need and standard `log` crate is missing, `slog` should have it.

This power comes with a little steeper learning curve, so if you experience any
problems, **please join [slog-rs gitter] channel** to get up to speed. If you'd
like to take a quick, convenient route, consider using
[sloggers](https://docs.rs/sloggers/) wrapper library.

While the code is reliable, the documentation sometimes could use an improvement.
Please report all issues and ideas.

### Features & technical documentation

Most of the interesting documentation is auto-generated and hosted on [https://docs.rs](https://docs.rs/slog/).

Go to [docs.rs/slog](https://docs.rs/slog/) to read about features and APIs
(examples included).

**Note**: `slog` is just a core, and the actual functionality is inside
many feature crates. To name a few:

* [slog-term](https://docs.rs/slog-term/) for terminal output
* [slog-async](https://docs.rs/slog-async/) for asynchronous logging
* [slog-json](https://docs.rs/slog-json/) for logging JSON
* [slog-syslog](https://docs.rs/slog-syslog/) for logging to syslog
* [sloggers](https://docs.rs/sloggers/) for convenience methods (note: [3rd-party library](https://github.com/sile/sloggers))

There are many more slog feature crates. Search for [more slog features on
crates.io](https://crates.io/search?q=slog). It is easy to write and publish
new ones. Look through all the [existing crates using
slog](https://crates.io/crates/slog/reverse_dependencies) for examples and ideas.

### Terminal output example

`slog-term` is only one of many `slog` features - useful showcase,
multi-platform, and featuring eg. automatic TTY detection and colors.

See following screenshot: same output in both compact and full output mode.

![slog-rs terminal example output](http://i.imgur.com/mqrG8yL.png)

## Using & help

Please use [slog-rs gitter] channel to ask for help or discuss
slog features.

See
[examples/features.rs](https://github.com/slog-rs/misc/blob/master/examples/features.rs)
for full quick code example overview.

Read [Documentation](https://docs.rs/slog/) for details and features.

To report a bug or ask for features use [github issues][issues].

[faq]: https://github.com/slog-rs/slog/wiki/FAQ
[wiki]: https://github.com/slog-rs/slog/wiki/
[rust]: http://rust-lang.org
[slog-rs gitter]: https://gitter.im/slog-rs/slog
[issues]: //github.com/slog-rs/slog/issues

## Slog community

Slog related crates are hosted under [slog github
organization](https://github.com/slog-rs).

Dawid Ciężarkiewicz is the original author and current maintainer of `slog` and
therefore self-appointed benevolent dictator over the project. When working on
slog Dawid follows and expects everyone to follow his [Code of
Conduct](https://github.com/dpc/public/blob/master/COC.md).

Any particular repositories under slog ecosystem might be created, controlled,
maintained by other entities with various levels of autonomy. Lets work together
toward a common goal in a respectful and welcoming atmosphere!

## Verification Recommendation

To help with the maintaince, the ownership of this crate is potentially shared between multiple developers.
It is recommended to always use [cargo-crev](https://github.com/crev-dev/cargo-crev)
to verify the trustworthiness of each of your dependencies, including this one.
