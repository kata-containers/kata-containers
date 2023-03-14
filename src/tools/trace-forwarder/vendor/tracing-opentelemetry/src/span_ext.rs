use crate::layer::WithContext;
use opentelemetry::{trace::SpanContext, Context, KeyValue};

/// Utility functions to allow tracing [`Span`]s to accept and return
/// [OpenTelemetry] [`Context`]s.
///
/// [`Span`]: https://docs.rs/tracing/latest/tracing/struct.Span.html
/// [OpenTelemetry]: https://opentelemetry.io
/// [`Context`]: opentelemetry::Context
pub trait OpenTelemetrySpanExt {
    /// Associates `self` with a given OpenTelemetry trace, using the provided
    /// parent [`Context`].
    ///
    /// [`Context`]: opentelemetry::Context
    ///
    /// # Examples
    ///
    /// ```rust
    /// use opentelemetry::{propagation::TextMapPropagator, trace::TraceContextExt};
    /// use opentelemetry::sdk::propagation::TraceContextPropagator;
    /// use tracing_opentelemetry::OpenTelemetrySpanExt;
    /// use std::collections::HashMap;
    /// use tracing::Span;
    ///
    /// // Example carrier, could be a framework header map that impls otel's `Extract`.
    /// let mut carrier = HashMap::new();
    ///
    /// // Propagator can be swapped with b3 propagator, jaeger propagator, etc.
    /// let propagator = TraceContextPropagator::new();
    ///
    /// // Extract otel parent context via the chosen propagator
    /// let parent_context = propagator.extract(&carrier);
    ///
    /// // Generate a tracing span as usual
    /// let app_root = tracing::span!(tracing::Level::INFO, "app_start");
    ///
    /// // Assign parent trace from external context
    /// app_root.set_parent(parent_context.clone());
    ///
    /// // Or if the current span has been created elsewhere:
    /// Span::current().set_parent(parent_context);
    /// ```
    fn set_parent(&self, cx: Context);

    /// Associates `self` with a given OpenTelemetry trace, using the provided
    /// followed span [`SpanContext`].
    ///
    /// [`SpanContext`]: opentelemetry::trace::SpanContext
    ///
    /// # Examples
    ///
    /// ```rust
    /// use opentelemetry::{propagation::TextMapPropagator, trace::TraceContextExt};
    /// use opentelemetry::sdk::propagation::TraceContextPropagator;
    /// use tracing_opentelemetry::OpenTelemetrySpanExt;
    /// use std::collections::HashMap;
    /// use tracing::Span;
    ///
    /// // Example carrier, could be a framework header map that impls otel's `Extract`.
    /// let mut carrier = HashMap::new();
    ///
    /// // Propagator can be swapped with b3 propagator, jaeger propagator, etc.
    /// let propagator = TraceContextPropagator::new();
    ///
    /// // Extract otel context of linked span via the chosen propagator
    /// let linked_span_otel_context = propagator.extract(&carrier);
    ///
    /// // Extract the linked span context from the otel context
    /// let linked_span_context = linked_span_otel_context.span().span_context().clone();
    ///
    /// // Generate a tracing span as usual
    /// let app_root = tracing::span!(tracing::Level::INFO, "app_start");
    ///
    /// // Assign linked trace from external context
    /// app_root.add_link(linked_span_context);
    ///
    /// // Or if the current span has been created elsewhere:
    /// let linked_span_context = linked_span_otel_context.span().span_context().clone();
    /// Span::current().add_link(linked_span_context);
    /// ```
    fn add_link(&self, cx: SpanContext);

    /// Associates `self` with a given OpenTelemetry trace, using the provided
    /// followed span [`SpanContext`] and attributes.
    ///
    /// [`SpanContext`]: opentelemetry::trace::SpanContext
    fn add_link_with_attributes(&self, cx: SpanContext, attributes: Vec<KeyValue>);

    /// Extracts an OpenTelemetry [`Context`] from `self`.
    ///
    /// [`Context`]: opentelemetry::Context
    ///
    /// # Examples
    ///
    /// ```rust
    /// use opentelemetry::Context;
    /// use tracing_opentelemetry::OpenTelemetrySpanExt;
    /// use tracing::Span;
    ///
    /// fn make_request(cx: Context) {
    ///     // perform external request after injecting context
    ///     // e.g. if the request's headers impl `opentelemetry::propagation::Injector`
    ///     // then `propagator.inject_context(cx, request.headers_mut())`
    /// }
    ///
    /// // Generate a tracing span as usual
    /// let app_root = tracing::span!(tracing::Level::INFO, "app_start");
    ///
    /// // To include tracing context in client requests from _this_ app,
    /// // extract the current OpenTelemetry context.
    /// make_request(app_root.context());
    ///
    /// // Or if the current span has been created elsewhere:
    /// make_request(Span::current().context())
    /// ```
    fn context(&self) -> Context;
}

impl OpenTelemetrySpanExt for tracing::Span {
    fn set_parent(&self, cx: Context) {
        let mut cx = Some(cx);
        self.with_subscriber(move |(id, subscriber)| {
            if let Some(get_context) = subscriber.downcast_ref::<WithContext>() {
                get_context.with_context(subscriber, id, move |builder, _tracer| {
                    if let Some(cx) = cx.take() {
                        builder.parent_context = cx;
                    }
                });
            }
        });
    }

    fn add_link(&self, cx: SpanContext) {
        self.add_link_with_attributes(cx, Vec::new())
    }

    fn add_link_with_attributes(&self, cx: SpanContext, attributes: Vec<KeyValue>) {
        if cx.is_valid() {
            let mut cx = Some(cx);
            let mut att = Some(attributes);
            self.with_subscriber(move |(id, subscriber)| {
                if let Some(get_context) = subscriber.downcast_ref::<WithContext>() {
                    get_context.with_context(subscriber, id, move |builder, _tracer| {
                        if let Some(cx) = cx.take() {
                            let attr = att.take().unwrap_or_default();
                            let follows_link = opentelemetry::trace::Link::new(cx, attr);
                            builder
                                .links
                                .get_or_insert_with(|| Vec::with_capacity(1))
                                .push(follows_link);
                        }
                    });
                }
            });
        }
    }

    fn context(&self) -> Context {
        let mut cx = None;
        self.with_subscriber(|(id, subscriber)| {
            if let Some(get_context) = subscriber.downcast_ref::<WithContext>() {
                get_context.with_context(subscriber, id, |builder, tracer| {
                    cx = Some(tracer.sampled_context(builder));
                })
            }
        });

        cx.unwrap_or_default()
    }
}
