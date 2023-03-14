# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.3.3 (2022-01-16)
### Added
- Error conversion support to `pkcs8::spki::Error` ([#333])

[#333]: https://github.com/RustCrypto/formats/pull/331

## 0.3.2 (2022-01-16)
### Added
- Error conversion support to `pkcs8::Error` ([#331])

[#331]: https://github.com/RustCrypto/formats/pull/331

## 0.3.1 (2021-11-29)
### Changed
- Use `finish_non_exhaustive` in Debug impls ([#245])

[#245]: https://github.com/RustCrypto/formats/pull/245

## 0.3.0 (2021-11-17)
### Added
- Support for multi-prime RSA keys ([#115])
- `pkcs8` feature ([#227], [#233])

### Changed
- Rename `From/ToRsa*Key` => `DecodeRsa*Key`/`EncodeRsa*Key` ([#120])
- Use `der::Document` to impl `RsaPrivateKeyDocument` ([#131])
- Rust 2021 edition upgrade; MSRV 1.56 ([#136])
- Make `RsaPrivateKey::version` implicit ([#188])
- Bump `der` crate dependency to v0.5 ([#222])
- Activate `pkcs8/pem` when `pem` feature is enabled ([#232])

### Removed
- `*_with_le` PEM encoding methods ([#109])
- I/O related errors ([#158])

[#109]: https://github.com/RustCrypto/formats/pull/109
[#115]: https://github.com/RustCrypto/formats/pull/115
[#120]: https://github.com/RustCrypto/formats/pull/120
[#131]: https://github.com/RustCrypto/formats/pull/131
[#136]: https://github.com/RustCrypto/formats/pull/136
[#158]: https://github.com/RustCrypto/formats/pull/158
[#188]: https://github.com/RustCrypto/formats/pull/188
[#222]: https://github.com/RustCrypto/formats/pull/222
[#227]: https://github.com/RustCrypto/formats/pull/227
[#232]: https://github.com/RustCrypto/formats/pull/232
[#233]: https://github.com/RustCrypto/formats/pull/233

## 0.2.4 (2021-09-14)
### Changed
- Moved to `formats` repo ([#2])

[#2]: https://github.com/RustCrypto/formats/pull/2

## 0.2.3 (2021-07-26)
### Added
- Support for customizing PEM `LineEnding`

### Changed
- Bump `pem-rfc7468` dependency to v0.2

## 0.2.2 (2021-07-25)
### Fixed
- `Version` encoder

## 0.2.1 (2021-07-25)
### Added
- `Error::Crypto` variant

## 0.2.0 (2021-07-25)
### Added
- `From*`/`To*` traits for `RsaPrivateKey`/`RsaPublicKey`

### Changed
- Use `FromRsa*`/`ToRsa*` traits with `*Document` types

## 0.1.1 (2021-07-24)
### Added
- Re-export `der` crate and `der::UIntBytes`

### Changed
- Replace `Error::{Decode, Encode}` with `Error::Asn1`

## 0.1.0 (2021-07-24) [YANKED]
- Initial release
