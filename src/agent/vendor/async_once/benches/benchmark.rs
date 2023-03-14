use async_once::AsyncOnce;
use criterion::{
    async_executor::FuturesExecutor, black_box, criterion_group, criterion_main, Criterion,
};
use lazy_static::lazy_static;
use std::ops::Deref;
use tokio::runtime::Builder;

async fn t() -> u32 {
    1
}

lazy_static! {
    static ref FOO: AsyncOnce<u32> = AsyncOnce::new(t());
}

lazy_static! {
    static ref FOO_SYNC: u32 = {
        let rt = Builder::new_current_thread().build().unwrap();
        rt.block_on(t())
    };
}

fn async_once_benchmark(c: &mut Criterion) {
    c.bench_function("async once", |b| {
        b.to_async(FuturesExecutor).iter(|| async {
            assert!(black_box(FOO.get().await) == &1);
        })
    });
}

fn sync_once_benchmark(c: &mut Criterion) {
    c.bench_function("sync once", |b| {
        b.to_async(FuturesExecutor).iter(|| async {
            assert!(black_box(FOO_SYNC.deref()) == &1);
        })
    });
}

criterion_group!(benches, async_once_benchmark, sync_once_benchmark);
criterion_main!(benches);
