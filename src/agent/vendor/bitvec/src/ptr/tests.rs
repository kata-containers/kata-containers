#![cfg(test)]

use crate::{
	mutability::Const,
	order::{
		Lsb0,
		Msb0,
	},
	ptr::BitPtr,
	slice::BitSlice,
};

use core::cell::Cell;

use static_assertions::assert_not_impl_any;

#[test]
fn pointers_not_send_sync() {
	assert_not_impl_any!(BitPtr<Const, Lsb0, u8>: Send, Sync);
}

#[test]
fn copies() {
	let mut data = [0xA5u8, 0, 0];

	let base = BitPtr::<_, Msb0, _>::from_mut_slice(&mut data);
	let step = unsafe { base.add(8) };

	unsafe {
		super::copy(base.immut(), step, 8);
		super::copy_nonoverlapping(base.add(4).immut(), step.add(8), 8);
	}
	assert_eq!(data[1], 0xA5);
	assert_eq!(data[2], 0x5A);

	unsafe {
		super::copy(base.add(4).immut(), step, 8);
	}
	assert_eq!(data[1], 0x5A);

	let mut other = 0u16;
	let dest = BitPtr::<_, Lsb0, _>::from_mut(&mut other);
	unsafe {
		super::copy(base.immut(), dest, 16);
	}
	if cfg!(target_endian = "little") {
		assert_eq!(other, 0x5AA5, "{:04x}", other);
	}
}

#[test]
fn misc() {
	let x = 0u32;
	let a = BitPtr::<_, Lsb0, _>::from_ref(&x);
	let b = a.cast::<Cell<u32>>();
	let c = unsafe { b.add(1) };

	assert!(super::eq(a, b));
	assert!(!super::eq(b, c));

	let d = a.cast::<u8>();
	let step = unsafe { d.add(1) }.align_offset(2);
	assert_eq!(step, 15);
	let step = unsafe { d.add(9) }.align_offset(4);
	assert_eq!(step, 23);
}

#[test]
fn io() {
	let mut data = 0u16;
	let base = BitPtr::<_, Msb0, _>::from_mut(&mut data);

	unsafe {
		assert!(!super::read(base.add(1).immut()));
		super::write(base.add(1), true);
		assert!(super::read(base.add(1).immut()));

		assert!(!super::read_volatile(base.add(2).immut()));
		super::write_volatile(base.add(2), true);
		assert!(super::read_volatile(base.add(2).immut()));
		super::write_volatile(base.add(2), false);

		assert!(!super::replace(base.add(3), true));
		assert!(super::read(base.add(3).immut()));

		super::swap(base, base.add(1));
		assert!(super::read(base.immut()));
		assert!(!super::read(base.add(1).immut()));

		super::swap_nonoverlapping(base, base.add(4), 4);
	}
}

#[test]
fn make_slices() {
	let mut data = 0u32;
	let base = BitPtr::<_, Msb0, _>::from_mut(&mut data);

	let a = super::bitslice_from_raw_parts_mut(base, 32);
	let b = super::bitslice_from_raw_parts(base.immut(), 32);

	assert!(core::ptr::eq(a as *const BitSlice<Msb0, u32>, b));
}
