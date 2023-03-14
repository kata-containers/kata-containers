# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->

## [Unreleased] - ReleaseDate

## [0.9.3] - 2021-09-16

### Added
- CI tests for MIPS/ARM. ([#55](https://github.com/metrics-rs/quanta/pull/55))

### Changed
- Fixed compilation issue with `Mock` on MIPS/ARM. ([#55](https://github.com/metrics-rs/quanta/pull/55))
- Simplified how TSC/RDTSC suppoort is detected, which should avoid some situations where it was
  assumed to be present, but actually was not. ([#57](https://github.com/metrics-rs/quanta/pull/57))

## [0.9.2] - 2021-08-25

### Changed
- Pinned `crossbeam-utils` to `v0.8.5` where `AtomicCell::fetch_update` was introduced to fix, which
  fixes broken builds where Cargo chooses a version between `0.8.0` and `0.8.5`.
- Update `raw-cpuid` to `10.2` and `average` to `0.13`.

## [0.9.1] - 2021-08-12

### Changed
- Switched from `atomic-shim` to `crossbeam-utils` for better cross-platform atomic support. ([#52](https://github.com/metrics-rs/quanta/pull/52))

## [0.9.0] - 2021-06-17

### Added
- Support for WASM/WASI targets. ([#45](https://github.com/metrics-rs/quanta/pull/45))

## [0.8.0] - 2021-06-07

### Removed
- `Instant::as_unix_duration` as it was added in error.
- `metrics` feature flag as `metrics-core` is no longer a relevant crate.

## [0.7.2] - 2021-01-25
### Changed
- Bumped dependency on `raw-cpuid` to `9.0` in order to deal with a [RustSec
  advisory](https://rustsec.org/advisories/RUSTSEC-2021-0013).

## [0.7.1] - 2021-01-24
### Fixed
- Incorrect method visibility for non-SSE2 implementation of `Counter`.
  ([#38](https://github.com/metrics-rs/quanta/issues/38))

## [0.7.0] - 2021-01-03
### Changed
- MSRV bumped to 1.45.0.
- `Clock::now` takes `&self` instead of `&mut self`.
- Fixed a bug where a failure to spawn the upkeep thread would not allow subsequent attempts to
  spawn the upkeep thread to proceed.

### Added
- New methods --`Instant::now` and `Instant::recent` for getting the current and recent time,
  respectively.
- New free function `quanta::with_clock` for setting an override on the current thread that affects
  calls made to `Instant::now` and `Instant::recent`.
- New free function `quanta::set_recent` to allow customization of how global recent time is
  updated.

## [0.6.5] - 2020-09-16
### Changed
- Fixed a bug with not being able to start the upkeep thread at all.
  ([#29](https://github.com/metrics-rs/quanta/issues/29))

## [0.6.4] - 2020-08-27
### Added
- Add `Instant::as_unix_duration` to get the duration of time since the Unix epoch from an
  `Instant`.
  ### Changed
- Remove `clocksource` from dependencies and tests as it no longer compiles on stable or nightly.

## [0.6.3] - 2020-08-03
### Changed
- Publicly expose `Clock::upkeep` for advanced use cases.
- Relax constraints around checking for multiple sockets.
  ([#25](https://github.com/metrics-rs/quanta/issues/25))

## [0.6.2] - 2020-07-20
### Added
- Add support for MIPS/PowerPC. ([#23](https://github.com/metrics-rs/quanta/pull/23))

## [0.6.1] - 2020-07-13
### Added
- Publicly expose the `Error` type returned by `Upkeep::start`.

## [0.6.0] - 2020-07-06
This version of `quanta` was a massive overhaul of man areas of the API and internals, which was
done in a single PR: ([#19](https://github.com/metrics-rs/quanta/pull/19)).  You can read the PR
description for the finer details.  All changes below are part of the aforementioned PR.

### Changed
- `Clock::now` now returns a monotonic value in all cases.
- No longer possible to get a negative value from `Clock::delta`.
- Calibration is no longer a fixed one second loop, and will complete when it detects it has a
  statistically viable calibration ratio, or when it exceeds 200ms of wall-clock time.  In most
  cases, it should complete in under 10ms.
- Calibration is now shared amongst all `Clock` instances, running only once when the first `Clock`
  is created.

## [0.5.2] - 2020-05-01
### Changed
- Fix the logic to figure out when calibration is required.
  ([#14](https://github.com/metrics-rs/quanta/pull/14))

## [0.5.1] - 2020-04-11
### Changed
- Small tweak to the docs.

## [0.5.0] - 2020-04-11
### Changed
- Switch to `mach` for macOS/iOS as it was deprecated in `libc`.
  ([#12](https://github.com/metrics-rs/quanta/pull/12))
- Switch to `core::arch` for instrinics, and drop the feature flagged configuration to use it.
  ([#12](https://github.com/metrics-rs/quanta/pull/12))
- Switch to `criterion` for benchmarking. ([#12](https://github.com/metrics-rs/quanta/pull/12))

## [0.4.0] - 2020-02-20
### Changed
- Differentiate between raw and scaled time by adding a new `Instant` type.
  ([#10](https://github.com/metrics-rs/quanta/pull/10))

## [0.2.0] - 2019-03-10
### Changed
- Fixed support for Windows.  It was in a bad way, but actually works correctly now!
- Switched to Azure Pipelines CI + Cirrus CI, including formatting, tests, and benchmarks, for
  Linux, macOS, Windows, and FreeBSD.

## [0.1.0] - 2019-01-14
### Added
- Initial commit.
