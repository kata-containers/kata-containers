#![feature(test)]

extern crate test;
use test::Bencher;

extern crate rusticata_macros;

use rusticata_macros::combinator::be_var_u64;

#[bench]
fn bench_bytes_to_u64(b: &mut Bencher) {
    let bytes = &[0x12, 0x34, 0x56, 0x78, 0x90, 0x12];
    b.iter(|| {
        let res = be_var_u64::<()>(bytes).unwrap();
        assert_eq!(res.1, 0x123456789012);
    });
}
