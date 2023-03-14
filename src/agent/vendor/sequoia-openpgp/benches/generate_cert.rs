use criterion::{criterion_group, Criterion};

use sequoia_openpgp as openpgp;
use openpgp::cert::{CertBuilder, CipherSuite};

fn generate_cert(cipher: CipherSuite) {
    // Parse the cert, ignore any errors
    let _ = CertBuilder::general_purpose(
        cipher,
        Some("Alice Lovelace <alice@example.org>"),
    )
    .generate()
    .unwrap();
}

fn bench_generate_certs(c: &mut Criterion) {
    let mut group = c.benchmark_group("generate cert");
    let cipher = CipherSuite::Cv25519;
    group.bench_function(format!("{:?}", cipher), |b| {
        b.iter(|| generate_cert(cipher))
    });
    let cipher = CipherSuite::P256;
    group.bench_function(format!("{:?}", cipher), |b| {
        b.iter(|| generate_cert(cipher))
    });
    let cipher = CipherSuite::P384;
    group.bench_function(format!("{:?}", cipher), |b| {
        b.iter(|| generate_cert(cipher))
    });
    let cipher = CipherSuite::P521;
    group.bench_function(format!("{:?}", cipher), |b| {
        b.iter(|| generate_cert(cipher))
    });
    group.finish();
}

criterion_group!(benches, bench_generate_certs);
