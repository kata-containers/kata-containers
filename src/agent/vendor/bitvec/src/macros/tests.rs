//! Unit tests for the `macros` module.

#![cfg(test)]

use crate::prelude::*;

use core::cell::Cell;

#[test]
fn compile_bitarr_typedef() {
	struct Slots {
		all: bitarr!(for 10, in Msb0, u8),
		typ: bitarr!(for 10, in u8),
		def: bitarr!(for 10),
	}

	let slots = Slots {
		all: bitarr!(Msb0, u8; 1, 1, 1, 1, 1, 1, 1, 1, 1, 1),
		typ: bitarr!(Lsb0, u8; 1, 1, 1, 1, 1, 1, 1, 1, 1, 1),
		def: bitarr!(1, 1, 1, 1, 1, 1, 1, 1, 1, 1),
	};

	assert_eq!(slots.all.value(), [!0u8, 192]);
	assert_eq!(slots.typ.value(), [!0u8, 3]);
	let def: [usize; 1] = slots.def.value();
	assert_eq!(def[0].count_ones(), 10);
}

#[test]
fn compile_bitarr() {
	let uint: BitArray<Lsb0, [u8; 1]> = bitarr![Lsb0, u8; 1, 0, 1, 0];
	assert_eq!(uint.value(), [5u8]);
	let cell: BitArray<Lsb0, [Cell<u8>; 1]> =
		bitarr![Lsb0, Cell<u8>; 1, 0, 1, 0];
	assert_eq!(cell.value()[0].get(), 5u8);

	let uint: BitArray<Msb0, [u16; 2]> = bitarr![Msb0, u16;
		0, 1, 0, 1, 0, 1, 0, 1,
		0, 1, 1, 0, 1, 0, 0, 1,
		0, 1, 1, 0, 1, 1, 1, 0,
		0, 1, 1, 1, 0, 1, 0, 0,
	];
	assert_eq!(uint.value(), [0x5569, 0x6e74]);
	let cell: BitArray<Msb0, [Cell<u16>; 2]> = bitarr![Msb0, Cell<u16>;
		0, 1, 0, 1, 0, 1, 0, 1,
		0, 1, 1, 0, 1, 0, 0, 1,
		0, 1, 1, 0, 1, 1, 1, 0,
		0, 1, 1, 1, 0, 1, 0, 0,
	];
	let cells = cell.value();
	assert_eq!(cells[0].get(), 0x5569);
	assert_eq!(cells[1].get(), 0x6e74);

	let uint: BitArray<Lsb0, [u32; 1]> = bitarr![crate::order::Lsb0, u32;
		1, 0, 1, 1,
	];
	assert_eq!(uint.value(), [13u32]);
	let cell: BitArray<Lsb0, [Cell<u32>; 1]> = bitarr![
		crate::order::Lsb0, Cell<u32>;
		1, 0, 1, 1,
	];
	assert_eq!(cell.value()[0].get(), 13u32);

	#[cfg(target_pointer_width = "64")]
	{
		let uint: BitArray<LocalBits, [u64; 2]> = bitarr![LocalBits, u64; 1; 70];
		assert_eq!(uint.value(), [!0u64; 2]);

		let cell: BitArray<LocalBits, [Cell<u64>; 2]> = bitarr![
			LocalBits, Cell<u64>; 1; 70
		];
		assert_eq!(cell.clone().value()[0].get(), !0u64);
		assert_eq!(cell.value()[1].get(), !0u64);
	}

	let uint: BitArray<Lsb0, [usize; 1]> = bitarr![1, 0, 1];
	assert_eq!(uint.value(), [5usize]);
	let uint: BitArray<Lsb0, [usize; 1]> = bitarr![1; 30];
	assert_eq!(uint.value(), [!0usize]);
}

#[test]
fn compile_bits() {
	let a: &mut BitSlice<Lsb0, Cell<u8>> = bits![mut Lsb0, Cell<u8>; 1, 0, 1];
	let b: &mut BitSlice<Lsb0, u8> = bits![mut Lsb0, u8; 1, 0, 1];
	let c: &mut BitSlice<Msb0, Cell<u8>> =
		bits![mut crate::order::Msb0, Cell<u8>; 1, 0, 1];
	let d: &mut BitSlice<Msb0, u8> = bits![mut crate::order::Msb0, u8; 1, 0, 1];
	assert_eq!(a, c);
	assert_eq!(b, d);

	let e: &mut BitSlice<Lsb0, Cell<u8>> = bits![mut Lsb0, Cell<u8>; 1; 100];
	let f: &mut BitSlice<Lsb0, u8> = bits![mut Lsb0, u8; 1; 100];
	let g: &mut BitSlice<Msb0, Cell<u8>> =
		bits![mut crate::order::Msb0, Cell<u8>; 1; 100];
	let h: &mut BitSlice<Msb0, u8> = bits![mut crate::order::Msb0, u8; 1; 100];
	assert_eq!(e, g);
	assert_eq!(f, h);
	assert_eq!(h.as_slice(), [!0u8; 13]);

	let i: &mut BitSlice<Lsb0, usize> = bits![mut 1, 0, 1];
	let j: &mut BitSlice<Lsb0, usize> = bits![mut 1; 3];
	j.set(1, false);
	assert_eq!(i, j);

	let _: &BitSlice<Lsb0, Cell<u8>> = bits![Lsb0, Cell<u8>; 1, 0, 1];
	let _: &BitSlice<Lsb0, u8> = bits![Lsb0, u8; 1, 0, 1];
	let _: &BitSlice<Msb0, Cell<u8>> =
		bits![crate::order::Msb0, Cell<u8>; 1, 0, 1];
	let _: &BitSlice<Msb0, u8> = bits![crate::order::Msb0, u8; 1, 0, 1];

	let _: &BitSlice<Lsb0, Cell<u8>> = bits![Lsb0, Cell<u8>; 1; 100];
	let _: &BitSlice<Lsb0, u8> = bits![Lsb0, u8; 1; 100];
	let _: &BitSlice<Msb0, Cell<u8>> =
		bits![crate::order::Msb0, Cell<u8>; 1; 100];
	let _: &BitSlice<Msb0, u8> = bits![crate::order::Msb0, u8; 1; 100];

	let _: &BitSlice<Lsb0, usize> = bits![1, 0, 1];
	let _: &BitSlice<Lsb0, usize> = bits![1; 100];

	let _: &BitSlice<Lsb0, Cell<u16>> = bits![Lsb0, Cell<u16>; 1, 0, 1];
	let _: &BitSlice<Lsb0, u16> = bits![Lsb0, u16; 1, 0, 1];
	let _: &BitSlice<Msb0, Cell<u16>> =
		bits![crate::order::Msb0, Cell<u16>; 1, 0, 1];
	let _: &BitSlice<Msb0, u16> = bits![crate::order::Msb0, u16; 1, 0, 1];

	let _: &BitSlice<Lsb0, Cell<u16>> = bits![Lsb0, Cell<u16>; 1; 100];
	let _: &BitSlice<Lsb0, u16> = bits![Lsb0, u16; 1; 100];
	let _: &BitSlice<Msb0, Cell<u16>> =
		bits![crate::order::Msb0, Cell<u16>; 1; 100];
	let _: &BitSlice<Msb0, u16> = bits![crate::order::Msb0, u16; 1; 100];

	let _: &BitSlice<Lsb0, Cell<u32>> = bits![Lsb0, Cell<u32>; 1, 0, 1];
	let _: &BitSlice<Lsb0, u32> = bits![Lsb0, u32; 1, 0, 1];
	let _: &BitSlice<Msb0, Cell<u32>> =
		bits![crate::order::Msb0, Cell<u32>; 1, 0, 1];
	let _: &BitSlice<Msb0, u32> = bits![crate::order::Msb0, u32; 1, 0, 1];

	let _: &BitSlice<Lsb0, Cell<u32>> = bits![Lsb0, Cell<u32>; 1; 100];
	let _: &BitSlice<Lsb0, u32> = bits![Lsb0, u32; 1; 100];
	let _: &BitSlice<Msb0, Cell<u32>> =
		bits![crate::order::Msb0, Cell<u32>; 1; 100];
	let _: &BitSlice<Msb0, u32> = bits![crate::order::Msb0, u32; 1; 100];

	let _: &BitSlice<Lsb0, Cell<usize>> = bits![Lsb0, Cell<usize>; 1, 0, 1];
	let _: &BitSlice<Lsb0, usize> = bits![Lsb0, usize; 1, 0, 1];
	let _: &BitSlice<Msb0, Cell<usize>> =
		bits![crate::order::Msb0, Cell<usize>; 1, 0, 1];
	let _: &BitSlice<Msb0, usize> = bits![crate::order::Msb0, usize; 1, 0, 1];

	let _: &BitSlice<Lsb0, Cell<usize>> = bits![Lsb0, Cell<usize>; 1; 100];
	let _: &BitSlice<Lsb0, usize> = bits![Lsb0, usize; 1; 100];
	let _: &BitSlice<Msb0, Cell<usize>> =
		bits![crate::order::Msb0, Cell<usize>; 1; 100];
	let _: &BitSlice<Msb0, usize> = bits![crate::order::Msb0, usize; 1; 100];

	#[cfg(target_pointer_width = "64")]
	{
		let _: &BitSlice<Lsb0, Cell<u64>> = bits![Lsb0, Cell<u64>; 1, 0, 1];
		let _: &BitSlice<Lsb0, u64> = bits![Lsb0, u64; 1, 0, 1];
		let _: &BitSlice<Msb0, Cell<u64>> =
			bits![crate::order::Msb0, Cell<u64>; 1, 0, 1];
		let _: &BitSlice<Msb0, u64> = bits![crate::order::Msb0, u64; 1, 0, 1];

		let _: &BitSlice<Lsb0, Cell<u64>> = bits![Lsb0, Cell<u64>; 1; 100];
		let _: &BitSlice<Lsb0, u64> = bits![Lsb0, u64; 1; 100];
		let _: &BitSlice<Msb0, Cell<u64>> =
			bits![crate::order::Msb0, Cell<u64>; 1; 100];
		let _: &BitSlice<Msb0, u64> = bits![crate::order::Msb0, u64; 1; 100];
	}

	radium::if_atomic! {
		if atomic(8) {
			use core::sync::atomic::*;

			let _: &BitSlice<LocalBits, AtomicU8> = bits![LocalBits, AtomicU8; 0, 1];
			let _: &BitSlice<Lsb0, AtomicU8> = bits![Lsb0, AtomicU8; 0, 1];
			let _: &BitSlice<Msb0, AtomicU8> = bits![Msb0, AtomicU8; 0, 1];
			let _: &BitSlice<LocalBits, AtomicU8> = bits![LocalBits, AtomicU8; 1; 100];
			let _: &BitSlice<Lsb0, AtomicU8> = bits![Lsb0, AtomicU8; 1; 100];
			let _: &BitSlice<Msb0, AtomicU8> = bits![Msb0, AtomicU8; 1; 100];
		}
		if atomic(16) {
			let _: &BitSlice<LocalBits, AtomicU16> = bits![LocalBits, AtomicU16; 0, 1];
			let _: &BitSlice<Lsb0, AtomicU16> = bits![Lsb0, AtomicU16; 0, 1];
			let _: &BitSlice<Msb0, AtomicU16> = bits![Msb0, AtomicU16; 0, 1];
			let _: &BitSlice<LocalBits, AtomicU16> = bits![LocalBits, AtomicU16; 1; 100];
			let _: &BitSlice<Lsb0, AtomicU16> = bits![Lsb0, AtomicU16; 1; 100];
			let _: &BitSlice<Msb0, AtomicU16> = bits![Msb0, AtomicU16; 1; 100];
		}
		if atomic(32) {
			let _: &BitSlice<LocalBits, AtomicU32> = bits![LocalBits, AtomicU32; 0, 1];
			let _: &BitSlice<Lsb0, AtomicU32> = bits![Lsb0, AtomicU32; 0, 1];
			let _: &BitSlice<Msb0, AtomicU32> = bits![Msb0, AtomicU32; 0, 1];
			let _: &BitSlice<LocalBits, AtomicU32> = bits![LocalBits, AtomicU32; 1; 100];
			let _: &BitSlice<Lsb0, AtomicU32> = bits![Lsb0, AtomicU32; 1; 100];
			let _: &BitSlice<Msb0, AtomicU32> = bits![Msb0, AtomicU32; 1; 100];
		}
		if atomic(size) {
			let _: &BitSlice<LocalBits, AtomicUsize> = bits![LocalBits, AtomicUsize; 0, 1];
			let _: &BitSlice<Lsb0, AtomicUsize> = bits![Lsb0, AtomicUsize; 0, 1];
			let _: &BitSlice<Msb0, AtomicUsize> = bits![Msb0, AtomicUsize; 0, 1];
			let _: &BitSlice<LocalBits, AtomicUsize> = bits![LocalBits, AtomicUsize; 1; 100];
			let _: &BitSlice<Lsb0, AtomicUsize> = bits![Lsb0, AtomicUsize; 1; 100];
			let _: &BitSlice<Msb0, AtomicUsize> = bits![Msb0, AtomicUsize; 1; 100];
		}
	}
	#[cfg(target_pointer_width = "64")]
	radium::if_atomic! {
		if atomic(64) {
			let _: &BitSlice<LocalBits, AtomicU64> = bits![LocalBits, AtomicU64; 0, 1];
			let _: &BitSlice<Lsb0, AtomicU64> = bits![Lsb0, AtomicU64; 0, 1];
			let _: &BitSlice<Msb0, AtomicU64> = bits![Msb0, AtomicU64; 0, 1];
			let _: &BitSlice<LocalBits, AtomicU64> = bits![LocalBits, AtomicU64; 1; 100];
			let _: &BitSlice<Lsb0, AtomicU64> = bits![Lsb0, AtomicU64; 1; 100];
			let _: &BitSlice<Msb0, AtomicU64> = bits![Msb0, AtomicU64; 1; 100];
		}
	}
}

#[test]
#[cfg(feature = "alloc")]
fn compile_bitvec() {
	let _: BitVec<Lsb0, Cell<u8>> = bitvec![Lsb0, Cell<u8>; 1, 0, 1];
	let _: BitVec<Lsb0, u8> = bitvec![Lsb0, u8; 1, 0, 1];
	let _: BitVec<Msb0, Cell<u8>> =
		bitvec![crate::order::Msb0, Cell<u8>; 1, 0, 1];
	let _: BitVec<Msb0, u8> = bitvec![crate::order::Msb0, u8; 1, 0, 1];

	let _: BitVec<Lsb0, Cell<u8>> = bitvec![Lsb0, Cell<u8>; 1; 100];
	let _: BitVec<Lsb0, u8> = bitvec![Lsb0, u8; 1; 100];
	let _: BitVec<Msb0, Cell<u8>> =
		bitvec![crate::order::Msb0, Cell<u8>; 1; 100];
	let _: BitVec<Msb0, u8> = bitvec![crate::order::Msb0, u8; 1; 100];

	let _: BitVec<Lsb0, usize> = bitvec![1, 0, 1];
	let _: BitVec<Lsb0, usize> = bitvec![1; 100];

	let _: BitVec<Lsb0, Cell<u16>> = bitvec![Lsb0, Cell<u16>; 1, 0, 1];
	let _: BitVec<Lsb0, u16> = bitvec![Lsb0, u16; 1, 0, 1];
	let _: BitVec<Msb0, Cell<u16>> =
		bitvec![crate::order::Msb0, Cell<u16>; 1, 0, 1];
	let _: BitVec<Msb0, u16> = bitvec![crate::order::Msb0, u16; 1, 0, 1];

	let _: BitVec<Lsb0, Cell<u16>> = bitvec![Lsb0, Cell<u16>; 1; 100];
	let _: BitVec<Lsb0, u16> = bitvec![Lsb0, u16; 1; 100];
	let _: BitVec<Msb0, Cell<u16>> =
		bitvec![crate::order::Msb0, Cell<u16>; 1; 100];
	let _: BitVec<Msb0, u16> = bitvec![crate::order::Msb0, u16; 1; 100];

	let _: BitVec<Lsb0, Cell<u32>> = bitvec![Lsb0, Cell<u32>; 1, 0, 1];
	let _: BitVec<Lsb0, u32> = bitvec![Lsb0, u32; 1, 0, 1];
	let _: BitVec<Msb0, Cell<u32>> =
		bitvec![crate::order::Msb0, Cell<u32>; 1, 0, 1];
	let _: BitVec<Msb0, u32> = bitvec![crate::order::Msb0, u32; 1, 0, 1];

	let _: BitVec<Lsb0, Cell<u32>> = bitvec![Lsb0, Cell<u32>; 1; 100];
	let _: BitVec<Lsb0, u32> = bitvec![Lsb0, u32; 1; 100];
	let _: BitVec<Msb0, Cell<u32>> =
		bitvec![crate::order::Msb0, Cell<u32>; 1; 100];
	let _: BitVec<Msb0, u32> = bitvec![crate::order::Msb0, u32; 1; 100];

	let _: BitVec<Lsb0, Cell<usize>> = bitvec![Lsb0, Cell<usize>; 1, 0, 1];
	let _: BitVec<Lsb0, usize> = bitvec![Lsb0, usize; 1, 0, 1];
	let _: BitVec<Msb0, Cell<usize>> =
		bitvec![crate::order::Msb0, Cell<usize>; 1, 0, 1];
	let _: BitVec<Msb0, usize> = bitvec![crate::order::Msb0, usize; 1, 0, 1];

	let _: BitVec<Lsb0, Cell<usize>> = bitvec![Lsb0, Cell<usize>; 1; 100];
	let _: BitVec<Lsb0, usize> = bitvec![Lsb0, usize; 1; 100];
	let _: BitVec<Msb0, Cell<usize>> =
		bitvec![crate::order::Msb0, Cell<usize>; 1; 100];
	let _: BitVec<Msb0, usize> = bitvec![crate::order::Msb0, usize; 1; 100];

	#[cfg(target_pointer_width = "64")]
	{
		let _: BitVec<Lsb0, Cell<u64>> = bitvec![Lsb0, Cell<u64>; 1, 0, 1];
		let _: BitVec<Lsb0, u64> = bitvec![Lsb0, u64; 1, 0, 1];
		let _: BitVec<Msb0, Cell<u64>> =
			bitvec![crate::order::Msb0, Cell<u64>; 1, 0, 1];
		let _: BitVec<Msb0, u64> = bitvec![crate::order::Msb0, u64; 1, 0, 1];

		let _: BitVec<Lsb0, Cell<u64>> = bitvec![Lsb0, Cell<u64>; 1; 100];
		let _: BitVec<Lsb0, u64> = bitvec![Lsb0, u64; 1; 100];
		let _: BitVec<Msb0, Cell<u64>> =
			bitvec![crate::order::Msb0, Cell<u64>; 1; 100];
		let _: BitVec<Msb0, u64> = bitvec![crate::order::Msb0, u64; 1; 100];
	}

	radium::if_atomic! {
		if atomic(8) {
			use core::sync::atomic::*;

			let _: BitVec<LocalBits, AtomicU8> = bitvec![LocalBits, AtomicU8; 0, 1];
			let _: BitVec<Lsb0, AtomicU8> = bitvec![Lsb0, AtomicU8; 0, 1];
			let _: BitVec<Msb0, AtomicU8> = bitvec![Msb0, AtomicU8; 0, 1];
			let _: BitVec<LocalBits, AtomicU8> = bitvec![LocalBits, AtomicU8; 1; 100];
			let _: BitVec<Lsb0, AtomicU8> = bitvec![Lsb0, AtomicU8; 1; 100];
			let _: BitVec<Msb0, AtomicU8> = bitvec![Msb0, AtomicU8; 1; 100];
		}
		if atomic(16) {
			let _: BitVec<LocalBits, AtomicU16> = bitvec![LocalBits, AtomicU16; 0, 1];
			let _: BitVec<Lsb0, AtomicU16> = bitvec![Lsb0, AtomicU16; 0, 1];
			let _: BitVec<Msb0, AtomicU16> = bitvec![Msb0, AtomicU16; 0, 1];
			let _: BitVec<LocalBits, AtomicU16> = bitvec![LocalBits, AtomicU16; 1; 100];
			let _: BitVec<Lsb0, AtomicU16> = bitvec![Lsb0, AtomicU16; 1; 100];
			let _: BitVec<Msb0, AtomicU16> = bitvec![Msb0, AtomicU16; 1; 100];
		}
		if atomic(32) {
			let _: BitVec<LocalBits, AtomicU32> = bitvec![LocalBits, AtomicU32; 0, 1];
			let _: BitVec<Lsb0, AtomicU32> = bitvec![Lsb0, AtomicU32; 0, 1];
			let _: BitVec<Msb0, AtomicU32> = bitvec![Msb0, AtomicU32; 0, 1];
			let _: BitVec<LocalBits, AtomicU32> = bitvec![LocalBits, AtomicU32; 1; 100];
			let _: BitVec<Lsb0, AtomicU32> = bitvec![Lsb0, AtomicU32; 1; 100];
			let _: BitVec<Msb0, AtomicU32> = bitvec![Msb0, AtomicU32; 1; 100];
		}
		if atomic(size) {
			let _: BitVec<LocalBits, AtomicUsize> = bitvec![LocalBits, AtomicUsize; 0, 1];
			let _: BitVec<Lsb0, AtomicUsize> = bitvec![Lsb0, AtomicUsize; 0, 1];
			let _: BitVec<Msb0, AtomicUsize> = bitvec![Msb0, AtomicUsize; 0, 1];
			let _: BitVec<LocalBits, AtomicUsize> = bitvec![LocalBits, AtomicUsize; 1; 100];
			let _: BitVec<Lsb0, AtomicUsize> = bitvec![Lsb0, AtomicUsize; 1; 100];
			let _: BitVec<Msb0, AtomicUsize> = bitvec![Msb0, AtomicUsize; 1; 100];
		}
	}
	#[cfg(target_pointer_width = "64")]
	radium::if_atomic! {
		if atomic(64) {
			let _: BitVec<LocalBits, AtomicU64> = bitvec![LocalBits, AtomicU64; 0, 1];
			let _: BitVec<Lsb0, AtomicU64> = bitvec![Lsb0, AtomicU64; 0, 1];
			let _: BitVec<Msb0, AtomicU64> = bitvec![Msb0, AtomicU64; 0, 1];
			let _: BitVec<LocalBits, AtomicU64> = bitvec![LocalBits, AtomicU64; 1; 100];
			let _: BitVec<Lsb0, AtomicU64> = bitvec![Lsb0, AtomicU64; 1; 100];
			let _: BitVec<Msb0, AtomicU64> = bitvec![Msb0, AtomicU64; 1; 100];
		}
	}
}

#[test]
#[cfg(feature = "alloc")]
fn compile_bitbox() {
	let _: BitBox<Lsb0, Cell<u8>> = bitbox![Lsb0, Cell<u8>; 1, 0, 1];
	let _: BitBox<Lsb0, u8> = bitbox![Lsb0, u8; 1, 0, 1];
	let _: BitBox<Msb0, Cell<u8>> =
		bitbox![crate::order::Msb0, Cell<u8>; 1, 0, 1];
	let _: BitBox<Msb0, u8> = bitbox![crate::order::Msb0, u8; 1, 0, 1];

	let _: BitBox<Lsb0, Cell<u8>> = bitbox![Lsb0, Cell<u8>; 1; 100];
	let _: BitBox<Lsb0, u8> = bitbox![Lsb0, u8; 1; 100];
	let _: BitBox<Msb0, Cell<u8>> =
		bitbox![crate::order::Msb0, Cell<u8>; 1; 100];
	let _: BitBox<Msb0, u8> = bitbox![crate::order::Msb0, u8; 1; 100];

	let _: BitBox<Lsb0, usize> = bitbox![1, 0, 1];
	let _: BitBox<Lsb0, usize> = bitbox![1; 100];

	let _: BitBox<Lsb0, Cell<u16>> = bitbox![Lsb0, Cell<u16>; 1, 0, 1];
	let _: BitBox<Lsb0, u16> = bitbox![Lsb0, u16; 1, 0, 1];
	let _: BitBox<Msb0, Cell<u16>> =
		bitbox![crate::order::Msb0, Cell<u16>; 1, 0, 1];
	let _: BitBox<Msb0, u16> = bitbox![crate::order::Msb0, u16; 1, 0, 1];

	let _: BitBox<Lsb0, Cell<u16>> = bitbox![Lsb0, Cell<u16>; 1; 100];
	let _: BitBox<Lsb0, u16> = bitbox![Lsb0, u16; 1; 100];
	let _: BitBox<Msb0, Cell<u16>> =
		bitbox![crate::order::Msb0, Cell<u16>; 1; 100];
	let _: BitBox<Msb0, u16> = bitbox![crate::order::Msb0, u16; 1; 100];

	let _: BitBox<Lsb0, Cell<u32>> = bitbox![Lsb0, Cell<u32>; 1, 0, 1];
	let _: BitBox<Lsb0, u32> = bitbox![Lsb0, u32; 1, 0, 1];
	let _: BitBox<Msb0, Cell<u32>> =
		bitbox![crate::order::Msb0, Cell<u32>; 1, 0, 1];
	let _: BitBox<Msb0, u32> = bitbox![crate::order::Msb0, u32; 1, 0, 1];

	let _: BitBox<Lsb0, Cell<u32>> = bitbox![Lsb0, Cell<u32>; 1; 100];
	let _: BitBox<Lsb0, u32> = bitbox![Lsb0, u32; 1; 100];
	let _: BitBox<Msb0, Cell<u32>> =
		bitbox![crate::order::Msb0, Cell<u32>; 1; 100];
	let _: BitBox<Msb0, u32> = bitbox![crate::order::Msb0, u32; 1; 100];

	let _: BitBox<Lsb0, Cell<usize>> = bitbox![Lsb0, Cell<usize>; 1, 0, 1];
	let _: BitBox<Lsb0, usize> = bitbox![Lsb0, usize; 1, 0, 1];
	let _: BitBox<Msb0, Cell<usize>> =
		bitbox![crate::order::Msb0, Cell<usize>; 1, 0, 1];
	let _: BitBox<Msb0, usize> = bitbox![crate::order::Msb0, usize; 1, 0, 1];

	let _: BitBox<Lsb0, Cell<usize>> = bitbox![Lsb0, Cell<usize>; 1; 100];
	let _: BitBox<Lsb0, usize> = bitbox![Lsb0, usize; 1; 100];
	let _: BitBox<Msb0, Cell<usize>> =
		bitbox![crate::order::Msb0, Cell<usize>; 1; 100];
	let _: BitBox<Msb0, usize> = bitbox![crate::order::Msb0, usize; 1; 100];

	#[cfg(target_pointer_width = "64")]
	{
		let _: BitBox<Lsb0, Cell<u64>> = bitbox![Lsb0, Cell<u64>; 1, 0, 1];
		let _: BitBox<Lsb0, u64> = bitbox![Lsb0, u64; 1, 0, 1];
		let _: BitBox<Msb0, Cell<u64>> =
			bitbox![crate::order::Msb0, Cell<u64>; 1, 0, 1];
		let _: BitBox<Msb0, u64> = bitbox![crate::order::Msb0, u64; 1, 0, 1];

		let _: BitBox<Lsb0, Cell<u64>> = bitbox![Lsb0, Cell<u64>; 1; 100];
		let _: BitBox<Lsb0, u64> = bitbox![Lsb0, u64; 1; 100];
		let _: BitBox<Msb0, Cell<u64>> =
			bitbox![crate::order::Msb0, Cell<u64>; 1; 100];
		let _: BitBox<Msb0, u64> = bitbox![crate::order::Msb0, u64; 1; 100];
	}

	radium::if_atomic! {
		if atomic(8) {
			use core::sync::atomic::*;

			let _: BitBox<LocalBits, AtomicU8> = bitbox![LocalBits, AtomicU8; 0, 1];
			let _: BitBox<Lsb0, AtomicU8> = bitbox![Lsb0, AtomicU8; 0, 1];
			let _: BitBox<Msb0, AtomicU8> = bitbox![Msb0, AtomicU8; 0, 1];
			let _: BitBox<LocalBits, AtomicU8> = bitbox![LocalBits, AtomicU8; 1; 100];
			let _: BitBox<Lsb0, AtomicU8> = bitbox![Lsb0, AtomicU8; 1; 100];
			let _: BitBox<Msb0, AtomicU8> = bitbox![Msb0, AtomicU8; 1; 100];
		}
		if atomic(16) {
			let _: BitBox<LocalBits, AtomicU16> = bitbox![LocalBits, AtomicU16; 0, 1];
			let _: BitBox<Lsb0, AtomicU16> = bitbox![Lsb0, AtomicU16; 0, 1];
			let _: BitBox<Msb0, AtomicU16> = bitbox![Msb0, AtomicU16; 0, 1];
			let _: BitBox<LocalBits, AtomicU16> = bitbox![LocalBits, AtomicU16; 1; 100];
			let _: BitBox<Lsb0, AtomicU16> = bitbox![Lsb0, AtomicU16; 1; 100];
			let _: BitBox<Msb0, AtomicU16> = bitbox![Msb0, AtomicU16; 1; 100];
		}
		if atomic(32) {
			let _: BitBox<LocalBits, AtomicU32> = bitbox![LocalBits, AtomicU32; 0, 1];
			let _: BitBox<Lsb0, AtomicU32> = bitbox![Lsb0, AtomicU32; 0, 1];
			let _: BitBox<Msb0, AtomicU32> = bitbox![Msb0, AtomicU32; 0, 1];
			let _: BitBox<LocalBits, AtomicU32> = bitbox![LocalBits, AtomicU32; 1; 100];
			let _: BitBox<Lsb0, AtomicU32> = bitbox![Lsb0, AtomicU32; 1; 100];
			let _: BitBox<Msb0, AtomicU32> = bitbox![Msb0, AtomicU32; 1; 100];
		}
		if atomic(size) {
			let _: BitBox<LocalBits, AtomicUsize> = bitbox![LocalBits, AtomicUsize; 0, 1];
			let _: BitBox<Lsb0, AtomicUsize> = bitbox![Lsb0, AtomicUsize; 0, 1];
			let _: BitBox<Msb0, AtomicUsize> = bitbox![Msb0, AtomicUsize; 0, 1];
			let _: BitBox<LocalBits, AtomicUsize> = bitbox![LocalBits, AtomicUsize; 1; 100];
			let _: BitBox<Lsb0, AtomicUsize> = bitbox![Lsb0, AtomicUsize; 1; 100];
			let _: BitBox<Msb0, AtomicUsize> = bitbox![Msb0, AtomicUsize; 1; 100];
		}
	}
	#[cfg(target_pointer_width = "64")]
	radium::if_atomic! {
		if atomic(64) {
			let _: BitBox<LocalBits, AtomicU64> = bitbox![LocalBits, AtomicU64; 0, 1];
			let _: BitBox<Lsb0, AtomicU64> = bitbox![Lsb0, AtomicU64; 0, 1];
			let _: BitBox<Msb0, AtomicU64> = bitbox![Msb0, AtomicU64; 0, 1];
			let _: BitBox<LocalBits, AtomicU64> = bitbox![LocalBits, AtomicU64; 1; 100];
			let _: BitBox<Lsb0, AtomicU64> = bitbox![Lsb0, AtomicU64; 1; 100];
			let _: BitBox<Msb0, AtomicU64> = bitbox![Msb0, AtomicU64; 1; 100];
		}
	}
}

#[test]
fn encode_bits() {
	let uint: [u8; 1] = __encode_bits!(Lsb0, u8; 1, 0, 1, 0, 0, 0, 0, 0);
	assert_eq!(uint, [5]);

	let cell: [Cell<u8>; 1] =
		__encode_bits!(Lsb0, Cell<u8>; 1, 0, 1, 0, 0, 0, 0, 0);
	assert_eq!(cell[0].get(), 5);

	let uint: [u16; 1] = __encode_bits!(Msb0, u16;
		0, 1, 0, 0, 1, 0, 0, 0,
		0, 1, 1, 0, 1, 0, 0, 1
	);
	assert_eq!(uint, [0x4869]);

	let cell: [Cell<u16>; 1] = __encode_bits!(Msb0, Cell<u16>;
		0, 1, 0, 0, 1, 0, 0, 0,
		0, 1, 1, 0, 1, 0, 0, 1
	);
	assert_eq!(cell[0].get(), 0x4869);

	let uint: [u32; 1] = __encode_bits!(LocalBits, u32; 1, 0, 1);
	assert_eq!(uint.view_bits::<LocalBits>()[.. 3], bits![1, 0, 1]);

	let cell: [Cell<u32>; 1] = __encode_bits!(LocalBits, Cell<u32>; 1, 0, 1);
	let bits: &BitSlice<LocalBits, Cell<u32>> = cell.view_bits::<_>();
	assert_eq!(bits[.. 3], bits![1, 0, 1]);
}

#[test]
fn make_elem() {
	let uint: u8 = __make_elem!(Lsb0, u8 as u8; 1, 0, 1, 0, 0, 0, 0, 0);
	assert_eq!(uint, 5);

	let cell: Cell<u8> =
		__make_elem!(Lsb0, Cell<u8> as u8; 1, 0, 1, 0, 0, 0, 0, 0);
	assert_eq!(cell.get(), 5);

	let uint: u16 = __make_elem!(Msb0, u16 as u16;
		0, 1, 0, 0, 1, 0, 0, 0,
		0, 1, 1, 0, 1, 0, 0, 1
	);
	assert_eq!(uint, 0x4869);

	let cell: Cell<u16> = __make_elem!(Msb0, Cell<u16> as u16;
		0, 1, 0, 0, 1, 0, 0, 0,
		0, 1, 1, 0, 1, 0, 0, 1
	);
	assert_eq!(cell.get(), 0x4869);

	let uint: u32 = __make_elem!(LocalBits, u32 as u32; 1, 0, 1);
	assert_eq!(uint.view_bits::<LocalBits>()[.. 3], bits![1, 0, 1]);

	let cell: Cell<u32> = __make_elem!(LocalBits, Cell<u32> as u32; 1, 0, 1);
	let bits: &BitSlice<LocalBits, Cell<u32>> = cell.view_bits::<_>();
	assert_eq!(bits[.. 3], bits![1, 0, 1]);

	//  `__make_elem!` is only called after `$ord` has already been made opaque
	//  to matchers as a single `:tt`. Calling it directly with a path will fail
	//  the `:tt`, so this macro wraps it as one and forwards the rest.
	macro_rules! invoke_make_elem {
		($ord:path, $($rest:tt)*) => {
			__make_elem!($ord, $($rest)*)
		};
	}
	let uint: usize =
		invoke_make_elem!(crate::order::Lsb0, usize as usize; 0, 0, 1, 1);
	assert_eq!(uint, 12);
	let cell: Cell<usize> =
		invoke_make_elem!(crate::order::Lsb0, Cell<usize> as usize; 0, 0, 1, 1);
	assert_eq!(cell.get(), 12);
}
