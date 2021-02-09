# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

## 2.7.0 - 2020-11-29

* Add #% for alternate display of the value part
* Implement `Eq` for dynamic `Key`s
* Add `emit_error` to `Serializer`, `#` for serializing foreign errors, and
  `impl Value for std::io::Error`

## 2.6.0 - 2019-10-28

* Add #? for pretty-debug printing the value part

## 2.5.3 - ????-??-??

* Use fully qualified call syntax for `Logger::log` in macros

## 2.5.2 - 2019-07-22

* Restored parsing of `Level` and `FilterLevel` truncated names

## 2.5.1 - 2019-07-11

* Added parsing of `Level` and `FilterLevel` short names

## 2.5.0 - 2019-07-11

* Added `FilterLevel::accepts`
* Added `as_str`, `as_short_str` and `Display` to `FilterLevel`

## 2.4.1 - 2018-10-03

* disable support for i128/u128 types if rustc is old

## 2.4.0 - 2018-09-19

* Implement Value for 128 bit integers
* Add support 2018-style macro imports
  * **WARNING**: This is a breaking change that we couldn't avoid. Users using
    explicitly macro import (like `#[macro_use(slog_o)]`) must add
    `__slog_builtin` to the import list.
* Bump miminum supported Rust version to 1.26

## 2.3.3 - 2018-07-20

* `impl Value for SocketAddr`

## 2.3.2 - 2018-07-20

* Revert broken changes:
  * Make `?` and `%` formatters in `kv!` more flexible
  * Export local inner macros to help with Rust 2018 testing

## 2.3.0 - 2018-07-20

* Export local inner macros to help with Rust 2018 testing
* Stabilize `Record::new`
* Make `?` and `%` formatters in `kv!` more flexible

## 2.2.3 - 2018-03-28

* Fix (again) problems introduced by `2.2.1`

## 2.2.2 - 2018-03-26

* Fix problems introduced by `2.2.1`

## 2.2.1 - 2018-03-24

* Add `is_x_enabled()` for queering (imprecise) log-level

## 2.2.0 - 2018-02-13
### Added

* Support for named format arguments in format messages. They will now become
  respectively named key-value pairs.

## 2.1.0 - 2017-12-10
### Added

* Support for nested-values through `emit_serde`, behind `nested-values` feature flag,
  disabled by default for backward compatibility. **Note**: Consider unstable for the time
  being.
* Support for dynamic key (`String` vs `&'static str`), behind `dynamic-keys` feature
  flag, disabled by default for backward compatibility. **Note**: Consider unstable for
  the time being.

## 2.0.12 - 2017-09-14
### Changed

* `#[allow(dead_code)` on unused log statements

## 2.0.11 - 2017-09-13
### Changed

* Impl `Value` for `std::path::Display`

## 2.0.10 - 2017-09-09
### Changed

* Remove unnecessary 'static bound on `FnValue`

## 2.0.9 - 2017-08-23
### Changed

* Update README

## 2.0.6 - 2017-05-27
### Changed

* Fix for https://github.com/rust-lang/rust/pull/42125

## 2.0.5 - 2017-05-15
### Changed

* Relicense under MPL/Apache/MIT

## 2.0.4 - 2017-05-05
### Fixed

* Documentation improvements

## 2.0.3 - 2017-05-05
### Fixed

* Documentation fixes

## 2.0.2 - 2017-04-12
### Fixed

* Compile time logging level filtering

## 2.0.0 - 2017-04-11
### Changed (since v1; bigger picture)

* Unified and simplified logging macros structure and ordering.
* Added logging Record `tags`.
* Refactored key-value pairs traits and structures and overall handling.
  * It's now possible to `impl KV for MyStruct`.
  * `kv!` can be used to create on stack key-value list.
  * `KV`-implementing data can appear on the key-value list directly.
* Support chaining of `OwnedKVList`s. Existing `Logger` can be used as a `Drain`
  to allow custom handling logic for a part of the logging hierarchy.
* Added associated `Ok` type to `Drain`.
* Support for `Drain`s unwind safety.
* Refactored `Record` to optimize performance on async operations.
* `slog-extra` has been renamed to `slog-async` since that's the only functionality it contained.
* `slog-stream` is obsoleted and won't be used in `slog v2` at all. It was a wrong abstraction.
  `Decorators` functionality was moved to `slog-term`.
* `slog-term` provides open `Decorator` traits to allow multiple terminal / file writing backends.
* `slog-term` default `Decorator`s use `term` crate and should work correctly on all supported OSes.
* `DrainExt` has been removed and utility methods moved directly to `Drain`
* `slog-stdlog` utilizes `slog-scope` directly.
* Support for "typed" `Logger`s to allow squeezing last drops of performance possible,
  at the cost of `T` in `Logger<T>`.

## 2.0.0-3.1 - 2017-03-25
### Added

* Support for `fmt::Display` values with `%` in `kv!`


## 2.0.0-3.0 - 2017-03-25
### Changed

* Added support for own `KV` and `Value` implementations
* Streamlined the formatting syntax for `log!` and friends; **BREAKING**
* Lazy values need explicit `FnValue` wrapper; **BREKING**

### Added

* `kv!` macro

## 2.0.0-2.2 - 2017-03-19
### Fixes

* Bunch of trait-related fixes

## 2.0.0-2.1 - 2017-03-11
### Fixed

* Require `MapErr` and `Filter` to be `UnwindSafe`

## 2.0.0-2.0 - 2017-03-11
### Changed

* Make `Logger::root` return "erased" version
* Introduce `Logger::root_typed` for "non-erased" `Logger` creation

## 2.0.0-1.0 - 2017-02-23

### Fixed

* `fmt::Debug` for `MutexDrainError`

### Changed
* Parametrize `Logger` over the `Drain` it holds and introduce "erased" version
* Enforcing `UnwindSafe` `Drain`s for `Logger`s
* Refactored key-value pairs traits and structures
* Renamed some types
* Support chaining of `OwnedKVList`s
* Added associated `Ok` type to `Drain`
* Refactored `Record` to optimize performance on async
  operations
* Minimal rustc version required: `1.15.0`
* `DrainExt` has been removed and utility methods moved directly to `Drain`

### Added

* Macros to create `OwnedKV` and `BorrowedKV`
* `Logger` implements `Drain`

## 1.5.0 - 2017-01-19
### Changed

* Order of key-value pairs is now strictly defined

### Added

* `Logger` implements `Drain`

### Deprecated

* Creation of `OwnedKeyValueList`

## 1.4.1 - 2017-01-19
### Fixed

* Fix an invalid syntax exposed by nightly rust change (Issue #103)

## 1.4.0 - 2016-12-27
### Changed

* Updated documentation

### Deprecated

* `OwnedKeyValueList::id`

## 1.3.2 - 2016-11-19
### Added

* `slog_o` as an alternative name for `o`

## 1.3.1 - 2016-11-19
### Fixed

* Cargo publishing mistake.

## 1.3.0 - 2016-10-31
### Changed

* **BREAKING**: Removed default `Send+Sync` from `Drain`

## 1.2.1 - 2016-10-27
### Added

* `OwnedKeyValueList::id` for owned key value unique identification

## 1.2.0 - 2016-10-21
### Changed

* **BREAKING**: `Serializer` takes `key : &'static str` now

### Fixed

* Corner cases in `info!(...)` and other macros

## 1.1.0 - 2016-10-17
### Changed

* **BREAKING**: Rewrite handling of owned values.

## 1.0.1
### Fixed

* `use std` in `o!`

### Added

* Implement `fmt::Debug` for `Logger`

## 1.0.0 - 2016-09-21

First stable release.
