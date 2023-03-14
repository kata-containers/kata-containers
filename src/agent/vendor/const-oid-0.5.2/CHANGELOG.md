# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.5.2 (2021-04-20)
### Added
- Expand README.md ([#376])

[#376]: https://github.com/RustCrypto/utils/pull/376

## 0.5.1 (2021-04-15)
### Added
- `ObjectIdentifier::MAX_LENGTH` constant ([#372])

### Changed
- Deprecate `ObjectIdentifier::max_len()` function ([#372])

[#372]: https://github.com/RustCrypto/utils/pull/372

## 0.5.0 (2021-03-21)
### Added
- `TryFrom<&[u8]>` impl on `ObjectIdentifier` ([#338])

## Changed
- MSRV 1.47+ ([#338])
- Renamed the following methods ([#338]):
  - `ObjectIdentifier::new` => `ObjectIdentifier::from_arcs`
  - `ObjectIdentifier::parse` => `ObjectIdentifier::new`
  - `ObjectIdentifier::from_ber` => `ObjectIdentifier::from_bytes`

### Removed
- Deprecated methods ([#338])
- `alloc` feature - only used by aforementioned deprecated methods ([#338])
- `TryFrom<&[Arc]>` impl on `ObjectIdentifier` - use `::from_arcs` ([#338])

[#338]: https://github.com/RustCrypto/utils/pull/338

## 0.4.5 (2021-03-04)
### Added
- `Hash` and `Ord` impls on `ObjectIdentifier` ([#323])

[#323]: https://github.com/RustCrypto/utils/pull/323

## 0.4.4 (2021-02-28)
### Added
- `ObjectIdentifier::as_bytes` method ([#317])

### Changed
- Internal representation changed to BER/DER ([#317])
- Deprecated `ObjectIdentifier::ber_len`, `::write_ber`, and `::to_ber` ([#317])

[#317]: https://github.com/RustCrypto/utils/pull/317

## 0.4.3 (2021-02-24)
### Added
- Const-friendly OID string parser ([#312])

[#312]: https://github.com/RustCrypto/utils/pull/312

## 0.4.2 (2021-02-19)
### Fixed
- Bug in root arc calculation ([#284])

[#284]: https://github.com/RustCrypto/utils/pull/284

## 0.4.1 (2020-12-21)
### Fixed
- Bug in const initializer ([#172])

[#172]: https://github.com/RustCrypto/utils/pull/172

## 0.4.0 (2020-12-16)
### Added
- `Arcs` iterator ([#141], [#142])

### Changed
- Rename "nodes" to "arcs" ([#142])
- Layout optimization ([#143])
- Refactor and improve length limits ([#144])

[#144]: https://github.com/RustCrypto/utils/pull/144
[#143]: https://github.com/RustCrypto/utils/pull/143
[#142]: https://github.com/RustCrypto/utils/pull/142
[#141]: https://github.com/RustCrypto/utils/pull/141

## 0.3.5 (2020-12-12)
### Added
- `ObjectIdentifier::{write_ber, to_ber}` methods ([#118])

[#118]: https://github.com/RustCrypto/utils/pull/118

## 0.3.4 (2020-12-06)
### Changed
- Documentation improvements ([#112])

[#112]: https://github.com/RustCrypto/utils/pull/110

## 0.3.3 (2020-12-05)
### Changed
- Improve description in Cargo.toml/README.md (#110)

[#110]: https://github.com/RustCrypto/utils/pull/110

## 0.3.2 (2020-12-05)
### Changed
- Documentation improvements ([#107])

[#107]: https://github.com/RustCrypto/utils/pull/107

## 0.3.1 (2020-12-05)
### Added
- Impl `TryFrom<&[u32]>` for ObjectIdentifier ([#105])

[#105]: https://github.com/RustCrypto/utils/pull/105

## 0.3.0 (2020-12-05) [YANKED]
### Added
- Byte and string parsers ([#89])

[#89]: https://github.com/RustCrypto/utils/pull/89

## 0.2.0 (2020-09-05)
### Changed
- Validate OIDs are well-formed; MSRV 1.46+ ([#76])

[#76]: https://github.com/RustCrypto/utils/pull/76

## 0.1.0 (2020-08-04)
- Initial release
