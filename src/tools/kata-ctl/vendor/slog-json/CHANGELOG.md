# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

## [Unreleased]

## 2.6.0 - 2022-02-20
### Changed
* Replaced `chrono` with `time` (PR #28). Thanks @ShellWowza
* Bump Minimum Supported Rust Version from `1.38` -> `1.53`
* Switch to Github Actions

## 2.5.0 - 2022-01-21
### Changed
* Upgrade to Rust 2018
* Relicense to Apache/MIT/MPL
    * Previous license was MPL only, so this is actually more permissive

### Security
* Disable default features of chrono. Avoids [RUSTSEC-2020-0071](https://rustsec.org/advisories/RUSTSEC-2020-0071.html)

## 2.4.0 - 2021-07-28
### Added

* Support serializing u128/i128 as json-numbers

## 2.3.0 - 2018-12-04
### Added

* Option `flush` to enable flushing of the `io::Write` after each record.

### Changed

* Improve error message for Serde serialization errors.

## 2.2.0 - 2017-12-10
### Added

* Support for experimental `slog` features. Consider unstable.
* `set_pretty` option

## 2.0.2 - 2017-08-22
### Changed

* Update dependencies

## 2.0.0 - 2017-04-29
### Changed

* Update dependencies

## 2.0.0-4.0 - 2017-04-11
### Changed

* Update slog dependency

## 2.0.0-3.1 - 2017-04-07
### Changed

* Update slog dependency

## 2.0.0-3.0 - 2017-03-27
### Changed

* Update slog dependency

## 2.0.0-0.2 - 2017-03-11
### Changed

* Update to `slog 2.0.0-0.2`

## 1.2.1 - 2016-11-30
### Changed

* Moved to own repository
