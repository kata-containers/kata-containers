# quanta

[![conduct-badge][]][conduct] [![gh-actions-badge][]][gh-actions] [![downloads-badge][] ![release-badge][]][crate] [![docs-badge][]][docs] [![libraries-io-badge][]][libraries-io] [![license-badge][]](#license)

[conduct-badge]: https://img.shields.io/badge/%E2%9D%A4-code%20of%20conduct-blue.svg
[gh-actions-badge]: https://github.com/metrics-rs/quanta/workflows/Rust/badge.svg
[downloads-badge]: https://img.shields.io/crates/d/quanta.svg
[release-badge]: https://img.shields.io/crates/v/quanta.svg
[license-badge]: https://img.shields.io/crates/l/quanta.svg
[docs-badge]: https://docs.rs/quanta/badge.svg
[libraries-io-badge]: https://img.shields.io/librariesio/github/metrics-rs/quanta.svg
[libraries-io]: https://libraries.io/cargo/quanta
[conduct]: https://github.com/metrics-rs/quanta/blob/main/CODE_OF_CONDUCT.md
[gh-actions]: https://github.com/metrics-rs/quanta/actions
[crate]: https://crates.io/crates/quanta
[docs]: https://docs.rs/quanta

__quanta__ is a high-speed timing library, useful for getting the current time _very quickly_, as
well as manipulating it.

## code of conduct

**NOTE**: All conversations and contributions to this project shall adhere to the [Code of Conduct][conduct].

## usage

The API documentation of this library can be found at [docs.rs/quanta](https://docs.rs/quanta/).

## general features
- count CPU cycles via Time Stamp Counter (TSC). or
- get monotonic time, in nanoseconds, based on TSC (or OS fallback)
- extremely low overhead where possible
- mockable
- cross-platform
- fun, science-y name!

## platform / architecture support

For most major platforms -- Linux, Windows, and macOS -- with processors made around or after 2008,
you should have no problems using `quanta` with full TSC support.  `quanta` will always fallback to
the included stdlib timing facilities if TSC support is not present.  The biggest caveat to this, as
evidenced in the compatibility matrix below, is that we only support the TSC  on x86/x86_64 platforms.


| Platform             | stdlib fallback | TSC support? | CI tests? |
|----------------------|-----------------|--------------|-----------|
| Linux (x86/x86_64)   | ✅               | ✅           | ✅        |
| Linux (MIPS/ARM)     | ✅               | ❌           | ✅        |
| Windows (x86/x86_64) | ✅               | ✅           | ✅        |
| Windows (ARM)        | ✅               | ❌           | ❌        |
| macOS (x86/x86_64)   | ✅               | ✅           | ✅        |
| macOS (ARM)          | ✅               | ❌           | ❌        |
| iOS (ARM)            | ✅               | ❌           | ❌        |

## performance

`quanta` sits neck-and-neck with native OS time facilities: the cost of `Clock::now` is on par
`Instant::now` from the stdlib, if not better.

## why use this over stdlib?

Beyond having a performance edge in specific situations, the ability to use mocked time makes it
easier to actually test that your application is doing the right thing when time is involved.

Additionally, and as mentioned in the general features section, `quanta` provides a safe/thin
wrapper over accessing the Time Stamp Counter, which allows measuring cycle counts over short
sections of code.  This can be relevant/important for accurately measuring performance-critical sections of code.

## alternative crates

- [`chrono`](https://docs.rs/chrono):
  - based on `std::time::SystemTime`: non-monotonic reads
  - focused on timezone-based "date/time" measurements, not intervals/elapsed time
  - clock cannot be altered at all (no pause, no discrete updates)
- [`time`](https://docs.rs/time):
  - based on `std::time::SystemTime` and `std::time::Instant`:
    - `time::Time`/`time::PrimitiveDateTime` use `SystemTime`: non-monotonic reads
    - `time::Instant` uses `Instant`: monotonic reads
  - focused on timezone-based "date/time" measurements, not interval/elapsed time
  - clock cannot be altered at all (no pause, no discrete updates)
- [`clock`](https://docs.rs/clock):
  - based on `std::time::SystemTime`: non-monotonic reads
  - clock can be swapped (trait-based)
  - no free function for acquiring time
- [`clocksource`](https://docs.rs/clocksource):
  - based on TSC w/ OS fallback; non-monotonic reads
  - clock cannot be altered at all (no pause, no discrete updates)
  - depends on unstable `asm!` macro + feature flag to enable TSC
  - no free function for acquiring time
- [`pausable_clock`](https://docs.rs/pausable_clock):
  - based on `std::time::Instant`: monotonic reads
  - clock can be paused (time can be delayed, but not discretely updated)
  - no free function for acquiring time

## license

__quanta__ is licensed under the MIT license. ([LICENSE](LICENSE) or http://opensource.org/licenses/MIT)
