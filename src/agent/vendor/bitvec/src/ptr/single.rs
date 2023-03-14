//! A pointer to a single bit.

use crate::{
	access::BitAccess,
	index::{
		BitIdx,
		BitIdxError,
	},
	mem::BitMemory,
	mutability::{
		Const,
		Mut,
		Mutability,
	},
	order::{
		BitOrder,
		Lsb0,
	},
	ptr::{
		Address,
		AddressError,
		BitPtrRange,
		BitRef,
		BitSpan,
		BitSpanError,
	},
	store::BitStore,
};

use wyz::fmt::FmtForward;

use core::{
	any::{
		type_name,
		TypeId,
	},
	cmp,
	convert::{
		Infallible,
		TryFrom,
		TryInto,
	},
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
	ptr,
};

/** Pointer to an individual bit in a memory element. Analagous to `*bool`.

# Original

[`*bool`](https://doc.rust-lang.org/std/primitive.pointer.html) and
[`NonNull<bool>`](core::ptr::NonNull)

# API Differences

This must be a structure, rather than a raw pointer, for two reasons:

- It is larger than a raw pointer.
- Raw pointers are not `#[fundamental]` and cannot have foreign implementations.

Additionally, rather than create two structures to map to `*const bool` and
`*mut bool`, respectively, this takes mutability as a type parameter.

Because the encoded span pointer requires that memory addresses are well
aligned, this type also imposes the alignment requirement and refuses
construction for misaligned element addresses. While this type is used in the
API equivalent of ordinary raw pointers, it is restricted in value to only be
*references* to memory elements.

# ABI Differences

This has alignment `1`, rather than an alignment to the processor word. This is
necessary for some crate-internal optimizations.

# Type Parameters

- `M`: Marks whether the pointer permits mutation of memory through it.
- `O`: The ordering of bits within a memory element.
- `T`: A memory type used to select both the register size and the access
  behavior when performing loads/stores.

# Usage

This structure is used as the [`bitvec`] equivalent to `*bool`. It is used in
all raw-pointer APIs, and provides behavior to emulate raw pointers. It cannot
be directly dereferenced, as it is not a pointer; it can only be transformed
back into higher referential types, or used in [`bitvec::ptr`] free functions.

These pointers can never be null, or misaligned.

[`bitvec`]: crate
[`bitvec::ptr`]: crate::ptr
**/
#[repr(C, packed)]
pub struct BitPtr<M, O = Lsb0, T = usize>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	/// Memory addresses must be well-aligned and non-null.
	addr: Address<M, T>,
	/// The index of the referent bit within `*addr`.
	head: BitIdx<T::Mem>,
	/// The ordering used to select the bit at `head` in `*addr`.
	_ord: PhantomData<O>,
}

impl<M, O, T> BitPtr<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	/// The dangling pointer. This selects the starting bit of the `T` dangling
	/// address.
	pub const DANGLING: Self = Self {
		addr: Address::DANGLING,
		head: BitIdx::ZERO,
		_ord: PhantomData,
	};

	/// Loads the address field, sidestepping any alignment problems.
	///
	/// This is the only safe way to access `(&self).addr`. Do not perform field
	/// access on `.addr` through a reference except through this method.
	#[cfg_attr(not(tarpaulin_include), inline(always))]
	pub(crate) fn get_addr(&self) -> Address<M, T> {
		unsafe {
			ptr::read_unaligned(self as *const Self as *const Address<M, T>)
		}
	}

	/// Tries to construct a `BitPtr` from a memory location and a bit index.
	///
	/// # Type Parameters
	///
	/// - `A`: This accepts anything that may be used as a memory address.
	///
	/// # Parameters
	///
	/// - `addr`: The memory address to use in the `BitPtr`. If this value
	///   violates the [`Address`] rules, then its conversion error will be
	///   returned.
	/// - `head`: The index of the bit in `*addr` that this pointer selects. If
	///   this value violates the [`BitIdx`] rules, then its conversion error
	///   will be returned.
	///
	/// # Returns
	///
	/// A new `BitPtr`, selecting the memory location `addr` and the bit `head`.
	/// If either `addr` or `head` are invalid values, then this propagates
	/// their error.
	///
	/// [`Address`]: crate::ptr::Address
	/// [`BitIdx`]: crate::index::BitIdx
	#[inline]
	pub fn try_new<A>(addr: A, head: u8) -> Result<Self, BitPtrError<T>>
	where
		A: TryInto<Address<M, T>>,
		BitPtrError<T>: From<A::Error>,
	{
		Ok(Self::new(addr.try_into()?, BitIdx::new(head)?))
	}

	/// Constructs a `BitPtr` from a memory location and a bit index.
	///
	/// Since this requires that the address and bit index are already
	/// well-formed, it can assemble the `BitPtr` without inspecting their
	/// values.
	///
	/// # Parameters
	///
	/// - `addr`: A well-formed memory address of `T`.
	/// - `head`: A well-formed bit index within `T`.
	///
	/// # Returns
	///
	/// A `BitPtr` selecting the `head` bit in the location `addr`.
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn new(addr: Address<M, T>, head: BitIdx<T::Mem>) -> Self {
		Self {
			addr,
			head,
			_ord: PhantomData,
		}
	}

	/// Decomposes the pointer into its element address and bit index.
	///
	/// # Parameters
	///
	/// - `self`
	///
	/// # Returns
	///
	/// - `.0`: The memory address in which the referent bit is located.
	/// - `.1`: The index of the referent bit within `*.0`.
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn raw_parts(self) -> (Address<M, T>, BitIdx<T::Mem>) {
		(self.addr, self.head)
	}

	/// Produces a `BitSpan`, starting at `self` and running for `bits`.
	///
	/// # Parameters
	///
	/// - `self`: The base bit-address of the returned span descriptor.
	/// - `bits`: The length in bits of the returned span descriptor.
	///
	/// # Returns
	///
	/// This returns an error if the combination of `self` and `bits` violates
	/// any of `BitSpan`’s requirements; otherwise, it encodes `self` and `bits`
	/// into a span descriptor and returns it. Conversion into a `BitSlice`
	/// pointer or reference is left to the caller.
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub(crate) fn span(
		self,
		bits: usize,
	) -> Result<BitSpan<M, O, T>, BitSpanError<T>> {
		BitSpan::new(self.addr, self.head, bits)
	}

	/// Produces a `BitSpan`, starting at `self` and running for `bits`.
	///
	/// This does not perform any validity checking; it only encodes the
	/// arguments into a `BitSpan`.
	///
	/// # Parameters
	///
	/// - `self`: The base bit-address of the returned span descriptor.
	/// - `bits`: The length in bits of the returned span descriptor.
	///
	/// # Retuns
	///
	/// `self` and `bits` encoded into a `BitSpan`. This `BitSpan` may be
	/// semantically invalid, and it may have modulated its length.
	///
	/// # Safety
	///
	/// This should only be called with values that had previously been
	/// extracted from a `BitSpan`.
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub(crate) unsafe fn span_unchecked(self, bits: usize) -> BitSpan<M, O, T> {
		BitSpan::new_unchecked(self.addr, self.head, bits)
	}

	/// Produces a pointer range starting at `self` and running for `count`
	/// bits.
	///
	/// This calls `self.add(count)`, then bundles the resulting pointer as the
	/// high end of the produced range.
	///
	/// # Parameters
	///
	/// - `self`: The starting pointer of the produced range.
	/// - `count`: The number of bits that the produced range includes.
	///
	/// # Returns
	///
	/// A half-open range of pointers, beginning at (and including) `self`,
	/// running for `count` bits, and ending at (and excluding)
	/// `self.add(count)`.
	///
	/// # Safety
	///
	/// `count` cannot violate the constraints in [`add`].
	///
	/// [`add`]: Self::add
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub unsafe fn range(self, count: usize) -> BitPtrRange<M, O, T> {
		BitPtrRange {
			start: self,
			end: self.add(count),
		}
	}

	/// Converts a bit-pointer into a proxy bit-reference.
	///
	/// # Safety
	///
	/// The pointer must be valid to dereference.
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub unsafe fn into_bitref<'a>(self) -> BitRef<'a, M, O, T> {
		BitRef::from_bitptr(self)
	}

	/// Removes write permissions from a bit-pointer.
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn immut(self) -> BitPtr<Const, O, T> {
		let Self { addr, head, .. } = self;
		BitPtr {
			addr: addr.immut(),
			head,
			..BitPtr::DANGLING
		}
	}

	/// Adds write permissions to a bit-pointer.
	///
	/// # Safety
	///
	/// This pointer must have been derived from a `*mut` pointer.
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub unsafe fn assert_mut(self) -> BitPtr<Mut, O, T> {
		let Self { addr, head, .. } = self;
		BitPtr {
			addr: addr.assert_mut(),
			head,
			..BitPtr::DANGLING
		}
	}

	//  `pointer` inherent API

	/// Tests if a bit-pointer is the null value.
	///
	/// This is always false, as `BitPtr` is a `NonNull` internally. Use
	/// `Option<BitPtr>` to express the potential for a null pointer.
	///
	/// # Original
	///
	/// [`pointer::is_null`](https://doc.rust-lang.org/std/primitive.pointer.html#method.is_null)
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	#[deprecated = "`BitPtr` is never null"]
	pub fn is_null(self) -> bool {
		false
	}

	/// Casts to a bit-pointer of another storage type, preserving the
	/// bit-ordering and mutability permissions.
	///
	/// # Original
	///
	/// [`pointer::cast`](https://doc.rust-lang.org/std/primitive.pointer.html#method.cast)
	///
	/// # Behavior
	///
	/// This is not a free typecast! It encodes the pointer as a crate-internal
	/// span descriptor, casts the span descriptor to the `U` storage element
	/// parameter, then decodes the result. This preserves general correctness,
	/// but will likely change both the virtual and physical bits addressed by
	/// this pointer.
	#[inline]
	pub fn cast<U>(self) -> BitPtr<M, O, U>
	where U: BitStore {
		let (addr, head, _) =
			unsafe { self.span_unchecked(1) }.cast::<U>().raw_parts();
		BitPtr::new(addr, head)
	}

	/// Produces a proxy reference to the referent bit.
	///
	/// Because `BitPtr` is a non-null, well-aligned, pointer, this never
	/// returns `None`.
	///
	/// # Original
	///
	/// [`pointer::as_ref`](https://doc.rust-lang.org/std/primitive.pointer.html#method.as_ref)
	///
	/// # API Differences
	///
	/// This produces a proxy type rather than a true reference. The proxy
	/// implements `Deref<Target = bool>`, and can be converted to `&bool` with
	/// `&*`.
	///
	/// # Safety
	///
	/// Since `BitPtr` does not permit null or misaligned pointers, this method
	/// will always dereference the pointer and you must ensure the following
	/// conditions are met:
	///
	/// - the pointer must be dereferencable as defined in the standard library
	///   documentation
	/// - the pointer must point to an initialized instance of `T`
	/// - you must ensure that no other pointer will race to modify the referent
	///   location while this call is reading from memory to produce the proxy
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let data = 1u8;
	/// let ptr = BitPtr::<_, Lsb0, _>::from_ref(&data);
	/// let val = unsafe { ptr.as_ref() }.unwrap();
	/// assert!(*val);
	/// ```
	#[inline]
	pub unsafe fn as_ref<'a>(self) -> Option<BitRef<'a, Const, O, T>> {
		Some(BitRef::from_bitptr(self.immut()))
	}

	/// Calculates the offset from a pointer.
	///
	/// `count` is in units of bits.
	///
	/// # Original
	///
	/// [`pointer::offset`](https://doc.rust-lang.org/std/primitive.pointer.html#method.offset)
	///
	/// # Safety
	///
	/// If any of the following conditions are violated, the result is Undefined
	/// Behavior:
	///
	/// - Both the starting and resulting pointer must be either in bounds or
	///   one byte past the end of the same allocated object. Note that in Rust,
	///   every (stack-allocated) variable is considered a separate allocated
	///   object.
	/// - The computed offset, **in bytes**, cannot overflow an `isize`.
	/// - The offset being in bounds cannot rely on “wrapping around” the
	///   address space. That is, the infinite-precision sum, **in bytes** must
	///   fit in a `usize`.
	///
	/// These pointers are almost always derived from [`BitSlice`] regions,
	/// which have an encoding limitation that the high three bits of the length
	/// counter are zero, so `bitvec` pointers are even less likely than
	/// ordinary pointers to run afoul of these limitations.
	///
	/// Use [`wrapping_offset`] if you expect to risk hitting the high edge of
	/// the address space.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let data = 5u8;
	/// let ptr = BitPtr::<_, Lsb0, _>::from_ref(&data);
	/// assert!(unsafe { ptr.read() });
	/// assert!(!unsafe { ptr.offset(1).read() });
	/// assert!(unsafe { ptr.offset(2).read() });
	/// ```
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	/// [`wrapping_offset`]: Self::wrapping_offset
	#[inline]
	pub unsafe fn offset(self, count: isize) -> Self {
		let (elts, head) = self.head.offset(count);
		Self::new(self.addr.offset(elts), head)
	}

	/// Calculates the offset from a pointer using wrapping arithmetic.
	///
	/// `count` is in units of bits.
	///
	/// # Original
	///
	/// [`pointer::wrapping_offset`](https://doc.rust/lang.org/std/primitive.pointer.html#method.wrapping_offset)
	///
	/// # Safety
	///
	/// The resulting pointer does not need to be in bounds, but it is
	/// potentially hazardous to dereference.
	///
	/// In particular, the resulting pointer remains attached to the same
	/// allocated object that `self` points to. It may *not* be used to access a
	/// different allocated object. Note that in Rust, every (stack-allocated)
	/// variable is considered a separate allocated object.
	///
	/// In other words, `x.wrapping_offset((y as usize).wrapping_sub(x as
	/// usize)` is not the same as `y`, and dereferencing it is undefined
	/// behavior unless `x` and `y` point into the same allocated object.
	///
	/// Compared to [`offset`], this method basically delays the requirement of
	/// staying within the same allocated object: [`offset`] is immediate
	/// Undefined Behavior when crossing object boundaries; `wrapping_offset`
	/// produces a pointer but still leads to Undefined Behavior if that pointer
	/// is dereferenced. [`offset`] can be optimized better and is thus
	/// preferable in performance-sensitive code.
	///
	/// If you need to cross object boundaries, destructure this pointer into
	/// its base address and bit index, cast the base address to an integer, and
	/// do the arithmetic in the purely integer space.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let data = 0u8;
	/// let mut ptr = BitPtr::<_, Lsb0, _>::from_ref(&data);
	/// let end = ptr.wrapping_offset(8);
	/// while ptr < end {
	///   # #[cfg(feature = "std")] {
	///   println!("{}", unsafe { ptr.read() });
	///   # }
	///   ptr = ptr.wrapping_offset(3);
	/// }
	/// ```
	///
	/// [`offset`]: Self::offset
	#[inline]
	pub fn wrapping_offset(self, count: isize) -> Self {
		let (elts, head) = self.head.offset(count);
		Self::new(self.addr.wrapping_offset(elts), head)
	}

	/// Calculates the distance between two pointers. The returned value is in
	/// units of bits.
	///
	/// This function is the inverse of [`offset`].
	///
	/// # Original
	///
	/// [`pointer::offset`](https://doc.rust-lang.org/std/primitive.pointer.html#method.offset_from)
	///
	/// # Safety
	///
	/// If any of the following conditions are violated, the result is Undefined
	/// Behavior:
	///
	/// - Both the starting and other pointer must be either in bounds or one
	///   byte past the end of the same allocated object. Note that in Rust,
	///   every (stack-allocated) variable is considered a separate allocated
	///   object.
	/// - Both pointers must be *derived from* a pointer to the same object.
	/// - The distance between the pointers, **in bytes**, cannot overflow an
	///   `isize`.
	/// - The distance being in bounds cannot rely on “wrapping around” the
	///   address space.
	///
	/// These pointers are almost always derived from [`BitSlice`] regions,
	/// which have an encoding limitation that the high three bits of the length
	/// counter are zero, so `bitvec` pointers are even less likely than
	/// ordinary pointers to run afoul of these limitations.
	///
	/// # Examples
	///
	/// Basic usage:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let data = 0u16;
	/// let base = BitPtr::<_, Lsb0, _>::from_ref(&data);
	/// let low = unsafe { base.add(5) };
	/// let high = unsafe { low.add(6) };
	/// unsafe {
	///   assert_eq!(high.offset_from(low), 6);
	///   assert_eq!(low.offset_from(high), -6);
	///   assert_eq!(low.offset(6), high);
	///   assert_eq!(high.offset(-6), low);
	/// }
	/// ```
	///
	/// *Incorrect* usage:
	///
	/// ```rust,no_run
	/// use bitvec::prelude::*;
	///
	/// let a = 0u8;
	/// let b = !0u8;
	/// let a_ptr = BitPtr::<_, Lsb0, _>::from_ref(&a);
	/// let b_ptr = BitPtr::<_, Lsb0, _>::from_ref(&b);
	/// let diff = (b_ptr.pointer() as isize)
	///   .wrapping_sub(a_ptr.pointer() as isize)
	///   // Remember: raw pointers are byte-addressed,
	///   // but these are bit-addressed.
	///   .wrapping_mul(8);
	/// // Create a pointer to `b`, derived from `a`.
	/// let b_ptr_2 = a_ptr.wrapping_offset(diff);
	///
	/// // The pointers are *arithmetically* equal now
	/// assert_eq!(b_ptr, b_ptr_2);
	/// // Undefined Behavior!
	/// unsafe {
	///   b_ptr_2.offset_from(b_ptr);
	/// }
	/// ```
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	/// [`offset`]: Self::offset
	#[inline]
	pub unsafe fn offset_from(self, origin: Self) -> isize {
		/* Miri complains when performing this arithmetic on pointers. To avoid
		both its costs, and the implicit scaling present in pointer arithmetic,
		this uses pure numeric arithmetic on the address values.
		*/
		self.addr
			.value()
			.wrapping_sub(origin.addr.value())
			//  Pointers step by `T`, but **address values** step by `u8`.
			.wrapping_mul(<u8 as BitMemory>::BITS as usize)
			//  `self.head` moves the end farther from origin,
			.wrapping_add(self.head.value() as usize)
			//  and `origin.head` moves the origin closer to the end.
			.wrapping_sub(origin.head.value() as usize) as isize
	}

	/// Calculates the offset from a pointer (convenience for `.offset(count as
	/// isize)`).
	///
	/// `count` is in units of bits.
	///
	/// # Original
	///
	/// [`pointer::add`](https://doc.rust-lang.org/std/primitive.pointer.html#method.add)
	///
	/// # Safety
	///
	/// See [`offset`].
	///
	/// [`offset`]: Self::offset
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub unsafe fn add(self, count: usize) -> Self {
		self.offset(count as isize)
	}

	/// Calculates the offset from a pointer (convenience for `.offset((count as
	/// isize).wrapping_neg())`).
	///
	/// `count` is in units of bits.
	///
	/// # Original
	///
	/// [`pointer::sub`](https://doc.rust-lang.org/std/primitive.pointer.html#method.sub)
	///
	/// # Safety
	///
	/// See [`offset`].
	///
	/// [`offset`]: Self::offset
	#[inline]
	pub unsafe fn sub(self, count: usize) -> Self {
		self.offset((count as isize).wrapping_neg())
	}

	/// Calculates the offset from a pointer using wrapping arithmetic
	/// (convenience for `.wrapping_offset(count as isize)`).
	///
	/// # Original
	///
	/// [`pointer::wrapping_add`](https://doc.rust-lang.org/std/primitive.pointer.html#method.wrapping_add)
	///
	/// # Safety
	///
	/// See [`wrapping_offset`].
	///
	/// [`wrapping_offset`]: Self::wrapping_offset
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn wrapping_add(self, count: usize) -> Self {
		self.wrapping_offset(count as isize)
	}

	/// Calculates the offset from a pointer using wrapping arithmetic
	/// (convenience for `.wrapping_offset((count as isize).wrapping_neg())`).
	///
	/// # Original
	///
	/// [`pointer::wrapping_sub`](https://doc.rust-lang.org/std/primitive.pointer.html#method.wrapping_sub)
	///
	/// # Safety
	///
	/// See [`wrapping_offset`].
	///
	/// [`wrapping_offset`]: Self::wrapping_offset
	#[inline]
	#[cfg(not(tarpaulin_include))]
	pub fn wrapping_sub(self, count: usize) -> Self {
		self.wrapping_offset((count as isize).wrapping_neg())
	}

	/// Reads the bit from `*self`.
	///
	/// # Original
	///
	/// [`pointer::read`](https://doc.rust-lang.org/std/primitive.pointer.html#method.read)
	///
	/// # Safety
	///
	/// See [`ptr::read`] for safety concerns and examples.
	///
	/// [`ptr::read`]: crate::ptr::read
	#[inline]
	pub unsafe fn read(self) -> bool {
		(&*self.addr.to_const())
			.load_value()
			.get_bit::<O>(self.head)
	}

	/// Performs a volatile read of the bit from `self`.
	///
	/// Volatile operations are intended to act on I/O memory, and are
	/// guaranteed to not be elided or reördered by the compiler across other
	/// volatile operations.
	///
	/// # Original
	///
	/// [`pointer::read_volatile`](https://doc.rust-lang.org/std/primitive.pointer.html#method.read_volatile)
	///
	/// # Safety
	///
	/// See [`ptr::read_volatile`] for safety concerns and examples.
	///
	/// [`ptr::read_volatile`]: crate::ptr::read_volatile
	#[inline]
	pub unsafe fn read_volatile(self) -> bool {
		self.addr.to_const().read_volatile().get_bit::<O>(self.head)
	}

	/// Copies `count` bits from `self` to `dest`. The source and destination
	/// may overlap.
	///
	/// NOTE: this has the *same* argument order as [`ptr::copy`].
	///
	/// # Original
	///
	/// [`pointer::copy_to`](https://doc.rust-lang.org/std/primitive.pointer.html#method.copy_to)
	///
	/// # Safety
	///
	/// See [`ptr::copy`] for safety concerns and examples.
	///
	/// [`ptr::copy`]: crate::ptr::copy
	#[inline]
	pub unsafe fn copy_to<O2, T2>(self, dest: BitPtr<Mut, O2, T2>, count: usize)
	where
		O2: BitOrder,
		T2: BitStore,
	{
		//  If the orderings match, then overlap is permitted and defined.
		if TypeId::of::<O>() == TypeId::of::<O2>() {
			let (addr, head) = dest.raw_parts();
			let dst = BitPtr::<Mut, O, T2>::new(addr, head);
			let src_pair = self.range(count);

			let rev = src_pair.contains(&dst);
			let iter = src_pair.zip(dest.range(count));
			if rev {
				for (from, to) in iter.rev() {
					to.write(from.read());
				}
			}
			else {
				for (from, to) in iter {
					to.write(from.read());
				}
			}
		}
		else {
			//  If the orderings differ, then it is undefined behavior to
			//  overlap in  memory.
			self.copy_to_nonoverlapping(dest, count);
		}
	}

	/// Copies `count` bits from `self` to `dest`. The source and destination
	/// may *not* overlap.
	///
	/// NOTE: this has the *same* argument order as
	/// [`ptr::copy_nonoverlapping`].
	///
	/// # Original
	///
	/// [`pointer::copy_to_nonoverlapping`](https://doc.rust-lang.org/std/primitive.pointer.html#method.copy_to_nonoverlapping)
	///
	/// # Safety
	///
	/// See [`ptr::copy_nonoverlapping`] for safety concerns and examples.
	///
	/// [`ptr::copy_nonoverlapping`](crate::ptr::copy_nonoverlapping)
	#[inline]
	pub unsafe fn copy_to_nonoverlapping<O2, T2>(
		self,
		dest: BitPtr<Mut, O2, T2>,
		count: usize,
	) where
		O2: BitOrder,
		T2: BitStore,
	{
		for (from, to) in self.range(count).zip(dest.range(count)) {
			to.write(from.read());
		}
	}

	/// Computes the offset (in bits) that needs to be applied to the pointer in
	/// order to make it aligned to `align`.
	///
	/// “Alignment” here means that the pointer is selecting the start bit of a
	/// memory location whose address satisfies the requested alignment.
	///
	/// `align` is measured in **bytes**. If you wish to align your bit-pointer
	/// to a specific fraction (½, ¼, or ⅛ of one byte), please file an issue
	/// and this functionality will be added to [`BitIdx`].
	///
	/// # Original
	///
	/// [`pointer::align_offset`](https://doc.rust-lang.org/std/primitive.pointer.html#method.align_offset)
	///
	/// If the base-element address of the pointer is already aligned to
	/// `align`, then this will return the bit-offset required to select the
	/// first bit of the successor element.
	///
	/// If it is not possible to align the pointer, the implementation returns
	/// `usize::MAX`. It is permissible for the implementation to *always*
	/// return `usize::MAX`. Only your algorithm’s performance can depend on
	/// getting a usable offset here, not its correctness.
	///
	/// The offset is expressed in number of bits, and not `T` elements or
	/// bytes. The value returned can be used with the [`wrapping_add`] method.
	///
	/// # Safety
	///
	/// There are no guarantees whatsoëver that offsetting the pointer will not
	/// overflow or go beyond the allocation that the pointer points into. It is
	/// up to the caller to ensure that the returned offset is correct in all
	/// terms other than alignment.
	///
	/// # Panics
	///
	/// The function panics if `align` is not a power-of-two.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let data = [0u8; 3];
	/// let ptr = BitPtr::<_, Lsb0, _>::from_ref(&data[0]);
	/// let ptr = unsafe { ptr.add(2) };
	/// let count = ptr.align_offset(2);
	/// assert!(count > 0);
	/// ```
	///
	/// [`BitIdx`]: crate::index::BitIdx
	/// [`wrapping_add`]: Self::wrapping_add
	#[inline]
	pub fn align_offset(self, align: usize) -> usize {
		let width = <T::Mem as BitMemory>::BITS as usize;
		match (
			self.addr.to_const().align_offset(align),
			self.head.value() as usize,
		) {
			(0, 0) => 0,
			(0, head) => align * 8 - head,
			(usize::MAX, _) => !0,
			(elts, head) => elts.wrapping_mul(width).wrapping_sub(head),
		}
	}
}

impl<O, T> BitPtr<Const, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// Constructs a `BitPtr` from an element reference.
	///
	/// # Parameters
	///
	/// - `elem`: A borrowed memory element.
	///
	/// # Returns
	///
	/// A read-only bit-pointer to the zeroth bit in the `*elem` location.
	#[inline]
	pub fn from_ref(elem: &T) -> Self {
		Self::new(elem.into(), BitIdx::ZERO)
	}

	/// Attempts to construct a `BitPtr` from an element location.
	///
	/// # Parameters
	///
	/// - `elem`: A read-only element address.
	///
	/// # Returns
	///
	/// A read-only bit-pointer to the zeroth bit in the `*elem` location, if
	/// `elem` is well-formed.
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn from_ptr(elem: *const T) -> Result<Self, BitPtrError<T>> {
		Self::try_new(elem, 0)
	}

	/// Constructs a `BitPtr` from a slice reference.
	///
	/// This differs from [`from_ref`] in that the returned pointer keeps its
	/// provenance over the entire slice, whereas producing a pointer to the
	/// base bit of a slice with `BitPtr::from_ref(&slice[0])` narrows its
	/// provenance to only the `slice[0]` element, and calling [`add`] to leave
	/// that element, even remaining in the slice, may cause UB.
	///
	/// # Parameters
	///
	/// - `slice`: An immutabily borrowed slice of memory.
	///
	/// # Returns
	///
	/// A read-only bit-pointer to the zeroth bit in the base location of the
	/// slice.
	///
	/// This pointer has provenance over the entire `slice`, and may safely use
	/// [`add`] to traverse memory elements as long as it stays within the
	/// slice.
	///
	/// [`add`]: Self::add
	/// [`from_ref`]: Self::from_ref
	#[inline]
	pub fn from_slice(slice: &[T]) -> Self {
		Self::new(
			unsafe { Address::new_unchecked(slice.as_ptr() as usize) },
			BitIdx::ZERO,
		)
	}

	/// Gets the pointer to the base memory location containing the referent
	/// bit.
	#[inline]
	#[cfg(not(tarpaulin_include))]
	pub fn pointer(&self) -> *const T {
		self.get_addr().to_const()
	}
}

impl<O, T> BitPtr<Mut, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// Constructs a `BitPtr` from an element reference.
	///
	/// # Parameters
	///
	/// - `elem`: A mutably borrowed memory element.
	///
	/// # Returns
	///
	/// A write-capable bit-pointer to the zeroth bit in the `*elem` location.
	///
	/// Note that even if `elem` is an address within a contiguous array or
	/// slice, the returned bit-pointer only has provenance for the `elem`
	/// location, and no other.
	///
	/// # Safety
	///
	/// The exclusive borrow of `elem` is released after this function returns.
	/// However, you must not use any other pointer than that returned by this
	/// function to view or modify `*elem`, unless the `T` type supports aliased
	/// mutation.
	#[inline]
	pub fn from_mut(elem: &mut T) -> Self {
		Self::new(elem.into(), BitIdx::ZERO)
	}

	/// Attempts to construct a `BitPtr` from an element location.
	///
	/// # Parameters
	///
	/// - `elem`: A write-capable element address.
	///
	/// # Returns
	///
	/// A write-capable bit-pointer to the zeroth bit in the `*elem` location,
	/// if `elem` is well-formed.
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn from_mut_ptr(elem: *mut T) -> Result<Self, BitPtrError<T>> {
		Self::try_new(elem, 0)
	}

	/// Constructs a `BitPtr` from a slice reference.
	///
	/// This differs from [`from_mut`] in that the returned pointer keeps its
	/// provenance over the entire slice, whereas producing a pointer to the
	/// base bit of a slice with `BitPtr::from_mut(&mut slice[0])` narrows its
	/// provenance to only the `slice[0]` element, and calling [`add`] to leave
	/// that element, even remaining in the slice, may cause UB.
	///
	/// # Parameters
	///
	/// - `slice`: A mutabily borrowed slice of memory.
	///
	/// # Returns
	///
	/// A write-capable bit-pointer to the zeroth bit in the base location of
	/// the slice.
	///
	/// This pointer has provenance over the entire `slice`, and may safely use
	/// [`add`] to traverse memory elements as long as it stays within the
	/// slice.
	///
	/// [`add`]: Self::add
	/// [`from_mut`]: Self::from_mut
	#[inline]
	pub fn from_mut_slice(slice: &mut [T]) -> Self {
		Self::new(
			unsafe { Address::new_unchecked(slice.as_mut_ptr() as usize) },
			BitIdx::ZERO,
		)
	}

	/// Gets the pointer to the base memory location containing the referent
	/// bit.
	#[inline]
	#[cfg(not(tarpaulin_include))]
	pub fn pointer(&self) -> *mut T {
		self.get_addr().to_mut()
	}

	//  `pointer` fundamental inherent API

	/// Produces a proxy mutable reference to the referent bit.
	///
	/// Because `BitPtr` is a non-null, well-aligned, pointer, this never
	/// returns `None`.
	///
	/// # Original
	///
	/// [`pointer::as_mut`](https://doc.rust-lang.org/std/primitive.pointer.html#method.as_mut)
	///
	/// # API Differences
	///
	/// This produces a proxy type rather than a true reference. The proxy
	/// implements `DerefMut<Target = bool>`, and can be converted to `&mut
	/// bool` with `&mut *`. Writes to the proxy are not reflected in the
	/// proxied location until the proxy is destroyed, either through `Drop` or
	/// with its [`set`] method.
	///
	/// The proxy must be bound as `mut` in order to write through the binding.
	///
	/// # Safety
	///
	/// Since `BitPtr` does not permit null or misaligned pointers, this method
	/// will always dereference the pointer and you must ensure the following
	/// conditions are met:
	///
	/// - the pointer must be dereferencable as defined in the standard library
	///   documentation
	/// - the pointer must point to an initialized instance of `T`
	/// - you must ensure that no other pointer will race to modify the referent
	///   location while this call is reading from memory to produce the proxy
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut data = 0u8;
	/// let ptr = BitPtr::<_, Lsb0, _>::from_mut(&mut data);
	/// let mut val = unsafe { ptr.as_mut() }.unwrap();
	/// assert!(!*val);
	/// *val = true;
	/// assert!(*val);
	/// ```
	///
	/// [`set`]: crate::ptr::BitRef::set
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub unsafe fn as_mut<'a>(self) -> Option<BitRef<'a, Mut, O, T>> {
		Some(BitRef::from_bitptr(self))
	}

	/// Copies `count` bits from `src` to `self`. The source and destination may
	/// overlap.
	///
	/// Note: this has the *opposite* argument order of [`ptr::copy`].
	///
	/// # Original
	///
	/// [`pointer::copy_from`](https://doc.rust-lang.org/std/primitive.pointer.html#method.copy_from)
	///
	/// # Safety
	///
	/// See [`ptr::copy`] for safety concerns and examples.
	///
	/// [`ptr::copy`]: crate::ptr::copy
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub unsafe fn copy_from<O2, T2>(
		self,
		src: BitPtr<Const, O2, T2>,
		count: usize,
	) where
		O2: BitOrder,
		T2: BitStore,
	{
		src.copy_to(self, count);
	}

	/// Copies `count` bits from `src` to `self`. The source and destination may
	/// *not* overlap.
	///
	/// NOTE: this has the *opposite* argument order of
	/// [`ptr::copy_nonoverlapping`].
	///
	/// # Original
	///
	/// [`pointer::copy_from_nonoverlapping`](https://doc.rust-lang.org/std/primitive.pointer.html#method.copy_from_nonoverlapping)
	///
	/// # Safety
	///
	/// See [`ptr::copy_nonoverlapping`] for safety concerns and examples.
	///
	/// [`ptr::copy_nonoverlapping`]: crate::ptr::copy_nonoverlapping
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub unsafe fn copy_from_nonoverlapping<O2, T2>(
		self,
		src: BitPtr<Const, O2, T2>,
		count: usize,
	) where
		O2: BitOrder,
		T2: BitStore,
	{
		src.copy_to_nonoverlapping(self, count);
	}

	/// Overwrites a memory location with the given bit.
	///
	/// See [`ptr::write`] for safety concerns and examples.
	///
	/// # Original
	///
	/// [`pointer::write`](https://doc.rust-lang.org/std/primitive.pointer.html#method.write)
	///
	/// [`ptr::write`]: crate::ptr::write
	#[inline]
	#[allow(clippy::clippy::missing_safety_doc)]
	pub unsafe fn write(self, value: bool) {
		(&*self.addr.to_access()).write_bit::<O>(self.head, value);
	}

	/// Performs a volatile write of a memory location with the given bit.
	///
	/// Because processors do not have single-bit write instructions, this must
	/// perform a volatile read of the location, perform the bit modification
	/// within the processor register, and then perform a volatile write back to
	/// memory. These three steps are guaranteed to be sequential, but are not
	/// guaranteed to be atomic.
	///
	/// Volatile operations are intended to act on I/O memory, and are
	/// guaranteed to not be elided or reördered by the compiler across other
	/// volatile operations.
	///
	/// # Original
	///
	/// [`pointer::write_volatile`](https://doc.rust-lang.org/std/primitive.pointer.html#method.write_volatile)
	///
	/// # Safety
	///
	/// See [`ptr::write_volatile`] for safety concerns and examples.
	///
	/// [`ptr::write_volatile`]: crate::ptr::write_volatile
	#[inline]
	pub unsafe fn write_volatile(self, val: bool) {
		let select = O::select(self.head).value();
		let mut tmp = self.addr.to_mem().read_volatile();
		if val {
			tmp |= &select;
		}
		else {
			tmp &= &!select;
		}
		self.addr.to_mem_mut().write_volatile(tmp);
	}

	/// Replaces the bit at `*self` with `src`, returning the old bit.
	///
	/// # Original
	///
	/// [`pointer::replace`](https://doc.rust-lang.org/std/primitive.pointer.html#method.replace)
	///
	/// # Safety
	///
	/// See [`ptr::replace`] for safety concerns and examples.
	///
	/// [`ptr::replace`]: crate::ptr::replace
	#[inline]
	pub unsafe fn replace(self, src: bool) -> bool {
		let out = self.read();
		self.write(src);
		out
	}

	/// Swaps the bits at two mutable locations. They may overlap.
	///
	/// # Original
	///
	/// [`pointer::swap`](https://doc.rust-lang.org/std/primitive.pointer.html#method.swap)
	///
	/// # Safety
	///
	/// See [`ptr::swap`] for safety concerns and examples.
	///
	/// [`ptr::swap`]: crate::ptr::swap
	#[inline]
	pub unsafe fn swap<O2, T2>(self, with: BitPtr<Mut, O2, T2>)
	where
		O2: BitOrder,
		T2: BitStore,
	{
		let (a, b) = (self.read(), with.read());
		self.write(b);
		with.write(a);
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> Clone for BitPtr<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline(always)]
	fn clone(&self) -> Self {
		Self {
			addr: self.get_addr(),
			..*self
		}
	}
}

impl<M, O, T> Eq for BitPtr<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> Ord for BitPtr<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline]
	fn cmp(&self, other: &Self) -> cmp::Ordering {
		self.partial_cmp(other).expect(
			"BitPtr has a total ordering when type parameters are identical",
		)
	}
}

#[cfg(not(tarpaulin_include))]
impl<M1, M2, O, T1, T2> PartialEq<BitPtr<M2, O, T2>> for BitPtr<M1, O, T1>
where
	M1: Mutability,
	M2: Mutability,
	O: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	#[inline]
	fn eq(&self, other: &BitPtr<M2, O, T2>) -> bool {
		if TypeId::of::<T1::Mem>() != TypeId::of::<T2::Mem>() {
			return false;
		}
		self.get_addr().value() == other.get_addr().value()
			&& self.head.value() == other.head.value()
	}
}

#[cfg(not(tarpaulin_include))]
impl<M1, M2, O, T1, T2> PartialOrd<BitPtr<M2, O, T2>> for BitPtr<M1, O, T1>
where
	M1: Mutability,
	M2: Mutability,
	O: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	#[inline]
	fn partial_cmp(&self, other: &BitPtr<M2, O, T2>) -> Option<cmp::Ordering> {
		if TypeId::of::<T1::Mem>() != TypeId::of::<T2::Mem>() {
			return None;
		}
		match (self.get_addr().value()).cmp(&other.get_addr().value()) {
			cmp::Ordering::Equal => {
				self.head.value().partial_cmp(&other.head.value())
			},
			ord => Some(ord),
		}
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, T> From<&T> for BitPtr<Const, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	#[inline]
	fn from(elem: &T) -> Self {
		Self::new(elem.into(), BitIdx::ZERO)
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, T> From<&mut T> for BitPtr<Mut, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	#[inline]
	fn from(elem: &mut T) -> Self {
		Self::new(elem.into(), BitIdx::ZERO)
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, T> TryFrom<*const T> for BitPtr<Const, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	type Error = BitPtrError<T>;

	#[inline(always)]
	fn try_from(elem: *const T) -> Result<Self, Self::Error> {
		Self::try_new(elem, 0)
	}
}

#[cfg(not(tarpaulin_include))]
impl<O, T> TryFrom<*mut T> for BitPtr<Mut, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	type Error = BitPtrError<T>;

	#[inline(always)]
	fn try_from(elem: *mut T) -> Result<Self, Self::Error> {
		Self::try_new(elem, 0)
	}
}

impl<M, O, T> Debug for BitPtr<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		write!(
			fmt,
			"*{} Bit<{}, {}>",
			match TypeId::of::<M>() {
				t if t == TypeId::of::<Const>() => "const",
				t if t == TypeId::of::<Mut>() => "mut",
				_ => unreachable!("No other implementors exist"),
			},
			type_name::<O>(),
			type_name::<T>()
		)?;
		Pointer::fmt(self, fmt)
	}
}

impl<M, O, T> Pointer for BitPtr<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		fmt.debug_tuple("")
			.field(&self.get_addr().fmt_pointer())
			.field(&self.head.fmt_binary())
			.finish()
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, O, T> Hash for BitPtr<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
	#[inline]
	fn hash<H>(&self, state: &mut H)
	where H: Hasher {
		self.get_addr().hash(state);
		self.head.hash(state);
	}
}

impl<M, O, T> Copy for BitPtr<M, O, T>
where
	M: Mutability,
	O: BitOrder,
	T: BitStore,
{
}

/// Errors produced by invalid bit-pointer components.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BitPtrError<T>
where T: BitStore
{
	/// The element address was somehow invalid.
	InvalidAddress(AddressError<T>),
	/// The bit index was somehow invalid.
	InvalidIndex(BitIdxError<T::Mem>),
}

#[cfg(not(tarpaulin_include))]
impl<T> From<AddressError<T>> for BitPtrError<T>
where T: BitStore
{
	#[inline(always)]
	fn from(err: AddressError<T>) -> Self {
		Self::InvalidAddress(err)
	}
}

#[cfg(not(tarpaulin_include))]
impl<T> From<BitIdxError<T::Mem>> for BitPtrError<T>
where T: BitStore
{
	#[inline(always)]
	fn from(err: BitIdxError<T::Mem>) -> Self {
		Self::InvalidIndex(err)
	}
}

#[cfg(not(tarpaulin_include))]
impl<T> From<Infallible> for BitPtrError<T>
where T: BitStore
{
	#[inline(always)]
	fn from(_: Infallible) -> Self {
		unreachable!("Infallible errors can never be produced");
	}
}

#[cfg(not(tarpaulin_include))]
impl<T> Display for BitPtrError<T>
where T: BitStore
{
	#[inline]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		match self {
			Self::InvalidAddress(addr) => Display::fmt(addr, fmt),
			Self::InvalidIndex(index) => Display::fmt(index, fmt),
		}
	}
}

unsafe impl<T> Send for BitPtrError<T> where T: BitStore
{
}

unsafe impl<T> Sync for BitPtrError<T> where T: BitStore
{
}

#[cfg(feature = "std")]
impl<T> std::error::Error for BitPtrError<T> where T: BitStore
{
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		mutability::Const,
		prelude::Lsb0,
	};

	#[test]
	fn ctor() {
		let data = 0u16;
		let head = 5;

		let bitptr = BitPtr::<Const, Lsb0, _>::try_new(&data, head).unwrap();
		let (addr, indx) = bitptr.raw_parts();
		assert_eq!(addr.to_const(), &data as *const _);
		assert_eq!(indx.value(), head);
	}

	#[test]
	fn bitref() {
		let data = 1u32 << 23;
		let head = 23;
		let bitptr = BitPtr::<Const, Lsb0, _>::try_new(&data, head).unwrap();
		let bitref = unsafe { bitptr.as_ref() }.unwrap();
		assert!(*bitref);
	}

	#[test]
	fn assert_size() {
		assert!(
			core::mem::size_of::<BitPtr<Const, Lsb0, u8>>()
				<= core::mem::size_of::<usize>() + core::mem::size_of::<u8>(),
		);
	}

	#[test]
	#[cfg(feature = "alloc")]
	fn format() {
		use crate::order::Msb0;
		#[cfg(not(feature = "std"))]
		use alloc::format;

		let base = 0u16;
		let bitptr = BitPtr::<_, Msb0, _>::from_ref(&base);
		let text = format!("{:?}", unsafe { bitptr.add(3) });
		let render = format!(
			"*const Bit<bitvec::order::Msb0, u16>({:p}, {:04b})",
			&base as *const u16, 3
		);
		assert_eq!(text, render);
	}
}
