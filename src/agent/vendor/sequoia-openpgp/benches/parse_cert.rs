use criterion::{
    criterion_group, BenchmarkGroup, BenchmarkId, Criterion, Throughput,
};

use sequoia_openpgp as openpgp;
use openpgp::cert::{Cert, CertBuilder};
use openpgp::packet::prelude::*;
use openpgp::packet::{Signature, UserID};
use openpgp::parse::Parse;
use openpgp::serialize::SerializeInto;
use openpgp::types::{Curve, KeyFlags, SignatureType};
use openpgp::Result;

use std::convert::TryInto;

fn generate_certifications<'a>(
    userid: &'a UserID,
    cert: &'a Cert,
    count: usize,
) -> Result<impl Iterator<Item = Signature> + 'a> {

    let k: Key<key::SecretParts, key::PrimaryRole> =
        Key4::generate_ecc(true, Curve::Ed25519)?.into();
    let mut keypair = k.into_keypair()?;

    let iter = (0..count).map(move |_| {
        userid
            .certify(
                &mut keypair,
                cert,
                SignatureType::PositiveCertification,
                None,
                None,
            )
            .unwrap()
    });
    Ok(iter)
}

fn generate_flooded_cert(
    key_count: usize,
    sigs_per_key: usize,
) -> Result<Vec<u8>> {
    // Generate a Cert for to be flooded
    let (mut floodme, _) = CertBuilder::new()
        .set_primary_key_flags(KeyFlags::empty().set_certification())
        .add_userid("flood.me@example.org")
        .generate()?;

    let floodme_cloned = floodme.clone();
    let userid = floodme_cloned.userids().next().unwrap();

    let certifications = (0..key_count).flat_map(|_| {
        generate_certifications(&userid, &floodme_cloned, sigs_per_key)
            .unwrap()
    });

    floodme = floodme.insert_packets(certifications)?;
    floodme.export_to_vec()
}

/// Parse the cert, unwrap to notice errors
fn read_cert(bytes: &[u8]) {
    Cert::from_bytes(bytes).unwrap();
}

/// Generate the cert and benchmark parsing.
/// The generated cert is signed by multiple other keys, 1 signature per key.
fn parse_cert_generated(
    group: &mut BenchmarkGroup<'_, criterion::measurement::WallTime>,
    name: &str,
    signature_count: usize,
) {
    let bytes = generate_flooded_cert(signature_count, 1).unwrap();

    group.throughput(Throughput::Bytes(bytes.len().try_into().unwrap()));

    group.bench_with_input(
        BenchmarkId::new(name, signature_count),
        &bytes,
        |b, bytes| b.iter(|| read_cert(bytes)),
    );
}

/// Benchmark parsing a generated cert with a given number of signatures
fn bench_parse_certs_generated(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse flooded cert");
    parse_cert_generated(&mut group, "flooded", 100);
    parse_cert_generated(&mut group, "flooded", 316);
    parse_cert_generated(&mut group, "flooded", 1000);
    parse_cert_generated(&mut group, "flooded", 3162);
    group.finish();
}

/// Benchmark parsing a typical cert
fn bench_parse_certs(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse typical cert");
    let bytes = include_bytes!("../tests/data/keys/neal.pgp");
    group.throughput(Throughput::Bytes(bytes.len().try_into().unwrap()));
    group.bench_function("neal.pgp", |b| b.iter(|| read_cert(bytes)));
    group.finish();
}

criterion_group!(benches, bench_parse_certs, bench_parse_certs_generated);
