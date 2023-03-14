/*! Representations of the [`BitSlice`] region memory model.

This module allows any [`BitSlice`] region to be decomposed into domains that
restricts [`T::Alias`] markers to only the edge elements that may require them.

Specifically, any given [`BitSlice`] region is one of:

- touches only interior indices of one element
- touches at least one edge index of any number of elements (including zero)

In the latter case, any elements *completely* spanned by the [`BitSlice`] handle
are known to not have any other write-capable handles to them, and in the case
of an `&mut BitSlice` handle specifically, no other views at all. As such, the
domain view of this memory is able to remove the aliasing marker type and permit
direct memory access to the underlying buffer for the duration of its existence.

[`BitSlice`]: crate::slice::BitSlice
[`T::Alias`]: crate::store::BitStore::Alias
!*/

use crate::{
	index::{
		BitIdx,
		BitTail,
	},
	mem::BitMemory,
	order::BitOrder,
	slice::BitSlice,
	store::BitStore,
};

use core::{
	fmt::{
		self,
		Binary,
		Debug,
		Formatter,
		LowerHex,
		Octal,
		UpperHex,
	},
	slice,
};

use tap::{
	pipe::Pipe,
	tap::Tap,
};

use wyz::fmt::FmtForward;

macro_rules! bit_domain {
	($t:ident $(=> $m:ident)? $(@ $a:ident)?) => {
		/// Granular representation of the memory region containing a
		/// [`BitSlice`].
		///
		/// [`BitSlice`] regions can be described in terms of edge and center
		/// partitions, where the edge partitions must retain the aliasing
		/// status of the source `BitSlice` handle, and the center partition is
		/// known to be completely unaliased by any other view. This property
		/// allows any `BitSlice` handle to be decomposed into smaller regions,
		/// and safely remove any aliasing markers from the center partition
		/// that no longer requires such safeguarding.
		///
		/// This enum acts like the `.split*` methods in that it only subdivides
		/// the source [`BitSlice`] into smaller `BitSlice`s, and makes
		/// appropriate modifications to the aliasing markers.
		///
		/// It does not add any aliasing markers: if the slice is marked as
		/// aliased, then the edge partitions will retain that marker, and if it
		/// is not, then the edge partitions do not need it.
		///
		/// This does not provide references to the underlying memory buffer. If
		/// you need such direct access, use the [`Domain`] or [`DomainMut`]
		/// enums.
		///
		/// # Lifetimes
		///
		/// - `'a`: The lifetime of the referent storage region.
		///
		/// # Type Parameters
		///
		/// - `O`: The ordering type of the source [`BitSlice`] handle.
		/// - `T`: The register type of the source [`BitSlice`] handle,
		///   including any aliasing markers.
		///
		/// # Aliasing Awareness
		///
		/// This enum does not grant access to memory outside the scope of the
		/// original [`BitSlice`] handle, and so does not need to modfiy any
		/// aliasing conditions.
		///
		/// [`BitSlice`]: crate::slice::BitSlice
		/// [`Domain`]: crate::domain::Domain
		/// [`DomainMut`]: crate::domain::DomainMut
		#[derive(Debug)]
		pub enum $t <'a, O, T>
		where
			O: BitOrder,
			T: BitStore,
		{
			/// Indicates that a [`BitSlice`] is contained entirely in the
			/// interior indices of a single memory register.
			///
			/// [`BitSlice`]: crate::slice::BitSlice
			Enclave {
				/// The start index of the [`BitSlice`].
				///
				/// This is not likely to be useful information, but is retained
				/// for structural similarity with the rest of the module.
				///
				/// [`BitSlice`]: crate::slice::BitSlice
				head: BitIdx<T::Mem>,
				/// The original [`BitSlice`] used to create this bit-domain
				/// view.
				///
				/// [`BitSlice`]: crate::slice::BitSlice
				body: &'a $($m)? BitSlice<O, T>,
				/// The end index of the [`BitSlice`].
				///
				/// This is not likely to be useful information, but is retained
				/// for structural similarity with the rest of the module.
				///
				/// [`BitSlice`]: crate::slice::BitSlice
				tail: BitTail<T::Mem>,
			},
			/// Indicates that a [`BitSlice`] region touches at least one edge
			/// index of any number of elements.
			///
			/// This contains two [`BitSlice`]s representing the
			/// partially-occupied edge elements, with their original aliasing
			/// marker, and one `BitSlice` representing the fully-occupied
			/// interior elements, marked as unaliased.
			///
			/// [`BitSlice`]: crate::slice::BitSlice
			Region {
				/// Any bits that partially-fill the base element of the slice
				/// region.
				///
				/// This does not modify its aliasing status, as it will already
				/// be appropriately marked before constructing this view.
				head: &'a $($m)? BitSlice<O, T>,
				/// Any bits inside elements that the source [`BitSlice`]
				/// completely covers.
				///
				/// This is marked as unaliased, because it is statically
				/// impossible for any other handle to have write access to the
				/// region it covers. As such, a [`BitSlice`] that was marked as
				/// entirely aliased, but contains interior unaliased elements,
				/// can safely remove its aliasing protections.
				///
				/// [`BitSlice`]: crate::slice::BitSlice
				body: &'a $($m)? BitSlice<O, T::Unalias>,
				/// Any bits that partially fill the last element of the slice
				/// region.
				///
				/// This does not modify its aliasing status, as it will already
				/// be appropriately marked before constructing this view.
				tail: &'a $($m)? BitSlice<O, T>,
			},
		}

		impl<'a, O, T> $t <'a, O, T>
		where
			O: BitOrder,
			T: BitStore,
		{
			/// Attempts to view the domain as an enclave variant.
			///
			/// # Parameters
			///
			/// - `self`
			///
			/// # Returns
			///
			/// If `self` is the [`Enclave`] variant, this returns `Some` of the
			/// enclave fields, as a tuple. Otherwise, it returns `None`.
			///
			/// [`Enclave`]: Self::Enclave
			pub fn enclave(self) -> Option<(
				BitIdx<T::Mem>,
				&'a $($m)? BitSlice<O, T>,
				BitTail<T::Mem>,
			)> {
				if let Self::Enclave { head, body, tail } = self {
					Some((head, body, tail))
				}
				else {
					None
				}
			}

			/// Attempts to view the domain as a region variant.
			///
			/// # Parameters
			///
			/// - `self`
			///
			/// # Returns
			///
			/// If `self` is the [`Region`] variant, this returns `Some` of the
			/// region fields, as a tuple. Otherwise, it returns `None`.
			///
			/// [`Region`]: Self::Region
			pub fn region(self) -> Option<(
				&'a $($m)? BitSlice<O, T>,
				&'a $($m)? BitSlice<O, T::Unalias>,
				&'a $($m)? BitSlice<O, T>,
			)> {
				if let Self::Region { head, body, tail } = self {
					Some((head, body, tail))
				}
				else {
					None
				}
			}

			/// Constructs a bit-domain view from a [`BitSlice`].
			///
			/// # Parameters
			///
			/// - `slice`: The source [`BitSlice`] for which the view is
			///   constructed.
			///
			/// # Returns
			///
			/// A bit-domain view over the source slice.
			///
			/// [`BitSlice`]: crate::slice::BitSlice
			pub(crate) fn new(slice: &'a $($m)? BitSlice<O, T>) -> Self {
				let bitspan = slice.as_bitspan();
				let h = bitspan.head();
				let (e, t) = h.span(bitspan.len());
				let w = T::Mem::BITS;

				match (h.value(), e, t.value()) {
					(_, 0, _) => Self::empty(),
					(0, _, t) if t == w => Self::spanning(slice),
					(_, _, t) if t == w => Self::partial_head(slice, h),
					(0, ..) => Self::partial_tail(slice, h, t),
					(_, 1, _) => Self::minor(slice, h, t),
					_ => Self::major(slice, h, t),
				}
			}

			fn empty() -> Self {
				Self::Region {
					head: Default::default(),
					body: Default::default(),
					tail: Default::default(),
				}
			}

			fn major(
				slice: &'a $($m)? BitSlice<O, T>,
				head: BitIdx<T::Mem>,
				tail: BitTail<T::Mem>,
			) -> Self {
				let (head, rest) = bit_domain!(split $($m)?
					slice,
					(T::Mem::BITS - head.value()) as usize,
				);
				let (body, tail) = bit_domain!(split $($m)?
					rest,
					rest.len() - (tail.value() as usize),
				);
				Self::Region {
					head: bit_domain!(retype $($m)? head),
					body: bit_domain!(retype $($m)? body),
					tail: bit_domain!(retype $($m)? tail),
				}
			}

			fn minor(
				slice: &'a $($m)? BitSlice<O, T>,
				head: BitIdx<T::Mem>,
				tail: BitTail<T::Mem>,
			) -> Self {
				Self::Enclave {
					head,
					body: slice,
					tail,
				}
			}

			fn partial_head(
				slice: &'a $($m)? BitSlice<O, T>,
				head: BitIdx<T::Mem>,
			) -> Self {
				let (head, rest) = bit_domain!(split $($m)?
					slice,
					(T::Mem::BITS - head.value()) as usize,
				);
				let (head, body) = (
					bit_domain!(retype $($m)? head),
					bit_domain!(retype $($m)? rest),
				);
				Self::Region {
					head,
					body,
					tail: Default::default(),
				}
			}

			fn partial_tail(
				slice: &'a $($m)? BitSlice<O, T>,
				/* This discarded head argument makes all constructor functions
				have the same register layout for the call, allowing the `::new`
				function to establish the arguments ahead of time, then select a
				constructor function to jump into.
				*/
				_head: BitIdx<T::Mem>,
				tail: BitTail<T::Mem>,
			) -> Self {
				let (rest, tail) = bit_domain!(split $($m)?
					slice,
					slice.len() - (tail.value() as usize),
				);
				let (body, tail) = (
					bit_domain!(retype $($m)? rest),
					bit_domain!(retype $($m)? tail),
				);
				Self::Region {
					head: Default::default(),
					body,
					tail,
				}
			}

			fn spanning(slice: &'a $($m)? BitSlice<O, T>) -> Self {
				Self::Region {
					head: Default::default(),
					body: bit_domain!(retype $($m)? slice),
					tail: Default::default(),
				}
			}
		}
	};

	(retype mut $slice:ident $(,)? ) => {
		unsafe { &mut *($slice as *mut BitSlice<O, _> as *mut BitSlice<O, _>) }
	};
	(retype $slice:ident $(,)? ) => {
		unsafe { &*($slice as *const BitSlice<O, _> as *const BitSlice<O, _>) }
	};

	(split mut $slice:ident, $at:expr $(,)? ) => {
		unsafe { $slice.split_at_unchecked_mut($at) }
	};
	(split $slice:ident, $at:expr $(,)? ) => {
		unsafe { $slice.split_at_unchecked($at) }
	};
}

bit_domain!(BitDomain);
bit_domain!(BitDomainMut => mut @ Alias);

#[cfg(not(tarpaulin_include))]
impl<O, T> Clone for BitDomain<'_, O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn clone(&self) -> Self {
		*self
	}
}

impl<O, T> Copy for BitDomain<'_, O, T>
where
	O: BitOrder,
	T: BitStore,
{
}

macro_rules! domain {
	($t:ident $(=> $m:ident @ $a:ident)?) => {
		/// Granular representation of the memory region containing a
		/// [`BitSlice`].
		///
		/// [`BitSlice`] regions can be described in terms of edge and center
		/// partitions, where the edge partitions must retain the aliasing
		/// status of the source `BitSlice` handle, and the center partition is
		/// known to be completely unaliased by any other view. This property
		/// allows any `BitSlice` handle to be decomposed into smaller regions,
		/// and safely remove any aliasing markers from the center partition
		/// that no longer requires such safeguarding.
		///
		/// This enum splits the underlying element slice `[T]` into the
		/// maybe-aliased edge elements and known-unaliased center elements. If
		/// you do not need to work with the memory elements directly, and only
		/// need to firmly specify the access behavior of the [`BitSlice`]
		/// handle, use the [`BitDomain`] and [`BitDomainMut`] enums.
		///
		/// # Lifetimes
		///
		/// - `'a`: The lifetime of the referent storage region.
		///
		/// # Type Parameters
		///
		/// - `T`: The register type of the source [`BitSlice`] handle,
		///   including any aliasing markers.
		///
		/// # Mutability
		///
		/// The immutable view produces [`T::Alias`] references, which permit
		/// foreign writes to the referent location but disallow writes through
		/// itself. The mutable view produces [`T::Access`] references, because
		/// `&mut _` references can only ever be produced when no other aliasing
		/// handle exists. The write permissions must be weakened from
		/// `&mut T::Alias` to `&T::Access` in order to satisfy the Rust memory
		/// rules.
		///
		/// The edge references do not forbid modifying bits outside of the
		/// source [`BitSlice`] domain, and writes out of bounds will be
		/// correctly handled by any other handles capable of viewing those
		/// elements. Doing so is still *incorrect*, though defined, and you are
		/// responsible for writing only within bounds when using `DomainMut`.
		///
		/// [`BitDomain`]: crate::domain::BitDomain
		/// [`BitDomainMut`]: crate::domain::BitDomainMut
		/// [`BitSlice`]: crate::slice::BitSlice
		/// [`T::Access`]: crate::store::BitStore::Access
		/// [`T::Alias`]: crate::store::BitStore::Alias
		#[derive(Debug)]
		pub enum $t <'a, T>
		where
			T: BitStore,
		{
			/// Indicates that a [`BitSlice`] is contained entirely in the
			/// interior indices of a single memory element.
			///
			/// [`BitSlice`]: crate::slice::BitSlice
			Enclave {
				/// The start index of the [`BitSlice`].
				///
				/// [`BitSlice`]: crate::slice::BitSlice
				head: BitIdx<T::Mem>,
				/// An aliased view of the element containing the [`BitSlice`].
				///
				/// This is necessary even on immutable views, because other
				/// views to the referent element may be permitted to modify it.
				///
				/// [`BitSlice`]: crate::slice::BitSlice
				elem: &'a T $(::$a)?,
				/// The end index of the [`BitSlice`].
				///
				/// [`BitSlice`]: crate::slice::BitSlice
				tail: BitTail<T::Mem>,
			},
			/// Indicates that a [`BitSlice`] region touches at least one edge
			/// index of any number of elements.
			///
			/// This contains two optional references to the aliased edges, and
			/// one reference to the unaliased middle. Each can be queried and
			/// used individually.
			///
			/// [`BitSlice`]: crate::slice::BitSlice
			Region {
				/// If the [`BitSlice`] started in the interior of its first
				/// element, this contains the starting index and the base
				/// address.
				///
				/// [`BitSlice`]: crate::slice::BitSlice
				head: Option<(BitIdx<T::Mem>, &'a T $(::$a)?)>,
				/// All fully-spanned, unaliased, elements.
				///
				/// This is marked as bare memory without any access
				/// protections, because it is statically impossible for any
				/// other handle to have write access to the region it covers.
				/// As such, a [`BitSlice`] that was marked as entirely aliased,
				/// but contains interior unaliased elements, can safely remove
				/// its aliasing protections.
				body: &'a $($m)? [T::Unalias],
				/// If the `BitSlice` ended in the interior of its last element,
				/// this contains the ending index and the last address.
				tail: Option<(&'a T $(::$a)?, BitTail<T::Mem>)>,
			}
		}

		impl<'a, T> $t <'a, T>
		where
			T: BitStore,
		{
			/// Attempts to view the domain as an enclave variant.
			///
			/// # Parameters
			///
			/// - `self`
			///
			/// # Returns
			///
			/// If `self` is the [`Enclave`] variant, this returns `Some` of the
			/// enclave fields, as a tuple. Otherwise, it returns `None`.
			///
			/// [`Enclave`]: Self::Enclave
			pub fn enclave(self) -> Option<(
				BitIdx<T::Mem>,
				&'a T $(::$a)?,
				BitTail<T::Mem>,
			)> {
				if let Self::Enclave { head, elem, tail } = self {
					Some((head, elem, tail))
				} else {
					None
				}
			}

			/// Attempts to view the domain as the region variant.
			///
			/// # Parameters
			///
			/// - `self`
			///
			/// # Returns
			///
			/// If `self` is the [`Region`] variant, this returns `Some` of the
			/// region fields, as a tuple. Otherwise, it returns `None`.
			///
			/// [`Region`]: Self::Region
			pub fn region(self) -> Option<(
				Option<(BitIdx<T::Mem>, &'a T $(::$a)?)>,
				&'a $($m)? [T::Unalias],
				Option<(&'a T $(::$a)?, BitTail<T::Mem>)>,
			)> {
				if let Self::Region { head, body, tail } = self {
					Some((head,body,tail))
				}
				else {
					None
				}
			}

			pub(crate) fn new<O>(slice: &'a $($m)? BitSlice<O, T>) -> Self
			where O: BitOrder {
				let bitspan = slice.as_bitspan();
				let head = bitspan.head();
				let elts = bitspan.elements();
				let tail = bitspan.tail();
				let bits = T::Mem::BITS;
				let base = bitspan.address().to_const() as *const _;
				match (head.value(), elts, tail.value()) {
					(_, 0, _) => Self::empty(),
					(0, _, t) if t == bits => Self::spanning(base, elts),
					(_, _, t) if t == bits => Self::partial_head(base, elts, head),
					(0, ..) => Self::partial_tail(base, elts, tail),
					(_, 1, _) => Self::minor(base, head, tail),
					_ => Self::major(base, elts, head, tail),
				}
			}

			fn empty() -> Self {
				Self::Region {
					head: None,
					body: & $($m)? [],
					tail: None,
				}
			}

			fn major(
				base: *const T $(::$a)?,
				elts: usize,
				head: BitIdx<T::Mem>,
				tail: BitTail<T::Mem>,
			) -> Self {
				let h = unsafe { &*base };
				let t = unsafe { &*base.add(elts - 1) };
				let body = domain!(slice $($m)? base.add(1), elts - 2);
				Self::Region {
					head: Some((head, h)),
					body,
					tail: Some((t, tail)),
				}
			}

			fn minor(
				addr: *const T $(::$a)?,
				head: BitIdx<T::Mem>,
				tail: BitTail<T::Mem>,
			) -> Self {
				Self::Enclave {
					head,
					elem: unsafe { &*addr },
					tail,
				}
			}

			fn partial_head(
				base: *const T $(::$a)?,
				elts: usize,
				head: BitIdx<T::Mem>,
			) -> Self {
				let h = unsafe { &*base };
				let body = domain!(slice $($m)? base.add(1), elts - 1);
				Self::Region {
					head: Some((head, h)),
					body,
					tail: None,
				}
			}

			fn partial_tail(
				base: *const T $(::$a)?,
				elts: usize,
				tail: BitTail<T::Mem>,
			) -> Self {
				let t = unsafe { &*base.add(elts - 1) };
				let body = domain!(slice $($m)? base, elts - 1);
				Self::Region {
					head: None,
					body,
					tail: Some((t, tail)),
				}
			}

			fn spanning(base: *const T $(::$a)?, elts: usize) -> Self {
				Self::Region {
					head: None,
					body: domain!(slice $($m)? base, elts),
					tail: None,
				}
			}
		}
	};

	(slice mut $base:expr, $elts:expr) => {
		unsafe { slice::from_raw_parts_mut($base as *const _ as *mut _, $elts) }
	};
	(slice $base:expr, $elts:expr) => {
		unsafe { slice::from_raw_parts($base as *const _, $elts) }
	};
}

domain!(Domain);
domain!(DomainMut => mut @ Access);

#[cfg(not(tarpaulin_include))]
impl<T> Clone for Domain<'_, T>
where T: BitStore
{
	fn clone(&self) -> Self {
		*self
	}
}

impl<'a, T> Iterator for Domain<'a, T>
where T: BitStore
{
	type Item = T::Mem;

	fn next(&mut self) -> Option<Self::Item> {
		match self {
			Self::Enclave { elem, .. } => {
				elem.load_value().pipe(Some).tap(|_| *self = Self::empty())
			},
			Self::Region { head, body, tail } => {
				if let Some((_, elem)) = *head {
					return elem.load_value().pipe(Some).tap(|_| *head = None);
				}
				if let Some((elem, rest)) = body.split_first() {
					*body = rest;
					return elem.load_value().into();
				}
				if let Some((elem, _)) = *tail {
					return elem.load_value().pipe(Some).tap(|_| *tail = None);
				}
				None
			},
		}
	}
}

impl<'a, T> DoubleEndedIterator for Domain<'a, T>
where T: BitStore
{
	fn next_back(&mut self) -> Option<Self::Item> {
		match self {
			Self::Enclave { elem, .. } => {
				elem.load_value().pipe(Some).tap(|_| *self = Self::empty())
			},
			Self::Region { head, body, tail } => {
				if let Some((elem, _)) = *tail {
					return elem.load_value().pipe(Some).tap(|_| *tail = None);
				}
				if let Some((elem, rest)) = body.split_last() {
					*body = rest;
					return elem.load_value().into();
				}
				if let Some((_, elem)) = *head {
					return elem.load_value().pipe(Some).tap(|_| *head = None);
				}
				None
			},
		}
	}
}

impl<T> ExactSizeIterator for Domain<'_, T>
where T: BitStore
{
	fn len(&self) -> usize {
		match self {
			Self::Enclave { .. } => 1,
			Self::Region { head, body, tail } => {
				head.is_some() as usize + body.len() + tail.is_some() as usize
			},
		}
	}
}

impl<T> core::iter::FusedIterator for Domain<'_, T> where T: BitStore
{
}

impl<T> Copy for Domain<'_, T> where T: BitStore
{
}

macro_rules! fmt {
	($($f:ty => $fwd:ident),+ $(,)?) => { $(
		impl<T> $f for Domain<'_, T>
		where T: BitStore
		{
			fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
				fmt.debug_list()
					.entries(self.into_iter().map(FmtForward::$fwd))
					.finish()
			}
		}
	)+ };
}

fmt!(
	Binary => fmt_binary,
	LowerHex => fmt_lower_hex,
	Octal => fmt_octal,
	UpperHex => fmt_upper_hex,
);

#[cfg(test)]
mod tests {
	use crate::prelude::*;

	#[test]
	fn domain_iter() {
		let data = [1u32, 2, 3];
		let bits = &data.view_bits::<LocalBits>()[4 .. 92];

		for (iter, elem) in bits.domain().rev().zip([3, 2, 1].iter().copied()) {
			assert_eq!(iter, elem);
		}
	}
}
