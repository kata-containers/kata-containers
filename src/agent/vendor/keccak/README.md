# RustCrypto: Keccak Sponge Function

[![crate][crate-image]][crate-link]
[![Docs][docs-image]][docs-link]
[![Build Status][build-image]][build-link]
![Apache2/MIT licensed][license-image]
![Rust Version][rustc-image]
[![Project Chat][chat-image]][chat-link]

Pure Rust implementation of the [Keccak Sponge Function][1] including the keccak-f
and keccak-p variants.

[Documentation][docs-link]

## About

This crate implements the core Keccak sponge function, upon which many other
cryptographic functions are built.

For the SHA-3 family including the SHAKE XOFs, see the [`sha3`] crate, which
is built on this crate.

## Minimum Supported Rust Version

Rust **1.41** or higher by default, or **1.59** with the `asm` feature enabled.

Minimum supported Rust version can be changed in the future, but it will be
done with a minor version bump.

## SemVer Policy

- All on-by-default features of this library are covered by SemVer
- MSRV is considered exempt from SemVer as noted above

## License

Licensed under either of:

 * [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
 * [MIT license](http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[//]: # (badges)

[crate-image]: https://img.shields.io/crates/v/keccak.svg
[crate-link]: https://crates.io/crates/keccak
[docs-image]: https://docs.rs/keccak/badge.svg
[docs-link]: https://docs.rs/keccak/
[license-image]: https://img.shields.io/badge/license-Apache2.0/MIT-blue.svg
[rustc-image]: https://img.shields.io/badge/rustc-1.41+-blue.svg
[chat-image]: https://img.shields.io/badge/zulip-join_chat-blue.svg
[chat-link]: https://rustcrypto.zulipchat.com/#narrow/stream/260041-hashes
[build-image]: https://github.com/RustCrypto/sponges/actions/workflows/keccak.yml/badge.svg
[build-link]: https://github.com/RustCrypto/sponges/actions/workflows/keccak.yml

[//]: # (general links)

[1]: https://keccak.team/keccak.html
[`sha3`]: https://github.com/RustCrypto/hashes/tree/master/sha3
