/*! Well-typed counters and register descriptors.

This module provides session types which encode a strict chain of modification
to map semantic indices within a [`BitSlice`] to the electrical register values
used to interact with the memory bus.

The main advantage of the types in this module is that they provide
register-dependent range requirements for counter values, making it impossible
to have an index out of bounds for a register. They also create a sequence of
type transformations that assure the library about the continued validity of
each value in its surrounding context.

By eliminating public constructors from arbitrary integers, [`bitvec`] can
guarantee that only it can produce seed values, and only trusted functions can
transform their numeric values or types, until the program reaches the property
that it requires. This chain of assurance means that memory operations can be
confident in the correctness of their actions and effects.

# Type Sequence

The library produces [`BitIdx`] values from region computation. These types
cannot be publicly constructed, and are only ever the result of pointer
analysis. As such, they rely on correctness of the memory regions provided to
library entry points, and those entry points can leverage the Rust type system
to ensure safety there.

[`BitIdx`] is transformed to [`BitPos`] through the [`BitOrder`] trait. The
[`order`] module provides verification functions that implementors can use to
demonstrate correctness. `BitPos` is the seed type that describes memory
operations, and is used to create selection masks [`BitSel`] and [`BitMask`].

[`BitIdx`]: crate::index::BitIdx
[`BitMask`]: crate::index::BitMask
[`BitOrder`]: crate::order::BitOrder
[`BitPos`]: crate::index::BitPos
[`BitSel`]: crate::index::BitSel
[`BitSlice`]: crate::slice::BitSlice
[`bitvec`]: crate
[`order`]: crate::order
!*/

use crate::{
	mem::BitRegister,
	order::BitOrder,
};

use core::{
	any,
	convert::TryFrom,
	fmt::{
		self,
		Binary,
		Debug,
		Display,
		Formatter,
	},
	iter::{
		FusedIterator,
		Sum,
	},
	marker::PhantomData,
	ops::{
		BitAnd,
		BitOr,
		Not,
	},
};

/** A semantic index counter within a register element `R`.

This type is a counter in the ring `0 .. R::BITS`, and serves to mark a semantic
index within some register element. It is a virtual index, and is the stored
value used in pointer encodings to track region start information.

It is translated to an electrical index through the [`BitOrder`] trait. This
virtual index is the only counter that can be used for address computation, and
once lowered to an electrical index through [`BitOrder::at`], the electrical
address can only be used for instruction selection.

# Type Parameters

- `R`: The register element that this index governs.

# Validity

Values of this type are **required** to be in the range `0 .. R::BITS`. Any
value not less than [`R::BITS`] makes the program invalid, and will likely cause
either a crash or incorrect memory access.

# Construction

This type can never be constructed outside of the [`bitvec`] crate. It is passed
in to [`BitOrder`] implementations, which may use it to construct electrical
position, selection, or mask values from it. All values of this type constructed
by [`bitvec`] are known to be correct in their region; no other construction
site can be trusted.

[`BitOrder`]: crate::order::BitOrder
[`BitOrder::at`]: crate::order::BitOrder::at
[`R::BITS`]: crate::mem::BitMemory::BITS
[`bitvec`]: crate
**/
// #[rustc_layout_scalar_valid_range_end(R::BITS)]
#[repr(transparent)]
#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BitIdx<R>
where R: BitRegister
{
	/// Semantic index counter within a register, constrained to `0 .. R::BITS`.
	idx: u8,
	/// Marker for the register type.
	_ty: PhantomData<R>,
}

impl<R> BitIdx<R>
where R: BitRegister
{
	/// The inclusive maximum index within an element `R`.
	pub const LAST: Self = Self {
		idx: R::MASK,
		_ty: PhantomData,
	};
	/// The inclusive minimum index within an element `R`.
	pub const ZERO: Self = Self {
		idx: 0,
		_ty: PhantomData,
	};

	/// Wraps a counter value as a known-good index into an `R` register.
	///
	/// # Parameters
	///
	/// - `value`: The counter value to mark as an index. This must be not less
	///   than [`Self::ZERO`] and not more than [`Self::LAST`].
	///
	/// # Returns
	///
	/// This returns `value`, marked as either a valid or invalid index by
	/// whether or not it is within the valid range `0 .. R::BITS`.
	///
	/// [`Self::LAST`]: Self::LAST
	/// [`Self::ZERO`]: Self::ZERO
	pub fn new(value: u8) -> Result<Self, BitIdxError<R>> {
		if value >= R::BITS {
			return Err(BitIdxError::new(value));
		}
		Ok(unsafe { Self::new_unchecked(value) })
	}

	/// Wraps a counter value as an assumed-good index into an `R` register.
	///
	/// # Parameters
	///
	/// - `value`: The counter value to mark as an index. This must be not less
	///   than [`Self::ZERO`] and not more than [`Self::LAST`].
	///
	/// # Returns
	///
	/// This returns `value`, marked as a valid index.
	///
	/// # Safety
	///
	/// If the `value` is outside the valid range, then the program is
	/// incorrect. Debug builds will panic; release builds do not inspect the
	/// `value`.
	///
	/// [`Self::LAST`]: Self::LAST
	/// [`Self::ZERO`]: Self::ZERO
	pub unsafe fn new_unchecked(value: u8) -> Self {
		debug_assert!(
			value < R::BITS,
			"Bit index {} cannot exceed type width {}",
			value,
			R::BITS,
		);
		Self {
			idx: value,
			_ty: PhantomData,
		}
	}

	/// Casts to a new index type.
	///
	/// This will always succeed if `self.value()` is a valid index in the `S`
	/// register; it will return an error if the `self` index is too wide for
	/// `S`.
	pub fn cast<S>(self) -> Result<BitIdx<S>, BitIdxError<S>>
	where S: BitRegister {
		BitIdx::new(self.value())
	}

	/// Removes the index wrapper, leaving the internal counter.
	#[cfg(not(tarpaulin_include))]
	pub fn value(self) -> u8 {
		self.idx
	}

	/// Increments an index counter, wrapping at the back edge of the register.
	///
	/// # Parameters
	///
	/// - `self`: The index to increment.
	///
	/// # Returns
	///
	/// - `.0`: The next index after `self`.
	/// - `.1`: Indicates that the new index is in the next register.
	pub fn next(self) -> (Self, bool) {
		let next = self.idx + 1;
		(
			unsafe { Self::new_unchecked(next & R::MASK) },
			next == R::BITS,
		)
	}

	/// Decrements an index counter, wrapping at the front edge of the register.
	///
	/// # Parameters
	///
	/// - `self`: The index to decrement.
	///
	/// # Returns
	///
	/// - `.0`: The previous index before `self`.
	/// - `.1`: Indicates that the new index is in the previous register.
	pub fn prev(self) -> (Self, bool) {
		let prev = self.idx.wrapping_sub(1);
		(
			unsafe { Self::new_unchecked(prev & R::MASK) },
			self.idx == 0,
		)
	}

	/// Computes the bit position corresponding to `self` under some ordering.
	///
	/// This forwards to [`O::at::<R>`], which is the only public, safe,
	/// constructor for a position counter.
	///
	/// [`O::at::<R>`]: crate::order::BitOrder::at
	#[cfg(not(tarpaulin_include))]
	pub fn position<O>(self) -> BitPos<R>
	where O: BitOrder {
		O::at::<R>(self)
	}

	/// Computes the bit selector corresponding to `self` under an ordering.
	///
	/// This forwards to [`O::select::<R>`], which is the only public, safe,
	/// constructor for a bit selector.
	///
	/// [`O::select::<R>`]: crate::order::BitOrder::select
	#[cfg(not(tarpaulin_include))]
	pub fn select<O>(self) -> BitSel<R>
	where O: BitOrder {
		O::select::<R>(self)
	}

	/// Computes the bit selector for `self` as an accessor mask.
	///
	/// This is a type-cast over [`Self::select`].
	///
	/// [`Self::select`]: Self::select
	#[cfg(not(tarpaulin_include))]
	pub fn mask<O>(self) -> BitMask<R>
	where O: BitOrder {
		self.select::<O>().mask()
	}

	/// Iterates over all indices between an inclusive start and exclusive end
	/// point.
	///
	/// Because implementation details of the range type family, including the
	/// [`RangeBounds`] trait, are not yet stable, and heterogenous ranges are
	/// not supported, this must be an opaque iterator rather than a direct
	/// [`Range<BitIdx<R>>`].
	///
	/// # Parameters
	///
	/// - `from`: The inclusive low bound of the range. This will be the first
	///   index produced by the iterator.
	/// - `upto`: The exclusive high bound of the range. The iterator will halt
	///   before yielding an index of this value.
	///
	/// # Returns
	///
	/// An opaque iterator that is equivalent to the range `from .. upto`.
	///
	/// # Requirements
	///
	/// `from` must be no greater than `upto`.
	///
	/// [`RangeBounds`]: core::ops::RangeBounds
	/// [`Range<BitIdx<R>>`]: core::ops::Range
	pub fn range(
		self,
		upto: BitTail<R>,
	) -> impl Iterator<Item = Self>
	+ DoubleEndedIterator
	+ ExactSizeIterator
	+ FusedIterator {
		let (from, upto) = (self.value(), upto.value());
		debug_assert!(from <= upto, "Ranges must run from low to high");
		(from .. upto).map(|val| unsafe { Self::new_unchecked(val) })
	}

	/// Iterates over all possible index values.
	pub fn range_all() -> impl Iterator<Item = Self>
	+ DoubleEndedIterator
	+ ExactSizeIterator
	+ FusedIterator {
		BitIdx::ZERO.range(BitTail::LAST)
	}

	/// Computes the jump distance for some number of bits away from a starting
	/// index.
	///
	/// This computes the number of elements by which to adjust a base pointer,
	/// and then the bit index of the destination bit in the new referent
	/// register element.
	///
	/// # Parameters
	///
	/// - `self`: An index within some element, from which the offset is
	///   computed.
	/// - `by`: The distance by which to jump. Negative values move lower in the
	///   index and element-pointer space; positive values move higher.
	///
	/// # Returns
	///
	/// - `.0`: The number of elements `R` by which to adjust a base pointer.
	///   This value can be passed directly into [`ptr::offset`].
	/// - `.1`: The index of the destination bit within the destination element.
	///
	/// [`ptr::offset`]: https://doc.rust-lang.org/stable/std/primitive.pointer.html#method.offset
	pub fn offset(self, by: isize) -> (isize, Self) {
		let val = self.value();

		/* Signed-add `val` to the jump distance. This will almost certainly not
		overflow (as the crate imposes restrictions well below `isize::MAX`),
		but correctness never hurts. The resulting sum is a bit index (`far`)
		and an overflow marker. Overflow only occurs when a negative `far` is
		the result of a positive `by`, and so `far` must instead be interpreted
		as an unsigned integer.

		`far` is permitted to be negative when `ovf` does not trigger, as `by`
		may be a negative value.

		The number line has its 0 at the front edge of the implicit current
		address, with -1 in index R::MASK at one element address less than the
		implicit current address.
		*/
		let (far, ovf) = by.overflowing_add(val as isize);
		//  If the `isize` addition does not overflow, then the sum can be used
		//  directly.
		if !ovf {
			//  If `far` is in the origin element, then the jump moves zero
			//  elements and produces `far` as an absolute index directly.
			if (0 .. R::BITS as isize).contains(&far) {
				(0, unsafe { Self::new_unchecked(far as u8) })
			}
			/* Otherwise, downshift the bit distance to compute the number of
			elements moved in either direction, and mask to compute the absolute
			bit index in the destination element.
			*/
			else {
				(far >> R::INDX, unsafe {
					Self::new_unchecked(far as u8 & R::MASK)
				})
			}
		}
		else {
			/* Overflowing `isize` addition happens to produce ordinary `usize`
			addition. In point of fact, `isize` addition and `usize` addition
			are the same machine instruction to perform the sum; it is merely
			the signed interpretation of the sum that differs. The sum can be
			recast back to `usize` without issue.
			*/
			let far = far as usize;
			//  This is really only needed in order to prevent sign-extension of
			//  the downshift; once shifted, the value can be safely re-signed.
			((far >> R::INDX) as isize, unsafe {
				Self::new_unchecked(far as u8 & R::MASK)
			})
		}
	}

	/// Computes the span information for a region beginning at `self` for `len`
	/// bits.
	///
	/// The span information is the number of elements in the region that hold
	/// live bits, and the position of the tail marker after the live bits.
	///
	/// This forwards to [`BitTail::span`], as the computation is identical for
	/// the two types. Beginning a span at any `Idx` is equivalent to beginning
	/// it at the tail of a previous span.
	///
	/// # Parameters
	///
	/// - `self`: The start bit of the span.
	/// - `len`: The number of bits in the span.
	///
	/// # Returns
	///
	/// - `.0`: The number of elements, starting in the element that contains
	///   `self`, that contain live bits of the span.
	/// - `.1`: The tail counter of the span’s end point.
	///
	/// [`BitTail::span`]: crate::index::BitTail::span
	pub fn span(self, len: usize) -> (usize, BitTail<R>) {
		unsafe { BitTail::<R>::new_unchecked(self.value()) }.span(len)
	}
}

impl<R> TryFrom<u8> for BitIdx<R>
where R: BitRegister
{
	type Error = BitIdxError<R>;

	fn try_from(value: u8) -> Result<Self, Self::Error> {
		Self::new(value)
	}
}

impl<R> Binary for BitIdx<R>
where R: BitRegister
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		write!(fmt, "{:0>1$b}", self.idx, R::INDX as usize)
	}
}

impl<R> Debug for BitIdx<R>
where R: BitRegister
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		write!(fmt, "BitIdx<{}>({})", any::type_name::<R>(), self)
	}
}

#[cfg(not(tarpaulin_include))]
impl<R> Display for BitIdx<R>
where R: BitRegister
{
	#[inline(always)]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		Binary::fmt(&self, fmt)
	}
}

/// Marks an index that is invalid for a register type.
#[repr(transparent)]
#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BitIdxError<R>
where R: BitRegister
{
	/// The value that is invalid as a [`BitIdx<R>`].
	///
	/// [`BitIdx<R>`]: crate::index::BitIdx
	err: u8,
	/// Marker for the register type.
	_ty: PhantomData<R>,
}

impl<R> BitIdxError<R>
where R: BitRegister
{
	/// Marks a counter value as invalid to be an index for an `R` register.
	///
	/// # Parameters
	///
	/// - `value`: The counter value to mark as an error. This must be greater
	///   than [`BitIdx::<R>::LAST`].
	///
	/// # Returns
	///
	/// This returns `value`, marked as an invalid index for `R`.
	///
	/// # Panics
	///
	/// Debug builds panic when `value` is a valid index for `R`.
	pub(crate) fn new(value: u8) -> Self {
		debug_assert!(
			value >= R::BITS,
			"Bit index {} is valid for type width {}",
			value,
			R::BITS
		);
		Self {
			err: value,
			_ty: PhantomData,
		}
	}

	/// Removes the error wrapper, leaving the internal counter.
	#[cfg(not(tarpaulin_include))]
	pub fn value(self) -> u8 {
		self.err
	}
}

impl<R> Debug for BitIdxError<R>
where R: BitRegister
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		write!(fmt, "BitIdxErr<{}>({})", any::type_name::<R>(), self.err)
	}
}

#[cfg(not(tarpaulin_include))]
impl<R> Display for BitIdxError<R>
where R: BitRegister
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		write!(
			fmt,
			"The value {} is too large to index into {} ({} bits)",
			self.err,
			any::type_name::<R>(),
			R::BITS
		)
	}
}

#[cfg(feature = "std")]
impl<R> std::error::Error for BitIdxError<R> where R: BitRegister
{
}

/** A semantic index counter within *or one bit past the end of* a register
element `R`.

This type is a counter in the ring `0 ..= R::BITS`, and serves to mark a
semantic index of a dead bit *after* a live region. As such, following in the
C++ and LLVM memory model of first-live/first-dead region descriptiors, it marks
an endpoint outside some bit-region, and may be used to compute the startpoint
of a bit-region immediately succeeding, but not overlapping, the source.

As a dead-bit index, this *cannot* be used for indexing into a register. It is
used only in abstract region computation.

This type is necessary in order to preserve the distinction between a dead
memory address that is *not* part of a buffer and a live memory address that is
within a region. [`BitIdx`] is insufficient to this task, and causes buffer
management errors when used in its stead.

# Type Parameters

- `R`: The register element that this end index governs.

# Validity

Values of this type are **required** to be in the range `0 ..= R::BITS`. Any
value greater than [`R::BITS`] makes the program invalid, and will likely cause
either a crash or incorrect memory access.

# Construction

This type can only be publicly constructed through [`BitIdx::span`].

[`BitIdx`]: crate::index::BitIdx
[`R::BITS`]: crate::mem::BitMemory::BITS
**/
#[repr(transparent)]
#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BitTail<R>
where R: BitRegister
{
	/// Semantic tail counter within or after a register, contained to `0 ..=
	/// R::BITS`.
	end: u8,
	/// Marker for the register type.
	_ty: PhantomData<R>,
}

impl<R> BitTail<R>
where R: BitRegister
{
	/// The inclusive maximum tail within an element `R`.
	pub const LAST: Self = Self {
		end: R::BITS,
		_ty: PhantomData,
	};
	/// The inclusive minimum tail within an element `R`.
	pub const ZERO: Self = Self {
		end: 0,
		_ty: PhantomData,
	};

	/// Wraps a counter value as a known-good tail of an `R` register.
	///
	/// # Parameters
	///
	/// - `value`: The counter value to mark as a tail. This must be not less
	///   than [`Self::ZERO`] and not more than [`Self::LAST`].
	///
	/// # Returns
	///
	/// This returns `Some(value)` when it is in the valid range `0 .. R::BITS`,
	/// and `None` when it is not.
	///
	/// [`Self::LAST`]: Self::LAST
	/// [`Self::ZERO`]: Self::ZERO
	pub fn new(value: u8) -> Option<Self> {
		if value > R::BITS {
			return None;
		}
		Some(unsafe { Self::new_unchecked(value) })
	}

	/// Wraps a counter value as an assumed-good tail of an `R` register.
	///
	/// # Parameters
	///
	/// - `value`: The counter value to mark as a tail. This must be not less
	///   than [`Self::ZERO` and not more than [`Self::LAST`].
	///
	/// # Returns
	///
	/// This returns `value`, marked as a valid tail.
	///
	/// # Safety
	///
	/// If the `value` is outside the valid range, then the program is
	/// incorrect. Debug builds will panic; release builds do not inspect the
	/// `value`.
	///
	/// [`Self::LAST`]: Self::LAST
	/// [`Self::ZERO`]: Self::ZERO
	pub(crate) unsafe fn new_unchecked(value: u8) -> Self {
		debug_assert!(
			value <= R::BITS,
			"Bit tail {} cannot exceed type width {}",
			value,
			R::BITS,
		);
		Self {
			end: value,
			_ty: PhantomData,
		}
	}

	/// Removes the tail wrapper, leaving the internal counter.
	#[cfg(not(tarpaulin_include))]
	pub fn value(self) -> u8 {
		self.end
	}

	/// Iterates over all tail indices at and after an inclusive starting point.
	///
	/// Because implementation details of the range type family, including the
	/// [`RangeBounds`] trait, are not yet stable, and heterogenous ranges are
	/// not yet supported, this must be an opaque iterator rather than a direct
	/// [`Range<BitTail<R>>`].
	///
	/// # Parameters
	///
	/// - `from`: The inclusive low bound of the range. This will be the first
	///   tail produced by the iterator.
	///
	/// # Returns
	///
	/// An opaque iterator that is equivalent to the range `from ..=
	/// Self::LAST`.
	///
	/// [`RangeBounds`]: core::ops::RangeBounds
	/// [`Range<BitTail<R>>`]: core::ops::Range
	pub fn range_from(
		from: BitIdx<R>,
	) -> impl Iterator<Item = Self>
	+ DoubleEndedIterator
	+ ExactSizeIterator
	+ FusedIterator {
		(from.idx ..= Self::LAST.end)
			.map(|tail| unsafe { BitTail::new_unchecked(tail) })
	}

	/// Computes the span information for a region beginning immediately after a
	/// preceding region.
	///
	/// The computed region of `len` bits has its start at the *live* bit that
	/// corresponds to the `self` dead tail. The return value is the number of
	/// memory elements containing live bits of the computed span and its tail
	/// marker.
	///
	/// # Parameters
	///
	/// - `self`: A dead bit immediately after some region.
	/// - `len`: The number of live bits in the span starting after `self`.
	///
	/// # Returns
	///
	/// - `.0`: The number of elements `R` that contain live bits in the
	///   computed region.
	/// - `.1`: The tail counter of the first dead bit after the new span.
	///
	/// # Behavior
	///
	/// If `len` is `0`, this returns `(0, self)`, as the span has no live bits.
	/// If `self` is [`BitTail::LAST`], then the new region starts at
	/// [`BitIdx::ZERO`] in the next element.
	///
	/// [`BitIdx::ZERO`]: crate::index::BitIdx::ZERO
	/// [`BitTail::LAST`]: crate::index::BitTail::LAST
	pub(crate) fn span(self, len: usize) -> (usize, Self) {
		if len == 0 {
			return (0, self);
		}

		let val = self.end;

		let head = val & R::MASK;
		let bits_in_head = (R::BITS - head) as usize;

		if len <= bits_in_head {
			return (1, unsafe { Self::new_unchecked(head + len as u8) });
		}

		let bits_after_head = len - bits_in_head;
		let elts = bits_after_head >> R::INDX;
		let tail = bits_after_head as u8 & R::MASK;

		let is_zero = (tail == 0) as u8;
		let edges = 2 - is_zero as usize;
		(elts + edges, unsafe {
			Self::new_unchecked((is_zero << R::INDX) | tail)
		})
	}
}

impl<R> Binary for BitTail<R>
where R: BitRegister
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		write!(fmt, "{:0>1$b}", self.end, R::INDX as usize + 1)
	}
}

impl<R> Debug for BitTail<R>
where R: BitRegister
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		write!(fmt, "BitTail<{}>({})", any::type_name::<R>(), self)
	}
}

#[cfg(not(tarpaulin_include))]
impl<R> Display for BitTail<R>
where R: BitRegister
{
	#[inline(always)]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		Binary::fmt(&self, fmt)
	}
}

/** An electrical position counter within a register element `R`.

This type is a counter in the ring `0 .. R::BITS`, and serves to mark an
electrical address of a real bit. It is the shift distance in the expression
`1 << n`. It is only produced by applying a [`BitOrder::at`] transformation to
some [`BitIdx`] produced by this library.

# Type Parameters

- `R`: The register element that this position governs.

# Validity

Values of this type are **required** to be in the range `0 .. R::BITS`. Any
value not less than [`R::BITS`] makes the program invalid, and will likely cause
a crash. In addition, [`BitOrder::at`] has a list of requirements that its
implementations must uphold in order to make construction of this type
semantically correct in a program.

# Construction

This type is publicly constructible. [`bitvec`] will only request its creation
by calling [`BitOrder::at`], and has no sites that can publicly accept untrusted
values.

[`BitIdx`]: crate::index::BitIdx
[`BitOrder::at`]: crate::order::BitOrder::at
[`R::BITS`]: crate::mem::BitMemory::BITS
**/
// #[rustc_layout_scalar_valid_range_end(R::BITS)]
#[repr(transparent)]
#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BitPos<R>
where R: BitRegister
{
	/// Electrical position counter within a register, constrained to `0 ..
	/// R::BITS`.
	pos: u8,
	/// Marker for the register type.
	_ty: PhantomData<R>,
}

impl<R> BitPos<R>
where R: BitRegister
{
	/// Wraps a counter value as a known-good position within an `R` register.
	///
	/// # Parameters
	///
	/// - `value`: The counter value to mark as a position. This must be in the
	///   range `0 .. R::BITS`.
	///
	/// # Returns
	///
	/// This returns `Some(value)` when it is in the valid range `0 .. R::BITS`,
	/// and `None` when it is not.
	pub fn new(value: u8) -> Option<Self> {
		if value >= R::BITS {
			return None;
		}
		Some(unsafe { Self::new_unchecked(value) })
	}

	/// Wraps a counter value as an assumed-good position within an `R`
	/// register.
	///
	/// # Parameters
	///
	/// - `value`: The counter value to mark as a position. This must be in the
	///   range `0 .. R::BITS`.
	///
	/// # Returns
	///
	/// This returns `value`, marked as a valid position.
	///
	/// # Safety
	///
	/// If the `value` is outside the valid range, then the program is
	/// incorrect. Debug builds will panic; release builds do not inspect the
	/// `value`.
	pub unsafe fn new_unchecked(value: u8) -> Self {
		debug_assert!(
			value < R::BITS,
			"Bit position {} cannot exceed type width {}",
			value,
			R::BITS,
		);
		Self {
			pos: value,
			_ty: PhantomData,
		}
	}

	/// Removes the position wrapper, leaving the internal counter.
	#[cfg(not(tarpaulin_include))]
	pub fn value(self) -> u8 {
		self.pos
	}

	/// Computes the bit selector corresponding to `self`.
	///
	/// This is always `1 << self.pos`.
	pub fn select(self) -> BitSel<R> {
		unsafe { BitSel::new_unchecked(R::ONE << self.pos) }
	}

	/// Computes the bit selector for `self` as an accessor mask.
	///
	/// This is a type-cast over [`Self::select`].
	///
	/// [`Self::select`]: Self::select
	#[cfg(not(tarpaulin_include))]
	pub fn mask(self) -> BitMask<R> {
		self.select().mask()
	}

	/// Iterates over all possible position values.
	pub(crate) fn range_all() -> impl Iterator<Item = Self>
	+ DoubleEndedIterator
	+ ExactSizeIterator
	+ FusedIterator {
		BitIdx::<R>::range_all()
			.map(|idx| unsafe { Self::new_unchecked(idx.value()) })
	}
}

impl<R> Binary for BitPos<R>
where R: BitRegister
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		write!(fmt, "{:0>1$b}", self.pos, R::INDX as usize)
	}
}

impl<R> Debug for BitPos<R>
where R: BitRegister
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		write!(fmt, "BitPos<{}>({})", any::type_name::<R>(), self)
	}
}

#[cfg(not(tarpaulin_include))]
impl<R> Display for BitPos<R>
where R: BitRegister
{
	#[inline(always)]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		Binary::fmt(&self, fmt)
	}
}

/** A one-hot selection mask for a register element `R`.

This type selects exactly one bit in a register. It is used to apply test and
write operations into memory.

# Type Parameters

- `R`: The register element this selector governs.

# Validity

Values of this type are required to have exactly one bit set high, and all
others set low.

# Construction

This type is only constructed from the [`BitPos::select`] and
[`BitOrder::select`] functions. It is always equivalent to
`1 << BitPos::unwrap`.

The chain of custody, from known-good [`BitIdx`] values, through proven-good
[`BitOrder`] implementations, into [`BitPos`] and then `BitSel`, proves that
values of this type are always correct to apply to underlying memory.

[`BitIdx`]: crate::index::BitIdx
[`BitOrder`]: crate::order::BitOrder
[`BitOrder::select`]: crate::order::BitOrder::select
[`BitPos`]: crate::index::BitPos
[`BitPos::select`]: crate::index::BitPos::select
**/
// #[rustc_layout_scalar_valid_range_end(R::BITS)]
#[repr(transparent)]
#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BitSel<R>
where R: BitRegister
{
	/// The one-hot selector mask.
	sel: R,
}

impl<R> BitSel<R>
where R: BitRegister
{
	/// Wraps a counter value as a known-good selection of an `R` register.
	///
	/// # Parameters
	///
	/// - `value`: A one-hot selection mask of a bit in an `R` register.
	///
	/// # Returns
	///
	/// If `value` is a power of two (exactly one bit set high and all others
	/// set low), it returns `Some` of a `BitSel` wrapping the `value`.
	///
	/// [`BitOrder::at`]: crate:order::BitOrder::at
	/// [`BitOrder::select`]: crate::order::BitOrder::select
	/// [`BitPos`]: crate::index::BitPos
	pub fn new(value: R) -> Option<Self> {
		if value.count_ones() != 1 {
			return None;
		}
		Some(unsafe { Self::new_unchecked(value) })
	}

	/// Wraps a counter value as an assumed-good selection of an `R` register.
	///
	/// # Parameters
	///
	/// - `value`: A one-hot selection mask of a bit in an `R` register.
	///
	/// # Returns
	///
	/// `value` wrapped in a `BitSel`.
	///
	/// # Safety
	///
	/// `value` **must** be a power of two: one bit set high and all others set
	/// low. In debug builds, invalid `value`s cause a panic; release builds do
	/// not check the input.
	///
	/// This function must only be called in a [`BitOrder::select`]
	/// implementation that is verified to be correct.
	///
	/// [`BitOrder::select`]: crate::order::BitOrder::select
	pub unsafe fn new_unchecked(value: R) -> Self {
		debug_assert!(
			value.count_ones() == 1,
			"Selections are required to have exactly one set bit: {:0>1$b}",
			value,
			R::BITS as usize,
		);
		Self { sel: value }
	}

	/// Removes the selector wrapper, leaving the internal counter.
	#[cfg(not(tarpaulin_include))]
	pub fn value(self) -> R {
		self.sel
	}

	/// Computes a bit-mask for `self`. This is a type-cast.
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn mask(self) -> BitMask<R> {
		BitMask::new(self.sel)
	}

	/// Iterates over all possible selector values.
	pub fn range_all() -> impl Iterator<Item = Self>
	+ DoubleEndedIterator
	+ ExactSizeIterator
	+ FusedIterator {
		BitPos::<R>::range_all().map(BitPos::select)
	}
}

impl<R> Binary for BitSel<R>
where R: BitRegister
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		write!(fmt, "{:0>1$b}", self.sel, R::BITS as usize)
	}
}

impl<R> Debug for BitSel<R>
where R: BitRegister
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		write!(fmt, "BitSel<{}>({})", any::type_name::<R>(), self)
	}
}

#[cfg(not(tarpaulin_include))]
impl<R> Display for BitSel<R>
where R: BitRegister
{
	#[inline(always)]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		Binary::fmt(&self, fmt)
	}
}

/** A multi-bit selection mask for a register `R`.

Unlike [`BitSel`], which enforces a strict one-hot mask encoding, this mask type
permits any number of bits to be set or unset. This is used to accumulate
selections for a batched operation on a register.

# Type Parameters

- `R`: The register element that this masks.

# Construction

This can only be constructed by combining [`BitSel`] selection mask produced
through the [`BitIdx`] and [`BitOrder`] chain of custody.

[`BitIdx`]: crate::index::BitIdx
[`BitOrder`]: crate::order::BitOrder
[`BitSel`]: crate::index::BitSel
**/
#[repr(transparent)]
#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BitMask<R>
where R: BitRegister
{
	/// A mask of any number of bits to select.
	mask: R,
}

impl<R> BitMask<R>
where R: BitRegister
{
	/// A full mask.
	pub const ALL: Self = Self { mask: R::ALL };
	/// An empty mask.
	pub const ZERO: Self = Self { mask: R::ZERO };

	/// Wraps any `R` value as a bit-mask.
	///
	/// This constructor is provided to explicitly declare that an operation is
	/// discarding the numeric value of an integer and instead using it only as
	/// a bit-mask.
	///
	/// # Parameters
	///
	/// - `value`: Some integer to use as a bit-mask.
	///
	/// # Returns
	///
	/// The `value` wrapped as a bit-mask, with its numeric context discarded.
	///
	/// Prefer accumulating [`BitSel`] values using the `Sum` implementation.
	///
	/// # Safety
	///
	/// The `value` must be computed from a set of valid bit positions in the
	/// caller’s context.
	///
	/// [`BitOrder::mask`]: crate::order::BitOrder::mask
	/// [`BitSel`]: crate::index::BitSel
	pub fn new(value: R) -> Self {
		Self { mask: value }
	}

	/// Removes the mask wrapper, leaving the internal value.
	#[cfg(not(tarpaulin_include))]
	pub fn value(self) -> R {
		self.mask
	}

	/// Tests whether the mask contains a given selector bit.
	///
	/// # Parameters
	///
	/// - `&self`
	/// - `sel`: Some single selection bit to test in `self`.
	///
	/// # Returns
	///
	/// Whether `self` is set high at `sel`.
	pub fn test(&self, sel: BitSel<R>) -> bool {
		self.mask & sel.sel != R::ZERO
	}

	/// Inserts a selector bit into an existing mask.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `sel`: A selector bit to set in `self`.
	///
	/// # Effects
	///
	/// The bit at `sel` is set high in `self`.
	pub fn insert(&mut self, sel: BitSel<R>) {
		self.mask |= sel.sel;
	}

	/// Creates a new mask with a selector bit activated.
	///
	/// # Parameters
	///
	/// - `self`
	/// - `sel`: A selector bit to set in `self`
	///
	/// # Returns
	///
	/// A copy of `self`, with `sel` set high.
	pub fn combine(self, sel: BitSel<R>) -> Self {
		Self {
			mask: self.mask | sel.sel,
		}
	}
}

impl<R> Binary for BitMask<R>
where R: BitRegister
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		write!(fmt, "{:0>1$b}", self.mask, R::BITS as usize)
	}
}

impl<R> Debug for BitMask<R>
where R: BitRegister
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		write!(fmt, "BitMask<{}>({})", any::type_name::<R>(), self)
	}
}

#[cfg(not(tarpaulin_include))]
impl<R> Display for BitMask<R>
where R: BitRegister
{
	#[inline(always)]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		Binary::fmt(&self, fmt)
	}
}

impl<R> Sum<BitSel<R>> for BitMask<R>
where R: BitRegister
{
	fn sum<I>(iter: I) -> Self
	where I: Iterator<Item = BitSel<R>> {
		iter.fold(Self::ZERO, Self::combine)
	}
}

impl<R> BitAnd<R> for BitMask<R>
where R: BitRegister
{
	type Output = Self;

	fn bitand(self, rhs: R) -> Self::Output {
		Self {
			mask: self.mask & rhs,
		}
	}
}

impl<R> BitOr<R> for BitMask<R>
where R: BitRegister
{
	type Output = Self;

	fn bitor(self, rhs: R) -> Self::Output {
		Self {
			mask: self.mask | rhs,
		}
	}
}

impl<R> Not for BitMask<R>
where R: BitRegister
{
	type Output = Self;

	fn not(self) -> Self::Output {
		Self { mask: !self.mask }
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::order::Lsb0;
	use tap::conv::TryConv;

	#[test]
	fn index_ctors() {
		for n in 0 .. 8 {
			assert!(BitIdx::<u8>::new(n).is_ok());
			assert!(n.try_conv::<BitIdx<u8>>().is_ok());
		}
		assert!(BitIdx::<u8>::new(8).is_err());
		assert!(8u8.try_conv::<BitIdx<u8>>().is_err());
		for n in 0 .. 16 {
			assert!(BitIdx::<u16>::new(n).is_ok());
		}
		assert!(BitIdx::<u16>::new(16).is_err());
		for n in 0 .. 32 {
			assert!(BitIdx::<u32>::new(n).is_ok());
		}
		assert!(BitIdx::<u32>::new(32).is_err());

		#[cfg(target_pointer_width = "64")]
		{
			for n in 0 .. 64 {
				assert!(BitIdx::<u64>::new(n).is_ok());
			}
			assert!(BitIdx::<u64>::new(64).is_err());
		}

		if cfg!(target_pointer_width = "32") {
			for n in 0 .. 32 {
				assert!(BitIdx::<usize>::new(n).is_ok());
			}
			assert!(BitIdx::<usize>::new(32).is_err());
		}
		else if cfg!(target_pointer_width = "64") {
			for n in 0 .. 64 {
				assert!(BitIdx::<usize>::new(n).is_ok());
			}
			assert!(BitIdx::<usize>::new(64).is_err());
		}
	}

	#[test]
	fn tail_ctors() {
		for n in 0 ..= 8 {
			assert!(BitTail::<u8>::new(n).is_some());
		}
		assert!(BitTail::<u8>::new(9).is_none());
		for n in 0 ..= 16 {
			assert!(BitTail::<u16>::new(n).is_some());
		}
		assert!(BitTail::<u16>::new(17).is_none());
		for n in 0 ..= 32 {
			assert!(BitTail::<u32>::new(n).is_some());
		}
		assert!(BitTail::<u32>::new(33).is_none());

		#[cfg(target_pointer_width = "64")]
		{
			for n in 0 ..= 64 {
				assert!(BitTail::<u64>::new(n).is_some());
			}
			assert!(BitTail::<u64>::new(65).is_none());
		}

		if cfg!(target_pointer_width = "32") {
			for n in 0 ..= 32 {
				assert!(BitTail::<usize>::new(n).is_some());
			}
			assert!(BitTail::<usize>::new(33).is_none());
		}
		else if cfg!(target_pointer_width = "64") {
			for n in 0 ..= 64 {
				assert!(BitTail::<usize>::new(n).is_some());
			}
			assert!(BitTail::<usize>::new(65).is_none());
		}
	}

	#[test]
	fn position_ctors() {
		for n in 0 .. 8 {
			assert!(BitPos::<u8>::new(n).is_some());
		}
		assert!(BitPos::<u8>::new(8).is_none());
		for n in 0 .. 16 {
			assert!(BitPos::<u16>::new(n).is_some());
		}
		assert!(BitPos::<u16>::new(16).is_none());
		for n in 0 .. 32 {
			assert!(BitPos::<u32>::new(n).is_some());
		}
		assert!(BitPos::<u32>::new(32).is_none());

		#[cfg(target_pointer_width = "64")]
		{
			for n in 0 .. 64 {
				assert!(BitPos::<u64>::new(n).is_some());
			}
			assert!(BitPos::<u64>::new(64).is_none());
		}

		if cfg!(target_pointer_width = "32") {
			for n in 0 .. 32 {
				assert!(BitPos::<usize>::new(n).is_some());
			}
			assert!(BitPos::<usize>::new(32).is_none());
		}
		else if cfg!(target_pointer_width = "64") {
			for n in 0 .. 64 {
				assert!(BitPos::<usize>::new(n).is_some());
			}
			assert!(BitPos::<usize>::new(64).is_none());
		}
	}

	#[test]
	fn select_ctors() {
		for n in 0 .. 8 {
			assert!(BitSel::<u8>::new(1 << n).is_some());
		}
		assert!(BitSel::<u8>::new(3).is_none());
		for n in 0 .. 16 {
			assert!(BitSel::<u16>::new(1 << n).is_some());
		}
		assert!(BitSel::<u16>::new(3).is_none());
		for n in 0 .. 32 {
			assert!(BitSel::<u32>::new(1 << n).is_some());
		}
		assert!(BitSel::<u32>::new(3).is_none());

		#[cfg(target_pointer_width = "64")]
		{
			for n in 0 .. 64 {
				assert!(BitSel::<u64>::new(1 << n).is_some());
			}
			assert!(BitSel::<u64>::new(3).is_none());
		}

		if cfg!(target_pointer_width = "32") {
			for n in 0 .. 32 {
				assert!(BitSel::<usize>::new(1 << n).is_some());
			}
			assert!(BitSel::<usize>::new(3).is_none());
		}
		else if cfg!(target_pointer_width = "64") {
			for n in 0 .. 64 {
				assert!(BitSel::<usize>::new(1 << n).is_some());
			}
			assert!(BitSel::<usize>::new(3).is_none());
		}
	}

	#[test]
	fn ranges() {
		let mut range = BitIdx::<u16>::range_all();
		assert_eq!(range.next(), BitIdx::new(0).ok());
		assert_eq!(range.next_back(), BitIdx::new(15).ok());
		assert_eq!(range.count(), 14);

		let mut range = BitTail::<u8>::range_from(BitIdx::new(1).unwrap());
		assert_eq!(range.next(), BitTail::new(1));
		assert_eq!(range.next_back(), BitTail::new(8));
		assert_eq!(range.count(), 6);

		let mut range = BitPos::<u8>::range_all();
		assert_eq!(range.next(), BitPos::new(0));
		assert_eq!(range.next_back(), BitPos::new(7));
		assert_eq!(range.count(), 6);

		let mut range = BitSel::<u8>::range_all();
		assert_eq!(range.next(), BitSel::new(1));
		assert_eq!(range.next_back(), BitSel::new(128));
		assert_eq!(range.count(), 6);
	}

	#[test]
	fn index_cycle() {
		let six = BitIdx::<u8>::new(6).unwrap();
		let (seven, step) = six.next();
		assert_eq!(seven, BitIdx::new(7).unwrap());
		assert!(!step);
		let (zero, step) = seven.next();
		assert_eq!(zero, BitIdx::ZERO);
		assert!(step);
		let (seven, step) = zero.prev();
		assert_eq!(seven, BitIdx::new(7).unwrap());
		assert!(step);
		let (six, step) = seven.prev();
		assert_eq!(six, BitIdx::new(6).unwrap());
		assert!(!step);

		let fourteen = BitIdx::<u16>::new(14).unwrap();
		let (fifteen, step) = fourteen.next();
		assert_eq!(fifteen, BitIdx::new(15).unwrap());
		assert!(!step);
		let (zero, step) = fifteen.next();
		assert_eq!(zero, BitIdx::ZERO);
		assert!(step);
		let (fifteen, step) = zero.prev();
		assert_eq!(fifteen, BitIdx::new(15).unwrap());
		assert!(step);
		let (fourteen, step) = fifteen.prev();
		assert_eq!(fourteen, BitIdx::new(14).unwrap());
		assert!(!step);
	}

	#[test]
	fn jumps() {
		let (jump, head) = BitIdx::<u8>::new(1).unwrap().offset(2);
		assert_eq!(jump, 0);
		assert_eq!(head, BitIdx::new(3).unwrap());

		let (jump, head) = BitIdx::<u8>::LAST.offset(1);
		assert_eq!(jump, 1);
		assert_eq!(head, BitIdx::ZERO);

		let (jump, head) = BitIdx::<u16>::new(10).unwrap().offset(40);
		// 10 is in 0..16; 10+40 is in 48..64
		assert_eq!(jump, 3);
		assert_eq!(head, BitIdx::new(2).unwrap());

		let (jump, head) = BitIdx::<u8>::LAST.offset(isize::MAX);
		assert_eq!(jump, ((isize::MAX as usize + 1) >> 3) as isize);
		assert_eq!(head, BitIdx::LAST.prev().0);

		let (elts, tail) = BitIdx::<u8>::new(4).unwrap().span(0);
		assert_eq!(elts, 0);
		assert_eq!(tail, BitTail::new(4).unwrap());

		let (elts, tail) = BitIdx::<u8>::new(3).unwrap().span(3);
		assert_eq!(elts, 1);
		assert_eq!(tail, BitTail::new(6).unwrap());

		let (elts, tail) = BitIdx::<u16>::new(10).unwrap().span(40);
		assert_eq!(elts, 4);
		assert_eq!(tail, BitTail::new(2).unwrap());
	}

	#[test]
	fn mask_operators() {
		let mut mask = BitIdx::<u8>::new(2)
			.unwrap()
			.range(BitTail::new(5).unwrap())
			.map(BitIdx::select::<Lsb0>)
			.sum::<BitMask<u8>>();
		assert_eq!(mask, BitMask::new(28));
		assert_eq!(mask & 25, BitMask::new(24));
		assert_eq!(mask | 32, BitMask::new(60));
		assert_eq!(!mask, BitMask::new(!28));
		let yes = BitSel::<u8>::new(16).unwrap();
		let no = BitSel::<u8>::new(64).unwrap();
		assert!(mask.test(yes));
		assert!(!mask.test(no));
		mask.insert(no);
		assert!(mask.test(no));
	}

	#[test]
	#[cfg(feature = "alloc")]
	fn render() {
		use crate::order::Msb0;

		#[cfg(not(feature = "std"))]
		use alloc::format;

		assert_eq!(format!("{:?}", BitIdx::<u8>::LAST), "BitIdx<u8>(111)");
		assert_eq!(format!("{:?}", BitIdx::<u16>::LAST), "BitIdx<u16>(1111)");
		assert_eq!(format!("{:?}", BitIdx::<u32>::LAST), "BitIdx<u32>(11111)");

		assert_eq!(
			format!("{:?}", BitIdx::<u8>::new(8).unwrap_err()),
			"BitIdxErr<u8>(8)",
		);
		assert_eq!(
			format!("{:?}", BitIdx::<u16>::new(16).unwrap_err()),
			"BitIdxErr<u16>(16)",
		);
		assert_eq!(
			format!("{:?}", BitIdx::<u32>::new(32).unwrap_err()),
			"BitIdxErr<u32>(32)",
		);

		assert_eq!(format!("{:?}", BitTail::<u8>::LAST), "BitTail<u8>(1000)");
		assert_eq!(format!("{:?}", BitTail::<u16>::LAST), "BitTail<u16>(10000)");
		assert_eq!(
			format!("{:?}", BitTail::<u32>::LAST),
			"BitTail<u32>(100000)",
		);

		assert_eq!(
			format!("{:?}", BitIdx::<u8>::LAST.position::<Msb0>()),
			"BitPos<u8>(000)",
		);
		assert_eq!(
			format!("{:?}", BitIdx::<u16>::LAST.position::<Lsb0>()),
			"BitPos<u16>(1111)",
		);
		assert_eq!(
			format!("{:?}", BitIdx::<u32>::LAST.position::<Msb0>()),
			"BitPos<u32>(00000)",
		);

		assert_eq!(
			format!("{:?}", BitIdx::<u8>::LAST.select::<Msb0>()),
			"BitSel<u8>(00000001)",
		);
		assert_eq!(
			format!("{:?}", BitIdx::<u16>::LAST.select::<Lsb0>()),
			"BitSel<u16>(1000000000000000)",
		);
		assert_eq!(
			format!("{:?}", BitIdx::<u32>::LAST.select::<Msb0>()),
			"BitSel<u32>(00000000000000000000000000000001)",
		);

		assert_eq!(
			format!("{:?}", BitMask::<u8>::new(1 | 4 | 32)),
			"BitMask<u8>(00100101)",
		);
		assert_eq!(
			format!("{:?}", BitMask::<u16>::new(1 | 4 | 32)),
			"BitMask<u16>(0000000000100101)",
		);
		assert_eq!(
			format!("{:?}", BitMask::<u32>::new(1 | 4 | 32)),
			"BitMask<u32>(00000000000000000000000000100101)",
		);

		#[cfg(target_pointer_width = "64")]
		{
			assert_eq!(
				format!("{:?}", BitIdx::<u64>::LAST),
				"BitIdx<u64>(111111)",
			);
			assert_eq!(
				format!("{:?}", BitIdx::<u64>::new(64).unwrap_err()),
				"BitIdxErr<u64>(64)",
			);
			assert_eq!(
				format!("{:?}", BitTail::<u64>::LAST),
				"BitTail<u64>(1000000)",
			);
			assert_eq!(
				format!("{:?}", BitIdx::<u64>::LAST.position::<Lsb0>()),
				"BitPos<u64>(111111)",
			);
			assert_eq!(
				format!("{:?}",BitIdx::<u64>::LAST.select::<Lsb0>()),
				"BitSel<u64>(1000000000000000000000000000000000000000000000000000000000000000)",
			);
			assert_eq!(
				format!("{:?}", BitMask::<u64>::new(1 | 4 | 32)),
				"BitMask<u64>(0000000000000000000000000000000000000000000000000000000000100101)",
			);
		}
	}
}
