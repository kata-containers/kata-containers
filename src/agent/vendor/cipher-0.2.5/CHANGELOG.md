# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.2.5 (2020-11-01)
### Fixed
- Nested macros used old deprecated names ([#360])

[#360]: https://github.com/RustCrypto/traits/pull/360

## 0.2.4 (2020-11-01)
### Fixed
- Macro expansion error ([#358])

[#358]: https://github.com/RustCrypto/traits/pull/358

## 0.2.3 (2020-11-01) [YANKED]
### Fixed
- Legacy macro wrappers ([#356])

[#356]: https://github.com/RustCrypto/traits/pull/356

## 0.2.2 (2020-11-01) [YANKED]
### Added
- `BlockCipher::{encrypt_slice, decrypt_slice}` methods ([#351])

### Changed
- Revamp macro names ([#350])

[#351]: https://github.com/RustCrypto/traits/pull/351
[#350]: https://github.com/RustCrypto/traits/pull/350

## 0.2.1 (2020-10-16)
### Added
- Re-export `generic_array` from toplevel ([#343])

### Fixed
- `dev` macro imports ([#345])

[#343]: https://github.com/RustCrypto/traits/pull/343
[#345]: https://github.com/RustCrypto/traits/pull/345

## 0.2.0 (2020-10-15) [YANKED]
### Changed
- Unify `block-cipher` and `stream-cipher` into `cipher` ([#337])

[#337]: https://github.com/RustCrypto/traits/pull/337

## 0.1.1 (2015-06-25)

## 0.1.0 (2015-06-24)
- Initial release
