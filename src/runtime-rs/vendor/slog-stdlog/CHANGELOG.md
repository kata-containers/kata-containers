# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

## 4.1.1 - 2022-03-22
### Added

* Support `feature="log/kv-unstable"` (PR #18)
  * Requires `slog/dynamic-keys`
  * This is unstable (obviously), so I don't consider it
    a change that warants a minor-version bump

### Changed

* Switch to github actions (PR #20)

### Fixed

* Avoid using log's private API to construct records (PR #21)
  * Fixes recent versions of `log` crate (`>=0.4.15`), resolving slog-rs/slog#309
* Fix formatting & clippy lints (part of switch to GH actions)

## 4.1.0 - 2020-10-21
### Changed

* Require `slog>=2.4`
* Require `log>=0.4.11

## Fixed

* Remove unused dependency `crossbeam = 0.7.1`

## 4.0.0 - 2018-08-13
### Changed

* Update to `log` 0.4

## 3.0.2 - 2017-05-29
### Fixed

* Documentation example for `init`

## 3.0.0 - 2017-05-28
### Changed

* Update to slog-scope v4 wh  default to panicking instead of discarding
  messages. Be warned!
* Relicensed under MPL-2.0/MIT/Apache-2.0

## 2.0.0-4.0 - 2017-04-29

### Changed

* Updated slog dependency

## 2.0.0-3.0 - 2017-04-02
### Changed

* Updated slog dependency

## 2.0.0-0.2
### Fixed

* Dependencies

## 2.0.0-0.1
### Changed

* Port to slog v2
* Base on `slog-scope`

## 1.1.0
### Changed

* BREAKING: Rewrite handling of owned values.

## 1.0.1 - 2016-10-02
### Changed

* Fixed `StdLog` not serializing the key-value pairs.
* `StdLog` message to `log` crate is lazily-evaluated.


## 1.0.0 - 2016-09-21

First stable release.
