//! Port of the `Vec<T>` operator implementations.

use crate::{
	order::BitOrder,
	slice::BitSlice,
	store::BitStore,
	vec::BitVec,
};

use core::{
	mem::ManuallyDrop,
	ops::{
		BitAnd,
		BitAndAssign,
		BitOr,
		BitOrAssign,
		BitXor,
		BitXorAssign,
		Deref,
		DerefMut,
		Index,
		IndexMut,
		Not,
	},
};

impl<O, T, Rhs> BitAnd<Rhs> for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
	BitSlice<O, T>: BitAndAssign<Rhs>,
{
	type Output = Self;

	fn bitand(mut self, rhs: Rhs) -> Self::Output {
		self &= rhs;
		self
	}
}

impl<O, T, Rhs> BitAndAssign<Rhs> for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
	BitSlice<O, T>: BitAndAssign<Rhs>,
{
	fn bitand_assign(&mut self, rhs: Rhs) {
		*self.as_mut_bitslice() &= rhs;
	}
}

impl<O, T, Rhs> BitOr<Rhs> for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
	BitSlice<O, T>: BitOrAssign<Rhs>,
{
	type Output = Self;

	fn bitor(mut self, rhs: Rhs) -> Self::Output {
		self |= rhs;
		self
	}
}

impl<O, T, Rhs> BitOrAssign<Rhs> for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
	BitSlice<O, T>: BitOrAssign<Rhs>,
{
	fn bitor_assign(&mut self, rhs: Rhs) {
		*self.as_mut_bitslice() |= rhs;
	}
}

impl<O, T, Rhs> BitXor<Rhs> for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
	BitSlice<O, T>: BitXorAssign<Rhs>,
{
	type Output = Self;

	fn bitxor(mut self, rhs: Rhs) -> Self::Output {
		self ^= rhs;
		self
	}
}

impl<O, T, Rhs> BitXorAssign<Rhs> for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
	BitSlice<O, T>: BitXorAssign<Rhs>,
{
	fn bitxor_assign(&mut self, rhs: Rhs) {
		*self.as_mut_bitslice() ^= rhs;
	}
}

impl<O, T> Deref for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	type Target = BitSlice<O, T>;

	fn deref(&self) -> &Self::Target {
		self.as_bitslice()
	}
}

impl<O, T> DerefMut for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.as_mut_bitslice()
	}
}

impl<O, T> Drop for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn drop(&mut self) {
		//  Run the `Vec` destructor to deällocate the buffer.
		self.with_vec(|slot| unsafe { ManuallyDrop::drop(slot) });
	}
}

impl<O, T, Idx> Index<Idx> for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
	BitSlice<O, T>: Index<Idx>,
{
	type Output = <BitSlice<O, T> as Index<Idx>>::Output;

	fn index(&self, index: Idx) -> &Self::Output {
		self.as_bitslice().index(index)
	}
}

impl<O, T, Idx> IndexMut<Idx> for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
	BitSlice<O, T>: IndexMut<Idx>,
{
	fn index_mut(&mut self, index: Idx) -> &mut Self::Output {
		self.as_mut_bitslice().index_mut(index)
	}
}

/** This implementation inverts all elements in the live buffer. You cannot rely
on the value of bits in the buffer that are outside the domain of
`BitVec::as_mit_bitslice`.
**/
impl<O, T> Not for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	type Output = Self;

	fn not(mut self) -> Self::Output {
		for elem in self.as_mut_raw_slice() {
			elem.store_value(!elem.load_value())
		}
		self
	}
}
