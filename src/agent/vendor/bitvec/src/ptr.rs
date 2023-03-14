/*! Mirror of the [`core::ptr`] module and `bitvec`-specific pointer structures.

# Types

As `bitvec` is not the standard library, it does not have the freedom to use
language builtins such as actual pointers. Instead, `bitvec` uses its own
analagous structures:

- [`BitPtr<M, O, T>`]: This represents a pointer to a single bit, and is vaguely
  similar to `*const bool`, `*mut bool`, and `NonNull<bool>`. It consists of a
  (non-null, well-aligned) pointer to a `T` memory element and a bit-index
  within that element. It uses the `O` ordering implementation to access the
  selected bit, and uses `M` to determine whether it has write permissions to
  the location.
- [`BitPtrRange<M, O, T>`]: This is equivalent to `Range<BitPtr<M, O, T>>`. It
  exists because [`Range`] has some associated types that are still unstable to
  implement for its type parameters. It is also smaller than the `Range` would
  be, because it can take advantage of layout optimizations.
- [`BitRef<M, O, T>`]: This is a proxy reference type, equivalent to the C++
  [`bitset<N>::reference`]. It implements `Deref` and, if `M` is `Mut`,
  `DerefMut` to bool, so that it can be read from and written to as if it were
  an `&bool` or `&mut bool`. It is **not** a referent type, and cannot be used
  in APIs that expect actual references. It is implemented under the hood as a
  `BitPtr` with a `bool` cached in one of the padding bytes.
- `BitSpan<M, O, T>`: This is a crate-internal type that encodes a `BitPtr` and
  a length counter into a two-word structure that can be transmuted into
  `*BitSlice<O, T>`. This type enforces the non-null/well-aligned rule, and is
  the source of the limitation that `bitvec` region types can only address ⅛ of
  a `usize`, rather than the ½ limitation of the standard library collections.

  This type is not public API; it will only ever appear in its transmuted form,
  `*BitSlice<O, T>`. Users are **not permitted** to use any of the [`core::ptr`]
  or [`pointer`] functions to view or modify `*BitSlice` pointers, or their
  referent locations, in any way.

# Safety

The functions in this module take `bitvec` equivalents to raw pointers as their
arguments and read from or write to them. For this to be safe, these pointers
must be *valid*. Whether a pointer is valid depends on the operation it is used
for (reading or writing), and the extent of the memory that is accessed (i.e.
how many bits are read/written in and how many underlying memory elements are
accessed). Most functions use [`BitPtr`] to access only a single bit, in which
case the documentation omits the size and implicitly assumes it to be one bit in
one `T` element.

The Rust rules about pointer validity are always in effect; `bitvec` refines
them to a bit-precision granularity, but must still respect the byte-level and
element-level rules.

# Crate-Specific Restrictions

`bitvec` uses an internal encoding scheme to make bit-region pointers fit into a
standard Rust slice pointer. This encoding requires that the base element
address of a bit-pointer be *non-null* and *well-aligned* for all pointers that
are used in the encoding scheme.

The `bitvec` structure used to emulate a pointer to a single bit is larger than
one processor word, and thus cannot be encoded to fit in a `*const Bit`. To ease
internal complexity, these restrictions are universal in `bitvec`: the crate as
a whole refuses to operate on null pointers, or pointers that are not aligned to
their referent type, even if your usage never touches the span encoding.

As such, the pointer types in this module can essentially only be sourced from
references, not from arbitrary address values. While this module attempts to
rely on actual Rust references as much as possible, and instead use only the
ordinary [`core::ptr`] and [`pointer`] functions. This is not always possible;
in particular, Rust does not offer stable atomic intrinsics, and instead only
allows them to be used on references.

[`BitPtr`]: crate::ptr::BitPtr
[`BitPtr<M, O, T>`]: crate::ptr::BitPtr
[`BitPtrRange<M, O, T>`]: crate::ptr::BitPtrRange
[`BitRef<M, O, T>`]: crate::ptr::BitRef
[`BitSpan<M, O, T>`]: crate::ptr::BitSpan
[`Range`]: core::ops::Range
[`bitset<N>::reference`]: https://en.cppreference.com/w/cpp/utility/bitset/reference
[`core::ptr`]: core::ptr
[`pointer`]: https://doc.rust-lang.org/std/primitive.pointer.html
!*/

use crate::{
	order::BitOrder,
	slice::BitSlice,
	store::BitStore,
};

use core::hash::{
	Hash,
	Hasher,
};

mod address;
mod proxy;
pub(crate) mod range;
mod single;
mod span;

pub(crate) use self::span::BitSpan;

pub use crate::{
	mutability::{
		Const,
		Mut,
	},
	ptr::{
		address::{
			Address,
			AddressError,
		},
		proxy::BitRef,
		range::BitPtrRange,
		single::{
			BitPtr,
			BitPtrError,
		},
		span::BitSpanError,
	},
};

/// Copies `count` bits from `src` to `dst`. The source and destination may
/// overlap.
///
/// If the source and destination will *never* overlap, [`copy_nonoverlapping`]
/// can be used instead.
///
/// `copy` is semantically equivalent to C’s [`memmove`], but with the argument
/// order swapped. Copying takes place as if the bits were copied from `src` to
/// a temporary array, then copied from the array into `dst`.
///
/// # Original
///
/// [`ptr::copy`](core::ptr::copy)
///
/// # API Differences
///
/// The pointers may differ in bit-ordering or storage element types. `bitvec`
/// considers it Undefined Behavior for two pointer regions to overlap in memory
/// if they have different bit-orderings, and so will only perform overlap
/// detection when `O1` and `O2` match.
///
/// # Safety
///
/// Behavior is undefined if any of the following conditions are violated:
///
/// - `src` must be [valid] for reads of `count` bits.
/// - `dst` must be [valid] for writes of `count` bits.
/// - `src` and `dst` must not overlap if they have different bit-ordering
///   parameters.
///
/// The type parameters `T1` and `T2` are permitted to differ.
///
/// # Examples
///
/// Basic usage:
///
/// ```rust
/// use bitvec::prelude::*;
///
/// let start = 0b1011u8;
/// let mut end = 0u16;
///
/// unsafe {
///   bitvec::ptr::copy::<Lsb0, Msb0, _, _>(
///     (&start).into(),
///     (&mut end).into(),
///     4,
///   );
/// }
/// assert_eq!(end, 0b1101_0000__0000_0000);
/// ```
///
/// Overlapping regions:
///
/// ```rust
/// use bitvec::prelude::*;
///
/// let mut x = 0b1111_0010u8;
/// let src = BitPtr::<_, Lsb0, _>::from_mut(&mut x);
/// let dst = unsafe { src.add(2) };
///
/// unsafe {
///   bitvec::ptr::copy(src.immut(), dst, 4);
/// }
/// assert_eq!(x, 0b1100_1010);
/// //                ^^ ^^ bottom nibble moved here
/// ```
///
/// [valid]: https://doc.rust-lang.org/std/ptr/index.html#safety
/// [`copy_nonoverlapping`]: crate::ptr::copy_nonoverlapping
/// [`memmove`]: https://en.cppreference.com/w/c/string/byte/memmove
#[inline]
pub unsafe fn copy<O1, O2, T1, T2>(
	src: BitPtr<Const, O1, T1>,
	dst: BitPtr<Mut, O2, T2>,
	count: usize,
) where
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	src.copy_to(dst, count);
}

/// Copies `count` bits from `src` to `dst`. The source and destination must
/// *not* overlap.
///
/// For regions of memory which might overlap, use [`copy`] instead.
///
/// `copy_nonoverlapping` is semantically equivalent to C’s [`memcpy`], but with
/// the argument order swapped.
///
/// # Original
///
/// [`ptr::copy_nonoverlapping`](core::ptr::copy_nonoverlapping)
///
/// # API Differences
///
/// The pointers may differ in bit-ordering or storage element parameters.
///
/// # Safety
///
/// Behavior is undefined if any of the following conditions are violated:
///
/// - `src` must be [valid] for reads of `count` bits.
/// - `dst` must be [valid] for writes of `count` bits.
/// - The region of memory beginning at `src` with a size of `count` bits must
///   not overlap with the region of memory beginning at `dst` with the same
///   size.
///
/// # Examples
///
/// ```rust
/// use bitvec::prelude::*;
///
/// let mut data = 0b1011u8;
/// let ptr = BitPtr::<_, Msb0, _>::from_mut(&mut data);
///
/// unsafe {
///   bitvec::ptr::copy_nonoverlapping(
///     ptr.add(4).immut(),
///     ptr,
///     4,
///   );
/// }
/// assert_eq!(data, 0b1011_1011);
/// ```
///
/// [valid]: https://doc.rust-lang.org/std/ptr/index.html#safety
/// [`copy`]: crate::ptr::copy
/// [`memcpy`]: https://en.cppreference.com/w/c/string/byte/memcpy
#[inline]
pub unsafe fn copy_nonoverlapping<O1, O2, T1, T2>(
	src: BitPtr<Const, O1, T1>,
	dst: BitPtr<Mut, O2, T2>,
	count: usize,
) where
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	src.copy_to_nonoverlapping(dst, count);
}

/** Compares raw bit-pointers for equality.

This is the same as using the `==` operator, but less generic: the arguments
have to be `BitPtr<Const, _, _>` bit-pointers, not anything that implements
`PartialEq`.

# Original

[`ptr::eq`](core::ptr::eq)

# API Differences

The two pointers can differ in their storage type parameters. `bitvec` defines
pointer equality only between pointers with the same underlying `BitStore::Mem`
register type.

This cannot compare span pointers. `*const BitSlice` can be used in the standard
library `ptr::eq` and does not need an override.

# Examples

```rust
use bitvec::prelude::*;
use core::cell::Cell;

let data = 0u8;
let bare_ptr = BitPtr::<_, Lsb0, _>::from_ref(&data);
let cell_ptr = bare_ptr.cast::<Cell<u8>>();

assert!(bitvec::ptr::eq(bare_ptr, cell_ptr));
```
**/
#[inline]
pub fn eq<O, T1, T2>(a: BitPtr<Const, O, T1>, b: BitPtr<Const, O, T2>) -> bool
where
	O: BitOrder,
	T1: BitStore,
	T2: BitStore,
	BitPtr<Const, O, T1>: PartialEq<BitPtr<Const, O, T2>>,
{
	a == b
}

/** Hash a raw bit-pointer.

This can be used to prove hashing the pointer address value, rather than the
referent bit.

# Original

[`ptr::hash`](core::ptr::hash)
**/
#[inline]
#[cfg(not(tarpaulin_include))]
pub fn hash<O, T, S>(hashee: BitPtr<Const, O, T>, into: &mut S)
where
	O: BitOrder,
	T: BitStore,
	S: Hasher,
{
	hashee.hash(into);
}

/** Reads the bit from `src`.

# Original

[`ptr::read`](core::ptr::read)

# Safety

Behavior is undefined if any of the following conditions are violated:

- `src` must be [valid] for reads.
- `src` must point to a properly initialized value of type `T`.

# Examples

```rust
use bitvec::prelude::*;

let data = 128u8;
let ptr = BitPtr::<_, Msb0, _>::from_ref(&data);
assert!(unsafe {
  bitvec::ptr::read(ptr)
});
```

[valid]: https://doc.rust-lang.org/std/ptr/index.html#safety
**/
#[inline]
pub unsafe fn read<O, T>(src: BitPtr<Const, O, T>) -> bool
where
	O: BitOrder,
	T: BitStore,
{
	src.read()
}

/** Performs a volatile read of the bit from `src`.

Volatile operations are intended to act on I/O memory, and are guaranteed to not
be elided or reördered by the compiler across other volatile operations.

# Original

[`ptr::read_volatile`](core::ptr::read_volatile)

# Notes

Rust does not curretnly have a rigorously and formally defined memory model, so
the precise semantics of what “volatile” means here is subject to change over
time. That being said, the semantics will almost always end up pretty similar to
[C11’s definition of volatile][c11].

The compiler shouldn’t change the relative order or number of volatile memory
operations.

# Safety

Behavior is undefined if any of the following conditions are violated:

- `dst` must be [valid] for reads
- `dst` must point to a properly initialized value of type `T`
- no other pointer must race `dst` to view or modify the referent location
  unless `T` is capable of ensuring race safety.

Just like in C, whether an operation is volatile has no bearing whatsoëver on
questions involving concurrent access from multiple threads. Volatile accesses
behave exactly like non-atomic accesses in that regard. In particular, a race
between a `read_volatile` and any write operation on the same location is
undefined behavior.

This is true even for atomic types! This instruction is an ordinary load that
the compiler will not remove. It is *not* an atomic instruction.

# Examples

```rust
use bitvec::prelude::*;

let data = 4u8;
let ptr = BitPtr::<_, Lsb0, _>::from_ref(&data);
unsafe {
  assert!(bitvec::ptr::read_volatile(ptr.add(2)));
}
```

[c11]: http://www.open-std.org/jtc1/sc22/wg14/www/docs/n1570.pdf
[valid]: https://doc.rust-lang.org/core/ptr/index.html#safety
**/
#[inline]
pub unsafe fn read_volatile<O, T>(src: BitPtr<Const, O, T>) -> bool
where
	O: BitOrder,
	T: BitStore,
{
	src.read_volatile()
}

/** Moves `src` into the pointed `dst`, returning the previous `dst` bit.

This function is semantically equivalent to [`BitRef::replace`] except that it
operates on raw pointers instead of references. When a proxy reference is
available, prefer [`BitRef::replace`].

# Original

[`ptr::replace`](core::ptr::replace)

# Safety

Behavior is undefined if any of the following conditions are violated:

- `dst` must be [valid] for both reads and writes.
- `dst` must point to a properly initialized value of type `T`.

# Examples

```rust
use bitvec::prelude::*;

let mut data = 4u8;
let ptr = BitPtr::<_, Lsb0, _>::from_mut(&mut data);
assert!(unsafe {
  bitvec::ptr::replace(ptr.add(2), false)
});
assert_eq!(data, 0);
```

[valid]: https://doc.rust-lang.org/std/ptr/index.html#safety
[`BitPtr::replace`]: crate::ptr::BitRef::replace
**/
#[inline]
pub unsafe fn replace<O, T>(dst: BitPtr<Mut, O, T>, src: bool) -> bool
where
	O: BitOrder,
	T: BitStore,
{
	dst.replace(src)
}

/** Forms a raw bit-slice from a bit-pointer and a length.

The `len` argument is the number of **bits**, not the number of elements.

This function is safe, but actually using the return value is unsafe. See the
documentation of [`slice::from_raw_parts`] for bit-slice safety requirements.

# Original

[`ptr::slice_from_raw_parts`](core::ptr::slice_from_raw_parts)

# Examples

```rust
use bitvec::ptr;
use bitvec::order::Lsb0;

let x = [5u8, 10, 15];
let bitptr = ptr::BitPtr::<_, Lsb0, _>::from_ref(&x[0]);
let bitslice = ptr::bitslice_from_raw_parts(bitptr, 24);
assert_eq!(unsafe { &*bitslice }[2], true);
```

[`slice::from_raw_parts`]: crate::slice::from_raw_parts
**/
#[inline]
pub fn bitslice_from_raw_parts<O, T>(
	data: BitPtr<Const, O, T>,
	len: usize,
) -> *const BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	unsafe { data.span_unchecked(len) }.to_bitslice_ptr()
}

/** Performs the same functionality as [`bitslice_from_raw_parts`], except that
a raw mutable bit-slice is returned, as opposed to a raw immutable bit-slice.

See the documentation of [`bitslice_from_raw_parts`] for more details.

This function is safe, but actually using the return value is unsafe. See the
documentation of [`slice::from_raw_parts_mut`] for bit-slice safety
requirements.

# Original

[`ptr::slice_from_raw_parts`](core::ptr::slice_from_raw_parts)

# Examples

```rust
use bitvec::ptr;
use bitvec::order::Lsb0;

let mut x = [5u8, 10, 15];
let bitptr = ptr::BitPtr::<_, Lsb0, _>::from_mut(&mut x[0]);
let bitslice = ptr::bitslice_from_raw_parts_mut(bitptr, 24);
unsafe { &mut *bitslice }.set(0, true);
assert!(unsafe { &*bitslice }[0]);
```

[`bitslice_from_raw_parts`]: crate::ptr::bitslice_from_raw_parts
[`slice::from_raw_parts_mut`]: crate::slice::from_raw_parts_mut
**/
#[inline]
pub fn bitslice_from_raw_parts_mut<O, T>(
	data: BitPtr<Mut, O, T>,
	len: usize,
) -> *mut BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	unsafe { data.span_unchecked(len).to_bitslice_ptr_mut() }
}

/** Swaps the values at two mutable locations.

But for the following exception, this function is semantically equivalent to
[`BitRef::swap`]: it operates on raw pointers instead of references. When
references are available, prefer [`BitRef::swap`].

# Original

[`ptr::swap`](core::ptr::swap)

# Safety

Behavior is undefined if any of the following conditions are violated:

- Both `x` and `y` must be [valid] for both reads and writes.
- Both `x` and `y` must point to initialized instances of type `T1` and `T2`,
  respectively.

# Examples

```rust
use bitvec::prelude::*;

let mut data = 2u8;
let x = BitPtr::<_, Lsb0, _>::from_mut(&mut data);
let y = unsafe { x.add(1) };

unsafe {
  bitvec::ptr::swap(x, y);
}
assert_eq!(data, 1);
```

[valid]: https://doc.rust-lang.org/std/ptr/index.html#safety
[`BitRef::swap`]: crate::ptr::BitRef::swap
**/
#[inline]
pub unsafe fn swap<O1, O2, T1, T2>(
	x: BitPtr<Mut, O1, T1>,
	y: BitPtr<Mut, O2, T2>,
) where
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	x.swap(y);
}

/** Swaps `count` bits between the two regions of memory beginning at `x` and
`y`. The two regions must *not* overlap.

# Original

[`ptr::swap_nonoverlapping`](core::ptr::swap_nonoverlapping)

# Safety

Behavior is undefined if any of the following conditions are violated:

- Both `x` and `y` must be [valid] for both reads and writes of `count` bits.
- Both `x` and `y` must be fully initialized instances of `T` for all `count`
  bits.
- The regions may have overlapping elements, but must not overlap the concrete
  bits they describe.

Note that even if `count` is `0`, the pointers must still be validly
constructed, non-null, and well-aligned.

# Examples

```rust
use bitvec::prelude::*;

let mut x = [0u8; 2];
let mut y = !0u16;
let x_ptr = BitPtr::<_, Lsb0, _>::from_mut(&mut x[0]);
let y_ptr = BitPtr::<_, Msb0, _>::from_mut(&mut y);

unsafe {
  bitvec::ptr::swap_nonoverlapping(x_ptr, y_ptr, 16);
}
assert_eq!(x, [!0; 2]);
assert_eq!(y, 0);
```

[valid]: https://doc.rust-lang.org/std/ptr/index.html#safety
**/
#[inline]
pub unsafe fn swap_nonoverlapping<O1, O2, T1, T2>(
	x: BitPtr<Mut, O1, T1>,
	y: BitPtr<Mut, O2, T2>,
	count: usize,
) where
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	for (a, b) in x.range(count).zip(y.range(count)) {
		swap(a, b);
	}
}

/** Overwrites a memory location with the given bit.

Because this reads from memory in order to construct the new value, it cannot be
used to set uninitialized memory. The referent `T` element must be fully
initialized (such as with [`core::ptr::write`]) before setting bits with this
function.

# Original

[`ptr::write`](core::ptr::write)

# Safety

Behavior is undefined if any of the following conditions are violated:

- `dst` must be [valid] for writes
- `dst` must point to a properly initialized value of type `T`
- no other pointer must race `dst` to view or modify the referent location
  unless `T` is capable of ensuring race safety.

# Examples

```rust
use bitvec::prelude::*;

let mut data = 0u8;
let ptr = BitPtr::<_, Lsb0, _>::from_mut(&mut data);
unsafe {
  bitvec::ptr::write(ptr.add(2), true);
}
assert_eq!(data, 4);
```

[valid]: https://doc.rust-lang.org/std/ptr/index.html#safety
[`core::ptr::write`]: core::ptr::write
**/
#[inline]
pub unsafe fn write<O, T>(dst: BitPtr<Mut, O, T>, value: bool)
where
	O: BitOrder,
	T: BitStore,
{
	dst.write(value);
}

/** Performs a volatile write of a memory location with the given bit.

Because processors do not have single-bit write instructions, this must
perform a volatile read of the location, perform the bit modification within
the processor register, then perform a volatile write back to memory. These
three steps are guaranteed to be atomic.

Volatile operations are intended to act on I/O memory, and are guaranteed
not to be elided or reördered by the compiler across other volatile
operations.

# Original

[`ptr::write_volatile`](core::ptr::write_volatile)

# Notes

Rust does not curretnly have a rigorously and formally defined memory model,
so the precise semantics of what “volatile” means here is subject to change
over time. That being said, the semantics will almost always end up pretty
similar to [C11’s definition of volatile][c11].

The compiler shouldn’t change the relative order or number of volatile
memory operations.

# Safety

Behavior is undefined if any of the following conditions are violated:

- `dst` must be [valid] for writes
- no other pointer must race `dst` to view or modify the referent location
  unless `T` is capable of ensuring race safety.

Just like in C, whether an operation is volatile has no bearing whatsoëver
on questions involving concurrent access from multiple threads. Volatile
accesses behave exactly like non-atomic accesses in that regard. In
particular, a race between a `write_volatile` and any other operation
(reading or writing) on the same location is undefined behavior.

This is true even for atomic types! This instruction is an ordinary store
that the compiler will not remove. It is *not* an atomic instruction.

# Examples

```rust
use bitvec::prelude::*;

let mut data = 0u8;
let ptr = BitPtr::<_, Lsb0, _>::from_mut(&mut data);
unsafe {
  bitvec::ptr::write_volatile(ptr, true);
  assert!(bitvec::ptr::read_volatile(ptr.immut()));
}
```

[c11]: http://www.open-std.org/jtc1/sc22/wg14/www/docs/n1570.pdf
[valid]: https://doc.rust-lang.org/core/ptr/index.html#safety
**/
#[inline]
pub unsafe fn write_volatile<O, T>(dst: BitPtr<Mut, O, T>, value: bool)
where
	O: BitOrder,
	T: BitStore,
{
	dst.write_volatile(value);
}

#[cfg(test)]
mod tests;
