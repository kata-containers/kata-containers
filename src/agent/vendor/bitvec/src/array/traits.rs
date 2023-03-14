//! Non-operator trait implementations.

use crate::{
	array::{
		iter::IntoIter,
		BitArray,
	},
	index::BitIdx,
	order::BitOrder,
	slice::BitSlice,
	store::BitStore,
	view::BitView,
};

use core::{
	borrow::{
		Borrow,
		BorrowMut,
	},
	cmp,
	convert::TryFrom,
	fmt::{
		self,
		Binary,
		Debug,
		Display,
		Formatter,
		LowerHex,
		Octal,
		UpperHex,
	},
	hash::{
		Hash,
		Hasher,
	},
};

#[cfg(not(tarpaulin_include))]
impl<O, V> Borrow<BitSlice<O, V::Store>> for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	#[inline(always)]
	fn borrow(&self) -> &BitSlice<O, V::Store> {
		self.as_bitslice()
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, V> BorrowMut<BitSlice<O, V::Store>> for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	#[inline(always)]
	fn borrow_mut(&mut self) -> &mut BitSlice<O, V::Store> {
		self.as_mut_bitslice()
	}
}

impl<O, V> Clone for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	#[inline]
	fn clone(&self) -> Self {
		let mut out = Self::zeroed();
		for (dst, src) in
			out.as_mut_raw_slice().iter_mut().zip(self.as_raw_slice())
		{
			dst.store_value(src.load_value());
		}
		out
	}
}

impl<O, V> Eq for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
}

impl<O, V> Ord for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	#[inline]
	fn cmp(&self, other: &Self) -> cmp::Ordering {
		self.as_bitslice().cmp(other.as_bitslice())
	}
}

impl<O, V, T> PartialEq<BitArray<O, V>> for BitSlice<O, T>
where
	O: BitOrder,
	V: BitView,
	T: BitStore,
{
	#[inline]
	fn eq(&self, other: &BitArray<O, V>) -> bool {
		self == other.as_bitslice()
	}
}

impl<O, V, Rhs> PartialEq<Rhs> for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
	Rhs: ?Sized,
	BitSlice<O, V::Store>: PartialEq<Rhs>,
{
	#[inline]
	fn eq(&self, other: &Rhs) -> bool {
		self.as_bitslice() == other
	}
}

impl<O, V, T> PartialOrd<BitArray<O, V>> for BitSlice<O, T>
where
	O: BitOrder,
	V: BitView,
	T: BitStore,
{
	#[inline]
	fn partial_cmp(&self, other: &BitArray<O, V>) -> Option<cmp::Ordering> {
		self.partial_cmp(other.as_bitslice())
	}
}

impl<O, V, Rhs> PartialOrd<Rhs> for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
	Rhs: ?Sized,
	BitSlice<O, V::Store>: PartialOrd<Rhs>,
{
	#[inline]
	fn partial_cmp(&self, other: &Rhs) -> Option<cmp::Ordering> {
		self.as_bitslice().partial_cmp(other)
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, V> AsRef<BitSlice<O, V::Store>> for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	#[inline(always)]
	fn as_ref(&self) -> &BitSlice<O, V::Store> {
		self.as_bitslice()
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, V> AsMut<BitSlice<O, V::Store>> for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	#[inline(always)]
	fn as_mut(&mut self) -> &mut BitSlice<O, V::Store> {
		self.as_mut_bitslice()
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, V> From<V> for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	#[inline(always)]
	fn from(data: V) -> Self {
		Self::new(data)
	}
}

impl<'a, O, V> TryFrom<&'a BitSlice<O, V::Store>> for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	type Error = TryFromBitSliceError<'a, O, V::Store>;

	#[inline]
	fn try_from(src: &'a BitSlice<O, V::Store>) -> Result<Self, Self::Error> {
		if src.len() != V::const_bits() {
			return Self::Error::err(src);
		}
		let mut out = Self::zeroed();
		out.copy_from_bitslice(src);
		Ok(out)
	}
}

impl<'a, O, V> TryFrom<&'a BitSlice<O, V::Store>> for &'a BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	type Error = TryFromBitSliceError<'a, O, V::Store>;

	#[inline]
	fn try_from(src: &'a BitSlice<O, V::Store>) -> Result<Self, Self::Error> {
		let bitspan = src.as_bitspan();
		//  This pointer cast can only happen if the slice is exactly as long as
		//  the array, and is aligned to the front of the element.
		if src.len() != V::const_bits() || bitspan.head() != BitIdx::ZERO {
			return Self::Error::err(src);
		}
		Ok(unsafe { &*(bitspan.address().to_const() as *const BitArray<O, V>) })
	}
}

impl<'a, O, V> TryFrom<&'a mut BitSlice<O, V::Store>> for &'a mut BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	type Error = TryFromBitSliceError<'a, O, V::Store>;

	#[inline]
	fn try_from(
		src: &'a mut BitSlice<O, V::Store>,
	) -> Result<Self, Self::Error> {
		let bitspan = src.as_mut_bitspan();
		if src.len() != V::const_bits() || bitspan.head() != BitIdx::ZERO {
			return Self::Error::err(&*src);
		}
		Ok(unsafe { &mut *(bitspan.address().to_mut() as *mut BitArray<O, V>) })
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, V> Default for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	#[inline(always)]
	fn default() -> Self {
		Self::zeroed()
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, V> Binary for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	#[inline]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		Binary::fmt(self.as_bitslice(), fmt)
	}
}

impl<O, V> Debug for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	#[inline]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		self.as_bitspan().render(fmt, "Array", None)?;
		fmt.write_str(" ")?;
		Display::fmt(self, fmt)
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, V> Display for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	#[inline]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		Display::fmt(self.as_bitslice(), fmt)
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, V> LowerHex for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	#[inline]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		LowerHex::fmt(self.as_bitslice(), fmt)
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, V> Octal for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	#[inline]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		Octal::fmt(self.as_bitslice(), fmt)
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, V> UpperHex for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	#[inline]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		UpperHex::fmt(self.as_bitslice(), fmt)
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, V> Hash for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	#[inline]
	fn hash<H>(&self, hasher: &mut H)
	where H: Hasher {
		self.as_bitslice().hash(hasher)
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, V> IntoIterator for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	type IntoIter = IntoIter<O, V>;
	type Item = bool;

	#[inline(always)]
	fn into_iter(self) -> Self::IntoIter {
		IntoIter::new(self)
	}
}

#[cfg(not(tarpaulin_include))]
impl<'a, O, V> IntoIterator for &'a BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	type IntoIter = <&'a BitSlice<O, V::Store> as IntoIterator>::IntoIter;
	type Item = <&'a BitSlice<O, V::Store> as IntoIterator>::Item;

	#[inline]
	fn into_iter(self) -> Self::IntoIter {
		self.as_bitslice().into_iter()
	}
}

#[cfg(not(tarpaulin_include))]
impl<'a, O, V> IntoIterator for &'a mut BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
	type IntoIter = <&'a mut BitSlice<O, V::Store> as IntoIterator>::IntoIter;
	type Item = <&'a mut BitSlice<O, V::Store> as IntoIterator>::Item;

	#[inline]
	fn into_iter(self) -> Self::IntoIter {
		self.as_mut_bitslice().into_iter()
	}
}

impl<O, V> Copy for BitArray<O, V>
where
	O: BitOrder,
	V: BitView + Copy,
{
}

impl<O, V> Unpin for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
{
}

/** The error type returned when a conversion from a [`BitSlice`] to a
[`BitArray`] fails.

[`BitArray`]: crate::array::BitArray
[`BitSlice`]: crate::slice::BitSlice
**/
#[repr(transparent)]
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct TryFromBitSliceError<'a, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	inner: &'a BitSlice<O, T>,
}

#[cfg(not(tarpaulin_include))]
impl<'a, O, T> TryFromBitSliceError<'a, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	#[inline(always)]
	fn err<A>(inner: &'a BitSlice<O, T>) -> Result<A, Self> {
		Err(Self { inner })
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, T> Debug for TryFromBitSliceError<'_, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	#[inline]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		fmt.debug_struct("TryFromBitSliceError")
			.field("inner", &self.inner)
			.finish()
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, T> Display for TryFromBitSliceError<'_, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	#[inline]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		fmt.write_fmt(format_args!(
			"could not convert bit-slice to bit-array: {:?}",
			self.inner,
		))
	}
}

#[cfg(feature = "std")]
impl<'a, O, T> std::error::Error for TryFromBitSliceError<'a, O, T>
where
	O: BitOrder,
	T: BitStore,
{
}
