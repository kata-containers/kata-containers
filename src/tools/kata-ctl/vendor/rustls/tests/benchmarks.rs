use criterion::criterion_group;
use criterion::criterion_main;
/// Microbenchmarks go here.  Larger benchmarks of (e.g..) protocol
/// performance go in examples/internal/bench.rs.
use criterion::Criterion;

#[allow(dead_code)]
mod common;
use crate::common::*;

use rustls::ServerConnection;

use std::io;
use std::sync::Arc;

fn bench_ewouldblock(c: &mut Criterion) {
    let server_config = make_server_config(KeyType::Rsa);
    let mut server = ServerConnection::new(Arc::new(server_config)).unwrap();
    let mut read_ewouldblock = FailsReads::new(io::ErrorKind::WouldBlock);
    c.bench_function("read_tls with EWOULDBLOCK", move |b| {
        b.iter(|| server.read_tls(&mut read_ewouldblock))
    });
}

criterion_group!(benches, bench_ewouldblock);
criterion_main!(benches);
