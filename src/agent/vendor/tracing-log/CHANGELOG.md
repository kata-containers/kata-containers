# 0.1.3 (April 21st, 2022)

### Added

- **log-tracer**: Added `LogTracer::with_interest_cache` to enable a limited
 form of per-record `Interest` caching for `log` records ([#1636])

### Changed

- Updated minimum supported Rust version (MSRV) to Rust 1.49.0 ([#1913])

### Fixed

- **log-tracer**: Fixed `LogTracer` not honoring `tracing` max level filters
  ([#1543])
- Broken links in documentation ([#2068], [#2077])

Thanks to @Millione, @teozkr, @koute, @Folyd, and @ben0x539 for contributing to
this release!

[#1636]: https://github.com/tokio-rs/tracing/pulls/1636
[#1913]: https://github.com/tokio-rs/tracing/pulls/1913
[#1543]: https://github.com/tokio-rs/tracing/pulls/1543
[#2068]: https://github.com/tokio-rs/tracing/pulls/2068
[#2077]: https://github.com/tokio-rs/tracing/pulls/2077

# 0.1.2 (February 19th, 2020)

### Added

- Re-export the `log` crate so that users can ensure consistent versions ([#602])
- `AsLog` implementation for `tracing::LevelFilter` ([#1248])
- `AsTrace` implementation for `log::LevelFilter` ([#1248])

### Fixed

- **log-tracer**: Fixed `Log::enabled` implementation for `LogTracer` not
  calling `Subscriber::enabled` ([#1254])
- **log-tracer**: Fixed `Log::enabled` implementation for `LogTracer` not
  checking the max level hint ([#1247])
- Several documentation fixes ([#483], [#485], [#537], [#595], [#941], [#981])

[#483]: https://github.com/tokio-rs/tracing/pulls/483
[#485]: https://github.com/tokio-rs/tracing/pulls/485
[#537]: https://github.com/tokio-rs/tracing/pulls/537
[#595]: https://github.com/tokio-rs/tracing/pulls/595
[#605]: https://github.com/tokio-rs/tracing/pulls/604
[#941]: https://github.com/tokio-rs/tracing/pulls/941
[#1247]: https://github.com/tokio-rs/tracing/pulls/1247
[#1248]: https://github.com/tokio-rs/tracing/pulls/1248
[#1254]: https://github.com/tokio-rs/tracing/pulls/1254

# 0.1.1 (October 29, 2019)

### Deprecated

- `TraceLogger` (use `tracing`'s "log" and "log-always" feature flags instead)

### Fixed

- Issues with `log/std` feature flag (#406)
- Minor documentation issues (#405, #408)

# 0.1.0 (September 3, 2019)

- Initial release
