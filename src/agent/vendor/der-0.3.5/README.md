# RustCrypto: ASN.1 DER

[![crate][crate-image]][crate-link]
[![Docs][docs-image]][docs-link]
![Apache2/MIT licensed][license-image]
![Rust Version][rustc-image]
[![Project Chat][chat-image]][chat-link]
[![Build Status][build-image]][build-link]

Pure Rust embedded-friendly implementation of the Distinguished Encoding Rules (DER)
for Abstract Syntax Notation One (ASN.1) as described in ITU X.690.

[Documentation][docs-link]

# About

This crate provides a `no_std`-friendly implementation of a subset of ASN.1 DER
necessary  for decoding/encoding various cryptography-related formats
implemented as part of the [RustCrypto] project, e.g. the [`pkcs8`] crate.

The core implementation avoids any heap usage (with convenience methods
that allocate gated under the off-by-default `alloc` feature).

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

[crate-image]: https://img.shields.io/crates/v/der.svg
[crate-link]: https://crates.io/crates/der
[docs-image]: https://docs.rs/der/badge.svg
[docs-link]: https://docs.rs/der/
[license-image]: https://img.shields.io/badge/license-Apache2.0/MIT-blue.svg
[rustc-image]: https://img.shields.io/badge/rustc-1.47+-blue.svg
[chat-image]: https://img.shields.io/badge/zulip-join_chat-blue.svg
[chat-link]: https://rustcrypto.zulipchat.com/#narrow/stream/260052-utils
[build-image]: https://github.com/RustCrypto/utils/actions/workflows/der.yml/badge.svg
[build-link]: https://github.com/RustCrypto/utils/actions/workflows/der.yml

[//]: # (general links)

[RustCrypto]: https://github.com/rustcrypto
[`pkcs8`]: https://docs.rs/pkcs8/
[RustCrypto/utils#370]: https://github.com/RustCrypto/utils/issues/370
