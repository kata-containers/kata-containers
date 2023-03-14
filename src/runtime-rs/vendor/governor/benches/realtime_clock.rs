//! Benchmarks to determine the performance of measuring against the default real-time clock.
//!
//! The two functions in here measure the throughput against a rate-limiter that mostly allows
//! (allowing max_value of `u32` per nanosecond), and one that mostly denies (allowing only one
//! per hour).

use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use governor::{clock, Quota, RateLimiter};
use nonzero_ext::*;
use std::time::Duration;

pub fn bench_all(c: &mut Criterion) {
    bench_mostly_allow(c);
    bench_mostly_deny(c);
}

macro_rules! with_realtime_clocks {
    {($name:expr, $group:ident) |$b:pat, $clock:pat| $closure:block} => {
        {
            let clock = clock::MonotonicClock::default();
            $group.bench_with_input(BenchmarkId::new($name, "MonotonicClock"), &clock, |$b, $clock| $closure);
        }
        {
            let clock = clock::QuantaClock::default();
            $group.bench_with_input(BenchmarkId::new($name, "QuantaClock"), &clock, |$b, $clock| $closure);
        }
        {
            let clock = clock::QuantaUpkeepClock::from_interval(Duration::from_micros(40))
                .expect("could not spawn upkeep thread");
            $group.bench_with_input(BenchmarkId::new($name, "QuantaUpkeepClock"), &clock, |$b, $clock| $closure);
        }
    };
}

fn bench_mostly_allow(c: &mut Criterion) {
    let mut group = c.benchmark_group("realtime_clock");
    group.throughput(Throughput::Elements(1));
    with_realtime_clocks! {("mostly_allow", group) |b, clock| {
        let rl = RateLimiter::direct_with_clock(
            #[allow(deprecated)] Quota::new(nonzero!(u32::max_value()), Duration::from_nanos(1)).unwrap(),
            clock
        );
        b.iter(|| {
            black_box(rl.check().is_ok());
        });
    }};
    group.finish();
}

fn bench_mostly_deny(c: &mut Criterion) {
    let mut group = c.benchmark_group("realtime_clock");
    group.throughput(Throughput::Elements(1));
    with_realtime_clocks! {("mostly_deny", group) |b, clock| {
        let rl = RateLimiter::direct_with_clock(Quota::per_hour(nonzero!(1u32)), clock);
        b.iter(|| {
            black_box(rl.check().is_ok());
        });
    }};
    group.finish();
}
