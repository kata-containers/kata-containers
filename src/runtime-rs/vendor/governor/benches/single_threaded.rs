use criterion::{black_box, BatchSize, BenchmarkId, Criterion, Throughput};
use governor::{clock, Quota, RateLimiter};
use governor::{
    middleware::NoOpMiddleware,
    state::keyed::{DashMapStateStore, HashMapStateStore, KeyedStateStore},
};
use nonzero_ext::*;
use std::time::Duration;
use tynm::type_name;

pub fn bench_all(c: &mut Criterion) {
    bench_direct(c);
    bench_keyed::<HashMapStateStore<u32>>(c);
    bench_keyed::<DashMapStateStore<u32>>(c);
}

fn bench_direct(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_threaded");
    group.throughput(Throughput::Elements(1));
    group.bench_function("direct", |b| {
        let clock = clock::FakeRelativeClock::default();
        let step = Duration::from_millis(20);
        let rl = RateLimiter::direct_with_clock(Quota::per_second(nonzero!(50u32)), &clock);
        b.iter_batched(
            || {
                clock.advance(step);
            },
            |()| {
                black_box(rl.check().is_ok());
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn bench_keyed<M: KeyedStateStore<u32> + Default + Send + Sync + 'static>(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_threaded");
    group.throughput(Throughput::Elements(3));

    group
        .bench_function(BenchmarkId::new("keyed", type_name::<M>()), |b| {
            let state: M = Default::default();
            let clock = clock::FakeRelativeClock::default();
            let step = Duration::from_millis(20);
            let rl: RateLimiter<
                _,
                _,
                _,
                NoOpMiddleware<<clock::FakeRelativeClock as clock::Clock>::Instant>,
            > = RateLimiter::new(Quota::per_second(nonzero!(50u32)), state, &clock);
            b.iter_batched(
                || {
                    clock.advance(step);
                },
                |()| {
                    black_box(rl.check_key(&1u32).is_ok());
                },
                BatchSize::SmallInput,
            );
        })
        .throughput(Throughput::Elements(1));
    group.finish();
}
