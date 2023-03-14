//! Unit tests for the `array` module.

#![cfg(test)]

use crate::prelude::*;

use tap::conv::TryConv;

#[test]
fn create_arrays() {
	macro_rules! make {
		($($elts:literal),+ $(,)?) => { $(
			let _ = BitArray::<LocalBits, [u8; $elts]>::zeroed();
			let _ = BitArray::<LocalBits, [u16; $elts]>::zeroed();
			let _ = BitArray::<LocalBits, [u32; $elts]>::zeroed();
			let _ = BitArray::<LocalBits, [usize; $elts]>::zeroed();

			#[cfg(target_pointer_width = "64")] {
			let _ = BitArray::<LocalBits, [u64; $elts]>::zeroed();
			}
		)+ };
	}

	make!(
		0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
		20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32
	);
}

#[test]
fn wrap_unwrap() {
	let data: [u8; 15] = *b"Saluton, mondo!";
	let bits = BitArray::<LocalBits, _>::new(data);
	assert_eq!(bits.value(), data);
}

#[test]
fn views() {
	let mut arr = bitarr![Msb0, u8; 0; 20];

	let s: &mut [u8] = arr.as_mut_raw_slice();
	s[0] = !0u8;
	let s: &[u8] = arr.as_raw_slice();
	assert_eq!(s, &[!0, 0, 0]);

	let a: &mut [u8; 3] = arr.as_mut_buffer();
	*a = [!0u8; 3];
	let a: &[u8; 3] = arr.as_buffer();
	assert_eq!(*a, [!0u8; 3]);
}

#[test]
fn convert() {
	let arr: BitArray<Lsb0, _> = 2u8.into();
	assert!(arr.any());

	let bits = bits![Msb0, u8; 1; 128];
	let arr = bits.try_conv::<BitArray<Msb0, [u8; 16]>>().unwrap();
	assert!(arr.all());

	let bits = bits![Lsb0, u32; 0; 64];
	let arr = bits.try_conv::<&BitArray<Lsb0, [u32; 2]>>().unwrap();
	assert!(arr.not_any());

	let bits = bits![mut Msb0, u16; 0; 64];
	let arr = bits.try_conv::<&mut BitArray<Msb0, [u16; 4]>>().unwrap();
	assert!(arr.not_any());

	let bits = bits![mut LocalBits, usize; 0; 4];
	assert!((&*bits).try_conv::<&BitArray<LocalBits, usize>>().is_err());
	assert!(bits.try_conv::<&mut BitArray<LocalBits, usize>>().is_err());
}

#[test]
#[allow(deprecated)]
fn iter() {
	let mut iter = bitarr![0, 0, 0, 1, 1, 1, 0, 0, 0].into_iter();
	let width = <[usize; 1] as BitView>::const_bits();

	let slice = iter.as_slice();
	let iter_slice = iter.as_bitslice();
	assert_eq!(slice, iter_slice);
	assert_eq!(iter_slice.count_ones(), 3);
	assert_eq!(iter_slice.len(), width);

	iter.as_mut_bitslice().set(0, true);
	iter.as_mut_slice().set(1, true);
	assert_eq!(iter.as_bitslice().count_ones(), 5);

	assert!(iter.next().unwrap());
	//  get the last bit of the literal
	assert!(!iter.nth_back(width - 9).unwrap());
	//  advance to the first `0` bit after the `1`s in the literal
	assert!(!iter.nth_back(1).unwrap());
	assert!(!iter.nth(1).unwrap());
	assert_eq!(iter.as_bitslice(), bits![1; 3]);

	assert!(iter.next().unwrap());
	assert!(iter.clone().last().unwrap());
	assert_eq!(iter.size_hint(), (2, Some(2)));
	assert_eq!(iter.clone().count(), 2);

	//  Reference iterators

	assert!((&bitarr![0]).into_iter().all(|b| !*b));
	assert!((&bitarr![1]).into_iter().any(|b| *b));

	let mut arr = bitarr![0];
	assert!(arr.not_any());
	for mut bit in &mut arr {
		*bit = !*bit;
	}
	assert!(arr.all(), "{:?}", arr);
}

#[test]
fn ops() {
	let a = bitarr![0, 0, 1, 1];
	let b = bitarr![0, 1, 0, 1];

	let c = a & b;
	assert_eq!(c, bitarr![0, 0, 0, 1]);

	let d = a | b;
	assert_eq!(d, bitarr![0, 1, 1, 1]);

	let e = a ^ b;
	assert_eq!(e, bitarr![0, 1, 1, 0]);

	let mut f = !e;
	//  Array literals are zero-extended to fill their `V` type, and do not
	//  store a length counter.
	assert_eq!(f[.. 4], bitarr![1, 0, 0, 1][.. 4]);

	let _: &BitSlice = &*a;
	let _: &mut BitSlice = &mut *f;
}

#[test]
fn traits() {
	use core::{
		borrow::{
			Borrow,
			BorrowMut,
		},
		cmp::Ordering,
	};

	let mut a = bitarr![0, 1, 0, 1];
	let bitspan = a.as_bitslice().as_bitspan();

	let bits: &BitSlice = a.borrow();
	assert_eq!(bits.as_bitspan(), bitspan);
	let bits_mut: &mut BitSlice = a.borrow_mut();
	assert_eq!(bits_mut.as_bitspan(), bitspan);

	let bits: &BitSlice = a.as_ref();
	assert_eq!(bits.as_bitspan(), bitspan);
	let bits_mut: &mut BitSlice = a.as_mut();
	assert_eq!(bits_mut.as_bitspan(), bitspan);

	let a = bitarr![0, 1];
	let b = bitarr![0, 0];
	assert!(a > b);
	assert_eq!(a.cmp(&b), Ordering::Greater);

	let a: BitArray = BitArray::default();
	assert!(a.not_any());
}

#[test]
#[cfg(feature = "alloc")]
fn format() {
	#[cfg(not(feature = "std"))]
	use alloc::format;

	let a = bitarr![0; 20];

	assert_eq!(format!("{}", a), format!("{}", a.as_bitslice()));
	assert_eq!(format!("{:b}", a), format!("{:b}", a.as_bitslice()));
	assert_eq!(format!("{:o}", a), format!("{:o}", a.as_bitslice()));
	assert_eq!(format!("{:x}", a), format!("{:x}", a.as_bitslice()));
	assert_eq!(format!("{:X}", a), format!("{:X}", a.as_bitslice()));

	let text = format!("{:?}", bitarr![Msb0, u8; 0, 1, 0, 0]);
	assert!(
		text.starts_with("BitArray<bitvec::order::Msb0, u8> { addr: 0x"),
		"{}",
		text
	);
	assert!(
		text.ends_with(", head: 000, bits: 8 } [01000000]"),
		"{}",
		text
	);
}
