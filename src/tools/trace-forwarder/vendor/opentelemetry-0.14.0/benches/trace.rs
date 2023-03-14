use criterion::{criterion_group, criterion_main, Criterion};
use opentelemetry::{
    sdk::{
        export::trace::{ExportResult, SpanData, SpanExporter},
        trace as sdktrace,
    },
    trace::{Span, Tracer, TracerProvider},
    Key, KeyValue,
};

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("EvictedHashMap");
    group.bench_function("insert 1", |b| {
        b.iter(|| insert_keys(sdktrace::EvictedHashMap::new(32, 1), 1))
    });
    group.bench_function("insert 5", |b| {
        b.iter(|| insert_keys(sdktrace::EvictedHashMap::new(32, 5), 5))
    });
    group.bench_function("insert 10", |b| {
        b.iter(|| insert_keys(sdktrace::EvictedHashMap::new(32, 10), 10))
    });
    group.bench_function("insert 20", |b| {
        b.iter(|| insert_keys(sdktrace::EvictedHashMap::new(32, 20), 20))
    });
    group.finish();

    trace_benchmark_group(c, "start-end-span", |tracer| tracer.start("foo").end());

    trace_benchmark_group(c, "start-end-span-4-attrs", |tracer| {
        let mut span = tracer.start("foo");
        span.set_attribute(Key::new("key1").bool(false));
        span.set_attribute(Key::new("key2").string("hello"));
        span.set_attribute(Key::new("key4").f64(123.456));
        span.end();
    });

    trace_benchmark_group(c, "start-end-span-8-attrs", |tracer| {
        let mut span = tracer.start("foo");
        span.set_attribute(Key::new("key1").bool(false));
        span.set_attribute(Key::new("key2").string("hello"));
        span.set_attribute(Key::new("key4").f64(123.456));
        span.set_attribute(Key::new("key11").bool(false));
        span.set_attribute(Key::new("key12").string("hello"));
        span.set_attribute(Key::new("key14").f64(123.456));
        span.end();
    });

    trace_benchmark_group(c, "start-end-span-all-attr-types", |tracer| {
        let mut span = tracer.start("foo");
        span.set_attribute(Key::new("key1").bool(false));
        span.set_attribute(Key::new("key2").string("hello"));
        span.set_attribute(Key::new("key3").i64(123));
        span.set_attribute(Key::new("key5").f64(123.456));
        span.end();
    });

    trace_benchmark_group(c, "start-end-span-all-attr-types-2x", |tracer| {
        let mut span = tracer.start("foo");
        span.set_attribute(Key::new("key1").bool(false));
        span.set_attribute(Key::new("key2").string("hello"));
        span.set_attribute(Key::new("key3").i64(123));
        span.set_attribute(Key::new("key5").f64(123.456));
        span.set_attribute(Key::new("key11").bool(false));
        span.set_attribute(Key::new("key12").string("hello"));
        span.set_attribute(Key::new("key13").i64(123));
        span.set_attribute(Key::new("key15").f64(123.456));
        span.end();
    });
}

const MAP_KEYS: [Key; 20] = [
    Key::from_static_str("key1"),
    Key::from_static_str("key2"),
    Key::from_static_str("key3"),
    Key::from_static_str("key4"),
    Key::from_static_str("key5"),
    Key::from_static_str("key6"),
    Key::from_static_str("key7"),
    Key::from_static_str("key8"),
    Key::from_static_str("key9"),
    Key::from_static_str("key10"),
    Key::from_static_str("key11"),
    Key::from_static_str("key12"),
    Key::from_static_str("key13"),
    Key::from_static_str("key14"),
    Key::from_static_str("key15"),
    Key::from_static_str("key16"),
    Key::from_static_str("key17"),
    Key::from_static_str("key18"),
    Key::from_static_str("key19"),
    Key::from_static_str("key20"),
];

fn insert_keys(mut map: sdktrace::EvictedHashMap, n: usize) {
    for (idx, key) in MAP_KEYS.iter().enumerate().take(n) {
        map.insert(KeyValue::new(key.clone(), idx as i64));
    }
}

#[derive(Debug)]
struct VoidExporter;

#[async_trait::async_trait]
impl SpanExporter for VoidExporter {
    async fn export(&mut self, _spans: Vec<SpanData>) -> ExportResult {
        Ok(())
    }
}

fn trace_benchmark_group<F: Fn(&sdktrace::Tracer)>(c: &mut Criterion, name: &str, f: F) {
    let mut group = c.benchmark_group(name);

    group.bench_function("always-sample", |b| {
        let provider = sdktrace::TracerProvider::builder()
            .with_config(sdktrace::config().with_sampler(sdktrace::Sampler::AlwaysOn))
            .with_simple_exporter(VoidExporter)
            .build();
        let always_sample = provider.get_tracer("always-sample", None);

        b.iter(|| f(&always_sample));
    });

    group.bench_function("never-sample", |b| {
        let provider = sdktrace::TracerProvider::builder()
            .with_config(sdktrace::config().with_sampler(sdktrace::Sampler::AlwaysOff))
            .with_simple_exporter(VoidExporter)
            .build();
        let never_sample = provider.get_tracer("never-sample", None);
        b.iter(|| f(&never_sample));
    });

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
