//! Iterators over `Vec<T>`.

use crate::{
	devel as dvl,
	mutability::Mutability,
	order::BitOrder,
	ptr::BitRef,
	slice::{
		BitSlice,
		Iter,
	},
	store::BitStore,
	vec::BitVec,
};

use alloc::vec::Vec;

use core::{
	fmt::{
		self,
		Debug,
		Formatter,
	},
	iter::{
		FromIterator,
		FusedIterator,
	},
	mem::{
		self,
		ManuallyDrop,
	},
	ops::{
		Range,
		RangeBounds,
	},
	ptr::NonNull,
};

use tap::{
	pipe::Pipe,
	tap::{
		Tap,
		TapOptional,
	},
};

impl<O, T> Extend<bool> for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn extend<I>(&mut self, iter: I)
	where I: IntoIterator<Item = bool> {
		let mut iter = iter.into_iter();
		match iter.size_hint() {
			(n, None) | (_, Some(n)) => {
				// This body exists to try to accelerate the push-per-bit loop.
				self.reserve(n);
				let len = self.len();
				let new_len = len + n;
				let new = unsafe { self.get_unchecked_mut(len .. new_len) };
				let mut pulled = 0;
				for (slot, bit) in
					unsafe { new.iter_mut().remove_alias() }.zip(iter.by_ref())
				{
					slot.set(bit);
					pulled += 1;
				}
				unsafe {
					self.set_len(len + pulled);
				}
			},
		}
		iter.for_each(|bit| self.push(bit));
	}
}

impl<'a, O, T> Extend<&'a bool> for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn extend<I>(&mut self, iter: I)
	where I: IntoIterator<Item = &'a bool> {
		self.extend(iter.into_iter().copied());
	}
}

impl<'a, M, O1, O2, T1, T2> Extend<BitRef<'a, M, O2, T2>> for BitVec<O1, T1>
where
	M: Mutability,
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	fn extend<I>(&mut self, iter: I)
	where I: IntoIterator<Item = BitRef<'a, M, O2, T2>> {
		self.extend(iter.into_iter().map(|bit| *bit));
	}
}

impl<O, T> Extend<T> for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn extend<I>(&mut self, iter: I)
	where I: IntoIterator<Item = T> {
		for elem in iter.into_iter() {
			self.extend(BitSlice::<O, T>::from_element(&elem));
		}
	}
}

impl<'a, O, T> Extend<&'a T> for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn extend<I>(&mut self, iter: I)
	where I: IntoIterator<Item = &'a T> {
		for elem in iter.into_iter() {
			self.extend(BitSlice::<O, T>::from_element(elem));
		}
	}
}

impl<O, T> FromIterator<bool> for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn from_iter<I>(iter: I) -> Self
	where I: IntoIterator<Item = bool> {
		Self::new().tap_mut(|bv| bv.extend(iter.into_iter()))
	}
}

impl<'a, M, O1, O2, T1, T2> FromIterator<BitRef<'a, M, O2, T2>>
	for BitVec<O1, T1>
where
	M: Mutability,
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	fn from_iter<I>(iter: I) -> Self
	where I: IntoIterator<Item = BitRef<'a, M, O2, T2>> {
		Self::new().tap_mut(|bv| bv.extend(iter.into_iter()))
	}
}

impl<'a, O, T> FromIterator<&'a bool> for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn from_iter<I>(iter: I) -> Self
	where I: IntoIterator<Item = &'a bool> {
		iter.into_iter().copied().pipe(Self::from_iter)
	}
}

/** Collect a sequence of memory elements into a bit-vector.

This is a short-hand for, and implemented as, `iter.collect::<Vec<_>>().into()`.

This is not a standard-library API, and was added for [Issue #83].

[Issue #83]: https://github.com/myrrlyn/bitvec/issues/83
**/
impl<O, T> FromIterator<T> for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn from_iter<I>(iter: I) -> Self
	where I: IntoIterator<Item = T> {
		iter.into_iter().collect::<Vec<_>>().pipe(Self::from_vec)
	}
}

impl<'a, O, T> FromIterator<&'a T> for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn from_iter<I>(iter: I) -> Self
	where I: IntoIterator<Item = &'a T> {
		let mut vec = iter
			.into_iter()
			.map(BitStore::load_value)
			.collect::<Vec<T::Mem>>()
			.pipe(ManuallyDrop::new);
		let (ptr, len, capa) = (vec.as_mut_ptr(), vec.len(), vec.capacity());
		unsafe { Vec::from_raw_parts(ptr as *mut T, len, capa) }
			.pipe(Self::from_vec)
	}
}

impl<O, T> IntoIterator for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	type IntoIter = IntoIter<O, T>;
	type Item = bool;

	fn into_iter(self) -> Self::IntoIter {
		IntoIter::new(self)
	}
}

impl<'a, O, T> IntoIterator for &'a BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	type IntoIter = <&'a BitSlice<O, T> as IntoIterator>::IntoIter;
	type Item = <&'a BitSlice<O, T> as IntoIterator>::Item;

	fn into_iter(self) -> Self::IntoIter {
		self.as_bitslice().into_iter()
	}
}

impl<'a, O, T> IntoIterator for &'a mut BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	type IntoIter = <&'a mut BitSlice<O, T> as IntoIterator>::IntoIter;
	type Item = <&'a mut BitSlice<O, T> as IntoIterator>::Item;

	fn into_iter(self) -> Self::IntoIter {
		self.as_mut_bitslice().into_iter()
	}
}

/** An iterator that moves out of a [`BitVec`].

This `struct` is created by the [`into_iter`] method on [`BitVec`] (provided by
the [`IntoIterator`] trait).

# Original

[`vec::IntoIter`](alloc::vec::IntoIter)

[`BitVec`]: crate::vec::BitVec
[`IntoIterator`]: core::iter::IntoIterator
[`into_iter`]: core::iter::IntoIterator::into_iter
**/
pub struct IntoIter<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// The base address of the allocation.
	base: NonNull<T>,
	/// The allocation capacity, measured in elements `T`.
	capa: usize,
	/// A [`BitSlice`] iterator over the vector’s contents.
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	iter: Iter<'static, O, T>,
}

impl<O, T> IntoIter<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// Constructs an iterator over a [`BitVec`].
	///
	/// [`BitVec`]: crate::vec::BitVec
	fn new(bv: BitVec<O, T>) -> Self {
		let capa = bv.capacity;
		//  Construct a `BitSlice` iterator over the region, and detach its
		//  lifetime.
		let iter = bv.bitspan.to_bitslice_ref().iter();
		//  Only the allocation’s base and capacity need to be kept for `Drop`.
		let base = bv.bitspan.address().to_nonnull();
		mem::forget(bv);
		Self { base, capa, iter }
	}

	/// Returns the remaining bits of this iterator as a [`BitSlice`].
	///
	/// # Original
	///
	/// [`vec::IntoIter::as_slice`](alloc::vec::IntoIter::as_slice)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bv = bitvec![0, 1, 0, 1];
	/// let mut into_iter = bv.into_iter();
	///
	/// assert_eq!(into_iter.as_bitslice(), bits![0, 1, 0, 1]);
	/// let _ = into_iter.next().unwrap();
	/// assert_eq!(into_iter.as_bitslice(), bits![1, 0, 1]);
	/// ```
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	pub fn as_bitslice(&self) -> &BitSlice<O, T> {
		self.iter.as_bitslice()
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpalin_include))]
	#[deprecated = "Use `as_bitslice` to view the underlying slice"]
	pub fn as_slice(&self) -> &BitSlice<O, T> {
		self.as_bitslice()
	}

	/// Returns the remaining bits of this iterator as a mutable [`BitSlice`].
	///
	/// # Original
	///
	/// [`vec::IntoIter::as_mut_slice`](alloc::vec::IntoIter::as_mut_slice)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bv = bitvec![0, 1, 0, 1];
	/// let mut into_iter = bv.into_iter();
	///
	/// assert_eq!(into_iter.as_bitslice(), bits![0, 1, 0, 1]);
	/// into_iter.as_mut_bitslice().set(2, true);
	/// assert!(!into_iter.next().unwrap());
	/// assert!(into_iter.next().unwrap());
	/// assert!(into_iter.next().unwrap());
	/// ```
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	pub fn as_mut_bitslice(&mut self) -> &mut BitSlice<O, T> {
		let span = self.iter.as_bitslice().as_bitspan();
		let span_mut = unsafe { span.assert_mut() };
		span_mut.to_bitslice_mut()
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Use `as_mut_bitslice` to view the underlying slice"]
	pub fn as_mut_slice(&mut self) -> &mut BitSlice<O, T> {
		self.as_mut_bitslice()
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, T> Debug for IntoIter<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		fmt.debug_tuple("IntoIter")
			.field(&self.as_bitslice())
			.finish()
	}
}

impl<O, T> Iterator for IntoIter<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	type Item = bool;

	fn next(&mut self) -> Option<Self::Item> {
		self.iter.next().as_deref().copied()
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		self.iter.size_hint()
	}

	fn count(self) -> usize {
		self.len()
	}

	fn nth(&mut self, n: usize) -> Option<Self::Item> {
		self.iter.nth(n).as_deref().copied()
	}

	fn last(mut self) -> Option<Self::Item> {
		self.next_back()
	}
}

impl<O, T> DoubleEndedIterator for IntoIter<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn next_back(&mut self) -> Option<Self::Item> {
		self.iter.next_back().as_deref().copied()
	}

	fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
		self.iter.nth_back(n).as_deref().copied()
	}
}

impl<O, T> ExactSizeIterator for IntoIter<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn len(&self) -> usize {
		self.iter.len()
	}
}

impl<O, T> FusedIterator for IntoIter<O, T>
where
	O: BitOrder,
	T: BitStore,
{
}

impl<O, T> Drop for IntoIter<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn drop(&mut self) {
		//  Rebuild the `Vec` governing the allocation, and run its destructor.
		drop(unsafe { Vec::from_raw_parts(self.base.as_ptr(), 0, self.capa) });
	}
}

/** A draining iterator for [`BitVec`].

This `struct` is created by the [`.drain()`] method on [`BitVec`].

# Original

[`vec::Drain`](alloc::vec::Drain)

[`BitVec`]: crate::vec::BitVec
[`.drain()`]: crate::vec::BitVec::drain
**/
pub struct Drain<'a, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// Exclusive reference to the vector this drains.
	source: NonNull<BitVec<O, T>>,
	/// The range of the source vector’s buffer being drained.
	drain: Iter<'a, O, T>,
	/// The range of the source vector’s preserved tail. This runs from the back
	/// edge of the drained region to the vector’s original length.
	tail: Range<usize>,
}

impl<'a, O, T> Drain<'a, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	pub(super) fn new<R>(source: &'a mut BitVec<O, T>, range: R) -> Self
	where R: RangeBounds<usize> {
		//  Hold the current vector size for bounds comparison.
		let len = source.len();
		//  Normalize the input range and assert that it is within bounds.
		let drain = dvl::normalize_range(range, len);
		dvl::assert_range(drain.clone(), len);

		//  The tail region is everything after the drain, before the real end.
		let tail = drain.end .. len;
		//  The drain span is an iterator over the provided range.
		let drain = unsafe {
			//  Set the source vector to end before the drain.
			source.set_len(drain.start);
			//  Grab the drain range and produce an iterator over it.
			source
				.as_bitslice()
				.get_unchecked(drain)
				//  Detach the region from the `source` borrow.
				.as_bitspan()
				.to_bitslice_ref()
				.iter()
		};
		let source = source.into();
		Self {
			source,
			drain,
			tail,
		}
	}

	/// Returns the remaining bits of this iterator as a [`BitSlice`].
	///
	/// # Original
	///
	/// [`Drain::as_slice`](alloc::vec::Drain::as_slice)
	///
	/// # API Differences
	///
	/// This method is renamed, as it operates on a [`BitSlice`] rather than an
	/// element slice.
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	pub fn as_bitslice(&self) -> &'a BitSlice<O, T> {
		self.drain.as_bitslice()
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Use `as_bitslice` to view the underlying slice"]
	pub fn as_slice(&self) -> &BitSlice<O, T> {
		self.as_bitslice()
	}

	/// Attempts to overwrite the drained region with another iterator.
	///
	/// # Type Parameters
	///
	/// - `I`: Some source of `bool`s.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `iter`: A source of `bool`s with which to overwrite the drained span.
	///
	/// # Returns
	///
	/// Whether the drained span was completely filled, or if the replacement
	/// source `iter`ator was exhausted first.
	///
	/// # Effects
	///
	/// The source vector is extended to include all bits filled in from the
	/// replacement `iter`ator, but is *not* extended to include the tail, even
	/// if drained region is completely filled. This work is done in the
	/// destructor.
	fn fill<I>(&mut self, iter: &mut I) -> FillStatus
	where I: Iterator<Item = bool> {
		let bitvec = unsafe { self.source.as_mut() };
		//  Get the length of the source vector. This will be grown as `iter`
		//  writes into the drain span.
		let mut len = bitvec.len();
		//  Get the drain span as a bit-slice.
		let span = unsafe { bitvec.get_unchecked_mut(len .. self.tail.start) };

		//  Set the exit flag to assume completion.
		let mut out = FillStatus::FullSpan;
		//  Write the `iter` bits into the drain `span`.
		for slot in span {
			//  While the `iter` is not exhausted, write it into the span and
			//  increase the vector length counter.
			if let Some(bit) = iter.next() {
				slot.set(bit);
				len += 1;
			}
			//  If the `iter` exhausts before the drain `span` is filled, set
			//  the exit flag accordingly.
			else {
				out = FillStatus::EmptyInput;
				break;
			}
		}
		//  Update the vector length counter to include the bits written by
		//  `iter`.
		unsafe {
			bitvec.set_len(len);
		}
		out
	}

	/// Inserts `additional` capacity between the vector and the tail.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `additional`: The amount of new bits to reserve between the head and
	///   tail sections of the vector.
	///
	/// # Effects
	///
	/// This is permitted to reällocate the buffer in order to grow capacity.
	/// After completion, the tail segment will be relocated to begin
	/// `additional` bits after the head segment ends. The drain iteration
	/// cursor will not be modified.
	unsafe fn move_tail(&mut self, additional: usize) {
		if additional == 0 {
			return;
		}

		let bitvec = self.source.as_mut();
		let tail_len = self.tail.end - self.tail.start;

		//  Reserve allocation capacity for `additional` and the tail.
		//  `.reserve()` begins from the `bitvec.len()`, so the tail length must
		//  still be included.
		let full_len = additional + tail_len;
		bitvec.reserve(full_len);
		let new_tail_start = additional + self.tail.start;
		let orig_tail = mem::replace(
			&mut self.tail,
			new_tail_start .. new_tail_start + tail_len,
		);
		//  Temporarily resize the vector to include the full buffer. This is
		//  necessary until `copy_within_unchecked` stops using `.len()`
		//  internally.
		let len = bitvec.len();
		bitvec.set_len(full_len);
		bitvec.copy_within_unchecked(orig_tail, new_tail_start);
		bitvec.set_len(len);
	}
}

impl<O, T> AsRef<BitSlice<O, T>> for Drain<'_, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn as_ref(&self) -> &BitSlice<O, T> {
		self.as_bitslice()
	}
}

#[cfg(not(tarpaulin_include))]
impl<'a, O, T> Debug for Drain<'a, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		fmt.debug_tuple("Drain")
			.field(&self.drain.as_bitslice())
			.finish()
	}
}

impl<O, T> Iterator for Drain<'_, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	type Item = bool;

	fn next(&mut self) -> Option<Self::Item> {
		self.drain.next().as_deref().copied()
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		self.drain.size_hint()
	}

	fn count(self) -> usize {
		self.len()
	}

	fn nth(&mut self, n: usize) -> Option<Self::Item> {
		self.drain.nth(n).as_deref().copied()
	}

	fn last(mut self) -> Option<Self::Item> {
		self.next_back()
	}
}

impl<O, T> DoubleEndedIterator for Drain<'_, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn next_back(&mut self) -> Option<Self::Item> {
		self.drain.next_back().as_deref().copied()
	}

	fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
		self.drain.nth_back(n).as_deref().copied()
	}
}

impl<O, T> ExactSizeIterator for Drain<'_, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn len(&self) -> usize {
		self.drain.len()
	}
}

impl<O, T> FusedIterator for Drain<'_, O, T>
where
	O: BitOrder,
	T: BitStore,
{
}

unsafe impl<O, T> Send for Drain<'_, O, T>
where
	O: BitOrder,
	T: BitStore,
{
}

unsafe impl<O, T> Sync for Drain<'_, O, T>
where
	O: BitOrder,
	T: BitStore,
{
}

impl<O, T> Drop for Drain<'_, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn drop(&mut self) {
		//  Grab the tail range descriptor
		let tail = self.tail.clone();
		//  And compute its length.
		let tail_len = tail.end - tail.start;
		//  If the tail region is empty, then there is no cleanup work to do.
		if tail_len == 0 {
			return;
		}
		//  Otherwise, access the source vector,
		let bitvec = unsafe { self.source.as_mut() };
		//  And grab its current end.
		let old_len = bitvec.len();
		let new_len = old_len + tail_len;
		unsafe {
			//  Expand the vector to include where the tail bits will be.
			bitvec.set_len(new_len);
			//  Then move the tail bits into the new location.
			bitvec.copy_within_unchecked(tail, old_len);
			//  This ordering is important! `copy_within_unchecked` uses the
			//  `len` boundary.
		}
	}
}

/** `std` uses a `bool` flag for done/not done, which is less clear about what
it signals.

See <https://github.com/rust-lang/rust/blob/5779815/library/alloc/src/vec.rs#L3327-L3348>.
**/
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum FillStatus {
	/// The drain span is completely filled.
	FullSpan   = 0,
	/// The replacement source is completely emptied.
	EmptyInput = 1,
}

/** A splicing iterator for [`BitVec`].

This struct is created by the [`.splice()`] method on [`BitVec`]. See its
documentation for more.

# Original

[`vec::Splice`](alloc::vec::Splice)

[`BitVec`]: crate::vec::BitVec
[`.splice()`]: crate::vec::BitVec::splice
**/
#[derive(Debug)]
pub struct Splice<'a, O, T, I>
where
	O: BitOrder,
	T: BitStore,
	I: Iterator<Item = bool>,
{
	/// The region of the vector being spliced.
	drain: Drain<'a, O, T>,
	/// The bitstream to be written into the drain.
	splice: I,
}

impl<'a, O, T, I> Splice<'a, O, T, I>
where
	O: BitOrder,
	T: BitStore,
	I: Iterator<Item = bool>,
{
	/// Constructs a splice out of a drain and a replacement.
	pub(super) fn new<II>(drain: Drain<'a, O, T>, splice: II) -> Self
	where II: IntoIterator<IntoIter = I, Item = bool> {
		let splice = splice.into_iter();
		Self { drain, splice }
	}
}

impl<O, T, I> Iterator for Splice<'_, O, T, I>
where
	O: BitOrder,
	T: BitStore,
	I: Iterator<Item = bool>,
{
	type Item = bool;

	fn next(&mut self) -> Option<Self::Item> {
		self.drain.next().tap_some(|_| {
			/* Attempt to write a bit into the now-vacated slot at the front of
			the `Drain`. If the `splice` stream produces a bit, then it is
			written into the end of the `Drain`’s buffer, extending it by one.
			This works because `Drain` always truncates its vector to the front
			edge of the drain region, so `bv.len()` is always the first bit of
			the `Drain` region if the `Drain` is willing to yield a bit.
			*/
			if let Some(bit) = self.splice.next() {
				unsafe {
					let bv = self.drain.source.as_mut();
					let len = bv.len();
					bv.set_len_unchecked(len + 1);
					bv.set_unchecked(len, bit);
				}
			}
		})
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		self.drain.size_hint()
	}

	fn count(self) -> usize {
		self.len()
	}

	fn last(mut self) -> Option<Self::Item> {
		self.next_back()
	}
}

impl<O, T, I> DoubleEndedIterator for Splice<'_, O, T, I>
where
	O: BitOrder,
	T: BitStore,
	I: Iterator<Item = bool>,
{
	fn next_back(&mut self) -> Option<Self::Item> {
		self.drain.next_back()
	}

	fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
		self.drain.nth_back(n)
	}
}

impl<O, T, I> ExactSizeIterator for Splice<'_, O, T, I>
where
	O: BitOrder,
	T: BitStore,
	I: Iterator<Item = bool>,
{
	fn len(&self) -> usize {
		self.drain.len()
	}
}

impl<O, T, I> FusedIterator for Splice<'_, O, T, I>
where
	O: BitOrder,
	T: BitStore,
	I: Iterator<Item = bool>,
{
}

impl<O, T, I> Drop for Splice<'_, O, T, I>
where
	O: BitOrder,
	T: BitStore,
	I: Iterator<Item = bool>,
{
	fn drop(&mut self) {
		let tail = self.drain.tail.clone();
		let tail_len = tail.end - tail.start;
		let bitvec = unsafe { self.drain.source.as_mut() };

		//  If the `drain` has no tail span, then extend the vector with the
		//  splice and exit.
		if tail_len == 0 {
			bitvec.extend(self.splice.by_ref());
			return;
		}

		//  Fill the drained range first. If the `splice` exhausts, then the
		//  `Drain` destructor will handle relocating the vector tail segment.
		if let FillStatus::EmptyInput = self.drain.fill(&mut self.splice) {
			return;
		}

		//  If the `splice` has not yet exhausted, then the `Drain` needs to
		//  adjust to receive its contents.
		let len = match self.splice.size_hint() {
			(n, None) | (_, Some(n)) => n,
		};
		unsafe {
			self.drain.move_tail(len);
		}
		//  Now that the tail has been relocated, fill the `splice` into it. If
		//  this exhausts the `splice`, exit.
		if let FillStatus::EmptyInput = self.drain.fill(&mut self.splice) {
			return;
		}

		/* If the `splice` *still* has bits to provide, then its `.size_hint()`
		is untrustworthy. Collect the `splice` into a vector, then insert the
		vector into the spliced region.
		*/
		let mut collected = self.splice.by_ref().collect::<BitVec>().into_iter();
		let len = collected.len();
		if len > 0 {
			unsafe {
				self.drain.move_tail(len);
			}
			let filled = self.drain.fill(&mut collected);
			debug_assert_eq!(filled, FillStatus::EmptyInput);
			debug_assert_eq!(collected.len(), 0);
		}
	}
}
