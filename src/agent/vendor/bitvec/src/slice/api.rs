//! Port of the `[T]` inherent API.

use crate::{
	array::BitArray,
	devel as dvl,
	mem::BitMemory,
	mutability::{
		Const,
		Mut,
	},
	order::BitOrder,
	ptr::{
		BitPtr,
		BitPtrRange,
		BitSpan,
		BitSpanError,
	},
	slice::{
		iter::{
			Chunks,
			ChunksExact,
			ChunksExactMut,
			ChunksMut,
			Iter,
			IterMut,
			RChunks,
			RChunksExact,
			RChunksExactMut,
			RChunksMut,
			RSplit,
			RSplitMut,
			RSplitN,
			RSplitNMut,
			Split,
			SplitMut,
			SplitN,
			SplitNMut,
			Windows,
		},
		BitRef,
		BitSlice,
	},
	store::BitStore,
};

use core::{
	cmp,
	ops::{
		Range,
		RangeBounds,
		RangeFrom,
		RangeFull,
		RangeInclusive,
		RangeTo,
		RangeToInclusive,
	},
};

use tap::{
	pipe::Pipe,
	tap::Tap,
};

#[cfg(feature = "alloc")]
use crate::vec::BitVec;

/// Port of the `[T]` inherent API.
impl<O, T> BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// Returns the number of bits in the slice.
	///
	/// # Original
	///
	/// [`slice::len`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.len)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let a = bits![0, 0, 1];
	/// assert_eq!(a.len(), 3);
	/// ```
	#[inline]
	pub fn len(&self) -> usize {
		self.as_bitspan().len()
	}

	/// Returns `true` if the slice has a length of 0.
	///
	/// # Original
	///
	/// [`slice::is_empty`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.is_empty)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let a = bits![0, 0, 1];
	/// assert!(!a.is_empty());
	/// ```
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.as_bitspan().len() == 0
	}

	/// Returns the first bit of the slice, or `None` if it is empty.
	///
	/// # Original
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![1, 0, 0];
	/// assert_eq!(Some(&true), v.first().as_deref());
	///
	/// let w = bits![];
	/// assert_eq!(None, w.first());
	/// ```
	#[inline]
	pub fn first(&self) -> Option<BitRef<Const, O, T>> {
		self.get(0)
	}

	/// Returns a mutable pointer to the first bit of the slice, or `None`
	/// if it is empty.
	///
	/// # Original
	///
	/// [`slice::first_mut`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.first_mut)
	///
	/// # API Differences
	///
	/// This crate cannot manifest `&mut bool` references, and must use the
	/// [`BitRef`] proxy type where `&mut bool` exists in the standard library
	/// API. The proxy value must be bound as `mut` in order to write through
	/// it.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let x = bits![mut 0, 1, 0];
	///
	/// if let Some(mut first) = x.first_mut() {
	///   *first = true;
	/// }
	/// assert_eq!(x, bits![1, 1, 0]);
	/// ```
	///
	/// [`BitRef`]: crate::ptr::BitRef
	#[inline]
	pub fn first_mut(&mut self) -> Option<BitRef<Mut, O, T>> {
		self.get_mut(0)
	}

	/// Returns the first and all the rest of the bits of the slice, or
	/// `None` if it is empty.
	///
	/// # Original
	///
	/// [`slice::split_first`](https://doc.rust-lang.org/stable/std/primitive.slice.html#split_first)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let x = bits![1, 0, 0];
	///
	/// if let Some((first, rest)) = x.split_first() {
	///   assert_eq!(first, &true);
	///   assert_eq!(rest, bits![0; 2]);
	/// }
	/// # fn end_the_block() {}
	/// ```
	#[inline]
	pub fn split_first(&self) -> Option<(BitRef<Const, O, T>, &Self)> {
		match self.len() {
			0 => None,
			_ => unsafe {
				let (head, rest) = self.split_at_unchecked(1);
				Some((head.get_unchecked(0), rest))
			},
		}
	}

	/// Returns the first and all the rest of the bits of the slice, or
	/// `None` if it is empty.
	///
	/// # Original
	///
	/// [`slice::split_first_mut`](https://doc.rust-lang.org/stable/std/primitive.slice.html#split_first_mut)
	///
	/// # API Differences
	///
	/// This crate cannot manifest `&mut bool` references, and must use the
	/// [`BitRef`] proxy type where `&mut bool` exists in the standard library
	/// API. The proxy value must be bound as `mut` in order to write through
	/// it.
	///
	/// Because the references are permitted to use the same memory address,
	/// they are marked as aliasing in order to satisfy Rust’s requirements
	/// about freedom from data races.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let x = bits![mut 0, 0, 1];
	///
	/// if let Some((mut first, rest)) = x.split_first_mut() {
	///   *first = true;
	///   rest.set(0, true);
	///   rest.set(1, false);
	/// }
	/// assert_eq!(x, bits![1, 1, 0]);
	/// ```
	///
	/// [`BitRef`]: crate::ptr::BitRef
	//  `pub type Aliased = BitSlice<O, T::Alias>;` is not allowed in inherents,
	//  so this will not be aliased.
	#[inline]
	#[allow(clippy::type_complexity)]
	pub fn split_first_mut(
		&mut self,
	) -> Option<(BitRef<Mut, O, T::Alias>, &mut BitSlice<O, T::Alias>)> {
		match self.len() {
			0 => None,
			_ => unsafe {
				let (head, rest) = self.split_at_unchecked_mut(1);
				Some((head.get_unchecked_mut(0), rest))
			},
		}
	}

	/// Returns the last and all the rest of the bits of the slice, or
	/// `None` if it is empty.
	///
	/// # Original
	///
	/// [`slice::split_last`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.split_last)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let x = bits![0, 0, 1];
	///
	/// if let Some((last, rest)) = x.split_last() {
	///   assert_eq!(last, &true);
	///   assert_eq!(rest, bits![0; 2]);
	/// }
	/// # fn end_the_block() {}
	/// ```
	#[inline]
	pub fn split_last(&self) -> Option<(BitRef<Const, O, T>, &Self)> {
		match self.len() {
			0 => None,
			len => unsafe {
				let (rest, tail) = self.split_at_unchecked(len.wrapping_sub(1));
				Some((tail.get_unchecked(0), rest))
			},
		}
	}

	/// Returns the last and all the rest of the bits of the slice, or
	/// `None` if it is empty.
	///
	/// # Original
	///
	/// [`slice::split_last_mut`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.split_last_mut)
	///
	/// # API Differences
	///
	/// This crate cannot manifest `&mut bool` references, and must use the
	/// [`BitRef`] proxy type where `&mut bool` exists in the standard library
	/// API. The proxy value must be bound as `mut` in order to write through
	/// it.
	///
	/// Because the references are permitted to use the same memory address,
	/// they are marked as aliasing in order to satisfy Rust’s requirements
	/// about freedom from data races.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let x = bits![mut 1, 0, 0];
	///
	/// if let Some((mut last, rest)) = x.split_last_mut() {
	///   *last = true;
	///   rest.set(0, false);
	///   rest.set(1, true);
	/// }
	/// assert_eq!(x, bits![0, 1, 1]);
	/// ```
	///
	/// [`BitRef`]: crate::slice::BitSlice
	//  `pub type Aliased = BitSlice<O, T::Alias>;` is not allowed in inherents,
	//  so this will not be aliased.
	#[inline]
	#[allow(clippy::type_complexity)]
	pub fn split_last_mut(
		&mut self,
	) -> Option<(BitRef<Mut, O, T::Alias>, &mut BitSlice<O, T::Alias>)> {
		match self.len() {
			0 => None,
			len => unsafe {
				let (rest, tail) = self.split_at_unchecked_mut(len - 1);
				Some((tail.get_unchecked_mut(0), rest))
			},
		}
	}

	/// Returns the last bit of the slice, or `None` if it is empty.
	///
	/// # Original
	///
	/// [`slice::last`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.last)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![0, 0, 1];
	/// assert_eq!(Some(&true), v.last().as_deref());
	///
	/// let w = bits![];
	/// assert_eq!(None, w.last());
	/// ```
	#[inline]
	pub fn last(&self) -> Option<BitRef<Const, O, T>> {
		match self.len() {
			0 => None,
			len => Some(unsafe { self.get_unchecked(len - 1) }),
		}
	}

	/// Returns a mutable pointer to the last bit in the slice.
	///
	/// # Original
	///
	/// [`slice::last_mut`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.last_mut)
	///
	/// # API Differences
	///
	/// This crate cannot manifest `&mut bool` references, and must use the
	/// [`BitRef`] proxy type where `&mut bool` exists in the standard library
	/// API. The proxy value must be bound as `mut` in order to write through
	/// it.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let x = bits![mut 0, 1, 0];
	///
	/// if let Some(mut last) = x.last_mut() {
	///   *last = true;
	/// }
	/// assert_eq!(x, bits![0, 1, 1]);
	/// ```
	///
	/// [`BitRef`]: crate::ptr::BitRef
	#[inline]
	pub fn last_mut(&mut self) -> Option<BitRef<Mut, O, T>> {
		match self.len() {
			0 => None,
			len => Some(unsafe { self.get_unchecked_mut(len - 1) }),
		}
	}

	/// Returns a reference to a bit or subslice depending on the type of index.
	///
	/// - If given a position, returns a reference to the bit at that position
	///   or `None` if out of bounds.
	/// - If given a range, returns the subslice corresponding to that range, or
	///   `None` if out of bounds.
	///
	/// # Original
	///
	/// [`slice::get`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.get)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![0, 1, 0];
	/// assert_eq!(Some(&true), v.get(1).as_deref());
	/// assert_eq!(Some(bits![0, 1]), v.get(0 .. 2));
	/// assert_eq!(None, v.get(3));
	/// assert_eq!(None, v.get(0 .. 4));
	/// ```
	#[inline]
	pub fn get<'a, I>(&'a self, index: I) -> Option<I::Immut>
	where I: BitSliceIndex<'a, O, T> {
		index.get(self)
	}

	/// Returns a mutable reference to a bit or subslice depending on the type
	/// of index (see [`.get()`]) or `None` if the index is out of bounds.
	///
	/// # Original
	///
	/// [`slice::get_mut`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.get_mut)
	///
	/// # API Differences
	///
	/// This crate cannot manifest `&mut bool` references, and must use the
	/// [`BitRef`] proxy type where `&mut bool` exists in the standard library
	/// API. The proxy value must be bound as `mut` in order to write through
	/// it.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let x = bits![mut 0, 0, 1];
	///
	/// if let Some(mut bit) = x.get_mut(1) {
	///   *bit = true;
	/// }
	/// assert_eq!(x, bits![0, 1, 1]);
	/// ```
	///
	/// [`BitRef`]: crate::ptr::BitRef
	/// [`.get()`]: Self::get
	#[inline]
	pub fn get_mut<'a, I>(&'a mut self, index: I) -> Option<I::Mut>
	where I: BitSliceIndex<'a, O, T> {
		index.get_mut(self)
	}

	/// Returns a reference to a bit or subslice, without doing bounds checking.
	///
	/// This is generally not recommended; use with caution! Calling this method
	/// with an out-of-bounds index is *[undefined behavior]* even if the
	/// resulting reference is not used. For a safe alternative, see [`.get()`].
	///
	/// # Original
	///
	/// [`slice::get_unchecked`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.get_unchecked)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let x = bits![0, 1, 0];
	///
	/// unsafe {
	///   assert_eq!(x.get_unchecked(1), &true);
	/// }
	/// ```
	///
	/// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
	/// [`.get()`]: Self::get
	#[inline]
	#[allow(clippy::missing_safety_doc)]
	pub unsafe fn get_unchecked<'a, I>(&'a self, index: I) -> I::Immut
	where I: BitSliceIndex<'a, O, T> {
		index.get_unchecked(self)
	}

	/// Returns a mutable reference to a bit or subslice, without doing bounds
	/// checking.
	///
	/// This is generally not recommended; use with caution! Calling this method
	/// with an out-of-bounds index is *[undefined behavior]* even if the
	/// resulting reference is not used. For a safe alternative, see
	/// [`.get_mut()`].
	///
	/// # Original
	///
	/// [`slice::get_unchecked_mut`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.get_unchecked_mut)
	///
	/// # API Differences
	///
	/// This crate cannot manifest `&mut bool` references, and must use the
	/// [`BitRef`] proxy type where `&mut bool` exists in the standard library
	/// API. The proxy value must be bound as `mut` in order to write through
	/// it.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let x = bits![mut 0; 3];
	/// unsafe {
	///   let mut bit = x.get_unchecked_mut(1);
	///   *bit = true;
	/// }
	/// assert_eq!(x, bits![0, 1, 0]);
	/// ```
	///
	/// [`BitRef`]: crate::ptr::BitRef
	/// [`get_mut`]: Self::get_mut
	/// [undefined behavior]: ../../reference/behavior-considered-undefined.html
	#[inline]
	#[allow(clippy::missing_safety_doc)]
	pub unsafe fn get_unchecked_mut<'a, I>(&'a mut self, index: I) -> I::Mut
	where I: BitSliceIndex<'a, O, T> {
		index.get_unchecked_mut(self)
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Use `as_bitptr` to access the region pointer"]
	pub fn as_ptr(&self) -> BitPtr<Const, O, T> {
		self.as_bitptr()
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Use `as_bitptr_range` to access the region pointers"]
	pub fn as_ptr_range(&self) -> BitPtrRange<Const, O, T> {
		self.as_bitptr_range()
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Use `as_mut_bitptr` to access the region pointer"]
	pub fn as_mut_ptr(&mut self) -> BitPtr<Mut, O, T> {
		self.as_mut_bitptr()
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Use `as_mut_bitptr_range` to access the region pointers"]
	pub fn as_mut_ptr_range(&mut self) -> BitPtrRange<Mut, O, T> {
		self.as_mut_bitptr_range()
	}

	/// Swaps two bits in the slice.
	///
	/// # Original
	///
	/// [`slice::swap`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.swap)
	///
	/// # Arguments
	///
	/// - `a`: The index of the first bit
	/// - `b`: The index of the second bit
	///
	/// # Panics
	///
	/// Panics if `a` or `b` are out of bounds.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![mut 0, 1, 1, 0];
	/// v.swap(1, 3);
	/// assert_eq!(v, bits![0, 0, 1, 1]);
	/// ```
	#[inline]
	pub fn swap(&mut self, a: usize, b: usize) {
		self.assert_in_bounds(a);
		self.assert_in_bounds(b);
		unsafe {
			self.swap_unchecked(a, b);
		}
	}

	/// Reverses the order of bits in the slice, in place.
	///
	/// # Original
	///
	/// [`slice::reverse`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.reverse)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![mut 0, 1, 1];
	/// v.reverse();
	/// assert_eq!(v, bits![1, 1, 0]);
	/// ```
	#[inline]
	pub fn reverse(&mut self) {
		/* This would be better written as a recursive algorithm that swaps the
		edge bits and recurses on `[1 .. len - 1]`, but Rust does not guarantee
		tail-call optimization, and manual iteration allows for slight
		performance optimization on the range reduction.

		Begin with raw pointer manipulation. That’s how you know this is a good
		function.
		*/
		let mut bitspan = self.as_mut_bitspan();
		//  The length does not need to be encoded into, and decoded back out
		//  of, the pointer at each iteration. It is just a loop counter.
		let mut len = bitspan.len();
		//  Reversing 1 or 0 bits has no effect.
		while len > 1 {
			unsafe {
				//  Bring `len` from one past the last to the last exactly.
				len -= 1;
				//  Swap the 0 and last indices.
				bitspan.to_bitslice_mut().swap_unchecked(0, len);

				//  Move the pointer upwards by one bit.
				bitspan.incr_head();
				//  `incr_head` slides the tail up by one, so decrease it again.
				len -= 1;

				//  TODO(myrrlyn): See if range subslicing can be made faster
				//  than this unpacking.
			}
		}
	}

	/// Returns an iterator over the slice.
	///
	/// # Original
	///
	/// [`slice::iter`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.iter)
	///
	/// # API Differences
	///
	/// This iterator yields [`BitRef`] proxy references, rather than `&bool`
	/// ordinary references. It does so in order to promote consistency in the
	/// crate, and make switching between immutable and mutable single-bit
	/// access easier.
	///
	/// The produced iterator has a [`by_ref`] adapter that yields `&bool`
	/// references, and a [`by_val`] adapter that yields `bool` values. Use
	/// these methods to fit this iterator into APIs that expect ordinary `bool`
	/// inputs.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let x = bits![0, 0, 1];
	/// let mut iterator = x.iter();
	///
	/// assert_eq!(iterator.next().as_deref(), Some(&false));
	/// assert_eq!(iterator.next().as_deref(), Some(&false));
	/// assert_eq!(iterator.next().as_deref(), Some(&true));
	/// assert_eq!(iterator.next().as_deref(), None);
	/// ```
	///
	/// [`BitRef`]: crate::ptr::BitRef
	/// [`by_ref`]: crate::slice::Iter::by_ref
	/// [`by_val`]: crate::slice::Iter::by_val
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn iter(&self) -> Iter<O, T> {
		Iter::new(self)
	}

	/// Returns an iterator that allows modifying each bit.
	///
	/// # Original
	///
	/// [`slice::iter_mut`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.iter_mut)
	///
	/// # API Differences
	///
	/// This crate cannot manifest `&mut bool` references, and must use the
	/// [`BitRef`] proxy type where `&mut bool` exists in the standard library
	/// API. The proxy value must be bound as `mut` in order to write through
	/// it.
	///
	/// This iterator marks each yielded item as aliased, as iterators can be
	/// used to yield multiple items into the same scope. If you are using
	/// the iterator in a manner that ensures that all yielded items have
	/// disjoint lifetimes, you can use the [`.remove_alias()`] adapter on it to
	/// remove the alias marker from the yielded subslices.
	///
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let x = bits![mut 0, 0, 1];
	/// for mut bit in x.iter_mut() {
	///   *bit = !*bit;
	/// }
	/// assert_eq!(x, bits![1, 1, 0]);
	/// ```
	///
	/// [`BitRef`]: crate::ptr::BitRef
	/// [`.remove_alias()`]: crate::slice::IterMut::remove_alias
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn iter_mut(&mut self) -> IterMut<O, T> {
		IterMut::new(self)
	}

	/// Returns an iterator over all contiguous windows of length `size`. The
	/// windows overlap. If the slice is shorter than `size`, the iterator
	/// returns no values.
	///
	/// # Original
	///
	/// [`slice::windows`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.windows)
	///
	/// # Panics
	///
	/// Panics if `size` is 0.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let slice = bits![0, 0, 1, 1];
	/// let mut iter = slice.windows(2);
	/// assert_eq!(iter.next().unwrap(), bits![0; 2]);
	/// assert_eq!(iter.next().unwrap(), bits![0, 1]);
	/// assert_eq!(iter.next().unwrap(), bits![1; 2]);
	/// assert!(iter.next().is_none());
	/// ```
	///
	/// If the slice is shorter than `size`:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let slice = bits![0; 3];
	/// let mut iter = slice.windows(4);
	/// assert!(iter.next().is_none());
	/// ```
	#[inline]
	pub fn windows(&self, size: usize) -> Windows<O, T> {
		assert_ne!(size, 0, "Window width cannot be 0");
		Windows::new(self, size)
	}

	/// Returns an iterator over `chunk_size` bits of the slice at a time,
	/// starting at the beginning of the slice.
	///
	/// The chunks are slices and do not overlap. If `chunk_size` does not
	/// divide the length of the slice, then the last chunk will not have length
	/// `chunk_size`.
	///
	/// See [`.chunks_exact()`] for a variant of this iterator that returns
	/// chunks of always exactly `chunk_size` bits, and [`.rchunks()`] for the
	/// same iterator but starting at the end of the slice.
	///
	/// # Original
	///
	/// [`slice::chunks`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.chunks)
	///
	/// # Panics
	///
	/// Panics if `chunk_size` is 0.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let slice = bits![0, 1, 0, 0, 1];
	/// let mut iter = slice.chunks(2);
	/// assert_eq!(iter.next().unwrap(), bits![0, 1]);
	/// assert_eq!(iter.next().unwrap(), bits![0, 0]);
	/// assert_eq!(iter.next().unwrap(), bits![1]);
	/// assert!(iter.next().is_none());
	/// ```
	///
	/// [`.chunks_exact()`]: Self::chunks_exact
	/// [`.rchunks()`]: Self::rchunks
	#[inline]
	pub fn chunks(&self, chunk_size: usize) -> Chunks<O, T> {
		assert_ne!(chunk_size, 0, "Chunk width cannot be 0");
		Chunks::new(self, chunk_size)
	}

	/// Returns an iterator over `chunk_size` bits of the slice at a time,
	/// starting at the beginning of the slice.
	///
	/// The chunks are mutable slices, and do not overlap. If `chunk_size` does
	/// not divide the length of the slice, then the last chunk will not have
	/// length `chunk_size`.
	///
	/// See [`.chunks_exact_mut()`] for a variant of this iterator that returns
	/// chunks of always exactly `chunk_size` bits, and [`.rchunks_mut()`] for
	/// the same iterator but starting at the end of the slice.
	///
	/// # Original
	///
	/// [`slice::chunks_mut`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.chunks_mut)
	///
	/// # API Differences
	///
	/// This iterator marks each yielded item as aliased, as iterators can be
	/// used to yield multiple items into the same scope. If you are using
	/// the iterator in a manner that ensures that all yielded items have
	/// disjoint lifetimes, you can use the [`.remove_alias()`] adapter on it to
	/// remove the alias marker from the yielded subslices.
	///
	/// # Panics
	///
	/// Panics if `chunk_size` is 0.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![mut 0; 5];
	/// let mut count = 1;
	///
	/// for chunk in v.chunks_mut(2) {
	///   for mut bit in chunk.iter_mut() {
	///     *bit = count % 2 == 0;
	///   }
	///   count += 1;
	/// }
	/// assert_eq!(v, bits![0, 0, 1, 1, 0]);
	/// ```
	///
	/// [`.chunks_exact_mut()`]: Self::chunks_exact_mut
	/// [`.rchunks_mut()`]: Self::rchunks_mut
	/// [`.remove_alias()`]: crate::slice::ChunksMut::remove_alias
	#[inline]
	pub fn chunks_mut(&mut self, chunk_size: usize) -> ChunksMut<O, T> {
		assert_ne!(chunk_size, 0, "Chunk width cannot be 0");
		ChunksMut::new(self, chunk_size)
	}

	/// Returns an iterator over `chunk_size` bits of the slice at a time,
	/// starting at the beginning of the slice.
	///
	/// The chunks are slices and do not overlap. If `chunk_size` does not
	/// divide the length of the slice, then the last up to `chunk_size-1` bits
	/// will be omitted and can be retrieved from the [`.remainder()`] method of
	/// the iterator.
	///
	/// Due to each chunk having exactly `chunk_size` bits, the compiler may be
	/// able to optimize the resulting code better than in the case of
	/// [`.chunks()`].
	///
	/// See [`.chunks()`] for a variant of this iterator that also returns the
	/// remainder as a smaller chunk, and [`.rchunks_exact()`] for the same
	/// iterator but starting at the end of the slice.
	///
	/// # Original
	///
	/// [`slice::chunks_exact`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.chunks_exact)
	///
	/// # Panics
	///
	/// Panics if `chunk_size` is 0.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let slice = bits![0, 1, 1, 0, 0];
	/// let mut iter = slice.chunks_exact(2);
	/// assert_eq!(iter.next().unwrap(), bits![0, 1]);
	/// assert_eq!(iter.next().unwrap(), bits![1, 0]);
	/// assert!(iter.next().is_none());
	/// assert_eq!(iter.remainder(), bits![0]);
	/// ```
	///
	/// [`.chunks()`]: Self::chunks
	/// [`.rchunks_exact()`]: Self::rchunks_exact
	/// [`.remainder()`]: crate::slice::ChunksExact::remainder
	#[inline]
	pub fn chunks_exact(&self, chunk_size: usize) -> ChunksExact<O, T> {
		assert_ne!(chunk_size, 0, "Chunk width cannot be 0");
		ChunksExact::new(self, chunk_size)
	}

	/// Returns an iterator over `chunk_size` bits of the slice at a time,
	/// starting at the beginning of the slice.
	///
	/// The chunks are mutable slices, and do not overlap. If `chunk_size` does
	/// not divide the length of the slice, then the last up to `chunk_size-1`
	/// bits will be omitted and can be retrieved from the [`.into_remainder()`]
	/// method of the iterator.
	///
	/// Due to each chunk having exactly `chunk_size` bits, the compiler may be
	/// able to optimize the resulting code better than in the case of
	/// [`.chunks_mut()`].
	///
	/// See [`.chunks_mut()`] for a variant of this iterator that also returns
	/// the remainder as a smaller chunk, and [`.rchunks_exact_mut()`] for the
	/// same iterator but starting at the end of the slice.
	///
	/// # Original
	///
	/// [`slice::chunks_exact_mut`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.chunks_exact_mut)
	///
	/// # API Differences
	///
	/// This iterator marks each yielded item as aliased, as iterators can be
	/// used to yield multiple items into the same scope. If you are using
	/// the iterator in a manner that ensures that all yielded items have
	/// disjoint lifetimes, you can use the [`.remove_alias()`] adapter on it to
	/// remove the alias marker from the yielded subslices.
	///
	/// # Panics
	///
	/// Panics if `chunk_size` is 0.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![mut 0; 5];
	///
	/// for chunk in v.chunks_exact_mut(2) {
	///   chunk.set_all(true);
	/// }
	/// assert_eq!(v, bits![1, 1, 1, 1, 0]);
	/// ```
	///
	/// [`.chunks_mut()`]: Self::chunks_mut
	/// [`.into_remainder()`]: crate::slice::ChunksExactMut::into_remainder
	/// [`.rchunks_exact_mut()`]: Self::rchunks_exact_mut
	/// [`.remove_alias()`]: crate::slice::ChunksExactMut::remove_alias
	#[inline]
	pub fn chunks_exact_mut(
		&mut self,
		chunk_size: usize,
	) -> ChunksExactMut<O, T> {
		assert_ne!(chunk_size, 0, "Chunk width cannot be 0");
		ChunksExactMut::new(self, chunk_size)
	}

	/// Returns an iterator over `chunk_size` bits of the slice at a time,
	/// starting at the end of the slice.
	///
	/// The chunks are slices and do not overlap. If `chunk_size` does not
	/// divide the length of the slice, then the last chunk will not have length
	/// `chunk_size`.
	///
	/// See [`.rchunks_exact()`] for a variant of this iterator that returns
	/// chunks of always exactly `chunk_size` bits, and [`.chunks()`] for the
	/// same iterator but starting at the beginning of the slice.
	///
	/// # Original
	///
	/// [`slice::rchunks`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.rchunks)
	///
	/// # Panics
	///
	/// Panics if `chunk_size` is 0.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let slice = bits![0, 1, 0, 0, 1];
	/// let mut iter = slice.rchunks(2);
	/// assert_eq!(iter.next().unwrap(), bits![0, 1]);
	/// assert_eq!(iter.next().unwrap(), bits![1, 0]);
	/// assert_eq!(iter.next().unwrap(), bits![0]);
	/// assert!(iter.next().is_none());
	/// ```
	///
	/// [`.chunks()`]: Self::chunks
	/// [`.rchunks_exact()`]: Self::rchunks_exact
	#[inline]
	pub fn rchunks(&self, chunk_size: usize) -> RChunks<O, T> {
		assert_ne!(chunk_size, 0, "Chunk width cannot be 0");
		RChunks::new(self, chunk_size)
	}

	/// Returns an iterator over `chunk_size` bits of the slice at a time,
	/// starting at the end of the slice.
	///
	/// The chunks are mutable slices, and do not overlap. If `chunk_size` does
	/// not divide the length of the slice, then the last chunk will not have
	/// length `chunk_size`.
	///
	/// See [`.rchunks_exact_mut()`] for a variant of this iterator that returns
	/// chunks of always exactly `chunk_size` bits, and [`.chunks_mut()`] for
	/// the same iterator but starting at the beginning of the slice.
	///
	/// # Original
	///
	/// [`slice::rchunks_mut`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.rchunks_mut)
	///
	/// # API Differences
	///
	/// This iterator marks each yielded item as aliased, as iterators can be
	/// used to yield multiple items into the same scope. If you are using
	/// the iterator in a manner that ensures that all yielded items have
	/// disjoint lifetimes, you can use the [`.remove_alias()`] adapter on it to
	/// remove the alias marker from the yielded subslices.
	///
	/// # Panics
	///
	/// Panics if `chunk_size` is 0.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![mut 0; 5];
	/// let mut count = 1;
	///
	/// for chunk in v.rchunks_mut(2) {
	///   for mut bit in chunk.iter_mut() {
	///     *bit = count % 2 == 0;
	///   }
	///   count += 1;
	/// }
	/// assert_eq!(v, bits![0, 1, 1, 0, 0]);
	/// ```
	///
	/// [`.chunks_mut()`]: Self::chunks_mut
	/// [`.rchunks_exact_mut()`]: Self::rchunks_exact_mut
	/// [`.remove_alias()`]: crate::slice::RChunksMut::remove_alias
	#[inline]
	pub fn rchunks_mut(&mut self, chunk_size: usize) -> RChunksMut<O, T> {
		assert_ne!(chunk_size, 0, "Chunk width cannot be 0");
		RChunksMut::new(self, chunk_size)
	}

	/// Returns an iterator over `chunk_size` bits of the slice at a time,
	/// starting at the end of the slice.
	///
	/// The chunks are slices and do not overlap. If `chunk_size` does not
	/// divide the length of the slice, then the last up to `chunk_size-1` bits
	/// will be omitted and can be retrieved from the [`.remainder()`] method of
	/// the iterator.
	///
	/// Due to each chunk having exactly `chunk_size` bits, the compiler may be
	/// able to optimize the resulting code better than in the case of
	/// [`.rchunks()`].
	///
	/// See [`.rchunks()`] for a variant of this iterator that also returns the
	/// remainder as a smaller chunk, and [`.chunks_exact()`] for the same
	/// iterator but starting at the beginning of the slice.
	///
	/// # Original
	///
	/// [`slice::rchunks_exact`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.rchunks_exact)
	///
	/// # Panics
	///
	/// Panics if `chunk_size` is 0.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let slice = bits![0, 0, 1, 1, 0];
	/// let mut iter = slice.rchunks_exact(2);
	/// assert_eq!(iter.next().unwrap(), bits![1, 0]);
	/// assert_eq!(iter.next().unwrap(), bits![0, 1]);
	/// assert!(iter.next().is_none());
	/// assert_eq!(iter.remainder(), bits![0]);
	/// ```
	///
	/// [`.chunks_exact()`]: Self::chunks_exact
	/// [`.rchunks()`]: Self::rchunks
	/// [`.remainder()`]: crate::slice::ChunksExact::remainder
	#[inline]
	pub fn rchunks_exact(&self, chunk_size: usize) -> RChunksExact<O, T> {
		assert_ne!(chunk_size, 0, "Chunk width cannot be 0");
		RChunksExact::new(self, chunk_size)
	}

	/// Returns an iterator over `chunk_size` bits of the slice at a time,
	/// starting at the end of the slice.
	///
	/// The chunks are mutable slices, and do not overlap. If `chunk_size` does
	/// not divide the length of the slice, then the last up to `chunk_size-1`
	/// bits will be omitted and can be retrieved from the [`.into_remainder()`]
	/// method of the iterator.
	///
	/// Due to each chunk having exactly `chunk_size` bits, the compiler may be
	/// able to optimize the resulting code better than in the case of
	/// [`.rchunks_mut()`].
	///
	/// See [`.rchunks_mut()`] for a variant of this iterator that also returns
	/// the remainder as a smaller chunk, and [`.chunks_exact_mut()`] for the
	/// same iterator but starting at the beginning of the slice.
	///
	/// # Original
	///
	/// [`slice::rchunks_exact_mut`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.rchunks_exact_mut)
	///
	/// # API Differences
	///
	/// This iterator marks each yielded item as aliased, as iterators can be
	/// used to yield multiple items into the same scope. If you are using
	/// the iterator in a manner that ensures that all yielded items have
	/// disjoint lifetimes, you can use the [`.remove_alias()`] adapter on it to
	/// remove the alias marker from the yielded subslices.
	///
	/// # Panics
	///
	/// Panics if `chunk_size` is 0.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![mut 0; 5];
	///
	/// for chunk in v.rchunks_exact_mut(2) {
	///   chunk.set_all(true);
	/// }
	/// assert_eq!(v, bits![0, 1, 1, 1, 1]);
	/// ```
	///
	/// [`.chunks_exact_mut()`]: Self::chunks_exact_mut
	/// [`.into_remainder()`]: crate::slice::ChunksExactMut::into_remainder
	/// [`.rchunks_mut()`]: Self::rchunks_mut
	/// [`.remove_alias()`]: crate::slice::ChunksExactMut::remove_alias
	#[inline]
	pub fn rchunks_exact_mut(
		&mut self,
		chunk_size: usize,
	) -> RChunksExactMut<O, T> {
		assert_ne!(chunk_size, 0, "Chunk width cannot be 0");
		RChunksExactMut::new(self, chunk_size)
	}

	/// Divides one slice into two at an index.
	///
	/// The first will contain all indices from `[0, mid)` (excluding the index
	/// `mid` itself) and the second will contain all indices from `[mid, len)`
	/// (excluding the index `len` itself).
	///
	/// # Original
	///
	/// [`slice::split_at`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.split_at)
	///
	/// # Panics
	///
	/// Panics if `mid > len`.
	///
	/// # Behavior
	///
	/// When `mid` is `0` or `self.len()`, then the left or right return values,
	/// respectively, are empty slices. Empty slice references produced by this
	/// method are specified to have the address information you would expect:
	/// a left empty slice has the same base address and start bit as `self`,
	/// and a right empty slice will have its address raised by `self.len()`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![0, 0, 0, 1, 1, 1];
	///
	/// {
	///   let (left, right) = v.split_at(0);
	///   assert_eq!(left, bits![]);
	///   assert_eq!(right, v);
	/// }
	///
	/// {
	///   let (left, right) = v.split_at(2);
	///   assert_eq!(left, bits![0, 0]);
	///   assert_eq!(right, bits![0, 1, 1, 1]);
	/// }
	///
	/// {
	///   let (left, right) = v.split_at(6);
	///   assert_eq!(left, v);
	///   assert_eq!(right, bits![]);
	/// }
	/// ```
	#[inline]
	pub fn split_at(&self, mid: usize) -> (&Self, &Self) {
		let len = self.len();
		assert!(mid <= len, "Index {} out of bounds: {}", mid, len);
		unsafe { self.split_at_unchecked(mid) }
	}

	/// Divides one mutable slice into two at an index.
	///
	/// The first will contain all indices from `[0, mid)` (excluding the index
	/// `mid` itself) and the second will contain all indices from `[mid, len)`
	/// (excluding the index `len` itself).
	///
	/// # Original
	///
	/// [`slice::split_at_mut`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.split_at_mut)
	///
	/// # API Differences
	///
	/// The partition index `mid` may occur anywhere in the slice, and as a
	/// result the two returned slices may both have write access to the memory
	/// address containing `mid`. As such, the returned slices must be marked
	/// with [`T::Alias`] in order to correctly manage memory access going
	/// forward.
	///
	/// This marking is applied to all memory accesses in both slices,
	/// regardless of whether any future accesses actually require it. To limit
	/// the alias marking to only the addresses that need it, use
	/// [`.bit_domain()`] or [`.bit_domain_mut()`] to split either slice into
	/// its aliased and unaliased subslices.
	///
	/// # Panics
	///
	/// Panics if `mid > len`.
	///
	/// # Behavior
	///
	/// When `mid` is `0` or `self.len()`, then the left or right return values,
	/// respectively, are empty slices. Empty slice references produced by this
	/// method are specified to have the address information you would expect:
	/// a left empty slice has the same base address and start bit as `self`,
	/// and a right empty slice will have its address raised by `self.len()`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![mut 0, 0, 0, 1, 1, 1];
	/// // scoped to restrict the lifetime of the borrows
	/// {
	///   let (left, right) = v.split_at_mut(2);
	///   assert_eq!(left, bits![0, 0]);
	///   assert_eq!(right, bits![0, 1, 1, 1]);
	///
	///   left.set(1, true);
	///   right.set(1, false);
	/// }
	/// assert_eq!(v, bits![0, 1, 0, 0, 1, 1]);
	/// ```
	///
	/// [`T::Alias`]: crate::store::BitStore::Alias
	/// [`.bit_domain`()]: Self::bit_domain
	/// [`.bit_domain_mut`()]: Self::bit_domain_mut
	//  `pub type Aliased = BitSlice<O, T::Alias>;` is not allowed in inherents,
	//  so this will not be aliased.
	#[inline]
	#[allow(clippy::type_complexity)]
	pub fn split_at_mut(
		&mut self,
		mid: usize,
	) -> (&mut BitSlice<O, T::Alias>, &mut BitSlice<O, T::Alias>) {
		self.assert_in_bounds(mid);
		unsafe { self.split_at_unchecked_mut(mid) }
	}

	/// Returns an iterator over subslices separated by bits that match `pred`.
	/// The matched bit is not contained in the subslices.
	///
	/// # Original
	///
	/// [`slice::split`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.split)
	///
	/// # API Differences
	///
	/// In order to allow more than one bit of information for the split
	/// decision, the predicate receives the index of each bit, as well as its
	/// value.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let slice = bits![0, 1, 1, 0];
	/// let mut iter = slice.split(|pos, _bit| pos % 3 == 2);
	///
	/// assert_eq!(iter.next().unwrap(), bits![0, 1]);
	/// assert_eq!(iter.next().unwrap(), bits![0]);
	/// assert!(iter.next().is_none());
	/// ```
	///
	/// If the first bit is matched, an empty slice will be the first item
	/// returned by the iterator. Similarly, if the last bit in the slice is
	/// matched, an empty slice will be the last item returned by the iterator:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let slice = bits![0, 0, 1];
	/// let mut iter = slice.split(|_pos, bit| *bit);
	///
	/// assert_eq!(iter.next().unwrap(), bits![0, 0]);
	/// assert_eq!(iter.next().unwrap(), bits![]);
	/// assert!(iter.next().is_none());
	/// ```
	///
	/// If two matched bits are directly adjacent, an empty slice will be
	/// present between them:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let slice = bits![1, 0, 0, 1];
	/// let mut iter = slice.split(|_pos, bit| !*bit);
	///
	/// assert_eq!(iter.next().unwrap(), bits![1]);
	/// assert_eq!(iter.next().unwrap(), bits![]);
	/// assert_eq!(iter.next().unwrap(), bits![1]);
	/// assert!(iter.next().is_none());
	/// ```
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn split<F>(&self, pred: F) -> Split<O, T, F>
	where F: FnMut(usize, &bool) -> bool {
		Split::new(self, pred)
	}

	/// Returns an iterator over mutable subslices separated by bits that match
	/// `pred`. The matched bit is not contained in the subslices.
	///
	/// # Original
	///
	/// [`slice::split_mut`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.split_mut)
	///
	/// # API Differences
	///
	/// In order to allow more than one bit of information for the split
	/// decision, the predicate receives the index of each bit, as well as its
	/// value.
	///
	/// This iterator marks each yielded item as aliased, as iterators can be
	/// used to yield multiple items into the same scope. If you are using
	/// the iterator in a manner that ensures that all yielded items have
	/// disjoint lifetimes, you can use the [`.remove_alias()`] adapter on it to
	/// remove the alias marker from the yielded subslices.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![mut 0, 0, 1, 0, 1, 0];
	/// for group in v.split_mut(|_pos, bit| *bit) {
	///   group.set(0, true);
	/// }
	/// assert_eq!(v, bits![1, 0, 1, 1, 1, 1]);
	/// ```
	///
	/// [`.remove_alias()`]: crate::slice::SplitMut::remove_alias
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn split_mut<F>(&mut self, pred: F) -> SplitMut<O, T, F>
	where F: FnMut(usize, &bool) -> bool {
		SplitMut::new(self.alias_mut(), pred)
	}

	/// Returns an iterator over subslices separated by bits that match `pred`,
	/// starting at the end of the slice and working backwards. The matched bit
	/// is not contained in the subslices.
	///
	/// # Original
	///
	/// [`slice::rsplit`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.rsplit)
	///
	/// # API Differences
	///
	/// In order to allow more than one bit of information for the split
	/// decision, the predicate receives the index of each bit, as well as its
	/// value.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let slice = bits![1, 1, 1, 0, 1, 1];
	/// let mut iter = slice.rsplit(|_pos, bit| !*bit);
	///
	/// assert_eq!(iter.next().unwrap(), bits![1; 2]);
	/// assert_eq!(iter.next().unwrap(), bits![1; 3]);
	/// assert!(iter.next().is_none());
	/// ```
	///
	/// As with [`.split()`], if the first or last bit is matched, an empty
	/// slice will be the first (or last) item returned by the iterator.
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![1, 0, 0, 1, 0, 0, 1];
	/// let mut it = v.rsplit(|_pos, bit| *bit);
	/// assert_eq!(it.next().unwrap(), bits![]);
	/// assert_eq!(it.next().unwrap(), bits![0; 2]);
	/// assert_eq!(it.next().unwrap(), bits![0; 2]);
	/// assert_eq!(it.next().unwrap(), bits![]);
	/// assert!(it.next().is_none());
	/// ```
	///
	/// [`.split()`]: Self::split
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn rsplit<F>(&self, pred: F) -> RSplit<O, T, F>
	where F: FnMut(usize, &bool) -> bool {
		RSplit::new(self, pred)
	}

	/// Returns an iterator over mutable subslices separated by bits that match
	/// `pred`, starting at the end of the slice and working backwards. The
	/// matched bit is not contained in the subslices.
	///
	/// # Original
	///
	/// [`slice::rsplit_mut`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.rsplit_mut)
	///
	/// # API Differences
	///
	/// In order to allow more than one bit of information for the split
	/// decision, the predicate receives the index of each bit, as well as its
	/// value.
	///
	/// This iterator marks each yielded item as aliased, as iterators can be
	/// used to yield multiple items into the same scope. If you are using
	/// the iterator in a manner that ensures that all yielded items have
	/// disjoint lifetimes, you can use the [`.remove_alias()`] adapter on it to
	/// remove the alias marker from the yielded subslices.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![mut 0, 0, 1, 0, 1, 0];
	/// for group in v.rsplit_mut(|_pos, bit| *bit) {
	///   group.set(0, true);
	/// }
	/// assert_eq!(v, bits![1, 0, 1, 1, 1, 1]);
	/// ```
	///
	/// [`.remove_alias()`]: crate::slice::RSplitMut::remove_alias
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn rsplit_mut<F>(&mut self, pred: F) -> RSplitMut<O, T, F>
	where F: FnMut(usize, &bool) -> bool {
		RSplitMut::new(self.alias_mut(), pred)
	}

	/// Returns an iterator over subslices separated by bits that match `pred`,
	/// limited to returning at most `n` items. The matched bit is not contained
	/// in the subslices.
	///
	/// The last item returned, if any, will contain the remainder of the slice.
	///
	/// # Original
	///
	/// [`slice::splitn`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.splitn)
	///
	/// # API Differences
	///
	/// In order to allow more than one bit of information for the split
	/// decision, the predicate receives the index of each bit, as well as its
	/// value.
	///
	/// # Examples
	///
	/// Print the slice split once by set bits (i.e., `[0, 0,]`, `[0, 1, 0]`):
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![0, 0, 1, 0, 1, 0];
	///
	/// for group in v.splitn(2, |_pos, bit| *bit) {
	///   # #[cfg(feature = "std")] {
	///   println!("{:b}", group);
	///   # }
	/// }
	/// ```
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn splitn<F>(&self, n: usize, pred: F) -> SplitN<O, T, F>
	where F: FnMut(usize, &bool) -> bool {
		SplitN::new(self, pred, n)
	}

	/// Returns an iterator over subslices separated by bits that match `pred`,
	/// limited to returning at most `n` items. The matched bit is not contained
	/// in the subslices.
	///
	/// The last item returned, if any, will contain the remainder of the slice.
	///
	/// # Original
	///
	/// [`slice::splitn_mut`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.splitn_mut)
	///
	/// # API Differences
	///
	/// In order to allow more than one bit of information for the split
	/// decision, the predicate receives the index of each bit, as well as its
	/// value.
	///
	/// This iterator marks each yielded item as aliased, as iterators can be
	/// used to yield multiple items into the same scope. If you are using
	/// the iterator in a manner that ensures that all yielded items have
	/// disjoint lifetimes, you can use the [`.remove_alias()`] adapter on it to
	/// remove the alias marker from the yielded subslices.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![mut 0, 0, 1, 0, 1, 0];
	///
	/// for group in v.splitn_mut(2, |_pos, bit| *bit) {
	///   group.set(0, true);
	/// }
	/// assert_eq!(v, bits![1, 0, 1, 1, 1, 0]);
	/// ```
	///
	/// [`.remove_alias()`]: crate::slice::SplitNMut::remove_alias
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn splitn_mut<F>(&mut self, n: usize, pred: F) -> SplitNMut<O, T, F>
	where F: FnMut(usize, &bool) -> bool {
		SplitNMut::new(self.alias_mut(), pred, n)
	}

	/// Returns an iterator over subslices separated by bits that match `pred`,
	/// limited to returning at most `n` items. This starts at the end of the
	/// slice and works backwards. The matched bit is not contained in the
	/// subslices.
	///
	/// The last item returned, if any, will contain the remainder of the slice.
	///
	/// # Original
	///
	/// [`slice::rsplitn`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.rsplitn)
	///
	/// # API Differences
	///
	/// In order to allow more than one bit of information for the split
	/// decision, the predicate receives the index of each bit, as well as its
	/// value.
	///
	/// # Examples
	///
	/// Print the slice split once, starting from the end, by set bits (i.e.,
	/// `[0]`, `[0, 0, 1, 0]`):
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![0, 0, 1, 0, 1, 0];
	///
	/// for group in v.rsplitn(2, |_pos, bit| *bit) {
	///   # #[cfg(feature = "std")] {
	///   println!("{:b}", group);
	///   # }
	/// }
	/// ```
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn rsplitn<F>(&self, n: usize, pred: F) -> RSplitN<O, T, F>
	where F: FnMut(usize, &bool) -> bool {
		RSplitN::new(self, pred, n)
	}

	/// Returns an iterator over subslices separated by bits that match `pred`,
	/// limited to returning at most `n` items. This starts at the end of the
	/// slice and works backwards. The matched bit is not contained in the
	/// subslices.
	///
	/// The last item returned, if any, will contain the remainder of the slice.
	///
	/// # Original
	///
	/// [`slice::rsplitn_mut`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.rsplitn_mut)
	///
	/// # API Differences
	///
	/// In order to allow more than one bit of information for the split
	/// decision, the predicate receives the index of each bit, as well as its
	/// value.
	///
	/// This iterator marks each yielded item as aliased, as iterators can be
	/// used to yield multiple items into the same scope. If you are using
	/// the iterator in a manner that ensures that all yielded items have
	/// disjoint lifetimes, you can use the [`.remove_alias()`] adapter on it to
	/// remove the alias marker from the yielded subslices.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![mut 0, 0, 1, 0, 1, 0];
	///
	/// for group in v.rsplitn_mut(2, |_pos, bit| *bit) {
	///   group.set(0, true);
	/// }
	/// assert_eq!(v, bits![1, 0, 1, 0, 1, 1]);
	/// ```
	///
	/// [`.remove_alias()`]: crate::slice::RSplitNMut::remove_alias
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn rsplitn_mut<F>(&mut self, n: usize, pred: F) -> RSplitNMut<O, T, F>
	where F: FnMut(usize, &bool) -> bool {
		RSplitNMut::new(self.alias_mut(), pred, n)
	}

	/// Returns `true` if the slice contains a subslice that matches the given
	/// span.
	///
	/// # Original
	///
	/// [`slice::contains`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.contains)
	///
	/// # API Differences
	///
	/// This searches for a matching subslice (allowing different type
	/// parameters) rather than for a specific bit. Searching for a contained
	/// element with a given value is not as useful on a collection of `bool`.
	///
	/// Furthermore, `BitSlice` defines [`any`] and [`not_all`], which are
	/// optimized searchers for any `true` or `false` bit, respectively, in a
	/// sequence.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let data = 0b0101_1010u8;
	/// let bits_msb = data.view_bits::<Msb0>();
	/// let bits_lsb = data.view_bits::<Lsb0>();
	/// assert!(bits_msb.contains(&bits_lsb[1 .. 5]));
	/// ```
	///
	/// This example uses a palindrome pattern to demonstrate that the slice
	/// being searched for does not need to have the same type parameters as the
	/// slice being searched.
	///
	/// [`any`]: Self::any
	/// [`not_all`]: Self::not_all
	#[inline]
	pub fn contains<O2, T2>(&self, x: &BitSlice<O2, T2>) -> bool
	where
		O2: BitOrder,
		T2: BitStore,
	{
		let len = x.len();
		if len > self.len() {
			return false;
		};
		self.windows(len).any(|s| s == x)
	}

	/// Returns `true` if `needle` is a prefix of the slice.
	///
	/// # Original
	///
	/// [`slice::starts_with`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.starts_with)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![0, 1, 0, 0];
	/// assert!(v.starts_with(bits![0]));
	/// assert!(v.starts_with(bits![0, 1]));
	/// assert!(!v.starts_with(bits![1]));
	/// assert!(!v.starts_with(bits![1, 0]));
	/// ```
	///
	/// Always returns `true` if `needle` is an empty slice:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![0, 1, 0];
	/// assert!(v.starts_with(bits![]));
	/// let v = bits![];
	/// assert!(v.starts_with(bits![]));
	/// ```
	#[inline]
	pub fn starts_with<O2, T2>(&self, needle: &BitSlice<O2, T2>) -> bool
	where
		O2: BitOrder,
		T2: BitStore,
	{
		let len = needle.len();
		self.len() >= len && needle == unsafe { self.get_unchecked(.. len) }
	}

	/// Returns `true` if `needle` is a suffix of the slice.
	///
	/// # Original
	///
	/// [`slice::ends_with`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.ends_with)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![0, 1, 0, 0];
	/// assert!(v.ends_with(bits![0]));
	/// assert!(v.ends_with(bits![0; 2]));
	/// assert!(!v.ends_with(bits![1]));
	/// assert!(!v.ends_with(bits![1, 0]));
	/// ```
	///
	/// Always returns `true` if `needle` is an empty slice:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let v = bits![0, 1, 0];
	/// assert!(v.ends_with(bits![]));
	/// let v = bits![];
	/// assert!(v.ends_with(bits![]));
	/// ```
	#[inline]
	pub fn ends_with<O2, T2>(&self, needle: &BitSlice<O2, T2>) -> bool
	where
		O2: BitOrder,
		T2: BitStore,
	{
		let nlen = needle.len();
		let len = self.len();
		len >= nlen && needle == unsafe { self.get_unchecked(len - nlen ..) }
	}

	/// Rotates the slice in-place such that the first `by` bits of the slice
	/// move to the end while the last `self.len() - by` bits move to the
	/// front. After calling `.rotate_left()`, the bit previously at index `by`
	/// will become the first bit in the slice.
	///
	/// # Original
	///
	/// [`slice::rotate_left`](https://doc.rust-lang.org/stable/std/primitive.slice.html#rotate_left)
	///
	/// # Panics
	///
	/// This function will panic if `by` is greater than the length of the
	/// slice. Note that `by == self.len()` does *not* panic and is a noöp.
	///
	/// # Complexity
	///
	/// Takes linear (in [`self.len()`]) time.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let a = bits![mut 0, 0, 1, 0, 1, 0];
	/// a.rotate_left(2);
	/// assert_eq!(a, bits![1, 0, 1, 0, 0, 0]);
	/// ```
	///
	/// Rotating a subslice:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let a = bits![mut 0, 0, 1, 0, 1, 1];
	/// a[1 .. 5].rotate_left(1);
	/// assert_eq!(a, bits![0, 1, 0, 1, 0, 1]);
	/// ```
	///
	/// [`self.len()`]: Self::len
	#[inline]
	pub fn rotate_left(&mut self, mut by: usize) {
		let len = self.len();
		assert!(
			by <= len,
			"Slices cannot be rotated by more than their length"
		);
		if by == 0 || by == len {
			return;
		}
		/* The standard one-element-at-a-time algorithm is necessary for `[T]`
		rotation, because it must not allocate, but bit slices have an advantage
		in that placing a single processor word on the stack as a temporary has
		significant logical acceleration.

		Instead, we can move `min(usize::BITS, by)` bits from the front of the
		slice into the stack, then shunt the rest of the slice downwards, then
		insert the stack bits into the now-open back, repeating until complete.

		There is no reason to use a stack temporary smaller than a processor
		word, so this uses `usize` instead of `T` for performance benefits.
		*/
		let mut tmp = BitArray::<O, usize>::zeroed();
		while by > 0 {
			let shamt = cmp::min(<usize as BitMemory>::BITS as usize, by);
			unsafe {
				let tmp_bits = tmp.get_unchecked_mut(.. shamt);
				tmp_bits.clone_from_bitslice(self.get_unchecked(.. shamt));
				self.copy_within_unchecked(shamt .., 0);
				self.get_unchecked_mut(len - shamt ..)
					.clone_from_bitslice(tmp_bits);
			}
			by -= shamt;
		}
	}

	/// Rotates the slice in-place such that the first `self.len() - by` bits of
	/// the slice move to the end while the last `by` bits move to the front.
	/// After calling `.rotate_right()`, the bit previously at index `self.len()
	/// - by` will become the first bit in the slice.
	///
	/// # Original
	///
	/// [`slice::rotate_right`](https://doc.rust-lang.org/stable/std/primitive.slice.html#rotate_right)
	///
	/// # Panics
	///
	/// This function will panic if `by` is greater than the length of the
	/// slice. Note that `by == self.len()` does *not* panic and is a noöp.
	///
	/// # Complexity
	///
	/// Takes linear (in [`self.len()`]) time.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let a = bits![mut 0, 0, 1, 1, 1, 0];
	/// a.rotate_right(2);
	/// assert_eq!(a, bits![1, 0, 0, 0, 1, 1]);
	/// ```
	///
	/// Rotating a subslice:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let a = bits![mut 0, 0, 1, 0, 1, 1];
	/// a[1 .. 5].rotate_right(1);
	/// assert_eq!(a, bits![0, 1, 0, 1, 0, 1]);
	/// ```
	///
	/// [`self.len()`]: Self::len
	#[inline]
	pub fn rotate_right(&mut self, mut by: usize) {
		let len = self.len();
		assert!(
			by <= len,
			"Slices cannot be rotated by more than their length"
		);
		if by == 0 || by == len {
			return;
		}
		let mut tmp = BitArray::<O, usize>::zeroed();
		while by > 0 {
			let shamt = cmp::min(<usize as BitMemory>::BITS as usize, by);
			let mid = len - shamt;
			unsafe {
				let tmp_bits = tmp.get_unchecked_mut(.. shamt);
				tmp_bits.clone_from_bitslice(self.get_unchecked(mid ..));
				self.copy_within_unchecked(.. mid, shamt);
				self.get_unchecked_mut(.. shamt)
					.clone_from_bitslice(tmp_bits);
			}
			by -= shamt;
		}
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Use `clone_from_bitslice` to copy between bitslices"]
	pub fn clone_from_slice<O2, T2>(&mut self, src: &BitSlice<O2, T2>)
	where
		O2: BitOrder,
		T2: BitStore,
	{
		self.clone_from_bitslice(src)
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Use `copy_from_bitslice` to copy between bitslices"]
	pub fn copy_from_slice(&mut self, src: &Self) {
		self.copy_from_bitslice(src)
	}

	/// Copies bits from one part of the slice to another part of itself.
	///
	/// `src` is the range within `self` to copy from. `dest` is the starting
	/// index of the range within `self` to copy to, which will have the same
	/// length as `src`. The two ranges may overlap. The ends of the two ranges
	/// must be less than or equal to [`self.len()`].
	///
	/// # Original
	///
	/// [`slice::copy_within`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.copy_within)
	///
	/// # Panics
	///
	/// This function will panic if either range exceeds the end of the slice,
	/// or if the end of `src` is before the start.
	///
	/// # Examples
	///
	/// Copying four bits within a slice:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![mut 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0];
	///
	/// bits.copy_within(1 .. 5, 8);
	///
	/// assert_eq!(bits, bits![1, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1, 0]);
	/// ```
	///
	/// [`self.len()`]: Self::len
	#[inline]
	pub fn copy_within<R>(&mut self, src: R, dest: usize)
	where R: RangeBounds<usize> {
		let len = self.len();
		let src = dvl::normalize_range(src, len);
		//  Check that the source range is within bounds,
		dvl::assert_range(src.clone(), len);
		//  And that the destination range is within bounds.
		dvl::assert_range(dest .. dest + (src.end - src.start), len);
		unsafe {
			self.copy_within_unchecked(src, dest);
		}
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Use `swap_with_bitslice` to swap between bitslices"]
	pub fn swap_with_slice<O2, T2>(&mut self, other: &mut BitSlice<O2, T2>)
	where
		O2: BitOrder,
		T2: BitStore,
	{
		self.swap_with_bitslice(other);
	}

	/// Transmute the bit-slice to a bit-slice of another type, ensuring
	/// alignment of the types is maintained.
	///
	/// # Original
	///
	/// [`slice::align_to`]
	///
	/// # API Differences
	///
	/// Type `U` is **required** to have the same [`BitStore`] type family as
	/// type `T`. If `T` is a fundamental integer, so must `U` be; if `T` is an
	/// [`::Alias`] type, then so must `U`. Changing the type family with this
	/// method is **unsound** and strictly forbidden. Unfortunately, this cannot
	/// be encoded in the type system, so you are required to abide by this
	/// limitation yourself.
	///
	/// # Implementation
	///
	/// The algorithm used to implement this function attempts to create the
	/// widest possible span for the middle slice. However, the slice divisions
	/// must abide by the [`Domain`] restrictions: the left and right slices
	/// produced by this function will include the head and tail elements of the
	/// domain (if present), as well as the left and right subslices (if any)
	/// produced by calling [`slice::align_to`] on the domain body (if present).
	///
	/// The standard library implementation currently maximizes the width of the
	/// center slice, but its API does not guarantee this property, and retains
	/// the right to produce pessimal slices. As such, this function cannot
	/// guarantee maximal center slice width either, and you cannot rely on this
	/// behavior for *correctness* of your work; it is only a possible
	/// performance improvement.
	///
	/// # Safety
	///
	/// This method is essentially a [`mem::transmute`][mt] with respect to the
	/// memory region in the retured middle slice, so all of the usual caveats
	/// pertaining to [`mem::transmute::<T, U>`][mt] also apply here.
	///
	/// # Examples
	///
	/// Basic usage:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// unsafe {
	///   let bytes: [u8; 7] = [1, 2, 3, 4, 5, 6, 7];
	///   let bits = bytes.view_bits::<LocalBits>();
	///   let (prefix, shorts, suffix) = bits.align_to::<u16>();
	///   match prefix.len() {
	///     0 => {
	///       assert_eq!(shorts, bits[.. 48]);
	///       assert_eq!(suffix, bits[48 ..]);
	///     },
	///     8 => {
	///       assert_eq!(prefix, bits[.. 8]);
	///       assert_eq!(shorts, bits[8 ..]);
	///     },
	///     _ => unreachable!("This case will not occur")
	///   }
	/// }
	/// ```
	///
	/// [mt]: core::mem::transmute
	/// [`BitStore`]: crate::store::BitStore
	/// [`Domain`]: crate::domain::Domain
	/// [`slice::align_to`]: https://doc.rust-lang.org/stable/std/primitive.slice.html#method.align_to
	/// [`::Alias`]: crate::store::BitStore::Alias
	#[inline]
	pub unsafe fn align_to<U>(&self) -> (&Self, &BitSlice<O, U>, &Self)
	where U: BitStore {
		let (l, c, r) = self.as_bitspan().align_to::<U>();
		(
			l.to_bitslice_ref(),
			c.to_bitslice_ref(),
			r.to_bitslice_ref(),
		)
	}

	/// Transmute the bit-slice to a bit-slice of another type, ensuring
	/// alignment of the types is maintained.
	///
	/// # Original
	///
	/// [`slice::align_to_mut`]
	///
	/// # API Differences
	///
	/// Type `U` is **required** to have the same [`BitStore`] type family as
	/// type `T`. If `T` is a fundamental integer, so must `U` be; if `T` is an
	/// [`::Alias`] type, then so must `U`. Changing the type family with this
	/// method is **unsound** and strictly forbidden. Unfortunately, this cannot
	/// be encoded in the type system, so you are required to abide by this
	/// limitation yourself.
	///
	/// # Implementation
	///
	/// The algorithm used to implement this function attempts to create the
	/// widest possible span for the middle slice. However, the slice divisions
	/// must abide by the [`DomainMut`] restrictions: the left and right slices
	/// produced by this function will include the head and tail elements of the
	/// domain (if present), as well as the left and right subslices (if any)
	/// produced by calling [`slice::align_to_mut`] on the domain body (if
	/// present).
	///
	/// The standard library implementation currently maximizes the width of the
	/// center slice, but its API does not guarantee this property, and retains
	/// the right to produce pessimal slices. As such, this function cannot
	/// guarantee maximal center slice width either, and you cannot rely on this
	/// behavior for *correctness* of your work; it is only a possible
	/// performance improvement.
	///
	/// # Safety
	///
	/// This method is essentially a [`mem::transmute`][mt] with respect to the
	/// memory region in the retured middle slice, so all of the usual caveats
	/// pertaining to [`mem::transmute::<T, U>`][mt] also apply here.
	///
	/// # Examples
	///
	/// Basic usage:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// unsafe {
	///   let mut bytes: [u8; 7] = [1, 2, 3, 4, 5, 6, 7];
	///   let bits = bytes.view_bits_mut::<LocalBits>();
	///   let (prefix, shorts, suffix) = bits.align_to_mut::<u16>();
	///   //  same access and behavior as in `align_to`
	/// }
	/// ```
	///
	/// [mt]: core::mem::transmute
	/// [`BitStore`]: crate::store::BitStore
	/// [`DomainMut`]: crate::domain::DomainMut
	/// [`slice::align_to_mut`]: https://doc.rust-lang.org/stable/std/primitive.slice.html#method.align_to_mut
	/// [`::Alias`]: crate::store::BitStore::Alias
	#[inline]
	pub unsafe fn align_to_mut<U>(
		&mut self,
	) -> (&mut Self, &mut BitSlice<O, U>, &mut Self)
	where U: BitStore {
		let (l, c, r) = self.as_mut_bitspan().align_to::<U>();
		(
			l.to_bitslice_mut(),
			c.to_bitslice_mut(),
			r.to_bitslice_mut(),
		)
	}
}

/** These functions only exist when [`BitVec`] does.

[`BitVec`]: crate::vec::BitVec
**/
#[cfg(feature = "alloc")]
impl<O, T> BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Prefer `to_bitvec`"]
	pub fn to_vec(&self) -> BitVec<O, T::Unalias> {
		self.to_bitvec()
	}

	/// Creates a vector by repeating a slice `n` times.
	///
	/// # Original
	///
	/// [`slice::repeat`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.repeat)
	///
	/// # Panics
	///
	/// This function will panic if the capacity would overflow.
	///
	/// # Examples
	///
	/// Basic usage:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// assert_eq!(bits![0, 1].repeat(3), bits![0, 1, 0, 1, 0, 1]);
	/// ```
	///
	/// A panic upon overflow:
	///
	/// ```rust,should_panic
	/// use bitvec::prelude::*;
	///
	/// // this will panic at runtime
	/// bits![0, 1].repeat(BitSlice::<LocalBits, usize>::MAX_BITS);
	/// ```
	#[inline]
	pub fn repeat(&self, n: usize) -> BitVec<O, T::Unalias> {
		let len = self.len();
		let total = len.checked_mul(n).expect("capacity overflow");

		//  The memory has to be initialized before `.clone_from_bitslice` can
		//  write into it.
		let mut out = BitVec::repeat(false, total);

		for chunk in unsafe { out.chunks_exact_mut(len).remove_alias() } {
			//  TODO(myrrlyn): Specialize for `BitField` access
			chunk.clone_from_bitslice(self);
		}

		out
	}

	/* As of 1.44, the `concat` and `join` methods use still-unstable traits to
	govern the collection of multiple subslices into one vector. These are
	possible to copy over and redefine locally, but unless a user asks for it,
	doing so is considered a low priority.
	*/
}

/** Converts a reference to `T` into a [`BitSlice`] over one element.

# Original

[`slice::from_ref`](core::slice::from_ref)

[`BitSlice`]: crate::slice::BitSlice
**/
#[inline(always)]
#[cfg(not(tarpaulin_include))]
pub fn from_ref<O, T>(elem: &T) -> &BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	BitSlice::from_element(elem)
}

/** Converts a reference to `T` into a [`BitSlice`] over one element.

# Original

[`slice::from_mut`](core::slice::from_mut)

[`BitSlice`]: crate::slice::BitSlice
**/
#[inline(always)]
#[cfg(not(tarpaulin_include))]
pub fn from_mut<O, T>(elem: &mut T) -> &mut BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	BitSlice::from_element_mut(elem)
}

/* NOTE: Crate style is to use block doc comments at the left margin. A bug in
`rustfmt` replaces four spaces at left margin with hard tab, which is incorrect
in comments. Once `rustfmt` is fixed, revert these to block comments.
*/

/// Forms a bit-slice from a bit-pointer and a length.
///
/// The `len` argument is the number of **bits**, not the number of bytes or
/// elements.
///
/// # Original
///
/// [`slice::from_raw_parts`](core::slice::from_raw_parts)
///
/// # API Differences
///
/// This takes a [`BitPtr`] as its base address, rather than a raw `*Bit`
/// pointer, as `bitvec` does not provide raw pointers to individual bits.
///
/// It returns a `Result`, because the `len` argument may be invalid to encode
/// into a `&BitSlice` reference.
///
/// # Safety
///
/// Behavior is undefined if any of the following conditions are violated:
///
/// - `data` must be valid for reads for `len` many bits, and it must be
///   properly aligned. This means in particular:
///   - The entire memory range of this slice must be contained within a single
///     allocated object! Slices can never span across multiple allocated
///     objects. See [below] for an example incorrectly not taking this into
///     account.
///   - `data` must be non-null, and its `T` portion must be aligned. Both of
///     these conditions are checked during safe construction of the [`BitPtr`],
///     and `unsafe` construction of it **must not** violate them. Doing so will
///     cause incorrect behavior in the crate.
/// - `data` must point to `len` consecutive bits within properly initialized
///   memory elements `T`.
/// - The memory referenced by the returned slice must not be mutated for the
///   duration of the lifetime `'a`, except if `T` is an atomic or a `Cell`
///   type.
/// - `len` cannot exceed [`BitSlice::MAX_BITS`].
///
/// # Caveat
///
/// The lifetime for the returned slice is inferred from its usage. To prevent
/// accidental misuse, it’s suggested to tie the lifetime to whichever source
/// lifetime is safe in the context, such as by providing a helper function
/// taking the lifetime of a host value for the slice, or by explicit
/// annotation.
///
/// # Examples
///
/// ```rust
/// use bitvec::prelude::*;
/// use bitvec::slice as bv_slice;
///
/// let x = 42u8;
/// let bitptr = BitPtr::from(&x);
/// let bits: &BitSlice<LocalBits, _> = unsafe {
///   bv_slice::from_raw_parts(bitptr, 8)
/// }
/// .unwrap();
/// assert_eq!(bits, x.view_bits::<LocalBits>());
/// ```
///
/// ### Incorrect Usage
///
/// The following `join_slices` function is **unsound** ⚠️
///
/// ```rust,no_run
/// use bitvec::prelude::*;
/// use bitvec::slice as bv_slice;
///
/// fn join_bitslices<'a, O, T>(
///   fst: &'a BitSlice<O, T>,
///   snd: &'a BitSlice<O, T>,
/// ) -> &'a BitSlice<O, T>
/// where O: BitOrder, T: BitStore {
///   let fst_end = unsafe {
///     fst.as_bitptr().wrapping_add(fst.len())
///   };
///   let snd_start = snd.as_bitptr();
///   assert_eq!(snd_start, fst_end, "Slices must be adjacent");
///   unsafe {
///     bv_slice::from_raw_parts(fst.as_bitptr(), fst.len() + snd.len())
///   }
///   .unwrap()
/// }
///
/// let a = [0u8; 3];
/// let b = [!0u8; 3];
/// let c = join_bitslices(
///   a.view_bits::<LocalBits>(),
///   b.view_bits::<LocalBits>(),
/// );
/// ```
///
/// In this example, the compiler may elect to place `a` and `b` in adjacent
/// stack slots, but because they are still *separate allocation* regions, it is
/// illegal for a single region descriptor to be created over both of them.
///
/// [below]: #incorrect-usage
/// [`BitPtr`]: crate::ptr::BitPtr
/// [`BitSlice::MAX_BITS`]: crate::slice::BitSlice::MAX_BITS
#[inline]
pub unsafe fn from_raw_parts<'a, O, T>(
	data: BitPtr<Const, O, T>,
	len: usize,
) -> Result<&'a BitSlice<O, T>, BitSpanError<T>>
where
	O: BitOrder,
	T: BitStore,
{
	data.span(len).map(BitSpan::to_bitslice_ref)
}

/// Performs the same functionality as [`from_raw_parts`], except that a mutable
/// slice is returned.
///
/// # Original
///
/// [`slice::from_raw_parts_mut`](core::slice::from_raw_parts_mut)
///
/// # API Differences
///
/// This takes a [`BitPtr`] as its base address, rather than a raw `*Bit`
/// pointer, as `bitvec` does not provide raw pointers to individual bits.
///
/// It returns a `Result`, because the `len` argument may be invalid to encode
/// into a `&BitSlice` reference.
///
/// # Safety
///
/// Behavior is undefined if any of the following conditions are violated:
///
/// - `data` must be [valid] for boths reads and writes for `len` many bits, and
///   it must be properly aligned. This means in particular:
///   - The entire memory range of this slice must be contained within a single
///     allocated object! Slices can never span across multiple allocated
///     objects.
///   - `data` must be non-null, and its `T` portion must be aligned. Both of
///     these conditions are checked during safe construction of the [`BitPtr`],
///     and `unsafe` construction of it **must not** violate them. Doing so will
///     cause incorrect behavior in the crate.
/// - `data` must point to `len` consecutive bits within properly initialized
///   memory elements `T`.
/// - The memory referenced by the returned slice must not be accessed through
///   any other pointer (not derived from the return value) for the duration of
///   lifetime `'a`. Both read and write accesses are forbidden. This is true
///   even if `T` supports aliased mutation! An `&mut` reference requires
///   **exclusive** access for its lifetime.
/// - `len` cannot exceed [`BitSlice::MAX_BITS`].
///
/// [valid]: https://doc.rust-lang.org/stable/core/ptr/index.html#safety
/// [`BitPtr`]: crate::ptr::BitPtr
/// [`from_raw_parts`]: crate::slice::from_raw_parts
#[inline]
pub unsafe fn from_raw_parts_mut<'a, O, T>(
	data: BitPtr<Mut, O, T>,
	len: usize,
) -> Result<&'a mut BitSlice<O, T>, BitSpanError<T>>
where
	O: BitOrder,
	T: BitStore,
{
	data.span(len).map(BitSpan::to_bitslice_mut)
}

/** A helper trait used for indexing operations.

This trait has its definition stabilized, but has not stabilized its associated
methods. This means it cannot be implemented outside of the distribution
libraries. *Furthermore*, since [`bitvec`] cannot create `&mut bool` references,
it is insufficient for `bitvec`’s uses.

There is no tracking issue for `feature(slice_index_methods)`.

# Original

[`slice::SliceIndex`](core::slice::SliceIndex)

# API Differences

[`SliceIndex::Output`] is not usable here, because the `usize` implementation
cannot produce `&mut bool`. Instead, two output types `Immut` and `Mut` are
defined. The range implementations define these to be the appropriately mutable
[`BitSlice`] reference; the `usize` implementation defines them to be `&bool`
and the proxy type.

[`BitSlice`]: crate::slice::BitSlice
[`SliceIndex::Output`]: core::slice::SliceIndex::Output
[`bitvec`]: crate
**/
pub trait BitSliceIndex<'a, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// The output type for immutable accessors.
	type Immut;

	/// The output type for mutable accessors.
	type Mut;

	/// Returns a shared reference to the output at this location, if in bounds.
	///
	/// # Original
	///
	/// [`SliceIndex::get`](core::slice::SliceIndex::get)
	fn get(self, slice: &'a BitSlice<O, T>) -> Option<Self::Immut>;

	/// Returns a mutable reference to the output at this location, if in
	/// bounds.
	///
	/// # Original
	///
	/// [`SliceIndex::get_mut`](core::slice::SliceIndex::get_mut)
	fn get_mut(self, slice: &'a mut BitSlice<O, T>) -> Option<Self::Mut>;

	/// Returns a shared reference to the output at this location, without
	/// performing any bounds checking. Calling this method with an
	/// out-of-bounds index is [undefined behavior] even if the resulting
	/// reference is not used.
	///
	/// # Original
	///
	/// [`SliceIndex::get_unchecked`](core::slice::SliceIndex::get_unchecked)
	///
	/// # Safety
	///
	/// As this function does not perform boundary checking, the caller must
	/// ensure that `self` is an index within the boundaries of `slice` before
	/// calling in order to prevent boundary escapes and the ensuing safety
	/// violations.
	///
	/// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
	unsafe fn get_unchecked(self, slice: &'a BitSlice<O, T>) -> Self::Immut;

	/// Returns a mutable reference to the output at this location, without
	/// performing any bounds checking. Calling this method with an
	/// out-of-bounds index is [undefined behavior] even if the resulting
	/// reference is not used.
	///
	/// # Original
	///
	/// [`SliceIndex::get_unchecked_mut`][orig]
	///
	/// # Safety
	///
	/// As this function does not perform boundary checking, the caller must
	/// ensure that `self` is an index within the boundaries of `slice` before
	/// calling in order to prevent boundary escapes and the ensuing safety
	/// violations.
	///
	/// [orig]: core::slice::SliceIndex::get_unchecked_mut
	/// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
	unsafe fn get_unchecked_mut(
		self,
		slice: &'a mut BitSlice<O, T>,
	) -> Self::Mut;

	/// Returns a shared reference to the output at this location, panicking if
	/// out of bounds.
	///
	/// # Original
	///
	/// [`SliceIndex::index`](core::slice::SliceIndex::index)
	fn index(self, slice: &'a BitSlice<O, T>) -> Self::Immut;

	/// Returns a mutable reference to the output at this location, panicking if
	/// out of bounds.
	///
	/// # Original
	///
	/// [`SliceIndex::index_mut`](core::slice::SliceIndex::index_mut)
	fn index_mut(self, slice: &'a mut BitSlice<O, T>) -> Self::Mut;
}

impl<'a, O, T> BitSliceIndex<'a, O, T> for usize
where
	O: BitOrder,
	T: BitStore,
{
	type Immut = BitRef<'a, Const, O, T>;
	type Mut = BitRef<'a, Mut, O, T>;

	#[inline]
	fn get(self, slice: &'a BitSlice<O, T>) -> Option<Self::Immut> {
		if self < slice.len() {
			Some(unsafe { self.get_unchecked(slice) })
		}
		else {
			None
		}
	}

	#[inline]
	fn get_mut(self, slice: &'a mut BitSlice<O, T>) -> Option<Self::Mut> {
		if self < slice.len() {
			Some(unsafe { self.get_unchecked_mut(slice) })
		}
		else {
			None
		}
	}

	#[inline]
	unsafe fn get_unchecked(self, slice: &'a BitSlice<O, T>) -> Self::Immut {
		BitRef::from_bitptr(slice.as_bitptr().add(self))
	}

	#[inline]
	unsafe fn get_unchecked_mut(
		self,
		slice: &'a mut BitSlice<O, T>,
	) -> Self::Mut {
		BitRef::from_bitptr(slice.as_mut_bitptr().add(self))
	}

	#[inline]
	fn index(self, slice: &'a BitSlice<O, T>) -> Self::Immut {
		self.get(slice).unwrap_or_else(|| {
			panic!("Index {} out of bounds: {}", self, slice.len())
		})
	}

	#[inline]
	fn index_mut(self, slice: &'a mut BitSlice<O, T>) -> Self::Mut {
		let len = slice.len();
		self.get_mut(slice)
			.unwrap_or_else(|| panic!("Index {} out of bounds: {}", self, len))
	}
}

/// Implement indexing for the different range types.
macro_rules! range_impl {
	( $r:ty { $get:item $unchecked:item } ) => {
		impl<'a, O, T> BitSliceIndex<'a, O, T> for $r
		where O: BitOrder, T: BitStore {
			type Immut = &'a BitSlice<O, T>;
			type Mut = &'a mut BitSlice<O, T>;

			#[inline]
			$get

			#[inline]
			fn get_mut(self, slice: Self::Mut) -> Option<Self::Mut> {
				self.get(slice).map(|s| unsafe {
					s.as_bitspan().assert_mut()
				}
				.to_bitslice_mut())
			}

			#[inline]
			$unchecked

			#[inline]
			unsafe fn get_unchecked_mut(self, slice: Self::Mut) -> Self::Mut {
				self.get_unchecked(slice)
					.as_bitspan()
					.assert_mut()
					.to_bitslice_mut()
			}

			#[inline]
			fn index(self, slice: Self::Immut) -> Self::Immut {
				let r = self.clone();
				let l = slice.len();
				self.get(slice)
					.unwrap_or_else(|| {
						panic!("Range {:?} out of bounds: {}", r, l)
					})
			}

			#[inline]
			fn index_mut(self, slice: Self::Mut) -> Self::Mut {
				self.index(slice).as_bitspan().pipe(|span| unsafe {
					span.assert_mut()
				})
				.to_bitslice_mut()
			}
		}
	};

	( $( $r:ty => map $func:expr; )* ) => { $(
		impl<'a, O, T> BitSliceIndex<'a, O, T> for $r
		where O: BitOrder, T: BitStore {
			type Immut = &'a BitSlice<O, T>;
			type Mut = &'a mut BitSlice<O, T>;

			#[inline]
			fn get(self, slice: Self::Immut) -> Option<Self::Immut> {
				$func(self).get(slice)
			}

			#[inline]
			fn get_mut(self, slice: Self::Mut) -> Option<Self::Mut> {
				$func(self).get_mut(slice)
			}

			#[inline]
			unsafe fn get_unchecked(self, slice: Self::Immut) -> Self::Immut {
				$func(self).get_unchecked(slice)
			}

			#[inline]
			unsafe fn get_unchecked_mut(self, slice: Self::Mut) -> Self::Mut {
				$func(self).get_unchecked_mut(slice)
			}

			#[inline]
			fn index(self, slice: Self::Immut) -> Self::Immut {
				$func(self).index(slice)
			}

			#[inline]
			fn index_mut(self, slice: Self::Mut) -> Self::Mut {
				$func(self).index_mut(slice)
			}
		}
	)* };
}

range_impl!(Range<usize> {
	fn get(self, slice: Self::Immut) -> Option<Self::Immut> {
		let len = slice.len();

		if self.start > len || self.end > len || self.start > self.end {
			return None;
		}

		Some(unsafe { (self.start .. self.end).get_unchecked(slice) })
	}

	unsafe fn get_unchecked(self, slice: Self::Immut) -> Self::Immut {
		slice.as_bitptr()
			.add(self.start)
			.span_unchecked(self.len())
			.to_bitslice_ref()
	}
});

range_impl!(RangeFrom<usize> {
	fn get(self, slice: Self::Immut) -> Option<Self::Immut> {
		let len = slice.len();
		if self.start <= len {
			Some(unsafe { (self.start ..).get_unchecked(slice) })
		}
		else {
			None
		}
	}

	unsafe fn get_unchecked(self, slice: Self::Immut) -> Self::Immut {
		slice.as_bitptr()
			.add(self.start)
			.span_unchecked(slice.len() - self.start)
			.to_bitslice_ref()
	}
});

range_impl!(RangeTo<usize> {
	// `.. end` only changes the length
	fn get(self, slice: Self::Immut) -> Option<Self::Immut> {
		let len = slice.len();
		if self.end <= len {
			Some(unsafe { (.. self.end).get_unchecked(slice) })
		}
		else {
			None
		}
	}

	unsafe fn get_unchecked(self, slice: Self::Immut) -> Self::Immut {
		slice.as_bitspan().tap_mut(|bp| bp.set_len(self.end)).to_bitslice_ref()
	}
});

range_impl! {
	RangeInclusive<usize> => map |this: Self| {
		#[allow(clippy::range_plus_one)]
		(*this.start() .. *this.end() + 1)
	};

	RangeToInclusive<usize> => map |RangeToInclusive { end }| {
		#[allow(clippy::range_plus_one)]
		(.. end + 1)
	};
}

/// `RangeFull` is the identity function.
#[cfg(not(tarpaulin_include))]
impl<'a, O, T> BitSliceIndex<'a, O, T> for RangeFull
where
	O: BitOrder,
	T: BitStore,
{
	type Immut = &'a BitSlice<O, T>;
	type Mut = &'a mut BitSlice<O, T>;

	#[inline(always)]
	fn get(self, slice: Self::Immut) -> Option<Self::Immut> {
		Some(slice)
	}

	#[inline(always)]
	fn get_mut(self, slice: Self::Mut) -> Option<Self::Mut> {
		Some(slice)
	}

	#[inline(always)]
	unsafe fn get_unchecked(self, slice: Self::Immut) -> Self::Immut {
		slice
	}

	#[inline(always)]
	unsafe fn get_unchecked_mut(self, slice: Self::Mut) -> Self::Mut {
		slice
	}

	#[inline(always)]
	fn index(self, slice: Self::Immut) -> Self::Immut {
		slice
	}

	#[inline(always)]
	fn index_mut(self, slice: Self::Mut) -> Self::Mut {
		slice
	}
}
