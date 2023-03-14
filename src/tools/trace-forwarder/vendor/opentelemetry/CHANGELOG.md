# Changelog

## [v0.16.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.15.0...v0.16.0)

### Changed

- Add default resource in `TracerProvider` #571
- Rename `get_tracer` to `tracer` #586
- Extract `trace::noop` module and update docs #587
- Add `Hash` impl for span context and allow spans to clone and export current state #592
- Enforce span status code's order #593
- Make `SpanRef` public #600
- Make `SpanProcessor::on_start` take a mutable span #601
- Renamed `label` to `attribute` to align with otel specification #609

### Performance

- Small performance boost for `Resource::get` #579

## [v0.15.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.14.0...v0.15.0)

### Added

- More resource detectors #573

### Changed

- Expose the Error type to allow users to set custom error handlers #551
- Allow users to use different channels based on runtime in batch span processor #560
- Move `Unit` into `metrics` module #564
- Update trace flags to match spec #565

### Fixed

- Fix debug loop, add notes for `#[tokio::test]` #552
- `TraceState` cannot insert new key-value pairs #567

## [v0.14.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.13.0...v0.14.0)

## Added

- Adding a dynamic dispatch to Aggregator Selector #497
- Add `global::force_flush_tracer_provider` #512
- Add config `max_attributes_per_event` and `max_attributes_per_link` #521
- Add dropped attribute counts to events and links #529

## Changed

- Remove unnecessary clone in `Key` type #491
- Remove `#[must_use]` from `set_tracer_provider` #501
- Rename remaining usage of `default_sampler` to `sampler` #509
- Use current span for SDK-less context propagation #510
- Always export span batch when limit reached #519
- Rename message events to events #530
- Update resource merge behaviour #537
- Ignore links with invalid context #538

## Removed

- Remove remote span context #508
- Remove metrics quantiles #525

# Fixed

- Allow users to use custom export kind selector #526

## Performance

- Improve simple span processor performance #502
- Local span perf improvements #505
- Reduce string allocations where possible #506

## [v0.13.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.12.0...v0.13.0)

Upgrade note: exporter pipelines do not return an uninstall guard as of #444,
use `opentelemetry::global::shutdown_tracer_provider` explicitly instead.

## Changed

- Pull configrations from environment variables by default when creating BatchSpanProcessor #445
- Convert doc links to intra-doc #466
- Switch to Cow for event names #471
- Use API to configure async runtime instead of features #481
- Rename trace config with_default_sampler to with_sampler #482

## Removed

- Removed tracer provider guard #444
- Removed `from_env` and use environment variables to initialize the configurations by default #459

## [v0.12.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.11.2...v0.12.0)

## Added

- Instrumentation library support #402
- Batch observer support #429
- `with_unit` methods in metrics #431
- Clone trait for noop tracer/tracer provider/span #479
- Abstracted traits for different runtimes #480

## Changed

- Dependencies updates #410
- Add `Send`, `Sync` to `AsyncInstrument` in metrics #422
- Add `Send`, `Sync` to `InstrumentCore` in metrics #423
- Replace regex with custom logic #411
- Update tokio to v1 #421

## Removed

- Moved `http` dependencies into a new opentelemetry-http crate #415
- Remove `tonic` dependency #414

## [v0.11.2](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.11.1...v0.11.2)

# Fixed

- Fix possible deadlock when dropping metric instruments #407

## [v0.11.1](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.11.0...v0.11.1)

# Fixed

- Fix remote implicit builder context sampling #405

## [v0.11.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.10.0...v0.11.0)

## Added

- Add `force_flush` method to span processors #358
- Add timeout for `force_flush` and `shutdown` #362

## Changed

- Implement Display trait for Key and Value types #353
- Remove Option from Array values #359
- Update `ShouldSample`'s parent parameter to be `Context` #368
- Consolidate error types in `trace` module into `TraceError` #371
- Add `#[must_use]` to uninstall structs #372
- Move 3rd party propagators and merge exporter into `sdk::export` #375
- Add instrumentation version to instrument config #392
- Use instrumentation library in metrics #393
- `start_from_context` renamed to `start_with_context` #399
- Removed `build_with_context` as full context is now stored in builder #399
- SpanBuilder's `with_parent` renamed to `with_parent_context` #399

# Fixed

- Fix parent based sampling in tracer #354
- StatusCode enum value ordering #377
- Counter adding the delta from last collection #395
- `HistogramAggregator` returning sum vs count #398

## [v0.10.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.9.1...v0.10.0)

## Added

- Add support for baggage metadata #287

## Changed

- Remove `api` prefix from modules #305
- Move `mark_as_active_span` and `get_active_span` functions into trace module #310
- Revert renaming of `SpanContext` to `SpanReference` #299
- Default trace propagator is now a no-op #329
- Return references to span contexts instead of clones #325
- Update exporter errors to be `Box<dyn Error + Send + Sync + 'static>` #284
- Rename `GenericProvider` to `GenericTracerProvider` #313
- Reduce `SpanStatus` enum to `Ok`, `Error`, and `Unset` variants #315
- update B3 propagator to more closely match spec #319
- Export missing pub global trace types #313
- Ensure kv array values are homogeneous #333
- Implement `Display` trait for `Key` and `Value` types #353
- Move `SpanProcessor` trait into `sdk` module #334
- Ensure `is_recording` is `false` and span is no-op after `end` #341
- Move binary propagator and base64 format to contrib #343
- Ensure metrics noop types go through constructors #345
- Change `ExportResult` to use `std::result::Result` #347
- Change `SpanExporter::export` to take `&mut self` instead of `&self` #350
- Add MSRV 1.42.0 #296

## Fixed

- Fix parent based sampling #354

## Removed

- Remove support for `u64` and `bytes` kv values #323
- Remove kv value conversion from `&str` #332

## [v0.9.1](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.9.0...v0.9.1)

## Added

- Allow metric instruments to be cloned #280

### Fixed

- Fix single threaded runtime tokio feature bug #278

## [v0.9.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.8.0...v0.9.0)

## Added

- Add resource detector #174
- Add `fields` method to TextMapFormat #178
- Add support for `tracestate` in `TraceContextPropagator` #191
- Propagate valid span context in noop tracer #197
- Add end_with_timestamp method for trace span #199
- Add ID methods for hex and byte array formatting #200
- Add AWS X-Ray ID Generator #201
- AWS X-Ray Trace Context Propagator #202
- Add instrumentation library information to spans #207
- Add keys method to extractors #209
- Add `TraceState` to `SpanContext` #217
- Add `from_env` config option for `BatchSpanProcessor` #228
- Add pipeline uninstall mechanism to shut down trace pipelines #229

### Changed

- Re-write metrics sdk to be spec compliant #179
- Rename `Sampler::Probability` to `Sampler::TraceIdRatioBased` #188
- Rename `HTTPTextPropagator` to `TextMapPropagator` #192
- Ensure extractors are case insensitive #193
- Rename `Provider` to `TracerProvider` #206
- Rename `CorrelationContext` into `Baggage` #208
- Pipeline builder for stdout trace exporter #224
- Switch to async exporters #232
- Allow `ShouldSample` implementation to modify trace state #237
- Ensure context guard is `!Send` #239
- Ensure trace noop structs use `new` constructor #240
- Switch to w3c `baggage` header #246
- Move trace module imports from `api` to `api::trace` #255
- Update `tonic` feature to use version `0.3.x` #258
- Update exporters to receive owned span data #264
- Move propagators to `sdk::propagation` #266
- Rename SpanContext to SpanReference #270
- Rename `SamplingDecision`'s `NotRecord`, `Record` and `RecordAndSampled` to
  `Drop` `RecordOnly` and `RecordAndSample` #247

## [v0.8.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.7.0...v0.8.0)

## Added

- Add custom span processors to `Provider::Builder` #166

### Changed

- Separate `Carrier` into `Injector` and `Extractor` #164
- Change the default sampler to be `ParentOrElse(AlwaysOn)` #163
- Move the `Sampler` interface to the SDK #169

## [v0.7.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.6.0...v0.7.0)

### Added

- New `ParentOrElse` sampler for fallback logic if parent is not sampled. #128
- Attributes can now have array values #146
- Added `record_exception` and `record_exception_with_stacktrace` methods to `Span` #152

### Changed

- Update sampler types #128
  - `Always` is now `AlwaysOn`. `Never` is now `AlwaysOff`. `Probability` now ignores parent
    sampled state.
- `base64` and `binary_propagator` have been moved to `experimental` module. #134
- `Correlation-Context` header has been updated to `otcorrelations` #145
- `B3Propagator` has been updated to more closely follow the spec #148

## [v0.6.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.5.0...v0.6.0)

### Added

- Add `http` and `tonic` features to impl `Carrier` for common types.

### Changed

- Removed `span_id` from sampling parameters when implementing custom samplers.

### Fixed

- Make `Context` `Send + Sync` in #127

## [v0.5.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.4.0...v0.5.0)

### Added

- Derive `Clone` for `B3Propagator`, `SamplingResult`, and `SpanBuilder`
- Ability to configure the span id / trace id generator
- impl `From<T>` for common `Key` and `Value` types
- Add global `tracer` method
- Add `Resource` API
- Add `Context` API
- Add `Correlations` API
- Add `HttpTextCompositePropagator` for composing `HttpTextPropagator`s
- Add `GlobalPropagator` for globally configuring a propagator
- Add `TraceContextExt` to provide methods for working with trace data in a context
- Expose `EvictedQueue` constructor

### Changed

- Ensure that impls of `Span` are `Send` and `Sync` when used in `global`
- Changed `Key` and `Value` method signatures to remove `Cow` references
- Tracer's `start` now uses the implicit current context instead of an explicit span context.
  `start_with_context` may be used to specify a context if desired.
- `with_span` now accepts a span for naming consistency and managing the active state of a more
  complex span (likely produced by a builder), and the previous functionality that accepts a
  `&str` has been renamed to `in_span`, both of which now yield a context to the provided closure.
- Tracer's `get_active_span` now accepts a closure
- The `Instrument` trait has been renamed to `FutureExt` to avoid clashing with metric instruments,
  and instead accepts contexts via `with_context`.
- Span's `get_context` method has been renamed to `span_context` to avoid ambiguity.
- `HttpTextPropagators` inject the current context instead of an explicit span context. The context
  can be specified with `inject_context`.
- `SpanData`'s `context` has been renamed to `span_context`

### Fixed

- Update the probability sampler to match the spec
- Rename `Traceparent` header to `traceparent`

### Removed

- `TracerGenerics` methods have been folded in to the `Tracer` trait so it is longer needed
- Tracer's `mark_span_as_inactive` has been removed
- Exporters no longer require an `as_any` method
- Span's `mark_as_active`, `mark_as_inactive`, and `as_any` have been removed

## [v0.4.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.3.0...v0.4.0)

### Added

- New async batch span processor
- New stdout exporter
- Add `trace_id` to `SpanBuilder`

### Changed

- Add `attributes` to `Event`s.
- Update `Span`'s `add_event` and `add_event_with_timestamp` to accept attributes.
- Record log fields in jaeger exporter
- Properly export span kind in jaeger exporter
- Add support for `Link`s
- Add `status_message` to `Span` and `SpanData`
- Rename `SpanStatus` to `StatusCode`
- Update `EvictedQueue` internals from LIFO to FIFO
- Switch span attributes to `EvictedHashMap`

### Fixed

- Call `shutdown` correctly when span processors and exporters are dropped

## [v0.3.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.2.0...v0.3.0)

### Added

- New Base64 propagator
- New SpanBuilder api
- Zipkin Exporter crate

### Changed

- Switch to `SpanId` and `TraceId` from `u64` and `u128`
- Remove `&mut self` requirements for `Span` API

### Fixed

- circular Tracer debug impl

## [v0.2.0](https://github.com/open-telemetry/opentelemetry-rust/compare/b5918251cc07f9f6957434ccddc35306f68bd791..v0.2.0)

### Added

- Make trace and metrics features optional
- ExportResult as specified in the specification
- Add Futures compatibility API
- Added serde serialise support to SpanData
- Separate OpenTelemetry Jaeger crate

### Changed

- Rename HttpTraceContextPropagator to TraceContextPropagator
- Rename HttpB3Propagator to B3Propagator
- Switch to Apache 2 license
- Resolve agent addresses to allow non-static IP
- Remove tracer name prefix from span name

### Removed

- Remove add_link from spans

## [v0.1.5](https://github.com/jtescher/opentelemetry-rust/compare/v0.1.4...v0.1.5)

### Added

- trace-context propagator

### Changed

- Prometheus API cleanup

## [v0.1.4](https://github.com/jtescher/opentelemetry-rust/compare/v0.1.3...v0.1.4)

### Added

- Parent option for default sampler

### Fixed

- SDK tracer default span id

## [v0.1.3](https://github.com/jtescher/opentelemetry-rust/compare/v0.1.2...v0.1.3)

### Changed

- Ensure spans are always send and sync
- Allow static lifetimes for span names
- Improve KeyValue ergonomics

## [v0.1.2](https://github.com/jtescher/opentelemetry-rust/compare/v0.1.1...v0.1.2)

### Added

- Implement global provider

## [v0.1.1](https://github.com/jtescher/opentelemetry-rust/compare/v0.1.0...v0.1.1)

### Added

- Documentation and API cleanup
- Tracking of active spans via thread local span stack

## [v0.1.0](https://github.com/jtescher/opentelemetry-rust/commit/ea368ea965aa035f46728d75e1be3b096b6cd6ec)

Initial debug alpha
