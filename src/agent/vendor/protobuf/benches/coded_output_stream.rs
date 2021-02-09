// `cargo test --benches` and `#[feature(test)]` work only in nightly
#![cfg(rustc_nightly)]
#![feature(test)]

extern crate protobuf;
extern crate test;

use protobuf::stream;

use self::test::Bencher;

#[inline]
fn buffer_write_byte(os: &mut stream::CodedOutputStream) {
    for i in 0..10 {
        os.write_raw_byte(test::black_box(i as u8)).unwrap();
    }
    os.flush().unwrap();
}

#[inline]
fn buffer_write_bytes(os: &mut stream::CodedOutputStream) {
    for _ in 0..10 {
        os.write_raw_bytes(test::black_box(b"1234567890")).unwrap();
    }
    os.flush().unwrap();
}

#[bench]
fn bench_buffer(b: &mut Bencher) {
    b.iter(|| {
        let mut v = Vec::new();
        {
            let mut os = stream::CodedOutputStream::new(&mut v);
            buffer_write_byte(&mut os);
        }
        v
    });
}

#[bench]
fn bench_buffer_bytes(b: &mut Bencher) {
    b.iter(|| {
        let mut v = Vec::new();
        {
            let mut os = stream::CodedOutputStream::new(&mut v);
            buffer_write_bytes(&mut os);
        }
        v
    });
}
