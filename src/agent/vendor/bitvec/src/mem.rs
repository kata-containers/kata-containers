/*! Memory element descriptions.

This module describes memory integers and processor registers used to hold and
manipulate [`bitvec`] data buffers.

The [`BitMemory`] trait adds descriptive information to the unsigned integers
available in the language.

The [`BitRegister`] trait marks the unsigned integers that correspond to
processor registers, and can therefore be used for buffer control. The integers
that are `BitMemory` but not `BitRegister` can be composed out of register
values, but are unable to be used in buffer type parameters.

[`BitMemory`]: crate::mem::BitMemory
[`BitRegister`]: crate::mem::BitRegister
[`bitvec`]: crate
!*/

use core::mem;

use funty::IsUnsigned;

use radium::marker::BitOps;

/** Description of an integer memory element.

This trait provides information used to describe integer-typed regions of memory
and enables other parts of the project to adequately describe the memory bus.
This trait has **no** bearing on the processor instructions or registers used to
interact with memory. It solely describes integers that can exist on a system.

This trait cannot be implemented outside this crate.
**/
pub trait BitMemory: IsUnsigned + seal::Sealed {
	/// The bit width of the integer.
	///
	/// [`mem::size_of`] returns the size in bytes, and bytes are always eight
	/// bits wide on architectures that Rust targets.
	///
	/// Issue #76904 will place this constant on the fundamental integers
	/// directly, as a `u32`.
	///
	/// [`mem::size_of`]: core::mem::size_of
	const BITS: u8 = mem::size_of::<Self>() as u8 * 8;

	/// The number of bits required to store an index in the range `0 .. BITS`.
	const INDX: u8 = Self::BITS.trailing_zeros() as u8;

	/// A mask over all bits that can be used as an index within the element.
	/// This is the value with the least significant `INDX`-many bits set high.
	const MASK: u8 = Self::BITS - 1;
}

/** Description of a processor register.

This trait provides information used to describe processor registers. It only
needs to contain constant values for `1` and `!0`; the rest of its information
is contained in the presence or absence of its implementation on particular
integers.
**/
pub trait BitRegister: BitMemory + BitOps {
	/// The literal `1`.
	const ONE: Self;
	/// The literal `!0`.
	const ALL: Self;
}

macro_rules! memory {
	($($t:ident),+ $(,)?) => { $(
		impl BitMemory for $t {}
		impl seal::Sealed for $t {}
	)+ };
}

memory!(u8, u16, u32, u64, u128, usize);

macro_rules! register {
	($($t:ident),+ $(,)?) => { $(
		impl BitRegister for $t {
			const ONE: Self = 1;
			const ALL: Self = !0;
		}
	)+ };
}

register!(u8, u16, u32);

/** `u64` can only be used as a register on processors whose word size is at
least 64 bits.

This implementation is not present on targets with 32-bit processor words.
**/
#[cfg(target_pointer_width = "64")]
impl BitRegister for u64 {
	const ALL: Self = !0;
	const ONE: Self = 1;
}

register!(usize);

/** Computes the number of elements required to store some number of bits.

# Parameters

- `bits`: The number of bits to store in a `[T]` array.

# Returns

The number of elements `T` required to store `bits`.

As this is a const function, when `bits` is a constant expression, this can be
used to compute the size of an array type `[T; elts(bits)]`.
**/
#[doc(hidden)]
pub const fn elts<T>(bits: usize) -> usize {
	let width = mem::size_of::<T>() * 8;
	bits / width + (bits % width != 0) as usize
}

/** Tests that a type is aligned to at least its size.

This property is not necessarily true for all integers; for instance, `u64` on
32-bit x86 is permitted to be 4-byte-aligned. `bitvec` requires this property to
hold for the pointer representation to correctly function.

# Type Parameters

- `T`: A type whose alignment and size are to be compared

# Returns

`0` if the alignment is at least the size; `1` if the alignment is less.
**/
#[doc(hidden)]
pub(crate) const fn aligned_to_size<T>() -> usize {
	(mem::align_of::<T>() < mem::size_of::<T>()) as usize
}

/** Tests whether two types have compatible layouts.

# Type Parameters

- `A`
- `B`

# Returns

Zero if `A` and `B` have equal alignments and sizes, non-zero if they do not.

# Uses

This function is designed to be used in the expression
`const CHECK: [(): 0] = [(); cmp_layout::<A, B>()];`. It will cause a compiler
error if the conditions do not hold.
**/
#[doc(hidden)]
pub(crate) const fn cmp_layout<A, B>() -> usize {
	(mem::align_of::<A>() != mem::align_of::<B>()) as usize
		+ (mem::size_of::<A>() != mem::size_of::<B>()) as usize
}

#[doc(hidden)]
mod seal {
	#[doc(hidden)]
	pub trait Sealed {}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::access::*;

	#[test]
	fn integer_properties() {
		assert_eq!(aligned_to_size::<u8>(), 0);
		assert_eq!(aligned_to_size::<BitSafeU8>(), 0);
		assert_eq!(cmp_layout::<u8, BitSafeU8>(), 0);

		assert_eq!(aligned_to_size::<u16>(), 0);
		assert_eq!(aligned_to_size::<BitSafeU16>(), 0);
		assert_eq!(cmp_layout::<u16, BitSafeU16>(), 0);

		assert_eq!(aligned_to_size::<u32>(), 0);
		assert_eq!(aligned_to_size::<BitSafeU32>(), 0);
		assert_eq!(cmp_layout::<u32, BitSafeU32>(), 0);

		assert_eq!(aligned_to_size::<usize>(), 0);
		assert_eq!(aligned_to_size::<BitSafeUsize>(), 0);
		assert_eq!(cmp_layout::<usize, BitSafeUsize>(), 0);

		#[cfg(target_pointer_width = "64")]
		{
			assert_eq!(aligned_to_size::<u64>(), 0);
			assert_eq!(aligned_to_size::<BitSafeU64>(), 0);
			assert_eq!(cmp_layout::<u64, BitSafeU64>(), 0);
		}
	}
}
