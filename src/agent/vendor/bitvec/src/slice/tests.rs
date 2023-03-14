//! Unit tests for the `slice` module.

#![cfg(test)]

use crate::prelude::*;

use tap::conv::TryConv;

#[test]
fn construction() {
	#[cfg(not(miri))]
	use core::slice;

	let data = 0u8;
	let bits = data.view_bits::<LocalBits>();
	assert_eq!(bits.len(), 8);

	#[cfg(not(miri))]
	assert!(
		BitSlice::<LocalBits, u8>::from_slice(unsafe {
			slice::from_raw_parts(
				1usize as *const _,
				BitSlice::<LocalBits, u8>::MAX_ELTS,
			)
		})
		.is_err()
	);

	#[cfg(not(miri))]
	assert!(
		BitSlice::<LocalBits, u8>::from_slice_mut(unsafe {
			slice::from_raw_parts_mut(
				1usize as *mut _,
				BitSlice::<LocalBits, u8>::MAX_ELTS,
			)
		})
		.is_err()
	);

	let mut data = [0u16; 2];
	assert!((&data[..]).try_conv::<&BitSlice<Msb0, _>>().is_ok());
	assert!((&mut data[..]).try_conv::<&mut BitSlice<Msb0, _>>().is_ok());
}

#[test]
fn cmp() {
	let data = 0x45u8;
	let bits = data.view_bits::<Msb0>();
	let a = &bits[.. 3]; // 010
	let b = &bits[.. 4]; // 0100
	let c = &bits[.. 5]; // 01000
	let d = &bits[4 ..]; // 0101

	assert!(a < b); // by length
	assert!(b < c); // by length
	assert!(c < d); // by different bit

	let bits = bits![Msb0, u8;
		1, 0, 1, 1, 0, 0,
		1, 0, 1, 1, 0, 0,
	];
	let (l, r) = bits.split_at(6);
	assert_eq!(l, r, "{:b} {:b}", l.load::<u8>(), r.load::<u8>());
	let bits = bits![Lsb0, u8;
		1, 1, 0, 0, 1, 0,
		1, 1, 0, 0, 1, 0,
	];
	let (l, r) = bits.split_at(6);
	assert_eq!(l, r);
}

#[test]
fn get_set() {
	let bits = bits![mut LocalBits, u8; 0; 8];

	for n in 0 .. 8 {
		assert!(!bits.get(n).unwrap());
		bits.set(n, true);
		assert!(bits.get(n).unwrap());
	}

	assert!(bits.get(9).is_none());
	assert!(bits.get_mut(9).is_none());
	assert!(bits.get(8 .. 10).is_none());
	assert!(bits.get_mut(8 .. 10).is_none());

	assert_eq!(bits.first().as_deref(), Some(&true));
	*bits.first_mut().unwrap() = false;
	assert_eq!(bits.last().as_deref(), Some(&true));
	*bits.last_mut().unwrap() = false;

	*crate::slice::BitSliceIndex::index_mut(1usize, bits) = false;
	assert_eq!(bits, bits![0, 0, 1, 1, 1, 1, 1, 0]);
	assert!(bits.get(100 ..).is_none());
	assert!(bits.get(.. 100).is_none());

	let (a, b) = (bits![mut Msb0, u8; 0, 1], bits![mut Lsb0, u16; 1, 0]);
	assert_eq!(a, bits![0, 1]);
	assert_eq!(b, bits![1, 0]);
	a.swap_with_bitslice(b);
	assert_eq!(a, bits![1, 0]);
	assert_eq!(b, bits![0, 1]);

	let proxy = a.get_mut(0).unwrap();
	assert!(*proxy);

	let byte = core::cell::Cell::new(0u8);
	let shared = byte.view_bits::<Lsb0>();
	let shared_2 = shared;

	shared.set_aliased(0, true);
	assert!(shared_2[0]);
}

#[test]
#[should_panic = "Index 1 out of bounds: 1"]
fn index_out_of_bounds() {
	bits![0][1];
}

#[test]
fn memcpy() {
	let mut dst = bitarr![0; 500];
	let src = bitarr![1; 500];

	//  Equal heads will fall into the fast path.
	dst[10 .. 20].copy_from_bitslice(&src[74 .. 84]);
	dst[100 .. 500].copy_from_bitslice(&src[36 .. 436]);

	//  Unequal heads will trip the slow path.
	dst[.. 490].copy_from_bitslice(&src[10 .. 500]);
}

#[test]
fn batch_copy() {
	let mut l = bitarr![Lsb0, usize; 0; 500];
	let mut m = bitarr![Msb0, usize; 0; 500];

	let l2 = bitarr![Lsb0, usize; 1; 500];
	let m2 = bitarr![Msb0, usize; 1; 500];

	assert!(l.not_any());
	l.copy_from_bitslice(&l2);
	assert!(l.all());

	assert!(m.not_any());
	m.copy_from_bitslice(&m2);
	assert!(m.all());
}

#[test]
fn query() {
	let data = [0x0Fu8, !0, 0xF0, 0, 0x0E];
	let bits = data.view_bits::<Msb0>();

	assert!(bits[36 .. 39].all());
	assert!(bits[4 .. 20].all());
	assert!(bits[.. 8].any());
	assert!(bits[4 .. 20].any());
	assert!(bits[32 ..].not_all());
	assert!(bits[.. 4].not_any());
	assert!(bits[.. 8].some());

	assert_eq!(bits[1 .. 7].count_ones(), 3);
	assert_eq!(bits[1 .. 7].count_zeros(), 3);
	assert_eq!(bits[.. 24].count_ones(), 16);
	assert_eq!(bits[16 ..].count_zeros(), 17);

	assert!(!bits![0].contains(bits![0, 1]));
	assert!(bits![0, 1, 0].contains(bits![1, 0]));
	assert!(bits![0, 1, 0].starts_with(bits![0, 1]));
	assert!(bits![0, 1, 0].ends_with(bits![1, 0]));
}

#[test]
fn modify() {
	let mut data = 0b0000_1111u8;

	let bits = data.view_bits_mut::<LocalBits>();
	bits.swap(3, 4);
	assert_eq!(data, 0b0001_0111);

	let bits = data.view_bits_mut::<Lsb0>();
	bits[1 .. 7].reverse();
	assert_eq!(data, 0b0110_1001);
	data.view_bits_mut::<Msb0>()[1 .. 7].reverse();

	let bits = data.view_bits_mut::<Msb0>();
	bits.copy_within(2 .. 4, 0);
	assert_eq!(data, 0b0101_0111);

	let bits = data.view_bits_mut::<Msb0>();
	bits.copy_within(5 .., 2);
	assert_eq!(data, 0b0111_1111);
}

#[test]
fn split() {
	assert!(
		BitSlice::<LocalBits, usize>::empty()
			.split_first()
			.is_none()
	);
	let (head, rest) = 1u8.view_bits::<Lsb0>().split_first().unwrap();
	assert!(head);
	assert_eq!(rest, bits![0; 7]);

	assert!(
		BitSlice::<LocalBits, usize>::empty_mut()
			.split_first_mut()
			.is_none()
	);
	let mut data = 0u8;
	let (head, _) = data.view_bits_mut::<Lsb0>().split_first_mut().unwrap();
	head.set(true);
	assert_eq!(data, 1);

	assert!(BitSlice::<LocalBits, usize>::empty().split_last().is_none());
	let (tail, rest) = 1u8.view_bits::<Msb0>().split_last().unwrap();
	assert!(tail);
	assert_eq!(rest, bits![0; 7]);

	assert!(
		BitSlice::<LocalBits, usize>::empty_mut()
			.split_first_mut()
			.is_none()
	);
	let mut data = 0u8;
	let (head, _) = data.view_bits_mut::<Msb0>().split_last_mut().unwrap();
	head.set(true);
	assert_eq!(data, 1);

	let mut data = 0b0000_1111u8;

	let bits = data.view_bits::<Msb0>();
	let (left, right) = bits.split_at(4);
	assert!(left.not_any());
	assert!(right.all());

	let bits = data.view_bits_mut::<Msb0>();
	let (left, right) = bits.split_at_mut(4);
	left.set_all(true);
	right.set_all(false);
	assert_eq!(data, 0b1111_0000u8);

	let data = 0u8;
	let bits = data.view_bits::<Lsb0>();
	let base_ptr = BitPtr::from_ref(&data);
	let next_ptr = base_ptr.wrapping_add(8);
	let (l, _) = bits.split_at(0);
	let (_, r) = bits.split_at(8);
	let (l_ptr, r_ptr) = (l.as_bitspan(), r.as_bitspan());
	assert_eq!(l_ptr.as_bitptr(), base_ptr);
	assert_eq!(r_ptr.as_bitptr(), next_ptr);
}

#[test]
fn iterators() {
	assert!(bits![0; 2].iter().nth(2).is_none());
	assert!(bits![0; 2].iter().nth_back(2).is_none());

	let bits = bits![mut 0; 4];

	assert!(bits.chunks(2).nth(2).is_none());
	assert!(bits.chunks(2).nth_back(2).is_none());
	assert!(bits.chunks_mut(2).nth(2).is_none());
	assert!(bits.chunks_mut(2).nth_back(2).is_none());

	assert!(bits.rchunks(2).nth(2).is_none());
	assert!(bits.rchunks(2).nth_back(2).is_none());
	assert!(bits.rchunks_mut(2).nth(2).is_none());
	assert!(bits.rchunks_mut(2).nth_back(2).is_none());
	assert!(bits![mut].rchunks_mut(1).next().is_none());

	bits![Msb0, u8; 0, 1, 0, 0, 1, 0, 0, 0]
		.split(|_, bit| *bit)
		.zip([1usize, 2, 3].iter())
		.for_each(|(bits, len)| assert_eq!(bits.len(), *len));

	let mut data = 0b0100_1000u8;
	data.view_bits_mut::<Msb0>()
		.split_mut(|_, bit| *bit)
		.zip([1usize, 2, 3].iter())
		.for_each(|(bits, len)| {
			assert_eq!(bits.len(), *len);
			bits.set_all(true)
		});
	assert_eq!(data, !0);

	bits![Msb0, u8; 0, 1, 0, 0, 1, 0, 0, 0]
		.rsplit(|_, bit| *bit)
		.zip([3usize, 2, 1].iter())
		.for_each(|(bits, len)| assert_eq!(bits.len(), *len));

	let mut data = 0b0100_1000u8;
	data.view_bits_mut::<Msb0>()
		.rsplit_mut(|_, bit| *bit)
		.zip([3usize, 2, 1].iter())
		.for_each(|(bits, len)| {
			assert_eq!(bits.len(), *len);
			bits.set_all(true)
		});
	assert_eq!(data, !0);

	bits![Msb0, u8; 0, 1, 0, 0, 1, 0, 0, 0]
		.splitn(2, |_, bit| *bit)
		.zip([1usize, 6].iter())
		.for_each(|(bits, len)| assert_eq!(bits.len(), *len));

	let mut data = 0b0100_1000u8;
	data.view_bits_mut::<Msb0>()
		.splitn_mut(2, |_, bit| *bit)
		.zip([1usize, 6].iter())
		.for_each(|(bits, len)| {
			assert_eq!(bits.len(), *len);
			bits.set_all(true)
		});
	assert_eq!(data, !0);

	bits![Msb0, u8; 0, 1, 0, 0, 1, 0, 0, 0]
		.rsplitn(2, |_, bit| *bit)
		.zip([3usize, 4].iter())
		.for_each(|(bits, len)| assert_eq!(bits.len(), *len));

	let mut data = 0b0100_1000u8;
	data.view_bits_mut::<Msb0>()
		.rsplitn_mut(2, |_, bit| *bit)
		.zip([3usize, 4].iter())
		.for_each(|(bits, len)| {
			assert_eq!(bits.len(), *len);
			bits.set_all(true)
		});
	assert_eq!(data, !0);
}

#[test]
fn alignment() {
	let mut data = [0u32; 3];
	let bits = data.view_bits_mut::<Msb0>();

	let bp = bits[10 .. 20].as_bitspan();
	let (l0, c0, r0) = unsafe { bits[10 .. 20].align_to_mut::<u8>() };
	assert_eq!(l0.as_bitspan(), bp);
	assert!(c0.is_empty());
	assert!(r0.is_empty());

	let (l1, c1, r1) = unsafe { bits[10 .. 86].align_to::<u8>() };
	assert_eq!(l1.len(), 22);
	assert_eq!(r1.len(), 22);
	assert_eq!(c1.len(), 32);

	let (l2, c2, r2) = unsafe { c1.align_to::<u16>() };
	assert!(l2.is_empty());
	assert!(r2.is_empty());
	assert_eq!(c1.len(), c2.len());
}

#[test]
#[cfg(feature = "alloc")]
fn repetition() {
	let bits = bits![0, 0, 1, 1];
	let bv = bits.repeat(2);
	assert_eq!(bv, bits![0, 0, 1, 1, 0, 0, 1, 1]);
}

#[test]
fn pointer_offset() {
	let data = [0u8; 2];
	let msb0 = data.view_bits::<Msb0>();

	assert_eq!(msb0[2 ..].offset_from(&msb0[12 ..]), 10);
	assert_eq!(msb0[5 ..].offset_from(&msb0[10 ..]), 5);
	assert_eq!(msb0[8 ..].offset_from(&msb0[4 ..]), -4);
	assert_eq!(msb0[14 ..].offset_from(&msb0[1 ..]), -13);
}

#[test]
fn shift() {
	let bits = bits![mut 1; 6];
	bits.shift_left(0);
	bits.shift_right(0);
	assert_eq!(bits, bits![1; 6]);

	bits.shift_left(4);
	assert_eq!(bits, bits![1, 1, 0, 0, 0, 0]);
	bits.shift_right(2);
	assert_eq!(bits, bits![0, 0, 1, 1, 0, 0]);
}

#[test]
fn invert() {
	let mut data = [0u8; 4];
	let bits = data.view_bits_mut::<Lsb0>();

	let inv = !&mut bits[2 .. 6];
	assert!(inv.all());

	let inv = !&mut bits[12 .. 28];
	assert!(inv.all());

	assert_eq!(data, [0x3C, 0xF0, 0xFF, 0x0F]);
}

#[test]
fn rotate() {
	let bits = bits![mut 0, 1, 0, 0, 1, 0];

	bits.rotate_left(0);
	bits.rotate_right(0);
	bits.rotate_left(6);
	bits.rotate_right(6);

	assert_eq!(bits, bits![0, 1, 0, 0, 1, 0]);
}

#[test]
fn unspecialized() {
	use crate::{
		index::{
			BitIdx,
			BitPos,
		},
		mem::BitRegister,
		prelude::*,
	};

	pub struct Swizzle;

	unsafe impl BitOrder for Swizzle {
		fn at<R>(index: BitIdx<R>) -> BitPos<R>
		where R: BitRegister {
			match R::BITS {
				8 => BitPos::new(index.value() ^ 0b100).unwrap(),
				16 => BitPos::new(index.value() ^ 0b1100).unwrap(),
				32 => BitPos::new(index.value() ^ 0b11100).unwrap(),
				64 => BitPos::new(index.value() ^ 0b111100).unwrap(),
				_ => unreachable!("No other integers are supported"),
			}
		}
	}

	let mut data = [!0u8, 0];
	let bits = data.view_bits_mut::<Swizzle>();

	bits.copy_within(4 .. 12, 8);
	bits.copy_within(.. 4, 12);
	assert!(bits.all());
	assert_eq!(bits[.. 8], bits[8 ..]);

	//  Dodge the memcpy accelerant
	bits[.. 8].copy_from_bitslice(&bits![Swizzle, u8; 0; 9][1 ..]);
	assert_eq!(bits, [0u8, !0].view_bits::<Swizzle>());
}

#[test]
#[allow(deprecated)]
fn misc() {
	let a = bits![mut 0; 4];
	let b = bits![mut 0, 1, 0, 1];
	let c = bits![mut 0, 0, 1, 1];

	a.clone_from_slice(b);
	assert_eq!(a, b);
	b.swap_with_slice(c);
	a.copy_from_slice(b);
	assert_eq!(a, b);

	#[cfg(feature = "alloc")]
	{
		assert_eq!(a.to_vec(), bitvec![0, 0, 1, 1]);
	}
}

#[test]
#[allow(deprecated)]
fn iter() {
	let bits = bits![Lsb0, u8; 0, 1, 1, 0, 1, 0, 0, 1];
	let mut iter = bits.iter();

	assert_eq!(iter.as_bitslice(), bits);
	assert_eq!(iter.as_slice(), bits);
	assert_eq!(iter.next().as_deref(), Some(&false));
	assert_eq!(iter.as_bitslice(), &bits[1 ..]);
	assert_eq!(iter.next().as_deref(), Some(&true));

	assert_eq!(iter.as_bitslice(), &bits[2 ..]);
	assert_eq!(iter.next_back().as_deref(), Some(&true));
	assert_eq!(iter.as_bitslice(), &bits[2 .. 7]);
	assert_eq!(iter.next_back().as_deref(), Some(&false));

	assert_eq!(iter.as_bitslice(), &bits[2 .. 6]);
	assert_eq!(iter.next().as_deref(), Some(&true));
	assert_eq!(iter.as_bitslice(), &bits[3 .. 6]);
	assert_eq!(iter.next().as_deref(), Some(&false));

	assert_eq!(iter.as_bitslice(), &bits[4 .. 6]);
	assert_eq!(iter.next_back().as_deref(), Some(&false));
	assert_eq!(iter.as_bitslice(), &bits[4 .. 5]);

	assert_eq!(iter.next_back().as_deref(), Some(&true));
	assert!(iter.as_bitslice().is_empty());
	assert!(iter.next().is_none());
	assert!(iter.next_back().is_none());

	let iter2 = iter.clone();
	let bits: &BitSlice<_, _> = iter2.as_ref();
	assert!(bits.is_empty());
}

#[test]
#[allow(deprecated)]
fn iter_mut() {
	let bits = bits![mut Msb0, u8; 0, 1, 1, 0, 1, 0, 0, 1];
	let mut iter = bits.iter_mut();

	*iter.next().unwrap() = true;
	*iter.nth_back(1).unwrap() = true;
	*iter.nth(2).unwrap() = true;
	*iter.next_back().unwrap() = true;

	assert_eq!(iter.into_bitslice().as_bitspan(), bits[4 .. 5].as_bitspan());

	let bitspan = bits.as_bitspan();
	assert_eq!(bits.iter_mut().into_slice().as_bitspan(), bitspan);
	assert_eq!(bits.iter_mut().as_bitslice().as_bitspan(), bitspan);
}

#[test]
fn windows() {
	let bits = bits![LocalBits, u8; 0; 8];

	let mut windows = bits.windows(5);
	assert_eq!(
		windows.next().unwrap().as_bitspan(),
		bits[.. 5].as_bitspan()
	);
	assert_eq!(
		windows.next_back().unwrap().as_bitspan(),
		bits[3 ..].as_bitspan()
	);

	let mut windows = bits.windows(3);
	assert_eq!(
		windows.nth(2).unwrap().as_bitspan(),
		bits[2 .. 5].as_bitspan()
	);
	assert_eq!(
		windows.nth_back(2).unwrap().as_bitspan(),
		bits[3 .. 6].as_bitspan()
	);
	assert!(windows.next().is_none());
	assert!(windows.next_back().is_none());
	assert!(windows.nth(1).is_none());
	assert!(windows.nth_back(1).is_none());
}

#[test]
fn chunks() {
	let bits = bits![Lsb0, u16; 0; 16];

	let mut chunks = bits.chunks(5);
	assert_eq!(chunks.next().unwrap().as_bitspan(), bits[.. 5].as_bitspan());
	assert_eq!(
		chunks.next_back().unwrap().as_bitspan(),
		bits[15 ..].as_bitspan()
	);

	let mut chunks = bits.chunks(3);
	assert_eq!(
		chunks.nth(2).unwrap().as_bitspan(),
		bits[6 .. 9].as_bitspan()
	);
	assert_eq!(
		chunks.nth_back(2).unwrap().as_bitspan(),
		bits[9 .. 12].as_bitspan()
	);
}

#[test]
fn chunks_mut() {
	let bits = bits![mut Msb0, u16; 0; 16];

	let (one, two, three, four) = (
		bits[.. 5].as_bitspan(),
		bits[15 ..].as_bitspan(),
		bits[6 .. 9].as_bitspan(),
		bits[9 .. 12].as_bitspan(),
	);

	let mut chunks = bits.chunks_mut(5);
	assert_eq!(chunks.next().unwrap().as_bitspan(), one);
	assert_eq!(chunks.next_back().unwrap().as_bitspan(), two);

	let mut chunks = bits.chunks_mut(3);
	assert_eq!(chunks.nth(2).unwrap().as_bitspan(), three);
	assert_eq!(chunks.nth_back(2).unwrap().as_bitspan(), four);
}

#[test]
fn chunks_exact() {
	let bits = bits![Lsb0, u32; 0; 32];

	let mut chunks = bits.chunks_exact(5);
	assert_eq!(chunks.remainder().as_bitspan(), bits[30 ..].as_bitspan());
	assert_eq!(chunks.next().unwrap().as_bitspan(), bits[.. 5].as_bitspan());
	assert_eq!(
		chunks.next_back().unwrap().as_bitspan(),
		bits[25 .. 30].as_bitspan(),
	);
	assert_eq!(
		chunks.nth(1).unwrap().as_bitspan(),
		bits[10 .. 15].as_bitspan()
	);
	assert_eq!(
		chunks.nth_back(1).unwrap().as_bitspan(),
		bits[15 .. 20].as_bitspan(),
	);

	assert!(chunks.next().is_none());
	assert!(chunks.next_back().is_none());
	assert!(chunks.nth(1).is_none());
	assert!(chunks.nth_back(1).is_none());
}

#[test]
fn chunks_exact_mut() {
	let bits = bits![mut Msb0, u32; 0; 32];

	let (one, two, three, four, rest) = (
		bits[.. 5].as_bitspan(),
		bits[10 .. 15].as_bitspan(),
		bits[15 .. 20].as_bitspan(),
		bits[25 .. 30].as_bitspan(),
		bits[30 ..].as_bitspan(),
	);

	let mut chunks = bits.chunks_exact_mut(5);
	assert_eq!(chunks.next().unwrap().as_bitspan(), one);
	assert_eq!(chunks.next_back().unwrap().as_bitspan(), four);
	assert_eq!(chunks.nth(1).unwrap().as_bitspan(), two);
	assert_eq!(chunks.nth_back(1).unwrap().as_bitspan(), three);

	assert!(chunks.next().is_none());
	assert!(chunks.next_back().is_none());
	assert!(chunks.nth(1).is_none());
	assert!(chunks.nth_back(1).is_none());

	assert_eq!(chunks.into_remainder().as_bitspan(), rest);
}

#[test]
fn rchunks() {
	let bits = bits![Lsb0, u16; 0; 16];

	let mut rchunks = bits.rchunks(5);
	assert_eq!(
		rchunks.next().unwrap().as_bitspan(),
		bits[11 ..].as_bitspan()
	);
	assert_eq!(
		rchunks.next_back().unwrap().as_bitspan(),
		bits[.. 1].as_bitspan()
	);

	let mut rchunks = bits.rchunks(3);
	assert_eq!(
		rchunks.nth(2).unwrap().as_bitspan(),
		bits[7 .. 10].as_bitspan()
	);
	assert_eq!(
		rchunks.nth_back(2).unwrap().as_bitspan(),
		bits[4 .. 7].as_bitspan()
	);
}

#[test]
fn rchunks_mut() {
	let bits = bits![mut Msb0, u16; 0; 16];

	let (one, two, three, four) = (
		bits[11 ..].as_bitspan(),
		bits[.. 1].as_bitspan(),
		bits[7 .. 10].as_bitspan(),
		bits[4 .. 7].as_bitspan(),
	);

	let mut rchunks = bits.rchunks_mut(5);
	assert_eq!(rchunks.next().unwrap().as_bitspan(), one);
	assert_eq!(rchunks.next_back().unwrap().as_bitspan(), two);

	let mut rchunks = bits.rchunks_mut(3);
	assert_eq!(rchunks.nth(2).unwrap().as_bitspan(), three);
	assert_eq!(rchunks.nth_back(2).unwrap().as_bitspan(), four);
}

#[test]
fn rchunks_exact() {
	let bits = bits![Lsb0, u32; 0; 32];

	let mut rchunks = bits.rchunks_exact(5);
	assert_eq!(rchunks.remainder().as_bitspan(), bits[.. 2].as_bitspan());
	assert_eq!(
		rchunks.next().unwrap().as_bitspan(),
		bits[27 ..].as_bitspan()
	);
	assert_eq!(
		rchunks.next_back().unwrap().as_bitspan(),
		bits[2 .. 7].as_bitspan(),
	);
	assert_eq!(
		rchunks.nth(1).unwrap().as_bitspan(),
		bits[17 .. 22].as_bitspan()
	);
	assert_eq!(
		rchunks.nth_back(1).unwrap().as_bitspan(),
		bits[12 .. 17].as_bitspan(),
	);

	assert!(rchunks.next().is_none());
	assert!(rchunks.next_back().is_none());
	assert!(rchunks.nth(1).is_none());
	assert!(rchunks.nth_back(1).is_none());
}

#[test]
fn rchunks_exact_mut() {
	let bits = bits![mut Msb0, u32; 0; 32];

	let (rest, one, two, three, four) = (
		bits[.. 2].as_bitspan(),
		bits[2 .. 7].as_bitspan(),
		bits[12 .. 17].as_bitspan(),
		bits[17 .. 22].as_bitspan(),
		bits[27 ..].as_bitspan(),
	);

	let mut rchunks = bits.rchunks_exact_mut(5);
	assert_eq!(rchunks.next().unwrap().as_bitspan(), four);
	assert_eq!(rchunks.next_back().unwrap().as_bitspan(), one);
	assert_eq!(rchunks.nth(1).unwrap().as_bitspan(), three);
	assert_eq!(rchunks.nth_back(1).unwrap().as_bitspan(), two);

	assert!(rchunks.next().is_none());
	assert!(rchunks.next_back().is_none());
	assert!(rchunks.nth(1).is_none());
	assert!(rchunks.nth_back(1).is_none());

	assert_eq!(rchunks.into_remainder().as_bitspan(), rest);
}

#[test]
fn iter_ones_zeros() {
	//                          0  1  2  3  4  5  6  7
	let bits = bits![0, 0, 1, 1, 0, 1, 0, 1];
	let mut ones = bits.iter_ones();
	let mut zeros = bits.iter_zeros();

	assert_eq!(ones.next(), Some(2));
	assert_eq!(zeros.next(), Some(0));
	assert_eq!(ones.next_back(), Some(7));
	assert_eq!(zeros.next_back(), Some(6));

	assert_eq!(ones.size_hint(), (2, Some(2)));
	assert_eq!(zeros.size_hint(), (2, Some(2)));
	assert_eq!(ones.clone().count(), 2);
	assert_eq!(zeros.clone().count(), 2);

	assert_eq!(ones.clone().last(), Some(5));
	assert_eq!(zeros.clone().last(), Some(4));

	assert!(ones.nth(2).is_none());
	assert!(zeros.nth(2).is_none());
	assert!(ones.nth_back(0).is_none());
	assert!(zeros.nth_back(0).is_none());
}

#[test]
fn specialized_iter_ones() {
	let data = [0x08u8, 0x20, 0, 0x04, 0x08];

	let bits = data.view_bits::<Msb0>();
	assert!(bits[17 .. 23].sp_iter_ones_first().is_none());
	assert!(bits[17 .. 23].sp_iter_ones_last().is_none());
	assert!(bits[12 .. 28].sp_iter_ones_first().is_none());
	assert!(bits[12 .. 28].sp_iter_ones_last().is_none());

	assert_eq!(bits[3 ..].sp_iter_ones_first(), Some(1));
	assert_eq!(bits[5 ..].sp_iter_ones_first(), Some(5));
	assert_eq!(bits[11 ..].sp_iter_ones_first(), Some(18));
	assert_eq!(bits[30 .. 38].sp_iter_ones_first(), Some(6));
	assert_eq!(bits[34 .. 38].sp_iter_ones_first(), Some(2));

	assert_eq!(bits[.. 38].sp_iter_ones_last(), Some(36));
	assert_eq!(bits[.. 36].sp_iter_ones_last(), Some(29));
	assert_eq!(bits[.. 29].sp_iter_ones_last(), Some(10));
	assert_eq!(bits[2 .. 10].sp_iter_ones_last(), Some(2));
	assert_eq!(bits[2 .. 6].sp_iter_ones_last(), Some(2));

	let bits = data.view_bits::<Lsb0>();
	assert!(bits[17 .. 23].sp_iter_ones_first().is_none());
	assert!(bits[17 .. 23].sp_iter_ones_last().is_none());
	assert!(bits[14 .. 26].sp_iter_ones_first().is_none());
	assert!(bits[14 .. 26].sp_iter_ones_last().is_none());

	assert_eq!(bits[2 ..].sp_iter_ones_first(), Some(1));
	assert_eq!(bits[4 ..].sp_iter_ones_first(), Some(9));
	assert_eq!(bits[14 ..].sp_iter_ones_first(), Some(12));
	assert_eq!(bits[27 .. 38].sp_iter_ones_first(), Some(8));
	assert_eq!(bits[34 .. 38].sp_iter_ones_first(), Some(1));

	assert_eq!(bits[.. 38].sp_iter_ones_last(), Some(35));
	assert_eq!(bits[.. 35].sp_iter_ones_last(), Some(26));
	assert_eq!(bits[.. 26].sp_iter_ones_last(), Some(13));
	assert_eq!(bits[2 .. 13].sp_iter_ones_last(), Some(1));
	assert_eq!(bits[2 .. 6].sp_iter_ones_last(), Some(1));
}

#[test]
fn specialized_iter_zeros() {
	let data = [!0x08u8, !0x20, !0, !0x04, !0x08];

	let bits = data.view_bits::<Msb0>();
	assert!(bits[17 .. 23].sp_iter_zeros_first().is_none());
	assert!(bits[17 .. 23].sp_iter_zeros_last().is_none());
	assert!(bits[12 .. 28].sp_iter_zeros_first().is_none());
	assert!(bits[12 .. 28].sp_iter_zeros_last().is_none());

	assert_eq!(
		bits[3 ..].sp_iter_zeros_first(),
		Some(1),
		"{:b}",
		&bits[3 ..]
	);
	assert_eq!(bits[5 ..].sp_iter_zeros_first(), Some(5));
	assert_eq!(bits[11 ..].sp_iter_zeros_first(), Some(18));
	assert_eq!(bits[30 .. 38].sp_iter_zeros_first(), Some(6));
	assert_eq!(bits[34 .. 38].sp_iter_zeros_first(), Some(2));

	assert_eq!(bits[.. 38].sp_iter_zeros_last(), Some(36));
	assert_eq!(bits[.. 36].sp_iter_zeros_last(), Some(29));
	assert_eq!(bits[.. 29].sp_iter_zeros_last(), Some(10));
	assert_eq!(bits[2 .. 10].sp_iter_zeros_last(), Some(2));
	assert_eq!(bits[2 .. 6].sp_iter_zeros_last(), Some(2));

	let bits = data.view_bits::<Lsb0>();
	assert!(bits[17 .. 23].sp_iter_zeros_first().is_none());
	assert!(bits[17 .. 23].sp_iter_zeros_last().is_none());
	assert!(bits[14 .. 26].sp_iter_zeros_first().is_none());
	assert!(bits[14 .. 26].sp_iter_zeros_last().is_none());

	assert_eq!(bits[2 ..].sp_iter_zeros_first(), Some(1));
	assert_eq!(bits[4 ..].sp_iter_zeros_first(), Some(9));
	assert_eq!(bits[14 ..].sp_iter_zeros_first(), Some(12));
	assert_eq!(bits[27 .. 38].sp_iter_zeros_first(), Some(8));
	assert_eq!(bits[34 .. 38].sp_iter_zeros_first(), Some(1));

	assert_eq!(bits[.. 38].sp_iter_zeros_last(), Some(35));
	assert_eq!(bits[.. 35].sp_iter_zeros_last(), Some(26));
	assert_eq!(bits[.. 26].sp_iter_zeros_last(), Some(13));
	assert_eq!(bits[2 .. 13].sp_iter_zeros_last(), Some(1));
	assert_eq!(bits[2 .. 6].sp_iter_zeros_last(), Some(1));
}

#[cfg(feature = "alloc")]
mod format {
	use crate::prelude::*;

	#[cfg(not(feature = "std"))]
	use alloc::format;

	#[test]
	fn binary() {
		let data = [0u8, 0x0F, !0];
		let bits = data.view_bits::<Msb0>();

		assert_eq!(format!("{:b}", &bits[.. 0]), "[]");
		assert_eq!(format!("{:#b}", &bits[.. 0]), "[]");

		assert_eq!(format!("{:b}", &bits[9 .. 15]), "[000111]");
		assert_eq!(
			format!("{:#b}", &bits[9 .. 15]),
			"[
    0b000111,
]"
		);

		assert_eq!(format!("{:b}", &bits[4 .. 20]), "[0000, 00001111, 1111]");
		assert_eq!(
			format!("{:#b}", &bits[4 .. 20]),
			"[
    0b0000,
    0b00001111,
    0b1111,
]"
		);

		assert_eq!(format!("{:b}", &bits[4 ..]), "[0000, 00001111, 11111111]");
		assert_eq!(
			format!("{:#b}", &bits[4 ..]),
			"[
    0b0000,
    0b00001111,
    0b11111111,
]"
		);

		assert_eq!(format!("{:b}", &bits[.. 20]), "[00000000, 00001111, 1111]");
		assert_eq!(
			format!("{:#b}", &bits[.. 20]),
			"[
    0b00000000,
    0b00001111,
    0b1111,
]"
		);

		assert_eq!(format!("{:b}", bits), "[00000000, 00001111, 11111111]");
		assert_eq!(
			format!("{:#b}", bits),
			"[
    0b00000000,
    0b00001111,
    0b11111111,
]"
		);
	}

	#[test]
	fn octal() {
		let data = [0u8, 0x0F, !0];
		let bits = data.view_bits::<Msb0>();

		assert_eq!(format!("{:o}", &bits[.. 0]), "[]");
		assert_eq!(format!("{:#o}", &bits[.. 0]), "[]");

		assert_eq!(format!("{:o}", &bits[9 .. 15]), "[07]");
		assert_eq!(
			format!("{:#o}", &bits[9 .. 15]),
			"[
    0o07,
]"
		);

		//  …0_000 00_001_111 1_111…
		assert_eq!(format!("{:o}", &bits[4 .. 20]), "[00, 017, 17]");
		assert_eq!(
			format!("{:#o}", &bits[4 .. 20]),
			"[
    0o00,
    0o017,
    0o17,
]"
		);

		assert_eq!(format!("{:o}", &bits[4 ..]), "[00, 017, 377]");
		assert_eq!(
			format!("{:#o}", &bits[4 ..]),
			"[
    0o00,
    0o017,
    0o377,
]"
		);

		assert_eq!(format!("{:o}", &bits[.. 20]), "[000, 017, 17]");
		assert_eq!(
			format!("{:#o}", &bits[.. 20]),
			"[
    0o000,
    0o017,
    0o17,
]"
		);

		assert_eq!(format!("{:o}", bits), "[000, 017, 377]");
		assert_eq!(
			format!("{:#o}", bits),
			"[
    0o000,
    0o017,
    0o377,
]"
		);
	}

	#[test]
	fn hex_lower() {
		let data = [0u8, 0x0F, !0];
		let bits = data.view_bits::<Msb0>();

		assert_eq!(format!("{:x}", &bits[.. 0]), "[]");
		assert_eq!(format!("{:#x}", &bits[.. 0]), "[]");

		//  …00_0111 …
		assert_eq!(format!("{:x}", &bits[9 .. 15]), "[07]");
		assert_eq!(
			format!("{:#x}", &bits[9 .. 15]),
			"[
    0x07,
]"
		);

		//  …0000 00001111 1111…
		assert_eq!(format!("{:x}", &bits[4 .. 20]), "[0, 0f, f]");
		assert_eq!(
			format!("{:#x}", &bits[4 .. 20]),
			"[
    0x0,
    0x0f,
    0xf,
]"
		);

		assert_eq!(format!("{:x}", &bits[4 ..]), "[0, 0f, ff]");
		assert_eq!(
			format!("{:#x}", &bits[4 ..]),
			"[
    0x0,
    0x0f,
    0xff,
]"
		);

		assert_eq!(format!("{:x}", &bits[.. 20]), "[00, 0f, f]");
		assert_eq!(
			format!("{:#x}", &bits[.. 20]),
			"[
    0x00,
    0x0f,
    0xf,
]"
		);

		assert_eq!(format!("{:x}", bits), "[00, 0f, ff]");
		assert_eq!(
			format!("{:#x}", bits),
			"[
    0x00,
    0x0f,
    0xff,
]"
		);
	}

	#[test]
	fn hex_upper() {
		let data = [0u8, 0x0F, !0];
		let bits = data.view_bits::<Msb0>();

		assert_eq!(format!("{:X}", &bits[.. 0]), "[]");
		assert_eq!(format!("{:#X}", &bits[.. 0]), "[]");

		assert_eq!(format!("{:X}", &bits[9 .. 15]), "[07]");
		assert_eq!(
			format!("{:#X}", &bits[9 .. 15]),
			"[
    0x07,
]"
		);

		assert_eq!(format!("{:X}", &bits[4 .. 20]), "[0, 0F, F]");
		assert_eq!(
			format!("{:#X}", &bits[4 .. 20]),
			"[
    0x0,
    0x0F,
    0xF,
]"
		);

		assert_eq!(format!("{:X}", &bits[4 ..]), "[0, 0F, FF]");
		assert_eq!(
			format!("{:#X}", &bits[4 ..]),
			"[
    0x0,
    0x0F,
    0xFF,
]"
		);

		assert_eq!(format!("{:X}", &bits[.. 20]), "[00, 0F, F]");
		assert_eq!(
			format!("{:#X}", &bits[.. 20]),
			"[
    0x00,
    0x0F,
    0xF,
]"
		);

		assert_eq!(format!("{:X}", bits), "[00, 0F, FF]");
		assert_eq!(
			format!("{:#X}", bits),
			"[
    0x00,
    0x0F,
    0xFF,
]"
		);
	}
}
