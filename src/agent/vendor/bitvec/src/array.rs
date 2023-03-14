/*! A statically-allocated, fixed-size, buffer containing a [`BitSlice`] region.

You can read the language’s [array fundamental documentation][std] here.

This module defines the [`BitArray`] immediate type, and its associated support
code.

[`BitArray`] is equivalent to `[bool; N]`, in its operation and in its
relationship to the [`BitSlice`] type. It has little behavior or properties in
its own right, and serves solely as a type capable of being used in immediate
value position, and delegates to `BitSlice` for all actual work.

[`BitArray`]: crate::array::BitArray
[`BitSlice`]: crate::slice::BitSlice
[std]: https://doc.rust-lang.org/stable/std/primitive.array.html
!*/

use crate::{
	order::{
		BitOrder,
		Lsb0,
	},
	slice::BitSlice,
	view::BitView,
};

use core::{
	marker::PhantomData,
	mem::MaybeUninit,
	slice,
};

/* Note on C++ `std::bitset<N>` compatibility:

The ideal API for `BitArray` is as follows:

```rust
struct BitArray<O, T, const N: usize>
where
  O: BitOrder,
  T: BitStore,
  N < T::MAX_BITS,
{
  _ord: PhantomData<O>,
  data: [T; crate::mem::elts::<T>(N)],
}

impl<O, T, const N: usize> BitArray<O, T, N>
where
  O: BitOrder,
  T: BitStore,
{
  pub fn len(&self) -> usize { N }
}
```

This allows the structure to be parametric over the number of bits, rather than
a scalar or array type that satisfies the number of bits. Unfortunately, it is
inexpressible until the Rust compiler’s const-evaluation engine permits using
numeric type parameters in type-level expressions.
*/

/** An array of individual bits, able to be held by value on the stack.

This type is generic over all [`Sized`] implementors of the [`BitView`] trait.
Due to limitations in the Rust language’s const-generics implementation (it is
both unstable and incomplete), this must take an array type parameter directly,
rather than register type and bit-count integer parameters. This makes it less
convenient to use than C++’s [`std::bitset<N>`] array type. The [`bitarr!`]
macro is capable of constructing both values and specific types of `BitArray`,
and this macro should be preferred for most use.

The advantage of using this wrapper is that it implements [`Deref`]/[`Mut`] to
[`BitSlice`], as well as implementing all of `BitSlice`s traits by forwarding to
the `BitSlice` view of its contained data. This allows it to have `BitSlice`
behavior by itself, without requiring explicit [`.as_bitslice()`] calls in user
code.

# Limitations

This does not track start or end indices of its [`BitSlice`] view, and so that
view will always fully span the buffer. You cannot produce, for example, an
array of twelve bits.

# Type Parameters

- `O`: The ordering of bits within memory registers.
- `V`: Some buffer which can be used as the basis for a [`BitSlice`] view. This
  will usually be an array of `[T: BitRegister; N]`.

# Examples

This type is useful for marking that some value is always to be used as a
[`BitSlice`].
**/
///
/// ```rust
/// use bitvec::prelude::*;
///
/// struct HasBitfields {
///   header: u32,
///   // creates a type declaration.
///   fields: bitarr!(for 20, in Msb0, u8),
/// }
///
/// impl HasBitfields {
///   pub fn new() -> Self {
///     Self {
///       header: 0,
///       // creates a value object.
///       // the type paramaters must be repeated.
///       fields: bitarr![Msb0, u8; 0; 20],
///     }
///   }
///
///   /// Access a bit region directly
///   pub fn get_subfield(&self) -> &BitSlice<Msb0, u8> {
///     &self.fields[.. 4]
///   }
///
///   /// Read a 12-bit value out of a region
///   pub fn read_value(&self) -> u16 {
///     self.fields[4 .. 16].load()
///   }
///
///   /// Write a 12-bit value into a region
///   pub fn set_value(&mut self, value: u16) {
///     self.fields[4 .. 16].store(value);
///   }
/// }
/// ```
/**
# Eventual Obsolescence

When const-generics stabilize, this will be modified to have a signature more
like `BitArray<O, T, const N: usize>([T; elts::<T>(N)]);`, to mirror the
behavior of ordinary arrays `[T; N]` as they stand today.

[`BitSlice`]: crate::slice::BitSlice
[`BitView`]: crate::view::BitView
[`Deref`]: core::ops::Deref
[`Mut`]: core::ops::DerefMut
[`Sized`]: core::marker::Sized
[`bitarr!`]: macro@crate::bitarr
[`std::bitset<N>`]: https://en.cppreference.com/w/cpp/utility/bitset
[`.as_bitslice()`]: Self::as_bitslice
**/
#[repr(transparent)]
pub struct BitArray<O = Lsb0, V = [usize; 1]>
where
	O: BitOrder,
	V: BitView,
{
	/// The ordering of bits within a storage element `V::Store`.
	_ord: PhantomData<O>,
	/// The wrapped data store.
	data: V,
}

impl<O, V> BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	/// Constructs a new `BitArray` with its memory set to zero.
	#[inline]
	pub fn zeroed() -> Self {
		Self {
			_ord: PhantomData,
			data: unsafe { MaybeUninit::zeroed().assume_init() },
		}
	}

	/// Wraps a buffer in a `BitArray`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let data = [0u8; 2];
	/// let bits: BitArray<Msb0, _> = BitArray::new(data);
	/// assert_eq!(bits.len(), 16);
	/// ```
	#[inline]
	pub fn new(data: V) -> Self {
		Self {
			_ord: PhantomData,
			data,
		}
	}

	/// Removes the `BitArray` wrapper, leaving the contained buffer.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bitarr = bitarr![Lsb0, usize; 0; 30];
	/// let native: [usize; 1] = bitarr.value();
	/// ```
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn value(self) -> V {
		self.data
	}

	/// Views the array as a [`BitSlice`].
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn as_bitslice(&self) -> &BitSlice<O, V::Store> {
		self.data.view_bits::<O>()
	}

	/// Views the array as a mutable [`BitSlice`].
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn as_mut_bitslice(&mut self) -> &mut BitSlice<O, V::Store> {
		self.data.view_bits_mut::<O>()
	}

	/// Views the array as a slice of its underlying memory registers.
	#[inline]
	pub fn as_raw_slice(&self) -> &[V::Store] {
		unsafe {
			slice::from_raw_parts(
				&self.data as *const V as *const V::Store,
				V::const_elts(),
			)
		}
	}

	/// Views the array as a mutable slice of its underlying memory registers.
	#[inline]
	pub fn as_mut_raw_slice(&mut self) -> &mut [V::Store] {
		unsafe {
			slice::from_raw_parts_mut(
				&mut self.data as *mut V as *mut V::Store,
				V::const_elts(),
			)
		}
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "This is renamed to `as_raw_slice`"]
	pub fn as_slice(&self) -> &[V::Store] {
		self.as_raw_slice()
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "This is renamed to `as_mut_raw_slice`"]
	pub fn as_mut_slice(&mut self) -> &mut [V::Store] {
		self.as_mut_raw_slice()
	}

	/// Views the interior buffer.
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn as_buffer(&self) -> &V {
		&self.data
	}

	/// Mutably views the interior buffer.
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn as_mut_buffer(&mut self) -> &mut V {
		&mut self.data
	}
}

mod iter;
mod ops;
mod traits;

pub use self::iter::IntoIter;

#[cfg(test)]
mod tests;
