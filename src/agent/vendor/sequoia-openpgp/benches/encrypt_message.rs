use criterion::{criterion_group, BenchmarkId, Criterion, Throughput};

use sequoia_openpgp as openpgp;
use openpgp::cert::Cert;
use openpgp::parse::Parse;

use crate::common::encrypt;

lazy_static::lazy_static! {
    static ref ZEROS_1_MB: Vec<u8> = vec![0; 1024 * 1024];
    static ref ZEROS_10_MB: Vec<u8> = vec![0; 10 * 1024 * 1024];
}

pub fn encrypt_to_testy(bytes: &[u8]) {
    let testy =
        Cert::from_bytes(&include_bytes!("../tests/data/keys/testy.pgp")[..])
            .unwrap();
    encrypt::encrypt_to_cert(bytes, &testy).unwrap();
}

pub fn encrypt_with_password(bytes: &[u8]) {
    let password = "ściśle tajne";
    encrypt::encrypt_with_password(bytes, password).unwrap();
}

fn bench_encrypt(c: &mut Criterion) {
    let mut group = c.benchmark_group("encrypt message");

    // Encrypt a very short, medium and very long message.
    let messages = &[b"Hello world.", &ZEROS_1_MB[..], &ZEROS_10_MB[..]];

    for message in messages {
        group.throughput(Throughput::Bytes(message.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("password", message.len()),
            &message,
            |b, m| b.iter(|| encrypt_with_password(m)),
        );
        group.bench_with_input(
            BenchmarkId::new("cert", message.len()),
            &message,
            |b, m| b.iter(|| encrypt_to_testy(m)),
        );
    }
    group.finish();
}

criterion_group!(benches, bench_encrypt);
