/*! Proxy reference for `&mut bool`.

Rust does not allow assignment through a reference type to be anything other
than a direct load from or store to memory, using the value in the reference as
the memory address. As such, this module provides a proxy type which contains a
pointer to a single bit, and acts as a referential façade, similar to the C++
type [`std::bitset<N>::reference`].

[`std::bitset<N>::reference`]: https://en.cppreference.com/w/cpp/utility/bitset/reference
!*/

use crate::{
	mutability::{
		Const,
		Mut,
		Mutability,
	},
	order::{
		BitOrder,
		Lsb0,
	},
	ptr::BitPtr,
	store::BitStore,
};

use core::{
	any::TypeId,
	cell::Cell,
	cmp,
	fmt::{
		self,
		Debug,
		Display,
		Formatter,
		Pointer,
	},
	hash::{
		Hash,
		Hasher,
	},
	marker::PhantomData,
	mem,
	ops::{
		Deref,
		DerefMut,
		Not,
	},
};

/** A proxy reference, equivalent to C++ [`std::bitset<N>::reference`].

This type wraps a `BitPtr` and caches a `bool` in a padding byte. It is then
able to freely produce references to the cached bool, and commits the cache back
to the referent bit location on `drop`.

# Lifetimes

- `'a`: The lifetime of the source `&'a mut BitSlice` that created the `BitRef`.

# Type Parameters

- `M`: The write permission of the reference. When this is `Const`, the
  `DerefMut` implementation is removed, forbidding the proxy from writing back
  to memory.
- `O`: The ordering used to address a bit in memory.
- `T`: The storage type containing the referent bit.

# Quirks

Because this type has both a lifetime and a destructor, it can introduce an
uncommon error condition in Rust. When an expression that produces this type is
in the final expression of a block, including if that expression is used as a
condition in a `match`, `if let`, or `if`, then the compiler will attempt to
extend the drop scope of this type to the outside of the block. This causes a
lifetime mismatch error if the source region from which this proxy is produced
goes out of scope at the end of the block.

If you get a compiler error that this type causes something to be dropped while
borrowed, you can end the borrow by putting any expression-ending syntax element
after the offending expression that produces this type, including a semicolon or
an item definition.

# Examples

```rust
use bitvec::prelude::*;

let bits = bits![mut 0; 2];

let (left, right) = bits.split_at_mut(1);
let mut first = left.get_mut(0).unwrap();
let second = right.get_mut(0).unwrap();

// Referential behavior
*first = true;
// Direct write
second.set(true);

drop(first); // it’s not a reference!
assert_eq!(bits, bits![1; 2]);
```

[`std::bitset<N>::reference`]: https://en.cppreference.com/w/cpp/utility/bitset/reference
**/
// Restore alignemnt properties, since `BitPtr` does not have them.
#[cfg_attr(target_pointer_width = "32", repr(C, align(4)))]
#[cfg_attr(target_pointer_width = "64", repr(C, align(8)))]
#[cfg_attr(
	not(any(target_pointer_width = "32", target_pointer_width = "64")),
	repr(C)
)]
pub struct BitRef<'a, M, O = Lsb0, T = usize>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	/// The proxied address.
	bitptr: BitPtr<M, O, T>,
	/// A local, dereferencable, cache of the proxied bit.
	data: bool,
	/// Pad the structure out to be two words wide.
	_pad: [u8; PADDING],
	/// Attach the lifetime and possibility of mutation.
	_ref: PhantomData<&'a Cell<bool>>,
}

impl<M, O, T> BitRef<'_, M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	/// Converts a bit-pointer into a proxy bit-reference.
	///
	/// The conversion reads from the pointer, then stores the `bool` in a
	/// padding byte.
	///
	/// # Parameters
	///
	/// - `bitptr`: A bit-pointer to turn into a bit-reference.
	///
	/// # Returns
	///
	/// A bit-reference pointing at `bitptr`.
	///
	/// # Safety
	///
	/// The `bitptr` must address a location that is valid for reads and, if `M`
	/// is `Mut`, writes.
	#[inline]
	pub unsafe fn from_bitptr(bitptr: BitPtr<M, O, T>) -> Self {
		let data = bitptr.read();
		Self {
			bitptr,
			data,
			_pad: [0; PADDING],
			_ref: PhantomData,
		}
	}

	/// Removes an alias marking.
	///
	/// This is only safe when the proxy is known to be the only handle to its
	/// referent element during its lifetime.
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub(crate) unsafe fn remove_alias(this: BitRef<M, O, T::Alias>) -> Self {
		Self {
			bitptr: this.bitptr.cast::<T>(),
			data: this.data,
			_pad: [0; PADDING],
			_ref: PhantomData,
		}
	}

	/// Decays the bit-reference to an ordinary bit-pointer.
	///
	/// # Parameters
	///
	/// - `self`
	///
	/// # Returns
	///
	/// The interior bit-pointer, without the associated cache. If this was a
	/// write-capable pointer, then the cached bit is committed to memory before
	/// this method returns.
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn into_bitptr(self) -> BitPtr<M, O, T> {
		self.bitptr
	}
}

impl<O, T> BitRef<'_, Mut, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// Moves `src` into the referenced bit, returning the previous value.
	///
	/// # Original
	///
	/// [`mem::replace`](core::mem::replace)
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn replace(&mut self, src: bool) -> bool {
		mem::replace(&mut self.data, src)
	}

	/// Swaps the values at two mutable locations, without deïnitializing either
	/// one.
	///
	/// # Original
	///
	/// [`mem::swap`](core::mem::swap)
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn swap<O2, T2>(&mut self, other: &mut BitRef<Mut, O2, T2>)
	where
		O2: BitOrder,
		T2: BitStore,
	{
		mem::swap(&mut self.data, &mut other.data)
	}

	/// Writes a bit into the proxied location without an intermediate copy.
	///
	/// This function writes `value` directly into the proxied location, and
	/// does not store `value` in the proxy’s internal cache. This should be
	/// equivalent to the behavior seen when using ordinary [`DerefMut`]
	/// proxying, but the latter depends on compiler optimization.
	///
	/// # Parameters
	///
	/// - `self`: This destroys the proxy, as it becomes invalid when writing
	///   directly to the location without updating the cache.
	/// - `value`: The new bit to write into the proxied slot.
	///
	/// [`DerefMut`]: core::ops::DerefMut
	#[inline]
	pub fn set(mut self, value: bool) {
		self.write(value);
		mem::forget(self);
	}

	/// Commits a bit into memory.
	///
	/// This is the internal function used to drive `.set()` and `.drop()`.
	#[inline]
	fn write(&mut self, value: bool) {
		self.data = value;
		unsafe {
			self.bitptr.write(value);
		}
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, T> Clone for BitRef<'_, Const, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	#[inline(always)]
	fn clone(&self) -> Self {
		Self { ..*self }
	}
}

/// Implement equality by comparing the proxied `bool` values.
impl<M, O, T> Eq for BitRef<'_, M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
}

/// Implement ordering by comparing the proxied `bool` values.
#[cfg(not(tarpaulin_include))]
impl<M, O, T> Ord for BitRef<'_, M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline(always)]
	fn cmp(&self, other: &Self) -> cmp::Ordering {
		self.data.cmp(&other.data)
	}
}

/// Test equality of proxy references by the value of their proxied bit.
///
/// To test equality by address, decay to a [`BitPtr`] with [`into_bitptr`].
///
/// [`BitPtr`]: crate::ptr::BitPtr
/// [`into_bitptr`]: Self::into_bitptr
#[cfg(not(tarpaulin_include))]
impl<M1, M2, O1, O2, T1, T2> PartialEq<BitRef<'_, M2, O2, T2>>
	for BitRef<'_, M1, O1, T1>
where
	M1: Mutability,
	M2: Mutability,
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	#[inline(always)]
	fn eq(&self, other: &BitRef<'_, M2, O2, T2>) -> bool {
		self.data == other.data
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> PartialEq<bool> for BitRef<'_, M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline(always)]
	fn eq(&self, other: &bool) -> bool {
		self.data == *other
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> PartialEq<&bool> for BitRef<'_, M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline(always)]
	fn eq(&self, other: &&bool) -> bool {
		self.data == **other
	}
}

/// Order proxy references by the value of their proxied bit.
///
/// To order by address, decay to a [`BitPtr`] with [`into_bitptr`].
///
/// [`BitPtr`]: crate::ptr::BitPtr
/// [`into_bitptr`]: Self::into_bitptr
#[cfg(not(tarpaulin_include))]
impl<M1, M2, O1, O2, T1, T2> PartialOrd<BitRef<'_, M2, O2, T2>>
	for BitRef<'_, M1, O1, T1>
where
	M1: Mutability,
	M2: Mutability,
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	#[inline(always)]
	fn partial_cmp(
		&self,
		other: &BitRef<'_, M2, O2, T2>,
	) -> Option<cmp::Ordering> {
		self.data.partial_cmp(&other.data)
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> PartialOrd<bool> for BitRef<'_, M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline(always)]
	fn partial_cmp(&self, other: &bool) -> Option<cmp::Ordering> {
		self.data.partial_cmp(other)
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> PartialOrd<&bool> for BitRef<'_, M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline(always)]
	fn partial_cmp(&self, other: &&bool) -> Option<cmp::Ordering> {
		self.data.partial_cmp(*other)
	}
}

impl<M, O, T> Debug for BitRef<'_, M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		unsafe { self.bitptr.span_unchecked(1) }
			.render(fmt, "Ref", &[("bit", &self.data as &dyn Debug)])
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> Display for BitRef<'_, M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline(always)]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		Display::fmt(&self.data, fmt)
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> Pointer for BitRef<'_, M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline(always)]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		Pointer::fmt(&self.bitptr, fmt)
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> Hash for BitRef<'_, M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline(always)]
	fn hash<H>(&self, state: &mut H)
	where H: Hasher {
		self.bitptr.hash(state);
	}
}

// This cannot be implemented until `Drop` is specialized to only
// `<Mut, O, T>`.
// impl<O, T> Copy for BitRef<'_, Const, O, T>
// where O: BitOrder, T: BitStore {}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> Deref for BitRef<'_, M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	type Target = bool;

	#[inline(always)]
	fn deref(&self) -> &Self::Target {
		&self.data
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, T> DerefMut for BitRef<'_, Mut, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	#[inline(always)]
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.data
	}
}

impl<M, O, T> Drop for BitRef<'_, M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	fn drop(&mut self) {
		//  `Drop` cannot specialize, but only mutable proxies can commit to
		//  memory.
		if TypeId::of::<M>() == TypeId::of::<Mut>() {
			let value = self.data;
			unsafe {
				self.bitptr.assert_mut().write(value);
			}
		}
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> Not for BitRef<'_, M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	type Output = bool;

	#[inline(always)]
	fn not(self) -> Self::Output {
		!self.data
	}
}

/// Compute the padding needed to make a packed `(BitPtr, bool)` tuple as wide
/// as a `(*const _, usize)` tuple.
const PADDING: usize = mem::size_of::<*const u8>() + mem::size_of::<usize>()
	- mem::size_of::<BitPtr<Const, Lsb0, usize>>()
	- mem::size_of::<bool>();

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn proxy_ref() {
		let bits = bits![mut 0; 2];
		assert!(bits.not_any());

		let mut proxy = bits.first_mut().unwrap();
		*proxy = true;

		//  We can inspect the cache, but `proxy` locks the entire `bits` for
		//  the duration of its binding, so we cannot observe that the cache is
		//  not written into the main buffer.
		assert!(*proxy);
		drop(proxy);

		//  The proxy commits the cache on drop, releasing its lock on the main
		//  buffer, permitting us to see that the writeback occurred.
		assert!(bits[0]);

		let proxy = bits.get_mut(1).unwrap();
		proxy.set(true);
		assert!(bits[1]);
	}

	#[test]
	#[cfg(feature = "alloc")]
	fn format() {
		use crate::order::Msb0;
		#[cfg(not(feature = "std"))]
		use alloc::format;

		let bits = bits![mut Msb0, u8; 0];
		let mut bit = bits.get_mut(0).unwrap();

		let text = format!("{:?}", bit);
		assert!(text.starts_with("BitRef<bitvec::order::Msb0, u8> { addr: 0x"));
		assert!(text.ends_with(", head: 000, bits: 1, bit: false }"));
		*bit = true;
		let text = format!("{:?}", bit);
		assert!(text.starts_with("BitRef<bitvec::order::Msb0, u8> { addr: 0x"));
		assert!(text.ends_with(", head: 000, bits: 1, bit: true }"));
	}

	#[test]
	fn assert_size() {
		assert_eq!(
			mem::size_of::<BitRef<'static, Const, Lsb0, u8>>(),
			2 * mem::size_of::<usize>(),
		);

		assert_eq!(
			mem::align_of::<BitRef<'static, Const, Lsb0, u8>>(),
			mem::align_of::<*const u8>(),
		);
	}
}
