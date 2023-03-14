# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.5.1 (2021-11-17)
### Added
- `Any::NULL` constant ([#226])

[#226]: https://github.com/RustCrypto/formats/pull/226

## 0.5.0 (2021-11-15) [YANKED]
### Added
- Support for `IMPLICIT` mode `CONTEXT-SPECIFIC` fields ([#61])
- `DecodeValue`/`EncodeValue` traits ([#63])
- Expose `DateTime` through public API ([#75])
- `SEQUENCE OF` support for `[T; N]` ([#90])
- `SequenceOf` type ([#95])
- `SEQUENCE OF` support for `Vec` ([#96])
- `Document` trait ([#117])
- Basic integration with `time` crate ([#129])
- `Tag::NumericString` ([#132])
- Support for unused bits to `BitString` ([#141])
- `Decoder::{peek_tag, peek_header}` ([#142])
- Type hint in `encoder `sequence` method ([#147])
- `Tag::Enumerated` ([#153])
- `ErrorKind::TagNumberInvalid` ([#156])
- `Tag::VisibleString` and `Tag::BmpString` ([#160])
- Inherent constants for all valid `TagNumber`s ([#165])
- `DerOrd` and `ValueOrd` traits ([#190])
- `ContextSpecificRef` type ([#199])

### Changed
- Make `ContextSpecific` generic around an inner type ([#60])
- Removed `SetOf` trait; rename `SetOfArray` => `SetOf` ([#97])
- Rename `Message` trait to `Sequence` ([#99])
- Make `GeneralizedTime`/`UtcTime` into `DateTime` newtypes ([#102])
- Rust 2021 edition upgrade; MSRV 1.56 ([#136])
- Replace `ErrorKind::Truncated` with `ErrorKind::Incomplete` ([#143])
- Rename `ErrorKind::UnknownTagMode` => `ErrorKind::TagModeUnknown` ([#155])
- Rename `ErrorKind::UnexpectedTag` => `ErrorKind::TagUnexpected` ([#155])
- Rename `ErrorKind::UnknownTag` => `ErrorKind::TagUnknown` ([#155])
- Consolidate `ErrorKind::{Incomplete, Underlength}` ([#157])
- Rename `Tagged` => `FixedTag`; add new `Tagged` trait ([#189])
- Use `DerOrd` for `SetOf*` types ([#200])
- Switch `impl From<BitString> for &[u8]` to `TryFrom` ([#203])
- Bump `crypto-bigint` dependency to v0.3 ([#215])
- Bump `const-oid` dependency to v0.7 ([#216])
- Bump `pem-rfc7468` dependency to v0.3 ([#217])
- Bump `der_derive` dependency to v0.5 ([#221])

### Removed
- `Sequence` struct ([#98])
- `Tagged` bound on `ContextSpecific::decode_implicit` ([#161])
- `ErrorKind::DuplicateField` ([#162])

[#60]: https://github.com/RustCrypto/formats/pull/60
[#61]: https://github.com/RustCrypto/formats/pull/61
[#63]: https://github.com/RustCrypto/formats/pull/63
[#75]: https://github.com/RustCrypto/formats/pull/75
[#90]: https://github.com/RustCrypto/formats/pull/90
[#95]: https://github.com/RustCrypto/formats/pull/95
[#96]: https://github.com/RustCrypto/formats/pull/96
[#97]: https://github.com/RustCrypto/formats/pull/97
[#98]: https://github.com/RustCrypto/formats/pull/98
[#99]: https://github.com/RustCrypto/formats/pull/99
[#102]: https://github.com/RustCrypto/formats/pull/102
[#117]: https://github.com/RustCrypto/formats/pull/117
[#129]: https://github.com/RustCrypto/formats/pull/129
[#132]: https://github.com/RustCrypto/formats/pull/132
[#136]: https://github.com/RustCrypto/formats/pull/136
[#141]: https://github.com/RustCrypto/formats/pull/141
[#142]: https://github.com/RustCrypto/formats/pull/142
[#143]: https://github.com/RustCrypto/formats/pull/143
[#147]: https://github.com/RustCrypto/formats/pull/147
[#153]: https://github.com/RustCrypto/formats/pull/153
[#155]: https://github.com/RustCrypto/formats/pull/155
[#156]: https://github.com/RustCrypto/formats/pull/156
[#157]: https://github.com/RustCrypto/formats/pull/157
[#160]: https://github.com/RustCrypto/formats/pull/160
[#161]: https://github.com/RustCrypto/formats/pull/161
[#162]: https://github.com/RustCrypto/formats/pull/162
[#165]: https://github.com/RustCrypto/formats/pull/165
[#189]: https://github.com/RustCrypto/formats/pull/189
[#190]: https://github.com/RustCrypto/formats/pull/190
[#199]: https://github.com/RustCrypto/formats/pull/199
[#200]: https://github.com/RustCrypto/formats/pull/200
[#203]: https://github.com/RustCrypto/formats/pull/203
[#215]: https://github.com/RustCrypto/formats/pull/215
[#216]: https://github.com/RustCrypto/formats/pull/216
[#217]: https://github.com/RustCrypto/formats/pull/217
[#221]: https://github.com/RustCrypto/formats/pull/221

## 0.4.4 (2021-10-06)
### Removed
- Accidentally checked-in `target/` directory ([#66])

[#66]: https://github.com/RustCrypto/formats/pull/66

## 0.4.3 (2021-09-15)
### Added
- `Tag::unexpected_error` ([#33])

[#33]: https://github.com/RustCrypto/formats/pull/33

## 0.4.2 (2021-09-14)
### Changed
- Moved to `formats` repo ([#2])

### Fixed
- ASN.1 `SET` type now flagged with the constructed bit

[#2]: https://github.com/RustCrypto/formats/pull/2

## 0.4.1 (2021-08-08)
### Fixed
- Encoding `UTCTime` for dates with `20xx` years

## 0.4.0 (2021-06-07)
### Added
- `TagNumber` type
- Const generic integer de/encoders with support for all of Rust's integer
  primitives
- `crypto-bigint` support
- `Tag` number helpers
- `Tag::octet`
- `ErrorKind::Value` helpers
- `SequenceIter`

### Changed
- Bump `const-oid` crate dependency to v0.6
- Make `Tag` structured
- Namespace ASN.1 types in `asn1` module
- Refactor context-specific field decoding
- MSRV 1.51
- Rename `big-uint` crate feature to `bigint`
- Rename `BigUInt` to `UIntBytes`
- Have `Decoder::error()` return an `Error`
  
### Removed
- Deprecated methods replaced by associated constants

## 0.3.5 (2021-05-24)
### Added
- Helper methods for context-specific fields
- `ContextSpecific` field wrapper
- Decoder position tracking for errors during `Any<'a>` decoding

### Fixed
- `From` conversion for `BitString` into `Any`

## 0.3.4 (2021-05-16)
### Changed
- Support `Length` of up to 1 MiB

## 0.3.3 (2021-04-15)
### Added
- `Length` constants

### Changed
- Deprecate `const fn` methods replaced by `Length` constants

## 0.3.2 (2021-04-15)
### Fixed
- Non-critical bug allowing `Length` to exceed the max invariant

## 0.3.1 (2021-04-01) [YANKED]
### Added
- `PartialOrd` + `Ord` impls to all ASN.1 types

## 0.3.0 (2021-03-22) [YANKED]
### Added
- Impl `Decode`/`Encoded`/`Tagged` for `String`
- `Length::one` and `Length::for_tlv`
- `SET OF` support with `SetOf` trait and `SetOfRef`

### Changed
- Rename `Decodable::from_bytes` => `Decodable::from_der`
- Separate `sequence` and `message`
- Rename `ErrorKind::Oid` => `ErrorKind::MalformedOid`
- Auto-derive `From` impls for variants when deriving `Choice`
- Make `Length` use `u32` internally
- Make `Sequence` constructor private
- Bump `const_oid` to v0.5
- Bump `der_derive` to v0.3

### Removed
- Deprecated methods
- `BigUIntSize`

## 0.2.10 (2021-02-28)
### Added
- Impl `From<ObjectIdentifier>` for `Any`

### Changed
- Bump minimum `const-oid` dependency to v0.4.4

## 0.2.9 (2021-02-24)
### Added
- Support for `IA5String`

## 0.2.8 (2021-02-22)
### Added
- `Choice` trait

## 0.2.7 (2021-02-20)
### Added
- Export `Header` publicly
- Make `Encoder::reserve` public

## 0.2.6 (2021-02-19)
### Added
- Make the unit type an encoding of `NULL`

## 0.2.5 (2021-02-18)
### Added
- `ErrorKind::UnknownOid` variant

## 0.2.4 (2021-02-16)
### Added
- `Any::is_null` method

### Changed
- Deprecate `Any::null` method

## 0.2.3 (2021-02-15)
### Added
- Additional `rustdoc` documentation

## 0.2.2 (2021-02-12)
### Added
- Support for `UTCTime` and `GeneralizedTime`

## 0.2.1 (2021-02-02)
### Added
- Support for `PrintableString` and `Utf8String`

## 0.2.0 (2021-01-22)
### Added
- `BigUInt` type
- `i16` support
- `u8` and `u16` support
- Integer decoder helper methods

### Fixed
- Handle leading byte of `BIT STRING`s

## 0.1.0 (2020-12-21)
- Initial release
