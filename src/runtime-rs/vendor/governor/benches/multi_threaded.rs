//! Benchmarks measuring how long it takes a set of 20 threads to measure `iter` cells each.
//! The longest-running thread's time is reported. These benchmarks unfortunately measure a certain
//! amount of overhead in thread setup and teardown.

use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use governor::{clock, Quota, RateLimiter};
use governor::{
    middleware::NoOpMiddleware,
    state::keyed::{DashMapStateStore, HashMapStateStore, KeyedStateStore},
};
use nonzero_ext::*;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tynm::type_name;

pub fn bench_all(c: &mut Criterion) {
    bench_direct(c);
    bench_keyed::<HashMapStateStore<u32>>(c);
    bench_keyed::<DashMapStateStore<u32>>(c);
}

const THREADS: u32 = 20;

fn bench_direct(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_threaded");
    group.throughput(Throughput::Elements(1));
    group.bench_function("direct", |b| {
        let clock = clock::QuantaUpkeepClock::from_interval(Duration::from_micros(10))
            .expect("Could not spawn upkeep thread");

        b.iter_custom(|iters| {
            let lim = Arc::new(RateLimiter::direct_with_clock(
                Quota::per_second(nonzero!(50u32)),
                &clock,
            ));
            let mut children = vec![];
            let start = Instant::now();
            for _i in 0..THREADS {
                let lim = Arc::clone(&lim);
                children.push(thread::spawn(move || {
                    for _i in 0..iters {
                        black_box(lim.check().is_ok());
                    }
                }));
            }
            for child in children {
                child.join().unwrap()
            }
            start.elapsed()
        })
    });
    group.finish();
}

fn bench_keyed<M: KeyedStateStore<u32> + Default + Send + Sync + 'static>(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_threaded");

    // We perform 3 checks per thread per iter:
    group.throughput(Throughput::Elements(3));

    group.bench_function(BenchmarkId::new("keyed", type_name::<M>()), |b| {
        let clock = clock::QuantaUpkeepClock::from_interval(Duration::from_micros(10))
            .expect("Could not spawn upkeep thread");

        b.iter_custom(|iters| {
            let state: M = Default::default();
            let lim: Arc<RateLimiter<_, _, _, NoOpMiddleware>> = Arc::new(RateLimiter::new(
                Quota::per_second(nonzero!(50u32)),
                state,
                &clock,
            ));

            let mut children = vec![];
            let start = Instant::now();
            for _i in 0..THREADS {
                let lim = Arc::clone(&lim);
                children.push(thread::spawn(move || {
                    for _i in 0..iters {
                        black_box(lim.check_key(&1u32).is_ok());
                        black_box(lim.check_key(&2u32).is_ok());
                        black_box(lim.check_key(&3u32).is_ok());
                    }
                }));
            }
            for child in children {
                child.join().unwrap()
            }
            start.elapsed()
        })
    });
    group.finish();
}
