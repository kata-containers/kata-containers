use criterion::{criterion_group, criterion_main, Criterion};
use opentelemetry::metrics::{InstrumentKind, Number, NumberKind};
use opentelemetry::sdk::export::metrics::Aggregator;
use opentelemetry::{
    metrics::Descriptor,
    sdk::metrics::aggregators::{ArrayAggregator, DdSketchAggregator, DdSketchConfig},
};
use rand::Rng;
use std::sync::Arc;

fn generate_normal_data(num: usize) -> Vec<f64> {
    let mut data = Vec::with_capacity(num);
    for _ in 0..num {
        data.push(rand::thread_rng().gen_range(-100..10000) as f64);
    }
    data
}

fn ddsketch(data: Vec<f64>) {
    let aggregator =
        DdSketchAggregator::new(&DdSketchConfig::new(0.001, 2048, 1e-9), NumberKind::F64);
    let descriptor = Descriptor::new(
        "test".to_string(),
        "test",
        None,
        InstrumentKind::ValueRecorder,
        NumberKind::F64,
    );
    for f in data {
        aggregator.update(&Number::from(f), &descriptor).unwrap();
    }
    let new_aggregator: Arc<(dyn Aggregator + Send + Sync)> = Arc::new(DdSketchAggregator::new(
        &DdSketchConfig::new(0.001, 2048, 1e-9),
        NumberKind::F64,
    ));
    aggregator
        .synchronized_move(&new_aggregator, &descriptor)
        .unwrap();
}

fn array(data: Vec<f64>) {
    let aggregator = ArrayAggregator::default();
    let descriptor = Descriptor::new(
        "test".to_string(),
        "test",
        None,
        InstrumentKind::ValueRecorder,
        NumberKind::F64,
    );
    for f in data {
        aggregator.update(&Number::from(f), &descriptor).unwrap();
    }
    let new_aggregator: Arc<(dyn Aggregator + Send + Sync)> = Arc::new(ArrayAggregator::default());
    aggregator
        .synchronized_move(&new_aggregator, &descriptor)
        .unwrap();
}

pub fn histogram(c: &mut Criterion) {
    let data = generate_normal_data(5000);
    c.bench_function("ddsketch", |b| {
        b.iter(|| {
            ddsketch(data.clone());
        })
    });
    c.bench_function("array", |b| b.iter(|| array(data.clone())));
}

criterion_group!(benches, histogram);
criterion_main!(benches);
