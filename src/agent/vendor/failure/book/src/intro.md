# failure

This is the documentation for the failure crate, which provides a system for
creating and managing errors in Rust. Additional documentation is found here:

* [API documentation][api]
* [failure source code][repo]

[api]: https://docs.rs/failure
[repo]: https://github.com/rust-lang-nursery/failure

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
