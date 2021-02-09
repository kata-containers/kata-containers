# error-chain - Consistent error handling for Rust

[![Build Status](https://travis-ci.com/rust-lang-nursery/error-chain.svg?branch=master)](https://travis-ci.com/rust-lang-nursery/error-chain)
[![Latest Version](https://img.shields.io/crates/v/error-chain.svg)](https://crates.io/crates/error-chain)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-green.svg)](https://github.com/rust-lang-nursery/error-chain)

`error-chain` makes it easy to take full advantage of Rust's error
handling features without the overhead of maintaining boilerplate
error types and conversions. It implements an opinionated strategy for
defining your own error types, as well as conversions from others'
error types.

[Documentation (crates.io)](https://docs.rs/error-chain).

[Documentation (master)](https://rust-lang-nursery.github.io/error-chain).

## Quick start

If you just want to set up your new project with error-chain,
follow the [quickstart.rs] template, and read this [intro]
to error-chain.

[quickstart.rs]: https://github.com/rust-lang-nursery/error-chain/blob/master/examples/quickstart.rs
[intro]: http://brson.github.io/2016/11/30/starting-with-error-chain

## Supported Rust version

Please view the beginning of the [Travis configuration file](.travis.yml)
to see the oldest supported Rust version.

Note that `error-chain` supports older versions of Rust when built with
`default-features = false`.

## License

MIT/Apache-2.0
