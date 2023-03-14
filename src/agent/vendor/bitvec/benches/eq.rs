#![feature(test)]

extern crate test;

use bitvec::prelude::*;
use test::Bencher;

#[bench]
fn bitwise_eq(bench: &mut Bencher) {
	let a = bitarr![0; 500];
	let b = bitarr![0; 500];

	bench.iter(|| {
		a.iter()
			.by_val()
			.zip(b.iter().by_val())
			.all(|(a, b)| a == b)
	});
}

#[bench]
fn lsb0_accel_eq(bench: &mut Bencher) {
	let a = bitarr![0; 500];
	let b = bitarr![0; 500];

	bench.iter(|| a == b);
}

#[bench]
fn msb0_accel_eq(bench: &mut Bencher) {
	let a = bitarr![Msb0, usize; 0; 500];
	let b = bitarr![Msb0, usize; 0; 500];

	bench.iter(|| a == b);
}
