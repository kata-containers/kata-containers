# 0.2.5 (February 16, 2021)

### Changed

- Updated `pin-project` dependency to 1.0 ([#1038])

### Fixed

- Several documentation fixes and improvements ([#832], [#911], [#913], [#941],
  [#953], [#981])

[#1038]: https://github.com/tokio-rs/tracing/pulls/1038
[#832]: https://github.com/tokio-rs/tracing/pulls/832
[#911]: https://github.com/tokio-rs/tracing/pulls/911
[#913]: https://github.com/tokio-rs/tracing/pulls/913
[#941]: https://github.com/tokio-rs/tracing/pulls/941
[#953]: https://github.com/tokio-rs/tracing/pulls/953
[#981]: https://github.com/tokio-rs/tracing/pulls/981
# 0.2.4 (April 21, 2020)

### Fixed

- docs.rs build failures (#618)
- Spelling in documentation skins -> sinks (#643)

# 0.2.3 (Feb 26, 2020)

### Added

- `WithDispatch::inner` and `WithDispatch::inner_mut` methods to allow borrowing
  the wrapped type (#589)
- `WithDispatch::with_dispatch` method, to propagate the subscriber to another
  type (#589)
- `inner_pin_ref` and `inner_pin_mut` methods to `Instrumented` and
  `WithDispatch` to project to the inner future when pinned (#590)

# 0.2.2 (Feb 14, 2020)

### Added

- Support for `futures` 0.3 `Stream`s and `Sink`s (#544)

### Fixed

- Compilation errors when using the `futures-03` feature (#576)

Thanks to @obergner and @najamelan for their contributions to this release!

# 0.2.1 (Jan 15, 2020)

### Added

- API documentation now shows which features are required by feature-flagged items (#523)
- `no_std` support (#498)

# 0.2.0 (Dec 3, 2019)

### Changed

- **Breaking Change**: the default `Future` implementation comes from the `std-future` feature.
  Compatibility with futures v0.1 is available via the `futures-01` feature.

# 0.1.1 (Oct 25, 2019)

### Added

- `Instrumented::inner` and `inner_mut` methods that expose access to the
  instrumented future (#386)

# 0.1.0 (Oct 8, 2019)

- Initial release
