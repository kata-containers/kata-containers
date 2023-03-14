# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.3.5 (2021-05-24)
### Added
- Helper methods for context-specific fields ([#422], [#423], [#428], [#429])
- `ContextSpecific` field wrapper ([#428])
- Decoder position tracking for errors during `Any<'a>` decoding ([#431])

### Fixed
- `From` conversion for `BitString` into `Any` ([#428])

[#422]: https://github.com/RustCrypto/utils/pull/422
[#423]: https://github.com/RustCrypto/utils/pull/423
[#428]: https://github.com/RustCrypto/utils/pull/428
[#429]: https://github.com/RustCrypto/utils/pull/429
[#431]: https://github.com/RustCrypto/utils/pull/431

## 0.3.4 (2021-05-16)
### Changed
- Support `Length` of up to 1 MiB ([#411])

[#411]: https://github.com/RustCrypto/utils/pull/411

## 0.3.3 (2021-04-15)
### Added
- `Length` constants ([#371])

### Changed
- Deprecate `const fn` methods replaced by `Length` constants ([#371])

[#371]: https://github.com/RustCrypto/utils/pull/371

## 0.3.2 (2021-04-15)
### Fixed
- Non-critical bug allowing `Length` to exceed the max invariant ([#367])

[#367]: https://github.com/RustCrypto/utils/pull/367

## 0.3.1 (2021-04-01) [YANKED]
### Added
- `PartialOrd` + `Ord` impls to all ASN.1 types ([#363])

[#363]: https://github.com/RustCrypto/utils/pull/363

## 0.3.0 (2021-03-22) [YANKED]
### Added
- Impl `Decode`/`Encoded`/`Tagged` for `String` ([#344])
- `Length::one` and `Length::for_tlv` ([#351])
- `SET OF` support with `SetOf` trait and `SetOfRef` ([#346], [#352])

### Changed
- Rename `Decodable::from_bytes` => `Decodable::from_der` ([#339])
- Separate `sequence` and `message` ([#341])
- Rename `ErrorKind::Oid` => `ErrorKind::MalformedOid` ([#342])
- Auto-derive `From` impls for variants when deriving `Choice` ([#345])
- Make `Length` use `u32` internally ([#349])
- Make `Sequence` constructor private ([#348])
- Bump `const_oid` to v0.5 ([#350])
- Bump `der_derive` to v0.3 ([#353])

### Removed
- Deprecated methods ([#340])
- `BigUIntSize` ([#347])

[#339]: https://github.com/RustCrypto/utils/pull/339
[#340]: https://github.com/RustCrypto/utils/pull/340
[#341]: https://github.com/RustCrypto/utils/pull/341
[#342]: https://github.com/RustCrypto/utils/pull/342
[#344]: https://github.com/RustCrypto/utils/pull/344
[#345]: https://github.com/RustCrypto/utils/pull/345
[#346]: https://github.com/RustCrypto/utils/pull/346
[#347]: https://github.com/RustCrypto/utils/pull/347
[#348]: https://github.com/RustCrypto/utils/pull/348
[#349]: https://github.com/RustCrypto/utils/pull/349
[#350]: https://github.com/RustCrypto/utils/pull/350
[#351]: https://github.com/RustCrypto/utils/pull/351
[#352]: https://github.com/RustCrypto/utils/pull/352
[#353]: https://github.com/RustCrypto/utils/pull/353

## 0.2.10 (2021-02-28)
### Added
- Impl `From<ObjectIdentifier>` for `Any` ([#317], [#319])

### Changed
- Bump minimum `const-oid` dependency to v0.4.4 ([#318])

[#317]: https://github.com/RustCrypto/utils/pull/317
[#318]: https://github.com/RustCrypto/utils/pull/318
[#319]: https://github.com/RustCrypto/utils/pull/319

## 0.2.9 (2021-02-24)
### Added
- Support for `IA5String` ([#310])

[#310]: https://github.com/RustCrypto/utils/pull/310

## 0.2.8 (2021-02-22)
### Added
- `Choice` trait ([#295])

[#295]: https://github.com/RustCrypto/utils/pull/295

## 0.2.7 (2021-02-20)
### Added
- Export `Header` publicly ([#283])
- Make `Encoder::reserve` public ([#285])

[#283]: https://github.com/RustCrypto/utils/pull/283
[#285]: https://github.com/RustCrypto/utils/pull/285

## 0.2.6 (2021-02-19)
### Added
- Make the unit type an encoding of `NULL` ([#281])

[#281]: https://github.com/RustCrypto/utils/pull/281

## 0.2.5 (2021-02-18)
### Added
- `ErrorKind::UnknownOid` variant ([#273], [#275])

[#273]: https://github.com/RustCrypto/utils/pull/273
[#275]: https://github.com/RustCrypto/utils/pull/275

## 0.2.4 (2021-02-16)
### Added
- `Any::is_null` method ([#262])

### Changed
- Deprecate `Any::null` method ([#262])

[#262]: https://github.com/RustCrypto/utils/pull/262

## 0.2.3 (2021-02-15)
### Added
- Additional `rustdoc` documentation ([#252], [#256])

[#252]: https://github.com/RustCrypto/utils/pull/252
[#256]: https://github.com/RustCrypto/utils/pull/256

## 0.2.2 (2021-02-12)
### Added
- Support for `UTCTime` and `GeneralizedTime` ([#250])

[#250]: https://github.com/RustCrypto/utils/pull/250

## 0.2.1 (2021-02-02)
### Added
- Support for `PrintableString` and `Utf8String` ([#245])

[#245]: https://github.com/RustCrypto/utils/pull/245

## 0.2.0 (2021-01-22)
### Added
- `BigUInt` type ([#196])
- `i16` support ([#199])
- `u8` and `u16` support ([#210])
- Integer decoder helper methods ([#219])

### Fixed
- Handle leading byte of `BIT STRING`s ([#193])

[#193]: https://github.com/RustCrypto/utils/pull/193
[#196]: https://github.com/RustCrypto/utils/pull/196
[#199]: https://github.com/RustCrypto/utils/pull/199
[#210]: https://github.com/RustCrypto/utils/pull/210
[#219]: https://github.com/RustCrypto/utils/pull/219

## 0.1.0 (2020-12-21)
- Initial release
