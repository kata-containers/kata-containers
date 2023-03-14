//! Implementation of `Range<BitPtr>`.

use crate::{
	mutability::Mutability,
	order::{
		BitOrder,
		Lsb0,
	},
	ptr::{
		BitPtr,
		BitSpan,
	},
	store::BitStore,
};

use core::{
	any::TypeId,
	fmt::{
		self,
		Debug,
		Formatter,
		Pointer,
	},
	hash::{
		Hash,
		Hasher,
	},
	iter::FusedIterator,
	ops::{
		Bound,
		Range,
		RangeBounds,
	},
};

/** Equivalent to `Range<BitPtr<M, O, T>>`.

As with `Range`, this is a half-open set: the starting pointer is included in
the set of live addresses, while the ending pointer is one-past-the-end of live
addresses, and is not usable.

This structure exists because `Range` does not permit foreign implementations of
its internal traits.

# Original

[`Range<*bool>`](core::ops::Range)

# API Differences

This cannot be constructed directly from the `..` syntax, though a `From`
implementation is provided.

# Type Parameters

- `M`: The write permissions of the pointers this range produces.
- `O`: The bit-ordering within a storage element used to access bits.
- `T`: The storage element type containing the referent bits.
**/
// Restore alignemnt properties, since `BitPtr` does not have them.
#[cfg_attr(target_pointer_width = "32", repr(C, align(4)))]
#[cfg_attr(target_pointer_width = "64", repr(C, align(8)))]
#[cfg_attr(
	not(any(target_pointer_width = "32", target_pointer_width = "64")),
	repr(C)
)]
pub struct BitPtrRange<M, O = Lsb0, T = usize>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	/// The lower bound of the range (inclusive).
	pub start: BitPtr<M, O, T>,
	/// The higher bound of the range (exclusive).
	pub end: BitPtr<M, O, T>,
}

impl<M, O, T> BitPtrRange<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	/// The canonical empty range. All ranges with zero length are equally
	/// empty.
	pub const EMPTY: Self = Self {
		start: BitPtr::DANGLING,
		end: BitPtr::DANGLING,
	};

	/// Destructures the range back into its start and end pointers.
	#[inline]
	#[cfg(not(tarpaulin_include))]
	pub fn raw_parts(&self) -> (BitPtr<M, O, T>, BitPtr<M, O, T>) {
		(self.start, self.end)
	}

	/// Converts the structure into an actual `Range`. The `Range` will have
	/// limited functionality compared to `self`.
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn into_range(self) -> Range<BitPtr<M, O, T>> {
		self.start .. self.end
	}

	/// Tests if the range is empty (the distance between pointers is `0`).
	///
	/// # Original
	///
	/// [`Range::is_empty`](core::ops::Range::is_empty)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	/// use bitvec::ptr::BitPtrRange;
	///
	/// let data = 0u8;
	/// let ptr = BitPtr::<_, Lsb0, _>::from_ref(&data);
	/// let mut range = unsafe { ptr.range(1) };
	///
	/// assert!(!range.is_empty());
	/// range.next();
	/// assert!(range.is_empty());
	/// ```
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.start == self.end
	}

	/// Returns `true` if the `pointer` is contained in the range.
	///
	/// # Original
	///
	/// [`Range::contains`](core::ops::Range::contains)
	///
	/// # API Differences
	///
	/// The candidate pointer may differ in mutability permissions and exact
	/// storage type.
	///
	/// If `T2::Mem` is not `T::Mem`, then this always returns `false`. If `T2`
	/// and `T` have the same memory type, but different alias permissions, then
	/// the comparison can continue.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	/// use bitvec::ptr::BitPtrRange;
	/// use core::cell::Cell;
	///
	/// let data = 0u16;
	/// let ptr = BitPtr::<_, Lsb0, _>::from_ref(&data);
	///
	/// let mut range = unsafe { ptr.range(16) };
	/// // Reduce the range contents.
	/// range.nth(2);
	/// range.nth_back(2);
	///
	/// // The start pointer is now excluded, but the interior remains.
	/// assert!(!range.contains(&ptr));
	/// assert!(range.contains(&unsafe { ptr.add(8) }));
	///
	/// // Different base types are always excluded.
	/// let casted = ptr.cast::<u8>();
	/// assert!(!range.contains(&unsafe { casted.add(8) }));
	///
	/// // Casting to a different alias model with the same width is valid.
	/// let casted = ptr.cast::<Cell<u16>>();
	/// assert!(range.contains(&unsafe { casted.add(8) }));
	/// ```
	#[inline]
	pub fn contains<M2, T2>(&self, pointer: &BitPtr<M2, O, T2>) -> bool
	where
		M2: Mutability,
		T2: BitStore,
	{
		self.start <= *pointer && *pointer < self.end
	}

	/// Converts the pair into a single span descriptor over all included bits.
	///
	/// The produced span does *not* include the bit addressed by the end
	/// pointer, as this is an exclusive range.
	#[inline]
	pub(crate) fn into_bitspan(self) -> BitSpan<M, O, T> {
		unsafe { self.start.span_unchecked(self.len()) }
	}

	/// Snapshots the current start pointer for return, then increments the
	/// start.
	///
	/// This method may only be called when the range is non-empty.
	#[inline]
	fn take_front(&mut self) -> BitPtr<M, O, T> {
		let start = self.start;
		self.start = unsafe { start.add(1) };
		start
	}

	/// Decrements the current end pointer, then returns it.
	///
	/// This method may only be called when the range is non-empty.
	#[inline]
	fn take_back(&mut self) -> BitPtr<M, O, T> {
		let prev = unsafe { self.end.sub(1) };
		self.end = prev;
		prev
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> Clone for BitPtrRange<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline(always)]
	fn clone(&self) -> Self {
		Self { ..*self }
	}
}

impl<M, O, T> Eq for BitPtrRange<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
}

#[cfg(not(tarpaulin_include))]
impl<M1, M2, O, T1, T2> PartialEq<BitPtrRange<M2, O, T2>>
	for BitPtrRange<M1, O, T1>
where
	M1: Mutability,
	M2: Mutability,
	O: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	#[inline(always)]
	fn eq(&self, other: &BitPtrRange<M2, O, T2>) -> bool {
		if TypeId::of::<T1::Mem>() != TypeId::of::<T2::Mem>() {
			return false;
		}
		self.start == other.start && self.end == other.end
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> Default for BitPtrRange<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline(always)]
	fn default() -> Self {
		Self::EMPTY
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> From<Range<BitPtr<M, O, T>>> for BitPtrRange<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline(always)]
	fn from(Range { start, end }: Range<BitPtr<M, O, T>>) -> Self {
		Self { start, end }
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> Into<Range<BitPtr<M, O, T>>> for BitPtrRange<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline(always)]
	fn into(self) -> Range<BitPtr<M, O, T>> {
		self.into_range()
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> Debug for BitPtrRange<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		let (start, end) = self.raw_parts();
		Pointer::fmt(&start, fmt)?;
		write!(fmt, "{0}..{0}", if fmt.alternate() { " " } else { "" })?;
		Pointer::fmt(&end, fmt)
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> Hash for BitPtrRange<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline]
	fn hash<H>(&self, state: &mut H)
	where H: Hasher {
		self.start.hash(state);
		self.end.hash(state);
	}
}

impl<M, O, T> Iterator for BitPtrRange<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	type Item = BitPtr<M, O, T>;

	#[inline]
	fn next(&mut self) -> Option<Self::Item> {
		if Self::is_empty(&*self) {
			return None;
		}
		Some(self.take_front())
	}

	#[inline]
	fn nth(&mut self, n: usize) -> Option<Self::Item> {
		if n >= self.len() {
			self.start = self.end;
			return None;
		}
		self.start = unsafe { self.start.add(n) };
		Some(self.take_front())
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		let len = self.len();
		(len, Some(len))
	}

	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	fn count(self) -> usize {
		self.len()
	}

	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	fn last(mut self) -> Option<Self::Item> {
		self.next_back()
	}
}

impl<M, O, T> DoubleEndedIterator for BitPtrRange<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline]
	fn next_back(&mut self) -> Option<Self::Item> {
		if Self::is_empty(&*self) {
			return None;
		}
		Some(self.take_back())
	}

	#[inline]
	fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
		if n >= self.len() {
			self.end = self.start;
			return None;
		}
		let out = unsafe { self.end.sub(n.wrapping_add(1)) };
		self.end = out;
		Some(out)
	}
}

impl<M, O, T> ExactSizeIterator for BitPtrRange<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[cfg_attr(not(tarpaulin_include), inline(always))]
	fn len(&self) -> usize {
		(unsafe { self.end.offset_from(self.start) }) as usize
	}
}

impl<M, O, T> FusedIterator for BitPtrRange<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> RangeBounds<BitPtr<M, O, T>> for BitPtrRange<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline(always)]
	fn start_bound(&self) -> Bound<&BitPtr<M, O, T>> {
		Bound::Included(&self.start)
	}

	#[inline(always)]
	fn end_bound(&self) -> Bound<&BitPtr<M, O, T>> {
		Bound::Excluded(&self.end)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		mutability::Const,
		order::Lsb0,
	};
	use core::mem::size_of;

	#[test]
	fn assert_size() {
		assert!(
			size_of::<BitPtrRange<Const, Lsb0, u8>>() <= 3 * size_of::<usize>()
		);
	}
}
