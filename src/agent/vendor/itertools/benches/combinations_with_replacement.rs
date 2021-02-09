#![feature(test)]

extern crate itertools;
extern crate test;

use itertools::Itertools;
use test::{black_box, Bencher};

#[bench]
fn comb_replacement_n10_k5(b: &mut Bencher) {
    b.iter(|| {
        for i in (0..10).combinations_with_replacement(5) {
            black_box(i);
        }
    });
}

#[bench]
fn comb_replacement_n5_k10(b: &mut Bencher) {
    b.iter(|| {
        for i in (0..5).combinations_with_replacement(10) {
            black_box(i);
        }
    });
}

#[bench]
fn comb_replacement_n10_k10(b: &mut Bencher) {
    b.iter(|| {
        for i in (0..10).combinations_with_replacement(10) {
            black_box(i);
        }
    });
}
