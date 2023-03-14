//! Unit tests for the `boxed` module.

use crate::prelude::*;

use core::convert::TryInto;

#[cfg(not(feature = "std"))]
use alloc::{
	boxed::Box,
	format,
};

#[test]
#[allow(deprecated)]
fn api() {
	let boxed: Box<[u8]> = Box::new([0; 4]);
	let bb = BitBox::<LocalBits, _>::from_boxed_slice(boxed);
	assert_eq!(bb, bits![0; 32]);
	let boxed = bb.into_boxed_slice();
	assert_eq!(boxed[..], [0u8; 4][..]);

	let pinned = BitBox::pin(bits![0, 1, 0, 1]);
	let unpinned = BitBox::new(bits![0, 1, 0, 1]);
	assert_eq!(pinned.as_ref().get_ref(), unpinned[..]);

	let boxed = bitbox![0; 10];
	let bitspan = boxed.as_bitspan();
	let reboxed = unsafe { BitBox::from_raw(BitBox::into_raw(boxed)) };
	#[allow(deprecated)]
	{
		let _: BitVec = reboxed.clone().into_vec();
	}
	let bv = reboxed.into_bitvec();
	let bb = bv.into_boxed_bitslice();
	assert_eq!(bb.as_bitspan(), bitspan);

	let mut bb = 0b1001_0110u8.view_bits::<Msb0>()[2 .. 6]
		.to_bitvec()
		.into_boxed_bitslice();
	bb.set_uninitialized(false);
	assert_eq!(bb.as_slice(), &[0b0001_0100]);
	bb.set_uninitialized(true);
	assert_eq!(bb.as_slice(), &[0b1101_0111]);
	assert_eq!(bb, bits![0, 1, 0, 1]);
}

#[test]
fn ops() {
	let a = bitbox![0, 0, 1, 1];
	let b = bitbox![0, 1, 0, 1];

	let c = a.clone() & b.clone();
	assert_eq!(c, bitbox![0, 0, 0, 1]);

	let d = a.clone() | b.clone();
	assert_eq!(d, bitbox![0, 1, 1, 1]);

	let e = a.clone() ^ b.clone();
	assert_eq!(e, bitbox![0, 1, 1, 0]);

	let mut f = !e;
	assert_eq!(f, bitbox![1, 0, 0, 1]);

	let _: &BitSlice = &*a;
	let _: &mut BitSlice = &mut *f;

	let mut g = a.clone();
	assert!(g[.. 2].not_any());
	g[.. 2].set_all(true);
	assert!(g[.. 2].all());
}

#[test]
fn convert() {
	let boxed: BitBox = bits![1; 64].into();
	assert!(boxed.all());

	let boxed: BitBox<Lsb0, u32> = bitvec![Lsb0, u32; 0; 64].into();
	assert!(boxed.not_any());
	let boxed: Box<[u32]> = boxed.into();
	assert_eq!(&boxed[..], &[0; 2]);

	let _: BitBox<Lsb0, u32> = boxed.try_into().unwrap();
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

	let mut b = bitbox![0, 1, 0, 0];
	let bitspan = b.as_bitslice().as_bitspan();

	let bits: &BitSlice = b.borrow();
	assert_eq!(bits.as_bitspan(), bitspan);
	let bits_mut: &mut BitSlice = b.borrow_mut();
	assert_eq!(bits_mut.as_bitspan(), bitspan);

	let bits: &BitSlice = b.as_ref();
	assert_eq!(bits.as_bitspan(), bitspan);
	let bits_mut: &mut BitSlice = b.as_mut();
	assert_eq!(bits_mut.as_bitspan(), bitspan);

	let b1 = bitbox![0, 1];
	let b2 = bitbox![0, 0];
	assert!(b1 > b2);
	assert_eq!(b1.cmp(&b2), Ordering::Greater);
	assert_ne!(b1.as_bitslice(), b2);

	let b1_ref: &BitSlice = &*b1;
	assert_eq!((&b1_ref).partial_cmp(&b2), Some(Ordering::Greater));
	assert!(b1_ref.eq(&b1));

	let b: BitBox = BitBox::default();
	assert!(b.is_empty());
}

#[test]
#[cfg(feature = "alloc")]
fn format() {
	let b = bitbox![0; 20];

	assert_eq!(format!("{}", b), format!("{}", b.as_bitslice()));
	assert_eq!(format!("{:b}", b), format!("{:b}", b.as_bitslice()));
	assert_eq!(format!("{:o}", b), format!("{:o}", b.as_bitslice()));
	assert_eq!(format!("{:x}", b), format!("{:x}", b.as_bitslice()));
	assert_eq!(format!("{:X}", b), format!("{:X}", b.as_bitslice()));

	let text = format!("{:?}", bitbox![Msb0, u8; 0, 1, 0, 0]);
	assert!(
		text.starts_with("BitBox<bitvec::order::Msb0, u8> { addr: 0x"),
		"{}",
		text
	);
	assert!(text.ends_with(", head: 000, bits: 4 } [0100]"), "{}", text);
}
