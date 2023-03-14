use criterion::{criterion_group, criterion_main, Criterion};
use opentelemetry::{
    sdk::trace::{Tracer, TracerProvider},
    trace::{SpanBuilder, Tracer as _, TracerProvider as _},
    Context,
};
use std::time::SystemTime;
use tracing::trace_span;
use tracing_subscriber::prelude::*;

fn many_children(c: &mut Criterion) {
    let mut group = c.benchmark_group("otel_many_children");

    group.bench_function("spec_baseline", |b| {
        let provider = TracerProvider::default();
        let tracer = provider.get_tracer("bench", None);
        b.iter(|| {
            fn dummy(tracer: &Tracer, cx: &Context) {
                for _ in 0..99 {
                    tracer.start_with_context("child", cx.clone());
                }
            }

            tracer.in_span("parent", |cx| dummy(&tracer, &cx));
        });
    });

    {
        let _subscriber = tracing_subscriber::registry()
            .with(RegistryAccessLayer)
            .set_default();
        group.bench_function("no_data_baseline", |b| b.iter(tracing_harness));
    }

    {
        let _subscriber = tracing_subscriber::registry()
            .with(OtelDataLayer)
            .set_default();
        group.bench_function("data_only_baseline", |b| b.iter(tracing_harness));
    }

    {
        let provider = TracerProvider::default();
        let tracer = provider.get_tracer("bench", None);
        let otel_layer = tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_tracked_inactivity(false);
        let _subscriber = tracing_subscriber::registry()
            .with(otel_layer)
            .set_default();

        group.bench_function("full", |b| b.iter(tracing_harness));
    }
}

struct NoDataSpan;
struct RegistryAccessLayer;

impl<S> tracing_subscriber::Layer<S> for RegistryAccessLayer
where
    S: tracing_core::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
{
    fn new_span(
        &self,
        _attrs: &tracing_core::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let span = ctx.span(id).expect("Span not found, this is a bug");
        let mut extensions = span.extensions_mut();
        extensions.insert(NoDataSpan);
    }

    fn on_close(&self, id: tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let span = ctx.span(&id).expect("Span not found, this is a bug");
        let mut extensions = span.extensions_mut();

        if let Some(no_data) = extensions.remove::<NoDataSpan>() {
            drop(no_data)
        }
    }
}

struct OtelDataLayer;

impl<S> tracing_subscriber::Layer<S> for OtelDataLayer
where
    S: tracing_core::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
{
    fn new_span(
        &self,
        attrs: &tracing_core::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let span = ctx.span(id).expect("Span not found, this is a bug");
        let mut extensions = span.extensions_mut();
        extensions.insert(
            SpanBuilder::from_name(attrs.metadata().name().to_string())
                .with_start_time(SystemTime::now()),
        );
    }

    fn on_close(&self, id: tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let span = ctx.span(&id).expect("Span not found, this is a bug");
        let mut extensions = span.extensions_mut();

        if let Some(builder) = extensions.remove::<SpanBuilder>() {
            builder.with_end_time(SystemTime::now());
        }
    }
}

fn tracing_harness() {
    fn dummy() {
        for _ in 0..99 {
            let child = trace_span!("child");
            let _enter = child.enter();
        }
    }

    let parent = trace_span!("parent");
    let _enter = parent.enter();

    dummy();
}

criterion_group!(benches, many_children);
criterion_main!(benches);
