use criterion::{
    criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};

use sequoia_openpgp as openpgp;
use openpgp::cert::Cert;
use openpgp::parse::Parse;

use crate::common::{decrypt, encrypt};

lazy_static::lazy_static! {
    static ref SENDER: Cert =
        Cert::from_bytes(&include_bytes!("../tests/data/keys/sender.pgp")[..])
        .unwrap();
    static ref RECIPIENT: Cert =
        Cert::from_bytes(&include_bytes!("../tests/data/keys/recipient.pgp")[..])
        .unwrap();
    static ref ZEROS_1_MB: Vec<u8> = vec![0; 1024 * 1024];
    static ref ZEROS_10_MB: Vec<u8> = vec![0; 10 * 1024 * 1024];
}

fn decrypt_and_verify(bytes: &[u8], sender: &Cert, recipient: &Cert) {
    let mut sink = Vec::new();
    decrypt::decrypt_and_verify(&mut sink, bytes, sender, recipient).unwrap();
}

fn bench_decrypt_verify(c: &mut Criterion) {
    let mut group = c.benchmark_group("decrypt and verify message");

    // Encrypt a very short, medium and very long message,
    // and then benchmark decryption.
    let messages = &[b"Hello world.", &ZEROS_1_MB[..], &ZEROS_10_MB[..]];

    messages.iter().for_each(|m| {
        let encrypted =
            encrypt::encrypt_to_cert_and_sign(m, &SENDER, &RECIPIENT).unwrap();
        group.throughput(Throughput::Bytes(encrypted.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("decrypt and verify", m.len()),
            &encrypted,
            |b, e| b.iter(|| decrypt_and_verify(e, &SENDER, &RECIPIENT)),
        );
    });

    group.finish();
}

criterion_group!(benches, bench_decrypt_verify);
criterion_main!(benches);
