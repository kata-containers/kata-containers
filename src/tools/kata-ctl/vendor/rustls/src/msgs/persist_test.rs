use super::codec::{Codec, Reader};
use super::enums::*;
use super::persist::*;

use crate::key::Certificate;
use crate::ticketer::TimeBase;
use crate::tls13::TLS13_AES_128_GCM_SHA256;

use std::convert::TryInto;

#[test]
fn clientsessionkey_is_debug() {
    let name = "hello".try_into().unwrap();
    let csk = ClientSessionKey::session_for_server_name(&name);
    println!("{:?}", csk);
}

#[test]
fn clientsessionkey_cannot_be_read() {
    let bytes = [0; 1];
    let mut rd = Reader::init(&bytes);
    assert!(ClientSessionKey::read(&mut rd).is_none());
}

#[test]
fn clientsessionvalue_is_debug() {
    let csv = ClientSessionValue::from(Tls13ClientSessionValue::new(
        TLS13_AES_128_GCM_SHA256
            .tls13()
            .unwrap(),
        vec![],
        vec![1, 2, 3],
        vec![Certificate(b"abc".to_vec()), Certificate(b"def".to_vec())],
        TimeBase::now().unwrap(),
        15,
        10,
        128,
    ));
    println!("{:?}", csv);
}

#[test]
fn serversessionvalue_is_debug() {
    let ssv = ServerSessionValue::new(
        None,
        ProtocolVersion::TLSv1_3,
        CipherSuite::TLS13_AES_128_GCM_SHA256,
        vec![1, 2, 3],
        None,
        None,
        vec![4, 5, 6],
        TimeBase::now().unwrap(),
        0x12345678,
    );
    println!("{:?}", ssv);
}

#[test]
fn serversessionvalue_no_sni() {
    let bytes = [
        0x00, 0x03, 0x03, 0xc0, 0x23, 0x03, 0x01, 0x02, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x12,
        0x23, 0x34, 0x45, 0x56, 0x67, 0x78, 0x89, 0xfe, 0xed, 0xf0, 0x0d,
    ];
    let mut rd = Reader::init(&bytes);
    let ssv = ServerSessionValue::read(&mut rd).unwrap();
    assert_eq!(ssv.get_encoding(), bytes);
}

#[test]
fn serversessionvalue_with_cert() {
    let bytes = [
        0x00, 0x03, 0x03, 0xc0, 0x23, 0x03, 0x01, 0x02, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x12,
        0x23, 0x34, 0x45, 0x56, 0x67, 0x78, 0x89, 0xfe, 0xed, 0xf0, 0x0d,
    ];
    let mut rd = Reader::init(&bytes);
    let ssv = ServerSessionValue::read(&mut rd).unwrap();
    assert_eq!(ssv.get_encoding(), bytes);
}
