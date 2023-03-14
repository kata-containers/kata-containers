![OpenTelemetry — An observability framework for cloud-native software.][splash]

[splash]: https://raw.githubusercontent.com/open-telemetry/opentelemetry-rust/main/assets/logo-text.png

# OpenTelemetry Rust

The Rust [OpenTelemetry](https://opentelemetry.io/) implementation.

[![Crates.io: opentelemetry](https://img.shields.io/crates/v/opentelemetry.svg)](https://crates.io/crates/opentelemetry)
[![Documentation](https://docs.rs/opentelemetry/badge.svg)](https://docs.rs/opentelemetry)
[![LICENSE](https://img.shields.io/crates/l/opentelemetry)](./LICENSE)
[![GitHub Actions CI](https://github.com/open-telemetry/opentelemetry-rust/workflows/CI/badge.svg)](https://github.com/open-telemetry/opentelemetry-rust/actions?query=workflow%3ACI+branch%3Amain)
[![codecov](https://codecov.io/gh/open-telemetry/opentelemetry-rust/branch/main/graph/badge.svg)](https://codecov.io/gh/open-telemetry/opentelemetry-rust)
[![Gitter chat](https://img.shields.io/badge/gitter-join%20chat%20%E2%86%92-brightgreen.svg)](https://gitter.im/open-telemetry/opentelemetry-rust)

[Website](https://opentelemetry.io/) |
[Chat](https://gitter.im/open-telemetry/opentelemetry-rust) |
[Documentation](https://docs.rs/opentelemetry)

## Overview

OpenTelemetry is a collection of tools, APIs, and SDKs used to instrument,
generate, collect, and export telemetry data (metrics, logs, and traces) for
analysis in order to understand your software's performance and behavior. You
can export and analyze them using [Prometheus], [Jaeger], and other
observability tools.

*Compiler support: [requires `rustc` 1.46+][msrv]*

[Prometheus]: https://prometheus.io
[Jaeger]: https://www.jaegertracing.io
[msrv]: #supported-rust-versions

## Getting Started

```rust
use opentelemetry::{global, sdk::export::trace::stdout, trace::Tracer};

fn main() {
    // Create a new trace pipeline that prints to stdout
    let tracer = stdout::new_pipeline().install_simple();

    tracer.in_span("doing_work", |cx| {
        // Traced app logic here...
    });

    // Shutdown trace pipeline
    global::shutdown_tracer_provider();
}
```

See the [examples](./examples) directory for different integration patterns.

## Ecosystem

### Related Crates

In addition to `opentelemetry`, the [`open-telemetry/opentelemetry-rust`]
repository contains several additional crates designed to be used with the
`opentelemetry` ecosystem. This includes a collection of trace `SpanExporter`
and metrics pull and push controller implementations, as well as utility and
adapter crates to assist in propagating state and instrumenting applications.

In particular, the following crates are likely to be of interest:

- [`opentelemetry-http`] provides an interface for injecting and extracting
  trace information from [`http`] headers.
- [`opentelemetry-jaeger`] provides a pipeline and exporter for sending trace
  information to [`Jaeger`].
- [`opentelemetry-otlp`] exporter for sending trace and metric data in the OTLP
  format to the OpenTelemetry collector.
- [`opentelemetry-prometheus`] provides a pipeline and exporter for sending
  metrics information to [`Prometheus`].
- [`opentelemetry-zipkin`] provides a pipeline and exporter for sending trace
  information to [`Zipkin`].
- [`opentelemetry-datadog`] provides additional exporters to [`Datadog`].
- [`opentelemetry-aws`] provides unofficial propagators for AWS X-ray.
- [`opentelemetry-contrib`] provides additional exporters and propagators that
  are experimental.
- [`opentelemetry-semantic-conventions`] provides standard names and semantic
  otel conventions.
- [`opentelemetry-stackdriver`] provides an exporter for Google's [Cloud Trace]
  (which used to be called StackDriver).

Additionally, there are also several third-party crates which are not
maintained by the `opentelemetry` project. These include:

- [`tracing-opentelemetry`] provides integration for applications instrumented
  using the [`tracing`] API and ecosystem.
- [`actix-web-opentelemetry`] provides integration for the [`actix-web`] web
  server and ecosystem.
- [`opentelemetry-application-insights`] provides an unofficial [Azure
  Application Insights] exporter.
- [`opentelemetry-tide`] provides integration for the [`Tide`] web server and
  ecosystem.

If you're the maintainer of an `opentelemetry` ecosystem crate not listed
above, please let us know! We'd love to add your project to the list!

[`open-telemetry/opentelemetry-rust`]: https://github.com/open-telemetry/opentelemetry-rust
[`opentelemetry-jaeger`]: https://crates.io/crates/opentelemetry-jaeger
[`Jaeger`]: https://www.jaegertracing.io
[`opentelemetry-otlp`]: https://crates.io/crates/opentelemetry-otlp
[`opentelemetry-http`]: https://crates.io/crates/opentelemetry-http
[`opentelemetry-prometheus`]: https://crates.io/crates/opentelemetry-prometheus
[`opentelemetry-aws`]: https://crates.io/crates/opentelemetry-aws
[`Prometheus`]: https://prometheus.io
[`opentelemetry-zipkin`]: https://crates.io/crates/opentelemetry-zipkin
[`Zipkin`]: https://zipkin.io
[`opentelemetry-contrib`]: https://crates.io/crates/opentelemetry-contrib
[`Datadog`]: https://www.datadoghq.com
[`opentelemetry-datadog`]: https://crates.io/crates/opentelemetry-datadog
[`opentelemetry-semantic-conventions`]: https://crates.io/crates/opentelemetry-semantic-conventions
[`http`]: https://crates.io/crates/http

[`tracing-opentelemetry`]: https://crates.io/crates/tracing-opentelemetry
[`tracing`]: https://crates.io/crates/tracing
[`actix-web-opentelemetry`]: https://crates.io/crates/actix-web-opentelemetry
[`actix-web`]: https://crates.io/crates/actix-web
[`opentelemetry-application-insights`]: https://crates.io/crates/opentelemetry-application-insights
[Azure Application Insights]: https://docs.microsoft.com/en-us/azure/azure-monitor/app/app-insights-overview
[`opentelemetry-tide`]: https://crates.io/crates/opentelemetry-tide
[`Tide`]: https://crates.io/crates/tide
[`opentelemetry-stackdriver`]: https://crates.io/crates/opentelemetry-stackdriver
[Cloud Trace]: https://cloud.google.com/trace/

## Supported Rust Versions

OpenTelemetry is built against the latest stable release. The minimum supported
version is 1.46. The current OpenTelemetry version is not guaranteed to build
on Rust versions earlier than the minimum supported version.

The current stable Rust compiler and the three most recent minor versions
before it will always be supported. For example, if the current stable compiler
version is 1.49, the minimum supported version will not be increased past 1.46,
three minor versions prior. Increasing the minimum supported compiler version
is not considered a semver breaking change as long as doing so complies with
this policy.

## Contributing

See the [contributing file](CONTRIBUTING.md).
