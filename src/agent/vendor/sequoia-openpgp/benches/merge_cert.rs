use criterion::{
    criterion_group, Criterion,
};

use sequoia_openpgp as openpgp;
use openpgp::cert::Cert;
use openpgp::parse::Parse;

/// Benchmark merging a typical cert with itself.
fn bench_merge_certs(c: &mut Criterion) {
    let mut group = c.benchmark_group("merge cert with itself");
    let neal = Cert::from_bytes(include_bytes!("../tests/data/keys/neal.pgp"))
        .unwrap();
    group.bench_function("neal.pgp", |b| b.iter(|| {
        neal.clone().merge_public(neal.clone()).unwrap();
    }));
    group.finish();
}

criterion_group!(benches, bench_merge_certs);
