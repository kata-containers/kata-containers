/*! [`BitSlice`] view adapters for memory regions.

The [`&BitSlice`][`BitSlice`] type is a referential view over existing memory.
The inherent constructor functions are awkward to call, as they require function
syntax rather than method syntax, and must provide a token for the memory type
argument even though this is informed by the already-existing reference being
used.

This module provides an extension trait, [`BitView`], which provides methods on
many memory types (all [`BitRegister`] integers, and slices and arrays of them)
to construct [`BitSlice`] over those values.

In addition, the traits [`AsBits`] and [`AsBitsMut`] are analogues of [`AsRef`]
and [`AsMut`], respectively. These traits have a blanket implementation for all
`A: As{Ref,Mut}<[T: BitRegister]>`, so that any type that implements a view to a
suitable memory region automatically implements a view to that region’s bits.

These traits are distinct because [`BitView`] combines the im/mutable view
functions into one trait, and can provide specialized implementations with a
slight performance increase over the generic, but `AsBits{,Mut}` can fit in the
generic type system of any library without undue effort.

[`AsBits`]: crate::view::AsBits
[`AsBitsMut`]: crate::view::AsBitsMut
[`AsMut`]: core::convert::AsMut
[`AsRef`]: core::convert::AsRef
[`BitRegister`]: crate::mem::BitRegister
[`BitSlice`]: crate::slice::BitSlice
[`BitView`]: crate::view::BitView
!*/

use crate::{
	mem::{
		BitMemory,
		BitRegister,
	},
	order::BitOrder,
	ptr::BitPtr,
	slice::{
		from_raw_parts_unchecked,
		from_raw_parts_unchecked_mut,
		BitSlice,
	},
	store::BitStore,
};

/** Creates a [`BitSlice`] view over some type that supports it.

This trait is implemented on all [`BitRegister`] types, and the arrays and slices
of them that are supported by the standard library.

This means that until type-level integers are stabilized, only arrays in
`[T: BitRegister; 0 ..= 64]` will implement the trait; wider arrays will need to
reborrow as slices `[T]` in order to use the slice implementation.

If you have a type that contains a [`BitRegister`] type that can be viewed with
this trait, then you can implement this trait by forwarding to the interior
view.

[`BitSlice`]: crate::slice::BitSlice
[`BitRegister`]: crate::mem::BitRegister
**/
pub trait BitView {
	/// The region’s storage type.
	type Store: BitStore;

	/// Views a memory region as a [`BitSlice`].
	///
	/// # Type Parameters
	///
	/// - `O`: The bit ordering used for the region.
	///
	/// # Parameters
	///
	/// - `&self`: The region to view as individual bits.
	///
	/// # Returns
	///
	/// A `&BitSlice` view over the region at `*self`.
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	fn view_bits<O>(&self) -> &BitSlice<O, Self::Store>
	where O: BitOrder;

	/// Views a memory region as a mutable [`BitSlice`].
	///
	/// # Type Parameters
	///
	/// - `O`: The bit ordering used for the region.
	///
	/// # Parameters
	///
	/// - `&mut self`: The region to view as individual mutable bits.
	///
	/// # Returns
	///
	/// A `&mut BitSlice` view over the region at `*self`.
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	fn view_bits_mut<O>(&mut self) -> &mut BitSlice<O, Self::Store>
	where O: BitOrder;

	/// Produces the number of bits that the implementing type can hold.
	#[doc(hidden)]
	#[inline]
	fn const_bits() -> usize
	where Self: Sized {
		Self::const_elts()
			* <<Self::Store as BitStore>::Mem as BitMemory>::BITS as usize
	}

	/// Produces the number of memory elements that the implementing type holds.
	#[doc(hidden)]
	fn const_elts() -> usize
	where Self: Sized;
}

#[cfg(not(tarpaulin_include))]
impl<T> BitView for T
where T: BitStore
{
	type Store = T;

	#[inline(always)]
	fn view_bits<O>(&self) -> &BitSlice<O, T>
	where O: BitOrder {
		BitSlice::from_element(self)
	}

	#[inline(always)]
	fn view_bits_mut<O>(&mut self) -> &mut BitSlice<O, T>
	where O: BitOrder {
		BitSlice::from_element_mut(self)
	}

	#[doc(hidden)]
	#[inline(always)]
	fn const_elts() -> usize {
		1
	}
}

impl<T> BitView for [T]
where T: BitStore
{
	type Store = T;

	#[inline]
	fn view_bits<O>(&self) -> &BitSlice<O, T>
	where O: BitOrder {
		BitSlice::from_slice(self).expect("slice was too long to view as bits")
	}

	#[inline]
	fn view_bits_mut<O>(&mut self) -> &mut BitSlice<O, T>
	where O: BitOrder {
		BitSlice::from_slice_mut(self)
			.expect("slice was too long to view as bits")
	}

	/// Slices cannot implement this function.
	#[cold]
	#[doc(hidden)]
	#[inline(never)]
	fn const_elts() -> usize {
		unreachable!("This cannot be called on unsized slices")
	}
}

#[cfg(not(tarpaulin_include))]
impl<T> BitView for [T; 0]
where T: BitStore
{
	type Store = T;

	#[inline(always)]
	fn view_bits<O>(&self) -> &BitSlice<O, T>
	where O: BitOrder {
		BitSlice::empty()
	}

	#[inline(always)]
	fn view_bits_mut<O>(&mut self) -> &mut BitSlice<O, T>
	where O: BitOrder {
		BitSlice::empty_mut()
	}

	#[doc(hidden)]
	fn const_elts() -> usize {
		0
	}
}

//  Replace with a const-generic once that becomes available.
macro_rules! view_bits {
	($($n:expr),+ $(,)?) => { $(
		impl<T> BitView for [T; $n]
		where T: BitStore {
			type Store = T;

			#[inline]
			fn view_bits<O>(&self) -> &BitSlice<O, T>
			where O: BitOrder {
				unsafe { from_raw_parts_unchecked(
					BitPtr::from_slice(&self[..]),
					$n * T::Mem::BITS as usize,
				) }
			}

			#[inline]
			fn view_bits_mut<O>(&mut self) -> &mut BitSlice<O, T>
			where O: BitOrder {
				unsafe { from_raw_parts_unchecked_mut(
					BitPtr::from_mut_slice(&mut self[..]),
					$n * T::Mem::BITS as usize,
				) }
			}

			#[doc(hidden)]
			#[inline(always)]
			#[cfg(not(tarpaulin_include))]
			fn const_elts() -> usize {
				$n
			}
		}
	)+ };
}

view_bits!(
	1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
	22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
	41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59,
	60, 61, 62, 63, 64
);

/** Views a region as an immutable [`BitSlice`] only.

This trait is an analogue to the [`AsRef`] trait, in that it enables any type to
provide an immutable-only view of a bit slice.

It does not require an `AsRef<[T: BitStore]>` implementation, and a blanket
implementation for all such types is provided. This allows you to choose whether
to implement only one of `AsBits<T>` or `AsRef<[T]>`, and gain a [`BitSlice`]
view with either choice.

# Type Parameters

- `T`: The underlying storage region.

# Notes

You are not *forbidden* from creating multiple views with different element
types to the same region, but doing so is likely to cause inconsistent and
surprising behavior.

Refrain from implementing this trait with more than one storage argument unless
you are sure that you can uphold the memory region requirements of all of them,
and are aware of the behavior conflicts that may arise.

[`AsRef`]: core::convert::AsRef
[`BitSlice`]: crate::slice::BitSlice
**/
pub trait AsBits<T>
where T: BitStore
{
	/// Views memory as a slice of immutable bits.
	///
	/// # Type Parameters
	///
	/// - `O`: The bit ordering used for the region.
	///
	/// # Parameters
	///
	/// - `&self`: The value that is providing a [`BitSlice`] view.
	///
	/// # Returns
	///
	/// An immutable view into some bits.
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	fn as_bits<O>(&self) -> &BitSlice<O, T>
	where O: BitOrder;
}

/** Views a region as a mutable [`BitSlice`].

This trait is an analogue to the [`AsMut`] trait, in that it enables any type to
provide a mutable view of a bit slice.

It does not require an `AsMut<[T: BitStore]>` implementation, and a blanket
implementation for all such types is provided. This allows you to choose whether
to implement only one of `AsBitsMut<T>` or `AsMut<[T]>`, and gain a [`BitSlice`]
view with either choice.

# Type Parameters

- `T`: The underlying storage region.

# Notes

You are not *forbidden* from creating multiple views with different element
types to the same region, but doing so is likely to cause inconsistent and
surprising behavior.

Refrain from implementing this trait with more than one storage argument unless
you are sure that you can uphold the memory region requirements of all of them,
and are aware of the behavior conflicts that may arise.

[`AsMut`]: core::convert::AsMut
[`BitSlice`]: crate::slice::BitSlice
**/
pub trait AsBitsMut<T>
where T: BitStore
{
	/// Views memory as a slice of mutable bits.
	///
	/// # Type Parameters
	///
	/// - `O`: The bit ordering used for the region.
	///
	/// # Parameters
	///
	/// - `&mut self`: The value that is providing a [`BitSlice`] view.
	///
	/// # Returns
	///
	/// A mutable view into some bits.
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	fn as_bits_mut<O>(&mut self) -> &mut BitSlice<O, T>
	where O: BitOrder;
}

#[cfg(not(tarpaulin_include))]
impl<A, T> AsBits<T> for A
where
	A: AsRef<[T]>,
	T: BitStore + BitRegister,
{
	#[inline]
	fn as_bits<O>(&self) -> &BitSlice<O, T>
	where O: BitOrder {
		self.as_ref().view_bits::<O>()
	}
}

#[cfg(not(tarpaulin_include))]
impl<A, T> AsBitsMut<T> for A
where
	A: AsMut<[T]>,
	T: BitStore + BitRegister,
{
	#[inline]
	fn as_bits_mut<O>(&mut self) -> &mut BitSlice<O, T>
	where O: BitOrder {
		self.as_mut().view_bits_mut::<O>()
	}
}

#[cfg(test)]
mod tests {
	use crate::prelude::*;

	#[test]
	fn impls() {
		let mut byte = 0u8;
		let mut bytes = [0u8; 2];
		assert!(byte.view_bits::<LocalBits>().not_any());
		assert!(byte.view_bits_mut::<LocalBits>().not_any());
		assert!(bytes.view_bits::<LocalBits>().not_any());
		assert!(bytes.view_bits_mut::<LocalBits>().not_any());
		assert!(bytes[..].view_bits::<LocalBits>().not_any());
		assert!(bytes[..].view_bits_mut::<LocalBits>().not_any());

		let mut blank: [u8; 0] = [];
		assert!(blank.view_bits::<LocalBits>().is_empty());
		assert!(blank.view_bits_mut::<LocalBits>().is_empty());

		assert_eq!(<u8 as BitView>::const_bits(), 8);
		assert_eq!(<u16 as BitView>::const_bits(), 16);
		assert_eq!(<u32 as BitView>::const_bits(), 32);

		#[cfg(target_pointer_width = "64")]
		{
			assert_eq!(<u64 as BitView>::const_bits(), 64);
		}
	}
}
