# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.7.1 (2021-11-30)
### Changed
- Increase `MAX_SIZE` to 39 ([#258])

[#258]: https://github.com/RustCrypto/formats/pull/258

## 0.7.0 (2021-11-14) [YANKED]
### Changed
- Rust 2021 edition upgrade; MSRV 1.56 ([#136])
- Rename `MAX_LENGTH` to `MAX_SIZE`; bump to `31` ([#174])
- Make `length` the first field of `ObjectIdentifier` ([#178])

### Fixed
- `debug_assert!` false positive on large arc ([#180])

[#136]: https://github.com/RustCrypto/formats/pull/136
[#174]: https://github.com/RustCrypto/formats/pull/174
[#178]: https://github.com/RustCrypto/formats/pull/178
[#180]: https://github.com/RustCrypto/formats/pull/180

## 0.6.2 (2021-10-14)
### Fixed
- Off-by-one error parsing large BER arcs ([#84])

[#84]: https://github.com/RustCrypto/formats/pull/84

## 0.6.1 (2021-09-14) [YANKED]
### Changed
- Moved to `formats` repo ([#2])

[#2]: https://github.com/RustCrypto/formats/pull/2

## 0.6.0 (2021-06-03) [YANKED]
### Changed
- Modernize and remove deprecations; MSRV 1.51+

## 0.5.2 (2021-04-20)
### Added
- Expand README.md

## 0.5.1 (2021-04-15)
### Added
- `ObjectIdentifier::MAX_LENGTH` constant

### Changed
- Deprecate `ObjectIdentifier::max_len()` function

## 0.5.0 (2021-03-21)
### Added
- `TryFrom<&[u8]>` impl on `ObjectIdentifier`

## Changed
- MSRV 1.47+
- Renamed the following methods:
  - `ObjectIdentifier::new` => `ObjectIdentifier::from_arcs`
  - `ObjectIdentifier::parse` => `ObjectIdentifier::new`
  - `ObjectIdentifier::from_ber` => `ObjectIdentifier::from_bytes`

### Removed
- Deprecated methods
- `alloc` feature - only used by aforementioned deprecated methods
- `TryFrom<&[Arc]>` impl on `ObjectIdentifier` - use `::from_arcs`

## 0.4.5 (2021-03-04)
### Added
- `Hash` and `Ord` impls on `ObjectIdentifier`

## 0.4.4 (2021-02-28)
### Added
- `ObjectIdentifier::as_bytes` method

### Changed
- Internal representation changed to BER/DER
- Deprecated `ObjectIdentifier::ber_len`, `::write_ber`, and `::to_ber`

## 0.4.3 (2021-02-24)
### Added
- Const-friendly OID string parser

## 0.4.2 (2021-02-19)
### Fixed
- Bug in root arc calculation

## 0.4.1 (2020-12-21)
### Fixed
- Bug in const initializer

## 0.4.0 (2020-12-16)
### Added
- `Arcs` iterator

### Changed
- Rename "nodes" to "arcs"
- Layout optimization
- Refactor and improve length limits

## 0.3.5 (2020-12-12)
### Added
- `ObjectIdentifier::{write_ber, to_ber}` methods

## 0.3.4 (2020-12-06)
### Changed
- Documentation improvements

## 0.3.3 (2020-12-05)
### Changed
- Improve description in Cargo.toml/README.md

## 0.3.2 (2020-12-05)
### Changed
- Documentation improvements

## 0.3.1 (2020-12-05)
### Added
- Impl `TryFrom<&[u32]>` for ObjectIdentifier

## 0.3.0 (2020-12-05) [YANKED]
### Added
- Byte and string parsers

## 0.2.0 (2020-09-05)
### Changed
- Validate OIDs are well-formed; MSRV 1.46+

## 0.1.0 (2020-08-04)
- Initial release
