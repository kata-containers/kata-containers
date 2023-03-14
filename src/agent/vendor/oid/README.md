# Object Identifier Library for Rust

[![All Contributors](https://img.shields.io/badge/all_contributors-1-orange.svg?style=flat-square)](#contributors)

[![Build Status](https://travis-ci.org/UnnecessaryEngineering/oid.svg?branch=master)](https://travis-ci.org/UnnecessaryEngineering/oid)
[![Crate](https://img.shields.io/crates/v/oid.svg)](https://crates.io/crates/oid)
[![codecov](https://codecov.io/gh/UnnecessaryEngineering/oid/branch/master/graph/badge.svg)](https://codecov.io/gh/UnnecessaryEngineering/oid)
[![API](https://docs.rs/oid/badge.svg)](https://docs.rs/oid)
![Minimum rustc version](https://img.shields.io/badge/rustc-1.34+-lightgray.svg)
[![Average time to resolve an issue](https://isitmaintained.com/badge/resolution/UnnecessaryEngineering/oid.svg)](http://isitmaintained.com/project/UnnecessaryEngineering/oid)
[![Percentage of issues still open](https://isitmaintained.com/badge/open/UnnecessaryEngineering/oid.svg)](http://isitmaintained.com/project/UnnecessaryEngineering/oid)

[Object Identifiers] are a standard of the [ITU] used to reference objects, things, and
concepts in a globally unique way. This crate provides for data structures and methods
to build, parse, and format OIDs.

## Basic Utilization

### Running Example

You can run the example code from [examples/basic.rs](examples/basic.rs) using cargo:

```sh
cargo run --example basic
```

### Parsing OID String Representation

```rust
use oid::prelude::*;
let oid = ObjectIdentifier::try_from("0.1.2.3")?;
```

### Parsing OID Binary Representation

```rust
use oid::prelude::*;
let oid = ObjectIdentifier::try_from(vec![0x00, 0x01, 0x02, 0x03])?;
```

### Encoding OID as String Representation

```rust
use oid::prelude::*;
let oid = ObjectIdentifier::try_from("0.1.2.3")?;
let oid: String = oid.into();
assert_eq!(oid, "0.1.2.3");
```

### Encoding OID as Binary Representation

```rust
use oid::prelude::*;
let oid = ObjectIdentifier::try_from(vec![0x00, 0x01, 0x02, 0x03])?;
let oid: Vec<u8> = oid.into();
assert_eq!(oid, "0.1.2.3");
```

### Adding as a dependency with [cargo-edit]

```sh
cargo add oid
```

### Adding as a dependency with [cargo-edit] for a `!#[no_std]` crate

```sh
cargo add oid --no-default-features
```

### Adding as a dependency directly to `Cargo.toml`

```toml
[dependencies]
oid = "0.1.0"
```

### Adding as a dependency directly to `Cargo.toml` for a `!#[no_std]` crate

```toml
[dependencies]
oid = { default-features = false }
```

## Building

The build routines have been automated with [cargo-make]. If you're not using [cargo-make], you can check [Makefile.toml] for the relevant manual build procedures.

### Building for a platform with Rust Standard Library

```sh
cargo make
```

### Building for an embedded platform or `#![no_std]`

```sh
cargo make build_no_std
```

### Fuzzing Inputs

Profiles for [cargo-fuzz] are included for fuzzing the inputs on public method parameters.

#### Fuzz Binary OID Parsing

```sh
cargo make fuzz_parse_binary
```

#### Fuzz String OID Parsing

```sh
cargo make fuzz_parse_string
```

## Contributors ✨

Thanks goes to these wonderful people ([emoji key](https://allcontributors.org/docs/en/emoji-key)):

<!-- ALL-CONTRIBUTORS-LIST:START - Do not remove or modify this section -->
<!-- prettier-ignore -->
<table>
  <tr>
    <td align="center"><a href="https://github.com/sbruton"><img src="https://avatars2.githubusercontent.com/u/961430?v=4" width="100px;" alt="Sean Bruton"/><br /><sub><b>Sean Bruton</b></sub></a><br /><a href="https://github.com/UnnecessaryEngineering/oid/commits?author=sbruton" title="Tests">⚠️</a> <a href="https://github.com/UnnecessaryEngineering/oid/commits?author=sbruton" title="Code">💻</a></td>
    <td align="center"><a href="https://github.com/bcortier-devolutions"><img src="https://avatars2.githubusercontent.com/u/54852465?v=4" width="100px;" alt="Benoît C."/><br /><sub><b>Benoît C.</b></sub></a><br /><a href="https://github.com/UnnecessaryEngineering/oid/commits?author=bcortier-devolutions" title="Tests">⚠️</a> <a href="https://github.com/UnnecessaryEngineering/oid/commits?author=bcortier-devolutions" title="Code">💻</a></td>
    <td align="center"><a href="https://github.com/snake66"><img src="https://avatars2.githubusercontent.com/u/852601?v=4" width="100px;" alt="snake66"/><br /><sub><b>snake66</b></sub></a><br /><a href="https://github.com/UnnecessaryEngineering/oid/commits?author=snake66" title="Tests">⚠️</a> <a href="https://github.com/UnnecessaryEngineering/oid/commits?author=snake66" title="Code">💻</a></td>
  </tr>
</table>

<!-- ALL-CONTRIBUTORS-LIST:END -->

This project follows the [all-contributors](https://github.com/all-contributors/all-contributors) specification. Contributions of any kind welcome!

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this library by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

[object identifiers]: https://en.wikipedia.org/wiki/Object_identifier
[itu]: https://en.wikipedia.org/wiki/International_Telecommunications_Union
[cargo-edit]: https://github.com/killercup/cargo-edit
[cargo-make]: https://github.com/sagiegurari/cargo-make
[cargo-fuzz]: https://github.com/rust-fuzz/cargo-fuzz
