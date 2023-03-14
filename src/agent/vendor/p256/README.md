# RustCrypto: NIST P-256 (secp256r1) elliptic curve

[![crate][crate-image]][crate-link]
[![Docs][docs-image]][docs-link]
![Apache2/MIT licensed][license-image]
![Rust Version][rustc-image]
[![Project Chat][chat-image]][chat-link]
[![Build Status][build-image]][build-link]

NIST P-256 elliptic curve (a.k.a. prime256v1, secp256r1) types implemented
in terms of traits from the [`elliptic-curve`] crate.

Optionally includes an [`arithmetic`] feature providing scalar and
affine/projective point types with support for constant-time scalar
multiplication, which can be used to implement protocols such as [ECDH].

[Documentation][docs-link]

## ⚠️ Security Warning

The elliptic curve arithmetic contained in this crate has never been
independently audited!

This crate has been designed with the goal of ensuring that secret-dependent
operations are performed in constant time (using the `subtle` crate and
constant-time formulas). However, it has not been thoroughly assessed to ensure
that generated assembly is constant time on common CPU architectures.

USE AT YOUR OWN RISK!

## Supported Algorithms

- [Elliptic Curve Diffie-Hellman (ECDH)][ECDH]: gated under the `ecdh` feature.
- [Elliptic Curve Digital Signature Algorithm (ECDSA)][ECDSA]: gated under the
  `ecdsa` feature.

## About NIST P-256

NIST P-256 is a Weierstrass curve specified in FIPS 186-4: Digital Signature
Standard (DSS):

<https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.186-4.pdf>

Also known as prime256v1 (ANSI X9.62) and secp256r1 (SECG), it's included in
the US National Security Agency's "Suite B" and is widely used in protocols
like TLS and the associated X.509 PKI.

## Minimum Supported Rust Version

Rust **1.47** or higher.

Minimum supported Rust version can be changed in the future, but it will be
done with a minor version bump.

## SemVer Policy

- All on-by-default features of this library are covered by SemVer
- MSRV is considered exempt from SemVer as noted above

## License

All crates licensed under either of

 * [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
 * [MIT license](http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[//]: # (badges)

[crate-image]: https://img.shields.io/crates/v/p256.svg
[crate-link]: https://crates.io/crates/p256
[docs-image]: https://docs.rs/p256/badge.svg
[docs-link]: https://docs.rs/p256/
[license-image]: https://img.shields.io/badge/license-Apache2.0/MIT-blue.svg
[rustc-image]: https://img.shields.io/badge/rustc-1.47+-blue.svg
[chat-image]: https://img.shields.io/badge/zulip-join_chat-blue.svg
[chat-link]: https://rustcrypto.zulipchat.com/#narrow/stream/260040-elliptic-curves
[build-image]: https://github.com/RustCrypto/elliptic-curves/workflows/p256/badge.svg?branch=master&event=push
[build-link]: https://github.com/RustCrypto/elliptic-curves/actions?query=workflow%3Ap256

[//]: # (general links)

[`elliptic-curve`]: https://github.com/RustCrypto/traits/tree/master/elliptic-curve
[`arithmetic`]: https://docs.rs/p256/latest/p256/arithmetic/index.html
[ECDH]: https://en.wikipedia.org/wiki/Elliptic-curve_Diffie-Hellman
[ECDSA]: https://en.wikipedia.org/wiki/Elliptic_Curve_Digital_Signature_Algorithm
