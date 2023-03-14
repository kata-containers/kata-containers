/*! A dynamically-allocated, fixed-size, buffer containing a [`BitSlice`]
region.

You can read the standard library’s [`alloc::boxed` module documentation][std]
here.

This module defines the [`BitBox`] buffer, and all of its associated support
code.

[`BitBox`] is equivalent to `Box<[bool]>`, in its operation and in its
relationship to the [`BitSlice`] and [`BitVec`] types. Most of the interesting
work to be done on a bit-sequence is implemented in `BitSlice`, to which
`BitBox` dereferences, and the box container itself only exists to maintain
wonership and provide some specializations that cannot safely be done on
`BitSlice` alone.

There is almost never a reason to use this type, as it is a mixture of
[`BitArray`]’s fixed width and [`BitVec`]’s heap allocation. You should only use
it when you have a bit-sequence whose width is either unknowable at compile-time
or inexpressable in `BitArray`, and are constructing the sequence in a `BitVec`
before freezing it.

[`BitArray`]: crate::array::BitArray
[`BitBox`]: crate::boxed::BitBox
[`BitSlice`]: crate::slice::BitSlice
[`BitVec`]: crate::vec::BitVec
[std]: alloc::boxed
!*/

#![cfg(feature = "alloc")]

use crate::{
	index::BitIdx,
	mem::BitMemory,
	mutability::Mut,
	order::{
		BitOrder,
		Lsb0,
	},
	ptr::{
		BitPtr,
		BitSpan,
	},
	slice::BitSlice,
	store::BitStore,
	vec::BitVec,
};

use alloc::boxed::Box;

use core::{
	mem::ManuallyDrop,
	slice,
};

use tap::pipe::Pipe;

/** A frozen heap-allocated buffer of individual bits.

This is essentially a [`BitVec`] that has frozen its allocation, and given up
the ability to change size. It is analagous to `Box<[bool]>`. You should prefer
[`BitArray`] over `BitBox` where possible, and may freely box it if you need the
indirection.

# Documentation

All APIs that mirror something in the standard library will have an `Original`
section linking to the corresponding item. All APIs that have a different
signature or behavior than the original will have an `API Differences` section
explaining what has changed, and how to adapt your existing code to the change.

These sections look like this:

# Original

[`Box<[T]>`](alloc::boxed::Box)

# API Differences

The buffer type `Box<[bool]>` has no type parameters. `BitBox<O, T>` has the
same two type parameters as `BitSlice<O, T>`. Otherwise, `BitBox` is able to
implement the full API surface of `Box<[bool]>`.

# Behavior

Because `BitBox` is a fully-owned buffer, it is able to operate on its memory
without concern for any other views that may alias. This enables it to
specialize some [`BitSlice`] behavior to be faster or more efficient.

# Type Parameters

This takes the same [`BitOrder`] and [`BitStore`] parameters as [`BitSlice`].
Unlike `BitSlice`, it is restricted to only accept the fundamental integers as
its `BitStore` arguments; `BitBox` buffers can never be aliased by other
`BitBox`es, and do not need to share memory access.

# Safety

`BitBox` is a wrapper over a `NonNull<BitSlice<O, T>>` pointer; this allows it
to remain exactly two words in size, and means that it is subject to the same
representational incompatibility restrictions as [`BitSlice`] references. You
must never attempt to type-cast between `Box<[bool]>` and `BitBox` in any way,
nor may you attempt to modify the memory value of a `BitBox` handle. Doing so
will cause allocator and memory errors in your program, likely inducing a panic.

Everything in the `BitBox` public API, even the `unsafe` parts, are guaranteed
to have no more unsafety or potential for incorrectness than their equivalent
items in the standard library. All `unsafe` APIs will have documentation
explicitly detailing what the API requires you to uphold in order for it to
function safely and correctly. All safe APIs will do so themselves.

# Macro Construction

Heap allocation can only occur at runtime, but the [`bitbox!`] macro will
construct an appropriate [`BitSlice`] buffer at compile-time, and at run-time,
only copy the buffer into a heap allocation.

[`BitArray`]: crate::array::BitArray
[`BitOrder`]: crate::order::BitOrder
[`BitSlice`]: crate::slice::BitSlice
[`BitStore`]: crate::store::BitStore
[`BitVec`]: crate::vec::BitVec
[`bitbox!`]: macro@crate::bitbox
**/
#[repr(transparent)]
pub struct BitBox<O = Lsb0, T = usize>
where
	O: BitOrder,
	T: BitStore,
{
	bitspan: BitSpan<Mut, O, T>,
}

/// General-purpose functions not present on `Box<[T]>`.
impl<O, T> BitBox<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// Copies a [`BitSlice`] region into a new `BitBox` allocation.
	///
	/// # Effects
	///
	/// This delegates to [`BitVec::from_bitslice`], then discards the excess
	/// capacity.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![0, 1, 0, 1, 1, 0, 1, 1];
	/// let bb = BitBox::from_bitslice(&bits[2 ..]);
	/// assert_eq!(bb, bits[2 ..]);
	/// assert_eq!(bb.as_slice(), bits.as_slice());
	/// ```
	///
	/// [`BitVec::from_bitslice`]: crate::vec::BitVec::from_bitslice
	pub fn from_bitslice(slice: &BitSlice<O, T>) -> Self {
		BitVec::from_bitslice(slice).into_boxed_bitslice()
	}

	/// Converts a `Box<[T]>` into a `BitBox`<O, T>` without copying its buffer.
	///
	/// # Parameters
	///
	/// - `boxed`: A boxed slice to view as bits.
	///
	/// # Returns
	///
	/// A `BitBox` over the `boxed` buffer.
	///
	/// # Panics
	///
	/// This panics if `boxed` is too long to convert into a `BitBox`. See
	/// [`BitSlice::MAX_ELTS`].
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let boxed: Box<[u8]> = Box::new([0; 4]);
	/// let addr = boxed.as_ptr();
	/// let bb = BitBox::<LocalBits, _>::from_boxed_slice(boxed);
	/// assert_eq!(bb, bits![0; 32]);
	/// assert_eq!(addr, bb.as_slice().as_ptr());
	/// ```
	///
	/// [`BitSlice::MAX_ELTS`]: crate::slice::BitSlice::MAX_ELTS
	pub fn from_boxed_slice(boxed: Box<[T]>) -> Self {
		Self::try_from_boxed_slice(boxed)
			.expect("Slice was too long to be converted into a `BitBox`")
	}

	/// Converts a `Box<[T]>` into a `BitBox<O, T>` without copying its buffer.
	///
	/// This method takes ownership of a memory buffer and enables it to be used
	/// as a bit-box. Because `Box<[T]>` can be longer than `BitBox`es, this is
	/// a fallible method, and the original box will be returned if it cannot be
	/// converted.
	///
	/// # Parameters
	///
	/// - `boxed`: Some boxed slice of memory, to be viewed as bits.
	///
	/// # Returns
	///
	/// If `boxed` is short enough to be viewed as a `BitBox`, then this returns
	/// a `BitBox` over the `boxed` buffer. If `boxed` is too long, then this
	/// returns `boxed` unmodified.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let boxed: Box<[u8]> = Box::new([0; 4]);
	/// let addr = boxed.as_ptr();
	/// let bb = BitBox::<LocalBits, _>::try_from_boxed_slice(boxed).unwrap();
	/// assert_eq!(bb[..], bits![0; 32]);
	/// assert_eq!(addr, bb.as_slice().as_ptr());
	/// ```
	pub fn try_from_boxed_slice(boxed: Box<[T]>) -> Result<Self, Box<[T]>> {
		let mut boxed = ManuallyDrop::new(boxed);

		BitPtr::from_mut_slice(&mut boxed[..])
			.span(boxed.len() * T::Mem::BITS as usize)
			.map(|bitspan| Self { bitspan })
			.map_err(|_| ManuallyDrop::into_inner(boxed))
	}

	/// Converts the slice back into an ordinary slice of memory elements.
	///
	/// This does not affect the slice’s buffer, only the handle used to control
	/// it.
	///
	/// # Parameters
	///
	/// - `self`
	///
	/// # Returns
	///
	/// An ordinary boxed slice containing all of the bit-slice’s memory buffer.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bb = bitbox![0; 5];
	/// let addr = bb.as_slice().as_ptr();
	/// let boxed = bb.into_boxed_slice();
	/// assert_eq!(boxed[..], [0][..]);
	/// assert_eq!(addr, boxed.as_ptr());
	/// ```
	pub fn into_boxed_slice(self) -> Box<[T]> {
		self.pipe(ManuallyDrop::new)
			.as_mut_slice()
			.pipe(|slice| unsafe { Box::from_raw(slice) })
	}

	/// Converts `self` into a vector without clones or allocation.
	///
	/// The resulting vector can be converted back into a box via [`BitVec<O,
	/// T>`]’s [`.into_boxed_bitslice()`] method.
	///
	/// # Original
	///
	/// [`slice::into_vec`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.into_vec)
	///
	/// # API Differences
	///
	/// Despite taking a `Box<[T]>` receiver, this function is written in an
	/// `impl<T> [T]` block.
	///
	/// Rust does not allow the text
	///
	/// ```rust,ignore
	/// impl<O, T> BitSlice<O, T> {
	///   fn into_bitvec(self: BitBox<O, T>);
	/// }
	/// ```
	///
	/// to be written, and `BitBox` exists specifically because
	/// `Box<BitSlice<>>` cannot be written either, so this function must be
	/// implemented directly on `BitBox` rather than on `BitSlice` with a boxed
	/// receiver.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bb = bitbox![0, 1, 0, 1];
	/// let bv = bb.into_bitvec();
	///
	/// assert_eq!(bv, bitvec![0, 1, 0, 1]);
	/// ```
	///
	/// [`BitVec<O, T>`]: crate::vec::BitVec
	/// [`.into_boxed_bitslice()`]: crate::vec::BitVec::into_boxed_bitslice
	pub fn into_bitvec(self) -> BitVec<O, T> {
		let mut bitspan = self.bitspan;
		let mut raw = self
			//  Disarm the `self` destructor
			.pipe(ManuallyDrop::new)
			//  Extract the `Box<[T]>` handle, invalidating `self`
			.with_box(|b| unsafe { ManuallyDrop::take(b) })
			//  The distribution guarantees this to be correct and in-place.
			.into_vec()
			//  Disarm the `Vec<T>` destructor *also*.
			.pipe(ManuallyDrop::new);
		/* The distribution claims that `[T]::into_vec(Box<[T]>) -> Vec<T>` does
		not alter the address of the heap allocation, and only modifies the
		buffer handle. Nevertheless, update the bit-pointer with the address of
		the vector as returned by this transformation Just In Case.

		Inspection of the distribution’s implementation shows that the
		conversion from `(buf, len)` to `(buf, cap, len)` is done by using the
		slice length as the buffer capacity. However, this is *not* a behavior
		guaranteed by the distribution, and so the pipeline above must remain in
		place in the event that this behavior ever changes. It should compile
		away to nothing, as it is almost entirely typesystem manipulation.
		*/
		unsafe {
			bitspan.set_address(raw.as_mut_ptr());
			BitVec::from_fields(bitspan, raw.capacity())
		}
	}

	/// Views the buffer’s contents as a `BitSlice`.
	///
	/// This is equivalent to `&bb[..]`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bb = bitbox![0, 1, 1, 0];
	/// let bits = bb.as_bitslice();
	/// ```
	pub fn as_bitslice(&self) -> &BitSlice<O, T> {
		self.bitspan.to_bitslice_ref()
	}

	/// Extracts a mutable bit-slice of the entire vector.
	///
	/// Equivalent to `&mut bv[..]`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![0, 1, 0, 1];
	/// let bits = bv.as_mut_bitslice();
	/// bits.set(0, true);
	/// ```
	pub fn as_mut_bitslice(&mut self) -> &mut BitSlice<O, T> {
		self.bitspan.to_bitslice_mut()
	}

	/// Extracts an element slice containing the entire box.
	///
	/// # Analogue
	///
	/// See [`.as_bitslice()`] for a `&BitBox -> &BitSlice` transform.
	///
	/// # Examples
	///
	/// ```rust
	/// # #[cfg(feature = "std")] {
	/// use bitvec::prelude::*;
	/// use std::io::{self, Write};
	/// let buffer = bitbox![Msb0, u8; 0, 1, 0, 1, 1, 0, 0, 0];
	/// io::sink().write(buffer.as_slice()).unwrap();
	/// # }
	/// ```
	///
	/// [`.as_bitslice()`]: Self::as_bitslice
	pub fn as_slice(&self) -> &[T] {
		let (data, len) =
			(self.bitspan.address().to_const(), self.bitspan.elements());
		unsafe { slice::from_raw_parts(data, len) }
	}

	/// Extracts a mutable slice of the entire box.
	///
	/// # Analogue
	///
	/// See [`.as_mut_bitslice()`] for a `&mut BitBox -> &mut BitSlice`
	/// transform.
	///
	/// # Examples
	///
	/// ```rust
	/// # #[cfg(feature = "std")] {
	/// use bitvec::prelude::*;
	/// use std::io::{self, Read};
	/// let mut buffer = bitbox![Msb0, u8; 0; 24];
	/// io::repeat(0b101).read_exact(buffer.as_mut_slice()).unwrap();
	/// # }
	/// ```
	///
	/// [`.as_mut_bitslice()`]: Self::as_mut_bitslice
	pub fn as_mut_slice(&mut self) -> &mut [T] {
		let (data, len) =
			(self.bitspan.address().to_mut(), self.bitspan.elements());
		unsafe { slice::from_raw_parts_mut(data, len) }
	}

	/// Sets the uninitialized bits of the vector to a fixed value.
	///
	/// This method modifies all bits in the allocated buffer that are outside
	/// the `self.as_bitslice()` view so that they have a consistent value. This
	/// can be used to zero the uninitialized memory so that when viewed as a
	/// raw memory slice, bits outside the live region have a predictable value.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bb = BitBox::new(&220u8.view_bits::<Lsb0>()[.. 4]);
	/// assert_eq!(bb.count_ones(), 2);
	/// assert_eq!(bb.as_slice(), &[220u8]);
	///
	/// bb.set_uninitialized(false);
	/// assert_eq!(bb.as_slice(), &[12u8]);
	///
	/// bb.set_uninitialized(true);
	/// assert_eq!(bb.as_slice(), &[!3u8]);
	/// ```
	pub fn set_uninitialized(&mut self, value: bool) {
		let mut bp = self.bitspan;
		let (_, head, bits) = bp.raw_parts();
		let head = head.value() as usize;
		let tail = head + bits;
		let full = crate::mem::elts::<T::Mem>(tail) * T::Mem::BITS as usize;
		unsafe {
			bp.set_head(BitIdx::ZERO);
			bp.set_len(full);
			let bits = bp.to_bitslice_mut();
			bits.get_unchecked_mut(.. head).set_all(value);
			bits.get_unchecked_mut(tail ..).set_all(value);
		}
	}

	/// Permits a function to modify the `Box<[T]>` backing storage of a
	/// `BitBox<_, T>`.
	///
	/// This produces a temporary `Box<[T]>` structure governing the `BitBox`’s
	/// buffer and allows a function to view it mutably. After the
	/// callback returns, the `Box` is written back into `self` and forgotten.
	///
	/// # Type Parameters
	///
	/// - `F`: A function which operates on a mutable borrow of a `Box<[T]>`
	///   buffer controller.
	/// - `R`: The return type of the `F` function.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `func`: A function which receives a mutable borrow of a `Box<[T]>`
	///   controlling `self`’s buffer.
	///
	/// # Returns
	///
	/// The return value of `func`. `func` is forbidden from borrowing any part
	/// of the `Box<[T]>` temporary view.
	fn with_box<F, R>(&mut self, func: F) -> R
	where F: FnOnce(&mut ManuallyDrop<Box<[T]>>) -> R {
		self.as_mut_slice()
			.pipe(|raw| unsafe { Box::from_raw(raw) })
			.pipe(ManuallyDrop::new)
			.pipe_ref_mut(func)
	}
}

mod api;
mod ops;
mod traits;

#[cfg(test)]
mod tests;
