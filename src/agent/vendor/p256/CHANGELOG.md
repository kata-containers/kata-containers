# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.8.1 (2021-05-10)
### Fixed
- Mixed coordinate addition with the point at infinity ([#337])

[#337]: https://github.com/RustCrypto/elliptic-curves/pull/337

## 0.8.0 (2021-04-29)
### Added
- `jwk` feature ([#279])
- Wycheproof ECDSA P-256 test vectors ([#313])
- `Order` constant ([#328])

### Changed
- Rename `ecdsa::Asn1Signature` to `::DerSignature` ([#288])
- Migrate to `FromDigest` trait from `ecdsa` crate ([#292])
- Bump `elliptic-curve` to v0.9.2 ([#296])
- Bump `pkcs8` to v0.6 ([#319])
- Bump `ecdsa` crate dependency to v0.11 ([#330])

### Fixed
- `DigestPrimitive` feature gating ([#324])

[#279]: https://github.com/RustCrypto/elliptic-curves/pull/279
[#288]: https://github.com/RustCrypto/elliptic-curves/pull/288
[#292]: https://github.com/RustCrypto/elliptic-curves/pull/292
[#296]: https://github.com/RustCrypto/elliptic-curves/pull/296
[#313]: https://github.com/RustCrypto/elliptic-curves/pull/313
[#319]: https://github.com/RustCrypto/elliptic-curves/pull/319
[#324]: https://github.com/RustCrypto/elliptic-curves/pull/324
[#328]: https://github.com/RustCrypto/elliptic-curves/pull/328
[#330]: https://github.com/RustCrypto/elliptic-curves/pull/330

## 0.7.3 (2021-04-16)
### Changed
- Make `ecdsa` a default feature ([#325])

[#325]: https://github.com/RustCrypto/elliptic-curves/pull/325

## 0.7.2 (2021-01-13)
### Changed
- Have `std` feature activate `ecdsa-core/std` ([#273])

[#273]: https://github.com/RustCrypto/elliptic-curves/pull/273

## 0.7.1 (2020-12-16)
### Fixed
- Trigger docs.rs rebuild with nightly bugfix ([RustCrypto/traits#412])

[RustCrypto/traits#412]: https://github.com/RustCrypto/traits/pull/412

## 0.7.0 (2020-12-16)
### Changed
- Bump `elliptic-curve` dependency to v0.8 ([#260])
- Bump `ecdsa` to v0.10 ([#260])

[#260]: https://github.com/RustCrypto/elliptic-curves/pull/260

## 0.6.0 (2020-12-06)
### Added
- PKCS#8 support ([#243], [#244], [#245])
- `PublicKey` type ([#239])

### Changed
- Bump `elliptic-curve` crate dependency to v0.7; MSRV 1.46+ ([#247])
- Bump `ecdsa` crate dependency to v0.9 ([#247])

[#247]: https://github.com/RustCrypto/elliptic-curves/pull/247
[#245]: https://github.com/RustCrypto/elliptic-curves/pull/245
[#244]: https://github.com/RustCrypto/elliptic-curves/pull/244
[#243]: https://github.com/RustCrypto/elliptic-curves/pull/243
[#239]: https://github.com/RustCrypto/elliptic-curves/pull/239

## 0.5.2 (2020-10-08)
### Fixed
- Regenerate `rustdoc` on https://docs.rs after nightly breakage

## 0.5.1 (2020-10-08)
### Added
- `SecretValue` impl when `arithmetic` feature is disabled ([#222])

[#222]: https://github.com/RustCrypto/elliptic-curves/pull/222

## 0.5.0 (2020-09-18)
### Added
- `ecdsa::Asn1Signature` type alias ([#186])
- `ff` and `group` crate dependencies; MSRV 1.44+ ([#169], [#174])
- `AffinePoint::identity()` and `::is_identity()` ([#167])

### Changed
- Bump `elliptic-curve` crate to v0.6; `ecdsa` to v0.8 ([#180])
- Refactor ProjectiveArithmetic trait ([#179])
- Support generic inner type for `elliptic_curve::SecretKey<C>` ([#177])
- Rename `ElementBytes` => `FieldBytes` ([#176])
- Rename `ecdsa::{Signer, Verifier}` => `::{SigningKey, VerifyKey}` ([#153])
- Rename `Curve::ElementSize` => `FieldSize` ([#150])
- Implement RFC6979 deterministic ECDSA ([#146], [#147])
- Rename `PublicKey` to `EncodedPoint` ([#141])

### Removed
- `rand` feature ([#162])

[#186]: https://github.com/RustCrypto/elliptic-curves/pull/186
[#180]: https://github.com/RustCrypto/elliptic-curves/pull/180
[#179]: https://github.com/RustCrypto/elliptic-curves/pull/179
[#177]: https://github.com/RustCrypto/elliptic-curves/pull/177
[#176]: https://github.com/RustCrypto/elliptic-curves/pull/176
[#174]: https://github.com/RustCrypto/elliptic-curves/pull/174
[#169]: https://github.com/RustCrypto/elliptic-curves/pull/164
[#167]: https://github.com/RustCrypto/elliptic-curves/pull/167
[#162]: https://github.com/RustCrypto/elliptic-curves/pull/162
[#153]: https://github.com/RustCrypto/elliptic-curves/pull/153
[#150]: https://github.com/RustCrypto/elliptic-curves/pull/150
[#147]: https://github.com/RustCrypto/elliptic-curves/pull/147
[#146]: https://github.com/RustCrypto/elliptic-curves/pull/146
[#141]: https://github.com/RustCrypto/elliptic-curves/pull/141

## 0.4.1 (2020-08-11)
### Fixed
- Builds with either `ecdsa-core` or `sha256` in isolation ([#133])

[#133]: https://github.com/RustCrypto/elliptic-curves/pull/133

## 0.4.0 (2020-08-10)
### Added
- ECDSA support ([#73], [#101], [#104], [#105])
- ECDSA public key recovery support ([#110])
- OID support ([#103], [#113])
- Elliptic Curve Diffie-Hellman ([#120])

### Changed
- Bump `elliptic-curve` crate dependency to v0.5 ([#126])

[#73]: https://github.com/RustCrypto/elliptic-curves/pull/73
[#101]: https://github.com/RustCrypto/elliptic-curves/pull/101
[#103]: https://github.com/RustCrypto/elliptic-curves/pull/103
[#104]: https://github.com/RustCrypto/elliptic-curves/pull/104
[#105]: https://github.com/RustCrypto/elliptic-curves/pull/105
[#110]: https://github.com/RustCrypto/elliptic-curves/pull/110
[#113]: https://github.com/RustCrypto/elliptic-curves/pull/113
[#120]: https://github.com/RustCrypto/elliptic-curves/pull/120
[#126]: https://github.com/RustCrypto/elliptic-curves/pull/126

## 0.3.0 (2020-06-08)
### Changed
- Bump `elliptic-curve` crate dependency to v0.4 ([#39])

[#39]: https://github.com/RustCrypto/elliptic-curves/pull/39

## 0.2.0 (2020-04-30)
### Added
- Constant time scalar multiplication ([#18])
- Group operation ([#15])

[#18]: https://github.com/RustCrypto/elliptic-curves/pull/18
[#15]: https://github.com/RustCrypto/elliptic-curves/pull/15

## 0.1.0 (2020-01-15)
- Initial release
