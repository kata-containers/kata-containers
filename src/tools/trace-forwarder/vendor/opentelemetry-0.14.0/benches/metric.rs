use criterion::{
    criterion_group, criterion_main, measurement::Measurement, BenchmarkGroup, BenchmarkId,
    Criterion,
};
use opentelemetry::{
    metrics::{Descriptor, Meter},
    sdk::{
        export::metrics::{AggregatorSelector, Processor},
        metrics::{accumulator, aggregators},
    },
    Key, KeyValue,
};
use rand::{rngs, Rng};
use std::cell::RefCell;
use std::sync::Arc;

pub fn counters(c: &mut Criterion) {
    let meter = build_meter();

    let mut g = c.benchmark_group("Counter");

    // unbound u64
    let counter = meter.u64_counter("u64_unbound.sum").init();
    benchmark_unbound_metric("u64_unbound", &mut g, |labels| counter.add(1, labels));

    // bound u64
    g.bench_with_input(
        BenchmarkId::new("u64_bound", 1),
        &meter
            .u64_counter("u64_bound.sum")
            .init()
            .bind(build_kv(1).as_ref()),
        |b, counter| b.iter(|| counter.add(1)),
    );

    // unbound f64
    let counter = meter.f64_counter("f64_unbound.sum").init();
    benchmark_unbound_metric("f64_unbound", &mut g, |labels| counter.add(1.0, labels));

    // bound f64
    g.bench_with_input(
        BenchmarkId::new("f64_bound", 1.0),
        &meter
            .f64_counter("f64_bound.sum")
            .init()
            .bind(build_kv(1).as_ref()),
        |b, counter| b.iter(|| counter.add(1.0)),
    );

    // acquire handle
    benchmark_unbound_metric("f64_bind", &mut g, |labels| {
        let _ = counter.bind(labels);
    });

    g.finish();
}

fn benchmark_unbound_metric<M: Measurement, F: Fn(&[KeyValue])>(
    name: &str,
    g: &mut BenchmarkGroup<M>,
    f: F,
) {
    for (num, kvs) in [
        ("1", build_kv(1)),
        ("2", build_kv(2)),
        ("4", build_kv(4)),
        ("8", build_kv(8)),
        ("16", build_kv(16)),
    ]
    .iter()
    {
        g.bench_with_input(BenchmarkId::new(name, num), kvs, |b, kvs| b.iter(|| f(kvs)));
    }
}

fn build_kv(n: u8) -> Vec<KeyValue> {
    let mut res = Vec::new();

    CURRENT_RNG.with(|rng| {
        let mut rng = rng.borrow_mut();
        for _ in 0..n {
            let k = Key::new(format!("k_{}", rng.gen::<f64>() * 1_000_000_000.0));
            res.push(k.string(format!("v_{}", rng.gen::<f64>() * 1_000_000_000.0)));
        }
    });

    res
}
thread_local! {
    static CURRENT_RNG: RefCell<rngs::ThreadRng> = RefCell::new(rngs::ThreadRng::default());
}

#[derive(Debug, Default)]
struct BenchAggregatorSelector;

impl AggregatorSelector for BenchAggregatorSelector {
    fn aggregator_for(
        &self,
        descriptor: &Descriptor,
    ) -> Option<Arc<dyn opentelemetry::sdk::export::metrics::Aggregator + Send + Sync>> {
        match descriptor.name() {
            name if name.ends_with(".disabled") => None,
            name if name.ends_with(".sum") => Some(Arc::new(aggregators::sum())),
            name if name.ends_with(".minmaxsumcount") => {
                Some(Arc::new(aggregators::min_max_sum_count(descriptor)))
            }
            name if name.ends_with(".lastvalue") => Some(Arc::new(aggregators::last_value())),
            name if name.ends_with(".histogram") => {
                Some(Arc::new(aggregators::histogram(descriptor, &[])))
            }
            name if name.ends_with(".exact") => Some(Arc::new(aggregators::array())),
            _ => panic!(
                "Invalid instrument name for test AggregatorSelector: {}",
                descriptor.name()
            ),
        }
    }
}

#[derive(Debug, Default)]
struct BenchProcessor {
    aggregation_selector: BenchAggregatorSelector,
}

impl Processor for BenchProcessor {
    fn aggregation_selector(&self) -> &dyn AggregatorSelector {
        &self.aggregation_selector
    }
}

fn build_meter() -> Meter {
    let processor = Arc::new(BenchProcessor::default());
    let core = accumulator(processor).build();
    Meter::new("benches", None, Arc::new(core))
}

criterion_group!(benches, counters);
criterion_main!(benches);
