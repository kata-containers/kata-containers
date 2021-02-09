#![feature(test)]

extern crate test;
extern crate itertools;

use test::{black_box, Bencher};
use itertools::Itertools;

// approximate 100_000 iterations for each combination
const N1: usize = 100_000;
const N2: usize = 448;
const N3: usize = 86;
const N4: usize = 41;

#[bench]
fn comb_for1(b: &mut Bencher) {
    b.iter(|| {
        for i in 0..N1 {
            black_box(i);
        }
    });
}

#[bench]
fn comb_for2(b: &mut Bencher) {
    b.iter(|| {
        for i in 0..N2 {
            for j in (i + 1)..N2 {
                black_box(i + j);
            }
        }
    });
}

#[bench]
fn comb_for3(b: &mut Bencher) {
    b.iter(|| {
        for i in 0..N3 {
            for j in (i + 1)..N3 {
                for k in (j + 1)..N3 {
                    black_box(i + j + k);
                }
            }
        }
    });
}

#[bench]
fn comb_for4(b: &mut Bencher) {
    b.iter(|| {
        for i in 0..N4 {
            for j in (i + 1)..N4 {
                for k in (j + 1)..N4 {
                    for l in (k + 1)..N4 {
                        black_box(i + j + k + l);
                    }
                }
            }
        }
    });
}

#[bench]
fn comb_c1(b: &mut Bencher) {
    b.iter(|| {
        for (i,) in (0..N1).tuple_combinations() {
            black_box(i);
        }
    });
}

#[bench]
fn comb_c2(b: &mut Bencher) {
    b.iter(|| {
        for (i, j) in (0..N2).tuple_combinations() {
            black_box(i + j);
        }
    });
}

#[bench]
fn comb_c3(b: &mut Bencher) {
    b.iter(|| {
        for (i, j, k) in (0..N3).tuple_combinations() {
            black_box(i + j + k);
        }
    });
}

#[bench]
fn comb_c4(b: &mut Bencher) {
    b.iter(|| {
        for (i, j, k, l) in (0..N4).tuple_combinations() {
            black_box(i + j + k + l);
        }
    });
}
