# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.6.4 (2020-11-16)
### Changed
- Rework of xor_columns ([#197])
- Implement semi-fixsliced support under `semi_fixslice` Cargo feature ([#195])

[#197]: https://github.com/RustCrypto/block-ciphers/pull/197
[#195]: https://github.com/RustCrypto/block-ciphers/pull/195

## 0.6.3 (2020-11-01)
### Changed
- Comprehensive refactoring of fixslice code ([#192])
- Forbid `unsafe` ([#190])
- Re-order (`inv`)`_sbox` using custom scheduler ([#189])

[#192]: https://github.com/RustCrypto/block-ciphers/pull/192
[#190]: https://github.com/RustCrypto/block-ciphers/pull/190
[#189]: https://github.com/RustCrypto/block-ciphers/pull/189

## 0.6.2 (2020-10-28)
### Added
- 64-bit fixsliced AES implementation ([#180])

### Changed
- Fixsliced AES decryption ([#185])
- Improved AES fixsliced MixColumns algorithms ([#184])

[#185]: https://github.com/RustCrypto/block-ciphers/pull/185
[#184]: https://github.com/RustCrypto/block-ciphers/pull/184
[#180]: https://github.com/RustCrypto/block-ciphers/pull/180

## 0.6.1 (2020-10-26)
### Changed
- Use fixslicing for AES encryption - 3X performance boost ([#174], [#176], [#177])
- Additional bitslicing performance optimizations ([#171], [#175])

[#177]: https://github.com/RustCrypto/block-ciphers/pull/177
[#176]: https://github.com/RustCrypto/block-ciphers/pull/176
[#175]: https://github.com/RustCrypto/block-ciphers/pull/175
[#174]: https://github.com/RustCrypto/block-ciphers/pull/174
[#171]: https://github.com/RustCrypto/block-ciphers/pull/171

## 0.6.0 (2020-10-16)
### Changed
- Replace `block-cipher`/`stream-cipher` with `cipher` crate ([#167])
- Performance improvements ([#166])

[#167]: https://github.com/RustCrypto/block-ciphers/pull/167
[#166]: https://github.com/RustCrypto/block-ciphers/pull/166

## 0.5.0 (2020-08-07)
### Changed
- Bump `block-cipher` dependency to v0.8 ([#138])
- Bump `opaque-debug` dependency to v0.3 ([#140])

[#138]: https://github.com/RustCrypto/block-ciphers/pull/138
[#140]: https://github.com/RustCrypto/block-ciphers/pull/140

## 0.4.0 (2020-06-05)
### Changed
- Bump `block-cipher` dependency to v0.7 ([#86], [#122])
- Update to Rust 2018 edition ([#86])
 
[#122]: https://github.com/RustCrypto/block-ciphers/pull/122
[#86]: https://github.com/RustCrypto/block-ciphers/pull/86

## 0.3.3 (2018-12-23)

## 0.3.2 (2018-10-04)

## 0.3.1 (2018-10-03)

## 0.3.0 (2018-10-03)

## 0.2.0 (2018-07-27)

## 0.1.0 (2018-03-04)
