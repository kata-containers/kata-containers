/*! Ordering of bits within register elements.

[`bitvec`] data structures are parametric over any ordering of bits within a
register type. The [`BitOrder`] trait translates a cursor position (indicated by
the [`BitIdx`] type) to an electrical position (indicated by the [`BitPos`]
type) within that register, thereby defining the order of traversal over a
register.

Implementors of [`BitOrder`] are required to satisfy a set of requirements on
their transform function, and must have identical behavior to the
default-provided trait functions if they choose to override them for
performance. These can all be proven by use of the [`verify`] or
[`verify_for_type`] functions in the implementor’s test suite.

[`BitOrder`] is a stateless trait, and implementors should be zero-sized types.

[`BitIdx`]: crate::index::BitIdx
[`BitOrder`]: crate::order::BitOrder
[`BitPos`]: crate::index::BitPos
[`bitvec`]: crate
[`verify`]: crate::order::verify
[`verify_for_type`]: crate::order::verify_for_type
!*/

use crate::{
	index::{
		BitIdx,
		BitMask,
		BitPos,
		BitSel,
		BitTail,
	},
	mem::BitRegister,
};

/** An ordering over a register.

# Usage

[`bitvec`] structures store and operate on semantic index counters, not
electrical bit positions. The [`BitOrder::at`] function takes a semantic
ordering, [`BitIdx`], and produces a corresponding electrical position,
[`BitPos`].

# Safety

If your implementation violates any of the requirements on these functions, then
the program will become incorrect, and have unspecified behavior. The best-case
outcome is that operations relying on your implementation will crash the
program; the worst-case is that memory access will silently become corrupt.

You are responsible for adhering to the requirements of these functions. There
are verification functions that you can use in your test suite; however, it is
not yet possible to prove correctness at compile-time.

This is an `unsafe trait` to implement because you are responsible for upholding
the stated requirements.

The implementations of `BitOrder` are trusted to drive safe code, and once data
leaves a `BitOrder` implementation, it is considered safe to use as the basis
for interaction with memory.

# Verification

The [`verify`] and [`verify_for_type`] functions are available for your test
suites. They ensure that a `BitOrder` implementation satisfies the requirements
when invoked for a given register type.

# Examples

Implementations are not required to remain contiguous over a register. This
example swizzles the high and low halves of each byte, but any translation is
valid as long as it satisfies the strict one-to-one requirement of
index-to-position.
**/
///
/// ```rust
/// use bitvec::{
///   prelude::BitOrder,
///   // Additional symbols:
///   index::{BitIdx, BitPos},
///   mem::BitRegister,
/// };
///
/// pub struct HiLo;
/// unsafe impl BitOrder for HiLo {
///   fn at<R: BitRegister>(idx: BitIdx<R>) -> BitPos<R> {
///     BitPos::new(idx.value() ^ 4).unwrap()
///   }
/// }
///
/// #[test]
/// #[cfg(test)]
/// fn prove_hilo() {
///   bitvec::order::verify::<HiLo>();
/// }
/// ```
///
/// [`BitIdx`]: crate::index::BitIdx
/// [`BitOrder::at`]: Self::at
/// [`BitPos`]: crate::index::BitPos
/// [`bitvec`]: crate
/// [`verify`]: crate::order::verify
/// [`verify_for_type`]: crate::order::verify_for_type
pub unsafe trait BitOrder: 'static {
	/// Converts a semantic bit index into an electrical bit position.
	///
	/// This function is the basis of the trait, and must adhere to a number of
	/// requirements in order for an implementation to be correct.
	///
	/// # Type Parameters
	///
	/// - `R`: The register type that the index and position govern.
	///
	/// # Parameters
	///
	/// - `index`: The semantic index of a bit within a register `R`.
	///
	/// # Returns
	///
	/// The electrical position of the indexed bit within the register `R`. See
	/// the [`BitPos`] documentation for what electrical positions are
	/// considered to mean.
	///
	/// # Requirements
	///
	/// This function must satisfy the following requirements for all possible
	/// input and output values, for all possible `R` type parameters:
	///
	/// ## Totality
	///
	/// This function must be able to accept every input in the range
	/// [`BitIdx::ZERO`] to [`BitIdx::LAST`], and produce a value in the same
	/// range as a [`BitPos`].
	///
	/// ## Bijection
	///
	/// There must be an exactly one-to-one correspondence between input value
	/// and output value. No input index may choose its output from a set of
	/// more than one position, and no output position may be produced by more
	/// than one input index.
	///
	/// ## Purity
	///
	/// The translation from index to position must be consistent for the
	/// lifetime of *at least* all data structures in the program. This function
	/// *may* refer to global state, but that state **must** be immutable while
	/// any [`bitvec`] data structures exist, and must not be used to violate
	/// the totality or bijection requirements.
	///
	/// ## Output Validity
	///
	/// The produced [`BitPos`] must be within the valid range of that type.
	/// Call sites of this function will not take any steps to constrain or
	/// check the return value. If you use `unsafe` code to produce an invalid
	/// `BitPos`, the program is incorrect, and will likely crash.
	///
	/// # Usage
	///
	/// This function is only ever called with input values in the valid
	/// [`BitIdx`] range. Implementors are not required to consider any values
	/// outside this range in their function body.
	///
	/// [`BitIdx`]: crate::index::BitIdx
	/// [`BitIdx::LAST`]: crate::index::BitIdx::LAST
	/// [`BitIdx::ZERO`]: crate::index::BitIdx::ZERO
	/// [`BitPos`]: crate::index::BitPos
	/// [`bitvec`]: crate
	fn at<R>(index: BitIdx<R>) -> BitPos<R>
	where R: BitRegister;

	/// Converts a semantic bit index into a one-hot selector mask.
	///
	/// This is an optional function; a default implementation is provided for
	/// you. If you choose to override it, your implementation **must** retain
	/// the behavior of the default implementation.
	///
	/// The default implementation calls [`Self::at`] to convert the index into
	/// a position, then turns that position into a selector mask with the
	/// expression `1 << pos`. `BitOrder` implementations may choose to provide
	/// a faster mask production here, as long as they match this behavior.
	///
	/// # Type Parameters
	///
	/// - `R`: The register type that the index and selector govern.
	///
	/// # Parameters
	///
	/// - `index`: The semantic index of a bit within a register `R`.
	///
	/// # Returns
	///
	/// A one-hot selector mask for the bit indicated by the index value.
	///
	/// # Requirements
	///
	/// A one-hot encoding means that there is exactly one bit set in the
	/// produced value. It must be equivalent to `1 << Self::at::<R>(index)`.
	///
	/// As with `at`, this function must produce a unique mapping from each
	/// legal index in the [`BitIdx`] domain to a one-hot value of `R`.
	///
	/// [`BitIdx`]: crate::index::BitIdx
	/// [`Self::at`]: Self::at
	#[cfg(not(tarpaulin_include))]
	fn select<R>(index: BitIdx<R>) -> BitSel<R>
	where R: BitRegister {
		Self::at::<R>(index).select()
	}

	/// Constructs a multiple-bit selector mask for batched operations on a
	/// register `R`.
	///
	/// The default implementation of this function traverses the index range,
	/// converting each index into a single-bit selector with [`Self::select`]
	/// and accumulating into a combined register value.
	///
	/// # Type Parameters
	///
	/// - `R`: The register type for which the mask is built.
	///
	/// # Parameters
	///
	/// - `from`: The inclusive starting index for the mask.
	/// - `upto`: The exclusive ending index for the mask.
	///
	/// # Returns
	///
	/// A bit-mask with all bits corresponding to the input index range set high
	/// and all others set low.
	///
	/// # Requirements
	///
	/// This function must always be equivalent to this expression:
	///
	/// ```rust,ignore
	/// (from .. upto)
	///   .map(Self::select::<R>)
	///   .fold(0, |mask, sel| mask | sel)
	/// ```
	///
	/// [`Self::select`]: Self::select
	fn mask<R>(
		from: impl Into<Option<BitIdx<R>>>,
		upto: impl Into<Option<BitTail<R>>>,
	) -> BitMask<R>
	where
		R: BitRegister,
	{
		let (from, upto) = match (from.into(), upto.into()) {
			(None, None) => return BitMask::ALL,
			(Some(from), None) => (from, BitTail::LAST),
			(None, Some(upto)) => (BitIdx::ZERO, upto),
			(Some(from), Some(upto)) => (from, upto),
		};
		from.range(upto).map(Self::select::<R>).sum()
	}
}

/// Traverses a register from the least significant bit to the most significant.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Lsb0;

unsafe impl BitOrder for Lsb0 {
	fn at<R>(index: BitIdx<R>) -> BitPos<R>
	where R: BitRegister {
		unsafe { BitPos::new_unchecked(index.value()) }
	}

	fn select<R>(index: BitIdx<R>) -> BitSel<R>
	where R: BitRegister {
		unsafe { BitSel::new_unchecked(R::ONE << index.value()) }
	}

	fn mask<R>(
		from: impl Into<Option<BitIdx<R>>>,
		upto: impl Into<Option<BitTail<R>>>,
	) -> BitMask<R>
	where
		R: BitRegister,
	{
		let from = from.into().unwrap_or(BitIdx::ZERO).value();
		let upto = upto.into().unwrap_or(BitTail::LAST).value();
		debug_assert!(
			from <= upto,
			"Ranges must run from low index ({}) to high ({})",
			from,
			upto
		);
		let ct = upto - from;
		if ct == R::BITS {
			return BitMask::ALL;
		}
		//  1. Set all bits in the mask high
		//  2. Shift left by the number of bits in the mask. The mask bits are
		//     at LSedge and low.
		//  3. Invert. The mask bits are at LSedge and high; all else are low.
		//  4. Shift left by the `from` distance from LSedge.
		BitMask::new(!(R::ALL << ct) << from)
	}
}

/// Traverses a register from the most significant bit to the least significant.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Msb0;

unsafe impl BitOrder for Msb0 {
	fn at<R>(index: BitIdx<R>) -> BitPos<R>
	where R: BitRegister {
		unsafe { BitPos::new_unchecked(R::MASK - index.value()) }
	}

	fn select<R>(index: BitIdx<R>) -> BitSel<R>
	where R: BitRegister {
		/* Shift the MSbit down by the index count. This is not equivalent to
		the expression `1 << (mask - index)`, because that lowers to a
		subtraction followed by a rshift, while this lowers to a single rshift.
		*/
		let msbit: R = R::ONE << R::MASK;
		unsafe { BitSel::new_unchecked(msbit >> index.value()) }
	}

	fn mask<R>(
		from: impl Into<Option<BitIdx<R>>>,
		upto: impl Into<Option<BitTail<R>>>,
	) -> BitMask<R>
	where
		R: BitRegister,
	{
		let from = from.into().unwrap_or(BitIdx::ZERO).value();
		let upto = upto.into().unwrap_or(BitTail::LAST).value();
		debug_assert!(
			from <= upto,
			"Ranges must run from low index ({}) to high ({})",
			from,
			upto
		);
		let ct = upto - from;
		if ct == R::BITS {
			return BitMask::ALL;
		}
		//  1. Set all bits in the mask high.
		//  2. Shift right by the number of bits in the mask. The mask bits are
		// at MSedge and low.  3. Invert. The mask bits are at MSedge and high;
		// all else are low.  4. Shift right by the `from` distance from MSedge.
		BitMask::new(!(R::ALL >> ct) >> from)
	}
}

/** A default bit ordering.

Typically, your platform’s C compiler uses least-significant-bit-first ordering
for bitfields. The [`Lsb0`] bit ordering and little-endian byte ordering are
otherwise completely unrelated.

[`Lsb0`]: crate::order::Lsb0
**/
#[cfg(target_endian = "little")]
pub use self::Lsb0 as LocalBits;

/** A default bit ordering.

Typically, your platform’s C compiler uses most-significant-bit-first ordering
for bitfields. The [`Msb0`] bit ordering and big-endian byte ordering are
otherwise completely unrelated.

[`Msb0`]: crate::order::Msb0
**/
#[cfg(target_endian = "big")]
pub use self::Msb0 as LocalBits;

#[cfg(not(any(target_endian = "big", target_endian = "little")))]
compile_fail!(concat!(
	"This architecture is currently not supported. File an issue at ",
	env!(CARGO_PKG_REPOSITORY)
));

/** Verifies a [`BitOrder`] implementation’s adherence to the stated rules.

This function checks some [`BitOrder`] implementation’s behavior on each of the
[`BitRegister`] types it must handle, and reports any violation of the rules
that it detects.

# Type Parameters

- `O`: The [`BitOrder`] implementation to test.

# Parameters

- `verbose`: Sets whether the test should print diagnostic information to
  `stdout`.

# Panics

This panics if it detects any violation of the [`BitOrder`] implementation rules
for `O`.

[`BitOrder`]: crate::order::BitOrder
[`BitRegister`]: crate::mem::BitRegister
**/
pub fn verify<O>(verbose: bool)
where O: BitOrder {
	verify_for_type::<O, u8>(verbose);
	verify_for_type::<O, u16>(verbose);
	verify_for_type::<O, u32>(verbose);
	verify_for_type::<O, usize>(verbose);

	#[cfg(target_pointer_width = "64")]
	verify_for_type::<O, u64>(verbose);
}

/** Verifies a [`BitOrder`] implementation’s adherence to the stated rules, for
one register type.

This function checks some [`BitOrder`] implementation against only one of the
[`BitRegister`] types that it will encounter. This is useful if you are
implementing an ordering that only needs to be concerned with a subset of the
types, and you know that you will never use it with the types it does not
support.

# Type Parameters

- `O`: The [`BitOrder`] implementation to test.
- `R`: The [`BitRegister`] type for which to test `O`.

# Parameters

- `verbose`: Sets whether the test should print diagnostic information to
  `stdout`.

# Panics

This panics if it detects any violation of the [`BitOrder`] implementation rules
for the combination of input types and index values.

[`BitOrder`]: crate::order::BitOrder
[`BitRegister`]: crate::mem::BitRegister
**/
pub fn verify_for_type<O, R>(verbose: bool)
where
	O: BitOrder,
	R: BitRegister,
{
	use core::any::type_name;
	let mut accum = BitMask::<R>::ZERO;

	let oname = type_name::<O>();
	let mname = type_name::<R>();

	for n in 0 .. R::BITS {
		//  Wrap the counter as an index.
		let idx = unsafe { BitIdx::<R>::new_unchecked(n) };

		//  Compute the bit position for the index.
		let pos = O::at::<R>(idx);
		if verbose {
			#[cfg(feature = "std")]
			println!(
				"`<{} as BitOrder>::at::<{}>({})` produces {}",
				oname,
				mname,
				n,
				pos.value(),
			);
		}

		//  If the computed position exceeds the valid range, fail.
		assert!(
			pos.value() < R::BITS,
			"Error when verifying the implementation of `BitOrder` for `{}`: \
			 Index {} produces a bit position ({}) that exceeds the type width \
			 {}",
			oname,
			n,
			pos.value(),
			R::BITS,
		);

		//  Check `O`’s implementation of `select`
		let sel = O::select::<R>(idx);
		if verbose {
			#[cfg(feature = "std")]
			println!(
				"`<{} as BitOrder>::select::<{}>({})` produces {:b}",
				oname, mname, n, sel,
			);
		}

		//  If the selector bit is not one-hot, fail.
		assert_eq!(
			sel.value().count_ones(),
			1,
			"Error when verifying the implementation of `BitOrder` for `{}`: \
			 Index {} produces a bit selector ({:b}) that is not a one-hot mask",
			oname,
			n,
			sel,
		);

		//  Check that the selection computed from the index matches the
		//  selection computed from the position.
		let shl = pos.select();
		//  If `O::select(idx)` does not produce `1 << pos`, fail.
		assert_eq!(
			sel,
			shl,
			"Error when verifying the implementation of `BitOrder` for `{}`: \
			 Index {} produces a bit selector ({:b}) that is not equal to `1 \
			 << {}` ({:b})",
			oname,
			n,
			sel,
			pos.value(),
			shl,
		);

		//  Check that the produced selector bit has not already been added to
		//  the accumulator.
		assert!(
			!accum.test(sel),
			"Error when verifying the implementation of `BitOrder` for `{}`: \
			 Index {} produces a bit position ({}) that has already been \
			 produced by a prior index",
			oname,
			n,
			pos.value(),
		);
		accum.insert(sel);
		if verbose {
			#[cfg(feature = "std")]
			println!(
				"`<{} as BitOrder>::at::<{}>({})` accumulates  {:b}",
				oname, mname, n, accum,
			);
		}
	}

	//  Check that all indices produced all positions.
	assert_eq!(
		accum,
		BitMask::ALL,
		"Error when verifying the implementation of `BitOrder` for `{}`: The \
		 bit positions marked with a `0` here were never produced from an \
		 index, despite all possible indices being passed in for translation: \
		 {:b}",
		oname,
		accum,
	);

	//  Check that `O::mask` is correct for all range combinations.
	for from in BitIdx::<R>::range_all() {
		for upto in BitTail::<R>::range_from(from) {
			let mask = O::mask(from, upto);
			let check = from
				.range(upto)
				.map(O::at)
				.map(BitPos::select)
				.sum::<BitMask<R>>();
			assert_eq!(
				mask,
				check,
				"Error when verifying the implementation of `BitOrder` for \
				 `{o}`: `{o}::mask::<{m}>({f}, {u})` produced {bad:b}, but \
				 expected {good:b}",
				o = oname,
				m = mname,
				f = from,
				u = upto,
				bad = mask,
				good = check,
			);
		}
	}
}

#[cfg(all(test, not(miri)))]
mod tests {
	use super::*;

	#[test]
	fn verify_impls() {
		verify::<Lsb0>(cfg!(feature = "testing"));
		verify::<Msb0>(cfg!(feature = "testing"));
	}
}
