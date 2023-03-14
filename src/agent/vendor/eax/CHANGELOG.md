# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.3.0 (2020-10-16)
### Changed
- Replace `block-cipher`/`stream-cipher` with `cipher` crate ([#229])

[#229]: https://github.com/RustCrypto/AEADs/pull/229

## 0.2.0 (2020-09-30
### Added
- API for online encryption/decryption ([#214])
- Optional `std` feature; disabled by default ([#217])

### Changed
- Use `aead` crate; MSRV 1.41+
- Upgrade `aes` to v0.5, `block-cipher` to v0.8, `cmac` to v0.4, `ctr` to v0.5 ([#209])

[#217]: https://github.com/RustCrypto/AEADs/pull/217
[#214]: https://github.com/RustCrypto/AEADs/pull/214
[#209]: https://github.com/RustCrypto/AEADs/pull/209

## 0.1.0 (2019-03-29)
- Initial release
