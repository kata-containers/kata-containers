# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## [0.3.1] 2022-05-19

### Changed

- Make `ApplicationTag`’s inner value public
- Update dependencies

## [0.3.0] 2022-02-02

### Added

- Support for `GeneralString`
- `ApplicationTag` to encode ASN.1 application tags

### Changed

- Bump minimal rustc version to 1.56

## [0.2.5] 2021-05-27

### Added

- Support for `BMP_STRING` type

## [0.2.4] 2020-08-31

### Changed

- Update dependencies

## [0.2.3] 2020-07-07

### Changed

- Dependencies clean up

## [0.2.2] 2020-01-14

### Fixed

- Fix `Asn1RawDer` behind Application/Context tags issue [#14](https://github.com/Devolutions/picky-rs/issues/14).

## [0.2.1] 2020-01-10

### Added

- `Asn1RawDer` wrapper for user-provided raw DER.

## [0.2.0] 2019-12-23

### Added

- Add `from_reader_with_max_len` deserialization function to limit how many bytes can be read at most.

### Changed

- `from_reader` function has a default limit of 10240 bytes before returning a truncated data error.
    Uses `from_reader_with_max_len` to change the limit.

### Fixed

- Fix various crash found by fuzzing.
