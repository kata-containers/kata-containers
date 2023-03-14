use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use opentelemetry::runtime::Tokio;
use opentelemetry::sdk::export::trace::SpanData;
use opentelemetry::sdk::trace::{BatchSpanProcessor, EvictedHashMap, EvictedQueue, SpanProcessor};
use opentelemetry::trace::{
    noop::NoopSpanExporter, SpanContext, SpanId, SpanKind, StatusCode, TraceFlags, TraceId,
    TraceState,
};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::runtime::Runtime;

fn get_span_data() -> Vec<SpanData> {
    (0..200)
        .into_iter()
        .map(|_| SpanData {
            span_context: SpanContext::new(
                TraceId::from_u128(12),
                SpanId::from_u64(12),
                TraceFlags::default(),
                false,
                TraceState::default(),
            ),
            parent_span_id: SpanId::from_u64(12),
            span_kind: SpanKind::Client,
            name: Default::default(),
            start_time: SystemTime::now(),
            end_time: SystemTime::now(),
            attributes: EvictedHashMap::new(12, 12),
            events: EvictedQueue::new(12),
            links: EvictedQueue::new(12),
            status_code: StatusCode::Unset,
            status_message: Default::default(),
            resource: None,
            instrumentation_lib: Default::default(),
        })
        .collect::<Vec<SpanData>>()
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("BatchSpanProcessor");
    group.sample_size(50);

    for task_num in [1, 2, 4, 8, 16, 32].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("with {} concurrent task", task_num)),
            task_num,
            |b, &task_num| {
                b.iter(|| {
                    let rt = Runtime::new().unwrap();
                    rt.block_on(async move {
                        let span_processor =
                            BatchSpanProcessor::builder(NoopSpanExporter::new(), Tokio)
                                .with_max_queue_size(10_000)
                                .build();
                        let mut shared_span_processor = Arc::new(span_processor);
                        let mut handles = Vec::with_capacity(10);
                        for _ in 0..task_num {
                            let span_processor = shared_span_processor.clone();
                            let spans = get_span_data();
                            handles.push(tokio::spawn(async move {
                                for span in spans {
                                    span_processor.on_end(span);
                                    tokio::task::yield_now().await;
                                }
                            }));
                        }
                        futures::future::join_all(handles).await;
                        let _ =
                            Arc::<BatchSpanProcessor<Tokio>>::get_mut(&mut shared_span_processor)
                                .unwrap()
                                .shutdown();
                    });
                })
            },
        );
    }

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
