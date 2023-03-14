// SPDX-License-Identifier: MIT

#![cfg(test)]

use crate::{
    constants::*,
    nlas::NlasIterator,
    tc::{ingress, Nla, Stats, Stats2, StatsBuffer, TC_HEADER_LEN},
    traits::{Emitable, Parseable},
    TcHeader,
    TcMessage,
    TcMessageBuffer,
};

#[rustfmt::skip]
    static QDISC_INGRESS_PACKET: [u8; 136] = [
        0,       // family
        0, 0, 0, // pad1 + pad2
        84, 0, 0, 0, // Interface index = 84
        0, 0, 255, 255, // handle:  0xffff0000
        241, 255, 255, 255, // parent: 0xfffffff1
        1, 0, 0, 0, // info: refcnt: 1

        // nlas
        12, 0, // length
        1, 0,  // type: TCA_KIND
        105, 110, 103, 114, 101, 115, 115, 0, // ingress\0

        4, 0, // length
        2, 0, // type: TCA_OPTIONS

        5, 0, // length
        12, 0,// type: TCA_HW_OFFLOAD
        0,    // data: 0
        0, 0, 0,// padding

        48, 0, // length
        7, 0,  // type: TCA_STATS2
            20, 0, // length
            1, 0, // type: TCA_STATS_BASIC
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            24, 0,
            3, 0, // type: TCA_STATS_QUEUE
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,

        44, 0, // length
        3, 0,  // type: TCA_STATS
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0
    ];

#[test]
fn tc_packet_header_read() {
    let packet = TcMessageBuffer::new(QDISC_INGRESS_PACKET);
    assert_eq!(packet.family(), 0);
    assert_eq!(packet.index(), 84);
    assert_eq!(packet.handle(), 0xffff0000);
    assert_eq!(packet.parent(), 0xfffffff1);
    assert_eq!(packet.info(), 1);
}

#[test]
fn tc_packet_header_build() {
    let mut buf = vec![0xff; TC_HEADER_LEN];
    {
        let mut packet = TcMessageBuffer::new(&mut buf);
        packet.set_family(0);
        packet.set_pad1(0);
        packet.set_pad2(0);
        packet.set_index(84);
        packet.set_handle(0xffff0000);
        packet.set_parent(0xfffffff1);
        packet.set_info(1);
    }
    assert_eq!(&buf[..], &QDISC_INGRESS_PACKET[0..TC_HEADER_LEN]);
}

#[test]
fn tc_packet_nlas_read() {
    let packet = TcMessageBuffer::new(&QDISC_INGRESS_PACKET[..]);
    assert_eq!(packet.nlas().count(), 5);
    let mut nlas = packet.nlas();

    let nla = nlas.next().unwrap().unwrap();
    nla.check_buffer_length().unwrap();
    assert_eq!(nla.length(), 12);
    assert_eq!(nla.kind(), TCA_KIND);
    assert_eq!(nla.value(), "ingress\0".as_bytes());

    let nla = nlas.next().unwrap().unwrap();
    nla.check_buffer_length().unwrap();
    assert_eq!(nla.length(), 4);
    assert_eq!(nla.kind(), TCA_OPTIONS);
    assert_eq!(nla.value(), []);

    let nla = nlas.next().unwrap().unwrap();
    nla.check_buffer_length().unwrap();
    assert_eq!(nla.length(), 5);
    assert_eq!(nla.kind(), TCA_HW_OFFLOAD);
    assert_eq!(nla.value(), [0]);

    let nla = nlas.next().unwrap().unwrap();
    nla.check_buffer_length().unwrap();
    assert_eq!(nla.length(), 48);
    assert_eq!(nla.kind(), TCA_STATS2);

    let mut stats2_iter = NlasIterator::new(nla.value());
    let stats2_nla = stats2_iter.next().unwrap().unwrap();
    stats2_nla.check_buffer_length().unwrap();
    assert_eq!(stats2_nla.length(), 20);
    assert_eq!(stats2_nla.kind(), TCA_STATS_BASIC);
    assert_eq!(stats2_nla.value(), [0; 16]);
    let s2 = Stats2::parse(&stats2_nla).unwrap();
    assert!(matches!(s2, Stats2::StatsBasic(_)));

    let stats2_nla = stats2_iter.next().unwrap().unwrap();
    stats2_nla.check_buffer_length().unwrap();
    assert_eq!(stats2_nla.length(), 24);
    assert_eq!(stats2_nla.kind(), TCA_STATS_QUEUE);
    assert_eq!(stats2_nla.value(), [0; 20]);
    let s2 = Stats2::parse(&stats2_nla).unwrap();
    assert!(matches!(s2, Stats2::StatsQueue(_)));

    let nla = nlas.next().unwrap().unwrap();
    nla.check_buffer_length().unwrap();
    assert_eq!(nla.length(), 44);
    assert_eq!(nla.kind(), TCA_STATS);
    assert_eq!(nla.value(), [0; 40]);
    let s = Stats::parse(&StatsBuffer::new(nla.value())).unwrap();
    assert_eq!(s.packets, 0);
    assert_eq!(s.backlog, 0);
}

#[test]
fn tc_qdisc_ingress_emit() {
    let header = TcHeader {
        index: 84,
        handle: 0xffff0000,
        parent: 0xfffffff1,
        info: 1,
        ..Default::default()
    };

    let nlas = vec![Nla::Kind(ingress::KIND.into()), Nla::Options(vec![])];

    let msg = TcMessage::from_parts(header, nlas);
    let mut buf = vec![0; 36];
    assert_eq!(msg.buffer_len(), 36);
    msg.emit(&mut buf[..]);
    assert_eq!(&buf, &QDISC_INGRESS_PACKET[..36]);
}

#[test]
fn tc_qdisc_ingress_read() {
    let packet = TcMessageBuffer::new_checked(&QDISC_INGRESS_PACKET).unwrap();

    let msg = TcMessage::parse(&packet).unwrap();
    assert_eq!(msg.header.index, 84);
    assert_eq!(msg.nlas.len(), 5);

    let mut iter = msg.nlas.iter();

    let nla = iter.next().unwrap();
    assert_eq!(nla, &Nla::Kind(String::from(ingress::KIND)));

    let nla = iter.next().unwrap();
    assert_eq!(nla, &Nla::Options(vec![]));

    let nla = iter.next().unwrap();
    assert_eq!(nla, &Nla::HwOffload(0));
}
