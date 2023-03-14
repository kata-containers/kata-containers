//! # Span
//!
//! `Span`s represent a single operation within a trace. `Span`s can be nested to form a trace
//! tree. Each trace contains a root span, which typically describes the end-to-end latency and,
//! optionally, one or more sub-spans for its sub-operations.
//!
//! The `Span`'s start and end timestamps reflect the elapsed real time of the operation. A `Span`'s
//! start time is set to the current time on span creation. After the `Span` is created, it
//! is possible to change its name, set its `Attributes`, and add `Links` and `Events`.
//! These cannot be changed after the `Span`'s end time has been set.
use crate::sdk::trace::SpanLimits;
use crate::trace::{Event, SpanContext, SpanId, SpanKind, StatusCode};
use crate::{sdk, trace, KeyValue};
use std::borrow::Cow;
use std::sync::Arc;
use std::time::SystemTime;

/// Single operation within a trace.
#[derive(Debug)]
pub struct Span {
    span_context: SpanContext,
    data: Option<SpanData>,
    tracer: sdk::trace::Tracer,
    span_limits: SpanLimits,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SpanData {
    /// Span parent id
    pub(crate) parent_span_id: SpanId,
    /// Span kind
    pub(crate) span_kind: SpanKind,
    /// Span name
    pub(crate) name: Cow<'static, str>,
    /// Span start time
    pub(crate) start_time: SystemTime,
    /// Span end time
    pub(crate) end_time: SystemTime,
    /// Span attributes
    pub(crate) attributes: sdk::trace::EvictedHashMap,
    /// Span events
    pub(crate) events: sdk::trace::EvictedQueue<trace::Event>,
    /// Span Links
    pub(crate) links: sdk::trace::EvictedQueue<trace::Link>,
    /// Span status code
    pub(crate) status_code: StatusCode,
    /// Span status message
    pub(crate) status_message: Cow<'static, str>,
}

impl Span {
    pub(crate) fn new(
        span_context: SpanContext,
        data: Option<SpanData>,
        tracer: sdk::trace::Tracer,
        span_limit: SpanLimits,
    ) -> Self {
        Span {
            span_context,
            data,
            tracer,
            span_limits: span_limit,
        }
    }

    /// Operate on a mutable reference to span data
    fn with_data<T, F>(&mut self, f: F) -> Option<T>
    where
        F: FnOnce(&mut SpanData) -> T,
    {
        self.data.as_mut().map(f)
    }
}

impl crate::trace::Span for Span {
    /// Records events at a specific time in the context of a given `Span`.
    ///
    /// Note that the OpenTelemetry project documents certain ["standard event names and
    /// keys"](https://github.com/open-telemetry/opentelemetry-specification/tree/v0.5.0/specification/trace/semantic_conventions/README.md)
    /// which have prescribed semantic meanings.
    fn add_event_with_timestamp(
        &mut self,
        name: String,
        timestamp: SystemTime,
        mut attributes: Vec<KeyValue>,
    ) {
        let event_attributes_limit = self.span_limits.max_attributes_per_event as usize;
        self.with_data(|data| {
            let dropped_attributes_count = attributes.len().saturating_sub(event_attributes_limit);
            attributes.truncate(event_attributes_limit);

            data.events.push_back(Event::new(
                name,
                timestamp,
                attributes,
                dropped_attributes_count as u32,
            ))
        });
    }

    /// Returns the `SpanContext` for the given `Span`.
    fn span_context(&self) -> &SpanContext {
        &self.span_context
    }

    /// Returns true if this `Span` is recording information like events with the `add_event`
    /// operation, attributes using `set_attributes`, status with `set_status`, etc.
    /// Always returns false after span `end`.
    fn is_recording(&self) -> bool {
        self.data.is_some()
    }

    /// Sets a single `Attribute` where the attribute properties are passed as arguments.
    ///
    /// Note that the OpenTelemetry project documents certain ["standard
    /// attributes"](https://github.com/open-telemetry/opentelemetry-specification/tree/v0.5.0/specification/trace/semantic_conventions/README.md)
    /// that have prescribed semantic meanings.
    fn set_attribute(&mut self, attribute: KeyValue) {
        self.with_data(|data| {
            data.attributes.insert(attribute);
        });
    }

    /// Sets the status of the `Span`. If used, this will override the default `Span`
    /// status, which is `Unset`. `message` MUST be ignored when the status is `OK` or `Unset`
    fn set_status(&mut self, code: StatusCode, message: String) {
        self.with_data(|data| {
            if code == StatusCode::Error {
                data.status_message = message.into();
            }
            data.status_code = code;
        });
    }

    /// Updates the `Span`'s name.
    fn update_name(&mut self, new_name: String) {
        self.with_data(|data| {
            data.name = new_name.into();
        });
    }

    /// Finishes the span with given timestamp.
    fn end_with_timestamp(&mut self, timestamp: SystemTime) {
        self.ensure_ended_and_exported(Some(timestamp));
    }
}

impl Span {
    fn ensure_ended_and_exported(&mut self, timestamp: Option<SystemTime>) {
        if let Some(mut data) = self.data.take() {
            // Ensure end time is set via explicit end or implicitly on drop
            if let Some(timestamp) = timestamp {
                data.end_time = timestamp;
            } else if data.end_time == data.start_time {
                data.end_time = crate::time::now();
            }

            // Notify each span processor that the span has ended
            if let Some(provider) = self.tracer.provider() {
                let mut processors = provider.span_processors().iter().peekable();
                let resource = provider.config().resource.clone();
                let mut span_data = Some((data, resource));
                while let Some(processor) = processors.next() {
                    let span_data = if processors.peek().is_none() {
                        // last loop or single processor/exporter, move data
                        span_data.take()
                    } else {
                        // clone so each exporter gets owned data
                        span_data.clone()
                    };

                    if let Some((span_data, resource)) = span_data {
                        processor.on_end(build_export_data(
                            span_data,
                            self.span_context.clone(),
                            resource,
                            &self.tracer,
                        ));
                    }
                }
            }
        }
    }
}

impl Drop for Span {
    /// Report span on inner drop
    fn drop(&mut self) {
        self.ensure_ended_and_exported(None);
    }
}

fn build_export_data(
    data: SpanData,
    span_context: SpanContext,
    resource: Option<Arc<sdk::Resource>>,
    tracer: &sdk::trace::Tracer,
) -> sdk::export::trace::SpanData {
    sdk::export::trace::SpanData {
        span_context,
        parent_span_id: data.parent_span_id,
        span_kind: data.span_kind,
        name: data.name,
        start_time: data.start_time,
        end_time: data.end_time,
        attributes: data.attributes,
        events: data.events,
        links: data.links,
        status_code: data.status_code,
        status_message: data.status_message,
        resource,
        instrumentation_lib: *tracer.instrumentation_library(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk::trace::span_limit::{
        DEFAULT_MAX_ATTRIBUTES_PER_EVENT, DEFAULT_MAX_ATTRIBUTES_PER_LINK,
    };
    use crate::trace::{Link, NoopSpanExporter, TraceId, Tracer};
    use crate::{core::KeyValue, trace::Span as _, trace::TracerProvider};
    use std::time::Duration;

    fn init() -> (sdk::trace::Tracer, SpanData) {
        let provider = sdk::trace::TracerProvider::default();
        let config = provider.config();
        let tracer = provider.get_tracer("opentelemetry", Some(env!("CARGO_PKG_VERSION")));
        let data = SpanData {
            parent_span_id: SpanId::from_u64(0),
            span_kind: trace::SpanKind::Internal,
            name: "opentelemetry".into(),
            start_time: crate::time::now(),
            end_time: crate::time::now(),
            attributes: sdk::trace::EvictedHashMap::new(
                config.span_limits.max_attributes_per_span,
                0,
            ),
            events: sdk::trace::EvictedQueue::new(config.span_limits.max_events_per_span),
            links: sdk::trace::EvictedQueue::new(config.span_limits.max_links_per_span),
            status_code: StatusCode::Unset,
            status_message: "".into(),
        };
        (tracer, data)
    }

    fn create_span() -> Span {
        let (tracer, data) = init();
        Span::new(
            SpanContext::empty_context(),
            Some(data),
            tracer,
            Default::default(),
        )
    }

    #[test]
    fn create_span_without_data() {
        let (tracer, _) = init();
        let mut span = Span::new(
            SpanContext::empty_context(),
            None,
            tracer,
            Default::default(),
        );
        span.with_data(|_data| panic!("there are data"));
    }

    #[test]
    fn create_span_with_data_mut() {
        let (tracer, data) = init();
        let mut span = Span::new(
            SpanContext::empty_context(),
            Some(data.clone()),
            tracer,
            Default::default(),
        );
        span.with_data(|d| assert_eq!(*d, data));
    }

    #[test]
    fn add_event() {
        let mut span = create_span();
        let name = "some_event".to_string();
        let attributes = vec![KeyValue::new("k", "v")];
        span.add_event(name.clone(), attributes.clone());
        span.with_data(|data| {
            if let Some(event) = data.events.iter().next() {
                assert_eq!(event.name, name);
                assert_eq!(event.attributes, attributes);
            } else {
                panic!("no event");
            }
        });
    }

    #[test]
    fn add_event_with_timestamp() {
        let mut span = create_span();
        let name = "some_event".to_string();
        let attributes = vec![KeyValue::new("k", "v")];
        let timestamp = crate::time::now();
        span.add_event_with_timestamp(name.clone(), timestamp, attributes.clone());
        span.with_data(|data| {
            if let Some(event) = data.events.iter().next() {
                assert_eq!(event.timestamp, timestamp);
                assert_eq!(event.name, name);
                assert_eq!(event.attributes, attributes);
            } else {
                panic!("no event");
            }
        });
    }

    #[test]
    fn record_exception() {
        let mut span = create_span();
        let err = std::io::Error::from(std::io::ErrorKind::Other);
        span.record_exception(&err);
        span.with_data(|data| {
            if let Some(event) = data.events.iter().next() {
                assert_eq!(event.name, "exception");
                assert_eq!(
                    event.attributes,
                    vec![KeyValue::new("exception.message", err.to_string())]
                );
            } else {
                panic!("no event");
            }
        });
    }

    #[test]
    fn record_exception_with_stacktrace() {
        let mut span = create_span();
        let err = std::io::Error::from(std::io::ErrorKind::Other);
        let stacktrace = "stacktrace...".to_string();
        span.record_exception_with_stacktrace(&err, stacktrace.clone());
        span.with_data(|data| {
            if let Some(event) = data.events.iter().next() {
                assert_eq!(event.name, "exception");
                assert_eq!(
                    event.attributes,
                    vec![
                        KeyValue::new("exception.message", err.to_string()),
                        KeyValue::new("exception.stacktrace", stacktrace),
                    ]
                );
            } else {
                panic!("no event");
            }
        });
    }

    #[test]
    fn set_attribute() {
        let mut span = create_span();
        let attributes = KeyValue::new("k", "v");
        span.set_attribute(attributes.clone());
        span.with_data(|data| {
            if let Some(val) = data.attributes.get(&attributes.key) {
                assert_eq!(*val, attributes.value);
            } else {
                panic!("no attribute");
            }
        });
    }

    #[test]
    fn set_status() {
        {
            let mut span = create_span();
            let status = StatusCode::Ok;
            let message = "OK".to_string();
            span.set_status(status, message);
            span.with_data(|data| {
                assert_eq!(data.status_code, status);
                assert_eq!(data.status_message, "");
            });
        }
        {
            let mut span = create_span();
            let status = StatusCode::Unset;
            let message = "OK".to_string();
            span.set_status(status, message);
            span.with_data(|data| {
                assert_eq!(data.status_code, status);
                assert_eq!(data.status_message, "");
            });
        }
        {
            let mut span = create_span();
            let status = StatusCode::Error;
            let message = "Error".to_string();
            span.set_status(status, message);
            span.with_data(|data| {
                assert_eq!(data.status_code, status);
                assert_eq!(data.status_message, "Error");
            });
        }
    }

    #[test]
    fn update_name() {
        let mut span = create_span();
        let name = "new_name".to_string();
        span.update_name(name.clone());
        span.with_data(|data| {
            assert_eq!(data.name, name);
        });
    }

    #[test]
    fn end() {
        let mut span = create_span();
        span.end();
    }

    #[test]
    fn end_with_timestamp() {
        let mut span = create_span();
        let timestamp = crate::time::now();
        span.end_with_timestamp(timestamp);
        span.with_data(|data| assert_eq!(data.end_time, timestamp));
    }

    #[test]
    fn allows_to_get_span_context_after_end() {
        let mut span = create_span();
        span.end();
        assert_eq!(span.span_context(), &SpanContext::empty_context());
    }

    #[test]
    fn end_only_once() {
        let mut span = create_span();
        let timestamp = crate::time::now();
        span.end_with_timestamp(timestamp);
        span.end_with_timestamp(timestamp.checked_add(Duration::from_secs(10)).unwrap());
        span.with_data(|data| assert_eq!(data.end_time, timestamp));
    }

    #[test]
    fn noop_after_end() {
        let mut span = create_span();
        let initial = span.with_data(|data| data.clone()).unwrap();
        span.end();
        span.add_event("some_event".to_string(), vec![KeyValue::new("k", "v")]);
        span.add_event_with_timestamp(
            "some_event".to_string(),
            crate::time::now(),
            vec![KeyValue::new("k", "v")],
        );
        let err = std::io::Error::from(std::io::ErrorKind::Other);
        span.record_exception(&err);
        span.record_exception_with_stacktrace(&err, "stacktrace...".to_string());
        span.set_attribute(KeyValue::new("k", "v"));
        span.set_status(StatusCode::Error, "ERROR".to_string());
        span.update_name("new_name".to_string());
        span.with_data(|data| {
            assert_eq!(data.events, initial.events);
            assert_eq!(data.attributes, initial.attributes);
            assert_eq!(data.status_code, initial.status_code);
            assert_eq!(data.status_message, initial.status_message);
            assert_eq!(data.name, initial.name);
        });
    }

    #[test]
    fn is_recording_true_when_not_ended() {
        let span = create_span();
        assert!(span.is_recording());
    }

    #[test]
    fn is_recording_false_after_end() {
        let mut span = create_span();
        span.end();
        assert!(!span.is_recording());
    }

    #[test]
    fn exceed_event_attributes_limit() {
        let exporter = NoopSpanExporter::new();
        let provider_builder = sdk::trace::TracerProvider::builder().with_simple_exporter(exporter);
        let provider = provider_builder.build();
        let tracer = provider.get_tracer("opentelemetry-test", None);

        let mut event1 = Event::with_name("test event");
        for i in 0..(DEFAULT_MAX_ATTRIBUTES_PER_EVENT * 2) {
            event1
                .attributes
                .push(KeyValue::new(format!("key {}", i), i.to_string()))
        }
        let event2 = event1.clone();

        // add event when build
        let span_builder = tracer.span_builder("test").with_events(vec![event1]);
        let mut span = tracer.build(span_builder);

        // add event after build
        span.add_event("another test event".into(), event2.attributes);

        let event_queue = span
            .data
            .clone()
            .expect("span data should not be empty as we already set it before")
            .events;
        let event_vec: Vec<_> = event_queue.iter().take(2).collect();
        let processed_event_1 = event_vec.get(0).expect("should have at least two events");
        let processed_event_2 = event_vec.get(1).expect("should have at least two events");
        assert_eq!(processed_event_1.attributes.len(), 128);
        assert_eq!(processed_event_2.attributes.len(), 128);
    }

    #[test]
    fn exceed_link_attributes_limit() {
        let exporter = NoopSpanExporter::new();
        let provider_builder = sdk::trace::TracerProvider::builder().with_simple_exporter(exporter);
        let provider = provider_builder.build();
        let tracer = provider.get_tracer("opentelemetry-test", None);

        let mut link = Link::new(
            SpanContext::new(
                TraceId::from_u128(12),
                SpanId::from_u64(12),
                0,
                false,
                Default::default(),
            ),
            Vec::new(),
        );
        for i in 0..(DEFAULT_MAX_ATTRIBUTES_PER_LINK * 2) {
            link.attributes
                .push(KeyValue::new(format!("key {}", i), i.to_string()));
        }

        let span_builder = tracer.span_builder("test").with_links(vec![link]);
        let span = tracer.build(span_builder);
        let link_queue = span
            .data
            .clone()
            .expect("span data should not be empty as we already set it before")
            .links;
        let link_vec: Vec<_> = link_queue.iter().collect();
        let processed_link = link_vec.get(0).expect("should have at least one link");
        assert_eq!(processed_link.attributes().len(), 128);
    }
}
