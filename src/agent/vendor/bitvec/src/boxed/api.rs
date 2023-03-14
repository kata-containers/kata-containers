//! Port of the `Box<[T]>` inherent API.

use crate::{
	boxed::BitBox,
	order::BitOrder,
	ptr::BitSpan,
	slice::BitSlice,
	store::BitStore,
	vec::BitVec,
};

use core::{
	marker::Unpin,
	mem,
	pin::Pin,
};

use tap::pipe::Pipe;

impl<O, T> BitBox<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// Allocates memory on the heap and then copies `x` into it.
	///
	/// This doesn’t actually allocate if `x` is zero-length.
	///
	/// # Original
	///
	/// [`Box::new`](alloc::boxed::Box::new)
	///
	/// # API Differences
	///
	/// `Box::<[T]>::new` does not exist, because unsized types cannot be taken
	/// by value. Instead, this takes a slice reference, and boxes the referent
	/// slice.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let boxed = BitBox::new(bits![0; 5]);
	/// ```
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Prefer `from_bitslice`"]
	pub fn new(x: &BitSlice<O, T>) -> Self {
		Self::from_bitslice(x)
	}

	/// Constructs a new `Pin<BitBox<O, T>>`.
	///
	/// [`BitSlice`] is always [`Unpin`], so this has no actual effect.
	///
	/// # Original
	///
	/// [`Box::pin`](alloc::boxed::Box::pin)
	///
	/// # API Differences
	///
	/// As with [`new`], this only exists on `Box` when `T` is not unsized. This
	/// takes a slice reference, and pins the referent slice.
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	/// [`Unpin`]: core::marker::Unpin
	/// [`new`]: Self::new
	#[inline]
	#[cfg(not(tarpaulin_include))]
	pub fn pin(x: &BitSlice<O, T>) -> Pin<Self>
	where
		O: Unpin,
		T: Unpin,
	{
		x.pipe(Self::from_bitslice).pipe(Pin::new)
	}

	/// Constructs a bit-box from a raw bit-slice pointer.
	///
	/// After calling this function, the raw bit-slice pointer is owned by the
	/// resulting `BitBox`. Specifically, the `BitBox` destructor will free the
	/// memory allocation at the bit-slice pointer’s address. For this to be
	/// safe, the bit-slice pointer can only have been produced by a `BitBox`
	/// previously destroyed using [`into_raw`].
	///
	/// # Original
	///
	/// [`Box::from_raw`](alloc::boxed::Box::from_raw)
	///
	/// # Safety
	///
	/// This function is unsafe because improper use may lead to memory
	/// problems. For example, a double-free may occur if the function is called
	/// twice on the same raw bit-slice pointer.
	///
	/// # Examples
	///
	/// Recreate a `BitBox` which was previously converted to a raw bit-slice
	/// pointer using [`BitBox::into_raw`]:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let x = bitbox![0; 10];
	/// let ptr = BitBox::into_raw(x);
	/// let x = unsafe { BitBox::from_raw(ptr) };
	/// ```
	///
	/// [`BitBox::into_raw`]: Self::into_raw
	/// [`into_raw`]: Self::into_raw
	#[inline]
	pub unsafe fn from_raw(raw: *mut BitSlice<O, T>) -> Self {
		Self {
			bitspan: BitSpan::from_bitslice_ptr_mut(raw),
		}
	}

	/// Consumes the `BitBox`, returning a raw bit-slice pointer.
	///
	/// The bit-slice pointer will be properly encoded and non-null.
	///
	/// After calling this function, the caller is responsible for the memory
	/// previously managed by the `BitBox`. In particular, the caller should
	/// properly release the memory by converting the bit-slice pointer back
	/// into a `BitBox` with the [`from_raw`] function, allowing the `BitBox`
	/// destructor to perform the cleanup.
	///
	/// Note: this is an associated function, which means that you have to call
	/// it as `BitBox::into_raw(b)` instead of `b.into_raw()`. This is to match
	/// signatures with the standard library’s [`Box`] API; there will never be
	/// a name conflict with [`BitSlice`].
	///
	/// # Original
	///
	/// [`Box::into_raw`](alloc::boxed::Box::into_raw)
	///
	/// # Examples
	///
	/// Converting the raw bit-slice pointer back into a `BitBox` with
	/// [`BitBox::from_raw`] for automatic cleanup:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let x = bitbox![0; 50];
	/// let p = BitBox::into_raw(x);
	/// let x = unsafe { BitBox::from_raw(p) };
	/// ```
	///
	/// You may not deällocate pointers produced by this function through any
	/// other manner.
	///
	/// [`BitBox::from_raw`]: Self::from_raw
	/// [`BitSlice`]: crate::slice::BitSlice
	/// [`Box`]: alloc::boxed::Box
	/// [`from_raw`]: Self::from_raw
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn into_raw(this: Self) -> *mut BitSlice<O, T> {
		Self::leak(this)
	}

	/// Consumes and leaks the `BitBox`, returning a mutable reference, `&'a mut
	/// BitSlice<O, T>`. This is eligible to be promoted to the `'static`
	/// lifetime.
	///
	/// # Original
	///
	/// [`Box::leak`](alloc::boxed::Box::leak)
	///
	/// This function is mainly useful for data that lives for the remainder
	/// of the program’s life. Dropping the returned reference will cause a
	/// memory leak. If this is not acceptable, the reference should first be
	/// wrapped with the [`BitBox::from_raw`] function producing a `BitBox`.
	/// This `BitBox` can then be dropped which will properly deällocate the
	/// memory.
	///
	/// Note: this is an associated function, which means that you have to call
	/// it as `BitBox::leak(b)` instead of `b.leak()`. This is to match
	/// signatures with the standard library’s [`Box`] API; there will never be
	/// a name conflict with [`BitSlice`].
	///
	/// # Original
	///
	/// [`Box::leak`](alloc::boxed::Box::leak)
	///
	/// # Examples
	///
	/// Simple usage:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let b = bitbox![0; 50];
	/// let static_ref: &'static mut BitSlice = BitBox::leak(b);
	/// static_ref.set(0, true);
	/// assert!(static_ref[0]);
	/// # drop(unsafe { BitBox::from_raw(static_ref) });
	/// ```
	///
	/// [`BitBox::from_raw`]: Self::from_raw
	/// [`BitSlice`]: crate::slice::BitSlice
	/// [`Box`]: alloc::boxed::Box
	#[inline]
	pub fn leak<'a>(this: Self) -> &'a mut BitSlice<O, T>
	where T: 'a {
		let out = this.bitspan.to_bitslice_mut();
		mem::forget(this);
		out
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Prefer `into_bitvec`"]
	pub fn into_vec(self) -> BitVec<O, T> {
		self.into_bitvec()
	}
}
