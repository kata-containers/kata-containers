# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.9.12 (2021-05-18)
### Added
- `Ord` and `PartialOrd` impls on `PublicKey` ([#637])

[#637]: https://github.com/RustCrypto/traits/pull/637

## 0.9.11 (2021-04-21)
### Added
- Impl `subtle` traits on `ScalarBytes<C>` ([#612])

### Fixed
- Always re-export ScalarBytes ([#613])

[#612]: https://github.com/RustCrypto/traits/pull/612
[#613]: https://github.com/RustCrypto/traits/pull/613

## 0.9.10 (2021-04-21)
### Added
- `ScalarBytes` type ([#610])

[#610]: https://github.com/RustCrypto/traits/pull/610

## 0.9.9 (2021-04-21) [YANKED]
### Added
- `Order::is_scalar_repr_in_range` ([#608])

[#608]: https://github.com/RustCrypto/traits/pull/608

## 0.9.8 (2021-04-21)
### Added
- Define `Order` for `MockCurve` ([#606])

[#606]: https://github.com/RustCrypto/traits/pull/606

## 0.9.7 (2021-04-21)
### Added
- `Order` trait ([#603])

### Fixed
- Warnings from `pkcs8` imports ([#604])

[#603]: https://github.com/RustCrypto/traits/pull/603
[#604]: https://github.com/RustCrypto/traits/pull/604

## 0.9.6 (2021-03-22)
### Changed
- Bump `pkcs8` dependency to v0.6 ([#585])

[#585]: https://github.com/RustCrypto/traits/pull/585

## 0.9.5 (2021-03-17) [YANKED]
### Added
- Implement `{to,char}_le_bits` for `MockCurve` ([#565])
- Implement `one()` for mock `Scalar` ([#566])

### Changed
- Use string-based OID constants ([#561])
- Bump `base64ct` dependency to v1.0 ([#581])

[#561]: https://github.com/RustCrypto/traits/pull/561
[#565]: https://github.com/RustCrypto/traits/pull/565
[#566]: https://github.com/RustCrypto/traits/pull/566
[#581]: https://github.com/RustCrypto/traits/pull/581

## 0.9.4 (2021-02-18) [YANKED]
### Fixed
- Breakage related to the `pkcs8` v0.5.1 crate ([#556]) 

[#556]: https://github.com/RustCrypto/traits/pull/556

## 0.9.3 (2021-02-16) [YANKED]
### Changed
- Bump `pkcs8` dependency to v0.5.0 ([#549])

### Fixed
- Workaround for bitvecto-rs/bitvec#105 ([#550])

[#549]: https://github.com/RustCrypto/traits/pull/549
[#550]: https://github.com/RustCrypto/traits/pull/550

## 0.9.2 (2021-02-12) [YANKED]
### Changed
- Flatten `weierstrass` module ([#542])

[#542]: https://github.com/RustCrypto/traits/pull/542

## 0.9.1 (2021-02-11) [YANKED]
### Removed
- `BitView` re-export ([#540])

[#540]: https://github.com/RustCrypto/traits/pull/540

## 0.9.0 (2021-02-10) [YANKED]
### Added
- JWK support ([#483])
- `sec1::ValidatePublicKey` trait ([#485])
- `hazmat` crate feature ([#487])
- `Result` alias ([#534])

### Changed
- Bump `ff` and `group` crates to v0.9 ([#452])
- Simplify ECDH trait bounds ([#475])
- Flatten API ([#487])
- Bump `pkcs8` crate dependency to v0.4 ([#493])

### Removed
- Direct `bitvec` dependency ([#484])
- `FromDigest` trait ([#532])

[#452]: https://github.com/RustCrypto/traits/pull/452
[#475]: https://github.com/RustCrypto/traits/pull/475
[#483]: https://github.com/RustCrypto/traits/pull/483
[#484]: https://github.com/RustCrypto/traits/pull/484
[#485]: https://github.com/RustCrypto/traits/pull/485
[#487]: https://github.com/RustCrypto/traits/pull/487
[#493]: https://github.com/RustCrypto/traits/pull/493
[#432]: https://github.com/RustCrypto/traits/pull/432
[#532]: https://github.com/RustCrypto/traits/pull/532
[#534]: https://github.com/RustCrypto/traits/pull/534

## 0.8.5 (2021-02-17)
### Fixed
- Workaround for bitvecto-rs/bitvec#105 ([#553])

[#553]: https://github.com/RustCrypto/traits/pull/553

## 0.8.4 (2020-12-23)
### Fixed
- Rust `nightly` regression ([#432])

[#432]: https://github.com/RustCrypto/traits/pull/432

## 0.8.3 (2020-12-22)
### Fixed
- Regression in combination of `pem`+`zeroize` features ([#429])

[#429]: https://github.com/RustCrypto/traits/pull/429

## 0.8.2 (2020-12-22) [YANKED]
### Added
- Low-level ECDH API ([#418])
- `dev` module ([#419])
- Impl `pkcs8::ToPrivateKey` for `SecretKey<C>` ([#423])
- Impl `pkcs8::ToPublicKey` for `PublicKey<C>` ([#427])

### Changed
- Bump `subtle` dependency to 2.4.0 ([#414])
- Bump `pkcs8` dependency to v0.3.3 ([#425])
- Use `der` crate to parse `SecretKey` ([#422])

### Fixed
- Make `PublicKey::from_encoded_point` go through `PublicKey::from_affine` ([#416])

[#414]: https://github.com/RustCrypto/traits/pull/414
[#416]: https://github.com/RustCrypto/traits/pull/416
[#418]: https://github.com/RustCrypto/traits/pull/418
[#419]: https://github.com/RustCrypto/traits/pull/419
[#422]: https://github.com/RustCrypto/traits/pull/422
[#423]: https://github.com/RustCrypto/traits/pull/423
[#425]: https://github.com/RustCrypto/traits/pull/425
[#427]: https://github.com/RustCrypto/traits/pull/427

## 0.8.1 (2020-12-16) [YANKED]
### Fixed
- Builds on Rust `nightly` compiler ([#412])

[#412]: https://github.com/RustCrypto/traits/pull/412

## 0.8.0 (2020-12-16) [YANKED]
### Added
- Impl `subtle::ConditionallySelectable` for `sec1::EncodedPoint` ([#409])
- `sec1::EncodedPoint::identity()` method ([#408])
- `sec1::Coordinates::tag` method ([#407])
- Support for SEC1 identity encoding ([#401])

### Changed
- Bump `pkcs8` crate dependency to v0.3 ([#405])
- Ensure `PublicKey<C>` is not the identity point ([#404])
- Have `SecretKey::secret_scalar` return `NonZeroScalar` ([#402])

### Removed
- `SecretKey::secret_value` ([#403])

[#409]: https://github.com/RustCrypto/traits/pull/409
[#408]: https://github.com/RustCrypto/traits/pull/408
[#407]: https://github.com/RustCrypto/traits/pull/407
[#405]: https://github.com/RustCrypto/traits/pull/405
[#404]: https://github.com/RustCrypto/traits/pull/404
[#403]: https://github.com/RustCrypto/traits/pull/403
[#402]: https://github.com/RustCrypto/traits/pull/402
[#401]: https://github.com/RustCrypto/traits/pull/401

## 0.7.1 (2020-12-07)
### Changed
- Have `SecretKey::secret_value` always return `NonZeroScalar` ([#390])

[#390]: https://github.com/RustCrypto/traits/pull/390

## 0.7.0 (2020-12-06) [YANKED]
### Added
- Impl `pkcs8::FromPublicKey` for `PublicKey<C>` ([#385])
- Impl `pkcs8::FromPrivateKey` trait for `SecretKey<C>` ([#381], [#383])
- PKCS#8 PEM support ([#382])
- `SecretKey::secret_value()` method ([#375])
- `PublicKey<C>` type ([#363], [#366])

### Changed
- Rename `PublicKey::from_bytes()` to `::from_sec1_bytes()` ([#376])
- `sec1::EncodedPoint` uses `Option` instead of `subtle::CtOption` ([#367])
- Bump `const-oid` to v0.3; MSRV 1.46+ ([#365], [#381])

### Fixed
- `ecdh` rustdoc ([#364])

[#385]: https://github.com/RustCrypto/traits/pull/385
[#383]: https://github.com/RustCrypto/traits/pull/383
[#382]: https://github.com/RustCrypto/traits/pull/382
[#381]: https://github.com/RustCrypto/traits/pull/381
[#376]: https://github.com/RustCrypto/traits/pull/376
[#375]: https://github.com/RustCrypto/traits/pull/375
[#367]: https://github.com/RustCrypto/traits/pull/367
[#366]: https://github.com/RustCrypto/traits/pull/366
[#365]: https://github.com/RustCrypto/traits/pull/365
[#364]: https://github.com/RustCrypto/traits/pull/364
[#363]: https://github.com/RustCrypto/traits/pull/363

## 0.6.6 (2020-10-08)
### Added
- Derive `Clone` on `SecretBytes` ([#330])

[#300]: https://github.com/RustCrypto/traits/pull/300

## 0.6.5 (2020-10-08)
### Fixed
- Work around `nightly-2020-10-06` breakage ([#328])

[#328]: https://github.com/RustCrypto/traits/pull/328

## 0.6.4 (2020-10-08)
### Added
- Impl `From<SecretBytes<C>>` for `FieldBytes<C>` ([#326])

[#326]: https://github.com/RustCrypto/traits/pull/326

## 0.6.3 (2020-10-08)
### Added
- `SecretBytes` newtype ([#324])

[#324]: https://github.com/RustCrypto/traits/pull/324

## 0.6.2 (2020-09-24)
### Added
- `sec1::EncodedPoint::to_untagged_bytes()` method ([#312])

[#312]: https://github.com/RustCrypto/traits/pull/312

## 0.6.1 (2020-09-21)
### Fixed
- `sec1::EncodedPoint::decompress` ([#309])

[#309]: https://github.com/RustCrypto/traits/pull/309

## 0.6.0 (2020-09-11) [YANKED]
### Added
- `arithmetic` feature ([#293])
- Generic curve/field arithmetic using the `ff` and `group` crates
  ([#287], [#291], [#292])
- `sec1::Coordinates` ([#286])
- `weierstrass::point::Compression` trait ([#283], [#300])
- Arithmetic helper functions ([#281])
- `digest` feature and `FromDigest` trait ([#279])
- impl `Deref` for `NonZeroScalar` ([#278])
- Conditionally impl `Invert` for `NonZeroScalar` ([#277])
- `NonZeroScalar::to_bytes` ([#276])
- `EncodedPoint::decompress` ([#275])
- `sec1::Tag` ([#270])
- `weierstrass::point::Decompress` trait ([#266])
- `alloc` feature + `EncodedPoint::to_bytes()` ([#265])

### Changed
- Renamed `Arithmetic` trait to `point::ProjectiveArithmetic` ([#300])
- Replaced `Arithmetic::Scalar` and `Arithmetic::AffinePoint`
  with `Scalar<C>` and `AffinePoint<C>` ([#300])
- Made `SecretKey<C>` inner type generic ([#297])
- Renamed `ElementBytes<C>` to `FieldBytes<C>` ([#296])
- MSRV 1.44 ([#292])
- Minimum `subtle` version now v2.3 ([#290])
- Renamed `Curve::ElementSize` to `::FieldSize` ([#282])
- Refactor `PublicKey` into `sec1::EncodedPoint` ([#264])

### Removed
- `FromBytes` trait ([#300])
- `Generate` trait ([#295])

[#300]: https://github.com/RustCrypto/traits/pull/300
[#297]: https://github.com/RustCrypto/traits/pull/297
[#296]: https://github.com/RustCrypto/traits/pull/296
[#295]: https://github.com/RustCrypto/traits/pull/295
[#293]: https://github.com/RustCrypto/traits/pull/293
[#292]: https://github.com/RustCrypto/traits/pull/292
[#291]: https://github.com/RustCrypto/traits/pull/291
[#290]: https://github.com/RustCrypto/traits/pull/290
[#287]: https://github.com/RustCrypto/traits/pull/293
[#286]: https://github.com/RustCrypto/traits/pull/286
[#283]: https://github.com/RustCrypto/traits/pull/283
[#282]: https://github.com/RustCrypto/traits/pull/282
[#281]: https://github.com/RustCrypto/traits/pull/281
[#279]: https://github.com/RustCrypto/traits/pull/279
[#278]: https://github.com/RustCrypto/traits/pull/278
[#277]: https://github.com/RustCrypto/traits/pull/277
[#276]: https://github.com/RustCrypto/traits/pull/276
[#275]: https://github.com/RustCrypto/traits/pull/275
[#270]: https://github.com/RustCrypto/traits/pull/270
[#266]: https://github.com/RustCrypto/traits/pull/266
[#265]: https://github.com/RustCrypto/traits/pull/265
[#264]: https://github.com/RustCrypto/traits/pull/264

## 0.5.0 (2020-08-10)
### Added
- `Arithmetic` trait ([#219])
- `Generate` trait ([#220], [#226])
- Toplevel `Curve` trait ([#223])
- `Invert` trait ([#228])
- `FromPublicKey` trait ([#229], [#248])
- Re-export `zeroize` ([#233])
- OID support ([#240], [#245])
- `NonZeroScalar` type ([#241])
- `Generator` trait ([#241])
- `weierstrass::PublicKey::compress` method ([#243])
- Derive `Clone` on `SecretKey` ([#244])
- Generic Elliptic Curve Diffie-Hellman support ([#251])

### Changed
- Moved repo to https://github.com/RustCrypto/traits ([#213])
- Rename `ScalarBytes` to `ElementBytes` ([#246])
- Rename `CompressedCurvePoint`/`UncompressedCurvePoint` to
  `CompressedPoint`/`UncompressedPoint`

[#213]: https://github.com/RustCrypto/traits/pull/213
[#219]: https://github.com/RustCrypto/traits/pull/219
[#220]: https://github.com/RustCrypto/traits/pull/220
[#223]: https://github.com/RustCrypto/traits/pull/223
[#226]: https://github.com/RustCrypto/traits/pull/226
[#228]: https://github.com/RustCrypto/traits/pull/228
[#229]: https://github.com/RustCrypto/traits/pull/229
[#233]: https://github.com/RustCrypto/traits/pull/233
[#240]: https://github.com/RustCrypto/traits/pull/240
[#241]: https://github.com/RustCrypto/traits/pull/241
[#243]: https://github.com/RustCrypto/traits/pull/243
[#244]: https://github.com/RustCrypto/traits/pull/244
[#245]: https://github.com/RustCrypto/traits/pull/245
[#246]: https://github.com/RustCrypto/traits/pull/246
[#248]: https://github.com/RustCrypto/traits/pull/248
[#251]: https://github.com/RustCrypto/traits/pull/251

## 0.4.0 (2020-06-04)
### Changed
- Bump `generic-array` dependency from v0.12 to v0.14

## 0.3.0 (2020-01-15)
### Added
- `Scalar` struct type

### Changed
- Repository moved to <https://github.com/RustCrypto/elliptic-curves>

### Removed
- Curve definitions/arithmetic extracted out into per-curve crates

## 0.2.0 (2019-12-11)
### Added
- `secp256r1` (P-256) point compression and decompression

### Changed
- Bump MSRV to 1.37

## 0.1.0 (2019-12-06)
- Initial release
