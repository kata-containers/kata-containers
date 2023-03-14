//! Port of the `Vec<T>` inherent API.

use crate::{
	boxed::BitBox,
	index::BitTail,
	mem::BitMemory,
	mutability::{
		Const,
		Mut,
	},
	order::BitOrder,
	ptr::{
		Address,
		BitPtr,
		BitSpan,
	},
	slice::BitSlice,
	store::BitStore,
	vec::{
		iter::{
			Drain,
			Splice,
		},
		BitVec,
	},
};

use alloc::vec::Vec;

use core::{
	mem::{
		self,
		ManuallyDrop,
	},
	ops::RangeBounds,
};

use tap::pipe::Pipe;

/// Port of the `Vec<T>` inherent API.
impl<O, T> BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// Constructs a new, empty, `BitVec<O, T>`.
	///
	/// The bit-vector will not allocate until bits are pushed onto it.
	///
	/// # Original
	///
	/// [`Vec::new`](alloc::vec::Vec::new)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv: BitVec = BitVec::new();
	/// ```
	#[cfg_attr(not(tarpaulin_include), inline(always))]
	pub fn new() -> Self {
		Self {
			bitspan: BitSpan::EMPTY,
			capacity: 0,
		}
	}

	/// Constructs a new, empty, `BitVec<O, T>` with the specified capacity (in
	/// bits).
	///
	/// The bit-vector will be able to hold at least `capacity` bits without
	/// reällocating. If `capacity` is 0, the bit-vector will not allocate.
	///
	/// It is important to note that although the returned bit-vector has the
	/// *capacity* specified, the bit-vector will have a zero *length*. For an
	/// explanation of the difference between length and capacity, see
	/// *[Capacity and reällocation]*.
	///
	/// # Original
	///
	/// [`Vec::with_capacity`](alloc::vec::Vec::with_capacity)
	///
	/// # Panics
	///
	/// Panics if the requested capacity exceeds the bit-vector’s limits.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv: BitVec = BitVec::with_capacity(128);
	///
	/// // The bit-vector contains no bits, even
	/// // though it has the capacity for more.
	/// assert_eq!(bv.len(), 0);
	/// assert!(bv.capacity() >= 128);
	///
	/// // These are all done
	/// // without reällocating…
	/// for i in 0 .. 128 {
	///   bv.push(i & 0xC0 == i);
	/// }
	/// assert_eq!(bv.len(), 128);
	/// assert!(bv.capacity() >= 128);
	///
	/// // …but this may make the
	/// // bit-vector reällocate.
	/// bv.push(false);
	/// assert_eq!(bv.len(), 129);
	/// assert!(bv.capacity() >= 129);
	/// ```
	///
	/// [Capacity and reällocation]: #capacity-and-reallocation
	#[inline]
	pub fn with_capacity(capacity: usize) -> Self {
		assert!(
			capacity <= BitSlice::<O, T>::MAX_BITS,
			"Bit-Vector capacity exceeded: {} > {}",
			capacity,
			BitSlice::<O, T>::MAX_BITS,
		);

		let mut vec = capacity
			.pipe(crate::mem::elts::<T>)
			.pipe(Vec::<T>::with_capacity)
			.pipe(ManuallyDrop::new);
		let (addr, capacity) = (vec.as_mut_ptr(), vec.capacity());
		let bitspan = BitSpan::uninhabited(unsafe {
			Address::new_unchecked(addr as usize)
		});
		Self { bitspan, capacity }
	}

	/// Decomposes a `BitVec<O, T>` into its raw components.
	///
	/// Returns the raw bit-pointer to the underlying data, the length of the
	/// bit-vector (in bits), and the allocated capacity of the buffer (in
	/// bits). These are the same arguments in the same order as the arguments
	/// to [`from_raw_parts`].
	///
	/// After calling this function, the caller is responsible for the memory
	/// previously managed by the `BitVec`. The only way to do this is to
	/// convert the raw bit-pointer, length, and capacity back into a `BitVec`
	/// with the [`from_raw_parts`] function, allowing the destructor to perform
	/// the cleanup.
	///
	/// # Original
	///
	/// [`Vec::into_raw_parts`](alloc::vec::Vec::into_raw_parts)
	///
	/// # API Differences
	///
	/// This returns a `BitPtr`, rather than a `*mut T`. If you need the actual
	/// memory address, [`BitPtr::pointer`] will produce it.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	/// use core::cell::Cell;
	///
	/// let bv: BitVec = bitvec![0, 1, 0, 0, 1];
	///
	/// let (ptr, len, cap) = bv.into_raw_parts();
	///
	/// let rebuilt = unsafe {
	///   // We can now make changes to the components, such
	///   // as casting the pointer to a compatible type.
	///   let ptr = ptr.cast::<Cell<usize>>();
	///   BitVec::from_raw_parts(ptr, len, cap)
	/// };
	/// assert_eq!(rebuilt, bits![0, 1, 0, 0, 1]);
	/// ```
	///
	/// [`BitPtr::pointer`]: crate::ptr::BitPtr::pointer
	/// [`from_raw_parts`]: Self::from_raw_parts
	#[inline]
	pub fn into_raw_parts(self) -> (BitPtr<Mut, O, T>, usize, usize) {
		let (bitspan, capacity) = (self.bitspan, self.capacity());
		mem::forget(self);
		(bitspan.as_bitptr(), bitspan.len(), capacity)
	}

	/// Creates a `BitVec<O, T>` directly from the raw components of another
	/// bit-vector.
	///
	/// # Original
	///
	/// [`Vec::from_raw_parts`](alloc::vec::Vec::from_raw_parts)
	///
	/// # API Differences
	///
	/// This takes a `BitPtr`, rather than a `*mut T`. If you only have a
	/// pointer, you can construct a `BitPtr` to its zeroth bit before calling
	/// this.
	///
	/// # Safety
	///
	/// This is highly unsafe, due to the number of invariants that aren’t
	/// checked:
	///
	/// - `bitptr` needs to have been previously allocated by `BitVec<O, T>`, or
	///   constructed from a pointer allocated by [`Vec<T>`].
	/// - `T` needs to have the same size and alignment as what `bitptr` was
	///   allocated with. (`T` having a less strict alignment is not sufficient;
	///   the alignment really needs to be equal to satisf the [`dealloc`]
	///   requirement that memory must be allocated and deällocated with the
	///   same layout.) However, you can safely cast between bare integers,
	///   `BitSafe` integers, `Cell` wrappers, and atomic integers, as long as
	///   they all have the same width.
	/// - `length` needs to be less than or equal to capacity.
	/// - `capacity` needs to be the capacity (in bits) that the bit-pointer was
	///   allocated with (less any head offset in `bitptr`).
	///
	/// Violating these **will** cause problems. For example, it is **not** safe
	/// to build a `BitVec<_, u8>` from a pointer to a `u16` sequence and twice
	/// its original length, because the allocator cares about the alignment,
	/// and these two types have different alignments. The buffer was allocated
	/// with alignment 2 (for `u16`), but after turning it into a `BitVec<_,
	/// u8>`, it’ll be deällocated with alignment 1.
	///
	/// The ownership of `bitptr`is effectively transferred to the `BitVec<O,
	/// T>` which may then deällocate, reällocate, or change the contents of
	/// memory pointed to by the bit-pointer at will. Ensure that nothing else
	/// uses the pointer after calling this function.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	/// use bitvec::ptr as bv_ptr;
	/// use core::mem::ManuallyDrop;
	///
	/// let bv = bitvec![0, 1, 0, 0, 1];
	/// let mut bv = ManuallyDrop::new(bv);
	/// let bp = bv.as_mut_bitptr();
	/// let len = bv.len();
	/// let cap = bv.capacity();
	///
	/// unsafe {
	///   // Overwrite memory with the inverse bits.
	///   for i in 0 .. len {
	///     let bp = bp.add(i);
	///     bv_ptr::write(bp, !bv_ptr::read(bp.immut()));
	///   }
	///
	///   // Put everything back together into a `BitVec`.
	///   let rebuilt = BitVec::from_raw_parts(bp, len, cap);
	///   assert_eq!(rebuilt, bits![1, 0, 1, 1, 0]);
	/// }
	/// ```
	///
	/// [`Vec<T>`]: alloc::vec::Vec
	/// [`dealloc`]: alloc::alloc::GlobalAlloc::dealloc
	#[inline]
	pub unsafe fn from_raw_parts(
		bitptr: BitPtr<Mut, O, T>,
		length: usize,
		capacity: usize,
	) -> Self {
		Self {
			bitspan: bitptr.span_unchecked(length),
			capacity: crate::mem::elts::<T>(capacity),
		}
	}

	/// Returns the number of bits the bit-vector can hold without reällocating.
	///
	/// # Original
	///
	/// [`Vec::capacity`](alloc::vec::Vec::capacity)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bv: BitVec = BitVec::with_capacity(100);
	/// assert!(bv.capacity() >= 100);
	/// ```
	#[inline]
	pub fn capacity(&self) -> usize {
		self.capacity
			.checked_mul(T::Mem::BITS as usize)
			.expect("Bit-Vector capacity exceeded")
			//  Don’t forget to subtract any dead bits in the front of the base!
			//  This has to be saturating, becase a non-zero head on a zero
			//  capacity underflows.
			.saturating_sub(self.bitspan.head().value() as usize)
	}

	/// Reserves capacity for at least `additional` more bits to be inserted in
	/// the given `BitVec<O, T>`. The collection may reserve more space to avoid
	/// frequent reällocations. After calling `reserve`, capacity will be
	/// greater than or equal to `self.len() + additional`. Does nothing if
	/// capacity is already sufficient.
	///
	/// # Original
	///
	/// [`Vec::reserve`](alloc::vec::Vec::reserve)
	///
	/// # Panics
	///
	/// Panics if the new capacity exceeds the bit-vector’s limits.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![1];
	/// bv.reserve(100);
	/// assert!(bv.capacity() >= 101);
	/// ```
	#[cfg_attr(not(tarpaulin_include), inline(always))]
	pub fn reserve(&mut self, additional: usize) {
		self.do_reservation(additional, Vec::<T>::reserve);
	}

	/// Reserves the minimum capacity for exactly `additional` more bits to be
	/// inserted in the given `BitVec<O, T>`. After calling `reserve_exact`,
	/// capacity will be greater than or equal to `self.len() + additional`.
	/// Does nothing if the capacity is already sufficient.
	///
	/// Note that the allocator may give the collection more space than it
	/// requests. Therefore, capacity can not be relied upon to be precisely
	/// minimal. Prefer `reserve` if future insertions are expected.
	///
	/// # Original
	///
	/// [`Vec::reserve_exact`](alloc::vec::Vec::reserve_exact)
	///
	/// # Panics
	///
	/// Panics if the new capacity exceeds the vector’s limits.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![1];
	/// bv.reserve_exact(100);
	/// assert!(bv.capacity() >= 101);
	/// ```
	#[cfg_attr(not(tarpaulin_include), inline(always))]
	pub fn reserve_exact(&mut self, additional: usize) {
		self.do_reservation(additional, Vec::<T>::reserve_exact);
	}

	/// Shrinks the capacity of the bit-vector as much as possible.
	///
	/// It will drop down as close as possible to the length but the allocator
	/// may still inform the bit-vector that there is space for a few more bits.
	///
	/// # Original
	///
	/// [`Vec::shrink_to_fit`](alloc::vec::Vec::shrink_to_fit)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv: BitVec = BitVec::with_capacity(100);
	/// bv.extend([false, true, false].iter().copied());
	/// assert!(bv.capacity() >= 100);
	/// bv.shrink_to_fit();
	/// assert!(bv.capacity() >= 3);
	/// ```
	#[inline]
	pub fn shrink_to_fit(&mut self) {
		self.with_vec(|vec| vec.shrink_to_fit());
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Prefer `into_boxed_bitslice`"]
	pub fn into_boxed_slice(self) -> BitBox<O, T> {
		self.into_boxed_bitslice()
	}

	/// Shortens the bit-vector, keeping the first `len` bits and dropping the
	/// rest.
	///
	/// If `len` is greater than the bit-vector’s current length, this has no
	/// effect.
	///
	/// The [`drain`] method can emulate `truncate`, but causes the excess bits
	/// to be returned instead of dropped.
	///
	/// Note that this method has no effect on the allocated capacity of the
	/// bit-vector, **nor does it erase truncated memory**. Bits in the
	/// allocated memory that are outside of the [`as_bitslice`] view always
	/// have **unspecified** values, and cannot be relied upon to be zero.
	///
	/// # Original
	///
	/// [`Vec::truncate`](alloc::vec::Vec::truncate)
	///
	/// # Examples
	///
	/// Truncating a five-bit vector to two bits:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![1; 5];
	/// bv.truncate(2);
	/// assert_eq!(bv.len(), 2);
	/// assert!(bv.as_raw_slice()[0].count_ones() >= 5);
	/// ```
	///
	/// No truncation occurs when `len` is greater than the vector’s current
	/// length:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![1; 3];
	/// bv.truncate(8);
	/// assert_eq!(bv.len(), 3);
	/// ```
	///
	/// Truncating when `len == 0` is equivalent to calling the [`clear`]
	/// method.
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![0; 3];
	/// bv.truncate(0);
	/// assert!(bv.is_empty());
	/// ```
	///
	/// [`as_bitslice`]: Self::as_bitslice
	/// [`clear`]: Self::clear
	/// [`drain`]: Self::drain
	#[inline]
	pub fn truncate(&mut self, len: usize) {
		if len < self.len() {
			unsafe { self.set_len_unchecked(len) }
		}
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Prefer `as_bitslice`, or `as_raw_slice` to view the \
	                underlying memory"]
	pub fn as_slice(&self) -> &BitSlice<O, T> {
		self.as_bitslice()
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Prefer `as_mut_bitslice`, or `as_mut_raw_slice` to view the \
	                underlying memory"]
	pub fn as_mut_slice(&mut self) -> &mut BitSlice<O, T> {
		self.as_mut_bitslice()
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Prefer `as_bitptr`, or `as_raw_ptr` to take the address of \
	                the underlying memory"]
	pub fn as_ptr(&self) -> BitPtr<Const, O, T> {
		self.as_bitptr()
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Prefer `as_mut_bitptr`, or `as_mut_raw_ptr` to take the \
	                address of the underlying memory"]
	pub fn as_mut_ptr(&mut self) -> BitPtr<Mut, O, T> {
		self.as_mut_bitptr()
	}

	/// Forces the length of the bit-vector to `new_len`.
	///
	/// This is a low-level operation that maintains none of the normal
	/// invariants of the type. Normall changing the length of a bit-vector is
	/// done using one of the safe operations instead, such as [`truncate`],
	/// [`resize`], [`extend`], or [`clear`].
	///
	/// # Original
	///
	/// [`Vec::set_len`](alloc::vec::Vec::set_len)
	///
	/// # Safety
	///
	/// - `new_len` must be less than or equal to [`self.capacity()`].
	/// - The memory elements underlying `old_len .. new_len` must be
	///   initialized.
	///
	/// # Examples
	///
	/// This method can be useful for situations in which the bit-vector is
	/// serving as a buffer for other code, particularly over FFI:
	///
	/// ```rust
	/// # #![allow(dead_code)]
	/// # #![allow(improper_ctypes)]
	/// # const ERL_OK: i32 = 0;
	/// # extern "C" {
	/// #   fn erl_read_bits(
	/// #     bv: *mut BitVec<Msb0, u8>,
	/// #     bits_reqd: usize,
	/// #     bits_read: *mut usize,
	/// #   ) -> i32;
	/// # }
	/// use bitvec::prelude::*;
	///
	/// // `bitvec` could pair with `rustler` for a better bitstream
	/// type ErlBitstring = BitVec<Msb0, u8>;
	/// # pub fn _test() {
	/// let mut bits_read = 0;
	/// // An imaginary Erlang function wants a large bit buffer.
	/// let mut buf = ErlBitstring::with_capacity(32_768);
	/// // SAFETY: When `erl_read_bits` returns `ERL_OK`, it holds that:
	/// // 1. `bits_read` bits were initialized.
	/// // 2. `bits_read` <= the capacity (32_768)
	/// // which makes `set_len` safe to call.
	/// unsafe {
	///   // Make the FFI call…
	///   let status = erl_read_bits(&mut buf, 10, &mut bits_read);
	///   if status == ERL_OK {
	///     // …and update the length to what was read in.
	///     buf.set_len(bits_read);
	///   }
	/// }
	/// # }
	/// ```
	///
	/// [`clear`]: Self::clear
	/// [`extend`]: Self::extend
	/// [`resize`]: Self::resize
	/// [`self.capacity()`]: Self::capacity
	/// [`truncate`]: Self::truncate
	#[inline]
	pub unsafe fn set_len(&mut self, new_len: usize) {
		assert!(
			new_len <= BitSlice::<O, T>::MAX_BITS,
			"Bit-Vector capacity exceeded: {} > {}",
			new_len,
			BitSlice::<O, T>::MAX_BITS,
		);
		let cap = self.capacity();
		assert!(
			new_len <= cap,
			"Bit-Vector capacity exceeded: {} > {}",
			new_len,
			cap,
		);
		self.set_len_unchecked(new_len);
	}

	/// Removes a bit from the bit-vector and returns it.
	///
	/// The removed bit is replaced by the last bit of the bit-vector.
	///
	/// This does not preserve ordering, but is O(1).
	///
	/// # Original
	///
	/// [`Vec::swap_remove`](alloc::vec::Vec::swap_remove)
	///
	/// # Panics
	///
	/// Panics if `index` is out of bounds.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![0, 0, 1, 0, 1];
	/// assert!(!bv.swap_remove(1));
	/// assert_eq!(bv, bits![0, 1, 1, 0]);
	///
	/// assert!(!bv.swap_remove(0));
	/// assert_eq!(bv, bits![0, 1, 1]);
	/// ```
	#[inline]
	pub fn swap_remove(&mut self, index: usize) -> bool {
		self.assert_in_bounds(index);
		let last = self.len() - 1;
		unsafe {
			self.swap_unchecked(index, last);
			self.set_len(last);
			*self.get_unchecked(last)
		}
	}

	/// Inserts a bit at position `index` within the bit-vector, shifting all
	/// bits after it to the right.
	///
	/// # Original
	///
	/// [`Vec::insert`](alloc::vec::Vec::insert)
	///
	/// # Panics
	///
	/// Panics if `index > len`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![0; 5];
	/// bv.insert(4, true);
	/// assert_eq!(bv, bits![0, 0, 0, 0, 1, 0]);
	/// bv.insert(2, true);
	/// assert_eq!(bv, bits![0, 0, 1, 0, 0, 1, 0]);
	/// ```
	#[inline]
	pub fn insert(&mut self, index: usize, value: bool) {
		self.assert_in_bounds(index);
		self.push(value);
		unsafe { self.get_unchecked_mut(index ..) }.rotate_right(1);
	}

	/// Removes and returns the bit at position `index` within the bit-vector,
	/// shifting all bits after it to the left.
	///
	/// # Original
	///
	/// [`Vec::remove`](alloc::vec::Vec::remove)
	///
	/// # Panics
	///
	/// Panics if `index` is out of bounds.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![0, 1, 0];
	/// assert!(bv.remove(1));
	/// assert_eq!(bv, bits![0, 0]);
	/// ```
	#[inline]
	pub fn remove(&mut self, index: usize) -> bool {
		self.assert_in_bounds(index);
		let last = self.len() - 1;
		unsafe {
			self.get_unchecked_mut(index ..).rotate_left(1);
			self.set_len(last);
			*self.get_unchecked(last)
		}
	}

	/// Retains only the bits specified by the predicate.
	///
	/// In other words, remove all bits `b` such that `func(idx(b), &b)` returns
	/// `false`. This method operates in place, visiting each bit exactly once
	/// in the original order, and preserves the order of the retained bits.
	///
	/// # Original
	///
	/// [`Vec::retain`](alloc::vec::Vec::retain)
	///
	/// # API Differences
	///
	/// In order to allow more than one bit of information for the retention
	/// decision, the predicate receives the index of each bit, as well as its
	/// value.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![0, 1, 1, 0, 0, 1];
	/// bv.retain(|i, b| (i % 2 == 0) ^ b);
	/// assert_eq!(bv, bits![0, 1, 0, 1]);
	/// ```
	#[inline]
	pub fn retain<F>(&mut self, mut func: F)
	where F: FnMut(usize, &bool) -> bool {
		let len = self.len();
		let mut del = 0;
		/* Walk the vector, testing each bit and its index. This loop sorts the
		vector in-place, partitioning it with consecutive retained bits at the
		front and consecutive discarded bits at the back.
		*/
		for (idx, bitptr) in self.as_bitslice().as_bitptr_range().enumerate() {
			//  If the bit/index fails the test, bump the deletion counter.
			if !func(idx, &unsafe { bitptr.read() }) {
				del += 1
			}
			//  If the test passes, swap the bit with the first failed bit.
			else if del > 0 {
				self.swap(idx - del, idx);
			}
		}
		// Drop discarded bits.
		if del > 0 {
			self.truncate(len - del);
		}
	}

	/// Appends a bit to the back of a collection.
	///
	/// # Original
	///
	/// [`Vec::push`](alloc::vec::Vec::push)
	///
	/// # Panics
	///
	/// Panics if the number of bits in the bit-vector exceeds the maximum
	/// bit-vector capacity.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![0, 0];
	/// bv.push(true);
	/// assert_eq!(bv.count_ones(), 1);
	/// ```
	#[inline]
	pub fn push(&mut self, value: bool) {
		let len = self.len();
		assert!(
			len <= BitSlice::<O, T>::MAX_BITS,
			"Bit-Vector capacity exceeded: {} > {}",
			len,
			BitSlice::<O, T>::MAX_BITS,
		);
		//  Push a new `T` into the underlying buffer if needed
		if self.is_empty() || self.bitspan.tail() == BitTail::LAST {
			self.with_vec(|vec| vec.push(unsafe { mem::zeroed() }))
		}
		//  Write `value` into the now-safely-allocated `len` slot.
		unsafe {
			self.set_len_unchecked(len + 1);
			self.set_unchecked(len, value);
		}
	}

	/// Removes the last bit from a bit-vector and returns it, or [`None`] if it
	/// is empty.
	///
	/// # Original
	///
	/// [`Vec::pop`](alloc::vec::Vec::pop)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![0, 0, 1];
	/// assert_eq!(bv.pop(), Some(true));
	/// assert_eq!(bv, bits![0, 0]);
	/// ```
	///
	/// [`None`]: core::option::Option::None
	#[inline]
	pub fn pop(&mut self) -> Option<bool> {
		match self.len() {
			0 => None,
			n => unsafe {
				let new_len = n - 1;
				self.set_len_unchecked(new_len);
				Some(*self.get_unchecked(new_len))
			},
		}
	}

	/// Moves all the bits of `other` into `self`, leaving `other` empty.
	///
	/// # Original
	///
	/// [`Vec::append`](alloc::vec::Vec::append)
	///
	/// # API Differences
	///
	/// This permits `other` to have different type parameters than `self`, and
	/// does not require that it be of literally `Self`.
	///
	/// # Panics
	///
	/// Panics if the number of bits overflows the maximum bit-vector capacity.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv1 = bitvec![Msb0, u16; 0; 10];
	/// let mut bv2 = bitvec![Lsb0, u32; 1; 10];
	///
	/// bv1.append(&mut bv2);
	///
	/// assert_eq!(bv1.count_ones(), 10);
	/// assert!(bv2.is_empty());
	/// ```
	#[inline]
	pub fn append<O2, T2>(&mut self, other: &mut BitVec<O2, T2>)
	where
		O2: BitOrder,
		T2: BitStore,
	{
		let this_len = self.len();
		let new_len = this_len + other.len();
		self.resize(new_len, false);
		unsafe { self.get_unchecked_mut(this_len .. new_len) }
			.clone_from_bitslice(other.as_bitslice());
		other.clear();
	}

	/// Creates a draining iterator that removes the specified range in the
	/// bit-vector and yields the removed bits.
	///
	/// When the iterator **is** dropped, all bits in the range are removed from
	/// the bit-vector, even if the iterator was not fully consumed. If the
	/// iterator **is not** dropped (with [`mem::forget`] for example), it is
	/// unspecified how many bits are removed.
	///
	/// # Original
	///
	/// [`Vec::drain`](alloc::vec::Vec::drain)
	///
	/// # Panics
	///
	/// Panics if the starting point is greater than the end point or if the end
	/// point is greater than the length of the bit-vector.
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![0, 1, 1];
	/// let bv2: BitVec = bv.drain(1 ..).collect();
	/// assert_eq!(bv, bits![0]);
	/// assert_eq!(bv2, bits![1, 1]);
	///
	/// // A full range clears the vector
	/// bv.drain(..);
	/// assert_eq!(bv, bits![]);
	/// ```
	///
	/// [`mem::forget`]: core::mem::forget
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn drain<R>(&mut self, range: R) -> Drain<O, T>
	where R: RangeBounds<usize> {
		Drain::new(self, range)
	}

	/// Clears the bit-vector, removing all values.
	///
	/// Note that this method has no effect on the allocated capacity of the
	/// bit-vector.
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![0, 1, 0, 1];
	/// bv.clear();
	/// assert!(bv.is_empty());
	/// ```
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn clear(&mut self) {
		self.truncate(0);
	}

	/// Returns the number of bits in the bit-vector, also referred to as its
	/// ‘length’.
	///
	/// # Original
	///
	/// [`Vec::len`](alloc::vec::Vec::len)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bv = bitvec![0, 0, 1];
	/// assert_eq!(bv.len(), 3);
	/// ```
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn len(&self) -> usize {
		self.bitspan.len()
	}

	/// Returns `true` if the bit-vector contains no bits.
	///
	/// # Original
	///
	/// [`Vec::is_empty`](alloc::vec::Vec::is_empty)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv: BitVec = BitVec::new();
	/// assert!(bv.is_empty());
	///
	/// bv.push(true);
	/// assert!(!bv.is_empty());
	/// ```
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn is_empty(&self) -> bool {
		self.bitspan.len() == 0
	}

	/// Splits the collection into two at the given index.
	///
	/// Returns a newly allocated bit-vector containing the bits in range `[at,
	/// len)`. After the call, the original bit-vector will be left containing
	/// the bits `[0, at)` with its previous capacity unchanged.
	///
	/// # Original
	///
	/// [`Vec::split_off`](alloc::vec::Vec::split_off)
	///
	/// # Panics
	///
	/// Panics if `at > len`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![0, 0, 1];
	/// let bv2 = bv.split_off(1);
	/// assert_eq!(bv, bits![0]);
	/// assert_eq!(bv2, bits![0, 1]);
	/// ```
	#[inline]
	#[must_use = "use `.truncate()` if you don’t need the other half"]
	pub fn split_off(&mut self, at: usize) -> Self {
		let len = self.len();
		assert!(at <= len, "Index {} out of bounds: {}", at, len);
		match at {
			0 => mem::replace(self, Self::new()),
			n if n == len => Self::new(),
			_ => unsafe {
				self.set_len(at);
				self.get_unchecked(at .. len)
					.to_bitvec()
					.pipe(Self::strip_unalias)
			},
		}
	}

	/// Resizes the `BitVec` in-place so that `len` is equal to `new_len`.
	///
	/// If `new_len` is greater than `len`, the `BitVec` is extended by the
	/// difference, with each additional slot filled with the result of calling
	/// the closure `func`. The return values from `func` will end up in the
	/// `BitVec` in the order they have been generated.
	///
	/// If `new_len` is less than `len`, the `BitVec` is simply truncated.
	///
	/// This method uses a closure to create new values on every push. If you’d
	/// rather [`Clone`] a given value, use [`resize`]. If you want to use the
	/// [`Default`] trait to generate values, you can pass
	/// [`Default::default()`] as the second argument.
	///
	/// # Original
	///
	/// [`Vec::resize_with`](alloc::vec::Vec::resize_with)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![1; 3];
	/// bv.resize_with(5, Default::default);
	/// assert_eq!(bv, bits![1, 1, 1, 0, 0]);
	///
	/// let mut bv = bitvec![];
	/// let mut p = 0;
	/// bv.resize_with(4, || { p += 1; p % 2 == 0 });
	/// assert_eq!(bv, bits![0, 1, 0, 1]);
	/// ```
	///
	/// [`Clone`]: core::clone::Clone
	/// [`Default`]: core::default::Default
	/// [`Default::default()`]: core::default::Default::default
	/// [`resize`]: Self::resize
	#[inline]
	pub fn resize_with<F>(&mut self, new_len: usize, func: F)
	where F: FnMut() -> bool {
		let len = self.len();
		if new_len > len {
			self.extend_with(len, new_len, func);
		}
		else {
			self.truncate(new_len);
		}
	}

	/// Consumes and leaks the `BitVec`, returning a mutable reference to the
	/// contents, `&'a mut BitSlice<O, T>`. This lifetime may be chosen to be
	/// `'static`.
	///
	/// This function is similar to the [`leak`] function on [`BitBox`].
	///
	/// This function is mainly useful for data that lives for the remainder of
	/// the program’s life. Dropping the returned reference will cause a memory
	/// leak.
	///
	/// # Original
	///
	/// [`Vec::leak`](alloc::vec::Vec::leak)
	///
	/// # Examples
	///
	/// Simple usage:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let x = bitvec![0, 0, 1];
	/// let static_ref: &'static mut BitSlice = x.leak();
	/// static_ref.set(0, true);
	/// assert_eq!(static_ref, bits![1, 0, 1]);
	/// ```
	///
	/// [`BitBox`]: crate::boxed::BitBox
	/// [`leak`]: crate::boxed::BitBox::leak
	#[inline]
	pub fn leak<'a>(self) -> &'a mut BitSlice<O, T> {
		self.into_boxed_bitslice().pipe(BitBox::leak)
	}

	/// Resizes the `BitVec` in-place so that `len` is equal to `new_len`.
	///
	/// If `new_len` is greater than `len`, the `BitVec` is extended by the
	/// difference, with each additional slot filled with `value`. If `new_len`
	/// is less than `len`, the `BitVec` is simply truncated.
	///
	/// This method requires a single `bool` value. If you need more
	/// flexibility, use [`resize_with`].
	///
	/// # Original
	///
	/// [`Vec::resize`](alloc::vec::Vec::resize)
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![1];
	/// bv.resize(3, false);
	/// assert_eq!(bv, bits![1, 0, 0]);
	///
	/// let mut bv = bitvec![1; 4];
	/// bv.resize(2, false);
	/// assert_eq!(bv, bits![1; 2]);
	/// ```
	///
	/// [`resize_with`]: Self::resize_with
	#[inline]
	pub fn resize(&mut self, new_len: usize, value: bool) {
		let len = self.len();
		if new_len > len {
			self.extend_with(len, new_len, || value);
		}
		else {
			self.truncate(new_len);
		}
	}

	#[doc(hidden)]
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "Prefer `extend_from_bitslice`. If you need to extend from a \
	                slice of `T` elements, use `extend_from_raw_slice`"]
	pub fn extend_from_slice<O2, T2>(&mut self, other: &BitSlice<O2, T2>)
	where
		O2: BitOrder,
		T2: BitStore,
	{
		self.extend_from_bitslice(other);
	}

	/// Resizes the `BitVec` in-place so that `len` is equal to `new_len`.
	#[inline]
	#[deprecated = "`Vec::resize_default` is deprecated"]
	pub fn resize_default(&mut self, new_len: usize) {
		let len = self.len();
		if new_len > len {
			self.extend_with(len, new_len, Default::default);
		}
		else {
			self.truncate(new_len);
		}
	}

	/// Creates a splicing iterator that replaces the specified range in the
	/// bit-vector with the given `replace_with` iterator and yields the removed
	/// items. `replace_with` does not need to be the same length as `range`.
	///
	/// `range` is removed even if the iterator is not consumed until the end.
	///
	/// It is unspecified how many bits are removed from the vector if the
	/// [`Splice`] value is leaked.
	///
	/// The input iterator `replace_with` is only consumed when the [`Splice`]
	/// value is dropped.
	///
	/// This is optimal if:
	///
	/// - the tail (bits in the vector after `range`) is empty
	/// - or `replace_with` yields fewer bits than `range`’s length
	/// - or the lower bound of its [`size_hint`] is exact.
	///
	/// Otherwise, a temporary bit-vector is allocated and the tail is moved
	/// twice.
	///
	/// # Original
	///
	/// [`Vec::splice`](alloc::vec::Vec::splice)
	///
	/// # Panics
	///
	/// Panics if the starting point is greater than the end point or if the end
	/// point is greater than the length of the bit-vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut bv = bitvec![0, 1, 0];
	/// let new = bits![1, 0];
	/// let old: BitVec = bv.splice(.. 2, new.iter().by_val()).collect();
	/// assert_eq!(bv, bits![1, 0, 0]);
	/// assert_eq!(old, bits![0, 1]);
	/// ```
	///
	/// [`Splice`]: crate::vec::Splice
	/// [`size_hint`]: core::iter::Iterator::size_hint
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn splice<R, I>(
		&mut self,
		range: R,
		replace_with: I,
	) -> Splice<O, T, I::IntoIter>
	where
		R: RangeBounds<usize>,
		I: IntoIterator<Item = bool>,
	{
		Splice::new(self.drain(range), replace_with)
	}

	fn extend_with<F>(&mut self, len: usize, new_len: usize, mut func: F)
	where F: FnMut() -> bool {
		self.reserve(new_len - len);

		unsafe {
			for bitptr in
				self.get_unchecked_mut(len .. new_len).as_mut_bitptr_range()
			{
				bitptr.write(func());
			}
			self.set_len_unchecked(new_len);
		}
	}
}
