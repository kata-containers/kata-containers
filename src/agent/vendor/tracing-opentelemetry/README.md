![Tracing — Structured, application-level diagnostics][splash]

[splash]: https://raw.githubusercontent.com/tokio-rs/tracing/master/assets/splash.svg

# Tracing OpenTelemetry

Utilities for adding [OpenTelemetry] interoperability to [`tracing`].

[![Crates.io][crates-badge]][crates-url]
[![Documentation][docs-badge]][docs-url]
[![Documentation (master)][docs-master-badge]][docs-master-url]
[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]
[![Discord chat][discord-badge]][discord-url]
![maintenance status][maint-badge]

[Documentation][docs-url] | [Chat][discord-url]

[crates-badge]: https://img.shields.io/crates/v/tracing-opentelemetry.svg
[crates-url]: https://crates.io/crates/tracing-opentelemetry/0.13.0
[docs-badge]: https://docs.rs/tracing-opentelemetry/badge.svg
[docs-url]: https://docs.rs/tracing-opentelemetry/0.13.0/tracing_opentelemetry
[docs-master-badge]: https://img.shields.io/badge/docs-master-blue
[docs-master-url]: https://tracing-rs.netlify.com/tracing_opentelemetry
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE
[actions-badge]: https://github.com/tokio-rs/tracing/workflows/CI/badge.svg
[actions-url]:https://github.com/tokio-rs/tracing/actions?query=workflow%3ACI
[discord-badge]: https://img.shields.io/discord/500028886025895936?logo=discord&label=discord&logoColor=white
[discord-url]: https://discord.gg/EeF3cQw
[maint-badge]: https://img.shields.io/badge/maintenance-actively--developed-brightgreen.svg

## Overview

[`tracing`] is a framework for instrumenting Rust programs to collect
structured, event-based diagnostic information. This crate provides a layer
that connects spans from multiple systems into a trace and emits them to
[OpenTelemetry]-compatible distributed tracing systems for processing and
visualization.

The crate provides the following types:

* [`OpenTelemetryLayer`] adds OpenTelemetry context to all `tracing` [span]s.
* [`OpenTelemetrySpanExt`] allows OpenTelemetry parent trace information to be
  injected and extracted from a `tracing` [span].

[`OpenTelemetryLayer`]: https://docs.rs/tracing-opentelemetry/latest/tracing_opentelemetry/struct.OpenTelemetryLayer.html
[`OpenTelemetrySpanExt`]: https://docs.rs/tracing-opentelemetry/latest/tracing_opentelemetry/trait.OpenTelemetrySpanExt.html
[span]: https://docs.rs/tracing/latest/tracing/span/index.html
[`tracing`]: https://crates.io/crates/tracing
[OpenTelemetry]: https://opentelemetry.io/

*Compiler support: [requires `rustc` 1.42+][msrv]*

[msrv]: #supported-rust-versions

## Examples

### Basic Usage

```rust
use opentelemetry::exporter::trace::stdout;
use tracing::{error, span};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

fn main() {
    // Install a new OpenTelemetry trace pipeline
    let (tracer, _uninstall) = stdout::new_pipeline().install();

    // Create a tracing layer with the configured tracer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    // Use the tracing subscriber `Registry`, or any other subscriber
    // that impls `LookupSpan`
    let subscriber = Registry::default().with(telemetry);

    // Trace executed code
    tracing::subscriber::with_default(subscriber, || {
        // Spans will be sent to the configured OpenTelemetry exporter
        let root = span!(tracing::Level::TRACE, "app_start", work_units = 2);
        let _enter = root.enter();

        error!("This event will be logged in the root span.");
    });
}
```

### Visualization example

```console
# Run a supported collector like jaeger in the background
$ docker run -d -p6831:6831/udp -p6832:6832/udp -p16686:16686 jaegertracing/all-in-one:latest

# Run example to produce spans (from parent examples directory)
$ cargo run --example opentelemetry

# View spans (see the image below)
$ firefox http://localhost:16686/
```

![Jaeger UI](trace.png)

## Supported Rust Versions

Tracing is built against the latest stable release. The minimum supported
version is 1.42. The current Tracing version is not guaranteed to build on Rust
versions earlier than the minimum supported version.

Tracing follows the same compiler support policies as the rest of the Tokio
project. The current stable Rust compiler and the three most recent minor
versions before it will always be supported. For example, if the current stable
compiler version is 1.45, the minimum supported version will not be increased
past 1.42, three minor versions prior. Increasing the minimum supported compiler
version is not considered a semver breaking change as long as doing so complies
with this policy.

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Tracing by you, shall be licensed as MIT, without any additional
terms or conditions.
