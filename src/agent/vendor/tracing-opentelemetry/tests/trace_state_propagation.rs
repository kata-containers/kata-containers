use async_trait::async_trait;
use opentelemetry::{
    propagation::TextMapPropagator,
    sdk::{
        export::trace::{SpanData, SpanExporter},
        propagation::{BaggagePropagator, TextMapCompositePropagator, TraceContextPropagator},
        trace::{Tracer, TracerProvider},
    },
    trace::{SpanContext, TraceContextExt, Tracer as _, TracerProvider as _},
    Context,
};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tracing::Subscriber;
use tracing_opentelemetry::{layer, OpenTelemetrySpanExt};
use tracing_subscriber::prelude::*;

#[test]
fn trace_with_active_otel_context() {
    let (cx, subscriber, exporter, provider) = build_sampled_context();
    let attached = cx.attach();

    tracing::subscriber::with_default(subscriber, || {
        tracing::debug_span!("child");
    });

    drop(attached); // end implicit parent
    drop(provider); // flush all spans

    let spans = exporter.0.lock().unwrap();
    assert_eq!(spans.len(), 2);
    assert_shared_attrs_eq(&spans[0].span_context, &spans[1].span_context);
}

#[test]
fn trace_with_assigned_otel_context() {
    let (cx, subscriber, exporter, provider) = build_sampled_context();

    tracing::subscriber::with_default(subscriber, || {
        let child = tracing::debug_span!("child");
        child.set_parent(cx);
    });

    drop(provider); // flush all spans
    let spans = exporter.0.lock().unwrap();
    assert_eq!(spans.len(), 2);
    assert_shared_attrs_eq(&spans[0].span_context, &spans[1].span_context);
}

#[test]
fn trace_root_with_children() {
    let (_tracer, provider, exporter, subscriber) = test_tracer();

    tracing::subscriber::with_default(subscriber, || {
        // Propagate trace information through tracing parent -> child
        let root = tracing::debug_span!("root");
        root.in_scope(|| tracing::debug_span!("child"));
    });

    drop(provider); // flush all spans
    let spans = exporter.0.lock().unwrap();
    assert_eq!(spans.len(), 2);
    assert_shared_attrs_eq(&spans[0].span_context, &spans[1].span_context);
}

#[test]
fn inject_context_into_outgoing_requests() {
    let (_tracer, _provider, _exporter, subscriber) = test_tracer();
    let propagator = test_propagator();
    let carrier = test_carrier();
    let cx = propagator.extract(&carrier);
    let mut outgoing_req_carrier = HashMap::new();

    tracing::subscriber::with_default(subscriber, || {
        let root = tracing::debug_span!("root");
        root.set_parent(cx);
        let _g = root.enter();
        let child = tracing::debug_span!("child");
        propagator.inject_context(&child.context(), &mut outgoing_req_carrier);
    });

    // Ensure all values that should be passed between services are preserved
    assert_carrier_attrs_eq(&carrier, &outgoing_req_carrier);
}

fn assert_shared_attrs_eq(sc_a: &SpanContext, sc_b: &SpanContext) {
    assert_eq!(sc_a.trace_id(), sc_b.trace_id());
    assert_eq!(sc_a.trace_state(), sc_b.trace_state());
}

fn assert_carrier_attrs_eq(
    carrier_a: &HashMap<String, String>,
    carrier_b: &HashMap<String, String>,
) {
    // Match baggage unordered
    assert_eq!(
        carrier_a
            .get("baggage")
            .map(|b| b.split_terminator(',').collect::<HashSet<_>>()),
        carrier_b
            .get("baggage")
            .map(|b| b.split_terminator(',').collect())
    );
    // match trace parent values, except span id
    assert_eq!(
        carrier_a.get("traceparent").unwrap()[0..36],
        carrier_b.get("traceparent").unwrap()[0..36],
    );
    // match tracestate values
    assert_eq!(carrier_a.get("tracestate"), carrier_b.get("tracestate"));
}

fn test_tracer() -> (Tracer, TracerProvider, TestExporter, impl Subscriber) {
    let exporter = TestExporter::default();
    let provider = TracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let tracer = provider.get_tracer("test", None);
    let subscriber = tracing_subscriber::registry().with(layer().with_tracer(tracer.clone()));

    (tracer, provider, exporter, subscriber)
}

fn test_propagator() -> TextMapCompositePropagator {
    let baggage_propagator = BaggagePropagator::new();
    let trace_context_propagator = TraceContextPropagator::new();

    TextMapCompositePropagator::new(vec![
        Box::new(baggage_propagator),
        Box::new(trace_context_propagator),
    ])
}

fn test_carrier() -> HashMap<String, String> {
    let mut carrier = HashMap::new();
    carrier.insert(
        "baggage".to_string(),
        "key2=value2,key1=value1;property1;property2,key3=value3;propertyKey=propertyValue"
            .to_string(),
    );
    carrier.insert("tracestate".to_string(), "test1=test2".to_string());
    carrier.insert(
        "traceparent".to_string(),
        "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01".to_string(),
    );

    carrier
}

fn build_sampled_context() -> (Context, impl Subscriber, TestExporter, TracerProvider) {
    let (tracer, provider, exporter, subscriber) = test_tracer();
    let span = tracer.start("sampled");
    let cx = Context::current_with_span(span);

    (cx, subscriber, exporter, provider)
}

#[derive(Clone, Default, Debug)]
struct TestExporter(Arc<Mutex<Vec<SpanData>>>);

#[async_trait]
impl SpanExporter for TestExporter {
    async fn export(
        &mut self,
        mut batch: Vec<SpanData>,
    ) -> opentelemetry::sdk::export::trace::ExportResult {
        if let Ok(mut inner) = self.0.lock() {
            inner.append(&mut batch);
        }
        Ok(())
    }
}
