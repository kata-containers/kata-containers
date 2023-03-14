# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.12.5] - 2022-10-03
### Security Fixes
- Update `chrono` and enable only necessary features to address RUSTSEC-2020-0071, thanks @flavio [#506]

### Changes
- Various dependency updates [#501], [#502], [#503], [#508], [#509], [#514]

[#501]: https://github.com/awslabs/tough/pull/501
[#502]: https://github.com/awslabs/tough/pull/502
[#503]: https://github.com/awslabs/tough/pull/503
[#506]: https://github.com/awslabs/tough/pull/506
[#508]: https://github.com/awslabs/tough/pull/508
[#509]: https://github.com/awslabs/tough/pull/509
[#514]: https://github.com/awslabs/tough/pull/514

## [0.12.4] - 2022-08-12
### Changes
- Various dependency updates

## [0.12.3] - 2022-07-26
- Various dependency updates

## [0.12.2] - 2022-04-26
### Changes
- Blanket impl sign for references [#448]
- Fix clippy warnings [#455]
- Various other dependency updates

[#448]: https://github.com/awslabs/tough/pull/448
[#455]: https://github.com/awslabs/tough/pull/455

## [0.12.1] - 2022-01-28
### Fixes
- ECDSA keys now use the correct keytype name, thanks @flavio [#425]

### Changes
- Added support for hex-encoded ECDSA keys, thanks @flavio [#426]
- Updated to snafu 0.7, thanks @shepmaster [#435]
- Various other dependency updates

[#425]: https://github.com/awslabs/tough/pull/425
[#426]: https://github.com/awslabs/tough/pull/426
[#435]: https://github.com/awslabs/tough/pull/435

## [0.12.0] - 2021-10-19
### Breaking Changes
- Target names are now specified with a struct, `TargetName`, instead of `String`.

### Changes
- Update dependencies.
- Fix an issue where delegated role names with path traversal constructs could cause files to be written in unexpected locations.
- Fix a similar issue with path traversal constructs in target names.

## [0.11.3] - 2021-09-15
### Changes
- Update dependencies.

## [0.11.2] - 2021-08-24
### Changes
- Add `Repository.cache_metadata` method.  [#403]

[#403]: https://github.com/awslabs/tough/pull/403

## [0.11.1] - 2021-07-30
### Changes
- Update dependencies.  [#363], [#364], [#365], [#366], [#367], [#379], [#381], [#382], [#384], [#391], [#393], [#396], [#398]
- Fix clippy warnings.  [#372], [#378], [#383], [#399]
- Add license check to CI.  [#385]

[#363]: https://github.com/awslabs/tough/pull/363
[#364]: https://github.com/awslabs/tough/pull/364
[#365]: https://github.com/awslabs/tough/pull/365
[#366]: https://github.com/awslabs/tough/pull/366
[#367]: https://github.com/awslabs/tough/pull/367
[#372]: https://github.com/awslabs/tough/pull/372
[#378]: https://github.com/awslabs/tough/pull/378
[#379]: https://github.com/awslabs/tough/pull/379
[#381]: https://github.com/awslabs/tough/pull/381
[#382]: https://github.com/awslabs/tough/pull/382
[#383]: https://github.com/awslabs/tough/pull/383
[#384]: https://github.com/awslabs/tough/pull/384
[#385]: https://github.com/awslabs/tough/pull/385
[#391]: https://github.com/awslabs/tough/pull/391
[#393]: https://github.com/awslabs/tough/pull/393
[#396]: https://github.com/awslabs/tough/pull/396
[#398]: https://github.com/awslabs/tough/pull/398
[#399]: https://github.com/awslabs/tough/pull/399

## [0.11.0] - 2021-03-01
### Breaking Changes
- Update tokio to v1, hyper to v0.14 and reqwest to v0.11 [#330]

## Added
- `tough` now compiles and works on Windows. Thanks @Cytro54! [#342]

[#308]: https://github.com/awslabs/tough/pull/342
[#330]: https://github.com/awslabs/tough/pull/330

## [0.10.0] - 2020-01-14
### Breaking Changes

- Repositories are now loaded with the `RepositoryLoader`. `Repository::load` is no longer available. [#256]
- The `Repository` and `RepositoryEditor` objects no longer have type and lifetime parameters.
- Modifying `HttpTransport` behavior now uses `HttpTransportBuilder` instead of `ClientSettings`. [#308]

### Added
- A `DefaultTransport` that supports both file and http transport. [#256]

[#256]: https://github.com/awslabs/tough/pull/256
[#308]: https://github.com/awslabs/tough/pull/308

## [0.9.0] - 2020-07-20
### Breaking Changes
- `RepositoryEditor` requires lifetime and `Transport` type parameters
- Minor breaking changes across the `editor` module to support delegated targets

### Added
- The `editor` module now supports delegated targets

### Changed
- `RepositoryEditor` ensures the root has enough keys to fulfill the signing threshold
- `RepositoryEditor` allows adding multiple signatures at once
- Added a `add_old_signatures` method to `SignedRole` to enable cross-signing from an old root role

## [0.8.0] - 2020-07-20
### Breaking Changes
- The `HttpTransport` type and the `Read` and `Error` types that it uses have changed.
- Remove `root.json` from Snapshot metadata per [theupdateframework/specification#40](https://github.com/theupdateframework/specification/pull/40)

### Added
- Added HTTP retry logic.
- Added early support for delegations.
- Added logging.
- Added documentation to all remaining items.
- Allow control of link/copy behavior for existing paths.

### Changed
- Dependency updates.
- Fix new clippy lints in Rust 1.45.

## [0.7.1] - 2020-07-09

### Security
- Fixed uniqueness verification of signature threshold. ([CVE-2020-15093](https://github.com/awslabs/tough/security/advisories/GHSA-5q2r-92f9-4m49))

## [0.7.0] - 2020-06-26

### Added
- Added `#[non_exhaustive]` to `tough::Error` and `tough::schema::Error`
- `editor::signed::SignedRepository`:
  - Added `copy_targets`, which does the same as `link_targets` except copies rather than symlinks
  - Added `link_target` and `copy_target`, which are used by the above functions and allow for handling single files with custom filenames

## [0.6.0] - 2020-06-11

### Added
- Added `Target::from_path()` method.
- Added the `KeySource` trait, which allows users to fetch signing keys.
- Added `RepositoryEditor`, which allow users to update a `tough::Repository`'s metadata and optionally add targets.

### Changed
- Dependency updates.

## [0.5.0] - 2020-05-18

For changes that require modification of calling code see #120 and #121.

### Added
- Add optional ability to load an expired repository.

### Changed
- Rename `target_base_url` to `targets_base_url`.
- Dependency updates.

## [0.4.0] - 2020-02-11
- Updated `reqwest` to `0.10.1` to fix an issue with https failures. Note this requires use of `reqwest::blocking::*` instead of `reqwest::*` in code that is using HttpTransport.
- Update all dependencies with `cargo update`.

## [0.3.0] - 2019-12-16
### Added
- Added the `Sign` trait to `tough`, which allows users to sign data.
- Added the `canonical_form` method to the `Role` trait, which serializes the role into canonical JSON.

## [0.2.0] - 2019-12-04
### Added
- New methods `root`, `snapshot`, and `timestamp` on `Repository` to access the signed roles.

### Changed
- Changed the return type of `Repository::targets` to the signed role (`Signed<Targets>`). The top-level `Target` type is no longer necessary. **This is a breaking change.**
- Updated snafu to v0.6. **This is a breaking change** to the `snafu::ErrorCompat` implementation on library error types.
- Updated pem to v0.7.
- Switched to using `ring::digest` for SHA-256 digest calculation.
- Added `Debug`, `Clone`, and `Copy` implementations to structs when appropriate.

## [0.1.0] - 2019-11-08
### Added
- Everything!

[Unreleased]: https://github.com/awslabs/tough/compare/tough-v0.12.5...develop
[0.12.5]: https://github.com/awslabs/tough/compare/tough-v0.12.4...tough-v0.12.5
[0.12.4]: https://github.com/awslabs/tough/compare/tough-v0.12.3...tough-v0.12.4
[0.12.3]: https://github.com/awslabs/tough/compare/tough-v0.12.2...tough-v0.12.3
[0.12.2]: https://github.com/awslabs/tough/compare/tough-v0.12.1...tough-v0.12.2
[0.12.1]: https://github.com/awslabs/tough/compare/tough-v0.12.0...tough-v0.12.1
[0.12.0]: https://github.com/awslabs/tough/compare/tough-v0.11.3...tough-v0.12.0
[0.11.3]: https://github.com/awslabs/tough/compare/tough-v0.11.2...tough-v0.11.3
[0.11.2]: https://github.com/awslabs/tough/compare/tough-v0.11.1...tough-v0.11.2
[0.11.1]: https://github.com/awslabs/tough/compare/tough-v0.11.0...tough-v0.11.1
[0.11.0]: https://github.com/awslabs/tough/compare/tough-v0.10.0...tough-v0.11.0
[0.10.0]: https://github.com/awslabs/tough/compare/tough-v0.9.0...tough-v0.10.0
[0.9.0]: https://github.com/awslabs/tough/compare/tough-v0.8.0...tough-v0.9.0
[0.8.0]: https://github.com/awslabs/tough/compare/tough-v0.7.1...tough-v0.8.0
[0.7.1]: https://github.com/awslabs/tough/compare/tough-v0.7.0...tough-v0.7.1
[0.7.0]: https://github.com/awslabs/tough/compare/tough-v0.6.0...tough-v0.7.0
[0.6.0]: https://github.com/awslabs/tough/compare/tough-v0.5.0...tough-v0.6.0
[0.5.0]: https://github.com/awslabs/tough/compare/tough-v0.4.0...tough-v0.5.0
[0.4.0]: https://github.com/awslabs/tough/compare/tough-v0.3.0...tough-v0.4.0
[0.3.0]: https://github.com/awslabs/tough/compare/tough-v0.2.0...tough-v0.3.0
[0.2.0]: https://github.com/awslabs/tough/compare/tough-v0.1.0...tough-v0.2.0
[0.1.0]: https://github.com/awslabs/tough/releases/tag/tough-v0.1.0
