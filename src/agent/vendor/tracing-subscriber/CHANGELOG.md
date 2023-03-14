# 0.2.25 (October 5, 2021)

This release fixes an issue where a `Layer` implementation's custom
`downcast_raw` implementation was lost when wrapping that layer with a per-layer
filter.

### Fixed

- **registry**: Forward `Filtered::downcast_raw` to wrapped `Layer` ([#1619])

### Added

- Documentation improvements ([#1596], [#1601])

Thanks to @bryanburgers for contributing to this release!

[#1619]: https://github.com/tokio-rs/tracing/pull/1619
[#1601]: https://github.com/tokio-rs/tracing/pull/1601
[#1596]: https://github.com/tokio-rs/tracing/pull/1596

# 0.2.24 (September 19, 2021)

This release contains a number of bug fixes, including a fix for
`tracing-subscriber` failing to compile on the minimum supported Rust version of
1.42.0. It also adds `IntoIterator` implementations for the `Targets` type.

### Fixed

- Fixed compilation on Rust 1.42.0 ([#1580], [#1581])
- **registry**: Ensure per-layer filter `enabled` state is cleared when a global
  filter short-circuits filter evaluation ([#1575])
- **layer**: Fixed `Layer::on_layer` not being called for `Box`ed `Layer`s,
  which broke  per-layer filtering ([#1576])

### Added

- **filter**: Added `Targets::iter`, returning an iterator over the set of
  target-level pairs enabled by a `Targets` filter ([#1574])
- **filter**:  Added `IntoIterator` implementations for `Targets` and `&Targets`
  ([#1574])

Thanks to new contributor @connec for contributing to this release!

[#1580]: https://github.com/tokio-rs/tracing/pull/1580
[#1581]: https://github.com/tokio-rs/tracing/pull/1581
[#1575]: https://github.com/tokio-rs/tracing/pull/1575
[#1576]: https://github.com/tokio-rs/tracing/pull/1576
[#1574]: https://github.com/tokio-rs/tracing/pull/1574

# 0.2.23 (September 16, 2021)

This release fixes a few bugs in the per-layer filtering API added in v0.2.21.

### Fixed

- **env-filter**: Fixed excessive `EnvFilter` memory use ([#1568])
- **filter**: Fixed a panic that may occur in debug mode when using per-layer
  filters together with global filters ([#1569])
- Fixed incorrect documentation formatting ([#1572])

[#1568]: https://github.com/tokio-rs/tracing/pull/1568
[#1569]: https://github.com/tokio-rs/tracing/pull/1569
[#1572]: https://github.com/tokio-rs/tracing/pull/1572

# 0.2.22 (September 13, 2021)

This fixes a regression where the `filter::ParseError` type was accidentally
renamed.

### Fixed

- **filter**: Fix `filter::ParseError` accidentally being renamed to
  `filter::DirectiveParseError` ([#1558])

[#1558]: https://github.com/tokio-rs/tracing/pull/1558

# 0.2.21 (September 12, 2021)

This release introduces the [`Filter`] trait, a new API for [per-layer
filtering][plf]. This allows controlling which spans and events are recorded by
various layers individually, rather than globally.

In addition, it adds a new [`Targets`] filter, which provides a lighter-weight
version of the filtering provided by [`EnvFilter`], as well as other smaller API
improvements and fixes.

### Deprecated

- **registry**: `SpanRef::parent_id`, which cannot properly support per-layer
  filtering. Use `.parent().map(SpanRef::id)` instead. ([#1523])

### Fixed

- **layer** `Context` methods that are provided when the `Subscriber` implements
  `LookupSpan` no longer require the "registry" feature flag ([#1525])
- **layer** `fmt::Debug` implementation for `Layered` no longer requires the `S`
  type parameter to implement `Debug` ([#1528])

### Added

- **registry**: `Filter` trait, `Filtered` type, `Layer::with_filter` method,
  and other APIs for per-layer filtering ([#1523])
- **filter**: `FilterFn` and `DynFilterFn` types that implement global (`Layer`)
  and per-layer (`Filter`) filtering for closures and function pointers
  ([#1523])
- **filter**: `Targets` filter, which implements a lighter-weight form of
  `EnvFilter`-like filtering ([#1550])
- **env-filter**: Added support for filtering on floating-point values ([#1507])
- **layer**: `Layer::on_layer` callback, called when layering the `Layer` onto a
`Subscriber` ([#1523])
- **layer**: `Layer` implementations for `Box<L>` and `Arc<L>` where `L: Layer`
  ([#1536])
- **layer**: `Layer` implementations for `Box<dyn Layer<S> + Send + Sync + 'static>`
  and `Arc<dyn Layer<S> + Send + Sync + 'static>` ([#1536])
- A number of small documentation fixes and improvements ([#1553], [#1544],
  [#1539], [#1524])

Special thanks to new contributors @jsgf and @maxburke for contributing to this
release!

[`Filter`]: https://docs.rs/tracing-subscriber/0.2.21/tracing_subscriber/layer/trait.Filter.html
[plf]: https://docs.rs/tracing-subscriber/0.2.21/tracing_subscriber/layer/index.html#per-layer-filtering
[`Targets`]: https://docs.rs/tracing-subscriber/0.2.21/tracing_subscriber/filter/struct.Targets.html
[`EnvFilter`]: https://docs.rs/tracing-subscriber/0.2.21/tracing_subscriber/filter/struct.EnvFilter.html
[#1507]: https://github.com/tokio-rs/tracing/pull/1507
[#1523]: https://github.com/tokio-rs/tracing/pull/1523
[#1524]: https://github.com/tokio-rs/tracing/pull/1524
[#1525]: https://github.com/tokio-rs/tracing/pull/1525
[#1528]: https://github.com/tokio-rs/tracing/pull/1528
[#1539]: https://github.com/tokio-rs/tracing/pull/1539
[#1544]: https://github.com/tokio-rs/tracing/pull/1544
[#1550]: https://github.com/tokio-rs/tracing/pull/1550
[#1553]: https://github.com/tokio-rs/tracing/pull/1553

# 0.2.20 (August 17, 2021)

### Fixed

- **fmt**: Fixed `fmt` printing only the first `source` for errors with a chain
  of sources ([#1460])
- **fmt**: Fixed missing space between level and event in the `Pretty` formatter
  ([#1498])
- **json**: Fixed `Json` formatter not honoring `without_time` and `with_level`
  configurations ([#1463])

### Added

- **registry**: Improved panic message when cloning a span whose ID doesn't
  exist, to aid in debugging issues with multiple subscribers ([#1483])
- **registry**: Improved documentation on span ID generation ([#1453])

[#1460]: https://github.com/tokio-rs/tracing/pull/1460
[#1483]: https://github.com/tokio-rs/tracing/pull/1483
[#1463]: https://github.com/tokio-rs/tracing/pull/1463
[#1453]: https://github.com/tokio-rs/tracing/pull/1453
[#1498]: https://github.com/tokio-rs/tracing/pull/1498

Thanks to new contributors @joshtriplett and @lerouxrgd, and returning
contributor @teozkr, for contributing to this release!

# 0.2.19 (June 25, 2021)

### Deprecated

- **registry**: `SpanRef::parents`, `SpanRef::from_root`, and `Context::scope`
  iterators, which are replaced by new `SpanRef::scope` and `Scope::from_root`
  iterators ([#1413])

### Added

- **registry**: `SpanRef::scope` method, which returns a leaf-to-root `Iterator`
  including the leaf span ([#1413])
- **registry**: `Scope::from_root` method, which reverses the `scope` iterator
  to iterate root-to-leaf ([#1413])
- **registry**: `Context::event_span` method, which looks up the parent span of
  an event ([#1434])
- **registry**: `Context::event_scope` method, returning a `Scope` iterator over
  the span scope of an event ([#1434])
- **fmt**: `MakeWriter::make_writer_for` method, which allows returning a
  different writer based on a span or event's metadata ([#1141])
- **fmt**: `MakeWriterExt` trait, with `with_max_level`, `with_min_level`,
  `with_filter`, `and`, and `or_else` combinators ([#1274])
- **fmt**: `MakeWriter` implementation for `Arc<W> where &W: io::Write`
  ([#1274])

Thanks to @teozkr and @Folyd for contributing to this release!

[#1413]: https://github.com/tokio-rs/tracing/pull/1413
[#1434]: https://github.com/tokio-rs/tracing/pull/1434
[#1141]: https://github.com/tokio-rs/tracing/pull/1141
[#1274]: https://github.com/tokio-rs/tracing/pull/1274

# 0.2.18 (April 30, 2021)

### Deprecated

- Deprecated the `CurrentSpan` type, which is inefficient and largely superseded
  by the `registry` API ([#1321])

### Fixed

- **json**: Invalid JSON emitted for events in spans with no fields ([#1333])
- **json**: Missing span data for synthesized new span, exit, and close events
  ([#1334])
- **fmt**: Extra space before log lines when timestamps are disabled ([#1355])

### Added

- **env-filter**: Support for filters on spans whose names contain any
  characters other than `{` and `]` ([#1368])

Thanks to @Folyd, and new contributors @akinnane and @aym-v for contributing to
this release!

[#1321]: https://github.com/tokio-rs/tracing/pull/1321
[#1333]: https://github.com/tokio-rs/tracing/pull/1333
[#1334]: https://github.com/tokio-rs/tracing/pull/1334
[#1355]: https://github.com/tokio-rs/tracing/pull/1355
[#1368]: https://github.com/tokio-rs/tracing/pull/1368

# 0.2.17 (March 12, 2021)

### Fixed

- **fmt**: `Pretty` formatter now honors `with_ansi(false)` to disable ANSI
  terminal formatting ([#1240])
- **fmt**: Fixed extra padding when using `Pretty` formatter ([#1275])
- **chrono**: Removed extra trailing space with `ChronoLocal` time formatter
  ([#1103])

### Added

- **fmt**: Added `FmtContext::current_span()` method, returning the current span
  ([#1290])
- **fmt**: `FmtSpan` variants may now be combined using the `|` operator for
  more granular control over what span events are generated ([#1277])

Thanks to new contributors @cratelyn, @dignati, and @zicklag, as well as @Folyd,
@matklad, and @najamelan, for contributing to this release!

[#1240]: https://github.com/tokio-rs/tracing/pull/1240
[#1275]: https://github.com/tokio-rs/tracing/pull/1275
[#1103]: https://github.com/tokio-rs/tracing/pull/1103
[#1290]: https://github.com/tokio-rs/tracing/pull/1290
[#1277]: https://github.com/tokio-rs/tracing/pull/1277

# 0.2.16 (February 19, 2021)

### Fixed

- **env-filter**: Fixed directives where the level is in mixed case (such as
  `Info`) failing to parse ([#1126])
- **fmt**: Fixed `fmt::Subscriber` not providing a max-level hint ([#1251])
- `tracing-subscriber` no longer enables `tracing` and `tracing-core`'s default
  features ([#1144])
  
### Changed

- **chrono**: Updated `chrono` dependency to 0.4.16 ([#1189])
- **log**: Updated `tracing-log` dependency to 0.1.2

Thanks to @salewski, @taiki-e, @davidpdrsn and @markdingram for contributing to
this release!

[#1126]: https://github.com/tokio-rs/tracing/pull/1126
[#1251]: https://github.com/tokio-rs/tracing/pull/1251
[#1144]: https://github.com/tokio-rs/tracing/pull/1144
[#1189]: https://github.com/tokio-rs/tracing/pull/1189

# 0.2.15 (November 2, 2020)

### Fixed

- **fmt**: Fixed wrong lifetime parameters on `FormatFields` impl for
  `FmtContext` ([#1082])

### Added

- **fmt**: `format::Pretty`, an aesthetically pleasing, human-readable event
  formatter for local development and user-facing CLIs ([#1080])
- **fmt**: `FmtContext::field_format`, which returns the subscriber's field
  formatter ([#1082])

[#1082]: https://github.com/tokio-rs/tracing/pull/1082
[#1080]: https://github.com/tokio-rs/tracing/pull/1080

# 0.2.14 (October 22, 2020)

### Fixed

- **registry**: Fixed `Registry::new` allocating an excessively large amount of
  memory, most of which would never be used ([#1064])

### Changed

- **registry**: Improved `new_span` performance by reusing `HashMap` allocations
  for `Extensions` ([#1064])
- **registry**: Significantly improved the performance of `Registry::enter` and
  `Registry::exit` ([#1058])

[#1064]: https://github.com/tokio-rs/tracing/pull/1064
[#1058]: https://github.com/tokio-rs/tracing/pull/1058

# 0.2.13 (October 7, 2020)

### Changed

- Updated `tracing-core` to 0.1.17 ([#992])

### Added

- **env-filter**: Added support for filtering on targets which contain dashes
  ([#1014])
- **env-filter**: Added a warning when creating an `EnvFilter` that contains
  directives that would enable a level disabled by the `tracing` crate's
  `static_max_level` features ([#1021])

Thanks to @jyn514 and @bkchr for contributing to this release!

[#992]: https://github.com/tokio-rs/tracing/pull/992
[#1014]: https://github.com/tokio-rs/tracing/pull/1014
[#1021]: https://github.com/tokio-rs/tracing/pull/1021

# 0.2.12 (September 11, 2020)

### Fixed

- **env-filter**: Fixed a regression where `Option<Level>` lost its
  `Into<LevelFilter>` impl ([#966])
- **env-filter**: Fixed `EnvFilter` enabling spans that should not be enabled
  when multiple subscribers are in use ([#927])

### Changed

- **json**: `format::Json` now outputs fields in a more readable order ([#892])
- Updated `tracing-core` dependency to 0.1.16

### Added

- **fmt**: Add `BoxMakeWriter` for erasing the type of a `MakeWriter`
  implementation ([#958])
- **fmt**: Add `TestWriter` `MakeWriter` implementation to support libtest
  output capturing ([#938])
- **layer**: Add `Layer` impl for `Option<T> where T: Layer` ([#910])
- **env-filter**: Add `From<Level>` impl for `Directive` ([#918])
- Multiple documentation fixes and improvements

Thanks to @Pothulapati, @samrg472, @bryanburgers, @keetonian, and @SriRamanujam
for contributing to this release!

[#927]: https://github.com/tokio-rs/tracing/pull/927
[#966]: https://github.com/tokio-rs/tracing/pull/966
[#958]: https://github.com/tokio-rs/tracing/pull/958
[#892]: https://github.com/tokio-rs/tracing/pull/892
[#938]: https://github.com/tokio-rs/tracing/pull/938
[#910]: https://github.com/tokio-rs/tracing/pull/910
[#918]: https://github.com/tokio-rs/tracing/pull/918

# 0.2.11 (August 10, 2020)

### Fixed

- **env-filter**: Incorrect max level hint when filters involving span field
  values are in use (#907)
- **registry**: Fixed inconsistent span stacks when multiple registries are in
  use on the same thread (#901)

### Changed

- **env-filter**: `regex` dependency enables fewer unused feature flags (#899)

Thanks to @bdonlan and @jeromegn for contributing to this release!

# 0.2.10 (July 31, 2020)

### Fixed

- **docs**: Incorrect formatting (#862)

### Changed

- **filter**: `LevelFilter` is now a re-export of the
  `tracing_core::LevelFilter` type, it can now be used interchangably with the
  versions in `tracing` and `tracing-core` (#853)
- **filter**: Significant performance improvements when comparing `LevelFilter`s
  and `Level`s (#853)
- Updated the minimum `tracing-core` dependency to 0.1.12 (#853)

### Added

- **filter**: `LevelFilter` and `EnvFilter` now participate in `tracing-core`'s
  max level hinting, improving performance significantly in some use cases where
  levels are disabled globally (#853)

# 0.2.9 (July 23, 2020)

### Fixed

- **fmt**: Fixed compilation failure on MSRV when the `chrono` feature is
  disabled (#844)

### Added

- **fmt**: Span lookup methods defined by `layer::Context` are now also provided
  by `FmtContext` (#834)

# 0.2.8 (July 17, 2020)

### Changed

- **fmt**: When the `chrono` dependency is enabled, the `SystemTime` timestamp
  formatter now emits human-readable timestamps rather than using `SystemTime`'s
  `fmt::Debug`implementation (`chrono` is still required for customized
  timestamp formatting) (#807)
- **ansi**: Updated `ansi_term` dependency to 0.12 (#816)

### Added

- **json**: `with_span_list` method to configure the JSON formatter to include a
  list of all spans in the current trace in formatting events (similarly to the
  text formatter) (#741) 
- **json**: `with_current_span` method to configure the JSON formatter to include
  a field for the _current_ span (the leaf of the trace) in formatted events
  (#741)
- **fmt**: `with_thread_names` and `with_thread_ids` methods to configure
  `fmt::Subscriber`s and `fmt::Layer`s to include the thread name and/or thread ID
  of the current thread when formatting events (#818)
  
Thanks to new contributors @mockersf, @keetonian, and @Pothulapati for
contributing to this release!

# 0.2.7 (July 1, 2020)

### Changed

- **parking_lot**: Updated the optional `parking_lot` dependency to accept the
  latest `parking_lot` version (#774)

### Fixed

- **fmt**: Fixed events with explicitly overridden parent spans being formatted
  as though they were children of the current span (#767)

### Added

- **fmt**: Added the option to print synthesized events when spans are created,
  entered, exited, and closed, including span durations (#761)
- Documentation clarification and improvement (#762, #769)

Thanks to @rkuhn, @greenwoodcm, and @Ralith for contributing to this release!

# 0.2.6 (June 19, 2020)

### Fixed

- **fmt**: Fixed an issue in the JSON formatter where using `Span::record` would
  result in malformed spans (#709)

# 0.2.5 (April 21, 2020)

### Changed

- **fmt**: Bump sharded-slab dependency (#679)

### Fixed

- **fmt**: remove trailing space in `ChronoUtc` `format_time` (#677)

# 0.2.4 (April 6, 2020)

This release includes several API ergonomics improvements, including shorthand
constructors for many types, and an extension trait for initializing subscribers
using method-chaining style. Additionally, several bugs in less commonly used
`fmt` APIs were fixed.

### Added

- **fmt**: Shorthand free functions for constructing most types in `fmt`
  (including `tracing_subscriber::fmt()` to return a `SubscriberBuilder`,
  `tracing_subscriber::fmt::layer()` to return a format `Layer`, etc) (#660)
- **registry**: Shorthand free function `tracing_subscriber::registry()` to
  construct a new registry (#660)
- Added `SubscriberInitExt` extension trait for more ergonomic subscriber
  initialization (#660)
  
### Changed

- **fmt**: Moved `LayerBuilder` methods to `Layer` (#655)

### Deprecated

- **fmt**: `LayerBuilder`, as `Layer` now implements all builder methods (#655)
  
### Fixed

- **fmt**: Fixed `Compact` formatter not omitting levels with
  `with_level(false)` (#657)
- **fmt**: Fixed `fmt::Layer` duplicating the fields for a new span if another
  layer has already formatted its fields (#634)
- **fmt**: Added missing space when using `record` to add new fields to a span
  that already has fields (#659)
- Updated outdated documentation (#647)


# 0.2.3 (March 5, 2020)

### Fixed

- **env-filter**: Regression where filter directives were selected in the order
  they were listed, rather than most specific first (#624)

# 0.2.2 (February 27, 2020)

### Added

- **fmt**: Added `flatten_event` to `SubscriberBuilder` (#599)
- **fmt**: Added `with_level` to `SubscriberBuilder` (#594)

# 0.2.1 (February 13, 2020)

### Changed

- **filter**: `EnvFilter` directive selection now behaves correctly (i.e. like
  `env_logger`) (#583)

### Fixed

- **filter**: Fixed `EnvFilter` incorrectly allowing less-specific filter
  directives to enable events that are disabled by more-specific filters (#583)
- **filter**: Multiple significant `EnvFilter` performance improvements,
  especially when filtering events generated by `log` records (#578, #583)
- **filter**: Replaced `BTreeMap` with `Vec` in `DirectiveSet`, improving
  iteration performance significantly with typical numbers of filter directives
  (#580)

A big thank-you to @samschlegel for lots of help with `EnvFilter` performance
tuning in this release!

# 0.2.0 (February 4, 2020)

### Breaking Changes

- **fmt**: Renamed `Context` to `FmtContext` (#420, #425)
- **fmt**: Renamed `Builder` to `SubscriberBuilder` (#420)
- **filter**: Removed `Filter`. Use `EnvFilter` instead (#434)

### Added

- **registry**: `Registry`, a `Subscriber` implementation that `Layer`s can use
  as a high-performance, in-memory span store. (#420, #425, #432, #433, #435)
- **registry**: Added `LookupSpan` trait, implemented by `Subscriber`s to expose
  stored span data to `Layer`s (#420)
- **fmt**: Added `fmt::Layer`, to allow composing log formatting with other `Layer`s
- **fmt**: Added support for JSON field and event formatting (#377, #415)
- **filter**: Documentation for filtering directives (#554)

### Changed

- **fmt**: Renamed `Context` to `FmtContext` (#420, #425) (BREAKING)
- **fmt**: Renamed `Builder` to `SubscriberBuilder` (#420) (BREAKING)
- **fmt**: Reimplemented `fmt::Subscriber` in terms of the `Registry`
  and `Layer`s (#420)

### Removed

- **filter**: Removed `Filter`. Use `EnvFilter` instead (#434) (BREAKING)

### Fixed

- **fmt**: Fixed memory leaks in the slab used to store per-span data
  (3c35048)
- **fmt**: `fmt::SubscriberBuilder::init` not setting up `log` compatibility
  (#489)
- **fmt**: Spans closed by a child span closing not also closing _their_
  parents (#514)
- **Layer**: Fixed `Layered` subscribers failing to downcast to their own type
  (#549)
- **Layer**: Fixed `Layer::downcast_ref` returning invalid references (#454)

# 0.2.0-alpha.6 (February 3, 2020)

### Fixed

- **fmt**: Fixed empty `{}` printed after spans with no fields (f079f2d)
- **fmt**: Fixed inconsistent formatting when ANSI colors are disabled (506a482)
- **fmt**: Fixed mis-aligned levels when ANSI colors are disabled (eba1adb)
- Fixed warnings on nightly Rust compilers (#558)

# 0.2.0-alpha.5 (January 31, 2020)

### Added

- **env_filter**: Documentation for filtering directives (#554)
- **registry**, **env_filter**: Updated `smallvec` dependency to 0.1 (#543)

### Fixed

- **registry**: Fixed a memory leak in the slab used to store per-span data
  (3c35048)
- **Layer**: Fixed `Layered` subscribers failing to downcast to their own type
  (#549)
- **fmt**: Fixed a panic when multiple layers insert `FormattedFields`
  extensions from the same formatter type (1c3bb70)
- **fmt**: Fixed `fmt::Layer::on_record` inserting a new `FormattedFields` when
  formatted fields for a span already exist (1c3bb70)

# 0.2.0-alpha.4 (January 11, 2020)

### Fixed

- **registry**: Removed inadvertently committed `dbg!` macros (#533)

# 0.2.0-alpha.3 (January 10, 2020)

### Added

- **fmt**: Public `FormattedFields::new` constructor (#478)
- **fmt**: Added examples to `fmt::Layer` documentation (#510)
- Documentation now shows what feature flags are required by each API item (#525)

### Fixed

- **fmt**: Missing space between timestamp and level (#480)
- **fmt**: Incorrect formatting with `with_target(false)` (#481)
- **fmt**: `fmt::SubscriberBuilder::init` not setting up `log` compatibility
  (#489)
- **registry**: Spans exited out of order not being closed properly on exit
  (#509)
- **registry**: Memory leak when spans are closed by a child span closing (#514)
- **registry**: Spans closed by a child span closing not also closing _their_
  parents (#514)
- Compilation errors with `no-default-features` (#499, #500)

# 0.2.0-alpha.2 (December 8, 2019)

### Added

- `LookupSpans` implementation for `Layered` (#448)
- `SpanRef::from_root` to iterate over a span's parents from the root (#460)
- `Context::scope`, to iterate over the current context from the root (#460)
- `Context::lookup_current`, which returns a `SpanRef` to the current
  span's data (#460)

### Changed

- Lifetimes on some new `Context` methods to be less restrictive (#460)

### Fixed

- `Layer::downcast_ref` returning invalid references (#454)
- Compilation failure on 32-bit platforms (#462)
- Compilation failure with ANSI formatters (#438)

# 0.2.0-alpha.1 (November 18, 2019)

### Added

- `Registry`, a reusable span store that `Layer`s can use a
  high-performance, in-memory store. (#420, #425, #432, #433, #435)
- Reimplemented `fmt::Subscriber` in terms of the `Registry`
  and `Layer`s (#420)
- Add benchmarks for fmt subscriber (#421)
- Add support for JSON field and event formatting (#377, #415)

### Changed

- **BREAKING**: Change `fmt::format::FormatFields` and
  `fmt::format::FormatEvent` to accept a mandatory `FmtContext`. These
  `FormatFields` and `FormatEvent` will likely see additional breaking
  changes in subsequent alpha. (#420, #425)
- **BREAKING**: Removed `Filter`. Use `EnvFilter` instead (#434)

### Contributers

Thanks to all the contributers to this release!

- @pimeys for #377 and #415

# 0.1.6 (October 29, 2019)

### Added

- Add `init` and `try_init` functions to `FmtSubscriber` (#385)
- Add `ChronoUtc` and `ChronoLocal` timers, RFC 3339 support (#387)
- Add `tracing::subscriber::set_default` which sets the default
  subscriber and returns a drop guard. This drop guard will reset the
  dispatch on drop (#388).

### Fixed

- Fix default level for `EnvFilter`. Setting `RUST_LOG=target`
  previously only the `ERROR` level, while it should enable everything.
  `tracing-subscriber` now defaults to `TRACE` if no level is specified
  (#401)
- Fix `tracing-log` feature flag for init + try_init. The feature flag
  `tracing_log` was used instead of the correct `tracing-log`. As a
  result, both `tracing-log` and `tracing_log` needed to be specified in
  order to initialize the global logger. Only `tracing-log` needs to be
  specified now (#400).

### Contributers

Thanks to all the contributers to this release!

- @emschwartz for #385, #387, #400 and #401
- @bIgBV for #388

# 0.1.5 (October 7, 2019)

### Fixed

- Spans not being closed properly when `FmtSubscriber::current_span` is used
  (#371)

# 0.1.4 (September 26, 2019)

### Fixed

- Spans entered twice on the same thread sometimes being completely exited when
  the more deeply-nested entry is exited (#361)
- Setting `with_ansi(false)` on `FmtSubscriber` not disabling ANSI color
  formatting for timestamps (#354)
- Incorrect reference counting in `FmtSubscriber` that could cause spans to not
  be closed when all references are dropped (#366)

# 0.1.3 (September 16, 2019)

### Fixed

- `Layered` subscribers not properly forwarding calls to `current_span`
  (#350)

# 0.1.2 (September 12, 2019)

### Fixed

- `EnvFilter` ignoring directives with targets that are the same number of
  characters (#333)
- `EnvFilter` failing to properly apply filter directives to events generated
  from `log` records by`tracing-log` (#344)

### Changed

- Renamed `Filter` to `EnvFilter`, deprecated `Filter` (#339)
- Renamed "filter" feature flag to "env-filter", deprecated "filter" (#339)
- `FmtSubscriber` now defaults to enabling only the `INFO` level and above when
  a max level filter or `EnvFilter` is not set (#336)
- Made `parking_lot` dependency an opt-in feature flag (#348)

### Added

- `EnvFilter::add_directive` to add new directives to filters after they are
  constructed (#334)
- `fmt::Builder::with_max_level` to set a global level filter for a
  `FmtSubscriber` without requiring the use of `EnvFilter` (#336)
- `Layer` implementation for `LevelFilter` (#336)
- `EnvFilter` now implements `fmt::Display` (#329)

### Removed

- Removed dependency on `crossbeam-util` (#348)

# 0.1.1 (September 4, 2019)

### Fixed

- Potential double panic in `CurrentSpan` (#325)

# 0.1.0 (September 3, 2019)

- Initial release
