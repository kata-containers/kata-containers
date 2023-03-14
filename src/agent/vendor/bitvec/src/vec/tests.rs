#![cfg(test)]

use crate::prelude::*;

use core::{
	borrow::{
		Borrow,
		BorrowMut,
	},
	cmp::Ordering,
	convert::TryInto,
	iter,
};

#[cfg(not(feature = "std"))]
use alloc::{
	format,
	vec,
	vec::Vec,
};

#[cfg(feature = "std")]
use std::panic::catch_unwind;

#[test]
fn from_vec() {
	let mut bv = BitVec::<Msb0, u8>::from_vec(vec![0, 1, 2, 3]);
	let bp_mut = bv.as_mut_bitslice() as *mut _;
	assert_eq!(bv.len(), 32);
	assert_eq!(bv.count_ones(), 4);

	let capacity = bv.capacity();
	let bits = bv.leak();
	assert_eq!(bits as *mut _, bp_mut);
	let (ptr, length) = (bits.as_mut_bitptr(), bits.len());
	let bv = unsafe { BitVec::from_raw_parts(ptr, length, capacity) };
	assert_eq!(bv.as_raw_slice(), &[0, 1, 2, 3]);
}

#[test]
fn push() {
	let mut bvm08 = BitVec::<Msb0, u8>::new();
	assert!(bvm08.is_empty());

	bvm08.push(false);
	assert_eq!(bvm08.len(), 1);
	assert!(!bvm08[0]);

	bvm08.push(true);
	assert_eq!(bvm08.len(), 2);
	assert!(bvm08[1]);

	bvm08.extend(&[true; 3]);
	bvm08.extend(&[false; 3]);
	assert_eq!(bvm08, bits![0, 1, 1, 1, 1, 0, 0, 0]);
}

#[test]
fn check_buffers() {
	let mut bv = bitvec![LocalBits, u16; 0; 40];
	assert_eq!(bv.elements(), 3);

	assert_eq!(bv.as_raw_ptr(), bv.as_raw_slice().as_ptr());
	assert_eq!(bv.as_mut_raw_ptr(), bv.as_mut_raw_slice().as_mut_ptr());
}

#[test]
fn buffer_control() {
	let data = 0xA5u8;
	let bits = data.view_bits::<Msb0>();

	let mut bv = bits[2 ..].to_bitvec();
	assert_eq!(bv.as_raw_slice(), &[0xA5u8]);
	bv.force_align();
	assert_eq!(bv.as_raw_slice(), &[0b1001_0101]);
	bv.force_align();
	assert_eq!(bv.as_raw_slice(), &[0b1001_0101]);

	bv.truncate(6);
	bv.set_uninitialized(false);
	assert_eq!(bv.as_raw_slice(), &[0b1001_0100]);
	bv.set_uninitialized(true);
	assert_eq!(bv.as_raw_slice(), &[0b1001_0111]);
	assert_eq!(bv, bits![1, 0, 0, 1, 0, 1]);
}

#[test]
#[cfg(not(target_arch = "riscv64"))]
#[should_panic(expected = "Vector capacity exceeded")]
fn overcommit() {
	BitVec::<LocalBits, usize>::with_capacity(
		BitSlice::<LocalBits, usize>::MAX_BITS + 1,
	);
}

#[test]
#[cfg(feature = "std")]
fn reservations() {
	let mut bv = bitvec![1; 40];
	assert!(bv.capacity() >= 40);
	bv.reserve(100);
	assert!(bv.capacity() >= 140, "{}", bv.capacity());
	bv.shrink_to_fit();
	assert!(bv.capacity() >= 40);

	//  Trip the first assertion by wrapping around the `usize` boundary.
	assert!(
		catch_unwind(|| {
			let mut bv = bitvec![1; 100];
			bv.reserve(!0 - 50);
		})
		.is_err()
	);

	//  Trip the second assertion by exceeding `MAX_BITS` without wrapping.
	assert!(
		catch_unwind(|| {
			let mut bv = bitvec![1; 100];
			bv.reserve(BitSlice::<LocalBits, usize>::MAX_BITS - 50);
		})
		.is_err()
	);

	let mut bv = bitvec![1; 40];
	assert!(bv.capacity() >= 40);
	bv.reserve_exact(100);
	assert!(bv.capacity() >= 140, "{}", bv.capacity());

	//  Trip the first assertion by wrapping around the `usize` boundary.
	assert!(
		catch_unwind(|| {
			let mut bv = bitvec![1; 100];
			bv.reserve_exact(!0 - 50);
		})
		.is_err()
	);

	//  Trip the second assertion by exceeding `MAX_BITS` without wrapping.
	assert!(
		catch_unwind(|| {
			let mut bv = bitvec![1; 100];
			bv.reserve_exact(BitSlice::<LocalBits, usize>::MAX_BITS - 50);
		})
		.is_err()
	);
}

#[test]
#[allow(deprecated)]
fn iterators() {
	let bv: BitVec<Msb0, u8> = [0xC3, 0x96].iter().collect();
	assert_eq!(bv.count_ones(), 8);
	assert_eq!(bv, bits![1, 1, 0, 0, 0, 0, 1, 1, 1, 0, 0, 1, 0, 1, 1, 0]);

	let data = 0x35u8.view_bits::<Msb0>();
	let bv: BitVec<Msb0, u8> = data.iter().collect();
	assert_eq!(bv.count_ones(), 4);

	for (l, r) in (&bv).into_iter().zip(bits![0, 0, 1, 1, 0, 1, 0, 1]) {
		assert_eq!(*l, *r);
	}

	let mut bv = bv;
	*(&mut bv).into_iter().next().unwrap() = true;

	let mut iter = bv.into_iter();
	assert!(iter.next().unwrap());
	assert_eq!(iter.as_bitslice(), data[1 ..]);
	assert_eq!(iter.as_mut_bitslice(), data[1 ..]);

	assert_eq!(iter.size_hint(), (7, Some(7)));
	assert_eq!(bitvec![0; 10].into_iter().count(), 10);
	assert!(bitvec![0, 0, 1, 0].into_iter().nth(2).unwrap());
	assert!(bitvec![0, 1].into_iter().last().unwrap());
	assert!(bitvec![0, 0, 1].into_iter().next_back().unwrap());
	assert!(bitvec![0, 1, 0, 0].into_iter().nth_back(2).unwrap());

	let mut bv = bitvec![0, 0, 0, 1, 1, 1, 0, 0, 0];
	let mut drain = bv.drain(3 .. 6);
	let mid = bits![1; 3];
	assert_eq!(drain.as_bitslice(), mid);
	let drain_span: &BitSlice = drain.as_ref();
	assert_eq!(drain_span, mid);

	assert!(drain.nth(1).unwrap());
	assert!(drain.last().unwrap());
	assert_eq!(bitvec![0, 0, 1, 1, 0, 0,].drain(2 .. 4).count(), 2);

	let mut bv = bitvec![0, 0, 1, 0, 1, 1, 0, 1, 0, 0];
	let mut splice = bv.splice(2 .. 8, iter::repeat(false).take(4));
	assert!(splice.next().unwrap());
	assert!(splice.next_back().unwrap());
	assert!(splice.nth(1).unwrap());
	assert!(splice.nth_back(1).unwrap());
	drop(splice);
	assert_eq!(bv, bits![0; 8]);

	let mut bv = bitvec![0, 1, 1, 1, 1, 0];
	let splice = bv.splice(1 .. 5, Some(false));
	assert_eq!(splice.count(), 4);
	assert_eq!(bv, bits![0; 3]);

	//  Attempt to hit branches in the Splice destructor.

	let mut bv = bitvec![0, 0, 0, 1, 1, 1];
	drop(bv.splice(3 .., Some(false)));
	assert_eq!(bv, bits![0; 4]);

	let mut bv = bitvec![0, 0, 0, 1, 1, 1];
	let mut splice = bv.splice(3 .., [false; 2].iter().copied());
	assert!(splice.next().unwrap());
	assert!(splice.last().unwrap());
	assert_eq!(bv, bits![0; 5]);
}

#[test]
fn misc() {
	let mut bv = bitvec![1; 10];
	bv.truncate(20);
	assert_eq!(bv, bits![1; 10]);
	bv.truncate(5);
	assert_eq!(bv, bits![1; 5]);

	let mut bv = bitvec![0, 0, 1, 0, 0];
	assert!(bv.swap_remove(2));
	assert!(bv.not_any());

	bv.insert(2, true);
	assert_eq!(bv, bits![0, 0, 1, 0, 0]);

	bv.remove(2);
	assert!(bv.not_any());

	let mut bv = bitvec![0, 0, 1, 1, 0, 1, 0, 1, 0, 0];
	bv.retain(|idx, bit| !(idx & 1 == 0 && *bit));
	//                                         ^^^ even ^^^    prime
	assert_eq!(bv, bits![0, 0, 1, 0, 1, 0, 1, 0, 0]);
	//                        ^ 2 is the only even prime

	let mut bv_1 = bitvec![Lsb0, u8; 0; 5];
	let mut bv_2 = bitvec![Msb0, u16; 1; 5];
	let mut bv_3 = bv_1.clone();
	bv_1.append(&mut bv_2);

	assert_eq!(bv_1, bits![0, 0, 0, 0, 0, 1, 1, 1, 1, 1]);
	assert!(bv_2.is_empty());

	bv_1.append(&mut bv_3);
	assert_eq!(bv_1, bits![0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0]);

	let bv_4 = bv_1.split_off(5);
	assert!(bv_1.not_any());
	assert!(bv_4.some());

	let mut last = false;
	bv_1.resize_with(10, || {
		last = !last;
		last
	});
	assert_eq!(bv_1, bits![0, 0, 0, 0, 0, 1, 0, 1, 0, 1]);
}

#[test]
fn cloning() {
	let mut a = bitvec![0];
	let b = bitvec![1; 20];

	assert_ne!(a, b);
	a.clone_from(&b);
	assert_eq!(a, b);
}

#[test]
fn vec_splice() {
	let mut bv = bitvec![0, 1, 0];
	let new = bits![1, 0];
	let old: BitVec = bv.splice(.. 2, new.iter().by_val()).collect();
	assert_eq!(bv, bits![1, 0, 0]);
	assert_eq!(old, bits![0, 1]);

	let mut bv = bitvec![0, 1, 0];
	let new = bits![1, 1, 0, 0, 1, 1];
	let old: BitVec = bv.splice(1 .. 2, new.iter().by_val()).collect();
	assert_eq!(bv, bits![0, 1, 1, 0, 0, 1, 1, 0]);
	assert_eq!(old, bits![1]);
}

#[test]
fn ops() {
	let a = bitvec![0, 0, 1, 1];
	let b = bitvec![0, 1, 0, 1];

	let c = a.clone() & b.clone();
	assert_eq!(c, bits![0, 0, 0, 1]);
	let d = a.clone() | b.clone();
	assert_eq!(d, bits![0, 1, 1, 1]);
	let e = a.clone() ^ b.clone();
	assert_eq!(e, bits![0, 1, 1, 0]);
	let f = !e;
	assert_eq!(f, bits![1, 0, 0, 1]);
}

#[test]
fn traits() {
	let mut bv = bitvec![0, 0, 1, 1];
	let bits: &BitSlice = bv.borrow();
	assert_eq!(bv, bits);
	let bits: &mut BitSlice = bv.borrow_mut();
	assert_eq!(bits, bits![0, 0, 1, 1]);
	assert!(bv.as_bitslice().eq(&bv));

	let bv2 = bitvec![0, 1, 0, 1];
	assert_eq!(bv.cmp(&bv2), Ordering::Less);
	assert!(!bv.eq(&bv2));
	assert_eq!((&bv.as_bitslice()).partial_cmp(&bv2), Some(Ordering::Less));

	let _: &BitSlice = bv.as_ref();
	let _: &mut BitSlice = bv.as_mut();

	let bv: BitVec = bits![mut 0, 1, 0, 1].into();
	assert_eq!(bv, bits![0, 1, 0, 1]);
	let bv: BitVec = bitbox![0, 1, 0, 1].into();
	assert_eq!(bv, bits![0, 1, 0, 1]);
	let vec: Vec<usize> = bv.into();
	assert_eq!(vec.len(), 1);
	let bv: Result<BitVec, Vec<usize>> = vec.try_into();
	assert!(bv.is_ok());
}

#[test]
fn format() {
	let bv = bitvec![0, 0, 1, 1, 0, 1, 0, 1];
	assert_eq!(format!("{}", bv), format!("{}", bv.as_bitslice()));
	assert_eq!(format!("{:b}", bv), format!("{:b}", bv.as_bitslice()));
	assert_eq!(format!("{:o}", bv), format!("{:o}", bv.as_bitslice()));
	assert_eq!(format!("{:x}", bv), format!("{:x}", bv.as_bitslice()));
	assert_eq!(format!("{:X}", bv), format!("{:X}", bv.as_bitslice()));

	let text = format!("{:?}", bitvec![Msb0, u8; 0, 1, 0, 0]);
	assert!(
		text.starts_with("BitVec<bitvec::order::Msb0, u8> { addr: 0x"),
		"{}",
		text
	);
	assert!(
		text.contains(", head: 000, bits: 4, capacity: "),
		"{}",
		text
	);
	assert!(text.ends_with(" } [0100]"), "{}", text);
}
