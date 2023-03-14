/*! A dynamically-allocated buffer containing a [`BitSlice`] region.

You can read the standard library’s [`alloc::vec` module documentation][std]
here.

This module defines the [`BitVec`] buffer, and all of its associated support
code.

[`BitVec`] is equivalent to [`Vec<bool>`], in its operation and in its
relationship to the [`BitSlice`] type. Most of the interesting work to be done
on a bit-sequence is implemented in `BitSlice`, to which `BitVec` dereferences,
and the vector container itself only exists to maintain ownership, implement
dynamic resizing, and provide some specializations that cannot safely be done on
`BitSlice` alone.

[`BitSlice`]: crate::slice::BitSlice
[`BitVec`]: crate::vec::BitVec
[`Vec<bool>`]: alloc::vec::Vec
[std]: mod@alloc::vec
!*/

#![cfg(feature = "alloc")]

use crate::{
	boxed::BitBox,
	domain::Domain,
	index::BitIdx,
	mem::{
		BitMemory,
		BitRegister,
	},
	mutability::{
		Const,
		Mut,
	},
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
};

use alloc::vec::Vec;

use core::{
	mem::{
		self,
		ManuallyDrop,
	},
	slice,
};

use funty::IsInteger;

use tap::{
	pipe::Pipe,
	tap::Tap,
};

/** A contiguous growable array of bits.

This is a managed, heap-allocated, buffer that contains a [`BitSlice`] region.
It is analagous to [`Vec<bool>`], and is written to be very nearly a drop-in
replacement for it. This type contains little interesting behavior in its own
right; most of its behavior is provided by dereferencing to its managed
[`BitSlice`] buffer. It instead serves primarily as an interface to the
allocator, and has some specialized behaviors for its fully-owned memory buffer.

# Documentation

All APIs that mirror something in the standard library will have an `Original`
section linking to the corresponding item. All APIs that have a different
signature or behavior than the original will have an `API Differences` section
explaining what has changed, and how to adapt your existing code to the change.

These sections look like this:

# Original

[`Vec<T>`](alloc::vec::Vec)

# API Differences

The buffer type [`Vec<bool>`] has no type parameters. `BitVec<O, T>` has the
same two type parameters as [`BitSlice<O, T>`][`BitSlice`]. Otherwise, `BitVec`
is able to implement the full API surface of `Vec<bool>`.

# Examples

Because `BitVec` takes type parameters, but has default type arguments for them,
you will need to specify its type parameters when using its associated
functions. The easiest way to do this is to declare bindings type as `: BitVec`,
which uses the default type arguments.

```rust
use bitvec::prelude::*;

let mut bv: BitVec = BitVec::new();
bv.push(false);
bv.push(true);

assert_eq!(bv.len(), 2);
assert_eq!(bv[0], false);

assert_eq!(bv.pop(), Some(true));
assert_eq!(bv.len(), 1);

// `BitVec` cannot yet support `[]=` write indexing.
*bv.get_mut(0).unwrap() = true;
assert_eq!(bv[0], true);

bv.extend(bits![0, 1, 0]);

for bit in &bv {
  println!("{}", bit);
}
assert_eq!(bv, bits![1, 0, 1, 0]);
```

The [`bitvec!`] macro is provided to make initialization more convenient:

```rust
use bitvec::prelude::*;

let mut bv = bitvec![0, 0, 1];
bv.push(true);
assert_eq!(bv, bits![0, 0, 1, 1]);
```

It has the same argument syntax as [`vec!`]. In addition, it can take type
arguments for ordering and storage:

```rust
use bitvec::prelude::*;

let bv = bitvec![Msb0, u16; 1; 30];
assert!(bv.all());
assert_eq!(bv.len(), 30);
```

# Indexing

The `BitVec` type allows you to access bits by index, because it implements the
[`Index`] trait. However, because [`IndexMut`] requires producing an `&mut bool`
reference, it cannot implement `[]=` index assignment syntax. Instead, you must
use [`get_mut`] or [`get_unchecked_mut`] to produce proxy types that can serve
the same purpose.

# Slicing

A `BitVec` is resizable, while [`BitSlice`] is a fixed-size view of a buffer.
Just as with ordinary [`Vec`]s and slices, you can get a `BitSlice` from a
`BitVec` by borrowing it:

```rust
use bitvec::prelude::*;

fn read_bitslice(slice: &BitSlice) {
  // …
}

let bv = bitvec![0; 30];
read_bitslice(&bv);

// … and that’s all!
// you can also do it like this:
let x: &BitSlice = &bv;
```

As with ordinary Rust types, you should prefer passing bit-slices rather than
buffers when you just want to inspect the data, and not manage the underlying
memory region.

# Behavior

Because `BitVec` is a fully-owned buffer, it is able to operate on its memory
without concern for any other views that may alias. This enables it to
specialize some [`BitSlice`] behavior to be faster or more efficient. However,
`BitVec` is *not* restricted to only using unaliased integer storage, and
technically permits the construction of `BitVec<_, AtomicType>`.

This restriction is extremely awkward and constraining to write in the library,
and clients will probably never attempt to construct them, but the possibility
is still present. Be aware of this possibility when using generic code to
convert from `BitSlice` to `BitVec`. Fully-typed code does not need to be
concerned with this possibility.

# Capacity and Reällocation

The capacity of a bit-vector is the amount of space allocated for any future
bits that will be added onto the vector. This is not to be confused with the
*length* of a vector, which specifies the number of actual bits within the
vector. If a vector’s length exceeds its capacity, its capacity will
automatically be increased, but its buffer will have to be reällocated

For example, a bit-vector with capacity 64 and length 0 would be an empty vector
with space for 64 more bits. Pushing 64 or fewer bits onto the vector will not
change its capacity or cause reällocation to occur. However, if the vector’s
length is increased to 65, it *may* have to reällocate, which can be slow. For
this reason, it is recommended to use [`BitVec::with_capacity`] whenever
possible to specify how big the vector is expected to get.

# Safety

Like [`BitSlice`], `BitVec` is exactly equal in size to [`Vec`], and is also
absolutely representation-incompatible with it. You must never attempt to
type-cast between `Vec<T>` and `BitVec` in any way, nor attempt to modify the
memory value of a `BitVec` handle. Doing so will cause allocator and memory
errors in your program, likely inducing a panic.

Everything in the `BitVec` public API, even the `unsafe` parts, are guaranteed
to have no more unsafety than their equivalent items in the standard library.
All `unsafe` APIs will have documentation explicitly detailing what the API
requires you to uphold in order for it to function safely and correctly. All
safe APIs will do so themselves.

# Performance

The choice of [`BitStore`] type parameter can impact your vector’s performance,
as the allocator operates in units of `T` rather than in bits. This means that
larger register types will increase the amount of memory reserved in each call
to the allocator, meaning fewer calls to [`push`] will actually cause a
reällocation. In addition, iteration over the vector is governed by the
[`BitSlice`] characteristics on the type parameter. You are generally better off
using larger types when your vector is a data collection rather than a specific
I/O protocol buffer.

# Macro Construction

Heap allocation can only occur at runtime, but the [`bitvec!`] macro will
construct an appropriate [`BitSlice`] buffer at compile-time, and at run-time,
only copy the buffer into a heap allocation.

[`BitStore`]: crate::store::BitStore
[`BitSlice`]: crate::slice::BitSlice
[`BitVec::with_capacity`]: Self::with_capacity
[`Index`]: core::ops::Index
[`IndexMut`]: core::ops::IndexMut
[`Vec`]: alloc::vec::Vec
[`Vec<bool>`]: alloc::vec::Vec
[`bitvec!`]: macro@crate::bitvec
[`vec!`]: macro@alloc::vec
[`get_mut`]: crate::slice::BitSlice::get_mut
[`get_unchecked_mut`]: crate::slice::BitSlice::get_unchecked_mut
[`push`]: Self::push
**/
#[repr(C)]
pub struct BitVec<O = Lsb0, T = usize>
where
	O: BitOrder,
	T: BitStore,
{
	/// Region pointer describing the live portion of the owned buffer.
	bitspan: BitSpan<Mut, O, T>,
	/// Allocated capacity, in elements `T`, of the owned buffer.
	capacity: usize,
}

/// General-purpose functions not present on `Vec<T>`.
impl<O, T> BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// Constructs a `BitVec` from a value repeated many times.
	///
	/// This function is equivalent to the `bitvec![O, T; bit; len]` [macro]
	/// call, and is in fact the implementation of that macro syntax.
	///
	/// # Parameters
	///
	/// - `bit`: The bit value to which all `len` allocated bits will be set.
	/// - `len`: The number of live bits in the constructed `BitVec`.
	///
	/// # Returns
	///
	/// A `BitVec` with `len` live bits, all set to `bit`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bv = BitVec::<Msb0, u8>::repeat(true, 20);
	/// assert_eq!(bv, bits![1; 20]);
	/// ```
	///
	/// [macro]: macro@crate::bitvec
	#[inline]
	pub fn repeat(bit: bool, len: usize) -> Self {
		let mut out = Self::with_capacity(len);
		unsafe {
			out.set_len(len);
		}
		out.set_elements(if bit { T::Mem::ALL } else { T::Mem::ZERO });
		out
	}

	/// Copies the contents of a [`BitSlice`] into a new allocation.
	///
	/// This is an exact copy: the newly-created bit-vector is initialized with
	/// a direct copy of the `slice`’s underlying contents, and its handle is
	/// set to use `slice`’s head index. Slices that do not begin at the zeroth
	/// bit of the base element will thus create misaligned vectors.
	///
	/// You can move the bit-vector contents down to begin at the zero index of
	/// the bit-vector’s buffer with [`force_align`].
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![0, 1, 0, 1, 1, 0, 1, 1];
	/// let bv = BitVec::from_bitslice(&bits[2 ..]);
	/// assert_eq!(bv, bits[2 ..]);
	/// assert_eq!(bits.as_slice(), bv.as_raw_slice());
	/// ```
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	/// [`force_align`]: Self::force_align
	#[inline]
	pub fn from_bitslice(slice: &BitSlice<O, T>) -> Self {
		let mut bitspan = slice.as_bitspan();

		let mut vec = bitspan
			.elements()
			.pipe(Vec::with_capacity)
			.pipe(ManuallyDrop::new);

		match slice.domain() {
			Domain::Enclave { elem, .. } => vec.push(elem.load_value()),
			Domain::Region { head, body, tail } => {
				if let Some((_, elem)) = head {
					vec.push(elem.load_value());
				}
				vec.extend(body.iter().map(BitStore::load_value));
				if let Some((elem, _)) = tail {
					vec.push(elem.load_value());
				}
			},
		}

		let bitspan = unsafe {
			bitspan.set_address(vec.as_ptr() as *const T);
			bitspan.assert_mut()
		};

		let capacity = vec.capacity();
		Self { bitspan, capacity }
	}

	/// Converts a [`Vec<T>`] into a `BitVec<O, T>` without copying its buffer.
	///
	/// # Parameters
	///
	/// - `vec`: A vector to view as bits.
	///
	/// # Returns
	///
	/// A `BitVec` over the `vec` buffer.
	///
	/// # Panics
	///
	/// This panics if `vec` is too long to convert into a `BitVec`. See
	/// [`BitSlice::MAX_ELTS`].
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let vec = vec![0u8; 4];
	/// let bv = BitVec::<LocalBits, _>::from_vec(vec);
	/// assert_eq!(bv, bits![0; 32]);
	/// ```
	///
	/// [`BitSlice::MAX_ELTS`]: crate::slice::BitSlice::MAX_ELTS
	/// [`Vec<T>`]: alloc::vec::Vec
	#[inline]
	pub fn from_vec(vec: Vec<T>) -> Self {
		Self::try_from_vec(vec)
			.expect("Vector was too long to be converted into a `BitVec`")
	}

	/// Converts a [`Vec<T>`] into a `BitVec<O, T>` without copying its buffer.
	///
	/// This method takes ownership of a memory buffer and enables it to be used
	/// as a bit-vector. Because [`Vec`] can be longer than `BitVec`s, this is a
	/// fallible method, and the original vector will be returned if it cannot
	/// be converted.
	///
	/// # Parameters
	///
	/// - `vec`: Some vector of memory, to be viewed as bits.
	///
	/// # Returns
	///
	/// If `vec` is short enough to be viewed as a `BitVec`, then this returns
	/// a `BitVec` over the `vec` buffer. If `vec` is too long, then this
	/// returns `vec` unmodified.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let vec = vec![0u8; 4];
	/// let bv = BitVec::<LocalBits, _>::try_from_vec(vec).unwrap();
	/// assert_eq!(bv, bits![0; 32]);
	/// ```
	///
	/// An example showing this function failing would require an allocation
	/// exceeding `!0usize >> 3` bytes in size, which is infeasible to produce.
	///
	/// [`Vec`]: alloc::vec::Vec
	/// [`Vec<T>`]: alloc::vec::Vec
	#[inline]
	pub fn try_from_vec(vec: Vec<T>) -> Result<Self, Vec<T>> {
		let mut vec = ManuallyDrop::new(vec);
		let capacity = vec.capacity();

		BitPtr::from_mut_slice(vec.as_mut_slice())
			.span(vec.len() * T::Mem::BITS as usize)
			.map(|bitspan| Self { bitspan, capacity })
			.map_err(|_| ManuallyDrop::into_inner(vec))
	}

	/// Copies all bits in a [`BitSlice`] into the `BitVec`.
	///
	/// # Original
	///
	/// [`Vec::extend_from_slice`](alloc::vec::Vec::extend_from_slice)
	///
	/// # Type Parameters
	///
	/// This can extend from a [`BitSlice`] of any type arguments. Where the
	/// source `&BitSlice` matches `self`’s type parameters, the implementation
	/// is able to attempt to accelerate the copy; however, if the type
	/// parameters do not match, then the implementation falls back to a
	/// bit-by-bit iteration and is equivalent to the `Extend` implementation.
	///
	/// You should only use this method when the type parameters match and there
	/// is a possibility of copy acceleration. Otherwise, `.extend()` is the
	/// correct API.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![0, 1];
	/// bv.extend_from_bitslice(bits![1, 1, 0, 1]);
	///
	/// assert_eq!(bv, bits![0, 1, 1, 1, 0, 1]);
	/// ```
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	//  Implementation note: per #85, users want this method to stay generic.
	#[inline]
	pub fn extend_from_bitslice<O2, T2>(&mut self, other: &BitSlice<O2, T2>)
	where
		O2: BitOrder,
		T2: BitStore,
	{
		let len = self.len();
		let olen = other.len();
		self.resize(len + olen, false);
		unsafe { self.get_unchecked_mut(len ..) }.clone_from_bitslice(other);
	}

	/// Appends a slice of elements `T` to the `BitVec`.
	///
	/// The `slice` is interpreted as a `BitSlice<O, T>`, then appended directly
	/// to the bit-vector.
	///
	/// # Original
	///
	/// [`Vec::extend_from_slice`](alloc::vec::Vec::extend_from_slice)
	#[inline]
	pub fn extend_from_raw_slice(&mut self, slice: &[T]) {
		self.extend_from_bitslice(
			BitSlice::<O, T>::from_slice(slice)
				.expect("Slice is too long to encode as a BitSlice"),
		);
	}

	/// Gets the number of elements `T` that contain live bits of the
	/// bit-vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bv = bitvec![LocalBits, u16; 1; 50];
	/// assert_eq!(bv.elements(), 4);
	/// ```
	#[inline]
	pub fn elements(&self) -> usize {
		self.as_bitspan().elements()
	}

	/// Converts the bit-vector into [`BitBox<O, T>`].
	///
	/// Note that this will drop any excess capacity.
	///
	/// # Original
	///
	/// [`Vec::into_boxed_slice`](alloc::vec::Vec::into_boxed_slice)
	///
	/// # API Differences
	///
	/// This returns a `bitvec` boxed bit-slice, not a standard boxed slice. To
	/// convert the underlying buffer into a boxed element slice, use
	/// `.into_boxed_bitslice().into_boxed_slice()`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bv = bitvec![0, 1, 0, 0, 1];
	/// let bitslice = bv.into_boxed_slice();
	/// ```
	///
	/// Any excess capacity is removed:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv: BitVec = BitVec::with_capacity(100);
	/// bv.extend([0, 1, 0, 0, 1].iter().copied());
	///
	/// assert!(bv.capacity() >= 100);
	/// let bs = bv.into_boxed_bitslice();
	/// assert!(bs.into_bitvec().capacity() >= 5);
	/// ```
	///
	/// [`BitBox<O, T>`]: crate::boxed::BitBox
	#[inline]
	pub fn into_boxed_bitslice(mut self) -> BitBox<O, T> {
		let mut bitspan = self.as_mut_bitspan();
		let mut boxed =
			self.into_vec().into_boxed_slice().pipe(ManuallyDrop::new);
		unsafe {
			bitspan.set_address(boxed.as_mut_ptr());
			BitBox::from_raw(bitspan.to_bitslice_ptr_mut())
		}
	}

	/// Removes the bit-precision view, returning the underlying [`Vec`].
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bv = bitvec![Lsb0, u8; 0, 1, 0, 0, 1];
	/// let vec = bv.into_vec();
	/// assert_eq!(vec, &[18]);
	/// ```
	///
	/// [`Vec`]: alloc::vec::Vec
	#[inline]
	pub fn into_vec(self) -> Vec<T> {
		let (bitspan, capacity) = (self.bitspan, self.capacity);
		mem::forget(self);
		unsafe {
			Vec::from_raw_parts(
				bitspan.address().to_mut(),
				bitspan.elements(),
				capacity,
			)
		}
	}

	/// Writes a value into every element that the bit-vector considers live.
	///
	/// This unconditionally writes `element` into each live location in the
	/// backing buffer, without altering the `BitVec`’s length or capacity.
	///
	/// It is unspecified what effects this has on the allocated but dead
	/// elements in the buffer. You may not rely on them being zeroed *or* being
	/// set to the `value` integer.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `element`: The value which will be written to each live location in
	///   the bit-vector’s buffer.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![LocalBits, u8; 0; 10];
	/// assert_eq!(bv.as_raw_slice(), [0, 0]);
	/// bv.set_elements(0xA5);
	/// assert_eq!(bv.as_raw_slice(), [0xA5, 0xA5]);
	/// ```
	#[inline]
	pub fn set_elements(&mut self, element: T::Mem) {
		self.as_mut_raw_slice()
			.iter_mut()
			.for_each(|elt| elt.store_value(element));
	}

	/// Sets the uninitialized bits of the bit-vector to a fixed value.
	///
	/// This method modifies all bits in the allocated buffer that are outside
	/// the [`as_bitslice`] view so that they have a consistent value. This can
	/// be used to zero the uninitialized memory so that when viewed as a raw
	/// memory slice, bits outside the live region have a predictable value.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = 220u8.view_bits::<Lsb0>().to_bitvec();
	/// assert_eq!(bv.as_raw_slice(), &[220u8]);
	///
	/// bv.truncate(4);
	/// assert_eq!(bv.count_ones(), 2);
	/// assert_eq!(bv.as_raw_slice(), &[220u8]);
	///
	/// bv.set_uninitialized(false);
	/// assert_eq!(bv.as_raw_slice(), &[12u8]);
	///
	/// bv.set_uninitialized(true);
	/// assert_eq!(bv.as_raw_slice(), &[!3u8]);
	/// ```
	///
	/// [`as_bitslice`]: Self::as_bitslice
	#[inline]
	pub fn set_uninitialized(&mut self, value: bool) {
		let head = self.as_bitspan().head().value() as usize;
		let tail = head + self.len();
		let capa = self.capacity();
		let mut bp = self.as_mut_bitspan();
		unsafe {
			bp.set_head(BitIdx::ZERO);
			bp.set_len(capa);
			let bits = bp.to_bitslice_mut();
			bits.get_unchecked_mut(.. head).set_all(value);
			bits.get_unchecked_mut(tail ..).set_all(value);
		}
	}

	/// Ensures that the live region of the bit-vector’s contents begins at the
	/// leading edge of the buffer.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let data = 0x3Cu8;
	/// let bits = data.view_bits::<Msb0>();
	///
	/// let mut bv = bits[2 .. 6].to_bitvec();
	/// assert_eq!(bv, bits[2 .. 6]);
	/// assert_eq!(bv.as_raw_slice()[0], data);
	///
	/// bv.force_align();
	/// assert_eq!(bv, bits[2 .. 6]);
	/// // It is not specified what happens
	/// // to bits that are no longer used.
	/// assert_eq!(bv.as_raw_slice()[0] & 0xF0, 0xF0);
	/// ```
	#[inline]
	pub fn force_align(&mut self) {
		let bitspan = self.as_mut_bitspan();
		let head = bitspan.head().value() as usize;
		if head == 0 {
			return;
		}
		let last = bitspan.len() + head;
		unsafe {
			self.bitspan = bitspan.tap_mut(|bp| bp.set_head(BitIdx::ZERO));
			self.copy_within_unchecked(head .. last, 0);
		}
	}

	/// Writes a new length value into the pointer without any checks.
	///
	/// # Safety
	///
	/// `new_len` must not exceed `self.capacity() - self.bitspan.head()`.
	#[cfg_attr(not(tarpaulin_include), inline(always))]
	pub(crate) unsafe fn set_len_unchecked(&mut self, new_len: usize) {
		self.bitspan.set_len(new_len);
	}

	/// Extracts a bit-slice containing the entire bit-vector.
	///
	/// Equivalent to `&bv[..]`.
	///
	/// # Original
	///
	/// [`Vec::as_slice`](alloc::vec::Vec::as_slice)
	///
	/// # API Differences
	///
	/// This returns a `bitvec` bit-slice, not a standard slice. To view the
	/// underlying element buffer, use [`as_raw_slice`].
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bv = bitvec![0, 1, 0, 0, 1];
	/// let bits = bv.as_bitslice();
	/// ```
	///
	/// [`as_raw_slice`]: Self::as_raw_slice
	#[cfg_attr(not(tarpaulin_include), inline(always))]
	pub fn as_bitslice(&self) -> &BitSlice<O, T> {
		self.bitspan.to_bitslice_ref()
	}

	/// Extracts a mutable bit-slice of the entire bit-vector.
	///
	/// Equivalent to `&mut bv[..]`.
	///
	/// # Original
	///
	/// [`Vec::as_mut_slice`](alloc::vec::Vec::as_mut_slice)
	///
	/// # API Differences
	///
	/// This returns a `bitvec` bit-slice, not a standard slice. To view the
	/// underlying element buffer, use [`as_mut_raw_slice`].
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![0, 1, 0, 0, 1];
	/// let bits = bv.as_mut_bitslice();
	/// ```
	///
	/// [`as_mut_raw_slice`]: Self::as_mut_raw_slice
	#[cfg_attr(not(tarpaulin_include), inline(always))]
	pub fn as_mut_bitslice(&mut self) -> &mut BitSlice<O, T> {
		self.bitspan.to_bitslice_mut()
	}

	/// Returns a raw pointer to the bit-vector’s buffer.
	///
	/// The caller must ensure that the bit-vector outlives the bit-pointer this
	/// function returns, or else it will end up pointing to garbage. Modifying
	/// the bit-vector may cause its buffer to be reällocated, which would also
	/// make any bit-pointers to it invalid.
	///
	/// The caller must also ensure that the memory the bit-pointer
	/// (non-transitively) points to is never written to (except inside an
	/// [`UnsafeCell`]) using this bit-pointer or any bit-pointer derived from
	/// it. If you need to mutate the contents of the buffer, use
	/// [`as_mut_bitptr`].
	///
	/// # Original
	///
	/// [`Vec::as_ptr`](alloc::vec::Vec::as_ptr)
	///
	/// # API Differences
	///
	/// This returns a `bitvec` bit-pointer, not a standard pointer. To take the
	/// address of the underlying element buffer, use [`as_raw_ptr`].
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bv = bitvec![0, 1, 0, 0, 1];
	/// let bp = bv.as_bitptr();
	///
	/// unsafe {
	///   for i in 0 .. bv.len() {
	///     assert_eq!(bp.add(i).read(), bv[i]);
	///   }
	/// }
	/// ```
	///
	/// [`UnsafeCell`]: core::cell::UnsafeCell
	/// [`as_raw_ptr`]: Self::as_raw_ptr
	/// [`as_mut_bitptr`]: Self::as_mut_bitptr
	#[inline]
	pub fn as_bitptr(&self) -> BitPtr<Const, O, T> {
		self.bitspan.as_bitptr().immut()
	}

	/// Returns an unsafe mutable bit-pointer to the bit-vector’s region.
	///
	/// The caller must ensure that the bit-vector outlives the bit-pointer this
	/// function returns, or else it will end up pointing to garbage. Modifying
	/// the bit-vector may cause its buffer to be reällocated, which would also
	/// make any bit-pointers to it invalid.
	///
	/// # Original
	///
	/// [`Vec::as_mut_ptr`](alloc::vec::Vec::as_mut_ptr)
	///
	/// # API Differences
	///
	/// This returns a `bitvec` bit-pointer, not a standard pointer. To take the
	/// address of the underlying element buffer, use [`as_mut_raw_ptr`].
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = BitVec::<Msb0, u8>::with_capacity(4);
	/// let bp = bv.as_mut_bitptr();
	/// unsafe {
	///   for i in 0 .. 4 {
	///     bp.add(i).write(true);
	///   }
	///   bv.set_len(4);
	/// }
	/// assert_eq!(bv, bits![1; 4]);
	/// ```
	///
	/// [`as_mut_raw_ptr`]: Self::as_mut_raw_ptr
	#[cfg_attr(not(tarpaulin_include), inline(always))]
	pub fn as_mut_bitptr(&mut self) -> BitPtr<Mut, O, T> {
		self.bitspan.as_bitptr()
	}

	/// Views the underlying buffer as a shared element slice.
	///
	/// # Original
	///
	/// [`Vec::as_slice`](alloc::vec::Vec::as_slice)
	///
	/// # API Differences
	///
	/// This method is renamed in order to emphasize the semantic distinction
	/// between borrowing the bit-vector contents, and borrowing the memory that
	/// implements the collection contents.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bv = bitvec![Msb0, u8; 0, 1, 0, 0, 1, 1, 0, 1];
	/// let raw = bv.as_raw_slice();
	/// assert_eq!(raw, &[0x4D]);
	/// ```
	#[inline]
	pub fn as_raw_slice(&self) -> &[T] {
		unsafe {
			slice::from_raw_parts(
				self.bitspan.address().to_const(),
				self.bitspan.elements(),
			)
		}
	}

	/// Views the underlying buffer as an exclusive element slice.
	///
	/// # Original
	///
	/// [`Vec::as_mut_slice`](alloc::vec::Vec::as_mut_slice)
	///
	/// # API Differences
	///
	/// This method is renamed in order to emphasize the semantic distinction
	/// between borrowing the bit-vector contents, and borrowing the memory that
	/// implements the collection contents.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![Msb0, u8; 0, 1, 0, 0, 1, 1, 0, 1];
	/// let raw = bv.as_mut_raw_slice();
	/// assert_eq!(raw, &[0x4D]);
	/// raw[0] = 0xD4;
	/// assert_eq!(bv, bits![1, 1, 0, 1, 0, 1, 0, 0]);
	/// ```
	#[inline]
	pub fn as_mut_raw_slice(&mut self) -> &mut [T] {
		unsafe {
			slice::from_raw_parts_mut(
				self.bitspan.address().to_mut(),
				self.bitspan.elements(),
			)
		}
	}

	/// Returns a raw pointer to the bit-vector’s buffer.
	///
	/// # Original
	///
	/// [`Vec::as_ptr`](alloc::vec::Vec::as_ptr)
	///
	/// # API Differences
	///
	/// This method is renamed in order to emphasize the semantic distinction
	/// between taking a pointer to the start of the bit-vector contents, and
	/// taking a pointer to the underlying memory that implements the collection
	/// contents.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bv = bitvec![Msb0, u8; 0, 1, 0, 0, 1];
	/// let addr = bv.as_raw_ptr();
	/// ```
	#[inline]
	pub fn as_raw_ptr(&self) -> *const T {
		self.bitspan.address().to_const()
	}

	/// Returns an unsafe mutable pointer to the bit-vector’s buffer.
	///
	/// # Original
	///
	/// [`Vec::as_mut_ptr`](alloc::vec::Vec::as_mut_ptr)
	///
	/// # API Differences
	///
	/// This method is renamed in order to emphasize the semantic distinction
	/// between taking a pointer to the start of the bit-vector contents, and
	/// taking a pointer to the underlying memory that implements the collection
	/// contents.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![0, 1, 0, 0, 1];
	/// let addr = bv.as_mut_raw_ptr();
	/// ```
	#[inline]
	pub fn as_mut_raw_ptr(&mut self) -> *mut T {
		self.bitspan.address().to_mut()
	}

	/// Construct a `BitVec` from its exact fields, rather than using a formal
	/// constructor.
	///
	/// This is used for handle construction elsewhere in the crate, where a
	/// vector allocation and `BitSpan` descriptor exist, and need to be bundled
	/// into a `BitVec` without going through the ordinary construction process.
	///
	/// # Parameters
	///
	/// - `bitspan`: A span descriptor.
	/// - `capacity`: An allocation capacity, measured in `T` elements rather
	///   than in bits.
	///
	/// # Returns
	///
	/// `BitVec { bitspan, capacity }`
	///
	/// # Safety
	///
	/// The arguments must be derived from a known-good buffer allocation and
	/// span description. They will be directly used to construct the returned
	/// bit-vector, and drive all future memory access and allocation control.
	pub(crate) unsafe fn from_fields(
		bitspan: BitSpan<Mut, O, T>,
		capacity: usize,
	) -> Self {
		Self { bitspan, capacity }
	}

	/// Removes the `::Unalias` marker from a bit-vector’s type signature.
	fn strip_unalias(this: BitVec<O, T::Unalias>) -> Self {
		let (bitspan, capacity) = (this.bitspan.cast::<T>(), this.capacity);
		core::mem::forget(this);
		Self { bitspan, capacity }
	}

	/// Combines the logic for `BitVec::reserve` and `BitVec::reserve_exact`.
	#[inline]
	fn do_reservation(
		&mut self,
		additional: usize,
		func: impl FnOnce(&mut Vec<T>, usize),
	) {
		let len = self.len();
		let new_len = len
			.checked_add(additional)
			.expect("Bit-Vector capacity exceeded");
		assert!(
			new_len <= BitSlice::<O, T>::MAX_BITS,
			"Bit-Vector capacity exceeded: {} > {}",
			new_len,
			BitSlice::<O, T>::MAX_BITS,
		);
		let bitspan = self.bitspan;
		let head = bitspan.head();
		let elts = bitspan.elements();
		let new_elts = crate::mem::elts::<T>(head.value() as usize + new_len);
		let extra = new_elts - elts;
		self.with_vec(|vec| {
			func(&mut **vec, extra);
			//  Initialize any newly-allocated elements to zero, without
			//  initializing leftover dead capacity.
			vec.resize_with(new_elts, || unsafe { mem::zeroed() });
		});
	}

	/// Permits manipulation of the underlying vector allocation.
	///
	/// The caller receives a mutable borrow of a `Vec<T>` with its destructor
	/// disarmed. The caller may modify the buffer controls, including its
	/// location and its capacity, and these changes will be committed back into
	/// `self`. Modifications to the referent `[T]` handle, such as length
	/// changes, will not be preserved.
	fn with_vec<F, R>(&mut self, func: F) -> R
	where F: FnOnce(&mut ManuallyDrop<Vec<T>>) -> R {
		let capacity = self.capacity;
		let (ptr, length) =
			(self.bitspan.address().to_mut(), self.bitspan.elements());

		let mut vec = unsafe { Vec::from_raw_parts(ptr, length, capacity) }
			.pipe(ManuallyDrop::new);
		let out = func(&mut vec);

		unsafe {
			self.bitspan.set_address(vec.as_mut_ptr());
		}
		self.capacity = vec.capacity();
		out
	}
}

mod api;
mod iter;
mod ops;
mod traits;

pub use self::iter::{
	Drain,
	IntoIter,
	Splice,
};

#[cfg(test)]
mod tests;
