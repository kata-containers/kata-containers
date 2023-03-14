//! Non-null, well-aligned, `BitStore` addresses with limited casting capability

use crate::{
	mem::BitMemory,
	mutability::{
		Const,
		Mut,
		Mutability,
	},
	store::BitStore,
};

use core::{
	any::{
		type_name,
		TypeId,
	},
	cmp,
	convert::{
		Infallible,
		TryFrom,
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
	mem::align_of,
	ptr::NonNull,
};

use tap::pipe::Pipe;

/** A non-null, well-aligned, `BitStore` element address.

This adds non-null and well-aligned requirements to memory addresses so that the
crate can rely on these invariants throughout its implementation. The type is
public API, but opaque, and only constructible through conversions of pointer
and reference values.

# Type Parameters

- `M`: The mutability permissions of the source pointer.
- `T`: The referent type of the source pointer.
**/
#[repr(transparent)]
pub struct Address<M, T = usize>
where
	M: Mutability,
	T: BitStore,
{
	/// `Address` is just a wrapper over `NonNull` with some additional casting
	/// abilities.
	inner: NonNull<T>,
	/// In addition, `Address` tracks the write permissions of its source.
	_mut: PhantomData<M>,
}

#[cfg(not(tarpaulin_include))]
impl<M, T> Address<M, T>
where
	M: Mutability,
	T: BitStore,
{
	/// The dangling address.
	pub(crate) const DANGLING: Self = Self {
		inner: NonNull::dangling(),
		_mut: PhantomData,
	};

	/// Attempts to create a new `Address` from a location value.
	///
	/// # Parameters
	///
	/// - `addr`: Any location value.
	///
	/// # Returns
	///
	/// If `addr` is not the null address, and is well-aligned for `T`, this
	/// returns an `Address` wrapping it; if either condition is violated, this
	/// returns the corresponding error.
	#[inline]
	pub(crate) fn new(addr: usize) -> Result<Self, AddressError<T>> {
		let align_mask = align_of::<T>() - 1;
		if addr & align_mask != 0 {
			return Err(AddressError::Misaligned(addr as *const T));
		}
		NonNull::new(addr as *mut T)
			.ok_or(AddressError::Null)
			.map(|inner| Self {
				inner,
				_mut: PhantomData,
			})
	}

	/// Creates a new `Address` without checking the value for correctness.
	///
	/// # Safety
	///
	/// `addr` must be well-aligned and not null.
	#[inline(always)]
	pub(crate) unsafe fn new_unchecked(addr: usize) -> Self {
		Self {
			inner: NonNull::new_unchecked(addr as *mut T),
			_mut: PhantomData,
		}
	}

	/// Removes write permissions from the `Address`.
	#[inline(always)]
	pub(crate) fn immut(self) -> Address<Const, T> {
		let Self { inner, .. } = self;
		Address {
			inner,
			..Address::DANGLING
		}
	}

	/// Adds write permissions to the `Address`.
	///
	/// # Safety
	///
	/// When called on an `Address<Const, _>`, the address must have been
	/// previously constructed from a mutable location and lowered with `immut`.
	#[inline(always)]
	pub(crate) unsafe fn assert_mut(self) -> Address<Mut, T> {
		let Self { inner, .. } = self;
		Address {
			inner,
			..Address::DANGLING
		}
	}

	/// Applies `<*mut T>::offset`.
	///
	/// # Panics
	///
	/// This panics if the result of applying the offset is the null pointer.
	#[inline]
	pub(crate) unsafe fn offset(mut self, count: isize) -> Self {
		self.inner = self
			.inner
			.as_ptr()
			.offset(count)
			.pipe(NonNull::new)
			.expect("Offset cannot produce the null pointer");
		self
	}

	/// Applies `<*mut T>::wrapping_offset`.
	///
	/// # Panics
	///
	/// This panics if the result of applying the offset is the null pointer.
	#[inline]
	pub(crate) fn wrapping_offset(mut self, count: isize) -> Self {
		self.inner = self
			.inner
			.as_ptr()
			.wrapping_offset(count)
			.pipe(NonNull::new)
			.expect("Wrapping offset cannot produce the null pointer");
		self
	}

	/// Gets the address as a pointer to the access type.
	#[inline(always)]
	pub(crate) fn to_access(self) -> *const T::Access {
		self.to_const().cast::<T::Access>()
	}

	/// Gets the address as a read-only pointer.
	#[inline(always)]
	pub fn to_const(self) -> *const T {
		self.inner.as_ptr() as *const T
	}

	/// Gets the address as a read-only pointer to the register type.
	#[inline(always)]
	pub(crate) fn to_mem(self) -> *const T::Mem {
		self.to_const().cast::<T::Mem>()
	}

	/// Gets the address as a non-null pointer.
	#[inline(always)]
	#[cfg(feature = "alloc")]
	pub(crate) fn to_nonnull(self) -> NonNull<T> {
		self.inner
	}

	/// Gets the raw numeric value of the address.
	#[inline(always)]
	pub(crate) fn value(self) -> usize {
		self.inner.as_ptr() as usize
	}
}

#[cfg(not(tarpaulin_include))]
impl<T> Address<Mut, T>
where T: BitStore
{
	/// Gets the address as a write-capable pointer.
	#[inline(always)]
	#[allow(clippy::clippy::wrong_self_convention)]
	pub fn to_mut(self) -> *mut T {
		self.inner.as_ptr()
	}

	/// Gets the address as a write-capable pointer to the register type.
	#[inline(always)]
	pub(crate) fn to_mem_mut(self) -> *mut T::Mem {
		self.to_mut().cast::<T::Mem>()
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, T> Clone for Address<M, T>
where
	M: Mutability,
	T: BitStore,
{
	#[inline(always)]
	fn clone(&self) -> Self {
		*self
	}
}

#[cfg(not(tarpaulin_include))]
impl<T> From<&T> for Address<Const, T>
where T: BitStore
{
	#[inline(always)]
	fn from(elem: &T) -> Self {
		Self {
			inner: elem.into(),
			_mut: PhantomData,
		}
	}
}

#[cfg(not(tarpaulin_include))]
impl<T> From<&mut T> for Address<Mut, T>
where T: BitStore
{
	#[inline(always)]
	fn from(elem: &mut T) -> Self {
		Self {
			inner: elem.into(),
			_mut: PhantomData,
		}
	}
}

#[cfg(not(tarpaulin_include))]
impl<T> TryFrom<*const T> for Address<Const, T>
where T: BitStore
{
	type Error = AddressError<T>;

	#[inline(always)]
	fn try_from(elem: *const T) -> Result<Self, Self::Error> {
		Self::new(elem as usize)
	}
}

#[cfg(not(tarpaulin_include))]
impl<T> TryFrom<*mut T> for Address<Mut, T>
where T: BitStore
{
	type Error = AddressError<T>;

	#[inline(always)]
	fn try_from(elem: *mut T) -> Result<Self, Self::Error> {
		Self::new(elem as usize)
	}
}

impl<M, T> Eq for Address<M, T>
where
	M: Mutability,
	T: BitStore,
{
}

#[cfg(not(tarpaulin_include))]
impl<M1, M2, T1, T2> PartialEq<Address<M2, T2>> for Address<M1, T1>
where
	M1: Mutability,
	M2: Mutability,
	T1: BitStore,
	T2: BitStore,
{
	#[inline]
	fn eq(&self, other: &Address<M2, T2>) -> bool {
		TypeId::of::<T1::Mem>() == TypeId::of::<T2::Mem>()
			&& self.value() == other.value()
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, T> Ord for Address<M, T>
where
	M: Mutability,
	T: BitStore,
{
	#[inline]
	fn cmp(&self, other: &Self) -> cmp::Ordering {
		self.partial_cmp(&other)
			.expect("Addresses have a total ordering")
	}
}

#[cfg(not(tarpaulin_include))]
impl<M1, M2, T1, T2> PartialOrd<Address<M2, T2>> for Address<M1, T1>
where
	M1: Mutability,
	M2: Mutability,
	T1: BitStore,
	T2: BitStore,
{
	#[inline]
	fn partial_cmp(&self, other: &Address<M2, T2>) -> Option<cmp::Ordering> {
		if TypeId::of::<T1::Mem>() != TypeId::of::<T2::Mem>() {
			return None;
		};
		self.value().partial_cmp(&other.value())
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, T> Debug for Address<M, T>
where
	M: Mutability,
	T: BitStore,
{
	#[inline(always)]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		Debug::fmt(&self.to_const(), fmt)
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, T> Pointer for Address<M, T>
where
	M: Mutability,
	T: BitStore,
{
	#[inline(always)]
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		Pointer::fmt(&self.to_const(), fmt)
	}
}

#[cfg(not(tarpaulin_include))]
impl<M, T> Hash for Address<M, T>
where
	M: Mutability,
	T: BitStore,
{
	#[inline(always)]
	fn hash<H>(&self, state: &mut H)
	where H: Hasher {
		self.inner.hash(state)
	}
}

impl<M, T> Copy for Address<M, T>
where
	M: Mutability,
	T: BitStore,
{
}

/// An error produced when consuming `BitStore` memory addresses.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AddressError<T>
where T: BitStore
{
	/// `Address` cannot use the null pointer.
	Null,
	/// `Address` cannot be misaligned for the referent type `T`.
	Misaligned(*const T),
}

impl<T> From<Infallible> for AddressError<T>
where T: BitStore
{
	fn from(_: Infallible) -> Self {
		unreachable!("Infallible errors can never be produced");
	}
}

impl<T> Display for AddressError<T>
where T: BitStore
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		match *self {
			Self::Null => {
				fmt.write_str("`bitvec` will not operate on the null pointer")
			},
			Self::Misaligned(ptr) => write!(
				fmt,
				"`bitvec` requires that the address {:p} clear its least {} \
				 bits to be aligned for type {}",
				ptr,
				T::Mem::INDX - 3,
				type_name::<T::Mem>(),
			),
		}
	}
}

#[cfg(feature = "std")]
impl<T> std::error::Error for AddressError<T> where T: BitStore
{
}
