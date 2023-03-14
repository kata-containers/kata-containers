# 0.16.0 (October 23, 2021)

### Breaking Changes

- Upgrade to `v0.3.0` of `tracing-subscriber` ([#1677])
  For list of breaking changes in `tracing-subscriber`, see the
  [v0.3.0 changelog].

### Added

- `OpenTelemetrySpanExt::add_link` method for adding a link between a `tracing`
  span and a provided OpenTelemetry `Context` ([#1516])

Thanks to @LehMaxence for contributing to this release!

[v0.3.0 changelog]: https://github.com/tokio-rs/tracing/releases/tag/tracing-subscriber-0.3.0
[#1516]: https://github.com/tokio-rs/tracing/pull/1516
[#1677]: https://github.com/tokio-rs/tracing/pull/1677

# 0.15.0 (August 7, 2021)

### Breaking Changes

- Upgrade to `v0.16.0` of `opentelemetry` (#1497)
  For list of breaking changes in OpenTelemetry, see the
  [v0.16.0 changelog](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry/CHANGELOG.md#v0160).

# 0.14.0 (July 9, 2021)

### Breaking Changes

- Upgrade to `v0.15.0` of `opentelemetry` ([#1441])
  For list of breaking changes in OpenTelemetry, see the
  [v0.14.0 changelog](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry/CHANGELOG.md#v0140).

### Added

- Spans now include Opentelemetry `code.namespace`, `code.filepath`, and
  `code.lineno` attributes ([#1411])

### Changed

- Improve performance by pre-allocating attribute `Vec`s ([#1327])

Thanks to @Drevoed, @lilymara-onesignal, and @Folyd for contributing
to this release!

[#1441]: https://github.com/tokio-rs/tracing/pull/1441
[#1411]: https://github.com/tokio-rs/tracing/pull/1411
[#1327]: https://github.com/tokio-rs/tracing/pull/1327

# 0.13.0 (May 15, 2021)

### Breaking Changes

- Upgrade to `v0.14.0` of `opentelemetry` (#1394)
  For list of breaking changes in OpenTelemetry, see the
  [v0.14.0 changelog](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry/CHANGELOG.md#v0140).

# 0.12.0 (March 31, 2021)

### Breaking Changes

- Upgrade to `v0.13.0` of `opentelemetry` (#1322)
  For list of breaking changes in OpenTelemetry, see the
  [v0.13.0 changelog](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry/CHANGELOG.md#v0130).

### Changed

- Improve performance when tracked inactivity is disabled (#1315)

# 0.11.0 (January 25, 2021)

### Breaking Changes

- Upgrade to `v0.12.0` of `opentelemetry` (#1200)
  For list of breaking changes in OpenTelemetry, see the
  [v0.12.0 changelog](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry/CHANGELOG.md#v0120).

# 0.10.0 (December 30, 2020)

### Breaking Changes

- Upgrade to `v0.11.0` of `opentelemetry` (#1161)
  For list of breaking changes in OpenTelemetry, see the
  [v0.11.0 changelog](https://github.com/open-telemetry/opentelemetry-rust/blob/master/opentelemetry/CHANGELOG.md#v0110).
- Update `OpenTelemetrySpanExt::set_parent` to take a context by value as it is
  now stored and propagated. (#1161)
- Rename `PreSampledTracer::sampled_span_context` to
  `PreSampledTracer::sampled_context` as it now returns a full otel context. (#1161)

# 0.9.0 (November 13, 2020)

### Added

- Track busy/idle timings as attributes via `with_tracked_inactivity` (#1096)

### Breaking Changes

- Upgrade to `v0.10.0` of `opentelemetry` (#1049)
  For list of breaking changes in OpenTelemetry, see the
  [v0.10.0 changelog](https://github.com/open-telemetry/opentelemetry-rust/blob/master/opentelemetry/CHANGELOG.md#v0100).

# 0.8.0 (October 13, 2020)

### Added

- Implement additional record types (bool, i64, u64) (#1007)

### Breaking changes

- Add `PreSampledTracer` interface, removes need to specify sampler (#962)

### Fixed

- Connect external traces (#956)
- Assign default ids if missing (#1027)

# 0.7.0 (August 14, 2020)

### Breaking Changes

- Upgrade to `v0.8.0` of `opentelemetry` (#932)
  For list of breaking changes in OpenTelemetry, see the
  [v0.8.0 changelog](https://github.com/open-telemetry/opentelemetry-rust/blob/master/CHANGELOG.md#v080).

# 0.6.0 (August 4, 2020)

### Breaking Changes

- Upgrade to `v0.7.0` of `opentelemetry` (#867)
  For list of breaking changes in OpenTelemetry, see the
  [v0.7.0 changelog](https://github.com/open-telemetry/opentelemetry-rust/blob/master/CHANGELOG.md#v070).

# 0.5.0 (June 2, 2020)

### Added

- Support `tracing-log` special values (#735)
- Support `Span::follows_from` creating otel span links (#723)
- Dynamic otel span names via `otel.name` field (#732)

### Breaking Changes

- Upgrade to `v0.6.0` of `opentelemetry` (#745)

### Fixed

- Filter out invalid parent contexts when building span contexts (#743)

# 0.4.0 (May 12, 2020)

### Added

- `tracing_opentelemetry::layer()` method to construct a default layer.
- `OpenTelemetryLayer::with_sampler` method to configure the opentelemetry
  sampling behavior.
- `OpenTelemetryLayer::new` method to configure both the tracer and sampler.

### Breaking Changes

- `OpenTelemetrySpanExt::set_parent` now accepts a reference to an extracted
  parent `Context` instead of a `SpanContext` to match propagators.
- `OpenTelemetrySpanExt::context` now returns a `Context` instead of a
  `SpanContext` to match propagators.
- `OpenTelemetryLayer::with_tracer` now takes `&self` as a parameter
- Upgrade to `v0.5.0` of `opentelemetry`.

### Fixed

- Fixes bug where child spans were always marked as sampled

# 0.3.1 (April 19, 2020)

### Added

- Change span status code to unknown on error event

# 0.3.0 (April 5, 2020)

### Added

- Span extension for injecting and extracting `opentelemetry` span contexts
  into `tracing` spans

### Removed

- Disabled the `metrics` feature of the opentelemetry as it is unused.

# 0.2.0 (February 7, 2020)

### Changed

- Update `tracing-subscriber` to 0.2.0 stable
- Update to `opentelemetry` 0.2.0
