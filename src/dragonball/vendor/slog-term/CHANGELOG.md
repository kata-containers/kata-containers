# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

<!-- next-url -->
## [Unreleased](https://github.com/slog-rs/term/compare/v2.8.1...HEAD) - ReleaseDate

## 2.9.0 - 2022-02-20
### Changed

* Switch from `chrono` to `term`
    * Merges PR #39 - Thanks @JanZerebecki
    * Avoids [RUSTSEC-2020-0159](https://rustsec.org/advisories/RUSTSEC-2020-0159)
* BREAKING: Bump MSRV to 1.53
* Switch from Travis CI to Github Actions

## 2.8.1 - 2022-02-09
### Fixed

* Disable default features on chrono to address RUSTSEC-2020-0071 aka CVE-2020-26235
* Use SPDX compliant license name

## 2.8.0 - 2021-02-10
### Changed

* BREAKING: bump MRSV to 1.36
* update `term` dependency

## 2.7.0 - 2021-02-06
### Added

* option in full format builder to enable file location
* `print_msg_header` customization

## 2.6.0 - 2020-05-28

* Fix detection of terminals without color support
* Add support for slog/nested-values
* Documentation fixes

## 2.5.0 - 2020-01-29

* Upgrade to thread_local 1
* Fix `clippy` warnings on 2018 edition
* Cargo.toml - 2018 edition

## 2.4.2 - 2019-10-25
### Changed

Make public the following elements to be able to reuse for your own implementations:

* `print_msg_header`
* `Serializer`
* `Serializer::new`
* `Serializer.finish()`
* `CompactFormatSerializer`
* `CompactFormatSerializer::new`
* `CompactFormatSerializer.finish()`
* `CountingWriter`
* `CountingWriter::new`
* `CountingWriter.count()`

## 2.4.1 - 2019-07-10
### Changed

* Lazily evaluate color detection (GH-214).

## 2.4.0 - 2018-04-13
### Changed

* Bump `term` dependency

## 2.3.0 - 2017-08-26
### Changed

* Change semantics of `TermDecorator::build` that didn't make sense before.

## 2.2.0 - 2017-08-26
### Added

* `use_original_order` for `FullFormat`

## 2.1.0 - 2017-08-05
### Added

* Writer that plays nicely with unit tests

## 2.0.4 - 2017-07-03
### Changed

* Improved documentation
* Relicense under MPL/Apache/MIT


## 2.0.1 - 2017-05-08
### Changed

* Fix commas on empty message


## 2.0.0 - 2017-04-29
### Changed

* Just release major version

## 2.0.0-4.0 - 2017-04-11
### Changed

* Update slog dependency

## 2.0.0-3.0 - 2017-03-27
### Changed

* Update slog dependency

## 2.0.0-2.1 - 2017-03-16
### Fixed

* `TermDecoratorBuilder::build()` will not panic anymore
* `TermDecorator` will automatically detect if colors should be used
* Add helper functions for drains with default settings

### Added

* Color settings for `TermDecorator`
* `TermDecoratorBuilder::try_build()` that returns `Option`

## 2.0.0-2.0 - 2017-03-11
### Changed

* Full rewrite, ditch `slog-stream`
* Use `term` so should work on Linux and Windows shells out of the box
* Allow extending (eg. for `termion` and other terminal crates)
* Compact mode now prints one value per line. Groups are not exposed by `slog`
  anymore.

## 1.5.0 - 2017-02-05
### Change

* Reverse the order of record values in full mode to match slog 1.5
  definition

## 1.4.0 - 2017-01-29
### Changed

* Fix a bug in `new_plain` that would make it still use colors.
* No comma will be printed after an empty "msg" field
* Changed order of full format values

## 1.3.5 - 2016-01-13
### Fixed

* [1.3.4 with `?` operator breaks semver](https://github.com/slog-rs/term/issues/6) - fix allows builds on stable Rust back to 1.11

## 1.3.4 - 2016-12-27
### Fixed

* [Fix compact formatting grouping messages incorrectly](https://github.com/slog-rs/slog/issues/90)

## 1.3.3 - 2016-10-31
### Changed

* Added `Send+Sync` to `Drain` returned on `build`

## 1.3.2 - 2016-10-22
### Changed

* Fix compact format, repeating some values unnecessarily.

## 1.3.1 - 2016-10-22
### Changed

* Make `Format` public so it can be reused

## 1.3.0 - 2016-10-21
### Changed

* **BREAKING**: Switched `AsyncStramer` to `slog_extra::Async`

## 1.2.0 - 2016-10-17
### Changed

* **BREAKING**: Rewrite handling of owned values.

## 1.1.0 - 2016-09-28
### Added

* Custom timestamp function support

### Changed

* Logging level color uses only first 8 ANSI terminal colors for better compatibility

## 1.0.0 - 2016-09-21

First stable release.
