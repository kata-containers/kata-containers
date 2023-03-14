# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.3.0 (2021-03-22)
### Changed
- Bump `der` to v0.3 ([#354])

### Removed
- `AlgorithmParameters` enum ([#343])

[#343]: https://github.com/RustCrypto/utils/pull/343
[#354]: https://github.com/RustCrypto/utils/pull/354

## 0.2.1 (2021-02-22)
### Added
- Impl `Choice` for `AlgorithmParameters` ([#295])

[#295]: https://github.com/RustCrypto/utils/pull/295

## 0.2.0 (2021-02-18)
### Changed
- Return `Result` from `AlgorithmIdentifier::params_*` ([#274])

[#274]: https://github.com/RustCrypto/utils/pull/274

## 0.1.0 (2021-02-16)
- Initial release
