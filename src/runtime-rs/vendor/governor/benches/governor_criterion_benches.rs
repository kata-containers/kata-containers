use criterion::{criterion_group, criterion_main};

mod multi_threaded;
mod realtime_clock;
mod single_threaded;

criterion_group!(
    benches,
    realtime_clock::bench_all,
    single_threaded::bench_all,
    multi_threaded::bench_all,
);
criterion_main!(benches);
