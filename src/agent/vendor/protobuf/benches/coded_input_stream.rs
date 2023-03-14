// `cargo test --benches` and `#[feature(test)]` work only in nightly
#![cfg(rustc_nightly)]
#![feature(test)]

extern crate protobuf;
extern crate test;

use std::io;
use std::io::Read;

use protobuf::CodedInputStream;

use self::test::Bencher;

fn make_bytes(len: usize) -> Vec<u8> {
    let mut r = Vec::with_capacity(len);
    for i in 0..len {
        r.push((i % 10) as u8);
    }
    test::black_box(r)
}

#[bench]
fn read_byte(b: &mut Bencher) {
    let v = make_bytes(1_000);
    b.iter(|| {
        let mut is = CodedInputStream::from_bytes(test::black_box(&v));
        while !is.eof().expect("eof") {
            test::black_box(is.read_raw_byte().expect("read"));
        }
    });
}

#[bench]
fn read_byte_no_eof(b: &mut Bencher) {
    let v = make_bytes(1_000);
    b.iter(|| {
        let mut is = CodedInputStream::from_bytes(test::black_box(&v));
        for _ in 0..v.len() {
            test::black_box(is.read_raw_byte().expect("read"));
        }
        assert!(is.eof().expect("eof"));
    });
}

#[bench]
fn read_byte_from_vec(b: &mut Bencher) {
    let v = make_bytes(1_000);
    b.iter(|| {
        let mut v = io::Cursor::new(test::black_box(&v));
        loop {
            let mut buf = [0];
            let count = v.read(&mut buf).expect("read");
            if count == 0 {
                break;
            }
            test::black_box(buf);
        }
    });
}

#[bench]
fn read_varint_12(b: &mut Bencher) {
    let mut v = Vec::new();
    {
        let mut v = protobuf::CodedOutputStream::vec(&mut v);
        for i in 0..1000 {
            // one or two byte varints
            v.write_raw_varint32((i * 7919) % (1 << 14)).expect("write");
        }
        v.flush().expect("flush");
    }
    b.iter(|| {
        let mut is = CodedInputStream::from_bytes(test::black_box(&v));
        let mut count = 0;
        while !is.eof().expect("eof") {
            test::black_box(is.read_raw_varint32().expect("read"));
            count += 1;
        }
        assert_eq!(1000, count);
    })
}

#[bench]
fn read_varint_1(b: &mut Bencher) {
    let mut v = Vec::new();
    {
        let mut v = protobuf::CodedOutputStream::vec(&mut v);
        for i in 0..1000 {
            // one or two byte varints
            v.write_raw_varint32((i * 7919) % (1 << 7)).expect("write");
        }
        v.flush().expect("flush");
    }
    b.iter(|| {
        let mut is = CodedInputStream::from_bytes(test::black_box(&v));
        let mut count = 0;
        while !is.eof().expect("eof") {
            test::black_box(is.read_raw_varint32().expect("read"));
            count += 1;
        }
        assert_eq!(1000, count);
    })
}
