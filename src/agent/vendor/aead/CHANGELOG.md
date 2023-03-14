# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.3.2 (2020-07-01)
### Added
- `dev` module ([#194])

[#194]: https://github.com/RustCrypto/traits/pull/194

## 0.3.1 (2020-06-12)
### Added
- `NewAead::new_varkey` method ([#191])

[#191]: https://github.com/RustCrypto/traits/pull/191

## 0.3.0 (2020-06-04)
### Added
- Type aliases for `Key`, `Nonce`, and `Tag` ([#125])
- Optional `std` feature ([#63])

### Changed
- `NewAead` now borrows the key ([#124])
- Split `Aead`/`AeadMut` into `AeadInPlace`/`AeadMutInPlace` ([#120])
- Bump `generic-array` dependency to v0.14 ([#95])

[#125]: https://github.com/RustCrypto/traits/pull/125
[#124]: https://github.com/RustCrypto/traits/pull/124
[#120]: https://github.com/RustCrypto/traits/pull/120
[#95]: https://github.com/RustCrypto/traits/pull/95
[#63]: https://github.com/RustCrypto/traits/pull/63

## 0.2.0 (2019-11-17)

## 0.1.2 (2019-11-17) [YANKED]

## 0.1.1 (2019-08-30)

## 0.1.0 (2019-08-29)
