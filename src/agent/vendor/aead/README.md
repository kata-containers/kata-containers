# RustCrypto: Authenticated Encryption with Additional Data Traits

[![crate][crate-image]][crate-link]
[![Docs][docs-image]][docs-link]
![Apache2/MIT licensed][license-image]
![Rust Version][rustc-image]
[![Build Status][build-image]][build-link]

This crate provides an abstract interface for [AEAD] ciphers, which guarantee
both confidentiality and integrity, even from a powerful attacker who is
able to execute [chosen-ciphertext attacks]. The resulting security property,
[ciphertext indistinguishability], is considered a basic requirement for
modern cryptographic implementations.

See [RustCrypto/AEADs] for cipher implementations which use this trait.

[Documentation][docs-link]

## Minimum Supported Rust Version

Rust **1.41** or higher.

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

[crate-image]: https://img.shields.io/crates/v/aead.svg
[crate-link]: https://crates.io/crates/aead
[docs-image]: https://docs.rs/aead/badge.svg
[docs-link]: https://docs.rs/aead/
[license-image]: https://img.shields.io/badge/license-Apache2.0/MIT-blue.svg
[rustc-image]: https://img.shields.io/badge/rustc-1.41+-blue.svg
[build-image]: https://github.com/RustCrypto/traits/workflows/aead/badge.svg?branch=master&event=push
[build-link]: https://github.com/RustCrypto/traits/actions?query=workflow%3Aaead

[//]: # (general links)

[AEAD]: https://en.wikipedia.org/wiki/Authenticated_encryption
[chosen-ciphertext attacks]: https://en.wikipedia.org/wiki/Chosen-ciphertext_attack
[ciphertext indistinguishability]: https://en.wikipedia.org/wiki/Ciphertext_indistinguishability
[RustCrypto/AEADs]: https://github.com/RustCrypto/AEADs
