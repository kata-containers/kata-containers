use crate::sdk;
use crate::trace::{IdGenerator, SpanId, TraceId};
use std::time::{Duration, UNIX_EPOCH};

/// Generates AWS X-Ray compliant Trace and Span ids.
///
/// Generates OpenTelemetry formatted `TraceId`'s and `SpanId`'s. The `TraceId`'s are generated so
/// they can be backed out into X-Ray format by the [AWS X-Ray Exporter][xray-exporter] in the
/// [OpenTelemetry Collector][otel-collector].
///
/// ## Trace ID Format
///
/// A `trace_id` consists of three numbers separated by hyphens. For example, `1-58406520-a006649127e371903a2de979`.
/// This includes:
///
/// * The version number, that is, 1.
/// * The time of the original request, in Unix epoch time, in 8 hexadecimal digits.
/// * For example, 10:00AM December 1st, 2016 PST in epoch time is 1480615200 seconds, or 58406520 in hexadecimal digits.
/// * A 96-bit identifier for the trace, globally unique, in 24 hexadecimal digits.
///
/// See the [AWS X-Ray Documentation][xray-trace-id] for more details.
///
/// ## Example
///
/// ```
/// use opentelemetry::trace::noop::NoopSpanExporter;
/// use opentelemetry::sdk::trace::{self, TracerProvider, XrayIdGenerator};
///
/// let _provider: TracerProvider = TracerProvider::builder()
///     .with_simple_exporter(NoopSpanExporter::new())
///     .with_config(trace::config().with_id_generator(XrayIdGenerator::default()))
///     .build();
/// ```
///
/// [otel-collector]: https://github.com/open-telemetry/opentelemetry-collector-contrib#opentelemetry-collector-contrib
/// [xray-exporter]: https://godoc.org/github.com/open-telemetry/opentelemetry-collector-contrib/exporter/awsxrayexporter
/// [xray-trace-id]: https://docs.aws.amazon.com/xray/latest/devguide/xray-api-sendingdata.html#xray-api-traceids
#[derive(Debug, Default)]
pub struct XrayIdGenerator {
    sdk_default_generator: sdk::trace::IdGenerator,
}

impl IdGenerator for XrayIdGenerator {
    /// Generates a new `TraceId` that can be converted to an X-Ray Trace ID
    fn new_trace_id(&self) -> TraceId {
        let mut default_trace_id: String = format!(
            "{:024x}",
            self.sdk_default_generator.new_trace_id().to_u128()
        );

        default_trace_id.truncate(24);

        let epoch_time_seconds: u64 = crate::time::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_secs();

        TraceId::from_hex(format!("{:08x}{}", epoch_time_seconds, default_trace_id).as_str())
    }

    /// Generates a new `SpanId` that can be converted to an X-Ray Segment ID
    fn new_span_id(&self) -> SpanId {
        self.sdk_default_generator.new_span_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_trace_id_generation() {
        let before: u64 = crate::time::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        sleep(Duration::from_secs(1));

        let generator: XrayIdGenerator = XrayIdGenerator::default();
        let trace_id: TraceId = generator.new_trace_id();

        sleep(Duration::from_secs(1));
        let after: u64 = crate::time::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let trace_as_hex: String = format!("{:032x}", trace_id.to_u128());
        let (timestamp, _xray_id) = trace_as_hex.split_at(8_usize);

        let trace_time: u64 = u64::from_str_radix(timestamp, 16).unwrap();

        assert!(before <= trace_time);
        assert!(after >= trace_time);
    }
}
