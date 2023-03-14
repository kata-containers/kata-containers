# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.3.2 (2021-11-17)
### Added
- `Output = Self` to all bitwise ops on `Integer` trait ([#53])

[#53]: https://github.com/RustCrypto/crypto-bigint/pull/53

## 0.3.1 (2021-11-17)
### Added
- Bitwise ops to `Integer` trait ([#51])

[#51]: https://github.com/RustCrypto/crypto-bigint/pull/51

## 0.3.0 (2021-11-14) [YANKED]
### Added
- Bitwise `Xor`/`Not` operations ([#27])
- `Zero` trait ([#35])
- `Checked*` traits ([#41])
- `prelude` module ([#45])
- `saturating_*` ops ([#47])

### Changed
- Rust 2021 edition upgrade; MSRV 1.56 ([#33])
- Reverse ordering of `UInt::mul_wide` return tuple ([#34])
- Have `Div` and `Rem` impls always take `NonZero` args ([#39])
- Rename `limb::Inner` to `LimbUInt` ([#40])
- Make `limb` module private ([#40])
- Use `Zero`/`Integer` traits for `is_zero`, `is_odd`, and `is_even` ([#46])

### Fixed
- `random_mod` performance for small moduli ([#36])
- `NonZero` moduli ([#36])

### Removed
- Deprecated `LIMB_BYTES` constant ([#43])

[#27]: https://github.com/RustCrypto/crypto-bigint/pull/27
[#33]: https://github.com/RustCrypto/crypto-bigint/pull/33
[#34]: https://github.com/RustCrypto/crypto-bigint/pull/34
[#35]: https://github.com/RustCrypto/crypto-bigint/pull/35
[#36]: https://github.com/RustCrypto/crypto-bigint/pull/36
[#39]: https://github.com/RustCrypto/crypto-bigint/pull/39
[#40]: https://github.com/RustCrypto/crypto-bigint/pull/40
[#41]: https://github.com/RustCrypto/crypto-bigint/pull/41
[#43]: https://github.com/RustCrypto/crypto-bigint/pull/43
[#45]: https://github.com/RustCrypto/crypto-bigint/pull/45
[#46]: https://github.com/RustCrypto/crypto-bigint/pull/46
[#47]: https://github.com/RustCrypto/crypto-bigint/pull/47

## 0.2.11 (2021-10-16)
### Added
- `AddMod` proptests ([#24])
- Bitwise `And`/`Or` operations ([#25])

[#24]: https://github.com/RustCrypto/crypto-bigint/pull/24
[#25]: https://github.com/RustCrypto/crypto-bigint/pull/25

## 0.2.10 (2021-09-21)
### Added
- `ArrayDecoding` trait ([#12])
- `NonZero` wrapper ([#13], [#16])
- Impl `Div`/`Rem` for `NonZero<UInt>` ([#14])

[#12]: https://github.com/RustCrypto/crypto-bigint/pull/12
[#13]: https://github.com/RustCrypto/crypto-bigint/pull/13
[#14]: https://github.com/RustCrypto/crypto-bigint/pull/14
[#16]: https://github.com/RustCrypto/crypto-bigint/pull/16

## 0.2.9 (2021-09-16)
### Added
- `UInt::sqrt` ([#9])

### Changed
- Make `UInt` division similar to other interfaces ([#8])

[#8]: https://github.com/RustCrypto/crypto-bigint/pull/8
[#9]: https://github.com/RustCrypto/crypto-bigint/pull/9

## 0.2.8 (2021-09-14) [YANKED]
### Added
- Implement constant-time division and modulo operations

### Changed
- Moved from RustCrypto/utils to RustCrypto/crypto-bigint repo ([#2])

[#2]: https://github.com/RustCrypto/crypto-bigint/pull/2

## 0.2.7 (2021-09-12)
### Added
- `UInt::shl_vartime` 

### Fixed
- `add_mod` overflow handling

## 0.2.6 (2021-09-08)
### Added
- `Integer` trait
- `ShrAssign` impl for `UInt`
- Recursive Length Prefix (RLP) encoding support for `UInt`

## 0.2.5 (2021-09-02)
### Fixed
- `ConditionallySelectable` impl for `UInt`

## 0.2.4 (2021-08-23) [YANKED]
### Added
- Expose `limb` module
- `[limb::Inner; LIMBS]` conversions for `UInt`
- Bitwise right shift support for `UInt` ([#586], [#590])

## 0.2.3 (2021-08-16) [YANKED]
### Fixed
- `UInt::wrapping_mul`

### Added
- Implement the `Hash` trait for `UInt` and `Limb`

## 0.2.2 (2021-06-26) [YANKED]
### Added
- `Limb::is_odd` and `UInt::is_odd`
- `UInt::new`
- `rand` feature

### Changed
- Deprecate `LIMB_BYTES` constant
- Make `Limb`'s `Inner` value public

## 0.2.1 (2021-06-21) [YANKED]
### Added
- `Limb` newtype
- Target-specific rustdocs

## 0.2.0 (2021-06-07) [YANKED]
### Added
- `ConstantTimeGreater`/`ConstantTimeLess` impls for UInt
- `From` conversions between `UInt` and limb arrays
- `zeroize` feature
- Additional `ArrayEncoding::ByteSize` bounds
- `UInt::into_limbs`
- `Encoding` trait

### Removed
- `NumBits`/`NumBytes` traits; use `Encoding` instead

## 0.1.0 (2021-05-30)
- Initial release
