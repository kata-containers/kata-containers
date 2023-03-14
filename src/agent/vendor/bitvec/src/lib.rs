/*! # `bitvec` — Addressable Bits

`bitvec` provides the foundation tools needed to implement truly single-bit
`bool` collections and arbitrary bit-precision addressing. It builds compact
collections and performant [bitfield] regions with a high-level, expressive, API
that compiles down to the simple machine instructions you would expect.

# Examples

The [`examples/`] directory of the project repository contains some programs
that showcase different `bitvec` features and use cases. In addition, each data
structure’s API documentation contains more focused samples.

To begin using `bitvec`, you need only import its [prelude]. Once in scope,
`bitvec` can take over existing memory buffers or create entirely new values:

```rust
use bitvec::prelude::*;

let data = &[0u8, 1, 2, 3];
let data_bits = data.view_bits::<Msb0>();

let literal_bits = bits![Lsb0, u16; 1, 0, 1, 1];
assert_eq!(literal_bits.as_slice()[0], 0b1101);

let array_bool = bitarr![1; 40];
# #[cfg(feature = "alloc")] {
let boxed_bool = bitbox![Lsb0, u32; 1; 50];
let vec_bool = bitvec![Msb0, usize; 1; 60];
# }
```

The two easiest entry points into `bitvec` are through the [`BitView`] trait,
which provides extension methods on ordinary memory to view it as a
[`BitSlice`], and the [macro] constructors, which convert token strings into
appropriate buffers at compile time. Each data structure also has its own
constructor functions that create new buffers or borrow existing values.

Once in use, `bitvec`’s types obey all the same patterns and APIs that you have
come to expect from their analogues in the [`core`], [`alloc`], and [`std`]
libraries.

# Usage

`bitvec` provides data structures that specialize the major sequence types in
the standard libraries:

- `[bool]` becomes [`BitSlice`]
- `[bool; N]` becomes [`BitArray`]
- `Box<[bool]>` becomes [`BitBox`]
- `Vec<bool>` becomes [`BitVec`]

You can start using the crate in an existing codebase by replacing types and
chasing compiler errors from there.

As an example,

```rust
# #[cfg(feature = "alloc")] {
let mut io_buf: Vec<u8> = Vec::new();
io_buf.extend(&[0x47, 0xA5]);

let stats: Vec<bool> = vec![
  true, false, true, true,
  false, false, true, false,
];
# }
```

would become

```rust
# #[cfg(feature = "alloc")] {
use bitvec::prelude::*;

let mut io_buf: BitVec<Msb0, u8> = BitVec::new();
io_buf.resize(16, false);
io_buf[.. 4].store(4u8);
io_buf[4 .. 8].store(7u8);
io_buf[8 .. 16].store(0xA5u8);

let stats: BitVec = bitvec![
  1, 0, 1, 1,
  0, 0, 1, 0,
];
# }
```

## Type Arguments

The `bitvec` data structures are all generic over two type parameters which
control how they view and manage the memory they use. These type parameters
allow users to precisely control the memory layout, value bit-patterns, and
generated instructions, but most users of the library will not need to be
generic over them. Instead, you probably either do not care about the details of
the underlying memory, or you have a specific and fixed layout requirement. In
either case, you will likely select a specific combination of type arguments and
use it consistently throughout your project.

You *can* write your project to be generic over these type arguments, and this
is certainly useful when writing code that is not coupled directly to memory,
increases complexity with little practical gain.

The default type arguments are chosen for optimal behavior in memory use and
instruction selection. The unadorned types [`BitArray`], [`BitSlice`],
[`BitBox`], and [`BitVec`] can all be used in type-annotation position (`let`
bindings, `struct` fields, and function arguments). Users who need to specify
their type arguments should prefer to do so in a `type` alias, and use that
alias throughout their project instead of the much longer fully-qualified
`bitvec` type names:

```rust
use bitvec::prelude::*;

pub type MySlice = BitSlice<Msb0, u8>;
pub type MyArray20 = bitarr![for 20, in Msb0, u8];
# #[cfg(feature = "alloc")]
pub type MyVec = BitVec<Msb0, u8>;

fn make_buffer() -> MyVec {
  MyVec::new()
}
```

In general, you will probably work with [`BitSlice`] borrows and [`BitVec`]
owned buffers. The [`BitArray`] and [`BitBox`] types are provided for
completeness and have their uses, but the additional constraints and frozen size
render them less commonly useful.

## Additional Details

As a replacement for `bool` sequences, you should be able to replace old type
definition and value construction sites with their corresponding items from this
project, and the rest of your project should just work with the new types.

To use `bitvec` for structural [bitfields] or specialized I/O protocol buffers,
you should use [`BitArray`] or [`BitVec`] to manage your data buffers (for
compile-time statically-sized and run-time dynamically-sized, respectively), and
the [`BitField`] trait to manage transferring values into and out of them.

The [`BitSlice`] type contains most of the behavior that interacts with the
*contents* of a memory buffer. [`BitVec`] adds behavior that operates on
allocations, and specializes [`BitSlice`] behaviors that can take advantage of
owned buffers.

The [`domain`] module, whose types are accessed by the `.{bit_,}domain{,_mut}`
methods on [`BitSlice`], allows users to split their views of memory at aliasing
boundaries. This removes synchronization guards where `bitvec` can prove that
doing so is legal and correct.

There are many ways to construct a bit-level view of data. The [`BitArray`],
[`BitBox`], and [`BitVec`] types all own a buffer of memory and dereference it
to [`BitSlice`] in order to view it. In addition, you can borrow any piece of
ordinary Rust memory as a `BitSlice` view by using its borrowing constructor
functions or the [`BitView`] trait’s extension methods.

# Capabilities

`bitvec` stands out from other bit-sequence libraries, both in Rust and in other
languages, in a few significant ways.

Unlike other Rust libraries, `bitvec` stores its region information in
specially-encoded pointers *to* memory regions, rather than in the region
itself. By using its own pointer encoding scheme, `bitvec` can use references
(`&BitSlice<_, _>` and `&mut BitSlice<_, _>`) to manage memory accesses and fit
seamlessly into the Rust language rules and API signatures.

Unlike *any* other bit-sequence system, `bitvec` enables users to specify both
the register element type used to store data and also the ordering of bits
within each register element. This sidesteps the problems found in C
[bitfields], C++ [`std::bitset`] and [`std::vector<bool>`], Python’s
[`bitstring`], Erlang’s [`bitstream`], and other Rust libraries such as
[`bit-vec`].

By permitting the in-memory layout to be specified by the user, rather than
hard-coding it within the library, `bitvec` enables users to select the behavior
characteristics they want or need without significant effort on their part.

This works by supplying two type parameters: an `O` [`BitOrder`] ordering of
bits within a register element, and a `T` [`BitStore`] register element used for
storage and memory description. `T` is restricted to be only the raw unsigned
integers, and [`bitvec`-provided wrappers][`BitSafe`] over [atomic] and [`Cell`]
synchronization guards, that fit within processor registers on your target.

These parameters permit the `bitvec` type system to track memory access rules
and bit addressing, thus enabling a nearly seamless use of [`BitSlice`]s as if
they were ordinary Rust slices.

`bitvec` correctly handles memory aliasing by leveraging the type system to mark
regions that have become subject to shared mutability. This mark can, depending
on your build settings, either forbid moving such slices across threads, or
issue lock instructions to the memory bus when accessing memory. You will never
need to add your own guards to prevent race conditions, and [`BitSlice`]
provides interfaces to separate any bit-slice into its aliased and unaliased
subslices.

Where possible, `bitvec` uses its knowledge of bit ordering and memory
availability to accelerate memory operations from individual bit-by-bit walks to
batched operations within a register. This is an area of ongoing development,
and is an implementation detail rather than an aspect of public API.

`bitvec`’s performance even when working with individual bits is as close to
ideal as a general-purpose library can be, but the width of processor registers
means that no amount of performance improvement at the individual bit level can
compete with instructions operating on 32 or 64 bits at once. If you encounter
performance bottlenecks, you can escape `bitvec`’s views to operate on the
memory directly, or submit an issue for future work on specialized batch
parallelization.

# Project Structure

You should generally import the library [prelude], with

```rust
use bitvec::prelude::*;
```

The prelude contains the basic symbols you will need to make use of the crate:
the names of data structures, ordering parameters, useful traits, and
constructor macros. Almost all symbols begin with the prefix `Bit`; only the
orderings [`Lsb0`], [`Msb0`], and [`LocalBits`] do not. This will reduce the
likelihood of name collisions.

Each major component in the library is divided into its own module. This
includes each data structure and trait, as well as utility objects used for
implementation. The data structures that mirror the language distribution have
submodules for each part of their mirroring: `api` ports inherent methods,
`iter` contains iteration logic, `ops` overrides operator sigils, and `traits`
holds all other trait implementations. The data structure’s own module typically
only contains its own definition and its inherent methods that are not ports of
the standard libraries.

[atomic]: core::sync::atomic
[bitfield]: https://en.cppreference.com/w/c/language/bit_field
[bitfields]: https://en.cppreference.com/w/c/language/bit_field
[macro]: #macros
[prelude]: crate::prelude

[`BitArray`]: crate::array::BitArray
[`BitBox`]: crate::boxed::BitBox
[`BitField`]: crate::field::BitField
[`BitOrder`]: crate::order::BitOrder
[`BitSafe`]: crate::access::BitSafe
[`BitSlice`]: crate::slice::BitSlice
[`BitStore`]: crate::store::BitStore
[`BitVec`]: crate::vec::BitVec
[`BitView`]: crate::view::BitView
[`Cell`]: core::cell::Cell
[`LocalBits`]: crate::order::LocalBits
[`Lsb0`]: crate::order::Lsb0
[`Msb0`]: crate::order::Msb0

[`alloc`]: alloc
[`bitstream`]: https://erlang.org/doc/programming_examples/bit_syntax.html
[`bitstring`]: https://pypi.org/project/bitstring/
[`bit-vec`]: https://crates.io/crates/bit-vec
[`core`]: core
[`domain`]: crate::domain
[`examples/`]: https://github.com/myrrlyn/bitvec/tree/HEAD/examples
[`std`]: std
[`std::bitset`]: https://en.cppreference.com/w/cpp/utility/bitset
[`std::vector<bool>`]: https://en.cppreference.com/w/cpp/container/vector_bool
!*/

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(debug_assertions, warn(missing_docs))]
#![cfg_attr(not(debug_assertions), deny(missing_docs))]
#![deny(unconditional_recursion)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[macro_use]
pub mod macros;

pub mod access;
pub mod array;
mod devel;
pub mod domain;
pub mod field;
pub mod index;
pub mod mem;
mod mutability;
pub mod order;
pub mod prelude;
pub mod ptr;
pub mod slice;
pub mod store;
pub mod view;

#[cfg(feature = "alloc")]
pub mod boxed;

#[cfg(feature = "alloc")]
pub mod vec;

#[cfg(feature = "serde")]
mod serdes;
