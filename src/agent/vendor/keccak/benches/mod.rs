#![feature(test)]
#![cfg_attr(feature = "simd", feature(portable_simd))]

extern crate keccak;
extern crate test;

use keccak::{f1600, f200, f400, f800};

macro_rules! impl_bench {
    ($name:ident, $fn:ident, $type:expr) => {
        #[bench]
        fn $name(b: &mut test::Bencher) {
            let mut data = [$type; 25];
            b.iter(|| $fn(&mut data));
        }
    };
}

impl_bench!(b_f200, f200, 0u8);
impl_bench!(b_f400, f400, 0u16);
impl_bench!(b_f800, f800, 0u32);
impl_bench!(b_f1600, f1600, 0u64);

#[cfg(feature = "simd")]
mod simd {
    use keccak::simd::{f1600x2, f1600x4, f1600x8, u64x2, u64x4, u64x8};

    impl_bench!(b_f1600x2, f1600x2, u64x2::splat(0));
    impl_bench!(b_f1600x4, f1600x4, u64x4::splat(0));
    impl_bench!(b_f1600x8, f1600x8, u64x8::splat(0));
}
