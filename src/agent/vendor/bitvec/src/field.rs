/*! Batched load/store access to bitfields.

This module provides load/store access to bitfield regions that emulates the
ordinary memory bus. This functionality enables any [`BitSlice`] span to be used
as a memory region, and provides the basis of a library-level analogue to the
bitfield language feature found in C and C++. Additionally, orderings that have
contiguous positions can transfer more than one bit in an operation, allowing a
performance acceleration over sequential bit-by-bit traversal.

The [`BitField`] trait is open for implementation. Rust’s implementation rules
currently disallow a crate to implement a foreign trait on a foreign type, even
when parameterized over a local type. If you need such a `BitField`
implementation with a new `BitOrder` type, please file an issue.

# Batched Behavior

The first purpose of [`BitField`] is to provide access to [`BitSlice`] regions
as if they were an ordinary memory location. However, this can be done through
the `BitSlice` sequential API. The second purpose of this trait is to accelerate
such access by using the parallel memory bus to transfer more than one bit at a
time when the region permits it. As such, implementors should provide a transfer
behavior based on shift/mask operations wherever possible, for as wide a span in
a memory element as possible.

# Register Bit Order Preservation

As a default assumption, each element of the underlying memory region used to
store part of a value should not reörder the bit-pattern of that value. While
the [`BitOrder`] argument is used to determine which segments of the memory
register are live for the purposes of this transfer, it should not be used to
map each individual bit of the transferred value to a corresponding bit of the
storage element. As an example, the [`Lsb0`] and [`Msb0`] implementations both
store the value `12u8` in memory as a four-bit span with its two
more-significant bits set and its two less-significant bits cleared; the
difference is only in *which* bits of an element are used to store the span.

# Endianness

The `_le` and `_be` methods of [`BitField`] refer to the order in which
successive `T` elements of a storage region are assigned numeric significance
during a transfer. Within any particular `T` element, the ordering of its memory
is not governed by the `BitField` trait.

The provided [`BitOrder`] implementors [`Lsb0`] and [`Msb0`] use the local
machine’s byte ordering, and do not reörder bytes during transfer.

## `_le` Methods

When storing a value `M` into a sequence of memory elements `T`, [`store_le`]
breaks `M` into chunks from the least significant edge. The least significant
chunk is placed in the lowest-addressed element `T`, then the next more
significant chunk is placed in the successive address, until the most
significant chunk of the value `M` is placed in the highest address of a
location `T`.

When loading a value `M` out of a sequence of memory elements `T`, [`load_le`]
uses the same chunking behavior: the lowest-addressed `T` contains the least
significant chunk of the returned `M`, then each successive address contains a
more significant chunk, until the highest address contains the most significant.

The [`BitOrder`] implementation governs *where* in each `T` location a fragment
of `M` is stored.

Let us store 8 bits into memory, over an element boundary, using both [`Lsb0`]
and [`Msb0`] orderings:

```rust
use bitvec::prelude::*;

let val: u8 = 0b11010_011;
//              STUVW XYZ
let mut store = [0u8; 2];

store.view_bits_mut::<Lsb0>()
  [5 .. 13]
  .store_le(val);
assert_eq!(
  store,
  [0b011_00000, 0b000_11010],
//   XYZ               STUVW
# "[{:08b}, {:08b}]",
# store[0],
# store[1],
);
store = [0u8; 2];

store.view_bits_mut::<Msb0>()
  [5 .. 13]
  .store_le(val);
assert_eq!(
  store,
  [0b00000_011, 0b11010_000],
//         XYZ    STUVW
# "[{:08b}, {:08b}]",
# store[0],
# store[1],
);
```

In both cases, the lower three bits of `val` were placed into the element at the
lower memory address. The choice of [`Lsb0`] vs [`Msb0`] changed *which* three
bits in the element were considered to be indexed by `5 .. 8`, but [`store_le`]
always placed the least three bits of `val`, *in ordinary register order*, into
element `[0]`. Similarly, the higher five bits of `val` were placed into element
`[1]`; `Lsb0` and `Msb0` selected *which* five bits in the element were indexed
by `8 .. 13`, and the bits retained their register order.

## `_be` Methods

When storing a value `M` into a sequence of memory elements `T`, [`store_be`]
breaks `M` into chunks from the most significant edge. The most significant
chunk is placed in the lowest-addressed element `T`, then the next less
significant chunk is placed in the successive address, until the least
significant chunk of the value `M` is placed in the highest address of a
location `T`.

When loading a value `M` out of a sequence of memory elements `T`, [`load_be`]
uses the same chunking behavior: the lowest-addressed `T` contains the most
significant chunk of the returned `M`, then each successive address contains a
less significant chunk, until the highest address contains the least
significant.

The [`BitOrder`] implementation governs *where* in each `T` location a fragment
of `M` is stored.

Let us store 8 bits into memory, over an element boundary, using both [`Lsb0`]
and [`Msb0`] orderings:

```rust
use bitvec::prelude::*;

let val: u8 = 0b110_10011;
//              STU VWXYZ
let mut store = [0u8; 2];

store.view_bits_mut::<Lsb0>()
  [5 .. 13]
  .store_be(val);
assert_eq!(
  store,
  [0b110_00000, 0b000_10011],
//   STU              VWXYZ
# "[{:08b}, {:08b}]",
# store[0],
# store[1],
);
store = [0u8; 2];

store.view_bits_mut::<Msb0>()
  [5 .. 13]
  .store_be(val);
assert_eq!(
  store,
  [0b00000_110, 0b10011_000],
//         STU    VWXYZ
# "[{:08b}, {:08b}]",
# store[0],
# store[1],
);
```

In both cases, the higher three bits of `val` were placed into the element at
the lower memory address. The choice of [`Lsb0`] vs [`Msb0`] changed *which*
three bits in the element were considered to be indexed by `5 .. 8`, but
[`store_be`] always placed the greatest three bits of `val`, *in ordinary*
*register order*, into element `[0]`. Similarly, the lower five bits of `val`
were placed into element `[1]`; `Lsb0` and `Msb0` selected *which* five bits in
the element were indexed by `8 .. 13`, and the bits retained their register
order.

# `M` and `T` Relationships

`BitField` permits any type of (unsigned) integer `M` to be stored into or
loaded from a bit-slice region with any storage type `T`. While the examples
used `u8` for both, for brevity of writing out values, `BitField` will still
operate correctly for any other combination of types.

`Bitfield` implementations use the processor’s own concept of integer registers
to operate. As such, the byte-wise memory access patterns for types wider than
`u8` depends on your processor’s byte-endianness, as well as which `BitField`
method and which `BitOrder` implementation you are using.

`BitField` only operates within processor registers; traffic of `T` elements
between the memory bank and the processor register is controlled entirely by the
processor.

If you do not want to introduce the processor’s byte-endianness as a variable
that affects the in-memory representation of stored integers, stick to
`BitSlice<_, u8>` as the bit-field driver. `BitSlice<Msb0, u8>` will fill memory
in a way that matches a debugger or other memory inspections.

[`BitField`]: crate::field::BitField
[`BitOrder`]: crate::order::BitOrder
[`BitSlice`]: crate::slice::BitSlice
[`Lsb0`]: crate::order::Lsb0
[`Msb0`]: crate::order::Msb0
[`load_be`]: crate::field::BitField::load_be
[`load_le`]: crate::field::BitField::load_le
[`store_be`]: crate::field::BitField::store_be
[`store_le`]: crate::field::BitField::store_le
!*/

use crate::{
	access::BitAccess,
	array::BitArray,
	domain::{
		Domain,
		DomainMut,
	},
	index::BitMask,
	mem::BitMemory,
	order::{
		BitOrder,
		Lsb0,
		Msb0,
	},
	slice::BitSlice,
	store::BitStore,
	view::BitView,
};

use core::{
	mem,
	ptr,
};

use tap::pipe::Pipe;

#[cfg(feature = "alloc")]
use crate::{
	boxed::BitBox,
	vec::BitVec,
};

/** Performs C-style bitfield access through a [`BitSlice`].

This trait transfers data between a [`BitSlice`] region and a local integer. The
trait functions always place the live bits of the value against the least
significant bit edge of the local integer (the return value of the load methods,
and the argument value of the store methods).

Methods should be called as `bits[start .. end].load_or_store()`, where the
range subslice selects no more than the [`M::BITS`] element width.

# Target-Specific Behavior

When you are using this trait to manage memory that never leaves your machine,
you can use the [`load`] and [`store`] methods. However, if you are using this
trait to operate on a de/serialization buffer, where the exact bit pattern in
memory is important to your work and/or you need to be aware of the processor
byte endianness, you must not use these methods.

Instead, use [`load_le`], [`load_be`], [`store_le`], or[`store_be`] directly.

The un-suffixed methods choose their implementation based on the target
processor byte endianness; the suffixed methods have a consistent and fixed
behavior.

# Element- and Bit- Ordering Combinations

The `_le` and `_be` method suffices refer to the significance of successive
elements `T` in memory, while the `BitOrder` trait refers to the order that bits
within a single element `T` are traversed. The `BitField` methods and the
`BitOrder` implementors are ***not*** related.

When a load or store operation is contained in only one memory element, then the
`_le` and `_be` methods have the same behavior. They differ when the operation
must touch more than one element.

The module documentation contains a more detailed explanation, and examples, for
this behavior.

[`BitSlice`]: crate::slice::BitSlice
[`M::BITS`]: crate::mem::BitMemory::BITS
[`load`]: Self::load
[`load_be`]: Self::load_be
[`load_le`]: Self::load_le
[`store`]: Self::store
[`store_be`]: Self::store_be
[`store_le`]: Self::store_le
**/
pub trait BitField {
	/// Loads the bits in the `self` region into a local value.
	///
	/// This can load into any of the unsigned integers which implement
	/// [`BitMemory`]. Any further transformation must be done by the user.
	///
	/// # Target-Specific Behavior
	///
	/// **THIS FUNCTION CHANGES BEHAVIOR FOR DIFFERENT TARGETS.**
	///
	/// The default implementation of this function calls [`load_le`] on
	/// little-endian byte-ordered CPUs, and [`load_be`] on big-endian
	/// byte-ordered CPUs.
	///
	/// If you are using this function from a region that crosses multiple
	/// elements in memory, be aware that it will behave differently on
	/// big-endian and little-endian target architectures.
	///
	/// # Parameters
	///
	/// - `&self`: A read reference to some bits in memory. This slice must be
	///   trimmed to have a width no more than the [`M::BITS`] width of the type
	///   being loaded. This can be accomplished with range indexing on a larger
	///   slice.
	///
	/// # Returns
	///
	/// A value `M` whose least [`self.len()`] significant bits are filled with
	/// the bits of `self`.
	///
	/// # Panics
	///
	/// This method is encouraged to panic if `self` is empty, or wider than a
	/// single element `M`.
	///
	/// [`BitMemory`]: crate::mem::BitMemory
	/// [`M::BITS`]: crate::mem::BitMemory::BITS
	/// [`load_be`]: Self::load_be
	/// [`load_le`]: Self::load_le
	/// [`self.len()`]: crate::slice::BitSlice::len
	fn load<M>(&self) -> M
	where M: BitMemory {
		#[cfg(target_endian = "little")]
		return self.load_le::<M>();

		#[cfg(target_endian = "big")]
		return self.load_be::<M>();
	}

	/// Stores a sequence of bits from the user into the domain of `self`.
	///
	/// This can store any of the unsigned integers which implement
	/// [`BitMemory`]. Any other types must first be transformed by the user.
	///
	/// # Target-Specific Behavior
	///
	/// **THIS FUNCTION CHANGES BEHAVIOR FOR DIFFERENT TARGETS.**
	///
	/// The default implementation of this function calls [`store_le`] on
	/// little-endian byte-ordered CPUs, and [`store_be`] on big-endian
	/// byte-ordered CPUs.
	///
	/// If you are using this function to store into a region that crosses
	/// multiple elements in memory, be aware that it will behave differently on
	/// big-endian and little-endian target architectures.
	///
	/// # Parameters
	///
	/// - `&mut self`: A write reference to some bits in memory. This slice must
	///   be trimmed to have a width no more than the [`M::BITS`] width of the
	///   type being stored. This can be accomplished with range indexing on a
	///   larger slice.
	/// - `value`: A value, whose [`self.len()`] least significant bits will be
	///   stored into `self`.
	///
	/// # Behavior
	///
	/// The [`self.len()`] least significant bits of `value` are written into
	/// the domain of `self`.
	///
	/// # Panics
	///
	/// This method is encouraged to panic if `self` is empty, or wider than a
	/// single element `M`.
	///
	/// [`BitMemory`]: crate::mem::BitMemory
	/// [`M::BITS`]: crate::mem::BitMemory::BITS
	/// [`self.len()`]: crate::slice::BitSlice::len
	/// [`store_be`]: Self::store_be
	/// [`store_le`]: Self::store_le
	fn store<M>(&mut self, value: M)
	where M: BitMemory {
		#[cfg(target_endian = "little")]
		self.store_le(value);

		#[cfg(target_endian = "big")]
		self.store_be(value);
	}

	/// Loads from `self`, using little-endian element `T` ordering.
	///
	/// This function interprets a multi-element slice as having its least
	/// significant chunk in the low memory address, and its most significant
	/// chunk in the high memory address. Each element `T` is still interpreted
	/// from individual bytes according to the local CPU ordering.
	///
	/// # Parameters
	///
	/// - `&self`: A read reference to some bits in memory. This slice must be
	///   trimmed to have a width no more than the [`M::BITS`] width of the type
	///   being loaded. This can be accomplished with range indexing on a larger
	///   slice.
	///
	/// # Returns
	///
	/// A value `M` whose least [`self.len()`] significant bits are filled with
	/// the bits of `self`. If `self` spans multiple elements `T`, then the
	/// lowest-address `T` is interpreted as containing the least significant
	/// bits of the return value `M`, and the highest-address `T` is interpreted
	/// as containing its most significant bits.
	///
	/// # Panics
	///
	/// This method is encouraged to panic if `self` is empty, or wider than a
	/// single element `M`.
	///
	/// # Examples
	///
	/// This example shows how a value is segmented across multiple storage
	/// elements:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut data = [0u8; 3];
	/// data.view_bits_mut::<Msb0>()
	///   [5 .. 17]
	///   .store_le(0b0000_1_1011_1000_110u16);
	/// //                 O PQRS TUVW XYZ
	///
	/// assert_eq!(data, [
	///   0b00000_110, 0b1011_1000, 0b1_0000000
	/// //        XYZ    PQRS TUVW    O
	/// ]);
	///
	/// let val = data.view_bits::<Msb0>()
	///   [5 .. 17]
	///   .load_le::<u16>();
	/// assert_eq!(
	///   val,
	///   0b0000_1_1011_1000_110,
	/// //       O PQRS TUVW XYZ
	/// );
	/// ```
	///
	/// And this example shows how the same memory region will be read by
	/// different `BitOrder` implementors:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// // Bit pos:   14                                     19  16
	/// // Lsb0:     ─┤                                       ├──┤
	/// let arr = [0b0100_0000_0000_0011u16, 0b0001_0000_0000_1110u16];
	/// // Msb0:                      ├─       ├──┤
	/// // Bit pos:                  14       16  19
	///
	/// assert_eq!(
	///   arr.view_bits::<Lsb0>()[14 .. 20].load_le::<u8>(),
	///   0b111001,
	/// );
	/// assert_eq!(
	///   arr.view_bits::<Msb0>()[14 .. 20].load_le::<u8>(),
	///   0b000111,
	/// );
	/// ```
	///
	/// [`M::BITS`]: crate::mem::BitMemory::BITS
	/// [`self.len()`]: crate::slice::BitSlice::len
	fn load_le<M>(&self) -> M
	where M: BitMemory;

	/// Loads from `self`, using big-endian element `T` ordering.
	///
	/// This function interprets a multi-element slice as having its most
	/// significant chunk in the low memory address, and its least significant
	/// chunk in the high memory address. Each element `T` is still interpreted
	/// from individual bytes according to the local CPU ordering.
	///
	/// # Parameters
	///
	/// - `&self`: A read reference to some bits in memory. This slice must be
	///   trimmed to have a width no more than the [`M::BITS`] width of the type
	///   being loaded. This can be accomplished with range indexing on a larger
	///   slice.
	///
	/// # Returns
	///
	/// A value `M` whose least [`self.len()`] significant bits are filled with
	/// the bits of `self`. If `self` spans multiple elements `T`, then the
	/// lowest-address `T` is interpreted as containing the most significant
	/// bits of the return value `M`, and the highest-address `T` is interpreted
	/// as containing its least significant bits.
	///
	/// # Panics
	///
	/// This method is encouraged to panic if `self` is empty, or wider than a
	/// single element `M`.
	///
	/// # Examples
	///
	/// This example shows how a value is segmented across multiple storage
	/// elements:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut data = [0u8; 3];
	/// data.view_bits_mut::<Msb0>()
	///   [5 .. 17]
	///   .store_be(0b0000_110_1000_1011_1u16);
	/// //                 OPQ RSTU VWXY Z
	///
	/// assert_eq!(data, [
	///   0b00000_110, 0b1000_1011, 0b1_0000000
	/// //        OPQ    RSTU VWXY    Z
	/// ]);
	///
	/// let val = data.view_bits::<Msb0>()
	///   [5 .. 17]
	///   .load_be::<u16>();
	/// assert_eq!(
	///   val,
	///   0b0000_110_1000_1011_1,
	/// //       OPQ RSTU VWXY Z
	/// # "{:012b}",
	/// # val,
	/// );
	/// ```
	///
	/// And this example shows how the same memory region will be read by
	/// different `BitOrder` implementations:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	/// // Bit pos:   14                                     19  16
	/// // Lsb0:     ─┤                                       ├──┤
	/// let arr = [0b0100_0000_0000_0011u16, 0b0001_0000_0000_1110u16];
	/// // Msb0:                      ├─       ├──┤
	/// // Bit pos:                  14       15  19
	///
	/// assert_eq!(
	///   arr.view_bits::<Lsb0>()[14 .. 20].load_be::<u8>(),
	///   0b011110,
	/// );
	/// assert_eq!(
	///   arr.view_bits::<Msb0>()[14 .. 20].load_be::<u8>(),
	///   0b110001,
	/// );
	/// ```
	///
	/// [`M::BITS`]: crate::mem::BitMemory::BITS
	/// [`self.len()`]: crate::slice::BitSlice::len
	fn load_be<M>(&self) -> M
	where M: BitMemory;

	/// Stores into `self`, using little-endian element ordering.
	///
	/// This function interprets a multi-element slice as having its least
	/// significant chunk in the low memory address, and its most significant
	/// chunk in the high memory address. Each element `T` is still interpreted
	/// from individual bytes according to the local CPU ordering.
	///
	/// # Parameters
	///
	/// - `&mut self`: A write reference to some bits in memory. This slice must
	///   be trimmed to have a width no more than the [`M::BITS`] width of the
	///   type being stored. This can be accomplished with range indexing on a
	///   larger slice.
	/// - `value`: A value, whose [`self.len()`] least significant bits will be
	///   stored into `self`.
	///
	/// # Behavior
	///
	/// The [`self.len()`] least significant bits of `value` are written into
	/// the domain of `self`. If `self` spans multiple elements `T`, then the
	/// lowest-address `T` is interpreted as containing the least significant
	/// bits of the `M` return value, and the highest-address `T` is interpreted
	/// as containing its most significant bits.
	///
	/// # Panics
	///
	/// This method is encouraged to panic if `self` is empty, or wider than a
	/// single element `M`.
	///
	/// # Examples
	///
	/// This example shows how a value is segmented across multiple storage
	/// elements:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut data = [0u8; 3];
	/// data.view_bits_mut::<Lsb0>()
	///   [5 .. 17]
	///   .store_le(0b0000_1_1011_1000_110u16);
	/// //                 O PQRS TUVW XYZ
	///
	/// assert_eq!(data, [
	///   0b110_00000, 0b1011_1000, 0b0000000_1
	/// //  XYZ          PQRS TUVW            O
	/// ]);
	///
	/// let val = data.view_bits::<Lsb0>()
	///   [5 .. 17]
	///   .load_le::<u16>();
	/// assert_eq!(
	///   val,
	///   0b0000_1_1011_1000_110u16,
	/// //       O PQRS TUVW XYZ
	/// );
	/// ```
	///
	/// And this example shows how the same memory region is written by
	/// different `BitOrder` implementations:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	/// let mut lsb0 = bitarr![Lsb0, u16; 0; 32];
	/// let mut msb0 = bitarr![Msb0, u16; 0; 32];
	///
	/// // Bit pos:        14                                     19  16
	/// // Lsb0:          ─┤                                       ├──┤
	/// let exp_lsb0 = [0b0100_0000_0000_0000u16, 0b0000_0000_0000_1110u16];
	/// let exp_msb0 = [0b0000_0000_0000_0011u16, 0b0001_0000_0000_0000u16];
	/// // Msb0:                           ├─       ├──┤
	/// // Bit pos:                       14       15  19
	///
	/// lsb0[14 ..= 19].store_le(0b111001u8);
	/// msb0[14 ..= 19].store_le(0b000111u8);
	/// assert_eq!(lsb0.as_raw_slice(), exp_lsb0);
	/// assert_eq!(msb0.as_raw_slice(), exp_msb0);
	/// ```
	///
	/// [`M::BITS`]: crate::mem::BitMemory::BITS
	/// [`self.len()`]: crate::slice::BitSlice::len
	fn store_le<M>(&mut self, value: M)
	where M: BitMemory;

	/// Stores into `self`, using big-endian element ordering.
	///
	/// This function interprets a multi-element slice as having its most
	/// significant chunk in the low memory address, and its least significant
	/// chunk in the high memory address. Each element `T` is still interpreted
	/// from individual bytes according to the local CPU ordering.
	///
	/// # Parameters
	///
	/// - `&mut self`: A write reference to some bits in memory. This slice must
	///   be trimmed to have a width no more than the [`M::BITS`] width of the
	///   type being stored. This can be accomplished with range indexing on a
	///   larger slice.
	/// - `value`: A value, whose [`self.len()`] least significant bits will be
	///   stored into `self`.
	///
	/// # Behavior
	///
	/// The [`self.len()`] least significant bits of `value` are written into
	/// the domain of `self`. If `self` spans multiple elements `T`, then the
	/// lowest-address `T` is interpreted as containing the most significant
	/// bits of the `M` return value, and the highest-address `T` is interpreted
	/// as containing its least significant bits.
	///
	/// # Panics
	///
	/// This method is encouraged to panic if `self` is empty, or wider than a
	/// single element `M`.
	///
	/// # Examples
	///
	/// This example shows how a value is segmented across multiple storage
	/// elements:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut data = [0u8; 3];
	/// data.view_bits_mut::<Lsb0>()
	///   [5 .. 17]
	///   .store_be(0b0000_110_1000_1011_1u16);
	/// //                 OPQ RSTU VWXY Z
	///
	/// assert_eq!(data, [
	///   0b110_00000, 0b1000_1011, 0b0000000_1
	/// //  OPQ          RSTU VWXY            Z
	/// ]);
	///
	/// let val = data.view_bits::<Lsb0>()
	///   [5 .. 17]
	///   .load_be::<u16>();
	/// assert_eq!(
	///   val,
	///   0b0000_110_1000_1011_1u16,
	/// //       OPQ RSTU VWXY Z
	/// );
	/// ```
	///
	/// And this example shows how the same memory region is written by
	/// different `BitOrder` implementations:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	/// let mut lsb0 = bitarr![Lsb0, u16; 0; 32];
	/// let mut msb0 = bitarr![Msb0, u16; 0; 32];
	///
	/// // Bit pos:        14                                     19  16
	/// // Lsb0:          ─┤                                       ├──┤
	/// let exp_lsb0 = [0b0100_0000_0000_0000u16, 0b0000_0000_0000_1110u16];
	/// let exp_msb0 = [0b0000_0000_0000_0011u16, 0b0001_0000_0000_0000u16];
	/// // Msb0:                           ├─       ├──┤
	/// // Bit pos:                       14       15  19
	///
	/// lsb0[14 ..= 19].store_be(0b011110u8);
	/// msb0[14 ..= 19].store_be(0b110001u8);
	/// assert_eq!(lsb0.as_raw_slice(), exp_lsb0);
	/// assert_eq!(msb0.as_raw_slice(), exp_msb0);
	/// ```
	///
	/// [`M::BITS`]: crate::mem::BitMemory::BITS
	/// [`self.len()`]: crate::slice::BitSlice::len
	fn store_be<M>(&mut self, value: M)
	where M: BitMemory;
}

impl<T> BitField for BitSlice<Lsb0, T>
where T: BitStore
{
	/// Loads from `self`, using little-endian element ordering if `self` spans
	/// more than one `T` element.
	///
	/// If [`self.domain()`] produces a [`Domain::Region`], then:
	///
	/// - its [`head`] element contains the least significant segment of the
	///   returned value, in the bits at the most significant edge of the
	///   element,
	/// - its [`body`] slice contains successively more-significant segments,
	///   and
	/// - its [`tail`] element contains the most significant segment of the
	///   returned value, in the bits at the least significant edge of the
	///   element.
	///
	/// If the domain is an [`Enclave`], then the referent element is merely
	/// loaded, shifted, and masked; no recombination of segments is necessary.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut data = [0u8; 3];
	/// data.view_bits_mut::<Lsb0>()[5 .. 21].store_le::<u16>(
	///   0b1_1011_0100_1100_011
	/// //  K LMNO PQRS TUVW XYZ
	/// );
	/// assert_eq!(data, [
	///   0b011_00000, 0b0100_1100, 0b000_1_1011
	/// //  XYZ          PQRS TUVW        K LMNO
	/// ]);
	/// let val = data.view_bits::<Lsb0>()[5 .. 21].load_le::<u16>();
	/// assert_eq!(
	///   val,
	///   0b1_1011_0100_1100_011,
	/// //  K LMNO PQRS TUVW XYZ
	/// );
	/// ```
	///
	/// [`Domain::Region`]: crate::domain::Domain::Region
	/// [`Enclave`]: crate::domain::Domain::Enclave
	/// [`body`]: crate::domain::Domain::Region::body
	/// [`head`]: crate::domain::Domain::Region::head
	/// [`self.domain()`]: crate::slice::BitSlice::domain
	/// [`tail`]: crate::domain::Domain::Region::tail
	fn load_le<M>(&self) -> M
	where M: BitMemory {
		check::<M>("load", self.len());

		match self.domain() {
			//  In Lsb0, a `head` index counts distance from LSedge, and a
			//  `tail` index counts element width minus distance from MSedge.
			Domain::Enclave { head, elem, tail } => {
				get::<T, M>(elem, Lsb0::mask(head, tail), head.value())
			},
			Domain::Region { head, body, tail } => {
				let mut accum = M::ZERO;

				/* For multi-`T::Mem` domains, the most significant chunk is
				stored in the highest memory address, the tail. Each successive
				memory address lower has a chunk of decreasing significance,
				until the least significant chunk is stored in the lowest memory
				address, the head.
				*/

				if let Some((elem, tail)) = tail {
					accum = get::<T, M>(elem, Lsb0::mask(None, tail), 0);
				}

				for elem in body.iter().rev().map(BitStore::load_value) {
					/* Rust does not allow the use of shift instructions of
					exactly a type width to clear a value. This loop only enters
					when `M` is not narrower than `T::Mem`, and the shift is
					only needed when `M` occupies *more than one* `T::Mem` slot.
					When `M` is exactly as wide as `T::Mem`, this loop either
					does not run (head and tail only), or runs once (single
					element), and thus the shift is unnecessary.

					As a const-expression, this branch folds at compile-time to
					conditionally remove or retain the instruction.
					*/
					if M::BITS > T::Mem::BITS {
						accum <<= T::Mem::BITS;
					}
					accum |= resize::<T::Mem, M>(elem);
				}

				if let Some((head, elem)) = head {
					let shamt = head.value();
					if M::BITS > T::Mem::BITS - shamt {
						accum <<= T::Mem::BITS - shamt;
					}
					else {
						accum = M::ZERO;
					}
					accum |= get::<T, M>(elem, Lsb0::mask(head, None), shamt);
				}

				accum
			},
		}
	}

	/// Loads from `self`, using big-endian element ordering if `self` spans
	/// more than one `T` element.
	///
	/// If [`self.domain()`] produces a [`Domain::Region`], then:
	///
	/// - its [`head`] element contains the most significant segment of the
	///   returned value, in the bits at the most significant edge of the
	///   element,
	/// - its [`body`] slice contains successively less-significant segments,
	///   and
	/// - its [`tail`] element contains the least significant segment of the
	///   returned value, in the bits at the least significant edge of the
	///   element.
	///
	/// If the domain is an [`Enclave`], then the referent element is merely
	/// loaded, shifted, and masked; no recombination of segments is necessary.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut data = [0u8; 3];
	/// data.view_bits_mut::<Lsb0>()[5 .. 21].store_be::<u16>(
	///   0b011_1100_0100_1011_1,
	/// //  KLM NOPQ RSTU VWXY Z
	/// );
	/// assert_eq!(data, [
	///   0b011_00000, 0b1100_0100, 0b000_1011_1
	/// //  KLM          NOPQ RSTU        VWXY Z
	/// ]);
	/// let val = data.view_bits::<Lsb0>()[5 .. 21].load_be::<u16>();
	/// assert_eq!(
	///   val,
	///   0b011_1100_0100_1011_1,
	/// //  KLM NOPQ RSTU VWXY Z
	/// );
	/// ```
	///
	/// [`Domain::Region`]: crate::domain::Domain::Region
	/// [`Enclave`]: crate::domain::Domain::Enclave
	/// [`body`]: crate::domain::Domain::Region::body
	/// [`head`]: crate::domain::Domain::Region::head
	/// [`self.domain()`]: crate::slice::BitSlice::domain
	/// [`tail`]: crate::domain::Domain::Region::tail
	fn load_be<M>(&self) -> M
	where M: BitMemory {
		check::<M>("load", self.len());

		match self.domain() {
			Domain::Enclave { head, elem, tail } => {
				get::<T, M>(elem, Lsb0::mask(head, tail), head.value())
			},
			Domain::Region { head, body, tail } => {
				let mut accum = M::ZERO;

				if let Some((head, elem)) = head {
					accum =
						get::<T, M>(elem, Lsb0::mask(head, None), head.value());
				}

				for elem in body.iter().map(BitStore::load_value) {
					if M::BITS > T::Mem::BITS {
						accum <<= T::Mem::BITS;
					}
					accum |= resize::<T::Mem, M>(elem);
				}

				if let Some((elem, tail)) = tail {
					let shamt = tail.value();
					if M::BITS > shamt {
						accum <<= shamt;
					}
					else {
						accum = M::ZERO;
					}
					accum |= get::<T, M>(elem, Lsb0::mask(None, tail), 0);
				}

				accum
			},
		}
	}

	/// Stores into `self`, using little-endian element ordering if `self` spans
	/// more than one `T` element.
	///
	/// If [`self.domain()`] produces a [`Domain::Region`], then:
	///
	/// - its [`head`] element receives the least significant segment of
	///   `value`, in the bits at the most significant edge of the element,
	/// - its [`body`] slice receives successively more-significant segments of
	///   `value`, and
	/// - its [`tail`] element receives the most significant segment of `value`,
	///   in the bits at the least significant edge of the element.
	///
	/// If the domain is an [`Enclave`], then `value` is shifted into place and
	/// written without any segmentation.
	///
	/// # Examples
	///
	/// See the documentation for `<BitSlice<Lsb0, u8> as BitField>::load_le`.
	///
	/// [`Domain::Region`]: crate::domain::Domain::Region
	/// [`Enclave`]: crate::domain::Domain::Enclave
	/// [`head`]: crate::domain::Domain::Region::head
	/// [`body`]: crate::domain::Domain::Region::body
	/// [`self.domain()`]: crate::slice::BitSlice::domain
	/// [`tail`]: crate::domain::Domain::Region::tail
	fn store_le<M>(&mut self, mut value: M)
	where M: BitMemory {
		check::<M>("store", self.len());

		match self.domain_mut() {
			DomainMut::Enclave { head, elem, tail } => {
				set::<T, M>(elem, value, Lsb0::mask(head, tail), head.value());
			},
			DomainMut::Region { head, body, tail } => {
				if let Some((head, elem)) = head {
					let shamt = head.value();
					set::<T, M>(elem, value, Lsb0::mask(head, None), shamt);
					if M::BITS > T::Mem::BITS - shamt {
						value >>= T::Mem::BITS - shamt;
					}
					else {
						value = M::ZERO;
					}
				}

				for elem in body.iter_mut() {
					elem.store_value(resize(value));
					if M::BITS > T::Mem::BITS {
						value >>= T::Mem::BITS;
					}
				}

				if let Some((elem, tail)) = tail {
					set::<T, M>(elem, value, Lsb0::mask(None, tail), 0);
				}
			},
		}
	}

	/// Stores into `self`, using big-endian element ordering if `self` spans
	/// more than one `T` element.
	///
	/// If [`self.domain()`] produces a [`Domain::Region`], then:
	///
	/// - its [`head`] element receives the most significant segment of `value`,
	///   in the bits at the most significant edge of the element,
	/// - its [`body`] slice receives successively less-significant segments of
	///   `value`, and
	/// - its [`tail`] element receives the least significant segment of
	///   `value`, in the bits at the least significant edge of the element.
	///
	/// If the domain is an [`Enclave`], then `value` is shifted into place and
	/// written without any segmentation.
	///
	/// # Examples
	///
	/// See the documentation for `<BitSlice<Lsb0, u8> as BitField>::load_be`.
	///
	/// [`Domain::Region`]: crate::domain::Domain::Region
	/// [`Enclave`]: crate::domain::Domain::Enclave
	/// [`head`]: crate::domain::Domain::Region::head
	/// [`body`]: crate::domain::Domain::Region::body
	/// [`self.domain()`]: crate::slice::BitSlice::domain
	/// [`tail`]: crate::domain::Domain::Region::tail
	fn store_be<M>(&mut self, mut value: M)
	where M: BitMemory {
		check::<M>("store", self.len());

		match self.domain_mut() {
			DomainMut::Enclave { head, elem, tail } => {
				set::<T, M>(elem, value, Lsb0::mask(head, tail), head.value());
			},
			DomainMut::Region { head, body, tail } => {
				if let Some((elem, tail)) = tail {
					set::<T, M>(elem, value, Lsb0::mask(None, tail), 0);
					let shamt = tail.value();
					if M::BITS > shamt {
						value >>= shamt;
					}
					else {
						value = M::ZERO;
					}
				}

				for elem in body.iter_mut().rev() {
					elem.store_value(resize(value));
					if M::BITS > T::Mem::BITS {
						value >>= T::Mem::BITS;
					}
				}

				if let Some((head, elem)) = head {
					set::<T, M>(
						elem,
						value,
						Lsb0::mask(head, None),
						head.value(),
					);
				}
			},
		}
	}
}

impl<T> BitField for BitSlice<Msb0, T>
where T: BitStore
{
	/// Loads from `self`, using little-endian element ordering if `self` spans
	/// more than one `T` element.
	///
	/// If [`self.domain()`] produces a [`Domain::Region`], then:
	///
	/// - its [`head`] element contains the least significant segment of the
	///   returned value, in the bits at the least significant edge of the
	///   element,
	/// - its [`body`] slice contains successively more-significant segments,
	///   and
	/// - its [`tail`] element contains the most significant segment of the
	///   returned value, in the bits at the most significant edge of the
	///   element.
	///
	/// If the domain is an [`Enclave`], then the referent element is merely
	/// loaded, shifted, and masked; no recombination of segments is necessary.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut data = [0u8; 3];
	/// data.view_bits_mut::<Msb0>()[5 .. 21].store_le::<u16>(
	///   0b1_1011_0100_1100_110
	/// //  K LMNO PQRS TUVW XYZ
	/// );
	/// assert_eq!(data, [
	///   0b00000_110, 0b0100_1100, 0b1_1011_000
	/// //        XYZ    PQRS TUVW    K LMNO
	/// ]);
	/// let val = data.view_bits::<Msb0>()[5 .. 21].load_le::<u16>();
	/// assert_eq!(
	///   val,
	///   0b1_1011_0100_1100_110,
	/// //  K LMNO PQRS TUVW XYZ
	/// );
	/// ```
	///
	/// [`Domain::Region`]: crate::domain::Domain::Region
	/// [`Enclave`]: crate::domain::Domain::Enclave
	/// [`body`]: crate::domain::Domain::Region::body
	/// [`head`]: crate::domain::Domain::Region::head
	/// [`self.domain()`]: crate::slice::BitSlice::domain
	/// [`tail`]: crate::domain::Domain::Region::tail
	fn load_le<M>(&self) -> M
	where M: BitMemory {
		check::<M>("load", self.len());

		match self.domain() {
			Domain::Enclave { head, elem, tail } => get::<T, M>(
				elem,
				Msb0::mask(head, tail),
				T::Mem::BITS - tail.value(),
			),
			Domain::Region { head, body, tail } => {
				let mut accum = M::ZERO;

				if let Some((elem, tail)) = tail {
					accum = get::<T, M>(
						elem,
						Msb0::mask(None, tail),
						T::Mem::BITS - tail.value(),
					);
				}

				for elem in body.iter().rev().map(BitStore::load_value) {
					if M::BITS > T::Mem::BITS {
						accum <<= T::Mem::BITS;
					}
					accum |= resize::<T::Mem, M>(elem);
				}

				if let Some((head, elem)) = head {
					let shamt = T::Mem::BITS - head.value();
					if M::BITS > shamt {
						accum <<= shamt;
					}
					else {
						accum = M::ZERO;
					}
					accum |= get::<T, M>(elem, Msb0::mask(head, None), 0);
				}

				accum
			},
		}
	}

	/// Loads from `self`, using big-endian element ordering if `self` spans
	/// more than one element `T`.
	///
	/// If [`self.domain()`] produces a [`Domain::Region`], then:
	///
	/// - its [`head`] element contains the most significant segment of the
	///   returned value, in the bits at the least significant edge of the
	///   element,
	/// - its [`body`] slice contains successively less-significant segments,
	///   and
	/// - its [`tail`] element contains the least significant segment of the
	///   returned value, in the bits at the most significant edge of the
	///   element.
	///
	/// If the domain is an [`Enclave`], then the referent element is merely
	/// loaded, shifted, and masked; no recombination of segments is necessary.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut data = [0u8; 3];
	/// data.view_bits_mut::<Msb0>()[5 .. 21].store_be::<u16>(
	///   0b110_1011_1100_0100_1
	/// //  KLM NOPQ RSTU VWXY Z
	/// );
	/// assert_eq!(data, [
	///   0b00000_110, 0b1011_1100, 0b0100_1_000
	/// //        KLM    NOPQ RSTU    VWXY Z
	/// ]);
	/// let val = data.view_bits::<Msb0>()[5 .. 21].load_be::<u16>();
	/// assert_eq!(
	///   val,
	///   0b110_1011_1100_0100_1,
	/// //  KLM NOPQ RSTU VWXY Z
	/// );
	/// ```
	///
	/// [`Domain::Region`]: crate::domain::Domain::Region
	/// [`Enclave`]: crate::domain::Domain::Enclave
	/// [`body`]: crate::domain::Domain::Region::body
	/// [`head`]: crate::domain::Domain::Region::head
	/// [`self.domain()`]: crate::slice::BitSlice::domain
	/// [`tail`]: crate::domain::Domain::Region::tail
	fn load_be<M>(&self) -> M
	where M: BitMemory {
		check::<M>("load", self.len());

		match self.domain() {
			Domain::Enclave { head, elem, tail } => get::<T, M>(
				elem,
				Msb0::mask(head, tail),
				T::Mem::BITS - tail.value(),
			),
			Domain::Region { head, body, tail } => {
				let mut accum = M::ZERO;

				if let Some((head, elem)) = head {
					accum = get::<T, M>(elem, Msb0::mask(head, None), 0);
				}

				for elem in body.iter().map(BitStore::load_value) {
					if M::BITS > T::Mem::BITS {
						accum <<= T::Mem::BITS;
					}
					accum |= resize::<T::Mem, M>(elem);
				}

				if let Some((elem, tail)) = tail {
					let shamt = tail.value();
					if M::BITS > shamt {
						accum <<= shamt;
					}
					else {
						accum = M::ZERO;
					}
					accum |= get::<T, M>(
						elem,
						Msb0::mask(None, tail),
						T::Mem::BITS - shamt,
					);
				}

				accum
			},
		}
	}

	/// Stores into `self`, using little-endian element ordering if `self` spans
	/// more than one `T` element.
	///
	/// If [`self.domain()`] produces a [`Domain::Region`], then:
	///
	/// - its [`head`] element receives the least significant segment of
	///   `value`, in the bits at the least significant edge of the element,
	/// - its [`body`] slice receives successively more-significant segments of
	///   `value`, and
	/// - its [`tail`] element receives the most significant segment of `value`,
	///   in the bits at the most significant edge of the element.
	///
	/// If the domain is an [`Enclave`], then `value` is shifted into place and
	/// written without any segmentation.
	///
	/// # Examples
	///
	/// See the documentation for `<BitSlice<Msb0, u8> as BitField>::load_le`.
	///
	/// [`Domain::Region`]: crate::domain::Domain::Region
	/// [`Enclave`]: crate::domain::Domain::Enclave
	/// [`head`]: crate::domain::Domain::Region::head
	/// [`body`]: crate::domain::Domain::Region::body
	/// [`self.domain()`]: crate::slice::BitSlice::domain
	/// [`tail`]: crate::domain::Domain::Region::tail
	fn store_le<M>(&mut self, mut value: M)
	where M: BitMemory {
		check::<M>("store", self.len());

		match self.domain_mut() {
			DomainMut::Enclave { head, elem, tail } => set::<T, M>(
				elem,
				value,
				Msb0::mask(head, tail),
				T::Mem::BITS - tail.value(),
			),
			DomainMut::Region { head, body, tail } => {
				if let Some((head, elem)) = head {
					set::<T, M>(elem, value, Msb0::mask(head, None), 0);
					let shamt = T::Mem::BITS - head.value();
					if M::BITS > shamt {
						value >>= shamt;
					}
					else {
						value = M::ZERO;
					}
				}

				for elem in body.iter_mut() {
					elem.store_value(resize(value));
					if M::BITS > T::Mem::BITS {
						value >>= T::Mem::BITS;
					}
				}

				if let Some((elem, tail)) = tail {
					set::<T, M>(
						elem,
						value,
						Msb0::mask(None, tail),
						T::Mem::BITS - tail.value(),
					);
				}
			},
		}
	}

	/// Stores into `self`, using big-endian element ordering if `self` spans
	/// more than one `T` element.
	///
	/// If [`self.domain()`] produces a [`Domain::Region`], then:
	///
	/// - its [`head`] element receives the most significant segment of `value`,
	///   in the bits at the least significant edge of the element,
	/// - its [`body`] slice receives successively less-significant segments of
	///   `value`, and
	/// - its [`tail`] element receives the least significant segment of
	///   `value`, in the bits at the most significant edge of the element.
	///
	/// If the domain is an [`Enclave`], then `value` is shifted into place and
	/// written without any segmentation.
	///
	/// # Examples
	///
	/// See the documentation for `<BitSlice<Lsb0, u8> as BitField>::load_be`.
	///
	/// [`Domain::Region`]: crate::domain::Domain::Region
	/// [`Enclave`]: crate::domain::Domain::Enclave
	/// [`head`]: crate::domain::Domain::Region::head
	/// [`body`]: crate::domain::Domain::Region::body
	/// [`self.domain()`]: crate::slice::BitSlice::domain
	/// [`tail`]: crate::domain::Domain::Region::tail
	fn store_be<M>(&mut self, mut value: M)
	where M: BitMemory {
		check::<M>("store", self.len());

		match self.domain_mut() {
			DomainMut::Enclave { head, elem, tail } => set::<T, M>(
				elem,
				value,
				Msb0::mask(head, tail),
				T::Mem::BITS - tail.value(),
			),
			DomainMut::Region { head, body, tail } => {
				if let Some((elem, tail)) = tail {
					set::<T, M>(
						elem,
						value,
						Msb0::mask(None, tail),
						T::Mem::BITS - tail.value(),
					);
					if M::BITS > tail.value() {
						value >>= tail.value();
					}
					else {
						value = M::ZERO;
					}
				}

				for elem in body.iter_mut().rev() {
					elem.store_value(resize(value));
					if M::BITS > T::Mem::BITS {
						value >>= T::Mem::BITS;
					}
				}

				if let Some((head, elem)) = head {
					set::<T, M>(elem, value, Msb0::mask(head, None), 0);
				}
			},
		}
	}
}

impl<O, V> BitField for BitArray<O, V>
where
	O: BitOrder,
	V: BitView,
	BitSlice<O, V::Store>: BitField,
{
	fn load_le<M>(&self) -> M
	where M: BitMemory {
		self.as_bitslice().load_le()
	}

	fn load_be<M>(&self) -> M
	where M: BitMemory {
		self.as_bitslice().load_be()
	}

	fn store_le<M>(&mut self, value: M)
	where M: BitMemory {
		self.as_mut_bitslice().store_le(value)
	}

	fn store_be<M>(&mut self, value: M)
	where M: BitMemory {
		self.as_mut_bitslice().store_be(value)
	}
}

#[cfg(feature = "alloc")]
impl<O, T> BitField for BitBox<O, T>
where
	O: BitOrder,
	T: BitStore,
	BitSlice<O, T>: BitField,
{
	fn load_le<M>(&self) -> M
	where M: BitMemory {
		self.as_bitslice().load_le()
	}

	fn load_be<M>(&self) -> M
	where M: BitMemory {
		self.as_bitslice().load_be()
	}

	fn store_le<M>(&mut self, value: M)
	where M: BitMemory {
		self.as_mut_bitslice().store_le(value)
	}

	fn store_be<M>(&mut self, value: M)
	where M: BitMemory {
		self.as_mut_bitslice().store_be(value)
	}
}

#[cfg(feature = "alloc")]
impl<O, T> BitField for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
	BitSlice<O, T>: BitField,
{
	fn load_le<M>(&self) -> M
	where M: BitMemory {
		self.as_bitslice().load_le()
	}

	fn load_be<M>(&self) -> M
	where M: BitMemory {
		self.as_bitslice().load_be()
	}

	fn store_le<M>(&mut self, value: M)
	where M: BitMemory {
		self.as_mut_bitslice().store_le(value)
	}

	fn store_be<M>(&mut self, value: M)
	where M: BitMemory {
		self.as_mut_bitslice().store_be(value)
	}
}

/// Asserts that a slice length is within a memory element width.
///
/// # Panics
///
/// This panics if len is 0, or wider than [`M::BITS`].
///
/// [`M::BITS`]: crate::mem::BitMemory::BITS
fn check<M>(action: &'static str, len: usize)
where M: BitMemory {
	if !(1 ..= M::BITS as usize).contains(&len) {
		panic!(
			"Cannot {} {} bits from a {}-bit region",
			action,
			M::BITS,
			len,
		);
	}
}

/** Reads a value out of a section of a memory element.

This function is used to extract a portion of an `M` value from a portion of a
`T` value. The [`BitField`] implementations call it as they assemble a complete
`M`. It performs the following steps:

1. the referent value of the `elem` pointer is copied into local memory,
2. `mask`ed to discard the portions of `*elem` that are not live,
3. shifted to the LSedge of the [`T::Mem`] temporary,
4. then `resize`d into an `M` value.

This is the exact inverse of `set`.

# Type Parameters

- `T`: The [`BitStore`] type of a [`BitSlice`] that is the source of a read
  event.
- `M`: The local type of the data contained in that [`BitSlice`].

# Parameters

- `elem`: An aliased reference to a single element of a [`BitSlice`] storage.
  This is required to remain aliased, as other write-capable references to the
  location may exist.
- `mask`: A [`BitMask`] of the live region of the value at `*elem` to be used as
  the contents of the returned value.
- `shamt`: The distance of the least significant bit of the mask region from the
  least significant edge of the [`T::Mem`] fetched value.

# Returns

`resize((*elem & mask) >> shamt)`

[`BitField`]: crate::field::BitField
[`BitMask`]: crate::index::BitMask
[`BitSlice`]: crate::slice::BitSlice
[`BitStore`]: crate::store::BitStore
[`T::Mem`]: crate::store::BitStore::Mem
**/
//  The trait resolution system fails here, and only resolves to `<&usize>` as
//  the RHS operand.
#[allow(clippy::op_ref)]
fn get<T, M>(elem: &T, mask: BitMask<T::Mem>, shamt: u8) -> M
where
	T: BitStore,
	M: BitMemory,
{
	//  Read the value out of the `elem` reference
	elem.load_value()
		//  Mask it against the slot
		.pipe(|val| val & &mask.value())
		//  Shift it down to the LSedge
		.pipe(|val| val >> &(shamt as usize))
		//  And resize to the expected output
		.pipe(resize::<T::Mem, M>)
}

/** Writes a value into a section of a memory element.

This function is used to emplace a portion of an `M` value into a portion of a
`T` value. The [`BitField`] implementations call it as they disassemble a
complete `M`. It performs the following steps:

1. the provided `value` is `resize`d from `M` to [`T::Mem`],
2. then shifted from the LSedge of the [`T::Mem`] temporary by `shamt`,
3. `mask`ed to discard the portions of `value` that are not live,
4. then written into the `mask`ed portion of `*elem`.

This is the exact inverse of `get`.

# Type Parameters

- `T`: The [`BitStore`] type of a [`BitSlice`] that is the sink of a write event.
- `M`: The local type of the data being written into that [`BitSlice`].

# Parameters

- `elem`: An aliased reference to a single element of a [`BitSlice`] storage.
- `value`: The value whose least-significant bits will be written into the
  subsection of `*elt` covered by `mask`.
- `mask`: A `BitMask` of the live region of the value at `*elem` to be used as
  a filter on the provided value.
- `shamt`: The distance of the least significant bit of the mask region from the
  least significant edge of the [`T::Mem`] destination value.

# Effects

`*elem &= !mask; *elem |= (resize(value) << shamt) & mask;`

[`BitField`]: crate::field::BitField
[`BitMask`]: crate::index::BitMask
[`BitSlice`]: crate::slice::BitSlice
[`BitStore`]: crate::store::BitStore
[`T::Mem`]: crate::store::BitStore::Mem
**/
#[allow(clippy::op_ref)]
fn set<T, M>(elem: &T::Access, value: M, mask: BitMask<T::Mem>, shamt: u8)
where
	T: BitStore,
	M: BitMemory,
{
	//  Convert the `mask` type to fit into the accessor.
	let mask = BitMask::new(mask.value());
	let value = value
		//  Resize the value to the expected input
		.pipe(resize::<M, T::Mem>)
		//  Shift it up from the LSedge
		.pipe(|val| val << &(shamt as usize))
		//  And mask it to the slot
		.pipe(|val| mask & val);

	//  Erase the slot
	elem.clear_bits(mask);
	//  And write the shift/masked value into it
	elem.set_bits(value);
}

/** Resizes a value from one register width to another.

This zero-extends or truncates its source value in order to fit in the target
type.

# Type Parameters

- `T`: The initial register type of the value to resize.
- `U`: The final register type of the resized value.

# Parameters

- `value`: Any register value.

# Returns

`value`, either zero-extended if `U` is wider than `T` or truncated if `U` is
narrower than `T`.
**/
fn resize<T, U>(value: T) -> U
where
	T: BitMemory,
	U: BitMemory,
{
	let mut out = U::ZERO;
	let size_t = mem::size_of::<T>();
	let size_u = mem::size_of::<U>();

	unsafe {
		resize_inner::<T, U>(&value, &mut out, size_t, size_u);
	}

	out
}

/// Performs little-endian byte-order register resizing.
#[cfg(target_endian = "little")]
unsafe fn resize_inner<T, U>(
	src: &T,
	dst: &mut U,
	size_t: usize,
	size_u: usize,
) {
	//  In LE, the least significant byte is the base address, so resizing is
	//  just a memcpy into a zeroed slot, taking only the smaller width.
	ptr::copy_nonoverlapping(
		src as *const T as *const u8,
		dst as *mut U as *mut u8,
		core::cmp::min(size_t, size_u),
	);
}

/// Performs big-endian byte-order register resizing.
#[cfg(target_endian = "big")]
unsafe fn resize_inner<T, U>(
	src: &T,
	dst: &mut U,
	size_t: usize,
	size_u: usize,
) {
	let src = src as *const T as *const u8;
	let dst = dst as *mut U as *mut u8;

	//  In BE, shrinking a value requires moving the source base pointer up,
	if size_t > size_u {
		ptr::copy_nonoverlapping(src.add(size_t - size_u), dst, size_u);
	}
	//  While expanding a value requires moving the destination base pointer up.
	else {
		ptr::copy_nonoverlapping(src, dst.add(size_u - size_t), size_t);
	}
}

#[cfg(not(any(target_endian = "big", target_endian = "little")))]
compile_fail!(concat!(
	"This architecture is currently not supported. File an issue at ",
	env!(CARGO_PKG_REPOSITORY)
));

#[cfg(feature = "std")]
mod io;

#[cfg(test)]
mod tests;

// These tests are purely mathematical, and do not need to run more than once.
#[cfg(all(test, feature = "std", not(miri), not(tarpaulin)))]
mod permutation_tests;
