# serial_test
[![Version](https://img.shields.io/crates/v/serial_test.svg)](https://crates.io/crates/serial_test)
[![Downloads](https://img.shields.io/crates/d/serial_test)](https://crates.io/crates/serial_test)
[![Docs](https://docs.rs/serial_test/badge.svg)](https://docs.rs/serial_test/)
[![MIT license](https://img.shields.io/crates/l/serial_test.svg)](./LICENSE)
[![Build Status](https://travis-ci.com/palfrey/serial_test.svg?branch=main)](https://travis-ci.com/palfrey/serial_test)
[![MSRV: 1.39.0](https://flat.badgen.net/badge/MSRV/1.39.0/purple)](https://blog.rust-lang.org/2019/11/07/Rust-1.39.0.html)
[![dependency status](https://deps.rs/repo/github/palfrey/serial_test/status.svg)](https://deps.rs/repo/github/palfrey/serial_test)

`serial_test` allows for the creation of serialised Rust tests using the `serial` attribute
e.g.
```rust
#[test]
#[serial]
fn test_serial_one() {
  // Do things
}

#[test]
#[serial]
fn test_serial_another() {
  // Do things
}

#[tokio::test]
#[serial]
async fn test_serial_another() {
  // Do things asynchronously
}
```
Multiple tests with the `serial` attribute are guaranteed to be executed in serial. Ordering of the tests is not guaranteed however.

## Usage
We require at least Rust 1.39 for [async/await](https://blog.rust-lang.org/2019/11/07/Async-await-stable.html) support

Add to your Cargo.toml
```toml
[dev-dependencies]
serial_test = "*"
```

plus `use serial_test::serial;` (for Rust 2018) or
```rust
#[macro_use]
extern crate serial_test;
```
for earlier versions.

You can then either add `#[serial]` or `#[serial(some_text)]` to tests as required.
