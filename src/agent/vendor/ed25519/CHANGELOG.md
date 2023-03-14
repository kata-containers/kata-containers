# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 1.5.2 (2022-05-16)
### Fixed
- Overflow handling in `serde` deserializers ([#482])

[#482]: https://github.com/RustCrypto/signatures/pull/482

## 1.5.1 (2022-05-15)
### Fixed
- Use `TryInto` in `serde` deserializers ([#479])

[#479]: https://github.com/RustCrypto/signatures/pull/479

## 1.5.0 (2022-05-09)
### Changed
- Bump `pkcs8` dependency to v0.9 ([#473])

[#473]: https://github.com/RustCrypto/signatures/pull/473

## 1.4.1 (2022-03-18)
### Added
- License files ([#447])
- `pkcs8::PublicKeyBytes` type ([#455])

[#447]: https://github.com/RustCrypto/signatures/pull/447
[#455]: https://github.com/RustCrypto/signatures/pull/455

## 1.4.0 (2022-02-25) [YANKED]

This crate now requires **Rust 1.56** at a minimum as the Rust edition has been
upgraded to 2021.

Previous 1.x releases of this crate supported an MSRV of 1.47. If you would
like to use this crate with earlier releases of Rust, add the following version
constraint in your project's Cargo.toml to constrain it to the supported
version range:

```toml
[dependencies]
ed25519 = ">=1, <1.4" # ed25519 1.4 requires MSRV 1.56
```

Note that is our policy that we may change the MSRV in the future, but it will
be accompanied by a minor version bump.

### Added
- `Signature::to_vec` ([#428])

### Changed
- Rust 2021 edition upgrade ([#412])

[#412]: https://github.com/RustCrypto/signatures/pull/412
[#428]: https://github.com/RustCrypto/signatures/pull/428

## 1.3.0 (2021-11-18)
### Added
- `Signature::BYTE_SIZE` constant ([#380])
- PKCS#8 support via `KeypairBytes` type ([#381])
- `zeroize` feature ([#400])
- Impl `Display`/`LowerHex`/`UpperHex`/`FromStr` for `Signature` ([#402])

### Changed
- Deprecate `SIGNATURE_LENGTH` constant in favor of `Signature::BYTE_SIZE` ([#380])
- Deprecate `Signature::new` in favor of `Signature::from_bytes`/`TryFrom` ([#401])
- `Signature::new` now panics on invalid signatures ([#403])

[#380]: https://github.com/RustCrypto/signatures/pull/380
[#381]: https://github.com/RustCrypto/signatures/pull/381
[#400]: https://github.com/RustCrypto/signatures/pull/400
[#401]: https://github.com/RustCrypto/signatures/pull/401
[#402]: https://github.com/RustCrypto/signatures/pull/402
[#403]: https://github.com/RustCrypto/signatures/pull/403

## 1.2.0 (2021-07-21)
### Added
- `serde_bytes` optional dependency ([#337])

[#337]: https://github.com/RustCrypto/signatures/pull/337

## 1.1.1 (2021-04-30)
### Changed
- Updates for `ring-compat` v0.2.1 ([#291])

[#291]: https://github.com/RustCrypto/signatures/pull/291

## 1.1.0 (2021-04-30)
### Changed
- Bump `ring-compat` to v0.2; MSRV 1.47+ ([#289])

### Fixed
- Compile error in example ([#246])

[#246]: https://github.com/RustCrypto/signatures/pull/246
[#289]: https://github.com/RustCrypto/signatures/pull/289

## 1.0.3 (2020-10-12)
### Added
- `ring-compat` usage example ([#187])

[#187]: https://github.com/RustCrypto/signatures/pull/187

## 1.0.2 (2020-09-11)
### Added
- `ed25519-dalek` usage example ([#167])

[#167]: https://github.com/RustCrypto/signatures/pull/167

## 1.0.1 (2020-04-20)
### Added
- Usage documentation ([#83])

[#83]: https://github.com/RustCrypto/signatures/pull/83

## 1.0.0 (2020-04-18)
### Changed
- Upgrade `signature` crate to v1.0 final release ([#80])

[#80]: https://github.com/RustCrypto/signatures/pull/80

## 1.0.0-pre.4 (2020-03-17)
### Changed
- Avoid serializing a length prefix with `serde` ([#78])

[#78]: https://github.com/RustCrypto/signatures/pull/78

## 1.0.0-pre.3 (2020-03-16)
### Changed
- Upgrade `signature` crate to v1.0.0-pre.3 ([#74])
- Bump MSRV to 1.40 ([#75])

[#74]: https://github.com/RustCrypto/signatures/pull/74
[#75]: https://github.com/RustCrypto/signatures/pull/75

## 1.0.0-pre.2 (2020-03-08)
### Changed
- Upgrade to `signature` crate v1.0.0-pre.3 ([#71])
- Bump MSRV to 1.37 ([#63])

[#71]: https://github.com/RustCrypto/signatures/pull/71
[#63]: https://github.com/RustCrypto/signatures/pull/63

## 1.0.0-pre.1 (2019-10-11)
### Added
- Optional `serde` support ([#40])
- Add `TryFrom` impl for `Signature` ([#39])

### Changed
- Upgrade to `signature` 1.0.0-pre.1 ([#41])

[#41]: https://github.com/RustCrypto/signatures/pull/41
[#40]: https://github.com/RustCrypto/signatures/pull/40
[#39]: https://github.com/RustCrypto/signatures/pull/39

## 1.0.0-pre.0 (2019-10-11)
### Changed
- Upgrade to `signature` 1.0.0-pre.0 ([#34])

[#34]: https://github.com/RustCrypto/signatures/pull/34

## 0.2.0 (2019-10-10)
### Changed
- Upgrade to `signature` v0.3; MSRV 1.36+ ([#29])

[#29]: https://github.com/RustCrypto/signatures/pull/29

## 0.1.0 (2019-08-10)

- Initial release
