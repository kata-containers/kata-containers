# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

## 4.3.0 - 2019-10-25

* Add `GlobalLoggerGuard::is_canceled`

## 4.2.0 - 2019-10-16

* Update the arc-swap dependency to 0.4.

## 4.1.2 - 2019-07-18

* Call `slog` macros via `$crate::` prefix to prevent the users of this crate from having to manually import `slog_trace`, `slog_debug`, etc.

## 4.1.1 - 2018-12-20

* Fix dependency reqs for building with older `slog` versions

## 4.1.0 - 2018-12-16
### Changed

* Update `crossbeam` and `lazy_static`

## 4.0.1 - 2017-12-14
### Changed

* Fix `nightly` compilation

## 4.0.0 - 2017-05-28
### Changed

* Default to panicking instead of discarding messages
* Relicensed under MPL-2.0/MIT/Apache-2.0

## 3.0.0 - 2017-04-11
### Changed

* Switch to reference-based API, which avoids allocations

## 2.0.0 - 2017-04-11
### Changed

* Update dependencies

## 2.0.0-3.1 - 2017-04-06
### Fixed

* Example in documentation was not saving the guard

## 2.0.0-3.0 - 2017-03-27
### Changed

* Add `GlobalLoggerGuard`
* Bump dependencies

## 2.0.0-2.0 - 2017-03-11

* Fix examples
* Bump slog dependency version

## 2.0.0-0.1 - 2017-03-05

* Move to use slog v2 developement release

## 0.2.2 - 2016-11-30
### Changed

* Moved to own repository
