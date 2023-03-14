/*! Memory access guards.

[`bitvec`] allows a program to produce handles over memory that do not logically
alias their bits, but may alias in hardware. This module provides a unified
interface for memory accesses that can be specialized to handle aliased and
unaliased access events.

The [`BitAccess`] trait provides capabilities to access bits in memory elements
through shared references, and its implementations are responsible for
coördinating synchronization and contention as needed.

The [`BitSafe`] trait abstracts over wrappers to the [`Cell`] and [atomic] types
that forbid writing through their references, even when other references to the
same location may write.

[`BitAccess`]: crate::access::BitAccess
[`BitSafe`]: crate::access::BitSafe
[`Cell`]: core::cell::Cell
[`bitvec`]: crate
!*/

use crate::{
	index::{
		BitIdx,
		BitMask,
	},
	mem::BitRegister,
	order::BitOrder,
};

use core::sync::atomic;

use radium::Radium;

/** Abstracts over the instructions used when accessing a memory location.

This trait provides functions to manipulate bits in a referent memory register
through the appropriate access instructions, so that use sites elsewhere in the
crate can select their required behavior without changing their interface.

This is automatically implemented for all types that permit shared/mutable
memory access to memory registers through the [`radium`] crate. Its use is
constrained in the [`store`] module.

This trait is only ever used by [`bitvec`] internals, and is never exposed
outside the crate. It must be marked as public so that it can be used as an
associated item in [`BitStore`], even though it is never made accessible.

[`BitStore`]: crate::store::BitStore
[`bitvec`]: crate
[`radium`]: radium
[`store`]: crate::store
**/
pub trait BitAccess: Radium
where <Self as Radium>::Item: BitRegister
{
	/// Clears any number of bits in a memory register to `0`.
	///
	/// The mask provided to this method must be constructed from indices that
	/// are valid in the caller’s context. As the mask is already computed by
	/// the caller, this does not take an ordering type parameter.
	///
	/// # Parameters
	///
	/// - `&self`
	/// - `mask`: A mask of any number of bits. This is a selection mask: all
	///   bits in the mask that are set to `1` will be modified in the element
	///   at `*self`.
	///
	/// # Effects
	///
	/// All bits in `*self` that are selected (set to `1` in the `mask`) will be
	/// cleared. All bits in `*self` that are not selected (cleared to `0` in
	/// the `mask`) are unchanged.
	///
	/// Do not invert the `mask` prior to calling this function in order to save
	/// the unselected bits and clear the selected bits. [`BitMask`] is a
	/// selection type, not a bitwise-operation argument.
	///
	/// [`BitMask`]: crate::index::BitMask
	fn clear_bits(&self, mask: BitMask<Self::Item>) {
		self.fetch_and(!mask.value(), atomic::Ordering::Relaxed);
	}

	/// Sets any number of bits in a memory register to `1`.
	///
	/// The mask provided to this method must be constructed from indices that
	/// are valid in the caller’s context. As the mask is already computed by
	/// the caller, this does not take an ordering type parameter.
	///
	/// # Parameters
	///
	/// - `&self`
	/// - `mask`: A mask of any number of bits. This is a selection mask: all
	///   bits in the mask that are set to `1` will be modified in the element
	///   at `*self`.
	///
	/// # Effects
	///
	/// All bits in `*self` that are selected (set to `1` in the `mask`) will be
	/// cleared. All bits in `*self` that are not selected (cleared to `0` in
	/// the `mask`) are unchanged.
	fn set_bits(&self, mask: BitMask<Self::Item>) {
		self.fetch_or(mask.value(), atomic::Ordering::Relaxed);
	}

	/// Inverts any number of bits in a memory register.
	///
	/// The mask provided to this method must be constructed from indices that
	/// are valid in the caller’s context. As the mask is already computed by
	/// the caller, this does not take an ordering type parameter.
	///
	/// # Parameters
	///
	/// - `&self`
	/// - `mask`: A mask of any number of bits. This is a selection mask: all
	///   bits in the mask that are set to `1` will be modified in the element
	///   at `*self`.
	///
	/// # Effects
	///
	/// All bits in `*self` that are selected (set to `1` in the `mask`) will be
	/// inverted. All bits in `*self` that are not selected (cleared to `0` in
	/// the `mask`) are unchanged.
	fn invert_bits(&self, mask: BitMask<Self::Item>) {
		self.fetch_xor(mask.value(), atomic::Ordering::Relaxed);
	}

	/// Writes a value to one bit in a memory register.
	///
	/// # Type Parameters
	///
	/// - `O`: A bit ordering.
	///
	/// # Parameters
	///
	/// - `&self`
	/// - `index`: The semantic index of the bit in `*self` to write.
	/// - `value`: The bit value to write into `*self` at `index`.
	///
	/// # Effects
	///
	/// The memory register at address `self` has the bit corresponding to the
	/// `index` cursor under the `O` order written with the new `value`, and all
	/// other bits are unchanged.
	fn write_bit<O>(&self, index: BitIdx<Self::Item>, value: bool)
	where O: BitOrder {
		if value {
			self.fetch_or(
				index.select::<O>().value(),
				atomic::Ordering::Relaxed,
			);
		}
		else {
			self.fetch_and(
				!index.select::<O>().value(),
				atomic::Ordering::Relaxed,
			);
		}
	}

	/// Gets the function that writes `value` into all bits under a mask.
	///
	/// # Parameters
	///
	/// - `value`: The bit that will be directly written by the returned
	///   function.
	///
	/// # Returns
	///
	/// A function which, when applied to a reference and a mask, will write
	/// `value` into memory. If `value` is `false`, then this produces
	/// [`clear_bits`]; if it is `true`, then this produces [`set_bits`].
	///
	/// [`clear_bits`]: Self::clear_bits
	/// [`set_bits`]: Self::set_bits
	fn get_writers(value: bool) -> for<'a> fn(&'a Self, BitMask<Self::Item>) {
		if value {
			Self::set_bits
		}
		else {
			Self::clear_bits
		}
	}
}

impl<A> BitAccess for A
where
	A: Radium,
	A::Item: BitRegister,
{
}

/** Restricts memory modification to only exclusive references.

The shared-mutability types do not permit locking their references to prevent
writing through them when inappropriate. Implementors of this trait are able to
view aliased memory and handle other references writing to it, even though they
themselves may be forbidden from doing so.
**/
pub trait BitSafe {
	/// The register type being guarded against shared mutation.
	///
	/// This is only present as an extra proof that the type graph all uses the
	/// same underlying integers.
	type Mem: BitRegister;

	/// The accessor type being prevented from mutating while shared.
	///
	/// This is exposed as an associated type so that `BitStore` can name it
	/// without having to re-select it based on crate configuration.
	type Rad: Radium<Item = Self::Mem>;

	/// Reads the value out of memory only if a shared reference to the location
	/// can be produced.
	fn load(&self) -> Self::Mem;

	/// Writes a value into memory only if an exclusive reference to the
	/// location can be produced.
	fn store(&mut self, value: Self::Mem);
}

macro_rules! safe {
	($($t:ident => $w:ident => $r:path),+ $(,)?) => { $(
		/// A wrapper over a shared-mutable type that forbids writing to the
		/// location through its own reference. Other references to the location
		/// may still write to it, and reads from this reference will be aware
		/// of this possibility.
		///
		/// This is necessary in order to enforce [`bitvec`]’s memory model,
		/// which disallows shared mutation to individual bits. [`BitSlice`]s
		/// may produce memory views that use this type in order to ensure that
		/// handles that lack write permission to an area may not write to it,
		/// even if other handles may.
		///
		/// Under the `"atomic"` feature, this uses [`radium`]’s best-effort
		/// atomic alias; when this feature is disabled, then it uses a [`Cell`]
		/// directly.
		///
		/// [`BitSlice`]: crate::slice::BitSlice
		/// [`Cell`]: core::cell::Cell
		/// [`radium`]: radium::types
		#[derive(Debug)]
		#[repr(transparent)]
		pub struct $w {
			inner: <Self as BitSafe>::Rad,
		}

		impl BitSafe for $w {
			type Mem = $t;

			#[cfg(feature = "atomic")]
			type Rad = $r;

			#[cfg(not(feature = "atomic"))]
			type Rad = core::cell::Cell<$t>;

			fn load(&self) -> $t {
				radium::Radium::load(
					&self.inner,
					core::sync::atomic::Ordering::Relaxed,
				)
			}

			fn store(&mut self, value: $t) {
				radium::Radium::store(
					&self.inner,
					value,
					core::sync::atomic::Ordering::Relaxed,
				)
			}
		}
	)+ };
}

safe! {
	u8 => BitSafeU8 => radium::types::RadiumU8,
	u16 => BitSafeU16 => radium::types::RadiumU16,
	u32 => BitSafeU32 => radium::types::RadiumU32,
}

#[cfg(target_pointer_width = "64")]
safe!(u64 => BitSafeU64 => radium::types::RadiumU64);

safe!(usize => BitSafeUsize => radium::types::RadiumUsize);

#[cfg(test)]
mod tests {
	use super::*;
	use crate::prelude::*;

	#[test]
	fn touch_memory() {
		let mut data = 0u8;
		let bits = data.view_bits_mut::<LocalBits>();
		let accessor = unsafe { &*(bits.as_bitspan().address().to_access()) };
		let aliased = unsafe {
			&*(bits.as_bitspan().address().to_const()
				as *const <u8 as BitStore>::Alias)
		};

		BitAccess::set_bits(accessor, BitMask::ALL);
		assert_eq!(accessor.get(), !0);

		BitAccess::clear_bits(accessor, BitMask::ALL);
		assert_eq!(accessor.get(), 0);

		BitAccess::invert_bits(accessor, BitMask::ALL);
		assert_eq!(accessor.get(), !0);

		assert!(BitStore::get_bit::<Lsb0>(aliased, BitIdx::ZERO));
		assert_eq!(accessor.get(), !0);

		BitAccess::write_bit::<Lsb0>(accessor, BitIdx::new(1).unwrap(), false);
		assert_eq!(accessor.get(), !2);
	}

	#[test]
	#[cfg(not(miri))]
	fn sanity_check_prefetch() {
		use core::cell::Cell;
		assert_eq!(
			<Cell<u8> as BitAccess>::get_writers(false) as *const (),
			<Cell<u8> as BitAccess>::clear_bits as *const ()
		);

		assert_eq!(
			<Cell<u8> as BitAccess>::get_writers(true) as *const (),
			<Cell<u8> as BitAccess>::set_bits as *const ()
		);
	}

	#[test]
	fn safe_wrappers() {
		use super::BitSafe;

		let bits = bits![mut Msb0, u8; 0; 24];
		let (l, c): (&mut BitSlice<Msb0, BitSafeU8>, _) = bits.split_at_mut(4);
		let (c, _): (&mut BitSlice<Msb0, BitSafeU8>, _) = c.split_at_mut(16);

		//  Get a write-capable shared reference to the base address,
		let l_redge: &<BitSafeU8 as BitSafe>::Rad =
			l.domain_mut().region().unwrap().2.unwrap().0;
		//  and a write-incapable shared reference to the same base address.
		let c_ledge: &BitSafeU8 = c.domain().region().unwrap().0.unwrap().1;

		//  The split location means that the two subdomains share a location.
		assert_eq!(
			l_redge as *const _ as *const u8,
			c_ledge as *const _ as *const u8,
		);

		//  The center reference can only read,
		assert_eq!(c_ledge.load(), 0);
		//  while the left reference can write,
		l_redge.set_bits(BitMask::new(6));
		//  and be observed by the center.
		assert_eq!(c_ledge.load(), 6);
	}
}
