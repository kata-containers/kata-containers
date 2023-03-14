// SPDX-License-Identifier: MIT

use criterion::{criterion_group, criterion_main, Criterion};

use netlink_packet_route::{
    nlas::link::Nla,
    traits::{Parseable, ParseableParametrized},
    LinkHeader,
    LinkMessage,
    LinkMessageBuffer,
};

const LINKMSG1: [u8; 96] = [
    0x00, // address family
    0x00, // reserved
    0x04, 0x03, // link layer type 772 = loopback
    0x01, 0x00, 0x00, 0x00, // interface index = 1
    // Note: in the wireshark capture, the thrid byte is 0x01
    // but that does not correpond to any of the IFF_ flags...
    0x49, 0x00, 0x00, 0x00, // device flags: UP, LOOPBACK, RUNNING, LOWERUP
    0x00, 0x00, 0x00, 0x00, // reserved 2 (aka device change flag)
    // nlas
    0x07, 0x00, 0x03, 0x00, 0x6c, 0x6f, 0x00, // device name L=7,T=3,V=lo
    0x00, // padding
    0x08, 0x00, 0x0d, 0x00, 0xe8, 0x03, 0x00, 0x00, // TxQueue length L=8,T=13,V=1000
    0x05, 0x00, 0x10, 0x00, 0x00, // OperState L=5,T=16,V=0 (unknown)
    0x00, 0x00, 0x00, // padding
    0x05, 0x00, 0x11, 0x00, 0x00, // Link mode L=5,T=17,V=0
    0x00, 0x00, 0x00, // padding
    0x08, 0x00, 0x04, 0x00, 0x00, 0x00, 0x01, 0x00, // MTU L=8,T=4,V=65536
    0x08, 0x00, 0x1b, 0x00, 0x00, 0x00, 0x00, 0x00, // Group L=8,T=27,V=9
    0x08, 0x00, 0x1e, 0x00, 0x00, 0x00, 0x00, 0x00, // Promiscuity L=8,T=30,V=0
    0x08, 0x00, 0x1f, 0x00, 0x01, 0x00, 0x00, 0x00, // Number of Tx Queues L=8,T=31,V=1
    0x08, 0x00, 0x28, 0x00, 0xff, 0xff, 0x00,
    0x00, // Maximum GSO segment count L=8,T=40,V=65536
    0x08, 0x00, 0x29, 0x00, 0x00, 0x00, 0x01, 0x00, // Maximum GSO size L=8,T=41,V=65536
];

fn b1(c: &mut Criterion) {
    c.bench_function("parse LinkMessage header", |b| {
        b.iter(|| {
            LinkHeader::parse(&LinkMessageBuffer::new(&LINKMSG1[..])).unwrap();
        })
    });

    c.bench_function("parse LinkMessage nlas", |b| {
        b.iter(|| {
            Vec::<Nla>::parse_with_param(&LinkMessageBuffer::new(&&LINKMSG1[..]), 0_u8).unwrap();
        })
    });

    c.bench_function("parse LinkMessage", |b| {
        b.iter(|| {
            LinkMessage::parse(&LinkMessageBuffer::new(&&LINKMSG1[..])).unwrap();
        })
    });
}

criterion_group!(benches, b1);
criterion_main!(benches);
