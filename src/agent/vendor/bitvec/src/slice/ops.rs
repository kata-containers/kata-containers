//! Port of the `[T]` operator implementations.

use crate::{
	access::BitAccess,
	domain::DomainMut,
	order::BitOrder,
	slice::{
		BitSlice,
		BitSliceIndex,
	},
	store::BitStore,
};

use core::ops::{
	BitAndAssign,
	BitOrAssign,
	BitXorAssign,
	Index,
	IndexMut,
	Not,
	Range,
	RangeFrom,
	RangeFull,
	RangeInclusive,
	RangeTo,
	RangeToInclusive,
};

impl<O, T, Rhs> BitAndAssign<Rhs> for BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
	Rhs: IntoIterator<Item = bool>,
{
	fn bitand_assign(&mut self, rhs: Rhs) {
		let mut iter = rhs.into_iter();
		self.for_each(|_, bit| bit & iter.next().unwrap_or(false));
	}
}

impl<O, T, Rhs> BitOrAssign<Rhs> for BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
	Rhs: IntoIterator<Item = bool>,
{
	fn bitor_assign(&mut self, rhs: Rhs) {
		let mut iter = rhs.into_iter();
		self.for_each(|_, bit| bit | iter.next().unwrap_or(false));
	}
}

impl<O, T, Rhs> BitXorAssign<Rhs> for BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
	Rhs: IntoIterator<Item = bool>,
{
	fn bitxor_assign(&mut self, rhs: Rhs) {
		let mut iter = rhs.into_iter();
		self.for_each(|_, bit| bit ^ iter.next().unwrap_or(false));
	}
}

impl<O, T> Index<usize> for BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	type Output = bool;

	/// Looks up a single bit by semantic index.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![Msb0, u8; 0, 1, 0];
	/// assert!(!bits[0]); // -----^  |  |
	/// assert!( bits[1]); // --------^  |
	/// assert!(!bits[2]); // -----------^
	/// ```
	///
	/// If the index is greater than or equal to the length, indexing will
	/// panic.
	///
	/// The below test will panic when accessing index 1, as only index 0 is
	/// valid.
	///
	/// ```rust,should_panic
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![0,  ];
	/// bits[1]; // --------^
	/// ```
	fn index(&self, index: usize) -> &Self::Output {
		//  Convert the `BitRef` to `&'static bool`
		match *index.index(self) {
			true => &true,
			false => &false,
		}
	}
}

/// Generate `Index`/`Mut` implementations for subslicing.
macro_rules! index {
	($($t:ty),+ $(,)?) => { $(
		impl<O, T> Index<$t> for BitSlice<O, T>
		where
			O: BitOrder,
			T: BitStore,
		{
			type Output = Self;

			fn index(&self, index: $t) -> &Self::Output {
				index.index(self)
			}
		}

		impl<O, T> IndexMut<$t> for BitSlice<O, T>
		where
			O: BitOrder,
			T: BitStore,
		{
			fn index_mut(&mut self, index: $t) -> &mut Self::Output {
				index.index_mut(self)
			}
		}
	)+ };
}

//  Implement `Index`/`Mut` subslicing with all the ranges.
index!(
	Range<usize>,
	RangeFrom<usize>,
	RangeFull,
	RangeInclusive<usize>,
	RangeTo<usize>,
	RangeToInclusive<usize>,
);

impl<'a, O, T> Not for &'a mut BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	type Output = Self;

	fn not(self) -> Self::Output {
		match self.domain_mut() {
			DomainMut::Enclave { head, elem, tail } => {
				elem.invert_bits(O::mask(head, tail));
			},
			DomainMut::Region { head, body, tail } => {
				if let Some((head, elem)) = head {
					elem.invert_bits(O::mask(head, None));
				}
				for elem in body {
					elem.store_value(!elem.load_value());
				}
				if let Some((elem, tail)) = tail {
					elem.invert_bits(O::mask(None, tail));
				}
			},
		}
		self
	}
}
