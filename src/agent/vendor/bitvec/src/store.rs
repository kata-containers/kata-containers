/*! Memory modeling.

This module provides the [`BitStore`] trait, which contains all of the logic
required to perform memory accesses from a data structure handle.

# `bitvec` Memory Model

`bitvec` considers all memory within [`BitSlice`] regions as if it were composed
of discrete bits, each divisible and indipendent from its neighbors, just as the
Rust memory model considers elements `T` in a slice `[T]`. Much as ordinary byte
slices `[u8]` provide an API where each byte is distinct and independent from
its neighbors, but the underlying processor silicon clusters them in words and
cachelines, both the processor silicon *and* the Rust compiler require that bits
in a `BitSlice` be grouped into memory elements, and collectively subjected to
aliasing rules within their batch.

`bitvec` manages this through the [`BitStore`] trait. It is implemented on three
type families available from the Rust standard libraries:

- [unsigned integers]
- [atomic] unsigned integers
- [`Cell`] wrappers of unsigned integers

`bitvec` receives a memory region typed with one of these three families and
wraps it in one of its data structures based on [`BitSlice`]. The target
processor is responsible for handling any contention between memory elements;
this is irrelevant to the `bitvec` model. `bitvec` is solely responsible for
proving to the Rust compiler that all memory accesses through its types are
correctly managed according to the `&`/`&mut` shared/exclusion reference model,
and the [`UnsafeCell`] shared-mutation model.

Through [`BitStore`], `bitvec` is able to demonstrate that `&mut BitSlice`
references to a region of *bits* have no other `BitSlice` references capable of
viewing those bits. However, `&mut BitSlice` references *may* have other
`&BitSlice` references capable of viewing the memory elements at locations that
it modifies, and the Rust compiler considers it undefined behavior for such
conditions to allow racing writes and reads without synchronization.

As such, [`BitStore`] provides a closed type-system graph that the [`BitSlice`]
API uses to mark events that can induce aliases to memory locations. When a
`&mut BitSlice<_, T>` typed with an ordinary unsigned integer use any of the
APIs that call [`.split_at_mut()`], it transitions to
`&mut BitSlice<_, T::Alias>`. The [`::Alias`] associated type is always a type
that manages aliasing references to a single memory location: either an [atomic]
unsigned integer `T` or a [`Cell`] of the unsigned integer `T`. The Rust
standard library guarantees that these types will behave correctly when multiple
references to a single location attempt to read from and write to it.

The [atomic] and [`Cell`] types stay as themselves when [`BitSlice`] introduces
aliasing conditions, as they are already alias-aware.

Lastly, the `bitvec` memory description model as implemented in the [`domain`]
module is able to perform the inverse transition: where it can demonstrate a
static awareness that the `&`/`&mut` exclusion rules are satisfied for a
particular element slice `[T]`, it may apply the [`::Unalias`] marker to undo
any `::Alias`ing, and present a type that has no more aliasing protection than
that with which the memory region was initially declared.

Namely, this means that the [atomic] and [`Cell`] wrappers will never be removed
from a region that had them before it was given to `bitvec`, while a region of
ordinary integers may regain the ability to be viewed without synchrony guards
if `bitvec` can prove safety in the [`domain`] module.

In order to retain `bitvec`’s promise that an `&mut BitSlice<_, T>` has the sole
right of observation for all bits in its region, the unsigned integers alias to
a crate-internal wrapper over the alias-capable standard-library types. This
wrapper forbids mutation through shared references, so two [`BitSlice`]
references that alias a memory location, but do not overlap in bits, may not be
coërced to interfere with each other.

[atomic]: core::sync::atomic
[unsigned integers]: core::primitive
[`BitSlice`]: crate::slice::BitSlice
[`BitStore`]: crate::store::BitStore
[`Cell`]: core::cell::Cell
[`UnsafeCell`]: core::cell::UnsafeCell
[`domain`]: crate::domain
[`::Alias`]: crate::store::BitStore::Alias
[`::Unalias`]: crate::store::BitStore::Unalias
[`.split_at_mut()`]: crate::slice::BitSlice::split_at_mut
!*/

use crate::{
	access::*,
	index::{
		BitIdx,
		BitMask,
	},
	mem::{
		self,
		BitRegister,
	},
	order::BitOrder,
};

use core::{
	cell::Cell,
	fmt::Debug,
};

use tap::pipe::Pipe;

/** Common interface for memory regions.

This trait is used to describe how [`BitSlice`] regions interact with the memory
bus when reading to or writing from locations. It manages the behavior required
when locations are contended for write permissions by multiple handles, and
ensures that Rust’s `&`/`&mut` shared/exclusion system, as well as its
[`UnsafeCell`] shared-mutation system, are upheld for individual bits as well as
for the memory operations that power the slice.

This trait is publicly implemented on the unsigned integers that implement
[`BitRegister`], their [`Cell`] wrappers, and (if present) their [atomic]
variants. You may freely construct [`BitSlice`] regions over elements or slices
of any of these types.

Shared [`BitSlice`] references (`&BitSlice<_, T: BitStore>`) permit multiple
handles to view the bits they describe. When `T` is a [`Cell`] or [atom], these
handles may use the methods [`.set_aliased()`] and [`.set_aliased_unchecked()`]
to modify memory; when `T` is an ordinary integer, they may not.

Exclusive [`BitSlice`] references (`&mut BitSlice<_, T: BitStore>`) do not allow
any other handle to view the bits they describe. However, other handles may view
the **memory locations** containing their bits! When `T` is a [`Cell`] or
[atom], no special behavior occurs. When `T` is an ordinary integer, [`bitvec`]
detects the creation of multiple `&mut BitSlice<_, T>` handles that do not alias
bits but *do* alias memory, and enforces that these handles use `Cell` or atomic
behavior to access the underlying memory, even though individual bits in the
slices are not contended.

# Integer Width Restricitons

Currently, [`bitvec`] is only tested on 32- and 64- bit architectures. This
means that `u8`, `u16`, `u32`, and `usize` unconditionally implement `BitStore`,
but `u64` will only do so on 64-bit targets. This is a necessary restriction of
`bitvec` internals. Please comment on [Issue #76] if this affects you.

[Issue #76]: https://github.com/myrrlyn/bitvec/issues/76
[atom]: core::sync::atomic
[atomic]: core::sync::atomic
[`BitSlice`]: crate::slice::BitSlice
[`BitRegister`]: crate::mem::BitRegister
[`Cell`]: core::cell::Cell
[`UnsafeCell`]: core::cell::UnsafeCell
[`bitvec`]: crate
[`.set_aliased()`]: crate::slice::BitSlice::set_aliased
[`.set_aliased_unchecked()`]: crate::slice::BitSlice::set_aliased_unchecked
**/
pub trait BitStore: 'static + seal::Sealed + Debug {
	/// The register type used in the slice region underlying a [`BitSlice`]
	/// handle. It is always an unsigned integer.
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	type Mem: BitRegister + BitStore<Mem = Self::Mem>;
	/// A type that selects appropriate load/store instructions used for
	/// accessing the memory bus. It determines what instructions are used when
	/// moving a `Self::Mem` value between the processor and the memory system.
	type Access: BitAccess<Item = Self::Mem> + BitStore<Mem = Self::Mem>;
	/// A sibling `BitStore` implementor. It is used when a [`BitSlice`]
	/// introduces multiple handles that view the same memory location, and at
	/// least one of them has write permission to it.
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	type Alias: BitStore<Mem = Self::Mem>;
	/// The inverse of `Alias`. It is used when a [`BitSlice`] removes the
	/// conditions that required a `T -> T::Alias` transition.
	///
	/// [`BitSlice`]: crate::slice::BitSlice
	type Unalias: BitStore<Mem = Self::Mem>;

	/// Loads a value out of the memory system according to the `::Access`
	/// rules.
	fn load_value(&self) -> Self::Mem;

	/// Stores a value into the memory system according to the `::Access` rules.
	fn store_value(&mut self, value: Self::Mem);

	/// Reads a single bit out of the memory system according to the `::Access`
	/// rules. This is lifted from [`BitAccess`] so that it can be used
	/// elsewhere without additional casts.
	///
	/// # Type Parameters
	///
	/// - `O`: The ordering of bits within `Self::Mem` to use for looking up the
	///   bit at `index`.
	///
	/// # Parameters
	///
	/// - `&self`
	/// - `index`: The semantic index of the bit in `*self` to read.
	///
	/// # Returns
	///
	/// The value of the bit in `*self` at `index`.
	///
	/// [`BitAccess`]: crate::access::BitAccess
	fn get_bit<O>(&self, index: BitIdx<Self::Mem>) -> bool
	where O: BitOrder {
		self.load_value()
			.pipe(BitMask::new)
			.test(index.select::<O>())
	}

	/// Require that all implementors are aligned to their width.
	#[doc(hidden)]
	const __ALIGNED_TO_SIZE: [(); 0];

	/// Require that the `::Alias` associated type has the same width and
	/// alignment as `Self`.
	#[doc(hidden)]
	const __ALIAS_WIDTH: [(); 0];
}

/// Batch implementation of `BitStore` on integers, safety wrappers, and `Cell`s
macro_rules! store {
	( $($base:ty => $safe:ty),+ $(,)? ) => { $(
		impl BitStore for $base {
			type Mem = Self;
			/// The unsigned integers will only be `BitStore` type parameters
			/// for handles to unaliased memory, following the normal Rust
			/// reference rules.
			type Access = Cell<$base>;
			type Alias = $safe;
			type Unalias = Self;

			fn load_value(&self) -> Self::Mem {
				*self
			}

			fn store_value(&mut self, value: Self::Mem) {
				*self = value;
			}

			#[doc(hidden)]
			const __ALIGNED_TO_SIZE: [(); 0]
				= [(); mem::aligned_to_size::<Self>()];

			#[doc(hidden)]
			const __ALIAS_WIDTH: [(); 0]
				= [(); mem::cmp_layout::<Self, Self::Alias>()];
		}

		/// This type is only ever produced by calling [`.split_at_mut()`] on
		/// [`BitSlice<_, T>`] where `T` is an unsigned integer. It cannot be
		/// constructed as a base data source.
		///
		/// [`BitSlice<_, T>`]: crate::slice::BitSlice
		/// [`.split_at_mut()`]: crate::slice::BitSlice::split_at_mut
		impl BitStore for $safe {
			type Mem = $base;
			type Access = <Self as BitSafe>::Rad;
			type Alias = Self;
			type Unalias = $base;

			#[inline(always)]
			fn load_value(&self) -> Self::Mem {
				self.load()
			}

			#[inline(always)]
			fn store_value(&mut self, value: Self::Mem) {
				self.store(value);
			}

			#[doc(hidden)]
			const __ALIGNED_TO_SIZE: [(); 0]
				= [(); mem::aligned_to_size::<Self>()];

			#[doc(hidden)]
			const __ALIAS_WIDTH: [(); 0]
				= [(); mem::cmp_layout::<Self, Self::Unalias>()];
		}

		impl BitStore for Cell<$base> {
			type Mem = $base;
			type Access = Self;
			type Alias = Self;
			type Unalias = Self;

			#[inline(always)]
			fn load_value(&self) -> Self::Mem {
				self.get()
			}

			#[inline(always)]
			fn store_value(&mut self, value: Self::Mem) {
				self.set(value);
			}

			#[doc(hidden)]
			const __ALIGNED_TO_SIZE: [(); 0]
				= [(); mem::aligned_to_size::<Self>()];

			#[doc(hidden)]
			const __ALIAS_WIDTH: [(); 0] = [];
		}

		impl seal::Sealed for $base {}
		impl seal::Sealed for $safe {}
		impl seal::Sealed for Cell<$base> {}
	)+ };
}

store! {
	u8 => BitSafeU8,
	u16 => BitSafeU16,
	u32 => BitSafeU32,
}

#[cfg(target_pointer_width = "64")]
store!(u64 => BitSafeU64);

store!(usize => BitSafeUsize);

macro_rules! atomic_store {
	($($w:tt , $base:ty => $atom:ident);+ $(;)?) => { $(
		radium::if_atomic!(if atomic($w) {
			use core::sync::atomic::$atom;

			impl BitStore for $atom {
				type Mem = $base;
				type Access = Self;
				type Alias = Self;
				type Unalias = Self;

				fn load_value(&self) -> Self::Mem {
					self.load(core::sync::atomic::Ordering::Relaxed)
				}

				fn store_value(&mut self, value: Self::Mem) {
					self.store(value, core::sync::atomic::Ordering::Relaxed);
				}

				#[doc(hidden)]
				const __ALIGNED_TO_SIZE: [(); 0]
					= [(); mem::aligned_to_size::<Self>()];

				#[doc(hidden)]
				const __ALIAS_WIDTH: [(); 0] = [];
			}

			impl seal::Sealed for $atom {}
		});
	)+ };
}

atomic_store! {
	8, u8 => AtomicU8;
	16, u16 => AtomicU16;
	32, u32 => AtomicU32;
}

#[cfg(target_pointer_width = "64")]
atomic_store!(64, u64 => AtomicU64);

atomic_store!(size, usize => AtomicUsize);

#[cfg(not(any(target_pointer_width = "32", target_pointer_width = "64")))]
compile_fail!(concat!(
	"This architecture is currently not supported. File an issue at ",
	env!("CARGO_PKG_REPOSITORY")
));

/// Enclose the `Sealed` trait against client use.
mod seal {
	/// Marker trait to seal `BitStore` against downstream implementation.
	///
	/// This trait is public in the module, so that other modules in the crate
	/// can use it, but so long as it is not exported by the crate root and this
	/// module is private, this trait effectively forbids downstream
	/// implementation of the `BitStore` trait.
	#[doc(hidden)]
	pub trait Sealed {}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::prelude::*;
	use core::cell::Cell;
	use static_assertions::*;

	#[test]
	fn load_store() {
		let mut word = 0usize;

		word.store_value(39usize);
		assert_eq!(word.load_value(), 39usize);

		let safe: &mut BitSafeUsize =
			unsafe { &mut *(&mut word as *mut _ as *mut _) };
		safe.store_value(57usize);
		assert_eq!(safe.load_value(), 57);

		let mut cell = Cell::new(0usize);
		cell.store_value(39);
		assert_eq!(cell.load_value(), 39);

		radium::if_atomic!(if atomic(size) {
			let mut atom = AtomicUsize::new(0);
			atom.store_value(39);
			assert_eq!(atom.load_value(), 39usize);
		});
	}

	/// Unaliased `BitSlice`s are universally threadsafe, because they satisfy
	/// Rust’s unysnchronized mutation rules.
	#[test]
	fn unaliased_send_sync() {
		assert_impl_all!(BitSlice<LocalBits, u8>: Send, Sync);
		assert_impl_all!(BitSlice<LocalBits, u16>: Send, Sync);
		assert_impl_all!(BitSlice<LocalBits, u32>: Send, Sync);
		assert_impl_all!(BitSlice<LocalBits, usize>: Send, Sync);

		#[cfg(target_pointer_width = "64")]
		assert_impl_all!(BitSlice<LocalBits, u64>: Send, Sync);
	}

	#[test]
	fn cell_unsend_unsync() {
		assert_not_impl_any!(BitSlice<LocalBits, Cell<u8>>: Send, Sync);
		assert_not_impl_any!(BitSlice<LocalBits, Cell<u16>>: Send, Sync);
		assert_not_impl_any!(BitSlice<LocalBits, Cell<u32>>: Send, Sync);
		assert_not_impl_any!(BitSlice<LocalBits, Cell<usize>>: Send, Sync);
		#[cfg(target_pointer_width = "64")]
		assert_not_impl_any!(BitSlice<LocalBits, Cell<u64>>: Send, Sync);
	}

	/// In non-atomic builds, aliased `BitSlice`s become universally
	/// thread-unsafe. An `&mut BitSlice` is an `&Cell`, and `&Cell` cannot be
	/// sent across threads.
	///
	/// This test cannot be meaningfully expressed in atomic builds, because the
	/// atomiticy of a `BitSafeUN` type is target-specific, and expressed in
	/// `radium` rather than in `bitvec`.
	#[test]
	#[cfg(not(feature = "atomic"))]
	fn aliased_nonatomic_unsend_unsync() {
		use crate::access::*;

		assert_not_impl_any!(BitSlice<LocalBits, BitSafeU8>: Send, Sync);
		assert_not_impl_any!(BitSlice<LocalBits, BitSafeU16>: Send, Sync);
		assert_not_impl_any!(BitSlice<LocalBits, BitSafeU32>: Send, Sync);
		assert_not_impl_any!(BitSlice<LocalBits, BitSafeUsize>: Send, Sync);

		#[cfg(target_pointer_width = "64")]
		assert_not_impl_any!(BitSlice<LocalBits, BitSafeU64>: Send, Sync);
	}
}
