# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

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
