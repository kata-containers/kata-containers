# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.10.0 (2020-10-16)
### Changed
- Replace `block-cipher`/`stream-cipher` with `cipher` crate ([#167])

[#167]: https://github.com/RustCrypto/block-ciphers/pull/167

## 0.9.0 (2020-08-25)
### Changed
- Bump `stream-cipher` dependency to v0.7 ([#158])

### Fixed
- Incorrect values returned by the `SyncStreamCipherSeek::current_pos` method  ([#71])

[#71]: https://github.com/RustCrypto/block-ciphers/issues/71
[#158]: https://github.com/RustCrypto/block-ciphers/pull/158

## 0.8.0 (2020-08-07)
### Changed
- Bump `block-cipher` dependency to v0.8 and `stream-cipher` to v0.6 ([#138])
- Bump `opaque-debug` dependency to v0.3 ([#140])

[#138]: https://github.com/RustCrypto/block-ciphers/pull/138
[#140]: https://github.com/RustCrypto/block-ciphers/pull/140

## 0.7.0 (2020-06-05)
### Added
- Impl `FromBlockCipher` for AES-CTR types ([#121])

### Changed
- Bump `block-cipher` dependency to v0.7 ([#86], [#122])
- Update to Rust 2018 edition ([#86])
- Use `mem::zeroed` instead of `mem::uninitialized` on XMM registers ([#109], [#110])

[#122]: https://github.com/RustCrypto/block-ciphers/pull/122
[#121]: https://github.com/RustCrypto/block-ciphers/pull/121
[#110]: https://github.com/RustCrypto/block-ciphers/pull/110
[#109]: https://github.com/RustCrypto/block-ciphers/pull/109
[#86]: https://github.com/RustCrypto/block-ciphers/pull/86

## 0.6.0 (2018-11-01)

## 0.5.1 (2018-10-04)

## 0.5.0 (2018-10-03)

## 0.4.1 (2018-08-07)

## 0.4.0 (2018-07-27)

## 0.3.5 (2018-06-22)

## 0.3.4 (2018-06-13)

## 0.3.3 (2018-06-13)

## 0.3.2 (2018-06-13)

## 0.3.1 (2018-03-06)

## 0.3.0 (2018-03-06)

## 0.2.2 (2018-03-06)

## 0.2.1 (2017-12-01)

## 0.2.0 (2017-11-26)

## 0.1.4 (2017-08-06)

## 0.1.3 (2017-08-06)

## 0.1.2 (2017-08-02)

## 0.1.1 (2017-07-31)

## 0.1.0 (2017-07-21)~~~~
