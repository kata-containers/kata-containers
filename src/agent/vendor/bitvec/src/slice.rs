/*! A dynamically-sized view into individual bits of a memory region.

You can read the language’s [`slice` module documentation][std] here.

This module defines the [`BitSlice`] region, and all of its associated support
code.

[`BitSlice`] is the primary working type of this crate. It is a wrapper type
over `[T]` which enables you to view, manipulate, and take the address of
individual bits in memory. It behaves in every possible respect exactly like an
ordinary slice: it is dynamically-sized, and must be held by `&` or `&mut`
reference, just like `[T]`, and implements every inherent method and trait that
`[T]` does, to the absolute limits of what Rust permits.

The key to [`BitSlice`]’s powerful capability is that references to it use a
special encoding that store, in addition to the address of the base element and
the bit length, the index of the starting bit in the base element. This custom
reference encoding has some costs in what APIs are possible – for instance, Rust
forbids it from supporting `&mut BitSlice[index] = bool` write indexing – but in
exchange, enables it to be *far* more capable than any other bit-slice crate in
existence.

Because of the volume of code that must be written to match the `[T]` standard
API, this module is organized very differently than the slice implementation in
the [`core`] and [`std`] distribution libraries.

- the root module `slice` contains new APIs that have no counterpart in `[T]`
- `slice/api` contains reïmplementations of the `[T]` inherent methods
- `slice/iter` implements all of the iteration capability
- `slice/ops` implements the traits in `core::ops`
- `slice/proxy` implements the proxy reference used in place of `&mut bool`
- `slice/traits` implements all other traits not in `core::ops`
- lastly, `slice/tests` contains all the unit tests.

[`BitSlice`]: struct.BitSlice.html
[`core`]: core
[`std`]: std
[std]: https://doc.rust-lang.org/stable/std/slice
!*/

use crate::{
	access::{
		BitAccess,
		BitSafe,
	},
	devel as dvl,
	domain::{
		BitDomain,
		BitDomainMut,
		Domain,
		DomainMut,
	},
	index::BitMask,
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
		Msb0,
	},
	ptr::{
		BitPtr,
		BitPtrRange,
		BitRef,
		BitSpan,
		BitSpanError,
	},
	store::BitStore,
};

use core::{
	marker::PhantomData,
	ops::RangeBounds,
	ptr,
	slice,
};

use funty::IsInteger;

#[cfg(feature = "alloc")]
use crate::{
	ptr::Address,
	vec::BitVec,
};

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(feature = "alloc")]
use core::mem::ManuallyDrop;

#[cfg(feature = "alloc")]
use tap::pipe::Pipe;

/** A slice of individual bits, anywhere in memory.

`BitSlice<O, T>` is an unsized region type; you interact with it through
`&BitSlice<O, T>` and `&mut BitSlice<O, T>` references, which work exactly like
all other Rust references. As with the standard slice’s relationship to arrays
and vectors, this is [`bitvec`]’s primary working type, but you will probably
hold it through one of the provided [`BitArray`], [`BitBox`], or [`BitVec`]
containers.

`BitSlice` is conceptually a `[bool]` slice, and provides a nearly complete
mirror of `[bool]`’s API.

Every bit-vector crate can give you an opaque type that hides shift/mask
calculations from you. `BitSlice` does far more than this: it offers you the
full Rust guarantees about reference behavior, including lifetime tracking,
mutability and aliasing awareness, and explicit memory control, *as well as* the
full set of tools and APIs available to the standard `[bool]` slice type.
`BitSlice` can arbitrarily split and subslice, just like `[bool]`. You can write
a linear consuming function and keep the patterns you already know.

For example, to trim all the bits off either edge that match a condition, you
could write

```rust
use bitvec::prelude::*;

fn trim<O: BitOrder, T: BitStore>(
  bits: &BitSlice<O, T>,
  to_trim: bool,
) -> &BitSlice<O, T> {
  let stop = |b: &bool| *b != to_trim;
  let front = bits.iter().by_ref().position(stop).unwrap_or(0);
  let back = bits.iter().by_ref().rposition(stop).unwrap_or(0);
  &bits[front ..= back]
}
# assert_eq!(trim(bits![0, 0, 1, 1, 0, 1, 0], false), bits![1, 1, 0, 1]);
```

to get behavior something like
`trim(&BitSlice[0, 0, 1, 1, 0, 1, 0], false) == &BitSlice[1, 1, 0, 1]`.

# Documentation

All APIs that mirror something in the standard library will have an `Original`
section linking to the corresponding item. All APIs that have a different
signature or behavior than the original will have an `API Differences` section
explaining what has changed, and how to adapt your existing code to the change.

These sections look like this:

# Original

[`slice`](https://doc.rust-lang.org/stable/std/primitive.slice.html)

# API Differences

The slice type `[bool]` has no type parameters. `BitSlice<O, T>` has two: one
for the memory type used as backing storage, and one for the order of bits
within that memory type.

`&BitSlice<O, T>` is capable of producing `&bool` references to read bits out
of its memory, but is not capable of producing `&mut bool` references to write
bits *into* its memory. Any `[bool]` API that would produce a `&mut bool` will
instead produce a [`BitRef<Mut, O, T>`] proxy reference.

# Behavior

`BitSlice` is a wrapper over `[T]`. It describes a region of memory, and must be
handled indirectly. This is most commonly through the reference types
`&BitSlice` and `&mut BitSlice`, which borrow memory owned by some other value
in the program. These buffers can be directly owned by the sibling types
[`BitBox`], which behaves like [`Box<[T]>`](alloc::boxed::Box), and [`BitVec`],
which behaves like [`Vec<T>`]. It cannot be used as the type parameter to a
standard-library-provided handle type.

The `BitSlice` region provides access to each individual bit in the region, as
if each bit had a memory address that you could use to dereference it. It packs
each logical bit into exactly one bit of storage memory, just like
[`std::bitset`] and [`std::vector<bool>`] in C++.

# Type Parameters

`BitSlice` has two type parameters which propagate through nearly every public
API in the crate. These are very important to its operation, and your choice
of type arguments informs nearly every part of this library’s behavior.

## `T: BitStore`

[`BitStore`] is the simpler of the two parameters. It refers to the integer type
used to hold bits. It must be one of the Rust unsigned integer fundamentals:
`u8`, `u16`, `u32`, `usize`, and on 64-bit systems only, `u64`. In addition, it
can also be an alias-safed wrapper over them (see the [`access`] module) in
order to permit bit-slices to share underlying memory without interfering with
each other.

`BitSlice` references can only be constructed over the integers, not over their
aliasing wrappers. `BitSlice` will only use aliasing types in its `T` slots when
you invoke APIs that produce them, such as [`.split_at_mut()`].

The default type argument is `usize`.

The argument you choose is used as the basis of a `[T]` slice, over which the
`BitSlice` view type is placed. `BitSlice<_, T>` is subject to all of the rules
about alignment that `[T]` is. If you are working with in-memory representation
formats, chances are that you already have a `T` type with which you’ve been
working, and should use it here.

If you are only using this crate to discard the seven wasted bits per `bool`
of a collection of `bool`s, and are not too concerned about the in-memory
representation, then you should use the default type argument of `usize`. This
is because most processors work best when moving an entire `usize` between
memory and the processor itself, and using a smaller type may cause it to slow
down.

## `O: BitOrder`

[`BitOrder`] is the more complex parameter. It has a default argument which,
like `usize`, is the good-enough choice when you do not explicitly need to
control the representation of bits in memory.

This parameter determines how to index the bits within a single memory element
`T`. Computers all agree that in a slice of elements `T`, the element with the
lower index has a lower memory address than the element with the higher index.
But the individual bits within an element do not have addresses, and so there is
no uniform standard of which bit is the zeroth, which is the first, which is the
penultimate, and which is the last.

To make matters even more confusing, there are two predominant ideas of
in-element ordering that often *correlate* with the in-element *byte* ordering
of integer types, but are in fact wholly unrelated! [`bitvec`] provides these
two main orders as types for you, and if you need a different one, it also
provides the tools you need to make your own.

### Least Significant Bit Comes First

This ordering, named the [`Lsb0`] type, indexes bits within an element by
placing the `0` index at the least significant bit (numeric value `1`) and the
final index at the most significant bit (numeric value [`T::MIN`][minval] for
signed integers on most machines).

For example, this is the ordering used by most C compilers to lay out bit-field
struct members on little-endian **byte**-ordered machines.

### Most Significant Bit Comes First

This ordering, named the [`Msb0`] type, indexes bits within an element by
placing the `0` index at the most significant bit (numeric value
[`T::MIN`][minval] for most signed integers) and the final index at the least
significant bit (numeric value `1`).

For example, this is the ordering used by the [TCP wire format], and by most C
compilers to lay out bit-field struct members on big-endian **byte**-ordered
machines.

### Default Ordering

The default ordering is [`Lsb0`], as it typically produces shorter object code
than [`Msb0`] does. If you are implementing a collection, then `Lsb0` is likely
the more performant ordering; if you are implementing a buffer protocol, then
your choice of ordering is dictated by the protocol definition.

# Safety

`BitSlice` is designed to never introduce new memory unsafety that you did not
provide yourself, either before or during the use of this crate. Bugs do, and
have, occured, and you are encouraged to submit any discovered flaw as a defect
report.

The `&BitSlice` reference type uses a private encoding scheme to hold all the
information needed in its stack value. This encoding is **not** part of the
public API of the library, and is not binary-compatible with `&[T]`.
Furthermore, in order to satisfy Rust’s requirements about alias conditions,
`BitSlice` performs type transformations on the `T` parameter to ensure that it
never creates the potential for undefined behavior.

You must never attempt to type-cast a reference to `BitSlice` in any way. You
must not use [`mem::transmute`] with `BitSlice` anywhere in its type arguments.
You must not use `as`-casting to convert between `*BitSlice` and any other type.
You must not attempt to modify the binary representation of a `&BitSlice`
reference value. These actions will all lead to runtime memory unsafety, are
(hopefully) likely to induce a program crash, and may possibly cause undefined
behavior at compile-time.

Everything in the `BitSlice` public API, even the `unsafe` parts, are guaranteed
to have no more unsafety than their equivalent parts in the standard library.
All `unsafe` APIs will have documentation explicitly detailing what the API
requires you to uphold in order for it to function safely and correctly. All
safe APIs will do so themselves.

# Performance

Like the standard library’s `[T]` slice, `BitSlice` is designed to be very easy
to use safely, while supporting `unsafe` when necessary. Rust has a powerful
optimizing engine, and `BitSlice` will frequently be compiled to have zero
runtime cost. Where it is slower, it will not be significantly slower than a
manual replacement.

As the machine instructions operate on registers rather than bits, your choice
of [`T: BitStore`] type parameter can influence your slice’s performance. Using
larger register types means that slices can gallop over completely-filled
interior elements faster, while narrower register types permit more graceful
handling of subslicing and aliased splits.

# Construction

`BitSlice` views of memory can be constructed over borrowed data in a number of
ways. As this is a reference-only type, it can only ever be built by borrowing
an existing memory buffer and taking temporary control of your program’s view of
the region.

## Macro Constructor

`BitSlice` buffers can be constructed at compile-time through the [`bits!`]
macro. This macro accepts a superset of the [`vec!`] arguments, and creates an
appropriate buffer in the local scope. The macro expands to a borrowed
[`BitArray`] temporary; currently, it cannot be assigned to a `static` binding.

```rust
use bitvec::prelude::*;

let immut = bits![Lsb0, u8; 0, 1, 0, 0, 1, 0, 0, 1];
let mutable: &mut BitSlice<_, _> = bits![mut Msb0, u8; 0; 8];

assert_ne!(immut, mutable);
mutable.clone_from_bitslice(immut);
assert_eq!(immut, mutable);
```

## Borrowing Constructors

The functions [`from_element`], [`from_element_mut`], [`from_slice`], and
[`from_slice_mut`] take references to existing memory, and construct
`BitSlice` references over them. These are the most basic ways to borrow memory
and view it as bits.

```rust
use bitvec::prelude::*;

let data = [0u16; 3];
let local_borrow = BitSlice::<Lsb0, _>::from_slice(&data);

let mut data = [0u8; 5];
let local_mut = BitSlice::<Lsb0, _>::from_slice_mut(&mut data);
```

## Trait Method Constructors

The [`BitView`] trait implements [`.view_bits::<O>()`] and
[`.view_bits_mut::<O>()`] methods on elements, arrays not larger than 64
elements, and slices. This trait, imported in the crate prelude, is *probably*
the easiest way for you to borrow memory.

```rust
use bitvec::prelude::*;

let data = [0u32; 5];
let trait_view = data.view_bits::<Lsb0>();

let mut data = 0usize;
let trait_mut = data.view_bits_mut::<Msb0>();
```

## Owned Bit Slices

If you wish to take ownership of a memory region and enforce that it is always
viewed as a `BitSlice` by default, you can use one of the [`BitArray`],
[`BitBox`], or [`BitVec`] types, rather than pairing ordinary buffer types with
the borrowing constructors.

```rust
use bitvec::prelude::*;

let slice = bits![0; 27];
let array = bitarr![LocalBits, u8; 0; 10];
# #[cfg(feature = "alloc")] fn allocs() {
let boxed = bitbox![0; 10];
let vec = bitvec![0; 20];
# } #[cfg(feature = "alloc")] allocs();

// arrays always round up
assert_eq!(array.as_bitslice(), slice[.. 16]);
# #[cfg(feature = "alloc")] fn allocs2() {
# let slice = bits![0; 27];
# let boxed = bitbox![0; 10];
# let vec = bitvec![0; 20];
assert_eq!(boxed.as_bitslice(), slice[.. 10]);
assert_eq!(vec.as_bitslice(), slice[.. 20]);
# } #[cfg(feature = "alloc")] allocs2();
```

[TCP wire format]: https://en.wikipedia.org/wiki/Transmission_Control_Protocol#TCP_segment_structure
[minval]: https://doc.rust-lang.org/stable/std/primitive.usize.html#associatedconstant.MIN

[`BitArray`]: crate::array::BitArray
[`BitBox`]: crate::boxed::BitBox
[`BitRef<Mut, O, T>`]: crate::ptr::BitRef
[`BitOrder`]: crate::order::BitOrder
[`BitStore`]: crate::store::BitStore
[`BitVec`]: crate::vec::BitVec
[`BitView`]: crate::view::BitView
[`Cell<T>`]: core::cell::Cell
[`Lsb0`]: crate::order::Lsb0
[`Msb0`]: crate::order::Msb0
[`T: BitStore`]: crate::store::BitStore
[`Vec<T>`]: alloc::vec::Vec

[`access`]: crate::access
[`bits!`]: macro@crate::bits
[`bitvec`]: crate
[`bitvec::prelude::LocalBits`]: crate::order::LocalBits
[`from_element`]: Self::from_element
[`from_element_mut`]: Self::from_element_mut
[`from_slice`]: Self::from_slice
[`from_slice_mut`]: Self::from_slice_mut
[`mem::transmute`]: core::mem::transmute
[`std::bitset`]: https://en.cppreference.com/w/cpp/utility/bitset
[`std::vector<bool>`]: https://en.cppreference.com/w/cpp/container/vector_bool
[`vec!`]: macro@alloc::vec

[`.split_at_mut()`]: Self::split_at_mut
[`.view_bits::<O>()`]: crate::view::BitView::view_bits
[`.view_bits_mut::<O>()`]: crate::view::BitView::view_bits_mut
**/
#[repr(transparent)]
pub struct BitSlice<O = Lsb0, T = usize>
where
	O: BitOrder,
	T: BitStore,
{
	/// The ordering of bits within a register `T`.
	_ord: PhantomData<O>,
	/// The register type used for storage.
	_typ: PhantomData<[T]>,
	/// Indicate that this is a newtype wrapper over a wholly-untyped slice.
	///
	/// This is necessary in order for the Rust compiler to remove restrictions
	/// on the possible values of references to this slice `&BitSlice` and
	/// `&mut BitSlice`.
	///
	/// Rust has firm requirements that *any* reference that is directly usable
	/// to dereference a real value must conform to its rules about address
	/// liveness, type alignment, and for slices, trustworthy length. It is
	/// undefined behavior for a slice reference *to a dereferencable type* to
	/// violate any of these restrictions.
	///
	/// However, the value of a reference to a zero-sized type has *no* such
	/// restrictions, because that reference can never perform direct memory
	/// access. The compiler will accept any value in a slot typed as `&[()]`,
	/// because the values in it will never be used for a load or store
	/// instruction. If this were `[T]`, then Rust would make the pointer
	/// encoding used to manage values of `&BitSlice` become undefined behavior.
	///
	/// See the `ptr` module for information on the encoding used.
	_mem: [()],
}

/// General-purpose functions not present on `[T]`.
impl<O, T> BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// Constructs a shared `&BitSlice` reference over a shared element.
	///
	/// The [`BitView`] trait, implemented on all [`BitStore`] implementors,
	/// provides a method [`.view_bits::<O>()`] which delegates to this function
	/// and may be more convenient for you to write.
	///
	/// # Parameters
	///
	/// - `elem`: A shared reference to a memory element.
	///
	/// # Returns
	///
	/// A shared `&BitSlice` over the `elem` element.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let elem = 0u8;
	/// let bits = BitSlice::<Lsb0, _>::from_element(&elem);
	/// assert_eq!(bits.len(), 8);
	/// ```
	///
	/// [`BitStore`]: crate::store::BitStore
	/// [`BitView`]: crate::view::BitView
	/// [`.view_bits::<O>()`]: crate::view::BitView::view_bits
	pub fn from_element(elem: &T) -> &Self {
		unsafe { BitPtr::from_ref(elem).span_unchecked(T::Mem::BITS as usize) }
			.to_bitslice_ref()
	}

	/// Constructs an exclusive `&mut BitSlice` reference over an element.
	///
	/// The [`BitView`] trait, implemented on all [`BitStore`] implementors,
	/// provides a method [`.view_bits_mut::<O>()`] which delegates to this
	/// function and may be more convenient for you to write.
	///
	/// # Parameters
	///
	/// - `elem`: An exclusive reference to a memory element.
	///
	/// # Returns
	///
	/// An exclusive `&mut BitSlice` over the `elem` element.
	///
	/// Note that the original `elem` reference will be inaccessible for the
	/// duration of the returned slice handle’s lifetime.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut elem = 0u16;
	/// let bits = BitSlice::<Msb0, _>::from_element_mut(&mut elem);
	/// bits.set(15, true);
	/// assert!(bits.get(15).unwrap());
	/// assert_eq!(elem, 1);
	/// ```
	///
	/// [`BitStore`]: crate::store::BitStore
	/// [`BitView`]: crate::view::BitView
	/// [`.view_bits_mut::<O>()`]: crate::view::BitView::view_bits_mut
	pub fn from_element_mut(elem: &mut T) -> &mut Self {
		unsafe { BitPtr::from_mut(elem).span_unchecked(T::Mem::BITS as usize) }
			.to_bitslice_mut()
	}

	/// Constructs a shared `&BitSlice` reference over a slice.
	///
	/// The [`BitView`] trait, implemented on all `[T]` slices, provides a
	/// method [`.view_bits::<O>()`] which delegates to this function and may be
	/// more convenient for you to write.
	///
	/// # Parameters
	///
	/// - `slice`: A shared reference over a sequence of memory elements.
	///
	/// # Returns
	///
	/// A `&BitSlice` view of the provided slice. The error condition is only
	/// encountered if the source slice is too long to be encoded in a
	/// `&BitSlice` handle, but such a slice is likely impossible to produce
	/// without causing errors long before calling this function.
	///
	/// # Conditions
	///
	/// The produced `&BitSlice` handle always begins at the zeroth bit of the
	/// zeroth element in `slice`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let slice = &[0u8, 1];
	/// let bits = BitSlice::<Msb0, _>::from_slice(slice).unwrap();
	/// assert!(bits[15]);
	/// ```
	///
	/// An example showing this function failing would require a slice exceeding
	/// `!0usize >> 3` bytes in size, which is infeasible to produce.
	///
	/// [`BitView`]: crate::view::BitView
	/// [`MAX_ELTS`]: Self::MAX_ELTS
	/// [`.view_bits::<O>()`]: crate::view::BitView::view_bits
	pub fn from_slice(slice: &[T]) -> Result<&Self, BitSpanError<T>> {
		let elts = slice.len();
		//  Starting at the zeroth bit makes this counter an exclusive cap, not
		//  an inclusive cap. This is also pretty much impossible to hit.
		if elts >= Self::MAX_ELTS {
			return Err(BitSpanError::TooLong(
				elts.saturating_mul(T::Mem::BITS as usize),
			));
		}
		Ok(unsafe { Self::from_slice_unchecked(slice) })
	}

	/// Constructs an exclusive `&mut BitSlice` reference over a slice.
	///
	/// The [`BitView`] trait, implemented on all `[T]` slices, provides a
	/// method [`.view_bits_mut::<O>()`] which delegates to this function and
	/// may be more convenient for you to write.
	///
	/// # Parameters
	///
	/// - `slice`: An exclusive reference over a sequence of memory elements.
	///
	/// # Returns
	///
	/// A `&mut BitSlice` view of the provided slice. The error condition is
	/// only encountered if the source slice is too long to be encoded in a
	/// `&mut BitSlice` handle, but such a slice is likely impossible to produce
	/// without causing errors long before calling this function.
	///
	/// Note that the original `slice` reference will be inaccessible for the
	/// duration of the returned slice handle’s lifetime.
	///
	/// # Conditions
	///
	/// The produced `&mut BitSlice` handle always begins at the zeroth bit of
	/// the zeroth element in `slice`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut slice = [0u8; 2];
	/// let bits = BitSlice::<Lsb0, _>::from_slice_mut(&mut slice).unwrap();
	///
	/// assert!(!bits[0]);
	/// bits.set(0, true);
	/// assert!(bits[0]);
	/// assert_eq!(slice[0], 1);
	/// ```
	///
	/// This example attempts to construct a `&mut BitSlice` handle from a slice
	/// that is too large to index. Either the `vec!` allocation will fail, or
	/// the bit-slice constructor will fail.
	///
	/// ```rust,should_panic
	/// # #[cfg(feature = "alloc")] {
	/// use bitvec::prelude::*;
	///
	/// let mut data = vec![0usize; BitSlice::<Lsb0, usize>::MAX_ELTS];
	/// let bits = BitSlice::<Lsb0, _>::from_slice_mut(&mut data[..]).unwrap();
	/// # }
	/// # #[cfg(not(feature = "alloc"))] panic!("No allocator present");
	/// ```
	///
	/// [`BitView`]: crate::view::BitView
	/// [`MAX_ELTS`]: Self::MAX_ELTS
	/// [`.view_bits_mut::<O>()`]: crate::view::BitView::view_bits_mut
	pub fn from_slice_mut(
		slice: &mut [T],
	) -> Result<&mut Self, BitSpanError<T>> {
		let elts = slice.len();
		if elts >= Self::MAX_ELTS {
			return Err(BitSpanError::TooLong(
				elts.saturating_mul(T::Mem::BITS as usize),
			));
		}
		Ok(unsafe { Self::from_slice_unchecked_mut(slice) })
	}

	/// Converts a slice reference into a `BitSlice` reference without checking
	/// that its size can be safely used.
	///
	/// # Safety
	///
	/// If the `slice` length is longer than [`MAX_ELTS`], then the returned
	/// `BitSlice` will have its length severely truncated. This is not a safety
	/// violation, but it is behavior that callers must avoid to remain correct.
	///
	/// Prefer [`::from_slice()`].
	///
	/// [`MAX_ELTS`]: Self::MAX_ELTS
	/// [`::from_slice()`]: Self::from_slice
	pub unsafe fn from_slice_unchecked(slice: &[T]) -> &Self {
		let bits = slice.len().wrapping_mul(T::Mem::BITS as usize);
		BitPtr::from_slice(slice)
			.span_unchecked(bits)
			.to_bitslice_ref()
	}

	/// Converts a slice reference into a `BitSlice` reference without checking
	/// that its size can be safely used.
	///
	/// # Safety
	///
	/// If the `slice` length is longer than [`MAX_ELTS`], then the returned
	/// `BitSlice` will have its length severely truncated. This is not a safety
	/// violation, but it is behavior that callers must avoid to remain correct.
	///
	/// Prefer [`::from_slice_mut()`].
	///
	/// [`MAX_ELTS`]: Self::MAX_ELTS
	/// [`::from_slice_mut()`]: Self::from_slice_mut
	pub unsafe fn from_slice_unchecked_mut(slice: &mut [T]) -> &mut Self {
		let bits = slice.len().wrapping_mul(T::Mem::BITS as usize);
		BitPtr::from_mut_slice(slice)
			.span_unchecked(bits)
			.to_bitslice_mut()
	}

	/// Produces the empty slice reference.
	///
	/// This is equivalent to `&[]` for ordinary slices.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits: &BitSlice = BitSlice::empty();
	/// assert!(bits.is_empty());
	/// ```
	pub fn empty<'a>() -> &'a Self {
		BitSpan::<Const, O, T>::EMPTY.to_bitslice_ref()
	}

	/// Produces the empty mutable slice reference.
	///
	/// This is equivalent to `&mut []` for ordinary slices.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits: &mut BitSlice = BitSlice::empty_mut();
	/// assert!(bits.is_empty());
	/// ```
	pub fn empty_mut<'a>() -> &'a mut Self {
		BitSpan::EMPTY.to_bitslice_mut()
	}

	/// Writes a new bit at a given index.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `index`: The bit index at which to write. It must be in the range `0
	///   .. self.len()`.
	/// - `value`: The value to be written; `true` for `1` or `false` for `0`.
	///
	/// # Effects
	///
	/// If `index` is valid, then the bit to which it refers is set to `value`.
	///
	/// # Panics
	///
	/// This method panics if `index` is not less than [`self.len()`].
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![mut 0];
	///
	/// assert!(!bits[0]);
	/// bits.set(0, true);
	/// assert!(bits[0]);
	/// ```
	///
	/// This example panics when it attempts to set a bit that is out of bounds.
	///
	/// ```rust,should_panic
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![mut 0];
	/// bits.set(1, false);
	/// ```
	///
	/// [`self.len()`]: Self::len
	pub fn set(&mut self, index: usize, value: bool) {
		self.assert_in_bounds(index);
		unsafe {
			self.set_unchecked(index, value);
		}
	}

	/// Writes a new bit at a given index.
	///
	/// This method supports writing through a shared reference to a bit that
	/// may be observed by other `BitSlice` handles. It is only present when the
	/// `T` type parameter supports such shared mutation (measured by the
	/// [`Radium`] trait).
	///
	/// # Parameters
	///
	/// - `&self`
	/// - `index`: The bit index at which to write. It must be in the range `0
	///   .. self.len()`.
	/// - `value`: The value to be written; `true` for `1` or `false` for `0`.
	///
	/// # Effects
	///
	/// If `index` is valid, then the bit to which it refers is set to `value`.
	/// If `T` is an [atomic], this will lock the memory bus for the referent
	/// address, and may cause stalls.
	///
	/// # Panics
	///
	/// This method panics if `index` is not less than [`self.len()`].
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	/// use core::cell::Cell;
	///
	/// let byte = Cell::new(0u8);
	/// let bits = byte.view_bits::<Msb0>();
	/// let bits_2 = bits;
	///
	/// bits.set_aliased(1, true);
	/// assert!(bits_2[1]);
	/// ```
	///
	/// This example panics when it attempts to set a bit that is out of bounds.
	///
	/// ```rust,should_panic
	/// use bitvec::prelude::*;
	/// use core::cell::Cell;
	///
	/// let byte = Cell::new(0u8);
	/// let bits = byte.view_bits::<Lsb0>();
	/// bits.set_aliased(8, false);
	/// ```
	///
	/// [atomic]: core::sync::atomic
	/// [`Radium`]: radium::Radium
	/// [`self.len()`]: Self::len
	pub fn set_aliased(&self, index: usize, value: bool)
	where T: radium::Radium {
		self.assert_in_bounds(index);
		unsafe {
			self.set_aliased_unchecked(index, value);
		}
	}

	/// Tests if *any* bit in the slice is set (logical `∨`).
	///
	/// # Truth Table
	///
	/// ```text
	/// 0 0 => 0
	/// 0 1 => 1
	/// 1 0 => 1
	/// 1 1 => 1
	/// ```
	///
	/// # Parameters
	///
	/// - `&self`
	///
	/// # Returns
	///
	/// Whether any bit in the slice domain is set. The empty slice returns
	/// `false`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![0, 1, 0, 0];
	/// assert!(bits[.. 2].any());
	/// assert!(!bits[2 ..].any());
	/// ```
	pub fn any(&self) -> bool {
		match self.domain() {
			Domain::Enclave { head, elem, tail } => {
				O::mask(head, tail) & elem.load_value() != BitMask::ZERO
			},
			Domain::Region { head, body, tail } => {
				head.map_or(false, |(head, elem)| {
					O::mask(head, None) & elem.load_value() != BitMask::ZERO
				}) || body.iter().any(|e| e.load_value() != T::Mem::ZERO)
					|| tail.map_or(false, |(elem, tail)| {
						O::mask(None, tail) & elem.load_value() != BitMask::ZERO
					})
			},
		}
	}

	/// Tests if *all* bits in the slice domain are set (logical `∧`).
	///
	/// # Truth Table
	///
	/// ```text
	/// 0 0 => 0
	/// 0 1 => 0
	/// 1 0 => 0
	/// 1 1 => 1
	/// ```
	///
	/// # Parameters
	///
	/// - `&self`
	///
	/// # Returns
	///
	/// Whether all bits in the slice domain are set. The empty slice returns
	/// `true`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![1, 1, 0, 1];
	/// assert!(bits[.. 2].all());
	/// assert!(!bits[2 ..].all());
	/// ```
	pub fn all(&self) -> bool {
		match self.domain() {
			Domain::Enclave { head, elem, tail } => {
				/* Due to a bug in `rustc`, calling `.value()` on the two
				`BitMask` types, to use `T::Mem | T::Mem == T::Mem`, causes type
				resolution failure and only discovers the
				`for<'a> BitOr<&'a Self>` implementation in the trait bounds
				`T::Mem: BitMemory: IsUnsigned: BitOr<Self> + for<'a> BitOr<&'a Self>`.

				Until this is fixed, routing through the `BitMask`
				implementation suffices. The by-val and by-ref operator traits
				are at the same position in the bounds chain, making this quite
				a strange bug.
				*/
				!O::mask(head, tail) | elem.load_value() == BitMask::ALL
			},
			Domain::Region { head, body, tail } => {
				head.map_or(true, |(head, elem)| {
					!O::mask(head, None) | elem.load_value() == BitMask::ALL
				}) && body
					.iter()
					.map(BitStore::load_value)
					.all(|e| e == T::Mem::ALL)
					&& tail.map_or(true, |(elem, tail)| {
						!O::mask(None, tail) | elem.load_value() == BitMask::ALL
					})
			},
		}
	}

	/// Tests if *all* bits in the slice are unset (logical `¬∨`).
	///
	/// # Truth Table
	///
	/// ```text
	/// 0 0 => 1
	/// 0 1 => 0
	/// 1 0 => 0
	/// 1 1 => 0
	/// ```
	///
	/// # Parameters
	///
	/// - `&self`
	///
	/// # Returns
	///
	/// Whether all bits in the slice domain are unset.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![0, 1, 0, 0];
	/// assert!(!bits[.. 2].not_any());
	/// assert!(bits[2 ..].not_any());
	/// ```
	pub fn not_any(&self) -> bool {
		!self.any()
	}

	/// Tests if *any* bit in the slice is unset (logical `¬∧`).
	///
	/// # Truth Table
	///
	/// ```text
	/// 0 0 => 1
	/// 0 1 => 1
	/// 1 0 => 1
	/// 1 1 => 0
	/// ```
	///
	/// # Parameters
	///
	/// - `&self`
	///
	/// # Returns
	///
	/// Whether any bit in the slice domain is unset.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![1, 1, 0, 1];
	/// assert!(!bits[.. 2].not_all());
	/// assert!(bits[2 ..].not_all());
	/// ```
	pub fn not_all(&self) -> bool {
		!self.all()
	}

	/// Tests whether the slice has some, but not all, bits set and some, but
	/// not all, bits unset.
	///
	/// This is `false` if either [`.all()`] or [`.not_any()`] are `true`.
	///
	/// # Truth Table
	///
	/// ```text
	/// 0 0 => 0
	/// 0 1 => 1
	/// 1 0 => 1
	/// 1 1 => 0
	/// ```
	///
	/// # Parameters
	///
	/// - `&self`
	///
	/// # Returns
	///
	/// Whether the slice domain has mixed content. The empty slice returns
	/// `false`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let data = 0b111_000_10u8;
	/// let bits = bits![1, 1, 0, 0, 1, 0];
	///
	/// assert!(!bits[.. 2].some());
	/// assert!(!bits[2 .. 4].some());
	/// assert!(bits.some());
	/// ```
	///
	/// [`.all()`]: Self::all
	/// [`.not_any()`]: Self::not_any
	pub fn some(&self) -> bool {
		self.any() && self.not_all()
	}

	/// Counts the number of bits set to `1` in the slice contents.
	///
	/// # Parameters
	///
	/// - `&self`
	///
	/// # Returns
	///
	/// The number of bits in the slice domain that are set to `1`.
	///
	/// # Examples
	///
	/// Basic usage:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![1, 1, 0, 0];
	/// assert_eq!(bits[.. 2].count_ones(), 2);
	/// assert_eq!(bits[2 ..].count_ones(), 0);
	/// ```
	pub fn count_ones(&self) -> usize {
		match self.domain() {
			Domain::Enclave { head, elem, tail } => (O::mask(head, tail)
				& elem.load_value())
			.value()
			.count_ones() as usize,
			Domain::Region { head, body, tail } => {
				head.map_or(0, |(head, elem)| {
					(O::mask(head, None) & elem.load_value())
						.value()
						.count_ones() as usize
				}) + body
					.iter()
					.map(BitStore::load_value)
					.map(|e| e.count_ones() as usize)
					.sum::<usize>() + tail.map_or(0, |(elem, tail)| {
					(O::mask(None, tail) & elem.load_value())
						.value()
						.count_ones() as usize
				})
			},
		}
	}

	/// Counts the number of bits cleared to `0` in the slice contents.
	///
	/// # Parameters
	///
	/// - `&self`
	///
	/// # Returns
	///
	/// The number of bits in the slice domain that are cleared to `0`.
	///
	/// # Examples
	///
	/// Basic usage:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![1, 1, 0, 0];
	/// assert_eq!(bits[.. 2].count_zeros(), 0);
	/// assert_eq!(bits[2 ..].count_zeros(), 2);
	/// ```
	pub fn count_zeros(&self) -> usize {
		match self.domain() {
			Domain::Enclave { head, elem, tail } => (!O::mask(head, tail)
				| elem.load_value())
			.value()
			.count_zeros() as usize,
			Domain::Region { head, body, tail } => {
				head.map_or(0, |(head, elem)| {
					(!O::mask(head, None) | elem.load_value())
						.value()
						.count_zeros() as usize
				}) + body
					.iter()
					.map(BitStore::load_value)
					.map(|e| e.count_zeros() as usize)
					.sum::<usize>() + tail.map_or(0, |(elem, tail)| {
					(!O::mask(None, tail) | elem.load_value())
						.value()
						.count_zeros() as usize
				})
			},
		}
	}

	/// Enumerates all bits in a `BitSlice` that are set to `1`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![0, 1, 0, 0, 1, 0, 0, 0, 1];
	/// let mut indices = [1, 4, 8].iter().copied();
	///
	/// let mut iter_ones = bits.iter_ones();
	/// let mut compose = bits.iter()
	///   .copied()
	///   .enumerate()
	///   .filter_map(|(idx, bit)| if bit { Some(idx) } else { None });
	///
	/// for ((a, b), c) in iter_ones.zip(compose).zip(indices) {
	///   assert_eq!(a, b);
	///   assert_eq!(b, c);
	/// }
	/// ```
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn iter_ones(&self) -> IterOnes<O, T> {
		IterOnes::new(self)
	}

	/// Enumerates all bits in a `BitSlice` that are cleared to `0`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![1, 0, 1, 1, 0, 1, 1, 1, 0];
	/// let mut indices = [1, 4, 8].iter().copied();
	///
	/// let mut iter_zeros = bits.iter_zeros();
	/// let mut compose = bits.iter()
	///   .copied()
	///   .enumerate()
	///   .filter_map(|(idx, bit)| if !bit { Some(idx) } else { None });
	///
	/// for ((a, b), c) in iter_zeros.zip(compose).zip(indices) {
	///   assert_eq!(a, b);
	///   assert_eq!(b, c);
	/// }
	/// ```
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn iter_zeros(&self) -> IterZeros<O, T> {
		IterZeros::new(self)
	}

	/// Gets the index of the first bit in the bit-slice set to `1`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// assert!(bits![].first_one().is_none());
	/// assert_eq!(bits![0, 0, 1].first_one().unwrap(), 2);
	/// ```
	#[inline]
	pub fn first_one(&self) -> Option<usize> {
		self.iter_ones().next()
	}

	/// Gets the index of the first bit in the bit-slice set to `0`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// assert!(bits![].first_zero().is_none());
	/// assert_eq!(bits![1, 1, 0].first_zero().unwrap(), 2);
	/// ```
	#[inline]
	pub fn first_zero(&self) -> Option<usize> {
		self.iter_zeros().next()
	}

	/// Gets the index of the last bit in the bit-slice set to `1`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// assert!(bits![].last_one().is_none());
	/// assert_eq!(bits![1, 0, 0, 1].last_one().unwrap(), 3);
	/// ```
	#[inline]
	pub fn last_one(&self) -> Option<usize> {
		self.iter_ones().next_back()
	}

	/// Gets the index of the last bit in the bit-slice set to `0`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// assert!(bits![].last_zero().is_none());
	/// assert_eq!(bits![0, 1, 1, 0].last_zero().unwrap(), 3);
	/// ```
	#[inline]
	pub fn last_zero(&self) -> Option<usize> {
		self.iter_zeros().next_back()
	}

	/// Counts the number of bits from the start of the bit-slice to the first
	/// bit set to `0`.
	///
	/// This returns `0` if the bit-slice is empty.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// assert_eq!(bits![].leading_ones(), 0);
	/// assert_eq!(bits![0].leading_ones(), 0);
	/// assert_eq!(bits![1, 0, 1, 1].leading_ones(), 1);
	/// assert_eq!(bits![1, 1, 1, 1].leading_ones(), 4);
	/// ```
	#[inline]
	pub fn leading_ones(&self) -> usize {
		self.first_zero().unwrap_or(self.len())
	}

	/// Counts the number of bits from the start of the bit-slice to the first
	/// bit set to `1`.
	///
	/// This returns `0` if the bit-slice is empty.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// assert_eq!(bits![].leading_zeros(), 0);
	/// assert_eq!(bits![1].leading_zeros(), 0);
	/// assert_eq!(bits![0, 1, 0, 0].leading_zeros(), 1);
	/// assert_eq!(bits![0, 0, 0, 0].leading_zeros(), 4);
	/// ```
	#[inline]
	pub fn leading_zeros(&self) -> usize {
		self.first_one().unwrap_or(self.len())
	}

	/// Counts the number of bits from the end of the bit-slice to the last bit
	/// set to `0`.
	///
	/// This returns `0` if the bit-slice is empty.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// assert_eq!(bits![].trailing_ones(), 0);
	/// assert_eq!(bits![0].trailing_ones(), 0);
	/// assert_eq!(bits![1, 0, 1, 1].trailing_ones(), 2);
	/// ```
	#[inline]
	pub fn trailing_ones(&self) -> usize {
		let len = self.len();
		self.last_zero().map(|idx| len - 1 - idx).unwrap_or(len)
	}

	/// Counts the number of bits from the end of the bit-slice to the last bit
	/// set to `1`.
	///
	/// This returns `0` if the bit-slice is empty.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// assert_eq!(bits![].trailing_zeros(), 0);
	/// assert_eq!(bits![1].trailing_zeros(), 0);
	/// assert_eq!(bits![0, 1, 0, 0].trailing_zeros(), 2);
	/// ```
	#[inline]
	pub fn trailing_zeros(&self) -> usize {
		let len = self.len();
		self.last_one().map(|idx| len - 1 - idx).unwrap_or(len)
	}

	/// Copies the bits from `src` into `self`.
	///
	/// The length of `src` must be the same as `self.
	///
	/// If `src` has the same type arguments as `self`, it can be more
	/// performant to use [`.copy_from_bitslice()`].
	///
	/// # Original
	///
	/// [`slice::clone_from_bitslice`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.clone_from_bitslice)
	///
	/// # API Differences
	///
	/// This method is renamed, as it takes a bit slice rather than an element
	/// slice.
	///
	/// # Panics
	///
	/// This function will panic if the two slices have different lengths.
	///
	/// # Examples
	///
	/// Cloning two bits from a slice into another:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let src = bits![Msb0, u16; 1; 4];
	/// let dst = bits![mut Lsb0, u8; 0; 2];
	///
	/// dst.clone_from_bitslice(&src[2 ..]);
	/// assert_eq!(dst, bits![1; 2]);
	/// ```
	///
	/// Rust enforces that there can only be one mutable reference with no
	/// immutable references to a particular piece of data in a particular
	/// scope. Because of this, attempting to use clone_from_slice on a single
	/// slice will result in a compile failure:
	///
	/// ```rust,compile_fail
	/// use bitvec::prelude::*;
	///
	/// let slice = bits![mut 0, 0, 0, 1, 1];
	/// slice[.. 2].clone_from_bitslice(&slice[3 ..]); // compile fail!
	/// ```
	///
	/// To work around this, we can use [`.split_at_mut()`] to create two
	/// distinct sub-slices from a slice:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let slice = bits![mut 0, 0, 0, 1, 1];
	///
	/// {
	///   let (left, right) = slice.split_at_mut(2);
	///   left.clone_from_bitslice(&right[1 ..]);
	/// }
	///
	/// assert_eq!(slice, bits![1, 1, 0, 1, 1]);
	/// ```
	///
	/// # Performance
	///
	/// If `self` and `src` use the same type arguments, this specializes to
	/// [`.copy_from_bitslice()`]; if you know statically that this is the case,
	/// prefer to call that method directly and avoid the cost of detection at
	/// runtime. Otherwise, this is a bit-by-bit crawl across both slices, which
	/// is a slow process.
	///
	/// [`.copy_from_bitslice()`]: Self::copy_from_bitslice
	/// [`.split_at_mut()`]: Self::split_at_mut
	pub fn clone_from_bitslice<O2, T2>(&mut self, src: &BitSlice<O2, T2>)
	where
		O2: BitOrder,
		T2: BitStore,
	{
		assert_eq!(
			self.len(),
			src.len(),
			"Cloning between slices requires equal lengths"
		);

		if dvl::match_types::<O, T, O2, T2>() {
			let that = src as *const _ as *const _;
			unsafe {
				self.copy_from_bitslice(&*that);
			}
		}
		else {
			for (to, from) in unsafe { self.iter_mut().remove_alias() }
				.zip(src.iter().by_val())
			{
				to.set(from);
			}
		}
	}

	/// Copies all bits from `src` into `self`, using a memcpy wherever
	/// possible.
	///
	/// The length of `src` must be same as `self`.
	///
	/// If `src` does not use the same type arguments as `self`, use
	/// [`.clone_from_bitslice()`].
	///
	/// # Original
	///
	/// [`slice::copy_from_slice`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.copy_from_slice)
	///
	/// # API Differences
	///
	/// This method is renamed, as it takes a bit slice rather than an element
	/// slice.
	///
	/// # Panics
	///
	/// This function will panic if the two slices have different lengths.
	///
	/// # Examples
	///
	/// Copying two bits from a slice into another:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let src = bits![1; 4];
	/// let dst = bits![mut 0; 2];
	///
	/// // Because the slices have to be the same length,
	/// // we slice the source slice from four bits to
	/// // two. It will panic if we don't do this.
	/// dst.clone_from_bitslice(&src[2..]);
	/// ```
	///
	/// Rust enforces that there can only be one mutable reference with no
	/// immutable references to a particular piece of data in a particular
	/// scope. Because of this, attempting to use [.copy_from_slice()] on a
	/// single slice will result in a compile failure:
	///
	/// ```rust,compile_fail
	/// use bitvec::prelude::*;
	///
	/// let slice = bits![mut 0, 0, 0, 1, 1];
	///
	/// slice[.. 2].copy_from_bitslice(&bits[3 ..]); // compile fail!
	/// ```
	///
	/// To work around this, we can use [`.split_at_mut()`] to create two
	/// distinct sub-slices from a slice:
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let slice = bits![mut 0, 0, 0, 1, 1];
	///
	/// {
	///   let (left, right) = slice.split_at_mut(2);
	///   left.copy_from_bitslice(&right[1 ..]);
	/// }
	///
	/// assert_eq!(slice, bits![1, 1, 0, 1, 1]);
	/// ```
	///
	/// [`.clone_from_bitslice()`]: Self::clone_from_bitslice
	/// [`.split_at_mut()`]: Self::split_at_mut
	pub fn copy_from_bitslice(&mut self, src: &Self) {
		assert_eq!(
			self.len(),
			src.len(),
			"Copying between slices requires equal lengths"
		);

		let (d_head, s_head) =
			(self.as_bitspan().head(), src.as_bitspan().head());
		//  Where the two slices have identical layouts (head index and length),
		//  the copy can be done by using the memory domains.
		if d_head == s_head {
			match (self.domain_mut(), src.domain()) {
				(
					DomainMut::Enclave {
						elem: d_elem, tail, ..
					},
					Domain::Enclave { elem: s_elem, .. },
				) => {
					let mask = O::mask(d_head, tail);
					d_elem.clear_bits(mask);
					d_elem.set_bits(mask & s_elem.load_value());
				},
				(
					DomainMut::Region {
						head: d_head,
						body: d_body,
						tail: d_tail,
					},
					Domain::Region {
						head: s_head,
						body: s_body,
						tail: s_tail,
					},
				) => {
					if let (Some((h_idx, dh_elem)), Some((_, sh_elem))) =
						(d_head, s_head)
					{
						let mask = O::mask(h_idx, None);
						dh_elem.clear_bits(mask);
						dh_elem.set_bits(mask & sh_elem.load_value());
					}
					for (dst, src) in d_body.iter_mut().zip(s_body.iter()) {
						dst.store_value(src.load_value())
					}
					if let (Some((dt_elem, t_idx)), Some((st_elem, _))) =
						(d_tail, s_tail)
					{
						let mask = O::mask(None, t_idx);
						dt_elem.clear_bits(mask);
						dt_elem.set_bits(mask & st_elem.load_value());
					}
				},
				_ => unreachable!(
					"Slices with equal type parameters, lengths, and heads \
					 will always have equal domains"
				),
			}
		}
		/* TODO(myrrlyn): Remove this when specialization stabilizes.

		This section simulates access to specialization through partial
		type-argument application. It detects accelerable type arguments (`O`
		values provided by `bitvec`, where `BitSlice<O, _>` implements
		`BitField`) and uses their batch load/store behavior to move more than
		one bit per cycle.

		Without language-level specialization, we cannot dispatch to
		individually well-typed functions, so instead this block uses the
		compiler’s `TypeId` API to inspect the type arguments passed to a
		monomorphization and select the appropriate codegen for it. We know that
		control will only enter any of these subsequent blocks when the type
		argument to monomorphization matches the guard, so the pointer casts
		become the identity function, which is safe and correct.

		This is only safe to do in `.copy_from_bitslice()`, not in
		`.clone_from_bitslice()`, because `BitField`’s behavior will only be
		correct when the two slices are matching in both their ordering and
		storage type arguments. Mismatches will cause an observed shuffling of
		sections as `BitField` reïnterprets raw bytes according to the machine
		register selected.
		*/
		else if dvl::match_order::<O, Lsb0>() {
			let this: &mut BitSlice<Lsb0, T> =
				unsafe { &mut *(self as *mut _ as *mut _) };
			let that: &BitSlice<Lsb0, T> =
				unsafe { &*(src as *const _ as *const _) };
			this.sp_copy_from_bitslice(that);
		}
		else if dvl::match_order::<O, Msb0>() {
			let this: &mut BitSlice<Msb0, T> =
				unsafe { &mut *(self as *mut _ as *mut _) };
			let that: &BitSlice<Msb0, T> =
				unsafe { &*(src as *const _ as *const _) };
			this.sp_copy_from_bitslice(that);
		}
		else {
			for (ptr, from) in
				self.as_mut_bitptr_range().zip(src.iter().by_val())
			{
				unsafe {
					ptr.write(from);
				}
			}
		}
	}

	/// Swaps all bits in `self` with those in `other`.
	///
	/// The length of `other` must be the same as `self`.
	///
	/// # Original
	///
	/// [`slice::swap_with_slice`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.swap_with_slice)
	///
	/// # API Differences
	///
	/// This method is renamed, as it takes a bit slice rather than an element
	/// slice.
	///
	/// # Panics
	///
	/// This function will panic if the two slices have different lengths.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut one = [0xA5u8, 0x69];
	/// let mut two = 0x1234u16;
	/// let one_bits = one.view_bits_mut::<Msb0>();
	/// let two_bits = two.view_bits_mut::<Lsb0>();
	///
	/// one_bits.swap_with_bitslice(two_bits);
	///
	/// assert_eq!(one, [0x2C, 0x48]);
	/// # #[cfg(target_endian = "little")] {
	/// assert_eq!(two, 0x96A5);
	/// # }
	/// ```
	pub fn swap_with_bitslice<O2, T2>(&mut self, other: &mut BitSlice<O2, T2>)
	where
		O2: BitOrder,
		T2: BitStore,
	{
		let len = self.len();
		assert_eq!(len, other.len());
		for (to, from) in unsafe {
			self.iter_mut()
				.remove_alias()
				.zip(other.iter_mut().remove_alias())
		} {
			let (this, that) = (*to, *from);
			to.set(that);
			from.set(this);
		}
	}

	/// Shifts the contents of a bit-slice left (towards index `0`).
	///
	/// This moves the contents of the slice from `by ..` down to
	/// `0 .. len - by`, and erases `len - by ..` to `0`. As this is a
	/// destructive (and linearly expensive) operation, you may prefer instead
	/// to use range subslicing.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `by`: The distance by which to shift the slice contents.
	///
	/// # Panics
	///
	/// This panics if `by` is not less than `self.len()`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![mut 1; 6];
	/// bits.shift_left(2);
	/// assert_eq!(bits, bits![1, 1, 1, 1, 0, 0]);
	/// ```
	pub fn shift_left(&mut self, by: usize) {
		let len = self.len();
		if by == 0 {
			return;
		}
		assert!(
			by < len,
			"Cannot shift a slice by more than its length: {} exceeds {}",
			by,
			len
		);

		unsafe {
			self.copy_within_unchecked(by .., 0);
			let trunc = len - by;
			self.get_unchecked_mut(trunc ..).set_all(false);
		}
	}

	/// Shifts the contents of a bit-slice right (towards index `self.len()`).
	///
	/// This moves the contents of the slice from `.. len - by` up to `by ..`,
	/// and erases `.. by` to `0`. As this is a destructive (and linearly
	/// expensive) operation, you may prefer instead to use range subslicing.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `by`: The distance by which to shift the slice contents.
	///
	/// # Panics
	///
	/// This panics if `by` is not less than `self.len()`.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![mut 1; 6];
	/// bits.shift_right(2);
	/// assert_eq!(bits, bits![0, 0, 1, 1, 1, 1]);
	/// ```
	pub fn shift_right(&mut self, by: usize) {
		let len = self.len();
		if by == 0 {
			return;
		}
		assert!(
			by < len,
			"Cannot shift a slice by more than its length: {} exceeds {}",
			by,
			len
		);

		let trunc = len - by;
		unsafe {
			self.copy_within_unchecked(.. trunc, by);
			self.get_unchecked_mut(.. by).set_all(false);
		}
	}

	/// Sets all bits in the slice to a value.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `value`: The bit value to which all bits in the slice will be set.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut src = 0u8;
	/// let bits = src.view_bits_mut::<Msb0>();
	/// bits[2 .. 6].set_all(true);
	/// assert_eq!(bits.as_slice(), &[0b0011_1100]);
	/// bits[3 .. 5].set_all(false);
	/// assert_eq!(bits.as_slice(), &[0b0010_0100]);
	/// bits[.. 1].set_all(true);
	/// assert_eq!(bits.as_slice(), &[0b1010_0100]);
	/// ```
	pub fn set_all(&mut self, value: bool) {
		//  Grab the function pointers used to commit bit-masks into memory.
		let setter = <T::Access>::get_writers(value);
		match self.domain_mut() {
			DomainMut::Enclave { head, elem, tail } => {
				setter(elem, O::mask(head, tail));
			},
			DomainMut::Region { head, body, tail } => {
				if let Some((head, elem)) = head {
					setter(elem, O::mask(head, None));
				}
				//  loop assignment is `memset`’s problem, not ours
				unsafe {
					ptr::write_bytes(
						body.as_mut_ptr(),
						[0, !0][value as usize],
						body.len(),
					);
				}
				if let Some((elem, tail)) = tail {
					setter(elem, O::mask(None, tail));
				}
			},
		}
	}

	/// Applies a function to each bit in the slice.
	///
	/// `BitSlice` cannot implement [`IndexMut`], as it cannot manifest `&mut
	/// bool` references, and the [`BitRef`] proxy reference has an unavoidable
	/// overhead. This method bypasses both problems, by applying a function to
	/// each pair of index and value in the slice, without constructing a proxy
	/// reference. Benchmarks indicate that this method is about 2–4 times
	/// faster than the `.iter_mut().enumerate()` equivalent.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `func`: A function which receives two arguments, `index: usize` and
	///   `value: bool`, and returns a `bool`.
	///
	/// # Effects
	///
	/// For each index in the slice, the result of invoking `func` with the
	/// index number and current bit value is written into the slice.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut data = 0u8;
	/// let bits = data.view_bits_mut::<Msb0>();
	/// bits.for_each(|idx, _bit| idx % 3 == 0);
	/// assert_eq!(data, 0b100_100_10);
	/// ```
	///
	/// [`BitRef`]: crate::ptr::BitRef
	/// [`IndexMut`]: core::ops::IndexMut
	pub fn for_each<F>(&mut self, mut func: F)
	where F: FnMut(usize, bool) -> bool {
		for idx in 0 .. self.len() {
			unsafe {
				let tmp = *self.get_unchecked(idx);
				let new = func(idx, tmp);
				self.set_unchecked(idx, new);
			}
		}
	}

	/// Produces the absolute offset in bits between two slice heads.
	///
	/// While this method is sound for any two arbitrary bit slices, the answer
	/// it produces is meaningful *only* when one argument is a strict subslice
	/// of the other. If the two slices are created from different buffers
	/// entirely, a comparison is undefined; if the two slices are disjoint
	/// regions of the same buffer, then the semantically correct distance is
	/// between the tail of the lower and the head of the upper, which this
	/// does not measure.
	///
	/// # Visual Description
	///
	/// Consider the following sequence of bits:
	///
	/// ```text
	/// [ 0 1 2 3 4 5 6 7 8 9 a b ]
	///   |       ^^^^^^^       |
	///   ^^^^^^^^^^^^^^^^^^^^^^^
	/// ```
	///
	/// It does not matter whether there are bits between the tail of the
	/// smaller and the larger slices. The offset is computed from the bit
	/// distance between the two heads.
	///
	/// # Behavior
	///
	/// This function computes the *semantic* distance between the heads, rather
	/// than the *electrical. It does not take into account the `BitOrder`
	/// implementation of the slice.
	///
	/// # Safety and Soundness
	///
	/// One of `self` or `other` must contain the other for this comparison to
	/// be meaningful.
	///
	/// # Parameters
	///
	/// - `&self`
	/// - `other`: Another bit slice. This must be either a strict subregion or
	///   a strict superregion of `self`.
	///
	/// # Returns
	///
	/// The distance in (semantic) bits betwen the heads of each region. The
	/// value is positive when `other` is higher in the address space than
	/// `self`, and negative when `other` is lower in the address space than
	/// `self`.
	pub fn offset_from(&self, other: &Self) -> isize {
		unsafe { other.as_bitptr().offset_from(self.as_bitptr()) }
	}

	#[doc(hidden)]
	#[deprecated = "Use `BitPtr::offset_from`"]
	pub fn electrical_distance(&self, _other: &Self) -> isize {
		unimplemented!(
			"This no longer exists! Offsets are only defined between two \
			 bit-pointers in the same bit-region, and `bitvec` considers two \
			 regions with different orderings, *even if they cover the same \
			 locations*, to be different. Use `BitPtr::offset_from`."
		);
	}
}

/// Unchecked variants of checked accessors.
impl<O, T> BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// Writes a new bit at a given index, without doing bounds checking.
	///
	/// This is generally not recommended; use with caution! Calling this method
	/// with an out-of-bounds index is *[undefined behavior]*. For a safe
	/// alternative, see [`.set()`].
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `index`: The bit index at which to write. It must be in the range `0
	///   .. self.len()`.
	/// - `value`: The value to be written; `true` for `1` or `false` for `0`.
	///
	/// # Effects
	///
	/// The bit at `index` is set to `value`. If `index` is out of bounds, then
	/// the memory access is incorrect, and its behavior is unspecified.
	///
	/// # Safety
	///
	/// This method is **not** safe. It performs raw pointer arithmetic to seek
	/// from the start of the slice to the requested index, and set the bit
	/// there. It does not inspect the length of `self`, and it is free to
	/// perform out-of-bounds memory *write* access.
	///
	/// Use this method **only** when you have already performed the bounds
	/// check, and can guarantee that the call occurs with a safely in-bounds
	/// index.
	///
	/// # Examples
	///
	/// This example uses a bit slice of length 2, and demonstrates
	/// out-of-bounds access to the last bit in the element.
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![mut 0; 2];
	/// let (first, _) = bits.split_at_mut(1);
	///
	/// unsafe {
	///   first.set_unchecked(1, true);
	/// }
	///
	/// assert_eq!(bits, bits![0, 1]);
	/// ```
	///
	/// [`self.len()`]: Self::len
	/// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
	/// [`.set()`]: Self::set
	pub unsafe fn set_unchecked(&mut self, index: usize, value: bool) {
		self.as_mut_bitptr().add(index).write(value);
	}

	/// Writes a new bit at a given index, without doing bounds checking.
	///
	/// This method supports writing through a shared reference to a bit that
	/// may be observed by other `BitSlice` handles. It is only present when the
	/// `T` type parameter supports such shared mutation (measured by the
	/// [`Radium`] trait).
	///
	/// # Effects
	///
	/// The bit at `index` is set to `value`. If `index` is out of bounds, then
	/// the memory access is incorrect, and its behavior is unspecified. If `T`
	/// is an [atomic], this will lock the memory bus for the referent
	/// address, and may cause stalls.
	///
	/// # Safety
	///
	/// This method is **not** safe. It performs raw pointer arithmetic to seek
	/// from the start of the slice to the requested index, and set the bit
	/// there. It does not inspect the length of `self`, and it is free to
	/// perform out-of-bounds memory *write* access.
	///
	/// Use this method **only** when you have already performed the bounds
	/// check, and can guarantee that the call occurs with a safely in-bounds
	/// index.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	/// use core::cell::Cell;
	///
	/// let byte = Cell::new(0u8);
	/// let bits = byte.view_bits::<Msb0>();
	/// let bits_2 = bits;
	///
	/// let (first, _) = bits.split_at(1);
	/// assert_eq!(first.len(), 1);
	/// unsafe { first.set_aliased_unchecked(2, true); }
	///
	/// assert!(bits_2[2]);
	/// ```
	///
	/// [atomic]: core::sync::atomic
	/// [`Radium`]: radium::Radium
	pub unsafe fn set_aliased_unchecked(&self, index: usize, value: bool)
	where T: radium::Radium {
		self.as_bitptr().add(index).assert_mut().write(value);
	}

	/// Swaps two bits in the slice.
	///
	/// See [`.swap()`].
	///
	/// # Safety
	///
	/// `a` and `b` must both be less than [`self.len()`].
	///
	/// [`self.len()`]: Self::len
	/// [`.swap()`]: Self::swap
	pub unsafe fn swap_unchecked(&mut self, a: usize, b: usize) {
		let bit_a = *self.get_unchecked(a);
		let bit_b = *self.get_unchecked(b);
		self.set_unchecked(a, bit_b);
		self.set_unchecked(b, bit_a);
	}

	/// Divides one slice into two at an index, without performing any bounds
	/// checking.
	///
	/// See [`.split_at()`].
	///
	/// # Safety
	///
	/// `mid` must not be greater than [`self.len()`]. If this condition is
	/// violated, the function behavior is *unspecified*.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![0, 0, 0, 1, 1, 1];
	/// let (l, r) = unsafe { bits.split_at_unchecked(3) };
	/// assert!(l.not_any());
	/// assert!(r.all());
	///
	/// let (l, r) = unsafe { bits.split_at_unchecked(6) };
	/// assert_eq!(l, bits);
	/// assert!(r.is_empty());
	/// ```
	///
	/// [`self.len()`]: Self::len
	/// [`.split_at()`]: Self::split_at
	pub unsafe fn split_at_unchecked(&self, mid: usize) -> (&Self, &Self) {
		(self.get_unchecked(.. mid), self.get_unchecked(mid ..))
	}

	/// Divides one mutable slice into two at an index.
	///
	/// See [`.split_at_mut()`].
	///
	/// # Safety
	///
	/// `mid` must not be greater than [`self.len()`].
	///
	/// [`self.len()`]: Self::len
	/// [`.split_at_mut()`]: Self::split_at_mut
	#[allow(clippy::type_complexity)]
	pub unsafe fn split_at_unchecked_mut(
		&mut self,
		mid: usize,
	) -> (&mut BitSlice<O, T::Alias>, &mut BitSlice<O, T::Alias>) {
		let bp = self.alias_mut().as_mut_bitspan();
		(
			bp.to_bitslice_mut().get_unchecked_mut(.. mid),
			bp.to_bitslice_mut().get_unchecked_mut(mid ..),
		)
	}

	/// Copies bits from one part of the slice to another part of itself,
	/// without doing bounds checks.
	///
	/// The ranges are allowed to overlap.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `src`: The range within `self` from which to copy.
	/// - `dst`: The starting index within `self` at which to paste.
	///
	/// # Effects
	///
	/// `self[src]` is copied to `self[dest .. dest + src.end() - src.start()]`.
	///
	/// # Safety
	///
	/// `src` and `dest .. dest + src.len()` must be entirely within
	/// [`self.len()`].
	///
	/// [`self.len()`]: Self::len
	pub unsafe fn copy_within_unchecked<R>(&mut self, src: R, dest: usize)
	where R: RangeBounds<usize> {
		if dvl::match_order::<O, Lsb0>() {
			let this: &mut BitSlice<Lsb0, T> = &mut *(self as *mut _ as *mut _);
			this.sp_copy_within_unchecked(src, dest);
		}
		else if dvl::match_order::<O, Msb0>() {
			let this: &mut BitSlice<Msb0, T> = &mut *(self as *mut _ as *mut _);
			this.sp_copy_within_unchecked(src, dest);
		}
		else {
			let source = dvl::normalize_range(src, self.len());
			let source_len = source.len();
			let rev = source.contains(&dest);
			let iter = source.zip(dest .. dest + source_len);
			if rev {
				for (from, to) in iter.rev() {
					let bit = *self.get_unchecked(from);
					self.set_unchecked(to, bit);
				}
			}
			else {
				for (from, to) in iter {
					let bit = *self.get_unchecked(from);
					self.set_unchecked(to, bit);
				}
			}
		}
	}
}

/// View conversions.
#[cfg(not(tarpaulin_include))]
impl<O, T> BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// Returns a raw bit-pointer to the base of the bit-slice’s region.
	///
	/// The caller must ensure that the bit-slice outlives the bit-pointer this
	/// function returns, or else it will end up pointing to garbage.
	///
	/// The caller must also ensure that the memory the bit-pointer
	/// (non-transitively) points to is never written to using this bit-pointer
	/// or any bit-pointer derived from it. If you need to mutate the contents
	/// of the slice, use [`.as_mut_bitptr()`].
	///
	/// Modifying the container referenced by this bit-slice may cause its
	/// buffer to be reällocated, which would also make any bit-pointers to it
	/// invalid.
	///
	/// # Original
	///
	/// [`slice::as_ptr`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.as_ptr)
	///
	/// # API Differences
	///
	/// This returns a structure, [`BitPtr`], rather than an actual raw pointer
	/// `*Bit`. The information required to address a bit within a memory
	/// element cannot be encoded into a single pointer.
	///
	/// This structure can be converted back into a `&BitSlice` with the
	/// function [`from_raw_parts`].
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let x = bits![0, 0, 1];
	/// let x_ptr = x.as_ptr();
	///
	/// unsafe {
	///   for i in 0 .. x.len() {
	///     assert_eq!(*x.get_unchecked(i), (&*x)[i]);
	///   }
	/// }
	/// ```
	///
	/// [`.as_mut_bitptr()`]: Self::as_mut_bitptr
	/// [`from_raw_parts`]: crate::slice::from_raw_parts
	#[inline(always)]
	pub fn as_bitptr(&self) -> BitPtr<Const, O, T> {
		self.as_bitspan().as_bitptr()
	}

	/// Returns an unsafe mutable bit-pointer to the bit-slice’s region.
	///
	/// The caller must ensure that the bit-slice outlives the bit-pointer this
	/// function returns, or else it will end up pointing to garbage.
	///
	/// Modifying the container referenced by this bit-slice may cause its
	/// buffer to be reällocated, which would also make any bit-pointers to it
	/// invalid.
	///
	/// # Original
	///
	/// [`slice::as_mut_ptr`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.as_mut_ptr)
	///
	/// # API Differences
	///
	/// This returns `*mut BitSlice`, which is the equivalont of `*mut [T]`
	/// instead of `*mut T`. The pointer encoding used requires more than one
	/// CPU word of space to address a single bit, so there is no advantage to
	/// removing the length information from the encoded pointer value.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![mut Lsb0, u8; 0; 8];
	/// let bits_ptr = bits.as_mut_ptr();
	///
	/// for i in 0 .. bits.len() {
	///   unsafe {
	///     bits_ptr.add(i).write(i % 3 == 0);
	///   }
	/// }
	/// assert_eq!(bits.as_slice()[0], 0b0100_1001);
	/// ```
	#[inline(always)]
	pub fn as_mut_bitptr(&mut self) -> BitPtr<Mut, O, T> {
		self.as_mut_bitspan().as_bitptr()
	}

	/// Returns the two raw bit-pointers spanning the bit-slice.
	///
	/// The returned range is half-open, which means that the end bit-pointer
	/// points *one past* the last bit of the bit-slice. This way, an empty
	/// bit-slice is represented by two equal bit-pointers, and the difference
	/// between the two bit-pointers represents the size of the bit-slice.
	///
	/// See [`as_bitptr`] for warnings on using these bit-pointers. The end
	/// bit-pointer requires extra caution, as it does not point to a valid bit
	/// in the bit-slice.
	///
	/// This function allows a more direct access to bit-pointers, without
	/// paying the cost of encoding into a `*BitSlice`, at the cost of no longer
	/// fitting into ordinary Rust interfaces.
	///
	/// # Original
	///
	/// [`slice::as_ptr_range`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.as_ptr_range)
	///
	/// # API Differences
	///
	/// This returns a dedicated structure, rather than a range of [`BitPtr`]s,
	/// because the traits needed for non-`core` types to correctly operate in
	/// ranges are still unstable. The structure can be converted into a range,
	/// but that range will not be an iterator.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![0, 1, 0, 0, 1];
	/// let mid_ptr = bits.get(2).unwrap().into_bitptr();
	/// let mut range = bits.as_bitptr_range();
	/// assert!(range.contains(&mid_ptr));
	/// unsafe {
	///   assert!(!range.next().unwrap().read());
	///   assert!(range.next_back().unwrap().read())
	/// }
	/// ```
	///
	/// [`BitPtr`]: crate::ptr::BitPtr
	/// [`as_bitptr`]: Self::as_bitptr
	pub fn as_bitptr_range(&self) -> BitPtrRange<Const, O, T> {
		unsafe { self.as_bitptr().range(self.len()) }
	}

	/// Returns the two unsafe mutable bit-pointers spanning the bit-slice.
	///
	/// The returned range is half-open, which means that the end bit-pointer
	/// points *one past* the last bitt of the bit-slice. This way, an empty
	/// bit-slice is represented by two equal bit-pointers, and the difference
	/// between the two bit-pointers represents the size of the bit-slice.
	///
	/// See [`as_mut_bitptr`] for warnings on using these bit-pointers. The end
	/// bit-pointer requires extra caution, as it does not point to a valid bit
	/// in the bit-slice.
	///
	/// # Original
	///
	/// [`slice::as_mut_ptr_range`](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.as_mut_ptr_range)
	///
	/// # API Differences
	///
	/// This returns a dedicated structure, rather than a range of [`BitPtr`]s,
	/// because the traits needed for non-`core` types to correctly operate in
	/// ranges are still unstable. The structure can be converted into a range,
	/// but that range will not be an iterator.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	/// use bitvec::ptr as bv_ptr;
	///
	/// let mut data = 0u8;
	/// let bits = data.view_bits_mut::<Msb0>();
	/// for mut bitptr in bits.as_mut_bitptr_range() {
	///   unsafe { bv_ptr::write(bitptr, true); }
	/// }
	/// assert_eq!(data, !0);
	/// ```
	///
	/// [`BitPtr`]: crate::ptr::BitPtr
	/// [`as_mut_bitptr`]: Self::as_mut_bitptr
	pub fn as_mut_bitptr_range(&mut self) -> BitPtrRange<Mut, O, T> {
		unsafe { self.as_mut_bitptr().range(self.len()) }
	}

	/// Splits the slice into subslices at alias boundaries.
	///
	/// This splits `self` into the memory locations that it partially fills and
	/// the memory locations that it completely fills. The locations that are
	/// completely filled may be accessed without any `bitvec`-imposed alias
	/// conditions, while the locations that are only partially filled are left
	/// unchanged.
	///
	/// You can read more about the [`BitDomain`] splitting in its
	/// documentation.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut data = [0u16; 3];
	/// let all = data.view_bits_mut::<Msb0>();
	/// let (_, rest) = all.split_at_mut(8);
	/// let bits: &BitSlice<Msb0, <u16 as BitStore>::Alias> = &rest[.. 32];
	///
	/// let (head, body, tail) = bits
	///   .bit_domain()
	///   .region()
	///   .unwrap();
	/// assert_eq!(head.len(), 8);
	/// assert_eq!(tail.len(), 8);
	/// let _: &BitSlice<Msb0, <u16 as BitStore>::Alias> = head;
	/// let _: &BitSlice<Msb0, <u16 as BitStore>::Alias> = tail;
	/// let _: &BitSlice<Msb0, u16> = body;
	/// ```
	///
	/// [`BitDomain`]: crate::domain::BitDomain
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn bit_domain(&self) -> BitDomain<O, T> {
		BitDomain::new(self)
	}

	/// Splits the slice into subslices at alias boundaries.
	///
	/// This splits `self` into the memory locations that it partially fills and
	/// the memory locations that it completely fills. The locations that are
	/// completely filled may be accessed without any `bitvec`-imposed alias
	/// conditions, while the locations that are only partially filled are left
	/// unchanged.
	///
	/// You can read more about the [`BitDomainMut`] splitting in its
	/// documentation.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut data = [0u16; 3];
	/// let all = data.view_bits_mut::<Msb0>();
	/// let (_, rest) = all.split_at_mut(8);
	/// let bits: &mut BitSlice<Msb0, <u16 as BitStore>::Alias>
	///   = &mut rest[.. 32];
	///
	/// let (head, body, tail) = bits
	///   .bit_domain_mut()
	///   .region()
	///   .unwrap();
	/// assert_eq!(head.len(), 8);
	/// assert_eq!(tail.len(), 8);
	/// let _: &mut BitSlice<Msb0, <u16 as BitStore>::Alias> = head;
	/// let _: &mut BitSlice<Msb0, <u16 as BitStore>::Alias> = tail;
	/// let _: &mut BitSlice<Msb0, u16> = body;
	/// ```
	///
	/// [`BitDomainMut`]: crate::domain::BitDomainMut
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn bit_domain_mut(&mut self) -> BitDomainMut<O, T> {
		BitDomainMut::new(self)
	}

	/// Views the underlying memory containing the slice, split at alias
	/// boundaries.
	///
	/// This splits `self` into the memory locations that it partially fills and
	/// the memory locatinos that it completely fills. The locations that are
	/// completely filled may be accessed without any `bitvec`-imposed alias
	/// conditions, while the locations that are only partially filled are left
	/// unchanged.
	///
	/// You can read more about the [`Domain`] splitting in its documentation.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut data = [0u16; 3];
	/// let all = data.view_bits_mut::<Msb0>();
	/// let (_, rest) = all.split_at_mut(8);
	/// let bits: &BitSlice<Msb0, <u16 as BitStore>::Alias> = &rest[.. 32];
	///
	/// let (head, body, tail) = bits
	///   .domain()
	///   .region()
	///   .unwrap();
	/// assert_eq!(body.len(), 1);
	///
	/// let _: &<u16 as BitStore>::Alias = head.unwrap().1;
	/// let _: &<u16 as BitStore>::Alias = tail.unwrap().0;
	/// let _: &[u16] = body;
	/// ```
	///
	/// [`Domain`]: crate::domain::Domain
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn domain(&self) -> Domain<T> {
		Domain::new(self)
	}

	/// Views the underlying memory containing the slice, split at alias
	/// boundaries.
	///
	/// This splits `self` into the memory locations that it partially fills and
	/// the memory locations that it completely fills. The locations that are
	/// completely filled may be accessed without any `bitvec`-imposed alias
	/// conditions, while the locations that are only partially filled are left
	/// unchanged.
	///
	/// You can read more about the [`DomainMut`] splitting in its
	/// documentation.
	///
	/// # Examples
	///
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let mut data = [0u16; 3];
	/// let all = data.view_bits_mut::<Msb0>();
	/// let (_, rest) = all.split_at_mut(8);
	/// let bits: &mut BitSlice<Msb0, <u16 as BitStore>::Alias> = &mut rest[.. 32];
	///
	/// let (head, body, tail) = bits
	///   .domain_mut()
	///   .region()
	///   .unwrap();
	/// assert_eq!(body.len(), 1);
	///
	/// let _: &<<u16 as BitStore>::Alias as BitStore>::Access = head.unwrap().1;
	/// let _: &<<u16 as BitStore>::Alias as BitStore>::Access = tail.unwrap().0;
	/// let _: &mut [u16] = body;
	/// ```
	///
	/// [`DomainMut`]: crate::domain::DomainMut
	#[inline(always)]
	#[cfg(not(tarpaulin_include))]
	pub fn domain_mut(&mut self) -> DomainMut<T> {
		DomainMut::new(self)
	}

	/// Views the underlying memory containing the slice.
	///
	/// The returned slice handle views all elements touched by `self`, and
	/// marks them all with `self`’s current aliasing state. For a more precise
	/// view, or one that permits mutation, use [`.domain()`] or
	/// [`.domain_mut()`].
	///
	/// [`.domain()`]: Self::domain
	/// [`.domain_mut()`]: Self::domain_mut
	pub fn as_slice(&self) -> &[T] {
		let bitspan = self.as_bitspan();
		let (base, elts) = (bitspan.address().to_const(), bitspan.elements());
		unsafe { slice::from_raw_parts(base, elts) }
	}
}

/// Crate-internal functions.
impl<O, T> BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// Type-cast the slice reference to its pointer structure.
	pub(crate) fn as_bitspan(&self) -> BitSpan<Const, O, T> {
		BitSpan::from_bitslice_ptr(self)
	}

	/// Type-cast the slice reference to its pointer structure.
	pub(crate) fn as_mut_bitspan(&mut self) -> BitSpan<Mut, O, T> {
		BitSpan::from_bitslice_ptr_mut(self)
	}

	/// Asserts that `index` is less than [`self.len()`].
	///
	/// # Parameters
	///
	/// - `&self`
	/// - `index`: The index to test against [`self.len()`].
	///
	/// # Panics
	///
	/// This method panics if `index` is not less than `self.len()`.
	///
	/// [`self.len()`]: Self::len
	pub(crate) fn assert_in_bounds(&self, index: usize) {
		let len = self.len();
		assert!(index < len, "Index out of range: {} >= {}", index, len);
	}

	/// Marks an immutable slice as referring to aliased memory region.
	pub(crate) fn alias(&self) -> &BitSlice<O, T::Alias> {
		unsafe { &*(self as *const Self as *const BitSlice<O, T::Alias>) }
	}

	/// Marks a mutable slice as describing an aliased memory region.
	pub(crate) fn alias_mut(&mut self) -> &mut BitSlice<O, T::Alias> {
		unsafe { &mut *(self as *mut Self as *mut BitSlice<O, T::Alias>) }
	}

	/// Removes the aliasing marker from a mutable slice handle.
	///
	/// # Safety
	///
	/// This must only be used when the slice is either known to be unaliased,
	/// or this call is combined with an operation that adds an aliasing marker
	/// and the total number of aliasing markers must remain unchanged.
	#[cfg(not(tarpaulin_include))]
	pub(crate) unsafe fn unalias_mut(
		this: &mut BitSlice<O, T::Alias>,
	) -> &mut Self {
		&mut *(this as *mut BitSlice<O, T::Alias> as *mut Self)
	}

	/// Splits a mutable slice at some mid-point, without checking boundary
	/// conditions or adding an alias marker.
	///
	/// This method has the same behavior as [`.split_at_unchecked_mut()`],
	/// except that it does not apply an aliasing marker to the partitioned
	/// subslices.
	///
	/// # Safety
	///
	/// See [`.split_at_unchecked_mut()`] for safety requirements.
	///
	/// Additionally, this is only safe when `T` is alias-safe.
	///
	/// [`.split_at_unchecked_mut()`]: Self::split_at_unchecked_mut
	pub(crate) unsafe fn split_at_unchecked_mut_noalias(
		&mut self,
		mid: usize,
	) -> (&mut Self, &mut Self) {
		//  Split the slice at the requested midpoint, adding an alias layer
		let (head, tail) = self.split_at_unchecked_mut(mid);
		//  Remove the new alias layer.
		(Self::unalias_mut(head), Self::unalias_mut(tail))
	}
}

/// Methods available only when `T` allows shared mutability.
impl<O, T> BitSlice<O, T>
where
	O: BitOrder,
	T: BitSafe + BitStore,
{
	/// Splits a mutable slice at some mid-point.
	///
	/// This method has the same behavior as [`.split_at_mut()`], except that it
	/// does not apply an aliasing marker to the partitioned subslices.
	///
	/// # Safety
	///
	/// Because this method is defined only on `BitSlice`s whose `T` type is
	/// alias-safe, the subslices do not need to be additionally marked.
	///
	/// [`.split_at_mut()`]: Self::split_at_mut
	pub fn split_at_aliased_mut(
		&mut self,
		mid: usize,
	) -> (&mut Self, &mut Self) {
		let (head, tail) = self.split_at_mut(mid);
		unsafe { (Self::unalias_mut(head), Self::unalias_mut(tail)) }
	}
}

/// Miscellaneous information.
impl<O, T> BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// The inclusive maximum length of a `BitSlice<_, T>`.
	///
	/// As `BitSlice` is zero-indexed, the largest possible index is one less
	/// than this value.
	///
	/// |CPU word width|         Value         |
	/// |-------------:|----------------------:|
	/// |32 bits       |     `0x1fff_ffff`     |
	/// |64 bits       |`0x1fff_ffff_ffff_ffff`|
	pub const MAX_BITS: usize = BitSpan::<Const, O, T>::REGION_MAX_BITS;
	/// The inclusive maximum length that a slice `[T]` can be for
	/// `BitSlice<_, T>` to cover it.
	///
	/// A `BitSlice<_, T>` that begins in the interior of an element and
	/// contains the maximum number of bits will extend one element past the
	/// cutoff that would occur if the slice began at the zeroth bit. Such a
	/// slice must be manually constructed, but will not otherwise fail.
	///
	/// |Type Bits|Max Elements (32-bit)| Max Elements (64-bit) |
	/// |--------:|--------------------:|----------------------:|
	/// |        8|    `0x0400_0001`    |`0x0400_0000_0000_0001`|
	/// |       16|    `0x0200_0001`    |`0x0200_0000_0000_0001`|
	/// |       32|    `0x0100_0001`    |`0x0100_0000_0000_0001`|
	/// |       64|    `0x0080_0001`    |`0x0080_0000_0000_0001`|
	pub const MAX_ELTS: usize = BitSpan::<Const, O, T>::REGION_MAX_ELTS;
}

#[cfg(feature = "alloc")]
impl<O, T> BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	/// Copies `self` into a new [`BitVec`].
	///
	/// This resets any alias markings from `self`, since the returned buffer is
	/// known to be newly allocated and thus unaliased.
	///
	/// # Examples
	///
	/// ```rust
	/// use bitvec::prelude::*;
	///
	/// let bits = bits![0, 1, 0, 1];
	/// let bv = bits.to_bitvec();
	/// assert_eq!(bits, bv);
	/// ```
	///
	/// [`BitVec`]: crate::vec::BitVec
	pub fn to_bitvec(&self) -> BitVec<O, T::Unalias> {
		let mut bitspan = self.as_bitspan();
		//  Create an allocation and copy `*self` into it.
		let mut vec = self.domain().collect::<Vec<_>>().pipe(ManuallyDrop::new);
		let capacity = vec.capacity();
		unsafe {
			bitspan
				.set_address(Address::new_unchecked(vec.as_mut_ptr() as usize));
			BitVec::from_fields(
				bitspan.assert_mut().cast::<T::Unalias>(),
				capacity,
			)
		}
	}
}

/** Performs the same functionality as [`from_raw_parts`], without checking the
`len` argument.

# Parameters

- `data`: A `BitPtr` to a dereferencable region of memory.
- `len`: The length, in bits, of the region beginning at `*data`. This is not
  checked against the maximum value, and is encoded directly into the bit-slice
  reference. If it exceeds [`BitSlice::MAX_BITS`], it will be modulated to fit
  (the high bits will be discarded).

# Returns

A `&BitSlice` reference starting at `data` and running for `len & MAX_BITS`
bits.

# Safety

See [`from_raw_parts`].

[`BitSlice::MAX_BITS`]: crate::slice::BitSlice::MAX_BITS
[`from_raw_parts`]: crate::slice::from_raw_parts
**/
pub unsafe fn from_raw_parts_unchecked<'a, O, T>(
	data: BitPtr<Const, O, T>,
	len: usize,
) -> &'a BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	data.span_unchecked(len).to_bitslice_ref()
}

/** Performs the same functionality as [`from_raw_parts_mut`], without checking
the `len` argument.

# Parameters

- `data`: A `BitPtr` to a dereferencable region of memory.
- `len`: The length, in bits, of the region beginning at `*data`. This is not
  checked against the maximum value, and is encoded directly into the bit-slice
  reference. If it exceeds [`BitSlice::MAX_BITS`], it will be modulated to fit
  (the high bits will be discarded).

# Returns

A `&mut BitSlice` reference starting at `data` and running for `len & MAX_BITS`
bits.

# Safety

See [`from_raw_parts_mut`].

[`BitSlice::MAX_BITS`]: crate::slice::BitSlice::MAX_BITS
[`from_raw_parts_mut`]: crate::slice::from_raw_parts_mut
**/
pub unsafe fn from_raw_parts_unchecked_mut<'a, O, T>(
	data: BitPtr<Mut, O, T>,
	len: usize,
) -> &'a mut BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	data.span_unchecked(len).to_bitslice_mut()
}

mod api;
mod iter;
mod ops;
mod specialization;
mod traits;

//  Match the `core::slice` module topology.

pub use self::{
	api::{
		from_mut,
		from_raw_parts,
		from_raw_parts_mut,
		from_ref,
		BitSliceIndex,
	},
	iter::{
		Chunks,
		ChunksExact,
		ChunksExactMut,
		ChunksMut,
		Iter,
		IterMut,
		IterOnes,
		IterZeros,
		RChunks,
		RChunksExact,
		RChunksExactMut,
		RChunksMut,
		RSplit,
		RSplitMut,
		RSplitN,
		RSplitNMut,
		Split,
		SplitMut,
		SplitN,
		SplitNMut,
		Windows,
	},
};

#[cfg(test)]
mod tests;
