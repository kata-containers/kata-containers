# [RustCrypto]: Ed25519

[![crate][crate-image]][crate-link]
[![Docs][docs-image]][docs-link]
[![Build Status][build-image]][build-link]
![Apache2/MIT licensed][license-image]
![Rust Version][rustc-image]
[![Project Chat][chat-image]][chat-link]

[Edwards Digital Signature Algorithm (EdDSA)][1] over Curve25519 as specified
in [RFC 8032][2].

[Documentation][docs-link]

## About

This crate doesn't contain an implementation of Ed25519, but instead
contains an [`ed25519::Signature`][3] type which other crates can use in
conjunction with the [`signature::Signer`][4] and [`signature::Verifier`][5]
traits.

These traits allow crates which produce and consume Ed25519 signatures
to be written abstractly in such a way that different signer/verifier
providers can be plugged in, enabling support for using different
Ed25519 implementations, including HSMs or Cloud KMS services.

## Minimum Supported Rust Version

This crate requires **Rust 1.57** at a minimum.

Previous 1.x releases of this crate supported an MSRV of 1.47. If you would
like to use this crate with earlier releases of Rust, add the following version
constraint in your project's Cargo.toml to constrain it to the supported
version range:

```toml
[dependencies]
ed25519 = ">=1, <1.4" # ed25519 1.4 requires MSRV 1.57
```

Note that is our policy that we may change the MSRV in the future, but it will
be accompanied by a minor version bump.

## SemVer Policy

- All on-by-default features of this library are covered by SemVer
- MSRV is considered exempt from SemVer as noted above
- The `pkcs8` module is exempted as it uses a pre-1.0 dependency, however, 
  breaking changes to this module will be accompanied by a minor version bump.

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

[crate-image]: https://buildstats.info/crate/ed25519
[crate-link]: https://crates.io/crates/ed25519
[docs-image]: https://docs.rs/ed25519/badge.svg
[docs-link]: https://docs.rs/ed25519/
[build-image]: https://github.com/RustCrypto/signatures/actions/workflows/ed25519.yml/badge.svg
[build-link]: https://github.com/RustCrypto/signatures/actions/workflows/ed25519.yml
[license-image]: https://img.shields.io/badge/license-Apache2.0/MIT-blue.svg
[rustc-image]: https://img.shields.io/badge/rustc-1.57+-blue.svg
[chat-image]: https://img.shields.io/badge/zulip-join_chat-blue.svg
[chat-link]: https://rustcrypto.zulipchat.com/#narrow/stream/260048-signatures

[//]: # (links)

[RustCrypto]: https://github.com/RustCrypto

[//]: # (footnotes)

[1]: https://en.wikipedia.org/wiki/EdDSA
[2]: https://tools.ietf.org/html/rfc8032
[3]: https://docs.rs/ed25519/latest/ed25519/struct.Signature.html
[4]: https://docs.rs/signature/latest/signature/trait.Signer.html
[5]: https://docs.rs/signature/latest/signature/trait.Verifier.html
