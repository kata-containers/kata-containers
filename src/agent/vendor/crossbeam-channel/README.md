# Crossbeam Channel

[![Build Status](https://travis-ci.org/crossbeam-rs/crossbeam.svg?branch=master)](
https://travis-ci.org/crossbeam-rs/crossbeam)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](
https://github.com/crossbeam-rs/crossbeam-channel)
[![Cargo](https://img.shields.io/crates/v/crossbeam-channel.svg)](
https://crates.io/crates/crossbeam-channel)
[![Documentation](https://docs.rs/crossbeam-channel/badge.svg)](
https://docs.rs/crossbeam-channel)
[![Rust 1.28+](https://img.shields.io/badge/rust-1.28+-lightgray.svg)](
https://www.rust-lang.org)
[![chat](https://img.shields.io/discord/569610676205781012.svg?logo=discord)](https://discord.gg/BBYwKq)

This crate provides multi-producer multi-consumer channels for message passing.
It is an alternative to [`std::sync::mpsc`] with more features and better performance.

Some highlights:

* [`Sender`]s and [`Receiver`]s can be cloned and shared among threads.
* Two main kinds of channels are [`bounded`] and [`unbounded`].
* Convenient extra channels like [`after`], [`never`], and [`tick`].
* The [`select!`] macro can block on multiple channel operations.
* [`Select`] can select over a dynamically built list of channel operations.
* Channels use locks very sparingly for maximum [performance](benchmarks).

[`std::sync::mpsc`]: https://doc.rust-lang.org/std/sync/mpsc/index.html
[`Sender`]: https://docs.rs/crossbeam-channel/*/crossbeam_channel/struct.Sender.html
[`Receiver`]: https://docs.rs/crossbeam-channel/*/crossbeam_channel/struct.Receiver.html
[`bounded`]: https://docs.rs/crossbeam-channel/*/crossbeam_channel/fn.bounded.html
[`unbounded`]: https://docs.rs/crossbeam-channel/*/crossbeam_channel/fn.unbounded.html
[`after`]: https://docs.rs/crossbeam-channel/*/crossbeam_channel/fn.after.html
[`never`]: https://docs.rs/crossbeam-channel/*/crossbeam_channel/fn.never.html
[`tick`]: https://docs.rs/crossbeam-channel/*/crossbeam_channel/fn.tick.html
[`select!`]: https://docs.rs/crossbeam-channel/*/crossbeam_channel/macro.select.html
[`Select`]: https://docs.rs/crossbeam-channel/*/crossbeam_channel/struct.Select.html

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
crossbeam-channel = "0.4"
```

Next, add this to your crate:

```rust
#[macro_use]
extern crate crossbeam_channel;
```

## Compatibility

The minimum supported Rust version is 1.28. Any change to this is considered a breaking change.

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

#### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

#### Third party software

This product includes copies and modifications of software developed by third parties:

* [examples/matching.rs](examples/matching.rs) includes
  [matching.go](http://www.nada.kth.se/~snilsson/concurrency/src/matching.go) by Stefan Nilsson,
  licensed under Creative Commons Attribution 3.0 Unported License.

* [src/flavors/array.rs](src/flavors/array.rs) is based on
  [Bounded MPMC queue](http://www.1024cores.net/home/lock-free-algorithms/queues/bounded-mpmc-queue)
  by Dmitry Vyukov, licensed under the Simplified BSD License and the Apache License, Version 2.0.

* [tests/mpsc.rs](tests/mpsc.rs) includes modifications of code from The Rust Programming Language,
  licensed under the MIT License and the Apache License, Version 2.0.

* [tests/golang.rs](tests/golang.rs) is based on code from The Go Programming Language, licensed
  under the 3-Clause BSD License.

See the source code files for more details.

Copies of third party licenses can be found in [LICENSE-THIRD-PARTY](LICENSE-THIRD-PARTY).
