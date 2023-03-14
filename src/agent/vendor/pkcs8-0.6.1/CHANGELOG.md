# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.6.1 (2021-05-24)
### Added
- Support for RFC5958's `OneAsymmetricKey` ([#424], [#425])

### Changed
- Bump `der` to v0.3.5 ([#430])

[#424]: https://github.com/RustCrypto/utils/pull/424
[#425]: https://github.com/RustCrypto/utils/pull/425
[#430]: https://github.com/RustCrypto/utils/pull/430

## 0.6.0 (2021-03-22)
### Changed
- Bump `der` dependency to v0.3 ([#354])
- Bump `spki` dependency to v0.3 ([#355])
- Bump `pkcs5` dependency to v0.2 ([#356])

[#354]: https://github.com/RustCrypto/utils/pull/354
[#355]: https://github.com/RustCrypto/utils/pull/355
[#356]: https://github.com/RustCrypto/utils/pull/356

## 0.5.5 (2021-03-17)
### Changed
- Bump `base64ct` dependency to v1.0 ([#335])

[#335]: https://github.com/RustCrypto/utils/pull/335

## 0.5.4 (2021-02-24)
### Added
- Encryption helper methods for `FromPrivateKey`/`ToPrivateKey` ([#308])

[#308]: https://github.com/RustCrypto/utils/pull/308

## 0.5.3 (2021-02-23)
### Added
- Support for decrypting/encrypting `EncryptedPrivateKeyInfo` ([#293], [#302])
- PEM support for `EncryptedPrivateKeyInfo` ([#301])
- `Error::Crypto` variant ([#305])

[#293]: https://github.com/RustCrypto/utils/pull/293
[#301]: https://github.com/RustCrypto/utils/pull/301
[#302]: https://github.com/RustCrypto/utils/pull/302
[#305]: https://github.com/RustCrypto/utils/pull/305

## 0.5.2 (2021-02-20)
### Changed
- Use `pkcs5` crate ([#290])

[#290]: https://github.com/RustCrypto/utils/pull/290

## 0.5.1 (2021-02-18) [YANKED]
### Added
- `pkcs5` feature ([#278])

### Changed
- Bump `spki` dependency to v0.2.0 ([#277])

[#277]: https://github.com/RustCrypto/utils/pull/277
[#278]: https://github.com/RustCrypto/utils/pull/278

## 0.5.0 (2021-02-16) [YANKED]
### Added
- Initial `EncryptedPrivateKeyInfo` support ([#262])

### Changed
- Extract SPKI-related types into the `spki` crate ([#261], [#268])

[#261]: https://github.com/RustCrypto/utils/pull/261
[#262]: https://github.com/RustCrypto/utils/pull/262
[#268]: https://github.com/RustCrypto/utils/pull/268

## 0.4.1 (2021-02-01)
### Changed
- Bump `basec4ct` dependency to v0.2 ([#238], [#243])

[#238]: https://github.com/RustCrypto/utils/pull/238
[#243]: https://github.com/RustCrypto/utils/pull/243

## 0.4.0 (2021-01-26)
### Changed
- Bump `der` crate dependency to v0.2 ([#224])
- Use `base64ct` v0.1 for PEM encoding ([#232])

[#224]: https://github.com/RustCrypto/utils/pull/224
[#232]: https://github.com/RustCrypto/utils/pull/232

## 0.3.3 (2020-12-21)
### Changed
- Use `der` crate for decoding/encoding ASN.1 DER ([#153], [#180])

[#153]: https://github.com/RustCrypto/utils/pull/153
[#180]: https://github.com/RustCrypto/utils/pull/180

## 0.3.2 (2020-12-16)
### Added
- `AlgorithmIdentifier::parameters_oid` method ([#148])

[#148]: https://github.com/RustCrypto/utils/pull/148

## 0.3.1 (2020-12-16)
### Changed
- Bump `const-oid` dependency to v0.4 ([#145])

[#145]: https://github.com/RustCrypto/utils/pull/145

## 0.3.0 (2020-12-16) [YANKED]
### Added
- `AlgorithmParameters` enum ([#138])

[#138]: https://github.com/RustCrypto/utils/pull/138

## 0.2.2 (2020-12-14)
### Fixed
- Decoding/encoding support for Ed25519 keys ([#134], [#135])

[#134]: https://github.com/RustCrypto/utils/pull/134
[#135]: https://github.com/RustCrypto/utils/pull/135

## 0.2.1 (2020-12-14)
### Added
- rustdoc improvements ([#130])

[#130]: https://github.com/RustCrypto/utils/pull/130

## 0.2.0 (2020-12-14)
### Added
- File writing methods for public/private keys ([#126])
- Methods for loading `*Document` types from files ([#125])
- DER encoding support ([#120], [#121])
- PEM encoding support ([#122], [#124])
- `ToPrivateKey`/`ToPublicKey` traits ([#123])

### Changed
- `Error` enum ([#128])
- Rename `load_*_file` methods to `read_*_file` ([#127])

[#128]: https://github.com/RustCrypto/utils/pull/128
[#127]: https://github.com/RustCrypto/utils/pull/127
[#126]: https://github.com/RustCrypto/utils/pull/126
[#125]: https://github.com/RustCrypto/utils/pull/125
[#124]: https://github.com/RustCrypto/utils/pull/124
[#123]: https://github.com/RustCrypto/utils/pull/123
[#122]: https://github.com/RustCrypto/utils/pull/122
[#121]: https://github.com/RustCrypto/utils/pull/121
[#120]: https://github.com/RustCrypto/utils/pull/120

## 0.1.1 (2020-12-06)
### Added
- Helper methods to load keys from the local filesystem ([#115])

[#115]: https://github.com/RustCrypto/utils/pull/115

## 0.1.0 (2020-12-05)
- Initial release
