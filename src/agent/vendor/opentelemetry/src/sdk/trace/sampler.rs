//! # OpenTelemetry ShouldSample Interface
//!
//! ## Sampling
//!
//! Sampling is a mechanism to control the noise and overhead introduced by
//! OpenTelemetry by reducing the number of samples of traces collected and
//! sent to the backend.
//!
//! Sampling may be implemented on different stages of a trace collection.
//! OpenTelemetry SDK defines a `ShouldSample` interface that can be used at
//! instrumentation points by libraries to check the sampling `SamplingDecision`
//! early and optimize the amount of telemetry that needs to be collected.
//!
//! All other sampling algorithms may be implemented on SDK layer in exporters,
//! or even out of process in Agent or Collector.
//!
//! The OpenTelemetry API has two properties responsible for the data collection:
//!
//! * `is_recording` method on a `Span`. If `true` the current `Span` records
//!   tracing events (attributes, events, status, etc.), otherwise all tracing
//!   events are dropped. Users can use this property to determine if expensive
//!   trace events can be avoided. `SpanProcessor`s will receive
//!   all spans with this flag set. However, `SpanExporter`s will
//!   not receive them unless the `Sampled` flag was set.
//! * `Sampled` flag in `trace_flags` on `SpanContext`. This flag is propagated
//!   via the `SpanContext` to child Spans. For more details see the [W3C
//!   specification](https://w3c.github.io/trace-context/). This flag indicates
//!   that the `Span` has been `sampled` and will be exported. `SpanProcessor`s
//!   and `SpanExporter`s will receive spans with the `Sampled` flag set for
//!   processing.
//!
//! The flag combination `Sampled == false` and `is_recording` == true` means
//! that the current `Span` does record information, but most likely the child
//! `Span` will not.
//!
//! The flag combination `Sampled == true` and `is_recording == false` could
//! cause gaps in the distributed trace, and because of this OpenTelemetry API
//! MUST NOT allow this combination.

use crate::{
    trace::{Link, SpanKind, TraceContextExt, TraceId, TraceState},
    Context, KeyValue,
};

/// The `ShouldSample` interface allows implementations to provide samplers
/// which will return a sampling `SamplingResult` based on information that
/// is typically available just before the `Span` was created.
pub trait ShouldSample: Send + Sync + std::fmt::Debug {
    /// Returns the `SamplingDecision` for a `Span` to be created.
    #[allow(clippy::too_many_arguments)]
    fn should_sample(
        &self,
        parent_context: Option<&Context>,
        trace_id: TraceId,
        name: &str,
        span_kind: &SpanKind,
        attributes: &[KeyValue],
        links: &[Link],
    ) -> SamplingResult;
}

/// The result of sampling logic for a given `Span`.
#[derive(Clone, Debug, PartialEq)]
pub struct SamplingResult {
    /// `SamplingDecision` reached by this result
    pub decision: SamplingDecision,
    /// Extra attributes added by this result
    pub attributes: Vec<KeyValue>,
    /// Trace state from parent context, might be modified by sampler
    pub trace_state: TraceState,
}

/// Decision about whether or not to sample
#[derive(Clone, Debug, PartialEq)]
pub enum SamplingDecision {
    /// `is_recording() == false`, span will not be recorded and all events and
    /// attributes will be dropped.
    Drop,
    /// `is_recording() == true`, but `Sampled` flag MUST NOT be set.
    RecordOnly,
    /// `is_recording() == true` AND `Sampled` flag` MUST be set.
    RecordAndSample,
}

/// Sampling options
#[derive(Clone, Debug)]
pub enum Sampler {
    /// Always sample the trace
    AlwaysOn,
    /// Never sample the trace
    AlwaysOff,
    /// Respects the parent span's sampling decision or delegates a delegate sampler for root spans.
    ParentBased(Box<Sampler>),
    /// Sample a given fraction of traces. Fractions >= 1 will always sample. If the parent span is
    /// sampled, then it's child spans will automatically be sampled. Fractions < 0 are treated as
    /// zero, but spans may still be sampled if their parent is.
    TraceIdRatioBased(f64),
}

impl ShouldSample for Sampler {
    fn should_sample(
        &self,
        parent_context: Option<&Context>,
        trace_id: TraceId,
        name: &str,
        span_kind: &SpanKind,
        attributes: &[KeyValue],
        links: &[Link],
    ) -> SamplingResult {
        let decision = match self {
            // Always sample the trace
            Sampler::AlwaysOn => SamplingDecision::RecordAndSample,
            // Never sample the trace
            Sampler::AlwaysOff => SamplingDecision::Drop,
            // The parent decision if sampled; otherwise the decision of delegate_sampler
            Sampler::ParentBased(delegate_sampler) => {
                parent_context.filter(|cx| cx.has_active_span()).map_or(
                    delegate_sampler
                        .should_sample(parent_context, trace_id, name, span_kind, attributes, links)
                        .decision,
                    |ctx| {
                        let span = ctx.span();
                        let parent_span_context = span.span_context();
                        if parent_span_context.is_sampled() {
                            SamplingDecision::RecordAndSample
                        } else {
                            SamplingDecision::Drop
                        }
                    },
                )
            }
            // Probabilistically sample the trace.
            Sampler::TraceIdRatioBased(prob) => {
                if *prob >= 1.0 {
                    SamplingDecision::RecordAndSample
                } else {
                    let prob_upper_bound = (prob.max(0.0) * (1u64 << 63) as f64) as u64;
                    // The trace_id is already randomly generated, so we don't need a new one here
                    let rnd_from_trace_id = (trace_id.to_u128() as u64) >> 1;

                    if rnd_from_trace_id < prob_upper_bound {
                        SamplingDecision::RecordAndSample
                    } else {
                        SamplingDecision::Drop
                    }
                }
            }
        };

        SamplingResult {
            decision,
            // No extra attributes ever set by the SDK samplers.
            attributes: Vec::new(),
            // all sampler in SDK will not modify trace state.
            trace_state: match parent_context {
                Some(ctx) => ctx.span().span_context().trace_state().clone(),
                None => TraceState::default(),
            },
        }
    }
}

#[cfg(all(test, feature = "testing", feature = "trace"))]
mod tests {
    use super::*;
    use crate::sdk::trace::{Sampler, SamplingDecision, ShouldSample};
    use crate::testing::trace::TestSpan;
    use crate::trace::{SpanContext, SpanId, TraceState, TRACE_FLAG_SAMPLED};
    use rand::Rng;

    #[rustfmt::skip]
    fn sampler_data() -> Vec<(&'static str, Sampler, f64, bool, bool)> {
        vec![
            // Span w/o a parent
            ("never_sample", Sampler::AlwaysOff, 0.0, false, false),
            ("always_sample", Sampler::AlwaysOn, 1.0, false, false),
            ("ratio_-1", Sampler::TraceIdRatioBased(-1.0), 0.0, false, false),
            ("ratio_.25", Sampler::TraceIdRatioBased(0.25), 0.25, false, false),
            ("ratio_.50", Sampler::TraceIdRatioBased(0.50), 0.5, false, false),
            ("ratio_.75", Sampler::TraceIdRatioBased(0.75), 0.75, false, false),
            ("ratio_2.0", Sampler::TraceIdRatioBased(2.0), 1.0, false, false),

            // Spans w/o a parent delegate
            ("delegate_to_always_on", Sampler::ParentBased(Box::new(Sampler::AlwaysOn)), 1.0, false, false),
            ("delegate_to_always_off", Sampler::ParentBased(Box::new(Sampler::AlwaysOff)), 0.0, false, false),
            ("delegate_to_ratio_-1", Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(-1.0))), 0.0, false, false),
            ("delegate_to_ratio_.25", Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(0.25))), 0.25, false, false),
            ("delegate_to_ratio_.50", Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(0.50))), 0.50, false, false),
            ("delegate_to_ratio_.75", Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(0.75))), 0.75, false, false),
            ("delegate_to_ratio_2.0", Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(2.0))), 1.0, false, false),

            // Spans with a parent that is *not* sampled act like spans w/o a parent
            ("unsampled_parent_with_ratio_-1", Sampler::TraceIdRatioBased(-1.0), 0.0, true, false),
            ("unsampled_parent_with_ratio_.25", Sampler::TraceIdRatioBased(0.25), 0.25, true, false),
            ("unsampled_parent_with_ratio_.50", Sampler::TraceIdRatioBased(0.50), 0.5, true, false),
            ("unsampled_parent_with_ratio_.75", Sampler::TraceIdRatioBased(0.75), 0.75, true, false),
            ("unsampled_parent_with_ratio_2.0", Sampler::TraceIdRatioBased(2.0), 1.0, true, false),
            ("unsampled_parent_or_else_with_always_on", Sampler::ParentBased(Box::new(Sampler::AlwaysOn)), 0.0, true, false),
            ("unsampled_parent_or_else_with_always_off", Sampler::ParentBased(Box::new(Sampler::AlwaysOff)), 0.0, true, false),
            ("unsampled_parent_or_else_with_ratio_.25", Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(0.25))), 0.0, true, false),

            // A ratio sampler with a parent that is sampled will ignore the parent
            ("sampled_parent_with_ratio_-1", Sampler::TraceIdRatioBased(-1.0), 0.0, true, true),
            ("sampled_parent_with_ratio_.25", Sampler::TraceIdRatioBased(0.25), 0.25, true, true),
            ("sampled_parent_with_ratio_2.0", Sampler::TraceIdRatioBased(2.0), 1.0, true, true),

            // Spans with a parent that is sampled, will always sample, regardless of the delegate sampler
            ("sampled_parent_or_else_with_always_on", Sampler::ParentBased(Box::new(Sampler::AlwaysOn)), 1.0, true, true),
            ("sampled_parent_or_else_with_always_off", Sampler::ParentBased(Box::new(Sampler::AlwaysOff)), 1.0, true, true),
            ("sampled_parent_or_else_with_ratio_.25", Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(0.25))), 1.0, true, true),

            // Spans with a sampled parent, but when using the NeverSample Sampler, aren't sampled
            ("sampled_parent_span_with_never_sample", Sampler::AlwaysOff, 0.0, true, true),
        ]
    }

    #[test]
    fn sampling() {
        let total = 10_000;
        let mut rng = rand::thread_rng();
        for (name, sampler, expectation, parent, sample_parent) in sampler_data() {
            let mut sampled = 0;
            for _ in 0..total {
                let parent_context = if parent {
                    let trace_flags = if sample_parent { TRACE_FLAG_SAMPLED } else { 0 };
                    let span_context = SpanContext::new(
                        TraceId::from_u128(1),
                        SpanId::from_u64(1),
                        trace_flags,
                        false,
                        TraceState::default(),
                    );

                    Some(Context::current_with_span(TestSpan(span_context)))
                } else {
                    None
                };

                let trace_id = TraceId::from_u128(rng.gen());
                if sampler
                    .should_sample(
                        parent_context.as_ref(),
                        trace_id,
                        name,
                        &SpanKind::Internal,
                        &[],
                        &[],
                    )
                    .decision
                    == SamplingDecision::RecordAndSample
                {
                    sampled += 1;
                }
            }
            let mut tolerance = 0.0;
            let got = sampled as f64 / total as f64;

            if expectation > 0.0 && expectation < 1.0 {
                // See https://en.wikipedia.org/wiki/Binomial_proportion_confidence_interval
                let z = 4.75342; // This should succeed 99.9999% of the time
                tolerance = z * (got * (1.0 - got) / total as f64).sqrt();
            }

            let diff = (got - expectation).abs();
            assert!(
                diff <= tolerance,
                "{} got {:?} (diff: {}), expected {} (w/tolerance: {})",
                name,
                got,
                diff,
                expectation,
                tolerance
            );
        }
    }

    #[test]
    fn filter_parent_sampler_for_active_spans() {
        let sampler = Sampler::ParentBased(Box::new(Sampler::AlwaysOn));
        let cx = Context::current_with_value("some_value");
        let result = sampler.should_sample(
            Some(&cx),
            TraceId::from_u128(1),
            "should sample",
            &SpanKind::Internal,
            &[],
            &[],
        );

        assert_eq!(result.decision, SamplingDecision::RecordAndSample);
    }
}
