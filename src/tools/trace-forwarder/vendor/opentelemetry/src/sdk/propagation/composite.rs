use crate::{
    propagation::{text_map_propagator::FieldIter, Extractor, Injector, TextMapPropagator},
    Context,
};
use std::collections::HashSet;

/// Composite propagator
///
/// A propagator that chains multiple [`TextMapPropagator`] propagators together,
/// injecting or extracting by their respective HTTP header names.
///
/// Injection and extraction from this propagator will preserve the order of the
/// injectors and extractors passed in during initialization.
///
/// [`TextMapPropagator`]: crate::propagation::TextMapPropagator
///
/// # Examples
///
/// ```
/// use opentelemetry::{
///     baggage::BaggageExt,
///     propagation::TextMapPropagator,
///     trace::{TraceContextExt, Tracer, TracerProvider},
///     Context, KeyValue,
/// };
/// use opentelemetry::sdk::propagation::{
///     BaggagePropagator, TextMapCompositePropagator, TraceContextPropagator,
/// };
/// use opentelemetry::sdk::trace as sdktrace;
/// use std::collections::HashMap;
///
/// // First create 1 or more propagators
/// let baggage_propagator = BaggagePropagator::new();
/// let trace_context_propagator = TraceContextPropagator::new();
///
/// // Then create a composite propagator
/// let composite_propagator = TextMapCompositePropagator::new(vec![
///     Box::new(baggage_propagator),
///     Box::new(trace_context_propagator),
/// ]);
///
/// // Then for a given implementation of `Injector`
/// let mut injector = HashMap::new();
///
/// // And a given span
/// let example_span = sdktrace::TracerProvider::default()
///     .tracer("example-component", None)
///     .start("span-name");
///
/// // with the current context, call inject to add the headers
/// composite_propagator.inject_context(
///     &Context::current_with_span(example_span)
///         .with_baggage(vec![KeyValue::new("test", "example")]),
///     &mut injector,
/// );
///
/// // The injector now has both `baggage` and `traceparent` headers
/// assert!(injector.get("baggage").is_some());
/// assert!(injector.get("traceparent").is_some());
/// ```
#[derive(Debug)]
pub struct TextMapCompositePropagator {
    propagators: Vec<Box<dyn TextMapPropagator + Send + Sync>>,
    fields: Vec<String>,
}

impl TextMapCompositePropagator {
    /// Constructs a new propagator out of instances of [`TextMapPropagator`].
    ///
    /// [`TextMapPropagator`]: crate::propagation::TextMapPropagator
    pub fn new(propagators: Vec<Box<dyn TextMapPropagator + Send + Sync>>) -> Self {
        let mut fields = HashSet::new();
        for propagator in &propagators {
            for field in propagator.fields() {
                fields.insert(field.to_string());
            }
        }

        TextMapCompositePropagator {
            propagators,
            fields: fields.into_iter().collect(),
        }
    }
}

impl TextMapPropagator for TextMapCompositePropagator {
    /// Encodes the values of the `Context` and injects them into the `Injector`.
    fn inject_context(&self, context: &Context, injector: &mut dyn Injector) {
        for propagator in &self.propagators {
            propagator.inject_context(context, injector)
        }
    }

    /// Retrieves encoded `Context` information using the `Extractor`. If no data was
    /// retrieved OR if the retrieved data is invalid, then the current `Context` is
    /// returned.
    fn extract_with_context(&self, cx: &Context, extractor: &dyn Extractor) -> Context {
        self.propagators
            .iter()
            .fold(cx.clone(), |current_cx, propagator| {
                propagator.extract_with_context(&current_cx, extractor)
            })
    }

    fn fields(&self) -> FieldIter<'_> {
        FieldIter::new(self.fields.as_slice())
    }
}

#[cfg(all(test, feature = "testing", feature = "trace"))]
mod tests {
    use crate::sdk::propagation::{TextMapCompositePropagator, TraceContextPropagator};
    use crate::testing::trace::TestSpan;
    use crate::{
        propagation::{text_map_propagator::FieldIter, Extractor, Injector, TextMapPropagator},
        trace::{SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState},
        Context,
    };
    use std::collections::HashMap;
    use std::str::FromStr;

    /// Dummy propagator for testing
    ///
    /// The format we are using is {trace id(in base10 u128)}-{span id(in base10 u64)}-{flag(in u8)}
    #[derive(Debug)]
    struct TestPropagator {
        fields: [String; 1],
    }

    impl TestPropagator {
        #[allow(unreachable_pub)]
        pub fn new() -> Self {
            TestPropagator {
                fields: ["testheader".to_string()],
            }
        }
    }

    impl TextMapPropagator for TestPropagator {
        fn inject_context(&self, cx: &Context, injector: &mut dyn Injector) {
            let span = cx.span();
            let span_context = span.span_context();
            injector.set(
                "testheader",
                format!(
                    "{}-{}-{:02x}",
                    span_context.trace_id().to_u128(),
                    span_context.span_id().to_u64(),
                    span_context.trace_flags()
                ),
            )
        }

        fn extract_with_context(&self, cx: &Context, extractor: &dyn Extractor) -> Context {
            let span = if let Some(val) = extractor.get("testheader") {
                let parts = val.split_terminator('-').collect::<Vec<&str>>();
                if parts.len() != 3 {
                    SpanContext::empty_context()
                } else {
                    SpanContext::new(
                        TraceId::from_u128(u128::from_str(parts[0]).unwrap_or(0)),
                        SpanId::from_u64(u64::from_str(parts[1]).unwrap_or(0)),
                        TraceFlags::new(u8::from_str(parts[2]).unwrap_or(0)),
                        true,
                        TraceState::default(),
                    )
                }
            } else {
                SpanContext::empty_context()
            };

            cx.with_remote_span_context(span)
        }

        fn fields(&self) -> FieldIter<'_> {
            FieldIter::new(&self.fields)
        }
    }

    fn test_data() -> Vec<(&'static str, &'static str)> {
        vec![
            ("testheader", "1-1-00"),
            (
                "traceparent",
                "00-00000000000000000000000000000001-0000000000000001-00",
            ),
        ]
    }

    #[test]
    fn zero_propogators_are_noop() {
        let composite_propagator = TextMapCompositePropagator::new(vec![]);

        let cx = Context::default().with_span(TestSpan(SpanContext::new(
            TraceId::from_u128(1),
            SpanId::from_u64(1),
            TraceFlags::default(),
            false,
            TraceState::default(),
        )));
        let mut injector = HashMap::new();
        composite_propagator.inject_context(&cx, &mut injector);

        assert_eq!(injector.len(), 0);

        for (header_name, header_value) in test_data() {
            let mut extractor = HashMap::new();
            extractor.insert(header_name.to_string(), header_value.to_string());
            assert_eq!(
                composite_propagator
                    .extract(&extractor)
                    .span()
                    .span_context(),
                &SpanContext::empty_context()
            );
        }
    }

    #[test]
    fn inject_multiple_propagators() {
        let test_propagator = TestPropagator::new();
        let trace_context = TraceContextPropagator::new();
        let composite_propagator = TextMapCompositePropagator::new(vec![
            Box::new(test_propagator),
            Box::new(trace_context),
        ]);

        let cx = Context::default().with_span(TestSpan(SpanContext::new(
            TraceId::from_u128(1),
            SpanId::from_u64(1),
            TraceFlags::default(),
            false,
            TraceState::default(),
        )));
        let mut injector = HashMap::new();
        composite_propagator.inject_context(&cx, &mut injector);

        for (header_name, header_value) in test_data() {
            assert_eq!(injector.get(header_name), Some(&header_value.to_string()));
        }
    }

    #[test]
    fn extract_multiple_propagators() {
        let test_propagator = TestPropagator::new();
        let trace_context = TraceContextPropagator::new();
        let composite_propagator = TextMapCompositePropagator::new(vec![
            Box::new(test_propagator),
            Box::new(trace_context),
        ]);

        for (header_name, header_value) in test_data() {
            let mut extractor = HashMap::new();
            extractor.insert(header_name.to_string(), header_value.to_string());
            assert_eq!(
                composite_propagator
                    .extract(&extractor)
                    .span()
                    .span_context(),
                &SpanContext::new(
                    TraceId::from_u128(1),
                    SpanId::from_u64(1),
                    TraceFlags::default(),
                    true,
                    TraceState::default(),
                )
            );
        }
    }

    #[test]
    fn test_get_fields() {
        let test_propagator = TestPropagator::new();
        let b3_fields = test_propagator
            .fields()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();

        let trace_context = TraceContextPropagator::new();
        let trace_context_fields = trace_context
            .fields()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();

        let composite_propagator = TextMapCompositePropagator::new(vec![
            Box::new(test_propagator),
            Box::new(trace_context),
        ]);

        let mut fields = composite_propagator
            .fields()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();
        fields.sort();

        let mut expected = vec![b3_fields, trace_context_fields]
            .into_iter()
            .flatten()
            .collect::<Vec<String>>();
        expected.sort();
        expected.dedup();

        assert_eq!(fields, expected);
    }
}
