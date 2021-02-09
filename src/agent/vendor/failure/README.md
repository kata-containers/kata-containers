# failure - a new error management story

**Notice**: `failure` is deprecated. If you liked `failure`'s API, consider using:
- [Anyhow](https://github.com/dtolnay/anyhow) is a good replacement for `failure::Error`.
- [thiserror](https://github.com/dtolnay/thiserror) is a good, near drop-in replacement for `#[derive(Fail)]`.

---

[![Build Status](https://travis-ci.org/rust-lang-nursery/failure.svg?branch=master)](https://travis-ci.org/rust-lang-nursery/failure)
[![Latest Version](https://img.shields.io/crates/v/failure.svg)](https://crates.io/crates/failure)
[![docs](https://docs.rs/failure/badge.svg)](https://docs.rs/failure)

`failure` is designed to make it easier to manage errors in Rust. It is
intended to replace error management based on `std::error::Error` with a new
system based on lessons learned over the past several years, including those
learned from experience with quick-error and error-chain.

`failure` provides two core components:

* `Fail`: A new trait for custom error types.
* `Error`: A struct which any type that implements `Fail` can be cast into.

## Evolution

Failure is currently evolving as a library.  First of all there is work going
on in Rust itself to [fix the error trait](https://github.com/rust-lang/rfcs/pull/2504)
secondarily the original plan for Failure towards 1.0 is unlikely to happen
in the current form.

As such the original master branch towards 1.0 of failure was removed and
master now represents the future iteration steps of 0.1 until it's clear
what happens in the stdlib.

The original 1.0 branch can be found in [evolution/1.0](https://github.com/rust-lang-nursery/failure/tree/evolution/1.0).

## Example

```rust
extern crate serde;
extern crate toml;

#[macro_use] extern crate failure;
#[macro_use] extern crate serde_derive;

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use failure::Error;

// This is a new error type that you've created. It represents the ways a
// toolchain could be invalid.
//
// The custom derive for Fail derives an impl of both Fail and Display.
// We don't do any other magic like creating new types.
#[derive(Debug, Fail)]
enum ToolchainError {
    #[fail(display = "invalid toolchain name: {}", name)]
    InvalidToolchainName {
        name: String,
    },
    #[fail(display = "unknown toolchain version: {}", version)]
    UnknownToolchainVersion {
        version: String,
    }
}

pub struct ToolchainId {
    // ... etc
}

impl FromStr for ToolchainId {
    type Err = ToolchainError;

    fn from_str(s: &str) -> Result<ToolchainId, ToolchainError> {
        // ... etc
    }
}

pub type Toolchains = HashMap<ToolchainId, PathBuf>;

// This opens a toml file containing associations between ToolchainIds and
// Paths (the roots of those toolchains).
//
// This could encounter an io Error, a toml parsing error, or a ToolchainError,
// all of them will be thrown into the special Error type
pub fn read_toolchains(path: PathBuf) -> Result<Toolchains, Error>
{
    use std::fs::File;
    use std::io::Read;

    let mut string = String::new();
    File::open(path)?.read_to_string(&mut string)?;

    let toml: HashMap<String, PathBuf> = toml::from_str(&string)?;

    let toolchains = toml.iter().map(|(key, path)| {
        let toolchain_id = key.parse()?;
        Ok((toolchain_id, path))
    }).collect::<Result<Toolchains, ToolchainError>>()?;

    Ok(toolchains)
}
```

## Requirements

Both failure and failure_derive are intended to compile on all stable versions
of Rust newer than 1.31.0, as well as the latest beta and the latest nightly.
If either crate fails to compile on any version newer than 1.31.0, please open
an issue.

failure is **no_std** compatible, though some aspects of it (primarily the
`Error` type) will not be available in no_std mode.

## License

failure is licensed under the terms of the MIT License or the Apache License
2.0, at your choosing.

## Code of Conduct

Contribution to the failure crate is organized under the terms of the
Contributor Covenant, the maintainer of failure, @withoutboats, promises to
intervene to uphold that code of conduct.
