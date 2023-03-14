//! # Tracer
//!
//! The OpenTelemetry library achieves in-process context propagation of
//! `Span`s by way of the `Tracer`.
//!
//! The `Tracer` is responsible for tracking the currently active `Span`,
//! and exposes methods for creating and activating new `Spans`.
//!
//! Docs: <https://github.com/open-telemetry/opentelemetry-specification/blob/v1.3.0/specification/trace/api.md#tracer>
use crate::sdk::trace::SpanLimits;
use crate::sdk::{
    trace::{
        provider::{TracerProvider, TracerProviderInner},
        span::{Span, SpanData},
        Config, EvictedHashMap, EvictedQueue, SamplingDecision, SamplingResult,
    },
    InstrumentationLibrary,
};
use crate::trace::{
    Link, SpanBuilder, SpanContext, SpanId, SpanKind, StatusCode, TraceContextExt, TraceFlags,
    TraceId, TraceState,
};
use crate::{Context, KeyValue};
use std::borrow::Cow;
use std::fmt;
use std::sync::Weak;

/// `Tracer` implementation to create and manage spans
#[derive(Clone)]
pub struct Tracer {
    instrumentation_lib: InstrumentationLibrary,
    provider: Weak<TracerProviderInner>,
}

impl fmt::Debug for Tracer {
    /// Formats the `Tracer` using the given formatter.
    /// Omitting `provider` here is necessary to avoid cycles.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tracer")
            .field("name", &self.instrumentation_lib.name)
            .field("version", &self.instrumentation_lib.version)
            .finish()
    }
}

impl Tracer {
    /// Create a new tracer (used internally by `TracerProvider`s).
    pub(crate) fn new(
        instrumentation_lib: InstrumentationLibrary,
        provider: Weak<TracerProviderInner>,
    ) -> Self {
        Tracer {
            instrumentation_lib,
            provider,
        }
    }

    /// TracerProvider associated with this tracer.
    pub fn provider(&self) -> Option<TracerProvider> {
        self.provider.upgrade().map(TracerProvider::new)
    }

    /// instrumentation library information of this tracer.
    pub fn instrumentation_library(&self) -> &InstrumentationLibrary {
        &self.instrumentation_lib
    }

    /// Make a sampling decision using the provided sampler for the span and context.
    #[allow(clippy::too_many_arguments)]
    fn make_sampling_decision(
        &self,
        parent_cx: &Context,
        trace_id: TraceId,
        name: &str,
        span_kind: &SpanKind,
        attributes: &[KeyValue],
        links: &[Link],
        config: &Config,
    ) -> Option<(TraceFlags, Vec<KeyValue>, TraceState)> {
        let sampling_result = config.sampler.should_sample(
            Some(parent_cx),
            trace_id,
            name,
            span_kind,
            attributes,
            links,
        );

        self.process_sampling_result(sampling_result, parent_cx)
    }

    fn process_sampling_result(
        &self,
        sampling_result: SamplingResult,
        parent_cx: &Context,
    ) -> Option<(TraceFlags, Vec<KeyValue>, TraceState)> {
        match sampling_result {
            SamplingResult {
                decision: SamplingDecision::Drop,
                ..
            } => None,
            SamplingResult {
                decision: SamplingDecision::RecordOnly,
                attributes,
                trace_state,
            } => {
                let trace_flags = parent_cx.span().span_context().trace_flags();
                Some((trace_flags.with_sampled(false), attributes, trace_state))
            }
            SamplingResult {
                decision: SamplingDecision::RecordAndSample,
                attributes,
                trace_state,
            } => {
                let trace_flags = parent_cx.span().span_context().trace_flags();
                Some((trace_flags.with_sampled(true), attributes, trace_state))
            }
        }
    }
}

impl crate::trace::Tracer for Tracer {
    /// This implementation of `Tracer` produces `sdk::Span` instances.
    type Span = Span;

    /// Returns a span with an inactive `SpanContext`. Used by functions that
    /// need to return a default span like `get_active_span` if no span is present.
    fn invalid(&self) -> Self::Span {
        Span::new(
            SpanContext::empty_context(),
            None,
            self.clone(),
            SpanLimits::default(),
        )
    }

    /// Starts a new `Span` with a given context.
    ///
    /// Each span has zero or one parent spans and zero or more child spans, which
    /// represent causally related operations. A tree of related spans comprises a
    /// trace. A span is said to be a _root span_ if it does not have a parent. Each
    /// trace includes a single root span, which is the shared ancestor of all other
    /// spans in the trace.
    fn start_with_context<T>(&self, name: T, cx: Context) -> Self::Span
    where
        T: Into<Cow<'static, str>>,
    {
        self.build(SpanBuilder::from_name_with_context(name, cx))
    }

    /// Creates a span builder
    ///
    /// An ergonomic way for attributes to be configured before the `Span` is started.
    fn span_builder<T>(&self, name: T) -> SpanBuilder
    where
        T: Into<Cow<'static, str>>,
    {
        SpanBuilder::from_name(name)
    }

    /// Starts a span from a `SpanBuilder`.
    ///
    /// Each span has zero or one parent spans and zero or more child spans, which
    /// represent causally related operations. A tree of related spans comprises a
    /// trace. A span is said to be a _root span_ if it does not have a parent. Each
    /// trace includes a single root span, which is the shared ancestor of all other
    /// spans in the trace.
    fn build(&self, mut builder: SpanBuilder) -> Self::Span {
        let provider = self.provider();
        if provider.is_none() {
            return Span::new(
                SpanContext::empty_context(),
                None,
                self.clone(),
                SpanLimits::default(),
            );
        }

        let provider = provider.unwrap();
        let config = provider.config();
        let span_limits = config.span_limits;
        let span_id = builder
            .span_id
            .take()
            .unwrap_or_else(|| config.id_generator.new_span_id());

        let span_kind = builder.span_kind.take().unwrap_or(SpanKind::Internal);
        let mut attribute_options = builder.attributes.take().unwrap_or_else(Vec::new);
        let mut link_options = builder.links.take();
        let mut flags = TraceFlags::default();
        let mut span_trace_state = Default::default();

        let parent_span = if builder.parent_context.has_active_span() {
            Some(builder.parent_context.span())
        } else {
            None
        };

        // Build context for sampling decision
        let (no_parent, trace_id, parent_span_id, remote_parent, parent_trace_flags) = parent_span
            .as_ref()
            .map(|parent| {
                let sc = parent.span_context();
                (
                    false,
                    sc.trace_id(),
                    sc.span_id(),
                    sc.is_remote(),
                    sc.trace_flags(),
                )
            })
            .unwrap_or((
                true,
                builder
                    .trace_id
                    .unwrap_or_else(|| config.id_generator.new_trace_id()),
                SpanId::invalid(),
                false,
                TraceFlags::default(),
            ));

        // There are 3 paths for sampling.
        //
        // * Sampling has occurred elsewhere and is already stored in the builder
        // * There is no parent or a remote parent, in which case make decision now
        // * There is a local parent, in which case defer to the parent's decision
        let sampling_decision = if let Some(sampling_result) = builder.sampling_result.take() {
            self.process_sampling_result(sampling_result, &builder.parent_context)
        } else if no_parent || remote_parent {
            self.make_sampling_decision(
                &builder.parent_context,
                trace_id,
                &builder.name,
                &span_kind,
                &attribute_options,
                link_options.as_deref().unwrap_or(&[]),
                provider.config(),
            )
        } else {
            // has parent that is local: use parent if sampled, or don't record.
            parent_span
                .filter(|span| span.span_context().is_sampled())
                .map(|span| {
                    (
                        parent_trace_flags,
                        Vec::new(),
                        span.span_context().trace_state().clone(),
                    )
                })
        };

        // Build optional inner context, `None` if not recording.
        let SpanBuilder {
            parent_context,
            name,
            start_time,
            end_time,
            events,
            status_code,
            status_message,
            ..
        } = builder;
        let inner = sampling_decision.map(|(trace_flags, mut extra_attrs, trace_state)| {
            flags = trace_flags;
            span_trace_state = trace_state;
            attribute_options.append(&mut extra_attrs);
            let mut attributes =
                EvictedHashMap::new(span_limits.max_attributes_per_span, attribute_options.len());
            for attribute in attribute_options {
                attributes.insert(attribute);
            }
            let mut links = EvictedQueue::new(span_limits.max_links_per_span);
            if let Some(link_options) = &mut link_options {
                let link_attributes_limit = span_limits.max_attributes_per_link as usize;
                for link in link_options.iter_mut() {
                    let dropped_attributes_count =
                        link.attributes.len().saturating_sub(link_attributes_limit);
                    link.attributes.truncate(link_attributes_limit);
                    link.dropped_attributes_count = dropped_attributes_count as u32;
                }
                links.append_vec(link_options);
            }
            let start_time = start_time.unwrap_or_else(crate::time::now);
            let end_time = end_time.unwrap_or(start_time);
            let mut events_queue = EvictedQueue::new(span_limits.max_events_per_span);
            if let Some(mut events) = events {
                let event_attributes_limit = span_limits.max_attributes_per_event as usize;
                for event in events.iter_mut() {
                    let dropped_attributes_count = event
                        .attributes
                        .len()
                        .saturating_sub(event_attributes_limit);
                    event.attributes.truncate(event_attributes_limit);
                    event.dropped_attributes_count = dropped_attributes_count as u32;
                }
                events_queue.append_vec(&mut events);
            }
            let status_code = status_code.unwrap_or(StatusCode::Unset);
            let status_message = status_message.unwrap_or(Cow::Borrowed(""));

            SpanData {
                parent_span_id,
                span_kind,
                name,
                start_time,
                end_time,
                attributes,
                events: events_queue,
                links,
                status_code,
                status_message,
            }
        });

        let span_context = SpanContext::new(trace_id, span_id, flags, false, span_trace_state);
        let mut span = Span::new(span_context, inner, self.clone(), span_limits);

        // Call `on_start` for all processors
        for processor in provider.span_processors() {
            processor.on_start(&mut span, &parent_context)
        }

        span
    }
}

#[cfg(all(test, feature = "testing", feature = "trace"))]
mod tests {
    use crate::{
        sdk::{
            self,
            trace::{Config, Sampler, SamplingDecision, SamplingResult, ShouldSample},
        },
        testing::trace::TestSpan,
        trace::{
            Link, Span, SpanBuilder, SpanContext, SpanId, SpanKind, TraceContextExt, TraceFlags,
            TraceId, TraceState, Tracer, TracerProvider,
        },
        Context, KeyValue,
    };

    #[derive(Debug)]
    struct TestSampler {}

    impl ShouldSample for TestSampler {
        fn should_sample(
            &self,
            parent_context: Option<&Context>,
            _trace_id: TraceId,
            _name: &str,
            _span_kind: &SpanKind,
            _attributes: &[KeyValue],
            _links: &[Link],
        ) -> SamplingResult {
            let trace_state = parent_context
                .unwrap()
                .span()
                .span_context()
                .trace_state()
                .clone();
            SamplingResult {
                decision: SamplingDecision::RecordAndSample,
                attributes: Vec::new(),
                trace_state: trace_state.insert("foo", "notbar").unwrap(),
            }
        }
    }

    #[test]
    fn allow_sampler_to_change_trace_state() {
        // Setup
        let sampler = TestSampler {};
        let config = Config::default().with_sampler(sampler);
        let tracer_provider = sdk::trace::TracerProvider::builder()
            .with_config(config)
            .build();
        let tracer = tracer_provider.tracer("test", None);
        let trace_state = TraceState::from_key_value(vec![("foo", "bar")]).unwrap();
        let span_builder = SpanBuilder {
            parent_context: Context::new().with_span(TestSpan(SpanContext::new(
                TraceId::from_u128(128),
                SpanId::from_u64(64),
                TraceFlags::SAMPLED,
                true,
                trace_state,
            ))),
            ..Default::default()
        };

        // Test sampler should change trace state
        let span = tracer.build(span_builder);
        let span_context = span.span_context();
        let expected = span_context.trace_state();
        assert_eq!(expected.get("foo"), Some("notbar"))
    }

    #[test]
    fn drop_parent_based_children() {
        let sampler = Sampler::ParentBased(Box::new(Sampler::AlwaysOn));
        let config = Config::default().with_sampler(sampler);
        let tracer_provider = sdk::trace::TracerProvider::builder()
            .with_config(config)
            .build();

        let context = Context::current_with_span(TestSpan(SpanContext::empty_context()));
        let tracer = tracer_provider.tracer("test", None);
        let span = tracer.start_with_context("must_not_be_sampled", context);

        assert!(!span.span_context().is_sampled());
    }

    #[test]
    fn uses_current_context_for_builders_if_unset() {
        let sampler = Sampler::ParentBased(Box::new(Sampler::AlwaysOn));
        let config = Config::default().with_sampler(sampler);
        let tracer_provider = sdk::trace::TracerProvider::builder()
            .with_config(config)
            .build();
        let tracer = tracer_provider.tracer("test", None);

        let _attached = Context::current_with_span(TestSpan(SpanContext::empty_context())).attach();
        let span = tracer.span_builder("must_not_be_sampled").start(&tracer);
        assert!(!span.span_context().is_sampled());

        let _attached = Context::current()
            .with_remote_span_context(SpanContext::new(
                TraceId::from_u128(1),
                SpanId::from_u64(1),
                TraceFlags::default(),
                true,
                Default::default(),
            ))
            .attach();
        let span = tracer.span_builder("must_not_be_sampled").start(&tracer);

        assert!(!span.span_context().is_sampled());
    }
}
